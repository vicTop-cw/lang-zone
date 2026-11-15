//! 运行时优化：把「值语义的重赋值」识别成可原地追加的形态。
//!
//! LZ 是值语义（同一个变量可以被重复读），所以通用路径不得不发
//!
//! ```text
//! s = s.to_string() + &"ab".to_string()[..];                  // str：每轮 3 次分配
//! a = { let mut __cat = a.clone(); __cat.extend(..); __cat };  // list：每轮一次全量 clone
//! ```
//!
//! 代价：累加模式退化成**时间/拷贝量 O(n^2)** + 每轮堆分配抖动。
//! 本模块提供「独占局部变量登记表」，让这两类重赋值退化成原地操作：
//!
//! ```text
//! s.push_str("ab");      // str：零分配
//! a.push(x);             // list：零分配
//! a.extend(other);       // list：省掉每轮的全量 clone
//! ```
//!
//! # 保守边界（为什么不会静默出错）
//!
//! 1. 只在 **by-value 的局部绑定**上生效：登记发生在 `Stmt::Let` 的声明路径，
//!    使用发生在「该变量已 declared」的赋值路径。
//! 2. 只登记 `is_ref == false` 的绑定；`ref r = x` / `ref mut r = x` 走独立发射
//!    路径，且重新赋值时被解引用成 `*r = ..`，两种都被排除。
//! 3. **形参一律不登记**（保守：by-value 形参也走通用路径，牺牲优化换零风险）。
//! 4. 函数入口清空（`gen_fn_def`），不会跨函数泄漏同名变量的所有权信息。
//! 5. RHS 必须自身形态匹配（str + str / list + list）；`s + 1`、`s + 'c'` 等
//!    一律不走本路径，保持通用加法语义。
//! 6. **借用检查器是最终保证**：任何能通过原路径编译的场景，目标变量必然是
//!    可独占的 `String`/`Vec`，所以本改写不引入新的可用性要求，最坏情况是
//!    编译期报错而不会静默改变语义。

use std::cell::RefCell;
use std::collections::HashMap;

use super::{BinOpKind, Expr, ExprKind, IrType, LitKind};
use super::CodeGen;

/// 登记的独占局部变量种类。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LocalKind {
    /// `String`：可 `push_str`。
    Str,
    /// `Vec<T>`：可 `push` / `extend`。
    List,
    /// `BTreeMap<K, V>`：可 `insert` / `extend`。
    Dict,
    /// `HashSet<T>`：可 `insert` / `extend`。
    Set,
    /// 只读借用形参（`ref x: T` → Rust `&T`）：不可赋值，只用于诊断。
    ReadonlyBorrow,
}

/// 容器追加的落地形态（List / Dict / Set 共用）。
pub(crate) enum ContainerAppend<'e> {
    /// 单元素 → `push(e)`（列表，零分配，优于 `extend(vec![e])`）。
    Push(&'e Expr),
    /// 单元素 → `insert(e)`（Set，零分配）。
    InsertOne(&'e Expr),
    /// 多元素列表/集合字面量 → `extend([e, ..])`。
    ExtendItems(Vec<String>),
    /// 字典字面量的条目：单条目 → `insert(k, v)`（零分配），
    /// 多条目 → `extend([(k, v), ..])`。
    ExtendPairs(Vec<(String, String)>),
    /// 其他表达式 → `extend(<expr>)`（与通用路径的实参文本一致，move 语义不变）。
    ExtendExpr(&'e Expr),
}

impl CodeGen {
    /// 函数入口清空登记表（避免跨函数泄漏同名变量）。
    pub(crate) fn owned_local_reset(&mut self) {
        self.owned_locals.borrow_mut().clear();
    }

    /// 登记 by-value 的 str 局部绑定。
    pub(crate) fn owned_local_mark_str(&self, name: &str) {
        self.owned_locals
            .borrow_mut()
            .insert(name.to_string(), LocalKind::Str);
    }

    /// 登记只读借用形参（`ref x: T`，非 mut）：不可赋值，仅用于诊断。
    pub(crate) fn owned_local_mark_readonly(&self, name: &str) {
        self.owned_locals
            .borrow_mut()
            .insert(name.to_string(), LocalKind::ReadonlyBorrow);
    }

    /// 登记 by-value 的列表局部绑定。
    pub(crate) fn owned_local_mark_list(&self, name: &str) {
        self.owned_locals
            .borrow_mut()
            .insert(name.to_string(), LocalKind::List);
    }

    /// 登记 by-value 的字典局部绑定。
    pub(crate) fn owned_local_mark_dict(&self, name: &str) {
        self.owned_locals
            .borrow_mut()
            .insert(name.to_string(), LocalKind::Dict);
    }

    /// 登记 by-value 的集合局部绑定。
    pub(crate) fn owned_local_mark_set(&self, name: &str) {
        self.owned_locals
            .borrow_mut()
            .insert(name.to_string(), LocalKind::Set);
    }

    /// 撤销登记（`ref` 绑定遮蔽同名 by-value 绑定时）。
    pub(crate) fn owned_local_unmark(&self, name: &str) {
        self.owned_locals.borrow_mut().remove(name);
    }

    pub(crate) fn owned_local_kind(&self, name: &str) -> Option<LocalKind> {
        self.owned_locals.borrow().get(name).copied()
    }

    /// `value` 是否形如 `name + <str>`（字符串追加），是则返回右操作数。
    pub(crate) fn str_append_rhs<'e>(&self, name: &str, value: &'e Expr) -> Option<&'e Expr> {
        let (lhs, rhs) = self.add_operands(name, value)?;
        if !is_str_ty(&lhs.ty) {
            return None;
        }
        // RHS 必须自身是 str 形态：非 str 走通用路径（str + int 等需 format!）
        if !is_str_ty(&rhs.ty) && !is_str_producing(rhs) {
            return None;
        }
        Some(rhs)
    }

    /// `value` 是否形如 `name + <容器>`（列表/字典/集合合并），是则给出落地形态。
    ///
    /// 通用路径对四类容器都发Rust 原生 `+`：
    /// - `Vec`：`{ let mut __cat = a.clone(); __cat.extend(..); __cat }`（每轮全量 clone）
    /// - `BTreeMap`/`HashSet`：`d = d + BTreeMap::from([(k, v)])` —— 每轮先**新建一张表**
    ///   再整表合并，字典/集合越大越慢（同样 O(n^2)）。
    /// 本函数把它们统一降级为 `insert` / `push` / `extend`。
    pub(crate) fn container_append_rhs<'e>(
        &self,
        name: &str,
        value: &'e Expr,
    ) -> Option<ContainerAppend<'e>> {
        let (lhs, rhs) = self.add_operands(name, value)?;
        match self.owned_local_kind(name)? {
            LocalKind::List => {
                if !is_list_ty(&lhs.ty) || !is_list_ty(&rhs.ty) {
                    return None;
                }
                match &rhs.kind {
                    // 列表字面量：单元素走 push（零分配），多元素走 extend([..])
                    ExprKind::ListLit(elems) if elems.len() == 1 => {
                        Some(ContainerAppend::Push(&elems[0]))
                    }
                    ExprKind::ListLit(elems) => {
                        let mut parts = Vec::with_capacity(elems.len());
                        for e in elems {
                            parts.push(self.gen_expr(e));
                        }
                        Some(ContainerAppend::ExtendItems(parts))
                    }
                    // 其他：实参文本与通用路径一致，move 语义不变
                    _ => Some(ContainerAppend::ExtendExpr(rhs)),
                }
            }
            LocalKind::Dict => {
                if !is_dict_ty(&lhs.ty) {
                    return None;
                }
                // Dict 字面量：StructCtor { name: "Dict", fields: [("_k0", k), ("_v0", v), ..] }
                if let ExprKind::StructCtor { name: n, fields } = &rhs.kind {
                    if n == "Dict" || n == "BTreeMap" || n == "HashMap" {
                        let mut pairs: Vec<(String, String)> = Vec::new();
                        let mut i = 0;
                        loop {
                            let key = fields.iter().find(|(n, _)| *n == format!("_k{}", i));
                            let val = fields.iter().find(|(n, _)| *n == format!("_v{}", i));
                            match (key, val) {
                                (Some((_, k)), Some((_, v))) => {
                                    pairs.push((self.gen_expr(k), self.gen_expr(v)));
                                }
                                _ => break,
                            }
                            i += 1;
                        }
                        if pairs.is_empty() {
                            // 空 Dict 字面量：无条目可追加（等价于 no-op），交回通用路径
                            return None;
                        }
                        return Some(ContainerAppend::ExtendPairs(pairs));
                    }
                }
                if !is_dict_ty(&rhs.ty) {
                    return None;
                }
                Some(ContainerAppend::ExtendExpr(rhs))
            }
            LocalKind::Set => {
                if !is_set_ty(&lhs.ty) {
                    return None;
                }
                // Set 字面量 `{a, b}` 被脱糖成 `set!(a, b)` 调用
                if let ExprKind::Call { callee, args, .. } = &rhs.kind {
                    if matches!(&callee.kind, ExprKind::Var(v) if v == "set!" || v == "set") {
                        if args.len() == 1 {
                            return Some(ContainerAppend::InsertOne(&args[0]));
                        }
                        if args.len() > 1 {
                            let mut parts = Vec::with_capacity(args.len());
                            for a in args {
                                parts.push(self.gen_expr(a));
                            }
                            return Some(ContainerAppend::ExtendItems(parts));
                        }
                        return None;
                    }
                }
                if !is_set_ty(&rhs.ty) {
                    return None;
                }
                Some(ContainerAppend::ExtendExpr(rhs))
            }
            LocalKind::ReadonlyBorrow | LocalKind::Str => None,
        }
    }

    /// 匹配「`<name> + <rhs>` 且 op 为 +」，返回 (lhs, rhs)。
    fn add_operands<'e>(&self, name: &str, value: &'e Expr) -> Option<(&'e Expr, &'e Expr)> {
        let ExprKind::BinOp { op, lhs, rhs, .. } = &value.kind else {
            return None;
        };
        if *op != BinOpKind::Add {
            return None;
        }
        // 左侧必须是同一个变量的引用（`s + x`），不是 `f() + x` / `a.b + x`
        match &lhs.kind {
            ExprKind::Var(v) if v == name => {}
            _ => return None,
        }
        Some((lhs, rhs))
    }
}

/// IR 类型是否为 str / String。
pub(crate) fn is_str_ty(ty: &IrType) -> bool {
    if matches!(ty, IrType::Str) {
        return true;
    }
    match ty {
        IrType::Named { path, .. } => path == "String" || path == "str",
        _ => false,
    }
}

/// IR 类型是否为列表（Vec）。注意 `IrType` 没有专门的 List 变体，
/// 列表一律是 `Named { path: "List", .. }`（映射到 Rust 的 Vec）。
pub(crate) fn is_list_ty(ty: &IrType) -> bool {
    match ty {
        IrType::Named { path, .. } => {
            let p = path.as_str();
            p == "List" || p == "Vec" || p == "Array"
        }
        _ => false,
    }
}

/// 表达式本身是否产出字符串（字面量 / f-string / 字符串方法链）。
/// IR 类型是否为字典（BTreeMap / HashMap）。
pub(crate) fn is_dict_ty(ty: &IrType) -> bool {
    match ty {
        IrType::Named { path, .. } => {
            let p = path.as_str();
            p == "Dict" || p == "HashMap" || p == "BTreeMap"
        }
        _ => false,
    }
}

/// IR 类型是否为集合（HashSet）。
pub(crate) fn is_set_ty(ty: &IrType) -> bool {
    match ty {
        IrType::Named { path, .. } => {
            let p = path.as_str();
            p == "Set" || p == "HashSet"
        }
        _ => false,
    }
}

fn is_str_producing(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Lit(LitKind::Str(_)) | ExprKind::Lit(LitKind::FStr(_)) => true,
        ExprKind::MethodCall { method, .. } => is_str_method(method),
        _ => false,
    }
}

fn is_str_method(m: &str) -> bool {
    matches!(
        m,
        "trim" | "trim_start" | "trim_end" | "to_upper" | "to_lower" | "capitalize"
            | "replace" | "repeat" | "reverse" | "format" | "join" | "strip" | "chars"
            | "split" | "lines" | "pad_start" | "pad_end" | "upper" | "lower" | "to_string"
    )
}

/// push_str 的实参文本：已是 `&str` 形态的直接用，其余加 `&`。
pub(crate) fn push_str_arg(rhs_text: &str, already_borrowed: bool) -> String {
    if already_borrowed {
        rhs_text.to_string()
    } else {
        format!("&{}", rhs_text)
    }
}

/// CodeGen 字段用的登记集合类型。
pub(crate) type OwnedLocals = RefCell<HashMap<String, LocalKind>>;

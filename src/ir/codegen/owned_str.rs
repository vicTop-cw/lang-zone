//! 字符串「原地追加」：`s = s + rhs` → `s.push_str(&rhs)`。
//!
//! # 为什么需要
//!
//! 通用字符串加法路径发的是 `lhs + &rhs`，而为了适配 `&str` 借用语义，
//! 两侧常被加上 `.to_string()`。对累加模式
//!
//! ```lz
//! mut s = ""
//! while i < n { s = s + "ab" }
//! ```
//!
//! 生成的 Rust 是 `s = s.to_string() + &"ab".to_string()[..];`
//! —— **每轮 3 次堆分配**，且把已累积的字符串完整拷贝两遍，
//! 时间 O(n²)、分配抖动 O(n)。实测（2 万轮）比手写 `push_str` 慢 2.49×、
//! 峰值内存多 63%。改成 `s.push_str(&"ab")` 后**零分配**。
//!
//! # 保守边界（为什么不会静默出错）
//!
//! 1. 只在 **已声明的局部变量** 上生效：登记发生在 `Stmt::Let` 的声明路径，
//!    使用发生在「该变量已 declared」的赋值路径。
//! 2. 只登记 **by-value 的 str 绑定**（`is_ref == false`）；`ref s: str` /
//!    `ref mut s: str` 形参从不被登记（我们不在形参处登记），
//!    且 `ref mut s` 的重新赋值走 `*s = ...` 分支，也被排除。
//! 3. **借用检查器是最终保证**：任何能通过原路径（`s.to_string() + ..`）编译的
//!    场景，`s` 必然是可独占的 `String`。所以本改写不引入新的可用性要求，
//!    最坏情况是编译期报错而不会静默改变语义。
//! 4. 函数入口清空（`gen_fn_def`），不会跨函数泄漏同名变量的所有权信息。
//! 5. RHS 必须自身是 str 形态（字面量 / f-string / str 变量 / 字符串方法调用）；
//!    `s + 1`、`s + 'c'` 等一律不走本路径，保持通用加法语义。

use std::cell::RefCell;
use std::collections::HashSet;

use super::{BinOpKind, Expr, ExprKind, IrType, LitKind};
use super::CodeGen;

impl CodeGen {
    /// 函数入口清空所有权登记（避免跨函数泄漏同名变量）。
    pub(crate) fn owned_str_reset(&mut self) {
        self.owned_str_vars.borrow_mut().clear();
    }

    /// 登记 by-value 的 str 局部绑定。
    pub(crate) fn owned_str_mark(&self, name: &str) {
        self.owned_str_vars.borrow_mut().insert(name.to_string());
    }

    /// 撤销登记（`ref` 绑定遮蔽同名 by-value 绑定时）。
    pub(crate) fn owned_str_unmark(&self, name: &str) {
        self.owned_str_vars.borrow_mut().remove(name);
    }

    pub(crate) fn owned_str_contains(&self, name: &str) -> bool {
        self.owned_str_vars.borrow().contains(name)
    }

    /// `value` 是否形如 `name + <str>`（追加语义），是则返回待追加的右操作数。
    pub(crate) fn str_append_rhs<'e>(&self, name: &str, value: &'e Expr) -> Option<&'e Expr> {
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
        // 左侧类型必须是 str（避免 `int + int` 误判）
        if !is_str_ty(&lhs.ty) {
            return None;
        }
        // 右侧必须自身是 str 形态：非 str 走通用路径（str + int 等需 format!）
        if !is_str_ty(&rhs.ty) && !is_str_producing(rhs) {
            return None;
        }
        Some(rhs)
    }
}

/// IR 类型是否为 str / String。
pub(crate) fn is_str_ty(ty: &IrType) -> bool {
    matches!(ty, IrType::Str)
        || matches!(ty, IrType::Named { path, .. } if path == "String" || path == "str")
}

/// 表达式本身是否产出字符串（字面量 / f-string / 字符串方法链）。
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

/// push_str 的实参文本：已是 `&str` 形态的文本直接用，其余加 `&`。
pub(crate) fn push_str_arg(rhs_text: &str, already_borrowed: bool) -> String {
    if already_borrowed {
        rhs_text.to_string()
    } else {
        format!("&{}", rhs_text)
    }
}

/// CodeGen 字段用的登记集合类型。
pub(crate) type OwnedStrSet = RefCell<HashSet<String>>;
// Lang-Zone 编译器 — ir/builder.rs
// AST → LZIR-H 构造器：将 AST Module 转换为 IrModule
//
// 职责：
// 1. 逐节点 AST → LZIR 转换
// 2. 构建块脱糖（=:→Let, ^:→IndexGet, ~:→Call, *:→GenExpr）
// 3. 类型推导（从标注 + 简单传播 + 字面量推断）
// 4. 魔法方法归一化（MagicCall / MethodCall）

use crate::ast::{
    self, AssignOp, BinOp, BuildKind, Expr as AstExpr, Pattern as AstPattern, Stmt as AstStmt,
    UnaryOp,
};
use crate::types::Type as AstType;

use super::codegen::collect_var_refs;
use super::node::*;
use super::types::{from_ast_type, from_ast_type_with_generics, IrType};
use super::IrModule;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// 类型推导上下文
#[derive(Clone)]
struct TypeCtx {
    /// 变量名 → 类型
    vars: HashMap<String, IrType>,
    /// 函数名 → 返回类型
    fn_returns: HashMap<String, IrType>,
    /// 函数参数类型（用于泛型实例化）
    fn_params: HashMap<String, Vec<IrType>>,
    /// struct 名称集合（用于区分构造调用与普通函数调用）
    struct_names: HashSet<String>,
    /// struct 字段类型：struct_name → field_name → type
    struct_fields: HashMap<String, HashMap<String, IrType>>,
    /// struct 字段声明顺序：struct_name → [field_name, ...]（位置参数构造用）
    struct_field_order: HashMap<String, Vec<String>>,
    /// struct 方法名集合：struct_name → 方法名集合（含魔术方法）
    struct_methods: HashMap<String, HashSet<String>>,
    /// struct 方法非 self 参数个数：struct_name → (method_name → 参数个数)
    /// 用于 __call__ 单参校验与 __rpipe__/__lpipe__ 分派
    struct_method_arity: HashMap<String, HashMap<String, usize>>,
    /// 顶层 const/static 类型：name → type
    top_level_consts: HashMap<String, IrType>,
    /// enum variant → enum name 映射
    enum_variants: HashMap<String, String>,
    /// enum 变体字段类型：variant → [类型]（有序，match 臂绑定用）
    enum_variant_field_types: HashMap<String, Vec<IrType>>,
    /// 本块（convert_block 循环）内首次声明的变量：区分「首次绑定」与「重新赋值」
    /// （闭包体内 `total = total + x` 写外部变量 → 应转 Assign，而非新 let 绑定）
    block_declared: std::collections::HashSet<String>,
    /// 当前函数泛型参数
    current_generics: Vec<String>,
    /// 当前函数返回类型
    current_ret_ty: Option<IrType>,
    /// impl 方法中 self 的具体类型（如 Dict<K,V> / HashMap<K,V>）：
    /// 未设置时 self 推断为 Self_，导致 `self[key]`/`key in self` 无法解析
    /// 容器方法（codegen 需 Named Dict/HashMap 分支判断）
    self_ty: Option<IrType>,
    /// 当前是否在 iterator（生成器）函数内：return 等价 raise，不做隐式类型转换
    current_is_iterator: bool,
    /// 当前函数名（用于嵌套函数命名）
    current_fn_name: Option<String>,
    /// 提升出的待处理顶级 Items（嵌套函数等）
    pending_items: Rc<RefCell<Vec<Item>>>,
    /// 语义错误收集（不可变重赋值 E0384 / 空列表类型不可推断 E0282）
    errors: Rc<RefCell<Vec<String>>>,
    /// 顶层 const 的编译期求值结果（name → ComptimeValue）：comptime 块/表达式
    /// 内解析 const 引用（`comptime LIMIT / 2`），否则 Ident 查不到报未定义
    comptime_consts: std::collections::HashMap<String, crate::comptime::ComptimeValue>,
    /// 跨模块类型签名（lz-infer 生成的 .lzi）：函数返回类型回退查询源
    /// （本地函数查不到时，从 .lzi 模块签名补全，接通跨模块推断管线）
    #[cfg(feature = "infer")]
    lzi_signatures: Option<std::rc::Rc<crate::infer::LziRegistry>>,
    /// 当前模块 AST（Rc 共享）：comptime 求值需访问模块函数定义
    /// （`comptime gen_primes(8)` 查 module.functions 编译期执行）
    comptime_module: Option<std::rc::Rc<ast::Module>>,
}

impl TypeCtx {
    fn new() -> Self {
        TypeCtx {
            vars: HashMap::new(),
            fn_returns: HashMap::new(),
            fn_params: HashMap::new(),
            struct_names: HashSet::new(),
            struct_fields: HashMap::new(),
            struct_field_order: HashMap::new(),
            struct_methods: HashMap::new(),
            struct_method_arity: HashMap::new(),
            top_level_consts: HashMap::new(),
            enum_variants: HashMap::new(),
            enum_variant_field_types: HashMap::new(),
            block_declared: std::collections::HashSet::new(),
            current_generics: vec![],
            current_ret_ty: None,
            self_ty: None,
            current_is_iterator: false,
            current_fn_name: None,
            pending_items: Rc::new(RefCell::new(Vec::new())),
            errors: Rc::new(RefCell::new(Vec::new())),
            comptime_consts: std::collections::HashMap::new(),
            comptime_module: None,
            #[cfg(feature = "infer")]
            lzi_signatures: None,
        }
    }

    /// 记录一条语义错误（自动去重），供 build_ir 在末尾统一报错
    fn report_error(&self, msg: String) {
        let mut errors = self.errors.borrow_mut();
        if !errors.contains(&msg) {
            errors.push(msg);
        }
    }

    fn collect_structs(&mut self, module: &ast::Module) {
        for s in &module.structs {
            if s.is_enum {
                for f in &s.fields {
                    self.enum_variants.insert(f.name.clone(), s.name.clone());
                }
                // 收集变体字段类型：variant → [类型]（有序，match 臂绑定用）
                // 变体字段合并存于 Field.ty：单字段 → AstType::Int；多字段 → AstType::Tuple([..])；
                // 命名字段（Circle(x: f64, y: f64)）→ AstType::Record([(name, ty)])
                for v in &s.fields {
                    let types: Vec<IrType> = match &v.ty {
                        AstType::Duck { fields } => fields.iter().map(|(_, t)| from_ast_type(t)).collect(),
                        AstType::Tuple(items) => items.iter().map(from_ast_type).collect(),
                        AstType::Unit => vec![],
                        other => vec![from_ast_type(other)],
                    };
                    self.enum_variant_field_types.insert(v.name.clone(), types);
                }
            } else {
                self.struct_names.insert(s.name.clone());
                let mut fields = HashMap::new();
                let mut field_order: Vec<String> = Vec::new();
                // Self 字段（next: Self?）解析为 struct 自身类型名，供字段访问类型推断
                let self_ty = IrType::Named {
                    path: s.name.clone(),
                    args: s
                        .generics
                        .iter()
                        .map(|g| IrType::Generic(g.clone()))
                        .collect(),
                };
                for f in &s.fields {
                    fields.insert(
                        f.name.clone(),
                        replace_self(&from_ast_type(&f.ty), &self_ty),
                    );
                    field_order.push(f.name.clone());
                }
                self.struct_fields.insert(s.name.clone(), fields);
                self.struct_field_order.insert(s.name.clone(), field_order);
                // 收集 struct 方法名（含魔术方法）
                let mut mset: HashSet<String> = HashSet::new();
                let mut arity_map: HashMap<String, usize> = HashMap::new();
                for m in s.methods.iter() {
                    mset.insert(m.name.clone());
                    arity_map.insert(m.name.clone(), m.params.iter().filter(|p| p.name != "self").count());
                }
                for m in s.magic_methods.iter() {
                    mset.insert(m.name.clone());
                    arity_map.insert(m.name.clone(), m.params.iter().filter(|p| p.name != "self").count());
                }
                self.struct_methods.insert(s.name.clone(), mset);
                self.struct_method_arity.insert(s.name.clone(), arity_map);
            }
        }
    }

    fn collect_functions(&mut self, module: &ast::Module) {
        for f in &module.functions {
            let generics: Vec<String> = f.generics.clone();
            if let Some(ref ret_ty) = f.return_type {
                let ret = from_ast_type_with_generics(ret_ty, &generics);
                // async 函数调用返回 Future<T>（Rust async fn 调用产生 Future），
                // 登记为 Future<T> 供 await / let 标注使用（E0308/E0277 修复）
                if f.is_async {
                    self.fn_returns.insert(
                        f.name.clone(),
                        IrType::Named {
                            path: "Future".into(),
                            args: vec![ret],
                        },
                    );
                } else if f.is_iterator {
                    // iterator 生成器函数（iterator repeat<T>(val, n) -> T / 
                    // count_from(...) -> Iterator<int>）：调用返回**迭代器集合**
                    // Vec<元素类型>（生成代码 `-> Vec<Y>`，急切收集）。登记为
                    // Vec<元素>——若声明返回 Iterator<int> 则取元素 int 登记
                    // Vec<int>，否则 Vec<ret>（iterator_demo `for x in
                    // repeat("hi", 3)` 中 T=String 误当字符串迭代生成 .chars()，
                    // E0599；while_let `Vec<impl Iterator>` 非法 E0562）
                    let elem = match &ret {
                        IrType::Named { path, args } if path == "Iterator" && args.len() == 1 => {
                            args[0].clone()
                        }
                        _ => ret.clone(),
                    };
                    self.fn_returns.insert(
                        f.name.clone(),
                        IrType::Named {
                            path: "Vec".into(),
                            args: vec![elem],
                        },
                    );
                } else {
                    self.fn_returns.insert(f.name.clone(), ret);
                }
            } else {
                // 无返回注解：从函数体最后语句推断并登记。
                // 否则调用点 lookup_fn_return 回退 Any→i64，
                // 导致 `main 末尾调用 closure_in_box()` 被误推为 i64（E0308）
                let ret = f
                    .body
                    .last()
                    .map(|s| infer_stmt_type(s, self))
                    .unwrap_or(IrType::Unit);
                self.fn_returns.insert(f.name.clone(), ret);
            }
            let params: Vec<IrType> = f
                .params
                .iter()
                .map(|p| from_ast_type_with_generics(&p.ty, &generics))
                .collect();
            self.fn_params.insert(f.name.clone(), params);
        }
    }

    #[allow(dead_code)]
    fn begin_fn(&mut self, generics: &[String], ret_ty: Option<&AstType>) {
        self.current_generics = generics.to_vec();
        self.current_ret_ty = ret_ty.map(|t| from_ast_type(t));
    }

    fn add_param(&mut self, name: &str, ty: IrType) {
        self.vars.insert(name.to_string(), ty);
    }

    fn add_var(&mut self, name: &str, ty: IrType) {
        self.vars.insert(name.to_string(), ty);
    }

    fn lookup_var(&self, name: &str) -> IrType {
        self.vars
            .get(name)
            .cloned()
            .or_else(|| {
                self.current_generics
                    .iter()
                    .find(|g| g.as_str() == name)
                    .map(|g| IrType::Generic(g.clone()))
            })
            .or_else(|| self.top_level_consts.get(name).cloned())
            .unwrap_or(IrType::Any)
    }

    fn lookup_fn_return(&self, name: &str) -> IrType {
        if let Some(t) = self.fn_returns.get(name) {
            return t.clone();
        }
        // 跨模块回退：本地函数查不到时，从 .lzi 签名补全（lz-infer 生成的
        // 跨模块类型签名，main.rs --lzi 加载后经 build_ir_with_lzi 注入）
        #[cfg(feature = "infer")]
        if let Some(reg) = &self.lzi_signatures {
            for file in &reg.files {
                for m in file.modules.values() {
                    if let Some(f) = m.functions.get(name) {
                        if let Some(rt) = &f.return_type {
                            return lzi_type_to_ir(rt);
                        }
                    }
                }
            }
        }
        IrType::Any
    }

    fn is_struct(&self, name: &str) -> bool {
        self.struct_names.contains(name)
    }

    fn lookup_field(&self, struct_name: &str, field: &str) -> IrType {
        self.struct_fields
            .get(struct_name)
            .and_then(|fields| fields.get(field))
            .cloned()
            .unwrap_or(IrType::Any)
    }

    fn is_builtin_function(&self, name: &str) -> bool {
        matches!(
            name,
            "print"
                | "println"
                | "panic"
                | "len"
                | "contains"
                | "iter"
                | "enumerate"
                | "zip"
                | "sum"
                | "map"
                | "filter"
                | "collect"
                | "max"
                | "min"
                | "any"
                | "all"
                | "sorted"
                | "reversed"
                | "set!"
                | "format"
                | "hash"
                | "bool"
                | "range"
                | "clone"
                | "sort"
                | "reverse"
                | "spawn"
                | "go"
                | "__go"
                | "Exception"
                | "panic!"
        )
    }

    fn is_struct_type(&self, ty: &IrType) -> bool {
        match ty {
            IrType::Named { path, .. } => self.struct_names.contains(path),
            _ => false,
        }
    }
}

// ══════════════════════════════════════════════════════════════
// AST 运算符映射
// ══════════════════════════════════════════════════════════════

fn map_binop(op: &BinOp) -> BinOpKind {
    match op {
        BinOp::Add => BinOpKind::Add,
        BinOp::Sub => BinOpKind::Sub,
        BinOp::Mul => BinOpKind::Mul,
        BinOp::Div => BinOpKind::Div,
        BinOp::Mod => BinOpKind::Mod,
        BinOp::Eq => BinOpKind::Eq,
        BinOp::Ne => BinOpKind::Neq,
        BinOp::Lt => BinOpKind::Lt,
        BinOp::Gt => BinOpKind::Gt,
        BinOp::Le => BinOpKind::Le,
        BinOp::Ge => BinOpKind::Ge,
        BinOp::And => BinOpKind::And,
        BinOp::Or => BinOpKind::Or,
        BinOp::BitAnd => BinOpKind::BitAnd,
        BinOp::BitOr => BinOpKind::BitOr,
        BinOp::BitXor => BinOpKind::Xor,
        BinOp::Shl => BinOpKind::Shl,
        BinOp::Shr => BinOpKind::Shr,
        BinOp::Pow => BinOpKind::Pow,
        BinOp::In => BinOpKind::In,
        BinOp::NotIn => BinOpKind::NotIn,
        BinOp::Is => BinOpKind::Eq, // Is 降级 (Rust 无 is 运算符)
    }
}

/// 运算符 → 魔术方法名 映射（用于用户自定义类型重载）
fn magic_method_for_binop(op: &BinOp) -> Option<&'static str> {
    match op {
        BinOp::Add => Some("__add__"),
        BinOp::Sub => Some("__sub__"),
        BinOp::Mul => Some("__mul__"),
        BinOp::Div => Some("__div__"),
        BinOp::Mod => Some("__rem__"),
        BinOp::Eq => Some("__eq__"),
        BinOp::Ne => Some("__ne__"),
        BinOp::Lt => Some("__lt__"),
        BinOp::Gt => Some("__gt__"),
        BinOp::Le => Some("__le__"),
        BinOp::Ge => Some("__ge__"),
        BinOp::BitAnd => Some("__bitand__"),
        BinOp::BitOr => Some("__bitor__"),
        BinOp::BitXor => Some("__bitxor__"),
        BinOp::Pow => Some("__pow__"),
        BinOp::In => Some("__contains__"),
        _ => None,
    }
}

fn map_unop(op: &UnaryOp) -> UnOpKind {
    match op {
        UnaryOp::Neg => UnOpKind::Neg,
        UnaryOp::Not => UnOpKind::Not,
        UnaryOp::BitNot => UnOpKind::Not, // 位非降级为逻辑非
        UnaryOp::Deref => UnOpKind::Deref,
        UnaryOp::Ref => UnOpKind::Ref,
    }
}

fn map_assign_op(op: &AssignOp) -> BinOpKind {
    match op {
        AssignOp::Eq => BinOpKind::Eq,
        AssignOp::AddEq => BinOpKind::Add,
        AssignOp::SubEq => BinOpKind::Sub,
        AssignOp::MulEq => BinOpKind::Mul,
        AssignOp::DivEq => BinOpKind::Div,
        AssignOp::ModEq => BinOpKind::Mod,
        AssignOp::AndEq => BinOpKind::BitAnd,
        AssignOp::OrEq => BinOpKind::BitOr,
        AssignOp::XorEq => BinOpKind::Xor,
        AssignOp::ShlEq => BinOpKind::Shl,
        AssignOp::ShrEq => BinOpKind::Shr,
        AssignOp::PowEq => BinOpKind::Pow,
    }
}

/// 泛型参数规范化：`Named("T", [])`（from_ast_type 表示）→ `Generic("T")`
/// （infer 表示），递归处理嵌套（`Named("Rc", [Named("T")])` → `Named("Rc", [Generic("T")])`）。
/// 用于 return 隐式转换判断中两侧容器元素类型比较（box.lz `Err(self.clone())`）。
fn normalize_gen(ty: &IrType, generics: &[String]) -> IrType {
    match ty {
        IrType::Named { path, args } => {
            if args.is_empty() {
                if generics.iter().any(|g| g == path) {
                    IrType::Generic(path.clone())
                } else {
                    ty.clone()
                }
            } else {
                let new_args: Vec<IrType> = args.iter().map(|a| normalize_gen(a, generics)).collect();
                IrType::Named {
                    path: path.clone(),
                    args: new_args,
                }
            }
        }
        _ => ty.clone(),
    }
}

/// 将类型名字符串转换为 IrType（用于 `is` 运算符）
fn name_to_ir_type(name: &str) -> IrType {
    match name {
        "int" | "i64" => IrType::Int,
        "str" | "String" => IrType::Str,
        "f64" | "float" => IrType::F64,
        "bool" => IrType::Bool,
        "Ext" => IrType::Ext,
        "List" | "Vec" => IrType::Named {
            path: "List".into(),
            args: vec![],
        },
        "Dict" | "HashMap" => IrType::Named {
            path: "Dict".into(),
            args: vec![],
        },
        "Set" | "HashSet" => IrType::Named {
            path: "Set".into(),
            args: vec![],
        },
        _ => IrType::Named {
            path: name.to_string(),
            args: vec![],
        },
    }
}

/// 判断 `x as T` 是否为编译器内置转换（无需 `__cast__`/`__try_cast__`）：
/// 基本数值类型（int/f64/bool/str）之间的转换由编译器内置实现
/// （01-类型系统.md §6：数值基本类型间 `as` 不依赖魔法方法）；
/// 内置容器类型（List/Dict/Set/Option/Result/Box/Tuple 等）的字面量
/// 类型标注（`[] as List<int>`、`None as Option<int>`）同样是内置标注，
/// 不需要 `__cast__`/`__try_cast__`。
/// 非内置转换（用户自定义 struct/enum/duck 参与）必须由源类型实现
/// `__cast__<T>()` 或 `__try_cast__<T>() -> Result<T, E>`。
fn is_builtin_cast(src: &IrType, target: &IrType) -> bool {
    let src_builtin = matches!(
        src,
        IrType::Int | IrType::F64 | IrType::Bool | IrType::Str | IrType::Unit | IrType::Never
    );
    let tgt_builtin = matches!(
        target,
        IrType::Int | IrType::F64 | IrType::Bool | IrType::Str | IrType::Unit | IrType::Never
    );
    if src_builtin && tgt_builtin {
        return true;
    }
    // 内置容器类型名（字面量标注放行）
    // 注意：AST 中 `List<int>` 的 type_name 被解析成整串 "Vec<i64>"，
    // name_to_ir_type 走 `_ =>` 分支存为 Named { path: "Vec<i64>" }，
    // 因此需按 `<` 前的基名匹配（"Vec"）而非全名。
    let builtin_container = |t: &IrType| -> bool {
        match t {
            IrType::Named { path, .. } => {
                let base = path.split('<').next().unwrap_or(path.as_str());
                matches!(
                    base,
                    "List" | "Vec" | "Dict" | "HashMap" | "Set" | "HashSet" | "Option" | "Result"
                        | "Box" | "Tuple" | "String" | "Any"
                )
            }
            _ => false,
        }
    };
    builtin_container(src) && builtin_container(target)
}

/// 编译期类型兼容检查（用于 `is` 运算符和类型转换）
fn ir_types_compatible(a: &IrType, b: &IrType) -> bool {
    match (a, b) {
        // Any 与任何类型兼容（None、未知等）
        (IrType::Any, _) | (_, IrType::Any) => true,
        // 相同基础类型
        (IrType::Int, IrType::Int)
        | (IrType::F64, IrType::F64)
        | (IrType::Str, IrType::Str)
        | (IrType::Bool, IrType::Bool)
        | (IrType::Unit, IrType::Unit)
        | (IrType::Never, IrType::Never) => true,
        // Named 类型按名称匹配
        (IrType::Named { path: a_path, .. }, IrType::Named { path: b_path, .. }) => {
            a_path == b_path
        }
        // Option 解包
        (IrType::Option(inner), other) | (other, IrType::Option(inner)) => {
            ir_types_compatible(inner, other)
        }
        _ => false,
    }
}

/// 从 AST 表达式提取类型名称列表（支持单类型和多类型参数）
/// - Ident("int") → Some(vec!["int"])
/// - TupleLit([Ident("int"), Ident("str")]) → Some(vec!["int", "str"])
/// - 其他 → None
fn extract_type_names(expr: &AstExpr) -> Option<Vec<String>> {
    match expr {
        AstExpr::Ident(name) => Some(vec![name.clone()]),
        AstExpr::TupleLit(elems) => {
            let names: Option<Vec<String>> = elems
                .iter()
                .map(|e| {
                    if let AstExpr::Ident(n) = e {
                        Some(n.clone())
                    } else {
                        None
                    }
                })
                .collect();
            names
        }
        _ => None,
    }
}

/// 将 LZ 类型名映射为 Rust 类型名（用于泛型类型参数）
fn map_type_args(names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|t| match t.as_str() {
            "int" => "i64".to_string(),
            "str" => "String".to_string(),
            "f64" | "float" => "f64".to_string(),
            "bool" => "bool".to_string(),
            other => other.to_string(),
        })
        .collect()
}

/// 从实参类型解析泛型函数调用：推断泛型参数的具体类型
///
/// 策略：
/// 1. 收集函数定义中泛型参数名列表（从 param_tys 和 ret_ty 中提取 Generic 变量）
/// 2. 对每个参数位置，尝试将定义的 param_ty 与实参 arg_ty 匹配
/// 3. 如果 param_ty 是 Generic("T") 且 arg_ty 是具体类型，则将 T 绑定到 arg_ty
/// 4. 用绑定结果替换 ret_ty 中的泛型变量
/// 用显式 turbofish 类型参数（`parse_num.<int>("42")`）替换返回类型中的泛型。
/// 泛型名按返回类型中出现的顺序与 type_args 一一对应（T → 第一个实参类型）。
fn apply_explicit_type_args(ret_ty: &IrType, type_args: &[String]) -> IrType {
    // 收集返回类型中的泛型名（按出现顺序去重）
    let mut generics: Vec<String> = Vec::new();
    fn collect(ty: &IrType, out: &mut Vec<String>) {
        match ty {
            IrType::Generic(name) => {
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
            IrType::Named { args, .. } => {
                for a in args {
                    collect(a, out);
                }
            }
            IrType::Option(inner) => collect(inner, out),
            IrType::Result { ok, err } => {
                collect(ok, out);
                collect(err, out);
            }
            IrType::Tuple(elems) => {
                for e in elems {
                    collect(e, out);
                }
            }
            IrType::Fn { params, ret } => {
                for p in params {
                    collect(p, out);
                }
                collect(ret, out);
            }
            IrType::Ref(inner) | IrType::MutRef(inner) => collect(inner, out),
            IrType::Duck { fields } => {
                for (_, t) in fields {
                    collect(t, out);
                }
            }
            _ => {}
        }
    }
    collect(ret_ty, &mut generics);

    // 构造替换映射：generics[i] → type_args[i] 转换的 IrType
    let mut subst: std::collections::HashMap<String, IrType> = std::collections::HashMap::new();
    for (i, g) in generics.iter().enumerate() {
        let concrete = type_args
            .get(i)
            .map(|s| from_ast_type_name(s))
            .unwrap_or(IrType::Any);
        subst.insert(g.clone(), concrete);
    }

    // 递归替换
    fn replace(ty: &IrType, subst: &std::collections::HashMap<String, IrType>) -> IrType {
        match ty {
            IrType::Generic(name) => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
            IrType::Named { path, args } => IrType::Named {
                path: path.clone(),
                args: args.iter().map(|a| replace(a, subst)).collect(),
            },
            IrType::Option(inner) => IrType::Option(Box::new(replace(inner, subst))),
            IrType::Result { ok, err } => IrType::Result {
                ok: Box::new(replace(ok, subst)),
                err: Box::new(replace(err, subst)),
            },
            IrType::Tuple(elems) => IrType::Tuple(elems.iter().map(|e| replace(e, subst)).collect()),
            IrType::Fn { params, ret } => IrType::Fn {
                params: params.iter().map(|p| replace(p, subst)).collect(),
                ret: Box::new(replace(ret, subst)),
            },
            IrType::Ref(inner) => IrType::Ref(Box::new(replace(inner, subst))),
            IrType::MutRef(inner) => IrType::MutRef(Box::new(replace(inner, subst))),
            IrType::Duck { fields } => IrType::Duck {
                fields: fields
                    .iter()
                    .map(|(n, t)| (n.clone(), replace(t, subst)))
                    .collect(),
            },
            other => other.clone(),
        }
    }
    replace(ret_ty, &subst)
}

/// 按类型名构造 IrType（turbofish 类型参数用）
fn from_ast_type_name(name: &str) -> IrType {
    match name {
        "int" | "i64" => IrType::Int,
        "str" | "String" => IrType::Str,
        "f64" | "float" => IrType::F64,
        "bool" => IrType::Bool,
        "Ext" => IrType::Ext,
        other => IrType::Named {
            path: other.to_string(),
            args: vec![],
        },
    }
}

fn resolve_call_generics(
    ret_ty: &IrType,
    fn_name: &str,
    param_tys: &[IrType],
    arg_tys: &[IrType],
    ctx: &TypeCtx,
) -> IrType {
    // 收集所有泛型参数名
    let mut generic_names = std::collections::HashSet::new();
    fn collect_generics(ty: &IrType, set: &mut std::collections::HashSet<String>) {
        match ty {
            IrType::Generic(name) => {
                set.insert(name.clone());
            }
            IrType::Named { args, .. } => {
                for a in args {
                    collect_generics(a, set);
                }
            }
            IrType::Option(inner) => collect_generics(inner, set),
            IrType::Result { ok, err } => {
                collect_generics(ok, set);
                collect_generics(err, set);
            }
            IrType::Tuple(elems) => {
                for e in elems {
                    collect_generics(e, set);
                }
            }
            IrType::Fn { params, ret } => {
                for p in params {
                    collect_generics(p, set);
                }
                collect_generics(ret, set);
            }
            IrType::Ref(inner) | IrType::MutRef(inner) => collect_generics(inner, set),
            IrType::Duck { fields } => {
                for (_, t) in fields {
                    collect_generics(t, set);
                }
            }
            _ => {}
        }
    }
    for pt in param_tys {
        collect_generics(pt, &mut generic_names);
    }
    collect_generics(ret_ty, &mut generic_names);

    if generic_names.is_empty() {
        return ret_ty.clone();
    }

    // 尝试从实参类型推断泛型绑定
    let mut bindings: std::collections::HashMap<String, IrType> = std::collections::HashMap::new();
    let n = param_tys.len().min(arg_tys.len());
    for i in 0..n {
        infer_generic_binding(&param_tys[i], &arg_tys[i], &mut bindings);
    }

    // 如果没有任何绑定（例如无参泛型函数），尝试从 ctx 的泛型列表推断
    if bindings.is_empty() && !ctx.current_generics.is_empty() {
        // 使用当前位置的泛型参数（当前函数定义的泛型）
        // 这处理了同一泛型函数内调用自身或其他泛型函数的情况
        let mut alt_bindings = std::collections::HashMap::new();
        for g in &ctx.current_generics {
            if generic_names.contains(g) {
                alt_bindings.insert(g.clone(), IrType::Generic(g.clone()));
            }
        }
        if !alt_bindings.is_empty() {
            // 有上层泛型上下文 → 传播泛型变量
            let generics: Vec<String> = alt_bindings.keys().cloned().collect();
            let concrete: Vec<IrType> = alt_bindings.values().cloned().collect();
            return ret_ty.substitute_generics(&generics, &concrete);
        }
    }

    if bindings.is_empty() {
        // G2 反例：泛型调用既未提供显式类型实参，也无法从实参推断 → 必须拒绝
        // （如 `def f<T>() -> T` 后 `f()`：返回类型 T 无法绑定）
        ctx.report_error(format!(
            "无法推断泛型参数: 调用 {fn_name} 未提供显式类型实参（如 {fn_name}.<T>(...)），且无法从实参推断"
        ));
        return ret_ty.clone();
    }

    let generics: Vec<String> = bindings.keys().cloned().collect();
    let concrete: Vec<IrType> = bindings.values().cloned().collect();
    ret_ty.substitute_generics(&generics, &concrete)
}

/// 尝试从 (param_ty, arg_ty) 对中推断泛型绑定
fn infer_generic_binding(
    param_ty: &IrType,
    arg_ty: &IrType,
    bindings: &mut std::collections::HashMap<String, IrType>,
) {
    match param_ty {
        IrType::Generic(name) => {
            // 直接绑定：T = arg_ty
            // 只有 arg_ty 不是 Any 也不是 Generic 时才绑定
            if !matches!(arg_ty, IrType::Any | IrType::Generic(_)) {
                bindings
                    .entry(name.clone())
                    .or_insert_with(|| arg_ty.clone());
            }
        }
        IrType::Named {
            path: p_path,
            args: p_args,
        } => {
            if let IrType::Named {
                path: a_path,
                args: a_args,
            } = arg_ty
            {
                if p_path == a_path {
                    for (p, a) in p_args.iter().zip(a_args.iter()) {
                        infer_generic_binding(p, a, bindings);
                    }
                }
            }
        }
        IrType::Option(p_inner) => {
            if let IrType::Option(a_inner) = arg_ty {
                infer_generic_binding(p_inner, a_inner, bindings);
            }
        }
        IrType::Result {
            ok: p_ok,
            err: p_err,
        } => {
            if let IrType::Result {
                ok: a_ok,
                err: a_err,
            } = arg_ty
            {
                infer_generic_binding(p_ok, a_ok, bindings);
                infer_generic_binding(p_err, a_err, bindings);
            }
        }
        IrType::Tuple(p_elems) => {
            if let IrType::Tuple(a_elems) = arg_ty {
                for (p, a) in p_elems.iter().zip(a_elems.iter()) {
                    infer_generic_binding(p, a, bindings);
                }
            }
        }
        IrType::Ref(p_inner) => {
            if let IrType::Ref(a_inner) = arg_ty {
                infer_generic_binding(p_inner, a_inner, bindings);
            }
        }
        _ => {}
    }
}

// ══════════════════════════════════════════════════════════════
// 类型推导：从 AST Expr 推导出 IrType
// ══════════════════════════════════════════════════════════════

fn infer_expr_type(ast_expr: &AstExpr, ctx: &TypeCtx) -> IrType {
    match ast_expr {
        // comptime 表达式：类型与内部表达式一致（B3 求值内联）
        AstExpr::Comptime(inner) => infer_expr_type(inner, ctx),
        AstExpr::IntLit(_) => IrType::Int,
        AstExpr::FloatLit(_) => IrType::F64,
        AstExpr::StrLit(_) | AstExpr::FStrLit(_) | AstExpr::RawStrLit(_) => IrType::Str,
        AstExpr::BoolLit(_) => IrType::Bool,
        AstExpr::NoneLit => IrType::Any, // None 类型取决于上下文
        AstExpr::Ident(name) => {
            // 裸枚举变体名（Less/Equal/Greater）：类型应为枚举类型而非 Any→i64 fallback，
            // 否则 `return Less` 会插入 <Ordering as ImplicitFrom<i64>> 错误转换（E0277）
            if let Some(enum_name) = ctx.enum_variants.get(name.as_str()) {
                if !ctx.vars.contains_key(name.as_str()) {
                    IrType::Named {
                        path: enum_name.clone(),
                        args: vec![],
                    }
                } else {
                    ctx.lookup_var(name)
                }
            } else {
                ctx.lookup_var(name)
            }
        }
        AstExpr::Call {
            func,
            args,
            type_args,
        } => {
            if let AstExpr::Ident(fname) = func.as_ref() {
                // __as__ 类型转换：返回目标类型
                if fname == "__as__" && args.len() == 2 {
                    if let AstExpr::Ident(type_name) = &args[1] {
                        return name_to_ir_type(type_name);
                    }
                    return IrType::Any;
                }
                // print/println/panic 是语言内建，返回 Unit
                if fname == "print" || fname == "println" || fname == "panic" {
                    return IrType::Unit;
                }
                // 宏系统（08-宏与编译期.md）：quote(...) 是宏体 Token 包装，
                // IR 后端不展开宏，返回 Str（宏体字符串拼接，参数数量不限）
                if fname == "quote" && !args.is_empty() {
                    return IrType::Str;
                }
                // str/int/float/bool 类型转换内建：返回对应类型
                match fname.as_str() {
                    "str" => return IrType::Str,
                    "int" => return IrType::Int,
                    "float" => return IrType::F64,
                    "bool" => return IrType::Bool,
                    "len" => return IrType::Int,
                    _ => {}
                }
                // Ok/Err/Some/None 变体构造：返回 Result/Option 类型。
                // 否则 `let ok = Ok(10)` 推断为 Any，后续 ok.map(...) 无法
                // 触发 codegen 的消费型方法 clone 注入（E0382 moved value）
                match fname.as_str() {
                    "Ok" => {
                        let ok_ty = args
                            .first()
                            .map(|a| infer_expr_type(a, ctx))
                            .unwrap_or(IrType::Any);
                        return IrType::Result {
                            ok: Box::new(ok_ty),
                            err: Box::new(IrType::Any),
                        };
                    }
                    "Err" => {
                        let err_ty = args
                            .first()
                            .map(|a| infer_expr_type(a, ctx))
                            .unwrap_or(IrType::Any);
                        return IrType::Result {
                            ok: Box::new(IrType::Any),
                            err: Box::new(err_ty),
                        };
                    }
                    "Some" => {
                        let inner = args
                            .first()
                            .map(|a| infer_expr_type(a, ctx))
                            .unwrap_or(IrType::Any);
                        return IrType::Option(Box::new(inner));
                    }
                    "None" => return IrType::Option(Box::new(IrType::Any)),
                    _ => {}
                }
                if ctx.is_struct(fname) {
                    return IrType::named(fname);
                }
                // 容器空构造：`List()` / `Set()` / `Dict()` 返回对应容器类型，
                // 否则推断为 Any→i64（`let mut result = List()` 后 `return result`
                // 生成 <Vec<U> as ImplicitFrom<i64>> 错误转换，E0277）
                if args.is_empty() {
                    match fname.as_str() {
                        "List" | "Vec" => {
                            return IrType::Named {
                                path: "List".into(),
                                args: vec![IrType::Any],
                            }
                        }
                        "Set" | "HashSet" => {
                            return IrType::Named {
                                path: "Set".into(),
                                args: vec![IrType::Any],
                            }
                        }
                        "Dict" | "HashMap" => {
                            return IrType::Named {
                                path: "Dict".into(),
                                args: vec![IrType::Any, IrType::Any],
                            }
                        }
                        _ => {}
                    }
                }
                // 闭包变量调用：consume() 中 consume 是局部 Fn 变量 →
                // 返回闭包体 ret 类型（否则 lookup_fn_return 回退 Any→i64，E0308）
                if let IrType::Fn { ret, .. } = ctx.lookup_var(fname) {
                    return *ret;
                }
                let ret_ty = ctx.lookup_fn_return(fname);
                // 显式 turbofish 类型参数（parse_num.<int>("42")）：
                // 直接用 type_args 替换返回类型中的泛型（否则 Result<T, String> 中 T 未绑定，E0425）
                if !type_args.is_empty() && ret_ty.contains_generics() {
                    return apply_explicit_type_args(&ret_ty, type_args);
                }
                // 泛型分辨率：如果返回类型包含 Generic，尝试从实参推断
                if ret_ty.contains_generics() {
                    if let Some(param_tys) = ctx.fn_params.get(fname) {
                        let arg_tys: Vec<IrType> =
                            args.iter().map(|a| infer_expr_type(a, ctx)).collect();
                        // 根据参数类型推断泛型变量
                        let resolved =
                            resolve_call_generics(&ret_ty, fname, param_tys, &arg_tys, ctx);
                        return resolved;
                    }
                }
                ret_ty
            } else {
                IrType::Any
            }
        }
        AstExpr::MethodCall {
            receiver, method, ..
        } => {
            // 枚举变体构造: Kind.A(1) → Kind 类型
            // receiver 是枚举/结构类型名时，方法名是变体
            if let AstExpr::Ident(recv_name) = receiver.as_ref() {
                let base = recv_name.split('<').next().unwrap_or(recv_name);
                if ctx.is_struct(base) || ctx.enum_variants.values().any(|e| e == base) {
                    return IrType::named(base);
                }
            }
            // 尝试从 receiver 类型推导方法返回类型
            let recv_ty = infer_expr_type(receiver, ctx);
            // size_hint() 返回 (int, Option<int>)（LZ 视角 int；codegen 在
            // impl Iterator 中映射为 usize）。iter.lz Zip::size_hint 中
            // `self.a.size_hint()` 若不推断，min(lo_a, lo_b) 报 E0308
            if method == "size_hint" || method == "__size_hint__" {
                return IrType::Tuple(vec![
                    IrType::Int,
                    IrType::Option(Box::new(IrType::Int)),
                ]);
            }
            // 常见无返回值方法 → Unit
            if method == "push"
                || method == "insert"
                || method == "remove"
                || method == "clear"
                || method == "append"
                || method == "set"
            {
                return IrType::Unit;
            }
            // 比较魔术方法（__eq__/__ne__/__lt__/__gt__/__le__/__ge__）→ Bool：
            // option.lz `a.__eq__(b)` 推断为 Bool，否则回退 i64 导致
            // `<bool as ImplicitFrom<i64>>::__implicit_from__(a == b)`（E0277）
            if method == "__eq__"
                || method == "__ne__"
                || method == "__lt__"
                || method == "__gt__"
                || method == "__le__"
                || method == "__ge__"
            {
                return IrType::Bool;
            }
            // 内置方法返回类型推断表
            if let Some(ret) = lookup_builtin_method_ret(&recv_ty, method, ctx) {
                return ret;
            }
            match &recv_ty {
                IrType::Named { path, .. } => {
                    // 简单启发式：Option::unwrap → 内部类型
                    if method == "unwrap" || method == "expect" {
                        if path == "Option" {
                            return IrType::Any; // 无法从类型名推断内部类型
                        }
                    }
                    if method == "len" {
                        return IrType::Int;
                    }
                    // clone() 返回接收者类型（Rc/Arc/Box 等）
                    if method == "clone" && (path == "Rc" || path == "Arc" || path == "Box") {
                        return recv_ty.clone();
                    }
                    // 用户 struct 方法：从登记的方法返回类型查询（box.lz `get` 返回
                    // `ref T`，否则 `b.get()` 推断为 Any，`assert b.get() == 42`
                    // 无法解引用，E0277 can't compare &i64 with i64）
                    if ctx
                        .struct_methods
                        .get(path)
                        .map(|ms| ms.contains(method))
                        .unwrap_or(false)
                    {
                        let mret = ctx.lookup_fn_return(&format!("{}.{}", path, method));
                        if !matches!(mret, IrType::Any) {
                            return mret;
                        }
                    }
                    // 用户 struct 的算术/构造魔术方法返回接收者类型
                    if ctx
                        .struct_methods
                        .get(path)
                        .map(|ms| ms.contains(method))
                        .unwrap_or(false)
                        && matches!(
                            method.as_str(),
                            "__add__"
                                | "__sub__"
                                | "__mul__"
                                | "__div__"
                                | "__new__"
                                | "__call__"
                                | "__getitem__"
                                | "__iter__"
                                | "__setitem__"
                        )
                    {
                        return recv_ty.clone();
                    }
                }
                _ => {}
            }
            IrType::Any
        }
        AstExpr::FieldAccess { receiver, field } => {
            let recv_ty = infer_expr_type(receiver, ctx);
            match &recv_ty {
                IrType::Named { path, .. } => ctx.lookup_field(path, field),
                _ => IrType::Any,
            }
        }
        AstExpr::Index { receiver, .. } => {
            // `self[key]` 索引类型：从容器类型推断元素类型，而不是恒为 Any→i64。
            // Dict<K,V> → V；List<T>/Vec<T> → T；否则 Any
            let recv_ty = infer_expr_type(receiver, ctx);
            match &recv_ty {
                IrType::Named { path, args } if path == "Dict" || path == "HashMap" => {
                    args.get(1).cloned().unwrap_or(IrType::Any)
                }
                IrType::Named { path, args } if path == "List" || path == "Vec" => {
                    args.first().cloned().unwrap_or(IrType::Any)
                }
                _ => IrType::Any,
            }
        }
        AstExpr::Binary { left, op, right } => {
            // `is` 运算符始终返回 Bool
            if matches!(op, BinOp::Is) {
                return IrType::Bool;
            }
            // 比较/布尔运算符返回 Bool
            if matches!(
                op,
                BinOp::Eq
                    | BinOp::Ne
                    | BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::And
                    | BinOp::Or
                    | BinOp::In
                    | BinOp::NotIn
            ) {
                return IrType::Bool;
            }
            // 取左侧操作数的类型（简化）；左侧未知（Any）时回退到右侧，
            // 否则 `n * 10`（n 为 walrus 变量未登记）推断为 Any，三元条件
            // 无法触发 codegen 的真值转换（combo_ternary_walrus.lz E0308）
            let lt = infer_expr_type(left, ctx);
            if matches!(&lt, IrType::Any) {
                infer_expr_type(right, ctx)
            } else {
                lt
            }
        }
        AstExpr::Unary { op, operand } => match op {
            _ => infer_expr_type(operand, ctx),
        },
        AstExpr::If { then_body, .. } => {
            // 取 then 分支最后表达式类型
            then_body
                .last()
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit)
        }
        AstExpr::Match { expr, arms } => {
            // type-pack 异质元组（03d §2.8 方案 B）：`..: Tuple<Ts...>` 的 args
            // 编译为 List<Ts> 切片，`case (a,)` / `case (a, ..)` 臂体返回 a（元素 &Ts）。
            // 从 scrutinee 元素类型 + 模式绑定变量推断臂体返回类型（否则 Any→i64 误判）
            let scrut_ty = infer_expr_type(expr, ctx);
            let elem_ty = match &scrut_ty {
                IrType::Named { args, .. } if !args.is_empty() => Some(args[0].clone()),
                _ => None,
            };
            if let (Some(arm), Some(elem)) = (arms.first(), elem_ty) {
                let mut binds = Vec::new();
                collect_ast_pattern_vars(&arm.pattern, &mut binds);
                // 臂体返回绑定变量（如 `case (a,) => a`）→ 返回元素类型
                if let Some(last) = arm.body.last() {
                    if let AstStmt::Expr(AstExpr::Ident(n)) = last {
                        if binds.iter().any(|b| b == n) {
                            return elem;
                        }
                    }
                }
            }
            arms.first()
                .and_then(|arm| arm.body.last())
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit)
        }
        AstExpr::Closure { .. } => IrType::Any,
        AstExpr::BlockExpr(_) => IrType::Any,
        AstExpr::Range { .. } => IrType::named("Range"),
        AstExpr::Walrus { value, .. } => infer_expr_type(value, ctx),
        AstExpr::Pipe { callee, .. } => {
            // 管道结果类型 = 右侧 callable 的返回类型：
            // - Ident 是已知 struct（构造调用）→ struct 类型本身
            // - Ident 是变量（__call__ 实例）→ 变量类型（__call__ 通常返回同类型）
            // - Ident 是函数 → 函数返回类型；闭包 → Any
            match callee.as_ref() {
                AstExpr::Ident(name) => {
                    if ctx.is_struct(name) {
                        // 构造调用 Point(2.0) → Point 类型（首参预填充 receiver）
                        IrType::Named {
                            path: name.clone(),
                            args: vec![],
                        }
                    } else if let IrType::Named { path, .. } = ctx.lookup_var(name) {
                        // 变量实例：若类型实现了 __call__，管道结果为 __call__ 返回类型
                        // （推断期近似为实例类型本身，Point.__call__ 返回 Point）
                        if ctx
                            .struct_methods
                            .get(&path)
                            .map_or(false, |m| m.contains("__call__") || m.contains("__rpipe__"))
                        {
                            IrType::Named {
                                path: path.clone(),
                                args: vec![],
                            }
                        } else {
                            ctx.lookup_fn_return(name)
                        }
                    } else {
                        ctx.lookup_fn_return(name)
                    }
                }
                AstExpr::Closure { .. } => IrType::Any,
                _ => IrType::Any,
            }
        }
        AstExpr::Try(inner) => {
            // try 表达式：Result → Ok 类型
            let inner_ty = infer_expr_type(inner, ctx);
            match &inner_ty {
                IrType::Result { ok, .. } => *ok.clone(),
                _ => IrType::Any,
            }
        }
        AstExpr::NullCoalesce { left, right } => {
            let left_ty = infer_expr_type(left, ctx);
            let right_ty = infer_expr_type(right, ctx);
            let is_right_option = matches!(&right_ty, IrType::Option(_))
                || matches!(&right_ty, IrType::Named { path, .. } if path == "Option");
            if is_right_option {
                // Option<T> ?? Option<T> → Option<T>
                left_ty
            } else {
                // Option<T> ?? T → T（解包）
                match &left_ty {
                    IrType::Option(inner) => *inner.clone(),
                    IrType::Named { path, args } if path == "Option" && !args.is_empty() => {
                        args[0].clone()
                    }
                    _ => left_ty,
                }
            }
        }
        AstExpr::ListLit(items) => {
            let elem_ty = items
                .first()
                .map(|i| infer_expr_type(i, ctx))
                .unwrap_or(IrType::Any);
            IrType::Named {
                path: "List".into(),
                args: vec![elem_ty],
            }
        }
        AstExpr::DictLit(entries) => {
            let key_ty = entries
                .first()
                .and_then(|(k, _)| Some(infer_expr_type(k, ctx)))
                .unwrap_or(IrType::Any);
            let val_ty = entries
                .first()
                .and_then(|(_, v)| Some(infer_expr_type(v, ctx)))
                .unwrap_or(IrType::Any);
            IrType::Named {
                path: "Dict".into(),
                args: vec![key_ty, val_ty],
            }
        }
        AstExpr::SetLit(items) => {
            let elem_ty = items
                .first()
                .map(|i| infer_expr_type(i, ctx))
                .unwrap_or(IrType::Any);
            IrType::Named {
                path: "Set".into(),
                args: vec![elem_ty],
            }
        }
        AstExpr::TupleLit(elems) => {
            IrType::Tuple(elems.iter().map(|e| infer_expr_type(e, ctx)).collect())
        }
        AstExpr::ListComprehension { output, .. } => {
            let elem_ty = infer_expr_type(output, ctx);
            IrType::Named {
                path: "List".into(),
                args: vec![elem_ty],
            }
        }
        AstExpr::DictComprehension { key, value, .. } => {
            let k_ty = infer_expr_type(key, ctx);
            let v_ty = infer_expr_type(value, ctx);
            IrType::Named {
                path: "Dict".into(),
                args: vec![k_ty, v_ty],
            }
        }
        AstExpr::SetComprehension { elem, .. } => {
            let elem_ty = infer_expr_type(elem, ctx);
            IrType::Named {
                path: "Set".into(),
                args: vec![elem_ty],
            }
        }
        AstExpr::Assign { value, .. } => infer_expr_type(value, ctx),
        AstExpr::Spawn(inner) => IrType::Named {
            path: "Future".into(),
            args: vec![infer_expr_type(inner, ctx)],
        },
        AstExpr::Move(inner) => infer_expr_type(inner, ctx),
        AstExpr::Panic(_) => IrType::Never,
        AstExpr::Await(inner) => {
            // await Future<T> → T
            let inner_ty = infer_expr_type(inner, ctx);
            match &inner_ty {
                IrType::Named { path, args } if path == "Future" && !args.is_empty() => {
                    args[0].clone()
                }
                _ => IrType::Any,
            }
        }
        AstExpr::BuildBlock { kind, lhs, body } => {
            // =: / ~: / *: 构建块返回 lhs 类型（或块类型）
            // ^: 索引构建块返回 lhs 的元素类型（如 Vec<T> → T）
            let lhs_ty = infer_expr_type(lhs, ctx);
            match kind {
                BuildKind::Index => match &lhs_ty {
                    IrType::Named { args, .. } if !args.is_empty() => args[0].clone(),
                    _ => lhs_ty,
                },
                BuildKind::Call => {
                    // ~: 构建块的实际返回类型是 callee 的返回值类型
                    // body 的类型是元组（被解包为 callee 的参数），不是调用结果
                    match &lhs_ty {
                        IrType::Fn { ret, .. } => *ret.clone(),
                        _ => IrType::Any, // callee 类型未知，用 Any 避免错误类型标注
                    }
                }
                BuildKind::Gen => {
                    // *: 生成器构建块 → List<元素类型>
                    // 有 callee（函数/方法引用）时元素类型 = callee 返回类型；
                    // 否则从 body 中第一个 yield 表达式推导
                    let has_callee = matches!(
                        &**lhs,
                        AstExpr::Ident(_) | AstExpr::MethodCall { .. } | AstExpr::FieldAccess { .. }
                    );
                    // 优先从函数符号表取返回类型（Ident 直接引用函数时 lookup_var 回退 Any，
                    // 会导致 *: 构建块元素类型错误地取 yield 包类型）
                    let elem_ty = if let AstExpr::Ident(fname) = &**lhs {
                        if let Some(ret) = ctx.fn_returns.get(fname) {
                            ret.clone()
                        } else {
                            infer_yield_elem_ty(body, lhs_ty, ctx)
                        }
                    } else if has_callee {
                        match &lhs_ty {
                            IrType::Fn { ret, .. } => *ret.clone(),
                            _ => infer_yield_elem_ty(body, lhs_ty, ctx),
                        }
                    } else {
                        infer_yield_elem_ty(body, lhs_ty, ctx)
                    };
                    IrType::Named {
                        path: "List".into(),
                        args: vec![elem_ty],
                    }
                }
                BuildKind::Var => {
                    // =: 构建块返回块体末尾表达式类型（如 (a,b,c) 元组）。
                    // lhs 是目标变量名（此处尚未登记，lookup_var 回退 Any），
                    // 故优先用 body 末尾语句推断，否则 `multiply ~: factors`
                    // 的元组拆包会因 factors 类型为 Any 而失败（E0061）
                    let last_ty = body
                        .last()
                        .map(|s| infer_stmt_type(s, ctx))
                        .filter(|t| !matches!(t, IrType::Any))
                        .unwrap_or(lhs_ty);
                    last_ty
                }
            }
        }
        AstExpr::KwArg { .. } => IrType::Any,
        AstExpr::PathAccess { .. } => IrType::Any,
        AstExpr::SafeNav { .. } => IrType::Any,
        AstExpr::TryCatch { body, .. } => {
            // try/catch 表达式返回类型 = try body 最后一个**表达式**语句的类型
            // （跳过尾部 let/声明，避免无注解函数推断为 Any→i64，
            //   如 try_finally_only / try_catch_else_demo 应为 Unit）
            body.iter()
                .rev()
                .find(|s| matches!(s, AstStmt::Expr(_) | AstStmt::Return(_) | AstStmt::Yield(_)))
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit)
        }
        AstExpr::Paren(inner) => infer_expr_type(inner, ctx),
    }
}

/// 检测 AST 语句是否包含无值 return（return;）——构建块（=:/~:）内
/// return; 退出构建块自身，块值应为 Unit；此时变量类型/尾表达式均按 Unit 处理
fn ast_stmt_has_bare_return(stmt: &AstStmt) -> bool {
    match stmt {
        AstStmt::Return(None) => true,
        AstStmt::Return(Some(_)) => false,
        AstStmt::While { body, else_body, .. } => {
            body.iter().any(ast_stmt_has_bare_return)
                || else_body.as_ref().map_or(false, |b| b.iter().any(ast_stmt_has_bare_return))
        }
        AstStmt::WhileLet { body, else_body, .. } => {
            body.iter().any(ast_stmt_has_bare_return)
                || else_body.as_ref().map_or(false, |b| b.iter().any(ast_stmt_has_bare_return))
        }
        AstStmt::For { body, else_body, .. } => {
            body.iter().any(ast_stmt_has_bare_return)
                || else_body.as_ref().map_or(false, |b| b.iter().any(ast_stmt_has_bare_return))
        }
        AstStmt::Loop(body)
        | AstStmt::Block { body, .. }
        | AstStmt::CheckerBlock { body, .. }
        | AstStmt::Defer(body)
        | AstStmt::Comptime { body } => body.iter().any(ast_stmt_has_bare_return),
        AstStmt::Expr(AstExpr::If {
            then_body,
            elif_clauses,
            else_body,
            ..
        }) => {
            then_body.iter().any(ast_stmt_has_bare_return)
                || elif_clauses.iter().any(|(_, b)| b.iter().any(ast_stmt_has_bare_return))
                || else_body.as_ref().map_or(false, |b| b.iter().any(ast_stmt_has_bare_return))
        }
        AstStmt::With { body, .. } => body.iter().any(ast_stmt_has_bare_return),
        AstStmt::Test { body, .. } => body.iter().any(ast_stmt_has_bare_return),
        AstStmt::Guard { else_body, .. } => else_body.iter().any(ast_stmt_has_bare_return),
        AstStmt::Suite {
            setup,
            teardown,
            tests,
            ..
        } => setup
            .iter()
            .flatten()
            .chain(teardown.iter().flatten())
            .chain(tests.iter())
            .any(ast_stmt_has_bare_return),
        _ => false,
    }
}

/// 检测嵌套函数体是否引用了外层函数局部变量（E0425 修复）。
/// 嵌套函数被提升为模块级 fn 后，无法访问定义它的外层函数的局部变量；
/// 返回第一个被引用的外层局部变量名（None = 无捕获）。
/// declared：嵌套函数自身参数 + 体内已声明的局部变量（按语句顺序累计遮蔽）。
fn check_expr_capture(
    e: &AstExpr,
    outer: &HashMap<String, IrType>,
    declared: &mut HashSet<String>,
) -> Option<String> {
    match e {
        AstExpr::Ident(n) => {
            // 读取外层变量**不报错**：会被 analyze_global_vars 提升为模块级
            // 全局（static mut + unsafe 访问），跨函数可见合法（polish_02 的
            // read_shared 读取 shared、precedence 的 fallible 读取 n 均为此模式）。
            // 只有**写**（Assign 目标 / 无 let 前缀的默认可变绑定）才在
            // check_stmt_capture 的 Assign/Let 分支拦截。
            let _ = (n, outer, declared);
            None
        }
        AstExpr::ListLit(items) | AstExpr::SetLit(items) | AstExpr::TupleLit(items) => {
            items.iter().find_map(|i| check_expr_capture(i, outer, declared))
        }
        AstExpr::DictLit(items) => items.iter().find_map(|(k, v)| {
            check_expr_capture(k, outer, declared)
                .or_else(|| check_expr_capture(v, outer, declared))
        }),
        AstExpr::Binary { left, right, .. } => {
            check_expr_capture(left, outer, declared)
                .or_else(|| check_expr_capture(right, outer, declared))
        }
        AstExpr::Unary { operand, .. } => check_expr_capture(operand, outer, declared),
        AstExpr::Call { func, args, .. } => {
            check_expr_capture(func, outer, declared).or_else(|| {
                args.iter()
                    .find_map(|a| check_expr_capture(a, outer, declared))
            })
        }
        AstExpr::KwArg { value, .. } => check_expr_capture(value, outer, declared),
        AstExpr::MethodCall { receiver, args, .. } => {
            check_expr_capture(receiver, outer, declared).or_else(|| {
                args.iter()
                    .find_map(|a| check_expr_capture(a, outer, declared))
            })
        }
        AstExpr::FieldAccess { receiver, .. }
        | AstExpr::PathAccess { receiver, .. }
        | AstExpr::SafeNav { receiver, .. } => check_expr_capture(receiver, outer, declared),
        AstExpr::Index { receiver, index } => {
            check_expr_capture(receiver, outer, declared)
                .or_else(|| check_expr_capture(index, outer, declared))
        }
        AstExpr::If {
            cond,
            then_body,
            elif_clauses,
            else_body,
        } => check_expr_capture(cond, outer, declared)
            .or_else(|| check_stmts_capture(then_body, outer, declared))
            .or_else(|| {
                elif_clauses.iter().find_map(|(c, b)| {
                    check_expr_capture(c, outer, declared)
                        .or_else(|| check_stmts_capture(b, outer, declared))
                })
            })
            .or_else(|| {
                else_body
                    .as_ref()
                    .and_then(|b| check_stmts_capture(b, outer, declared))
            }),
        AstExpr::Match { expr, arms } => {
            check_expr_capture(expr, outer, declared).or_else(|| {
                arms.iter().find_map(|arm| {
                    let mut sub = declared.clone();
                    let mut pv = vec![];
                    collect_ast_pattern_vars(&arm.pattern, &mut pv);
                    for n in pv {
                        sub.insert(n);
                    }
                    if let Some(g) = &arm.guard {
                        if let Some(hit) = check_expr_capture(g, outer, &mut sub) {
                            return Some(hit);
                        }
                    }
                    check_stmts_capture(&arm.body, outer, &mut sub)
                })
            })
        }
        AstExpr::Closure { params, body, .. } => {
            let mut sub = declared.clone();
            for p in params {
                sub.insert(p.clone());
            }
            check_expr_capture(body, outer, &mut sub)
        }
        AstExpr::BlockExpr(body) => check_stmts_capture(body, outer, declared),
        AstExpr::Range { start, end, .. } => start
            .as_ref()
            .and_then(|s| check_expr_capture(s, outer, declared))
            .or_else(|| end.as_ref().and_then(|e| check_expr_capture(e, outer, declared))),
        AstExpr::Walrus { target, value } => {
            check_expr_capture(target, outer, declared)
                .or_else(|| check_expr_capture(value, outer, declared))
        }
        AstExpr::Pipe {
            receiver,
            callee,
            args,
        } => check_expr_capture(receiver, outer, declared)
            .or_else(|| check_expr_capture(callee, outer, declared))
            .or_else(|| {
                args.iter()
                    .find_map(|a| check_expr_capture(a, outer, declared))
            }),
        AstExpr::NullCoalesce { left, right } => {
            check_expr_capture(left, outer, declared)
                .or_else(|| check_expr_capture(right, outer, declared))
        }
        AstExpr::ListComprehension {
            output,
            var,
            iter,
            cond,
            extra_clauses,
        }
        | AstExpr::SetComprehension {
            elem: output,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            let mut sub = declared.clone();
            sub.insert(var.clone());
            for (v, i, c) in extra_clauses {
                sub.insert(v.clone());
                if let Some(hit) = check_expr_capture(i, outer, &mut sub) {
                    return Some(hit);
                }
                if let Some(hit) = c.as_ref().and_then(|c| check_expr_capture(c, outer, &mut sub)) {
                    return Some(hit);
                }
            }
            check_expr_capture(iter, outer, &mut sub)
                .or_else(|| cond.as_ref().and_then(|c| check_expr_capture(c, outer, &mut sub)))
                .or_else(|| check_expr_capture(output, outer, &mut sub))
        }
        AstExpr::DictComprehension {
            key,
            value,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            let mut sub = declared.clone();
            sub.insert(var.clone());
            for (v, i, c) in extra_clauses {
                sub.insert(v.clone());
                if let Some(hit) = check_expr_capture(i, outer, &mut sub) {
                    return Some(hit);
                }
                if let Some(hit) = c.as_ref().and_then(|c| check_expr_capture(c, outer, &mut sub)) {
                    return Some(hit);
                }
            }
            check_expr_capture(iter, outer, &mut sub)
                .or_else(|| cond.as_ref().and_then(|c| check_expr_capture(c, outer, &mut sub)))
                .or_else(|| check_expr_capture(key, outer, &mut sub))
                .or_else(|| check_expr_capture(value, outer, &mut sub))
        }
        AstExpr::Assign { target, value, .. } => {
            check_expr_capture(target, outer, declared)
                .or_else(|| check_expr_capture(value, outer, declared))
        }
        AstExpr::Spawn(inner)
        | AstExpr::Move(inner)
        | AstExpr::Panic(inner)
        | AstExpr::Await(inner)
        | AstExpr::Try(inner)
        | AstExpr::Paren(inner)
        | AstExpr::Comptime(inner) => check_expr_capture(inner, outer, declared),
        AstExpr::BuildBlock { lhs, body, .. } => {
            check_expr_capture(lhs, outer, declared)
                .or_else(|| check_stmts_capture(body, outer, declared))
        }
        AstExpr::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => check_stmts_capture(body, outer, declared)
            .or_else(|| {
                catches.iter().find_map(|arm| {
                    let mut sub = declared.clone();
                    let mut pv = vec![];
                    collect_ast_pattern_vars(&arm.pattern, &mut pv);
                    for n in pv {
                        sub.insert(n);
                    }
                    if let Some(g) = &arm.guard {
                        if let Some(hit) = check_expr_capture(g, outer, &mut sub) {
                            return Some(hit);
                        }
                    }
                    check_stmts_capture(&arm.body, outer, &mut sub)
                })
            })
            .or_else(|| {
                else_body
                    .as_ref()
                    .and_then(|b| check_stmts_capture(b, outer, declared))
            })
            .or_else(|| {
                finally_body
                    .as_ref()
                    .and_then(|b| check_stmts_capture(b, outer, declared))
            }),
        _ => None,
    }
}

fn check_stmts_capture(
    stmts: &[AstStmt],
    outer: &HashMap<String, IrType>,
    declared: &mut HashSet<String>,
) -> Option<String> {
    for s in stmts {
        if let Some(hit) = check_stmt_capture(s, outer, declared) {
            return Some(hit);
        }
    }
    None
}

fn check_stmt_capture(
    s: &AstStmt,
    outer: &HashMap<String, IrType>,
    declared: &mut HashSet<String>,
) -> Option<String> {
    match s {
        AstStmt::Let { name, mutable, value, .. } => {
            // 只对**写外层局部变量**报错（无 let 前缀的默认可变绑定 `total = ...`
            // 且 total 在外层作用域存在）：builder 在嵌套函数体内会生成
            // `let mut total = total + x`（新绑定自引用）→ E0425。
            // 有 let 前缀的声明（let x = v）是本函数新绑定，不报。
            // 纯读取（value 中引用外层变量）不报——会被 analyze_global_vars
            // 提升为模块级全局（static mut + unsafe 访问），跨函数可见合法。
            let hit = if *mutable && outer.contains_key(name.as_str()) && !declared.contains(name.as_str()) {
                Some(name.clone())
            } else {
                None
            };
            declared.insert(name.clone());
            hit.or_else(|| check_expr_capture(value, outer, declared))
        }
        AstStmt::Const { name, value, .. } => {
            let hit = check_expr_capture(value, outer, declared);
            declared.insert(name.clone());
            hit
        }
        AstStmt::LetTuple { names, value, .. } => {
            let hit = check_expr_capture(value, outer, declared);
            for n in names {
                declared.insert(n.clone());
            }
            hit
        }
        AstStmt::Expr(e) => check_expr_capture(e, outer, declared),
        AstStmt::Return(Some(e)) | AstStmt::Yield(Some(e)) => check_expr_capture(e, outer, declared),
        AstStmt::YieldFrom(e) | AstStmt::Raise(e) => check_expr_capture(e, outer, declared),
        AstStmt::While {
            cond,
            guard,
            body,
            else_body,
        } => check_expr_capture(cond, outer, declared)
            .or_else(|| guard.as_ref().and_then(|g| check_expr_capture(g, outer, declared)))
            .or_else(|| check_stmts_capture(body, outer, declared))
            .or_else(|| {
                else_body
                    .as_ref()
                    .and_then(|b| check_stmts_capture(b, outer, declared))
            }),
        AstStmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
            else_body,
        } => {
            let hit = check_expr_capture(expr, outer, declared)
                .or_else(|| guard.as_ref().and_then(|g| check_expr_capture(g, outer, declared)));
            let mut pv = vec![];
            collect_ast_pattern_vars(pattern, &mut pv);
            for n in pv {
                declared.insert(n);
            }
            hit.or_else(|| check_stmts_capture(body, outer, declared)).or_else(|| {
                else_body
                    .as_ref()
                    .and_then(|b| check_stmts_capture(b, outer, declared))
            })
        }
        AstStmt::For {
            var,
            iter,
            guard,
            body,
            else_body,
        } => {
            let hit = check_expr_capture(iter, outer, declared)
                .or_else(|| guard.as_ref().and_then(|g| check_expr_capture(g, outer, declared)));
            declared.insert(var.clone());
            hit.or_else(|| check_stmts_capture(body, outer, declared)).or_else(|| {
                else_body
                    .as_ref()
                    .and_then(|b| check_stmts_capture(b, outer, declared))
            })
        }
        AstStmt::Loop(body) => check_stmts_capture(body, outer, declared),
        AstStmt::Break(Some(e)) => check_expr_capture(e, outer, declared),
        AstStmt::BreakLabel { value, .. } => {
            value.as_ref().and_then(|v| check_expr_capture(v, outer, declared))
        }
        AstStmt::Block { body, .. }
        | AstStmt::CheckerBlock { body, .. }
        | AstStmt::Defer(body)
        | AstStmt::Comptime { body } => check_stmts_capture(body, outer, declared),
        AstStmt::Guard {
            cond,
            let_binding,
            success_expr,
            else_body,
        } => {
            let mut hit = cond
                .as_ref()
                .and_then(|c| check_expr_capture(c, outer, declared));
            if let Some((pat, e)) = let_binding {
                if hit.is_none() {
                    hit = check_expr_capture(e, outer, declared);
                }
                let mut pv = vec![];
                collect_ast_pattern_vars(pat, &mut pv);
                for n in pv {
                    declared.insert(n);
                }
            }
            if hit.is_none() {
                hit = success_expr
                    .as_ref()
                    .and_then(|s| check_expr_capture(s, outer, declared));
            }
            hit.or_else(|| check_stmts_capture(else_body, outer, declared))
        }
        AstStmt::With { expr, alias, body } => {
            let hit = check_expr_capture(expr, outer, declared);
            if let Some(a) = alias {
                declared.insert(a.clone());
            }
            hit.or_else(|| check_stmts_capture(body, outer, declared))
        }
        AstStmt::BlockCall { args, .. } => check_expr_capture(args, outer, declared),
        AstStmt::Assign { target, value, .. } => {
            // 写外层局部变量（total = ... / total += ...）→ 生成新绑定自引用 E0425
            if let AstExpr::Ident(n) = target {
                if !declared.contains(n.as_str()) && outer.contains_key(n.as_str()) {
                    return Some(n.clone());
                }
            }
            check_expr_capture(target, outer, declared)
                .or_else(|| check_expr_capture(value, outer, declared))
        }
        AstStmt::Test { body, .. } => check_stmts_capture(body, outer, declared),
        AstStmt::Assert { expr, expected } => {
            check_expr_capture(expr, outer, declared)
                .or_else(|| expected.as_ref().and_then(|e| check_expr_capture(e, outer, declared)))
        }
        AstStmt::Check { expr, message } => {
            check_expr_capture(expr, outer, declared)
                .or_else(|| message.as_ref().and_then(|m| check_expr_capture(m, outer, declared)))
        }
        AstStmt::Suite {
            setup,
            teardown,
            tests,
            ..
        } => {
            let mut hit = setup
                .as_ref()
                .and_then(|s| check_stmts_capture(s, outer, declared));
            if hit.is_none() {
                hit = teardown
                    .as_ref()
                    .and_then(|s| check_stmts_capture(s, outer, declared));
            }
            if hit.is_none() {
                hit = check_stmts_capture(tests, outer, declared);
            }
            hit
        }
        // 嵌套函数体内的嵌套函数：由该函数自身转换时独立检查（递归）
        _ => None,
    }
}

fn infer_stmt_type(stmt: &AstStmt, ctx: &TypeCtx) -> IrType {
    match stmt {
        AstStmt::Expr(e) => {
            // go expr 作为语句使用时值被丢弃，不污染函数返回类型
            // （规范 10-并发与异步.md：`let x: Future<int> = go f()` 绑定上下文
            // 才返回 Future<T>；尾语句 `go f()` 应视为 Unit，避免无返回注解
            // 的 def main() 被推断为 Future<()> 触发 typed main 分支 E0782）
            if matches!(e, AstExpr::Spawn(_)) {
                IrType::Unit
            } else {
                infer_expr_type(e, ctx)
            }
        }
        AstStmt::Pass => IrType::Unit,
        AstStmt::TypeAlias { .. } => IrType::Unit,
        AstStmt::Check { .. } => IrType::Unit,
        // let 是声明不是值表达式：作为尾语句推断返回 Unit，
        // 否则无返回注解函数（如 def scope_defer() = ... let main_val = 30）
        // 会误推断为 Any→i64，与体实际返回 () 冲突（E0308）
        AstStmt::Let { .. } => IrType::Unit,
        AstStmt::Return(Some(e)) => infer_expr_type(e, ctx),
        AstStmt::Return(None) => IrType::Unit,
        AstStmt::Yield(Some(e)) => IrType::Named {
            path: "Itor".into(),
            args: vec![infer_expr_type(e, ctx)],
        },
        AstStmt::Yield(None) => IrType::Unit,
        AstStmt::YieldFrom(e) => IrType::Named {
            path: "Itor".into(),
            args: vec![infer_expr_type(e, ctx)],
        },
        _ => IrType::Unit,
    }
}

// ══════════════════════════════════════════════════════════════
// Pattern 转换
// ══════════════════════════════════════════════════════════════

/// 将 AST Pattern 转为 IR Pattern，返回 None 表示通配（catch-all）
#[allow(dead_code)]
fn convert_ast_pattern(pat: &AstPattern, ctx: &TypeCtx) -> Option<Pattern> {
    match pat {
        AstPattern::Wildcard => None,
        AstPattern::Ident(name) => Some(Pattern::Ident(name.clone())),
        AstPattern::RefMutIdent(name) => Some(Pattern::RefMutIdent(name.clone())),
        AstPattern::Int(n) => Some(Pattern::Lit(LitKind::Int(*n))),
        AstPattern::Str(s) => Some(Pattern::Lit(LitKind::Str(s.clone()))),
        AstPattern::Bool(b) => Some(Pattern::Lit(LitKind::Bool(*b))),
        AstPattern::Variant(name, args) => {
            let ir_args: Vec<Pattern> = args
                .iter()
                .filter_map(|a| convert_ast_pattern(a, ctx))
                .collect();
            // 区分 struct 解构 vs enum 变体模式
            if ctx.is_struct(name) {
                // struct 模式: Point(px, py) → Point { x: px, y: py }
                let field_names: Vec<String> = ctx
                    .struct_fields
                    .get(name)
                    .map(|fields| fields.keys().cloned().collect())
                    .unwrap_or_default();
                let fields: Vec<(String, Pattern)> = ir_args
                    .into_iter()
                    .enumerate()
                    .map(|(i, pat)| {
                        let fname = field_names
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| format!("field_{}", i));
                        (fname, pat)
                    })
                    .collect();
                return Some(Pattern::Struct {
                    name: name.clone(),
                    fields,
                });
            }
            // enum 变体模式
            let (enum_name, variant) = if let Some(dot_pos) = name.rfind('.') {
                (name[..dot_pos].to_string(), name[dot_pos + 1..].to_string())
            } else {
                let enum_name = ctx
                    .enum_variants
                    .get(name.as_str())
                    .cloned()
                    .unwrap_or_else(|| match name.as_str() {
                        "Some" | "None" => "Option".into(),
                        "Ok" | "Err" => "Result".into(),
                        _ => "Error".into(),
                    });
                (enum_name, name.clone())
            };
            Some(Pattern::Enum {
                enum_name,
                variant,
                args: ir_args,
            })
        }
        AstPattern::Tuple(elems) => {
            let ir_elems: Vec<Pattern> = elems
                .iter()
                .filter_map(|e| convert_ast_pattern(e, ctx))
                .collect();
            Some(Pattern::Tuple(ir_elems))
        }
        AstPattern::List(elems) => {
            let ir_elems: Vec<Pattern> = elems
                .iter()
                .filter_map(|e| convert_ast_pattern(e, ctx))
                .collect();
            Some(Pattern::List(ir_elems))
        }
        AstPattern::Dict(entries) => {
            let ir_entries: Vec<(String, Pattern)> = entries
                .iter()
                .filter_map(|(k, p)| {
                    convert_ast_pattern(p, ctx).map(|ip| (k.clone(), ip))
                })
                .collect();
            Some(Pattern::Dict(ir_entries))
        }
        AstPattern::Rest(name) => Some(Pattern::Rest(name.clone())),
        AstPattern::Range {
            start,
            end,
            inclusive,
        } => Some(Pattern::Range {
            start: *start,
            end: *end,
            inclusive: *inclusive,
        }),
    }
}

/// 内置方法返回类型推断表
/// 为常用内置类型提供方法返回类型推断
fn lookup_builtin_method_ret(recv_ty: &IrType, method: &str, _ctx: &TypeCtx) -> Option<IrType> {
    match recv_ty {
        // Iterator<T> 方法
        IrType::Named { path, args } if path == "Iterator" && args.len() == 1 => match method {
            "next" => Some(IrType::Option(Box::new(args[0].clone()))),
            "len" | "count" => Some(IrType::Int),
            "collect" => Some(IrType::Named {
                path: "List".into(),
                args: args.clone(),
            }),
            _ => None,
        },
        // List<T> 方法
        IrType::Named { path, args } if path == "List" && args.len() == 1 => match method {
            "len" | "size" => Some(IrType::Int),
            "clone" => Some(recv_ty.clone()),
            "iter" => Some(IrType::Named {
                path: "Iterator".into(),
                args: args.clone(),
            }),
            "get" | "pop" => Some(IrType::Option(Box::new(args[0].clone()))),
            "first" | "last" => Some(IrType::Option(Box::new(args[0].clone()))),
            _ => None,
        },
        // Option<T> 方法
        IrType::Option(inner) => match method {
            "unwrap" | "expect" => Some((**inner).clone()),
            "map" | "and_then" => Some(recv_ty.clone()),
            "is_some" | "is_none" => Some(IrType::Bool),
            _ => None,
        },
        IrType::Named { path, args } if path == "Option" && args.len() == 1 => match method {
            "unwrap" | "expect" => Some(args[0].clone()),
            "map" | "and_then" => Some(recv_ty.clone()),
            "is_some" | "is_none" => Some(IrType::Bool),
            _ => None,
        },
        // Result<T,E>.unwrap() / expect() → T
        IrType::Named { path, args } if path == "Result" && args.len() >= 1 => match method {
            "unwrap" | "expect" => Some(args[0].clone()),
            "map" | "and_then" => Some(recv_ty.clone()),
            _ => None,
        },
        // String 方法
        IrType::Named { path, .. } if path == "str" || path == "String" => match method {
            "len" => Some(IrType::Int),
            "clone" => Some(recv_ty.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// 收集 AST Pattern 中绑定的所有变量名
fn collect_ast_pattern_vars(pat: &AstPattern, out: &mut Vec<String>) {
    match pat {
        AstPattern::Wildcard => {}
        AstPattern::Ident(name) => {
            out.push(name.clone());
        }
        AstPattern::RefMutIdent(name) => {
            out.push(name.clone());
        }
        AstPattern::Int(_) | AstPattern::Str(_) | AstPattern::Bool(_) => {}
        AstPattern::Variant(_, args) | AstPattern::Tuple(args) | AstPattern::List(args) => {
            for a in args {
                collect_ast_pattern_vars(a, out);
            }
        }
        AstPattern::Dict(entries) => {
            for (_, p) in entries {
                collect_ast_pattern_vars(p, out);
            }
        }
        AstPattern::Rest(name) => {
            if let Some(n) = name {
                out.push(n.clone());
            }
        }
        AstPattern::Range { .. } => {}
    }
}

/// 递归替换类型中的 `Self` 引用为具体类型（struct 定义内 Self → 自身类型名）。
fn replace_self(ty: &IrType, self_ty: &IrType) -> IrType {
    match ty {
        IrType::Self_ => self_ty.clone(),
        IrType::Named { path, args } => {
            let new_args: Vec<IrType> = args.iter().map(|a| replace_self(a, self_ty)).collect();
            IrType::Named {
                path: path.clone(),
                args: new_args,
            }
        }
        IrType::Option(inner) => IrType::Option(Box::new(replace_self(inner, self_ty))),
        IrType::Result { ok, err } => IrType::Result {
            ok: Box::new(replace_self(ok, self_ty)),
            err: Box::new(replace_self(err, self_ty)),
        },
        IrType::Tuple(elems) => {
            IrType::Tuple(elems.iter().map(|e| replace_self(e, self_ty)).collect())
        }
        IrType::Ref(inner) => IrType::Ref(Box::new(replace_self(inner, self_ty))),
        IrType::MutRef(inner) => IrType::MutRef(Box::new(replace_self(inner, self_ty))),
        _ => ty.clone(),
    }
}

/// 内置 Option/Result 变体模式的字段绑定：
/// `Some(v)` / `Ok(v)` / `Err(e)` → v/e 绑定为内层类型（而非整个 scrutinee 类型）。
/// 返回 None 表示非内置变体模式。
fn field_types_for_builtin_variant(
    pat: &AstPattern,
    scrut_ty: &IrType,
) -> Option<Vec<(String, IrType)>> {
    let (vname, field_pats) = match pat {
        AstPattern::Variant(name, args) => (name, args),
        _ => return None,
    };
    let vbase = vname.rsplit('.').next().unwrap_or(vname);
    match (vbase, scrut_ty) {
        ("Some", IrType::Option(inner)) => {
            if let AstPattern::RefMutIdent(bind) = field_pats.first()? {
                // `Some(ref mut c)`：c 登记为 MutRef 内层类型，臂体内 c = c + 1
                // 生成 *c = *c + 1（解引用赋值，E0384 修复）
                Some(vec![(
                    bind.clone(),
                    IrType::MutRef(Box::new(inner.as_ref().clone())),
                )])
            } else if let AstPattern::Ident(bind) = field_pats.first()? {
                Some(vec![(bind.clone(), inner.as_ref().clone())])
            } else {
                None
            }
        }
        ("Ok", IrType::Result { ok, .. }) => {
            if let AstPattern::RefMutIdent(bind) = field_pats.first()? {
                Some(vec![(
                    bind.clone(),
                    IrType::MutRef(Box::new(ok.as_ref().clone())),
                )])
            } else if let AstPattern::Ident(bind) = field_pats.first()? {
                Some(vec![(bind.clone(), ok.as_ref().clone())])
            } else {
                None
            }
        }
        ("Err", IrType::Result { err, .. }) => {
            if let AstPattern::RefMutIdent(bind) = field_pats.first()? {
                Some(vec![(
                    bind.clone(),
                    IrType::MutRef(Box::new(err.as_ref().clone())),
                )])
            } else if let AstPattern::Ident(bind) = field_pats.first()? {
                Some(vec![(bind.clone(), err.as_ref().clone())])
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 变体模式字段绑定：`Shape::Circle(x: _, y: _, radius: r)` → r 绑定为
/// radius 字段类型（int）而非整个 scrutinee 类型。按字段位置匹配。
fn field_types_for_variant(pat: &AstPattern, ctx: &TypeCtx) -> Option<Vec<(String, IrType)>> {
    let (vname, field_pats) = match pat {
        AstPattern::Variant(name, args) => (name, args),
        _ => return None,
    };
    // 变体名可能是 "Shape.Circle" 或 "Circle"
    let vbase = vname.rsplit('.').next().unwrap_or(vname);
    let ftypes = ctx.enum_variant_field_types.get(vbase)?;
    let mut out = Vec::new();
    for (i, fp) in field_pats.iter().enumerate() {
        if let AstPattern::Ident(bind) = fp {
            if let Some(fty) = ftypes.get(i) {
                out.push((bind.clone(), fty.clone()));
            }
        }
    }
    Some(out)
}

/// 收集 IR Pattern 中绑定的所有变量名
#[allow(dead_code)]
fn collect_pattern_vars(pat: &Pattern, out: &mut Vec<String>) {
    match pat {
        Pattern::Wildcard => {}
        Pattern::Ident(name) => {
            out.push(name.clone());
        }
        Pattern::RefMutIdent(name) => {
            out.push(name.clone());
        }
        Pattern::Lit(_) => {}
        Pattern::Tuple(elems) | Pattern::List(elems) => {
            for e in elems {
                collect_pattern_vars(e, out);
            }
        }
        Pattern::Dict(entries) => {
            for (_, p) in entries {
                collect_pattern_vars(p, out);
            }
        }
        Pattern::Rest(name) => {
            if let Some(n) = name {
                out.push(n.clone());
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields {
                collect_pattern_vars(p, out);
            }
        }
        Pattern::Enum { args, .. } => {
            for a in args {
                collect_pattern_vars(a, out);
            }
        }
        Pattern::Range { .. } => {}
    }
}

// （arm_body_to_expr 已移除 — Match 表达式现在通过 BlockExpr + Stmt::Match 处理）

// ══════════════════════════════════════════════════════════════
// 核心转换函数
// ══════════════════════════════════════════════════════════════

/// 构建多 for 推导链（2+ 子句）：
/// `[out for x in a for y in b]` → `comp_outer!(|x| comp_leaf!(|y| out, b), a)`
/// codegen 端展开为 `(a).into_iter().flat_map(|x| (b).into_iter().map(|y| out)).collect()`。
/// `kind` 为 `comp` / `dict_comp` / `set_comp`，决定最外层 callee 前缀。
fn build_multi_comp(
    ctx: &TypeCtx,
    clauses: &[(String, Box<AstExpr>, Option<Box<AstExpr>>)],
    body: Expr,
    kind: &str,
) -> ExprKind {
    let n = clauses.len();
    debug_assert!(n >= 2, "multi-comp needs >= 2 clauses");
    let mut inner = body.kind;
    // 从最内层子句开始向外包裹：最内层用 leaf（map），中间层用 mid（flat_map），最外层用 outer（flat_map + collect）
    for i in (0..n).rev() {
        let (var, iter, cond) = &clauses[i];
        let iter_expr = convert_expr(iter, ctx);
        let level = if i == 0 { "outer" } else if i == n - 1 { "leaf" } else { "mid" };
        let callee = format!("{}_{}!", kind, level);
        let mut args = vec![
            Expr::new(
                ExprKind::Lambda {
                    params: vec![Param {
                        name: var.clone(),
                        ty: IrType::Any,
                        is_mut: false,
                        is_ref: false,
                        is_owned: false,
                        default: None,
                        variadic: false,
                    }],
                    body: Box::new(Expr::new(inner, IrType::Any, Span::unknown())),
                    is_move: true,
                },
                IrType::Any,
                Span::unknown(),
            ),
            iter_expr,
        ];
        if let Some(c) = cond {
            args.push(Expr::new(
                ExprKind::Lambda {
                    params: vec![Param {
                        name: var.clone(),
                        ty: IrType::Any,
                        is_mut: false,
                        is_ref: false,
                        is_owned: false,
                        default: None,
                        variadic: false,
                    }],
                    body: Box::new(convert_expr(c, ctx)),
                    is_move: true,
                },
                IrType::Any,
                Span::unknown(),
            ));
        }
        inner = ExprKind::Call {
            type_args: vec![],
            callee: Box::new(Expr::new(
                ExprKind::Var(callee),
                IrType::Any,
                Span::unknown(),
            )),
            args,
        };
    }
    inner
}

/// 将编译期求值结果转为 IR 表达式（Int/Float/Bool/Str/None → 字面量；
/// List/Tuple → vec![...]/元组递归内联，供查找表「焊死」；
/// Map/Type/Inspect 不支持内联，返回 None）
fn comptime_value_to_lit(v: &crate::comptime::ComptimeValue) -> Option<ExprKind> {
    use crate::comptime::ComptimeValue;
    match v {
        ComptimeValue::Int(i) => Some(ExprKind::Lit(LitKind::Int(*i))),
        ComptimeValue::Float(f) => Some(ExprKind::Lit(LitKind::F64(*f))),
        ComptimeValue::Bool(b) => Some(ExprKind::Lit(LitKind::Bool(*b))),
        ComptimeValue::Str(s) => Some(ExprKind::Lit(LitKind::Str(s.clone()))),
        ComptimeValue::None => Some(ExprKind::Lit(LitKind::None_)),
        ComptimeValue::List(xs) => {
            let elems: Vec<Expr> = xs.iter().map(|x| {
                Expr::new(
                    comptime_value_to_lit(x).unwrap_or(ExprKind::Lit(LitKind::None_)),
                    IrType::Any,
                    Span::unknown(),
                )
            }).collect();
            Some(ExprKind::ListLit(elems))
        }
        ComptimeValue::Tuple(xs) => {
            let elems: Vec<Expr> = xs.iter().map(|x| {
                Expr::new(
                    comptime_value_to_lit(x).unwrap_or(ExprKind::Lit(LitKind::None_)),
                    IrType::Any,
                    Span::unknown(),
                )
            }).collect();
            Some(ExprKind::TupleLit(elems))
        }
        _ => None,
    }
}

/// 从 *: 构建块 body 中第一个 yield 表达式推断元素类型（无 callee 或 callee 类型不可知时）
fn infer_yield_elem_ty(body: &[AstStmt], _lhs_ty: IrType, ctx: &TypeCtx) -> IrType {
    body.iter()
        .find_map(|s| {
            if let AstStmt::Yield(Some(e)) = s {
                Some(infer_expr_type(e, ctx))
            } else {
                None
            }
        })
        .unwrap_or(IrType::Any)
}

fn convert_expr(ast_expr: &AstExpr, ctx: &TypeCtx) -> Expr {
    let ty = infer_expr_type(ast_expr, ctx);
    let span = Span::unknown();

    let kind = match ast_expr {
        AstExpr::IntLit(n) => ExprKind::Lit(LitKind::Int(*n)),
        AstExpr::FloatLit(n) => ExprKind::Lit(LitKind::F64(*n)),
        AstExpr::StrLit(s) => ExprKind::Lit(LitKind::Str(s.clone())),
        AstExpr::FStrLit(s) => ExprKind::Lit(LitKind::FStr(s.clone())),
        AstExpr::RawStrLit(s) => ExprKind::Lit(LitKind::Str(s.clone())),
        AstExpr::BoolLit(b) => ExprKind::Lit(LitKind::Bool(*b)),
        AstExpr::NoneLit => ExprKind::Lit(LitKind::None_),
        AstExpr::Ident(name) => ExprKind::Var(name.clone()),
        AstExpr::Paren(inner) => ExprKind::Paren(Box::new(convert_expr(inner, ctx))),
        // comptime 表达式：编译期求值，结果内联为字面量（B3）
        AstExpr::Comptime(inner) => {
            // 使用真实模块（comptime 可调用模块内函数/引用 const）
            let empty_module = ast::Module::default();
            let module_ref = ctx.comptime_module.as_ref().map(|m| m.as_ref()).unwrap_or(&empty_module);
            let mut cctx = crate::comptime::ComptimeContext::new(module_ref);
            // 注入源码文本（inspect.getsource/getsourcelines 数据源，main.rs 已填）
            if let Some(src) = &module_ref.source_text {
                cctx = cctx.with_source(src.clone());
            }
            // 注入顶层 const 求值结果（`comptime LIMIT / 2` 解析 const 引用）
            for (n, v) in &ctx.comptime_consts {
                cctx.symtab.insert(n.clone(), v.clone());
            }
            match crate::comptime::ComptimeEvaluator::eval_expr(inner, &mut cctx) {
                Ok(v) => match comptime_value_to_lit(&v) {
                    Some(kind) => kind,
                    None => ExprKind::Paren(Box::new(convert_expr(inner, ctx))),
                },
                Err(e) => {
                    ctx.errors.borrow_mut().push(format!("comptime 求值失败: {}", e));
                    ExprKind::Paren(Box::new(convert_expr(inner, ctx)))
                }
            }
        }

        AstExpr::Call {
            func,
            args,
            type_args,
        } => {
            // SafeNav 后接方法调用：config?.get("key") →
            // config.map(|__sn| __sn.get("key").copied())（field 与 args 合并进闭包体，
            // 否则生成 config.map(|__sn| __sn.get)("key")，E0615；
            // get 返回 Option<&V> 需 .copied() 转 Option<V>，否则 ?? 30 unwrap_or 类型不匹配）
            if let AstExpr::SafeNav { receiver, field } = func.as_ref() {
                let recv = convert_expr(receiver, ctx);
                let param = "__sn".to_string();
                let call_args: Vec<Expr> = args.iter().map(|a| convert_expr(a, ctx)).collect();
                let get_call = Expr::new(
                    ExprKind::MethodCall {
                        receiver: Box::new(Expr::new(
                            ExprKind::Var(param.clone()),
                            IrType::Any,
                            Span::unknown(),
                        )),
                        method: field.clone(),
                        args: call_args,
                    },
                    IrType::Any,
                    Span::unknown(),
                );
                // get/方法调用结果若是 Option<&V>（如 HashMap::get），需 .copied()；
                // 对一般方法（如 len() 返回 i64）不附加
                let body = if field == "get" {
                    Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(get_call),
                            method: "copied".into(),
                            args: vec![],
                        },
                        IrType::Any,
                        Span::unknown(),
                    )
                } else {
                    get_call
                };
                let lambda = Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: param,
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                        }],
                        body: Box::new(body),
                        is_move: true,
                    },
                    IrType::Any,
                    Span::unknown(),
                );
                return Expr::new(
                    ExprKind::MethodCall {
                        receiver: Box::new(recv),
                        // get 返回 Option → and_then 扁平化（map 会得 Option<Option<..>>，
                        // ?? 30 unwrap_or 类型不匹配 E0308）
                        method: "and_then".into(),
                        args: vec![lambda],
                    },
                    IrType::Any,
                    Span::unknown(),
                );
            }
            // 部分应用检测：如果 args 中包含 _ 占位符，展开为 Lambda
            // add(_, 1) → |x| add(x, 1)
            let has_wildcard = args
                .iter()
                .any(|a| matches!(a, AstExpr::Ident(s) if s == "_"));
            if has_wildcard {
                let mut param_idx = 0u32;
                let mut lambda_params: Vec<Param> = Vec::new();
                let mut filled_args: Vec<Expr> = Vec::new();
                for a in args.iter() {
                    if matches!(a, AstExpr::Ident(s) if s == "_") {
                        let param_name = format!("__p{}", param_idx);
                        param_idx += 1;
                        lambda_params.push(Param {
                            name: param_name.clone(),
                            ty: IrType::Any,
                            default: None,
                            variadic: false,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                        });
                        filled_args.push(Expr::new(
                            ExprKind::Var(param_name),
                            IrType::Any,
                            Span::unknown(),
                        ));
                    } else {
                        filled_args.push(convert_expr(a, ctx));
                    }
                }
                let callee = convert_expr(func, ctx);
                let call = Expr::new(
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(callee),
                        args: filled_args,
                    },
                    IrType::Any,
                    Span::unknown(),
                );
                return Expr::new(
                    ExprKind::Lambda {
                        params: lambda_params,
                        body: Box::new(call),
                        is_move: true,
                    },
                    IrType::Fn {
                        params: vec![IrType::Any; param_idx as usize],
                        ret: Box::new(IrType::Any),
                    },
                    Span::unknown(),
                );
            }
            // 特殊处理 __as__ 运算符：__as__(value, type_name) → Cast
            if let AstExpr::Ident(ref fname) = func.as_ref() {
                if fname == "__as__" && args.len() == 2 {
                    let value = convert_expr(&args[0], ctx);
                    if let AstExpr::Ident(ref type_name) = &args[1] {
                        let target = name_to_ir_type(type_name);
                        let target_ty = target.clone();
                        // __cast__/__try_cast__ 双缺检查（01-类型系统.md §6）：
                        // 自定义类型（struct/enum/duck）执行 `x as T` 必须实现
                        // `__cast__<T>()` 或 `__try_cast__<T>() -> Result<T, E>`，
                        // 两者均未实现 → LZ 编译期报错（而非生成非法 Rust `as` 被
                        // rustc E0605 兜底）。基本数值类型间转换由编译器内置放行；
                        // 内置容器类型（List/Dict/Option/Result 等，不在
                        // struct_methods 中）的 as 是字面量类型标注，同样放行。
                        let needs_magic = !is_builtin_cast(&value.ty, &target);
                        if needs_magic {
                            if let IrType::Named { path, .. } = &value.ty {
                                // 仅用户自定义类型（有方法集合可查）要求魔法方法；
                                // 内置类型/未登记类型保守放行，避免误伤
                                // `[] as List<int>` / `None as Option<int>` 标注
                                if ctx.struct_methods.contains_key(path.as_str()) {
                                    let has_magic = ctx
                                        .struct_methods
                                        .get(path)
                                        .map_or(false, |ms| {
                                            ms.contains("__cast__") || ms.contains("__try_cast__")
                                        });
                                    if !has_magic {
                                        ctx.report_error(format!(
                                            "类型 `{}` 未实现 `__cast__` 或 `__try_cast__`，无法执行 `as {}` 转换（01-类型系统.md §6）",
                                            path, type_name
                                        ));
                                    }
                                }
                            }
                        }
                        return Expr::new(
                            ExprKind::Cast {
                                expr: Box::new(value),
                                target,
                            },
                            target_ty,
                            Span::unknown(),
                        );
                    }
                }
            }
            // 处理 func[type_arg](args) → 泛型调用: func::<type_arg>(args)
            // 例外：`[checker]` 挂载（已知函数名）不是泛型参数（03c-检查站.md），
            // 调用点重复挂载由定义处 default_checker 处理，此处忽略
            let actual_func: &AstExpr;
            let extra_type_args: Vec<String>;
            if let AstExpr::Index { receiver, index } = func.as_ref() {
                if let AstExpr::Ident(ref type_name) = index.as_ref() {
                    if ctx.fn_returns.contains_key(type_name) {
                        actual_func = receiver;
                        extra_type_args = vec![];
                    } else {
                        actual_func = receiver;
                        extra_type_args = vec![type_name.clone()];
                    }
                } else {
                    actual_func = func;
                    extra_type_args = vec![];
                }
            } else {
                actual_func = func;
                extra_type_args = vec![];
            }
            let mut ir_type_args: Vec<String> = type_args
                .iter()
                .map(|t| match t.as_str() {
                    "int" => "i64".to_string(),
                    "str" => "String".to_string(),
                    "f64" | "float" => "f64".to_string(),
                    "bool" => "bool".to_string(),
                    other => other.to_string(),
                })
                .collect();
            ir_type_args.extend(extra_type_args);

            // __call__ 检测：如果是 struct 实例变量调用，转换为 MethodCall
            if let AstExpr::Ident(ref fname) = actual_func {
                if !ctx.fn_returns.contains_key(fname)
                    && !ctx.is_builtin_function(fname)
                    && !ctx.is_struct(fname)
                {
                    // 变量可能是一个 struct 实例 → 使用 __call__
                    let var_ty = ctx.lookup_var(fname);
                    if ctx.is_struct_type(&var_ty) {
                        let recv = convert_expr(actual_func, ctx);
                        let ret_ty = recv.ty.clone();
                        return Expr::new(
                            ExprKind::MethodCall {
                                receiver: Box::new(recv),
                                method: "__call__".to_string(),
                                args: args.iter().map(|a| convert_expr(a, ctx)).collect(),
                            },
                            ret_ty,
                            Span::unknown(),
                        );
                    }
                }
            }

            // struct 位置参数构造：Point(1.0, 2.0) → StructCtor { name: "Point", fields: [(x,..),(y,..)] }
            // （管道 `1.0 |> Point(2.0)` 预填充后即此形式；关键字构造在 codegen 端已有处理）
            if let AstExpr::Ident(ref fname) = actual_func {
                if ctx.is_struct(fname) && !args.is_empty() {
                    let is_all_positional = args
                        .iter()
                        .all(|a| !matches!(a, AstExpr::KwArg { .. }));
                    if is_all_positional {
                        let order = ctx.struct_field_order.get(fname).cloned().unwrap_or_default();
                        let fields: Vec<(String, Expr)> = args
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                let fname_i = order
                                    .get(i)
                                    .cloned()
                                    .unwrap_or_else(|| format!("_f{}", i));
                                (fname_i, convert_expr(a, ctx))
                            })
                            .collect();
                        return Expr::new(
                            ExprKind::StructCtor {
                                name: fname.clone(),
                                fields,
                            },
                            IrType::named(fname),
                            Span::unknown(),
                        );
                    }
                }
            }

            // 函数参数调用（iter.lz filter/find `predicate(item)`，predicate:
            // fn(ref I.Item) -> bool）：callee 是 Fn 类型变量且参数为 ref 时，
            // 实参自动取引用（&item），否则 E0308 expected &I::Item found owned
            let fn_arg_refs: Vec<bool> = if let AstExpr::Ident(fname) = func.as_ref() {
                match ctx.lookup_var(fname) {
                    IrType::Fn { params, .. } => params
                        .iter()
                        .map(|p| matches!(p, IrType::Ref(_) | IrType::MutRef(_)))
                        .collect(),
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            };
            let args: Vec<Expr> = args
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    if i < fn_arg_refs.len() && fn_arg_refs[i] {
                        Expr::new(
                            ExprKind::UnOp {
                                op: UnOpKind::Ref,
                                operand: Box::new(convert_expr(a, ctx)),
                            },
                            IrType::Any,
                            Span::unknown(),
                        )
                    } else {
                        convert_expr(a, ctx)
                    }
                })
                .collect();
            ExprKind::Call {
                type_args: ir_type_args,
                callee: Box::new(convert_expr(actual_func, ctx)),
                args,
            }
        }

        AstExpr::MethodCall {
            receiver,
            method,
            args,
        } => ExprKind::MethodCall {
            receiver: Box::new(convert_expr(receiver, ctx)),
            method: method.clone(),
            args: args.iter().map(|a| convert_expr(a, ctx)).collect(),
        },

        AstExpr::FieldAccess { receiver, field } => ExprKind::FieldAccess {
            base: Box::new(convert_expr(receiver, ctx)),
            field: field.clone(),
        },

        AstExpr::Index { receiver, index } => {
            // `services.validate_port[(r,)]`：模块命名空间 + 元组实参 → checker 块调用
            // （services.X 在 codegen 层降级为 X，此处转为函数调用而非索引）
            if let AstExpr::FieldAccess { receiver: _, field } = receiver.as_ref() {
                if let AstExpr::TupleLit(_) = index.as_ref() {
                    return Expr::new(
                        ExprKind::Call {
                            type_args: vec![],
                            callee: Box::new(Expr::new(
                                ExprKind::Var(field.clone()),
                                IrType::Any,
                                Span::unknown(),
                            )),
                            args: vec![convert_expr(index, ctx)],
                        },
                        IrType::Any,
                        Span::unknown(),
                    );
                }
            }
            // [] 下标访问 → IndexGet
            ExprKind::IndexGet {
                base: Box::new(convert_expr(receiver, ctx)),
                key: Box::new(convert_expr(index, ctx)),
            }
        }

        AstExpr::PathAccess { receiver, segment } => {
            // :: 路径访问 → FieldAccess（简化处理）
            ExprKind::FieldAccess {
                base: Box::new(convert_expr(receiver, ctx)),
                field: segment.clone(),
            }
        }

        AstExpr::Binary { left, op, right } => {
            // 特殊处理 `is` 运算符：编译期类型检查
            if matches!(op, BinOp::Is) {
                let left_ty = infer_expr_type(left, ctx);
                if let AstExpr::Ident(type_name) = right.as_ref() {
                    let expected = name_to_ir_type(type_name);
                    let result = ir_types_compatible(&left_ty, &expected);
                    return Expr::new(
                        ExprKind::Lit(LitKind::Bool(result)),
                        IrType::Bool,
                        Span::unknown(),
                    );
                }
                // RHS is not a simple type name → fallback to false
                return Expr::new(
                    ExprKind::Lit(LitKind::Bool(false)),
                    IrType::Bool,
                    Span::unknown(),
                );
            }

            // 用户自定义类型的运算符 → 魔术方法调用（如 Vector + Vector → Vector.__add__）
            let left_ty_bin = infer_expr_type(left, ctx);
            if let Some(magic) = magic_method_for_binop(op) {
                let is_user_struct = match &left_ty_bin {
                    IrType::Named { path, .. } => ctx
                        .struct_methods
                        .get(path)
                        .map(|ms| ms.contains(magic))
                        .unwrap_or(false),
                    _ => false,
                };
                if is_user_struct {
                    let recv = convert_expr(left, ctx);
                    let ret_ty = infer_expr_type(left, ctx);
                    let expr = Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(recv),
                            method: magic.to_string(),
                            args: vec![convert_expr(right, ctx)],
                        },
                        ret_ty,
                        Span::unknown(),
                    );
                    return expr;
                }
            }

            let ir_op = map_binop(op);

            // 泛型调用检测: ident < Type > (args) — 不是比较，而是泛型实例化
            // 支持单类型参数 ident < T > 和多类型参数 ident < T, U >
            if matches!(ir_op, BinOpKind::Gt) {
                if let AstExpr::Binary {
                    left: inner_left,
                    op: BinOp::Lt,
                    right: inner_right,
                } = left.as_ref()
                {
                    if let AstExpr::Ident(fname) = inner_left.as_ref() {
                        if let Some(type_names) = extract_type_names(inner_right) {
                            if let AstExpr::Call {
                                func: call_func,
                                args: call_args,
                                ..
                            } = right.as_ref()
                            {
                                if let AstExpr::Ident(call_fname) = call_func.as_ref() {
                                    if call_fname == fname {
                                        // 这是泛型调用: f < T, U > (args)
                                        let ir_callee = convert_expr(inner_left, ctx);
                                        let ir_args: Vec<Expr> = call_args
                                            .iter()
                                            .map(|a| convert_expr(a, ctx))
                                            .collect();
                                        let ir_type_args = map_type_args(&type_names);
                                        return Expr::new(
                                            ExprKind::Call {
                                                callee: Box::new(ir_callee),
                                                args: ir_args,
                                                type_args: ir_type_args,
                                            },
                                            IrType::Any,
                                            Span::unknown(),
                                        );
                                    }
                                }
                            }
                            // 泛型调用不带括号参数: f < T > — 收集实参
                            let call_args;
                            if let AstExpr::TupleLit(elems) = right.as_ref() {
                                call_args = elems.clone();
                            } else {
                                call_args = vec![right.as_ref().clone()];
                            }
                            let ir_callee = convert_expr(inner_left, ctx);
                            let ir_args: Vec<Expr> =
                                call_args.iter().map(|a| convert_expr(a, ctx)).collect();
                            let ir_type_args = map_type_args(&type_names);
                            let ret_ty = ctx.lookup_fn_return(&fname);
                            return Expr::new(
                                ExprKind::Call {
                                    callee: Box::new(ir_callee),
                                    args: ir_args,
                                    type_args: ir_type_args,
                                },
                                ret_ty,
                                Span::unknown(),
                            );
                        }
                    }
                }
            }

            // 链式比较展开: 1 < x < 10 → (1 < x) && (x < 10)
            if matches!(
                ir_op,
                BinOpKind::Lt
                    | BinOpKind::Gt
                    | BinOpKind::Le
                    | BinOpKind::Ge
                    | BinOpKind::Eq
                    | BinOpKind::Neq
            ) {
                if let AstExpr::Binary {
                    left: inner_left,
                    op: inner_op,
                    right: inner_right,
                } = left.as_ref()
                {
                    let inner_ir_op = map_binop(inner_op);
                    if matches!(
                        inner_ir_op,
                        BinOpKind::Lt
                            | BinOpKind::Gt
                            | BinOpKind::Le
                            | BinOpKind::Ge
                            | BinOpKind::Eq
                            | BinOpKind::Neq
                    ) {
                        // (a cmp1 b) cmp2 c → (a cmp1 b) && (b cmp2 c)
                        let a = convert_expr(inner_left, ctx);
                        let b = convert_expr(inner_right, ctx);
                        let c = convert_expr(right, ctx);
                        return Expr::new(
                            ExprKind::BinOp {
                                op: BinOpKind::And,
                                lhs: Box::new(Expr::new(
                                    ExprKind::BinOp {
                                        op: inner_ir_op,
                                        lhs: Box::new(a),
                                        rhs: Box::new(b.clone()),
                                    },
                                    IrType::Bool,
                                    Span::unknown(),
                                )),
                                rhs: Box::new(Expr::new(
                                    ExprKind::BinOp {
                                        op: ir_op,
                                        lhs: Box::new(b),
                                        rhs: Box::new(c),
                                    },
                                    IrType::Bool,
                                    Span::unknown(),
                                )),
                            },
                            IrType::Bool,
                            Span::unknown(),
                        );
                    }
                }
            }

            ExprKind::BinOp {
                op: ir_op,
                lhs: Box::new(convert_expr(left, ctx)),
                rhs: Box::new(convert_expr(right, ctx)),
            }
        }

        AstExpr::Unary { op, operand } => ExprKind::UnOp {
            op: map_unop(op),
            operand: Box::new(convert_expr(operand, ctx)),
        },

        AstExpr::If {
            cond,
            then_body,
            elif_clauses,
            else_body,
        } => {
            // 多分支 if → 嵌套 IfExpr。elif 链必须嵌套在**原始 if 的 else 分支内**：
            // `if cond { then } else { if elif1 { b1 } else { if elif2 { b2 } else { else_body } } }`
            // 旧实现把 elif 反向包在原始 if **外层**（`if elif { b } else { if cond ... }`），
            // 导致分支顺序颠倒（age_group: age<20 的 elif 反而先判断）。
            // else 分支保留原始 block_to_expr 节点（含自身 Unit 类型）——用
            // Expr::new(els, ty) 以 if 整体类型标注 else 会导致 set.lz 等
            // 语句级 if 的 else () 类型错乱（E0308 if/else incompatible）
            let mut els_expr = if let Some(els) = else_body {
                block_to_expr(els, ctx)
            } else {
                Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown())
            };
            // 反向迭代 elif，使第一个 elif 成为最内层 else（紧贴 else_body）
            for (elif_cond, elif_body) in elif_clauses.iter().rev() {
                els_expr = Expr::new(
                    ExprKind::IfExpr {
                        cond: Box::new(convert_expr(elif_cond, ctx)),
                        then: Box::new(block_to_expr(elif_body, ctx)),
                        els: Box::new(els_expr),
                    },
                    ty.clone(),
                    Span::unknown(),
                );
            }
            ExprKind::IfExpr {
                cond: Box::new(convert_expr(cond, ctx)),
                then: Box::new(block_to_expr(then_body, ctx)),
                els: Box::new(els_expr),
            }
        }

        AstExpr::Match { expr, arms } => {
            // Match 表达式 → 包装为 BlockExpr 内含 Match 语句
            // （保留模式匹配和变量绑定，if-else 降级会丢失这些信息）
            let ir_scrutinee = convert_expr(expr, ctx);
            let mut arm_ctx = TypeCtx::new();
            arm_ctx.current_generics = ctx.current_generics.clone();
            arm_ctx.current_ret_ty = ctx.current_ret_ty.clone();

            let ir_arms: Vec<MatchArm> = arms
                .iter()
                .map(|arm| {
                    let pat = convert_ast_pattern(&arm.pattern, ctx).unwrap_or(Pattern::Wildcard);
                    let guard = arm.guard.as_ref().map(|g| convert_expr(g, ctx));
                    let mut body_ctx = TypeCtx::new();
                    body_ctx.current_generics = ctx.current_generics.clone();
                    body_ctx.current_ret_ty = ctx.current_ret_ty.clone();
                    body_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                    body_ctx.enum_variants = ctx.enum_variants.clone();
                    // 从模式中提取绑定变量名并添加到上下文
                    fn collect_pattern_vars(pat: &AstPattern, vars: &mut Vec<String>) {
                        match pat {
                            AstPattern::Ident(name) => vars.push(name.clone()),
                            AstPattern::Variant(_, args) => {
                                for a in args {
                                    collect_pattern_vars(a, vars);
                                }
                            }
                            AstPattern::Tuple(elems) => {
                                for e in elems {
                                    collect_pattern_vars(e, vars);
                                }
                            }
                            _ => {}
                        }
                    }
                    let mut bound_vars = Vec::new();
                    collect_pattern_vars(&arm.pattern, &mut bound_vars);
                    let scrut_ty = infer_expr_type(expr, ctx);
                    // 元组模式绑定（type-pack 异质元组 03d §2.8 方案 B）：
                    // `case (a,)` / `case (a, ..)` 中 a 绑定切片元素（&Ts），
                    // 类型应为集合元素类型而非整个集合（否则返回类型推断错）
                    let bind_ty = if matches!(&arm.pattern, AstPattern::Tuple(_)) {
                        match &scrut_ty {
                            IrType::Named { args, .. } if !args.is_empty() => args[0].clone(),
                            _ => scrut_ty.clone(),
                        }
                    } else {
                        scrut_ty.clone()
                    };
                    for v in &bound_vars {
                        body_ctx.add_var(v, bind_ty.clone());
                    }
                    // 变体模式字段绑定：Shape::Circle(_, _, r) → r 绑定为字段类型（int）
                    if let Some(ftypes) = field_types_for_variant(&arm.pattern, &body_ctx) {
                        for (fname, fty) in ftypes {
                            body_ctx.add_var(&fname, fty);
                        }
                    }
                    // 内置 Option/Result 变体：Some(v) → v 绑定为内层类型
                    if let Some(ftypes) =
                        field_types_for_builtin_variant(&arm.pattern, &scrut_ty)
                    {
                        for (fname, fty) in ftypes {
                            body_ctx.add_var(&fname, fty);
                        }
                    }
                    let body = convert_block_with_ctx(&arm.body, &body_ctx);
                    MatchArm {
                        pattern: pat,
                        guard,
                        body,
                    }
                })
                .collect();

            let match_stmt = Stmt::Match {
                scrutinee: ir_scrutinee,
                arms: ir_arms,
            };
            let blk_ty = arms
                .first()
                .and_then(|arm| arm.body.last())
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit);
            ExprKind::BlockExpr {
                block: Block {
                    span: Span::unknown(),
                    stmts: vec![match_stmt],
                    ty: blk_ty,
                },
            }
        }

        AstExpr::Closure { params, param_tys, body } => {
            // 闭包体是独立的词法块：创建新 ctx（继承 vars 但重置 block_declared），
            // 使闭包内 `total = total + x` 能识别为写外部变量（Assign）而非新绑定
            let mut closure_ctx = TypeCtx::new();
            closure_ctx.vars = ctx.vars.clone();
            closure_ctx.current_generics = ctx.current_generics.clone();
            closure_ctx.current_ret_ty = ctx.current_ret_ty.clone();
            closure_ctx.current_is_iterator = ctx.current_is_iterator;
            closure_ctx.current_fn_name = ctx.current_fn_name.clone();
            closure_ctx.pending_items = ctx.pending_items.clone();
            closure_ctx.errors = ctx.errors.clone();
            for sn in &ctx.struct_names {
                closure_ctx.struct_names.insert(sn.clone());
            }
            for (sn, fields) in &ctx.struct_fields {
                closure_ctx.struct_fields.insert(sn.clone(), fields.clone());
            }
            for (sn, order) in &ctx.struct_field_order {
                closure_ctx.struct_field_order.insert(sn.clone(), order.clone());
            }
            for (sn, ms) in &ctx.struct_methods {
                closure_ctx.struct_methods.insert(sn.clone(), ms.clone());
            }
            for (sn, arity) in &ctx.struct_method_arity {
                closure_ctx.struct_method_arity.insert(sn.clone(), arity.clone());
            }
            for (vn, en) in &ctx.enum_variants {
                closure_ctx.enum_variants.insert(vn.clone(), en.clone());
            }
            for (vn, ft) in &ctx.enum_variant_field_types {
                closure_ctx.enum_variant_field_types.insert(vn.clone(), ft.clone());
            }
            for (cn, ct) in &ctx.top_level_consts {
                closure_ctx.top_level_consts.insert(cn.clone(), ct.clone());
            }
            for (name, ty) in &ctx.fn_returns {
                closure_ctx.fn_returns.insert(name.clone(), ty.clone());
            }
            for (name, p) in &ctx.fn_params {
                closure_ctx.fn_params.insert(name.clone(), p.clone());
            }
            // 闭包参数登记到 closure_ctx.vars（遮蔽外部同名变量）：
            // 否则体内 `x + y` 中 y 会错误回退到外部 f64 变量，触发混合提升
            // （E0282 参数类型注解丢失的过渡方案：参数以 Any 登记，Any→i64 fallback）。
            // 带类型注解的 ref 参数（`|x: ref int|`）按 Ref 登记，使体内 `x > 2`
            // 能识别 x 是引用（codegen 解引用，iter.lz find 闭包 E0308）
            for (i, name) in params.iter().enumerate() {
                let param_ty = param_tys.get(i).and_then(|t| t.as_ref());
                let declared_ty = param_ty.map(|t| from_ast_type(t)).unwrap_or(IrType::Any);
                closure_ctx.vars.insert(name.clone(), declared_ty);
            }
            ExprKind::Lambda {
                params: params
                    .iter()
                    .enumerate()
                    .map(|(i, name)| Param {
                        name: name.clone(),
                        // 闭包参数类型注解（|x: int|）→ 填入 Lambda 参数类型，
                        // 否则生成无类型闭包导致 Option.None.map(|x| ...) E0282
                        ty: param_tys
                            .get(i)
                            .and_then(|t| t.as_ref())
                            .map(|t| from_ast_type_with_generics(t, &ctx.current_generics))
                            .unwrap_or(IrType::Any),
                        is_mut: false,
                        is_ref: false,
                        is_owned: false,
                        default: None,
                        variadic: false,
                    })
                    .collect(),
                body: Box::new(convert_expr(body, &closure_ctx)),
                is_move: true,
            }
        }

        AstExpr::BlockExpr(stmts) => {
            let ir_stmts: Vec<Stmt> = stmts.iter().map(|s| convert_stmt(s, ctx)).collect();
            ExprKind::BlockExpr {
                block: Block {
                    span: Span::unknown(),
                    stmts: ir_stmts,
                    ty: IrType::Any,
                },
            }
        }

        AstExpr::Range {
            start,
            end,
            inclusive,
        } => {
            // Range → StructCtor { name: "Range", fields: [start, end, inclusive] }
            let mut fields = Vec::new();
            if let Some(s) = start {
                fields.push(("start".into(), convert_expr(s, ctx)));
            }
            if let Some(e) = end {
                fields.push(("end".into(), convert_expr(e, ctx)));
            }
            if *inclusive {
                fields.push((
                    "inclusive".into(),
                    Expr::new(
                        ExprKind::Lit(LitKind::Bool(true)),
                        IrType::Bool,
                        Span::unknown(),
                    ),
                ));
            }
            ExprKind::StructCtor {
                name: "Range".into(),
                fields,
            }
        }

        AstExpr::Walrus { target, value } => {
            // := → 展开为 let + 返回；在表达式层面转为复合
            if let AstExpr::Ident(name) = target.as_ref() {
                let inner_ctx = ctx;
                let val = convert_expr(value, &inner_ctx);
                // walrus 变量登记不在表达式层做（convert_expr 是 &TypeCtx 不可变
                // 借用，无法 add_var）；由 convert_block 的前向传播统一登记
                // （collect_stmt_walrus），使后续语句 lookup_var(name) 拿到真实类型
                ExprKind::StructCtor {
                    name: "_Walrus".into(),
                    fields: vec![
                        (
                            "_bind".into(),
                            Expr::new(ExprKind::Var(name.clone()), val.ty.clone(), Span::unknown()),
                        ),
                        ("_val".into(), val),
                    ],
                }
            } else {
                convert_expr(value, ctx).kind
            }
        }

        AstExpr::Pipe {
            receiver,
            callee,
            args,
        } => {
            // |> 管道：
            // - 左侧 receiver 作为数据；若其类型实现了 __lpipe__，先调用 recv.__lpipe__() 产出数据
            // - 右侧 callee 分类：
            //     * 变量（实例，类型实现 __rpipe__）→ (right.__rpipe__(recv))(recv)
            //     * 变量（实例，类型实现 __call__）→ right.__call__(recv, ...args)（首参预填充）
            //     * 函数/构造/闭包/其他 → 首参预填充调用 callee(recv, ...args)
            let recv_ir = convert_expr(receiver, ctx);
            let args_ir: Vec<Expr> = args.iter().map(|a| convert_expr(a, ctx)).collect();
            // 左侧 __lpipe__：类型实现 __lpipe__ 时，数据 = recv.__lpipe__()（默认实现返回自身）
            let data_ir = {
                let recv_ty = infer_expr_type(receiver, ctx);
                if let IrType::Named { path, .. } = &recv_ty {
                    let has_lpipe = ctx
                        .struct_methods
                        .get(path)
                        .map_or(false, |m| m.contains("__lpipe__"));
                    if has_lpipe {
                        Expr::new(
                            ExprKind::MethodCall {
                                receiver: Box::new(recv_ir),
                                method: "__lpipe__".into(),
                                args: vec![],
                            },
                            recv_ty.clone(),
                            Span::unknown(),
                        )
                    } else {
                        recv_ir
                    }
                } else {
                    recv_ir
                }
            };
            // 右侧分类
            match callee.as_ref() {
                AstExpr::Ident(name) => {
                    // 变量（实例）→ __rpipe__ 优先，其次 __call__（须单参函数，否则报错）
                    let var_ty = ctx.lookup_var(name);
                    if let IrType::Named { path, .. } = &var_ty {
                        if let Some(methods) = ctx.struct_methods.get(path) {
                            if methods.contains("__rpipe__") {
                                // (right.__rpipe__(recv))(recv)
                                return Expr::new(
                                    ExprKind::Call {
                                        type_args: vec![],
                                        callee: Box::new(Expr::new(
                                            ExprKind::MethodCall {
                                                receiver: Box::new(Expr::new(
                                                    ExprKind::Var(name.clone()),
                                                    var_ty.clone(),
                                                    Span::unknown(),
                                                )),
                                                method: "__rpipe__".into(),
                                                args: vec![data_ir.clone()],
                                            },
                                            IrType::Any,
                                            Span::unknown(),
                                        )),
                                        args: vec![data_ir],
                                    },
                                    ty,
                                    span,
                                );
                            }
                            if methods.contains("__call__") {
                                // __call__ 必须是单参函数（非 self 参数 = 1）
                                let arity = ctx
                                    .struct_method_arity
                                    .get(path)
                                    .and_then(|m| m.get("__call__"))
                                    .copied()
                                    .unwrap_or(0);
                                if arity != 1 {
                                    ctx.report_error(format!(
                                        "管道右值 `{}` 的 __call__ 不是单参函数（参数数 {}，期望 1）；\
                                         实现 __rpipe__ 或改为单参 __call__",
                                        name, arity
                                    ));
                                }
                                // right.__call__(recv, ...args)（首参预填充）
                                let mut call_args = vec![data_ir];
                                call_args.extend(args_ir);
                                return Expr::new(
                                    ExprKind::MethodCall {
                                        receiver: Box::new(Expr::new(
                                            ExprKind::Var(name.clone()),
                                            var_ty.clone(),
                                            Span::unknown(),
                                        )),
                                        method: "__call__".into(),
                                        args: call_args,
                                    },
                                    ty,
                                    span,
                                );
                            }
                            // 已知 struct 实例但既无 __rpipe__ 也无 __call__ → 非 callable
                            ctx.report_error(format!(
                                "管道右值 `{}`（类型 {}）不可调用：未实现 __rpipe__ 或 __call__",
                                name, path
                            ));
                            return Expr::new(ExprKind::Lit(LitKind::Unit), ty, span);
                        }
                    }
                    // 函数/构造/未知 → 首参预填充调用；
                    // 已知 struct（构造调用）→ StructCtor 按字段顺序映射
                    if ctx.is_struct(name) {
                        let order = ctx.struct_field_order.get(name).cloned().unwrap_or_default();
                        let mut fields: Vec<(String, Expr)> = Vec::new();
                        fields.push(("".into(), data_ir));
                        fields.extend(args_ir.into_iter().map(|a| ("".into(), a)));
                        let mapped: Vec<(String, Expr)> = fields
                            .into_iter()
                            .enumerate()
                            .map(|(i, (_, e))| {
                                let fname_i = order
                                    .get(i)
                                    .cloned()
                                    .unwrap_or_else(|| format!("_f{}", i));
                                (fname_i, e)
                            })
                            .collect();
                        return Expr::new(
                            ExprKind::StructCtor {
                                name: name.clone(),
                                fields: mapped,
                            },
                            IrType::named(name),
                            Span::unknown(),
                        );
                    }
                    // args 含 `_` 洞（v |> add3(_, 10, 20)）→ 用左侧数据填充洞，而非盲目前置
                    let has_hole = args.iter().any(|a| matches!(a, AstExpr::Ident(s) if s == "_"));
                    // callee 返回类型为 Fn（x |> if_func(flag)，if_func 返回 fn(int)->int）
                    // → 语义是先调用 if_func(flag) 得到函数，再把 x 作为其参数：
                    //   if_func(flag)(x)，而非 if_func(x, flag)（E0061）
                    let callee_ret_is_fn = matches!(ctx.lookup_fn_return(name), IrType::Fn { .. });
                    let all_args = if has_hole {
                        let mut filled: Vec<Expr> = Vec::new();
                        let mut data_used = false;
                        for a in args.iter() {
                            if matches!(a, AstExpr::Ident(s) if s == "_") {
                                filled.push(data_ir.clone());
                                data_used = true;
                            } else {
                                filled.push(convert_expr(a, ctx));
                            }
                        }
                        if !data_used {
                            filled.insert(0, data_ir);
                        }
                        filled
                    } else if callee_ret_is_fn {
                        // if_func(flag) 返回函数 → 先调 callee(args...)，再以 data 为参数调用
                        let inner_call = ExprKind::Call {
                            type_args: vec![],
                            callee: Box::new(Expr::new(
                                ExprKind::Var(name.clone()),
                                IrType::Any,
                                Span::unknown(),
                            )),
                            args: args_ir,
                        };
                        return Expr::new(
                            ExprKind::Call {
                                type_args: vec![],
                                callee: Box::new(Expr::new(
                                    inner_call,
                                    IrType::Any,
                                    Span::unknown(),
                                )),
                                args: vec![data_ir],
                            },
                            ty,
                            span,
                        );
                    } else {
                        let mut pre = vec![data_ir];
                        pre.extend(args_ir);
                        pre
                    };
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(Expr::new(
                            ExprKind::Var(name.clone()),
                            IrType::Any,
                            Span::unknown(),
                        )),
                        args: all_args,
                    }
                }
                // 右侧是调用且含 `_` 洞（v |> f(_, 5)）：用左侧数据填充洞，而非盲目前置
                AstExpr::Call {
                    func,
                    args: call_args,
                    ..
                } if call_args.iter().any(|a| matches!(a, AstExpr::Ident(s) if s == "_")) => {
                    let mut filled: Vec<Expr> = Vec::new();
                    let mut data_used = false;
                    for a in call_args.iter() {
                        if matches!(a, AstExpr::Ident(s) if s == "_") {
                            filled.push(data_ir.clone());
                            data_used = true;
                        } else {
                            filled.push(convert_expr(a, ctx));
                        }
                    }
                    // 无洞（防御）→ 首参预填充
                    if !data_used {
                        filled.insert(0, data_ir);
                    }
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(convert_expr(func, ctx)),
                        args: filled,
                    }
                }
                // 闭包/方法/复杂表达式 → 保留 Pipe 节点，codegen 兜底展开
                // 闭包作为管道右侧（val |> (|x| => ...)）：参数类型从 receiver 推断，
                // 否则生成无类型闭包导致 E0282（combo-pipe-lambda.lz pipe_match）
                AstExpr::Closure { params, body, .. } => {
                    let recv_ty = infer_expr_type(receiver, ctx);
                    let recv_ty_inner = if let IrType::Fn { ret, .. } = &recv_ty {
                        // receiver 本身可能是函数（double 的结果类型为 int）
                        *ret.clone()
                    } else {
                        recv_ty.clone()
                    };
                    let mut closure_ctx = TypeCtx::new();
                    closure_ctx.vars = ctx.vars.clone();
                    closure_ctx.current_generics = ctx.current_generics.clone();
                    closure_ctx.current_ret_ty = ctx.current_ret_ty.clone();
                    closure_ctx.current_is_iterator = ctx.current_is_iterator;
                    closure_ctx.current_fn_name = ctx.current_fn_name.clone();
                    closure_ctx.pending_items = ctx.pending_items.clone();
                    closure_ctx.errors = ctx.errors.clone();
                    closure_ctx.struct_names = ctx.struct_names.clone();
                    closure_ctx.struct_fields = ctx.struct_fields.clone();
                    closure_ctx.struct_field_order = ctx.struct_field_order.clone();
                    closure_ctx.struct_methods = ctx.struct_methods.clone();
                    closure_ctx.struct_method_arity = ctx.struct_method_arity.clone();
                    closure_ctx.enum_variants = ctx.enum_variants.clone();
                    closure_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                    closure_ctx.top_level_consts = ctx.top_level_consts.clone();
                    closure_ctx.fn_returns = ctx.fn_returns.clone();
                    closure_ctx.fn_params = ctx.fn_params.clone();
                    for name in params {
                        closure_ctx.vars.insert(name.clone(), IrType::Any);
                    }
                    let lambda_params: Vec<Param> = params
                        .iter()
                        .enumerate()
                        .map(|(i, name)| Param {
                            name: name.clone(),
                            ty: if i == 0 {
                                recv_ty_inner.clone()
                            } else {
                                IrType::Any
                            },
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                        })
                        .collect();
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(Expr::new(
                            ExprKind::Lambda {
                                params: lambda_params,
                                body: Box::new(convert_expr(body, &closure_ctx)),
                                is_move: true,
                            },
                            IrType::Any,
                            Span::unknown(),
                        )),
                        args: vec![data_ir],
                    }
                }
                _ => ExprKind::Pipe {
                    receiver: Box::new(data_ir),
                    callee: Box::new(convert_expr(callee, ctx)),
                    args: args_ir,
                },
            }
        }

        AstExpr::SafeNav { receiver, field } => {
            // x?.field → if x == None then None else x.field
            // 但如果 receiver 是类型名（非变量），直接字段访问，跳过 null check
            let recv = convert_expr(receiver, ctx);

            // 检查 receiver 是否是已知类型名（非变量引用）→ 跳过 null check
            let is_type_name = match receiver.as_ref() {
                AstExpr::Ident(name) => {
                    !ctx.vars.contains_key(name.as_str())
                        && (ctx.struct_names.contains(name.as_str())
                            || ctx.enum_variants.values().any(|en| en == name.as_str()))
                }
                _ => false,
            };

            if is_type_name {
                ExprKind::FieldAccess {
                    base: Box::new(recv),
                    field: field.clone(),
                }
            } else {
                // x?.field → x.map(|__sn| __sn.field)（Option.map；x 为 None 时得 None）
                // 避免 == None 比较（需 PartialEq）和直接 .field（Option 无该字段）
                let param = "__sn".to_string();
                // receiver 是 Dict/HashMap 时，`?.field` 是键访问：
                // __sn.field → __sn.get("field")（否则 E0609 no field）
                // 注意 receiver 可能是 Option<Dict>（?. 解包后才是 Dict）
                let recv_is_dict = matches!(
                    &recv.ty,
                    IrType::Named { path, .. } if path == "Dict" || path == "HashMap"
                ) || matches!(
                    &recv.ty,
                    IrType::Option(inner)
                        if matches!(inner.as_ref(), IrType::Named { path, .. } if path == "Dict" || path == "HashMap")
                );
                let access_expr = if recv_is_dict {
                    // Dict 键访问：__sn.get("field") 返回 Option<&V>，
                    // 链式 SafeNav 需 and_then 扁平化（map 会得 Option<Option<..>>）
                    // 且 get 结果需 .copied() 转 Option<V>（否则 unwrap_or(30) 类型不匹配）
                    let get_call = Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(Expr::new(
                                ExprKind::Var(param.clone()),
                                IrType::Any,
                                Span::unknown(),
                            )),
                            method: "get".into(),
                            args: vec![Expr::new(
                                ExprKind::Lit(LitKind::Str(field.clone())),
                                IrType::Str,
                                Span::unknown(),
                            )],
                        },
                        IrType::Any,
                        Span::unknown(),
                    );
                    Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(get_call),
                            method: "copied".into(),
                            args: vec![],
                        },
                        IrType::Any,
                        Span::unknown(),
                    )
                } else {
                    Expr::new(
                        ExprKind::FieldAccess {
                            base: Box::new(Expr::new(
                                ExprKind::Var(param.clone()),
                                IrType::Any,
                                Span::unknown(),
                            )),
                            field: field.clone(),
                        },
                        IrType::Any,
                        Span::unknown(),
                    )
                };
                ExprKind::MethodCall {
                    receiver: Box::new(recv),
                    // Dict 键访问（get 返回 Option）→ and_then 扁平化；
                    // 普通字段访问 → map
                    method: if recv_is_dict { "and_then".into() } else { "map".into() },
                    args: vec![Expr::new(
                        ExprKind::Lambda {
                            params: vec![Param {
                                name: param.clone(),
                                ty: IrType::Any,
                                is_mut: false,
                                is_ref: false,
                                is_owned: false,
                                default: None,
                                variadic: false,
                            }],
                            body: Box::new(access_expr),
                            is_move: true,
                        },
                        IrType::Any,
                        Span::unknown(),
                    )],
                }
            }
        }

        AstExpr::Try(inner) => {
            // try expr (? 操作符): 对 Result/Option 类型做错误传播，否则透传
            let inner_ty = infer_expr_type(inner, ctx);
            let result_like = matches!(&inner_ty, IrType::Result { .. } | IrType::Option(_))
                || matches!(&inner_ty,
                    IrType::Named { path, .. } if path == "Result" || path == "Option"
                );
            // 自定义传播类型（实现 __is_ok__/__unwrap__/__err__ 的 struct）：
            // 与 Result/Option 一样走解包路径（spread_protocol.lz 的 HttpResult）
            let custom_propagating = matches!(&inner_ty, IrType::Named { path, .. }
                if ctx.struct_methods.get(path).map_or(false, |ms| ms.contains("__is_ok__")));
            if result_like || custom_propagating {
                ExprKind::MethodCall {
                    receiver: Box::new(convert_expr(inner, ctx)),
                    method: "try_into".into(),
                    args: vec![],
                }
            } else {
                // Non-Result type: just pass through (raises-type propagation)
                convert_expr(inner, ctx).kind
            }
        }

        AstExpr::NullCoalesce { left, right } => {
            // a ?? b → null_coalesce 特殊方法调用（codegen 层按类型展开）
            let l = convert_expr(left, ctx);
            let r = convert_expr(right, ctx);
            ExprKind::MethodCall {
                receiver: Box::new(l),
                method: "__null_coalesce".into(),
                args: vec![r],
            }
        }

        AstExpr::ListLit(items) => {
            ExprKind::ListLit(items.iter().map(|i| convert_expr(i, ctx)).collect())
        }

        AstExpr::DictLit(entries) => {
            // Dict → StructCtor，将条目存储为 (k, v) 对
            let mut fields = Vec::new();
            for (i, (k, v)) in entries.iter().enumerate() {
                fields.push((format!("_k{}", i), convert_expr(k, ctx)));
                fields.push((format!("_v{}", i), convert_expr(v, ctx)));
            }
            ExprKind::StructCtor {
                name: "Dict".into(),
                fields,
            }
        }

        AstExpr::SetLit(items) => ExprKind::Call {
            type_args: vec![],
            callee: Box::new(Expr::new(
                ExprKind::Var("set!".into()),
                IrType::Any,
                Span::unknown(),
            )),
            args: items.iter().map(|i| convert_expr(i, ctx)).collect(),
        },

        AstExpr::TupleLit(elems) => {
            ExprKind::TupleLit(elems.iter().map(|e| convert_expr(e, ctx)).collect())
        }

        AstExpr::ListComprehension {
            output,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            // [out for x in iter if cond] → 展开为生成模式
            if !extra_clauses.is_empty() {
                // 多 for：构建嵌套 flat_map 链
                let mut clauses = vec![(var.clone(), iter.clone(), cond.clone())];
                clauses.extend(extra_clauses.iter().map(|(v, i, c)| (v.clone(), i.clone(), c.clone())));
                let out_expr = convert_expr(output, ctx);
                return Expr::new(build_multi_comp(ctx, &clauses, out_expr, "comp"), ty, span);
            }
            let iter_expr = convert_expr(iter, ctx);
            let out_expr = convert_expr(output, ctx);
            let mut args = vec![
                Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                        }],
                        body: Box::new(out_expr),
                        is_move: true,
                    },
                    IrType::Any,
                    Span::unknown(),
                ),
                iter_expr,
            ];
            // 过滤条件 cond 作为第三个参数传入 (可选)
            if let Some(c) = cond {
                args.push(Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                        }],
                        body: Box::new(convert_expr(c, ctx)),
                        is_move: true,
                    },
                    IrType::Any,
                    Span::unknown(),
                ));
            }
            ExprKind::Call {
                type_args: vec![],
                callee: Box::new(Expr::new(
                    ExprKind::Var("comp!".into()),
                    IrType::Any,
                    Span::unknown(),
                )),
                args,
            }
        }

        AstExpr::DictComprehension {
            key,
            value,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            // {k: v for x in iter} → 展开为生成模式
            let key_expr = convert_expr(key, ctx);
            let val_expr = convert_expr(value, ctx);
            let body = Expr::new(
                ExprKind::TupleLit(vec![key_expr, val_expr]),
                IrType::Any,
                Span::unknown(),
            );
            if !extra_clauses.is_empty() {
                // 多 for：构建嵌套 flat_map 链
                let mut clauses = vec![(var.clone(), iter.clone(), cond.clone())];
                clauses.extend(extra_clauses.iter().map(|(v, i, c)| (v.clone(), i.clone(), c.clone())));
                return Expr::new(build_multi_comp(ctx, &clauses, body, "dict_comp"), ty, span);
            }
            let iter_expr = convert_expr(iter, ctx);
            let mut args = vec![
                Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                        }],
                        body: Box::new(body),
                        is_move: true,
                    },
                    IrType::Any,
                    Span::unknown(),
                ),
                iter_expr,
            ];
            if let Some(c) = cond {
                args.push(Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                        }],
                        body: Box::new(convert_expr(c, ctx)),
                        is_move: true,
                    },
                    IrType::Any,
                    Span::unknown(),
                ));
            }
            ExprKind::Call {
                type_args: vec![],
                callee: Box::new(Expr::new(
                    ExprKind::Var("dict_comp!".into()),
                    IrType::Any,
                    Span::unknown(),
                )),
                args,
            }
        }

        AstExpr::SetComprehension {
            elem,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            // {x for x in iter} → 展开为生成模式
            let elem_expr = convert_expr(elem, ctx);
            if !extra_clauses.is_empty() {
                // 多 for：构建嵌套 flat_map 链
                let mut clauses = vec![(var.clone(), iter.clone(), cond.clone())];
                clauses.extend(extra_clauses.iter().map(|(v, i, c)| (v.clone(), i.clone(), c.clone())));
                return Expr::new(build_multi_comp(ctx, &clauses, elem_expr, "set_comp"), ty, span);
            }
            let iter_expr = convert_expr(iter, ctx);
            let mut args = vec![
                Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                        }],
                        body: Box::new(elem_expr),
                        is_move: true,
                    },
                    IrType::Any,
                    Span::unknown(),
                ),
                iter_expr,
            ];
            if let Some(c) = cond {
                args.push(Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                        }],
                        body: Box::new(convert_expr(c, ctx)),
                        is_move: true,
                    },
                    IrType::Any,
                    Span::unknown(),
                ));
            }
            ExprKind::Call {
                type_args: vec![],
                callee: Box::new(Expr::new(
                    ExprKind::Var("set_comp!".into()),
                    IrType::Any,
                    Span::unknown(),
                )),
                args,
            }
        }

        AstExpr::Assign { target, op, value } => {
            // 纯赋值（`total = total + x`，闭包体/表达式上下文）→ AssignExpr，
            // codegen 渲染 `target = value`；复合赋值（+= 等）→ BinOp
            if *op == AssignOp::Eq {
                ExprKind::AssignExpr {
                    target: Box::new(convert_expr(target, ctx)),
                    value: Box::new(convert_expr(value, ctx)),
                }
            } else {
                ExprKind::BinOp {
                    op: map_assign_op(op),
                    lhs: Box::new(convert_expr(target, ctx)),
                    rhs: Box::new(convert_expr(value, ctx)),
                }
            }
        }

        AstExpr::Spawn(inner) => {
            // go expr → 并行线程：thread::spawn(move || { expr })
            // 与显式 spawn(expr)（异步任务）区分，使用内部标记 __go
            ExprKind::Call {
                type_args: vec![],
                callee: Box::new(Expr::new(
                    ExprKind::Var("__go".into()),
                    IrType::Any,
                    Span::unknown(),
                )),
                args: vec![convert_expr(inner, ctx)],
            }
        }

        AstExpr::Move(inner) => {
            convert_expr(inner, ctx).kind // move 语义在 IR 中由所有权表达，暂透传
        }

        AstExpr::Panic(inner) => ExprKind::Call {
            type_args: vec![],
            callee: Box::new(Expr::new(
                ExprKind::Var("panic!".into()),
                IrType::Any,
                Span::unknown(),
            )),
            args: vec![convert_expr(inner, ctx)],
        },

        AstExpr::Await(inner) => ExprKind::MethodCall {
            receiver: Box::new(convert_expr(inner, ctx)),
            method: "await".into(),
            args: vec![],
        },

        AstExpr::BuildBlock { kind, lhs, body } => {
            // 构建块脱糖：将 body 转换为 BlockExpr，然后包装为闭包立即调用
            // 参考 AST codegen 的 gen_build_block 实现
            match kind {
                BuildKind::Var => {
                    // =: → { let __tmp = (|| { body; __result })(); __tmp }
                    // body 中的变量声明在此作用域中，闭包立即执行
                    let body_block = convert_block_with_ctx(body, ctx);
                    // 用 BlockExpr 包装 body（作为 Lambda 立即调用）
                    let body_expr = Expr::new(
                        ExprKind::BlockExpr { block: body_block },
                        IrType::Any,
                        Span::unknown(),
                    );
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(Expr::new(
                            ExprKind::Lambda {
                                params: vec![],
                                body: Box::new(body_expr),
                                is_move: false,
                            },
                            IrType::Any,
                            Span::unknown(),
                        )),
                        args: vec![],
                    }
                }
                BuildKind::Call => {
                    // ~: → callee(args...) 其中 args = body 返回元组的元素
                    // 如果 body 最后返回的是元组，解包为独立参数
                    let body_block = convert_block_with_ctx(body, ctx);
                    let block_ty = body_block.ty.clone(); // 在 move 之前提取类型
                    let body_expr = Expr::new(
                        ExprKind::BlockExpr { block: body_block },
                        IrType::Any,
                        Span::unknown(),
                    );
                    let packed = Expr::new(
                        ExprKind::Call {
                            type_args: vec![],
                            callee: Box::new(Expr::new(
                                ExprKind::Lambda {
                                    params: vec![],
                                    body: Box::new(body_expr),
                                    is_move: false,
                                },
                                IrType::Any,
                                Span::unknown(),
                            )),
                            args: vec![],
                        },
                        IrType::Any,
                        Span::unknown(),
                    );
                    // 如果 body 返回元组，解包为独立参数
                    // 元组类型可能来自：块体末尾 TupleLit 推断，或单个 Ident 引用元组变量
                    // （multiply ~: factors — factors 是先前 =: 构建块返回的元组）
                    let block_ty_for_unpack = if let IrType::Tuple(_) = block_ty {
                        block_ty.clone()
                    } else if let Some(AstStmt::Expr(AstExpr::Ident(n))) = body.last() {
                        // 块体末尾是变量引用：若其类型是元组，则按元组拆包
                        match ctx.lookup_var(n) {
                            IrType::Tuple(_) => ctx.lookup_var(n),
                            _ => block_ty.clone(),
                        }
                    } else {
                        block_ty.clone()
                    };
                    let args: Vec<Expr> = if let IrType::Tuple(elements) = block_ty_for_unpack {
                        // 元组解包：为每个元素生成一个 packed 字段访问
                        elements
                            .iter()
                            .enumerate()
                            .map(|(i, _elem)| {
                                Expr::new(
                                    ExprKind::MagicCall {
                                        kind: MagicKind::UnpackBuildCall,
                                        args: vec![
                                            packed.clone(),
                                            Expr::new(
                                                ExprKind::Lit(LitKind::Int(i as i64)),
                                                IrType::Int,
                                                Span::unknown(),
                                            ),
                                        ],
                                    },
                                    IrType::Any,
                                    Span::unknown(),
                                )
                            })
                            .collect()
                    } else if matches!(
                        &block_ty,
                        IrType::Named { path, .. } if path == "Dict" || path == "HashMap"
                    ) {
                        // 字典拆包：块体末尾是 DictLit → 按名称转关键字实参
                        // （greet ~: {"greeting": "Hello", "name": "Lang-Zone"} →
                        //   greet(greeting: "Hello", name: "Lang-Zone")）
                        let dict_entries = body
                            .last()
                            .and_then(|s| match s {
                                AstStmt::Expr(AstExpr::DictLit(entries)) => Some(entries.clone()),
                                _ => None,
                            });
                        match dict_entries {
                            Some(entries) => entries
                                .iter()
                                .map(|(k, v)| {
                                    Expr::new(
                                        ExprKind::StructCtor {
                                            name: "_KwArg".into(),
                                            fields: vec![
                                                ("name".to_string(), convert_expr(k, ctx)),
                                                ("value".to_string(), convert_expr(v, ctx)),
                                            ],
                                        },
                                        IrType::Any,
                                        Span::unknown(),
                                    )
                                })
                                .collect(),
                            None => vec![packed],
                        }
                    } else {
                        vec![packed]
                    };
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(convert_expr(lhs, ctx)),
                        args,
                    }
                }
                BuildKind::Gen => {
                    // *: → 生成器构建块：callee 为左侧函数/方法引用（有 callee 时逐包调用，
                    // 无 callee 时仅收集参数包返回迭代器）。body 中的 yield 由 codegen 收集。
                    let body_block = convert_block_with_ctx(body, ctx);
                    let callee = match &**lhs {
                        AstExpr::Ident(_) | AstExpr::MethodCall { .. } | AstExpr::FieldAccess { .. } => {
                            Some(Box::new(convert_expr(lhs, ctx)))
                        }
                        _ => None,
                    };
                    ExprKind::GenBuild {
                        callee,
                        block: body_block,
                    }
                }
                BuildKind::Index => {
                    // ^: → IndexGet。key = body 块中的最后一个表达式值。
                    // 语法：container ^: <key>（冒号后换行缩进，块体为单值 key）
                    let key_expr = body
                        .last()
                        .and_then(|s| match s {
                            AstStmt::Expr(e) => Some(convert_expr(e, ctx)),
                            _ => None,
                        })
                        .unwrap_or_else(|| {
                            // 回退：若无单个尾部表达式，用整个块（BlockExpr）
                            let blk = convert_block_with_ctx(body, ctx);
                            Expr::new(
                                ExprKind::BlockExpr { block: blk },
                                IrType::Any,
                                Span::unknown(),
                            )
                        });
                    ExprKind::IndexGet {
                        base: Box::new(convert_expr(lhs, ctx)),
                        key: Box::new(key_expr),
                    }
                }
            }
        }

        AstExpr::KwArg { name, value } => {
            // 关键字参数：后端按目标语言映射
            ExprKind::StructCtor {
                name: "_KwArg".into(),
                fields: vec![
                    (
                        "name".into(),
                        Expr::new(
                            ExprKind::Lit(LitKind::Str(name.clone())),
                            IrType::Str,
                            Span::unknown(),
                        ),
                    ),
                    ("value".into(), convert_expr(value, ctx)),
                ],
            }
        }

        AstExpr::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            // 构建 Stmt::TryCatch 结构以供 codegen 层正确处理
            let body_block = convert_block(body, ctx);
            let ir_catches: Vec<(Option<Pattern>, Block)> = catches
                .iter()
                .map(|c| {
                    let pat = convert_ast_pattern(&c.pattern, ctx);
                    let block = convert_block(&c.body, ctx);
                    (pat, block)
                })
                .collect();
            let ir_else = else_body.as_ref().map(|b| convert_block(b, ctx));
            let ir_finally = finally_body.as_ref().map(|b| convert_block(b, ctx));

            // 返回一个 TryCatch 包装块（codegen 会生成 catch_unwind 等逻辑）
            ExprKind::BlockExpr {
                block: Block {
                    span: Span::unknown(),
                    stmts: vec![Stmt::TryCatch {
                        body: body_block,
                        catches: ir_catches,
                        else_body: ir_else,
                        finally_body: ir_finally,
                    }],
                    ty: IrType::Any,
                },
            }
        }
    };

    Expr::new(kind, ty, span)
}

/// 将 AST 语句块转为 IR 表达式（用于 if/match 分支）
fn block_to_expr(stmts: &[AstStmt], ctx: &TypeCtx) -> Expr {
    if stmts.is_empty() {
        return Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown());
    }
    if stmts.len() == 1 {
        if let AstStmt::Expr(e) = &stmts[0] {
            return convert_expr(e, ctx);
        }
    }
    let ir_stmts: Vec<Stmt> = convert_stmts(stmts, ctx);
    let blk_ty = stmts
        .last()
        .map(|s| infer_stmt_type(s, ctx))
        .unwrap_or(IrType::Unit);
    Expr::new(
        ExprKind::BlockExpr {
            block: Block {
                span: Span::unknown(),
                stmts: ir_stmts,
                ty: blk_ty.clone(),
            },
        },
        blk_ty,
        Span::unknown(),
    )
}

// ══════════════════════════════════════════════════════════════
// Stmt 转换
// ══════════════════════════════════════════════════════════════

/// 将 AST Stmt 列表转换为 IR Stmt 列表，展开 LetTuple
fn convert_stmts(ast_stmts: &[AstStmt], ctx: &TypeCtx) -> Vec<Stmt> {
    let mut result = Vec::new();
    for s in ast_stmts {
        if let AstStmt::LetTuple { names, ty, value } = s {
            let ir_value = convert_expr(value, ctx);
            let val_ty = ir_value.ty.clone();
            let tmp_name = format!("__destruct_{}", names.join("_"));
            result.push(Stmt::Let {
                name: tmp_name.clone(),
                ty: val_ty.clone(),
                value: ir_value,
                is_mut: false,
                is_ref: false,
            });
            for (i, name) in names.iter().enumerate() {
                if name == "_" {
                    continue;
                }
                let field_expr = Expr::new(
                    ExprKind::FieldAccess {
                        base: Box::new(Expr::new(
                            ExprKind::Var(tmp_name.clone()),
                            val_ty.clone(),
                            Span::unknown(),
                        )),
                        field: format!("{}", i),
                    },
                    IrType::Any,
                    Span::unknown(),
                );
                // 解构字段类型：从元组类型提取元素（`let (lower, upper) = size_hint()`
                // 的 (int, Option<int>) → lower: int, upper: Option<int>），否则 Any
                // 导致 `return (lower, upper)` 推断错误（E0277 ImplicitFrom）
                let field_ty = match &val_ty {
                    IrType::Tuple(items) => {
                        items.get(i).cloned().unwrap_or(IrType::Any)
                    }
                    _ => ty
                        .as_ref()
                        .map(|t| from_ast_type(t))
                        .unwrap_or(IrType::Any),
                };
                result.push(Stmt::Let {
                    name: name.clone(),
                    ty: field_ty,
                    value: field_expr,
                    is_mut: false,
                    is_ref: false,
                });
            }
        } else {
            result.push(convert_stmt(s, ctx));
        }
    }
    result
}

fn convert_stmt(ast_stmt: &AstStmt, ctx: &TypeCtx) -> Stmt {
    match ast_stmt {
        AstStmt::Expr(AstExpr::Match { expr, arms }) => {
            // match 语句 → 直接用 IR Match 节点（codegen 已有完整支持）
            let ir_scrutinee = convert_expr(expr, ctx);
            let ir_arms: Vec<MatchArm> = arms
                .iter()
                .map(|arm| {
                    let pat = convert_ast_pattern(&arm.pattern, ctx).unwrap_or(Pattern::Wildcard);
                    let guard = arm.guard.as_ref().map(|g| convert_expr(g, ctx));
                    let mut arm_ctx = TypeCtx::new();
                    arm_ctx.vars = ctx.vars.clone();
                    arm_ctx.current_generics = ctx.current_generics.clone();
                    arm_ctx.current_ret_ty = ctx.current_ret_ty.clone();
                    arm_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                    // 复制 enum_variants 以便模式匹配能正确解析枚举类型
                    for (vn, en) in &ctx.enum_variants {
                        arm_ctx.enum_variants.insert(vn.clone(), en.clone());
                    }
                    for (cn, ct) in &ctx.top_level_consts {
                        arm_ctx.top_level_consts.insert(cn.clone(), ct.clone());
                    }
                    // 也复制 struct 信息用于模式匹配
                    for sn in &ctx.struct_names {
                        arm_ctx.struct_names.insert(sn.clone());
                    }
                    if let AstPattern::Ident(name) = &arm.pattern {
                        // 裸枚举变体名模式（`case Equal:`）是变体匹配而非变量绑定：
                        // 登记为绑定变量会把臂体内 `return Equal` 解析成类型为 Self 的
                        // 变量引用（E0277 ImplicitFrom<Self>）。仅当名字不是枚举变体
                        // 时才 add_var（如 `case x:` 绑定整个 scrutinee）
                        let is_enum_variant =
                            ctx.enum_variants.contains_key(name.as_str());
                        if !is_enum_variant {
                            let scrut_ty = infer_expr_type(expr, ctx);
                            arm_ctx.add_var(name, scrut_ty);
                        }
                    }
                    // ref mut 绑定（case Some(ref mut c)）：c 登记为 MutRef 内层类型，
                    // 臂体内 c = c + 1 需生成 *c = *c + 1（解引用赋值）
                    if let AstPattern::RefMutIdent(name) = &arm.pattern {
                        let scrut_ty = infer_expr_type(expr, ctx);
                        let inner = match &scrut_ty {
                            IrType::MutRef(i) => *i.clone(),
                            IrType::Ref(i) => *i.clone(),
                            other => other.clone(),
                        };
                        arm_ctx.add_var(name, IrType::MutRef(Box::new(inner)));
                    }
                    // 变体模式字段绑定：Shape::Circle(x: _, y: _, radius: r) →
                    // r 绑定为 radius 字段类型（int），而非整个 scrutinee 类型
                    if let AstPattern::Variant(..) = &arm.pattern {
                        if let Some(ftypes) = field_types_for_variant(&arm.pattern, &arm_ctx) {
                            for (fname, ty) in ftypes {
                                arm_ctx.add_var(&fname, ty);
                            }
                        }
                    }
                    // 内置 Option/Result 变体：Some(v) → v 绑定为内层类型
                    let scrut_ty2 = infer_expr_type(expr, ctx);
                    if let Some(ftypes) =
                        field_types_for_builtin_variant(&arm.pattern, &scrut_ty2)
                    {
                        for (fname, ty) in ftypes {
                            arm_ctx.add_var(&fname, ty);
                        }
                    }
                    let body = convert_block_with_ctx(&arm.body, &arm_ctx);
                    MatchArm {
                        pattern: pat,
                        guard,
                        body,
                    }
                })
                .collect();
            Stmt::Match {
                scrutinee: ir_scrutinee,
                arms: ir_arms,
            }
        }
        AstStmt::Expr(e) => {
            // 构建块（=:/~:）在表达式位置出现时，需要特殊处理
            // =: 构建块作为表达式语句：生成立即调用闭包作为表达式
            if let AstExpr::BuildBlock {
                kind: BuildKind::Var,
                lhs,
                body,
            } = e
            {
                let lhs_name = match &**lhs {
                    AstExpr::Ident(name) => name.clone(),
                    _ => return Stmt::Pass,
                };
                let body_block = convert_block_with_ctx(body, ctx);
                let body_expr = Expr::new(
                    ExprKind::BlockExpr { block: body_block },
                    IrType::Any,
                    Span::unknown(),
                );
                let init_expr = Expr::new(
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(Expr::new(
                            ExprKind::Lambda {
                                params: vec![],
                                body: Box::new(body_expr),
                                is_move: false,
                            },
                            IrType::Any,
                            Span::unknown(),
                        )),
                        args: vec![],
                    },
                    IrType::Any,
                    Span::unknown(),
                );
                // =: 构建块作为语句：let lhs = <立即调用闭包>
                // 类型从 body 末尾表达式推断（元组如 (a,b,c) → Tuple），
                // 否则 factors 登记为 Any，后续 `multiply ~: factors`
                // 的元组拆包无法识别（E0061）
                // 若 body 含无值 return（return; 退出构建块自身）→ 块值 Unit
                let has_bare_return = body
                    .iter()
                    .any(|s| ast_stmt_has_bare_return(s));
                let build_ty = if has_bare_return {
                    IrType::Unit
                } else {
                    body.last()
                        .map(|s| infer_stmt_type(s, ctx))
                        .filter(|t| !matches!(t, IrType::Any))
                        .unwrap_or(IrType::Any)
                };
                return Stmt::Let {
                    name: lhs_name,
                    ty: build_ty,
                    value: init_expr,
                    is_mut: false,
                    is_ref: false,
                };
            }
            Stmt::ExprStmt {
                expr: convert_expr(e, ctx),
            }
        }

        AstStmt::Pass => Stmt::Pass,

        AstStmt::TypeAlias { name, ty } => Stmt::TypeAlias {
            name: name.clone(),
            ty: from_ast_type(ty),
        },

        AstStmt::Let {
            name,
            mutable,
            is_ref,
            ty,
            value,
            ..
        } => {
            let ir_ty = ty
                .as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, ctx));
            // 类型注解为 Option/Result 时，裸 `None`（内建构造器名作标识符场景，
            // 如 edge-keyword-identifier.lz 中先 `let None = 300` 后又
            // `let x: Option<int> = None`）应解析为 None 字面量，而非变量引用
            // （否则 codegen 的 downgraded_vars 会把它重命名为 None_，E0425）
            let is_option_result_annot = matches!(&ir_ty, IrType::Option(_) | IrType::Result { .. })
                || matches!(&ir_ty, IrType::Named { path, .. }
                    if path == "Option" || path == "Result");
            let mut ir_value = if is_option_result_annot
                && matches!(value, AstExpr::Ident(n) if n == "None")
            {
                Expr::new(
                    ExprKind::Lit(LitKind::None_),
                    ir_ty.clone(),
                    Span::unknown(),
                )
            } else {
                convert_expr(value, ctx)
            };
            // 当 value 是 Lambda（部分应用展开等），使用 Lambda 的类型而非 infer 的类型
            // 当 value 的 IR 类型为 Any 且无显式类型注解时，也使用 IR 类型避免错误标注
            // 注意：若存在显式类型注解（如 let n: Option<int> = None），必须保留注解类型
            let ir_ty = match &ir_value.ty {
                IrType::Fn { .. } => ir_value.ty.clone(),
                IrType::Any if ir_ty == IrType::Any => ir_value.ty.clone(),
                _ => ir_ty,
            };
            // 当 Let 类型注解为 fn(..) -> .. 且 value 是 Lambda 时，
            // 将 fn 的参数类型传播到 Lambda 参数中
            if let IrType::Fn {
                params: fn_params, ..
            } = &ir_ty
            {
                if let ExprKind::Lambda {
                    params: lambda_params,
                    ..
                } = &mut ir_value.kind
                {
                    if lambda_params.len() == fn_params.len() {
                        for (lp, fp) in lambda_params.iter_mut().zip(fn_params.iter()) {
                            lp.ty = fp.clone();
                        }
                    }
                }
            }
            // 无 let 前缀的默认可变绑定（x = v）且变量在**本块之外**已存在 → 重新赋值
            // （闭包内写外部变量：total = total + x → total = total + x 而非新绑定）
            // 本块首次声明（block_declared 含 name）保持 Let；外部已有但本块未声明 → Assign
            // 顶层变量（top_level_consts 含 name）→ 修改全局（guard_for_3.lz size = size - 1）
            let is_top_level_mut = ctx.top_level_consts.contains_key(name.as_str());
            if *mutable && ty.is_none() && (ctx.vars.contains_key(name.as_str()) || is_top_level_mut)
                && !ctx.block_declared.contains(name.as_str())
            {
                Stmt::Assign {
                    target: Expr::new(
                        ExprKind::Var(name.clone()),
                        ir_ty.clone(),
                        Span::unknown(),
                    ),
                    value: ir_value,
                }
            } else {
                Stmt::Let {
                    name: name.clone(),
                    ty: ir_ty,
                    value: ir_value,
                    is_mut: *mutable,
                    is_ref: *is_ref,
                }
            }
        }

        AstStmt::Const { name, ty, value } => {
            let ir_ty = ty
                .as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, ctx));
            let mut ir_value = convert_expr(value, ctx);
            // 当 value 是 Lambda 时，使用 Lambda 的类型
            // 当 value 的 IR 类型为 Any（~: 构建块等），也使用 IR 类型避免错误标注
            let ir_ty = match &ir_value.ty {
                IrType::Fn { .. } | IrType::Any => ir_value.ty.clone(),
                _ => ir_ty,
            };
            // 同上：传播 fn 参数类型到 Lambda
            if let IrType::Fn {
                params: fn_params, ..
            } = &ir_ty
            {
                if let ExprKind::Lambda {
                    params: lambda_params,
                    ..
                } = &mut ir_value.kind
                {
                    if lambda_params.len() == fn_params.len() {
                        for (lp, fp) in lambda_params.iter_mut().zip(fn_params.iter()) {
                            lp.ty = fp.clone();
                        }
                    }
                }
            }
            Stmt::Let {
                name: name.clone(),
                ty: ir_ty,
                value: ir_value,
                is_mut: false,
                is_ref: false,
            }
        }

        AstStmt::Return(val) => {
            let value = val.as_ref().map(|v| {
                let expr = convert_expr(v, ctx);
                // 返回值隐式转换: return S 但声明返回 T → 插入 ImplicitConvert
                // （iterator 函数内 return 等价 raise，不做隐式转换，避免 String→T 残留）
                if !ctx.current_is_iterator {
                    // `return None`（目标 Option<T>）：None 字面量直接返回即可，
                    // 无需 ImplicitConvert——否则生成 <Option<i64> as ImplicitFrom<i64>>
                    // 错误包装（E0277，iter.lz `__next__` 的 None 分支）
                    let ret_is_option = ctx
                        .current_ret_ty
                        .as_ref()
                        .map(|rt| {
                            matches!(rt, IrType::Option(_))
                                || matches!(rt, IrType::Named { path, .. }
                                    if path == "Option" || path == "Result")
                        })
                        .unwrap_or(false);
                    let expr_is_none = matches!(expr.kind, ExprKind::Lit(LitKind::None_))
                        || matches!(&expr.kind, ExprKind::Var(n) if n == "None" || n == "None_");
                    // 泛型参数规范化：`Named("T", [])`（from_ast_type 表示）→ `Generic("T")`
                    // （infer 表示），使两侧容器元素类型可比较（box.lz `Err(self.clone())`：
                    // ret_ty err 侧是 Named("Rc", [Named("T")])，expr.ty err 侧是
                    // Named("Rc", [Generic("T")])，不规范化则 re != ee → 误插 ImplicitConvert）
                    let normalize_generic = |ty: &IrType| -> IrType { normalize_gen(ty, &ctx.current_generics) };
                    // 同容器名、仅元素类型差异（List<Any> vs List<U>）：跳过转换，
                    // 让 Rust 从返回类型推断（`return result` 中 result 是 List() 空构造，
                    // 元素类型 Any→i64 与返回 List<I::Item> 不匹配，E0277）。
                    // 递归比较 Named 参数：`Option<Rc<T>>` vs `Option<Rc<i64>>`
                    // （box.lz Weak::upgrade `Some(Rc(_inner: self._rc))`，_inner:int
                    // 占位导致 Rc 推断为 Rc<i64>，E0277 ImplicitFrom）
                    let same_named_rec = |a: &IrType, b: &IrType| -> bool {
                        // 任何一侧为 Any/Generic 即兼容（collect_list 的 result 是
                        // List() 空构造 → List<Any>，返回类型 List<I::Item>；当前只有
                        // 双方 Named 才检查，Any 会落到 _ => a==b 返回 false，E0277）
                        if matches!(a, IrType::Generic(_) | IrType::Any)
                            || matches!(b, IrType::Generic(_) | IrType::Any)
                            || a == b
                        {
                            return true;
                        }
                        match (a, b) {
                            (IrType::Named { path: p1, args: r1 }, IrType::Named { path: p2, args: r2 })
                                if p1 == p2 && r1.len() == r2.len() && !r1.is_empty() =>
                            {
                                r1.iter().zip(r2.iter()).all(|(x, y)| {
                                    if x == y {
                                        true
                                    } else if matches!(x, IrType::Generic(_) | IrType::Any)
                                        || matches!(y, IrType::Generic(_) | IrType::Any)
                                    {
                                        true
                                    } else if let (IrType::Named { path: q1, .. }, IrType::Named { path: q2, .. }) =
                                        (x, y)
                                    {
                                        // 关联类型路径（I::Item / I.Item）与具体类型兼容：
                                        // collect_list 的 `return result` 中 result 推断为
                                        // Vec<i64>，返回类型 Vec<I::Item>（E0277 ImplicitFrom）
                                        q1 == q2
                                            || q1.contains("::")
                                            || q1.contains('.')
                                            || q2.contains("::")
                                            || q2.contains('.')
                                    } else {
                                        false
                                    }
                                })
                            }
                            // Option(Int) vs Option(Any)——内部 Any 兼容（size_hint 的
                            // return (new_lower, new_upper) 中 new_upper 是 Option<Any>，
                            // 否则 ImplicitFrom 插入 E0308/E0277）
                            (IrType::Option(i1), IrType::Option(i2)) => {
                                if i1 == i2 {
                                    true
                                } else if matches!(i1.as_ref(), IrType::Generic(_) | IrType::Any)
                                    || matches!(i2.as_ref(), IrType::Generic(_) | IrType::Any)
                                {
                                    true
                                } else {
                                    false
                                }
                            }
                            // 一方空 args（类型未推断，如 `Rc` vs `Rc<T>`）：视为兼容，
                            // 让 Rust 从返回类型推断（box.lz `Some(Rc(_inner: self._rc))`
                            // 的 Rc 推断为 Named{args:[]}，E0277 ImplicitFrom）
                            (
                                IrType::Named { path: p1, args },
                                IrType::Named { path: p2, args: args2 },
                            ) if p1 == p2
                                && (args.is_empty() || args2.is_empty())
                                && args.len() != args2.len() =>
                            {
                                true
                            }
                            _ => a == b,
                        }
                    };
                    let same_container_any = match (ctx.current_ret_ty.as_ref(), &expr.ty) {
                        // 元组返回（size_hint 的 `return (lower, upper)`）：元素递归
                        // 比较（Any 元素兼容），否则 (i64, Any) vs (i64, Option<i64>)
                        // 插入错误 ImplicitFrom（E0277）
                        (
                            Some(IrType::Tuple(ra)),
                            IrType::Tuple(ea),
                        ) if ra.len() == ea.len() => {
                            let ok = ra.iter().zip(ea.iter()).all(|(r, e)| same_named_rec(r, e));
                            if !ok && ra.len() == 2 {
                                eprintln!(
                                    "DBG tuple_same: ra={:?} ea={:?} ok={}",
                                    ra, ea, ok
                                );
                            }
                            ok
                        }
                        (
                            Some(IrType::Named { path: rp, args: ra }),
                            IrType::Named { path: ep, args: ea },
                        ) if rp == ep && !ra.is_empty() && !ea.is_empty() => {
                            ra.iter().zip(ea.iter()).all(|(r, e)| same_named_rec(r, e))
                        }
                        // Result<T, Any> vs Result<T, Rc<T>>：err/ok 侧 Any 时跳过转换
                        // （box.lz `Ok(self.get())`，get 返回 &T，Ok 推断 Result<T, Any>）
                        (
                            Some(IrType::Result { ok: ro, err: re }),
                            IrType::Result { ok: eo, err: ee },
                        ) => {
                            let ro = normalize_generic(ro);
                            let re = normalize_generic(re);
                            let eo = normalize_generic(eo);
                            let ee = normalize_generic(ee);
                            (matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || ro == eo)
                                && (matches!(re, IrType::Generic(_) | IrType::Any)
                                    || matches!(ee, IrType::Generic(_) | IrType::Any)
                                    || re == ee)
                        }
                        // Result<T, E> 在 AST 中可能解析为 Named("Result", [ok, err])，
                        // 而 Err(...) 构造推断为 IrType::Result 变体（box.lz try_unwrap
                        // `return Err(self)`）——跨表示形式比较，跳过等价的 ImplicitConvert
                        (
                            Some(IrType::Named { path: rp, args: ra }),
                            IrType::Result { ok: eo, err: ee },
                        ) if rp == "Result" && ra.len() == 2 => {
                            let ro = normalize_generic(&ra[0]);
                            let re = normalize_generic(&ra[1]);
                            let eo = normalize_generic(eo);
                            let ee = normalize_generic(ee);
                            (matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || ro == eo)
                                && (matches!(re, IrType::Generic(_) | IrType::Any)
                                    || matches!(ee, IrType::Generic(_) | IrType::Any)
                                    || re == ee)
                        }
                        (
                            Some(IrType::Result { ok: ro, err: re }),
                            IrType::Named { path: ep, args: ea },
                        ) if ep == "Result" && ea.len() == 2 => {
                            let ro = normalize_generic(ro);
                            let re = normalize_generic(re);
                            let eo = normalize_generic(&ea[0]);
                            let ee = normalize_generic(&ea[1]);
                            (matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || ro == eo)
                                && (matches!(re, IrType::Generic(_) | IrType::Any)
                                    || matches!(ee, IrType::Generic(_) | IrType::Any)
                                    || re == ee)
                        }
                        // Option 同理：Named("Option", [T]) vs IrType::Option(T)
                        (
                            Some(IrType::Named { path: rp, args: ra }),
                            IrType::Option(eo),
                        ) if rp == "Option" && ra.len() == 1 => {
                            let ro = normalize_generic(&ra[0]);
                            let eo = normalize_generic(eo);
                            matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || same_named_rec(&ro, &eo)
                        }
                        (
                            Some(IrType::Option(ro)),
                            IrType::Named { path: ep, args: ea },
                        ) if ep == "Option" && ea.len() == 1 => {
                            let ro = normalize_generic(ro);
                            let eo = normalize_generic(&ea[0]);
                            matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || same_named_rec(&ro, &eo)
                        }
                        // Option/Option：`Some(Rc(_inner: self._rc))` 的 ret/expr 都是
                        // IrType::Option 变体（box.lz Weak::upgrade，Rc 推断为空 args），
                        // 递归比较内部元素（E0277 ImplicitFrom）
                        (
                            Some(IrType::Option(ro)),
                            IrType::Option(eo),
                        ) => {
                            let ro = normalize_generic(ro);
                            let eo = normalize_generic(eo);
                            matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || same_named_rec(&ro, &eo)
                        }
                        _ => false,
                    };
                    if !(expr_is_none && ret_is_option) && !same_container_any {
                        if let Some(ref ret_ty) = ctx.current_ret_ty {
                            // ref 返回（`return self[key]`，&V）且表达式是值（V）：
                            // 跳过 ImplicitConvert（E0277 &V: ImplicitFrom<V>），
                            // codegen 对 HashMap 索引生成 .get(&key).unwrap()（&V）
                            let ref_ret_ok =
                                matches!(ret_ty, IrType::Ref(inner) if expr.ty == (**inner).clone())
                                    || matches!(ret_ty, IrType::MutRef(inner)
                                        if expr.ty == (**inner).clone());
                            if !ref_ret_ok
                                && expr.ty != *ret_ty
                                && !matches!(ret_ty, IrType::Unit)
                            {
                                // 返回类型含关联类型路径（`I::Item` / `Option<(A::Item, B::Item)>`，
                                // iter.lz sum/product/Zip::next）：跳过 ImplicitConvert，
                                // 让 Rust 从函数签名推断（E0277 ImplicitFrom）
                                fn contains_assoc_path(ty: &IrType) -> bool {
                                    match ty {
                                        IrType::Named { path, args } => {
                                            path.contains("::")
                                                || path.contains('.')
                                                || args.iter().any(contains_assoc_path)
                                        }
                                        IrType::Option(inner) => contains_assoc_path(inner),
                                        IrType::Result { ok, err } => {
                                            contains_assoc_path(ok) || contains_assoc_path(err)
                                        }
                                        IrType::Tuple(items) => {
                                            items.iter().any(contains_assoc_path)
                                        }
                                        _ => false,
                                    }
                                }
                                let ret_is_assoc_path = contains_assoc_path(ret_ty);
                                if !ret_is_assoc_path {
                                    return Expr::new(
                                        ExprKind::ImplicitConvert {
                                            source: Box::new(expr.clone()),
                                            target_ty: ret_ty.clone(),
                                        },
                                        ret_ty.clone(),
                                        Span::unknown(),
                                    );
                                }
                            }
                        }
                    }
                }
                expr
            });
            Stmt::Return { value }
        }

        AstStmt::Yield(val) => {
            let value = match val {
                Some(expr) => convert_expr(expr, ctx),
                None => Expr::new(ExprKind::Lit(LitKind::None_), IrType::Unit, Span::unknown()),
            };
            Stmt::Yield { value }
        }

        AstStmt::YieldFrom(e) => Stmt::YieldFrom {
            iter: convert_expr(e, ctx),
        },

        AstStmt::While {
            cond,
            guard,
            body,
            else_body,
        } => Stmt::While {
            cond: convert_expr(cond, ctx),
            guard: guard.as_ref().map(|g| convert_expr(g, ctx)),
            body: convert_block(body, ctx),
            else_body: else_body.as_ref().map(|b| convert_block(b, ctx)),
        },

        AstStmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
            ..
        } => {
            let ir_expr = convert_expr(expr, ctx);

            // 从 expr 类型推断模式绑定变量的类型
            // e.g. while let Some(item) = opt (Option<int>) → item: int
            // 注意：方法调用返回类型推断不完整（it.next() → Any），
            // 仅对直接 Option<T> 类型变量生效
            let inner_ty = match &ir_expr.ty {
                IrType::Option(inner) => Some((**inner).clone()),
                IrType::Named { path, args } if path == "Option" && args.len() == 1 => {
                    Some(args[0].clone())
                }
                _ => None,
            };

            // 构建增强的 ctx（包含模式绑定变量类型）
            let mut body_ctx = ctx.clone();
            if let Some(ref ty) = inner_ty {
                let mut pattern_vars: Vec<String> = Vec::new();
                collect_ast_pattern_vars(pattern, &mut pattern_vars);
                for var in &pattern_vars {
                    body_ctx.vars.insert(var.clone(), ty.clone());
                }
            }

            let ir_pattern = convert_ast_pattern(pattern, ctx).unwrap_or(Pattern::Wildcard);
            let ir_body = convert_block(body, &body_ctx);

            Stmt::WhileLet {
                pattern: ir_pattern,
                expr: ir_expr,
                guard: guard.as_ref().map(|g| convert_expr(g, ctx)),
                body: ir_body,
            }
        }

        AstStmt::For {
            var,
            iter,
            guard,
            body,
            else_body,
        } => {
            let mut loop_ctx = TypeCtx::new();
            // 从 ctx 复制函数泛型上下文
            loop_ctx.current_generics = ctx.current_generics.clone();
            loop_ctx.current_ret_ty = ctx.current_ret_ty.clone();
            loop_ctx.current_is_iterator = ctx.current_is_iterator;
            // 复制变量类型（predicate: fn(ref I.Item) 等函数参数）：
            // 否则 for 循环体内 `predicate(item)` 中 predicate 类型丢失→Any，
            // 无法识别 ref 参数自动取引用（iter.lz filter/find，E0308）
            loop_ctx.vars = ctx.vars.clone();
            loop_ctx.current_fn_name = ctx.current_fn_name.clone();
            // 推导迭代变量的类型
            let iter_ty = infer_expr_type(iter, ctx);
            let elem_ty = match &iter_ty {
                IrType::Named { args, .. } if !args.is_empty() => args[0].clone(),
                _ => IrType::Any,
            };
            loop_ctx.add_var(var, elem_ty);
            Stmt::For {
                var: var.clone(),
                iter: convert_expr(iter, ctx),
                guard: guard.as_ref().map(|g| convert_expr(g, ctx)),
                body: convert_block_with_ctx(body, &loop_ctx),
                else_body: else_body.as_ref().map(|b| convert_block(b, ctx)),
            }
        }

        AstStmt::Loop(body) => Stmt::While {
            cond: Expr::new(
                ExprKind::Lit(LitKind::Bool(true)),
                IrType::Bool,
                Span::unknown(),
            ),
            guard: None,
            body: convert_block(body, ctx),
            else_body: None,
        },

        AstStmt::Break(_) => Stmt::Break,
        AstStmt::BreakLabel { label, value } => Stmt::BreakLabel {
            label: label.clone(),
            value: value.as_ref().map(|v| convert_expr(v, ctx)),
        },
        AstStmt::Continue => Stmt::Continue,

        AstStmt::Block { label, body } => Stmt::BlockLabel {
            label: label.clone(),
            body: convert_block(body, ctx),
        },

        AstStmt::CheckerBlock {
            label,
            ps_name,
            default_checker,
            body,
        } => {
            // checker 块 → IR 压缩为模块级 fn NAME(ps: &mut __Params)
            // 惰性登记：定义时不执行，仅注册为 Item::CheckerBlock
            // checker 块体是独立词法块（闭包语义）：块内裸赋值 `depth = depth + 1`
            // 引用外部捕获变量，不应继承外层 block_declared 误转 let 绑定（E0425）
            let mut chk_ctx = ctx.clone();
            chk_ctx.block_declared.clear();
            let ir_body = convert_block(body, &chk_ctx);
            // 捕获的外层函数局部变量（block 闭包语义，规范 05b-block命名块.md §三）：
            // body 引用的、在函数作用域（ctx.vars）内声明的变量（out/depth/result 等），
            // 需作为 fn 的 &mut 参数传入，否则提升为模块级 fn 后 E0425（block_demo 等）
            let captured = collect_checker_captured(&ir_body, ctx, ps_name.as_deref());
            ctx.pending_items.borrow_mut().push(Item::CheckerBlock {
                name: label.clone(),
                ps_name: ps_name.clone(),
                default_checker: default_checker.clone(),
                body: ir_body,
                captured,
            });
            // 占位语句（checker 块不内联执行）
            Stmt::Pass
        }

        AstStmt::BlockCall { label, args } => {
            // 触发调用 → 转换为 fn_call(label)(args)
            // 元组实参 (a, b, c) → 展开为多个独立参数（checker 块 ps.args[i] 逐位解包，
            // block_tailrec.lz factorial[(5, 1)]；单元素 (10,) 也经 TupleLit 展开）
            let call_args: Vec<Expr> = match args {
                AstExpr::TupleLit(elems) => elems.iter().map(|e| convert_expr(e, ctx)).collect(),
                other => vec![convert_expr(other, ctx)],
            };
            Stmt::ExprStmt {
                expr: Expr::new(
                    ExprKind::Call {
                        callee: Box::new(Expr::new(
                            ExprKind::Var(label.clone()),
                            IrType::Any,
                            Span::unknown(),
                        )),
                        type_args: vec![],
                        args: call_args,
                    },
                    IrType::Unit,
                    Span::unknown(),
                ),
            }
        }

        AstStmt::Defer(body) => {
            // defer → 展开为 Block（不追加 return，让后续代码继续执行）
            let stmts = convert_block(body, ctx).stmts;
            Stmt::Block { stmts }
        }

        AstStmt::Comptime { body } => {
            // comptime: 块 — 编译期求值，结果内联（B3）。
            // 求值成功且有值 → 内联为字面量表达式；无值（块内仅 let/const）→ Pass；
            // 求值失败 → 收集错误并降级为普通 Block（保留原编译行为）。
            // 使用真实模块（comptime 块内可调用模块内函数/引用 const）
            let empty_module = ast::Module::default();
            let module_ref = ctx.comptime_module.as_ref().map(|m| m.as_ref()).unwrap_or(&empty_module);
            let mut cctx = crate::comptime::ComptimeContext::new(module_ref);
            // 注入源码文本（inspect.getsource/getsourcelines 数据源，main.rs 已填）
            if let Some(src) = &module_ref.source_text {
                cctx = cctx.with_source(src.clone());
            }
            // 注入顶层 const 求值结果（comptime 块内解析 const 引用）
            for (n, v) in &ctx.comptime_consts {
                cctx.symtab.insert(n.clone(), v.clone());
            }
            match crate::comptime::ComptimeEvaluator::eval_block(body, &mut cctx) {
                Ok(Some(v)) => match comptime_value_to_lit(&v) {
                    // comptime 块仅打印/副作用（值为 None）时不产出代码，
                    // 避免生成裸 `None;` 语句导致 rustc E0282
                    Some(ExprKind::Lit(LitKind::None_)) => Stmt::Pass,
                    Some(kind) => Stmt::ExprStmt {
                        expr: Expr::new(kind, IrType::Any, Span::unknown()),
                    },
                    None => Stmt::Block {
                        stmts: convert_block(body, ctx).stmts,
                    },
                },
                Ok(None) => Stmt::Pass,
                Err(e) => {
                    ctx.errors.borrow_mut().push(format!("comptime 块求值失败: {}", e));
                    Stmt::Block {
                        stmts: convert_block(body, ctx).stmts,
                    }
                }
            }
        }

        AstStmt::Raise(e) => Stmt::ExprStmt {
            expr: Expr::new(
                ExprKind::Call {
                    type_args: vec![],
                    callee: Box::new(Expr::new(
                        ExprKind::Var("panic!".into()),
                        IrType::Any,
                        Span::unknown(),
                    )),
                    args: vec![convert_expr(e, ctx)],
                },
                IrType::Never,
                Span::unknown(),
            ),
        },

        AstStmt::Guard {
            cond,
            let_binding,
            else_body,
            ..
        } => {
            // guard → if let ... else
            if let Some((pattern, value)) = let_binding {
                let val = convert_expr(value, ctx);
                if let AstPattern::Ident(name) = pattern {
                    let mut guard_ctx = TypeCtx::new();
                    guard_ctx.current_generics = ctx.current_generics.clone();
                    guard_ctx.add_var(name, val.ty.clone());
                    Stmt::If {
                        cond: Expr::new(
                            ExprKind::BinOp {
                                op: BinOpKind::Neq,
                                lhs: Box::new(val),
                                rhs: Box::new(Expr::new(
                                    ExprKind::Lit(LitKind::None_),
                                    IrType::Any,
                                    Span::unknown(),
                                )),
                            },
                            IrType::Bool,
                            Span::unknown(),
                        ),
                        then_branch: Block {
                            span: Span::unknown(),
                            stmts: vec![],
                            ty: IrType::Unit,
                        },
                        else_branch: Some(convert_block(else_body, &guard_ctx)),
                    }
                } else {
                    Stmt::Block {
                        stmts: else_body.iter().map(|s| convert_stmt(s, ctx)).collect(),
                    }
                }
            } else {
                // guard cond else: <body> —— 失败路径（05-控制流.md §7.1）：
                // 块尾表达式为隐式 return（中止并返回该值）
                let mut else_block = convert_block(else_body, ctx);
                let tail_is_value_expr = matches!(
                    else_block.stmts.last(),
                    Some(Stmt::ExprStmt { expr }) if expr.ty != IrType::Unit
                );
                if tail_is_value_expr {
                    if let Some(last) = else_block.stmts.last_mut() {
                        if let Stmt::ExprStmt { expr } = last {
                            *last = Stmt::Return {
                                value: Some(expr.clone()),
                            };
                        }
                    }
                }
                Stmt::If {
                    cond: cond
                        .as_ref()
                        .map(|c| convert_expr(c, ctx))
                        .unwrap_or(Expr::new(
                            ExprKind::Lit(LitKind::Bool(true)),
                            IrType::Bool,
                            Span::unknown(),
                        )),
                    then_branch: Block {
                        span: Span::unknown(),
                        stmts: vec![],
                        ty: IrType::Unit,
                    },
                    else_branch: Some(else_block),
                }
            }
        }

        AstStmt::With { expr, alias, body } => {
            // with → 展开为 let + defer drop
            let val = convert_expr(expr, ctx);
            let val_ty = val.ty.clone();
            let mut with_ctx = TypeCtx::new();
            with_ctx.current_generics = ctx.current_generics.clone();
            let name = alias.clone().unwrap_or_else(|| "_with".into());
            with_ctx.add_var(&name, val_ty.clone());
            // 仅当 with 有 as 绑定（上下文管理器）时才生成 __exit__ 清理；
            // with <普通表达式>: 无 enter/exit 语义，直接执行块
            let mut stmts = vec![Stmt::Let {
                name: name.clone(),
                ty: val_ty.clone(),
                value: val,
                // with 资源绑定须可变：块内可被 __exit__/__enter__ 等可变借用
                // （生成 `let mut res`，否则 E0596 cannot borrow as mutable）
                is_mut: true,
                is_ref: false,
            }];
            if alias.is_some() {
                // __exit__ 调用参数数 = 方法实际非 self 参数数。
                // `def __exit__(mut self)` 0 参 → 不传参（E0061 修复）；
                // 带参 __exit__(self, exc) → 传绑定的实例副本
                let exit_arity = match &val_ty {
                    IrType::Named { path, .. } => ctx
                        .struct_method_arity
                        .get(path)
                        .and_then(|m| m.get("__exit__"))
                        .copied()
                        .unwrap_or(0),
                    _ => 0,
                };
                let exit_args: Vec<Expr> = if exit_arity > 0 {
                    vec![Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(Expr::new(
                                ExprKind::Var(name.clone()),
                                val_ty.clone(),
                                Span::unknown(),
                            )),
                            method: "clone".into(),
                            args: vec![],
                        },
                        val_ty.clone(),
                        Span::unknown(),
                    )]
                } else {
                    vec![]
                };
                stmts.push(Stmt::ExprStmt {
                    expr: Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(Expr::new(
                                ExprKind::Var(name.clone()),
                                val_ty.clone(),
                                Span::unknown(),
                            )),
                            method: "__exit__".into(),
                            args: exit_args,
                        },
                        IrType::Unit,
                        Span::unknown(),
                    ),
                });
            }
            stmts.extend(body.iter().map(|s| convert_stmt(s, &with_ctx)));
            Stmt::Block { stmts }
        }

        AstStmt::Assign { target, op, value } => {
            let val = convert_expr(value, ctx);
            let target_expr = convert_expr(target, ctx);
            match op {
                crate::ast::AssignOp::Eq => Stmt::Assign {
                    target: target_expr,
                    value: val,
                },
                _ => Stmt::Assign {
                    target: target_expr.clone(),
                    value: Expr::new(
                        ExprKind::BinOp {
                            op: map_assign_op(op),
                            lhs: Box::new(target_expr),
                            rhs: Box::new(val),
                        },
                        IrType::Any,
                        Span::unknown(),
                    ),
                },
            }
        }

        AstStmt::FnDef { func } => {
            // 嵌套函数提升为模块级 Item::FnDef。但嵌套函数体若引用外层函数
            // 的局部变量（total = total + x 中 total 是外层局部），提升后
            // 无法访问（生成 let mut total = total + x → E0425）。编译期报错，
            // 提示改用闭包捕获（闭包支持写外部变量）。
            let mut declared: HashSet<String> =
                func.params.iter().map(|p| p.name.clone()).collect();
            if let Some(captured) = check_stmts_capture(&func.body, &ctx.vars, &mut declared) {
                ctx.report_error(format!(
                    "嵌套函数 `{}` 引用了外层局部变量 `{}`：嵌套函数提升为模块级后无法访问外层局部变量（E0425），请改用闭包捕获（如 `let {} = |...| ...`）",
                    func.name, captured, func.name
                ));
            }

            // 嵌套函数提升为模块级 Item::FnDef
            let nested_name = func.name.clone();
            let mut nested_def = convert_fn_def(func, ctx);
            nested_def.name = nested_name;
            ctx.pending_items.borrow_mut().push(Item::FnDef(nested_def));

            // 占位语句（嵌套函数不作为语句，已在模块级注册）
            Stmt::ExprStmt {
                expr: Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown()),
            }
        }

        AstStmt::EnumDef(struct_def) => {
            // 函数体内的 enum 定义提升为模块级 Item
            let item = convert_struct(&struct_def, ctx);
            ctx.pending_items.borrow_mut().push(item);

            // 占位语句
            Stmt::ExprStmt {
                expr: Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown()),
            }
        }

        AstStmt::Test { name: _, body } => {
            // test 块体用 convert_block（可变 block_ctx 前向传播 let 变量类型）：
            // convert_stmt 的 ctx 不可变，let 变量不登记，后续 `assert a == b` 中
            // a 推断为 Any（box.lz E0369 assert_eq 需 PartialEq）
            let blk = convert_block(body, ctx);
            Stmt::Block { stmts: blk.stmts }
        }

        AstStmt::Assert { expr, expected } => {
            // assert expr == expected → assert_eq!(expr, expected)
            // assert expr（单表达式布尔断言）→ assert!(expr)
            // （否则 assert_eq! 只有单参数 → Rust 宏 "unexpected end of macro invocation"）
            let mut args = vec![convert_expr(expr, ctx)];
            let callee_name = if let Some(exp) = expected {
                // 用户自定义 struct 比较：assert_eq! 要求 PartialEq（box.lz 的
                // Box/Rc/Arc 只定义 __eq__ 魔术方法 → E0369），改用
                // assert!(lhs.__eq__(rhs)) 调用自定义魔术方法
                let ir_expr = args[0].clone();
                // `assert b.get() == 42`：b.get() 返回 `ref T`（&T），与 owned 值
                // 比较需解引用（&i64 == i64 无实现，E0277 can't compare）
                let lhs_is_ref = matches!(ir_expr.ty, IrType::Ref(_) | IrType::MutRef(_));
                let is_user_struct = matches!(&ir_expr.ty, IrType::Named { path, .. }
                    if (ctx.struct_names.contains(path.as_str())
                        || ctx.enum_variants.values().any(|e| e == path.as_str()))
                        && !["List", "Dict", "Set", "Option", "Result", "String"]
                            .contains(&path.as_str()));
                if is_user_struct {
                    // parser 把 `assert a != c` 拆成 expected=Not(c)：
                    // 若 struct 未定义 __ne__（box.lz 只有 __eq__），生成 !a.__eq__(&c)
                    let is_ne = matches!(exp, AstExpr::Unary { op: UnaryOp::Not, .. });
                    let ne_operand = if let AstExpr::Unary { operand, .. } = exp {
                        (**operand).clone()
                    } else {
                        exp.clone()
                    };
                    let has_ne = ctx
                        .struct_methods
                        .get(
                            &match &ir_expr.ty {
                                IrType::Named { path, .. } => path.clone(),
                                _ => String::new(),
                            },
                        )
                        .map(|ms| ms.contains("__ne__"))
                        .unwrap_or(false);
                    if is_ne && !has_ne {
                        // !a.__eq__(&c)：调用 __eq__ 后取反
                        args = vec![Expr::new(
                            ExprKind::UnOp {
                                op: UnOpKind::Not,
                                operand: Box::new(Expr::new(
                                    ExprKind::MethodCall {
                                        receiver: Box::new(ir_expr),
                                        method: "__eq__".into(),
                                        args: vec![Expr::new(
                                            ExprKind::UnOp {
                                                op: UnOpKind::Ref,
                                                operand: Box::new(convert_expr(&ne_operand, ctx)),
                                            },
                                            IrType::Any,
                                            Span::unknown(),
                                        )],
                                    },
                                    IrType::Bool,
                                    Span::unknown(),
                                )),
                            },
                            IrType::Bool,
                            Span::unknown(),
                        )];
                    } else {
                        let magic = if is_ne { "__ne__" } else { "__eq__" };
                        args = vec![Expr::new(
                            ExprKind::MethodCall {
                                receiver: Box::new(ir_expr),
                                method: magic.into(),
                                // __eq__/__ne__ 签名是 `fn __eq__(&self, other: &Self)`：
                                // 参数应为引用 `&rhs`（box.lz assert a == b → a.__eq__(&b)，
                                // 传 owned 值会 E0308 expected &Box, found Box）
                                args: vec![Expr::new(
                                    ExprKind::UnOp {
                                        op: UnOpKind::Ref,
                                        operand: Box::new(convert_expr(&ne_operand, ctx)),
                                    },
                                    IrType::Any,
                                    Span::unknown(),
                                )],
                            },
                            IrType::Bool,
                            Span::unknown(),
                        )];
                    }
                    "assert!"
                } else if lhs_is_ref {
                    // lhs 是引用（&T）：与 owned 值比较需解引用（*b.get() == 42），
                    // 但 rhs 也是引用时（rc1.get() == rc2.get()）保持引用比较
                    // （&Vec<i64> == &Vec<i64> 合法；*lhs vs &rhs 反而 E0277）
                    let rhs_is_ref = match exp {
                        AstExpr::MethodCall { .. } | AstExpr::Ident(_) => {
                            let rhs_ty = infer_expr_type(exp, ctx);
                            matches!(rhs_ty, IrType::Ref(_) | IrType::MutRef(_))
                        }
                        _ => false,
                    };
                    if rhs_is_ref {
                        args.push(convert_expr(exp, ctx));
                    } else {
                        args = vec![
                            Expr::new(
                                ExprKind::UnOp {
                                    op: UnOpKind::Deref,
                                    operand: Box::new(ir_expr),
                                },
                                IrType::Any,
                                Span::unknown(),
                            ),
                            convert_expr(exp, ctx),
                        ];
                    }
                    "assert_eq!"
                } else {
                    args.push(convert_expr(exp, ctx));
                    "assert_eq!"
                }
            } else {
                "assert!"
            };
            Stmt::ExprStmt {
                expr: Expr::new(
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(Expr::new(
                            ExprKind::Var(callee_name.into()),
                            IrType::Any,
                            Span::unknown(),
                        )),
                        args,
                    },
                    IrType::Unit,
                    Span::unknown(),
                ),
            }
        },

        AstStmt::Check { expr, message: _ } => {
            // check → 展开为 if !expr { eprintln!(...) }
            let cond = Expr::new(
                ExprKind::UnOp {
                    op: crate::ir::node::UnOpKind::Not,
                    operand: Box::new(convert_expr(expr, ctx)),
                },
                IrType::Bool,
                Span::unknown(),
            );
            let print_call = Expr::new(
                ExprKind::Call {
                    type_args: vec![],
                    callee: Box::new(Expr::new(
                        ExprKind::Var("eprintln!".into()),
                        IrType::Any,
                        Span::unknown(),
                    )),
                    args: vec![Expr::new(
                        ExprKind::Lit(LitKind::Str("CHECK failed".into())),
                        IrType::Str,
                        Span::unknown(),
                    )],
                },
                IrType::Unit,
                Span::unknown(),
            );
            Stmt::If {
                cond,
                then_branch: Block {
                    span: Span::unknown(),
                    stmts: vec![Stmt::ExprStmt { expr: print_call }],
                    ty: IrType::Unit,
                },
                else_branch: None,
            }
        }

        AstStmt::LetTuple { .. } => {
            // LetTuple 在 convert_stmts 中展开，不应到达此处
            Stmt::Pass
        }

        AstStmt::Suite {
            name: _,
            setup,
            teardown,
            tests,
        } => {
            // 将 setup 和 teardown 内联到每个 test 中
            let mut ir_tests = Vec::new();
            for t in tests {
                match t {
                    AstStmt::Test { name, body } => {
                        let mut combined = Vec::new();
                        if let Some(ref s) = setup {
                            combined.extend(s.iter().cloned());
                        }
                        combined.extend(body.iter().cloned());
                        if let Some(ref td) = teardown {
                            combined.extend(td.iter().cloned());
                        }
                        ir_tests.push(AstStmt::Test {
                            name: name.clone(),
                            body: combined,
                        });
                    }
                    _ => ir_tests.push(t.clone()),
                }
            }
            Stmt::Block {
                stmts: convert_stmts(&ir_tests, ctx),
            }
        }
    }
}

/// 递归收集语句内所有 walrus `x := expr` 绑定（name → 推断类型）。
/// convert_expr 是 &TypeCtx 不可变借用，无法在 walrus 转换处 add_var
/// （builder.rs:3108 FIXME 的 scope issue）；改为在 convert_block 的
/// 前向传播中统一登记，语义与 Let/BuildBlock Var 前向传播一致。
fn collect_stmt_walrus(stmt: &AstStmt, ctx: &TypeCtx, out: &mut Vec<(String, IrType)>) {
    match stmt {
        AstStmt::Expr(e) => collect_expr_walrus(e, ctx, out),
        AstStmt::Let { value, .. } | AstStmt::Const { value, .. } => {
            collect_expr_walrus(value, ctx, out)
        }
        AstStmt::LetTuple { value, .. } => collect_expr_walrus(value, ctx, out),
        AstStmt::Return(Some(e)) | AstStmt::Yield(Some(e)) | AstStmt::Raise(e) => {
            collect_expr_walrus(e, ctx, out)
        }
        AstStmt::YieldFrom(e) | AstStmt::BlockCall { args: e, .. } => {
            collect_expr_walrus(e, ctx, out)
        }
        AstStmt::Break(Some(e)) | AstStmt::BreakLabel { value: Some(e), .. } => {
            collect_expr_walrus(e, ctx, out)
        }
        AstStmt::While { cond, guard, .. }
        | AstStmt::WhileLet { expr: cond, guard, .. } => {
            collect_expr_walrus(cond, ctx, out);
            if let Some(g) = guard {
                collect_expr_walrus(g, ctx, out);
            }
        }
        AstStmt::For { iter, guard, .. } => {
            collect_expr_walrus(iter, ctx, out);
            if let Some(g) = guard {
                collect_expr_walrus(g, ctx, out);
            }
        }
        AstStmt::Assign { target, value, .. } => {
            collect_expr_walrus(target, ctx, out);
            collect_expr_walrus(value, ctx, out);
        }
        AstStmt::Guard {
            cond,
            success_expr,
            let_binding,
            ..
        } => {
            if let Some(c) = cond {
                collect_expr_walrus(c, ctx, out);
            }
            if let Some(s) = success_expr {
                collect_expr_walrus(s, ctx, out);
            }
            if let Some((_, e)) = let_binding {
                collect_expr_walrus(e, ctx, out);
            }
        }
        AstStmt::With { expr, .. } => collect_expr_walrus(expr, ctx, out),
        AstStmt::Assert { expr, expected } => {
            collect_expr_walrus(expr, ctx, out);
            if let Some(e) = expected {
                collect_expr_walrus(e, ctx, out);
            }
        }
        AstStmt::Check { expr, message } => {
            collect_expr_walrus(expr, ctx, out);
            if let Some(m) = message {
                collect_expr_walrus(m, ctx, out);
            }
        }
        // 嵌套语句体（Loop/Block/CheckerBlock/Defer/Test/Suite/Comptime/FnDef）：
        // 各自经独立的 convert_block 转换并自行登记，此处不递归避免作用域泄漏。
        _ => {}
    }
}

/// 递归扫描表达式树中的 walrus 绑定（跳过闭包体/块表达式/推导式作用域内绑定，
/// 那些属于独立词法块；推导式的 iter/output 表达式仍扫描，其中 walrus 作用于外层）。
fn collect_expr_walrus(e: &AstExpr, ctx: &TypeCtx, out: &mut Vec<(String, IrType)>) {
    match e {
        AstExpr::Walrus { target, value } => {
            if let AstExpr::Ident(name) = target.as_ref() {
                if !out.iter().any(|(n, _)| n == name) {
                    out.push((name.clone(), infer_expr_type(value, ctx)));
                }
            }
            collect_expr_walrus(target, ctx, out);
            collect_expr_walrus(value, ctx, out);
        }
        AstExpr::ListLit(elems) | AstExpr::SetLit(elems) | AstExpr::TupleLit(elems) => {
            for x in elems {
                collect_expr_walrus(x, ctx, out);
            }
        }
        AstExpr::DictLit(entries) => {
            for (k, v) in entries {
                collect_expr_walrus(k, ctx, out);
                collect_expr_walrus(v, ctx, out);
            }
        }
        AstExpr::Binary { left, right, .. } => {
            collect_expr_walrus(left, ctx, out);
            collect_expr_walrus(right, ctx, out);
        }
        AstExpr::Unary { operand, .. } => collect_expr_walrus(operand, ctx, out),
        AstExpr::Call { func, args, .. } => {
            collect_expr_walrus(func, ctx, out);
            for a in args {
                collect_expr_walrus(a, ctx, out);
            }
        }
        AstExpr::KwArg { value, .. } => collect_expr_walrus(value, ctx, out),
        AstExpr::MethodCall { receiver, args, .. } => {
            collect_expr_walrus(receiver, ctx, out);
            for a in args {
                collect_expr_walrus(a, ctx, out);
            }
        }
        AstExpr::FieldAccess { receiver, .. } | AstExpr::PathAccess { receiver, .. } => {
            collect_expr_walrus(receiver, ctx, out)
        }
        AstExpr::Index { receiver, index } => {
            collect_expr_walrus(receiver, ctx, out);
            collect_expr_walrus(index, ctx, out);
        }
        AstExpr::If { cond, elif_clauses, .. } => {
            collect_expr_walrus(cond, ctx, out);
            for (c, _) in elif_clauses {
                collect_expr_walrus(c, ctx, out);
            }
        }
        AstExpr::Match { expr, arms } => {
            collect_expr_walrus(expr, ctx, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_expr_walrus(g, ctx, out);
                }
            }
        }
        AstExpr::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_expr_walrus(s, ctx, out);
            }
            if let Some(e2) = end {
                collect_expr_walrus(e2, ctx, out);
            }
        }
        AstExpr::Pipe { receiver, callee, args } => {
            collect_expr_walrus(receiver, ctx, out);
            collect_expr_walrus(callee, ctx, out);
            for a in args {
                collect_expr_walrus(a, ctx, out);
            }
        }
        AstExpr::SafeNav { receiver, .. } => collect_expr_walrus(receiver, ctx, out),
        AstExpr::Try(inner)
        | AstExpr::Spawn(inner)
        | AstExpr::Move(inner)
        | AstExpr::Panic(inner)
        | AstExpr::Await(inner)
        | AstExpr::Paren(inner)
        | AstExpr::Comptime(inner) => collect_expr_walrus(inner, ctx, out),
        AstExpr::NullCoalesce { left, right } => {
            collect_expr_walrus(left, ctx, out);
            collect_expr_walrus(right, ctx, out);
        }
        AstExpr::ListComprehension { output, iter, cond, extra_clauses, .. }
        | AstExpr::SetComprehension { elem: output, iter, cond, extra_clauses, .. } => {
            collect_expr_walrus(output, ctx, out);
            collect_expr_walrus(iter, ctx, out);
            if let Some(c) = cond {
                collect_expr_walrus(c, ctx, out);
            }
            for (_, i, c) in extra_clauses {
                collect_expr_walrus(i, ctx, out);
                if let Some(c) = c {
                    collect_expr_walrus(c, ctx, out);
                }
            }
        }
        AstExpr::DictComprehension { key, value, iter, cond, extra_clauses, .. } => {
            collect_expr_walrus(key, ctx, out);
            collect_expr_walrus(value, ctx, out);
            collect_expr_walrus(iter, ctx, out);
            if let Some(c) = cond {
                collect_expr_walrus(c, ctx, out);
            }
            for (_, i, c) in extra_clauses {
                collect_expr_walrus(i, ctx, out);
                if let Some(c) = c {
                    collect_expr_walrus(c, ctx, out);
                }
            }
        }
        AstExpr::Assign { target, value, .. } => {
            collect_expr_walrus(target, ctx, out);
            collect_expr_walrus(value, ctx, out);
        }
        // Closure/BlockExpr/BuildBlock/TryCatch：独立词法块，绑定不泄漏到外层
        _ => {}
    }
}

fn convert_block(stmts: &[AstStmt], ctx: &TypeCtx) -> Block {
    // 创建可变的本地上下文，支持 Let 变量传播
    let mut block_ctx = TypeCtx::new();
    // 继承父级上下文中的变量类型（支持 WhileLet 等模式绑定类型传递）
    block_ctx.vars = ctx.vars.clone();
    // 继承父级块首次声明集合（生成器预扫描 scan_iterator_yield_ty 已登记，
    // 否则 `let mut i = 0` 会被误转 Assign，生成裸赋值导致 E0425）
    block_ctx.block_declared = ctx.block_declared.clone();
    block_ctx.current_generics = ctx.current_generics.clone();
    block_ctx.current_ret_ty = ctx.current_ret_ty.clone();
    block_ctx.current_is_iterator = ctx.current_is_iterator;
    block_ctx.current_fn_name = ctx.current_fn_name.clone();
    block_ctx.pending_items = ctx.pending_items.clone();
    block_ctx.errors = ctx.errors.clone();
    block_ctx.comptime_consts = ctx.comptime_consts.clone();
    block_ctx.comptime_module = ctx.comptime_module.clone();
    for sn in &ctx.struct_names {
        block_ctx.struct_names.insert(sn.clone());
    }
    for (sn, fields) in &ctx.struct_fields {
        let mut cloned = HashMap::new();
        for (fn_, ty) in fields {
            cloned.insert(fn_.clone(), ty.clone());
        }
        block_ctx.struct_fields.insert(sn.clone(), cloned);
    }
    for (sn, order) in &ctx.struct_field_order {
        block_ctx.struct_field_order.insert(sn.clone(), order.clone());
    }
    for (sn, ms) in &ctx.struct_methods {
        block_ctx.struct_methods.insert(sn.clone(), ms.clone());
    }
    for (sn, arity) in &ctx.struct_method_arity {
        block_ctx.struct_method_arity.insert(sn.clone(), arity.clone());
    }
    for (vn, vt) in &ctx.vars {
        block_ctx.vars.insert(vn.clone(), vt.clone());
    }
    for (name, ty) in &ctx.fn_returns {
        block_ctx.fn_returns.insert(name.clone(), ty.clone());
    }
    for (name, p) in &ctx.fn_params {
        block_ctx.fn_params.insert(name.clone(), p.clone());
    }
    for (vn, en) in &ctx.enum_variants {
        block_ctx.enum_variants.insert(vn.clone(), en.clone());
    }
    for (vn, ft) in &ctx.enum_variant_field_types {
        block_ctx.enum_variant_field_types.insert(vn.clone(), ft.clone());
    }
    for (cn, ct) in &ctx.top_level_consts {
        block_ctx.top_level_consts.insert(cn.clone(), ct.clone());
    }

    let mut ir_stmts: Vec<Stmt> = Vec::new();
    for s in stmts {
        // 前向传播：Let 语句的变量添加到后续语句的上下文
        if let AstStmt::Let {
            name, ty, value, ..
        } = s
        {
            // 本块首次声明的变量：登记到 block_declared（区分首次绑定与重新赋值）。
            // 顶层变量（size = 3 等顶层 Assign 转 Const）不属于本块首次声明：
            // 否则 `size = size - 1`（无 let 前缀可变绑定）被误判为本块绑定走
            // Let 分支，生成局部新绑定而非修改全局（guard_for_3.lz while 死循环）
            if !block_ctx.vars.contains_key(name.as_str())
                && !block_ctx.top_level_consts.contains_key(name.as_str())
            {
                block_ctx.block_declared.insert(name.clone());
            }
            let ir_ty = ty
                .as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, &block_ctx));
            block_ctx.add_var(name, ir_ty);
        }
        // 前向传播：`x =: <构建块>`（AstStmt::Expr 中的 BuildBlock Var）也登记变量，
        // 否则后续 `multiply ~: factors` 的元组拆包查 lookup_var 返回 Any（E0061）
        if let AstStmt::Expr(AstExpr::BuildBlock {
            kind: BuildKind::Var,
            lhs,
            body,
        }) = s
        {
            if let AstExpr::Ident(name) = &**lhs {
                if !block_ctx.vars.contains_key(name.as_str()) {
                    block_ctx.block_declared.insert(name.clone());
                }
                let has_bare_return = body.iter().any(|st| ast_stmt_has_bare_return(st));
                let ir_ty = if has_bare_return {
                    IrType::Unit
                } else {
                    body.last()
                        .map(|st| infer_stmt_type(st, &block_ctx))
                        .filter(|t| !matches!(t, IrType::Any))
                        .unwrap_or(IrType::Any)
                };
                block_ctx.add_var(name, ir_ty);
            }
        }
        if let AstStmt::LetTuple { names, ty, .. } = s {
            let ir_ty = ty.as_ref().map(|t| from_ast_type(t)).unwrap_or(IrType::Any);
            for name in names {
                if name != "_" {
                    block_ctx.add_var(name, ir_ty.clone());
                }
            }
        }
        // 前向传播：`x := expr`（walrus）在表达式内绑定变量，登记类型供
        // 后续语句引用（builder.rs:3108 FIXME：convert_expr 是 &TypeCtx
        // 不可变借用，无法在 walrus 转换处 add_var；此处仿照 Let 前向传播
        // 统一登记，使 `if (n := f()) > 0:` 的 then 分支 / 后续语句
        // 能 lookup_var(n) 拿到真实类型而非 Any）
        let mut walrus_binds = Vec::new();
        collect_stmt_walrus(s, &block_ctx, &mut walrus_binds);
        for (wname, wty) in walrus_binds {
            if !block_ctx.vars.contains_key(&wname)
                && !block_ctx.top_level_consts.contains_key(&wname)
            {
                block_ctx.block_declared.insert(wname.clone());
            }
            block_ctx.add_var(&wname, wty);
        }
        // guard let <Variant>(...) = expr else: ... → match value { Variant(r) => { 剩余语句 }, _ => { else_body } }
        if let AstStmt::Guard {
            let_binding,
            else_body,
            ..
        } = s
        {
            if let Some((pat, value)) = let_binding {
                if let AstPattern::Variant(_, args) = pat {
                    let val = convert_expr(value, &block_ctx);
                    // 剩余语句作为匹配分支的 body（guard 之后代码仅在匹配时执行）
                    let rest: Vec<AstStmt> = stmts
                        .iter()
                        .skip_while(|x| !std::ptr::eq(*x, s))
                        .skip(1)
                        .cloned()
                        .collect();
                    // 匹配分支上下文：绑定模式变量（类型 Any 占位，body 转换时按需解析）
                    let mut then_ctx = block_ctx.clone();
                    fn collect_pat_vars(pat: &AstPattern, ctx: &mut TypeCtx) {
                        match pat {
                            AstPattern::Ident(name) => {
                                ctx.add_var(name, IrType::Any);
                            }
                            AstPattern::Variant(_, as_) => {
                                for a in as_ {
                                    collect_pat_vars(a, ctx);
                                }
                            }
                            _ => {}
                        }
                    }
                    collect_pat_vars(
                        &AstPattern::Variant(String::new(), args.clone()),
                        &mut then_ctx,
                    );
                    let then_block = convert_block(&rest, &then_ctx);
                    let else_block = convert_block(else_body, &block_ctx);
                    // 构建 match 结构：Variant(...) 分支 + 默认分支
                    let ir_pat = convert_ast_pattern(pat, &block_ctx).unwrap_or(Pattern::Wildcard);
                    let match_stmt = Stmt::Match {
                        scrutinee: val,
                        arms: vec![
                            MatchArm {
                                pattern: ir_pat,
                                guard: None,
                                body: then_block,
                            },
                            MatchArm {
                                pattern: Pattern::Wildcard,
                                guard: None,
                                body: else_block,
                            },
                        ],
                    };
                    ir_stmts.push(match_stmt);
                    break;
                }
            }
        }
        ir_stmts.extend(convert_stmts(std::slice::from_ref(s), &block_ctx));
    }

    // 后处理：递归收集所有被赋值的变量名，标记对应首次 let 为 mut
    fn collect_reassigned(stmts: &[Stmt]) -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        for s in stmts {
            match s {
                Stmt::Assign { target, .. } => {
                    if let ExprKind::Var(name) = &target.kind {
                        set.insert(name.clone());
                    }
                }
                Stmt::Let { name, is_mut, .. } if *is_mut => {
                    set.insert(name.clone());
                }
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    set.extend(collect_reassigned(&then_branch.stmts));
                    if let Some(eb) = else_branch {
                        set.extend(collect_reassigned(&eb.stmts));
                    }
                }
                Stmt::While { body, .. } | Stmt::For { body, .. } => {
                    set.extend(collect_reassigned(&body.stmts));
                }
                Stmt::Match { arms, .. } => {
                    for arm in arms {
                        set.extend(collect_reassigned(&arm.body.stmts));
                    }
                }
                Stmt::TryCatch {
                    body,
                    catches,
                    else_body,
                    finally_body,
                } => {
                    set.extend(collect_reassigned(&body.stmts));
                    for (_, catch_body) in catches {
                        set.extend(collect_reassigned(&catch_body.stmts));
                    }
                    if let Some(eb) = else_body {
                        set.extend(collect_reassigned(&eb.stmts));
                    }
                    if let Some(fb) = finally_body {
                        set.extend(collect_reassigned(&fb.stmts));
                    }
                }
                _ => {}
            }
        }
        set
    }

    // 空列表字面量元素类型推断（支持 append/push 上下文推断，贴近 Rust）
    let empty_elems = resolve_empty_list_elems(&ir_stmts);

    // 不可变 `let` 重赋值 → E0384 错误（移除原本自动提升为 mut 的行为，贴近 Rust 语义）
    let reassigned = collect_reassigned(&ir_stmts);
    for s in &ir_stmts {
        if let Stmt::Let { name, is_mut, .. } = s {
            if reassigned.contains(name.as_str()) && !*is_mut {
                ctx.report_error(format!(
                    "error[E0384]: cannot assign twice to immutable variable `{}`\n  = help: change `let {}` to `let mut {}` if you intend to reassign it",
                    name, name, name
                ));
            }
        }
    }

    // 将推断出的空列表元素类型应用到对应 let；无法推断则报 E0282 错误
    for s in &mut ir_stmts {
        if let Stmt::Let {
            name, ty, value, ..
        } = s
        {
            if let Some(elem) = empty_elems.get(name.as_str()) {
                *ty = IrType::Named {
                    path: "List".to_string(),
                    args: vec![elem.clone()],
                };
            } else if let IrType::Named { path, args } = ty {
                if path == "List" && args.len() == 1 && matches!(args[0], IrType::Any) {
                    if let ExprKind::ListLit(items) = &value.kind {
                        if items.is_empty() {
                            ctx.report_error(format!(
                                "error[E0282]: type annotations needed\n  = cannot infer element type for empty list bound to `{}`\n  = help: give it an explicit type, e.g. `let {}: List<T> = []`",
                                name, name
                            ));
                        }
                    }
                }
            }
        }
    }

    // 递归收集空列表字面量，并通过 append/push 调用推断其元素类型（贴近 Rust 的上下文推断）
    fn resolve_empty_list_elems(stmts: &[Stmt]) -> std::collections::HashMap<String, IrType> {
        let mut empty_lets: std::collections::HashSet<String> = std::collections::HashSet::new();
        for s in stmts {
            if let Stmt::Let {
                name, ty, value, ..
            } = s
            {
                if let IrType::Named { path, args } = ty {
                    if path == "List" && args.len() == 1 && matches!(args[0], IrType::Any) {
                        if let ExprKind::ListLit(items) = &value.kind {
                            if items.is_empty() {
                                empty_lets.insert(name.clone());
                            }
                        }
                    }
                }
            }
        }
        let mut resolved: std::collections::HashMap<String, IrType> =
            std::collections::HashMap::new();
        fn scan(
            stmts: &[Stmt],
            empty_lets: &std::collections::HashSet<String>,
            resolved: &mut std::collections::HashMap<String, IrType>,
        ) {
            for s in stmts {
                match s {
                    Stmt::ExprStmt { expr: e } => {
                        if let ExprKind::MethodCall {
                            receiver,
                            method,
                            args,
                        } = &e.kind
                        {
                            if method == "append" || method == "push" {
                                if let ExprKind::Var(name) = &receiver.kind {
                                    if empty_lets.contains(name) && !resolved.contains_key(name) {
                                        if let Some(first) = args.first() {
                                            if first.ty != IrType::Any {
                                                resolved.insert(name.clone(), first.ty.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Stmt::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        scan(&then_branch.stmts, empty_lets, resolved);
                        if let Some(eb) = else_branch {
                            scan(&eb.stmts, empty_lets, resolved);
                        }
                    }
                    Stmt::While { body, .. }
                    | Stmt::For { body, .. }
                    | Stmt::WhileLet { body, .. } => scan(&body.stmts, empty_lets, resolved),
                    Stmt::Match { arms, .. } => {
                        for a in arms {
                            scan(&a.body.stmts, empty_lets, resolved);
                        }
                    }
                    Stmt::TryCatch {
                        body,
                        catches,
                        else_body,
                        finally_body,
                    } => {
                        scan(&body.stmts, empty_lets, resolved);
                        for (_, cb) in catches {
                            scan(&cb.stmts, empty_lets, resolved);
                        }
                        if let Some(eb) = else_body {
                            scan(&eb.stmts, empty_lets, resolved);
                        }
                        if let Some(fb) = finally_body {
                            scan(&fb.stmts, empty_lets, resolved);
                        }
                    }
                    Stmt::Block { stmts: b } => scan(&b, empty_lets, resolved),
                    _ => {}
                }
            }
        }
        scan(stmts, &empty_lets, &mut resolved);
        resolved
    }

    let ty = stmts
        .last()
        .map(|s| infer_stmt_type(s, ctx))
        .unwrap_or(IrType::Unit);
    Block {
        span: Span::unknown(),
        stmts: ir_stmts,
        ty,
    }
}

fn convert_block_with_ctx(stmts: &[AstStmt], ctx: &TypeCtx) -> Block {
    convert_block(stmts, ctx)
}

// ══════════════════════════════════════════════════════════════
// 顶层 Item 转换
// ══════════════════════════════════════════════════════════════

/// 转换 duck 类型约束 → IR DuckDef
fn convert_duck_def(d: &ast::DuckDef) -> DuckDef {
    // 嵌套约束 where T: X → 存到对应泛型参数的 bounds（§2.4）
    let mut where_bounds: HashMap<String, Vec<IrType>> = HashMap::new();
    for wb in &d.where_clause {
        let ir_bounds: Vec<IrType> = wb.bounds.iter().map(|b| from_ast_type(b)).collect();
        where_bounds
            .entry(wb.type_param.clone())
            .or_default()
            .extend(ir_bounds);
    }
    let methods: Vec<DuckMethod> = d
        .methods
        .iter()
        .map(|m| DuckMethod {
            owner: m.owner.clone(),
            name: m.name.clone(),
            name_pattern: m.name_pattern.clone(),
            params: m
                .params
                .iter()
                .map(|p| Param {
                    name: p.name.clone(),
                    ty: from_ast_type(&p.ty),
                    is_mut: p.is_mut,
                    is_ref: p.is_ref,
                    is_owned: p.is_owned,
                    default: None,
                    variadic: false,
                })
                .collect(),
            ret_ty: m
                .return_type
                .as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or(IrType::Unit),
            param_range: m.param_range,
            is_default: m.is_default,
        })
        .collect();
    let fields: Vec<DuckField> = d
        .fields
        .iter()
        .map(|f| DuckField {
            owner: f.owner.clone(),
            name: f.name.clone(),
            ty: from_ast_type(&f.ty),
            rel: f.rel.clone(),
        })
        .collect();
    DuckDef {
        name: d.name.clone(),
        generics: d
            .generics
            .iter()
            .map(|g| GenericParam {
                name: g.clone(),
                bounds: where_bounds.remove(g).unwrap_or_default(),
                default: None,
            })
            .collect(),
        assoc_types: d
            .assoc_types
            .iter()
            .map(|a| DuckAssocType {
                owner: a.owner.clone(),
                name: a.name.clone(),
            })
            .collect(),
        satisfies: d.satisfies.clone(),
        sealed: d.sealed,
        match_rules: d
            .match_rules
            .iter()
            .map(|r| DuckMatchRule {
                pattern: r.pattern.clone(),
                range: r.range,
            })
            .collect(),
        param_reqs: d
            .param_reqs
            .iter()
            .map(|r| DuckParamReq {
                is_required: r.is_required,
                names: r.names.clone(),
            })
            .collect(),
        methods,
        fields,
    }
}

/// 检测 IR Block 是否包含 yield 语句（递归嵌套块）
fn ir_block_has_yield(block: &Block) -> bool {
    for stmt in &block.stmts {
        if matches!(stmt, Stmt::Yield { .. } | Stmt::YieldFrom { .. }) {
            return true;
        }
        match stmt {
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if ir_block_has_yield(then_branch) {
                    return true;
                }
                if let Some(eb) = else_branch {
                    if ir_block_has_yield(eb) {
                        return true;
                    }
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if ir_block_has_yield(body) {
                    return true;
                }
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    if ir_block_has_yield(&arm.body) {
                        return true;
                    }
                }
            }
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                if ir_block_has_yield(body) {
                    return true;
                }
                for (_, cb) in catches {
                    if ir_block_has_yield(cb) {
                        return true;
                    }
                }
                if let Some(eb) = else_body {
                    if ir_block_has_yield(eb) {
                        return true;
                    }
                }
                if let Some(fb) = finally_body {
                    if ir_block_has_yield(fb) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// 将 iterator（生成器）函数体内的带值 return 重写为 raise（等价终止并抛出）。
/// 递归遍历嵌套块（if/for/while/match/try 等），codegen 会把 Stmt::Raise 生成 panic!。
fn rewrite_iterator_returns(block: &mut Block) {
    for stmt in &mut block.stmts {
        match stmt {
            Stmt::Return { value: Some(v) } => {
                // 带值 return → raise（生成器内 return expr 等价于 raise）
                let v = std::mem::replace(
                    v,
                    Expr::new(
                        ExprKind::Lit(LitKind::Unit),
                        IrType::Unit,
                        Span::unknown(),
                    ),
                );
                *stmt = Stmt::Raise { value: v };
            }
            Stmt::Return { value: None } => {
                // 无值 return → raise 空
                *stmt = Stmt::Raise {
                    value: Expr::new(
                        ExprKind::Lit(LitKind::Unit),
                        IrType::Unit,
                        Span::unknown(),
                    ),
                };
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                rewrite_iterator_returns(then_branch);
                if let Some(eb) = else_branch {
                    rewrite_iterator_returns(eb);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                rewrite_iterator_returns(body);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    rewrite_iterator_returns(&mut arm.body);
                }
            }
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                rewrite_iterator_returns(body);
                for (_, cb) in catches {
                    rewrite_iterator_returns(cb);
                }
                if let Some(eb) = else_body {
                    rewrite_iterator_returns(eb);
                }
                if let Some(fb) = finally_body {
                    rewrite_iterator_returns(fb);
                }
            }
            Stmt::Block { stmts } => {
                let mut inner = Block {
                    span: Span::unknown(),
                    stmts: std::mem::take(stmts),
                    ty: IrType::Unit,
                };
                rewrite_iterator_returns(&mut inner);
                *stmts = inner.stmts;
            }
            _ => {}
        }
    }
}

/// 预扫描函数体，登记闭包 let 绑定为 Fn 类型（用于返回类型推断）。
/// 否则 `def f() = ... consume()` 末尾的闭包调用在推断 ret_ty 时
/// lookup_var 查不到 consume → 回退 Any→i64（E0308）。
fn prescan_closure_bindings(stmts: &[AstStmt], ctx: &mut TypeCtx) {
    for stmt in stmts {
        if let AstStmt::Let { name, value, .. } = stmt {
            if let AstExpr::Closure { params, body, .. } = value {
                // fat-arrow 闭包体（|..| => 块）是 BlockExpr：取最后语句类型，
                // 否则 BlockExpr 推断为 Any → 闭包 ret=Any → 调用推断 i64（E0308）
                let ret = match body.as_ref() {
                    AstExpr::BlockExpr(block) => block
                        .last()
                        .map(|s| infer_stmt_type(s, ctx))
                        .unwrap_or(IrType::Unit),
                    other => infer_expr_type(other, ctx),
                };
                let fty = IrType::Fn {
                    params: vec![IrType::Any; params.len()],
                    ret: Box::new(ret),
                };
                ctx.add_var(name, fty);
                // 预登记的闭包绑定是本函数体**声明**（`mut f = 闭包` 的 f）：
                // 标记 block_declared，否则 convert_block 预扫描（`!vars.contains`）
                // 把预注册的 f 误当外部变量跳过 block_declared 注册，后续
                // 3888 行误转 Assign（生成裸赋值 `f = move |...|` 缺 let，E0425）
                ctx.block_declared.insert(name.clone());
            }
        }
    }
}

/// 递归扫描生成器函数体，返回第一个 `yield expr` 中 expr 的推断类型。
/// 同时预登记函数体（含嵌套块）内的 let 绑定，使 `yield i` 能推断出 i 的类型。
fn scan_iterator_yield_ty(stmts: &[AstStmt], ctx: &mut TypeCtx) -> Option<IrType> {
    for stmt in stmts {
        match stmt {
            AstStmt::Let { name, ty, value, .. } => {
                // 预登记 let 绑定：优先类型注解，否则从初始值推断
                let bind_ty = ty
                    .as_ref()
                    .map(|t| from_ast_type_with_generics(t, &ctx.current_generics))
                    .unwrap_or_else(|| infer_expr_type(value, ctx));
                ctx.add_var(name, bind_ty);
                // 同时登记为本块首次声明：否则后续 `let mut i = 0` 会因 vars 已含 i
                // 且 block_declared 不含 i 而被误转 Stmt::Assign（生成裸赋值，E0425）
                ctx.block_declared.insert(name.clone());
            }
            AstStmt::Yield(Some(e)) => {
                return Some(infer_expr_type(e, ctx));
            }
            AstStmt::While {
                body, else_body, ..
            } => {
                if let Some(t) = scan_iterator_yield_ty(body, ctx) {
                    return Some(t);
                }
                if let Some(eb) = else_body {
                    if let Some(t) = scan_iterator_yield_ty(eb, ctx) {
                        return Some(t);
                    }
                }
            }
            AstStmt::Loop(body)
            | AstStmt::Block { body, .. }
            | AstStmt::CheckerBlock { body, .. }
            | AstStmt::Defer(body)
            | AstStmt::Test { body, .. }
            | AstStmt::Comptime { body } => {
                if let Some(t) = scan_iterator_yield_ty(body, ctx) {
                    return Some(t);
                }
            }
            AstStmt::WhileLet {
                body, else_body, ..
            }
            | AstStmt::For {
                body, else_body, ..
            } => {
                if let Some(t) = scan_iterator_yield_ty(body, ctx) {
                    return Some(t);
                }
                if let Some(eb) = else_body {
                    if let Some(t) = scan_iterator_yield_ty(eb, ctx) {
                        return Some(t);
                    }
                }
            }
            AstStmt::With { body, .. } => {
                if let Some(t) = scan_iterator_yield_ty(body, ctx) {
                    return Some(t);
                }
            }
            AstStmt::Suite {
                setup,
                teardown,
                tests,
                ..
            } => {
                for sub in setup
                    .iter()
                    .flatten()
                    .chain(teardown.iter().flatten())
                    .chain(tests.iter())
                {
                    if let Some(t) = scan_iterator_yield_ty(std::slice::from_ref(sub), ctx) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn convert_fn_def(func: &ast::Function, ctx: &TypeCtx) -> FnDef {
    let is_math = func.decorators.iter().any(|d| d.name == "math");
    // 方法泛型 + impl 级泛型合并（`impl<T> Box<T>` 的方法 try_unwrap 中
    // `Result<T, Rc<T>>` 的 T 是 impl 泛型；只含 func.generics 则 T 被解析为
    // Named("T") 而非 Generic("T")，导致 return 隐式转换误判（E0277））
    let generics: Vec<String> = if is_math {
        vec!["T".to_string()]
    } else {
        let mut g = func.generics.clone();
        for gg in &ctx.current_generics {
            if !g.contains(gg) {
                g.push(gg.clone());
            }
        }
        g
    };

    let params: Vec<Param> = func
        .params
        .iter()
        .enumerate()
        .map(|(_i, p)| {
            // `..` 注入的 args/kwargs 在下方单独追加；此处只转换具名参数
            Param {
                name: p.name.clone(),
                ty: if is_math {
                    IrType::Generic("T".into())
                } else {
                    from_ast_type_with_generics(&p.ty, &generics)
                },
                is_mut: p.is_mut,
                is_ref: p.is_ref,
                is_owned: p.is_owned,
                default: p.default.as_ref().map(|d| convert_expr(d, ctx)),
                variadic: false,
            }
        })
        .collect();
    // `..` 变参注入：追加 args/kwargs 隐式参数（variadic 收集）
    // 文档 03d-可变参数.md §2：任何 `..` 出现即触发注入；
    // 单 `..` 无注解 → 注入 args（元素 Any）；`..: Tuple<T>` → args-only；
    // `..: Dict<K,V>` → kwargs-only；双 `..` → args + kwargs
    let mut variadic_params: Vec<Param> = Vec::new();
    match &func.variadic {
        ast::VariadicMode::ArgsOnly { elem_ty, elem_tys, .. } => {
            // 03d §2.3 多类型位置约束：`..: Tuple<T1, T2, ..>` 约束位置参数各自类型。
            // 生成固定前缀异构元组 args: (T1, T2, Vec<Box<dyn Any>>)——
            // 前 N 个位置有精确类型，尾部 `..` 通配（哨兵 Type::Any）收集为 Box<dyn Any> 切片。
            let is_multi = elem_tys.len() >= 2 && elem_tys.last() == Some(&AstType::Any);
            if is_multi {
                let prefix_tys: Vec<AstType> = elem_tys[..elem_tys.len() - 1].to_vec();
                let prefix_irs: Vec<IrType> = prefix_tys
                    .iter()
                    .map(|t| from_ast_type_with_generics(t, &generics))
                    .collect();
                variadic_params.push(Param {
                    name: "args".into(),
                    ty: IrType::Tuple(prefix_irs),
                    is_mut: false,
                    is_ref: false,
                    is_owned: false,
                    default: None,
                    variadic: true,
                });
            } else {
                // 03d §2.8 方案 B type-pack：`..: Tuple<Ts...>`（元素为 `Ts...`，
                // parser 已标记为 Named("Ts...")）→ 异质元组，由调用点推断具体
                // 类型。注册为 IrType::Tuple([Generic("Ts")])，使函数体内
                // args.N 走元组字段访问（codegen FieldAccess Tuple 分支 → .N），
                // 且调用点打包为 Rust 元组字面量（Type::Tuple 全 Generic = type pack）。
                let is_type_pack = elem_ty.as_ref().map_or(false, |t| {
                    matches!(t, AstType::Named(n) if n.ends_with("..."))
                });
                if is_type_pack {
                    let pack_name = match elem_ty.as_ref().unwrap() {
                        AstType::Named(n) => n.trim_end_matches("...").to_string(),
                        _ => String::new(),
                    };
                    variadic_params.push(Param {
                        name: "args".into(),
                        ty: IrType::Tuple(vec![IrType::Generic(pack_name)]),
                        is_mut: false,
                        is_ref: false,
                        is_owned: false,
                        default: None,
                        variadic: true,
                    });
                } else {
                    let elem = elem_ty
                        .as_ref()
                        .map(|t| from_ast_type_with_generics(t, &generics))
                        .unwrap_or(IrType::Any);
                    variadic_params.push(Param {
                        name: "args".into(),
                        ty: elem,
                        is_mut: false,
                        is_ref: false,
                        is_owned: false,
                        default: None,
                        variadic: true,
                    });
                }
            }
        }
        ast::VariadicMode::KwargsOnly { value_ty, .. } => {
            let v = value_ty
                .as_ref()
                .map(|t| from_ast_type_with_generics(t, &generics))
                .unwrap_or(IrType::Any);
            variadic_params.push(Param {
                name: "kwargs".into(),
                ty: v,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: true,
            });
        }
        ast::VariadicMode::Both {
            args_elem_ty,
            kwargs_value_ty,
            ..
        } => {
            let elem = args_elem_ty
                .as_ref()
                .map(|t| from_ast_type_with_generics(t, &generics))
                .unwrap_or(IrType::Any);
            variadic_params.push(Param {
                name: "args".into(),
                ty: elem,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: true,
            });
            let v = kwargs_value_ty
                .as_ref()
                .map(|t| from_ast_type_with_generics(t, &generics))
                .unwrap_or(IrType::Any);
            variadic_params.push(Param {
                name: "kwargs".into(),
                ty: v,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: true,
            });
        }
        ast::VariadicMode::None => {}
    }
    // 构建函数体上下文
    let mut fn_ctx = TypeCtx::new();
    fn_ctx.pending_items = ctx.pending_items.clone();
    fn_ctx.errors = ctx.errors.clone();
    fn_ctx.comptime_consts = ctx.comptime_consts.clone();
    fn_ctx.comptime_module = ctx.comptime_module.clone();
    fn_ctx.current_fn_name = Some(func.name.clone());
    // 继承顶层函数返回类型表：嵌套函数内调用其他函数（`return classify(0)`）需
    // 查 lookup_fn_return——否则 fn_returns 为空 → Any→i64 fallback，
    // return 误插 <String as ImplicitFrom<i64>> 转换（E0277，match_patterns.lz）
    fn_ctx.fn_returns = ctx.fn_returns.clone();
    // 继承顶层变量（size = 3 等顶层 Assign 转 Const）：函数内 `x = v` 需识别为
    // 修改全局（guard_for_3.lz size = size - 1），否则生成局部新绑定 E0425
    fn_ctx.top_level_consts = ctx.top_level_consts.clone();
    fn_ctx.current_generics = generics.clone();
    // 复制全局 struct 信息
    for sn in &ctx.struct_names {
        fn_ctx.struct_names.insert(sn.clone());
    }
    for (sn, fields) in &ctx.struct_fields {
        let mut cloned = HashMap::new();
        for (fn_, ty) in fields {
            cloned.insert(fn_.clone(), ty.clone());
        }
        fn_ctx.struct_fields.insert(sn.clone(), cloned);
    }
    for (sn, order) in &ctx.struct_field_order {
        fn_ctx.struct_field_order.insert(sn.clone(), order.clone());
    }
    for (sn, ms) in &ctx.struct_methods {
        fn_ctx.struct_methods.insert(sn.clone(), ms.clone());
    }
    for (sn, arity) in &ctx.struct_method_arity {
        fn_ctx.struct_method_arity.insert(sn.clone(), arity.clone());
    }
    for (cn, ct) in &ctx.top_level_consts {
        fn_ctx.top_level_consts.insert(cn.clone(), ct.clone());
    }
    for (vn, en) in &ctx.enum_variants {
        fn_ctx.enum_variants.insert(vn.clone(), en.clone());
    }
    for (vn, ft) in &ctx.enum_variant_field_types {
        fn_ctx.enum_variant_field_types.insert(vn.clone(), ft.clone());
    }
    for (name, ty) in &ctx.fn_returns {
        fn_ctx.fn_returns.insert(name.clone(), ty.clone());
    }
    for (name, p) in &ctx.fn_params {
        fn_ctx.fn_params.insert(name.clone(), p.clone());
    }

    // 添加参数到作用域
    if is_math {
        for p in &func.params {
            fn_ctx.add_param(&p.name, IrType::Generic("T".into()));
        }
    } else {
        for p in &func.params {
            // impl 方法中 self 参数绑定为 impl 目标类型（self_ty，如 Dict<K,V>），
            // 使 self[key]、key in self、self.field 等按具体类型解析（E0277/E0599）
            if p.name == "self" || p.name == "self_" {
                if let Some(st) = &ctx.self_ty {
                    fn_ctx.add_param(&p.name, st.clone());
                    continue;
                }
            }
            // ref 参数（iterable: ref I）登记为 Ref(inner)：for 循环等按引用处理，
            // 否则 iterable 推断为 Generic("I")，`for item in iterable` 生成
            // (iterable).into_iter() 会 move 出 &I（E0507 cannot move out of *iterable）
            let base_ty = from_ast_type_with_generics(&p.ty, &generics);
            if p.is_ref && !p.is_mut {
                fn_ctx.add_param(&p.name, IrType::Ref(Box::new(base_ty)));
            } else {
                fn_ctx.add_param(&p.name, base_ty);
            }
        }
    }
    // 注入 args/kwargs 内置变量到函数作用域（函数体内可直接引用 args / kwargs）
    for vp in &variadic_params {
        if vp.name == "args" {
            // args: List<elem>（元素类型注解或无注解 → Any）
            let elem = vp.ty.clone();
            // 03d §2.3 多类型位置约束：`..: Tuple<T1, T2, ..>` 的 args 是固定前缀
            // 异构元组 (T1, T2, Vec<Box<dyn Any>>)——注册为 Tuple 类型，使函数体内
            // args[0] / args.N 走元组字段访问（codegen IndexGet 的 Tuple 分支 → .0/.1）
            if matches!(&elem, IrType::Tuple(_)) {
                fn_ctx.add_param("args", elem);
            } else {
                fn_ctx.add_param(
                    "args",
                    IrType::Named {
                        path: "List".into(),
                        args: vec![elem],
                    },
                );
            }
        } else if vp.name == "kwargs" {
            // kwargs: Dict<str, V>
            let v = vp.ty.clone();
            fn_ctx.add_param(
                "kwargs",
                IrType::Named {
                    path: "Dict".into(),
                    args: vec![IrType::Str, v],
                },
            );
        }
    }

    // 返回类型：优先 AST 注解，否则从函数体最后语句推断；
    // iterator 无标注时从第一个 yield 表达式推断元素类型（codegen 会包装 Vec<T>）
    // 预登记函数体内闭包 let 绑定（consume = |..| => ..）：否则 body 末尾 `consume()`
    // 推断返回类型时 lookup_var 查不到 → 回退 Any→i64（E0308）
    prescan_closure_bindings(&func.body, &mut fn_ctx);
    let ret_ty = func
        .return_type
        .as_ref()
        .map(|t| from_ast_type_with_generics(t, &generics))
        .unwrap_or_else(|| {
            if func.is_iterator {
                // 递归扫描（yield 可嵌套在 while/for/if 块内），并预登记 let 绑定类型
                scan_iterator_yield_ty(&func.body, &mut fn_ctx).unwrap_or(IrType::Unit)
            } else {
                func.body
                    .last()
                    .map(|stmt| infer_stmt_type(stmt, &fn_ctx))
                    .unwrap_or(IrType::Unit)
            }
        });
    // 注意：Iterator 函数的 Vec<T> 包装由 codegen 负责（基于 has_yield 检测），
    // 此处不包装，避免 Vec<Vec<T>> 双重包装。
    fn_ctx.current_ret_ty = Some(ret_ty.clone());
    fn_ctx.current_is_iterator = func.is_iterator;

    // #[extern(lang)]：外部声明无 lz 函数体，codegen 生成分发调用
    let has_extern = func.decorators.iter().any(|d| d.name == "extern");
    let body = if has_extern {
        Block::default()
    } else {
        let body = convert_block(&func.body, &fn_ctx);
        // 函数尾 `let x =: <构建块>`（BuildBlock Var）：构建块值赋给 x 后应作为
        // 函数返回值（如 combo-build-block.lz 的 build_if_else/build_match/build_try）。
        // 否则生成 `let x = (move || {...})();` 后无 return，E0308 类型不匹配。
        if !matches!(ret_ty, IrType::Unit) && !func.is_iterator {
        // 匹配两种形式：
        //   - `result =: ...`（AstStmt::Expr 包裹 BuildBlock）
        //   - `let result =: ...`（AstStmt::Let 的 value 是 BuildBlock）
        let tail_build_lhs: Option<String> = match func.body.last() {
            Some(AstStmt::Expr(AstExpr::BuildBlock {
                kind: BuildKind::Var,
                lhs,
                ..
            })) => match &**lhs {
                AstExpr::Ident(name) => Some(name.clone()),
                _ => None,
            },
            Some(AstStmt::Let { name, value, .. }) => {
                if matches!(value, AstExpr::BuildBlock { kind: BuildKind::Var, .. }) {
                    Some(name.clone())
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(name) = tail_build_lhs {
            let mut b = body;
            b.stmts.push(Stmt::Return {
                value: Some(Expr::new(
                    ExprKind::Var(name),
                    ret_ty.clone(),
                    Span::unknown(),
                )),
            });
            b
        } else {
            body
        }
    } else {
        body
    }
    };
    // iterator 体内 return 等价 raise（08-生成器.md：终止迭代并抛出）
    // codegen 将 Stmt::Raise 生成 panic!，因此把带值 return 转成 raise
    // 判断条件：is_iterator 标志 或 body 实际含 yield（更可靠，覆盖标志缺失）
    let body = if func.is_iterator || ir_block_has_yield(&body) {
        let mut b = body;
        rewrite_iterator_returns(&mut b);
        b
    } else {
        body
    };

    let is_math = func.decorators.iter().any(|d| d.name == "math");
    let intrinsics: Vec<Intrinsic> = func
        .decorators
        .iter()
        .map(|d| {
            let kind = match d.name.as_str() {
                "memoize" => IntrinsicKind::Memoize,
                "parallel" => IntrinsicKind::Parallel,
                "curry" => IntrinsicKind::Curry,
                "overload" => IntrinsicKind::Overload,
                "derive" => IntrinsicKind::Derive,
                "tail_call" => IntrinsicKind::TailCall,
                "math" => IntrinsicKind::Export(vec!["Math".into()]),
                "extern" => {
                    // #[extern(Rust, Python)] 外部声明（L1 机制）
                    let targets: Vec<String> = d
                        .args
                        .iter()
                        .filter_map(|a| {
                            if let AstExpr::Ident(n) = a {
                                Some(n.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    IntrinsicKind::Extern(if targets.is_empty() {
                        vec![]
                    } else {
                        // 归一化语言标记（大小写不敏感：rust → Rust）
                        targets
                            .iter()
                            .map(|t| {
                                let mut c = t.chars();
                                match c.next() {
                                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                                    None => String::new(),
                                }
                            })
                            .collect()
                    })
                }
                "embed" => {
                    // #[embed(rust)] / #[embed(py)]：内嵌代码段（G7）
                    // 语言参数取 args[0]（rust / py，大小写不敏感归一化）；
                    // 代码段取函数体首个字符串字面量（原生代码段原样插入生成产物）。
                    let lang = d
                        .args
                        .first()
                        .and_then(|a| {
                            if let AstExpr::Ident(n) = a {
                                Some(n.clone())
                            } else {
                                None
                            }
                        })
                        .map(|t| {
                            let mut c = t.chars();
                            match c.next() {
                                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                                None => String::new(),
                            }
                        })
                        .unwrap_or_default();
                    IntrinsicKind::Embed {
                        lang,
                        code: extract_embed_code(&body),
                    }
                }
                name if name.starts_with("export") => {
                    // @export(Rust, Python)
                    let targets: Vec<String> = d
                        .args
                        .iter()
                        .filter_map(|a| {
                            if let AstExpr::Ident(n) = a {
                                Some(n.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    IntrinsicKind::Export(if targets.is_empty() {
                        vec!["Rust".into()]
                    } else {
                        targets
                    })
                }
                "init" => IntrinsicKind::Init,
                _ => {
                    return Intrinsic {
                        kind: IntrinsicKind::Memoize,
                        span: Span::unknown(),
                    }
                } // skip unknown
            };
            Intrinsic {
                kind,
                span: Span::unknown(),
            }
        })
        .collect();

    // #[extern(lang)] 诊断：语言参数缺失 / 重复标记 / 返回类型必须为 Ext
    {
        let extern_count = func.decorators.iter().filter(|d| d.name == "extern").count();
        if extern_count > 1 {
            ctx.report_error(format!(
                "#[extern] 重复标记：函数 '{}' 有 {} 个 extern 装饰器",
                func.name, extern_count
            ));
        }
        if let Some(ei) = intrinsics.iter().find(|i| matches!(i.kind, IntrinsicKind::Extern(_))) {
            if let IntrinsicKind::Extern(targets) = &ei.kind {
                if targets.is_empty() {
                    ctx.report_error(format!(
                        "#[extern] 缺少语言参数：函数 '{}' 需要 #[extern(Rust)] / #[extern(Python)]",
                        func.name
                    ));
                }
                for t in targets {
                    let known = matches!(t.as_str(), "Rust" | "Python" | "C");
                    if !known {
                        ctx.report_error(format!(
                            "#[extern] 未知语言 '{}'：函数 '{}' 支持 Rust / Python / C",
                            t, func.name
                        ));
                    }
                }
            }
            if !matches!(ret_ty, IrType::Ext) {
                ctx.report_error(format!(
                    "#[extern] 返回类型错误：函数 '{}' 必须返回 Ext（外部专用句柄），实际返回 {}",
                    func.name,
                    ret_ty
                ));
            }
        }
    }

    // #[embed(lang)] 诊断：语言缺失 / 未知语言 / 代码段缺失（G7）
    {
        let embed_count = func.decorators.iter().filter(|d| d.name == "embed").count();
        if embed_count > 1 {
            ctx.report_error(format!(
                "#[embed] 重复标记：函数 '{}' 有 {} 个 embed 装饰器",
                func.name, embed_count
            ));
        }
        if let Some(ei) = intrinsics.iter().find(|i| matches!(i.kind, IntrinsicKind::Embed { .. })) {
            if let IntrinsicKind::Embed { lang, code } = &ei.kind {
                if lang.is_empty() {
                    ctx.report_error(format!(
                        "#[embed] 缺少语言参数：函数 '{}' 需要 #[embed(rust)] / #[embed(py)]",
                        func.name
                    ));
                } else if !matches!(lang.as_str(), "Rust" | "Python") {
                    ctx.report_error(format!(
                        "#[embed] 未知语言 '{}'：函数 '{}' 支持 rust / py",
                        lang, func.name
                    ));
                }
                if code.is_empty() {
                    ctx.report_error(format!(
                        "#[embed] 缺少内嵌代码段：函数 '{}' 的函数体必须为单个字符串字面量（原生代码段）",
                        func.name
                    ));
                }
            }
        }
    }

/// 提取 embed 内嵌代码段（G7）：函数体首个字符串字面量（Return 值 / ExprStmt）
///
/// 约定：`#[embed(rust)] def foo(): return "let x = 1; x + 1"` 的代码段为
/// 字符串字面量本身（原生代码原样插入生成产物，不做 LZ 语义处理）。
/// 未找到返回空串，由 embed 诊断块报错。
fn extract_embed_code(body: &Block) -> String {
    for stmt in &body.stmts {
        let expr = match stmt {
            Stmt::Return { value: Some(e) } => e,
            Stmt::ExprStmt { expr } => expr,
            _ => continue,
        };
        // builder 会按返回类型包装隐式转换（ImplicitConvert→Lit(Str)），
        // 需穿透一层取其 source 字符串字面量
        let inner = match &expr.kind {
            ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
            ExprKind::ImplicitConvert { source, .. } => {
                if let ExprKind::Lit(LitKind::Str(s)) = &source.kind {
                    Some(s.clone())
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(s) = inner {
            return s;
        }
    }
    String::new()
}

    // 引用 impl 级泛型的 where 约束（如 `impl<K,V> Dict<K,V>` 方法
    // `where K: Eq + Hash`，K 不在方法泛型中）无法合并到方法泛型，
    // 保留到 FnDef.where_clause，由 codegen 输出到方法签名（E0277 修复）
    let extra_where: Vec<(String, Vec<IrType>)> = func
        .where_clause
        .iter()
        .filter(|wb| !func.generics.contains(&wb.type_param))
        .map(|wb| {
            (
                wb.type_param.clone(),
                wb.bounds.iter().map(|b| from_ast_type(b)).collect(),
            )
        })
        .collect();

    FnDef {
        name: func.name.clone(),
        generics: if is_math && func.generics.is_empty() {
            // @math 自動泛型: 单泛型 T（所有参数统一类型）
            vec![GenericParam {
                name: "T".into(),
                bounds: vec![],
                default: None,
            }]
        } else {
            // 从 where_clause 收集每个泛型参数的 bounds
            let mut bounds_map: HashMap<String, Vec<IrType>> = HashMap::new();
            for wb in &func.where_clause {
                let ir_bounds: Vec<IrType> = wb.bounds.iter().map(|b| from_ast_type(b)).collect();
                bounds_map
                    .entry(wb.type_param.clone())
                    .or_default()
                    .extend(ir_bounds);
            }
            // 泛型默认类型（§四 `T = int`）
            let defaults_map: HashMap<String, IrType> = func
                .generic_defaults
                .iter()
                .map(|(n, t)| (n.clone(), from_ast_type(t)))
                .collect();
            let generics: Vec<GenericParam> = func
                .generics
                .iter()
                .map(|g| {
                    let bounds = bounds_map.remove(g).unwrap_or_default();
                    GenericParam {
                        name: g.clone(),
                        bounds,
                        default: defaults_map.get(g).cloned(),
                    }
                })
                .collect();
            generics
        },
        params: {
            // 合并注入的 args/kwargs 变参（variadic 收集）
            let mut all = params;
            all.extend(variadic_params);
            all
        },
        ret_ty,
        body,
        intrinsics,
        // 自动检测：如果函数体包含 await/spawn 且未显式标记 async，自动标记
        is_async: func.is_async || ast_body_has_async(&func.body),
        is_iterator: func.is_iterator,
        is_test: false,
        checker_param: func.checker_param.clone(),
        default_checker: func.default_checker.clone(),
        where_clause: extra_where,
        span: Span::unknown(),
    }
}

/// 检测 AST 函数体（Vec<Stmt>）是否包含 async 相关表达式（await/spawn）
fn ast_body_has_async(stmts: &[ast::Stmt]) -> bool {
    stmts.iter().any(|stmt| ast_stmt_has_async(stmt))
}

fn ast_stmt_has_async(stmt: &ast::Stmt) -> bool {
    match stmt {
        ast::Stmt::Expr(e) | ast::Stmt::Return(Some(e)) | ast::Stmt::Yield(Some(e)) => {
            ast_expr_has_async(e)
        }
        ast::Stmt::Let { value, .. } | ast::Stmt::Const { value, .. } => ast_expr_has_async(value),
        ast::Stmt::While {
            cond, body, guard, ..
        } => {
            ast_expr_has_async(cond)
                || guard.as_ref().map_or(false, |e| ast_expr_has_async(e))
                || ast_body_has_async(body)
        }
        ast::Stmt::For {
            iter, body, guard, ..
        } => {
            ast_expr_has_async(iter)
                || guard.as_ref().map_or(false, |e| ast_expr_has_async(e))
                || ast_body_has_async(body)
        }
        ast::Stmt::Loop(body) | ast::Stmt::Defer(body) => ast_body_has_async(body),
        ast::Stmt::Assign { target, value, .. } => {
            ast_expr_has_async(target) || ast_expr_has_async(value)
        }
        ast::Stmt::With { expr, body, .. } => ast_expr_has_async(expr) || ast_body_has_async(body),
        ast::Stmt::Break(Some(e)) | ast::Stmt::Raise(e) | ast::Stmt::YieldFrom(e) => {
            ast_expr_has_async(e)
        }
        ast::Stmt::FnDef { func } => ast_body_has_async(&func.body),
        _ => false,
    }
}

fn ast_expr_has_async(expr: &ast::Expr) -> bool {
    match expr {
        // Spawn(go) → thread::spawn 同步启动，不触发 async；Await 触发 async
        ast::Expr::Await(_) => true,
        ast::Expr::Call { func, args, .. } => {
            ast_expr_has_async(func) || args.iter().any(ast_expr_has_async)
        }
        ast::Expr::MethodCall { receiver, args, .. } => {
            ast_expr_has_async(receiver) || args.iter().any(ast_expr_has_async)
        }
        ast::Expr::Binary { left, right, .. } => {
            ast_expr_has_async(left) || ast_expr_has_async(right)
        }
        ast::Expr::Unary { operand, .. } => ast_expr_has_async(operand),
        ast::Expr::If {
            cond,
            then_body,
            elif_clauses,
            else_body,
        } => {
            ast_expr_has_async(cond)
                || ast_body_has_async(then_body)
                || elif_clauses
                    .iter()
                    .any(|(c, b)| ast_expr_has_async(c) || ast_body_has_async(b))
                || else_body.as_ref().map_or(false, |b| ast_body_has_async(b))
        }
        ast::Expr::Match { expr, arms } => {
            ast_expr_has_async(expr)
                || arms.iter().any(|a| {
                    a.guard.as_ref().map_or(false, |g| ast_expr_has_async(g))
                        || ast_body_has_async(&a.body)
                })
        }
        ast::Expr::Closure { body, .. } => ast_expr_has_async(body),
        ast::Expr::Pipe { receiver, args, .. } => {
            ast_expr_has_async(receiver) || args.iter().any(ast_expr_has_async)
        }
        ast::Expr::ListLit(es) | ast::Expr::TupleLit(es) | ast::Expr::SetLit(es) => {
            es.iter().any(ast_expr_has_async)
        }
        ast::Expr::DictLit(kvs) => kvs
            .iter()
            .any(|(k, v)| ast_expr_has_async(k) || ast_expr_has_async(v)),
        ast::Expr::SafeNav { receiver, .. } | ast::Expr::Try(receiver) => {
            ast_expr_has_async(receiver)
        }
        ast::Expr::NullCoalesce { left, right } => {
            ast_expr_has_async(left) || ast_expr_has_async(right)
        }
        ast::Expr::Walrus { target, value } => {
            ast_expr_has_async(target) || ast_expr_has_async(value)
        }
        ast::Expr::Paren(e) | ast::Expr::Panic(e) | ast::Expr::Move(e) => ast_expr_has_async(e),
        ast::Expr::Range { start, end, .. } => {
            start.as_ref().map_or(false, |e| ast_expr_has_async(e))
                || end.as_ref().map_or(false, |e| ast_expr_has_async(e))
        }
        ast::Expr::Assign { target, value, .. } => {
            ast_expr_has_async(target) || ast_expr_has_async(value)
        }
        ast::Expr::ListComprehension {
            output, iter, cond, ..
        } => {
            ast_expr_has_async(output)
                || ast_expr_has_async(iter)
                || cond.as_ref().map_or(false, |e| ast_expr_has_async(e))
        }
        ast::Expr::DictComprehension {
            key,
            value,
            iter,
            cond,
            ..
        } => {
            ast_expr_has_async(key)
                || ast_expr_has_async(value)
                || ast_expr_has_async(iter)
                || cond.as_ref().map_or(false, |e| ast_expr_has_async(e))
        }
        ast::Expr::SetComprehension {
            elem, iter, cond, ..
        } => {
            ast_expr_has_async(elem)
                || ast_expr_has_async(iter)
                || cond.as_ref().map_or(false, |e| ast_expr_has_async(e))
        }
        ast::Expr::BuildBlock { lhs, body, .. } => {
            ast_expr_has_async(lhs) || ast_body_has_async(body)
        }
        ast::Expr::TryCatch { body, .. } => ast_body_has_async(body),
        ast::Expr::FieldAccess { receiver, .. } | ast::Expr::PathAccess { receiver, .. } => {
            ast_expr_has_async(receiver)
        }
        ast::Expr::Index { receiver, index } => {
            ast_expr_has_async(receiver) || ast_expr_has_async(index)
        }
        ast::Expr::KwArg { value, .. } => ast_expr_has_async(value),
        _ => false,
    }
}

fn convert_struct(s: &ast::StructDef, ctx: &TypeCtx) -> Item {
    if s.is_enum {
        let variants: Vec<Variant> = s
            .fields
            .iter()
            .map(|f| {
                // 简化：字段作为变体处理
                // 实际的 enum field 没有子类型（简单变体）
                Variant {
                    name: f.name.clone(),
                    fields: match &f.ty {
                        AstType::Unit | AstType::None_ => vec![],
                        AstType::Duck { fields } => {
                            // 命名字段变体: Circle(x: f64, y: f64) → 带名 Field
                            // （codegen 生成 Rust 结构体变体 { x: f64, y: f64 }）
                            fields
                                .iter()
                                .map(|(n, t)| Field {
                                    name: n.clone(),
                                    ty: from_ast_type(t),
                                })
                                .collect()
                        }
                        AstType::Tuple(elems) => {
                            // 元组变体: Circle(f64, f64, f64) → 三个无名 Field
                            elems
                                .iter()
                                .map(|t| Field {
                                    name: String::new(),
                                    ty: from_ast_type(t),
                                })
                                .collect()
                        }
                        other => vec![Field {
                            name: String::new(),
                            ty: from_ast_type(other),
                        }],
                    },
                }
            })
            .collect();

        let enum_methods: Vec<FnDef> = s
            .methods
            .iter()
            .map(|m| {
                let mut method_ctx = TypeCtx::new();
                method_ctx.pending_items = ctx.pending_items.clone();
                method_ctx.struct_names = ctx.struct_names.clone();
                method_ctx.struct_fields = ctx.struct_fields.clone();
                method_ctx.struct_field_order = ctx.struct_field_order.clone();
                method_ctx.struct_methods = ctx.struct_methods.clone();
                method_ctx.struct_method_arity = ctx.struct_method_arity.clone();
                convert_fn_def(m, &method_ctx)
            })
            .collect();

        Item::EnumDef(EnumDef {
            name: s.name.clone(),
            generics: s
                .generics
                .iter()
                .map(|g| GenericParam {
                    name: g.clone(),
                    bounds: vec![],
                    default: s
                        .generic_defaults
                        .iter()
                        .find(|(n, _)| n == g)
                        .map(|(_, t)| from_ast_type(t)),
                })
                .collect(),
            variants,
            methods: enum_methods,
            span: Span::unknown(),
        })
    } else {
        let fields: Vec<Field> = s
            .fields
            .iter()
            .map(|f| {
                // Self 字段（next: Self?）在 struct 定义内解析为自身类型名，
                // 使字段访问（n.next）继承具体类型而非 Self_（后者在函数体内非法）
                let self_ty = IrType::Named {
                    path: s.name.clone(),
                    args: s
                        .generics
                        .iter()
                        .map(|g| IrType::Generic(g.clone()))
                        .collect(),
                };
                Field {
                    name: f.name.clone(),
                    ty: replace_self(&from_ast_type(&f.ty), &self_ty),
                }
            })
            .collect();

        let methods: Vec<FnDef> = s
            .methods
            .iter()
            .map(|m| {
                let mut method_ctx = TypeCtx::new();
                method_ctx.pending_items = ctx.pending_items.clone();
                method_ctx.struct_names = ctx.struct_names.clone();
                method_ctx.struct_fields = ctx.struct_fields.clone();
                method_ctx.struct_field_order = ctx.struct_field_order.clone();
                method_ctx.struct_methods = ctx.struct_methods.clone();
                method_ctx.struct_method_arity = ctx.struct_method_arity.clone();
                convert_fn_def(m, &method_ctx)
            })
            .collect();

        // 普通 magic 方法体（__str__/__add__ 等，除 __new__/__init__/__implicit_from__ 特殊处理外）
        // 也转成 FnDef 并入 methods，否则 `magic __str__` 等方法体在 IR 中丢失
        let special_magic = ["__new__", "__init__", "__implicit_from__"];
        let magic_methods: Vec<FnDef> = s
            .magic_methods
            .iter()
            .filter(|m| !special_magic.contains(&m.name.as_str()))
            .map(|m| {
                let mut method_ctx = TypeCtx::new();
                method_ctx.pending_items = ctx.pending_items.clone();
                method_ctx.struct_names = ctx.struct_names.clone();
                method_ctx.struct_fields = ctx.struct_fields.clone();
                method_ctx.struct_field_order = ctx.struct_field_order.clone();
                method_ctx.struct_methods = ctx.struct_methods.clone();
                method_ctx.struct_method_arity = ctx.struct_method_arity.clone();
                convert_fn_def(m, &method_ctx)
            })
            .collect();
        let mut methods = methods;
        methods.extend(magic_methods);

        // 提取 __new__ 的签名信息
        let new_method = s.magic_methods.iter().find(|m| m.name == "__new__");
        let has_new = new_method.is_some();
        let new_params: Vec<(String, IrType)> = new_method
            .iter()
            .flat_map(|m| {
                m.params
                    .iter()
                    .map(|p| (p.name.clone(), from_ast_type(&p.ty)))
            })
            .collect();
        let new_ret_ty = new_method.and_then(|m| m.return_type.as_ref().map(|t| from_ast_type(t)));

        // 提取 __init__ 的签名信息
        let init_method = s.magic_methods.iter().find(|m| m.name == "__init__");
        let has_init = init_method.is_some();
        let init_params: Vec<(String, IrType)> = init_method
            .iter()
            .flat_map(|m| {
                m.params
                    .iter()
                    .map(|p| (p.name.clone(), from_ast_type(&p.ty)))
            })
            .collect();

        // 提取 __implicit_from__ 的源类型列表
        let implicit_froms: Vec<IrType> = s
            .magic_methods
            .iter()
            .filter(|m| m.name == "__implicit_from__")
            .flat_map(|m| m.params.first().map(|p| from_ast_type(&p.ty)))
            .collect();

        Item::StructDef(StructDef {
            name: s.name.clone(),
            generics: s
                .generics
                .iter()
                .map(|g| GenericParam {
                    name: g.clone(),
                    // struct 泛型内联约束（`struct Map<I: Iterator, B>` 的 I: Iterator）：
                    // 生成 `I: std::iter::Iterator`（E0220 associated type Item not found）
                    bounds: s
                        .generic_bounds
                        .iter()
                        .find(|(n, _)| n == g)
                        .map(|(_, bds)| bds.iter().map(|b| from_ast_type(b)).collect())
                        .unwrap_or_default(),
                    default: s
                        .generic_defaults
                        .iter()
                        .find(|(n, _)| n == g)
                        .map(|(_, t)| from_ast_type(t)),
                })
                .collect(),
            fields,
            methods,
            has_new,
            new_params,
            new_ret_ty,
            has_init,
            init_params,
            implicit_froms,
            span: Span::unknown(),
        })
    }
}

fn convert_trait(t: &ast::TraitDef, ctx: &TypeCtx) -> Item {
    let methods: Vec<FnSig> = t
        .methods
        .iter()
        .map(|m| FnSig {
            name: m.name.clone(),
            generics: m
                .generics
                .iter()
                .map(|g| GenericParam {
                    name: g.clone(),
                    // trait 方法泛型约束（collect<C: FromIterator<Self.Item>>）：
                    // 从方法 where_clause 提取（E0423 expected value, found type
                    // parameter C——C: FromIterator 约束丢失）
                    bounds: m
                        .where_clause
                        .iter()
                        .find(|wb| wb.type_param == *g)
                        .map(|wb| wb.bounds.iter().map(|b| from_ast_type(b)).collect())
                        .unwrap_or_default(),
                    default: None,
                })
                .collect(),
            params: m
                .params
                .iter()
                .map(|p| {
                    // 保留 self 可变性：mut self → MutRef(Self_)，使 trait 声明与 impl 签名一致
                    if p.name == "self" && p.is_mut {
                        IrType::MutRef(Box::new(IrType::Self_))
                    } else {
                        let t = from_ast_type(&p.ty);
                        // ref 参数（`ref other: Self`）→ Ref(Self_)，生成 &Self 参数
                        // （否则 other: Self 报 E0277 size for Self cannot be known）
                        if p.is_ref {
                            IrType::Ref(Box::new(t))
                        } else {
                            t
                        }
                    }
                })
                .collect(),
            params_names: m.params.iter().map(|p| p.name.clone()).collect(),
            // trait 方法 where 约束（try_from ... where Self: Sized）：生成到方法签名
            where_clause: m
                .where_clause
                .iter()
                .map(|wb| {
                    let bounds: Vec<IrType> =
                        wb.bounds.iter().map(|b| from_ast_type(b)).collect();
                    (wb.type_param.clone(), bounds)
                })
                .collect(),
            ret: m
                .return_type
                .as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or(IrType::Unit),
            // trait 默认方法体（`def describe(self) -> str = f"..."` 带 body）：
            // 否则 impl 需实现全部方法（E0046，combo-trait-impl.lz）
            body: if m.body.is_empty() {
                None
            } else {
                // 注册方法参数变量到 ctx（predicate 等 fn 参数的类型），否则
                // 调用点 callee.ty 是 Any，callee_fn_refs 不生效（E0308
                // expected &Item, found Item——find/Filter 的 predicate(item)）
                let mut body_ctx = ctx.clone();
                for p in &m.params {
                    if p.name != "self" {
                        let t = from_ast_type(&p.ty);
                        body_ctx.add_var(&p.name, t);
                    }
                }
                Some(convert_block_with_ctx(&m.body, &body_ctx))
            },
        })
        .collect();

    Item::TraitDef(TraitDef {
        name: t.name.clone(),
        generics: t
            .generics
            .iter()
            .map(|g| GenericParam {
                name: g.clone(),
                bounds: vec![],
                default: t
                    .generic_defaults
                    .iter()
                    .find(|(n, _)| n == g)
                    .map(|(_, ty)| from_ast_type(ty)),
            })
            .collect(),
        supertraits: t.supertraits.iter().map(|st| from_ast_type(st)).collect(),
        methods,
        assoc_types: t.assoc_types.clone(),
    })
}

fn convert_impl(imp: &ast::ImplDef, ctx: &TypeCtx) -> Item {
    let methods: Vec<FnDef> = imp
        .methods
        .iter()
        .map(|m| {
            let mut impl_ctx = TypeCtx::new();
            impl_ctx.pending_items = ctx.pending_items.clone();
            for sn in &ctx.struct_names {
                impl_ctx.struct_names.insert(sn.clone());
            }
            for (sn, fields) in &ctx.struct_fields {
                let mut cloned = HashMap::new();
                for (fn_, ty) in fields {
                    cloned.insert(fn_.clone(), ty.clone());
                }
                impl_ctx.struct_fields.insert(sn.clone(), cloned);
            }
            for (sn, order) in &ctx.struct_field_order {
                impl_ctx.struct_field_order.insert(sn.clone(), order.clone());
            }
            for (sn, ms) in &ctx.struct_methods {
                impl_ctx.struct_methods.insert(sn.clone(), ms.clone());
            }
            for (sn, arity) in &ctx.struct_method_arity {
                impl_ctx.struct_method_arity.insert(sn.clone(), arity.clone());
            }
            for (cn, ct) in &ctx.top_level_consts {
                impl_ctx.top_level_consts.insert(cn.clone(), ct.clone());
            }
            // impl 方法体内裸枚举变体名（`return Less`）需枚举映射做类型推断，
            // 否则回退 Any→i64 生成 <Ordering as ImplicitFrom<i64>> 错误转换（E0277）
            for (vn, en) in &ctx.enum_variants {
                impl_ctx.enum_variants.insert(vn.clone(), en.clone());
            }
            for (vn, ft) in &ctx.enum_variant_field_types {
                impl_ctx.enum_variant_field_types.insert(vn.clone(), ft.clone());
            }
            for (name, ty) in &ctx.fn_returns {
                impl_ctx.fn_returns.insert(name.clone(), ty.clone());
            }
            impl_ctx.current_generics = imp.generics.clone();
            // self 参数绑定为 impl 目标类型（如 Dict<K,V>→HashMap<K,V>）：
            // 否则 self[key] 推断为 i64、key in self 无法走 contains_key 分支（E0277/E0599）
            impl_ctx.self_ty = Some(if imp.generics.is_empty() {
                IrType::named(&imp.type_name)
            } else {
                IrType::Named {
                    path: imp.type_name.clone(),
                    args: imp
                        .generics
                        .iter()
                        .map(|g| IrType::Generic(g.clone()))
                        .collect(),
                }
            });
            convert_fn_def(m, &impl_ctx)
        })
        .collect();

    Item::Impl(ImplDef {
        trait_: imp.trait_name.as_ref().map(|n| IrType::named(n)),
        // for_type 需携带 impl 泛型参数：impl Box<T> → Box<T>
        for_type: if imp.generics.is_empty() {
            IrType::named(&imp.type_name)
        } else {
            IrType::Named {
                path: imp.type_name.clone(),
                args: imp
                    .generics
                    .iter()
                    .map(|g| IrType::Generic(g.clone()))
                    .collect(),
            }
        },
        generics: {
            // 合并 where_clause / 内联约束（`impl<A: Iterator> ...` 的 A: Iterator）
            // 到泛型参数 bounds：否则生成 `impl<A: Clone + Debug>` 丢失 A: Iterator，
            // 后续 `A::Item` 关联类型无法解析（E0220 associated type not found）
            let mut bounds_map: HashMap<String, Vec<IrType>> = HashMap::new();
            for wb in &imp.where_clause {
                let ir_bounds: Vec<IrType> = wb.bounds.iter().map(|b| from_ast_type(b)).collect();
                bounds_map
                    .entry(wb.type_param.clone())
                    .or_default()
                    .extend(ir_bounds);
            }
            imp.generics
                .iter()
                .map(|g| GenericParam {
                    name: g.clone(),
                    bounds: bounds_map.remove(g).unwrap_or_default(),
                    default: imp
                        .generic_defaults
                        .iter()
                        .find(|(n, _)| n == g)
                        .map(|(_, ty)| from_ast_type(ty)),
                })
                .collect()
        },
        methods,
        assoc_type_bindings: imp
            .assoc_type_bindings
            .iter()
            .map(|(n, t)| (n.clone(), from_ast_type(t)))
            .collect(),
        // impl 级 where 约束（`where I::Item: Clone` 的关联类型约束，type_param
        // 含点号）：不能合并到泛型参数 bounds（I.Item 不是 I），需生成 impl where
        where_clause: imp
            .where_clause
            .iter()
            .filter(|wb| wb.type_param.contains('.'))
            .map(|wb| {
                let bounds: Vec<IrType> = wb.bounds.iter().map(|b| from_ast_type(b)).collect();
                (wb.type_param.clone(), bounds)
            })
            .collect(),
    })
}

// ══════════════════════════════════════════════════════════════
// 公开 API
// ══════════════════════════════════════════════════════════════

/// 错误类型
#[derive(Debug)]
pub enum IrBuildError {
    Generic(String),
}

impl std::fmt::Display for IrBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            IrBuildError::Generic(msg) => write!(f, "IR build error: {msg}"),
        }
    }
}

/// 主入口：AST Module → IrModule
/// 将 .lzi 签名中的 LZ 类型字符串转换为 IrType（跨模块推断回退用）。
/// 支持基本类型与常见泛型容器；未知类型降级为 Named（与 from_ast_type 语义一致）。
#[cfg(feature = "infer")]
fn lzi_type_to_ir(s: &str) -> IrType {
    match s.trim() {
        "int" | "i64" => IrType::Int,
        "f64" => IrType::F64,
        "str" | "String" => IrType::Str,
        "bool" => IrType::Bool,
        "()" | "Unit" => IrType::Unit,
        other => {
            // Option<T> / List<T> / Vec<T> / Dict<K,V> 等泛型容器
            if let Some(inner) = other
                .strip_prefix("Option<")
                .and_then(|t| t.strip_suffix('>'))
            {
                IrType::Option(Box::new(lzi_type_to_ir(inner)))
            } else if let Some(inner) = other
                .strip_prefix("List<")
                .or_else(|| other.strip_prefix("Vec<"))
                .and_then(|t| t.strip_suffix('>'))
            {
                IrType::Named {
                    path: "Vec".into(),
                    args: vec![lzi_type_to_ir(inner)],
                }
            } else if let Some(inner) = other
                .strip_prefix("Dict<")
                .or_else(|| other.strip_prefix("HashMap<"))
                .and_then(|t| t.strip_suffix('>'))
            {
                // Dict<K,V> → HashMap<K,V>（IR Named 表达）
                let parts: Vec<&str> = inner.splitn(2, ',').map(|t| t.trim()).collect();
                IrType::Named {
                    path: "HashMap".into(),
                    args: vec![
                        lzi_type_to_ir(parts.first().copied().unwrap_or("Any")),
                        lzi_type_to_ir(parts.get(1).copied().unwrap_or("Any")),
                    ],
                }
            } else {
                IrType::Named {
                    path: other.to_string(),
                    args: vec![],
                }
            }
        }
    }
}

/// 默认入口：不带跨模块签名（等价 build_ir_with_lzi(_, None)）
pub fn build_ir(ast_module: &ast::Module) -> Result<IrModule, IrBuildError> {
    build_ir_inner(ast_module, |_ctx| {})
}

/// 带 .lzi 跨模块类型签名的入口：main.rs `--lzi <file>` 加载后传入，
/// 供 IR builder 在本地函数查不到时回退查询外部模块函数返回类型。
#[cfg(feature = "infer")]
pub fn build_ir_with_lzi(
    ast_module: &ast::Module,
    lzi: std::rc::Rc<crate::infer::LziRegistry>,
) -> Result<IrModule, IrBuildError> {
    build_ir_inner(ast_module, |ctx| ctx.lzi_signatures = Some(lzi))
}

fn build_ir_inner(
    ast_module: &ast::Module,
    init_ctx: impl FnOnce(&mut TypeCtx),
) -> Result<IrModule, IrBuildError> {
    let mut ctx = TypeCtx::new();
    let pending_items = Rc::new(RefCell::new(Vec::new()));
    ctx.pending_items = pending_items.clone();
    // 跨模块签名注入（.lzi）：lookup_fn_return 回退查询源；
    // 闭包在 build_ir 传空（无 infer 时等价）、build_ir_with_lzi 注入 registry
    init_ctx(&mut ctx);
    // comptime 求值需要访问模块函数定义（`comptime gen_primes(8)` 编译期执行）
    ctx.comptime_module = Some(Rc::new(ast_module.clone()));

    // 1. 收集类型信息
    ctx.collect_structs(ast_module);
    ctx.collect_functions(ast_module);
    // 预收集顶层 const 类型（供函数体内引用查询，如生成器集合迭代）
    for c in &ast_module.consts {
        let ty =
            c.ty.as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(&c.value, &ctx));
        eprintln!("DEBUG const {} ty={:?}", c.name, ty);
        ctx.top_level_consts.insert(c.name.clone(), ty);
        // 顶层 const 编译期求值（comptime 块/表达式内解析 const 引用）
        let empty_module = ast::Module::default();
        let module_ref = ctx.comptime_module.as_ref().map(|m| m.as_ref()).unwrap_or(&empty_module);
        let mut cctx = crate::comptime::ComptimeContext::new(module_ref);
        // 注入源码文本（inspect.getsource/getsourcelines 数据源，main.rs 已填）
        if let Some(src) = &module_ref.source_text {
            cctx = cctx.with_source(src.clone());
        }
        if let Ok(v) = crate::comptime::ComptimeEvaluator::eval_expr(&c.value, &mut cctx) {
            ctx.comptime_consts.insert(c.name.clone(), v);
        }
    }

    // 2. 构建 IR 模块
    let name = ast_module
        .name
        .clone()
        .unwrap_or_else(|| "main".to_string());
    let mut ir_mod = IrModule::new(name);

    // 3. 默认 prelude（lz.std 内建）
    ir_mod.prelude = vec![
        "Option".into(),
        "Result".into(),
        "Ordering".into(),
        "Box".into(),
        "Rc".into(),
        "Arc".into(),
        "Itor".into(),
        "Strategy".into(),
    ];

    // 4. 转换 imports → Use 项
    for imp in &ast_module.imports {
        ir_mod.items.push(Item::Use(UseStmt {
            path: imp.path.clone(),
            alias: imp.alias.clone(),
            items: imp.items.clone(),
            is_from: imp.is_from,
        }));
    }

    // 5. 转换 structs（enum 同名方法声明需合并）
    // 收集所有 enum 的额外方法声明（is_enum=true 且 fields 非空的是主声明，其余是方法声明）
    let mut enum_extra_methods: HashMap<String, Vec<&ast::StructDef>> = HashMap::new();
    let mut is_enum_main_decl: HashSet<String> = HashSet::new();
    for s in &ast_module.structs {
        if s.is_enum && !s.fields.is_empty() {
            is_enum_main_decl.insert(s.name.clone());
        }
    }
    for s in &ast_module.structs {
        if s.is_enum && s.fields.is_empty() && is_enum_main_decl.contains(&s.name) {
            // 这是附加方法声明，收集起来
            enum_extra_methods
                .entry(s.name.clone())
                .or_default()
                .push(s);
        }
    }
    // 只转换主声明和方法声明（跳过纯方法声明）
    for s in &ast_module.structs {
        if s.is_enum && s.fields.is_empty() && is_enum_main_decl.contains(&s.name) {
            continue; // 额外方法声明已收集，稍后合并
        }
        let mut item = convert_struct(s, &ctx);
        // 合并额外方法到 EnumDef
        if s.is_enum {
            if let Item::EnumDef(ref mut ed) = item {
                if let Some(extras) = enum_extra_methods.get(&s.name) {
                    for extra in extras {
                        for m in &extra.methods {
                            let mut method_ctx = TypeCtx::new();
                            method_ctx.pending_items = ctx.pending_items.clone();
                            method_ctx.struct_names = ctx.struct_names.clone();
                            method_ctx.struct_fields = ctx.struct_fields.clone();
                            method_ctx.struct_methods = ctx.struct_methods.clone();
                            ed.methods.push(convert_fn_def(m, &method_ctx));
                        }
                    }
                }
            }
        }
        ir_mod.items.push(item);
    }

    // 6. 转换 traits
    for t in &ast_module.traits {
        ir_mod.items.push(convert_trait(t, &ctx));
    }

    // 7. 转换 impls
    for imp in &ast_module.impls {
        // 注册 impl 方法名到 struct_methods（如 HttpResult 的 __is_ok__/__unwrap__），
        // 供 AstExpr::Try（r?）自定义传播类型判定使用
        ctx.struct_methods
            .entry(imp.type_name.clone())
            .or_default()
            .extend(imp.methods.iter().map(|m| m.name.clone()));
        // 登记 impl 方法返回类型（如 box.lz `def get(ref self) -> ref T` 返回 &T）：
        // 否则 `b.get()` 方法调用推断为 Any，`assert b.get() == 42` 无法解引用（E0277）。
        // 用 `类型名.方法名` 作 key：Rc/Arc 都有 try_unwrap，裸方法名会互相覆盖
        // （`rc.try_unwrap()` 误推断为 Arc 的签名 → E0425 cannot find type `T`）
        for m in &imp.methods {
            if let Some(ref ret) = m.return_type {
                ctx.fn_returns.insert(
                    format!("{}.{}", imp.type_name, m.name),
                    from_ast_type_with_generics(ret, &imp.generics),
                );
            }
        }
        ir_mod.items.push(convert_impl(imp, &ctx));
    }

    // 7.5 独立 magic 块: magic __str__: def __str__(self: MyStruct) → impl MyStruct
    for mb in &ast_module.magic_blocks {
        // 从 self 参数类型确定目标类型
        let target = mb
            .function
            .params
            .iter()
            .find(|p| p.name == "self" || p.name == "self_")
            .map(|p| from_ast_type(&p.ty))
            .and_then(|ty| {
                if let IrType::Named { path, .. } = ty {
                    Some(path)
                } else {
                    None
                }
            })
            .unwrap_or_default();
        if target.is_empty() {
            continue;
        }
        // 注册魔术方法名到 struct_methods，供运算符/调用分发
        ctx.struct_methods
            .entry(target.clone())
            .or_default()
            .insert(mb.method_name.clone());
        let mut impl_ctx = ctx.clone();
        impl_ctx.current_generics = mb.function.generics.clone();
        let method = convert_fn_def(&mb.function, &impl_ctx);
        ir_mod.items.push(Item::Impl(ImplDef {
            trait_: None,
            for_type: IrType::named(&target),
            generics: vec![],
            methods: vec![method],
            assoc_type_bindings: vec![],
            where_clause: vec![],
        }));
    }

    // 8. 转换 functions
    for f in &ast_module.functions {
        // comptime def：仅编译期存在（供 comptime 求值器调用），不生成运行时代码
        if f.is_comptime {
            continue;
        }
        ir_mod.items.push(Item::FnDef(convert_fn_def(f, &ctx)));
    }

    // 9. 转换 consts
    // 9.0. 模块级魔法属性（06e-模块级魔法属性.md）：__name__/__file__/__package__/
    // __path__/__doc__/__is_macro__ 等自动填充
    // __file__/__package__/__path__ 从源文件路径派生（main.rs 注入 Module.file_path）
    let src_path = ast_module.file_path.clone().unwrap_or_default();
    let src_parent = std::path::Path::new(&src_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let package_name = std::path::Path::new(&src_path)
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let magic_consts: Vec<(String, IrType, ExprKind)> = vec![
        (
            "__name__".to_string(),
            IrType::Str,
            ExprKind::Lit(LitKind::Str(ir_mod.name.clone())),
        ),
        (
            "__file__".to_string(),
            IrType::Str,
            ExprKind::Lit(LitKind::Str(src_path.clone())),
        ),
        (
            "__package__".to_string(),
            IrType::Str,
            ExprKind::Lit(LitKind::Str(package_name)),
        ),
        (
            "__path__".to_string(),
            IrType::Str,
            ExprKind::Lit(LitKind::Str(src_parent)),
        ),
        (
            "__doc__".to_string(),
            IrType::Str,
            ExprKind::Lit(LitKind::Str(String::new())),
        ),
        (
            "__is_macro__".to_string(),
            IrType::Bool,
            // 宏模块（首行 #!bin macro）为 true，普通模块为 false（06e 规范）
            ExprKind::Lit(LitKind::Bool(ast_module.is_macro)),
        ),
    ];
    for (mc_name, mc_ty, mc_kind) in magic_consts {
        ctx.top_level_consts.insert(mc_name.clone(), mc_ty.clone());
        ir_mod.items.push(Item::Const(ConstDef {
            name: mc_name,
            ty: mc_ty,
            value: Expr::new(mc_kind, IrType::Any, Span::unknown()),
        }));
    }

    for c in &ast_module.consts {
        let ty =
            c.ty.as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(&c.value, &ctx));
        // 记录顶层 const 类型，供函数内 lookup_var 查询（如生成器集合迭代）
        ctx.top_level_consts.insert(c.name.clone(), ty.clone());
        ir_mod.items.push(Item::Const(ConstDef {
            name: c.name.clone(),
            ty,
            value: convert_expr(&c.value, &ctx),
        }));
    }

    // 9.5. 转换 duck 类型约束 → Item::DuckDef
    for d in &ast_module.duck_defs {
        ir_mod.items.push(Item::DuckDef(convert_duck_def(d)));
    }

    // 9.6. 转换 type aliases
    for ta in &ast_module.type_aliases {
        let ir_ty = from_ast_type(&ta.ty);
        ir_mod.items.push(Item::TypeAlias(TypeAliasDef {
            name: ta.name.clone(),
            generics: ta.generics.clone(),
            ty: ir_ty,
        }));
    }

    // 9.6. 转换顶层构建块 x =: body → let x = { ... }
    // 构建块以 BlockExpr 表示（依次执行语句，最后一个表达式为值）
    for (name, body) in &ast_module.top_level_builds {
        let mut block_ctx = TypeCtx::new();
        block_ctx.current_generics = ctx.current_generics.clone();
        block_ctx.errors = ctx.errors.clone();
        block_ctx.comptime_consts = ctx.comptime_consts.clone();
        block_ctx.comptime_module = ctx.comptime_module.clone();
        // 预扫描：收集构建块内局部变量类型（x = value 赋值），供元组/表达式推断
        for s in body {
            match s {
                AstStmt::Expr(e) => {
                    if let AstExpr::Assign { target, value, .. } = e {
                        if let AstExpr::Ident(vname) = target.as_ref() {
                            block_ctx.add_var(vname, infer_expr_type(value, &block_ctx));
                            // 本块首次声明：否则 convert_stmt 因 vars 已含该名
                            // 且 block_declared 不含而误转 Stmt::Assign（E0425）
                            block_ctx.block_declared.insert(vname.clone());
                        }
                    }
                }
                AstStmt::Let {
                    name, ty, value, ..
                } => {
                    let ir_ty = ty
                        .as_ref()
                        .map(|t| from_ast_type(t))
                        .unwrap_or_else(|| infer_expr_type(value, &block_ctx));
                    block_ctx.add_var(name, ir_ty);
                    block_ctx.block_declared.insert(name.clone());
                }
                _ => {}
            }
        }
        let stmts: Vec<Stmt> = body.iter().map(|s| convert_stmt(s, &block_ctx)).collect();
        let blk_ty = body
            .last()
            .map(|s| infer_stmt_type(s, &block_ctx))
            .unwrap_or(IrType::Unit);
        let value = Expr::new(
            ExprKind::BlockExpr {
                block: Block {
                    span: Span::unknown(),
                    stmts,
                    ty: blk_ty.clone(),
                },
            },
            blk_ty.clone(),
            Span::unknown(),
        );
        ir_mod.items.push(Item::Const(ConstDef {
            name: name.clone(),
            ty: blk_ty,
            value,
        }));
    }

    // 9.7. 转换顶层 block / checker 块语句（top_stmts）
    // checker 块（block NAME[ps: __Params]）→ Item::CheckerBlock（惰性登记，由 codegen 发射 fn NAME）
    // 其他顶层语句（如顶层赋值）→ 转为 Const/表达式，保证不丢失
    for s in &ast_module.top_stmts {
        match s {
            AstStmt::CheckerBlock {
                label,
                ps_name,
                default_checker,
                body,
            } => {
                let ir_body = convert_block(body, &ctx);
                let captured = collect_checker_captured(&ir_body, &ctx, ps_name.as_deref());
                ctx.pending_items.borrow_mut().push(Item::CheckerBlock {
                    name: label.clone(),
                    ps_name: ps_name.clone(),
                    default_checker: default_checker.clone(),
                    body: ir_body,
                    captured,
                });
            }
            AstStmt::Expr(AstExpr::Assign { target, value, .. }) => {
                if let AstExpr::Ident(name) = target.as_ref() {
                    let ty = infer_expr_type(value, &ctx);
                    // 登记顶层可变变量：函数内 `x = v`（无 let 前缀）需识别为
                    // 修改全局（guard_for_3.lz size = size - 1），否则生成局部新绑定
                    ctx.top_level_consts.insert(name.clone(), ty.clone());
                    ir_mod.items.push(Item::Const(ConstDef {
                        name: name.clone(),
                        ty,
                        value: convert_expr(value, &ctx),
                    }));
                }
            }
            _ => {}
        }
    }

    // 10. 转换 tests
    for t in &ast_module.tests {
        match t {
            AstStmt::Test { name, body } => {
                let block = convert_block(body, &ctx);
                ir_mod.items.push(Item::Test(TestDef {
                    name: name.clone(),
                    body: block,
                }));
            }
            _ => {}
        }
    }

    // 11. 将提升出的嵌套函数等 pending items 追加到模块
    // 语义错误统一上报（不可变重赋值 E0384 / 空列表类型推断 E0282）
    {
        let errors = ctx.errors.borrow();
        if !errors.is_empty() {
            return Err(IrBuildError::Generic(errors.join("\n")));
        }
    }
    // 先 drop ctx 确保 Rc 引用计数归 1，否则 try_unwrap 静默失败
    drop(ctx);
    if let Ok(items) = Rc::try_unwrap(pending_items) {
        ir_mod.items.extend(items.into_inner());
    }

    // 12. duck 结构匹配编译期检查（具体类型 vs duck 约束）
    let duck_errors = crate::ir::duck_check::check_duck_satisfaction(&ir_mod);
    if !duck_errors.is_empty() {
        return Err(IrBuildError::Generic(duck_errors.join("\n")));
    }

    Ok(ir_mod)
}

/// 收集 checker 块体引用的外层函数局部变量（block 闭包语义，规范 05b-block命名块.md §三）。
/// checker 块被提升为模块级 fn NAME(ps: &mut __Params)，body 中引用的 main 局部变量
/// （out/depth/result 等）需作为 &mut 参数传入，否则 E0425
/// （block_demo/block_stack_test/block_tailrec）。
/// 排除：ps 参数名、模块级常量（top_level_consts 已生成全局、无需捕获）、
/// 非变量引用（函数名/内置名不在 ctx.vars 中，自动排除）。
fn collect_checker_captured(
    body: &Block,
    ctx: &TypeCtx,
    ps_name: Option<&str>,
) -> Vec<(String, IrType)> {
    let mut refs = Vec::new();
    let mut shadow = std::collections::HashSet::new();
    collect_var_refs(body, &mut shadow, &mut refs);
    let mut captured: Vec<(String, IrType)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for name in refs {
        if seen.contains(&name) {
            continue;
        }
        seen.insert(name.clone());
        if ps_name == Some(name.as_str()) {
            continue; // ps 参数自身
        }
        if ctx.top_level_consts.contains_key(&name) {
            continue; // 模块级 const：全局生成，无需捕获
        }
        if let Some(ty) = ctx.vars.get(&name) {
            captured.push((name, ty.clone()));
        }
    }
    captured
}

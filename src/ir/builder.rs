// Lang-Zone 编译器 — ir/builder.rs
// AST → LZIR-H 构造器：将 AST Module 转换为 IrModule
//
// 职责：
// 1. 逐节点 AST → LZIR 转换
// 2. 构建块脱糖（=:→Let, ^:→IndexGet, ~:→Call, *:→GenExpr）
// 3. 类型推导（从标注 + 简单传播 + 字面量推断）
// 4. 魔法方法归一化（MagicCall / MethodCall）

use crate::ast::{self, Expr as AstExpr, Stmt as AstStmt, Pattern as AstPattern, BinOp, UnaryOp, AssignOp, BuildKind};
use crate::types::Type as AstType;

use super::types::{IrType, from_ast_type, from_ast_type_with_generics};
use super::node::*;
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
    /// struct 方法名集合：struct_name → 方法名集合（含魔术方法）
    struct_methods: HashMap<String, HashSet<String>>,
    /// 顶层 const/static 类型：name → type
    top_level_consts: HashMap<String, IrType>,
    /// enum variant → enum name 映射
    enum_variants: HashMap<String, String>,
    /// 当前函数泛型参数
    current_generics: Vec<String>,
    /// 当前函数返回类型
    current_ret_ty: Option<IrType>,
    /// 当前函数名（用于嵌套函数命名）
    current_fn_name: Option<String>,
    /// 提升出的待处理顶级 Items（嵌套函数等）
    pending_items: Rc<RefCell<Vec<Item>>>,
}

impl TypeCtx {
    fn new() -> Self {
        TypeCtx {
            vars: HashMap::new(),
            fn_returns: HashMap::new(),
            fn_params: HashMap::new(),
            struct_names: HashSet::new(),
            struct_fields: HashMap::new(),
            struct_methods: HashMap::new(),
            top_level_consts: HashMap::new(),
            enum_variants: HashMap::new(),
            current_generics: vec![],
            current_ret_ty: None,
            current_fn_name: None,
            pending_items: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn collect_structs(&mut self, module: &ast::Module) {
        for s in &module.structs {
            if s.is_enum {
                for f in &s.fields {
                    self.enum_variants.insert(f.name.clone(), s.name.clone());
                }
            } else {
                self.struct_names.insert(s.name.clone());
                let mut fields = HashMap::new();
                for f in &s.fields {
                    fields.insert(f.name.clone(), from_ast_type(&f.ty));
                }
                self.struct_fields.insert(s.name.clone(), fields);
                // 收集 struct 方法名（含魔术方法）
                let mut mset: HashSet<String> = HashSet::new();
                for m in s.methods.iter() {
                    mset.insert(m.name.clone());
                }
                for m in s.magic_methods.iter() {
                    mset.insert(m.name.clone());
                }
                self.struct_methods.insert(s.name.clone(), mset);
            }
        }
    }

    fn collect_functions(&mut self, module: &ast::Module) {
        for f in &module.functions {
            let generics: Vec<String> = f.generics.clone();
            if let Some(ref ret_ty) = f.return_type {
                self.fn_returns.insert(
                    f.name.clone(),
                    from_ast_type_with_generics(ret_ty, &generics),
                );
            }
            let params: Vec<IrType> = f.params.iter()
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
        self.vars.get(name).cloned()
            .or_else(|| self.current_generics.iter()
                .find(|g| g.as_str() == name)
                .map(|g| IrType::Generic(g.clone())))
            .or_else(|| self.top_level_consts.get(name).cloned())
            .unwrap_or(IrType::Any)
    }

    fn lookup_fn_return(&self, name: &str) -> IrType {
        self.fn_returns.get(name).cloned().unwrap_or(IrType::Any)
    }

    fn is_struct(&self, name: &str) -> bool {
        self.struct_names.contains(name)
    }

    fn lookup_field(&self, struct_name: &str, field: &str) -> IrType {
        self.struct_fields.get(struct_name)
            .and_then(|fields| fields.get(field))
            .cloned()
            .unwrap_or(IrType::Any)
    }

    fn is_builtin_function(&self, name: &str) -> bool {
        matches!(name, "print" | "println" | "panic" | "len" | "contains"
            | "iter" | "enumerate" | "zip" | "sum" | "map" | "filter" | "collect"
            | "max" | "min" | "any" | "all" | "sorted" | "reversed" | "set!"
            | "format" | "hash" | "bool" | "range" | "clone" | "sort" | "reverse"
            | "spawn" | "go" | "__go" | "Exception" | "panic!")
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
        BinOp::Is => BinOpKind::Eq,        // Is 降级 (Rust 无 is 运算符)
    }
}

/// 运算符 → 魔术方法名 映射（用于用户自定义类型重载）
fn magic_method_for_binop(op: &BinOp) -> Option<&'static str> {
    match op {
        BinOp::Add => Some("__add__"),
        BinOp::Sub => Some("__sub__"),
        BinOp::Mul => Some("__mul__"),
        BinOp::Div => Some("__truediv__"),
        BinOp::Mod => Some("__mod__"),
        BinOp::Eq => Some("__eq__"),
        BinOp::Ne => Some("__ne__"),
        BinOp::Lt => Some("__lt__"),
        BinOp::Gt => Some("__gt__"),
        BinOp::Le => Some("__le__"),
        BinOp::Ge => Some("__ge__"),
        BinOp::BitAnd => Some("__and__"),
        BinOp::BitOr => Some("__or__"),
        BinOp::BitXor => Some("__xor__"),
        BinOp::Pow => Some("__pow__"),
        BinOp::In => Some("__contains__"),
        _ => None,
    }
}

fn map_unop(op: &UnaryOp) -> UnOpKind {
    match op {
        UnaryOp::Neg => UnOpKind::Neg,
        UnaryOp::Not => UnOpKind::Not,
        UnaryOp::BitNot => UnOpKind::Not,   // 位非降级为逻辑非
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

/// 将类型名字符串转换为 IrType（用于 `is` 运算符）  
fn name_to_ir_type(name: &str) -> IrType {
    match name {
        "int" | "i64" => IrType::Int,
        "str" | "String" => IrType::Str,
        "f64" | "float" => IrType::F64,
        "bool" => IrType::Bool,
        "List" | "Vec" => IrType::Named { path: "List".into(), args: vec![] },
        "Dict" | "HashMap" => IrType::Named { path: "Dict".into(), args: vec![] },
        "Set" | "HashSet" => IrType::Named { path: "Set".into(), args: vec![] },
        _ => IrType::Named { path: name.to_string(), args: vec![] },
    }
}

/// 编译期类型兼容检查（用于 `is` 运算符和类型转换）  
fn ir_types_compatible(a: &IrType, b: &IrType) -> bool {
    match (a, b) {
        // Any 与任何类型兼容（None、未知等）
        (IrType::Any, _) | (_, IrType::Any) => true,
        // 相同基础类型
        (IrType::Int, IrType::Int) | (IrType::F64, IrType::F64)
        | (IrType::Str, IrType::Str) | (IrType::Bool, IrType::Bool)
        | (IrType::Unit, IrType::Unit) | (IrType::Never, IrType::Never) => true,
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
            let names: Option<Vec<String>> = elems.iter()
                .map(|e| if let AstExpr::Ident(n) = e { Some(n.clone()) } else { None })
                .collect();
            names
        }
        _ => None,
    }
}

/// 将 LZ 类型名映射为 Rust 类型名（用于泛型类型参数）
fn map_type_args(names: &[String]) -> Vec<String> {
    names.iter().map(|t| match t.as_str() {
        "int" => "i64".to_string(),
        "str" => "String".to_string(),
        "f64" | "float" => "f64".to_string(),
        "bool" => "bool".to_string(),
        other => other.to_string(),
    }).collect()
}

/// 从实参类型解析泛型函数调用：推断泛型参数的具体类型
///
/// 策略：
/// 1. 收集函数定义中泛型参数名列表（从 param_tys 和 ret_ty 中提取 Generic 变量）
/// 2. 对每个参数位置，尝试将定义的 param_ty 与实参 arg_ty 匹配
/// 3. 如果 param_ty 是 Generic("T") 且 arg_ty 是具体类型，则将 T 绑定到 arg_ty
/// 4. 用绑定结果替换 ret_ty 中的泛型变量
fn resolve_call_generics(
    ret_ty: &IrType,
    _fn_name: &str,
    param_tys: &[IrType],
    arg_tys: &[IrType],
    ctx: &TypeCtx,
) -> IrType {
    // 收集所有泛型参数名
    let mut generic_names = std::collections::HashSet::new();
    fn collect_generics(ty: &IrType, set: &mut std::collections::HashSet<String>) {
        match ty {
            IrType::Generic(name) => { set.insert(name.clone()); }
            IrType::Named { args, .. } => { for a in args { collect_generics(a, set); } }
            IrType::Option(inner) => collect_generics(inner, set),
            IrType::Result { ok, err } => { collect_generics(ok, set); collect_generics(err, set); }
            IrType::Tuple(elems) => { for e in elems { collect_generics(e, set); } }
            IrType::Fn { params, ret } => { for p in params { collect_generics(p, set); } collect_generics(ret, set); }
            IrType::Ref(inner) | IrType::MutRef(inner) => collect_generics(inner, set),
            IrType::Duck { fields } => { for (_, t) in fields { collect_generics(t, set); } }
            _ => {}
        }
    }
    for pt in param_tys { collect_generics(pt, &mut generic_names); }
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
                bindings.entry(name.clone()).or_insert_with(|| arg_ty.clone());
            }
        }
        IrType::Named { path: p_path, args: p_args } => {
            if let IrType::Named { path: a_path, args: a_args } = arg_ty {
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
        IrType::Result { ok: p_ok, err: p_err } => {
            if let IrType::Result { ok: a_ok, err: a_err } = arg_ty {
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
        AstExpr::IntLit(_) => IrType::Int,
        AstExpr::FloatLit(_) => IrType::F64,
        AstExpr::StrLit(_) | AstExpr::FStrLit(_) | AstExpr::RawStrLit(_) => IrType::Str,
        AstExpr::BoolLit(_) => IrType::Bool,
        AstExpr::NoneLit => IrType::Any,  // None 类型取决于上下文
        AstExpr::Ident(name) => ctx.lookup_var(name),
        AstExpr::Call { func, args, .. } => {
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
                // str/int/float/bool 类型转换内建：返回对应类型
                match fname.as_str() {
                    "str" => return IrType::Str,
                    "int" => return IrType::Int,
                    "float" => return IrType::F64,
                    "bool" => return IrType::Bool,
                    "len" => return IrType::Int,
                    _ => {}
                }
                if ctx.is_struct(fname) {
                    return IrType::named(fname);
                }
                let ret_ty = ctx.lookup_fn_return(fname);
                // 泛型分辨率：如果返回类型包含 Generic，尝试从实参推断
                if ret_ty.contains_generics() {
                    if let Some(param_tys) = ctx.fn_params.get(fname) {
                        let arg_tys: Vec<IrType> = args.iter()
                            .map(|a| infer_expr_type(a, ctx))
                            .collect();
                        // 根据参数类型推断泛型变量
                        let resolved = resolve_call_generics(
                            &ret_ty, fname, param_tys, &arg_tys, ctx
                        );
                        return resolved;
                    }
                }
                ret_ty
            } else {
                IrType::Any
            }
        }
        AstExpr::MethodCall { receiver, method, .. } => {
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
            // 常见无返回值方法 → Unit
            if method == "push" || method == "insert" || method == "remove" || method == "clear" {
                return IrType::Unit;
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
                    // 用户 struct 的算术/构造魔术方法返回接收者类型
                    if ctx.struct_methods.get(path).map(|ms| ms.contains(method)).unwrap_or(false)
                        && matches!(method.as_str(), "__add__" | "__sub__" | "__mul__" | "__div__" | "__new__" | "__call__" | "__getitem__" | "__iter__" | "__setitem__") {
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
        AstExpr::Index { .. } => IrType::Any,
        AstExpr::Binary { left, op, .. } => {
            // `is` 运算符始终返回 Bool
            if matches!(op, BinOp::Is) {
                return IrType::Bool;
            }
            // 比较/布尔运算符返回 Bool
            if matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
                | BinOp::And | BinOp::Or | BinOp::In) {
                return IrType::Bool;
            }
            // 取左侧操作数的类型（简化）
            infer_expr_type(left, ctx)
        }
        AstExpr::Unary { op, operand } => {
            match op {
                _ => infer_expr_type(operand, ctx),
            }
        }
        AstExpr::If { then_body, .. } => {
            // 取 then 分支最后表达式类型
            then_body.last()
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit)
        }
        AstExpr::Match { arms, .. } => {
            arms.first()
                .and_then(|arm| arm.body.last())
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit)
        }
        AstExpr::Closure { .. } => IrType::Any,
        AstExpr::Range { .. } => IrType::named("Range"),
        AstExpr::Walrus { value, .. } => infer_expr_type(value, ctx),
        AstExpr::Pipe { func, .. } => ctx.lookup_fn_return(func),
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
                    IrType::Named { path, args } if path == "Option" && !args.is_empty() => args[0].clone(),
                    _ => left_ty,
                }
            }
        }
        AstExpr::ListLit(items) => {
            let elem_ty = items.first()
                .map(|i| infer_expr_type(i, ctx))
                .unwrap_or(IrType::Any);
            IrType::Named { path: "List".into(), args: vec![elem_ty] }
        }
        AstExpr::DictLit(entries) => {
            let key_ty = entries.first()
                .and_then(|(k, _)| Some(infer_expr_type(k, ctx)))
                .unwrap_or(IrType::Any);
            let val_ty = entries.first()
                .and_then(|(_, v)| Some(infer_expr_type(v, ctx)))
                .unwrap_or(IrType::Any);
            IrType::Named { path: "Dict".into(), args: vec![key_ty, val_ty] }
        }
        AstExpr::SetLit(items) => {
            let elem_ty = items.first()
                .map(|i| infer_expr_type(i, ctx))
                .unwrap_or(IrType::Any);
            IrType::Named { path: "Set".into(), args: vec![elem_ty] }
        }
        AstExpr::TupleLit(elems) => {
            IrType::Tuple(elems.iter().map(|e| infer_expr_type(e, ctx)).collect())
        }
        AstExpr::ListComprehension { output, .. } => {
            let elem_ty = infer_expr_type(output, ctx);
            IrType::Named { path: "List".into(), args: vec![elem_ty] }
        }
        AstExpr::DictComprehension { key, value, .. } => {
            let k_ty = infer_expr_type(key, ctx);
            let v_ty = infer_expr_type(value, ctx);
            IrType::Named { path: "Dict".into(), args: vec![k_ty, v_ty] }
        }
        AstExpr::SetComprehension { elem, .. } => {
            let elem_ty = infer_expr_type(elem, ctx);
            IrType::Named { path: "Set".into(), args: vec![elem_ty] }
        }
        AstExpr::Assign { value, .. } => infer_expr_type(value, ctx),
        AstExpr::Spawn(inner) => IrType::Named { path: "Future".into(), args: vec![infer_expr_type(inner, ctx)] },
        AstExpr::Move(inner) => infer_expr_type(inner, ctx),
        AstExpr::Panic(_) => IrType::Never,
        AstExpr::Await(inner) => {
            // await Future<T> → T
            let inner_ty = infer_expr_type(inner, ctx);
            match &inner_ty {
                IrType::Named { path, args } if path == "Future" && !args.is_empty() => args[0].clone(),
                _ => IrType::Any,
            }
        }
        AstExpr::BuildBlock { kind, lhs, body } => {
            // =: / ~: / *: 构建块返回 lhs 类型（或块类型）
            // ^: 索引构建块返回 lhs 的元素类型（如 Vec<T> → T）
            let lhs_ty = infer_expr_type(lhs, ctx);
            match kind {
                BuildKind::Index => {
                    match &lhs_ty {
                        IrType::Named { args, .. } if !args.is_empty() => args[0].clone(),
                        _ => lhs_ty,
                    }
                }
                BuildKind::Call => {
                    // ~: 构建块的实际返回类型是 callee 的返回值类型
                    // body 的类型是元组（被解包为 callee 的参数），不是调用结果
                    match &lhs_ty {
                        IrType::Fn { ret, .. } => *ret.clone(),
                        _ => IrType::Any,  // callee 类型未知，用 Any 避免错误类型标注
                    }
                }
                BuildKind::Gen => {
                    // *: 生成器构建块 → List<yield 类型>
                    // 从 body 中第一个 yield 语句推导元素类型
                    let elem_ty = body.iter().find_map(|s| {
                        if let AstStmt::Yield(Some(e)) = s {
                            Some(infer_expr_type(e, ctx))
                        } else { None }
                    }).unwrap_or(IrType::Any);
                    IrType::Named { path: "List".into(), args: vec![elem_ty] }
                }
                _ => lhs_ty,
            }
        }
        AstExpr::KwArg { .. } => IrType::Any,
        AstExpr::PathAccess { .. } => IrType::Any,
        AstExpr::SafeNav { .. } => IrType::Any,
        AstExpr::TryCatch { .. } => IrType::Any,
        AstExpr::Paren(inner) => infer_expr_type(inner, ctx),
    }
}

fn infer_stmt_type(stmt: &AstStmt, ctx: &TypeCtx) -> IrType {
    match stmt {
        AstStmt::Expr(e) => infer_expr_type(e, ctx),
        AstStmt::Pass => IrType::Unit,
        AstStmt::TypeAlias { .. } => IrType::Unit,
        AstStmt::Check { .. } => IrType::Unit,
        AstStmt::Let { ty, .. } => ty.as_ref().map(|t| from_ast_type(t)).unwrap_or(IrType::Any),
        AstStmt::Return(Some(e)) => infer_expr_type(e, ctx),
        AstStmt::Return(None) => IrType::Unit,
        AstStmt::Yield(Some(e)) => IrType::Named { path: "Itor".into(), args: vec![infer_expr_type(e, ctx)] },
        AstStmt::Yield(None) => IrType::Unit,
        AstStmt::YieldFrom(e) => IrType::Named { path: "Itor".into(), args: vec![infer_expr_type(e, ctx)] },
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
        AstPattern::Int(n) => Some(Pattern::Lit(LitKind::Int(*n))),
        AstPattern::Str(s) => Some(Pattern::Lit(LitKind::Str(s.clone()))),
        AstPattern::Bool(b) => Some(Pattern::Lit(LitKind::Bool(*b))),
        AstPattern::Variant(name, args) => {
            let ir_args: Vec<Pattern> = args.iter()
                .filter_map(|a| convert_ast_pattern(a, ctx))
                .collect();
            // 区分 struct 解构 vs enum 变体模式
            if ctx.is_struct(name) {
                // struct 模式: Point(px, py) → Point { x: px, y: py }
                let field_names: Vec<String> = ctx.struct_fields.get(name)
                    .map(|fields| fields.keys().cloned().collect())
                    .unwrap_or_default();
                let fields: Vec<(String, Pattern)> = ir_args.into_iter().enumerate()
                    .map(|(i, pat)| {
                        let fname = field_names.get(i).cloned().unwrap_or_else(|| format!("field_{}", i));
                        (fname, pat)
                    })
                    .collect();
                return Some(Pattern::Struct { name: name.clone(), fields });
            }
            // enum 变体模式
            let (enum_name, variant) = if let Some(dot_pos) = name.rfind('.') {
                (name[..dot_pos].to_string(), name[dot_pos+1..].to_string())
            } else {
                let enum_name = ctx.enum_variants.get(name.as_str())
                    .cloned()
                    .unwrap_or_else(|| {
                        match name.as_str() {
                            "Some" | "None" => "Option".into(),
                            "Ok" | "Err" => "Result".into(),
                            _ => "Error".into(),
                        }
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
            let ir_elems: Vec<Pattern> = elems.iter()
                .filter_map(|e| convert_ast_pattern(e, ctx))
                .collect();
            Some(Pattern::Tuple(ir_elems))
        }
    }
}

// （arm_body_to_expr 已移除 — Match 表达式现在通过 BlockExpr + Stmt::Match 处理）

// ══════════════════════════════════════════════════════════════
// 核心转换函数
// ══════════════════════════════════════════════════════════════

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
        AstExpr::Paren(inner) => {
            ExprKind::Paren(Box::new(convert_expr(inner, ctx)))
        }

        AstExpr::Call { func, args, type_args } => {
            // 部分应用检测：如果 args 中包含 _ 占位符，展开为 Lambda
            // add(_, 1) → |x| add(x, 1)
            let has_wildcard = args.iter().any(|a| matches!(a, AstExpr::Ident(s) if s == "_"));
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
                            IrType::Any, Span::unknown(),
                        ));
                    } else {
                        filled_args.push(convert_expr(a, ctx));
                    }
                }
                let callee = convert_expr(func, ctx);
                let call = Expr::new(ExprKind::Call {
                    type_args: vec![],
                    callee: Box::new(callee),
                    args: filled_args,
                }, IrType::Any, Span::unknown());
                return Expr::new(ExprKind::Lambda {
                    params: lambda_params,
                    body: Box::new(call),
                    is_move: true,
                }, IrType::Fn {
                    params: vec![IrType::Any; param_idx as usize],
                    ret: Box::new(IrType::Any),
                }, Span::unknown());
            }
            // 特殊处理 __as__ 运算符：__as__(value, type_name) → Cast
            if let AstExpr::Ident(ref fname) = func.as_ref() {
                if fname == "__as__" && args.len() == 2 {
                    let value = convert_expr(&args[0], ctx);
                    if let AstExpr::Ident(ref type_name) = &args[1] {
                        let target = name_to_ir_type(type_name);
                        let target_ty = target.clone();
                    return Expr::new(ExprKind::Cast {
                            expr: Box::new(value),
                            target,
                        }, target_ty, Span::unknown());
                    }
                }
            }
            // 处理 func[type_arg](args) → 泛型调用: func::<type_arg>(args)
            let actual_func: &AstExpr;
            let extra_type_args: Vec<String>;
            if let AstExpr::Index { receiver, index } = func.as_ref() {
                if let AstExpr::Ident(ref type_name) = index.as_ref() {
                    actual_func = receiver;
                    extra_type_args = vec![type_name.clone()];
                } else {
                    actual_func = func;
                    extra_type_args = vec![];
                }
            } else {
                actual_func = func;
                extra_type_args = vec![];
            }
            let mut ir_type_args: Vec<String> = type_args.iter().map(|t| {
                match t.as_str() {
                    "int" => "i64".to_string(),
                    "str" => "String".to_string(),
                    "f64" | "float" => "f64".to_string(),
                    "bool" => "bool".to_string(),
                    other => other.to_string(),
                }
            }).collect();
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
                        return Expr::new(ExprKind::MethodCall {
                            receiver: Box::new(recv),
                            method: "__call__".to_string(),
                            args: args.iter().map(|a| convert_expr(a, ctx)).collect(),
                        }, ret_ty, Span::unknown());
                    }
                }
            }

            ExprKind::Call { type_args: ir_type_args,
                callee: Box::new(convert_expr(actual_func, ctx)),
                args: args.iter().map(|a| convert_expr(a, ctx)).collect(),
            }
        }

        AstExpr::MethodCall { receiver, method, args } => {
            ExprKind::MethodCall {
                receiver: Box::new(convert_expr(receiver, ctx)),
                method: method.clone(),
                args: args.iter().map(|a| convert_expr(a, ctx)).collect(),
            }
        }

        AstExpr::FieldAccess { receiver, field } => {
            ExprKind::FieldAccess {
                base: Box::new(convert_expr(receiver, ctx)),
                field: field.clone(),
            }
        }

        AstExpr::Index { receiver, index } => {
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
                    return Expr::new(ExprKind::Lit(LitKind::Bool(result)), IrType::Bool, Span::unknown());
                }
                // RHS is not a simple type name → fallback to false
                return Expr::new(ExprKind::Lit(LitKind::Bool(false)), IrType::Bool, Span::unknown());
            }

            // 用户自定义类型的运算符 → 魔术方法调用（如 Vector + Vector → Vector.__add__）
            let left_ty_bin = infer_expr_type(left, ctx);
            if let Some(magic) = magic_method_for_binop(op) {
                let is_user_struct = match &left_ty_bin {
                    IrType::Named { path, .. } => ctx.struct_methods.get(path)
                        .map(|ms| ms.contains(magic))
                        .unwrap_or(false),
                    _ => false,
                };
                if is_user_struct {
                    let recv = convert_expr(left, ctx);
                    let ret_ty = infer_expr_type(left, ctx);
                    let expr = Expr::new(ExprKind::MethodCall {
                        receiver: Box::new(recv),
                        method: magic.to_string(),
                        args: vec![convert_expr(right, ctx)],
                    }, ret_ty, Span::unknown());
                    return expr;
                }
            }

            let ir_op = map_binop(op);
            
            // 泛型调用检测: ident < Type > (args) — 不是比较，而是泛型实例化
            // 支持单类型参数 ident < T > 和多类型参数 ident < T, U >
            if matches!(ir_op, BinOpKind::Gt) {
                if let AstExpr::Binary { left: inner_left, op: BinOp::Lt, right: inner_right } = left.as_ref() {
                    if let AstExpr::Ident(fname) = inner_left.as_ref() {
                        if let Some(type_names) = extract_type_names(inner_right) {
                            if let AstExpr::Call { func: call_func, args: call_args, .. } = right.as_ref() {
                                if let AstExpr::Ident(call_fname) = call_func.as_ref() {
                                    if call_fname == fname {
                                        // 这是泛型调用: f < T, U > (args)
                                        let ir_callee = convert_expr(inner_left, ctx);
                                        let ir_args: Vec<Expr> = call_args.iter().map(|a| convert_expr(a, ctx)).collect();
                                        let ir_type_args = map_type_args(&type_names);
                                        return Expr::new(
                                            ExprKind::Call { callee: Box::new(ir_callee), args: ir_args, type_args: ir_type_args },
                                            IrType::Any, Span::unknown(),
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
                            let ir_args: Vec<Expr> = call_args.iter().map(|a| convert_expr(a, ctx)).collect();
                            let ir_type_args = map_type_args(&type_names);
                            let ret_ty = ctx.lookup_fn_return(&fname);
                            return Expr::new(
                                ExprKind::Call { callee: Box::new(ir_callee), args: ir_args, type_args: ir_type_args },
                                ret_ty, Span::unknown(),
                            );
                        }
                    }
                }
            }
            
            // 链式比较展开: 1 < x < 10 → (1 < x) && (x < 10)
            if matches!(ir_op, BinOpKind::Lt | BinOpKind::Gt | BinOpKind::Le | BinOpKind::Ge | BinOpKind::Eq | BinOpKind::Neq) {
                if let AstExpr::Binary { left: inner_left, op: inner_op, right: inner_right } = left.as_ref() {
                    let inner_ir_op = map_binop(inner_op);
                    if matches!(inner_ir_op, BinOpKind::Lt | BinOpKind::Gt | BinOpKind::Le | BinOpKind::Ge | BinOpKind::Eq | BinOpKind::Neq) {
                        // (a cmp1 b) cmp2 c → (a cmp1 b) && (b cmp2 c)
                        let a = convert_expr(inner_left, ctx);
                        let b = convert_expr(inner_right, ctx);
                        let c = convert_expr(right, ctx);
                        return Expr::new(
                            ExprKind::BinOp {
                                op: BinOpKind::And,
                                lhs: Box::new(Expr::new(
                                    ExprKind::BinOp { op: inner_ir_op, lhs: Box::new(a), rhs: Box::new(b.clone()) },
                                    IrType::Bool, Span::unknown(),
                                )),
                                rhs: Box::new(Expr::new(
                                    ExprKind::BinOp { op: ir_op, lhs: Box::new(b), rhs: Box::new(c) },
                                    IrType::Bool, Span::unknown(),
                                )),
                            },
                            IrType::Bool, Span::unknown(),
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

        AstExpr::Unary { op, operand } => {
            ExprKind::UnOp {
                op: map_unop(op),
                operand: Box::new(convert_expr(operand, ctx)),
            }
        }

        AstExpr::If { cond, then_body, elif_clauses, else_body } => {
            // 多分支 if → 嵌套 IfExpr
            let mut result = if let Some(els) = else_body {
                ExprKind::IfExpr {
                    cond: Box::new(convert_expr(cond, ctx)),
                    then: Box::new(block_to_expr(then_body, ctx)),
                    els: Box::new(block_to_expr(els, ctx)),
                }
            } else {
                ExprKind::IfExpr {
                    cond: Box::new(convert_expr(cond, ctx)),
                    then: Box::new(block_to_expr(then_body, ctx)),
                    els: Box::new(Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown())),
                }
            };

            // elif 链 = 嵌套 if（反向迭代，使第一个 elif 成为最外层 if）
            for (elif_cond, elif_body) in elif_clauses.iter().rev() {
                result = ExprKind::IfExpr {
                    cond: Box::new(convert_expr(elif_cond, ctx)),
                    then: Box::new(block_to_expr(elif_body, ctx)),
                    els: Box::new(Expr::new(result, ty.clone(), Span::unknown())),
                };
            }
            result
        }

        AstExpr::Match { expr, arms } => {
            // Match 表达式 → 包装为 BlockExpr 内含 Match 语句
            // （保留模式匹配和变量绑定，if-else 降级会丢失这些信息）
            let ir_scrutinee = convert_expr(expr, ctx);
            let mut arm_ctx = TypeCtx::new();
            arm_ctx.current_generics = ctx.current_generics.clone();
            arm_ctx.current_ret_ty = ctx.current_ret_ty.clone();
            
            let ir_arms: Vec<MatchArm> = arms.iter().map(|arm| {
                let pat = convert_ast_pattern(&arm.pattern, ctx)
                    .unwrap_or(Pattern::Wildcard);
                let guard = arm.guard.as_ref().map(|g| convert_expr(g, ctx));
                let mut body_ctx = TypeCtx::new();
                body_ctx.current_generics = ctx.current_generics.clone();
                body_ctx.current_ret_ty = ctx.current_ret_ty.clone();
                // 从模式中提取绑定变量名并添加到上下文
                fn collect_pattern_vars(pat: &AstPattern, vars: &mut Vec<String>) {
                    match pat {
                        AstPattern::Ident(name) => vars.push(name.clone()),
                        AstPattern::Variant(_, args) => {
                            for a in args { collect_pattern_vars(a, vars); }
                        }
                        AstPattern::Tuple(elems) => {
                            for e in elems { collect_pattern_vars(e, vars); }
                        }
                        _ => {}
                    }
                }
                let mut bound_vars = Vec::new();
                collect_pattern_vars(&arm.pattern, &mut bound_vars);
                let scrut_ty = infer_expr_type(expr, ctx);
                for v in &bound_vars {
                    body_ctx.add_var(v, scrut_ty.clone());
                }
                let body = convert_block_with_ctx(&arm.body, &body_ctx);
                MatchArm { pattern: pat, guard, body }
            }).collect();
            
            let match_stmt = Stmt::Match { scrutinee: ir_scrutinee, arms: ir_arms };
            let blk_ty = arms.first()
                .and_then(|arm| arm.body.last())
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit);
            ExprKind::BlockExpr {
                block: Block { stmts: vec![match_stmt], ty: blk_ty },
            }
        }

        AstExpr::Closure { params, body } => {
            ExprKind::Lambda {
                params: params.iter().map(|name| Param {
                    name: name.clone(),
                    ty: IrType::Any,
                    is_mut: false,
                    is_ref: false,
                    is_owned: false,
                    default: None,
                    variadic: false,
                }).collect(),
                body: Box::new(convert_expr(body, ctx)),
                is_move: true,
            }
        }

        AstExpr::Range { start, end, inclusive } => {
            // Range → StructCtor { name: "Range", fields: [start, end, inclusive] }
            let mut fields = Vec::new();
            if let Some(s) = start {
                fields.push(("start".into(), convert_expr(s, ctx)));
            }
            if let Some(e) = end {
                fields.push(("end".into(), convert_expr(e, ctx)));
            }
            if *inclusive {
                fields.push(("inclusive".into(), Expr::new(
                    ExprKind::Lit(LitKind::Bool(true)), IrType::Bool, Span::unknown()
                )));
            }
            ExprKind::StructCtor { name: "Range".into(), fields }
        }

        AstExpr::Walrus { target, value } => {
            // := → 展开为 let + 返回；在表达式层面转为复合
            if let AstExpr::Ident(name) = target.as_ref() {
                let inner_ctx = ctx;
                let val = convert_expr(value, &inner_ctx);
                // inner_ctx.add_var(name, val_ty); // FIXME: scope issue
                ExprKind::StructCtor {
                    name: "_Walrus".into(),
                    fields: vec![
                        ("_bind".into(), Expr::new(ExprKind::Var(name.clone()), val.ty.clone(), Span::unknown())),
                        ("_val".into(), val),
                    ],
                }
            } else {
                convert_expr(value, ctx).kind
            }
        }

        AstExpr::Pipe { receiver, func, args } => {
            // |> → 函数调用（receiver 作为第一个参数）
            let mut all_args = vec![convert_expr(receiver, ctx)];
            all_args.extend(args.iter().map(|a| convert_expr(a, ctx)));
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var(func.clone()), IrType::Any, Span::unknown())),
                args: all_args,
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
                ExprKind::FieldAccess { base: Box::new(recv), field: field.clone() }
            } else {
                ExprKind::IfExpr {
                    cond: Box::new(Expr::new(
                        ExprKind::BinOp {
                            op: BinOpKind::Eq,
                            lhs: Box::new(recv.clone()),
                            rhs: Box::new(Expr::new(ExprKind::Lit(LitKind::None_), IrType::Any, Span::unknown())),
                        },
                        IrType::Bool,
                        Span::unknown(),
                    )),
                    then: Box::new(Expr::new(ExprKind::Lit(LitKind::None_), IrType::Any, Span::unknown())),
                    els: Box::new(Expr::new(
                        ExprKind::FieldAccess { base: Box::new(recv), field: field.clone() },
                        IrType::Any, Span::unknown(),
                    )),
                }
            }
        }

        AstExpr::Try(inner) => {
            // try expr (? 操作符): 对 Result/Option 类型做错误传播，否则透传
            let inner_ty = infer_expr_type(inner, ctx);
            let result_like = matches!(&inner_ty,
                IrType::Result { .. } | IrType::Option(_)
            ) || matches!(&inner_ty,
                IrType::Named { path, .. } if path == "Result" || path == "Option"
            );
            if result_like {
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
            ExprKind::StructCtor { name: "Dict".into(), fields }
        }

        AstExpr::SetLit(items) => {
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var("set!".into()), IrType::Any, Span::unknown())),
                args: items.iter().map(|i| convert_expr(i, ctx)).collect(),
            }
        }

        AstExpr::TupleLit(elems) => {
            ExprKind::TupleLit(elems.iter().map(|e| convert_expr(e, ctx)).collect())
        }

        AstExpr::ListComprehension { output, var, iter, cond } => {
            // [out for x in iter if cond] → 展开为生成模式
            let iter_expr = convert_expr(iter, ctx);
            let out_expr = convert_expr(output, ctx);
            let mut args = vec![
                Expr::new(ExprKind::Lambda {
                    params: vec![Param { name: var.clone(), ty: IrType::Any, is_mut: false, is_ref: false, is_owned: false, default: None, variadic: false }],
                    body: Box::new(out_expr),
                    is_move: true,
                }, IrType::Any, Span::unknown()),
                iter_expr,
            ];
            // 过滤条件 cond 作为第三个参数传入 (可选)
            if let Some(c) = cond {
                args.push(Expr::new(ExprKind::Lambda {
                    params: vec![Param { name: var.clone(), ty: IrType::Any, is_mut: false, is_ref: false, is_owned: false, default: None, variadic: false }],
                    body: Box::new(convert_expr(c, ctx)),
                    is_move: true,
                }, IrType::Any, Span::unknown()));
            }
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var("comp!".into()), IrType::Any, Span::unknown())),
                args,
            }
        }

        AstExpr::DictComprehension { key, value, var, iter, cond } => {
            // {k: v for x in iter} → 展开为生成模式
            let iter_expr = convert_expr(iter, ctx);
            let key_expr = convert_expr(key, ctx);
            let val_expr = convert_expr(value, ctx);
            let mut args = vec![
                Expr::new(ExprKind::Lambda {
                    params: vec![Param { name: var.clone(), ty: IrType::Any, is_mut: false, is_ref: false, is_owned: false, default: None, variadic: false }],
                    body: Box::new(Expr::new(
                        ExprKind::TupleLit(vec![key_expr, val_expr]),
                        IrType::Any, Span::unknown(),
                    )),
                    is_move: true,
                }, IrType::Any, Span::unknown()),
                iter_expr,
            ];
            if let Some(c) = cond {
                args.push(Expr::new(ExprKind::Lambda {
                    params: vec![Param { name: var.clone(), ty: IrType::Any, is_mut: false, is_ref: false, is_owned: false, default: None, variadic: false }],
                    body: Box::new(convert_expr(c, ctx)),
                    is_move: true,
                }, IrType::Any, Span::unknown()));
            }
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var("dict_comp!".into()), IrType::Any, Span::unknown())),
                args,
            }
        }

        AstExpr::SetComprehension { elem, var, iter, cond } => {
            // {x for x in iter} → 展开为生成模式
            let iter_expr = convert_expr(iter, ctx);
            let elem_expr = convert_expr(elem, ctx);
            let mut args = vec![
                Expr::new(ExprKind::Lambda {
                    params: vec![Param { name: var.clone(), ty: IrType::Any, is_mut: false, is_ref: false, is_owned: false, default: None, variadic: false }],
                    body: Box::new(elem_expr),
                    is_move: true,
                }, IrType::Any, Span::unknown()),
                iter_expr,
            ];
            if let Some(c) = cond {
                args.push(Expr::new(ExprKind::Lambda {
                    params: vec![Param { name: var.clone(), ty: IrType::Any, is_mut: false, is_ref: false, is_owned: false, default: None, variadic: false }],
                    body: Box::new(convert_expr(c, ctx)),
                    is_move: true,
                }, IrType::Any, Span::unknown()));
            }
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var("set_comp!".into()), IrType::Any, Span::unknown())),
                args,
            }
        }

        AstExpr::Assign { target, op, value } => {
            // 复合赋值 → 仍在 Expr 层表达（Assign 语义，由后端处理）
            ExprKind::BinOp {
                op: map_assign_op(op),
                lhs: Box::new(convert_expr(target, ctx)),
                rhs: Box::new(convert_expr(value, ctx)),
            }
        }

        AstExpr::Spawn(inner) => {
            // go expr → 并行线程：thread::spawn(move || { expr })
            // 与显式 spawn(expr)（异步任务）区分，使用内部标记 __go
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var("__go".into()), IrType::Any, Span::unknown())),
                args: vec![convert_expr(inner, ctx)],
            }
        }

        AstExpr::Move(inner) => {
            convert_expr(inner, ctx).kind  // move 语义在 IR 中由所有权表达，暂透传
        }

        AstExpr::Panic(inner) => {
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var("panic!".into()), IrType::Any, Span::unknown())),
                args: vec![convert_expr(inner, ctx)],
            }
        }

        AstExpr::Await(inner) => {
            ExprKind::MethodCall {
                receiver: Box::new(convert_expr(inner, ctx)),
                method: "await".into(),
                args: vec![],
            }
        }

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
                    let args: Vec<Expr> = if let IrType::Tuple(elements) = block_ty {
                        // 元组解包：为每个元素生成一个 packed 字段访问
                        elements.iter().enumerate().map(|(i, _elem)| {
                            Expr::new(
                                ExprKind::MagicCall {
                                    kind: MagicKind::UnpackBuildCall,
                                    args: vec![packed.clone(), Expr::new(
                                        ExprKind::Lit(LitKind::Int(i as i64)),
                                        IrType::Int, Span::unknown(),
                                    )],
                                },
                                IrType::Any, Span::unknown(),
                            )
                        }).collect()
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
                    // *: → 生成器表达式，将 body 转换为迭代器
                    let body_block = convert_block_with_ctx(body, ctx);
                    ExprKind::BlockExpr { block: body_block }
                }
                BuildKind::Index => {
                    // ^: → IndexGet。key = body 块中的最后一个表达式值。
                    // 语法：container ^: <key>（冒号后换行缩进，块体为单值 key）
                    let key_expr = body.last().and_then(|s| match s {
                        AstStmt::Expr(e) => Some(convert_expr(e, ctx)),
                        _ => None,
                    }).unwrap_or_else(|| {
                        // 回退：若无单个尾部表达式，用整个块（BlockExpr）
                        let blk = convert_block_with_ctx(body, ctx);
                        Expr::new(ExprKind::BlockExpr { block: blk }, IrType::Any, Span::unknown())
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
                    ("name".into(), Expr::new(ExprKind::Lit(LitKind::Str(name.clone())), IrType::Str, Span::unknown())),
                    ("value".into(), convert_expr(value, ctx)),
                ],
            }
        }

        AstExpr::TryCatch { body, catches, else_body, finally_body } => {
            // 构建 Stmt::TryCatch 结构以供 codegen 层正确处理
            let body_block = convert_block(body, ctx);
            let ir_catches: Vec<(Option<Pattern>, Block)> = catches.iter().map(|c| {
                let pat = convert_ast_pattern(&c.pattern, ctx);
                let block = convert_block(&c.body, ctx);
                (pat, block)
            }).collect();
            let ir_else = else_body.as_ref().map(|b| convert_block(b, ctx));
            let ir_finally = finally_body.as_ref().map(|b| convert_block(b, ctx));

            // 返回一个 TryCatch 包装块（codegen 会生成 catch_unwind 等逻辑）
            ExprKind::BlockExpr {
                block: Block {
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
    let blk_ty = stmts.last()
        .map(|s| infer_stmt_type(s, ctx))
        .unwrap_or(IrType::Unit);
    Expr::new(
        ExprKind::BlockExpr { block: Block { stmts: ir_stmts, ty: blk_ty.clone() } },
        blk_ty, Span::unknown(),
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
            });
            for (i, name) in names.iter().enumerate() {
                if name == "_" { continue; }
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
                result.push(Stmt::Let {
                    name: name.clone(),
                    ty: ty.as_ref().map(|t| from_ast_type(t)).unwrap_or(IrType::Any),
                    value: field_expr,
                    is_mut: false,
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
            let ir_arms: Vec<MatchArm> = arms.iter().map(|arm| {
                let pat = convert_ast_pattern(&arm.pattern, ctx)
                    .unwrap_or(Pattern::Wildcard);
                let guard = arm.guard.as_ref().map(|g| convert_expr(g, ctx));
                let mut arm_ctx = TypeCtx::new();
                arm_ctx.current_generics = ctx.current_generics.clone();
                arm_ctx.current_ret_ty = ctx.current_ret_ty.clone();
                // 复制 enum_variants 以便模式匹配能正确解析枚举类型
                for (vn, en) in &ctx.enum_variants {
                    arm_ctx.enum_variants.insert(vn.clone(), en.clone());
                }
                for (cn, ct) in &ctx.top_level_consts { arm_ctx.top_level_consts.insert(cn.clone(), ct.clone()); }
                // 也复制 struct 信息用于模式匹配
                for sn in &ctx.struct_names {
                    arm_ctx.struct_names.insert(sn.clone());
                }
                if let AstPattern::Ident(name) = &arm.pattern {
                    let scrut_ty = infer_expr_type(expr, ctx);
                    arm_ctx.add_var(name, scrut_ty);
                }
                let body = convert_block_with_ctx(&arm.body, &arm_ctx);
                MatchArm { pattern: pat, guard, body }
            }).collect();
            Stmt::Match { scrutinee: ir_scrutinee, arms: ir_arms }
        }
        AstStmt::Expr(e) => {
            // 构建块（=:/~:）在表达式位置出现时，需要特殊处理
            // =: 构建块作为表达式语句：生成立即调用闭包作为表达式
            if let AstExpr::BuildBlock { kind: BuildKind::Var, lhs, body } = e {
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
                return Stmt::Let {
                    name: lhs_name,
                    ty: IrType::Any,
                    value: init_expr,
                    is_mut: false,
                };
            }
            Stmt::ExprStmt { expr: convert_expr(e, ctx) }
        }

        AstStmt::Pass => Stmt::Pass,

        AstStmt::TypeAlias { name, ty } => Stmt::TypeAlias {
            name: name.clone(),
            ty: from_ast_type(ty),
        },

        AstStmt::Let { name, mutable, ty, value, .. } => {
            let ir_ty = ty.as_ref().map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, ctx));
            let mut ir_value = convert_expr(value, ctx);
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
            if let IrType::Fn { params: fn_params, .. } = &ir_ty {
                if let ExprKind::Lambda { params: lambda_params, .. } = &mut ir_value.kind {
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
                is_mut: *mutable,
            }
        }

        AstStmt::Const { name, ty, value } => {
            let ir_ty = ty.as_ref().map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, ctx));
            let mut ir_value = convert_expr(value, ctx);
            // 当 value 是 Lambda 时，使用 Lambda 的类型
            // 当 value 的 IR 类型为 Any（~: 构建块等），也使用 IR 类型避免错误标注
            let ir_ty = match &ir_value.ty {
                IrType::Fn { .. } | IrType::Any => ir_value.ty.clone(),
                _ => ir_ty,
            };
            // 同上：传播 fn 参数类型到 Lambda
            if let IrType::Fn { params: fn_params, .. } = &ir_ty {
                if let ExprKind::Lambda { params: lambda_params, .. } = &mut ir_value.kind {
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
            }
        }

        AstStmt::Return(val) => Stmt::Return {
            value: val.as_ref().map(|v| convert_expr(v, ctx)),
        },

        AstStmt::Yield(val) => {
            let value = match val {
                Some(expr) => convert_expr(expr, ctx),
                None => Expr::new(ExprKind::Lit(LitKind::None_), IrType::Unit, Span::unknown()),
            };
            Stmt::Yield { value }
        },

        AstStmt::YieldFrom(e) => {
            Stmt::YieldFrom { iter: convert_expr(e, ctx) }
        },

        AstStmt::While { cond, guard, body, .. } => Stmt::While {
            cond: convert_expr(cond, ctx),
            guard: guard.as_ref().map(|g| convert_expr(g, ctx)),
            body: convert_block(body, ctx),
        },

        AstStmt::For { var, iter, guard, body, .. } => {
            let mut loop_ctx = TypeCtx::new();
            // 从 ctx 复制函数泛型上下文
            loop_ctx.current_generics = ctx.current_generics.clone();
            loop_ctx.current_ret_ty = ctx.current_ret_ty.clone();
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
            }
        }

        AstStmt::Loop(body) => Stmt::While {
            cond: Expr::new(ExprKind::Lit(LitKind::Bool(true)), IrType::Bool, Span::unknown()),
            guard: None,
            body: convert_block(body, ctx),
        },

        AstStmt::Break(_) => Stmt::Break,
        AstStmt::Continue => Stmt::Continue,

        AstStmt::Defer(body) => {
            // defer → 展开为 Block（不追加 return，让后续代码继续执行）
            let stmts = convert_block(body, ctx).stmts;
            Stmt::Block { stmts }
        },

        AstStmt::Comptime { body } => {
            // comptime: 块 — 内联为普通 Block
            Stmt::Block { stmts: convert_block(body, ctx).stmts }
        },

        AstStmt::Raise(e) => Stmt::ExprStmt {
            expr: Expr::new(
                ExprKind::Call { type_args: vec![],
                    callee: Box::new(Expr::new(ExprKind::Var("panic!".into()), IrType::Any, Span::unknown())),
                    args: vec![convert_expr(e, ctx)],
                },
                IrType::Never, Span::unknown(),
            ),
        },

        AstStmt::Guard { cond, let_binding, else_body } => {
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
                                rhs: Box::new(Expr::new(ExprKind::Lit(LitKind::None_), IrType::Any, Span::unknown())),
                            },
                            IrType::Bool, Span::unknown(),
                        ),
                        then_branch: Block { stmts: vec![], ty: IrType::Unit },
                        else_branch: Some(convert_block(else_body, &guard_ctx)),
                    }
                } else {
                    Stmt::Block { stmts: else_body.iter().map(|s| convert_stmt(s, ctx)).collect() }
                }
            } else {
                Stmt::If {
                    cond: cond.as_ref().map(|c| convert_expr(c, ctx))
                        .unwrap_or(Expr::new(ExprKind::Lit(LitKind::Bool(true)), IrType::Bool, Span::unknown())),
                    then_branch: Block { stmts: vec![], ty: IrType::Unit },
                    else_branch: Some(convert_block(else_body, ctx)),
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
            let mut stmts = vec![
                Stmt::Let { name: name.clone(), ty: val_ty.clone(), value: val, is_mut: false },
            ];
            if alias.is_some() {
                stmts.push(
                    Stmt::ExprStmt {
                        expr: Expr::new(
                            ExprKind::MethodCall {
                                receiver: Box::new(Expr::new(ExprKind::Var(name), val_ty.clone(), Span::unknown())),
                                method: "__exit__".into(),
                                args: vec![],
                            },
                            IrType::Unit, Span::unknown(),
                        )
                    }
                );
            }
            stmts.extend(body.iter().map(|s| convert_stmt(s, &with_ctx)));
            Stmt::Block { stmts }
        }

        AstStmt::Assign { target, op, value } => {
            let val = convert_expr(value, ctx);
            let target_expr = convert_expr(target, ctx);
            match op {
                crate::ast::AssignOp::Eq => Stmt::Assign { target: target_expr, value: val },
                _ => Stmt::Assign {
                    target: target_expr.clone(),
                    value: Expr::new(
                        ExprKind::BinOp { op: map_assign_op(op), lhs: Box::new(target_expr), rhs: Box::new(val) },
                        IrType::Any, Span::unknown(),
                    ),
                },
            }
        }

        AstStmt::FnDef { func } => {
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

        AstStmt::Test { name: _, body } => Stmt::Block {
            stmts: body.iter().map(|s| convert_stmt(s, ctx)).collect(),
        },

        AstStmt::Assert { expr, expected: _ } => Stmt::ExprStmt {
            expr: Expr::new(
                ExprKind::Call { type_args: vec![],
                    callee: Box::new(Expr::new(ExprKind::Var("assert!".into()), IrType::Any, Span::unknown())),
                    args: vec![convert_expr(expr, ctx)],
                },
                IrType::Unit, Span::unknown(),
            ),
        },

        AstStmt::Check { expr, message: _ } => {
            // check → 展开为 if !expr { eprintln!(...) }
            let cond = Expr::new(
                ExprKind::UnOp {
                    op: crate::ir::node::UnOpKind::Not,
                    operand: Box::new(convert_expr(expr, ctx)),
                },
                IrType::Bool, Span::unknown(),
            );
            let print_call = Expr::new(
                ExprKind::Call { type_args: vec![],
                    callee: Box::new(Expr::new(ExprKind::Var("eprintln!".into()), IrType::Any, Span::unknown())),
                    args: vec![Expr::new(
                        ExprKind::Lit(LitKind::Str("CHECK failed".into())),
                        IrType::Str, Span::unknown(),
                    )],
                },
                IrType::Unit, Span::unknown(),
            );
            Stmt::If {
                cond,
                then_branch: Block { stmts: vec![Stmt::ExprStmt { expr: print_call }], ty: IrType::Unit },
                else_branch: None,
            }
        },

        AstStmt::LetTuple { .. } => {
            // LetTuple 在 convert_stmts 中展开，不应到达此处
            Stmt::Pass
        }

        AstStmt::Suite { name: _, setup, teardown, tests } => {
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
                        ir_tests.push(AstStmt::Test { name: name.clone(), body: combined });
                    }
                    _ => ir_tests.push(t.clone()),
                }
            }
            Stmt::Block {
                stmts: convert_stmts(&ir_tests, ctx),
            }
        },
    }
}

fn convert_block(stmts: &[AstStmt], ctx: &TypeCtx) -> Block {
    // 创建可变的本地上下文，支持 Let 变量传播
    let mut block_ctx = TypeCtx::new();
    block_ctx.current_generics = ctx.current_generics.clone();
    block_ctx.current_ret_ty = ctx.current_ret_ty.clone();
    block_ctx.current_fn_name = ctx.current_fn_name.clone();
    block_ctx.pending_items = ctx.pending_items.clone();
    for sn in &ctx.struct_names { block_ctx.struct_names.insert(sn.clone()); }
    for (sn, fields) in &ctx.struct_fields {
        let mut cloned = HashMap::new();
        for (fn_, ty) in fields { cloned.insert(fn_.clone(), ty.clone()); }
        block_ctx.struct_fields.insert(sn.clone(), cloned);
    }
    for (sn, ms) in &ctx.struct_methods {
        block_ctx.struct_methods.insert(sn.clone(), ms.clone());
    }
    for (vn, vt) in &ctx.vars { block_ctx.vars.insert(vn.clone(), vt.clone()); }
    for (name, ty) in &ctx.fn_returns { block_ctx.fn_returns.insert(name.clone(), ty.clone()); }
    for (name, p) in &ctx.fn_params { block_ctx.fn_params.insert(name.clone(), p.clone()); }
    for (vn, en) in &ctx.enum_variants { block_ctx.enum_variants.insert(vn.clone(), en.clone()); }
    for (cn, ct) in &ctx.top_level_consts { block_ctx.top_level_consts.insert(cn.clone(), ct.clone()); }
    
    let mut ir_stmts: Vec<Stmt> = Vec::new();
    for s in stmts {
        // 前向传播：Let 语句的变量添加到后续语句的上下文
        if let AstStmt::Let { name, ty, value, .. } = s {
            let ir_ty = ty.as_ref().map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, &block_ctx));
            block_ctx.add_var(name, ir_ty);
        }
        if let AstStmt::LetTuple { names, ty, .. } = s {
            let ir_ty = ty.as_ref().map(|t| from_ast_type(t)).unwrap_or(IrType::Any);
            for name in names {
                if name != "_" {
                    block_ctx.add_var(name, ir_ty.clone());
                }
            }
        }
        // guard let <Variant>(...) = expr else: ... → match value { Variant(r) => { 剩余语句 }, _ => { else_body } }
        if let AstStmt::Guard { let_binding, else_body, .. } = s {
            if let Some((pat, value)) = let_binding {
                if let AstPattern::Variant(_, args) = pat {
                    let val = convert_expr(value, &block_ctx);
                    // 剩余语句作为匹配分支的 body（guard 之后代码仅在匹配时执行）
                    let rest: Vec<AstStmt> = stmts.iter().skip_while(|x| !std::ptr::eq(*x, s)).skip(1).cloned().collect();
                    // 匹配分支上下文：绑定模式变量（类型 Any 占位，body 转换时按需解析）
                    let mut then_ctx = block_ctx.clone();
                    fn collect_pat_vars(pat: &AstPattern, ctx: &mut TypeCtx) {
                        match pat {
                            AstPattern::Ident(name) => { ctx.add_var(name, IrType::Any); }
                            AstPattern::Variant(_, as_) => { for a in as_ { collect_pat_vars(a, ctx); } }
                            _ => {}
                        }
                    }
                    collect_pat_vars(&AstPattern::Variant(String::new(), args.clone()), &mut then_ctx);
                    let then_block = convert_block(&rest, &then_ctx);
                    let else_block = convert_block(else_body, &block_ctx);
                    // 构建 match 结构：Variant(...) 分支 + 默认分支
                    let ir_pat = convert_ast_pattern(pat, &block_ctx)
                        .unwrap_or(Pattern::Wildcard);
                    let match_stmt = Stmt::Match {
                        scrutinee: val,
                        arms: vec![
                            MatchArm { pattern: ir_pat, guard: None, body: then_block },
                            MatchArm { pattern: Pattern::Wildcard, guard: None, body: else_block },
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
                    if let ExprKind::Var(name) = &target.kind { set.insert(name.clone()); }
                }
                Stmt::Let { name, is_mut, .. } if *is_mut => { set.insert(name.clone()); }
                Stmt::If { then_branch, else_branch, .. } => {
                    set.extend(collect_reassigned(&then_branch.stmts));
                    if let Some(eb) = else_branch { set.extend(collect_reassigned(&eb.stmts)); }
                }
                Stmt::While { body, .. } | Stmt::For { body, .. } => {
                    set.extend(collect_reassigned(&body.stmts));
                }
                Stmt::Match { arms, .. } => {
                    for arm in arms { set.extend(collect_reassigned(&arm.body.stmts)); }
                }
                Stmt::TryCatch { body, catches, else_body, finally_body } => {
                    set.extend(collect_reassigned(&body.stmts));
                    for (_, catch_body) in catches { set.extend(collect_reassigned(&catch_body.stmts)); }
                    if let Some(eb) = else_body { set.extend(collect_reassigned(&eb.stmts)); }
                    if let Some(fb) = finally_body { set.extend(collect_reassigned(&fb.stmts)); }
                }
                _ => {}
            }
        }
        set
    }

    let reassigned = collect_reassigned(&ir_stmts);
    if !reassigned.is_empty() {
        fn mark_mut(stmts: &mut [Stmt], reassigned: &std::collections::HashSet<String>) {
            for s in stmts {
                if let Stmt::Let { name, is_mut, .. } = s {
                    if reassigned.contains(name.as_str()) { *is_mut = true; }
                }
                match s {
                    Stmt::If { then_branch, else_branch, .. } => {
                        mark_mut(&mut then_branch.stmts, reassigned);
                        if let Some(eb) = else_branch { mark_mut(&mut eb.stmts, reassigned); }
                    }
                    Stmt::While { body, .. } | Stmt::For { body, .. } => {
                        mark_mut(&mut body.stmts, reassigned);
                    }
                    Stmt::Match { arms, .. } => {
                        for arm in arms { mark_mut(&mut arm.body.stmts, reassigned); }
                    }
                    _ => {}
                }
            }
        }
        mark_mut(&mut ir_stmts, &reassigned);
    }

    let ty = stmts.last()
        .map(|s| infer_stmt_type(s, ctx))
        .unwrap_or(IrType::Unit);
    Block { stmts: ir_stmts, ty }
}

fn convert_block_with_ctx(stmts: &[AstStmt], ctx: &TypeCtx) -> Block {
    convert_block(stmts, ctx)
}

// ══════════════════════════════════════════════════════════════
// 顶层 Item 转换
// ══════════════════════════════════════════════════════════════

fn convert_fn_def(func: &ast::Function, ctx: &TypeCtx) -> FnDef {
    let is_math = func.decorators.iter().any(|d| d.name == "math");
    let generics: Vec<String> = if is_math {
        vec!["T".to_string()]
    } else {
        func.generics.clone()
    };

    let params: Vec<Param> = func.params.iter().enumerate().map(|(_i, p)| {
        // 检测 variadic:
        // - VariadicMode::Single { dotdot_at }: .. 分隔仅位置/仅关键字参数，不改变参数类型
        // - VariadicMode::Double { first_at, second_at }: second_at 之后的参数是真正的 variadic 收集
        let is_variadic = match &func.variadic {
            ast::VariadicMode::Double { second_at, .. } => _i >= *second_at,
            _ => false,
        };
        Param {
            name: p.name.clone(),
            ty: if is_math { IrType::Generic("T".into()) } else { from_ast_type_with_generics(&p.ty, &generics) },
            is_mut: p.is_mut,
            is_ref: p.is_ref,
            is_owned: p.is_owned,
            default: p.default.as_ref().map(|d| convert_expr(d, ctx)),
            variadic: is_variadic,
        }
    }).collect();

    // 构建函数体上下文
    let mut fn_ctx = TypeCtx::new();
    fn_ctx.pending_items = ctx.pending_items.clone();
    fn_ctx.current_fn_name = Some(func.name.clone());
    fn_ctx.current_generics = generics.clone();
    // 复制全局 struct 信息
    for sn in &ctx.struct_names { fn_ctx.struct_names.insert(sn.clone()); }
    for (sn, fields) in &ctx.struct_fields {
        let mut cloned = HashMap::new();
        for (fn_, ty) in fields { cloned.insert(fn_.clone(), ty.clone()); }
        fn_ctx.struct_fields.insert(sn.clone(), cloned);
    }
    for (sn, ms) in &ctx.struct_methods {
        fn_ctx.struct_methods.insert(sn.clone(), ms.clone());
    }
    for (cn, ct) in &ctx.top_level_consts { fn_ctx.top_level_consts.insert(cn.clone(), ct.clone()); }
    for (vn, en) in &ctx.enum_variants { fn_ctx.enum_variants.insert(vn.clone(), en.clone()); }
    for (name, ty) in &ctx.fn_returns { fn_ctx.fn_returns.insert(name.clone(), ty.clone()); }
    for (name, p) in &ctx.fn_params { fn_ctx.fn_params.insert(name.clone(), p.clone()); }

    // 添加参数到作用域
    if is_math {
        for p in &func.params {
            fn_ctx.add_param(&p.name, IrType::Generic("T".into()));
        }
    } else {
        for p in &func.params {
            fn_ctx.add_param(&p.name, from_ast_type_with_generics(&p.ty, &generics));
        }
    }

    // 返回类型：优先 AST 注解，否则从函数体最后语句推断
    let ret_ty = func.return_type.as_ref()
        .map(|t| from_ast_type_with_generics(t, &generics))
        .unwrap_or_else(|| {
            func.body.last()
                .map(|stmt| infer_stmt_type(stmt, &fn_ctx))
                .unwrap_or(IrType::Unit)
        });
    fn_ctx.current_ret_ty = Some(ret_ty.clone());

    let body = convert_block(&func.body, &fn_ctx);

    let is_math = func.decorators.iter().any(|d| d.name == "math");
    let intrinsics: Vec<Intrinsic> = func.decorators.iter().map(|d| {
        let kind = match d.name.as_str() {
            "memoize" => IntrinsicKind::Memoize,
            "parallel" => IntrinsicKind::Parallel,
            "curry" => IntrinsicKind::Curry,
            "overload" => IntrinsicKind::Overload,
            "derive" => IntrinsicKind::Derive,
            "tail_call" => IntrinsicKind::TailCall,
            "math" => IntrinsicKind::Export(vec!["Math".into()]),
            name if name.starts_with("export") => {
                // @export(Rust, Python)
                let targets: Vec<String> = d.args.iter()
                    .filter_map(|a| {
                        if let AstExpr::Ident(n) = a { Some(n.clone()) }
                        else { None }
                    })
                    .collect();
                IntrinsicKind::Export(if targets.is_empty() { vec!["Rust".into()] } else { targets })
            }
            "init" => IntrinsicKind::Init,
            _ => return Intrinsic { kind: IntrinsicKind::Memoize, span: Span::unknown() }, // skip unknown
        };
        Intrinsic { kind, span: Span::unknown() }
    }).collect();

    FnDef {
        name: func.name.clone(),
        generics: if is_math && func.generics.is_empty() {
            // @math 自动泛型: 单泛型 T（所有参数统一类型）
            vec![GenericParam { name: "T".into(), bounds: vec![], default: None }]
        } else {
            // 从 where_clause 收集每个泛型参数的 bounds
            let mut bounds_map: HashMap<String, Vec<IrType>> = HashMap::new();
            for wb in &func.where_clause {
                let ir_bounds: Vec<IrType> = wb.bounds.iter()
                    .map(|b| from_ast_type(b))
                    .collect();
                bounds_map.entry(wb.type_param.clone())
                    .or_default()
                    .extend(ir_bounds);
            }
            func.generics.iter().map(|g| {
                let bounds = bounds_map.remove(g).unwrap_or_default();
                GenericParam {
                    name: g.clone(),
                    bounds,
                    default: None,
                }
            }).collect()
        },
        params,
        ret_ty,
        body,
        intrinsics,
        // 自动检测：如果函数体包含 await/spawn 且未显式标记 async，自动标记
        is_async: func.is_async || ast_body_has_async(&func.body),
        is_iterator: func.is_iterator,
        is_test: false,
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
        ast::Stmt::Let { value, .. } | ast::Stmt::Const { value, .. } => {
            ast_expr_has_async(value)
        }
        ast::Stmt::While { cond, body, guard, .. } => {
            ast_expr_has_async(cond)
                || guard.as_ref().map_or(false, |e| ast_expr_has_async(e))
                || ast_body_has_async(body)
        }
        ast::Stmt::For { iter, body, guard, .. } => {
            ast_expr_has_async(iter)
                || guard.as_ref().map_or(false, |e| ast_expr_has_async(e))
                || ast_body_has_async(body)
        }
        ast::Stmt::Loop(body) | ast::Stmt::Defer(body) => ast_body_has_async(body),
        ast::Stmt::Assign { target, value, .. } => {
            ast_expr_has_async(target) || ast_expr_has_async(value)
        }
        ast::Stmt::With { expr, body, .. } => {
            ast_expr_has_async(expr) || ast_body_has_async(body)
        }
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
        ast::Expr::If { cond, then_body, elif_clauses, else_body } => {
            ast_expr_has_async(cond)
                || ast_body_has_async(then_body)
                || elif_clauses.iter().any(|(c, b)| ast_expr_has_async(c) || ast_body_has_async(b))
                || else_body.as_ref().map_or(false, |b| ast_body_has_async(b))
        }
        ast::Expr::Match { expr, arms } => {
            ast_expr_has_async(expr)
                || arms.iter().any(|a| a.guard.as_ref().map_or(false, |g| ast_expr_has_async(g))
                    || ast_body_has_async(&a.body))
        }
        ast::Expr::Closure { body, .. } => ast_expr_has_async(body),
        ast::Expr::Pipe { receiver, args, .. } => {
            ast_expr_has_async(receiver) || args.iter().any(ast_expr_has_async)
        }
        ast::Expr::ListLit(es) | ast::Expr::TupleLit(es) | ast::Expr::SetLit(es) => {
            es.iter().any(ast_expr_has_async)
        }
        ast::Expr::DictLit(kvs) => {
            kvs.iter().any(|(k, v)| ast_expr_has_async(k) || ast_expr_has_async(v))
        }
        ast::Expr::SafeNav { receiver, .. } | ast::Expr::Try(receiver) => ast_expr_has_async(receiver),
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
        ast::Expr::ListComprehension { output, iter, cond, .. } => {
            ast_expr_has_async(output) || ast_expr_has_async(iter)
                || cond.as_ref().map_or(false, |e| ast_expr_has_async(e))
        }
        ast::Expr::DictComprehension { key, value, iter, cond, .. } => {
            ast_expr_has_async(key) || ast_expr_has_async(value)
                || ast_expr_has_async(iter)
                || cond.as_ref().map_or(false, |e| ast_expr_has_async(e))
        }
        ast::Expr::SetComprehension { elem, iter, cond, .. } => {
            ast_expr_has_async(elem) || ast_expr_has_async(iter)
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
        let variants: Vec<Variant> = s.fields.iter().map(|f| {
            // 简化：字段作为变体处理
            // 实际的 enum field 没有子类型（简单变体）
            Variant {
                name: f.name.clone(),
                fields: match &f.ty {
                    AstType::Unit | AstType::None_ => vec![],
                    AstType::Tuple(elems) => {
                        // 元组变体: Circle(f64, f64, f64) → 三个无名 Field
                        elems.iter().map(|t| Field {
                            name: String::new(),
                            ty: from_ast_type(t),
                        }).collect()
                    }
                    other => vec![Field { name: String::new(), ty: from_ast_type(other) }],
                },
            }
        }).collect();

        let enum_methods: Vec<FnDef> = s.methods.iter().map(|m| {
            let mut method_ctx = TypeCtx::new();
            method_ctx.pending_items = ctx.pending_items.clone();
            method_ctx.struct_names = ctx.struct_names.clone();
            method_ctx.struct_fields = ctx.struct_fields.clone();
            method_ctx.struct_methods = ctx.struct_methods.clone();
            convert_fn_def(m, &method_ctx)
        }).collect();

        Item::EnumDef(EnumDef {
            name: s.name.clone(),
            generics: s.generics.iter().map(|g| GenericParam {
                name: g.clone(), bounds: vec![], default: None,
            }).collect(),
            variants,
            methods: enum_methods,
            span: Span::unknown(),
        })
    } else {
        let fields: Vec<Field> = s.fields.iter().map(|f| Field {
            name: f.name.clone(),
            ty: from_ast_type(&f.ty),
        }).collect();

        let methods: Vec<FnDef> = s.methods.iter().map(|m| {
            let mut method_ctx = TypeCtx::new();
            method_ctx.pending_items = ctx.pending_items.clone();
            method_ctx.struct_names = ctx.struct_names.clone();
            method_ctx.struct_fields = ctx.struct_fields.clone();
            method_ctx.struct_methods = ctx.struct_methods.clone();
            convert_fn_def(m, &method_ctx)
        }).collect();

        Item::StructDef(StructDef {
            name: s.name.clone(),
            generics: s.generics.iter().map(|g| GenericParam {
                name: g.clone(), bounds: vec![], default: None,
            }).collect(),
            fields,
            methods,
            has_new: s.magic_methods.iter().any(|m| m.name == "__new__"),
            span: Span::unknown(),
        })
    }
}

fn convert_trait(t: &ast::TraitDef) -> Item {
    let methods: Vec<FnSig> = t.methods.iter().map(|m| FnSig {
        name: m.name.clone(),
        generics: m.generics.iter().map(|g| GenericParam {
            name: g.clone(), bounds: vec![], default: None,
        }).collect(),
        params: m.params.iter().map(|p| from_ast_type(&p.ty)).collect(),
        ret: m.return_type.as_ref().map(|t| from_ast_type(t)).unwrap_or(IrType::Unit),
    }).collect();

    Item::TraitDef(TraitDef {
        name: t.name.clone(),
        generics: t.generics.iter().map(|g| GenericParam {
            name: g.clone(), bounds: vec![], default: None,
        }).collect(),
        supertraits: vec![],
        methods,
    })
}

fn convert_impl(imp: &ast::ImplDef, ctx: &TypeCtx) -> Item {
    let methods: Vec<FnDef> = imp.methods.iter().map(|m| {
        let mut impl_ctx = TypeCtx::new();
        impl_ctx.pending_items = ctx.pending_items.clone();
        for sn in &ctx.struct_names { impl_ctx.struct_names.insert(sn.clone()); }
        for (sn, fields) in &ctx.struct_fields {
            let mut cloned = HashMap::new();
            for (fn_, ty) in fields { cloned.insert(fn_.clone(), ty.clone()); }
            impl_ctx.struct_fields.insert(sn.clone(), cloned);
        }
        for (sn, ms) in &ctx.struct_methods {
            impl_ctx.struct_methods.insert(sn.clone(), ms.clone());
        }
        for (cn, ct) in &ctx.top_level_consts { impl_ctx.top_level_consts.insert(cn.clone(), ct.clone()); }
        for (name, ty) in &ctx.fn_returns { impl_ctx.fn_returns.insert(name.clone(), ty.clone()); }
        impl_ctx.current_generics = imp.generics.clone();
        convert_fn_def(m, &impl_ctx)
    }).collect();

    Item::Impl(ImplDef {
        trait_: imp.trait_name.as_ref().map(|n| IrType::named(n)),
        for_type: IrType::named(&imp.type_name),
        generics: imp.generics.iter().map(|g| GenericParam {
            name: g.clone(), bounds: vec![], default: None,
        }).collect(),
        methods,
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
pub fn build_ir(ast_module: &ast::Module) -> Result<IrModule, IrBuildError> {
    let mut ctx = TypeCtx::new();
    let pending_items = Rc::new(RefCell::new(Vec::new()));
    ctx.pending_items = pending_items.clone();

    // 1. 收集类型信息
    ctx.collect_structs(ast_module);
    ctx.collect_functions(ast_module);
    // 预收集顶层 const 类型（供函数体内引用查询，如生成器集合迭代）
    for c in &ast_module.consts {
        let ty = c.ty.as_ref().map(|t| from_ast_type(t))
            .unwrap_or_else(|| infer_expr_type(&c.value, &ctx));
        eprintln!("DEBUG const {} ty={:?}", c.name, ty);
        ctx.top_level_consts.insert(c.name.clone(), ty);
    }

    // 2. 构建 IR 模块
    let name = ast_module.name.clone().unwrap_or_else(|| "main".to_string());
    let mut ir_mod = IrModule::new(name);

    // 3. 默认 prelude（lz.std 内建）
    ir_mod.prelude = vec![
        "Option".into(), "Result".into(), "Ordering".into(),
        "Box".into(), "Rc".into(), "Arc".into(),
        "Itor".into(), "Strategy".into(),
    ];

    // 4. 转换 imports → Use 项
    for imp in &ast_module.imports {
        ir_mod.items.push(Item::Use(UseStmt {
            path: imp.path.clone(),
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
            enum_extra_methods.entry(s.name.clone()).or_default().push(s);
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
        ir_mod.items.push(convert_trait(t));
    }

    // 7. 转换 impls
    for imp in &ast_module.impls {
        ir_mod.items.push(convert_impl(imp, &ctx));
    }

    // 7.5 独立 magic 块: magic __str__: def __str__(self: MyStruct) → impl MyStruct
    for mb in &ast_module.magic_blocks {
        // 从 self 参数类型确定目标类型
        let target = mb.function.params.iter()
            .find(|p| p.name == "self" || p.name == "self_")
            .map(|p| from_ast_type(&p.ty))
            .and_then(|ty| if let IrType::Named { path, .. } = ty { Some(path) } else { None })
            .unwrap_or_default();
        if target.is_empty() { continue; }
        // 注册魔术方法名到 struct_methods，供运算符/调用分发
        ctx.struct_methods.entry(target.clone()).or_default().insert(mb.method_name.clone());
        let mut impl_ctx = ctx.clone();
        impl_ctx.current_generics = mb.function.generics.clone();
        let method = convert_fn_def(&mb.function, &impl_ctx);
        ir_mod.items.push(Item::Impl(ImplDef {
            trait_: None,
            for_type: IrType::named(&target),
            generics: vec![],
            methods: vec![method],
        }));
    }

    // 8. 转换 functions
    for f in &ast_module.functions {
        ir_mod.items.push(Item::FnDef(convert_fn_def(f, &ctx)));
    }

    // 9. 转换 consts
    for c in &ast_module.consts {
        let ty = c.ty.as_ref().map(|t| from_ast_type(t))
            .unwrap_or_else(|| infer_expr_type(&c.value, &ctx));
        // 记录顶层 const 类型，供函数内 lookup_var 查询（如生成器集合迭代）
        ctx.top_level_consts.insert(c.name.clone(), ty.clone());
        ir_mod.items.push(Item::Const(ConstDef {
            name: c.name.clone(),
            ty,
            value: convert_expr(&c.value, &ctx),
        }));
    }

    // 9.5. 转换 type aliases
    for ta in &ast_module.type_aliases {
        let ir_ty = from_ast_type(&ta.ty);
        ir_mod.items.push(Item::TypeAlias(TypeAliasDef {
            name: ta.name.clone(),
            ty: ir_ty,
        }));
    }

    // 9.6. 转换顶层构建块 x =: body → let x = { ... }
    // 构建块以 BlockExpr 表示（依次执行语句，最后一个表达式为值）
    for (name, body) in &ast_module.top_level_builds {
        let mut block_ctx = TypeCtx::new();
        block_ctx.current_generics = ctx.current_generics.clone();
        // 预扫描：收集构建块内局部变量类型（x = value 赋值），供元组/表达式推断
        for s in body {
            eprintln!("DEBUG buildbody stmt = {:?}", s);
            match s {
                AstStmt::Expr(e) => {
                    if let AstExpr::Assign { target, value, .. } = e {
                        if let AstExpr::Ident(vname) = target.as_ref() {
                            block_ctx.add_var(vname, infer_expr_type(value, &block_ctx));
                        }
                    }
                }
                AstStmt::Let { name, ty, value, .. } => {
                    let ir_ty = ty.as_ref().map(|t| from_ast_type(t))
                        .unwrap_or_else(|| infer_expr_type(value, &block_ctx));
                    block_ctx.add_var(name, ir_ty);
                }
                _ => {}
            }
        }
        let stmts: Vec<Stmt> = body.iter()
            .map(|s| convert_stmt(s, &block_ctx))
            .collect();
        let blk_ty = body.last()
            .map(|s| infer_stmt_type(s, &block_ctx))
            .unwrap_or(IrType::Unit);
        let value = Expr::new(
            ExprKind::BlockExpr { block: Block { stmts, ty: blk_ty.clone() } },
            blk_ty.clone(),
            Span::unknown(),
        );
        ir_mod.items.push(Item::Const(ConstDef {
            name: name.clone(),
            ty: blk_ty,
            value,
        }));
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
    // 先 drop ctx 确保 Rc 引用计数归 1，否则 try_unwrap 静默失败
    drop(ctx);
    if let Ok(items) = Rc::try_unwrap(pending_items) {
        ir_mod.items.extend(items.into_inner());
    }

    Ok(ir_mod)
}

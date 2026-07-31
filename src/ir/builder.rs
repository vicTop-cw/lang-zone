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

use super::types::{IrType, from_ast_type};
use super::node::*;
use super::IrModule;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// 类型推导上下文
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
            }
        }
    }

    fn collect_functions(&mut self, module: &ast::Module) {
        for f in &module.functions {
            if let Some(ref ret_ty) = f.return_type {
                self.fn_returns.insert(f.name.clone(), from_ast_type(ret_ty));
            }
            let params: Vec<IrType> = f.params.iter()
                .map(|p| from_ast_type(&p.ty))
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
        BinOp::Pow => BinOpKind::Mul,     // Pow 降级为 Mul（后端自行处理）
        BinOp::In => BinOpKind::Eq,        // In 降级，由后端处理
        BinOp::Is => BinOpKind::Eq,        // Is 降级
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
        AssignOp::PowEq => BinOpKind::Mul,
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
        AstExpr::Call { func, .. } => {
            if let AstExpr::Ident(fname) = func.as_ref() {
                // print/println/panic 是语言内建，返回 Unit
                if fname == "print" || fname == "println" || fname == "panic" {
                    return IrType::Unit;
                }
                if ctx.is_struct(fname) {
                    return IrType::named(fname);
                }
                ctx.lookup_fn_return(fname)
            } else {
                IrType::Any
            }
        }
        AstExpr::MethodCall { receiver, method, .. } => {
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
        AstExpr::ListLit(_) => IrType::named("List"),
        AstExpr::DictLit(_) => IrType::named("Dict"),
        AstExpr::SetLit(_) => IrType::named("Set"),
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
        AstExpr::BuildBlock { lhs, .. } => infer_expr_type(lhs, ctx),
        AstExpr::KwArg { .. } => IrType::Any,
        AstExpr::PathAccess { .. } => IrType::Any,
        AstExpr::SafeNav { .. } => IrType::Any,
        AstExpr::TryCatch { .. } => IrType::Any,
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
        AstExpr::FStrLit(s) | AstExpr::RawStrLit(s) => ExprKind::Lit(LitKind::Str(s.clone())),
        AstExpr::BoolLit(b) => ExprKind::Lit(LitKind::Bool(*b)),
        AstExpr::NoneLit => ExprKind::Lit(LitKind::None_),
        AstExpr::Ident(name) => ExprKind::Var(name.clone()),

        AstExpr::Call { func, args } => {
            ExprKind::Call { type_args: vec![],
                callee: Box::new(convert_expr(func, ctx)),
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

            let ir_op = map_binop(op);
            
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
                    default: None,
                }).collect(),
                body: Box::new(convert_expr(body, ctx)),
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
            let recv = convert_expr(receiver, ctx);
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

        AstExpr::Try(inner) => {
            // try expr → MethodCall (类似 ?操作符)
            ExprKind::MethodCall {
                receiver: Box::new(convert_expr(inner, ctx)),
                method: "try_into".into(),
                args: vec![],
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

        AstExpr::DictLit(_entries) => {
            // Dict → StructCtor 或保留为 Dict
            ExprKind::StructCtor { name: "Dict".into(), fields: vec![] }
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

        AstExpr::ListComprehension { output, var, iter, cond: _ } => {
            // [out for x in iter if cond] → 展开为 for + if 的生成模式
            let iter_expr = convert_expr(iter, ctx);
            let out_expr = convert_expr(output, ctx);
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var("comp!".into()), IrType::Any, Span::unknown())),
                args: vec![
                    Expr::new(ExprKind::Lambda {
                        params: vec![Param { name: var.clone(), ty: IrType::Any, is_mut: false, default: None }],
                        body: Box::new(out_expr),
                    }, IrType::Any, Span::unknown()),
                    iter_expr,
                ],
            }
        }

        AstExpr::DictComprehension { key, value, var, iter, cond: _ } => {
            // {k: v for x in iter} → 展开为生成模式
            let iter_expr = convert_expr(iter, ctx);
            let key_expr = convert_expr(key, ctx);
            let val_expr = convert_expr(value, ctx);
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var("dict_comp!".into()), IrType::Any, Span::unknown())),
                args: vec![
                    Expr::new(ExprKind::Lambda {
                        params: vec![Param { name: var.clone(), ty: IrType::Any, is_mut: false, default: None }],
                        body: Box::new(Expr::new(
                            ExprKind::TupleLit(vec![key_expr, val_expr]),
                            IrType::Any, Span::unknown(),
                        )),
                    }, IrType::Any, Span::unknown()),
                    iter_expr,
                ],
            }
        }

        AstExpr::SetComprehension { elem, var, iter, cond: _ } => {
            // {x for x in iter} → 展开为生成模式
            let iter_expr = convert_expr(iter, ctx);
            let elem_expr = convert_expr(elem, ctx);
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var("set_comp!".into()), IrType::Any, Span::unknown())),
                args: vec![
                    Expr::new(ExprKind::Lambda {
                        params: vec![Param { name: var.clone(), ty: IrType::Any, is_mut: false, default: None }],
                        body: Box::new(elem_expr),
                    }, IrType::Any, Span::unknown()),
                    iter_expr,
                ],
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
            ExprKind::Call { type_args: vec![],
                callee: Box::new(Expr::new(ExprKind::Var("spawn".into()), IrType::Any, Span::unknown())),
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

        AstExpr::BuildBlock { kind, lhs, body: _ } => {
            // 构建块脱糖 — 这是核心
            match kind {
                BuildKind::Var => {
                    // =: → 转为 Let 语句（在下层处理，这里给占位）
                    ExprKind::Lit(LitKind::Unit)
                }
                BuildKind::Index => {
                    // ^: → IndexGet
                    ExprKind::IndexGet {
                        base: Box::new(convert_expr(lhs, ctx)),
                        key: Box::new(convert_expr(lhs, ctx)),
                    }
                }
                BuildKind::Call => {
                    // ~: → Call — body 是块尾元组/字典作为参数
                    ExprKind::Call { type_args: vec![],
                        callee: Box::new(convert_expr(lhs, ctx)),
                        args: vec![Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown())],
                    }
                }
                BuildKind::Gen => {
                    // *: → Vec::new() 占位
                    ExprKind::ListLit(vec![])
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
            let body_block = convert_block(body, ctx);
            let mut stmts = body_block.stmts;

            // else 块：接在 try body 成功路径后面
            if let Some(ref else_blk) = else_body {
                let else_block = convert_block(else_blk, ctx);
                stmts.extend(else_block.stmts);
            }

            // catch 块：用 If+flag 模拟（TODO: 接入真正的 catch_unwind）
            if !catches.is_empty() {
                let catch_block = convert_block(&catches[0].body, ctx);
                // 注释标记 catch 逻辑（后端可识别为错误处理）
                stmts.push(Stmt::ExprStmt {
                    expr: Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown()),
                });
                stmts.extend(catch_block.stmts);
            }

            // finally 块：始终追加
            if let Some(ref finally_blk) = finally_body {
                let finally_block = convert_block(finally_blk, ctx);
                stmts.extend(finally_block.stmts);
            }

            ExprKind::BlockExpr {
                block: Block { stmts, ty: IrType::Any },
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
    let ir_stmts: Vec<Stmt> = stmts.iter().map(|s| convert_stmt(s, ctx)).collect();
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
                if let AstPattern::Ident(name) = &arm.pattern {
                    let scrut_ty = infer_expr_type(expr, ctx);
                    arm_ctx.add_var(name, scrut_ty);
                }
                let body = convert_block_with_ctx(&arm.body, &arm_ctx);
                MatchArm { pattern: pat, guard, body }
            }).collect();
            Stmt::Match { scrutinee: ir_scrutinee, arms: ir_arms }
        }
        AstStmt::Expr(e) => Stmt::ExprStmt { expr: convert_expr(e, ctx) },

        AstStmt::Pass => Stmt::Pass,

        AstStmt::TypeAlias { name, ty } => Stmt::TypeAlias {
            name: name.clone(),
            ty: from_ast_type(ty),
        },

        AstStmt::Let { name, mutable, ty, value, .. } => {
            let ir_ty = ty.as_ref().map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, ctx));
            Stmt::Let {
                name: name.clone(),
                ty: ir_ty,
                value: convert_expr(value, ctx),
                is_mut: *mutable,
            }
        }

        AstStmt::Const { name, ty, value } => {
            let ir_ty = ty.as_ref().map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, ctx));
            Stmt::Let {
                name: name.clone(),
                ty: ir_ty,
                value: convert_expr(value, ctx),
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
            // defer → 展开为 Block（在 block end 处追加 cleanup）
            let mut stmts = convert_block(body, ctx).stmts;
            stmts.push(Stmt::ExprStmt {
                expr: Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown()),
            });
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
            Stmt::Block {
                stmts: vec![
                    Stmt::Let { name: name.clone(), ty: val_ty.clone(), value: val, is_mut: false },
                    // defer → cleanup at block end
                    Stmt::ExprStmt {
                        expr: Expr::new(
                            ExprKind::MethodCall {
                                receiver: Box::new(Expr::new(ExprKind::Var(name), val_ty.clone(), Span::unknown())),
                                method: "drop".into(),
                                args: vec![],
                            },
                            IrType::Unit, Span::unknown(),
                        )
                    },
                ].into_iter()
                    .chain(body.iter().map(|s| convert_stmt(s, &with_ctx)))
                    .collect(),
            }
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
            // 嵌套函数提升为模块级 Item::FnDef，名称为 {父函数}_{子函数}
            let parent_name = ctx.current_fn_name.clone().unwrap_or_default();
            let nested_name = if parent_name.is_empty() {
                func.name.clone()
            } else {
                format!("{}_{}", parent_name, func.name)
            };
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
                stmts: ir_tests.iter().map(|s| convert_stmt(s, ctx)).collect(),
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
    for (vn, vt) in &ctx.vars { block_ctx.vars.insert(vn.clone(), vt.clone()); }
    for (name, ty) in &ctx.fn_returns { block_ctx.fn_returns.insert(name.clone(), ty.clone()); }
    for (name, p) in &ctx.fn_params { block_ctx.fn_params.insert(name.clone(), p.clone()); }
    for (vn, en) in &ctx.enum_variants { block_ctx.enum_variants.insert(vn.clone(), en.clone()); }
    
    let mut ir_stmts: Vec<Stmt> = Vec::new();
    for s in stmts {
        // 前向传播：Let 语句的变量添加到后续语句的上下文
        if let AstStmt::Let { name, ty, value, .. } = s {
            let ir_ty = ty.as_ref().map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, &block_ctx));
            block_ctx.add_var(name, ir_ty);
        }
        ir_stmts.push(convert_stmt(s, &block_ctx));
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

    let params: Vec<Param> = func.params.iter().enumerate().map(|(_i, p)| Param {
        name: p.name.clone(),
        ty: if is_math { IrType::Generic("T".into()) } else { from_ast_type(&p.ty) },
        is_mut: p.is_mut,
        default: p.default.as_ref().map(|d| convert_expr(d, ctx)),
    }).collect();

    // 构建函数体上下文
    let mut fn_ctx = TypeCtx::new();
    fn_ctx.pending_items = ctx.pending_items.clone();
    fn_ctx.current_fn_name = Some(func.name.clone());
    fn_ctx.current_generics = if is_math {
        vec!["T".to_string()]
    } else {
        func.generics.clone()
    };
    // 复制全�� struct 信息
    for sn in &ctx.struct_names { fn_ctx.struct_names.insert(sn.clone()); }
    for (sn, fields) in &ctx.struct_fields {
        let mut cloned = HashMap::new();
        for (fn_, ty) in fields { cloned.insert(fn_.clone(), ty.clone()); }
        fn_ctx.struct_fields.insert(sn.clone(), cloned);
    }
    for (name, ty) in &ctx.fn_returns { fn_ctx.fn_returns.insert(name.clone(), ty.clone()); }
    for (name, p) in &ctx.fn_params { fn_ctx.fn_params.insert(name.clone(), p.clone()); }

    // 添加参数到作用域
    if is_math {
        for p in &func.params {
            fn_ctx.add_param(&p.name, IrType::Generic("T".into()));
        }
    } else {
        for p in &func.params {
            fn_ctx.add_param(&p.name, from_ast_type(&p.ty));
        }
    }

    // 返回类型：优先 AST 注解，否则从函数体最后语句推断
    let ret_ty = func.return_type.as_ref()
        .map(|t| from_ast_type(t))
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
            func.generics.iter().map(|g| GenericParam {
                name: g.clone(),
                bounds: vec![],
                default: None,
            }).collect()
        },
        params,
        ret_ty,
        body,
        intrinsics,
        is_async: func.is_async,
        is_iterator: func.is_iterator,
        is_test: false,
        span: Span::unknown(),
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
            convert_fn_def(m, &method_ctx)
        }).collect();

        Item::StructDef(StructDef {
            name: s.name.clone(),
            generics: s.generics.iter().map(|g| GenericParam {
                name: g.clone(), bounds: vec![], default: None,
            }).collect(),
            fields,
            methods,
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

    // 8. 转换 functions
    for f in &ast_module.functions {
        ir_mod.items.push(Item::FnDef(convert_fn_def(f, &ctx)));
    }

    // 9. 转换 consts
    for c in &ast_module.consts {
        let ty = c.ty.as_ref().map(|t| from_ast_type(t))
            .unwrap_or_else(|| infer_expr_type(&c.value, &ctx));
        ir_mod.items.push(Item::Const(ConstDef {
            name: c.name.clone(),
            ty,
            value: convert_expr(&c.value, &ctx),
        }));
    }

    // 9.5. 转换 type aliases → Const 项（type UserId = int）
    for ta in &ast_module.type_aliases {
        let ir_ty = from_ast_type(&ta.ty);
        ir_mod.items.push(Item::Const(ConstDef {
            name: ta.name.clone(),
            ty: ir_ty,
            value: Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown()),
        }));
    }

    // 9.6. 转换顶层构建块 x =: body → let x = { ... }
    // 构建块以 BlockExpr 表示（依次执行语句，最后一个表达式为值）
    for (name, body) in &ast_module.top_level_builds {
        let mut block_ctx = TypeCtx::new();
        block_ctx.current_generics = ctx.current_generics.clone();
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
    if let Ok(items) = Rc::try_unwrap(pending_items) {
        ir_mod.items.extend(items.into_inner());
    }

    Ok(ir_mod)
}

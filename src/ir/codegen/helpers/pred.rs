// Lang-Zone 编译器 — ir/codegen/helpers/pred.rs
// 判定/查询（叶子层）：块/表达式/模式/IrType 的只读谓词，返回 bool / Option<..>，不产出文本、不写 out 集合
// 自 helpers.rs 逐字节拆出（move-only）：函数体/注释/签名原样保留，仅新增本文件头与 use。
use crate::ir::node::*;
use crate::ir::types::IrType;
use std::collections::HashMap;

pub(crate) fn is_kwarg_call(args: &[Expr]) -> bool {
    args.iter()
        .any(|a| matches!(&a.kind, ExprKind::StructCtor { name, .. } if name == "_KwArg"))
}

/// 内建类型/枚举构造器名（`Some(x)`、`None`、`Ok(x)`、`Err(x)` 及集合别名）。
/// 这些名字以 `Var(name)` 出现在 Call 的 callee 位时，**不是**对外层非 Copy 变量的
/// 引用，不应触发非 move 降级或 `.clone()`（否则生成 `Some::clone()(v*2)`，E0599）。
/// （BUG-358：option_chain.lz `|v| Some(v * 2)` 因把 Some 当外部变量，闭包被降级
/// 非 move 并 clone 了构造器名，生成 `y.flat_map(|v| { Some::clone()(v * 2i64) })`）
pub(crate) fn is_type_ctor_name(name: &str) -> bool {
    matches!(
        name,
        "Some" | "None" | "Ok" | "Err" | "Option" | "Result" | "List" | "Vec" | "list"
            | "Set" | "HashSet" | "set" | "Dict" | "HashMap" | "dict"
    )
}

/// 类型/枚举构造器名总判定：内建（`Some`/`Ok`/集合别名，见 `is_type_ctor_name`）
/// ∪ 用户自定义枚举变体（`enum Foo: Bar(int)` 的 `Bar`）。变体名以 `Var(name)` 出现
/// 在 Call 的 callee 位时是构造器，**不是**对外层非 Copy 变量的引用；不排除就会被
/// 闭包体 clone 路径改写成 `Foo::Bar::clone()(..)` → E0433（Bar is a variant, not a module）。
/// 变体表由调用方（CodeGen）传入：本组 helper 是自由函数，拿不到 CodeGen 的字段。
pub(crate) fn is_ctor_name(name: &str, variants: &HashMap<String, String>) -> bool {
    is_type_ctor_name(name) || variants.contains_key(name)
}

/// 判断实参是否为关键字实参节点：`_KwArg`（`name~` 糖）或
/// `AssignExpr { target: 简单变量 }`（`name=value` 全形，parser 按赋值表达式解析）。
pub(crate) fn is_kwarg_arg(a: &Expr) -> bool {
    kwarg_name_value(a).is_some()
}

/// 提取关键字实参的（参数名, 值表达式）。非关键字实参返回 None。
pub(crate) fn kwarg_name_value(a: &Expr) -> Option<(String, &Expr)> {
    match &a.kind {
        ExprKind::StructCtor { name, fields } if name == "_KwArg" => {
            let k = fields
                .iter()
                .find(|(n, _)| n == "name")
                .and_then(|(_, v)| match &v.kind {
                    ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                    _ => None,
                })?;
            let value = fields.iter().find(|(n, _)| n == "value")?;
            Some((k, &value.1))
        }
        ExprKind::AssignExpr { target, value } => {
            if let ExprKind::Var(name) = &target.kind {
                Some((name.clone(), value))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 判断模式是否为列表模式（[a, b, c] / [first, ..rest]）或其子模式包含列表模式
pub(crate) fn pattern_is_list(pat: &Pattern) -> bool {
    match pat {
        Pattern::List(_) => true,
        Pattern::Tuple(elems) => elems.iter().any(pattern_is_list),
        Pattern::Struct { fields, .. } => fields.iter().any(|(_, p)| pattern_is_list(p)),
        Pattern::Enum { args, .. } => args.iter().any(pattern_is_list),
        _ => false,
    }
}

/// 是否为消耗型（owned self）魔术方法
pub(crate) fn is_consuming_self(f: &FnDef) -> bool {
    matches!(f.name.as_str(), "__enter__" | "__iter__")
}

pub(crate) fn block_has_yield(block: &Block) -> bool {
    for stmt in &block.stmts {
        if matches!(stmt, Stmt::Yield { .. } | Stmt::YieldFrom { .. }) {
            return true;
        }
        match stmt {
            Stmt::ExprStmt { expr } => {
                if expr_has_yield(expr) {
                    return true;
                }
            }
            Stmt::Let { value, .. } => {
                if expr_has_yield(value) {
                    return true;
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if block_has_yield(then_branch) {
                    return true;
                }
                if let Some(ref e) = else_branch {
                    if block_has_yield(e) {
                        return true;
                    }
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
                if block_has_yield(body) {
                    return true;
                }
            }
            // BUG-285：`Stmt::Match` 各分支体是 Block，yield 嵌套其中需下钻
            // （TEMP/probe/sweep6/gen_match.lz：match 分支内 yield 漏检 →
            // 返回类型 i64 而非 Vec<i64>，且 __gen_vec 未声明 → E0425）。
            Stmt::Match { arms, .. } => {
                if arms.iter().any(|arm| block_has_yield(&arm.body)) {
                    return true;
                }
            }
            // BUG-289：`Stmt::TryCatch` 各分支体是 Block，yield 嵌套其中需下钻
            // （TEMP/probe/sweep6/gen_try.lz：try 块内 yield 漏检 → 返回类型
            // Itor<i64> 而非 Vec<i64>，且 __gen_vec 未声明 → E0425）。
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                if block_has_yield(body)
                    || catches.iter().any(|(_, b)| block_has_yield(b))
                    || else_body.as_ref().map_or(false, block_has_yield)
                    || finally_body.as_ref().map_or(false, block_has_yield)
                {
                    return true;
                }
            }
            Stmt::Block { stmts } => {
                if block_has_yield(&Block {
                    span: Span::unknown(),
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                }) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// 递归检查块内是否含 `Stmt::Raise` / `Stmt::Return`（含下钻 if/for/while/match/
/// try/defer/block 等嵌套块）。用途（BUG-295）：try 体闭包内含
/// `return __gen_vec`（生成器 raise 重写产物）时闭包返回 Vec<i64>，catch 臂须
/// 同样返回 Vec<i64>（否则 match arms 类型不兼容 E0308）。
pub(crate) fn block_has_raise_or_return(block: &Block) -> bool {
    for stmt in &block.stmts {
        if matches!(stmt, Stmt::Raise { .. } | Stmt::Return { .. }) {
            return true;
        }
        match stmt {
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if block_has_raise_or_return(then_branch) {
                    return true;
                }
                if let Some(ref e) = else_branch {
                    if block_has_raise_or_return(e) {
                        return true;
                    }
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
                if block_has_raise_or_return(body) {
                    return true;
                }
            }
            Stmt::Match { arms, .. } => {
                if arms.iter().any(|arm| block_has_raise_or_return(&arm.body)) {
                    return true;
                }
            }
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                if block_has_raise_or_return(body)
                    || catches.iter().any(|(_, b)| block_has_raise_or_return(b))
                    || else_body.as_ref().map_or(false, block_has_raise_or_return)
                    || finally_body.as_ref().map_or(false, block_has_raise_or_return)
                {
                    return true;
                }
            }
            Stmt::Defer { body } | Stmt::BlockLabel { body, .. } => {
                if block_has_raise_or_return(body) {
                    return true;
                }
            }
            Stmt::Block { stmts } => {
                if block_has_raise_or_return(&Block {
                    span: Span::unknown(),
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                }) {
                    return true;
                }
            }
            Stmt::ExprStmt { expr } => {
                if expr_has_raise_or_return(expr) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// 递归检查表达式内是否含 raise/return（if 表达式分支体可能内嵌 raise/return）。
fn expr_has_raise_or_return(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::IfExpr { then, els, .. } => {
            expr_has_raise_or_return(then) || expr_has_raise_or_return(els)
        }
        ExprKind::BlockExpr { block } => block_has_raise_or_return(block),
        _ => false,
    }
}

/// 检测 Block 中是否包含无值 return（return;）——构建块（=:/~:/*:）内
/// return; 退出构建块自身，块值应为 ()；此时尾表达式需生成 `expr;`（丢弃值），
/// 否则闭包返回类型被推断为尾值类型，与 return; 冲突（E0308）
pub(crate) fn block_has_bare_return(block: &Block) -> bool {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Return { value: None } => return true,
            Stmt::ExprStmt { expr } => {
                if expr_has_bare_return(expr) {
                    return true;
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if block_has_bare_return(then_branch) {
                    return true;
                }
                if let Some(ref e) = else_branch {
                    if block_has_bare_return(e) {
                        return true;
                    }
                }
            }
            Stmt::Block { stmts } => {
                if block_has_bare_return(&Block {
                    span: Span::unknown(),
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                }) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// 检测表达式内是否包含无值 return（return;）——覆盖 IfExpr 分支中的
/// BlockExpr 块体（构建块内 `if skip: return;` 被转换为此形态）
pub(crate) fn expr_has_bare_return(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::BlockExpr { block } => block_has_bare_return(block),
        ExprKind::IfExpr { then, els, .. } => {
            expr_has_bare_return(then) || expr_has_bare_return(els)
        }
        _ => false,
    }
}

/// 闭包体内是否赋值外部捕获变量（iter.lz for_each `|x| total = total + x`）：
/// 若是则用借用捕获（非 move），否则 move 复制副本导致外部变量不更新
pub(crate) fn block_has_external_assign(block: &Block, params: &[String]) -> bool {
    for stmt in &block.stmts {
        match stmt {
            Stmt::ExprStmt { expr } => {
                if expr_has_external_assign(expr, params) {
                    return true;
                }
            }
            Stmt::Let { value, .. } => {
                if expr_has_external_assign(value, params) {
                    return true;
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if block_has_external_assign(then_branch, params)
                    || else_branch
                        .as_ref()
                        .map_or(false, |e| block_has_external_assign(e, params))
                {
                    return true;
                }
            }
            Stmt::Block { stmts } => {
                if block_has_external_assign(
                    &Block {
                        span: Span::unknown(),
                        stmts: stmts.clone(),
                        ty: IrType::Unit,
                    },
                    params,
                ) {
                    return true;
                }
            }
            Stmt::While { body, .. } => {
                if block_has_external_assign(body, params) {
                    return true;
                }
            }
            Stmt::For { body, .. } => {
                if block_has_external_assign(body, params) {
                    return true;
                }
            }
            Stmt::Assign { target, .. } => {
                // 闭包体内赋值外部变量（`total = total + x` 是语句级 Assign）：
                // 否则漏检 → 误用 move 捕获 total 副本，外部变量不更新（输出 0）
                if let ExprKind::Var(n) = &target.kind {
                    if !params.iter().any(|p| p == n) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn expr_has_external_assign(expr: &Expr, params: &[String]) -> bool {
    match &expr.kind {
        ExprKind::AssignExpr { target, value } => {
            let is_param = if let ExprKind::Var(n) = &target.kind {
                params.iter().any(|p| p == n)
            } else {
                false
            };
            if !is_param {
                return true;
            }
            expr_has_external_assign(value, params)
        }
        ExprKind::BlockExpr { block } => block_has_external_assign(block, params),
        ExprKind::IfExpr { then, els, .. } => {
            expr_has_external_assign(then, params) || expr_has_external_assign(els, params)
        }
        ExprKind::Call { callee, args, .. } => {
            expr_has_external_assign(callee, params)
                || args.iter().any(|a| expr_has_external_assign(a, params))
        }
        _ => false,
    }
}

pub(crate) fn expr_has_yield(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::BlockExpr { block } => block_has_yield(block),
        ExprKind::GenBuild { block, .. } => {
            // 生成器构建块内的 yield 由构建块自身收集，不污染外层函数
            let _ = block;
            false
        }
        ExprKind::IfExpr { then, els, .. } => expr_has_yield(then) || expr_has_yield(els),
        ExprKind::Call { callee, args, .. } => {
            expr_has_yield(callee) || args.iter().any(expr_has_yield)
        }
        ExprKind::Lambda { body, .. } => expr_has_yield(body),
        // raises 生成器：builder 把函数体包成 `Result::Ok(BlockExpr{...})`
        // （ExprStmt(EnumCtor("Result","Ok",[BlockExpr]))），yield 藏在 Ok(...)
        // 实参里。此前未下钻 EnumCtor 实参 → expr_has_yield=false → has_yield=false
        // → 函数被当成普通 raises 函数：签名 `Result<T,E>` 而非 `Vec<Result<T,E>>`，
        // 且不声明 `__gen_vec`，体内 yield 仍生成 `__gen_vec.push(...)` → E0425
        // （TEMP/probe/sweep10/gen_try_raises.lz）。
        ExprKind::EnumCtor { args, .. } => args.iter().any(expr_has_yield),
        _ => false,
    }
}

/// 检测 Block 中是否包含 await 表达式
pub(crate) fn block_has_await(block: &Block) -> bool {
    for stmt in &block.stmts {
        if stmt_has_await(stmt) {
            return true;
        }
    }
    false
}

pub(crate) fn stmt_has_await(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::ExprStmt { expr } => expr_has_await(expr),
        Stmt::Return { value: Some(expr) } => expr_has_await(expr),
        Stmt::Yield { value } => expr_has_await(value),
        Stmt::Let { value, .. } => expr_has_await(value),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            expr_has_await(cond)
                || block_has_await(then_branch)
                || else_branch.as_ref().map_or(false, block_has_await)
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
            block_has_await(body)
        }
        Stmt::Block { stmts } => block_has_await(&Block {
            span: Span::unknown(),
            stmts: stmts.clone(),
            ty: IrType::Unit,
        }),
        _ => false,
    }
}

pub(crate) fn expr_has_await(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::MethodCall { method, .. } if method == "await" => true,
        ExprKind::Call { callee, args, .. } => {
            expr_has_await(callee) || args.iter().any(expr_has_await)
        }
        ExprKind::BinOp { lhs, rhs, .. } => expr_has_await(lhs) || expr_has_await(rhs),
        ExprKind::BlockExpr { block } => block_has_await(block),
        ExprKind::Lambda { body, .. } => expr_has_await(body),
        _ => false,
    }
}

/// 提取 _KwArg 的字段名（用于递归字段构造 Box 判断）
pub(crate) fn kwarg_field_name(arg: &Expr) -> Option<String> {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
        if name == "_KwArg" {
            return fields
                .iter()
                .find(|(n, _)| n == "name")
                .and_then(|(_, v)| match &v.kind {
                    ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                    _ => None,
                });
        }
    }
    None
}

/// 检测 IrType 是否引用了指定的类型名（用于递归枚举检测）
pub(crate) fn type_refers_to(ty: &IrType, name: &str) -> bool {
    match ty {
        IrType::Named { path, args } => {
            if path == name {
                return true;
            }
            args.iter().any(|a| type_refers_to(a, name))
        }
        IrType::Option(inner)
        | IrType::Result { ok: inner, err: _ }
        | IrType::Ref(inner)
        | IrType::MutRef(inner) => type_refers_to(inner, name),
        IrType::Tuple(elems) => elems.iter().any(|e| type_refers_to(e, name)),
        IrType::Fn { params, ret } => {
            params.iter().any(|p| type_refers_to(p, name)) || type_refers_to(ret, name)
        }
        _ => false,
    }
}

/// 字段是否需要自动 Box：仅当字段类型**直接**是自身（`Self` / `Self?` / `Option<Self>`），
/// 才需要 Box 打破无限大小。`Vec<Self>`、`Rc<Self>`、`Box<Self>` 等已间接，无需 Box。
pub(crate) fn field_needs_box(ty: &IrType, name: &str) -> bool {
    match ty {
        IrType::Self_ => true,
        IrType::Named { path, .. } => path == name,
        IrType::Option(inner) => match inner.as_ref() {
            IrType::Self_ => true,
            IrType::Named { path, .. } => path == name,
            _ => false,
        },
        _ => false,
    }
}

/// 从 IR 类型中提取命名字构体名：`Named{path:..}` 直接取 path，`Option(Named{..})`
/// 递归取内层（字段赋值 `root.left = ...` 中 base.ty 是 `Option<TreeNode<T>>`）。
pub(crate) fn struct_name_from_ty(ty: &IrType) -> Option<String> {
    match ty {
        IrType::Named { path, .. } => Some(path.clone()),
        IrType::Option(inner) => match inner.as_ref() {
            IrType::Named { path, .. } => Some(path.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// 判断表达式是否为 `None` 字面量/构造（None_ 字面量、`None` 变量、
/// `None` 结构构造、`Option::None` 枚举构造）。
pub(crate) fn is_none_expr(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Lit(LitKind::None_) => true,
        ExprKind::Var(n) if n == "None" => true,
        ExprKind::StructCtor { name, .. } if name == "None" => true,
        ExprKind::EnumCtor { variant, .. } if variant == "None" => true,
        _ => false,
    }
}

/// BUG-SG-002/003：目标类型是否是可空类型 `T?`。
/// LZ 的 `T?` 在 IR 里有两种表示：语义化的 `IrType::Option(T)` 与命名式
/// `Named("Option", [T])`，两者都要识别。
pub(crate) fn is_option_ty(ty: &IrType) -> bool {
    match ty {
        IrType::Option(_) => true,
        IrType::Named { path, args } => path == "Option" && args.len() == 1,
        _ => false,
    }
}

/// BUG-SG-002/003：把值写进 `T?`（Option<T>）位置时是否需要自动包 `Some(..)`。
///
/// `let z: int? = 10`、`DbConfig { host: "localhost" }`（host 为 `str?`）
/// 这类「可空位置 + 非空值」组合，Rust 侧必然 E0308（expected Option<T>,
/// found T），须由编译器补 `Some(..)`。
///
/// 不包的情形：
/// - 值本身已是 Option 类型（含 `Some(x)` 调用与 Option 变量）
/// - 值是 `None`（各种表示形态）
/// - 值类型未定（`Any`）——无法判定，交给 Rust 推断，避免误包
pub(crate) fn needs_some_wrap(target_ty: &IrType, value: &Expr) -> bool {
    if !is_option_ty(target_ty) {
        return false;
    }
    if is_option_ty(&value.ty) || matches!(value.ty, IrType::Any) {
        return false;
    }
    !is_none_expr(value)
}

/// 判断表达式是否引用了指定变量名（自引用重绑定检测用，如 `let parts = parts + [p]`）
pub(crate) fn expr_mentions_var(expr: &Expr, name: &str) -> bool {
    match &expr.kind {
        ExprKind::Var(v) => v == name,
        ExprKind::Lit(_) | ExprKind::Default => false,
        ExprKind::Spread(inner) => expr_mentions_var(inner, name),
        ExprKind::Call { callee, args, .. } => {
            expr_mentions_var(callee, name) || args.iter().any(|a| expr_mentions_var(a, name))
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            expr_mentions_var(receiver, name) || args.iter().any(|a| expr_mentions_var(a, name))
        }
        ExprKind::FieldAccess { base, .. } => expr_mentions_var(base, name),
        ExprKind::IndexGet { base, key } => {
            expr_mentions_var(base, name) || expr_mentions_var(key, name)
        }
        ExprKind::IndexSet { base, key, value } => {
            expr_mentions_var(base, name)
                || expr_mentions_var(key, name)
                || expr_mentions_var(value, name)
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            expr_mentions_var(lhs, name) || expr_mentions_var(rhs, name)
        }
        ExprKind::AssignExpr { target, value } => {
            expr_mentions_var(target, name) || expr_mentions_var(value, name)
        }
        ExprKind::UnOp { operand, .. } => expr_mentions_var(operand, name),
        ExprKind::IfExpr { cond, then, els } => {
            expr_mentions_var(cond, name)
                || expr_mentions_var(then, name)
                || expr_mentions_var(els, name)
        }
        ExprKind::Lambda { body, .. } => expr_mentions_var(body, name),
        ExprKind::StructCtor { fields, .. } => {
            fields.iter().any(|(_, e)| expr_mentions_var(e, name))
        }
        ExprKind::EnumCtor { args, .. } => args.iter().any(|a| expr_mentions_var(a, name)),
        ExprKind::GenExpr { yield_of } => expr_mentions_var(yield_of, name),
        ExprKind::GenBuild { .. } => false,
        ExprKind::Cast { expr, .. } => expr_mentions_var(expr, name),
        ExprKind::MagicCall { args, .. } => args.iter().any(|a| expr_mentions_var(a, name)),
        ExprKind::BlockExpr { block } => block.stmts.iter().any(|s| match s {
            Stmt::Let { name: n, value, .. } => *n == name && expr_mentions_var(value, name),
            Stmt::ExprStmt { expr } => expr_mentions_var(expr, name),
            Stmt::Return { value } => value.as_ref().map_or(false, |v| expr_mentions_var(v, name)),
            _ => false,
        }),
        ExprKind::TupleLit(items)
        | ExprKind::Tuple(items)
        | ExprKind::ListLit(items)
        | ExprKind::List(items) => items.iter().any(|i| expr_mentions_var(i, name)),
        ExprKind::Dict(entries) => entries
            .iter()
            .any(|(k, v)| expr_mentions_var(k, name) || expr_mentions_var(v, name)),
        ExprKind::Range { start, end, .. } => {
            start.as_ref().map_or(false, |s| expr_mentions_var(s, name))
                || expr_mentions_var(end, name)
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            expr_mentions_var(receiver, name)
                || expr_mentions_var(callee, name)
                || args.iter().any(|a| expr_mentions_var(a, name))
        }
        ExprKind::Paren(inner) => expr_mentions_var(inner, name),
        ExprKind::ImplicitConvert { source, .. } => expr_mentions_var(source, name),
    }
}

/// 闭包体内是否含嵌套 move 闭包且该嵌套闭包捕获了非 Copy 外部变量
/// （matrix.lz 列表推导 `|i| { ...map(move |j| { default }) }`）。
/// 若命中，外层闭包应改用借用捕获（非 move），否则外层把变量移入自身，
/// 内层 move 闭包再次移出即 E0507 cannot move out of captured variable.
pub(crate) fn block_contains_nested_move_lambda(
    block: &Block,
    params: &[String],
    variants: &HashMap<String, String>,
) -> bool {
    for stmt in &block.stmts {
        match stmt {
            Stmt::ExprStmt { expr } => {
                if expr_contains_nested_move_lambda(expr, params, variants) {
                    return true;
                }
            }
            Stmt::Let { value, .. } => {
                if expr_contains_nested_move_lambda(value, params, variants) {
                    return true;
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if block_contains_nested_move_lambda(then_branch, params, variants)
                    || else_branch.as_ref().map_or(false, |e| {
                        block_contains_nested_move_lambda(e, params, variants)
                    })
                {
                    return true;
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } => {
                if block_contains_nested_move_lambda(body, params, variants) {
                    return true;
                }
            }
            Stmt::Match { arms, .. } => {
                if arms.iter().any(|a| block_contains_nested_move_lambda(&a.body, params, variants)) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// 表达式内是否含嵌套 Lambda，且该 Lambda 体内引用了非本 Lambda 形参的
/// 非 Copy 外部变量（此时嵌套闭包需 move 捕获该变量）
pub(crate) fn expr_contains_nested_move_lambda(
    e: &Expr,
    params: &[String],
    variants: &HashMap<String, String>,
) -> bool {
    match &e.kind {
        ExprKind::Lambda { params: lparams, body, .. } => {
            // 嵌套闭包体内的变量引用：非本闭包形参、非 Copy 类型 → 需 move 捕获
            let lparam_names: Vec<String> =
                lparams.iter().map(|p| p.name.clone()).collect();
            body_refs_noncopy_external(body, &lparam_names, variants)
        }
        ExprKind::BlockExpr { block } => block_contains_nested_move_lambda(block, params, variants),
        ExprKind::Call { callee, args, .. } => {
            expr_contains_nested_move_lambda(callee, params, variants)
                || args
                    .iter()
                    .any(|a| expr_contains_nested_move_lambda(a, params, variants))
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            expr_contains_nested_move_lambda(receiver, params, variants)
                || args
                    .iter()
                    .any(|a| expr_contains_nested_move_lambda(a, params, variants))
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            expr_contains_nested_move_lambda(lhs, params, variants)
                || expr_contains_nested_move_lambda(rhs, params, variants)
        }
        ExprKind::UnOp { operand, .. } => expr_contains_nested_move_lambda(operand, params, variants),
        ExprKind::AssignExpr { target, value, .. } => {
            expr_contains_nested_move_lambda(target, params, variants)
                || expr_contains_nested_move_lambda(value, params, variants)
        }
        ExprKind::FieldAccess { base, .. } => expr_contains_nested_move_lambda(base, params, variants),
        ExprKind::IndexGet { base, key, .. } => {
            expr_contains_nested_move_lambda(base, params, variants)
                || expr_contains_nested_move_lambda(key, params, variants)
        }
        ExprKind::IfExpr { cond, then, els } => {
            expr_contains_nested_move_lambda(cond, params, variants)
                || expr_contains_nested_move_lambda(then, params, variants)
                || expr_contains_nested_move_lambda(els, params, variants)
        }
        ExprKind::TupleLit(items) => {
            items
                .iter()
                .any(|i| expr_contains_nested_move_lambda(i, params, variants))
        }
        ExprKind::ListLit(items) => {
            items
                .iter()
                .any(|i| expr_contains_nested_move_lambda(i, params, variants))
        }
        _ => false,
    }
}

/// Lambda 体内是否引用非本 Lambda 形参的裸外部变量，且该变量类型非 Copy
/// （matrix.lz `|j| { default }` 的 default: T）
pub(crate) fn body_refs_noncopy_external(
    body: &Expr,
    lparam_names: &[String],
    variants: &HashMap<String, String>,
) -> bool {
    match &body.kind {
        ExprKind::Var(name) => {
            // BUG-358：类型/枚举构造器名（Some/None/Ok/Err/集合别名）不是外部变量引用
            !is_ctor_name(name, variants)
                && !lparam_names.contains(name)
                && !matches!(body.ty, IrType::Int | IrType::F64 | IrType::Bool)
        }
        ExprKind::BlockExpr { block } => {
            // block 单条语句是裸外部变量
            if block.stmts.len() == 1 {
                if let Stmt::ExprStmt { expr: e } = &block.stmts[0] {
                    return body_refs_noncopy_external(e, lparam_names, variants);
                }
            }
            // 更深层嵌套闭包递归
            block_contains_nested_move_lambda(block, lparam_names, variants)
        }
        // BUG-356: 推导链 body 是 Call（如 comp_mid!(|v| (k, v), ...)），
        // 内含嵌套闭包引用外层形参 k（非 Copy）。若不递归 Call，外层闭包
        // 仍用 move 捕获 k，内层 move 闭包再 move 捕获 k → E0507。
        ExprKind::Call { callee, args, .. } => {
            body_refs_noncopy_external(callee, lparam_names, variants)
                || args
                    .iter()
                    .any(|a| body_refs_noncopy_external(a, lparam_names, variants))
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            body_refs_noncopy_external(receiver, lparam_names, variants)
                || args
                    .iter()
                    .any(|a| body_refs_noncopy_external(a, lparam_names, variants))
        }
        ExprKind::Lambda { params, body, .. } => {
            let inner_param_names: Vec<String> =
                params.iter().map(|p| p.name.clone()).collect();
            // 内层闭包引用的外部变量（相对内层形参）非 Copy → 内层需 move，
            // 外层若也 move 则双重 move → E0507，需降级外层为借用捕获。
            body_refs_noncopy_external(body, &inner_param_names, variants)
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            body_refs_noncopy_external(lhs, lparam_names, variants)
                || body_refs_noncopy_external(rhs, lparam_names, variants)
        }
        ExprKind::IfExpr { cond, then, els } => {
            body_refs_noncopy_external(cond, lparam_names, variants)
                || body_refs_noncopy_external(then, lparam_names, variants)
                || body_refs_noncopy_external(els, lparam_names, variants)
        }
        ExprKind::ListLit(items) | ExprKind::TupleLit(items) => items
            .iter()
            .any(|i| body_refs_noncopy_external(i, lparam_names, variants)),
        _ => false,
    }
}


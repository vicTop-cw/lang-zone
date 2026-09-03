// Lang-Zone 编译器 — ir/codegen/helpers.rs
// LZIR → Rust 代码生成辅助函数（自 codegen.rs 拆分，独立自由函数）
// 子模块：可访问父模块 CodeGen 的私有方法与字段（Rust 隐私规则）

use super::super::node::*;
use super::super::types::IrType;
use super::CodeGen;

pub(crate) fn is_kwarg_call(args: &[Expr]) -> bool {
    args.iter()
        .any(|a| matches!(&a.kind, ExprKind::StructCtor { name, .. } if name == "_KwArg"))
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
            expr_has_external_assign(callee, params) || args.iter().any(|a| expr_has_external_assign(a, params))
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
        _ => false,
    }
}

/// 查找 Block 中第一个 yield 表达式的 IR 类型（生成器构建块 func *: 收集器类型用）
pub(crate) fn first_yield_type(block: &Block) -> Option<IrType> {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Yield { value } => return Some(value.ty.clone()),
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(t) = first_yield_type(then_branch) {
                    return Some(t);
                }
                if let Some(e) = else_branch {
                    if let Some(t) = first_yield_type(e) {
                        return Some(t);
                    }
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
                if let Some(t) = first_yield_type(body) {
                    return Some(t);
                }
            }
            Stmt::ExprStmt { expr } => {
                if let Some(t) = expr_first_yield_type(expr) {
                    return Some(t);
                }
            }
            Stmt::Let { value, .. } => {
                if let Some(t) = expr_first_yield_type(value) {
                    return Some(t);
                }
            }
            _ => {}
        }
    }
    None
}

/// 查找 Expr 中第一个 yield 表达式的 IR 类型
pub(crate) fn expr_first_yield_type(expr: &Expr) -> Option<IrType> {
    match &expr.kind {
        ExprKind::BlockExpr { block } => first_yield_type(block),
        ExprKind::GenBuild { block, .. } => first_yield_type(block),
        ExprKind::IfExpr { then, els, .. } => {
            expr_first_yield_type(then).or_else(|| expr_first_yield_type(els))
        }
        ExprKind::Call { callee, args, .. } => expr_first_yield_type(callee)
            .or_else(|| args.iter().find_map(expr_first_yield_type)),
        ExprKind::Lambda { body, .. } => expr_first_yield_type(body),
        _ => None,
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

/// 从 _KwArg 中提取字段值（丢弃字段名，用于位置参数构造）
pub(crate) fn gen_kwarg_value(arg: &Expr, cg: &CodeGen) -> String {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
        if name == "_KwArg" {
            return fields
                .iter()
                .find(|(n, _)| n == "value")
                .map(|(_, v)| cg.gen_expr(v))
                .unwrap_or_default();
        }
    }
    cg.gen_expr(arg)
}

/// 转义 format! 字符串中的独立 { / }（避免被误判为占位符）
/// 已转义的 {{ 或 }} 保持不变
pub(crate) fn escape_format_braces(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    // 已是 {{，保留（显示字面 {）
                    chars.next();
                    out.push_str("{{");
                } else {
                    out.push_str("{{");
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    out.push_str("}}");
                } else {
                    out.push_str("}}");
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Strip `: Type` annotations from closure params (for comprehension closures)
/// "move |x: i64| { ... }" → "move |x| { ... }"
/// "move |acc: i64, x: i64| { ... }" → "move |acc, x| { ... }"（多参数逐个剥离）
pub(crate) fn strip_lambda_type(lambda: &str) -> String {
    // Find `|params|` region and strip each `name: Type` down to `name`
    if let Some(pipe_open) = lambda.find('|') {
        if let Some(rel_close) = lambda[pipe_open + 1..].find('|') {
            let pipe_close = pipe_open + 1 + rel_close;
            let params_part = &lambda[pipe_open + 1..pipe_close];
            // 多参数：按逗号分割，逐个剥离类型注解
            let stripped: String = params_part
                .split(',')
                .map(|p| {
                    let trimmed = p.trim();
                    if trimmed.is_empty() {
                        String::new()
                    } else if let Some(colon) = trimmed.find(':') {
                        trimmed[..colon].trim().to_string()
                    } else {
                        trimmed.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let before = &lambda[..pipe_open + 1];
            let after = &lambda[pipe_close..];
            return format!("{}{}{}", before, stripped, after);
        }
    }
    lambda.to_string()
}

/// Strip type annotations AND add `&` before each param for filter-style closures
/// "move |x: i64| { ... }" → "move |&x| { ... }"
/// "move |x| { ... }" → "move |&x| { ... }"
pub(crate) fn strip_lambda_type_with_ref(lambda: &str) -> String {
    let no_types = strip_lambda_type(lambda);
    // Now add `&` before each parameter name
    // Format: "move |x, y| { ... }" or "|x| { ... }"
    if let Some(pipe_open) = no_types.find('|') {
        if let Some(pipe_close) = no_types[pipe_open + 1..].find('|') {
            let params_part = &no_types[pipe_open + 1..pipe_open + 1 + pipe_close];
            let ref_params: String = params_part
                .split(',')
                .map(|p| {
                    let trimmed = p.trim();
                    if trimmed.is_empty() {
                        String::new()
                    } else {
                        format!("&{}", trimmed)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let before = &no_types[..pipe_open + 1];
            let after = &no_types[pipe_open + 1 + pipe_close..];
            return format!("{}{}{}", before, ref_params, after);
        }
    }
    no_types
}

/// 解析 lambda 字符串为 (参数名列表, body 表达式)
/// "move |&x| { x <= pivot }" → (["x"], "x <= pivot")
/// "|x, y| x + y" → (["x", "y"], "x + y")
/// body 自动剥离最外层花括号。
pub(crate) fn lambda_split(lambda: &str) -> Option<(Vec<String>, String)> {
    let s = lambda.trim();
    // 去掉 "move " 前缀（借用捕获，避免 move 外部变量）
    let s = s.strip_prefix("move ").unwrap_or(s).trim();
    let pipe_open = s.find('|')?;
    let rel_close = s[pipe_open + 1..].find('|')?;
    let pipe_close = pipe_open + 1 + rel_close;
    let params: Vec<String> = s[pipe_open + 1..pipe_close]
        .split(',')
        .map(|p| p.trim().trim_start_matches('&').to_string())
        .collect();
    let after = &s[pipe_close + 1..];
    let body = after.trim();
    let body = body
        .strip_prefix('{')
        .and_then(|b| b.strip_suffix('}'))
        .map(|b| b.trim().to_string())
        .unwrap_or_else(|| body.to_string());
    Some((params, body))
}

/// 将 _KwArg { name, value } 展开为 "field: value"
pub(crate) fn gen_kwarg_field(arg: &Expr, cg: &CodeGen) -> String {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
        if name == "_KwArg" {
            let name_raw = fields
                .iter()
                .find(|(n, _)| n == "name")
                .and_then(|(_, v)| match &v.kind {
                    ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let value = fields.iter().find(|(n, _)| n == "value")
                .map(|(_, v)| {
                    let s = cg.gen_expr(v);
                    // &self 方法内构造时 move self.字段 → 需 .clone()
                    if cg.borrow_self
                        && matches!(&v.kind, ExprKind::FieldAccess { base, .. } if matches!(&base.kind, ExprKind::Var(n) if n == "self")) {
                        format!("{}.clone()", s)
                    } else if matches!(&v.kind, ExprKind::Var(_))
                        && !matches!(&v.kind, ExprKind::Var(ref n)
                            if n == "None" || n == "None_")
                        && !matches!(&v.ty, IrType::Int | IrType::F64 | IrType::Bool)
                    {
                        // 非 Copy 变量实参（String/Vec/Option 等）：struct 构造会 move，
                        // 后续再用该变量报 E0382（combo-defer-guard.lz FileHandle{path: path}）。
                        // None 无需 clone（(None).clone() 报 E0277 Option<_>: Clone）
                        format!("({}).clone()", s)
                    } else {
                        s
                    }
                })
                .unwrap_or_default();
            return format!("{}: {}", name_raw, value);
        }
    }
    cg.gen_expr(arg)
}

/// 提取 _KwArg 的字段名（用于递归字段构造 Box 判断）
pub(crate) fn kwarg_field_name(arg: &Expr) -> Option<String> {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
        if name == "_KwArg" {
            return fields.iter().find(|(n, _)| n == "name").and_then(
                |(_, v)| match &v.kind {
                    ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                    _ => None,
                },
            );
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

/// 递归替换类型中的 `Self` 引用为具体类型（struct 定义内 Self → 自身类型名）。
pub(crate) fn replace_self(ty: &IrType, self_ty: &IrType) -> IrType {
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

/// 扫描块中是否存在对 const 名称的修改
pub(crate) fn scan_const_mutations(
    block: &Block,
    const_names: &std::collections::HashSet<String>,
    mutated: &mut std::collections::HashSet<String>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name, is_mut, .. } => {
                if *is_mut && const_names.contains(name) {
                    mutated.insert(name.clone());
                }
            }
            Stmt::Assign { target, .. } => {
                if let ExprKind::Var(v) = &target.kind {
                    if const_names.contains(v) {
                        mutated.insert(v.clone());
                    }
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                scan_const_mutations(then_branch, const_names, mutated);
                if let Some(ref e) = else_branch {
                    scan_const_mutations(e, const_names, mutated);
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
                scan_const_mutations(body, const_names, mutated);
            }
            Stmt::Block { stmts } => {
                let inner_block = Block {
                    span: Span::unknown(),
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                };
                scan_const_mutations(&inner_block, const_names, mutated);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    scan_const_mutations(&arm.body, const_names, mutated);
                }
            }
            Stmt::ExprStmt { expr } => {
                scan_expr_mutations(expr, const_names, mutated);
            }
            _ => {}
        }
    }
}

/// 递归扫描表达式中对 const 名称的修改（如 +=, -= 等复合赋值）
pub(crate) fn scan_expr_mutations(
    expr: &Expr,
    const_names: &std::collections::HashSet<String>,
    mutated: &mut std::collections::HashSet<String>,
) {
    match &expr.kind {
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_mutations(lhs, const_names, mutated);
            scan_expr_mutations(rhs, const_names, mutated);
        }
        ExprKind::StructCtor { name, fields } if name == "_Walrus" => {
            if let Some((_, bind_expr)) = fields.iter().find(|(n, _)| n == "_bind") {
                if let ExprKind::Var(v) = &bind_expr.kind {
                    if const_names.contains(v) {
                        mutated.insert(v.clone());
                    }
                }
            }
        }
        _ => {}
    }
}

/// 预扫描函数体，收集需要自动加 `mut` 的局部 let 变量名。
/// LZ 源码允许 `let v = ...; v.push(1)`（不写 mut），但生成 Rust 时 push
/// 需要可变绑定（E0596）。这里宽松收集：凡作为方法调用接收者、赋值目标或
/// IndexSet 基的变量名都标为需 mut（over-approximation 安全：多余的 let mut
/// 只产生 unused_mut warning，批测以 -A warnings 忽略；漏标则报 E0596）。
pub(crate) fn scan_auto_mut_locals(block: &Block, out: &mut std::collections::HashSet<String>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name, value, .. } => {
                // 自引用重绑定（`let parts = parts + [p]`）：后续需生成赋值而非
                // shadow-let，因此初始声明必须为 mut
                if expr_mentions_var(value, name) {
                    out.insert(name.clone());
                }
                scan_expr_auto_mut(value, out);
            }
            Stmt::Assign { target, value } => {
                if let ExprKind::Var(v) = &target.kind {
                    out.insert(v.clone());
                }
                scan_expr_auto_mut(target, out);
                scan_expr_auto_mut(value, out);
            }
            Stmt::ExprStmt { expr } => scan_expr_auto_mut(expr, out),
            Stmt::Return { value } => {
                if let Some(v) = value {
                    scan_expr_auto_mut(v, out);
                }
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                scan_expr_auto_mut(cond, out);
                scan_auto_mut_locals(then_branch, out);
                if let Some(e) = else_branch {
                    scan_auto_mut_locals(e, out);
                }
            }
            Stmt::For { iter, body, .. } => {
                scan_expr_auto_mut(iter, out);
                scan_auto_mut_locals(body, out);
            }
            Stmt::While { cond, body, .. } => {
                scan_expr_auto_mut(cond, out);
                scan_auto_mut_locals(body, out);
            }
            Stmt::WhileLet { expr, body, .. } => {
                scan_expr_auto_mut(expr, out);
                scan_auto_mut_locals(body, out);
            }
            Stmt::Match { scrutinee, arms } => {
                scan_expr_auto_mut(scrutinee, out);
                for arm in arms {
                    scan_auto_mut_locals(&arm.body, out);
                }
            }
            Stmt::Raise { value, .. } => {
                scan_expr_auto_mut(value, out);
            }
            Stmt::Assert { cond, .. } => scan_expr_auto_mut(cond, out),
            Stmt::Yield { value } => scan_expr_auto_mut(value, out),
            Stmt::YieldFrom { iter } => scan_expr_auto_mut(iter, out),
            Stmt::Block { stmts } => {
                let inner = Block {
                    span: Span::unknown(),
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                };
                scan_auto_mut_locals(&inner, out);
            }
            Stmt::TryCatch {
                body, catches, ..
            } => {
                scan_auto_mut_locals(body, out);
                for (_, cb) in catches {
                    scan_auto_mut_locals(cb, out);
                }
            }
            Stmt::CheckerBlock { body, .. } => scan_auto_mut_locals(body, out),
            Stmt::Defer { body } => scan_auto_mut_locals(body, out),
            _ => {}
        }
    }
}

/// 递归扫描表达式中的可变使用（方法调用接收者 / 赋值目标 / IndexSet 基）
fn scan_expr_auto_mut(expr: &Expr, out: &mut std::collections::HashSet<String>) {
    match &expr.kind {
        ExprKind::Var(_) | ExprKind::Lit(_) => {}
        ExprKind::Call { callee, args, .. } => {
            // 闭包变量调用（double_then_inc(5)）：FnMut 闭包需 mut 绑定（E0596）
            if let ExprKind::Var(v) = &callee.kind {
                if matches!(callee.ty, IrType::Fn { .. }) {
                    out.insert(v.clone());
                }
            }
            scan_expr_auto_mut(callee, out);
            for a in args {
                scan_expr_auto_mut(a, out);
            }
        }
        ExprKind::MethodCall {
            receiver, args, ..
        } => {
            if let ExprKind::Var(v) = &receiver.kind {
                out.insert(v.clone());
            }
            scan_expr_auto_mut(receiver, out);
            for a in args {
                scan_expr_auto_mut(a, out);
            }
        }
        ExprKind::FieldAccess { base, .. } => scan_expr_auto_mut(base, out),
        ExprKind::IndexGet { base, key } => {
            scan_expr_auto_mut(base, out);
            scan_expr_auto_mut(key, out);
        }
        ExprKind::IndexSet { base, key, value } => {
            if let ExprKind::Var(v) = &base.kind {
                out.insert(v.clone());
            }
            scan_expr_auto_mut(base, out);
            scan_expr_auto_mut(key, out);
            scan_expr_auto_mut(value, out);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_auto_mut(lhs, out);
            scan_expr_auto_mut(rhs, out);
        }
        ExprKind::AssignExpr { target, value } => {
            if let ExprKind::Var(v) = &target.kind {
                out.insert(v.clone());
            }
            scan_expr_auto_mut(target, out);
            scan_expr_auto_mut(value, out);
        }
        ExprKind::UnOp { operand, .. } => scan_expr_auto_mut(operand, out),
        ExprKind::IfExpr { cond, then, els } => {
            scan_expr_auto_mut(cond, out);
            scan_expr_auto_mut(then, out);
            scan_expr_auto_mut(els, out);
        }
        ExprKind::Lambda { body, .. } => scan_expr_auto_mut(body, out),
        ExprKind::StructCtor { fields, .. } => {
            for (_, e) in fields {
                scan_expr_auto_mut(e, out);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                scan_expr_auto_mut(a, out);
            }
        }
        ExprKind::GenExpr { yield_of } => scan_expr_auto_mut(yield_of, out),
        ExprKind::GenBuild { block, .. } => scan_auto_mut_locals(block, out),
        ExprKind::Cast { expr, .. } => scan_expr_auto_mut(expr, out),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                scan_expr_auto_mut(a, out);
            }
        }
        ExprKind::BlockExpr { block } => scan_auto_mut_locals(block, out),
        ExprKind::TupleLit(items) | ExprKind::Tuple(items) | ExprKind::ListLit(items)
        | ExprKind::List(items) => {
            for i in items {
                scan_expr_auto_mut(i, out);
            }
        }
        ExprKind::Dict(entries) => {
            for (k, v) in entries {
                scan_expr_auto_mut(k, out);
                scan_expr_auto_mut(v, out);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                scan_expr_auto_mut(s, out);
            }
            scan_expr_auto_mut(end, out);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            scan_expr_auto_mut(receiver, out);
            scan_expr_auto_mut(callee, out);
            for a in args {
                scan_expr_auto_mut(a, out);
            }
        }
        ExprKind::Paren(inner) => scan_expr_auto_mut(inner, out),
        ExprKind::ImplicitConvert { source, .. } => scan_expr_auto_mut(source, out),
    }
}

/// 判断表达式是否引用了指定变量名（自引用重绑定检测用，如 `let parts = parts + [p]`）
pub(crate) fn expr_mentions_var(expr: &Expr, name: &str) -> bool {
    match &expr.kind {
        ExprKind::Var(v) => v == name,
        ExprKind::Lit(_) => false,
        ExprKind::Call { callee, args, .. } => {
            expr_mentions_var(callee, name) || args.iter().any(|a| expr_mentions_var(a, name))
        }
        ExprKind::MethodCall {
            receiver, args, ..
        } => {
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
        ExprKind::StructCtor { fields, .. } => fields.iter().any(|(_, e)| expr_mentions_var(e, name)),
        ExprKind::EnumCtor { args, .. } => args.iter().any(|a| expr_mentions_var(a, name)),
        ExprKind::GenExpr { yield_of } => expr_mentions_var(yield_of, name),
        ExprKind::GenBuild { .. } => false,
        ExprKind::Cast { expr, .. } => expr_mentions_var(expr, name),
        ExprKind::MagicCall { args, .. } => args.iter().any(|a| expr_mentions_var(a, name)),
        ExprKind::BlockExpr { block } => block.stmts.iter().any(|s| match s {
            Stmt::Let {
                name: n,
                value,
                ..
            } => *n == name && expr_mentions_var(value, name),
            Stmt::ExprStmt { expr } => expr_mentions_var(expr, name),
            Stmt::Return { value } => {
                value.as_ref().map_or(false, |v| expr_mentions_var(v, name))
            }
            _ => false,
        }),
        ExprKind::TupleLit(items) | ExprKind::Tuple(items) | ExprKind::ListLit(items)
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

/// 收集块中的局部 let 绑定名 + 闭包参数（遮蔽名）
pub(crate) fn collect_local_lets(block: &Block, locals: &mut std::collections::HashSet<String>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name, .. } => {
                locals.insert(name.clone());
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_local_lets(then_branch, locals);
                if let Some(e) = else_branch {
                    collect_local_lets(e, locals);
                }
            }
            Stmt::For { var, body, .. } => {
                // 元组解构循环变量 `for (k, v) in ...`：var 形如 "(k, v)"，
                // 需把 k、v 分别收集为局部变量，否则 analyze_global_vars 把
                // 未收集的名字误判为跨函数全局变量（E0530 static mut 冲突）
                collect_for_var_bindings(var, locals);
                collect_local_lets(body, locals);
            }
            Stmt::While { body, .. } => {
                collect_local_lets(body, locals)
            }
            Stmt::WhileLet {
                pattern, body, ..
            } => {
                // while-let 模式绑定（如 Some(item) 中的 item）也是局部变量：
                // 不收集会导致 analyze_global_vars 误判为跨函数全局变量，
                // 生成 static mut item 与 for 绑定冲突（E0530，while_let.lz）
                collect_pattern_bindings(pattern, locals);
                collect_local_lets(body, locals);
            }
            Stmt::Block { stmts } => {
                let inner = Block {
                    span: Span::unknown(),
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                };
                collect_local_lets(&inner, locals);
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    // 收集 match 模式绑定名（遮蔽外部变量）
                    collect_pattern_bindings(&a.pattern, locals);
                    collect_local_lets(&a.body, locals);
                }
            }
            _ => {}
        }
    }
}

/// 收集模式中的绑定名（match 臂的 Ident/Tuple/Struct/Enum 绑定）
pub(crate) fn collect_pattern_bindings(pattern: &Pattern, locals: &mut std::collections::HashSet<String>) {
    match pattern {
        Pattern::Ident(name) => {
            locals.insert(name.clone());
        }
        Pattern::Tuple(ps) => {
            for p in ps {
                collect_pattern_bindings(p, locals);
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields {
                collect_pattern_bindings(p, locals);
            }
        }
        Pattern::Enum { args, .. } => {
            for p in args {
                collect_pattern_bindings(p, locals);
            }
        }
        _ => {}
    }
}

/// 收集 for 循环变量的绑定名：`for x in ...` → x；
/// 元组解构 `for (k, v) in ...` → k、v 分别收集（否则 analyze_global_vars
/// 把未收集的名字误判为跨函数全局变量，生成 static mut 与 for 绑定冲突 E0530）
pub(crate) fn collect_for_var_bindings(var: &str, locals: &mut std::collections::HashSet<String>) {
    let trimmed = var.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        // 元组解构：(k, v) → 分别收集
        let inner = &trimmed[1..trimmed.len() - 1];
        for part in inner.split(',') {
            let name = part.trim();
            if !name.is_empty() && name != "_" {
                locals.insert(name.to_string());
            }
        }
    } else if !trimmed.is_empty() && trimmed != "_" {
        locals.insert(trimmed.to_string());
    }
}

/// 递归收集块中引用的自由变量名（shadow 为遮蔽名集合：闭包参数）
/// 递归收集自由变量引用。in_closure=true 表示当前处于闭包作用域，
/// 此时裸赋值 `x = v` 视为局部声明（加入遮蔽集），而不是外部变量引用。
/// 用于构建块（=: → 闭包）内的赋值，避免被误提升为全局变量。
pub(crate) fn collect_var_refs(
    block: &Block,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
) {
    collect_var_refs_inner(block, shadow, refs, false);
}

pub(crate) fn collect_var_refs_inner(
    block: &Block,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
    in_closure: bool,
) {
    for stmt in &block.stmts {
        collect_stmt_var_refs(stmt, shadow, refs, in_closure);
    }
}

pub(crate) fn collect_stmt_var_refs(
    stmt: &Stmt,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
    in_closure: bool,
) {
    match stmt {
        Stmt::Let { name, value, .. } => {
            // let 声明引入新局部变量，遮蔽同名外部引用
            shadow.insert(name.clone());
            collect_expr_var_refs(value, shadow, refs, in_closure);
        }
        Stmt::Assign { target, value } => {
            // 闭包作用域内：裸赋值视为局部声明（如构建块内 a = 10）
            if in_closure {
                if let ExprKind::Var(n) = &target.kind {
                    shadow.insert(n.clone());
                }
            }
            collect_expr_var_refs(target, shadow, refs, in_closure);
            collect_expr_var_refs(value, shadow, refs, in_closure);
        }
        Stmt::ExprStmt { expr } => collect_expr_var_refs(expr, shadow, refs, in_closure),
        Stmt::Return { value } => {
            if let Some(v) = value {
                collect_expr_var_refs(v, shadow, refs, in_closure);
            }
        }
        Stmt::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr_var_refs(cond, shadow, refs, in_closure);
            collect_var_refs_inner(then_branch, shadow, refs, in_closure);
            if let Some(e) = else_branch {
                collect_var_refs_inner(e, shadow, refs, in_closure);
            }
        }
        Stmt::For {
            var,
            iter,
            guard,
            body,
            ..
        } => {
            // for 循环变量是局部绑定：遮蔽体内引用（`ts[idx]` 的 idx），
            // 否则 idx 被当作自由引用 → analyze_global_vars 误判为跨函数
            // 全局变量，生成 `static mut idx` 与 for 绑定冲突（E0530，
            // 自举试点 bootstrap/work/lz_ir 暴露：多函数 for idx 触发）
            shadow.insert(var.clone());
            collect_expr_var_refs(iter, shadow, refs, in_closure);
            if let Some(g) = guard {
                collect_expr_var_refs(g, shadow, refs, in_closure);
            }
            collect_var_refs_inner(body, shadow, refs, in_closure);
        }
        Stmt::While {
            cond,
            guard,
            body,
            else_body,
        } => {
            collect_expr_var_refs(cond, shadow, refs, in_closure);
            if let Some(g) = guard {
                collect_expr_var_refs(g, shadow, refs, in_closure);
            }
            collect_var_refs_inner(body, shadow, refs, in_closure);
            if let Some(eb) = else_body {
                collect_var_refs_inner(eb, shadow, refs, in_closure);
            }
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            collect_expr_var_refs(expr, shadow, refs, in_closure);
            if let Some(g) = guard {
                collect_expr_var_refs(g, shadow, refs, in_closure);
            }
            collect_var_refs_inner(body, shadow, refs, in_closure);
        }
        Stmt::Block { stmts } => {
            let inner = Block {
                span: Span::unknown(),
                stmts: stmts.clone(),
                ty: IrType::Unit,
            };
            collect_var_refs_inner(&inner, shadow, refs, in_closure);
        }
        Stmt::Match {
            scrutinee, arms, ..
        } => {
            collect_expr_var_refs(scrutinee, shadow, refs, in_closure);
            for arm in arms {
                // match 臂模式绑定是局部变量（`case X(path: p)` 的 p）：遮蔽
                // 臂体内引用，否则被 collect_var_refs 误收为自由引用 →
                // analyze_global_vars 误判为跨函数全局变量，生成 static mut
                // 与 let/参数绑定冲突（E0530，自举试点 bootstrap/work/lz_ir
                // 暴露：多函数同名模式绑定 a/o/es/r 触发）
                collect_pattern_bindings(&arm.pattern, shadow);
                collect_var_refs_inner(&arm.body, shadow, refs, in_closure);
            }
        }
        Stmt::Raise { value } => collect_expr_var_refs(value, shadow, refs, in_closure),
        Stmt::Assert { cond, .. } => collect_expr_var_refs(cond, shadow, refs, in_closure),
        _ => {}
    }
}

pub(crate) fn collect_expr_var_refs(
    expr: &Expr,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
    in_closure: bool,
) {
    match &expr.kind {
        ExprKind::Var(name) => {
            // 被闭包参数/局部变量遮蔽的跳过
            if !shadow.contains(name.as_str()) {
                refs.push(name.clone());
            }
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            collect_expr_var_refs(lhs, shadow, refs, in_closure);
            collect_expr_var_refs(rhs, shadow, refs, in_closure);
        }
        ExprKind::UnOp { operand, .. } => collect_expr_var_refs(operand, shadow, refs, in_closure),
        ExprKind::Call { callee, args, .. } => {
            collect_expr_var_refs(callee, shadow, refs, in_closure);
            for a in args {
                collect_expr_var_refs(a, shadow, refs, in_closure);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_expr_var_refs(receiver, shadow, refs, in_closure);
            for a in args {
                collect_expr_var_refs(a, shadow, refs, in_closure);
            }
        }
        ExprKind::FieldAccess { base, .. } => collect_expr_var_refs(base, shadow, refs, in_closure),
        ExprKind::IndexGet { base, key } => {
            collect_expr_var_refs(base, shadow, refs, in_closure);
            collect_expr_var_refs(key, shadow, refs, in_closure);
        }
        ExprKind::IfExpr { cond, then, els } => {
            collect_expr_var_refs(cond, shadow, refs, in_closure);
            collect_expr_var_refs(then, shadow, refs, in_closure);
            collect_expr_var_refs(els, shadow, refs, in_closure);
        }
        ExprKind::BlockExpr { block } => collect_var_refs_inner(block, shadow, refs, in_closure),
        ExprKind::Lambda { params, body, .. } => {
            // 闭包参数遮蔽：进入闭包体时加入遮蔽集；闭包内裸赋值视为局部声明
            let mut inner_shadow = shadow.clone();
            for p in params {
                inner_shadow.insert(p.name.clone());
            }
            collect_expr_var_refs(body, &mut inner_shadow, refs, true);
        }
        ExprKind::ListLit(elems) => {
            for e in elems {
                collect_expr_var_refs(e, shadow, refs, in_closure);
            }
        }
        ExprKind::TupleLit(elems) => {
            for e in elems {
                collect_expr_var_refs(e, shadow, refs, in_closure);
            }
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                collect_expr_var_refs(v, shadow, refs, in_closure);
            }
        }
        ExprKind::Cast { expr, .. } => collect_expr_var_refs(expr, shadow, refs, in_closure),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                collect_expr_var_refs(a, shadow, refs, in_closure);
            }
        }
        _ => {}
    }
}

/// 从函数体推断全局变量的类型（查找 name = value 赋值，从 value 推断）
pub(crate) fn infer_global_type(block: &Block, name: &str, params: &[Param]) -> IrType {
    for p in params {
        if p.name == name {
            return p.ty.clone();
        }
    }
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name: n, value, .. } if n == name => return value.ty.clone(),
            Stmt::Assign { target, value } => {
                if let ExprKind::Var(v) = &target.kind {
                    if v == name {
                        return value.ty.clone();
                    }
                }
            }
            _ => {}
        }
    }
    IrType::Int
}

/// 将可能包含空格/特殊字符的名称转换为合法 Rust 标识符。
/// 用于测试函数名等场景（如 "string concat" → "string_concat"）。
pub(crate) fn sanitize_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    // Rust 标识符不能以数字开头
    if out.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// 边界感知的标识符替换：将 s 中作为**独立标识符**出现的 `from` 替换为 `to`。
/// 用于 guard 闭包变量重命名（`i % 2 == 0` → `i_owned % 2i64 == 0i64`）：
/// 无脑 replace("i", "i_owned") 会把字面量后缀 `2i64` 里的 i 也替换成
/// i_owned64（invalid suffix `i_owned64`）。
pub(crate) fn replace_ident_boundary(s: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find(from) {
        let before_ok = pos == 0
            || !rest[..pos]
                .chars()
                .next_back()
                .map_or(false, |c| c.is_ascii_alphanumeric() || c == '_');
        let after = &rest[pos + from.len()..];
        let after_ok = after
            .chars()
            .next()
            .map_or(true, |c| !(c.is_ascii_alphanumeric() || c == '_'));
        if before_ok && after_ok {
            out.push_str(&rest[..pos]);
            out.push_str(to);
        } else {
            out.push_str(&rest[..pos + from.len()]);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}
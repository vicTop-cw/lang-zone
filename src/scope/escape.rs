// Lang-Zong 编译器 — scope/escape.rs
// 闭包逃逸分析：检测闭包捕获非 owned 变量，返回编译错误
//
// 规则：return 闭包 或 赋值给外层变量的闭包，只能捕获 owned 变量。
//       其他闭包（立即调用的构建块、函数参数、模块级变量）无限制。

use crate::ast::{Expr, Stmt, Function, Module};

/// 收集函数中标记为 `owned` 的参数名
pub fn owned_params_of(f: &Function) -> Vec<String> {
    f.params.iter()
        .filter(|p| p.is_owned)
        .map(|p| p.name.clone())
        .collect()
}

/// 遍历整个模块，检查所有闭包是否捕获了非 owned 变量。
/// 返回错误列表，每个错误是 (函数名, 变量名) 对。
pub fn validate_module(module: &Module) -> Vec<(String, String)> {
    let mut errors = Vec::new();

    for func in &module.functions {
        let owned = owned_params_of(func);
        validate_stmts(&func.body, &owned, &func.name, &mut errors);
    }

    for imp in &module.impls {
        for method in &imp.methods {
            let owned = owned_params_of(method);
            validate_stmts(&method.body, &owned, &method.name, &mut errors);
        }
    }

    errors
}

fn validate_stmts(stmts: &[Stmt], owned_params: &[String], fn_name: &str, errors: &mut Vec<(String, String)>) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(e) => validate_expr(e, owned_params, fn_name, errors),
            Stmt::Return(Some(e)) => validate_expr_escaping(e, owned_params, fn_name, errors),
            Stmt::Let { value, .. } | Stmt::Const { value, .. } => validate_expr(value, owned_params, fn_name, errors),
            Stmt::Assign { target, value, .. } => {
                validate_expr(target, owned_params, fn_name, errors);
                validate_expr(value, owned_params, fn_name, errors);
            }
            Stmt::While { cond, body, .. } => {
                validate_expr(cond, owned_params, fn_name, errors);
                validate_stmts(body, owned_params, fn_name, errors);
            }
            Stmt::For { iter, body, .. } => {
                validate_expr(iter, owned_params, fn_name, errors);
                validate_stmts(body, owned_params, fn_name, errors);
            }
            Stmt::Guard { cond, else_body, .. } => {
                if let Some(c) = cond { validate_expr(c, owned_params, fn_name, errors); }
                validate_stmts(else_body, owned_params, fn_name, errors);
            }
            Stmt::Defer(body) | Stmt::Comptime(body) => {
                validate_stmts(body, owned_params, fn_name, errors);
            }
            Stmt::Assert { expr, expected, .. } | Stmt::Check { expr, expected, .. } => {
                validate_expr(expr, owned_params, fn_name, errors);
                if let Some(e) = expected { validate_expr(e, owned_params, fn_name, errors); }
            }
            Stmt::Test { body, .. } | Stmt::Suite { tests: body, .. } => {
                validate_stmts(body, owned_params, fn_name, errors);
            }
            _ => {}
        }
    }
}

fn validate_expr(expr: &Expr, owned_params: &[String], fn_name: &str, errors: &mut Vec<(String, String)>) {
    match expr {
        Expr::Closure { body, .. } => {
            // 非逃逸闭包 → 不检查捕获所有权
            validate_expr(body, owned_params, fn_name, errors);
        }
        Expr::ClosureBlock { body, .. } => {
            // 非逃逸多行闭包 → 不检查捕获所有权
            validate_stmts(body, owned_params, fn_name, errors);
        }
        Expr::BuildBlock { body, .. } => {
            validate_stmts(body, owned_params, fn_name, errors);
        }
        Expr::If { cond, then_body, elif_clauses, else_body } => {
            validate_expr(cond, owned_params, fn_name, errors);
            validate_stmts(then_body, owned_params, fn_name, errors);
            for (c, body) in elif_clauses {
                validate_expr(c, owned_params, fn_name, errors);
                validate_stmts(body, owned_params, fn_name, errors);
            }
            if let Some(body) = else_body { validate_stmts(body, owned_params, fn_name, errors); }
        }
        Expr::Match { expr: matched, arms } => {
            validate_expr(matched, owned_params, fn_name, errors);
            for arm in arms { validate_stmts(&arm.body, owned_params, fn_name, errors); }
        }
        Expr::TryCatch { body, catches, else_body, finally_body } => {
            validate_stmts(body, owned_params, fn_name, errors);
            for arm in catches { validate_stmts(&arm.body, owned_params, fn_name, errors); }
            if let Some(b) = else_body { validate_stmts(b, owned_params, fn_name, errors); }
            if let Some(b) = finally_body { validate_stmts(b, owned_params, fn_name, errors); }
        }
        Expr::Binary { left, right, .. } => {
            validate_expr(left, owned_params, fn_name, errors);
            validate_expr(right, owned_params, fn_name, errors);
        }
        Expr::Unary { operand, .. } => validate_expr(operand, owned_params, fn_name, errors),
        Expr::Call { func, args, .. } => {
            validate_expr(func, owned_params, fn_name, errors);
            for a in args { validate_expr(a, owned_params, fn_name, errors); }
        }
        Expr::MethodCall { receiver, args, .. } => {
            validate_expr(receiver, owned_params, fn_name, errors);
            for a in args { validate_expr(a, owned_params, fn_name, errors); }
        }
        Expr::ListLit(elems) | Expr::TupleLit(elems) | Expr::SetLit(elems) => {
            for e in elems { validate_expr(e, owned_params, fn_name, errors); }
        }
        Expr::DictLit(pairs) => {
            for (k, v) in pairs {
                validate_expr(k, owned_params, fn_name, errors);
                validate_expr(v, owned_params, fn_name, errors);
            }
        }
        Expr::FieldAccess { receiver, .. } | Expr::PathAccess { receiver, .. } | Expr::Index { receiver, .. } => {
            validate_expr(receiver, owned_params, fn_name, errors);
        }
        _ => {}
    }
}

/// 逃逸上下文（return）中的表达式——严格检查闭包捕获
fn validate_expr_escaping(expr: &Expr, owned_params: &[String], fn_name: &str, errors: &mut Vec<(String, String)>) {
    match expr {
        Expr::Closure { params, body } => {
            let mut captures = Vec::new();
            collect_expr_idents(body, &mut captures);
            let param_set: std::collections::HashSet<_> = params.iter().collect();
            for name in captures {
                if !param_set.contains(&name) && !owned_params.contains(&name) {
                    errors.push((fn_name.to_string(), name));
                }
            }
        }
        Expr::ClosureBlock { params, body } => {
            let mut captures = Vec::new();
            for s in body {
                collect_stmt_idents(s, &mut captures);
            }
            let param_set: std::collections::HashSet<_> = params.iter().collect();
            for name in captures {
                if !param_set.contains(&name) && !owned_params.contains(&name) {
                    errors.push((fn_name.to_string(), name));
                }
            }
        }
        _ => validate_expr(expr, owned_params, fn_name, errors),
    }
}

/// 从 Expr 中递归收集所有引用的标识符名
fn collect_expr_idents(expr: &Expr, names: &mut Vec<String>) {
    match expr {
        Expr::Ident(n) => names.push(n.clone()),
        Expr::Binary { left, right, .. } => { collect_expr_idents(left, names); collect_expr_idents(right, names); }
        Expr::Unary { operand, .. } => collect_expr_idents(operand, names),
        Expr::Call { func, args, .. } => { collect_expr_idents(func, names); for a in args { collect_expr_idents(a, names); } }
        Expr::MethodCall { receiver, args, .. } => { collect_expr_idents(receiver, names); for a in args { collect_expr_idents(a, names); } }
        Expr::FieldAccess { receiver, .. } | Expr::PathAccess { receiver, .. } | Expr::Index { receiver, .. } => collect_expr_idents(receiver, names),
        Expr::Closure { params, body } => {
            // 闭包参数不是捕获变量——跳过。只收集 body 中的标识符。
            let param_names: Vec<_> = params.clone();
            let mut body_captures = Vec::new();
            collect_expr_idents(body, &mut body_captures);
            // 过滤掉是闭包参数本身的标识符引用
            for name in body_captures {
                if !param_names.contains(&name) {
                    names.push(name);
                }
            }
        }
        Expr::ClosureBlock { params, body } => {
            let param_names: Vec<_> = params.clone();
            let mut body_captures = Vec::new();
            for s in body {
                collect_stmt_idents(s, &mut body_captures);
            }
            for name in body_captures {
                if !param_names.contains(&name) {
                    names.push(name);
                }
            }
        }
        Expr::If { cond, then_body, elif_clauses, else_body } => {
            collect_expr_idents(cond, names);
            for s in then_body { collect_stmt_idents(s, names); }
            for (c, body) in elif_clauses { collect_expr_idents(c, names); for s in body { collect_stmt_idents(s, names); } }
            if let Some(body) = else_body { for s in body { collect_stmt_idents(s, names); } }
        }
        Expr::Match { expr: matched, arms } => {
            collect_expr_idents(matched, names); for arm in arms { for s in &arm.body { collect_stmt_idents(s, names); } }
        }
        Expr::ListLit(elems) | Expr::TupleLit(elems) | Expr::SetLit(elems) => { for e in elems { collect_expr_idents(e, names); } }
        Expr::DictLit(pairs) => { for (k, v) in pairs { collect_expr_idents(k, names); collect_expr_idents(v, names); } }
        _ => {}
    }
}

fn collect_stmt_idents(stmt: &Stmt, names: &mut Vec<String>) {
    match stmt {
        Stmt::Expr(e) => collect_expr_idents(e, names),
        Stmt::Return(Some(e)) | Stmt::Yield(Some(e)) | Stmt::Raise(e) => collect_expr_idents(e, names),
        Stmt::YieldFrom { expr, transform } => {
            collect_expr_idents(expr, names);
            if let Some(f) = transform { collect_expr_idents(f, names); }
        }
        Stmt::Let { value, .. } | Stmt::Const { value, .. } => collect_expr_idents(value, names),
        Stmt::Assign { target, value, .. } => { collect_expr_idents(target, names); collect_expr_idents(value, names); }
        Stmt::While { cond, body, .. } => { collect_expr_idents(cond, names); for s in body { collect_stmt_idents(s, names); } }
        Stmt::For { iter, body, .. } => { collect_expr_idents(iter, names); for s in body { collect_stmt_idents(s, names); } }
        Stmt::Guard { cond, else_body, .. } => { if let Some(c) = cond { collect_expr_idents(c, names); } for s in else_body { collect_stmt_idents(s, names); } }
        Stmt::Defer(body) | Stmt::Comptime(body) => { for s in body { collect_stmt_idents(s, names); } }
        Stmt::Assert { expr, expected, .. } | Stmt::Check { expr, expected, .. } => { collect_expr_idents(expr, names); if let Some(e) = expected { collect_expr_idents(e, names); } }
        _ => {}
    }
}

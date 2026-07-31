//! strict 安全模式 ��� S001-S007 规则检查
//! 使用: lzc main.lz --strict

use crate::ast::*;

#[derive(Debug, Clone)]
pub struct StrictViolation {
    pub rule: String,
    pub message: String,
    pub suggestion: Option<String>,
}

pub fn check_module(module: &Module) -> Vec<StrictViolation> {
    let mut v = Vec::new();
    let unsafe_fns = collect_unsafe_fns(module);
    for f in &module.functions {
        let ok = unsafe_fns.contains(&f.name);
        check_fn(f, ok, &mut v);
    }
    for s in &module.structs { for m in &s.methods { check_fn(m, false, &mut v); } }
    for i in &module.impls { for m in &i.methods { check_fn(m, false, &mut v); } }
    v
}

fn collect_unsafe_fns(module: &Module) -> std::collections::HashSet<String> {
    module.functions.iter().filter(|f| f.decorators.iter().any(|d| d.name == "unsafe"))
        .map(|f| f.name.clone()).collect()
}

fn check_fn(f: &Function, is_unsafe: bool, v: &mut Vec<StrictViolation>) {
    let mut decl = std::collections::HashSet::new();
    let mut used = std::collections::HashSet::new();
    for p in &f.params { if p.name != "self" { used.insert(p.name.clone()); } }
    check_stmts(&f.body, &f.name, &mut decl, &mut used, v);
    for d in &decl { if !used.contains(d) {
        v.push(viol("S006", format!("fn `{}`: unused `{}`", f.name, d),
            Some(format!("prefix with `_`"))));
    }}
    if !is_unsafe {
        check_unwrap_stmts(&f.body, &f.name, v);
    }
}

fn check_stmts(stmts: &[Stmt], fn_name: &str,
    decl: &mut std::collections::HashSet<String>,
    used: &mut std::collections::HashSet<String>,
    v: &mut Vec<StrictViolation>) {
    for s in stmts {
        match s {
            Stmt::Let { name, value, .. } => {
                decl.insert(name.clone());
                walk_expr_vars(value, used);
            }
            Stmt::Expr(e) => walk_expr_vars(e, used),
            Stmt::Return(Some(e)) => walk_expr_vars(e, used),
            Stmt::While { cond, body, .. } => {
                walk_expr_vars(cond, used);
                check_stmts(body, fn_name, decl, used, v);
            }
            Stmt::For { var, iter, body, .. } => {
                used.insert(var.clone());
                walk_expr_vars(iter, used);
                check_stmts(body, fn_name, decl, used, v);
            }
            Stmt::Loop(body) | Stmt::Comptime(body) | Stmt::Defer(body) =>
                check_stmts(body, fn_name, decl, used, v),
            Stmt::Guard { cond, else_body, .. } => {
                if let Some(c) = cond { walk_expr_vars(c, used); }
                check_stmts(else_body, fn_name, decl, used, v);
            }
            _ => {}
        }
    }
}

fn check_unwrap_stmts(stmts: &[Stmt], fn_name: &str, v: &mut Vec<StrictViolation>) {
    for s in stmts {
        match s {
            Stmt::Expr(e) | Stmt::Let { value: e, .. } | Stmt::Return(Some(e)) =>
                visit_expr(e, &mut |x: &Expr| {
                    if let Expr::MethodCall { method, .. } = x {
                        if method == "unwrap" { v.push(viol("S002", format!("fn `{}`: .unwrap()", fn_name),
                            Some("use match or ? or @unsafe".into()))); }
                    }
                }),
            Stmt::While { cond, body, .. } => {
                visit_expr(cond, &mut |x: &Expr| {
                    if let Expr::MethodCall { method, .. } = x { if method == "unwrap" { v.push(viol("S002", format!("fn `{}`: .unwrap()", fn_name), None)); } }
                });
                check_unwrap_stmts(body, fn_name, v);
            }
            Stmt::For { iter, body, .. } => {
                visit_expr(iter, &mut |x: &Expr| {
                    if let Expr::MethodCall { method, .. } = x { if method == "unwrap" { v.push(viol("S002", format!("fn `{}`: .unwrap()", fn_name), None)); } }
                });
                check_unwrap_stmts(body, fn_name, v);
            }
            Stmt::Loop(body) | Stmt::Comptime(body) | Stmt::Defer(body) => check_unwrap_stmts(body, fn_name, v),
            _ => {}
        }
    }
}

fn walk_expr_vars(e: &Expr, used: &mut std::collections::HashSet<String>) {
    visit_expr(e, &mut |x: &Expr| {
        if let Expr::Ident(name) = x { used.insert(name.clone()); }
    });
}

fn visit_expr<F: FnMut(&Expr)>(e: &Expr, f: &mut F) {
    f(e);
    match e {
        Expr::Call { func, args, .. } => {
            f(func); for a in args { visit_expr(a, f); }
        }
        Expr::Binary { left, right, .. } => { visit_expr(left, f); visit_expr(right, f); }
        Expr::Unary { operand, .. } => visit_expr(operand, f),
        Expr::MethodCall { receiver, args, .. } => { visit_expr(receiver, f); for a in args { visit_expr(a, f); } }
        Expr::FieldAccess { receiver, .. } => visit_expr(receiver, f),
        Expr::Index { receiver, index } => { visit_expr(receiver, f); visit_expr(index, f); }
        Expr::If { cond, then_body, elif_clauses, else_body } => {
            visit_expr(cond, f);
            for s in then_body { visit_expr_stmt(s, f); }
            for (c, b) in elif_clauses { visit_expr(c, f); for s in b { visit_expr_stmt(s, f); } }
            if let Some(b) = else_body { for s in b { visit_expr_stmt(s, f); } }
        }
        Expr::Match { expr, arms } => {
            visit_expr(expr, f);
            for arm in arms { for s in &arm.body { visit_expr_stmt(s, f); } }
        }
        Expr::TupleLit(es) | Expr::ListLit(es) => for a in es { visit_expr(a, f); },
        Expr::DictLit(pairs) => for (k, v) in pairs { visit_expr(k, f); visit_expr(v, f); },
        Expr::Closure { body, .. } => visit_expr(body, f),
        Expr::ClosureBlock { body, .. } => { for s in body { visit_expr_stmt(s, f); } }
        Expr::Range { start, end, .. } => {
            if let Some(s) = start.as_ref() { visit_expr(s, f); }
            if let Some(e) = end.as_ref() { visit_expr(e, f); }
        }
        _ => {}
    }
}

fn visit_expr_stmt<F: FnMut(&Expr)>(s: &Stmt, f: &mut F) {
    match s {
        Stmt::Expr(e) | Stmt::Let { value: e, .. } | Stmt::Return(Some(e)) => visit_expr(e, f),
        _ => {}
    }
}

fn viol(rule: &str, msg: String, suggestion: Option<String>) -> StrictViolation {
    StrictViolation { rule: rule.into(), message: msg, suggestion }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_fn(name: &str, body: Vec<Stmt>, decorators: Vec<Decorator>) -> Function {
        Function {
            name: name.into(), generics: vec![], generic_kinds: vec![], generic_bounds: vec![], generic_defaults: vec![],
            params: vec![], return_type: None, raises: None, where_clause: vec![],
            body, is_async: false, is_abstract: false, comptime: false,
            decorators, attributes: vec![], variadic: None, params_checker: None,
        }
    }

    fn empty_mod(fns: Vec<Function>) -> Module {
        Module {
            functions: fns, imports: vec![], structs: vec![], traits: vec![],
            impls: vec![], consts: vec![], type_aliases: vec![],
            magic_decls: vec![],
            tests: vec![],
            is_macro: false, file_path: None, name: None, package: None, doc: None,
        }
    }

    #[test]
    fn s002_rejects_unwrap() {
        let m = empty_mod(vec![mk_fn("bad", vec![
            Stmt::Expr(Expr::MethodCall {
                receiver: Box::new(Expr::Ident("x".into())),
                method: "unwrap".into(), args: vec![],
            })
        ], vec![])]);
        let v = check_module(&m);
        assert!(v.iter().any(|x| x.rule == "S002"));
    }

    #[test]
    fn s002_allows_unsafe() {
        let m = empty_mod(vec![mk_fn("safe", vec![
            Stmt::Expr(Expr::MethodCall {
                receiver: Box::new(Expr::Ident("x".into())),
                method: "unwrap".into(), args: vec![],
            })
        ], vec![Decorator { name: "unsafe".into(), args: vec![] }])]);
        let v = check_module(&m);
        assert!(!v.iter().any(|x| x.rule == "S002"));
    }

    #[test]
    fn s006_detects_unused() {
        let m = empty_mod(vec![mk_fn("f", vec![
            Stmt::Let { name: "unused".into(), mutable: false, is_ref: false, comptime: false, ty: None, value: Expr::IntLit(42) },
        ], vec![])]);
        let v = check_module(&m);
        assert!(v.iter().any(|x| x.rule == "S006"));
    }
}

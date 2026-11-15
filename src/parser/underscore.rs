// Lang-Zone 编译器 — parser/underscore.rs
// `_` 参数标识符的纯 AST 改写工具（自 parser/expr.rs move-only 拆出：参数位 ~: 闭包脱糖，BUG-IR-001）
// 正文逐字节搬运；仅两个被 expr.rs 调用的入口加 pub(super) 可见性标记（Rust 模块可见性硬性要求）

/// 构建块体是否引用 `_` 参数标识符（含嵌套表达式）：
/// `f(~: _ % 2 == 0)` 的 `_` 表示「对每个元素」的隐式参数，
/// 此类 `~:` 是参数位闭包而非调用构建块（BUG-IR-001）
pub(super) fn build_block_uses_underscore(body: &[crate::ast::Stmt]) -> bool {
    for stmt in body {
        if stmt_uses_underscore(stmt) {
            return true;
        }
    }
    false
}

/// 把闭包体中的 `_` 参数标识符统一重命名为 `name`（参数位 `~:` 闭包脱糖用）
pub(super) fn rename_underscore_params(body: &mut [crate::ast::Stmt], name: &str) {
    for stmt in body.iter_mut() {
        rename_stmt_underscore(stmt, name);
    }
}

fn rename_stmt_underscore(stmt: &mut crate::ast::Stmt, name: &str) {
    use crate::ast::Stmt;
    match stmt {
        Stmt::Expr(e) => rename_expr_underscore(e, name),
        Stmt::Let { value, .. } | Stmt::Const { value, .. } => rename_expr_underscore(value, name),
        Stmt::Return(Some(e)) | Stmt::Yield(Some(e)) => rename_expr_underscore(e, name),
        Stmt::While {
            cond,
            body,
            else_body,
            ..
        } => {
            rename_expr_underscore(cond, name);
            rename_underscore_params(body, name);
            if let Some(b) = else_body {
                rename_underscore_params(b, name);
            }
        }
        Stmt::For {
            iter,
            body,
            else_body,
            ..
        } => {
            rename_expr_underscore(iter, name);
            rename_underscore_params(body, name);
            if let Some(b) = else_body {
                rename_underscore_params(b, name);
            }
        }
        _ => {}
    }
}

fn rename_expr_underscore(e: &mut crate::ast::Expr, name: &str) {
    use crate::ast::Expr;
    match e {
        Expr::Ident(n) if n == "_" => *n = name.to_string(),
        Expr::Binary { left, right, .. } => {
            rename_expr_underscore(left, name);
            rename_expr_underscore(right, name);
        }
        Expr::Unary { operand, .. } => rename_expr_underscore(operand, name),
        Expr::Call { func, args, .. } => {
            rename_expr_underscore(func, name);
            for a in args.iter_mut() {
                rename_expr_underscore(a, name);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            rename_expr_underscore(receiver, name);
            for a in args.iter_mut() {
                rename_expr_underscore(a, name);
            }
        }
        Expr::FieldAccess { receiver, .. } | Expr::SafeNav { receiver, .. } => {
            rename_expr_underscore(receiver, name)
        }
        Expr::Index { receiver, index } => {
            rename_expr_underscore(receiver, name);
            rename_expr_underscore(index, name);
        }
        Expr::If {
            cond,
            then_body,
            elif_clauses,
            else_body,
        } => {
            rename_expr_underscore(cond, name);
            rename_underscore_params(then_body, name);
            for (c, b) in elif_clauses.iter_mut() {
                rename_expr_underscore(c, name);
                rename_underscore_params(b, name);
            }
            if let Some(b) = else_body {
                rename_underscore_params(b, name);
            }
        }
        Expr::ListLit(items) | Expr::SetLit(items) | Expr::TupleLit(items) => {
            for it in items.iter_mut() {
                rename_expr_underscore(it, name);
            }
        }
        Expr::DictLit(pairs) => {
            for (k, v) in pairs.iter_mut() {
                rename_expr_underscore(k, name);
                rename_expr_underscore(v, name);
            }
        }
        Expr::Paren(inner)
        | Expr::Try(inner)
        | Expr::Move(inner)
        | Expr::Panic(inner)
        | Expr::Spawn(inner)
        | Expr::Await(inner)
        | Expr::Comptime(inner)
        | Expr::Spread(inner) => rename_expr_underscore(inner, name),
        Expr::KwArg { value, .. } => rename_expr_underscore(value, name),
        Expr::Closure { body, .. } => rename_expr_underscore(body, name),
        Expr::BuildBlock { body, .. } => rename_underscore_params(body, name),
        _ => {}
    }
}

fn stmt_uses_underscore(stmt: &crate::ast::Stmt) -> bool {
    use crate::ast::Stmt;
    match stmt {
        Stmt::Expr(e) => expr_uses_underscore(e),
        Stmt::Let { value, .. } | Stmt::Const { value, .. } => expr_uses_underscore(value),
        Stmt::Return(Some(e)) | Stmt::Yield(Some(e)) => expr_uses_underscore(e),
        Stmt::While {
            cond,
            body,
            else_body,
            ..
        } => {
            expr_uses_underscore(cond)
                || body.iter().any(stmt_uses_underscore)
                || else_body
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .any(stmt_uses_underscore)
        }
        Stmt::For {
            iter,
            body,
            else_body,
            ..
        } => {
            expr_uses_underscore(iter)
                || body.iter().any(stmt_uses_underscore)
                || else_body
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .any(stmt_uses_underscore)
        }
        _ => false,
    }
}

fn expr_uses_underscore(e: &crate::ast::Expr) -> bool {
    use crate::ast::Expr;
    match e {
        Expr::Ident(name) => name == "_",
        Expr::Binary { left, right, .. } => {
            expr_uses_underscore(left) || expr_uses_underscore(right)
        }
        Expr::Unary { operand, .. } => expr_uses_underscore(operand),
        Expr::Call { func, args, .. } => {
            expr_uses_underscore(func) || args.iter().any(expr_uses_underscore)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_uses_underscore(receiver) || args.iter().any(expr_uses_underscore)
        }
        Expr::FieldAccess { receiver, .. } | Expr::SafeNav { receiver, .. } => {
            expr_uses_underscore(receiver)
        }
        Expr::Index { receiver, index } => {
            expr_uses_underscore(receiver) || expr_uses_underscore(index)
        }
        Expr::If {
            cond,
            then_body,
            elif_clauses,
            else_body,
        } => {
            expr_uses_underscore(cond)
                || then_body.iter().any(stmt_uses_underscore)
                || elif_clauses
                    .iter()
                    .any(|(c, b)| expr_uses_underscore(c) || b.iter().any(stmt_uses_underscore))
                || else_body
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .any(stmt_uses_underscore)
        }
        Expr::ListLit(items) | Expr::SetLit(items) | Expr::TupleLit(items) => {
            items.iter().any(expr_uses_underscore)
        }
        Expr::DictLit(pairs) => pairs
            .iter()
            .any(|(k, v)| expr_uses_underscore(k) || expr_uses_underscore(v)),
        Expr::Paren(inner)
        | Expr::Try(inner)
        | Expr::Move(inner)
        | Expr::Panic(inner)
        | Expr::Spawn(inner)
        | Expr::Await(inner)
        | Expr::Comptime(inner)
        | Expr::Spread(inner) => expr_uses_underscore(inner),
        Expr::KwArg { value, .. } => expr_uses_underscore(value),
        Expr::Closure { body, .. } => expr_uses_underscore(body),
        Expr::BuildBlock { body, .. } => body.iter().any(stmt_uses_underscore),
        _ => false,
    }
}

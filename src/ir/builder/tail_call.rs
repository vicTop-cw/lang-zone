// Lang-Zone 编译器 — ir/builder/tail_call.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

// ── @tail_call 尾递归结构验证辅助函数 ─────────────────────────────

/// 检查 block 内所有对 fname 的调用是否都处于尾位置。
/// 规则：递归调用只允许出现在「块末尾语句的末尾表达式」链上；
/// 基本情形分支返回不含递归调用的普通值即可。
pub(crate) fn tail_call_positions_ok(block: &Block, fname: &str) -> bool {
    for (i, stmt) in block.stmts.iter().enumerate() {
        let is_last = i == block.stmts.len() - 1;
        if !tail_stmt_ok(stmt, fname, is_last) {
            return false;
        }
    }
    true
}

pub(crate) fn tail_stmt_ok(stmt: &Stmt, fname: &str, is_last: bool) -> bool {
    match stmt {
        Stmt::ExprStmt { expr } | Stmt::Return { value: Some(expr) } => {
            tail_expr_ok(expr, fname, is_last)
        }
        Stmt::Return { value: None } => true,
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            tail_call_positions_ok(then_branch, fname)
                && else_branch
                    .as_ref()
                    .map_or(true, |b| tail_call_positions_ok(b, fname))
        }
        Stmt::Block { stmts } => {
            for (i, s) in stmts.iter().enumerate() {
                if !tail_stmt_ok(s, fname, i == stmts.len() - 1) {
                    return false;
                }
            }
            true
        }
        Stmt::For {
            body, else_body, ..
        } => {
            tail_call_positions_ok(body, fname)
                && else_body
                    .as_ref()
                    .map_or(true, |b| tail_call_positions_ok(b, fname))
        }
        Stmt::While { body, .. } => tail_call_positions_ok(body, fname),
        Stmt::Match { arms, .. } => arms.iter().all(|a| tail_call_positions_ok(&a.body, fname)),
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            tail_call_positions_ok(body, fname)
                && catches
                    .iter()
                    .all(|(_, b)| tail_call_positions_ok(b, fname))
                && else_body
                    .as_ref()
                    .map_or(true, |b| tail_call_positions_ok(b, fname))
                && finally_body
                    .as_ref()
                    .map_or(true, |b| tail_call_positions_ok(b, fname))
        }
        _ => !stmt_contains_call(stmt, fname),
    }
}

pub(crate) fn tail_expr_ok(expr: &Expr, fname: &str, is_last: bool) -> bool {
    if !is_last {
        return !expr_contains_call(expr, fname);
    }
    match &expr.kind {
        ExprKind::Call { callee, args, .. } => {
            let callee_ok = matches!(&callee.kind, ExprKind::Var(n) if n == fname)
                || !expr_contains_call(callee, fname);
            callee_ok && !args.iter().any(|a| expr_contains_call(a, fname))
        }
        ExprKind::IfExpr { then, els, .. } => {
            tail_expr_ok(then, fname, true) && tail_expr_ok(els, fname, true)
        }
        ExprKind::BlockExpr { block } => tail_call_positions_ok(block, fname),
        _ => !expr_contains_call(expr, fname),
    }
}

pub(crate) fn stmt_contains_call(stmt: &Stmt, fname: &str) -> bool {
    match stmt {
        Stmt::Let { value, .. } => expr_contains_call(value, fname),
        Stmt::Assign { value, .. } => expr_contains_call(value, fname),
        Stmt::Return { value: Some(v) } => expr_contains_call(v, fname),
        Stmt::ExprStmt { expr } => expr_contains_call(expr, fname),
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            stmts_contain_call(&then_branch.stmts, fname)
                || else_branch
                    .as_ref()
                    .map_or(false, |b| stmts_contain_call(&b.stmts, fname))
        }
        Stmt::Block { stmts } => stmts_contain_call(stmts, fname),
        Stmt::For {
            body, else_body, ..
        } => {
            stmts_contain_call(&body.stmts, fname)
                || else_body
                    .as_ref()
                    .map_or(false, |b| stmts_contain_call(&b.stmts, fname))
        }
        Stmt::While { body, .. } => stmts_contain_call(&body.stmts, fname),
        Stmt::Match { arms, .. } => arms
            .iter()
            .any(|a| stmts_contain_call(&a.body.stmts, fname)),
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            stmts_contain_call(&body.stmts, fname)
                || catches
                    .iter()
                    .any(|(_, b)| stmts_contain_call(&b.stmts, fname))
                || else_body
                    .as_ref()
                    .map_or(false, |b| stmts_contain_call(&b.stmts, fname))
                || finally_body
                    .as_ref()
                    .map_or(false, |b| stmts_contain_call(&b.stmts, fname))
        }
        Stmt::Defer { body } => stmts_contain_call(&body.stmts, fname),
        _ => false,
    }
}

pub(crate) fn stmts_contain_call(stmts: &[Stmt], fname: &str) -> bool {
    stmts.iter().any(|s| stmt_contains_call(s, fname))
}

/// 表达式树中是否包含对 fname 的调用（含嵌套子表达式）。
pub(crate) fn expr_contains_call(expr: &Expr, fname: &str) -> bool {
    match &expr.kind {
        ExprKind::Call { callee, args, .. } => {
            matches!(&callee.kind, ExprKind::Var(n) if n == fname)
                || expr_contains_call(callee, fname)
                || args.iter().any(|a| expr_contains_call(a, fname))
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            expr_contains_call(receiver, fname) || args.iter().any(|a| expr_contains_call(a, fname))
        }
        ExprKind::FieldAccess { base, .. } => expr_contains_call(base, fname),
        ExprKind::IndexGet { base, key } => {
            expr_contains_call(base, fname) || expr_contains_call(key, fname)
        }
        ExprKind::IndexSet { base, key, value } => {
            expr_contains_call(base, fname)
                || expr_contains_call(key, fname)
                || expr_contains_call(value, fname)
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            expr_contains_call(lhs, fname) || expr_contains_call(rhs, fname)
        }
        ExprKind::AssignExpr { target, value } => {
            expr_contains_call(target, fname) || expr_contains_call(value, fname)
        }
        ExprKind::UnOp { operand, .. } => expr_contains_call(operand, fname),
        ExprKind::IfExpr { cond, then, els } => {
            expr_contains_call(cond, fname)
                || expr_contains_call(then, fname)
                || expr_contains_call(els, fname)
        }
        ExprKind::Lambda { body, .. } => expr_contains_call(body, fname),
        ExprKind::StructCtor { fields, .. } => {
            fields.iter().any(|(_, e)| expr_contains_call(e, fname))
        }
        ExprKind::EnumCtor { args, .. } => args.iter().any(|a| expr_contains_call(a, fname)),
        ExprKind::GenExpr { yield_of } => expr_contains_call(yield_of, fname),
        ExprKind::GenBuild { callee, block } => {
            callee
                .as_ref()
                .map_or(false, |c| expr_contains_call(c, fname))
                || stmts_contain_call(&block.stmts, fname)
        }
        ExprKind::Cast { expr, .. } => expr_contains_call(expr, fname),
        ExprKind::MagicCall { args, .. } => args.iter().any(|a| expr_contains_call(a, fname)),
        ExprKind::BlockExpr { block } => stmts_contain_call(&block.stmts, fname),
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => es.iter().any(|e| expr_contains_call(e, fname)),
        ExprKind::Spread(e) => expr_contains_call(e, fname),
        ExprKind::Dict(pairs) => pairs
            .iter()
            .any(|(k, v)| expr_contains_call(k, fname) || expr_contains_call(v, fname)),
        ExprKind::Range { start, end, .. } => {
            start
                .as_ref()
                .map_or(false, |s| expr_contains_call(s, fname))
                || expr_contains_call(end, fname)
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            expr_contains_call(receiver, fname)
                || expr_contains_call(callee, fname)
                || args.iter().any(|a| expr_contains_call(a, fname))
        }
        ExprKind::Paren(e) => expr_contains_call(e, fname),
        ExprKind::ImplicitConvert { source, .. } => expr_contains_call(source, fname),
        _ => false,
    }
}

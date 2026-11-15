use super::super::node::*;

pub(crate) type ExprHook<'a> = &'a mut dyn FnMut(&mut Expr);
pub(crate) type StmtHook<'a> = &'a mut dyn FnMut(&mut Stmt);

pub(crate) fn walk_block(block: &mut Block, ef: ExprHook, sf: StmtHook) {
    for s in block.stmts.iter_mut() {
        walk_stmt(s, ef, sf);
    }
}

pub(crate) fn walk_stmt(s: &mut Stmt, ef: ExprHook, sf: StmtHook) {
    match s {
        Stmt::Let { value, .. } => walk_expr(value, ef, sf),
        Stmt::Assign { target, value } => {
            walk_expr(target, ef, sf);
            walk_expr(value, ef, sf);
        }
        Stmt::Return { value } => {
            if let Some(v) = value {
                walk_expr(v, ef, sf);
            }
        }
        Stmt::ExprStmt { expr } => walk_expr(expr, ef, sf),
        Stmt::If { cond, then_branch, else_branch } => {
            walk_expr(cond, ef, sf);
            walk_block(then_branch, ef, sf);
            if let Some(b) = else_branch {
                walk_block(b, ef, sf);
            }
        }
        Stmt::For { iter, guard, body, else_body, .. } => {
            walk_expr(iter, ef, sf);
            if let Some(g) = guard {
                walk_expr(g, ef, sf);
            }
            walk_block(body, ef, sf);
            if let Some(b) = else_body {
                walk_block(b, ef, sf);
            }
        }
        Stmt::While { cond, guard, body, else_body } => {
            walk_expr(cond, ef, sf);
            if let Some(g) = guard {
                walk_expr(g, ef, sf);
            }
            walk_block(body, ef, sf);
            if let Some(b) = else_body {
                walk_block(b, ef, sf);
            }
        }
        Stmt::WhileLet { expr, guard, body, .. } => {
            walk_expr(expr, ef, sf);
            if let Some(g) = guard {
                walk_expr(g, ef, sf);
            }
            walk_block(body, ef, sf);
        }
        Stmt::Match { scrutinee, arms } => {
            walk_expr(scrutinee, ef, sf);
            for arm in arms.iter_mut() {
                if let Some(g) = arm.guard.as_mut() {
                    walk_expr(g, ef, sf);
                }
                walk_block(&mut arm.body, ef, sf);
            }
        }
        Stmt::Raise { value } | Stmt::Yield { value } => walk_expr(value, ef, sf),
        Stmt::Assert { cond, message } => {
            walk_expr(cond, ef, sf);
            if let Some(m) = message {
                walk_expr(m, ef, sf);
            }
        }
        Stmt::YieldFrom { iter } => walk_expr(iter, ef, sf),
        Stmt::BreakLabel { value, .. } => {
            if let Some(v) = value {
                walk_expr(v, ef, sf);
            }
        }
        Stmt::BlockLabel { body, .. } => walk_block(body, ef, sf),
        Stmt::CheckerBlock { body, .. } => walk_block(body, ef, sf),
        Stmt::Defer { body } => walk_block(body, ef, sf),
        Stmt::TryCatch { body, catches, else_body, finally_body } => {
            walk_block(body, ef, sf);
            for (_, b) in catches.iter_mut() {
                walk_block(b, ef, sf);
            }
            if let Some(b) = else_body {
                walk_block(b, ef, sf);
            }
            if let Some(b) = finally_body {
                walk_block(b, ef, sf);
            }
        }
        Stmt::Block { stmts } => {
            for inner in stmts.iter_mut() {
                walk_stmt(inner, ef, sf);
            }
        }
        Stmt::Break | Stmt::Continue | Stmt::Pass | Stmt::TypeAlias { .. } => {}
    }
    sf(s);
}

pub(crate) fn walk_expr(e: &mut Expr, ef: ExprHook, sf: StmtHook) {
    match &mut e.kind {
        ExprKind::Lit(_) | ExprKind::Default | ExprKind::Var(_) => {}
        ExprKind::Call { callee, args, .. } => {
            walk_expr(callee, ef, sf);
            for a in args.iter_mut() {
                walk_expr(a, ef, sf);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            walk_expr(receiver, ef, sf);
            for a in args.iter_mut() {
                walk_expr(a, ef, sf);
            }
        }
        ExprKind::FieldAccess { base, .. } => walk_expr(base, ef, sf),
        ExprKind::IndexGet { base, key } => {
            walk_expr(base, ef, sf);
            walk_expr(key, ef, sf);
        }
        ExprKind::IndexSet { base, key, value } => {
            walk_expr(base, ef, sf);
            walk_expr(key, ef, sf);
            walk_expr(value, ef, sf);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            walk_expr(lhs, ef, sf);
            walk_expr(rhs, ef, sf);
        }
        ExprKind::AssignExpr { target, value } => {
            walk_expr(target, ef, sf);
            walk_expr(value, ef, sf);
        }
        ExprKind::UnOp { operand, .. } => walk_expr(operand, ef, sf),
        ExprKind::IfExpr { cond, then, els } => {
            walk_expr(cond, ef, sf);
            walk_expr(then, ef, sf);
            walk_expr(els, ef, sf);
        }
        ExprKind::Lambda { body, .. } => walk_expr(body, ef, sf),
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields.iter_mut() {
                walk_expr(v, ef, sf);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args.iter_mut() {
                walk_expr(a, ef, sf);
            }
        }
        ExprKind::GenExpr { yield_of } => walk_expr(yield_of, ef, sf),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                walk_expr(c, ef, sf);
            }
            walk_block(block, ef, sf);
        }
        ExprKind::Cast { expr, .. }
        | ExprKind::Spread(expr)
        | ExprKind::Paren(expr) => walk_expr(expr, ef, sf),
        ExprKind::MagicCall { args, .. } => {
            for a in args.iter_mut() {
                walk_expr(a, ef, sf);
            }
        }
        ExprKind::BlockExpr { block } => walk_block(block, ef, sf),
        ExprKind::TupleLit(v)
        | ExprKind::Tuple(v)
        | ExprKind::ListLit(v)
        | ExprKind::List(v) => {
            for x in v.iter_mut() {
                walk_expr(x, ef, sf);
            }
        }
        ExprKind::Dict(pairs) => {
            for (k, v) in pairs.iter_mut() {
                walk_expr(k, ef, sf);
                walk_expr(v, ef, sf);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                walk_expr(s, ef, sf);
            }
            walk_expr(end, ef, sf);
        }
        ExprKind::Pipe { receiver, callee, args } => {
            walk_expr(receiver, ef, sf);
            walk_expr(callee, ef, sf);
            for a in args.iter_mut() {
                walk_expr(a, ef, sf);
            }
        }
        ExprKind::ImplicitConvert { source, .. } => walk_expr(source, ef, sf),
    }
    ef(e);
}
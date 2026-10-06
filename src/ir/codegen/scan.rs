// Lang-Zone 编译器 — ir/codegen/scan.rs
// （由 mod.rs move-only 拆出，逻辑零改动）

use super::types_emit::is_dict_ty;
use super::types_emit::is_list_type;
use super::*;

// ════════════════════════════════════════════════════════════════
// 比较约束传播（comparison trait bound propagation）
//
// 泛型比较算法（如 unique/dedup/sort）需在参与 ==/!=/<,>,<=,>= 的泛型上
// 注入 PartialEq/Eq/Ord，否则 Rust 报 E0277/E0369。约束必须沿调用图
// 传播（如 merge_sort 调用 merge（比较 T）→ merge_sort 的 T 也需 Ord；
// unique 调用内置 contains（要求元素 PartialEq）→ unique 的 a 需 PartialEq）。
// 这里预扫描所有 FnDef 体，经调用图不动点计算得到每个函数的 eq/ord 泛型集合，
// 再由 gen_fn_generics 注入对应 Rust trait bound。
// ════════════════════════════════════════════════════════════════

/// 单个函数的比较约束收集结果
#[derive(Default)]
pub(crate) struct CmpInfo {
    /// 参与 ==/!= 的泛型名（需 PartialEq + Eq）
    pub(crate) eq: HashSet<String>,
    /// 参与 <,>,<=,>= 的泛型名（需 PartialEq + Eq + Ord）
    pub(crate) ord: HashSet<String>,
    /// 调用点：(被调函数名, 调用实参类型中出现的「当前函数泛型」集合)
    pub(crate) calls: Vec<(String, HashSet<String>)>,
}

/// 收集类型中出现的泛型名
pub(crate) fn cmp_collect_gen_names(ty: &IrType, out: &mut HashSet<String>) {
    match ty {
        IrType::Generic(n) => {
            out.insert(n.clone());
        }
        IrType::Named { args, .. } => {
            for a in args {
                cmp_collect_gen_names(a, out);
            }
        }
        IrType::Option(inner) => cmp_collect_gen_names(inner, out),
        IrType::Result { ok, err } => {
            cmp_collect_gen_names(ok, out);
            cmp_collect_gen_names(err, out);
        }
        IrType::Tuple(elems) => {
            for e in elems {
                cmp_collect_gen_names(e, out);
            }
        }
        IrType::Fn { params, ret } => {
            for p in params {
                cmp_collect_gen_names(p, out);
            }
            cmp_collect_gen_names(ret, out);
        }
        IrType::Ref(inner) | IrType::MutRef(inner) => cmp_collect_gen_names(inner, out),
        IrType::Duck { fields } => {
            for (_, t) in fields {
                cmp_collect_gen_names(t, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn cmp_walk_block(block: &Block, my_gen: &HashSet<String>, info: &mut CmpInfo) {
    for s in &block.stmts {
        cmp_walk_stmt(s, my_gen, info);
    }
}

pub(crate) fn cmp_walk_stmt(s: &Stmt, my_gen: &HashSet<String>, info: &mut CmpInfo) {
    match s {
        Stmt::Let { value, .. } => cmp_walk_expr(value, my_gen, info),
        Stmt::Assign { target, value } => {
            cmp_walk_expr(target, my_gen, info);
            cmp_walk_expr(value, my_gen, info);
        }
        Stmt::Return { value } => {
            if let Some(v) = value {
                cmp_walk_expr(v, my_gen, info);
            }
        }
        Stmt::ExprStmt { expr } => cmp_walk_expr(expr, my_gen, info),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            cmp_walk_expr(cond, my_gen, info);
            cmp_walk_block(then_branch, my_gen, info);
            if let Some(b) = else_branch {
                cmp_walk_block(b, my_gen, info);
            }
        }
        Stmt::For {
            var: _,
            iter,
            guard,
            body,
            else_body,
        } => {
            cmp_walk_expr(iter, my_gen, info);
            if let Some(g) = guard {
                cmp_walk_expr(g, my_gen, info);
            }
            cmp_walk_block(body, my_gen, info);
            if let Some(b) = else_body {
                cmp_walk_block(b, my_gen, info);
            }
        }
        Stmt::While {
            cond,
            guard,
            body,
            else_body,
        } => {
            cmp_walk_expr(cond, my_gen, info);
            if let Some(g) = guard {
                cmp_walk_expr(g, my_gen, info);
            }
            cmp_walk_block(body, my_gen, info);
            if let Some(b) = else_body {
                cmp_walk_block(b, my_gen, info);
            }
        }
        Stmt::WhileLet {
            pattern: _,
            expr,
            guard,
            body,
        } => {
            cmp_walk_expr(expr, my_gen, info);
            if let Some(g) = guard {
                cmp_walk_expr(g, my_gen, info);
            }
            cmp_walk_block(body, my_gen, info);
        }
        Stmt::Match { scrutinee, arms } => {
            cmp_walk_expr(scrutinee, my_gen, info);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    cmp_walk_expr(g, my_gen, info);
                }
                cmp_walk_block(&arm.body, my_gen, info);
            }
        }
        Stmt::Raise { value } => cmp_walk_expr(value, my_gen, info),
        Stmt::Assert { cond, message } => {
            cmp_walk_expr(cond, my_gen, info);
            if let Some(m) = message {
                cmp_walk_expr(m, my_gen, info);
            }
        }
        Stmt::Yield { value } => cmp_walk_expr(value, my_gen, info),
        Stmt::YieldFrom { iter } => cmp_walk_expr(iter, my_gen, info),
        Stmt::BreakLabel { value, .. } => {
            if let Some(v) = value {
                cmp_walk_expr(v, my_gen, info);
            }
        }
        Stmt::Block { stmts } => {
            for s2 in stmts {
                cmp_walk_stmt(s2, my_gen, info);
            }
        }
        Stmt::BlockLabel { label: _, body } => cmp_walk_block(body, my_gen, info),
        Stmt::CheckerBlock { body, .. } => cmp_walk_block(body, my_gen, info),
        Stmt::Defer { body } => cmp_walk_block(body, my_gen, info),
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            cmp_walk_block(body, my_gen, info);
            for (_, b) in catches {
                cmp_walk_block(b, my_gen, info);
            }
            if let Some(b) = else_body {
                cmp_walk_block(b, my_gen, info);
            }
            if let Some(b) = finally_body {
                cmp_walk_block(b, my_gen, info);
            }
        }
        Stmt::TypeAlias { .. } | Stmt::Pass | Stmt::Break | Stmt::Continue => {}
    }
}

pub(crate) fn cmp_walk_expr(e: &Expr, my_gen: &HashSet<String>, info: &mut CmpInfo) {
    // 比较运算：收集操作数类型中的泛型名
    if let ExprKind::BinOp { op, lhs, rhs } = &e.kind {
        if op.is_comparison() {
            for t in [&lhs.ty, &rhs.ty] {
                if let IrType::Generic(name) = t {
                    if my_gen.contains(name) {
                        if *op == BinOpKind::Eq || *op == BinOpKind::Neq {
                            info.eq.insert(name.clone());
                        } else {
                            info.ord.insert(name.clone());
                        }
                    }
                }
            }
        }
    }
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            if let ExprKind::Var(name) = &callee.kind {
                let mut flow = HashSet::new();
                for a in args {
                    cmp_collect_gen_names(&a.ty, &mut flow);
                }
                flow.retain(|n| my_gen.contains(n));
                if !flow.is_empty() {
                    info.calls.push((name.clone(), flow));
                }
            }
            cmp_walk_expr(callee, my_gen, info);
            for a in args {
                cmp_walk_expr(a, my_gen, info);
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            let mut flow = HashSet::new();
            cmp_collect_gen_names(&receiver.ty, &mut flow);
            for a in args {
                cmp_collect_gen_names(&a.ty, &mut flow);
            }
            flow.retain(|n| my_gen.contains(n));
            if !flow.is_empty() {
                info.calls.push((method.clone(), flow));
            }
            cmp_walk_expr(receiver, my_gen, info);
            for a in args {
                cmp_walk_expr(a, my_gen, info);
            }
        }
        ExprKind::FieldAccess { base, .. } => cmp_walk_expr(base, my_gen, info),
        ExprKind::IndexGet { base, key } => {
            cmp_walk_expr(base, my_gen, info);
            cmp_walk_expr(key, my_gen, info);
        }
        ExprKind::IndexSet { base, key, value } => {
            cmp_walk_expr(base, my_gen, info);
            cmp_walk_expr(key, my_gen, info);
            cmp_walk_expr(value, my_gen, info);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            cmp_walk_expr(lhs, my_gen, info);
            cmp_walk_expr(rhs, my_gen, info);
        }
        ExprKind::AssignExpr { target, value } => {
            cmp_walk_expr(target, my_gen, info);
            cmp_walk_expr(value, my_gen, info);
        }
        ExprKind::UnOp { operand, .. } => cmp_walk_expr(operand, my_gen, info),
        ExprKind::IfExpr { cond, then, els } => {
            cmp_walk_expr(cond, my_gen, info);
            cmp_walk_expr(then, my_gen, info);
            cmp_walk_expr(els, my_gen, info);
        }
        ExprKind::Lambda { body, .. } => cmp_walk_expr(body, my_gen, info),
        ExprKind::StructCtor { fields, .. } => {
            for (_, f) in fields {
                cmp_walk_expr(f, my_gen, info);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                cmp_walk_expr(a, my_gen, info);
            }
        }
        ExprKind::GenExpr { yield_of } => cmp_walk_expr(yield_of, my_gen, info),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                cmp_walk_expr(c, my_gen, info);
            }
            cmp_walk_block(block, my_gen, info);
        }
        ExprKind::Cast { expr, .. } => cmp_walk_expr(expr, my_gen, info),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                cmp_walk_expr(a, my_gen, info);
            }
        }
        ExprKind::BlockExpr { block } => cmp_walk_block(block, my_gen, info),
        ExprKind::TupleLit(v) | ExprKind::Tuple(v) | ExprKind::ListLit(v) | ExprKind::List(v) => {
            for x in v {
                cmp_walk_expr(x, my_gen, info);
            }
        }
        ExprKind::Spread(x) => cmp_walk_expr(x, my_gen, info),
        ExprKind::Dict(pairs) => {
            for (k, v) in pairs {
                cmp_walk_expr(k, my_gen, info);
                cmp_walk_expr(v, my_gen, info);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                cmp_walk_expr(s, my_gen, info);
            }
            cmp_walk_expr(end, my_gen, info);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            cmp_walk_expr(receiver, my_gen, info);
            cmp_walk_expr(callee, my_gen, info);
            for a in args {
                cmp_walk_expr(a, my_gen, info);
            }
        }
        ExprKind::Paren(x) => cmp_walk_expr(x, my_gen, info),
        ExprKind::ImplicitConvert { source, .. } => cmp_walk_expr(source, my_gen, info),
        ExprKind::Lit(_) | ExprKind::Var(_) | ExprKind::Default => {}
    }
}

// ── 变量引用计数（move 语义修复：被多次使用的非 Copy 变量在调用实参处克隆）──

pub(crate) fn count_vars_expr(e: &Expr, count: &mut HashMap<String, usize>) {
    if let ExprKind::Var(name) = &e.kind {
        *count.entry(name.clone()).or_insert(0) += 1;
    }
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            count_vars_expr(callee, count);
            for a in args {
                count_vars_expr(a, count);
            }
        }
        ExprKind::MethodCall {
            receiver,
            method: _,
            args,
        } => {
            count_vars_expr(receiver, count);
            for a in args {
                count_vars_expr(a, count);
            }
        }
        ExprKind::FieldAccess { base, .. } => count_vars_expr(base, count),
        ExprKind::IndexGet { base, key } => {
            count_vars_expr(base, count);
            count_vars_expr(key, count);
        }
        ExprKind::IndexSet { base, key, value } => {
            count_vars_expr(base, count);
            count_vars_expr(key, count);
            count_vars_expr(value, count);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            count_vars_expr(lhs, count);
            count_vars_expr(rhs, count);
        }
        ExprKind::AssignExpr { target, value, .. } => {
            count_vars_expr(target, count);
            count_vars_expr(value, count);
        }
        ExprKind::UnOp { operand, .. } => count_vars_expr(operand, count),
        ExprKind::IfExpr { cond, then, els } => {
            count_vars_expr(cond, count);
            count_vars_expr(then, count);
            count_vars_expr(els, count);
        }
        ExprKind::Lambda { body, .. } => count_vars_expr(body, count),
        ExprKind::StructCtor { fields, .. } => {
            for (_, f) in fields {
                count_vars_expr(f, count);
            }
        }
        ExprKind::EnumCtor {
            enum_name: _,
            variant: _,
            args,
        } => {
            for a in args {
                count_vars_expr(a, count);
            }
        }
        ExprKind::GenExpr { yield_of } => count_vars_expr(yield_of, count),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                count_vars_expr(c, count);
            }
            count_vars_block(block, count);
        }
        ExprKind::Cast { expr, .. } => count_vars_expr(expr, count),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                count_vars_expr(a, count);
            }
        }
        ExprKind::BlockExpr { block } => count_vars_block(block, count),
        ExprKind::TupleLit(v) | ExprKind::Tuple(v) | ExprKind::ListLit(v) | ExprKind::List(v) => {
            for x in v {
                count_vars_expr(x, count);
            }
        }
        ExprKind::Spread(x) => count_vars_expr(x, count),
        ExprKind::Dict(pairs) => {
            for (k, v) in pairs {
                count_vars_expr(k, count);
                count_vars_expr(v, count);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                count_vars_expr(s, count);
            }
            count_vars_expr(end, count);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            count_vars_expr(receiver, count);
            count_vars_expr(callee, count);
            for a in args {
                count_vars_expr(a, count);
            }
        }
        ExprKind::Paren(x) => count_vars_expr(x, count),
        ExprKind::ImplicitConvert { source, .. } => count_vars_expr(source, count),
        ExprKind::Lit(_) | ExprKind::Var(_) | ExprKind::Default => {}
    }
}

pub(crate) fn count_vars_stmt(s: &Stmt, count: &mut HashMap<String, usize>) {
    match s {
        Stmt::Let { value, .. } => count_vars_expr(value, count),
        Stmt::Assign { target, value } => {
            count_vars_expr(target, count);
            count_vars_expr(value, count);
        }
        Stmt::Return { value } => {
            if let Some(v) = value {
                count_vars_expr(v, count);
            }
        }
        Stmt::ExprStmt { expr } => count_vars_expr(expr, count),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            count_vars_expr(cond, count);
            count_vars_block(then_branch, count);
            if let Some(b) = else_branch {
                count_vars_block(b, count);
            }
        }
        Stmt::For {
            var: _,
            iter,
            guard,
            body,
            else_body,
        } => {
            count_vars_expr(iter, count);
            if let Some(g) = guard {
                count_vars_expr(g, count);
            }
            count_vars_block(body, count);
            if let Some(b) = else_body {
                count_vars_block(b, count);
            }
        }
        Stmt::While {
            cond,
            guard,
            body,
            else_body,
        } => {
            count_vars_expr(cond, count);
            if let Some(g) = guard {
                count_vars_expr(g, count);
            }
            count_vars_block(body, count);
            if let Some(b) = else_body {
                count_vars_block(b, count);
            }
        }
        Stmt::WhileLet {
            pattern: _,
            expr,
            guard,
            body,
        } => {
            count_vars_expr(expr, count);
            if let Some(g) = guard {
                count_vars_expr(g, count);
            }
            count_vars_block(body, count);
        }
        Stmt::Match { scrutinee, arms } => {
            count_vars_expr(scrutinee, count);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    count_vars_expr(g, count);
                }
                count_vars_block(&arm.body, count);
            }
        }
        Stmt::Raise { value } => count_vars_expr(value, count),
        Stmt::Assert { cond, message } => {
            count_vars_expr(cond, count);
            if let Some(m) = message {
                count_vars_expr(m, count);
            }
        }
        Stmt::Yield { value } => count_vars_expr(value, count),
        Stmt::YieldFrom { iter } => count_vars_expr(iter, count),
        Stmt::BreakLabel { value, .. } => {
            if let Some(v) = value {
                count_vars_expr(v, count);
            }
        }
        Stmt::Block { stmts } => {
            for s2 in stmts {
                count_vars_stmt(s2, count);
            }
        }
        Stmt::BlockLabel { body, .. } => count_vars_block(body, count),
        Stmt::CheckerBlock { body, .. } => count_vars_block(body, count),
        Stmt::Defer { body } => count_vars_block(body, count),
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            count_vars_block(body, count);
            for (_, b) in catches {
                count_vars_block(b, count);
            }
            if let Some(b) = else_body {
                count_vars_block(b, count);
            }
            if let Some(b) = finally_body {
                count_vars_block(b, count);
            }
        }
        Stmt::TypeAlias { .. } | Stmt::Pass | Stmt::Break | Stmt::Continue => {}
    }
}

pub(crate) fn count_vars_block(block: &Block, count: &mut HashMap<String, usize>) {
    for s in &block.stmts {
        count_vars_stmt(s, count);
    }
}

/// 递归检查语句块是否使用 Dict（类型标注或 Dict 字面量）
pub(crate) fn block_uses_dict(block: &Block) -> bool {
    block.stmts.iter().any(stmt_uses_dict)
}

pub(crate) fn stmt_uses_dict(s: &Stmt) -> bool {
    match s {
        Stmt::Let { ty, value, .. } => is_dict_ty(ty) || expr_uses_dict(value),
        Stmt::Assign { target, value } => expr_uses_dict(target) || expr_uses_dict(value),
        Stmt::Return { value } => value.as_ref().map(expr_uses_dict).unwrap_or(false),
        Stmt::ExprStmt { expr } => expr_uses_dict(expr),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            expr_uses_dict(cond)
                || block_uses_dict(then_branch)
                || else_branch.as_ref().map(block_uses_dict).unwrap_or(false)
        }
        Stmt::For {
            iter,
            body,
            else_body,
            ..
        } => {
            expr_uses_dict(iter)
                || block_uses_dict(body)
                || else_body.as_ref().map(block_uses_dict).unwrap_or(false)
        }
        Stmt::While {
            cond,
            body,
            else_body,
            ..
        } => {
            expr_uses_dict(cond)
                || block_uses_dict(body)
                || else_body.as_ref().map(block_uses_dict).unwrap_or(false)
        }
        Stmt::WhileLet { expr, body, .. } => expr_uses_dict(expr) || block_uses_dict(body),
        Stmt::Match { scrutinee, arms } => {
            expr_uses_dict(scrutinee)
                || arms.iter().any(|a| {
                    a.guard.as_ref().map(expr_uses_dict).unwrap_or(false)
                        || block_uses_dict(&a.body)
                })
        }
        Stmt::Raise { value } => expr_uses_dict(value),
        Stmt::Assert { cond, message } => {
            expr_uses_dict(cond) || message.as_ref().map(expr_uses_dict).unwrap_or(false)
        }
        Stmt::Yield { value } => expr_uses_dict(value),
        Stmt::YieldFrom { iter } => expr_uses_dict(iter),
        Stmt::Break | Stmt::Continue | Stmt::Pass => false,
        Stmt::BreakLabel { value, .. } => value.as_ref().map(expr_uses_dict).unwrap_or(false),
        Stmt::BlockLabel { body, .. } => block_uses_dict(body),
        Stmt::CheckerBlock { body, .. } => block_uses_dict(body),
        Stmt::Defer { body } => block_uses_dict(body),
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            block_uses_dict(body)
                || catches.iter().any(|(_, b)| block_uses_dict(b))
                || else_body.as_ref().map(block_uses_dict).unwrap_or(false)
                || finally_body.as_ref().map(block_uses_dict).unwrap_or(false)
        }
        Stmt::Block { stmts } => stmts.iter().any(stmt_uses_dict),
        Stmt::TypeAlias { .. } => false,
    }
}

pub(crate) fn expr_uses_dict(e: &Expr) -> bool {
    is_dict_ty(&e.ty)
        || match &e.kind {
            ExprKind::Dict(_) => true,
            ExprKind::Lit(_) | ExprKind::Var(_) | ExprKind::Default => false,
            ExprKind::BinOp { lhs, rhs, .. } => expr_uses_dict(lhs) || expr_uses_dict(rhs),
            ExprKind::UnOp { operand, .. } => expr_uses_dict(operand),
            ExprKind::Call { callee, args, .. } => {
                expr_uses_dict(callee) || args.iter().any(expr_uses_dict)
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                expr_uses_dict(receiver) || args.iter().any(expr_uses_dict)
            }
            ExprKind::FieldAccess { base, .. } => expr_uses_dict(base),
            ExprKind::IndexGet { base, key } => expr_uses_dict(base) || expr_uses_dict(key),
            ExprKind::IndexSet { base, key, value } => {
                expr_uses_dict(base) || expr_uses_dict(key) || expr_uses_dict(value)
            }
            ExprKind::AssignExpr { target, value } => {
                expr_uses_dict(target) || expr_uses_dict(value)
            }
            ExprKind::IfExpr { cond, then, els } => {
                expr_uses_dict(cond) || expr_uses_dict(then) || expr_uses_dict(els)
            }
            ExprKind::Lambda { body, .. } => expr_uses_dict(body),
            ExprKind::StructCtor { fields, .. } => fields.iter().any(|(_, f)| expr_uses_dict(f)),
            ExprKind::EnumCtor { args, .. } => args.iter().any(expr_uses_dict),
            ExprKind::GenExpr { yield_of } => expr_uses_dict(yield_of),
            ExprKind::GenBuild { callee, block } => {
                callee.as_ref().map(|c| expr_uses_dict(c)).unwrap_or(false)
                    || block_uses_dict(block)
            }
            ExprKind::Cast { expr, .. } => expr_uses_dict(expr),
            ExprKind::MagicCall { args, .. } => args.iter().any(expr_uses_dict),
            ExprKind::BlockExpr { block } => block_uses_dict(block),
            ExprKind::TupleLit(v)
            | ExprKind::Tuple(v)
            | ExprKind::ListLit(v)
            | ExprKind::List(v) => v.iter().any(expr_uses_dict),
            ExprKind::Spread(x) => expr_uses_dict(x),
            ExprKind::Range { start, end, .. } => {
                start.as_ref().map(|s| expr_uses_dict(s)).unwrap_or(false) || expr_uses_dict(end)
            }
            ExprKind::Pipe {
                receiver,
                callee,
                args,
            } => {
                expr_uses_dict(receiver)
                    || expr_uses_dict(callee)
                    || args.iter().any(expr_uses_dict)
            }
            ExprKind::Paren(x) => expr_uses_dict(x),
            ExprKind::ImplicitConvert { source, .. } => expr_uses_dict(source),
        }
}

// ── f-string 未绑定插值预扫描（lit_probe 等探测用例）──

/// 提取 f-string 中纯变量插值名（`f"hello {name}"` → "name"）；
/// 复杂插值（len(x)、a.b 等）不提取
pub(crate) fn fstring_var_interps(s: &str, out: &mut std::collections::HashSet<String>) {
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                continue;
            }
            let mut expr = String::new();
            let mut depth = 0usize;
            while let Some(&ec) = chars.peek() {
                match ec {
                    '}' if depth == 0 => {
                        chars.next();
                        break;
                    }
                    '{' => {
                        depth += 1;
                        expr.push(ec);
                        chars.next();
                    }
                    '}' => {
                        depth -= 1;
                        expr.push(ec);
                        chars.next();
                    }
                    _ => {
                        expr.push(ec);
                        chars.next();
                    }
                }
            }
            let t = expr.trim();
            // 纯标识符（含格式说明符前的冒号取前段）
            let ident = t.split(':').next().unwrap_or(t).trim();
            if !ident.is_empty()
                && ident
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                && !ident.chars().next().unwrap().is_ascii_digit()
            {
                out.insert(ident.to_string());
            }
        }
    }
}

/// 扫描表达式树，收集 f-string 插值变量引用
pub(crate) fn scan_expr_fstrings(e: &Expr, used: &mut std::collections::HashSet<String>) {
    match &e.kind {
        ExprKind::Lit(LitKind::FStr(s)) => fstring_var_interps(s, used),
        ExprKind::Lit(_) => {}
        ExprKind::Var(_) => {}
        ExprKind::Call { callee, args, .. } => {
            scan_expr_fstrings(callee, used);
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            scan_expr_fstrings(receiver, used);
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::FieldAccess { base, .. } => scan_expr_fstrings(base, used),
        ExprKind::IndexGet { base, key } => {
            scan_expr_fstrings(base, used);
            scan_expr_fstrings(key, used);
        }
        ExprKind::IndexSet { base, key, value } => {
            scan_expr_fstrings(base, used);
            scan_expr_fstrings(key, used);
            scan_expr_fstrings(value, used);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_fstrings(lhs, used);
            scan_expr_fstrings(rhs, used);
        }
        ExprKind::AssignExpr { target, value } => {
            scan_expr_fstrings(target, used);
            scan_expr_fstrings(value, used);
        }
        ExprKind::UnOp { operand, .. } => scan_expr_fstrings(operand, used),
        ExprKind::IfExpr { cond, then, els } => {
            scan_expr_fstrings(cond, used);
            scan_expr_fstrings(then, used);
            scan_expr_fstrings(els, used);
        }
        ExprKind::Lambda { body, .. } => scan_expr_fstrings(body, used),
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                scan_expr_fstrings(v, used);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::GenExpr { yield_of } => scan_expr_fstrings(yield_of, used),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                scan_expr_fstrings(c, used);
            }
            scan_block_fstrings(block, &mut std::collections::HashSet::new(), used);
        }
        ExprKind::Cast { expr, .. } => scan_expr_fstrings(expr, used),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::BlockExpr { block } => {
            scan_block_fstrings(block, &mut std::collections::HashSet::new(), used);
        }
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => {
            for a in es {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::Dict(kvs) => {
            for (k, v) in kvs {
                scan_expr_fstrings(k, used);
                scan_expr_fstrings(v, used);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                scan_expr_fstrings(s, used);
            }
            scan_expr_fstrings(end, used);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            scan_expr_fstrings(receiver, used);
            scan_expr_fstrings(callee, used);
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        _ => {}
    }
}

/// 收集模式中的绑定名（match 臂 / while let / catch 模式）到 bound：
/// f-string 插值若引用这些名字，不得降级为空串（lib_pattern 实测：
/// case Color.RGB(r, g, b) 臂体内 f"#{r}{g}{b}" 曾被降级为 ""）
pub(crate) fn scan_pattern_bindings(pat: &Pattern, bound: &mut std::collections::HashSet<String>) {
    match pat {
        Pattern::Ident(n) | Pattern::RefMutIdent(n) => {
            bound.insert(n.clone());
        }
        Pattern::Tuple(ps) | Pattern::List(ps) => {
            for p in ps {
                scan_pattern_bindings(p, bound);
            }
        }
        Pattern::Dict(kvps) => {
            for (_, p) in kvps {
                scan_pattern_bindings(p, bound);
            }
        }
        Pattern::Rest(Some(n)) => {
            bound.insert(n.clone());
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields {
                scan_pattern_bindings(p, bound);
            }
        }
        Pattern::Enum { args, .. } => {
            for p in args {
                scan_pattern_bindings(p, bound);
            }
        }
        _ => {}
    }
}

/// 扫描块：let 名收集到 bound，f-string 插值收集到 used
pub(crate) fn scan_block_fstrings(
    b: &Block,
    bound: &mut std::collections::HashSet<String>,
    used: &mut std::collections::HashSet<String>,
) {
    for st in &b.stmts {
        match st {
            Stmt::Let { name, value, .. } => {
                bound.insert(name.clone());
                scan_expr_fstrings(value, used);
            }
            Stmt::Assign { target, value } => {
                scan_expr_fstrings(target, used);
                scan_expr_fstrings(value, used);
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    scan_expr_fstrings(v, used);
                }
            }
            Stmt::ExprStmt { expr } => scan_expr_fstrings(expr, used),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                scan_expr_fstrings(cond, used);
                scan_block_fstrings(then_branch, bound, used);
                if let Some(eb) = else_branch {
                    scan_block_fstrings(eb, bound, used);
                }
            }
            Stmt::For {
                var,
                iter,
                guard,
                body,
                else_body,
            } => {
                bound.insert(var.clone());
                scan_expr_fstrings(iter, used);
                if let Some(g) = guard {
                    scan_expr_fstrings(g, used);
                }
                scan_block_fstrings(body, bound, used);
                if let Some(eb) = else_body {
                    scan_block_fstrings(eb, bound, used);
                }
            }
            Stmt::While {
                cond,
                guard,
                body,
                else_body,
            } => {
                scan_expr_fstrings(cond, used);
                if let Some(g) = guard {
                    scan_expr_fstrings(g, used);
                }
                scan_block_fstrings(body, bound, used);
                if let Some(eb) = else_body {
                    scan_block_fstrings(eb, bound, used);
                }
            }
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                scan_pattern_bindings(pattern, bound);
                scan_expr_fstrings(expr, used);
                if let Some(g) = guard {
                    scan_expr_fstrings(g, used);
                }
                scan_block_fstrings(body, bound, used);
            }
            Stmt::Match { scrutinee, arms } => {
                scan_expr_fstrings(scrutinee, used);
                for arm in arms {
                    // 臂模式绑定（含枚举/元组/结构子模式）也是有效绑定
                    scan_pattern_bindings(&arm.pattern, bound);
                    if let Some(g) = &arm.guard {
                        scan_expr_fstrings(g, used);
                    }
                    scan_block_fstrings(&arm.body, bound, used);
                }
            }
            Stmt::Raise { value } => scan_expr_fstrings(value, used),
            Stmt::Assert { cond, message } => {
                scan_expr_fstrings(cond, used);
                if let Some(m) = message {
                    scan_expr_fstrings(m, used);
                }
            }
            Stmt::Yield { value } => scan_expr_fstrings(value, used),
            Stmt::YieldFrom { iter } => scan_expr_fstrings(iter, used),
            Stmt::BreakLabel { value, .. } => {
                if let Some(v) = value {
                    scan_expr_fstrings(v, used);
                }
            }
            Stmt::BlockLabel { body, .. } => scan_block_fstrings(body, bound, used),
            Stmt::CheckerBlock { body, .. } => scan_block_fstrings(body, bound, used),
            Stmt::Defer { body } => scan_block_fstrings(body, bound, used),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                scan_block_fstrings(body, bound, used);
                for (cpat, cb) in catches {
                    if let Some(p) = cpat {
                        scan_pattern_bindings(p, bound);
                    }
                    scan_block_fstrings(cb, bound, used);
                }
                if let Some(eb) = else_body {
                    scan_block_fstrings(eb, bound, used);
                }
                if let Some(fb) = finally_body {
                    scan_block_fstrings(fb, bound, used);
                }
            }
            Stmt::Block { stmts } => {
                let tmp = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                    span: Span::unknown(),
                };
                scan_block_fstrings(&tmp, bound, used);
            }
            _ => {}
        }
    }
}

/// 收集表达式树中所有 lambda 参数名（f-string 插值若引用 lambda 参数，不得降级为空串）
pub(crate) fn collect_lambda_params_expr(e: &Expr, bound: &mut std::collections::HashSet<String>) {
    match &e.kind {
        ExprKind::Lambda { params, body, .. } => {
            for p in params {
                bound.insert(p.name.clone());
            }
            collect_lambda_params_expr(body, bound);
        }
        ExprKind::Call { callee, args, .. } => {
            collect_lambda_params_expr(callee, bound);
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_lambda_params_expr(receiver, bound);
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::FieldAccess { base, .. } => collect_lambda_params_expr(base, bound),
        ExprKind::IndexGet { base, key } => {
            collect_lambda_params_expr(base, bound);
            collect_lambda_params_expr(key, bound);
        }
        ExprKind::IndexSet { base, key, value } => {
            collect_lambda_params_expr(base, bound);
            collect_lambda_params_expr(key, bound);
            collect_lambda_params_expr(value, bound);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            collect_lambda_params_expr(lhs, bound);
            collect_lambda_params_expr(rhs, bound);
        }
        ExprKind::AssignExpr { target, value } => {
            collect_lambda_params_expr(target, bound);
            collect_lambda_params_expr(value, bound);
        }
        ExprKind::UnOp { operand, .. } => collect_lambda_params_expr(operand, bound),
        ExprKind::IfExpr { cond, then, els } => {
            collect_lambda_params_expr(cond, bound);
            collect_lambda_params_expr(then, bound);
            collect_lambda_params_expr(els, bound);
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                collect_lambda_params_expr(v, bound);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::GenExpr { yield_of } => collect_lambda_params_expr(yield_of, bound),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                collect_lambda_params_expr(c, bound);
            }
            collect_lambda_params_block(block, bound);
        }
        ExprKind::Cast { expr, .. } => collect_lambda_params_expr(expr, bound),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::BlockExpr { block } => collect_lambda_params_block(block, bound),
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => {
            for a in es {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::Dict(kvs) => {
            for (k, v) in kvs {
                collect_lambda_params_expr(k, bound);
                collect_lambda_params_expr(v, bound);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_lambda_params_expr(s, bound);
            }
            collect_lambda_params_expr(end, bound);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            collect_lambda_params_expr(receiver, bound);
            collect_lambda_params_expr(callee, bound);
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        _ => {}
    }
}

/// 收集 block 中所有 lambda 参数名
pub(crate) fn collect_lambda_params_block(
    b: &Block,
    bound: &mut std::collections::HashSet<String>,
) {
    for st in &b.stmts {
        match st {
            Stmt::Let { value, .. } => collect_lambda_params_expr(value, bound),
            Stmt::Assign { target, value } => {
                collect_lambda_params_expr(target, bound);
                collect_lambda_params_expr(value, bound);
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    collect_lambda_params_expr(v, bound);
                }
            }
            Stmt::ExprStmt { expr } => collect_lambda_params_expr(expr, bound),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_lambda_params_expr(cond, bound);
                collect_lambda_params_block(then_branch, bound);
                if let Some(eb) = else_branch {
                    collect_lambda_params_block(eb, bound);
                }
            }
            Stmt::For {
                iter,
                body,
                else_body,
                ..
            } => {
                collect_lambda_params_expr(iter, bound);
                collect_lambda_params_block(body, bound);
                if let Some(eb) = else_body {
                    collect_lambda_params_block(eb, bound);
                }
            }
            Stmt::While {
                cond,
                body,
                else_body,
                ..
            } => {
                collect_lambda_params_expr(cond, bound);
                collect_lambda_params_block(body, bound);
                if let Some(eb) = else_body {
                    collect_lambda_params_block(eb, bound);
                }
            }
            Stmt::WhileLet { expr, body, .. } => {
                collect_lambda_params_expr(expr, bound);
                collect_lambda_params_block(body, bound);
            }
            Stmt::Match { scrutinee, arms } => {
                collect_lambda_params_expr(scrutinee, bound);
                for arm in arms {
                    collect_lambda_params_block(&arm.body, bound);
                }
            }
            Stmt::Raise { value } => collect_lambda_params_expr(value, bound),
            Stmt::Assert { cond, message } => {
                collect_lambda_params_expr(cond, bound);
                if let Some(m) = message {
                    collect_lambda_params_expr(m, bound);
                }
            }
            Stmt::Yield { value } => collect_lambda_params_expr(value, bound),
            Stmt::YieldFrom { iter } => collect_lambda_params_expr(iter, bound),
            Stmt::BlockLabel { body, .. } => collect_lambda_params_block(body, bound),
            Stmt::CheckerBlock { body, .. } => collect_lambda_params_block(body, bound),
            Stmt::Defer { body } => collect_lambda_params_block(body, bound),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                collect_lambda_params_block(body, bound);
                for (_, cb) in catches {
                    collect_lambda_params_block(cb, bound);
                }
                if let Some(eb) = else_body {
                    collect_lambda_params_block(eb, bound);
                }
                if let Some(fb) = finally_body {
                    collect_lambda_params_block(fb, bound);
                }
            }
            Stmt::Block { stmts } => {
                let tmp = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                    span: Span::unknown(),
                };
                collect_lambda_params_block(&tmp, bound);
            }
            _ => {}
        }
    }
}

/// 预扫描 IR 模块，返回 f-string 插值中未绑定的变量名集合
pub(crate) fn collect_unbound_fstring_vars(module: &IrModule) -> std::collections::HashSet<String> {
    let mut bound: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 全局绑定：fn / const / struct / enum / trait / checker / alias / duck 名 + 函数参数
    for item in &module.items {
        match item {
            Item::FnDef(f) => {
                bound.insert(f.name.clone());
                for p in &f.params {
                    bound.insert(p.name.clone());
                }
                collect_lambda_params_block(&f.body, &mut bound);
            }
            Item::Const(c) => {
                bound.insert(c.name.clone());
                collect_lambda_params_expr(&c.value, &mut bound);
            }
            Item::StructDef(s) => {
                bound.insert(s.name.clone());
                for m in &s.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params {
                        bound.insert(p.name.clone());
                    }
                    collect_lambda_params_block(&m.body, &mut bound);
                }
            }
            Item::EnumDef(e) => {
                bound.insert(e.name.clone());
                for m in &e.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params {
                        bound.insert(p.name.clone());
                    }
                    collect_lambda_params_block(&m.body, &mut bound);
                }
            }
            Item::TraitDef(t) => {
                bound.insert(t.name.clone());
                for m in &t.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params_names {
                        bound.insert(p.clone());
                    }
                    if let Some(body) = &m.body {
                        collect_lambda_params_block(body, &mut bound);
                    }
                }
            }
            Item::Impl(im) => {
                for m in &im.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params {
                        bound.insert(p.name.clone());
                    }
                    collect_lambda_params_block(&m.body, &mut bound);
                }
            }
            Item::CheckerBlock { name, body, .. } => {
                bound.insert(name.clone());
                collect_lambda_params_block(body, &mut bound);
            }
            Item::TypeAlias(t) => {
                bound.insert(t.name.clone());
            }
            Item::DuckDef(d) => {
                bound.insert(d.name.clone());
            }
            _ => {}
        }
    }

    // 函数体/checker/方法/const 中的 let 绑定与 f-string 使用
    for item in &module.items {
        match item {
            Item::FnDef(f) => scan_block_fstrings(&f.body, &mut bound, &mut used),
            Item::CheckerBlock { body, .. } => scan_block_fstrings(body, &mut bound, &mut used),
            Item::StructDef(s) => {
                for m in &s.methods {
                    scan_block_fstrings(&m.body, &mut bound, &mut used);
                }
            }
            Item::EnumDef(e) => {
                for m in &e.methods {
                    scan_block_fstrings(&m.body, &mut bound, &mut used);
                }
            }
            Item::Impl(im) => {
                for m in &im.methods {
                    scan_block_fstrings(&m.body, &mut bound, &mut used);
                }
            }
            Item::Test(t) => scan_block_fstrings(&t.body, &mut bound, &mut used),
            Item::Const(c) => scan_expr_fstrings(&c.value, &mut used),
            _ => {}
        }
    }

    used.into_iter().filter(|u| !bound.contains(u)).collect()
}

// ── type-pack 调用点预扫描（03d §2.8 方案 B：`..: Tuple<Ts...>` 具体化）──

/// 递归扫描 Block，收集对 type-pack 变参函数的调用点实参类型
pub(crate) fn collect_typepack_calls(
    b: &Block,
    tp: &std::collections::HashMap<String, String>,
    out: &mut std::collections::HashMap<String, Vec<Vec<IrType>>>,
) {
    for s in &b.stmts {
        match s {
            Stmt::Let { value, .. } => collect_typepack_expr(value, tp, out),
            Stmt::Assign { target, value } => {
                collect_typepack_expr(target, tp, out);
                collect_typepack_expr(value, tp, out);
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    collect_typepack_expr(v, tp, out);
                }
            }
            Stmt::ExprStmt { expr } => collect_typepack_expr(expr, tp, out),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_typepack_expr(cond, tp, out);
                collect_typepack_calls(then_branch, tp, out);
                if let Some(eb) = else_branch {
                    collect_typepack_calls(eb, tp, out);
                }
            }
            Stmt::For {
                iter,
                guard,
                body,
                else_body,
                ..
            } => {
                collect_typepack_expr(iter, tp, out);
                if let Some(g) = guard {
                    collect_typepack_expr(g, tp, out);
                }
                collect_typepack_calls(body, tp, out);
                if let Some(eb) = else_body {
                    collect_typepack_calls(eb, tp, out);
                }
            }
            Stmt::While {
                cond,
                guard,
                body,
                else_body,
            } => {
                collect_typepack_expr(cond, tp, out);
                if let Some(g) = guard {
                    collect_typepack_expr(g, tp, out);
                }
                collect_typepack_calls(body, tp, out);
                if let Some(eb) = else_body {
                    collect_typepack_calls(eb, tp, out);
                }
            }
            Stmt::WhileLet {
                expr, guard, body, ..
            } => {
                collect_typepack_expr(expr, tp, out);
                if let Some(g) = guard {
                    collect_typepack_expr(g, tp, out);
                }
                collect_typepack_calls(body, tp, out);
            }
            Stmt::Match { scrutinee, arms } => {
                collect_typepack_expr(scrutinee, tp, out);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        collect_typepack_expr(g, tp, out);
                    }
                    collect_typepack_calls(&arm.body, tp, out);
                }
            }
            Stmt::Raise { value } => collect_typepack_expr(value, tp, out),
            Stmt::Assert { cond, message } => {
                collect_typepack_expr(cond, tp, out);
                if let Some(m) = message {
                    collect_typepack_expr(m, tp, out);
                }
            }
            Stmt::Yield { value } => collect_typepack_expr(value, tp, out),
            Stmt::YieldFrom { iter } => collect_typepack_expr(iter, tp, out),
            Stmt::BreakLabel { value, .. } => {
                if let Some(v) = value {
                    collect_typepack_expr(v, tp, out);
                }
            }
            Stmt::BlockLabel { body, .. } => collect_typepack_calls(body, tp, out),
            Stmt::CheckerBlock { body, .. } => collect_typepack_calls(body, tp, out),
            Stmt::Defer { body } => collect_typepack_calls(body, tp, out),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                collect_typepack_calls(body, tp, out);
                for (_, cb) in catches {
                    collect_typepack_calls(cb, tp, out);
                }
                if let Some(eb) = else_body {
                    collect_typepack_calls(eb, tp, out);
                }
                if let Some(fb) = finally_body {
                    collect_typepack_calls(fb, tp, out);
                }
            }
            Stmt::Block { stmts } => {
                let tmp = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                    span: Span::unknown(),
                };
                collect_typepack_calls(&tmp, tp, out);
            }
            _ => {}
        }
    }
}

/// 递归扫描表达式，收集 Call{ Var(typepack_fn), args } 的实参类型
pub(crate) fn collect_typepack_expr(
    e: &Expr,
    tp: &std::collections::HashMap<String, String>,
    out: &mut std::collections::HashMap<String, Vec<Vec<IrType>>>,
) {
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            if let ExprKind::Var(name) = &callee.kind {
                if tp.contains_key(name) {
                    out.entry(name.clone())
                        .or_insert_with(Vec::new)
                        .push(args.iter().map(|a| a.ty.clone()).collect());
                }
            }
            collect_typepack_expr(callee, tp, out);
            for a in args {
                collect_typepack_expr(a, tp, out);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_typepack_expr(receiver, tp, out);
            for a in args {
                collect_typepack_expr(a, tp, out);
            }
        }
        ExprKind::FieldAccess { base, .. } => collect_typepack_expr(base, tp, out),
        ExprKind::IndexGet { base, key } => {
            collect_typepack_expr(base, tp, out);
            collect_typepack_expr(key, tp, out);
        }
        ExprKind::IndexSet { base, key, value } => {
            collect_typepack_expr(base, tp, out);
            collect_typepack_expr(key, tp, out);
            collect_typepack_expr(value, tp, out);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            collect_typepack_expr(lhs, tp, out);
            collect_typepack_expr(rhs, tp, out);
        }
        ExprKind::AssignExpr { target, value } => {
            collect_typepack_expr(target, tp, out);
            collect_typepack_expr(value, tp, out);
        }
        ExprKind::UnOp { operand, .. } => collect_typepack_expr(operand, tp, out),
        ExprKind::IfExpr { cond, then, els } => {
            collect_typepack_expr(cond, tp, out);
            collect_typepack_expr(then, tp, out);
            collect_typepack_expr(els, tp, out);
        }
        ExprKind::Lambda { body, .. } => collect_typepack_expr(body, tp, out),
        ExprKind::StructCtor { fields, .. } => {
            for (_, e2) in fields {
                collect_typepack_expr(e2, tp, out);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                collect_typepack_expr(a, tp, out);
            }
        }
        ExprKind::GenExpr { yield_of } => collect_typepack_expr(yield_of, tp, out),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                collect_typepack_expr(c, tp, out);
            }
            collect_typepack_calls(block, tp, out);
        }
        ExprKind::Cast { expr, .. } => collect_typepack_expr(expr, tp, out),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                collect_typepack_expr(a, tp, out);
            }
        }
        ExprKind::BlockExpr { block } => collect_typepack_calls(block, tp, out),
        ExprKind::TupleLit(es) | ExprKind::Tuple(es) => {
            for e2 in es {
                collect_typepack_expr(e2, tp, out);
            }
        }
        _ => {}
    }
}

// ── 未定义外部函数预扫描（p16_lzi 等跨模块探测用例）──

/// lz_builtins crate 导出的函数名（use lz_builtins::* 已引入，不得再生成同名 stub）
pub(crate) const LZ_BUILTIN_FN_NAMES: &[&str] = &[
    // lz_builtins crate 实际导出（use lz_builtins::* 已引入）
    "__block_on",
    "__lz_bytes",
    "__lz_duration_ms",
    "__lz_duration_secs",
    "__lz_path",
    "__lz_pathbuf",
    "__lz_str_ref",
    "abs_f64",
    "abs_i64",
    "ceil",
    "chr",
    "chr_unchecked",
    "clear",
    "compile_warn",
    "contains",
    "currentframe",
    "divmod",
    "field",
    "field_count",
    "find_type",
    "floor",
    "from_args",
    "from_std",
    "get_raw",
    "getargs",
    "getcomments",
    "getdoc",
    "getmembers",
    "getmodulename",
    "getreturntype",
    "getsource",
    "getsourcefile",
    "getsourcelines",
    "init",
    "input",
    "is_empty",
    "is_eq",
    "is_ge",
    "is_gt",
    "is_le",
    "is_lt",
    "isclass",
    "isfunction",
    "ismethod",
    "ismodule",
    "len",
    "lz_abs",
    "lz_all",
    "lz_any",
    "lz_bool",
    "lz_clamp",
    "lz_count_if",
    "lz_ends_with",
    "lz_float",
    "lz_int",
    "lz_join_words",
    "lz_range",
    "lz_range_step",
    "lz_sum_ints",
    "max_f64",
    "max_i64",
    "min_f64",
    "min_i64",
    "new",
    "ord",
    "pow_f64",
    "pow_i64",
    "print",
    "print_str",
    "print_val",
    "range",
    "range2",
    "range3",
    "read_file",
    "register",
    "round",
    "set_raw",
    "set_str",
    "signature",
    "status_str",
    "to_std",
    "type_count",
    "with_start",
    "with_step",
    "write_file",
    // 集合可变自由函数：push/pop/append/extend/insert/remove —— 由 lz_builtins 或
    // Vec 固有方法提供，禁止 stub 遮蔽；gen_call 降级为 `(recv).push(item)` 等方法调用
    "push",
    "pop",
    "append",
    "extend",
    "insert",
    "remove",
    // collections.rs 集合方法（trait 提供，同样禁止 stub）
    "lz_push",
    "lz_pop",
    "lz_len",
    "lz_get",
    "lz_set",
    "lz_contains",
    "lz_index",
    "lz_remove",
    "lz_insert",
    "lz_sort",
    "lz_reverse",
    "lz_extend",
    "lz_clear",
    "lz_is_empty",
    "lz_first",
    "lz_last",
    "lz_slice",
    "lz_keys",
    "lz_values",
    "lz_items",
    "lz_update",
    "lz_set_default",
    "lz_add",
    "lz_union",
    "lz_intersection",
    "lz_difference",
    "lz_symmetric_difference",
    "lz_iter",
    "lz_next",
    "lz_collect",
    "lz_map",
    "lz_filter",
    "lz_enumerate",
    "lz_zip",
    "lz_reduce",
    "lz_fold",
    "lz_take",
    "lz_skip",
    "lz_repeat",
    "lz_chain",
    "lz_once",
    "lz_empty",
    // 标准枚举构造器（rustc 自带，禁止 stub 遮蔽）
    "Some",
    "None",
    "Ok",
    "Err",
    // lz 内置类型转换 / 内置函数（codegen 有专门生成路径，禁止 stub 遮蔽）
    "str",
    "int",
    "float",
    "bool",
    "list",
    "dict",
    "set",
    "tuple",
    "bytes",
    "bytearray",
    "frozenset",
    "slice",
    "memoryview",
    "object",
    "type",
    "repr",
    "ascii",
    "format",
    "hash",
    "id",
    "iter",
    "next",
    "enumerate",
    "zip",
    "map",
    "filter",
    "reversed",
    "sorted",
    "sum",
    "min",
    "max",
    "abs",
    "all",
    "any",
    "open",
    "super",
    "property",
    "staticmethod",
    "classmethod",
    "isinstance",
    "issubclass",
    "callable",
    "vars",
    "dir",
    "getattr",
    "setattr",
    "hasattr",
    "delattr",
    "locals",
    "globals",
];

/// 扫描表达式：收集对未定义顶层函数的 Call（callee 为 Var；排除宏名与局部绑定）
#[allow(clippy::too_many_arguments)]
pub(crate) fn scan_expr_extern_calls(
    e: &Expr,
    known: &std::collections::HashSet<String>,
    builtins: &std::collections::HashSet<&str>,
    bound: &std::collections::HashSet<String>,
    out: &mut std::collections::HashMap<String, usize>,
) {
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            if let ExprKind::Var(n) = &callee.kind {
                if !n.contains('!')
                    && !known.contains(n)
                    && !bound.contains(n)
                    && !builtins.contains(n.as_str())
                {
                    let cnt = out.entry(n.clone()).or_insert(0);
                    if args.len() > *cnt {
                        *cnt = args.len();
                    }
                }
            }
            scan_expr_extern_calls(callee, known, builtins, bound, out);
            for a in args {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            scan_expr_extern_calls(receiver, known, builtins, bound, out);
            for a in args {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        ExprKind::FieldAccess { base, .. } => {
            scan_expr_extern_calls(base, known, builtins, bound, out)
        }
        ExprKind::IndexGet { base, key } => {
            scan_expr_extern_calls(base, known, builtins, bound, out);
            scan_expr_extern_calls(key, known, builtins, bound, out);
        }
        ExprKind::IndexSet { base, key, value } => {
            scan_expr_extern_calls(base, known, builtins, bound, out);
            scan_expr_extern_calls(key, known, builtins, bound, out);
            scan_expr_extern_calls(value, known, builtins, bound, out);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_extern_calls(lhs, known, builtins, bound, out);
            scan_expr_extern_calls(rhs, known, builtins, bound, out);
        }
        ExprKind::AssignExpr { target, value } => {
            scan_expr_extern_calls(target, known, builtins, bound, out);
            scan_expr_extern_calls(value, known, builtins, bound, out);
        }
        ExprKind::UnOp { operand, .. } => {
            scan_expr_extern_calls(operand, known, builtins, bound, out)
        }
        ExprKind::IfExpr { cond, then, els } => {
            scan_expr_extern_calls(cond, known, builtins, bound, out);
            scan_expr_extern_calls(then, known, builtins, bound, out);
            scan_expr_extern_calls(els, known, builtins, bound, out);
        }
        ExprKind::Lambda { params, body, .. } => {
            let mut b2 = bound.clone();
            for p in params {
                b2.insert(p.name.clone());
            }
            scan_expr_extern_calls(body, known, builtins, &b2, out);
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                scan_expr_extern_calls(v, known, builtins, bound, out);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        ExprKind::GenExpr { yield_of } => {
            scan_expr_extern_calls(yield_of, known, builtins, bound, out)
        }
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                scan_expr_extern_calls(c, known, builtins, bound, out);
            }
            scan_block_extern_calls(block, known, builtins, bound, out);
        }
        ExprKind::Cast { expr, .. } => scan_expr_extern_calls(expr, known, builtins, bound, out),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        ExprKind::BlockExpr { block } => {
            scan_block_extern_calls(block, known, builtins, bound, out);
        }
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => {
            for a in es {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        ExprKind::Dict(kvs) => {
            for (k, v) in kvs {
                scan_expr_extern_calls(k, known, builtins, bound, out);
                scan_expr_extern_calls(v, known, builtins, bound, out);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(st) = start {
                scan_expr_extern_calls(st, known, builtins, bound, out);
            }
            scan_expr_extern_calls(end, known, builtins, bound, out);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            scan_expr_extern_calls(receiver, known, builtins, bound, out);
            scan_expr_extern_calls(callee, known, builtins, bound, out);
            for a in args {
                scan_expr_extern_calls(a, known, builtins, bound, out);
            }
        }
        _ => {}
    }
}

/// 扫描块：let/for/match 等绑定加入局部 bound，收集未定义顶层函数 Call
pub(crate) fn scan_block_extern_calls(
    b: &Block,
    known: &std::collections::HashSet<String>,
    builtins: &std::collections::HashSet<&str>,
    bound: &std::collections::HashSet<String>,
    out: &mut std::collections::HashMap<String, usize>,
) {
    let mut b2 = bound.clone();
    for st in &b.stmts {
        match st {
            Stmt::Let { name, value, .. } => {
                scan_expr_extern_calls(value, known, builtins, &b2, out);
                b2.insert(name.clone());
            }
            Stmt::Assign { target, value } => {
                scan_expr_extern_calls(target, known, builtins, &b2, out);
                scan_expr_extern_calls(value, known, builtins, &b2, out);
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    scan_expr_extern_calls(v, known, builtins, &b2, out);
                }
            }
            Stmt::ExprStmt { expr } => scan_expr_extern_calls(expr, known, builtins, &b2, out),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                scan_expr_extern_calls(cond, known, builtins, &b2, out);
                scan_block_extern_calls(then_branch, known, builtins, &b2, out);
                if let Some(eb) = else_branch {
                    scan_block_extern_calls(eb, known, builtins, &b2, out);
                }
            }
            Stmt::For {
                var,
                iter,
                guard,
                body,
                else_body,
                ..
            } => {
                scan_expr_extern_calls(iter, known, builtins, &b2, out);
                if let Some(g) = guard {
                    scan_expr_extern_calls(g, known, builtins, &b2, out);
                }
                let mut b3 = b2.clone();
                b3.insert(var.clone());
                scan_block_extern_calls(body, known, builtins, &b3, out);
                if let Some(eb) = else_body {
                    scan_block_extern_calls(eb, known, builtins, &b3, out);
                }
            }
            Stmt::While {
                cond,
                guard,
                body,
                else_body,
                ..
            } => {
                scan_expr_extern_calls(cond, known, builtins, &b2, out);
                if let Some(g) = guard {
                    scan_expr_extern_calls(g, known, builtins, &b2, out);
                }
                scan_block_extern_calls(body, known, builtins, &b2, out);
                if let Some(eb) = else_body {
                    scan_block_extern_calls(eb, known, builtins, &b2, out);
                }
            }
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
                ..
            } => {
                scan_expr_extern_calls(expr, known, builtins, &b2, out);
                if let Some(g) = guard {
                    scan_expr_extern_calls(g, known, builtins, &b2, out);
                }
                let mut b3 = b2.clone();
                collect_pattern_bindings(pattern, &mut b3);
                scan_block_extern_calls(body, known, builtins, &b3, out);
            }
            Stmt::Match { scrutinee, arms } => {
                scan_expr_extern_calls(scrutinee, known, builtins, &b2, out);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        scan_expr_extern_calls(g, known, builtins, &b2, out);
                    }
                    let mut b3 = b2.clone();
                    collect_pattern_bindings(&arm.pattern, &mut b3);
                    scan_block_extern_calls(&arm.body, known, builtins, &b3, out);
                }
            }
            Stmt::Raise { value } => scan_expr_extern_calls(value, known, builtins, &b2, out),
            Stmt::Assert { cond, message } => {
                scan_expr_extern_calls(cond, known, builtins, &b2, out);
                if let Some(m) = message {
                    scan_expr_extern_calls(m, known, builtins, &b2, out);
                }
            }
            Stmt::Yield { value } => scan_expr_extern_calls(value, known, builtins, &b2, out),
            Stmt::YieldFrom { iter } => scan_expr_extern_calls(iter, known, builtins, &b2, out),
            Stmt::BreakLabel { value, .. } => {
                if let Some(v) = value {
                    scan_expr_extern_calls(v, known, builtins, &b2, out);
                }
            }
            Stmt::BlockLabel { body, .. } => {
                scan_block_extern_calls(body, known, builtins, &b2, out)
            }
            Stmt::CheckerBlock { body, .. } => {
                scan_block_extern_calls(body, known, builtins, &b2, out)
            }
            Stmt::Defer { body } => scan_block_extern_calls(body, known, builtins, &b2, out),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                scan_block_extern_calls(body, known, builtins, &b2, out);
                for (_, cb) in catches {
                    scan_block_extern_calls(cb, known, builtins, &b2, out);
                }
                if let Some(eb) = else_body {
                    scan_block_extern_calls(eb, known, builtins, &b2, out);
                }
                if let Some(fb) = finally_body {
                    scan_block_extern_calls(fb, known, builtins, &b2, out);
                }
            }
            Stmt::Block { stmts } => {
                let tmp = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                    span: Span::unknown(),
                };
                scan_block_extern_calls(&tmp, known, builtins, &b2, out);
            }
            _ => {}
        }
    }
}

/// 收集 pattern 中的绑定名（match 分支）
pub(crate) fn collect_pattern_bindings(p: &Pattern, out: &mut std::collections::HashSet<String>) {
    match p {
        Pattern::Ident(name) | Pattern::RefMutIdent(name) => {
            out.insert(name.clone());
        }
        Pattern::Tuple(items) | Pattern::List(items) => {
            for it in items {
                collect_pattern_bindings(it, out);
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, fp) in fields {
                collect_pattern_bindings(fp, out);
            }
        }
        Pattern::Enum { args, .. } => {
            for a in args {
                collect_pattern_bindings(a, out);
            }
        }
        Pattern::Rest(Some(name)) => {
            out.insert(name.clone());
        }
        _ => {}
    }
}

/// 预扫描 IR 模块，返回被调用但模块内无定义的顶层函数名 → 最大实参个数
pub(crate) fn collect_unknown_extern_fns(
    module: &IrModule,
) -> std::collections::HashMap<String, usize> {
    let mut known: std::collections::HashSet<String> = std::collections::HashSet::new();
    for item in &module.items {
        match item {
            Item::FnDef(f) => {
                known.insert(f.name.clone());
                for p in &f.params {
                    known.insert(p.name.clone());
                }
            }
            Item::Const(c) => {
                known.insert(c.name.clone());
            }
            Item::StructDef(s) => {
                known.insert(s.name.clone());
                for m in &s.methods {
                    known.insert(m.name.clone());
                    for p in &m.params {
                        known.insert(p.name.clone());
                    }
                }
            }
            Item::EnumDef(e) => {
                known.insert(e.name.clone());
                for m in &e.methods {
                    known.insert(m.name.clone());
                    for p in &m.params {
                        known.insert(p.name.clone());
                    }
                }
            }
            Item::TraitDef(t) => {
                known.insert(t.name.clone());
            }
            Item::Impl(im) => {
                for m in &im.methods {
                    known.insert(m.name.clone());
                    for p in &m.params {
                        known.insert(p.name.clone());
                    }
                }
            }
            Item::CheckerBlock { name, .. } => {
                known.insert(name.clone());
            }
            Item::TypeAlias(t) => {
                known.insert(t.name.clone());
            }
            Item::DuckDef(d) => {
                known.insert(d.name.clone());
            }
            // 导入符号属于本 crate 已知名字：from lib_stats import sum_squares /
            // import lib_math 的目标名不得触发未知外部函数桩
            // （`fn NAME(__a0: i64) -> i64 { i64::MAX }`），否则增量拼接产物中
            // 桩与依赖模块的真身重复定义（rustc E0428，见
            // tests/incremental_golden.rs 增量拼接 rustc 校验步骤）。
            // 真正无签名的外部调用（p16_lzi 类）不走 Use item，兜底语义不变。
            Item::Use(u) => {
                if u.is_from {
                    for n in &u.items {
                        known.insert(n.clone());
                    }
                }
                if let Some(last) = u.path.last() {
                    known.insert(last.clone());
                }
                if let Some(a) = &u.alias {
                    known.insert(a.clone());
                }
            }
            _ => {}
        }
    }
    let builtins: std::collections::HashSet<&str> = LZ_BUILTIN_FN_NAMES.iter().copied().collect();
    let mut out: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for item in &module.items {
        match item {
            Item::FnDef(f) => scan_block_extern_calls(&f.body, &known, &builtins, &known, &mut out),
            Item::CheckerBlock { body, .. } => {
                scan_block_extern_calls(body, &known, &builtins, &known, &mut out)
            }
            Item::StructDef(s) => {
                for m in &s.methods {
                    scan_block_extern_calls(&m.body, &known, &builtins, &known, &mut out);
                }
            }
            Item::EnumDef(e) => {
                for m in &e.methods {
                    scan_block_extern_calls(&m.body, &known, &builtins, &known, &mut out);
                }
            }
            Item::Impl(im) => {
                for m in &im.methods {
                    scan_block_extern_calls(&m.body, &known, &builtins, &known, &mut out);
                }
            }
            Item::Test(t) => scan_block_extern_calls(&t.body, &known, &builtins, &known, &mut out),
            Item::Const(c) => scan_expr_extern_calls(&c.value, &known, &builtins, &known, &mut out),
            _ => {}
        }
    }
    out
}

pub(crate) fn rewrite_parallel_block(block: &mut Block) {
    for stmt in &mut block.stmts {
        rewrite_parallel_stmt(stmt);
    }
}

pub(crate) fn rewrite_parallel_stmt(stmt: &mut Stmt) {
    match stmt {
        Stmt::Let { value, .. } => rewrite_parallel_expr(value),
        Stmt::Assign { value, .. } => rewrite_parallel_expr(value),
        Stmt::Return { value: Some(v) } => rewrite_parallel_expr(v),
        Stmt::ExprStmt { expr } => rewrite_parallel_expr(expr),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            rewrite_parallel_expr(cond);
            rewrite_parallel_block(then_branch);
            if let Some(b) = else_branch {
                rewrite_parallel_block(b);
            }
        }
        Stmt::For {
            iter,
            guard,
            body,
            else_body,
            ..
        } => {
            rewrite_parallel_expr(iter);
            if let Some(g) = guard {
                rewrite_parallel_expr(g);
            }
            rewrite_parallel_block(body);
            if let Some(b) = else_body {
                rewrite_parallel_block(b);
            }
        }
        Stmt::While {
            cond, guard, body, ..
        } => {
            rewrite_parallel_expr(cond);
            if let Some(g) = guard {
                rewrite_parallel_expr(g);
            }
            rewrite_parallel_block(body);
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            rewrite_parallel_expr(expr);
            if let Some(g) = guard {
                rewrite_parallel_expr(g);
            }
            rewrite_parallel_block(body);
        }
        Stmt::Match { scrutinee, arms } => {
            rewrite_parallel_expr(scrutinee);
            for arm in arms {
                if let Some(g) = &mut arm.guard {
                    rewrite_parallel_expr(g);
                }
                rewrite_parallel_block(&mut arm.body);
            }
        }
        Stmt::Block { stmts } => {
            for s in stmts {
                rewrite_parallel_stmt(s);
            }
        }
        Stmt::BlockLabel { body, .. } => rewrite_parallel_block(body),
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            rewrite_parallel_block(body);
            for (_, b) in catches {
                rewrite_parallel_block(b);
            }
            if let Some(b) = else_body {
                rewrite_parallel_block(b);
            }
            if let Some(b) = finally_body {
                rewrite_parallel_block(b);
            }
        }
        Stmt::Defer { body } => rewrite_parallel_block(body),
        _ => {}
    }
}

pub(crate) fn rewrite_parallel_expr(expr: &mut Expr) {
    // `xs.map(lambda)` → `__lz_par_map(xs, lambda)`
    // 收窄条件：仅当 receiver 是 List 类型、lambda 参数数为 1 时改写
    if let ExprKind::MethodCall {
        receiver,
        method,
        args,
    } = &mut expr.kind
    {
        if method == "map"
            && args.len() == 1
            && matches!(args[0].kind, ExprKind::Lambda { .. })
            && is_list_type(&receiver.ty)
        {
            let span = expr.span.clone();
            let ty = expr.ty.clone();
            let recv = std::mem::replace(
                receiver,
                Box::new(Expr::new(
                    ExprKind::Lit(LitKind::Unit),
                    IrType::Unit,
                    span.clone(),
                )),
            );
            let lam = args[0].clone();
            expr.kind = ExprKind::Call {
                callee: Box::new(Expr::new(
                    ExprKind::Var("__lz_par_map".into()),
                    IrType::Any,
                    span.clone(),
                )),
                args: vec![*recv, lam],
                type_args: vec![],
            };
            expr.ty = ty;
            return;
        }
    }
    // 递归遍历所有子表达式
    match &mut expr.kind {
        ExprKind::Call { callee, args, .. } => {
            rewrite_parallel_expr(callee);
            for a in args {
                rewrite_parallel_expr(a);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            rewrite_parallel_expr(receiver);
            for a in args {
                rewrite_parallel_expr(a);
            }
        }
        ExprKind::FieldAccess { base, .. } => rewrite_parallel_expr(base),
        ExprKind::IndexGet { base, key } => {
            rewrite_parallel_expr(base);
            rewrite_parallel_expr(key);
        }
        ExprKind::IndexSet { base, key, value } => {
            rewrite_parallel_expr(base);
            rewrite_parallel_expr(key);
            rewrite_parallel_expr(value);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            rewrite_parallel_expr(lhs);
            rewrite_parallel_expr(rhs);
        }
        ExprKind::AssignExpr { target, value } => {
            rewrite_parallel_expr(target);
            rewrite_parallel_expr(value);
        }
        ExprKind::UnOp { operand, .. } => rewrite_parallel_expr(operand),
        ExprKind::IfExpr { cond, then, els } => {
            rewrite_parallel_expr(cond);
            rewrite_parallel_expr(then);
            rewrite_parallel_expr(els);
        }
        ExprKind::Lambda { body, .. } => rewrite_parallel_expr(body),
        ExprKind::StructCtor { fields, .. } => {
            for (_, e) in fields {
                rewrite_parallel_expr(e);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                rewrite_parallel_expr(a);
            }
        }
        ExprKind::GenExpr { yield_of } => rewrite_parallel_expr(yield_of),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                rewrite_parallel_expr(c);
            }
            rewrite_parallel_block(block);
        }
        ExprKind::Cast { expr, .. } => rewrite_parallel_expr(expr),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                rewrite_parallel_expr(a);
            }
        }
        ExprKind::BlockExpr { block } => rewrite_parallel_block(block),
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => {
            for e in es {
                rewrite_parallel_expr(e);
            }
        }
        ExprKind::Spread(e) => rewrite_parallel_expr(e),
        ExprKind::Dict(pairs) => {
            for (k, v) in pairs {
                rewrite_parallel_expr(k);
                rewrite_parallel_expr(v);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                rewrite_parallel_expr(s);
            }
            rewrite_parallel_expr(end);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            rewrite_parallel_expr(receiver);
            rewrite_parallel_expr(callee);
            for a in args {
                rewrite_parallel_expr(a);
            }
        }
        ExprKind::Paren(e) => rewrite_parallel_expr(e),
        ExprKind::ImplicitConvert { source, .. } => rewrite_parallel_expr(source),
        _ => {}
    }
}

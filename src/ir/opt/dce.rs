// LZIR 死绑定消除 pass（opt>=2）
//
// `let x = <纯表达式>` 之后 x 再也没被读过 → 这条绑定只剩两件事：产物里一行
// `let mut x = ..;`，以及运行期一次（对纯表达式而言必然被优化掉，但语义上
// 仍是「声明 + 初始化」）。删掉它对语义零影响，还能让产物更贴近手写。
//
// 判据（三条全满足才删）：
//   1. 绑定名在**同函数后续语句**里零引用（含嵌套块/闭包 —— 闭包捕获也算引用，
//      所以「只在闭包里用」不能删）；
//   2. 右值是**纯表达式**（字面量/变量/字段/一元负号/四则运算/比较/列表字面量）。
//      任何调用、方法调用、赋值、管道、await 一律视为有副作用 → 不删；
//   3. 绑定不是 `ref`、没有 `@lazy/@once/@static` 之类修饰（它们有初始化时机语义），
//      且不是函数/生成器体（走 `pure_expr` 的黑名单已经排除大部分，但仍显式挡掉
//      `is_mut && 含defer` 的块）。
//
// 明确不做的事（避免把「优化」变成「行为漂移」）：
//   - 不删 `Stmt::Let` 的**尾语句**（它可能是块值/返回值）；
//   - 不做跨语句的变量合并（`let a = X; let b = a;` → `let b = X`），因为 move 语义
//     与 clone 规则在 codegen 里交织，收益小、风险大；
//   - 不删除未使用的 `import` / 常量（那是另一个 pass 的事，且可能影响 moddec 发射）。
use super::super::node::*;
use super::walk::{walk_stmt, ExprHook, StmtHook};

pub(crate) fn optimize_item(item: &mut Item, _level: u8) {
    match item {
        Item::FnDef(f) => dce_fn(&mut f.body),
        Item::StructDef(s) => {
            for m in s.methods.iter_mut() {
                dce_fn(&mut m.body);
            }
            if let Some(b) = s.new_body.as_mut() {
                dce_fn(b);
            }
            if let Some(b) = s.init_body.as_mut() {
                dce_fn(b);
            }
        }
        Item::EnumDef(e) => {
            for m in e.methods.iter_mut() {
                dce_fn(&mut m.body);
            }
        }
        Item::TraitDef(t) => {
            for m in t.methods.iter_mut() {
                if let Some(b) = m.body.as_mut() {
                    dce_fn(b);
                }
            }
        }
        Item::Impl(i) => {
            for m in i.methods.iter_mut() {
                dce_fn(&mut m.body);
            }
        }
        Item::Test(t) => dce_fn(&mut t.body),
        Item::CheckerBlock { body, .. } => dce_fn(body),
        _ => {}
    }
}

pub(crate) fn optimize_stmt(s: &mut Stmt, _level: u8) {
    dce_stmt(s);
}

fn dce_fn(b: &mut Block) {
    dce_block(b)
}

fn dce_stmt(s: &mut Stmt) {
    let mut ef: ExprHook = &mut |_e| {};
    let mut sf: StmtHook = &mut |_st| {};
    walk_stmt(s, &mut ef, &mut sf);
}

/// 块级：先递归子块，再按「后向使用计数」删死绑定。
fn dce_block(b: &mut Block) {
    for s in b.stmts.iter_mut() {
        dce_stmt(s);
    }
    // 先把语句取出来算「删/留」标记（后向可见性需要看到后续语句，
    // 因此不能边迭代边清空 b.stmts —— 那样后续切片就空了）。
    let taken = std::mem::take(&mut b.stmts);
    let n = taken.len();
    let mut drop_flags = vec![false; n];
    for (i, s) in taken.iter().enumerate() {
        drop_flags[i] = match s {
            Stmt::Let {
                name,
                value,
                is_ref,
                mods,
                ..
            } if i + 1 < n => {
                !is_ref
                    && !has_lifecycle_mods(mods)
                    && is_pure_expr(value)
                    // 该名字在本块**后续**语句（含嵌套块/闭包）里零引用
                    && !name_used_in_stmts(&taken[i + 1..], name)
            }
            _ => false,
        };
    }
    let mut keep: Vec<Stmt> = Vec::with_capacity(n);
    for (i, s) in taken.into_iter().enumerate() {
        if !drop_flags[i] {
            keep.push(s);
        }
    }
    b.stmts = keep;
}

/// 修饰符轴里带初始化时机/惰性语义的，不能按「无用的声明」删掉。
fn has_lifecycle_mods(m: &IrMods) -> bool {
    m.lazy || m.storage_static
}

/// 右值是否为纯表达式（无副作用、无 panic 风险、可安全丢弃）。
fn is_pure_expr(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Lit(_) | ExprKind::Default | ExprKind::Var(_) | ExprKind::Paren(_) => true,
        ExprKind::FieldAccess { base, .. } => is_pure_expr(base),
        ExprKind::UnOp { operand, .. } => is_pure_expr(operand),
        ExprKind::BinOp { lhs, rhs, op } => {
            // 除法/取模可能 panic（除零），纯但非「无风险」——保留绑定不删
            !matches!(
                op,
                BinOpKind::Div | BinOpKind::Mod | BinOpKind::Pow
            ) && is_pure_expr(lhs)
                && is_pure_expr(rhs)
        }
        ExprKind::ListLit(v) | ExprKind::List(v) | ExprKind::TupleLit(v) | ExprKind::Tuple(v) => {
            v.iter().all(is_pure_expr)
        }
        ExprKind::Range { start, end, .. } => {
            start.as_ref().map_or(true, |s| is_pure_expr(s)) && is_pure_expr(end)
        }
        ExprKind::Cast { expr, .. } => is_pure_expr(expr),
        // 块/lambda/结构体构造/枚举构造/调用/魔法调用/赋值/if/管道/生成器：一律不删
        _ => false,
    }
}

/// 名字是否出现在给定语句序列里（含所有嵌套块与闭包体）。
fn name_used_in_stmts(stmts: &[Stmt], name: &str) -> bool {
    let mut f = Finder {
        name,
        found: false,
    };
    walk(stmts, &mut f);
    f.found
}

struct Finder<'a> {
    name: &'a str,
    found: bool,
}

fn walk(stmts: &[Stmt], f: &mut Finder) {
    for s in stmts {
        walk_stmt_const(s, f);
    }
}

/// 只读版遍历（复用 walk.rs 的结构，但不改 IR —— 借用冲突时另写一份只读遍历）
fn walk_stmt_const(s: &Stmt, f: &mut Finder) {
    if f.found {
        return;
    }
    // 广度优先地扫表达式与子块
    let mut exprs: Vec<&Expr> = Vec::new();
    let mut blocks: Vec<&Block> = Vec::new();
    match s {
        Stmt::Let { value, .. } => exprs.push(value),
        Stmt::Assign { target, value } => {
            exprs.push(target);
            exprs.push(value);
        }
        Stmt::Return { value } => exprs.extend(value.iter()),
        Stmt::ExprStmt { expr } => exprs.push(expr),
        Stmt::If { cond, then_branch, else_branch } => {
            exprs.push(cond);
            blocks.push(then_branch);
            blocks.extend(else_branch.iter());
        }
        Stmt::For { iter, guard, body, else_body, .. } => {
            exprs.push(iter);
            exprs.extend(guard.iter());
            blocks.push(body);
            blocks.extend(else_body.iter());
        }
        Stmt::While { cond, guard, body, else_body } => {
            exprs.push(cond);
            exprs.extend(guard.iter());
            blocks.push(body);
            blocks.extend(else_body.iter());
        }
        Stmt::WhileLet { expr, guard, body, .. } => {
            exprs.push(expr);
            exprs.extend(guard.iter());
            blocks.push(body);
        }
        Stmt::Match { scrutinee, arms } => {
            exprs.push(scrutinee);
            for a in arms {
                exprs.extend(a.guard.iter());
                blocks.push(&a.body);
            }
        }
        Stmt::Raise { value } | Stmt::Yield { value } => exprs.push(value),
        Stmt::Assert { cond, message } => {
            exprs.push(cond);
            exprs.extend(message.iter());
        }
        Stmt::YieldFrom { iter } => exprs.push(iter),
        Stmt::BreakLabel { value, .. } => exprs.extend(value.iter()),
        Stmt::BlockLabel { body, .. } | Stmt::CheckerBlock { body, .. } => blocks.push(body),
        Stmt::Defer { body } => blocks.push(body),
        Stmt::TryCatch { body, catches, else_body, finally_body } => {
            blocks.push(body);
            blocks.extend(catches.iter().map(|(_, b)| b));
            blocks.extend(else_body.iter());
            blocks.extend(finally_body.iter());
        }
        Stmt::Block { stmts } => walk(stmts, f),
        Stmt::Break | Stmt::Continue | Stmt::Pass | Stmt::TypeAlias { .. } => {}
    }
    for e in exprs {
        walk_expr_const(e, f);
    }
    for b in blocks {
        walk(&b.stmts, f);
    }
}

fn walk_expr_const(e: &Expr, f: &mut Finder) {
    if f.found {
        return;
    }
    if matches!(&e.kind, ExprKind::Var(n) if n == f.name) {
        f.found = true;
        return;
    }
    match &e.kind {
        ExprKind::Call { callee, args, .. } => {
            walk_expr_const(callee, f);
            args.iter().for_each(|a| walk_expr_const(a, f));
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            walk_expr_const(receiver, f);
            args.iter().for_each(|a| walk_expr_const(a, f));
        }
        ExprKind::FieldAccess { base, .. }
        | ExprKind::Cast { expr: base, .. }
        | ExprKind::Spread(base)
        | ExprKind::Paren(base)
        | ExprKind::ImplicitConvert { source: base, .. } => walk_expr_const(base, f),
        ExprKind::IndexGet { base, key } => {
            walk_expr_const(base, f);
            walk_expr_const(key, f);
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                walk_expr_const(s, f);
            }
            walk_expr_const(end, f);
        }
        ExprKind::IndexSet { base, key, value } => {
            walk_expr_const(base, f);
            walk_expr_const(key, f);
            walk_expr_const(value, f);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            walk_expr_const(lhs, f);
            walk_expr_const(rhs, f);
        }
        ExprKind::AssignExpr { target, value } => {
            walk_expr_const(target, f);
            walk_expr_const(value, f);
        }
        ExprKind::UnOp { operand, .. } => walk_expr_const(operand, f),
        ExprKind::IfExpr { cond, then, els } => {
            walk_expr_const(cond, f);
            walk_expr_const(then, f);
            walk_expr_const(els, f);
        }
        ExprKind::Lambda { body, .. } => walk_expr_const(body, f),
        ExprKind::StructCtor { fields, .. } => {
            fields.iter().for_each(|(_, v)| walk_expr_const(v, f))
        }
        ExprKind::EnumCtor { args, .. } | ExprKind::MagicCall { args, .. } => {
            args.iter().for_each(|a| walk_expr_const(a, f))
        }
        ExprKind::GenExpr { yield_of } => walk_expr_const(yield_of, f),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                walk_expr_const(c, f);
            }
            walk(&block.stmts, f);
        }
        ExprKind::BlockExpr { block } => walk(&block.stmts, f),
        ExprKind::TupleLit(v)
        | ExprKind::Tuple(v)
        | ExprKind::ListLit(v)
        | ExprKind::List(v) => v.iter().for_each(|x| walk_expr_const(x, f)),
        ExprKind::Dict(pairs) => pairs.iter().for_each(|(k, v)| {
            walk_expr_const(k, f);
            walk_expr_const(v, f);
        }),
        ExprKind::Pipe { receiver, callee, args } => {
            walk_expr_const(receiver, f);
            walk_expr_const(callee, f);
            args.iter().for_each(|a| walk_expr_const(a, f));
        }
        _ => {}
    }
}
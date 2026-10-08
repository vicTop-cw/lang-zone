// LZIR 分支简化 pass（opt>=1）
//
// 恒定条件的分支没有存在价值：`if true` / `if 1 == 1` 生成的条件判断与
// 不可达分支都是纯噪声（多占产物行、干扰后续阅读、分支预测也白做功）。
//
// 只做**语义可证的简化**：
//   1. `if <true>`：保留 then 分支语句、丢掉 else；
//   2. `if <false>`：丢弃 then、保留 else（无 else 时整条 If 变空块）；
//   3. `while <false>`：循环体不执行 → 替换为 Pass。
//      **For 不做**：迭代源（range/集合迭代）可能有副作用，求值不能省。
//   4. `if <true>` 且 then 分支以 break/continue 结尾的「守卫式循环保护」
//      ——同样跳过（有 break/continue 就已判为 risky）。
//
// 前置安全条件（任一不满足即放弃）：分支体内含 defer / break / continue /
// yield（这些语句的可达性变化会改变 defer 执行次数、循环控制流或生成器语义）。
// 只在Stmt::If / Stmt::While 上做，不碰表达式位置 IfExpr（那里两分支类型必须一致）。
use super::super::node::*;
use super::fold::const_bool;
use super::walk::{walk_block, walk_stmt, ExprHook, StmtHook};

pub(crate) fn optimize_item(item: &mut Item, _level: u8) {
    match item {
        Item::FnDef(f) => opt_block(&mut f.body),
        Item::StructDef(s) => {
            for m in s.methods.iter_mut() {
                opt_block(&mut m.body);
            }
        }
        Item::EnumDef(e) => {
            for m in e.methods.iter_mut() {
                opt_block(&mut m.body);
            }
        }
        Item::TraitDef(t) => {
            for m in t.methods.iter_mut() {
                if let Some(b) = m.body.as_mut() {
                    opt_block(b);
                }
            }
        }
        Item::Impl(i) => {
            for m in i.methods.iter_mut() {
                opt_block(&mut m.body);
            }
        }
        Item::Test(t) => opt_block(&mut t.body),
        Item::CheckerBlock { body, .. } => opt_block(body),
        _ => {}
    }
}

pub(crate) fn optimize_stmt(s: &mut Stmt, _level: u8) {
    let mut ef: ExprHook = &mut |_e| {};
    let mut sf: StmtHook = &mut |_s| {};
    walk_stmt(s, &mut ef, &mut sf);
}

/// 块级改写：先递归子块，再把恒定条件的 if/while 就地换成存活分支的语句序列。
///
/// 为什么必须重建 `stmts` 向量：IR 里没有「if 的 then 就地变成语句序列」的表达，
/// 钩子拿到的是单个 `&mut Stmt`，无法把一段语句塞回父块。
fn opt_block(b: &mut Block) {
    // 1) 先让子块完成各自的递归与改写
    let mut ef: ExprHook = &mut |_e| {};
    let mut sf: StmtHook = &mut |_s| {};
    walk_block(b, &mut ef, &mut sf);
    // 2) 再做本块的语句级简化
    let mut out: Vec<Stmt> = Vec::with_capacity(b.stmts.len());
    for s in std::mem::take(&mut b.stmts) {
        match &s {
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => match simplify_if(cond, then_branch, else_branch.as_ref()) {
                Some(repl) => out.extend(repl),
                None => out.push(s),
            },
            Stmt::While { cond, body, .. } => match simplify_while(cond, body) {
                Some(repl) => out.push(repl),
                None => out.push(s),
            },
            _ => out.push(s),
        }
    }
    b.stmts = out;
}

/// 块内是否含 defer / break / continue / yield（含单层 block 包裹）。
fn block_is_risky(b: &Block) -> bool {
    fn stmt_risky(s: &Stmt) -> bool {
        match s {
            Stmt::Defer { .. }
            | Stmt::Break
            | Stmt::Continue
            | Stmt::BreakLabel { .. }
            | Stmt::Yield { .. }
            | Stmt::YieldFrom { .. } => true,
            Stmt::BlockLabel { body, .. } | Stmt::CheckerBlock { body, .. } => block_is_risky(body),
            Stmt::Block { stmts } => stmts.iter().any(stmt_risky),
            _ => false,
        }
    }
    b.stmts.iter().any(stmt_risky)
}

/// `if <const>` → 直接把存活分支的语句**提到当前层**。
/// 不能原地改写（IR 没有「if 的then 就地变成语句序列」的表达），故返回新语句序列。
fn simplify_if(
    cond: &Expr,
    then_branch: &Block,
    else_branch: Option<&Block>,
) -> Option<Vec<Stmt>> {
    let truth = const_bool(cond)?;
    let keep = match (truth, else_branch) {
        (true, _) => then_branch,
        (false, Some(b)) => b,
        (false, None) => return Some(vec![Stmt::Pass]),
    };
    if block_is_risky(keep) {
        return None;
    }
    Some(keep.stmts.clone())
}

/// `while <false>` → Pass（循环体永不执行；但 body 有 defer 时仍放弃，
/// 因为 defer 在 Rust 里的注册位置与循环同域，替换后行为需另行验证）。
fn simplify_while(cond: &Expr, body: &Block) -> Option<Stmt> {
    match const_bool(cond) {
        Some(false) if !block_is_risky(body) => Some(Stmt::Pass),
        _ => None,
    }
}
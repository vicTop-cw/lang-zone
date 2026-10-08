// LZIR 常量折叠 pass（opt>=1）
//
// 折叠「两侧都是字面量」的二元/一元运算为字面量。收益：把 `2 * 3 + 1` 这类
// 编译期可算的表达式从运行期开销降为零，并让分支简化 pass 能看到常量条件。
//
// 保守边界（关键：保持 Rust 的可观察行为）：
//   - 整型运算一律用 `checked_*`，溢出即**放弃折叠**。否则 debug 构建下
//     `i64::MAX + 1` 会从「panic」变成「静默回绕」，属于语义漂移；
//   - 除零 / 取模零不折叠（Rust 是运行时 panic，折成常量后语义不同）；
//   - 只折叠 Int / Bool；F64 涉及 NaN / 无穷 / 精度，不折叠；
//   - 位移仅在合法区间折叠（越界在 debug/release 行为不一致）。
use super::super::node::*;
use super::walk::{walk_block, walk_expr, ExprHook, StmtHook};

pub(crate) fn optimize_item(item: &mut Item, _level: u8) {
    let mut ef: ExprHook = &mut fold_expr;
    let mut sf: StmtHook = &mut |_s| {};
    match item {
        Item::FnDef(f) => walk_block(&mut f.body, &mut ef, &mut sf),
        Item::StructDef(s) => {
            for m in s.methods.iter_mut() {
                walk_block(&mut m.body, &mut ef, &mut sf);
            }
            if let Some(b) = s.new_body.as_mut() {
                walk_block(b, &mut ef, &mut sf);
            }
            if let Some(b) = s.init_body.as_mut() {
                walk_block(b, &mut ef, &mut sf);
            }
        }
        Item::EnumDef(e) => {
            for m in e.methods.iter_mut() {
                walk_block(&mut m.body, &mut ef, &mut sf);
            }
        }
        Item::TraitDef(t) => {
            for m in t.methods.iter_mut() {
                if let Some(b) = m.body.as_mut() {
                    walk_block(b, &mut ef, &mut sf);
                }
            }
        }
        Item::Impl(i) => {
            for m in i.methods.iter_mut() {
                walk_block(&mut m.body, &mut ef, &mut sf);
            }
        }
        Item::Test(t) => walk_block(&mut t.body, &mut ef, &mut sf),
        Item::CheckerBlock { body, .. } => walk_block(body, &mut ef, &mut sf),
        Item::Const(c) => {
            walk_expr(&mut c.value, &mut ef, &mut sf);
        }
        Item::DuckDef(_) => {}
        _ => {}
    }
}

pub(crate) fn optimize_stmt(s: &mut Stmt, _level: u8) {
    let mut ef: ExprHook = &mut fold_expr;
    let mut sf: StmtHook = &mut |_st| {};
    super::walk::walk_stmt(s, &mut ef, &mut sf);
}

fn fold_expr(e: &mut Expr) {
    let folded: Option<LitKind> = match &e.kind {
        ExprKind::BinOp { op, lhs, rhs } => fold_binop(op.clone(), lhs, rhs),
        ExprKind::UnOp { op, operand } => fold_unop(op.clone(), operand),
        _ => None,
    };
    if let Some(lit) = folded {
        e.kind = ExprKind::Lit(lit);
    }
}

/// 剥掉 Paren 冗余包装（不改求值结果，仅让常量判定能穿透括号）。
pub(crate) fn strip_parens(e: &Expr) -> &Expr {
    let mut cur = e;
    while let ExprKind::Paren(inner) = &cur.kind {
        cur = inner;
    }
    cur
}

fn lit_int(e: &Expr) -> Option<i64> {
    match &strip_parens(e).kind {
        ExprKind::Lit(LitKind::Int(v)) => Some(*v),
        _ => None,
    }
}

fn lit_bool(e: &Expr) -> Option<bool> {
    match &strip_parens(e).kind {
        ExprKind::Lit(LitKind::Bool(b)) => Some(*b),
        _ => None,
    }
}

fn fold_binop(op: BinOpKind, lhs: &Expr, rhs: &Expr) -> Option<LitKind> {
    if let (Some(a), Some(b)) = (lit_bool(lhs), lit_bool(rhs)) {
        return match op {
            BinOpKind::And => Some(LitKind::Bool(a && b)),
            BinOpKind::Or => Some(LitKind::Bool(a || b)),
            BinOpKind::Eq => Some(LitKind::Bool(a == b)),
            BinOpKind::Neq => Some(LitKind::Bool(a != b)),
            _ => None,
        };
    }
    let (a, b) = (lit_int(lhs)?, lit_int(rhs)?);
    let v = match op.clone() {
        BinOpKind::Add => a.checked_add(b)?,
        BinOpKind::Sub => a.checked_sub(b)?,
        BinOpKind::Mul => a.checked_mul(b)?,
        BinOpKind::Div => a.checked_div(b)?,
        BinOpKind::Mod => a.checked_rem(b)?,
        BinOpKind::Pow => a.checked_pow(u32::try_from(b).ok()?)?,
        BinOpKind::Eq => return Some(LitKind::Bool(a == b)),
        BinOpKind::Neq => return Some(LitKind::Bool(a != b)),
        BinOpKind::Lt => return Some(LitKind::Bool(a < b)),
        BinOpKind::Gt => return Some(LitKind::Bool(a > b)),
        BinOpKind::Le => return Some(LitKind::Bool(a <= b)),
        BinOpKind::Ge => return Some(LitKind::Bool(a >= b)),
        BinOpKind::BitAnd => a & b,
        BinOpKind::BitOr => a | b,
        BinOpKind::Xor => a ^ b,
        BinOpKind::Shl => a.checked_shl(u32::try_from(b).ok()?)?,
        BinOpKind::Shr => a.checked_shr(u32::try_from(b).ok()?)?,
        _ => return None,
    };
    Some(LitKind::Int(v))
}

fn fold_unop(op: UnOpKind, operand: &Expr) -> Option<LitKind> {
    match op.clone() {
        UnOpKind::Neg => Some(LitKind::Int(lit_int(operand)?.checked_neg()?)),
        UnOpKind::Not => Some(LitKind::Bool(!lit_bool(operand)?)),
        _ => None,
    }
}

/// 常量布尔判定（供branch pass 复用）：字面量 bool / 非零整数。
pub(crate) fn const_bool(e: &Expr) -> Option<bool> {
    match &strip_parens(e).kind {
        ExprKind::Lit(LitKind::Bool(b)) => Some(*b),
        ExprKind::Lit(LitKind::Int(v)) => Some(*v != 0),
        _ => None,
    }
}
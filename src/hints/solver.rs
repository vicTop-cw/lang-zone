//! 约束求解器
//!
//! 依次对约束集做统一（[`unify`]）。采用 fail-fast 策略：遇到第一条
//! 失败的约束即返回错误，便于精确定位首个类型冲突点。
//!
//! 后续 P1 可在此层叠加“递归展开深度预算”，对齐 Zig `@setEvalBranchQuota`
//! 与 TypeScript `--recursiveTypeDepth` 的 runaway 保护机制。

use crate::hints::tyvar::{InferCtx, TypeError};
use crate::hints::constraint::Constraint;
use crate::hints::unify::unify;

/// 求解约束集。全部成功返回 Ok(())，否则返回首个错误。
pub fn solve(ctx: &mut InferCtx, cs: &[Constraint]) -> Result<(), TypeError> {
    for c in cs {
        match c {
            Constraint::Eq(a, b) => unify(ctx, a, b)?,
        }
    }
    Ok(())
}

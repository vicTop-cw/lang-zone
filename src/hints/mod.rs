//! Lang-Zong 类型自推断基础库 —— `hints`
//!
//! 本库提供类型自推断系统的 **P0 基石**，对齐调研报告中的三项优先借鉴：
//!
//! | 借鉴来源 | 落地点 |
//! |---------|--------|
//! | Haskell HM（Algorithm W 风格）| [`unify`] 统一算法 + [`solver::solve`] 约束求解 |
//! | rustc / `ena` union-find | [`tyvar::InferCtx`] 以并查集管理推断变量 |
//! | Zig `comptime` runaway 上限 | [`unify`] 的 occurs-check 从算法层杜绝无限类型展开 |
//!
//! 设计目标：推断引擎独立于现有 `codegen` —— 推断跑完后调用 [`subst::zonk`]
//! 解出完整 `Type`，再交给既有转译逻辑，不扰动已稳定的代码生成路径。
//!
//! ## 使用流程
//! ```text
//! 解析 AST → 收集约束(constraint) → solve(unify) → zonk(替换) → codegen
//! ```
//!
//! 后续 P1/P2 将在此之上叠加：let 泛化（level 字段已预留）、子类型约束、
//! 递归深度预算、类型自判断（comptime + @typeInfo 式内建）。

mod tyvar;
mod unify;
mod constraint;
mod solver;
mod subst;

pub use tyvar::{InferCtx, TyVar, TypeError};
pub use unify::unify;
pub use constraint::Constraint;
pub use solver::solve;
pub use subst::zonk;

#[cfg(test)]
mod tests;

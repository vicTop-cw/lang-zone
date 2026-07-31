//! 类型约束表示
//!
//! P0 仅实现相等约束 `Eq`。子类型约束（Subtype）留待 P1，对应调研报告中
//! 的“泛型约束下的递归类型推导”与 `where T <: Any` 等场景。

use crate::types::def::Type;

/// 一条类型约束
#[derive(Debug, Clone)]
pub enum Constraint {
    /// a 与 b 必须统一（相等）
    Eq(Type, Type),
}

impl Constraint {
    /// 构造相等约束
    pub fn eq(a: Type, b: Type) -> Self {
        Constraint::Eq(a, b)
    }
}

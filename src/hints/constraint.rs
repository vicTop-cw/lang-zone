//! 类型约束表示
//!
//! LZ 无子类型运算符 `<:` 与 `>:` 已移除。约束仅 `Eq`。
//! 型变由编译器按位置自动推断。

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

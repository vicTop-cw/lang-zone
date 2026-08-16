// lz_builtins::types — 共享类型定义 Ordering

use std::cmp::Ordering as StdOrdering;
use std::fmt::{Debug, Display};
use std::hash::Hash;

// ══════════════════════════════════════════════════════════════
// Ordering — 全序比较结果
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ordering {
    Less,
    Equal,
    Greater,
}

impl Ordering {
    pub fn from_std(o: StdOrdering) -> Self {
        match o {
            StdOrdering::Less => Ordering::Less,
            StdOrdering::Equal => Ordering::Equal,
            StdOrdering::Greater => Ordering::Greater,
        }
    }

    pub fn to_std(self) -> StdOrdering {
        match self {
            Ordering::Less => StdOrdering::Less,
            Ordering::Equal => StdOrdering::Equal,
            Ordering::Greater => StdOrdering::Greater,
        }
    }

    pub fn is_eq(self) -> bool {
        self == Ordering::Equal
    }
    pub fn is_lt(self) -> bool {
        self == Ordering::Less
    }
    pub fn is_gt(self) -> bool {
        self == Ordering::Greater
    }
    pub fn is_le(self) -> bool {
        self != Ordering::Greater
    }
    pub fn is_ge(self) -> bool {
        self != Ordering::Less
    }
}

impl Display for Ordering {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ordering::Less => write!(f, "Less"),
            Ordering::Equal => write!(f, "Equal"),
            Ordering::Greater => write!(f, "Greater"),
        }
    }
}

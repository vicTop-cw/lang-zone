// Lang-Zong 编译器 — bridge/mod.rs

pub mod cli;
pub mod core;
pub mod embed;
pub mod r#extern; // Level 1: extern "Rust" 链接桥接
pub mod ffi;
pub mod ledger;
pub mod python;
pub mod rust;
pub mod source;
pub mod std; // Level 4: 共享内存嵌入桥接

pub use core::*; // Re-export Bridge trait, BridgeRegistry, etc.
pub use ledger::*;
pub use std::*; // Re-export StdBridge as the primary bridge interface // Re-export Ledger, LedgerReport

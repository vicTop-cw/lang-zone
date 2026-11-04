// Lang-Zong 编译器 — bridge/mod.rs

pub mod cli;
pub mod core;
pub mod embed;
pub mod project; // M3 T0r55.1.2: cli.rs 子命令下沉到 lib crate
pub mod r#extern; // Level 1: extern "Rust" 链接桥接
pub mod ffi;
pub mod ledger;
pub mod python;
pub mod rust;
pub mod source;
pub mod std; // Level 4: 共享内存嵌入桥接
pub mod tnr; // Tnr 桥接：lib API 转译 + 已编译库链接 + 计算后端委托

pub use core::*; // Re-export Bridge trait, BridgeRegistry, etc.
pub use ledger::*;
pub use std::*; // Re-export StdBridge as the primary bridge interface // Re-export Ledger, LedgerReport

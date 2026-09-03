// Lang-Zong 编译器 — bridge/mod.rs

pub mod core;
pub mod std;
pub mod source;
pub mod ffi;
pub mod cli;
pub mod ledger;
pub mod python;
pub mod rust;
pub mod r#extern;   // Level 1: extern "Rust" 链接桥接
pub mod embed;     // Level 4: 共享内存嵌入桥接

pub use std::*;  // Re-export StdBridge as the primary bridge interface
pub use core::*; // Re-export Bridge trait, BridgeRegistry, etc.
pub use ledger::*; // Re-export Ledger, LedgerReport

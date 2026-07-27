// Lang-Zong 编译器 — bridge/mod.rs

pub mod core;
pub mod std;
pub mod source;
pub mod ffi;
pub mod cli;

pub use std::*;  // Re-export StdBridge as the primary bridge interface
pub use core::*; // Re-export Bridge trait, BridgeRegistry, etc.

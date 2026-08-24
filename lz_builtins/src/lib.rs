// lz_builtins — Lang-Zone 运行时内置库
//
// 模块分层:
//   runtime/    — 任何上下文可用（运行时 + 编译期均可）
//                 builtins, ops, collections, iter, types
//   comptime/   — 仅编译期可用（typeof / inspect 等）
//                 inspect (预留给 compiler 实现)
//   reflect.rs  — 运行时反射（类型注册、字段内省）
//
// 零外部依赖，纯 Rust std

mod comptime;
pub mod reflect;
mod runtime;

// prelude: runtime + reflect（不包含 comptime — 需显式 use lz_builtins::comptime::*）
pub use reflect::*;
pub use runtime::*;
// touch

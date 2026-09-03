// lz_builtins — Lang-Zone 运行时内置库
//
// 模块分层:
//   runtime/    — 任何上下文可用（运行时 + 编译期均可）
//                 builtins, ops, collections, iter, types
//                 error, functional, lz_bootstrap_builtins
//   comptime/   — 仅编译期可用（typeof / inspect / size_of 等）
//                 type_name, type_id, size_of, align_of, fields_of
//   reflect.rs  — 运行时反射（类型注册、字段内省）
//
// 零外部依赖，纯 Rust std

pub mod comptime;
pub mod reflect;
pub mod runtime;

// prelude: runtime + reflect（不包含 comptime — 需显式 use lz_builtins::comptime::*）
pub use reflect::*;
pub use runtime::*;

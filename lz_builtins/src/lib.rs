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
// 外部依赖：num-bigint / num-complex —— 为 bigint / complex 基础类型提供运行时
// 支持，并由本 crate 统一 re-export（见下方 `pub use`）。生成码所在的 crate
// 只链接 lz_builtins，不再直接触达 num-* crate 名。

pub mod comptime;
pub mod reflect;
pub mod runtime;

// ── bigint / complex 基础类型的对外通道 ────────────────────────────────────
// 生成产物统一以 `lz_builtins::BigInt` / `lz_builtins::Complex64` 触达这两个
// 类型（BUG-1：num-* 仅是本 crate 的传递依赖，孤立 crate 里裸
// `use num_bigint::BigInt;` 解析不到 ⇒ rustc E0432）。
// `pub extern crate` 让既有的 `use lz_builtins::*;` 同时把 `num_bigint` /
// `num_complex` 模块名带进作用域，历史产物里的 `num_bigint::…` 路径不致断链。
pub extern crate num_bigint;
pub extern crate num_complex;
pub use num_bigint::BigInt;
pub use num_complex::Complex64;

// prelude: runtime + reflect（不包含 comptime — 需显式 use lz_builtins::comptime::*）
pub use reflect::*;
pub use runtime::*;

// Lang-Zong 编译器 — 库根
// 5 层架构: L1(lexer/util) → L2(parser/ast/macros) → L3(types/magic/bridge) → L4(codegen) → L5(main)

// ── L1 基础层 ──
pub mod cache;
pub mod config;
pub mod incr;
pub mod lexer;
pub mod project;
pub mod simd;
pub mod util;

// ─── FIST T4.5 / 升级计划第4章：热重载（方向C）与 LSP（方向D） ───
pub mod hotreload;
pub mod lsp;

// ── L2 语法与宏层 ──
pub mod ast;
pub mod macros;
pub mod moddec;
pub mod parser;

// ── L3 语义与类型层 ──
pub mod bridge;
pub mod magic;
pub mod semantic_check;
pub mod types;

// ── L3.5 编译期求值层 ──
pub mod comptime;

// ── L3.5 跨模块类型签名层（lz-infer 生成的 .lzi 文件加载与查询；可选增强，
// 由 infer 特性门控，非主编译管线依赖） ──
#[cfg(feature = "infer")]
pub mod infer;

// ── L3.5 中间表示层 ──
pub mod ir;

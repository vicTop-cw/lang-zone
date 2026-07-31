// Lang-Zong 编译器 — 库根
// 5 层架构: L1(lexer/util) → L2(parser/ast/macros) → L3(types/magic/bridge) → L4(codegen) → L5(main)

// ── L1 基础层 ──
pub mod lexer;
pub mod config;
pub mod util;
pub mod project;
pub mod cache;
pub mod simd;

// ── L2 语法与宏层 ──
pub mod ast;
pub mod parser;
pub mod macros;

// ── L3 语义与类型层 ──
pub mod types;
pub mod magic;
pub mod bridge;

// ── L3.5 中间表示层 ──
pub mod ir;

// ── L4 代码生成层 ──
pub mod codegen;

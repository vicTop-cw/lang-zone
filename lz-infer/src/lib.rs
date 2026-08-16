//! LZ 外部辅助类型推断引擎（lz-infer）
//!
//! 为 Lang-Zong 模块生成 `.lzi` 类型签名文件。
//! 支持单文件独立推断 + 两阶段跨模块推断。

pub mod eval;
pub mod infer;
pub mod lzi;
pub mod type_parser;

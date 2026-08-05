// Lang-Zong 编译器集成测试
// 将 DEMO/ 目录下的 .lz 文件作为测试用例，验证编译器能正确处理。
//
// 测试分两类：
// - ir_snapshots: 正面测试 — 主 DEMO/ 应成功产出 LZIR（IR 路径，仅 --emit=ir）
// - reject_errors: 负面测试 — 错误边界 DEMO 应产生编译错误
//
// 注：原 compile_demos（AST→RUST 全量代码生成路径）因违反 IR-only 技术路线约束，
// 已移入 tests/deprecated/ 并停用（见 issues/2026-08-05-tech-debt-compile-demos-ast-rust.md）。

mod reject_errors;
mod ir_snapshots;

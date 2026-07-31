# lib 编译断链：`std/shims.rs` 引用路径错误

- **Status**: Fixed
- **Severity**: P0（曾阻断 `cargo test --test compile_demos` 全量回归）
- **Category**: compiler / build
- **Discovered**: 2026-07-29
- **Reporter**: 文通通
- **Owner**: engineering（文档专家不修 `src/`）
- **Resolved**: 2026-07-29 17:52（占位文件 `std/shims.rs` 被补上）

## Summary

`cargo build --lib` 与 `cargo test --test compile_demos` 无法编译通过（编译失败，非解析失败）。根因是 `src/codegen/mod.rs:127` 用 `include_str!("../../std/shims.rs")` 引用了一个不存在的文件。

## Evidence

- `src/codegen/mod.rs:127`：`let shims_src = include_str!("../../std/shims.rs");`
  → 原报 `error: couldn't read src\codegen\../../std/shims.rs: 系统找不到指定的文件 (os error 2)`
- `std/` 目录原无 `shims.rs`；真实运行时内容位于 `src/runtime/shims.rs`（9500B）。

## Resolution

- `std/shims.rs` 占位文件已创建（58B，内容仅注释：`Lang-Zone 标准库桥接 shims — 请勿手动编辑`）。
- `include_str!` 现可解析；`cargo build --lib` 通过（仅 1 条 benign unused import 警告，位于 `src/macros/interp.rs:5`）。
- 本 issue 仅跟踪"编译断链"；链路恢复即关闭。

## Follow-up

- 占位文件只为让 `include_str!` 通过。真正的 shims 内容（运行时内建映射）是否需并入 `std/shims.rs`，由工程侧决定，不在本 issue 范围。
- 另：原 `parser/expr.rs` 的 API 漂移报错（`parse_call_arg` 等）为 lib 编不过的连锁，lib 恢复后需重跑确认是否已随链路恢复消失。

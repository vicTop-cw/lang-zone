# TECH_DEBT：tests/compile_demos.rs 依赖 AST→RUST 代码路径（违反 IR-only 约束）

- **标记类型**：TECH_DEBT
- **严重等级**：P2（测试基础设施违规，不阻塞 IR 语义，但违反项目强制技术路线）
- **标记时间（UTC）**：2026-08-05T17:19:00Z
- **标记人**：auto-sdet

## 问题描述

`tests/compile_demos.rs` 的 `all_demos_compile_successfully` 测试通过 CLI 二进制编译 DEMO 文件时，**未指定 `--emit=ir`**（见 L61-63）：

```rust
let output = Command::new(&bin_path)
    .arg(file.as_os_str())   // 无 --emit=ir → 触发 AST→RUST 全量代码生成
    .output();
```

其注释（L58-59）明确写道「验证 .lz 文件可成功解析为 .rs」「完整的 --test 编译+运行测试通道」。这直接调用了 **AST → RUST 代码生成路径**，违反 SDET 任务的最高优先级约束：

> ⛔ 严禁使用 AST → RUST 代码路径的任何逻辑、工具或测试用例

## 影响

- 该测试会实际触发 Rust 代码发射（需 Rust 工具链），与 IR-only 质量保障目标不一致。
- 在 IR 路线约束下，不应被执行；本轮已 **跳过该测试**（未运行）。

## 建议修复

1. 将 `compile_demos.rs` 改为 `--emit=ir` 模式（与 `ir_snapshots.rs` 一致），仅验证 IR 产出不报错；或
2. 将其重命名为 `tests/deprecated/compile_demos.rs` 并加 `ignore` 标记，待 AST→RUST 路径正式弃用后删除。

## 环境信息

- OS：Microsoft Windows NT 10.0.26200.0
- 分支：master
- commit：7571650
- IR 路径回溯锚点：`tests/compile_demos.rs:61-63`（Command 调用缺 `--emit=ir`）

## 状态

- 未修复（待人工排期）。本轮未运行该测试，避免污染 IR-only 测试结论。

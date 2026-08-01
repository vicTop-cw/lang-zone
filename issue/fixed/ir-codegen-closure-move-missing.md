# ir-codegen-closure-move-missing

> 状态: Open | 严重等级: P2 | 发现日期: 2026-08-01 | 分类: IR Codegen

## Bug 标题 (N14)
返回闭包缺少 `move` 关键字 — E0373

## 复现步骤
1. 编译 `DEMO/04_functions/closures_more.lz` (IR路线)
2. 用 rustc 编译生成的 `closures_more.rs`

## 预期结果
LZ 闭包捕获外部变量 `n` 并返回。codegen 应生成 `move |x| x + n` 确保闭包拥有 `n` 的所有权。

## 实际结果
```rust
return |x: i64| { x + n };  // E0373: closure may outlive the current function
```

## 影响
- `closures_more.lz` (1 文件)
- 所有返回捕获外部变量的闭包场景

## 环境信息
- 编译器版本: commit cc5ebad
- Rust 版本: rustc 1.96.0 stable-x86_64-pc-windows-msvc
- 复现率: 100%

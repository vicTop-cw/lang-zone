# ir-codegen-callable-objects-e0618

> 状态: Open | 严重等级: P2 | 发现日期: 2026-08-01 | 分类: IR Codegen

## Bug 标题 (N13)
callable_objects 未生成 `Fn` trait impl — E0618

## 复现步骤
1. 编译 `DEMO/07_data_structures/callable_objects.lz` (IR路线)
2. 用 rustc 编译生成的 `callable_objects.rs`

## 预期结果
LZ 支持 `__call__` 魔术方法使 struct 实例可调用。codegen 应生成对应的 `Fn`/`FnMut`/`FnOnce` trait 实现，或将 `doubler(21)` 转换为 `doubler.call(21)`。

## 实际结果
```rust
let doubler = Multiplier { factor: 2 };
doubler(21);  // E0618: expected function, found `Multiplier`
```

## 影响
- `callable_objects.lz` (1 文件)
- 所有使用 `__call__` 魔术方法的 struct

## 环境信息
- 编译器版本: commit cc5ebad
- Rust 版本: rustc 1.96.0 stable-x86_64-pc-windows-msvc
- 复现率: 100%

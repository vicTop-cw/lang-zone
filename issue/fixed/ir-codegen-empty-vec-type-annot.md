# ir-codegen-empty-vec-type-annot

> 状态: Open | 严重等级: P2 | 发现日期: 2026-07-31 | 分类: IR Codegen

## Bug 标题
空列表 `[]` 生成 `vec![]` 缺少类型标注（E0282）

## 复现步骤
1. 编译 `DEMO/01_basics/literals.lz` 或 `DEMO/02_types/primitives.lz` (IR路线)
2. 用 rustc 编译

## 预期结果
空列表 `let empty: Nil = []` 应生成带类型标注的代码，如 `let empty: Vec<_> = vec![];` 或根据上下文推断。

## 实际结果
```rust
let empty = vec![];  // ❌ E0282: type annotations needed for `Vec<_>`
```

## 影响文件
- `DEMO/01_basics/literals.lz`
- `DEMO/02_types/primitives.lz`
- `DEMO/99_spec/ir-edge-empty-collections.lz`

## 根因分析
空列表 `[]` 的 IR 类型为 Any/Unit 时，codegen 降级为 `vec![]` 但未附加类型标注。应在有上下文类型信息时（如 `let empty: Nil` → `Nil` = `Vec<?>`）推断并标注。

## 环境信息
- 编译器版本: commit 4aa367e
- Rust 版本: stable-x86_64-pc-windows-msvc
- 复现率: 100%

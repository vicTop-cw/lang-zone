# ir-codegen-constant-as-type

> 状态: Open | 严重等级: P1 | 发现日期: 2026-07-31 | 分类: IR Codegen

## Bug 标题
常量值被误用为类型注解（E0573: expected type, found constant）

## 复现步骤
1. 编译 `DEMO/01_basics/identifiers.lz` (IR路线)
2. 用 rustc 编译生成的 `identifiers.rs`

## 预期结果
`const instance: MyType = 30;` 能正常编译（MyType 是类型别名或 struct 类型）

## 实际结果
```
error[E0573]: expected type, found constant `MyType`
  --> DEMO/01_basics/identifiers.rs:61:17
   |
61 | const instance: MyType = 30;
   |                 ^^^^^^ not a type
```

## 根因分析
LZ 源文件中的类型别名 `MyType` 在 IR→Rust 阶段未正确输出为类型定义，导致后续使用时 rustc 找不到对应的类型名，而是找到了某个同名的常量/值。

## 影响文件
- `DEMO/01_basics/identifiers.lz` (E0573 + E0425 + E0599)

## 环境信息
- 编译器版本: commit 4aa367e
- Rust 版本: stable-x86_64-pc-windows-msvc
- 复现率: 100%

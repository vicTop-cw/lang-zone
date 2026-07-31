# 🔴 P0: 按位取反 `~` 运算符静默丢失

**Bug ID**: N2
**严重等级**: 🔴 P0 — 编译器撒谎（静默丢弃运算符）
**发现日期**: 2026-07-31 20:45
**环境**: commit `b99448f`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// DEMO/05_expressions/operators.lz (~ line 58)
let binv: int = ~5   // LZ 按位取反，期望 -6
```

编译: `lang-zone operators.lz` → `rustc operators.rs`

## 实际结果

```rust
let binv: i64 = 5;  // ❌ ~ 被完全忽略！结果 = 5
```

## 预期结果

```rust
let binv: i64 = !5;  // ✅ 按位取反 = -6
```

## 根因分析

按位取反运算符 `~` 在 IR builder 或 codegen 中被忽略或未实现转换。LZ 使用 `~` 表示按位取反（Python 风格），应映射为 Rust 的 `!` 前缀运算符。

可能原因：
1. IR node 类型 `BitNot` 未在 codegen 中处理
2. 或者 parser/IR builder 层丢失了 `~` 运算符

## 影响范围

- `DEMO/05_expressions/operators.lz` — 直接受影响
- 任何使用 `~` 按位取反的 LZ 代码

## 严重程度说明

「编译器撒谎」类 —— 运算符被静默丢弃，用户代码产生错误值而没有任何警告或错误。这是最危险的一类编译器缺陷。

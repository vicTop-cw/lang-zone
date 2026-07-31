# 🔴 P0: 指数运算符 `**` 错误生成为乘法 `*`

**Bug ID**: N1
**严重等级**: 🔴 P0 — 编译器撒谎（静默替换运算符）
**发现日期**: 2026-07-31 20:45
**环境**: commit `b99448f`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// DEMO/05_expressions/operators.lz (line 36)
let pow: int = 2 ** 10   // LZ 指数运算
```

编译: `lang-zone operators.lz` → `rustc operators.rs`

## 实际结果

```rust
let pow: i64 = 2 * 10;  // ❌ 生成了乘法！
// pow 实际值为 20，期望 1024
```

## 预期结果

```rust
let pow: i64 = 2_i64.pow(10);  // ✅ pow(10) = 1024
// 或: let pow: i64 = (2i64).pow(10);
```

## 根因分析

IR builder 或 codegen 将 LZ 的 `**` 运算符映射为 Rust 的 `*` 二元乘法。`**` 在运算符优先级表 (§17) 中比 `*` 高一级，但 codegen 层面未做 `pow()` 方法调用转换。

可能的代码位置：
- `src/ir/builder.rs` — BinOp 转换
- `src/ir/codegen.rs` — Rust 代码输出

## 影响范围

- `DEMO/05_expressions/operators.lz` — 直接失败
- `DEMO/13_operators/compound_assign_more.lz` — 复合赋值 `**=` 可能也受影响
- 任何使用 `**` 运算符的 LZ 代码

## 严重程度说明

这是一类「编译器撒谎」Bug —— 用户写 `**` 期望指数运算，编译器静默替换为 `*` 乘法，给出完全不同的结果而无任何报错。与 findings §一 #2（数字字面量静默变0）同级别危险。

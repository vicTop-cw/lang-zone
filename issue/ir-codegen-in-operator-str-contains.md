# 🔴 P1: `in` 运算符 `.contains()` 的 String 参数类型不匹配 (E0277)

**Bug ID**: B2 (2026-07-31 21:24)
**严重等级**: 🔴 P1 — 编译器生成无法编译的 Rust 代码（特定场景）
**发现日期**: 2026-07-31 21:24
**影响文件**: `05_expressions/operators.lz`
**相关提交**: 未提交改动（`src/ir/codegen.rs`）

---

## 复现步骤

1. 编译包含字符串 `in` 操作且 RHS 是 `String` 类型（已 `.to_string()` 转换）的 `.lz` 文件：
```lz
def main() =
    let in_str = "llo" in "hello"    # LHS 是字面量，RHS 是字符串
    print(in_str)
```

2. 当前生成（`"hello"` 的 `.to_string()` 由编译器自动插入）：
```rust
let in_str: bool = "hello".to_string().contains("llo".to_string());
```

3. 用 rustc 编译：
```
error[E0277]: the trait bound `String: Pattern` is not satisfied
  --> operators.rs:61:53
   |
61 | ... = "hello".to_string().contains("llo".to_string());
   |        -------- ^^^^^^^^^^^^^^^^^ the trait `Pattern` is not implemented for `String`
```

## 预期结果

生成：`"hello".contains("llo")` 或 `"hello".to_string().contains(&"llo".to_string())` 或 `"hello".to_string().contains("llo")`

## 实际结果

`"hello".to_string().contains("llo".to_string())` — Rust 的 `str::contains()` 要求参数实现 `Pattern` trait，`String` 不直接实现（需要 `&str` 或 `&String`）。

## 根本原因

未提交改动在 `codegen.rs` 处理 `BinOpKind::In` 时，未考虑参数类型转换：
```rust
// 当前 codegen:
if matches!(&rhs.ty, IrType::Str) {
    return format!("{}.contains({})", cont_s, elem_s);
    // "hello".to_string().contains("llo".to_string()) — ❌ E0277
}
```

当 `cont_s` 是 `.to_string()` 结果（`String` 类型），Rust 的 `str::contains` 需要 `&str` 参数。`String` 不实现 `Pattern`。

## 影响范围

- `operators.lz`: `let in_str = "llo" in "hello"` — 通过 `.to_string()` 自动转换后触发
- 任何 `in` 运算符左侧或右侧为 `String` 类型（非 `&str`）的场景

## 修复方向

两种方案：

**方案 A（推荐）**: 对字符串类型的 `.contains()` 调用，参数自动借用：
```rust
format!("{}.contains(&{})", cont_s, elem_s)
```
或仅在参数类型为 `String`（含 `.to_string()`）时添加 `&`。

**方案 B**: 对 `IrType::Str` 的 `in` 操作，生成不同的调用模式：
```rust
// 用 &str 调用而非 String
format!("{}.contains({})", cont_s, elem_s)  // cont_s 为 &str 时正确
```

## 环境信息

- 编译器: lang-zone.exe (dev profile, 2026-07-31 21:30)
- Rust: rustc 1.92.0 (stable-x86_64-pc-windows-msvc)
- 操作系统: Windows x86_64

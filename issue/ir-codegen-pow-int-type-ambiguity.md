# 🔴 P0: `.pow()` 方法调用整数类型歧义 (E0689)

**Bug ID**: B1 (2026-07-31 21:24)
**严重等级**: 🔴 P0 — 编译器生成无法编译的 Rust 代码
**发现日期**: 2026-07-31 21:24
**影响文件**: `05_expressions/operators.lz`, `13_operators/precedence.lz`
**相关提交**: 未提交改动（`src/ir/builder.rs`, `src/ir/codegen.rs`）

---

## 复现步骤

1. 编译任意包含 `**` 运算符且接收者为整数字面量的 `.lz` 文件：
```lz
def main() =
    let pow = 2 ** 10
    print(pow)
```

2. 查看生成的 Rust 代码：
```rust
let pow: i64 = 2.pow(10);  // ← 问题行
```

3. 用 rustc 编译：
```
error[E0689]: can't call method `pow` on ambiguous numeric type `{integer}`
```

## 预期结果

生成可编译的 Rust 代码，例如：`2_i64.pow(10)` 或 `i64::pow(2, 10)`。

## 实际结果

`2.pow(10)` 中 `2` 是 `{integer}` 占位类型，Rust 无法确定具体整数类型，编译失败。

## 根本原因

未提交改动（`builder.rs`）将 `BinOp::Pow` 从 `BinOpKind::Mul`（降级）改为 `BinOpKind::Pow`（正确），并在 `codegen.rs` 中新增 `.pow()` 方法调用生成。但 `.pow()` 在 Rust 中是具体整数类型（`i32`/`i64` 等）上的方法，整数字面量需要显式类型标注。

当前生成的 `2.pow(10)` 中 `2` 的类型是 `{integer}`，Rust 类型推导无法确定具体是实现 `pow` 的哪个整数类型。

## 影响范围

- `operators.lz`: `2 ** 10` + `mx **= 2`（后者因 `mx` 已标注类型不受影响）
- `precedence.lz`: `2 ** 3 ** 2`（嵌套，2 处 E0689）
- 任何 `**` 运算符左侧为整数字面量的场景

## 修复方向

在 `codegen.rs` 的 `gen_expr` 中，处理 `BinOpKind::Pow` 时，检查 `lhs` 是否为整数字面量：
```rust
// 生成 2_i64.pow(10) 而非 2.pow(10)
if is_integer_literal(lhs) {
    let ty_suffix = infer_integer_type_suffix(lhs);  // e.g. "_i64"
    format!("{}{}.pow({})", lhs_val, ty_suffix, rhs_s)
}
```

或更简单的方式：根据左侧的 IR 类型标注推断后缀。`operators.lz` 中 `let pow = 2 ** 10` 的类型已标注为 `i64`。

## 环境信息

- 编译器: lang-zone.exe (dev profile, 2026-07-31 21:30)
- Rust: rustc 1.92.0 (stable-x86_64-pc-windows-msvc)
- 操作系统: Windows x86_64

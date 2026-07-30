# IR codegen: 尾表达式 / 隐式返回 发射缺失

- **Status**: Fixed ✅ (2026-07-30)
- **Severity**: P1（阻塞无 return 的函数 body 通过 rustc）
- **Category**: ir/codegen
- **Discovered**: 2026-07-30
- **Reporter**: 文通通

## Summary

IR codegen 对函数体尾表达式不发射 `return` 关键字，导致隐式返回的函数产生 `expected (), found i64` 类型不匹配错误。

## Evidence

`DEMO/04_functions/basic.lz`：

```lz
def double(x: int) -> int = x * 2   // 隐式返回
def sq(x: int, y: int) -> int = x * x + y * y
```

**旧 codegen 产出**：识别尾表达式为返回值，函数声明 `-> i64 { x * 2 }`

**IR codegen 产出**：

```rust
pub fn double(x: i64) {       // ← 无返回类型
    x * 2                     // ← expected (), found i64
}
```

隐式返回的尾表达式没有被转换为 `return` 或保留为表达式值，导致返回类型 `()` 与实际 `i64` 表达式不匹配。

## Root Cause

`src/ir/codegen.rs` 的 `gen_fn_body`（或等效逻辑）在发射函数体时，将整个块作为语句列表发射，但最后一个语句如果是表达式，没有添加 `return` 或转换为表达式语句。

## Fix direction

`ir/codegen.rs` 中函数体发射逻辑需要：
1. 检查函数体最后一个语句的类型
2. 如果是 `Stmt::ExprStmt { expr }`，发射 `return expr;` 或直接用表达式（Rust 允许块尾表达式）
3. 同时确保函数签名有正确的返回类型（`fn double(x: i64) -> i64 { ... }`

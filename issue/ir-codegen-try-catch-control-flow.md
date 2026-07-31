# 🔴 P1: try-catch 表达式生成无效控制流

**Bug ID**: N6
**严重等级**: 🔴 P1 — 生成不可编译且语义错误的代码
**发现日期**: 2026-07-31 20:45
**环境**: commit `b99448f`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// DEMO/10_error_handling/panic_raise_try.lz
def safe_divide(a: int, b: int) -> int =
    try:
        checked_divide(a, b)    // 可能 panic
    else:
        -1                      // panic 时返回默认值
    finally:
        print("division attempted")

def try_finally_only() -> int =
    try:
        print("open resource")
    finally:
        print("close resource")
```

编译: `lang-zone panic_raise_try.lz` → `rustc panic_raise_try.rs`

## 实际结果

```rust
// safe_divide (完全错误)
pub fn safe_divide(a: i64, b: i64) -> i64 {
    return {
        checked_divide(a, b);    // 返回 i64（但被丢弃）
        ();                       // Unit
        -1;                       // i64
        return println!(...);     // ❌ finally 中早期 return
    };
}

// try_finally_only (错误)
pub fn try_finally_only() -> i64 {
    return {
        println!("open resource");
        return println!("close resource");  // ❌ 不应在 return 中
    };
}
```

问题:
1. **块表达式类型混乱**: `checked_divide()`(→i64)、`()`(Unit)、`-1`(i64) 混合在同一块中，类型不兼容
2. **finally 块错误**: `finally` 中的语句生成为 `return println!(...)`，导致函数提前返回
3. **try 体未正确包装**: try 体应被包装为可恢复的错误处理（Rust 中无 try/catch 直接对应）

## 预期结果

LZ 的 `try/else/finally` 语义类似 Python，在 Rust 中可考虑：

```rust
pub fn safe_divide(a: i64, b: i64) -> i64 {
    let result = std::panic::catch_unwind(|| checked_divide(a, b));
    let val = match result {
        Ok(v) => v,
        Err(_) => -1,
    };
    println!("division attempted");
    val
}
```

或者（如果 panic 不跨 FFI）使用 Result 风格：

```rust
pub fn safe_divide(a: i64, b: i64) -> i64 {
    let val = match checked_divide_result(a, b) {
        Ok(v) => v,
        Err(_) => -1,
    };
    println!("division attempted");
    val
}
```

## 根因分析

1. **try 表达式 IR 降低**: try 表达式被直接展开为顺序语句块，而非包装为 `catch_unwind` 或 match 控制流
2. **finally 代码生成**: finally 块中的语句被错误地包裹在 `return` 中
3. **else 分支**: catcher 块（else）的逻辑未正确编入 match 臂

## 影响范围

- `DEMO/10_error_handling/panic_raise_try.lz` — E0425/E0308
- `DEMO/combo-syntax/combo_defer_guard_try.lz` — E0308
- `DEMO/combo-syntax/combo_try_raise_guard.lz` — E0425
- `DEMO/combo-syntax/combo_while_guard_try.lz` — E0425

## 修复建议

IR codegen 需要正确识别 try/catch/finally 语义模式：
1. 将 try 体编译为 `std::panic::catch_unwind(AssertUnwindSafe(|| { ... }))`
2. else 分支编译为 match 的 `Err(_)` 臂
3. finally 编译为在所有分支后执行的代码

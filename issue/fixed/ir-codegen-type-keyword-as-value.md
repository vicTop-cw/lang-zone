# 🔴 P1: IR codegen 类型关键字错误用作值表达式

**Bug 标题**: IR 路线生成代码中 `int`、`f64`、`str` 等类型关键字出现在值位置，导致 rustc 编译错误

**严重等级**: 🔴 P1 — 类型转换/is 运算符功能受阻
**发现日期**: 2026-07-31
**环境**: commit `6a85c17`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// operators.lz
def main() =
    let is_int = 42 == int     // 应该是 is 运算符: 42 is int
    let is_str = "hello" == str

// type_conversion.lz
def main() =
    let x = int(3.14)          // 类型转换
    let y = str(42)
```

编译: `lang-zone operators.lz` → 生成 .rs → `rustc operators.rs`

## 实际结果

```rust
// operators.rs (异常)
let is_int: i64 = 42 == int;       // ❌ int 不是值
let is_str: String = "hello" == str;  // ❌ str 不是值

// type_conversion.rs (异常)  
let x = f64(3.14);                  // ❌ f64 不是函数
```

rustc 错误:
- `E0425: cannot find value 'int' in this scope` (operators)
- `E0423: expected value, found builtin type 'f64'` (type_conversion)
- `E0423: expected value, found builtin type 'str'`

## 预期结果

对于 LZ 的 `is` 运算符和类型转换，应生成有效的 Rust：

```rust
// operators.rs (正确)
let is_int: bool = true;  // 42 is int → 编译时已知为 true
// 或: (type_of(42) == TypeId::of::<i64>())

// type_conversion.rs (正确)
let x: i64 = 3.14 as i64;   // int(3.14) → as i64
let y: String = 42.to_string();  // str(42) → to_string()
```

## 根因分析

1. **类型运算符未转换**: LZ 的 `is` 关键字是类型检查运算符，应转换为 Rust 的 `std::any::TypeId` 或编译时消解
2. **类型转换降低**: 已知的修复（commit `fc8877e`）只修复了部分转换函数：`str(x)→format!("{}", x)`、`int(x)→x as i64`，但 `f64`、`float` 等遗漏
3. **`int`/`str`/`f64` 标识符引用**: 在表达式位置出现 `int`/`str` 时，IR codegen 未识别为类型名，直接当作变量输出

## 影响范围

- `DEMO/05_expressions/operators.lz` — `int` 和 `str` 在值位置
- `DEMO/02_types/type_aliases_more.lz` — `f64` 引用
- `DEMO/02_types/type_conversion.lz` — `float()`/`int()` 转换

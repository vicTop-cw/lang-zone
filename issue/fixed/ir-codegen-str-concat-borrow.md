# 🔴 P1: 字符串 `+` 拼接右侧未自动借用 — E0308 类型不匹配

**Bug ID**: B4 (2026-07-31 22:33) · **状态**: Fixed · **修复日期**: 2026-08-01 · **修复提交**: 350e54d
**严重等级**: 🔴 P1 — 编译器生成无法编译的 Rust 代码
**发现日期**: 2026-07-31 22:33
**分类**: IR codegen — 运算符生成
**相关提交**: b591e39

---

## 复现步骤

1. 编写 `.lz` 文件：
```lz
def main() =
    let greeting: str = "Hello, " + "World!"
    print(greeting)
```

2. 用 IR 路线编译：
```
cargo run -- test.lz
```

3. 生成 Rust 代码：
```rust
let greeting: String = "Hello, ".to_string() + "World!".to_string();
//                      ^^^^^^^^^^^^^^^^^^^^^   ^^^^^^^^^^^^^^^^^^^^
//                               String          String → ❌ 需要 &str
```

4. rustc 编译失败：
```
error[E0308]: mismatched types
  --> test.rs:XX:XX
   |
XX | ... = "Hello, ".to_string() + "World!".to_string();
   |                               ^^^^^^^^^^^^^^^^^^^^ expected `&str`, found `String`
```

## 预期结果

生成可编译的 Rust 代码：
```rust
let greeting: String = "Hello, ".to_string() + "World!";
// 或
let greeting: String = "Hello, ".to_string() + &"World!".to_string();
```

Rust 的 `String + &str` 要求 RHS 是 `&str` 而非 `String`。

## 实际结果

LZ 对字符串字面量自动加 `.to_string()` → RHS 变成 `String` → Rust `+` 运算符期望 `&str` → E0308。

## 根本原因

LZ 的 `str` 类型映射为 Rust 的 `String`，字面量 `"xxx"` 自动生成 `.to_string()`。但 Rust 的 `String::add(self, other: &str)` 要求 RHS 是 `&str`。当两侧都自动 `.to_string()` 后，RHS 类型变为 `String`，不匹配。

类似问题已在 `in` 运算符 fix (b591e39) 中通过添加 `&` 借用解决（`.contains(&"llo".to_string())`），但 `+` 运算符尚未处理。

## 影响范围

- 任何字符串字面量使用 `+` 拼接的场景
- 变体：变量间的 `+`，如 `s1 + s2` 其中 s2 类型为 `String`
- 已知受影响：`variadic_dotdot.lz`、新增边界测试
- 对 `"a" + "b"`（纯字面量）更简单的优化：直接生成 `"ab".to_string()`

## 修复方向

**方案 A**（推荐）：当 `+` 运算符两侧均为 `IrType::Str` 时，对 RHS 生成自动借用：
```rust
// 当前：format!("{}.to_string() + {}.to_string()", lhs, rhs)
// 修复：format!("{}.to_string() + &{}.to_string()", lhs, rhs)
// 更优：format!("{}.to_string() + {}", lhs, rhs_s)
```

**方案 B**：编译时常量折叠，`"Hello, " + "World!"` → `"Hello, World!".to_string()`（最优但对变量场景无效）。

## 环境信息

- 编译器: lang-zone.exe (dev profile, commit b591e39)
- Rust: rustc 1.92.0 (stable-x86_64-pc-windows-msvc)
- 操作系统: Windows 11 x86_64

# Bug: codegen 生成无效 Rust 代码 — `str()` 转换 + 字符串拼接

**状态**: Open  
**发现日期**: 2026-07-31 14:29  
**严重等级**: 🔴 P1 — 生成不可编译的目标代码  
**发现方式**: 边界值测试 + rustc 编译验证  
**测试工程师**: 自动化边界测试

---

## 描述

LZ 编译器 codegen 在两种情况下生成无法通过 `rustc` 编译的 Rust 代码：

### 问题 1: `str(expr)` 生成 `str(expr)`

```lz
def main() =
    let s = "num: " + str(42)
    print(s)
```

**生成的 Rust**:
```rust
let s2 = "num: " + str(42);
```

**rustc 报错**:
```
error[E0423]: expected function, found builtin type `str`
```

Rust 中 `str` 是原始字符串切片类型，不是转换函数。应为 `format!("{}", 42)` 或 `42.to_string()`。

### 问题 2: `"a" + "b"` 生成无效 Rust

```lz
def main() = print("hello" + " world")
```

**生成的 Rust**:
```rust
let s1 = "hello" + " world";
```

**rustc 报错**:
```
error[E0369]: cannot add `&str` to `&str`
```

Rust 的 `+` 运算符要求 `String + &str`（左操作数必须是 `String`），两个 `&str` 不能直接拼接。

---

## 技术根因

- `src/codegen/expr.rs` 的 `gen_call` / `gen_binop` 对函数调用和二元运算做原文透传
- 对 `str()`、字符串 `+` 等语义特殊形式无特判转换
- `print` 已有特殊处理（→ `println!`），但 `str()` 和字符串拼接缺失

---

## 影响范围

- 所有使用 `str(x)` 做类型转换的 `.lz` → 产物 Rust 代码不可编译
- 所有使用 `"a" + "b"` 字符串拼接的 `.lz` → 产物 Rust 代码不可编译
- 影响可能波及其他 LZ builtin 函数（如 `int()`、`float()` 等）如果存在类似透传

---

## 修复建议

### 修复 1: `str()` 转换

在 `src/codegen/expr.rs` 的 `gen_call` 中对 `str(args...)` 做特判：
```rust
if func_s == "str" {
    if args.len() == 1 {
        return format!("format!(\"{{}}\", {})", self.gen_expr(&args[0]));
    }
}
```

### 修复 2: 字符串拼接

在 `gen_binop` 中对 `StrLit + StrLit` 模式做特判：
```rust
// 检查是否是字符串拼接
if op == "+" && (is_str_type(left) || is_str_type(right)) {
    return format!("format!(\"{{}}{{}}\", {}, {})", left_s, right_s);
}
```

或更简单：对字符串使用 `.to_owned()` + `&`：
```rust
format!("{}.to_owned() + &{}", left_s, right_s)
```

---

## 复现验证

```bash
# 问题 1
echo 'def main() = print(str(42))' > test_str.lz
cargo run -- test_str.lz
rustc --edition 2021 test_str.rs  # 应编译失败

# 问题 2
echo 'def main() = print("a" + "b")' > test_concat.lz
cargo run -- test_concat.lz
rustc --edition 2021 test_concat.rs  # 应编译失败
```

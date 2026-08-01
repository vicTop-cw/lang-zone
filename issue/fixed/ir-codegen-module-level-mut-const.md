# 🟡 P1: 模块级变量 `count = 0` 生成 `const` 而非可变 — E0070 赋值错误

**Bug ID**: B5 (2026-07-31 22:33) · **状态**: Fixed · **修复日期**: 2026-08-01 · **修复提交**: 350e54d
**严重等级**: 🟡 P1 — walrus 预声明 fix (b591e39) 后暴露的残留问题
**发现日期**: 2026-07-31 22:33
**分类**: IR codegen — 变量声明
**相关提交**: b591e39 (walrus predecl fix)

---

## 复现步骤

1. 查看 `DEMO/03_variables/walrus.lz`：
```lz
count = 0                          # 模块级可变变量

def count_up() -> int =
    count += 1                     # ← 修改模块级变量
    count

def main() =
    while (val := count_up()) < 10:
        print(val)
```

2. 用 IR 路线编译并查看生成的 Rust：
```rust
pub fn count_up() -> i64 {
    count = count + 1;             // ← E0070: invalid left-hand side
    return count;
}

const count: i64 = 0;              // ← 模块级变量生成为 const！
```

3. rustc 编译：
```
error[E0070]: invalid left-hand side of assignment
  --> walrus.rs:14:11
   |
14 |     count = count + 1;
   |     ----- ^
   |     cannot assign to this expression
```

## 预期结果

模块级 `count = 0` 应生成可变静态变量：
```rust
static mut count: i64 = 0;        // 允许在 unsafe 块中修改
```
或使用内部可变性包装：
```rust
use std::cell::Cell;
static count: Cell<i64> = Cell::new(0);
```

## 实际结果

生成 `const count: i64 = 0;` — 不可变的编译期常量，无法在运行时修改。

## 根本原因

IR codegen 在遇到模块级 `let` / 赋值语句时，一律映射为 `const`。未区分变量是否会在此后被修改。

`count += 1` 通过对 walrus 预声明 fix (b591e39) 已能正确找到 `count` 变量（不再 E0425），但赋值的目标 `const` 仍然是不可变的。

## 影响范围

- `walrus.lz`: `count` 变量 → E0070
- 任何模块级 `x = value` 且在函数中 `x += 1` / `x = new_value` 的模式

## 修复方向

在 codegen 中，对模块级变量检测是否有后续修改（写入引用），若有则生成 `static mut` 或 `Cell<T>` 包装：

```rust
// 简单方案（需 unsafe）
if is_mutated_globally {
    format!("static mut {}: {} = {};", name, ty, init)
} else {
    format!("const {}: {} = {};", name, ty, init)
}
```

注意：`static mut` 在 Rust 2024 edition 中访问需要 `unsafe` 块，可能需要配合生成 `unsafe { count += 1; }`。

## 环境信息

- 编译器: lang-zone.exe (dev profile, commit b591e39)
- Rust: rustc 1.92.0 (stable-x86_64-pc-windows-msvc)
- 操作系统: Windows 11 x86_64

# 🔴 P1: IR codegen comprehension 宏缺失

**Bug 标题**: IR 路线推导式（list/dict/set comprehension）生成的 Rust 代码引用未定义的宏

**严重等级**: 🔴 P1 — 3 个 DEMO 文件 rustc 编译失败
**发现日期**: 2026-07-31 16:10
**环境**: commit `3101d07`, Windows, rustc 1.92.0, IR codegen（默认路线）

## 复现步骤

```lz
def main() =
    let squares = [x * x for x in 1..5]
    print(squares)
```

编译: `lang-zone test.lz` → 生成 .rs → `rustc x.rs`

## 实际结果

```rust
let squares: Vec<i64> = comp!(|x: i64| { x * x }, 1..5);
//                       ^^^^ error: cannot find macro `comp` in this scope
```

同样的问题：
- dict comprehension → `dict_comp!` 未定义
- set comprehension → `set_comp!` 未定义

## 预期结果

推导式应展开为 Rust 原生迭代器链：
```rust
let squares: Vec<i64> = (1..5).map(|x: i64| x * x).collect();
```

## 技术根因

IR codegen (`src/ir/codegen.rs`) 对 comprehension 表达式生成宏调用而非展开为标准迭代器链。这些宏在生成的 Rust 代码中未被定义或导入。

## 影响范围

- `DEMO/05_expressions/comprehension.lz` — `comp!` 缺失
- `DEMO/05_expressions/comprehension_more.lz` — `comp!` 缺失
- `DEMO/99_spec/dict_comprehension.lz` — `dict_comp!` 缺失
- `DEMO/99_spec/set_comprehension.lz` — `set_comp!` 缺失

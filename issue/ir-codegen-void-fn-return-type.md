# 🟡 P2: IR codegen 无返回类型函数推断为 `i64` 导致类型不匹配

**Bug 标题**: 未标注返回类型的函数体被推断返回 `i64`，`return println!(...)` 类型为 `()` 不匹配

**严重等级**: 🟡 P2 — 影响特定编码模式
**发现日期**: 2026-07-31
**环境**: commit `ff8c61a` (含未提交 codegen 改动), Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
def empty_list_test() =
    let a = []
    let b: List = []
    print(a, b)
```

编译: `lang-zone test.lz` → 生成 .rs → `rustc test.rs`

## 实际结果

```rust
pub fn empty_list_test() -> i64 {       // ❌ 推断为 i64
    let a = vec![];
    let b = vec![];
    return println!("{:?} {:?}", a, b); // ⚠️ println! 返回 () 而非 i64
}
```

rustc 错误: `E0308: mismatched types — expected i64, found ()`

## 预期结果

函数体最后一句是 `print(...)` 时，应推断返回类型为 `()`：

```rust
pub fn empty_list_test() {
    let a = vec![];
    let b = vec![];
    println!("{:?} {:?}", a, b);
}
```

## 根因

`infer_expr_type` 对函数体最后表达式的类型推断过于激进，将 `print()` 的返回类型推断为默认 `i64` 而非 `()`。同时 `gen_block_inner` 将最后语句包装为 `return ...` 而非裸表达式。

## 影响范围

- 所有 `def fn() = ... print(...)` 无显式返回类型标注的函数
- 测试文件: `DEMO/99_spec/ir-edge-empty-collections.lz`

## 附注

同一文件中还存在另一个问题：`let y: Option = None` 的类型标注被丢弃（生成 `let y = None;`），导致 Rust 无法推断 None 的具体类型。

# 🔴 P1: IR codegen 方法定义语法生成无效 Rust

**Bug 标题**: IR 路线生成 `pub fn config.get(key: ...)` 点号方法定义语法，Rust 中非法

**严重等级**: 🔴 P1
**发现日期**: 2026-07-31 15:43
**环境**: commit `488718d`, Windows, rustc 1.92.0

## 复现步骤

```lz
def config.get(key: str) -> Option<int> =
    // LZ 方法定义（impl 块中的方法）
```

编译: `lang-zone test.lz --emit=ir` → 生成 .rs → `rustc x.rs`

## 实际结果

```rust
pub fn config.get(key: String) -> Option<i64> {
    // ^^^^^^^^^^^^^^^ error: missing parameters for function definition
    // E0569: expected one of `->`, `<`, `where`, or `{`, found `.`
}
```

Rust 不允许函数名中包含点号。

## 预期结果

应该生成 `impl` 块：
```rust
impl Config {
    pub fn get(key: String) -> Option<i64> { ... }
}
```

## 影响范围

- `DEMO/05_expressions/operators.lz`
- 所有定义 struct/impl 方法的文件

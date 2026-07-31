# 🔴 P1: trait 定义中 `Self` 参数未转为 `&self`

**Bug ID**: N5
**严重等级**: 🔴 P1 — trait 方法签名不兼容
**发现日期**: 2026-07-31 20:45
**环境**: commit `b99448f`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// DEMO/07_data_structures/trait_impl.lz
trait Drawable:
    def draw()            // LZ 方法（隐含 self）
    def bounds() -> (f64, f64, f64, f64)

impl Drawable for Rectangle =
    def draw() =
        print("drawing")
    def bounds() =
        (0.0, 0.0, 10.0, 5.0)
```

编译: `lang-zone trait_impl.lz` → `rustc trait_impl.rs`

## 实际结果

```rust
// Trait 定义 (错误)
pub trait Drawable {
    fn draw(Self) -> ();                           // ❌ Self 按值传参
    fn bounds(Self) -> (f64, f64, f64, f64);       // ❌ 同上
}

// Impl 块 (部分正确)
impl Drawable for Rectangle {
    fn draw(&self) -> () { ... }                   // ✅ &self
    fn bounds(&self) -> (f64, f64, f64, f64) { ... }
}
```

**问题**: Trait 定义使用 `Self`（按值），但 impl 块使用 `&self`（按引用）——
两者签名不匹配，导致 rustc `E0185` 错误。

## 预期结果

```rust
// Trait 定义 (正确)
pub trait Drawable {
    fn draw(&self) -> ();                          // ✅
    fn bounds(&self) -> (f64, f64, f64, f64);      // ✅
}
```

## 根因分析

LZ 的方法定义不需要显式 `self` 参数（隐含），但 IR codegen 在生成 Rust trait 定义时：
1. trait 中的方法参数被生成为 `Self`（类型名）
2. impl 中的方法参数被生成为 `&self`（借用）

这种不一致导致 trait 和 impl 签名不匹配。

## 影响范围

- `DEMO/07_data_structures/trait_impl.lz` — E0185
- 任何定义了 trait + 有方法的结构体

## 修复建议

在 trait 定义的 codegen 中，将隐含 self 参数统一生成为 `&self`（或根据方法是否 mut 选择 `&mut self`）。

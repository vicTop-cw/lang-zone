# IR codegen: self 参数类型发射错误

- **Status**: Fixed ✅ (2026-07-30)
- **Severity**: P0（阻塞 struct/trait/impl 的 IR 产出通过 rustc）
- **Category**: ir/codegen
- **Discovered**: 2026-07-30（验证 IR codegen rustc 编译时发现）
- **Reporter**: 文通通

## Summary

IR codegen 将方法定义中的 `self` 参数发射为 `self: i64`，而非 `&self` / `self` / `mut self` 等正确形态。同时 trait 签名中的 self 参数完全丢失。

## Evidence

`DEMO/07_data_structures/struct.lz` 中方法：

```lz
struct Rectangle =
    width: f64
    height: f64
    def area(self) -> f64 =
        self.width * self.height
    def scale(mut self, factor: f64) =
        self.width = self.width * factor
        self.height = self.height * factor
```

**旧 codegen 产出（正确）**：

```rust
impl Rectangle {
    fn area(&self) -> f64 { ... }
    fn scale(&mut self, factor: f64) { ... }
}
```

**IR codegen 产出（错误）**：

```rust
impl Rectangle {
    pub fn area(self: i64) -> f64 { ... }   // ← self: i64 错！
    pub fn scale(self: i64, factor: f64) { ... }
}
```

`DEMO/07_data_structures/trait_impl.lz` 中 trait 定义更严重——self 参数**完全丢失**：

```rust
// IR codegen 产出
trait Drawable {
    fn draw(i64) -> ();     // ← self 在哪？
    fn bounds(i64) -> (f64, f64, f64, f64);
}
```

## Root Cause

`src/ir/codegen.rs` 在发射方法参数时，对 `self` 关键字没有特殊处理——直接把 `self` 当作普通参数，按类型映射处理，导致：
1. LZ 中的 `self` → IR 中的 `self: IrType::Self_` → codegen 中映射为 `i64`
2. 未根据 `ref`/`mut`/`owned` 修饰生成 `&self`/`&mut self`/`self`
3. trait 定义中 self 参数完全被忽略（只发类型不发参数名）

## Impact

所有包含 struct 方法的 demo（struct.lz、struct_more.lz、magic_methods.lz、trait_impl.lz 等 ~10 个 demo）的 IR codegen 产出均无法通过 rustc。即使构造器和类型修复后，self 类型仍然阻塞。

## Fix direction

`ir/codegen.rs` 的 `gen_fn_params`（或等效逻辑）需要：
1. 检测 `IrType::Self_` → 发射 `self` / `&self` / `&mut self` / `self: Box<Self>`（取决于 ref/mut/owned 修饰）
2. trait 定义中保持参数名（`fn draw(&self)` 而非 `fn draw(i64)`）
3. impl 块方法**不加 `pub`**（trait 项继承 trait 的可见性）

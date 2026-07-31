# 🔴 P1: IR codegen 元组枚举变体构造语法错误

**Bug 标题**: IR 路线生成 `Shape::Circle { x: 0.0, y: 0.0 }` 结构体语法构造元组变体，rustc 编译失败

**严重等级**: 🔴 P1
**发现日期**: 2026-07-31 15:43
**环境**: commit `488718d`, Windows, rustc 1.92.0

## 复现步骤

```lz
enum Shape:
    Circle(f64, f64, f64)   // 元组变体
    Rect(f64, f64, f64, f64)

def main() =
    let circle = Circle(0.0, 0.0, 5.0)   // LZ 元组风格构造
```

编译: `lang-zone test.lz --emit=ir` → 生成 .rs → `rustc x.rs`

## 实际结果

```rust
let circle = Shape::Circle { x: 0.0, y: 0.0, radius: 5.0 };
//           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// E0559: variant has no field named `x`
// E0559: variant has no field named `y`
// E0559: variant has no field named `radius`
// help: `Shape::Circle` is a tuple variant, use `Shape::Circle((f64, f64, f64))`
```

## 预期结果

```rust
let circle = Shape::Circle((0.0, 0.0, 5.0));
// 或解构为三个位置参数
```

## 根因

IR codegen 将 LZ 的 `Circle(0.0, 0.0, 5.0)` 构造调用映射为 Rust 结构体字面量语法 `{ field: value }`，但实际应该是元组变体语法 `((values))`。IR builder 需区分元组变体和结构体变体的构造方式。

## 影响范围

- `DEMO/06_control_flow/match.lz` — `Shape::Circle { x: 0.0, ...}`
- `DEMO/07_data_structures/enum.lz`
- 所有使用元组枚举变体的文件

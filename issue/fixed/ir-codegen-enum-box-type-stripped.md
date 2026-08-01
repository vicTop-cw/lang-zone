# N17 🔴 P0: IR 层丢失 Box<> 类型信息，EnumCtor 构造缺失 Box::new() 包装

> 发现日期: 2026-08-01 08:50 · 编译器: cc5ebad + uncommitted codegen · Rust: rustc 1.96.0

---

## 一、现象

`DEMO/07_data_structures/enum_more.lz` 递归枚举构造调用报 8 个 E0308 类型不匹配错误。

```lz
enum Expr:
    Num(f64)
    Add(Expr, Expr)   // LZ 源中不显式标注 Box
    Mul(Expr, Expr)
```

生成 Rust 枚举定义正确包含 `Box<Expr>`：
```rust
pub enum Expr {
    Num(f64),
    Add(Box<Expr>, Box<Expr>),  // ✅ 定义正确
    Mul(Box<Expr>, Box<Expr>),
}
```

但构造调用未生成 `Box::new()`：
```rust
let e = Expr::Add(Expr::Num(2.0), Expr::Mul(...));  // ❌ 缺少 Box::new()
// 期望: let e = Expr::Add(Box::new(Expr::Num(2.0)), Box::new(Expr::Mul(...)));
```

---

## 二、根因分析

**问题层级**: IR Builder (`src/ir/builder.rs`) → CodeGen (`src/ir/codegen.rs`) 跨层类型信息丢失。

### 2.1 CodeGen 层的 Box 检测机制

Uncommitted codegen 已实现 `enum_variant_fields` 映射 + `is_box_type()` 检测：

```rust
// codegen.rs (uncommitted)
let field_types = self.enum_variant_fields.get(&(enum_name.clone(), variant.clone()));
let needs_box = field_types.map_or(false, |types| {
    types.get(i).map_or(false, |ty| is_box_type(ty))  // 检查是否为 Box<T>
});
```

但 `is_box_type()` 要求类型为 `IrType::Named { path: "Box", .. }`。

### 2.2 IR 层实际存储的类型

运行时的 Debug 输出确认：
```
field_types for (Expr, Add) = Some([Named { path: "Expr", args: [] }, Named { path: "Expr", args: [] }])
```

**字段类型存的是 `Expr`，不是 `Box<Expr>`！**

### 2.3 推断

IR Builder 在构建 `EnumDef` 时，`from_ast_type_with_generics()` 或 `Item::EnumDef` 收集变体字段类型的过程中，**剥离了 `Box<>` 包装器**。这导致 codegen 层无论怎么检测都看不到 `Box` 类型，`Box::new()` 包装永远不会触发。

---

## 三、影响范围

| 文件 | 影响 | 错误数 |
|------|------|--------|
| `DEMO/07_data_structures/enum_more.lz` | 构造 + match 解构全失败 | 8 E0308 |
| 所有包含递归 enum 定义的 .lz | 100% 受影响 | - |

**严重等级**: 🔴 P0 — 递归枚举（语言核心特性）完全不可用。即使 codegen 的 Box::new() 修复提交后仍无法工作。

---

## 四、修复建议

1. 定位 IR Builder 中 `EnumDef` 变体字段类型的收集位置
2. 确保 `variant.fields[i].ty` 保留 `Box<T>` 包装（即 `IrType::Named { path: "Box", args: [IrType::Named { path: "Expr" }] }`）
3. 同时确认 `Pattern::Enum` 的 match 解构路径是否也受影响

---

## 五、验证方法

修复后重编译 + 运行：
```bash
cargo build
./target/debug/lang-zone.exe DEMO/07_data_structures/enum_more.lz
rustc DEMO/07_data_structures/enum_more.rs --edition 2021 --out-dir _ir_out
```

预期：0 errors，编译通过。

# IR codegen: struct 构造器 / ���键字参数发射错误

- **Status**: Fixed ✅ (2026-07-30)
- **Severity**: P0（阻塞所有 struct 构造和使用 demo 的 IR 产出通过 rustc）
- **Category**: ir/codegen
- **Discovered**: 2026-07-30（验证 IR codegen rustc 编译时发现）
- **Reporter**: 文通通

## Summary

IR codegen 将 struct 构造调用（`Point(x: 3, y: 4)` 和函数关键字参数调用）错误地发射为带有 `_KwArg` wrapper 对象的函数调用形态，而不是正确的 `Struct { field: value }` 字面量语法。`_KwArg` 类型未定义在任何地方，且 struct 不能作为函数调用。

## Evidence

`DEMO/07_data_structures/struct.lz`：

```lz
let p = Point(x: 3, y: 4)
```

**旧 codegen 产出（正确）**：

```rust
let p = Point { x: 3.0, y: 4.0 };
```

**IR codegen 产出（错误）**：

```rust
let p: Point = Point(_KwArg { name: "x".to_string(), value: 3 }, _KwArg { name: "y".to_string(), value: 4 });
```

此的问题：
1. `_KwArg` 未定义——不是 LZ 标准库中的类型
2. `Point(...)` 是函数调用语法，但 `Point` 是 struct → `E0423: expected function, found struct`
3. Rust 正确的构造是 `Point { x: 3, y: 4 }`

## Root Cause

`src/ir/builder.rs` 在代码生成中，关键字参数没有按照 struct 名解析脱糖为 Rust 的 struct literal 语法。而是保留了一组 `_KwArg{name, value}` 结构体，预留由 codegen 后处理。但 `ir/codegen.rs` 没有实现这个后处理。

## Impact

所有使用 struct 构造器或关键字参数调用的 demo 均阻塞（struct.lz、struct_more.lz、magic_methods.lz 等 ~8 个 demo）。

## Fix direction

`src/ir/codegen.rs` 中需要：
1. 检测 `ExprKind::Call` 配合 `IrType::Named(path="StructName")` → 发射 Rust 的 `StructName { field: value, ... }` 字面量
2. 字段名来自 `**KwArg{name, value}` 的 name 字段，而非将 name/value 当作两个字段
3. 或���更干净的方式：在 IR builder 中就将关键字参数解析为标准字段赋值列表，codegen 直接发射 `Struct { f1: v1, f2: v2 }`

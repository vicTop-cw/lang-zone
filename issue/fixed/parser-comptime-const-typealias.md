# parser 不支持 comptime/const 块、type 别名、as 类型转换

- **Status**: Open → **候选关闭**（2026-07-29 审计：const.lz + type_aliases.lz + type_aliases_more.lz + type_conversion.lz 共 4 demo 全绿 ✅）
- **Severity**: P1
- **Category**: parser（表达式 / 顶层声明）
- **Parent**: [parser-new-syntax-regression.md](parser-new-syntax-regression.md)
- **Owner**: engineering

## Summary

v3.1 的类型相关写法（`comptime`/`const` 块、`type X = ...` 多行别名、`expr as Type` 转换）解析器尚未支持，报 `Unexpected token: Comptime` / `As` / `Newline`。

## Evidence（受影响 demo 与错误）

| demo | 错误 |
|------|------|
| `03_variables/const.lz` | `Unexpected token at top level: Comptime` |
| `02_types/type_aliases.lz` | `Unexpected token in expression: Newline` |
| `02_types/type_aliases_more.lz` | `Unexpected token in expression: Newline` |
| `02_types/type_conversion.lz` | `Unexpected token in expression: As` |

复现（`02_types/type_conversion.lz` 片段）：

```lz
let x = 3.14
let y = x as int        // as 类型转换
```

`type_aliases.lz` 疑似多行 `type Vec2 = struct ..` 或 `type Name = Other`，解析器在换行处报错。

## Impact

`comptime`/`const` 是编译期计算核心特性；`type` 别名与 `as` 转换是泛型/FFI 桥接基础。4 个 demo 红。

## Recommendation

- 顶层与块内支持 `comptime` / `const` 块（冒号后换行缩进；`comptime x = expr` 为隐式 const）。
- `type Name = Type` 支持单行与多行别名（含泛型参数 `<T>`）。
- 表达式中支持 `expr as Type` 转换节点。
- 修复后 4 个 demo 转绿。

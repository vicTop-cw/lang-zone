# parser 顶层声明失败（模块魔法属性 / 宏定义 / prelude）

- **Status**: Open
- **Severity**: P1
- **Category**: parser（顶层声明）
- **Parent**: [parser-new-syntax-regression.md](parser-new-syntax-regression.md)
- **Owner**: engineering

## Summary

v3.1 顶层声明写法（模块级魔法属性 `__str__ = ...`、宏定义 `#[macro]` / `macro`、prelude 导入形式）解析器尚未支持，报 `Unexpected token: MagicMethod("__str__")` / `expected macro name` / `Expected variable, got LParen`。

## Evidence（受影响 demo 与错误）

| demo | 错误 |
|------|------|
| `01_basics/identifiers.lz` | `Unexpected token at top level: MagicMethod("__str__")` |
| `09_macros/macro_demo.lz` | `Macro definition error: expected macro name at token 3` |
| `99_prelude/prelude_demo.lz` | `Expected variable, got LParen` |

复现（`01_basics/identifiers.lz` 片段，模块级魔法方法）：

```lz
__str__ = self ->
    "Point(" + self.x.to_string() + ")"
```

`macro_demo.lz` 涉及宏定义语法（如 `#[macro] def name(...) = ...` 或 `macro name = ...`），解析器在 token 3 期望宏名失败。

## Impact

模块级魔法属性是 `lz.std` 元数据与桥接基础；宏是元编程核心。3 个 demo 红。

## Recommendation

- 顶层支持模块级魔法属性 `__name__` / `__str__` / `__all__` 等（`MagicDecl`，参考 `08-模块级魔法属性设计.md`）。
- 宏定义语法：识别 `#[macro]` 或 `macro` 关键字，正确解析宏名与模板体。
- prelude 导入形式（`prelude_demo.lz`）解析为默认导入声明。
- 修复后 3 个 demo 转绿。

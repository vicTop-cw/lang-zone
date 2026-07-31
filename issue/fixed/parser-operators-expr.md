# parser 表达式解析失败（二元 + / 复合 += / 基础构造器 / mut let）

- **Status**: Open
- **Severity**: P1
- **Category**: parser（表达式）
- **Parent**: [parser-new-syntax-regression.md](parser-new-syntax-regression.md)
- **Owner**: engineering

## Summary

v3.1 表达式层写法（二元运算符 `+`、复合赋值 `+=`、基础类型构造器、可变 `let`）解析器尚未完全支持，报 `Unexpected token: Plus` / `Eq` / `Expected variable name, got LParen`。

## Evidence（受影响 demo 与错误）

| demo | 错误 |
|------|------|
| `05_expressions/operators.lz` | `Unexpected token in expression: Plus` |
| `13_operators/compound_assign_more.lz` | `Unexpected token in expression: Eq` |
| `02_types/primitives.lz` | `Expected variable name, got LParen` |
| `03_variables/mutable_let.lz` | `Expected variable name, got LParen` |

复现（`13_operators/compound_assign_more.lz` 片段）：

```lz
let x = 10
x += 5          // 复合赋值
```

`primitives.lz` / `mutable_let.lz` 的 `Expected variable name, got LParen` 暗示基础类型构造（如 `int(...)`）或 `mut let` 绑定解析异常。

## Impact

表达式与赋值是最常用构造；4 个 demo 红，且几乎所有算法类 demo 间接依赖。

## Recommendation

- 二进制运算符 `+ - * / %` 等表达式优先级解析（参考 `12-操作符.md` 优先级表）。
- 复合赋值 `+= -= *= /=` 解析为赋值表达式。
- 基础类型构造器 `Type(expr)` 与 `mut let` / `let mut` 绑定。
- 修复后 4 个 demo 转绿。

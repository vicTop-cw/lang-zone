# parser 控制流新形态失败（ternary / guard / try-raise）

- **Status**: Open
- **Severity**: P1
- **Category**: parser（控制流）
- **Parent**: [parser-new-syntax-regression.md](parser-new-syntax-regression.md)
- **Owner**: engineering

## Summary

v3.1 控制流写法（三元 `a if c else b`、守卫 `guard`、异常 `try`/`raise` 块）解析器尚未支持，报 `Expected Colon, got Else` / `got IntLit(0)` / `got Dot`。

## Evidence（受影响 demo 与错误）

| demo | 错误 |
|------|------|
| `05_expressions/ternary.lz` | `Expected Colon, got Else at pos 26` |
| `combo-syntax/combo_ternary_walrus.lz` | `Expected Colon, got Else at pos 33` |
| `06_control_flow/guard.lz` | `Expected Colon, got IntLit(0) at pos 82` |
| `06_control_flow/match_more.lz` | `Expected Colon, got Dot at pos 35` |
| `10_error_handling/panic_raise_try.lz` | `Expected Colon, got Dot at pos 500` |
| `combo-syntax/combo_try_raise_guard.lz` | `Expected Colon, got Dot at pos 82` |

复现（`05_expressions/ternary.lz` 片段）：

```lz
let max = a if a > b else b
```

`guard.lz` 与 `panic_raise_try.lz` 涉及 `guard` / `try:` / `raise` 的 `:`+缩进块体解析。

## Impact

三元表达式与守卫是函数式/条件精简写法，异常控制流是错误处理基础。覆盖 4+ demo。

## Recommendation

- 表达式支持三元 `cond if true_val else false_val`（注意 `if/else` 关键字形式，非 `?:`）。
- `guard` 语句：`guard cond else { .. }` 或 `guard cond:` 块。
- `try:` / `raise X:` 块（`raise` 后跟表达式，支持 `raise E.X` 点号路径）。
- 修复后上述 demo 转绿。

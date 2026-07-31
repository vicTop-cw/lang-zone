# parser 函数定义多形态解析失败（def / 泛型 / 闭包 / 复合）

- **Status**: Open → **候选关闭**（2026-07-29 审计：5 demo 全绿 ✅ ，含泛型/扩展方法/闭包/复合 def 均已可解析）
- **Severity**: P1
- **Category**: parser（函数声明）
- **Parent**: [parser-new-syntax-regression.md](parser-new-syntax-regression.md)
- **Owner**: engineering

## Summary

v3.1 的函数定义写法（`def f(x: T) -> T = body`、`def f<T>()` 泛型、`\(..)`/closure 参数、`def f(a, b,)` 复合）解析器尚未支持，报一系列 `Expected Colon` / `Expected LParen` / `Expected param name` 错误。

## Evidence（受影响 demo 与错误）

| demo | 错误 |
|------|------|
| `04_functions/basic.lz` | `Expected Colon, got RParen at pos 23` |
| `04_functions/checker.lz` | `Expected LParen, got LBrack at pos 52` |
| `04_functions/closures_more.lz` | `Expected param name, got LParen` |
| `04_functions/composite.lz` | `Expected Colon, got Comma at pos 77` |
| `04_functions/generics.lz` | `Expected LParen, got Colon at pos 39` |

复现（`04_functions/basic.lz` 片段）：

```lz
def add(a: int, b: int) -> int = a + b
def double(x) = x * 2
def max(a: int, b: int) -> int =
    if a > b: a else: b
```

`04_functions/generics.lz` 疑似使用 `def f<T>:` 泛型头；`checker.lz` 疑似 `def checker[T](...)` 方括号泛型；`closures_more.lz` 涉及闭包参数解析。

## Impact

函数是最基础构造；5 个 demo 红，且间接影响所有调用函数的 demo。

## Recommendation

- `def` 头支持：`(params)` 后可选 `-> RetType`，随后 `=` 等式体或 `:`+缩进块体。
- 泛型参数支持 `<T>` 与 `[T]` 两种形式（与 SYNTAX v3.1 一致），位置在 `def name` 与 `(`。
- 闭包参数（如 `\(x)` / `|x|`）解析为 param name。
- 复合/多返回值参数尾逗号容忍。
- 修复后 5 个 demo 转绿。

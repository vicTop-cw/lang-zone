# parser 不支持 struct/enum/trait/match 新形态（类型注解与块体）

- **Status**: Open
- **Severity**: P1
- **Category**: parser（类型声明 / 模式匹配）
- **Parent**: [parser-new-syntax-regression.md](parser-new-syntax-regression.md)
- **Owner**: engineering

## Summary

v3.1 的数据结构写法（`struct Point =` 缩进字段块、`enum E:` 变体、`trait`/`impl`、带类型的 `match` 分支、`defer`/`with` 块）解析器尚未支持，报 `Expected type, got Colon` / `Expected Colon, got RParen` / `got Comma`。

## Evidence（受影响 demo 与错误）

| demo | 错误 |
|------|------|
| `07_data_structures/struct.lz` | `Expected Colon, got RParen at pos 33` |
| `07_data_structures/struct_more.lz` | `Expected Colon, got RParen at pos 15` |
| `07_data_structures/trait_impl.lz` | `Expected Colon, got RParen at pos 10` |
| `07_data_structures/enum.lz` | `Expected type, got Colon` |
| `07_data_structures/enum_more.lz` | `Expected type, got Colon` |
| `07_data_structures/magic_methods.lz` | `Expected Colon, got Comma at pos 18` |
| `06_control_flow/match.lz` | `Expected type, got Colon` |
| `06_control_flow/match_more.lz` | `Expected Colon, got Dot at pos 35` |
| `06_control_flow/with_defer.lz` | `Expected Colon, got RParen at pos 14` |
| `10_error_handling/panic_raise_try.lz` | `Expected Colon, got Dot at pos 500` |

复现（`07_data_structures/struct.lz` 片段）：

```lz
struct Point =
    x: f64
    y: f64

struct Rectangle =
    width: f64
    height: f64
    def area(self) -> f64 =
        self.width * self.height
```

`enum` 报 `Expected type, got Colon` 表明变体写法 `Enum:` 后跟类型/值未被接受；`match` 分支的类型注解同理。

## Impact

数据结构 + 模式匹配是 v3.1 核心；9 个 demo 红，覆盖最广。

## Recommendation

- `struct Name =` 后接缩进字段块（`field: Type` 每行）。
- `enum Name:` 后接变体（`Variant` / `Variant(T)` / `Variant{..}`）。
- `trait` / `impl` 块头与方法体（`def` 在 impl 内）。
- `match scrut:` 分支支持 `Pattern:`（含类型注解）或 `Pattern ->`。
- `defer` / `with` 块语法支持。
- `try` / `raise` 语句的 `:`+缩进块（见 `panic_raise_try.lz`）。
- 修复后 10 个 demo 转绿。

# lz-zed 语法高亮测试报告

- 运行时间: 2026-08-11T07:28:59.649Z
- 模式: VERIFY（校验快照）
- 语法基准: SYNTAX 规范 v3.3（00-词法基础 / 附录B / 12-操作符）

1. grammar/grammar.js 结构检查: OK（存在 grammar 定义）

2. syntaxes/lz.tmLanguage.json 加载与正则编译: OK（32 个解析后 pattern，含 include 展开）

3. 快照测试（17 个 fixture）

| fixture | 行数 | tokens | scopes | 状态 |
|---------|-----:|-------:|-------:|------|
| 01-keywords.lz | 142 | 1291 | 24 | ✅ 通过 |
| 02-literals.lz | 37 | 250 | 11 | ✅ 通过 |
| 03-strings.lz | 47 | 419 | 19 | ✅ 通过 |
| 04-comments.lz | 25 | 141 | 8 | ✅ 通过 |
| 05-operators.lz | 74 | 617 | 13 | ✅ 通过 |
| 06-identifiers.lz | 31 | 236 | 14 | ✅ 通过 |
| 07-control-flow.lz | 105 | 1364 | 18 | ✅ 通过 |
| 08-functions.lz | 77 | 905 | 17 | ✅ 通过 |
| 09-structs-enums-traits.lz | 88 | 780 | 19 | ✅ 通过 |
| 10-imports.lz | 32 | 212 | 13 | ✅ 通过 |
| 11-macros.lz | 52 | 521 | 17 | ✅ 通过 |
| 12-tests.lz | 45 | 473 | 16 | ✅ 通过 |
| 13-errors.lz | 53 | 485 | 14 | ✅ 通过 |
| 14-async-generators.lz | 51 | 552 | 14 | ✅ 通过 |
| 15-build-blocks.lz | 51 | 356 | 10 | ✅ 通过 |
| 16-advanced.lz | 71 | 787 | 13 | ✅ 通过 |
| 17-duck.lz | 44 | 398 | 18 | ✅ 通过 |

合计: 17 个 fixture，9787 个高亮 token

4. 词法元素覆盖率（SYNTAX v3.3 → grammar.js → tmLanguage → fixtures 三方映射）

| 分组 | 元素 | grammar.js | tmLanguage | fixtures | 状态 |
|------|------|:---:|:---:|:---:|:---:|
| keyword | `def` | ✅ | ✅ | ✅ | ✅ |
| keyword | `struct` | ✅ | ✅ | ✅ | ✅ |
| keyword | `enum` | ✅ | ✅ | ✅ | ✅ |
| keyword | `trait` | ✅ | ✅ | ✅ | ✅ |
| keyword | `impl` | ✅ | ✅ | ✅ | ✅ |
| keyword | `type` | ✅ | ✅ | ✅ | ✅ |
| keyword | `const` | ✅ | ✅ | ✅ | ✅ |
| keyword | `mut` | ✅ | ✅ | ✅ | ✅ |
| keyword | `ref` | ✅ | ✅ | ✅ | ✅ |
| keyword | `let` | ✅ | ✅ | ✅ | ✅ |
| keyword | `owned` | ✅ | ✅ | ✅ | ✅ |
| keyword | `magic` | ✅ | ✅ | ✅ | ✅ |
| keyword | `duck` | ✅ | ✅ | ✅ | ✅ |
| keyword | `iterator` | ✅ | ✅ | ✅ | ✅ |
| keyword | `if` | ✅ | ✅ | ✅ | ✅ |
| keyword | `elif` | ✅ | ✅ | ✅ | ✅ |
| keyword | `else` | ✅ | ✅ | ✅ | ✅ |
| keyword | `match` | ✅ | ✅ | ✅ | ✅ |
| keyword | `case` | ✅ | ✅ | ✅ | ✅ |
| keyword | `guard` | ✅ | ✅ | ✅ | ✅ |
| keyword | `for` | ✅ | ✅ | ✅ | ✅ |
| keyword | `while` | ✅ | ✅ | ✅ | ✅ |
| keyword | `loop` | ✅ | ✅ | ✅ | ✅ |
| keyword | `block` | ✅ | ✅ | ✅ | ✅ |
| keyword | `pass` | ✅ | ✅ | ✅ | ✅ |
| keyword | `break` | ✅ | ✅ | ✅ | ✅ |
| keyword | `continue` | ✅ | ✅ | ✅ | ✅ |
| keyword | `return` | ✅ | ✅ | ✅ | ✅ |
| keyword | `with` | ✅ | ✅ | ✅ | ✅ |
| keyword | `defer` | ✅ | ✅ | ✅ | ✅ |
| keyword | `raise` | ✅ | ✅ | ✅ | ✅ |
| keyword | `raises` | ✅ | ✅ | ✅ | ✅ |
| keyword | `try` | ✅ | ✅ | ✅ | ✅ |
| keyword | `catch` | ✅ | ✅ | ✅ | ✅ |
| keyword | `finally` | ✅ | ✅ | ✅ | ✅ |
| keyword | `async` | ✅ | ✅ | ✅ | ✅ |
| keyword | `await` | ✅ | ✅ | ✅ | ✅ |
| keyword | `spawn` | ✅ | ✅ | ✅ | ✅ |
| keyword | `go` | ✅ | ✅ | ✅ | ✅ |
| keyword | `yield` | ✅ | ✅ | ✅ | ✅ |
| keyword | `where` | ✅ | ✅ | ✅ | ✅ |
| keyword | `Self` | ✅ | ✅ | ✅ | ✅ |
| keyword | `macro` | ✅ | ✅ | ✅ | ✅ |
| keyword | `comptime` | ✅ | ✅ | ✅ | ✅ |
| keyword | `template` | ✅ | ✅ | ✅ | ✅ |
| keyword | `test` | ✅ | ✅ | ✅ | ✅ |
| keyword | `suite` | ✅ | ✅ | ✅ | ✅ |
| keyword | `setup` | ✅ | ✅ | ✅ | ✅ |
| keyword | `teardown` | ✅ | ✅ | ✅ | ✅ |
| keyword | `assert` | ✅ | ✅ | ✅ | ✅ |
| keyword | `check` | ✅ | ✅ | ✅ | ✅ |
| keyword | `and` | ✅ | ✅ | ✅ | ✅ |
| keyword | `or` | ✅ | ✅ | ✅ | ✅ |
| keyword | `not` | ✅ | ✅ | ✅ | ✅ |
| keyword | `is` | ✅ | ✅ | ✅ | ✅ |
| keyword | `in` | ✅ | ✅ | ✅ | ✅ |
| keyword | `import` | ✅ | ✅ | ✅ | ✅ |
| keyword | `from` | ✅ | ✅ | ✅ | ✅ |
| keyword | `as` | ✅ | ✅ | ✅ | ✅ |
| keyword | `True` | ✅ | ✅ | ✅ | ✅ |
| keyword | `False` | ✅ | ✅ | ✅ | ✅ |
| keyword | `... (abstract marker)` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `require` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `optional` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `exact` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `min` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `max` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `range` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `at_least` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `at_most` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `satisfies` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `sealed` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `default` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `StackType` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `RefType` | ✅ | ✅ | ✅ | ✅ |
| duck-soft | `Any` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `int` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `f64` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `bool` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `str` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `List` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Dict` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Set` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Option` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Result` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Tuple` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Box` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Rc` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Arc` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Cell` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `RefCell` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `IOError` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Tokens` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Never` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Unit` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Nil` | ✅ | ✅ | ✅ | ✅ |
| builtin-type | `Number` | ✅ | ✅ | ✅ | ✅ |
| builtin-ctor | `None` | ✅ | ✅ | ✅ | ✅ |
| builtin-ctor | `Some` | ✅ | ✅ | ✅ | ✅ |
| builtin-ctor | `Ok` | ✅ | ✅ | ✅ | ✅ |
| builtin-ctor | `Err` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `print` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `panic` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `len` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `contains` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `iter` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `enumerate` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `zip` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `map` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `filter` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `collect` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `sort` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `reverse` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `clone` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `drop` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `format` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `hash` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `callable` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `quote` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `merge_tokens` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `sum` | ✅ | ✅ | ✅ | ✅ |
| builtin-fn | `prod` | ✅ | ✅ | ✅ | ✅ |
| operator | `=` | ✅ | ✅ | ✅ | ✅ |
| operator | `+=` | ✅ | ✅ | ✅ | ✅ |
| operator | `-=` | ✅ | ✅ | ✅ | ✅ |
| operator | `*=` | ✅ | ✅ | ✅ | ✅ |
| operator | `/=` | ✅ | ✅ | ✅ | ✅ |
| operator | `%=` | ✅ | ✅ | ✅ | ✅ |
| operator | `**=` | ✅ | ✅ | ✅ | ✅ |
| operator | `&=` | ✅ | ✅ | ✅ | ✅ |
| operator | `|=` | ✅ | ✅ | ✅ | ✅ |
| operator | `^=` | ✅ | ✅ | ✅ | ✅ |
| operator | `<<=` | ✅ | ✅ | ✅ | ✅ |
| operator | `>>=` | ✅ | ✅ | ✅ | ✅ |
| operator | `:=` | ✅ | ✅ | ✅ | ✅ |
| operator | `==` | ✅ | ✅ | ✅ | ✅ |
| operator | `!=` | ✅ | ✅ | ✅ | ✅ |
| operator | `<` | ✅ | ✅ | ✅ | ✅ |
| operator | `>` | ✅ | ✅ | ✅ | ✅ |
| operator | `<=` | ✅ | ✅ | ✅ | ✅ |
| operator | `>=` | ✅ | ✅ | ✅ | ✅ |
| operator | `&&` | ✅ | ✅ | ✅ | ✅ |
| operator | `||` | ✅ | ✅ | ✅ | ✅ |
| operator | `!` | ✅ | ✅ | ✅ | ✅ |
| operator | `&` | ✅ | ✅ | ✅ | ✅ |
| operator | `|` | ✅ | ✅ | ✅ | ✅ |
| operator | `^` | ✅ | ✅ | ✅ | ✅ |
| operator | `<<` | ✅ | ✅ | ✅ | ✅ |
| operator | `>>` | ✅ | ✅ | ✅ | ✅ |
| operator | `~` | ✅ | ✅ | ✅ | ✅ |
| operator | `+` | ✅ | ✅ | ✅ | ✅ |
| operator | `-` | ✅ | ✅ | ✅ | ✅ |
| operator | `*` | ✅ | ✅ | ✅ | ✅ |
| operator | `/` | ✅ | ✅ | ✅ | ✅ |
| operator | `%` | ✅ | ✅ | ✅ | ✅ |
| operator | `**` | ✅ | ✅ | ✅ | ✅ |
| operator | `|>` | ✅ | ✅ | ✅ | ✅ |
| operator | `??` | ✅ | ✅ | ✅ | ✅ |
| operator | `..` | ✅ | ✅ | ✅ | ✅ |
| operator | `..=` | ✅ | ✅ | ✅ | ✅ |
| operator | `?` | ✅ | ✅ | ✅ | ✅ |
| operator | `?.` | ✅ | ✅ | ✅ | ✅ |
| operator | `=:` | ✅ | ✅ | ✅ | ✅ |
| operator | `^:` | ✅ | ✅ | ✅ | ✅ |
| operator | `~:` | ✅ | ✅ | ✅ | ✅ |
| operator | `*:` | ✅ | ✅ | ✅ | ✅ |
| operator | `->` | ✅ | ✅ | ✅ | ✅ |
| operator | `=>` | ✅ | ✅ | ✅ | ✅ |
| operator | `@ (decorator)` | ✅ | ✅ | ✅ | ✅ |
| operator | `# (attribute macro)` | ✅ | ✅ | ✅ | ✅ |
| literal | `hex int 0xFF` | ✅ | ✅ | ✅ | ✅ |
| literal | `octal int 0o77` | ✅ | ✅ | ✅ | ✅ |
| literal | `binary int 0b1010` | ✅ | ✅ | ✅ | ✅ |
| literal | `underscore int 1_000_000` | ✅ | ✅ | ✅ | ✅ |
| literal | `float 3.14` | ✅ | ✅ | ✅ | ✅ |
| literal | `float exp 1e10` | ✅ | ✅ | ✅ | ✅ |
| literal | `float neg exp 1.5e-3` | ✅ | ✅ | ✅ | ✅ |
| literal | `plain string "…"` | ✅ | ✅ | ✅ | ✅ |
| literal | `f-string f"…"` | ✅ | ✅ | ✅ | ✅ |
| literal | `raw string r"…"` | ✅ | ✅ | ✅ | ✅ |
| literal | `triple string """…"""` | ✅ | ✅ | ✅ | ✅ |
| literal | `f-triple f"""…"""` | ✅ | ✅ | ✅ | ✅ |
| literal | `backtick quote ```…```` | ✅ | ✅ | ✅ | ✅ |
| literal | `escape \u{...}` | ✅ | ✅ | ✅ | ✅ |
| literal | `escape \n` | ✅ | ✅ | ✅ | ✅ |
| comment | `line comment //` | ✅ | ✅ | ✅ | ✅ |
| comment | `block comment /* */` | ✅ | ✅ | ✅ | ✅ |
| special | `magic method __init__` | ✅ | ✅ | ✅ | ✅ |
| special | `unicode identifier café` | ✅ | ✅ | ✅ | ✅ |
| special | `wildcard underscore _` | ✅ | ✅ | ✅ | ✅ |
| special | `regex literal /pat/ (duck)` | ✅ | ✅ | ✅ | ✅ |
| special | `named-arg sugar x~` | ✅ | ✅ | ✅ | ✅ |
| special | `ownership suffix x^` | ✅ | ✅ | ✅ | ✅ |

覆盖元素: 193/193

## 结论

✅ 全部检查通过：快照一致、词法元素三方映射完整。

# lz-zed 词法元素对照清单（三方映射）

> 语法基准：`E:\IDEProjects\AI\lang-zone\SYNTAX` 规范 v3.3（00-词法基础 / 附录B / 12-操作符 / 05-控制流 / 08-宏 / 15-测试 / 99-内置库）
> 本文档逐项列明 SYNTAX 文档中每个词法元素由哪条高亮规则实现，以及对应的测试样例，可据此人工抽查一致性。
> 实现文件：`grammar/grammar.js`（tree-sitter 规则）、`languages/lz/highlights.scm`（capture）、`syntaxes/lz.tmLanguage.json`（TextMate scope）、`test/fixtures/*.lz`（样例）。

## 统计

| 类别 | 元素数 | 说明 |
|-----:|-------:|------|
| 关键字（含 True/False/`...`） | 62 | 00 §1.1–1.11 + 附录B §1 |
| duck 软关键字 | 14 | 附录B §1.13（仅 duck 体内生效） |
| 内建类型（含类型值） | 22 | 99 §2.0 + 00 §1.12 |
| 内建构造器 | 4 | None / Some / Ok / Err |
| 内建函数 | 22 | 99 §2.1/2.6 + 08 |
| 数字字面量形态 | 8 | 00 §2/§2.2 |
| 字符串字面量形态 | 10 | 00 §2.1 + 08 §3.2 |
| 注释形态 | 2 | 00 §三 |
| 运算符 | 45 | 12 §1.1–1.17 + 00 §5.1 |
| 标识符/特殊标识符 | 4 | 00 §四 |
| 合计 | 193 | 全部经测试运行器自动校验三方映射 |

## 一、关键字（62）

高亮规则：
- tree-sitter：`grammar.js` 中关键字以字面量 token 出现于各语句/表达式规则；`highlights.scm` 按类捕获。
- TextMate：`lz.tmLanguage.json` `#keywords`（`keyword.*.lz`）。
- 所有关键字在 `test/fixtures/01-keywords.lz` 中各出现一次（控制流关键字另有 07/13/14 等上下文样例）。

| 元素 | tree-sitter 规则 / capture | TextMate scope | 测试样例 |
|------|---------------------------|----------------|----------|
| `def` | 各定义规则 / `@keyword` | `keyword.lz` | 01: `def greet(...)` |
| `struct` | `struct_definition` / `@keyword` | `keyword.lz` | 01: `struct Point =` |
| `enum` | `enum_definition` / `@keyword` | `keyword.lz` | 01: `enum Color =` |
| `trait` | `trait_definition` / `@keyword` | `keyword.lz` | 01: `trait Drawable =` |
| `impl` | `impl_definition` / `@keyword` | `keyword.lz` | 01: `impl Drawable for Point` |
| `type` | `type_alias` / `@keyword` | `keyword.lz` | 01: `type Alias = int` |
| `const` | `const_definition` / `@keyword` | `keyword.lz` | 01: `const MAX: int = 100` |
| `mut` | `let_statement`/`parameter` / `@keyword` | `keyword.lz` | 01: `mut x = 1` |
| `ref` | `let_statement`/`ref_pattern` / `@keyword` | `keyword.lz` | 01: `ref r = x` |
| `let` | `let_statement` / `@keyword` | `keyword.lz` | 01: `let y = 2` |
| `owned` | `let_statement`/`parameter` / `@keyword` | `keyword.lz` | 01: `owned z = y^` |
| `magic` | `magic_block` / `@keyword` | `keyword.lz` | 01: `magic __str__:` |
| `duck` | `duck_definition` / `@keyword` | `keyword.lz`（duck 体内另见 §二） | 01: `duck Shape =` |
| `iterator` | `iterator_definition` / `@keyword` | `keyword.lz` | 01: `iterator gen() -> int` |
| `if` | `if_statement`/`ternary_expression` / `@keyword.control` | `keyword.control.lz` | 01/07: `if n > 0:` |
| `elif` | `elif_statement` / `@keyword.control` | `keyword.control.lz` | 01/07: `elif n < 0:` |
| `else` | `else_statement`/`ternary_expression` / `@keyword.control` | `keyword.control.lz` | 01/07: `else:` |
| `match` | `match_expression`/`case_statement` / `@keyword.control` | `keyword.control.lz` | 01/07: `match x:` |
| `case` | `case_statement` / `@keyword.control` | `keyword.control.lz` | 01/07: `case 1 => "one"` |
| `guard` | `guard_statement` / `@keyword.control` | `keyword.control.lz` | 07: `guard b != 0 else 0` |
| `for` | `for_statement`/`declarative_for` / `@keyword.control` | `keyword.control.lz` | 01/07: `for i in xs:` |
| `while` | `while_statement` / `@keyword.control` | `keyword.control.lz` | 01/07: `while x > 0:` |
| `loop` | `loop_statement` / `@keyword.control` | `keyword.control.lz` | 01/07: `loop:` |
| `block` | `block_statement` / `@keyword.control` | `keyword.control.lz` | 07: `block outer:` |
| `pass` | `pass_statement` / `@keyword.control` | `keyword.control.lz` | 01: `pass` |
| `break` | `break_statement` / `@keyword.control` | `keyword.control.lz` | 07: `break outer` / `break with i` |
| `continue` | `continue_statement` / `@keyword.control` | `keyword.control.lz` | 07: `continue` |
| `return` | `return_statement` / `@keyword.control` | `keyword.control.lz` | 07: `return 42` |
| `with` | `with_statement` / `@keyword.control` | `keyword.control.lz` | 07: `with open(path) as f:` |
| `defer` | `defer_statement` / `@keyword.control` | `keyword.control.lz` | 07: `defer cleanup()` |
| `raise` | `raise_statement` / `@keyword.exception` | `keyword.exception.lz` | 13: `raise ValueError(...)` |
| `raises` | `raises_statement` / `@keyword.exception` | `keyword.exception.lz` | 13: `def risky() -> int raises ValueError` |
| `try` | `try_statement` / `@keyword.exception` | `keyword.exception.lz` | 13: `try:` |
| `catch` | `catch_statement` / `@keyword.exception` | `keyword.exception.lz` | 13: `catch IOError:` |
| `finally` | `finally_statement` / `@keyword.exception` | `keyword.exception.lz` | 13: `finally:` |
| `async` | `function_definition`（修饰符）/ `@keyword.control` | `keyword.control.lz` | 14: `async def fetch(...)` |
| `await` | `await_expression` / `@keyword.control` | `keyword.control.lz` | 14: `await http.get(url)` |
| `spawn` | `spawn_expression` / `@keyword.control` | `keyword.control.lz` | 14: `spawn worker(1)` |
| `go` | `spawn_expression` / `@keyword.control` | `keyword.control.lz` | 14: `go heavy_task()` |
| `yield` | `yield_statement` / `@keyword` | `keyword.control.lz` | 14: `yield n` / `yield from inner` |
| `where` | `where_clause` / `@keyword` | `keyword.lz` | 08: `where T: Clone` |
| `Self` | `_type`/`type_parameters` 等 / `@keyword` | `keyword.lz` | 09: `def build(self) -> Self` |
| `macro` | `macro_definition` / `@keyword` | `keyword.lz` | 11: `macro id(...)` |
| `comptime` | `comptime_statement` / `@keyword` | `keyword.lz` | 11: `comptime:` |
| `template` | `template_definition`（与 macro 共用）/ `@keyword` | `keyword.lz` | 11: `template tpl(...)` |
| `test` | `test_definition` / `@keyword` | `keyword.lz` | 12: `test "addition":` |
| `suite` | `suite_definition` / `@keyword` | `keyword.lz` | 12: `suite MathTests:` |
| `setup` | `setup_statement` / `@keyword` | `keyword.lz` | 12: `setup:` |
| `teardown` | `teardown_statement` / `@keyword` | `keyword.lz` | 12: `teardown:` |
| `assert` | `assert_statement` / `@keyword` | `keyword.lz` | 12: `assert 1 + 1 == 2` |
| `check` | `check_statement` / `@keyword` | `keyword.lz` | 12: `check True` |
| `and` | `and_expression` / `@keyword.operator` | `keyword.operator.lz` | 01: `a and b` |
| `or` | `or_expression` / `@keyword.operator` | `keyword.operator.lz` | 01: `a or b` |
| `not` | `not_expression` / `@keyword.operator` | `keyword.operator.lz` | 01: `not a` |
| `is` | `identity_expression` / `@keyword.operator` | `keyword.operator.lz` | 01: `a is None` |
| `in` | `identity_expression`/`for_statement` / `@keyword.operator` | `keyword.operator.lz` | 01: `a in xs` |
| `import` | `import_statement` / `@keyword.import` | `keyword.import.lz` | 10: `import std.io` |
| `from` | `import_statement` / `@keyword.import` | `keyword.import.lz` | 10: `from std.io import print` |
| `as` | `import_statement`/`identity_expression` / `@keyword.import` | `keyword.import.lz` | 10: `import ... as Map` |
| `True` | `_expr` 字面量 / `@constant.builtin.boolean` | `constant.language.boolean.lz` | 02: `let yes = True` |
| `False` | `_expr` 字面量 / `@constant.builtin.boolean` | `constant.language.boolean.lz` | 02: `let no = False` |
| `...`（抽象方法标记） | `_expr` 字面量 / `@keyword` | `keyword.operator.ellipsis.lz` | 01/09: `def draw(self) -> () = ...` |

## 二、duck 软关键字（14，仅 duck 体内生效）

- tree-sitter：`grammar.js` `duck_soft_keyword` token，只在 `duck_body` 规则中可匹配 → 体外自动恢复为普通标识符；capture `@keyword.control`。
- TextMate：`#duck` 区域的 `keyword.control.duck.lz`（`begin: duck … =/:`，`end: 行首非空白`）。
- 样例：`test/fixtures/17-duck.lz`；体外恢复为标识符的样例见同文件 `let require = 1`。

| 元素 | 规则 | 测试样例 |
|------|------|----------|
| `require` | `duck_soft_keyword` / duck 区域 | 17: `require(key)` |
| `optional` | 同上 | 17: `optional(reverse)` |
| `exact` | 同上 | 17: `exact(2)` |
| `min` | 同上 | 17: `min(1)` |
| `max` | 同上 | 17: `max(3)` |
| `range` | 同上 | 17: `range(1, 3)` |
| `at_least` | 同上 | 17: `at_least(1)` |
| `at_most` | 同上 | 17: `at_most(2)` |
| `satisfies` | 同上 | 17: `satisfies Base` |
| `sealed` | 同上 | 17: `sealed` |
| `default` | 同上 | 17: `default` |
| `StackType` | 同上 | 17: `value: StackType` |
| `RefType` | 同上 | 17: `get(self) -> RefType` |
| `Any` | 同上 | 17: `left: Any` |

## 三、内建类型 / 构造器 / 函数（48，非关键字，prelude 提供）

- tree-sitter：`builtin_type` / `builtin_type_value` / `builtin_constructor` / `builtin_function` token 规则；capture 分别为 `@type.builtin` / `@type.builtin` / `@constant.builtin` / `@function.builtin`。
- TextMate：`#builtins`（`support.type.lz` / `constant.language.lz` / `support.function.lz`）。
- 样例：`09`（类型注解）、`01`/`16`（构造器）、`08`/`16`/`demo`（函数调用）。

| 分组 | 元素 | 测试样例 |
|------|------|----------|
| 内建类型 | `int` `f64` `bool` `str` | 09: `let a: int = 1` |
| 容器类型 | `List` `Dict` `Set` `Option` `Result` `Tuple` | 09: `let e: List<int> = []` |
| 指针/内部可变 | `Box` `Rc` `Arc` `Cell` `RefCell` | 09: `next: Option<Box<Node>>` |
| 其他 | `IOError` `Tokens` | 09: `Result<int, IOError>`；11: `x: Tokens` |
| 类型值 | `Never` `Unit` `Nil` `Number` | 13: `-> Never`；01: `is None` |
| 构造器 | `None` `Some` `Ok` `Err` | 09/16: `Option<str> = None`；07: `case Ok(Some(v))` |
| 内建函数 | `print` `panic` `len` `contains` `iter` `enumerate` `zip` `map` `filter` `collect` `sort` `reverse` `clone` `drop` `format` `hash` `callable` `quote` `merge_tokens` `sum` `prod` | 16: `print(n)`、`map(\|x\| x * 2)`；11: `quote(...)` |

## 四、数字字面量（8 形态）

- tree-sitter：`float` / `integer` token 规则 → `@number.float` / `@number`。
- TextMate：`#numbers`（`constant.numeric.float.lz` / `constant.numeric.integer.lz`，float 优先）。
- 样例：`test/fixtures/02-literals.lz`。

| 形态 | 规则 | 测试样例 |
|------|------|----------|
| 十进制整数 | `integer` | `42` `0` |
| 十六进制 `0x` | `integer` | `0xFF` |
| 八进制 `0o` | `integer` | `0o77` |
| 二进制 `0b` | `integer` | `0b1010` |
| 整数下划线 | `integer` | `1_000_000` |
| 小数 | `float`（点两侧必须有数字） | `3.14` |
| 科学计数 | `float` | `1e10` `2.5E+6` |
| 负指数 | `float` | `1.5e-3` |

## 五、字符串字面量（10 形态）

- tree-sitter：`string` token 规则（含前缀与三引号/反引号变体）→ `@string`。
- TextMate：`#strings`（`string.quoted.double*.lz`），转义 `#escape`（`constant.character.escape.lz`），插值 `#interpolation`（`meta.interpolation.lz`，内嵌表达式高亮）。
- 样例：`test/fixtures/03-strings.lz`。

| 形态 | 规则 | 测试样例 |
|------|------|----------|
| `"..."` | `string` | `"hello"` |
| 转义 `\n \t \\ \" \' \r \0` | `string`（tm: `#escape`） | `"\n\t\\\"\'"` |
| 转义 `\u{XXXX}` | `string`（tm: `#escape`） | `"\u{1F600}"` |
| f-string `f"..."` | `string`（tm: 插值区域） | `f"sum = {1 + 2}"` |
| f-string 转义花括号 `\{ \}` | `string`（tm: `#escape`） | `f"literal \{x\}"` |
| 原始字符串 `r"..."`（不处理转义） | `string` | `r"\d+"` |
| 多行 `"""..."""` | `string` | `"""line1\nline2"""` |
| f-多行 `f"""..."""` | `string` | `f"""x={x}"""` |
| r-多行 `r"""..."""` | `string` | `r"""\d+"""` |
| 反引号 quote 块 `` f```…``` `` / `` r```…``` ``（08 §3.2） | `string` | 03/11: `f\`\`\`...\`\`\`` |

## 六、注释（2 形态）

| 元素 | tree-sitter | TextMate | 测试样例 |
|------|-------------|----------|----------|
| `//` 单行 | `line_comment` → `@comment` | `comment.line.double-slash.lz` | 04 |
| `/* */` 块注释 | `block_comment` → `@comment` | `comment.block.lz` | 04（含多行） |
| `#` 非注释（属性宏标记） | `attribute_macro` → `@attribute` | `meta.preprocessor.attribute.lz` | 04: `#!bin macro` |

## 七、运算符（45）

- tree-sitter：各优先级表达式规则中的字面量 token → `@operator`（`@punctuation.bracket/delimiter/special` 见 §八）。
- TextMate：`#operators`（`keyword.operator.lz`），长符号优先。
- 样例：`test/fixtures/05-operators.lz`（全量）+ 16（管道/海牙/区间）。

| 类别 | 元素 | 测试样例 |
|------|------|----------|
| 赋值 | `=` | `x = 42` |
| 复合赋值 | `+=` `-=` `*=` `/=` `%=` `**=` `&=` `|=` `^=` `<<=` `>>=` | `x += 1` … |
| 海象 | `:=` | 16: `(n := len(xs))` |
| 算术 | `+` `-` `*` `/` `%` `**` | `a + b`、`2 ** 8` |
| 一元 | `+` `-` `~` `*`（解引用）`&`（取引用）`!` | `-x`、`~bits`、`*ptr`、`&val`、`!flag` |
| 比较 | `==` `!=` `<` `>` `<=` `>=` | `a == b` |
| 逻辑符号 | `&&` `\|\|` | `a && b` |
| 位运算 | `&` `\|` `^` `<<` `>>` | `a ^ b`、`a << 2` |
| 区间 | `..` `..=` | 16: `0..5`、`0..=5` |
| 管道 | `\|>` | 16: `data \|> process` |
| 空安全 | `?` `?.` `??` | 13: `read_file(path)?`；16: `user?.address?.city`；16: `?? "localhost"` |
| 所有权/命名参数糖 | `^`（后缀）`~`（后缀） | 05: `y = x^`、`name(b~, a~)` |
| 构建块 | `=:` `^:` `~:` `*:` | 15 全量 |
| 标点 | `->` `=>` | 08: `-> int`；07: `case 1 => "one"` |
| 宏/装饰 | `@` `#`（含 `#!` `#[`） | 11: `@export(Rust)`、`#!bin macro` |
| 抽象标记 | `...` | 见 §一 |

## 八、标点（TextMate scope / tree-sitter capture）

| 元素 | tree-sitter | TextMate | 测试样例 |
|------|-------------|----------|----------|
| `( )` `[ ]` `{ }` | `@punctuation.bracket` | `punctuation.section.lz` | 09/16 |
| `:` `,` | `@punctuation.delimiter` | `punctuation.separator.lz` | 05: `let t1: int = 5` |
| `.`（路径/成员） | `@punctuation.delimiter`；成员名 `@function` | `punctuation.accessor.lz` | 10: `std.io.print` |
| `\|`（闭包/位或/或模式） | `@operator` | `keyword.operator.lz` | 08: `\|x\| x + 1` |

## 九、标识符 / 特殊标识符（4）

| 元素 | tree-sitter | TextMate | 测试样例 |
|------|-------------|----------|----------|
| 普通标识符 `[a-zA-Z_][a-zA-Z0-9_]*` | `identifier` → `@variable`（def 名 `@function`、类型名 `@type`、字段名 `@field`） | `variable.lz` | 06 |
| Unicode 字母标识符 | `identifier`（规范主形式为 ASCII；Unicode 字母序列由 TextMate 路径的 `\p{L}` 覆盖，tree-sitter 路径为保证 CLI 兼容性使用 ASCII 形式） | `variable.lz`（`\p{L}`） | 06: `café` |
| 魔法方法 `__name__` | `magic_method` → `@function.magic` | `entity.name.function.magic.lz` | 06: `__init__` `__add__` |
| 下划线 `_`（通配/忽略/洞） | `identifier` → `@variable` | `variable.lz` | 06: `case _`、`let (a, _)`、`_ = expr`、`add(_, 10)` |

## 十、特殊语法形态

| 元素 | tree-sitter | TextMate | 测试样例 |
|------|-------------|----------|----------|
| 闭包 `\|params\| body`、无参 `\| \|` | `closure`（`\|` 为 `@operator`，体内按表达式高亮） | 运算符 + 表达式 pattern | 08 |
| 胖箭头块体 `\|x\| => body` | `closure` 内 `=>`（`@punctuation.special`） | `keyword.operator.lz` | 08 |
| 属性宏 `#!bin macro` / `#!export(Rust)` / `#![derive(Clone)]` | `attribute_macro` → `@attribute` | `meta.preprocessor.attribute.lz` | 04/11 |
| 装饰器 `@name(args)` | `decorator_statement` → `@attribute` + `@function` | `meta.annotation.lz` | 11 |
| duck 正则约束 `match /pat/ at_least(N)` | `regex_literal` → `@string.regex` | `string.regexp.lz`（duck 区域内） | 17: `/^get_/` |
| 变参注入 `..` `..: Tuple` `..: Dict` | `parameter`（`..` 为 `@operator`） | `keyword.operator.lz` | 08 |
| 安全分隔符 `/` `*`（签名内） | `parameter`（`@operator`） | `keyword.operator.lz` | 08: `def boundary(a: int, /, b: int, *, c: int)` |
| 命名块 `block NAME:` / `block NAME[ps]:` | `block_statement`（NAME → `@label`） | 标识符 + `keyword.control.lz` | 07 |
| 检查站 `[ps]` / `[chk]` | `block_statement` 方括号内容 | `punctuation.section.lz` | 07（`block NAME[...]` 形态） |
| 模式（变体/元组/列表/字典/范围/引用/或/rest） | `_pattern` 系列规则 | 关键字/字面量/标识符 pattern | 07 |
| 声明式 for `sum x in xs:` / `prod i in 1..n:` | `declarative_for`（`sum`/`prod` 为 `@function.builtin`） | `support.function.lz` + `keyword.control.lz` | 07 |
| 推导式 `[x * x for x in 1..10]` | `list_literal` + `comprehension` | 关键字/字面量 pattern | 16 |

## 覆盖校验方式

`node test/run-tests.js` 内置 `ELEMENTS` 清单（与本节一致，192 项），逐项自动校验：
1. `grammar.js` 包含该元素（规则存在）；
2. `lz.tmLanguage.json` 包含该元素（scope 存在）；
3. 至少一个 fixture 包含该元素（样例存在）。

任一缺失即测试失败并写入 `docs/TEST-REPORT.md`，实现"对照清单可人工抽查、机器复核"双保险。

# FIND_BUG.md — Lang-Zone Bug 挖掘测试套件
> 更新：2026-09-02 18:10（三轮语法核查后修复 11 处错误）

## 测试文件清单（44 个 .lz）

```
find_bug/
├── core/          (3 个)
│   ├── fold.lz       — fold/fold1/reduce 折叠族
│   ├── compose.lz    — compose/pipe 组合函数
│   └── unique.lz    — unique/unique_by/dedup 去重族
├── lexer/         (5 个)
│   ├── bug-escape-unicode.lz        — BUG-LX-001: \u{} 空转义
│   ├── bug-comment-nested.lz         — BUG-LX-002: 嵌套 /* */
│   ├── bug-tilde-colon-eof.lz        — BUG-LX-003: ~: 行尾悬挂
│   ├── bug-multiline-indent.lz       — BUG-LX-004: """ 公共缩进
│   └── bug-equals-colon-ambiguity.lz — BUG-LX-005: =: vs ==
├── parser/        (5 个)
│   ├── bug-top-level-build.lz        — BUG-PR-001: 顶层 x =: body
│   ├── bug-raises-return-type.lz     — BUG-PR-002: raises + -> 共存
│   ├── bug-varargs-slash.lz         — BUG-PR-003: .. 与 / 互斥
│   ├── bug-typealias-magic.lz        — BUG-PR-004: type = __add__ 报错
│   └── bug-decorator-on-var.lz      — BUG-PR-005: @ 修饰非 def
├── typer/         (4 个)
│   ├── bug-duck-generic.lz           — BUG-TY-001: duck + 泛型冲突
│   ├── bug-self-underscore.lz        — BUG-TY-002: self: Self_ 参数
│   ├── bug-params-type-erase.lz      — BUG-TY-004: __Params 擦除
│   └── bug-generic-default-conflict.lz— BUG-TY-005: T: Clone = Vec<i64>
├── ir/            (4 个)
│   ├── bug-ir-build-block.lz         — BUG-IR-001: ~: → IR 表达式
│   ├── bug-ir-defer.lz               — BUG-IR-002: defer guard IR
│   ├── bug-ir-nested-function.lz     — BUG-IR-003: 嵌套 def 提升
│   └── bug-ir-comptime.lz            — BUG-IR-005: comptime: 块位置
├── codegen/       (4 个)
│   ├── bug-codegen-varargs.lz        — BUG-CG-001: ..: → Vec Rust
│   ├── bug-codegen-call-magic.lz     — BUG-CG-002: __call__ 接线
│   ├── bug-codegen-export.lz         — BUG-CG-003: #!export → pub fn
│   └── bug-codegen-raises.lz         — BUG-CG-004: raises → Result
├── stdbridge/     (4 个)
│   ├── bug-stdbridge-time-method.lz  — BUG-SB-001: fromMillis→from_millis
│   ├── bug-stdbridge-vec-contains.lz — BUG-SB-002: contains 统一映射
│   ├── bug-stdbridge-startswith.lz   — BUG-SB-003: startsWith→starts_with
│   └── bug-stdbridge-kebab.lz        — BUG-SB-004: kebab-case 透传
├── syntax/        (4 个)
│   ├── bug-syntax-null-coalesce.lz    — BUG-SG-002: ?? 空值合并
│   ├── bug-syntax-safe-nav.lz         — BUG-SG-003: ?. 安全导航
│   ├── bug-syntax-build-return.lz     — BUG-SG-004: =: 块返回值
│   └── bug-syntax-spread.lz           — BUG-SG-005: ... 展开
└── edge/          (5 个)
    ├── bug-edge-int-overflow.lz      — BUG-EC-002: 9223372036854775808
    ├── bug-edge-empty-dict.lz         — BUG-EC-003: {} 空字典推断
    ├── bug-edge-float-scientific.lz   — BUG-EC-004: 1e308/1e-323 精度
    ├── bug-edge-type-name.lz          — BUG-EC-006: type_name() 内省
    └── bug-edge-underscore.lz         — BUG-EC-007: _ 变量名

lz_builtins/std/  (8 个库文件)
├── core_subset.lz — 自举核心（lz_all/lz_any/lz_sum_ints/lz_abs...）
├── func.lz        — fold/compose/pipe/partition/group_by/chunk/unique...
├── string.lz      — contains/starts_with/trim/split/replace/to_upper
├── list.lz        — find/count/sorted_by/join_str/window
├── dict.lz        — get_or/update/invert/group_by/count_by/from_pairs
├── iter.lz        — enumerate/take_while/scan/intersperse/chain/drop_while
├── math.lz        — abs/sign/max_int/min_int/gcd/factorial/fibonacci/primes_upto
└── option.lz      — Option<T> 工具 map/and_then/unwrap_or/both/join
```

## 编译命令（请手动执行）

```bash
# 位置：E:/IDEProjects/AI/lang-zone

# 逐文件测试（老 AST codegen）
cargo run --release --bin lzc -- file.lz

# IR codegen（如编译器支持）
cargo run --release --bin lzc -- file.lz --ir-codegen

# 调试输出
cargo run --release --bin lzc -- file.lz --emit=tokens   # 词法
cargo run --release --bin lzc -- file.lz --emit=ast      # AST
cargo run --release --bin lzc -- file.lz --emit=ir       # IR

# 批量测试
cargo test

# 示例（单个文件）
cargo run --release --bin lzc -- find_bug/core/fold.lz
cargo run --release --bin lzc -- lz_builtins/std/core_subset.lz
```

## 第三轮核查：已修复 11 处错误

| # | 文件 | 问题 | 修复 | 状态 |
|---|------|------|------|------|
| 1 | bug-codegen-call-magic | `Adder { x = 5 }` → `:` | `{ x: 5 }` | ✅ |
| 2 | bug-syntax-safe-nav | `type X struct:` | `struct X =` | ✅ |
| 3 | bug-duck-generic | `type X struct:` | 删除结构体定义 | ✅ |
| 4 | bug-ir-comptime | `comptime { }` 块内 `=` | `comptime:` 冒号块 | ✅ |
| 5 | bug-syntax-build-return | `def f():` 缺 `=` | `def f() =` | ✅ |
| 6 | bug-syntax-spread | `def f():` 缺 `=` | `def f() =` | ✅ |
| 7 | bug-varargs-slash | `def f():` 缺 `=` | `def f() =` + `int` 小写 | ✅ |
| 8 | iter.lz | `drop_while` 逻辑反转 | 修复跳过条件 | ✅ |
| 9 | math.lz | `pow_f` 调用 `pow_int` | 改 `pow(base as int, ...)` | ✅ |
| 10 | dict.lz | `grouped[5]` 应 `6` | 修正索引 + 注释 | ✅ |
| 11 | compose.lz | `pipe4` 缺 `)` | 补右括号 | ✅ |

## 语法规范（经源码核实）

| 特性 | 正确语法 | 错误写法 |
|------|----------|----------|
| struct 定义 | `struct X = field: Type` | `struct X:` / `type X struct:` |
| struct 字面量 | `Point { x: 10, y: 20 }` | `Point { x = 10 }` |
| duck 定义 | `duck X = def method(self) -> T ...` | `duck X:` |
| type 别名 | `type X = ConcreteType` | `type X (A, B)` |
| def 函数体 | `def f() = expr` 或 `def f(): body` | `def f(): expr` (缺 `=`) |
| ~: 构建块 | `~: _ % 2 == 0`（两侧空格） | `~:_` 无空格 |
| =: 构建块 | `x =: expr`（两侧空格） | `x=: expr` |
| 类型名 | `int`, `float`, `str`, `bool` | `Int`, `Float`, `str` 大小写错 |
| List泛型 | `List<int>` `List<str>` | `List<Int>` |
| comptime | `comptime: x = 1`（冒号形式） | `comptime { x = 1 }`（花括号） |
| 条件三元 | `if x < 0: -x else x` | `if x < 0 then -x else x` ⚠️ |

## ⚠️ 待实测项目（无法静态验证）

1. **三元表达式 `then` 关键字**（math.lz 使用 `if x < 0 then -x else x`）— token.rs 无 `Then` token，可能是真实语法或编译器错误
2. **`comptime:` 冒号块** — 源码有 `Token::Comptime`，但块内是否支持 let 绑定需实测
3. **`__Params::new()` 静态方法调用** — 语法未在 parser 源码中明确验证
4. **`type_name()` builtin** — `type_name` 是否为保留标识符
5. **`duck` 体内方法签名** — `duck X = def method(self) -> T` 具体 body 格式
6. **`defer guard:` 语法** — guard 是否作为语句关键字
7. **`raises` + `->` 共存** — raises 在类型注解前后的解析
8. **`#!export` 语法** — 文件头注释还是 token
9. **`vec![]` 宏** — 是否在 LZ 中存在
10. **`...` 展开运算符** — `DotDotDot` token 是否在表达式中生效

## Bug 追踪表（36 个 find_bug 用例）

| ID | 严重 | 分类 | 描述 | 状态 |
|----|------|------|------|------|
| BUG-LX-001 | P2 | lexer | `\u{}` 空转义 + emoji 解析 | ⬜ 待测 |
| BUG-LX-002 | P3 | lexer | `/* /* */ */` 嵌套块注释 | ⬜ 待测 |
| BUG-LX-003 | P1 | lexer | `~:` 行尾悬挂 LexError | ⬜ 待测 |
| BUG-LX-004 | P3 | lexer | `"""..."""` 公共缩进边界 | ⬜ 待测 |
| BUG-LX-005 | P2 | lexer | `=:` vs `==` 歧义 | ⬜ 待测 |
| BUG-PR-001 | P0 | parser | 顶层 `x =:` 多行 body | ⬜ 待测 |
| BUG-PR-002 | P2 | parser | `raises` + `->` 共存 | ⬜ 待测 |
| BUG-PR-003 | P2 | parser | `..:` 与 `/` 变参互斥 | ⬜ 待测 |
| BUG-PR-004 | P1 | parser | `type X = __add__` 应报错 | ⬜ 待测 |
| BUG-PR-005 | P2 | parser | `@decorator` 用于非函数 | ⬜ 待测 |
| BUG-TY-001 | P0 | typer | `duck` + 泛型约束冲突 | ⬜ 待测 |
| BUG-TY-002 | P2 | typer | `self: Self_` 类型注解 | ⬜ 待测 |
| BUG-TY-004 | P1 | typer | `__Params` 类型擦除 downcast | ⬜ 待测 |
| BUG-TY-005 | P2 | typer | 泛型默认 `T: Clone = Vec<int>` | ⬜ 待测 |
| BUG-IR-001 | P0 | ir | `~:` 构建块 IR 表示 | ⬜ 待测 |
| BUG-IR-002 | P1 | ir | `defer guard:` IR 表示 | ⬜ 待测 |
| BUG-IR-003 | P0 | ir | 嵌套 def 提升 / 闭包 IR | ⬜ 待测 |
| BUG-IR-005 | P1 | ir | `comptime:` 块位置 | ⬜ 待测 |
| BUG-CG-001 | P0 | codegen | `..:` 变参 Vec Rust | ⬜ 待测 |
| BUG-CG-002 | P0 | codegen | `__call__` 魔法接线 | ⬜ 待测 |
| BUG-CG-003 | P1 | codegen | `#!export` → `pub fn` | ⬜ 待测 |
| BUG-CG-004 | P1 | codegen | `raises` → `Result<T, E>` | ⬜ 待测 |
| BUG-SB-001 | P1 | stdbridge | `fromMillis` → `from_millis` | ⬜ 待测 |
| BUG-SB-002 | P1 | stdbridge | `Vec.contains` 映射 | ⬜ 待测 |
| BUG-SB-003 | P1 | stdbridge | `startsWith` → `starts_with` | ⬜ 待测 |
| BUG-SB-004 | P2 | stdbridge | kebab-case 透传 | ⬜ 待测 |
| BUG-SG-002 | P2 | syntax | `??` 空值合并 | ⬜ 待测 |
| BUG-SG-003 | P1 | syntax | `?.` 安全导航链 | ⬜ 待测 |
| BUG-SG-004 | P2 | syntax | `=:` 块返回值 | ⬜ 待测 |
| BUG-SG-005 | P2 | syntax | `...` 展开运算符 | ⬜ 待测 |
| BUG-EC-002 | P0 | edge | `9223372036854775808` i128 透传 | ⬜ 待测 |
| BUG-EC-003 | P2 | edge | `{}` 空 Dict 推断 | ⬜ 待测 |
| BUG-EC-004 | P3 | edge | `1e308` 浮点精度 | ⬜ 待测 |
| BUG-EC-006 | P2 | edge | `type_name()` 内省 | ⬜ 待测 |
| BUG-EC-007 | P3 | edge | `_` 变量名语义 | ⬜ 待测 |

**P0** = 阻塞级 / **P1** = 重要 / **P2** = 一般 / **P3** = 提示

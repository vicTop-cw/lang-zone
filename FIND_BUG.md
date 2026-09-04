# FIND_BUG.md — Lang-Zone Bug 挖掘测试套件
> 更新：2026-09-03 14:10（三轮复验：SB-001/002/003 经未提交 codegen 接线修复转 ✅，21❌→18❌；用当前二进制全量重测）

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

## Bug 追踪表（36 个 find_bug 用例）— 2026-09-03 首轮实测

实测环境：`lang-zone.exe 0.1.180-alpha`（release）+ `rustc --edition 2021 --extern lz_builtins=target/debug/liblz_builtins.rlib`。
图例：✅ 无 bug（按预期工作/正确拒绝）· ❌ 确认 bug（需工程侧修复）· 🟡 部分问题（详见实测记录）。

| ID | 严重 | 分类 | 描述 | 状态 |
|----|------|------|------|------|
| BUG-LX-001 | P2 | lexer | `\u{}` 空转义 + emoji 解析 | ✅ emoji + `\u{1F600}` 全通；`\u{}` 正确拒绝（p14/p15 探针） |
| BUG-LX-002 | P3 | lexer | `/* /* */ */` 嵌套块注释 | ❌ P3 嵌套注释内层即终止 |
| BUG-LX-003 | P1 | lexer | `~:` 行尾悬挂 LexError | ✅ 拒绝符合规范（留白约束） |
| BUG-LX-004 | P3 | lexer | `"""..."""` 公共缩进边界 | ✅ 全链路通过，语义按当前实现 |
| BUG-LX-005 | P2 | lexer | `=:` vs `==` 歧义 | ❌ P1 内联 `x =: expr` 拒绝 |
| BUG-PR-001 | P0 | parser | 顶层 `x =:` 多行 body | ❌ P1 仅函数内支持，顶层拒绝 |
| BUG-PR-002 | P2 | parser | `raises` + `->` 共存 | ❌ P2 `-> R raises E` 前置语法拒绝 |
| BUG-PR-003 | P2 | parser | `..` 与 `/` 变参互斥 | ✅ 混用正确拒绝；用例内 `..: nums: int` 非法（规范是 `nums: List<T>` 收集），拒绝方向正确 |
| BUG-PR-004 | P1 | parser | `type X = __add__` 应报错 | ✅ 正确拒绝 |
| BUG-PR-005 | P2 | parser | `@decorator` 用于非函数 | ✅ **轮次8 已修** — 装饰器后接非声明（变量/语句）解析阶段直接拒绝 |
| BUG-TY-001 | P0 | typer | `duck` + 泛型约束冲突 | ❌ P1 生成自引用 trait（E0391） |
| BUG-TY-002 | P2 | typer | `self: Self_` 类型注解 | ✅ **轮次5 已修** — 顶层 `def m(self: S, ...)` 挂 `impl S`，调用点改方法语法（E0568 消除） |
| BUG-TY-004 | P1 | typer | `__Params` 类型擦除 downcast | ❌ P1 `__Params.new()` 点调用错编 |
| BUG-TY-005 | P2 | typer | 泛型默认 `T: Clone = Vec<int>` | 🟡 语法不支持但报错误导 |
| BUG-IR-001 | P0 | ir | `~:` 构建块 IR 表示 | ❌ P1 参数位拒绝（BuildCall） |
| BUG-IR-002 | P1 | ir | `defer guard:` IR 表示 | ❌ P1 双缺陷：push 桩 + 立即执行 |
| BUG-IR-003 | P0 | ir | 嵌套 def 提升 / 闭包 IR | ❌ P0 static mut 捕获（E0530） |
| BUG-IR-005 | P1 | ir | `comptime:` 块位置 | ✅ 块解析 + 变量 const 提升折叠（p22/p23 探针：`z = 6 * 7` → `const z: i64 = 6i64 * 7i64`，运行 42） |
| BUG-CG-001 | P0 | codegen | `..:` 变参 Vec Rust | ✅ 全链路通过（`..: int` 形态） |
| BUG-CG-002 | P0 | codegen | `__call__` 魔法接线 | ✅ **轮次5 已修** — `__init__`/`__call__` 挂 impl，`add5(10)` → `add5.__call__(10)` 接线 |
| BUG-CG-003 | P1 | codegen | `#!export` → `pub fn` | ✅ 编译通过；export 语义待 ABI 验证 |
| BUG-CG-004 | P1 | codegen | `raises` → `Result<T, E>` | ❌ P1 raises 修饰被静默丢弃 |
| BUG-SB-001 | P1 | stdbridge | `fromMillis` → `from_millis` | ✅ **三轮已修**（codegen 接线，输出 1.5s/500ms） |
| BUG-SB-002 | P1 | stdbridge | `Vec.contains` 映射 | ✅ **三轮已修**（变量 receiver 补 &，输出 true/false） |
| BUG-SB-003 | P1 | stdbridge | `startsWith` → `starts_with` | ✅ **三轮已修**（camelCase 表接入，输出 true/false） |
| BUG-SB-004 | P2 | stdbridge | kebab-case 透传 | ✅ 全链路通过 |
| BUG-SG-002 | P2 | syntax | `??` 空值合并 | ✅ **轮次6 已修** — `T?` 位置自动 Some 包装（let 绑定 + struct 构造） |
| BUG-SG-003 | P1 | syntax | `?.` 安全导航链 | ✅ **轮次6 已修** — 同上 + 可空字段 `?.` 走 and_then 扁平化 |
| BUG-SG-004 | P2 | syntax | `=:` 块返回值 | ✅ 函数内全链路通过 |
| BUG-SG-005 | P2 | syntax | `...` 展开运算符 | ✅ **轮次9 已修** — 列表字面量支持 `...`/`..` 展开（`Spread` AST+IR 变体；含展开时降级为 `Vec::new()` + `extend`/`push` 块） |
| BUG-EC-002 | P0 | edge | `9223372036854775808` i128 透传 | ✅ **轮次7 已修** — 拒绝越界字面量（LZ 暂不支持 i128），仅一元负号 `-9223372036854775808` 合法透传 i64::MIN |
| BUG-EC-003 | P2 | edge | `{}` 空 Dict 推断 | ✅ 运行正常；类型推断宽度待议 |
| BUG-EC-004 | P3 | edge | `1e308` 浮点精度 | ✅ 全链路通过 |
| BUG-EC-006 | P2 | edge | `type_name()` 内省 | ❌ P1 stub 返回 i64::MAX |
| BUG-EC-007 | P3 | edge | `_` 变量名语义 | ✅ 全链路通过 |
| —（core 3 例） | — | core | fold/compose/unique 自由函数族 | ❌ P1 fn 类型参数解析失败 |

**P0** = 阻塞级 / **P1** = 重要 / **P2** = 一般 / **P3** = 提示
**实测汇总（2026-09-03 轮次 9 后）**：36 编号 = ✅22 · ❌11 · 🟡1 · 2 项三轮接线修复转正（SB-001/002/003）+ 7 项代码修复转正（CG-002/TY-002 轮次 5、SG-002/SG-003 轮次 6、EC-002 轮次 7、PR-005 轮次 8、SG-005 轮次 9）；`\u{}`、comptime 补测全绿；core 3 例仍败于 fn 类型注解解析。
⚠️ **首轮实测教训**：首轮跑在旧 release 二进制上，SB 三项的修复当时已在工作区但未重建二进制 → 误判 ❌。**判定前必须重建二进制（cargo build --release）再测**。

## 实测记录（2026-09-03，证据索引）

以下判定均经「.lz → lzc → .rs → rustc（带 lz_builtins rlib）→ 运行」全链路或最小探针复现，证据文件在 `FIND_BUG/_work_0903/`。

### ❌ 确认 bug（21 项，按修复价值排序）

| # | ID | 现象（一手证据） | 影响面 | 建议 |
|---|----|------------------|--------|------|
| 1 | BUG-IR-003 (P0) | 嵌套 def 捕获外层参数 x → 生成 `static mut x: i64` + `pub fn inner` 全局提升，`outer(x: i64)` 参数遮蔽 static 报 E0530；即便绕过遮蔽，x 也不是闭包捕获而是全局共享——**语义错误** | 闭包/嵌套函数是函数式核心；当前产物不可编译且捕获语义静默错 | codegen 支持闭包：`move` 闭包或捕获结构体；短期先报「嵌套 def 不支持捕获」 |
| 2 | BUG-TY-001 (P0) | `duck Comparable = def __lt__(self, other: Comparable)` → `trait Comparable { fn __lt__(&self, other: Comparable) }` 自引用非 dyn 兼容 → E0391 | duck 是 LZ 结构类型核心卖点 | trait 方法参数若引用 duck 自身 → 生成 `&dyn Comparable` |
| 3 | ~~BUG-CG-002 (P0)~~ **轮次5 已修 ✅** | 见下方「轮次 5」章节：`def m(self: S, ...)` 归属 `impl S`，`mut self` 透传 `&mut self`，调用点 `inc(c)` → `c.inc()` | struct 魔法方法已可用 | — |
| 4 | BUG-CG-004 (P1) | `def f() raises IOError` 编译为 `-> String` 普通 fn，raises 修饰静默丢弃；`raise "boom"` → `panic!`（运行即崩）；try/catch 语法不存在 | raises/try-catch 语义链断裂 | raises → Result<T, E> + try? 或 catch 语法糖 |
| 5 | BUG-IR-002 (P1) | defer 体**内联立即执行**（`{ push(log, "cleanup") }` 位置在块中段）+ `push(log, x)` 自由函数调被 stub 成 `fn push(i64,i64)->i64 {i64::MAX}`（E0308） | defer 语义完全缺失 | IR 建 defer 节点 → codegen Drop guard；push 需映射 Vec::push |
| 6 | ~~BUG-SB-001/003 (P1)~~ **三轮已修 ✅** | codegen 方法表已接入 camelCase 映射（startsWith/endsWith/isEmpty/fromMillis 等，`!recv_is_struct` 守卫），全链路输出正确（1.5s/500ms、true/false） | 已修复（未提交 diff，随下个 commit 入库） | — |
| 7 | ~~BUG-SB-002 (P1)~~ **三轮已修 ✅** | contains 的 `&` 规则已从「仅 ListLit 字面量」扩展到「变量 receiver 且无自定义 contains」（recv_has_custom_contains 守卫），E0308 消除 | 已修复（未提交 diff） | — |
| 8 | ~~BUG-SG-002/003 (P1)~~ **轮次6 已修 ✅** | 见下方「轮次 6」章节：`T?` 位置（let 绑定、struct 构造）自动补 `Some(..)`；`?.` 链上可空字段改用 `and_then` 扁平化（原 `map` 得 `Option<Option<T>>` → E0609） | Option 语义糖已可用 | — |
| 9 | ~~BUG-TY-002 (P1)~~ **轮次5 已修 ✅** | 同 BUG-CG-002 根因，随 self-def 归属 impl 一并修复；`def inc(mut self: Counter) -> Counter = ...; self` 尾表达式 `self` 自动 clone 为 owned | 带 self 的顶层 def 已可编译 | — |
| 10 | BUG-TY-004 (P1) | `__Params::new()` → `__Params.new`（点调用，E0423 struct 不能当值）；`p.set(0, 42)` 静默丢弃 → `()` | 运行时反射库不可用 | `::` 静态调用保留 + 方法链接线 |
| 11 | ~~BUG-EC-002 (P1)~~ **轮次7 已修 ✅** | 见下方「轮次 7」章节：`9223372036854775808` 裸字面量 / 二元减操作数 → LexError 拒绝；仅一元负号 `-9223372036854775808` 合法透传 i64::MIN | 数值边界已收敛 | — |
| 12 | BUG-EC-006 (P1) | `type_name(42)` → stub `fn type_name(i64)->i64 { i64::MAX }`，输出 9223372036854775807 | 内省函数假实现 | 实现 type_name builtin 或拒绝 |
| 13 | BUG-PR-001 (P1) | 顶层 `greet =: name:` 报「Expected Indent, got Ident("name")」——`=:` 构建块仅支持函数体内，顶层无缩进 body 语法 | 顶层构建块不可用 | parser 顶层项支持 `=:` 块 |
| 14 | BUG-LX-005 (P1) | 内联 `y =: x + 1`（`=:` 与表达式同行）被拒；`=:` 仅支持「换行缩进块」形态（p9 探针证实块形态正常返回 42） | 语法能力边界，文档需明确 | 若规范允许内联形态需实现；否则文档明确仅块形态 |
| 15 | BUG-IR-001 (P1) | `nums.filter(~: _ % 2 == 0)` 报「Unexpected token in expression: BuildCall」——加空格后仍拒绝，`~:` 仅支持「值位」不支持「参数位」 | 构建块语法位受限 | parser 表达式层支持 BuildCall 或文档明确边界 |
| 16 | BUG-PR-002 (P2) | `def f() raises IOError -> str` 拒绝；`-> str raises IOError` 也拒绝；仅 `-> str` 后**换行缩进**的 raises 可解析（parser.rs:831 在 skip_newlines 之后取 Raises） | raises 位置语法窄 | 支持 raises 与返回类型同行两种顺序 |
| 17 | ~~BUG-PR-005 (P2)~~ **轮次8 已修 ✅** | 见下方「轮次 8」章节：`@dec` 后接变量/语句 → 解析阶段 `Err`（装饰器边界护城河），不再静默丢弃 | 装饰器边界已收敛 | — |
| 18 | ~~BUG-SG-005 (P2)~~ **轮次9 已修 ✅** | 见下方「轮次 9」章节：`[0, ...a, 4]` / `[...x, ...y]` 经 `Spread` AST+IR 变体 → 列表字面量含展开时降级为 `{ let mut __spread_v = Vec::new(); __spread_v.extend(a.iter().cloned()); ...; __spread_v }`，输出 `[0,1,2,3,4]` / `[1,2,3,4]` | 展开运算符已可用 | — |
| 19 | BUG-LX-002 (P3) | `/* a /* b */ c */` 在第一个 `*/` 即终止注释，剩余 `c */` 变裸 token | 嵌套块注释不支持 | 若规范要嵌套需深度计数；否则文档明确非嵌套 |
| 20 | core 3 例 (P1) | `fold(xs: List<a>, init: b, f: fn(b, a) -> b)` 报「Expected param, got LParen」——**fn 类型注解作为参数类型不可解析**；fold 用例另有逗号解析失败（Unexpected token: Comma） | 函数类型注解全线不可用，阻塞 core/func.lz 标准库 | parser 支持 `fn(A, B) -> C` 参数类型 |
| 21 | BUG-TY-005 (🟡→按 bug 计) | `struct Container<T: Clone = List<int>>` 报「Expected type, got Gt」——泛型默认值语法不存在，但报错点在 `= List<int>` 的 `>` 处而非 `=` 处，**报错误导** | 语法不支持 + 诊断质量 | 若语法冻结不加默认值，应报「不支持泛型默认值」并指位 `=` |

### ✅ 无 bug（12 项）

| ID | 实测证据 |
|----|----------|
| BUG-LX-001 | emoji "😀" 全链路正常（p15）；`\u{}` 空转义正确拒绝（p14）；`\u41` 无花括号也正确拒绝（p16） |
| BUG-LX-003 | 行尾/无右参 `~:` 按留白规范拒绝，报错文案明确 |
| BUG-LX-004 | 多行字符串公共缩进语义按实现运行正常，s2 无缩进形态也对 |
| BUG-PR-003 | `def bad(.., /)` 正确拒绝「`/` `*` 与 `..` 不能混用」；用例内 `..: nums: int` 按规范正确拒绝（具名收集应写 `nums: List<T>`，见 03d-可变参数.md §一） |
| BUG-PR-004 | `type MyAdder = __add__` 正确拒绝「Expected type, got MagicMethod」（探针 p2 锁定）；正向 `type IntPair = (int,int)` 全链路对 |
| BUG-IR-005 | `comptime:` 块解析 + const 提升折叠（p22/p23）：`z = 6*7` → `const z: i64 = 6i64 * 7i64`，运行 42 正确 |
| BUG-CG-001 | `def sum_all(..: int)` 变参编译运行全对（sum=15） |
| BUG-CG-003 | `#!export` 函数编译运行正常（导出 ABI 语义超出本次范围） |
| BUG-SB-004 | kebab-case 路径透传全链路输出正确 |
| BUG-SG-004 | 函数内 `=:` 块返回值 30，全链路对 |
| BUG-EC-003/004/007 | 空 Dict 运行正常、1e308/1e-323 精度无损、`_` 变量正常 |

### 测试基建备忘

- rustc 段必须带 `--extern lz_builtins=target/debug/liblz_builtins.rlib`，否则 22 例假性 E0432（首轮基线的教训）。
- lzc 默认走 IR codegen；`--ir-codegen` 无需显式。
- `test_all.sh` 已修正（rlib 参数 / 临时产物目录 / release 直跑）。
- **回归守护**：`tests/find_bug_bugs.rs`（cargo test --test find_bug_bugs）——15 转正绿 + 25 #[ignore] 挂起（修复一个转正一个）；负向守护（\u{}、type=魔法方法）内联最小源码锁定。

## lib_* 12 库基线（2026-09-03 二轮实测）

全链路（lz→rs→rustc带rlib→run，断言 stdout 含 OK）：

| 库 | 状态 | 卡点 |
|----|------|------|
| lib_option / lib_pattern | ✅（v180 前已转正） | — |
| lib_sort | ✅ **本轮转正** | E0507/E0382 已被 26cd418 修复消除 |
| lib_result | ✅ **本轮转正** | and_then 泛型推断已通（v180 Result/Option 桥接） |
| lib_vector | ✅ **本轮转正** | E0599 已消除 |
| lib_closure | ✅ **本轮转正** | E0382 闭包捕获已消除 |
| lib_linked_list | ✅ **本轮转正** | E0308/E0599 已消除 |
| lib_hashmap | ❌ 挂 | E0382 borrow of moved value `key`（移动语义长尾） |
| lib_iterator | ❌ 挂 | E0599 `&mut MapIter` 无 `f` 方法（需用户 trait 参数特性） |
| lib_json | ❌ 挂 | E0308 |
| lib_string | ❌ 挂 | E0308/E0277 |
| lib_tree | ❌ 挂 | E0369 `Vec<i64> + Vec<i64>` |

**库转正进度：2 → 7 / 12**（tests/find_bug_libs.rs 已同步摘 ignore）。

## 轮次 5（2026-09-03 16:xx）：BUG-CG-002 / TY-002 修复入库 —— 顶层 self-def 挂 impl

### 根因
IR codegen 对**顶层** `def m(self: S, ...)` 一律作自由函数发射 → `fn m(&self, ...)`，
触发 Rust **E0568**（`self` 参数只允许出现在 impl/trait 关联项）。impl 块内定义的
方法路径已存在（`gen_fn_def` 的 is_method 分支），缺的只是「顶层 def 归属判定」。

### 修复（`src/ir/codegen/mod.rs`，+106 行）
1. **归属表 `self_fns: HashMap<fn名, (struct名, is_mut)>`**：新增独立 pass，
   在 `struct_method_names_map` 全量收集**之后**扫描顶层 `Item::FnDef`，
   首参名 `self` 且注解类型是本模块 struct（剥离泛型 `<>` 后按基础名匹配）→
   登记归属，并把方法名并入该 struct 的方法集合（user_plain / magic 映射守卫自动生效）。
   独立 pass 的原因：struct 定义与 def 的源码顺序任意。
2. **跳过自由函数发射**：主 item 循环中命中 `self_fns` 的 def 直接 continue。
3. **`gen_self_fn_impls`**：按 struct 分组（保持源码序）发射 `impl S { fn m(...) }`，
   复用 `gen_fn_def` 方法路径渲染 `&self` / `&mut self`（`mut self` → `&mut self` 透传）。
4. **调用点方法语法改写**：`inc(c)` → `c.inc()`、`get_count(c2)` → `c2.get_count()`，
   在通用 callee 处理（args_s 值语义 clone 注入）**之前**拦截——方法语法是借用调用。
5. **尾表达式 `self` 自动 clone**：`def inc(mut self: Counter) -> Counter = ...; self`
   的 `self` 是 `&mut self` 引用，返回 owned 需 `self.clone()`（E0308）。

### 生成证据（最小复现）
```lz
struct Counter = count: int
def get_count(self: Counter) -> int = self.count
def inc(mut self: Counter) -> Counter =
  self.count = self.count + 1
  self
def main() =
  c = Counter { count: 0 }
  c2 = inc(c)
  print(get_count(c2))
```
```rust
impl Counter {
    fn get_count(&self) -> i64 { return self.count.clone(); }
    fn inc(&mut self) -> Counter { self.count = self.count.clone() + 1i64; return self.clone(); }
}
pub fn main() {
    let mut c = Counter { count: 0i64 };
    let mut c2 = c.inc();
    println!("{:?}", c2.get_count());
}
```

### 用例修订说明（非掩盖 bug）
两处 `self` → `mut self`（`bug-codegen-call-magic.lz` 的 `__init__`、
`bug-typer-self-underscore.lz` 的 `inc`）**符合规范**：`SYNTAX/06a-struct.md:164`
明确 `mut self` = `&mut Self` 才可修改字段。原用例在 E0568 下本就编译不过，
不存在既有行为回归。

### 结果
- `tests/find_bug_bugs.rs`：`ty002_self_underscore` / `cg002_call_magic` 摘 ignore 转正。
- 全量回归 **590 passed / 0 failed / 25 ignored**（基线 588 / 25，+2 转正负向无回归）；
  `cargo check --all-targets` 0 代码 warning（日志中「拒绝访问」为 Windows 文件锁，非代码问题）。
- 剩余 ❌：**16 项**。

### 下一步建议（工程侧参考，优先级从高到低）

1. **闭包/嵌套 def 捕获**（BUG-IR-003）——牵一发动全身，建议先出 IR 设计再动手。
2. **fn 类型注解参数解析**（core 3 例）——阻塞 std/func.lz 函数式标准库。
3. **duck 自引用参数**（BUG-TY-001）——trait 方法参数引用 duck 自身 → `&dyn Comparable`（E0391）。
4. **raises 语义链**（BUG-CG-004 / PR-002）。
5. lib_* 剩余 5 库：string/json E0308 类、hashmap E0382 移动语义、tree E0369 运算符、iterator 需 trait 参数特性。
6. 其余 P2/P3 按表顺位处理。

## 轮次 6（2026-09-03 17:xx）：BUG-SG-002 / SG-003 修复入库 —— Option 自动 Some + ?. and_then 扁平化

### 根因
`T?`（`Option<T>`）位置的初始化值不会自动包 `Some(..)`，导致 Rust 侧
`Option<T> = T`（E0308）；连带 `?.` 安全导航链在可空字段上用 `map` 会得到
`Option<Option<T>>`，二次取字段报 E0609（no field on Option）。

### 修复

**A. `src/ir/codegen/helpers.rs`（新增 3 个 free fn）**
- `is_none_expr(e)`：`None_/None 变量/None 构造/Option::None` 四种 None 形态判定。
- `is_option_ty(ty)`：识别 `IrType::Option(T)` 与 `Named("Option",[T])` 两种 `T?` 表示。
- `needs_some_wrap(target, value)`：`target` 为 `T?` 且 `value` 非 Option、非 None、非
  `Any` → 需补 `Some(..)`；其余情形不包（避免误包已 Option 值 / 未定类型）。

**B. `src/ir/codegen/mod.rs`**
- **let 绑定**：`value_s` 算出后，若 `needs_some_wrap(ty, value)` 则 `Some(value_s)`
  （元组解构 / 空容器分支产出的同样是非 Option 值，一并适用）。
- **struct 构造（kwarg 路径，即 `S { x: 5 }`）**：查 `struct_fields_info` 取字段声明类型，
  可空字段 + 非空值 → `fname: Some(value)`，在递归字段 Box 处理之后、普通返回之前。
  （StructCtor 字面量分支（`ExprKind::StructCtor`）同样加了备用 wrap；kwarg 路径是实际走的路径。）

**C. `src/ir/builder.rs`（SafeNav `?.`）**
- 新增 `strip_option_ty` / `is_option_ty_ir` / 递归 `safe_nav_field_ty`：
  沿嵌套 `cfg?.db?.host` 的 `and_then(|__sn| __sn.f)` 链回溯，取到最内层字段声明类型。
- SafeNav 发射时：字段声明类型为可空（`db: DbConfig?`/`host: str?`）→ 用 `and_then`
  扁平化；普通字段 → 维持 `map`；Dict 键访问（`get` 返 Option）→ 仍 `and_then`。

### 生成证据
```rust
// bug-syntax-null-coalesce.lz
let mut z: Option<i64> = Some(10i64);   // 原 let z: Option<i64> = 10i64  E0308
// bug-syntax-safe-nav.lz
let mut cfg: Option<Config> = Some(Config { db: Some(DbConfig { host: Some("localhost".to_string()) }), logging: None });
// ?. 链扁平化（关键修复点）
(cfg).clone().and_then(move |__sn| __sn.db).and_then(move |__sn| __sn.host).unwrap_or("unknown".to_string());
```

### 结果
- `tests/find_bug_bugs.rs`：`sg002_null_coalesce` / `sg003_safe_nav` 摘 ignore 转正
  （期望串按 `{:?}` 逐参带引号输出修正为 `"None ?? 42:" 42` / `"safe nav host:" "localhost"`）。
- 全量回归 **592 passed / 0 failed / 23 ignored**（轮次 5 基线 590 / 25 → +2 转正）；
  `cargo check --all-targets` 0 代码 warning。
- 剩余 ❌：**14 项**。

## 轮次 7（2026-09-03）：BUG-EC-002 修复入库 —— i64 越界字面量拒绝

### 决策（用户拍板）
BUG-EC-002（`9223372036854775808` 溢出）按「**报错优先**」收口：LZ 暂不支持 i128，
越界字面量一律拒绝，避免静默环绕成 `i64::MIN`；保留 `-9223372036854775808`（`i64::MIN`）
经一元负号路径合法透传。

### 根因
`src/lexer/lexer.rs` 的 `read_number` 在 `num.parse::<i64>()` 失败时，原逻辑对
`9223372036854775808`（i64::MAX + 1）无差别透传为 `i64::MIN` 哨兵。裸字面量
`9223372036854775808` 会被静默环绕成 `-9223372036854775808`，运行输出无警告（数值边界静默错）。

### 修复
**`src/lexer/lexer.rs`（`read_number` 溢出分支）**
- 仅当 `9223372036854775808` 前接一元负号 `-`（且 `-` 前为合法边界字符：行首 / 空白 /
  `([{ =:,+-*/<>&|`）时，透传 `i64::MIN` 哨兵（即 `-9223372036854775808` 合法）。
- 其余情形（裸字面量、二元减操作数 `a - 9223372036854775808`）→ 发射 `LexError` 拒绝，
  提示「整数字面量超出 i64 范围（LZ 暂不支持 i128）」。
- 其他超长整数（如 `99999999999999999999999`）→ 发射 `LexError`「无效的整数（可能溢出）」。
- 位置计算修正：`read_number` 读完所有数字后 `self.pos` 指向末位之后，首位数字位置 =
  `self.pos - num.len()`，其前字符取 `wrapping_sub(1)`，再前取 `wrapping_sub(2)`。

### 生成证据
```
=== positive 9223372036854775808 (reject) ===
Parse error: 整数字面量 9223372036854775808 超出 i64 范围（LZ 暂不支持 i128），请改用更小的字面量
=== negative -9223372036854775808 (accept → i64::MIN) ===
Generated .probe/ec002b.lz -> .probe/ec002b.rs (IR codegen)
=== huge 99999999999999999999999 (reject) ===
Parse error: 无效的整数（可能溢出）: 99999999999999999999999
```

### 结果
- `tests/find_bug_bugs.rs`：`ec002_int_overflow` 由 `[ignore]` 转正为 `reject(...)`；
  `FIND_BUG/edge/bug-edge-int-overflow.lz` 改写为拒绝用例（保留 `y = 9223372036854775807`
  边界正常断言 `test_big_int_direct`）。
- EC-002 单测通过：`test ec002_int_overflow ... ok`。
- 全量回归保持 **592 passed / 0 failed / 23 ignored**；`cargo check --all-targets` 0 代码 warning。
- 剩余 ❌：**13 项**（BUG-LX-002, BUG-LX-005, BUG-PR-001, BUG-PR-002, BUG-PR-005,
  BUG-TY-001, BUG-TY-004, BUG-IR-001, BUG-IR-002, BUG-IR-003, BUG-CG-004, BUG-SG-005, BUG-EC-006）。

### 下一步建议（工程侧参考，优先级从高到低）

1. **闭包/嵌套 def 捕获**（BUG-IR-003）——牵一发动全身，建议先出 IR 设计再动手。
2. **fn 类型注解参数解析**（core 3 例）——阻塞 std/func.lz 函数式标准库。
3. **duck 自引用参数**（BUG-TY-001）——trait 方法参数引用 duck 自身 → `&dyn Comparable`（E0391）。
4. **raises 语义链**（BUG-CG-004 / PR-002）。
5. lib_* 剩余 5 库：string/json E0308 类、hashmap E0382 移动语义、tree E0369 运算符、iterator 需 trait 参数特性。
6. 其余 P2/P3 按表顺位处理。

## 轮次 8（2026-09-03）：BUG-PR-005 修复入库 —— 装饰器修饰非声明显式拒绝

### 根因
`src/parser/parser.rs` 顶层循环先解析装饰器（`while self.check(&Token::At)`），随后 `match self.peek()`。
当 `@decorator` 后接非装饰器承载项（顶层变量赋值 `x = 42`、语句等）时，命中 `_ =>` 兜底分支，
`decorators` 向量被直接丢弃 → **SILENT_PASS**（装饰器无效但变量保留、编译通过、运行无警告）。
这是装饰器边界的「负向漏洞」：本应拒绝的非法形态被放行。

### 修复
**`src/parser/parser.rs`（装饰器解析循环后、顶层 `match` 前新增护城河）**
- 若已解析出装饰器且后续 token **不属于**装饰器合法目标，直接 `return Err(...)`，不再静默丢弃。
- 合法目标白名单：`def` / `iterator` / `async` / `struct` / `enum`，以及 `comptime def`
  （`comptime` 后接非 `def` 仍拒绝，因该路径本就丢弃装饰器）。
- 宏装饰器（`@make_const!(...)` 等）在 token 层展开管线中已在解析前展开，不会以 `@` 形式到达解析层，
  故白名单不影响宏用法；全量 DEMO（含 `@export`/`@memoize`/`@overload` 等）回归全绿。

### 生成证据
```
=== @cache 修饰顶层变量（修复后应拒绝） ===
Parse error: 装饰器只能用于 def/struct/enum/async/iterator 等声明，不能用于 Ident("x")
```

### 结果
- `tests/find_bug_bugs.rs`：`pr005_decorator_on_var_negative` 摘 `#[ignore]` 转正为 `reject(...)`；
  `FIND_BUG/parser/bug-decorator-on-var.lz` 补回真实复现（`@cache\nx = 42`，原文件缺装饰器、无法复现）。
- PR-005 单测通过：`test pr005_decorator_on_var_negative ... ok`。
- 回归：`find_bug_bugs` 24 passed / 16 ignored（pr005 由 ignore→pass）；`lz_frontend_bootstrap`
  4 passed；`reject_errors` / `reject_more` / `lexer_parser_core` / `lz_semantic_cases` 全绿；
  `cargo check --all-targets` 0 代码 warning。
- 剩余 ❌：**12 项**（BUG-LX-002, BUG-LX-005, BUG-PR-001, BUG-PR-002,
  BUG-TY-001, BUG-TY-004, BUG-IR-001, BUG-IR-002, BUG-IR-003, BUG-CG-004, BUG-SG-005, BUG-EC-006）。

## 轮次 9（2026-09-03）：BUG-SG-005 修复入库 —— 列表字面量 `...` 展开运算符

### 根因
`...`（词法 `DotDotDot`，亦接受 `..`+`Dot` 连续两 token）在词法层存在 token，但表达式层
无对应 AST/IR 节点，解析列表字面量元素时直接命中「Unexpected token in expression: DotDotDot」。
无展开能力，列表拼接只能靠 `a + b`（产生新 Vec，且类型推断弱），无法就地 `extend`。

### 修复
**1. AST + IR 新增 `Spread` 变体（语义清晰，优于在 IR 层用 BlockExpr 兜底，后者会引发 `convert_expr` 无限递归）**
- `src/ast/expr.rs`：`Expr::Spread(Box<Expr>)`。
- `src/ir/node.rs`：`ExprKind::Spread(Box<Expr>)`，注释明确「codegen 在 ListLit 内遇到 Spread 时
  降级为 `let mut v = vec![..]; v.extend(..)` 块」。

**2. 解析层 `parse_list_element`（trait + impl）**（`src/parser/expr.rs`）
- 元素先探测 `DotDotDot` 或 `Dot`+`Dot`；命中则消费并 `parse_expr()` 内层，返回 `Expr::Spread(Box::new(inner))`；
  否则走普通 `parse_expr()`。列表字面量首元素与循环元素均改走该方法。

**3. 类型推断 / 转换 / 检查补齐 `Spread` 分支**
- `src/ir/builder.rs`：
  - `infer_expr_type`：列表元素类型优先取「非展开」首元素；**全为展开时**从首个展开的
    内部元素类型推导（将 `List[T]`/`Vec[T]` 拆包为 `T`，否则误判为 `List[List[T]]`）；
  - `convert_expr`：`AstExpr::Spread(inner) => ExprKind::Spread(convert_expr(inner))`；
  - `ex_check_expr`：递归检查 inner。
- `src/semantic_check.rs`：`check_expr` 递归检查 inner。
- 其余 7 处非穷尽 match 补齐（仅递归/透传，不改变语义）：
  `ir/codegen/mod.rs`（ListLit 降级，见下）、`ir/codegen/helpers.rs`（scan_expr_auto_mut /
  expr_mentions_var）、`ir/codegen_cython.rs`（Cython `*inner`）、`ir/duck_check.rs`（walk_expr）、
  `src/ir/lz_codegen.rs`（LZ AST `Expr.Spread`）、`src/codegen/expr.rs`（旧 codegen `...inner`）。

**4. Rust codegen 降级**（`src/ir/codegen/mod.rs` ListLit 臂）
- 列表元素含任一 `ExprKind::Spread` → 生成块：
  ```rust
  {
      let mut __spread_v = Vec::new();
      __spread_v.extend(<inner>.iter().cloned());
      __spread_v.push(<elem>);   // 非展开元素按既有 .clone() 规则
      __spread_v
  }
  ```
  `__spread_v` 为块级固定名，每个含展开列表独立成块，作用域天然不冲突。

### 生成证据
```
=== bug-syntax-spread.lz（修复后全链路通过） ===
spread [0,...a,4]: [0, 1, 2, 3, 4]
spread nested: [1, 2, 3, 4]
```

### 结果
- `tests/find_bug_bugs.rs`：`sg005_spread` 摘 `#[ignore]` 转正为 `full(...)`；
  `FIND_BUG/syntax/bug-syntax-spread.lz` 保留真实复现 `[0, ...a, 4]` 与 `[...x, ...y]`。
- SG-005 单测通过：`test sg005_spread ... ok`。
- `cargo check --all-targets` 0 代码 warning。
- 剩余 ❌：**11 项**（BUG-LX-002, BUG-LX-005, BUG-PR-001, BUG-PR-002,
  BUG-TY-001, BUG-TY-004, BUG-IR-001, BUG-IR-002, BUG-IR-003, BUG-CG-004, BUG-EC-006）。

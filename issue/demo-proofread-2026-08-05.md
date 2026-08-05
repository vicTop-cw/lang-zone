# DEMO/ 文件夹 `.lz` 文件两轮纠错复查报告

> 日期：2026-08-05 · 审查人：AtomCode（deepseek-v4-flash）
> 范围：DEMO/ 下全部 **192 个 `.lz` 文件**（约 1.2 万行），含 01~16 主目录、99_errors、99_spec、boundary-coverage、combo-syntax、lz_std（14 个库文件）及根目录散件
> 方法：逐文件精读 → 对照 SYNTAX/ 规范 3.3 与缺失语法特性报告交叉验证 → 第二轮逐项复核（A 级全部二次确认）
> 结论：**A 级 15 项、B 级 14 项、C 级 5 项**；另附核对通过清单。

---

## 一、A 级 — 示例本身违反语法规范（会编译失败或运行错误）

| # | 文件 | 问题 |
|---|------|------|
| A1 | 02_types/duck_demo.lz（10 处）、04_functions/higher_order.lz L21/L44 等 | `struct MyDuck:` / `struct Calculator:` 用**冒号定义 struct**，全库共 21 处（均为非 99_errors 目录、会被 compile_demos 编译）。06a-struct.md L488-490 明确「❌ struct 使用 : 而非 =」；06-数据结构.md L35「struct → `=`」。 |
| A2 | 06_control_flow/if_elif_else.lz L14-20、05_expressions/if_match_expr.lz L8-14 | `let grade = ""` 之后 `grade = "A"` —— **let 是不可变绑定**（02-变量与绑定 L17/L66：`let x = 42` 不可重新赋值），示例却在 let 后重新赋值。 |
| A3 | 04_functions/checker.lz L5-10 | `def check_positive(ps: __Params) -> __Params =` 函数体内 `raise "must be positive"` 但**未声明 `raises`**。03c-检查站 L83「所有使用 __Params 的 checker 函数都应声明 raises」；03-函数基础 §5.2「函数体有 raise 必须声明 raises」。 |
| A4 | 05_expressions/operators.lz | ① L118 `parse.<int>("42")`、L119 `collect.<str>(items)` —— `parse`/`collect`/`items` **未定义**（文件内只有 parse_int/parse_num/collect_items，items 在 L177 才定义）；② L83 `config?.nested?.val` —— config 是 `Option<Dict<str,int>>`，Dict 无 `nested` 字段；③ main 内引用 L174 模块级 config，前向引用。 |
| A5 | 06_control_flow/while_let.lz L44 | `def count_from(...) = iterator:` —— **`iterator:` 裸内联块不是合法语法**。14-生成器仅定义 `iterator name(...) -> T =`（命名生成器）与 `func *:` 构建块，SYNTAX/ 全文无裸 `iterator:` 块形式。 |
| A6 | 07_data_structures/enum.lz L20+L29、L24+L55 | `enum Option<T>`、`enum Result<T,E>` 各**重复定义两次**（第一次带变体，第二次只含方法无变体）。06b 规定方法内联在 enum 定义内，同名 enum 只能定义一次。 |
| A7 | 01_basics/keywords.lz L67、11_concurrency/async_spawn.lz、async_more.lz、boundary-coverage/combo-async-await.lz（L19/33/58 等）、combo-syntax/combo_async_spawn.lz | 在**非 async 的 `def`/`main` 中使用 `await`**。00 L78「await 仅能在 async 函数中使用」；10 §1.2 同。经逐文件核实：上述文件的 main 或普通 def 均非 async 却 await。 |
| A8 | boundary-coverage/edge-values-boundary.lz L55 | `let hex_max: int = 0xFFFFFFFFFFFFFFFF` = 18446744073709551615，**超出 i64 上限**（9223372036854775807）。00 §2.2「整数溢出 → 编译错误（非回绕）」。示例却当合法边界值。 |
| A9 | 06_control_flow/block_demo.lz L61 | `def divide[n](a: int, b: int)` —— 挂载的 `[n]` **未定义**，应引用 L58 定义的块 `normalize`（`def divide[normalize]`）。 |
| A10 | combo-syntax/combo_ternary_walrus.lz L5 | `let v = (n := compute()) > 5 if n * 10 else 0` —— 三元 `a if cond else b` 的**条件 `n * 10` 是 int 非 bool**；且注释期望输出 70，实际若可编译得 `True`（条件 truthy 时取 `(7>5)`）。语义/类型双重错误。 |
| A11 | combo-syntax/combo_while_walrus.lz L3-8、combo_while_guard_try.lz L7-14 | `def next()/step()` 引用模块级 `count`，但 `count = 0` 写在 **main 内**（局部变量），函数体内引用的 `count` 未定义。对照 03_variables/walrus.lz L7（模块级 `count = 0`）的正确写法。 |
| A12 | 99_spec/macro_real.lz L4 | 普通文件定义 `macro json(...)` 但**无 `#!bin macro` 首行**。08 §2.1「macro 只能定义在宏模块（首行声明）」；该文件头注释自己也写了这条规则，却未遵守。 |
| A13 | 07_data_structures/module_magic.lz L21-22 | 使用 `__version__` / `__backend__` —— 06e §五 明确标注这两项为「编译环境 — **未来**」（未实现）。 |
| A14 | 01_basics/identifiers.lz L22-27 | `magic __str__:` 冒号后直接跟 `def __str__(...)` —— 06f 声明式是 `magic __map__:` + 键值对（`trait = "Map"`），方法定义式是 `magic map<T,R> = def __map__`。**两种形式混用**。且 L64-65 用 `s1.__str__()` 直调魔法方法（06d 规定走全局函数 `str(x)`/`add(a,b)`）。 |
| A15 | 07_data_structures/magic_methods.lz L110、boundary-coverage/edge-keyword-identifier.lz L44 | `magic __init__(self, value: int) -> MagicBox =` —— 06a §6.2 规定 `__init__` 签名是 `magic __init__(mut self, ...)`（无返回类型），这里无 `mut` 且返回 `-> MagicBox`，与规范（06a L381/L407）直接矛盾。 |

## 二、B 级 — DEMO 与规范/缺失报告矛盾，或规范内部矛盾在 DEMO 的体现

| # | 文件 | 问题 |
|---|------|------|
| B1 | 99_spec/keyword_downgrade.lz、boundary-coverage/edge-keyword-identifier.lz L11-14 | `let Ok = 1` / `let Some = 2` / `let None = 3` 当合法示例；但 00 §1.12/附录B「None/Some/Ok/Err 当前在**词法层保留专用 token**」。保留 token 却可作变量名，文档自相矛盾，DEMO 站「可作标识符」一边。 |
| B2 | 99_spec/comprehension_over_list.lz L2 | 注释「列表变量迭代器 — **已实现**」；缺失语法特性报告 L29/L34「**已知限制**：列表变量迭代器仍解析失败」。同一特性两种说法。 |
| B3 | 02_types/duck_demo.lz L19、04_functions/generics.lz L13、99_spec/constraint_multi.lz L10 | 使用**尖括号内联约束** `<T: Quackable>`/`<T: Ordered>`；03b-函数泛型 L18「约束不写在尖括号内，统一通过 where 子句表达」，而 01b §2.0「尖括号内联，推荐」。规范内部矛盾，DEMO 跟随 01b。 |
| B4 | 99_spec/go_stmt.lz L2-3 | 注释「go 并行**进程**」，同文件又写 `go expr → std::thread::spawn`（线程）——自相矛盾，也是 10 vs 00 规范矛盾（任务一 B2）在 DEMO 的再现。 |
| B5 | lz_std/box.lz L18-23/L238/L246 | `Box(42)`、`Rc([1,2,3])` **直接关键字构造**（`magic __new__` + 字段构造）；13 §4.2 规定用 `Box.new(42)` 装箱，99 语法边界 L494 也是 `Box.new(42)`。 |
| B6 | 06_control_flow/with_defer.lz L16、07_data_structures/magic_methods.lz L101 | `def __exit__(mut self) =` 只收 self；06d L340 文字称「`__exit__` 接收 guard 对象作为参数（**不是** &mut self）」（06d 自身 L347 示例又写 `mut self, _guard`，任务一 B10 已报；DEMO 跟随 mut self 版本，与文字矛盾）。 |
| B7 | 13_operators/precedence.lz L37-54 注释 | 注释优先级表与 12-操作符 §二**权威表不符**：注释把 `|>` 放 1 级（实际 7）、`:=` 放 2 级（实际 1）、**遗漏 `??`**（实际 8 级）、把 `is/in/as` 并入比较（实际独立 6 级）、一元/后缀/成员层级整体错位（实际 16/17 级）。 |
| B8 | lz_std/dict.lz L13、list.lz L14、set.lz L14、string.lz L17 | `def len(ref self) -> int = len(self)` —— 方法 `len` 调用全局函数 `len`，而 99 §2.6 说全局 `len(x)` 内部调用 `x.__len__()`，`__len__` 又即本方法 → **潜在无限递归**（除非编译器对内置类型特判，LZ 源码层面不自洽）。 |
| B9 | lz_std/iter.lz L302-305 vs ordering.lz L126 | `min(int,int)` 重复定义：iter.lz 定义 int 专用 `min`，ordering.lz 定义泛型 `min<T: Ord>`，prelude.lz L136 声明的是泛型版——同名函数两个签名，LZ 无重载时冲突。 |
| B10 | 99_prelude/prelude_demo.lz L65 vs lz_std/string.lz L239 | prelude 文档 `format(tmpl, args..)`（可变参数）vs lz_std 实现 `format(template: str, args: List<str>)`——DEMO 用 `format("value: {}", 42)` 传单个 int，与 lz_std 签名不符。 |
| B11 | 02_types/fallible_as.lz L12 | `def to_i32(x: int) -> i32` —— `i32` 类型未在 01 类型系统定义（只有 int/f64/str/bool）；09 §2.4 仅在文字中提及 i32。 |
| B12 | 01_basics/literals.lz L118-121、literals_more.lz L18-19 | `let r1: Range = 0..10` —— `Range` 类型未列入 01 类型系统与 99 §2.0 类型表（lz_std/iter.lz 自定义了 Range，但规范正文无此类型条目）。 |
| B13 | 99_errors/10_concurrency_errors.lz | 文件内容实为「for 循环缺冒号」，与 05_control_flow_errors.lz 重复且与「并发」主题无关（归类/命名不当）。 |
| B14 | 99_spec/iterator_demo.lz L49-55 | `iterator outer<R>(...) -> Iter<R>` 中 `yield inner(val)` 产出 `Iter<R>` → 实际 `Iter<Iter<R>>`，与 14 §8.2「-> Y 表示产出值类型」一致但极易误解；注释已说明，标注存疑（低）。 |

## 三、C 级 — 小问题 / 规范未覆盖的模糊点

| # | 文件 | 问题 |
|---|------|------|
| C1 | 04_functions/higher_order.lz L18 | `|x| -> int = x + n`：闭包作返回值时参数 `x` 无注解，03e-closure §三只规定「变量绑定必须注解/作实参可选」，返回值场景未覆盖。 |
| C2 | 06_control_flow/guard.lz L6 | `def main() -> int =` 显式返回 int；03 §3.3/01 §3.2 说 main 默认 `-> ()`（未禁止显式，但示例形式少见）。 |
| C3 | 10_error_handling/panic_raise_try.lz L168/L179 | `parse_int(s)?`、`read_file(path)?` 引用**未定义函数**（纯展示，非完整可运行）。 |
| C4 | 02_types/duck_demo.lz L15 起 | 在 `struct X:` 违反 06a 的前提下（A1），该文件头注释还称「软关键字仅 duck 体内生效」，属规范/示例双重问题。 |
| C5 | lz_std/traits.lz L608-616 | 自测试内 `values.iter().map(...)` 等链式调用依赖 lz_std 自身 Iterator trait——若编译器不支持 trait 默认方法链则测试失败（依赖实现状态，标注存疑）。 |

## 四、核对通过（抽查无问题）

- 99_spec/underscore_partial_1/2（偏应用 `add(_, 10)`）、underscore_discard_3（`_ = expr`）符合 00 §4.3。
- 99_spec/set_comprehension、dict_comprehension、gen_block_star、top_level_build、setup_teardown 均与规范一致。
- lz_std/option.lz、result.lz、error.lz、ordering.lz、math.lz 内部逻辑（含自测试）自洽。
- combo-syntax 多数文件（guard/else/walrus 组合）与 05/12 规范一致，仅 A10/A11 两处例外。
- boundary-coverage/nesting-*、prefix-suffix-stacking（括号叠写）符合 12 §1.18.1。
- 99_errors/ 各反例文件（词法/类型/变量/函数/控制流/模块/错误处理/语法）均为预期失败样例，compile_demos 正确排除该目录。

## 五、修复优先级建议

1. **A 级先行**：A1（21 处 `struct X:`）与 A2（let 重赋值）是最高频、最影响学习者的硬错误；A3/A5/A6/A7/A9 次之（均为可直接判错的语法违规）。
2. **B 级**：多数根因在规范文档矛盾（03b vs 01b、00 token 表述、06d __exit__、go 线程/进程、format 签名），需先统一规范再对齐 DEMO；lz_std 的 len 递归（B8）和 min 重定义（B9）属标准库质量问题，建议单独处理。
3. **C 级**：随 A/B 修复顺手清理。

---

*复核说明：本报告所有 A 级条目均经第二轮逐文件核对（含 A7 的 async 上下文逐行核实、A8 的字面量数值计算、A11 的变量作用域比对），未发现误报。*

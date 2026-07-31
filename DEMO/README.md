# LZ Demo 演示代码

> 对应语法规范：`SYNTAX/` 目录
> 用途：lang-zone 编译器测试基础

---

## 目录 → 规范 → 文件 映射

| 目录 | 规范 | 文件数 | 覆盖内容 |
|:----|------|:-----:|---------|
| `01_basics/` | [00-词法基础.md](../SYNTAX/00-词法基础.md) | 5 | 关键字、字面量、标识符、注释、字面量扩展 |
| `02_types/` | [01-类型系统.md](../SYNTAX/01-类型系统.md) | 6 | 基本类型、容器、Option/Result、类型别名、类型转换、别名扩展 |
| `03_variables/` | [02-变量与绑定.md](../SYNTAX/02-变量与绑定.md) | 7 | 默认可变、let、const、ref、owned/^、walrus、walrus/const 扩展 |
| `04_functions/` | [03-系列](../SYNTAX/03-函数基础.md) | 6 | def、泛型、checker、可变参数、复合（装饰器/闭包）、闭包扩展 |
| `05_expressions/` | [04-表达式.md](../SYNTAX/04-表达式.md) | 7 | 运算符、管道、推导式、if/match 表达式、空值合并、推导扩展、三元 |
| `06_control_flow/` | [05-控制流.md](../SYNTAX/05-控制流.md) | 8 | if/elif/else、match、for/while/loop、break/continue、guard、with/defer、loop 扩展、match 扩展 |
| `07_data_structures/` | [06a-06g](../SYNTAX/06-数据结构.md) | 7 | struct、enum、trait/impl、魔法方法、模块级魔法属性、struct 扩展、enum 扩展 |
| `08_modules/` | [07-模块与导入.md](../SYNTAX/07-模块与导入.md) | 2 | import/from/as、@export、bridge、import 扩展 |
| `09_macros/` | [08-宏与编译期.md](../SYNTAX/08-宏与编译期.md) + [08b](../SYNTAX/08b-comptime编译期.md) | 2 | macro/template、comptime |
| `10_error_handling/` | [09-错误处理.md](../SYNTAX/09-错误处理.md) | 2 | panic/raise/try/catch/? 传播、? 链扩展 |
| `11_concurrency/` | [10-并发与异步.md](../SYNTAX/10-并发与异步.md) | 2 | async/await、spawn、go、Futures、async 扩展 |
| `12_build_blocks/` | [11-构建块.md](../SYNTAX/11-构建块.md) | 1 | =: 变量构建块、~: 调用构建块、构建块扩展 |
| `13_operators/` | [12-操作符.md](../SYNTAX/12-操作符.md) | 2 | 运算符优先级表、空白规则、复合赋值扩展 |
| `14_pointers/` | [13-指针与引用.md](../SYNTAX/13-指针与引用.md) | 2 | Box/Rc/Arc/Cell/RefCell、ref、^、*、Rc/Arc 扩展 |
| `15_generators/` | [14-生成器.md](../SYNTAX/14-生成器.md) | 2 | yield/yield from、PartialFunc、*:、生成器扩展 |
| `16_testing/` | [15-测试框架.md](../SYNTAX/15-测试框架.md) | 2 | test/suite/assert/check、一等数据组合、测试扩展 |
| `99_prelude/` | [99-内置预导入库.md](../SYNTAX/99-内置预导入库.md) | 1 | 内置函数、str/List 方法 |
| `99_spec/` | [缺失语法特性报告](../SYNTAX/overview/缺失语法特性报告.md) | ~43 | 按规范撰写的特性目标案例，已纳入编译测试（详见 `99_spec/README.md`） |
| `combo-syntax/` | 组合语法测试 | 14 | 多语法特性组合测试用例 |

> 注：上表「文件数」为 2026-07-31 刷新。

---

## 各文件覆盖特性清单

### 01_basics/ — 词法基础

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `keywords.lz` | 82 | 全部关键字（声明/控制流/异常/异步/生成器/模块/泛型/宏/测试/运算符/字面量） |
| `literals.lz` | 122 | 全部字面量形式：十进制/十六进/八进/二进整数、浮点/科学计数、字符串、布尔/None/Some/Ok/Err/Unit/Nil、List/Tuple/Dict/Set/空 dict、区间 |
| `identifiers.lz` | 69 | 普通标识符、魔法方法名、下划线 7 种语义 |
| `comments.lz` | 41 | 单行注释 `//`、块注释 `/* */`、行内注释 |
| `literals_more.lz` | 26 | 字面量扩展测试 |

### 02_types/ — 类型系统

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `primitives.lz` | 54 | int/f64/str/bool、Unit/Never、类型注解、类型推断、Nil、Tuple、引用、函数类型 fn、指针类型 |
| `containers.lz` | 51 | List<T>、Dict<K,V>、Set<T> |
| `option_result.lz` | 27 | Option<T>/int? 简写、Result<T,E>、模式匹配、? 传播 |
| `type_aliases.lz` | 10 | 类型别名基本用法 |
| `type_conversion.lz` | 82 | `as` 转换、类型推断、函数类型 |
| `type_aliases_more.lz` | 23 | 类型别名扩展（泛型/where 约束） |

### 03_variables/ — 变量与绑定

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `mutable_let.lz` | 38 | 默认可变、let 不可变、mut no-op、解构 |
| `const.lz` | 28 | 模块级 const、编译期求值、comptime 块 |
| `ref_binding.lz` | 30 | ref（可变/不可变/字面量绑定） |
| `ownership.lz` | 25 | owned 形参、^ 后缀转移 |
| `walrus.lz` | 37 | := 在 if/while 中、模块级 count |
| `const_more.lz` | 18 | const 扩展测试 |
| `walrus_more.lz` | 20 | walrus 扩展测试 |

### 04_functions/ — 函数

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `basic.lz` | 93 | def 等式体/块体、ref/mut ref 参数、默认参数值、return、raises、@math |
| `generics.lz` | 63 | `<T>`、trait 约束、where 子句、默认泛型参数 |
| `checker.lz` | 16 | `[checker]` 语法、__Params 契约 |
| `variadic.lz` | 14 | List<T> 安全收集、.. 位置/关键字分隔 |
| `composite.lz` | 26 | 函数嵌套捕获、装饰器（@export/@memoize/@math）、闭包 |
| `closures_more.lz` | 28 | 闭包扩展测试 |

### 05_expressions/ — 表达式

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `operators.lz` | 176 | 算术/比较/逻辑/位运算、identity/membership、复合赋值（11 种）、turbofish、索引、安全导航、空值合并、错误传播、f-string |
| `pipe.lz` | 19 | `|>` 管道、链式管道 |
| `comprehension.lz` | 9 | List/Dict/Set 推导基础 |
| `if_match_expr.lz` | 36 | if/elif/else 表达式、match 表达式 |
| `null_coalesce.lz` | 16 | `??` 空值合并、`?.` 安全导航 |
| `comprehension_more.lz` | 8 | 推导扩展测试 |
| `ternary.lz` | 14 | 三元表达式 `a if c else b` |

### 06_control_flow/ — 控制流

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `if_elif_else.lz` | 21 | if/elif/else 语句+表达式、三元表达式、for/else、while/else、pass |
| `match.lz` | 89 | `:` 块体 / `=>` 内联、字符串/嵌套/枚举模式 |
| `for_while_loop.lz` | 18 | for/while/loop、range/list/dict 迭代 |
| `break_continue.lz` | 21 | break（含值）、continue、嵌套循环 |
| `guard.lz` | 41 | guard 条件（块+单行）、guard let 模式 |
| `with_defer.lz` | 65 | with `__enter__`/`__exit__`、defer LIFO 顺序 |
| `loop_demo.lz` | 25 | loop 无限循环扩展 |
| `match_more.lz` | 32 | match 扩展测试 |

### 07_data_structures/ — 数据结构

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `struct.lz` | 37 | struct 字段/方法/静态方法、嵌套构造 |
| `enum.lz` | 142 | 单元/数据/泛型变体、关键字参数构造、enum 方法、嵌套模式匹配 |
| `trait_impl.lz` | 18 | trait 抽象/默认方法、泛型 trait、trait impl |
| `magic_methods.lz` | 169 | `__add__`/`__sub__`/`__mul__`、`__str__`/`__eq__`/`__hash__`/`__getitem__`/`__len__`、`__setitem__`/`__next__`、`__call__`、`__enter__`/`__exit__`、`__new__`、`__bool__` |
| `module_magic.lz` | 12 | `__all__`、`__slots__`、`__bridge__` 等模块级魔法属性 |
| `struct_more.lz` | 27 | struct 扩展测试 |
| `enum_more.lz` | 23 | enum 扩展测试 |

### 08_modules/ → 16_testing/

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `import_demo.lz` | — | import/from/as、相对导入、@export、bridge |
| `macro_demo.lz` | 2 | `#!bin macro` 模块声明（占位） |
| `comptime_demo.lz` | 8 | comptime 块/表达式基础 |
| `panic_raise_try.lz` | 242 | panic、raise/raises、try/catch/finally/else、多 catch、? 传播、自定义异常 enum |
| `try_more.lz` | 31 | try/catch 扩展测试 |
| `async_spawn.lz` | 13 | async/await、spawn 基础 |
| `async_more.lz` | 19 | 异步扩展测试 |
| `precedence.lz` | 75 | 优先级表 0-17、空白规则、结合性示例 |
| `compound_assign_more.lz` | 16 | 复合赋值扩展测试 |
| `box_rc_arc.lz` | 14 | Box/Rc/Arc 基础用法 |
| `rc_arc_more.lz` | 15 | 指针/引用扩展测试 |
| `yield_demo.lz` | 11 | yield 基础、yield from |
| `generator_more.lz` | 24 | 生成器扩展测试 |
| `test_suite.lz` | 9 | test 基本声明、suite 框架 |
| `test_more.lz` | 20 | 测试框架扩展测试 |
| `prelude_demo.lz` | 86 | print/panic/len/str/int/float/bool/hash/contains/iter/enumerate/zip/clone/sort/reverse/format、str 方法、List 方法 |

> 注：行数为 `wc -l` 实测值（2026-07-31）。部分文件较早期大幅精简，以当前实测为准。

---
## 错误边界覆盖状态

规范中各定义 `// ❌` 错误边界示例的覆盖情况：

| 规范文件 | ❌ 总数 | DEMO 覆盖 | 未覆盖 |
|:--------|:------:|:--------:|:------|
| 00-词法基础.md | 6 | 6 ✅ | 0 |
| 01-类型系统.md | 8 | 8 ✅ | 0 |
| 02-变量与绑定.md | 9 | 9 ✅ | 0 |
| 03-函数基础.md | 4 | 4 ✅ | 0 |
| 05-控制流.md | 7 | 7 ✅ | 0 |
| 07-模块与导入.md | 5 | 5 ✅ | 0 |
| 09-错误处理.md | 7 | 7 ✅ | 0 |
| 10-并发与异步.md | 3 | 3 ✅ | 0 |
| **合计** | **49** | **49 ✅** | **0** |

> 计数刷新于 2026-07-31。所有错误边界示例已在对应 demo 中以 `// ❌` 注释形式覆盖。

---
## 设计说明

1. **模块级关键字**（struct/enum/trait/impl/type/const/magic/import/comptime/test/suite）声明在文件顶层
2. **表达式级关键字**（if/for/while/guard/return/yield 等）在 `def main()` 内演示
3. macro/template 定义在 `#!bin macro` 模块中（`09_macros/macro_demo.lz` 为占位）
4. 枚举数据变体统一使用**关键字参数**构造（`Option.Some(value: 42)`）
5. `T?` 是 `Option<T>` 的合法简写（`int?` = `Option<int>`）
6. 魔法方法 self 模式：`__add__`/`__sub__`/`__mul__` 用 `self`（owned）；`__str__`/`__eq__`/`__hash__`/`__getitem__`/`__len__` 用 `ref self`（borrowed）

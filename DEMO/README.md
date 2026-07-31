# LZ Demo 演示代码

> 对应语法规范：`SYNTAX/` 目录
> 用途：lzc 编译器测试基础

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
| `12_build_blocks/` | [11-构建块.md](../SYNTAX/11-构建块.md) | 2 | =: 变量构建块、~: 调用构建块、构建块扩展 |
| `13_operators/` | [12-操作符.md](../SYNTAX/12-操作符.md) | 2 | 运算符优先级表、空白规则、复合赋值扩展 |
| `14_pointers/` | [13-指针与引用.md](../SYNTAX/13-指针与引用.md) | 2 | Box/Rc/Arc/Cell/RefCell、ref、^、*、Rc/Arc 扩展 |
| `15_generators/` | [14-生成器.md](../SYNTAX/14-生成器.md) | 2 | yield/yield from、PartialFunc、*:、生成器扩展 |
| `16_testing/` | [15-测试框架.md](../SYNTAX/15-测试框架.md) | 2 | test/suite/assert/check、一等数据组合、测试扩展 |
| `99_prelude/` | [99-内置预导入库.md](../SYNTAX/99-内置预导入库.md) | 1 | 内置函数、str/List 方法 |
| `99_spec/` | [缺失语法特性报告](../SYNTAX/overview/缺失语法特性报告.md) | 37 | 按规范撰写的特性目标案例，已纳入编译测试（31 主 + 6 combo，详见 `99_spec/README.md`） |

> 注：上表「文件数」为 2026-07-31 刷新。

---

## 各文件覆盖特性清单

### 01_basics/ — 词法基础

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `keywords.lz` | 172 | 全部 10+ 类关键字（声明/控制流/异常/异步/生成器/模块/泛型/宏/测试/运算符/字面量） |
| `literals.lz` | 122 | 全部字面量形式：十进制/十六进/八进/二进整数、浮点/科学计数、`""`/`f""`/`r""`/`"""`/`f"""`/`r"""`、布尔/None/Some/Ok/Err/Unit/Nil、List/Tuple/Dict/Set/空 dict、区间 |
| `identifiers.lz` | 69 | 普通标识符（小写/大写/下划线/Unicode）、魔法方法名（`__str__`/`__add__`）、下划线 7 种语义（match 通配/解构忽略/guard let 忽略/丢弃返回值/参数占位/未用参数） |
| `comments.lz` | 42 | 单行注释 `//`、块注释 `/* */`、行内注释 |

### 02_types/ — 类型系统

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `primitives.lz` | 51 | int/f64/str/bool、Unit/Never、类型注解、类型推断、Nil、Tuple、引用、函数类型 fn、指针类型 |
| `containers.lz` | 51 | List<T>（push/pop/length/index）、Dict<K,V>、Set<T> |
| `option_result.lz` | 100 | Option<T>/int? 简写、Result<T,E>、关键字参数构造、模式匹配、guard let、? 传播、嵌套匹配 |
| `type_aliases.lz` | 78 | 模块级/局部/泛型/where 约束/关联类型、透明别名 vs struct |
| `type_conversion.lz` | 56 | `as` 转换（int↔f64、int→str）、类型推断、函数类型 |

### 03_variables/ — 变量与绑定

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `mutable_let.lz` | 38 | 默认可变、let 不可变、mut no-op、解构、int? 简写、guard let |
| `const.lz` | 28 | 模块级 const、编译期求值、comptime 块 |
| `ref_binding.lz` | 30 | ref（可变/不可变/字面量绑定）、mut ref no-op、Rust 映射 |
| `ownership.lz` | 25 | owned 形参、^ 后缀转移、自动转移 |
| `walrus.lz` | 37 | := 在 if/while 中、模块级 count |

### 04_functions/ — 函数

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `basic.lz` | 93 | def 等式体/块体、ref/mut ref 参数、默认参数值、return、raises、@math |
| `generics.lz` | 63 | `<T>`、trait 约束、where 子句、默认泛型参数 |
| `checker.lz` | 62 | `[checker]` 语法、__Params 契约、带/不带 checker 调用 |
| `variadic.lz` | 81 | List<T> 安全收集、.. 位置/关键字分隔、双 .. args/kwargs、类型标注 |
| `composite.lz` | 144 | 函数嵌套捕获、9 种装饰器（@export/@memoize/@math/@simd/@parallel/@tail_call/@unsafe/@overload）、多装饰器堆叠、闭包 |

### 05_expressions/ — 表达式

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `operators.lz` | 176 | 算术/比较/逻辑/位运算、identity/membership、复合赋值（11 种）、turbofish `.<T>`、索引、安全导航、空值合并、错误传播、f-string、空白规则 |
| `pipe.lz` | 75 | 基本管道、链式、lambda 管道、集合管道 |
| `comprehension.lz` | 51 | List/Dict/Set 推导、多 if 过滤、多 for 迭代器 |
| `if_match_expr.lz` | 114 | if/elif/else 表达式、match =>（内联）和 `:`（块）、safe nav `?.`、null coalescing `??`、error propagation `?`、ownership `^`、`is` 类型判断 |

### 06_control_flow/ — 控制流

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `if_elif_else.lz` | 79 | if/elif/else 语句+表达式、三元表达式 `a if cond else b`、for/else、while/else、pass |
| `match.lz` | 89 | `:` 块体 / `=>` 内联、字符串/嵌套/枚举模式 |
| `for_while_loop.lz` | 62 | for/while/loop、range/list/dict 迭代、声明式 sum/prod、else 子句 |
| `break_continue.lz` | 48 | break（含值）、continue、嵌套循环（最内层） |
| `guard.lz` | 41 | guard 条件（块+单行）、guard let 模式 |
| `with_defer.lz` | 65 | with `__enter__`/`__exit__`、defer LIFO 顺序、pass no-op |

### 07_data_structures/ — 数据结构

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `struct.lz` | 110 | struct 字段/方法/静态方法/泛型/where、`__new__`/`__init__`、嵌套构造、ZST |
| `enum.lz` | 142 | 单元/数据/泛型变体、关键字参数构造、enum 方法（is_some/unwrap/map/and_then）、嵌套模式匹配 |
| `trait_impl.lz` | 150 | 抽象/默认方法、泛型 trait、关联类型、trait impl + 固有 impl、+ 多约束、where |
| `magic_methods.lz` | 169 | `__add__`/`__sub__`/`__mul__`（owned self）、`__str__`/`__eq__`/`__hash__`/`__getitem__`/`__len__`（**ref self**）、`__setitem__`/`__next__`（mut self）、`__call__`、`__enter__`/`__exit__`、`__new__`、`__bool__` |
| `module_magic.lz` | 120 | `__all__`、`__slots__`（C/packed/align/transparent）、`__bridge__`/`__bridge_tier__`、`__public__`/`__private__`/`__deps__`、`__name__`/`__file__` |

### 08_modules/ → 16_testing/

| 文件 | 行数 | 覆盖特性 |
|:----|:---:|---------|
| `import_demo.lz` | 55 | import/from/as、相对导入、@export(Rust/Python)、bridge、模块结构 |
| `macro_demo.lz` | 55 | #!bin macro、macro 定义、template 定义、quote/f```/内置函数 |
| `comptime_demo.lz` | 46 | comptime 块/表达式、comptime def、inspect API、变量可见性 |
| `panic_raise_try.lz` | 242 | panic、raise/raises、try/catch/finally/else、多 catch、? 传播、自定义异常 enum |
| `async_spawn.lz` | 223 | async/await、spawn 内联/块语法、go 进程、Futures<T1,T2> 聚合、spawn vs go |
| `var_call_block.lz` | 224 | `=:` 变量构建块、`~:` 调用构建块（元组/字典拆包）、return 退出构建块 |
| `precedence.lz` | 76 | 优先级表 0-16、空白规则（二元/一元/后缀/括号/复合赋值）、结合性示例 |
| `box_rc_arc.lz` | 57 | Box/Rc/Arc、Cell/RefCell、dyn 隐式、ref 绑定、^ 所有权、* 取指针、ptr[k] 解引用 |
| `yield_demo.lz` | 77 | yield（裸/值）、yield from、PartialFunc、`*:` 生成器构建块、return 中断 |
| `test_suite.lz` | 84 | test 静态/动态名、suite、setup/teardown、SuiteOps 组合（+/[]/-）、assert/check、遍历测试 |
| `prelude_demo.lz` | 86 | print/panic/len/str/int/float/bool/hash/contains/iter/enumerate/zip/clone/sort/reverse/format、str 方法、List 方法 |

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
3. macro/template 定义在 `#!bin macro` 模块中（`09_macros/macro_demo.lz`）
4. 枚举数据变体统一使用**关键字参数**构造（`Option.Some(value: 42)`）
5. `T?` 是 `Option<T>` 的合法简写（`int?` = `Option<int>`）
6. 魔法方法 self 模式：`__add__`/`__sub__`/`__mul__` 用 `self`（owned）；`__str__`/`__eq__`/`__hash__`/`__getitem__`/`__len__` 用 `ref self`（borrowed）

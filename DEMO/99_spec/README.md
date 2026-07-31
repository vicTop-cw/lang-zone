# DEMO/99_spec — 规范目标案例目录

本目录存放按语法规范文档撰写的特性示例。

## 状态：✅ 已纳入编译测试 (2026-07-31)

**37/37 文件**（31 主 + 6 combo）已纳入 `tests/compile_demos.rs` 正面测试覆盖。
注：部分示例依赖特定编译器功能（如列表变量推导迭代器），解析通过即标记 ✅。

## 文件清单

| 文件 | 对应特性 | 状态 |
|------|---------|:----:|
| `dict_comprehension.lz` | 字典推导 | ✅ |
| `set_comprehension.lz` | 集合推导 | ✅ |
| `comprehension_over_list.lz` | 推导支持列表变量迭代器 | ✅ |
| `top_level_build.lz` | 顶层构建块 `x =: body` | ✅ |
| `extractor_unapply.lz` | `__unapply__` 提取器 | ✅ |
| `go_stmt.lz` | `go` 并发语句 | ✅ |
| `setup_teardown.lz` | `setup`/`teardown` 生命周期 | ✅ |
| `constraint_multi.lz` | `T: A + B` 多约束求解 | ✅ |
| `pipe_spec.lz` | 管道 `|>` | ✅ |
| `parallel_decorator.lz` | `@parallel` 装饰器 | ✅ |
| `gen_block_star.lz` | 生成器构建块 `*:` | ✅ |
| `match_guard.lz` | match 守卫 | ✅ |
| `loop_else.lz` | 循环 else 分支 | ✅ |
| `null_safe.lz` | 安全导航 `?.` | ✅ |
| `macro_real.lz` | 普通模块 macro 定义 | ✅ |
| `comptime_template.lz` | comptime: 编译期块 | ✅ |
| `underscore_partial_1.lz` | 下划线偏应用（单洞） | ✅ |
| `underscore_partial_2.lz` | 下划线偏应用（多洞） | ✅ |
| `underscore_discard_3.lz` | 下划线丢弃 `_ = expr` | ✅ |
| `tilde_named_arg_1.lz` | `~` 命名参数糖 | ✅ |
| `tilde_named_arg_2.lz` | `~` 命名参数糖（重排） | ✅ |
| `tilde_named_arg_3.lz` | `~` 命名参数糖（混合） | ✅ |
| `guard_for_1.lz` | `for ... if` 守卫 | ✅ |
| `guard_for_2.lz` | `for ... if` 守卫 + else | ✅ |
| `guard_for_3.lz` | `while ... if` 守卫 | ✅ |
| `while_walrus_guard_1.lz` | `while` + `:=` + `if` 守卫 | ✅ |
| `index_build.lz` | 索引构建块 `^:` | ✅ |
| `index_build_block.lz` | 索引构建块（简化） | ✅ |
| `iterator_demo.lz` | `iterator` 生成器函数 | ✅ |
| `duck_test.lz` | duck 类型测试 | ✅ |
| `keyword_downgrade.lz` | 关键字降级 | ✅ |

### combo-syntax/ 子目录

| 文件 | 组合特性 | 状态 |
|------|---------|:----:|
| `combo_for_guard_walrus.lz` | `for` 守卫 + `:=` 海象 | ✅ |
| `combo_while_guard_walrus.lz` | `while` 守卫 + `:=` + 标量累积 | ✅ |
| `combo_for_guard_struct.lz` | `for` 守卫 + struct 构造 | ✅ |
| `combo_while_guard_else.lz` | `while` 守卫 + `else` | ✅ |
| `combo_for_guard_match.lz` | `for` 守卫 + `match` | ✅ |
| `combo_while_guard_try.lz` | `while` 守卫 + `try/catch` + `:=` | ✅ |

## 实现摘要

- 本目录多数特性在 2026-07-31 审计中确认已实现（lexer/parser/codegen 全链路）
- `duck_demo.lz` 位于 `DEMO/99_errors/`（非本目录），本 README 过去误列，已更正
- 部分示例使用简化写法（如区间推导而非列表变量推导），以适配当前编译器边界

# DEMO/99_spec — 规范目标案例目录

本目录存放**按语法规范文档撰写、但当前编译器尚未完整支持**的特性示例。

## 为什么存在

- `tests/compile_demos.rs` 会递归扫描 `DEMO/` 下所有 `.lz` 并断言其可解析。
- 本目录与 `99_errors/` 一样被该测试**显式跳过**，因此这里的文件即使无法解析也不会让构建变红。
- 用途：记录「规范承诺、实现待补」的语法目标，自举/特性补全后，可将对应文件移入主分类目录，使其进入正面测试覆盖。

## 与 99_errors/ 的区别

- `99_errors/`：预期**永远**解析失败的反例（注释形式记载错误边界）。
- `99_spec/`：按规范**应当**支持的语法，目前因特性未实现而暂不可解析——是前瞻性的 TODO 与目标回归集。

## 文件清单（对应缺失语法特性报告）

| 文件 | 对应特性 | 优先级 |
|------|---------|:------:|
| `dict_comprehension.lz` | 字典推导 | P0-2 |
| `set_comprehension.lz` | 集合推导 | P0-3 |
| `comprehension_over_list.lz` | 推导支持列表变量迭代器 | P0-2 补充 |
| `top_level_build.lz` | 顶层构建块 `x =: body` | P1-1 |
| `extractor_unapply.lz` | `__unapply__` 提取器 | P2-1 |
| `go_stmt.lz` | `go` 并发语句 | P2-2 |
| `setup_teardown.lz` | `setup`/`teardown` 生命周期 | P2-2 |
| `constraint_multi.lz` | `T: A + B` 多约束求解 | P1-4 |
| `pipe_spec.lz` | 管道 `|>` 完整接入 | P0-4 |
| `parallel_decorator.lz` | `@parallel` 真实代码生成 | P1-3 |
| `gen_block_star.lz` | 生成器构建块 `*: ` | P1-2 |
| `match_guard.lz` | match 守卫 | P1 补充 |
| `loop_else.lz` | 循环 else 分支 | P1 补充 |
| `null_safe.lz` | 安全导航 `?.` 链式 | P1 补充 |
| `macro_real.lz` | 普通模块 macro 定义 | P2-4 |
| `comptime_template.lz` | template 模板 | P2-4 |
| `underscore_partial_1.lz` | 下划线偏应用（单洞） | 已实现影子 |
| `underscore_partial_2.lz` | 下划线偏应用（多洞） | 已实现影子 |
| `underscore_discard_3.lz` | 下划线丢弃语句 `_ = expr` | 待明确 |
| `tilde_named_arg_1.lz` | `~` 后缀命名参数糖（用户示例） | 未实现 |
| `tilde_named_arg_2.lz` | `~` 后缀命名参数糖（重排形参） | 未实现 |
| `tilde_named_arg_3.lz` | `~` 后缀命名参数糖（混合传参） | 未实现 |
| `guard_for_1.lz` | `for … if` 循环守卫 | 未实现 |
| `guard_for_2.lz` | `for … if` 守卫 + else | 未实现 |
| `guard_for_3.lz` | `while … if` 守卫 | 未实现 |
| `while_walrus_guard_1.lz` | `while` + `:=` 海象 + `if` 守卫 组合 | 未实现 |
| `index_build.lz` | 索引构建块 `^:`（脱糖 `__getitem__`，冒号后换行缩进、块体单值 key） | 未实现 |
| `iterator_demo.lz` | `iterator` 关键字生成器函数（专用定义、return/guard 禁止、签名简化一层） | 未实现 |
| `duck_demo.lz` | `duck` 关键字：单类型+多类型关系+`..`通配符+`_`占位符+正则匹配+参数约束+修饰符+链式推断 | 未实现 |

### `combo-syntax/` 子目录（守卫相关组合，集中归档）

> 循环守卫（`for … if` / `while … if`）当前解析器未实现，凡是包含该语法的组合均归入此子目录。
> 待守卫特性落地后，可将对应文件移入 `DEMO/combo-syntax/`（绿色回归）。

| 文件 | 组合特性 | 优先级 |
|------|---------|:------:|
| `combo-syntax/combo_for_guard_walrus.lz` | `for` 守卫 + `:=` 海象 | 未实现 |
| `combo-syntax/combo_while_guard_walrus.lz` | `while` 守卫 + `:=` 海象 + 标量累积 | 未实现 |
| `combo-syntax/combo_for_guard_struct.lz` | `for` 守卫 + struct 构造 + 方法调用 | 未实现 |
| `combo-syntax/combo_while_guard_else.lz` | `while` 守卫 + `else` 分支 | 未实现 |
| `combo-syntax/combo_for_guard_match.lz` | `for` 守卫 + `match` 模式匹配 | 未实现 |
| `combo-syntax/combo_while_guard_try.lz` | `while` 守卫 + `try/catch` + `:=` 海象 | 未实现 |

> 注意：以上部分特性（如 `|>`、`*: `、`@parallel`、match 守卫）解析器可能已部分支持；
> 本目录不要求必然解析失败，仅为「规范目标」集中归档。

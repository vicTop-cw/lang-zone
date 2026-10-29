# lz 语法特性测试矩阵报告

- 日期：2026-10-29 17:18:35
- commit：0ddb6ca
- 二进制：target\debug\lang-zone.exe（86655046 字节，2026-10-28 04:48）
- 测试脚本：div-tools\syntax_matrix.ps1（判据 v2，本报告由脚本生成，勿手改）
- shell：5.1.19041.1682（powershell.exe；本机无 pwsh）

## 判据（可复核）

- 采集：子进程 stdout、stderr **分别重定向到独立临时文件**（不经 PowerShell 管道），再用
  `Get-Content -Raw -Encoding UTF8` 读回，临时文件用完即删。这样子进程 stderr 不会被
  PS 5.1 逐行包成 ErrorRecord（`FullyQualifiedErrorId : NativeCommandError`），也不会被 GBK 解码吞字节。
- 正例判据：`rc == 0`。
- 反例判据：`rc != 0` **且** 输出含「行首诊断」，并排除噪声：
  - 行首诊断（正则）：`^(Parse error|IR build error|Macro definition error|Semantic error|Lexer error|Type error|Codegen error|Link error|Compile error|Error|error[E\d]+)`
  - 排除：`NativeCommandError`、`FullyQualifiedErrorId`、`LZ LINK RECIPE`、`Generated` 路径回显、`Bridge registry`、`提示：`、`warning/note/info`。
- 旧判据（v1，已废弃）：`(2>&1 | Out-String) -match 'error'`。在 PS 5.1 下 stderr 被包成
  ErrorRecord 且带 `NativeCommandError` 字样，因此**任何产生 stderr 的样本都命中**，判据实际退化成只看 rc。
- 判据自检（可复跑）：`powershell -NoProfile -File div-tools\syntax_matrix.ps1 -SelfTest`

## 正例（DEMO 编译）

- 特性总数：40
- 正例通过：38 / 38（分母 = 非 SKIP 特性数）
- 失败：0
- SKIP：2 条（无正例文件，显式列在第 1 张表末尾与下方清单）
- 正例中 rc=0 但 stderr 非空：38 条 —— v1 旧判据会把它们全判成「诊断可读」
- 失败清单：无

### 正例明细

| 特性 | 规范文档 | 正例 | rc | 结论 | stderr 首行 |
|---|---|---|---|---|---|
| 词法-缩进块 | 00 | 01_basics\control_flow.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 词法-字符串/转义 | 00 | 01_basics\strings.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 词法-数字字面量 | 00 | 01_basics\literals.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 类型-基础类型 | 01 | 02_types\primitives.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 类型-容器 | 01 | 02_types\containers.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 类型-类型标注 | 01 | 03_variables\const.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| duck 关系约束 | 01b | 07_data_structures\duck_typing.lz | - | SKIP | 无正例文件 |
| 变量-不可变绑定 | 02 | 03_variables\const.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 变量-可变绑定 | 02 | 03_variables\mutable_let.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 变量-引用绑定 | 02 | 03_variables\ref_binding.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 变量-海象 | 02 | 03_variables\walrus.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 函数-基础 | 03 | 04_functions\basic.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 函数-泛型 | 03b | 04_functions\generics.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 函数-复合 | 03e | 04_functions\composite.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 检查站 checker | 03c | 06_control_flow\def_checker.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 可变参数 | 03d | 04_functions\varargs.lz | - | SKIP | 无正例文件 |
| 闭包 | 03e | 01_basics\functions_advanced.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 表达式-管道 | 04 | 05_expressions\pipe.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 表达式-三元 | 04 | 05_expressions\ternary.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 表达式-推导式 | 04 | 05_expressions\comprehension.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 控制流-if/elif | 05 | 06_control_flow\if_elif_else.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 控制流-循环 | 05 | 06_control_flow\for_while_loop.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 控制流-break/continue | 05 | 06_control_flow\break_continue.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 控制流-guard | 05 | 06_control_flow\guard.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| block 命名块 | 05b | 06_control_flow\block_demo.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 数据结构-struct | 06a | 07_data_structures\struct.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 数据结构-enum | 06b | 01_basics\enums.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| trait/impl | 06c | 07_data_structures\trait_impl.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 魔法方法 | 06f | 07_data_structures\magic_methods.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 自引用 Self | 06h | 07_data_structures\self_recursive.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 模块与导入 | 07 | 08_modules\import_demo.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 宏 | 08 | 09_macros\macro_demo.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| comptime | 08b | 09_macros\comptime_demo.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 错误处理 | 09 | 10_error_handling\panic_raise_try.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 并发异步 | 10 | 11_concurrency\async_spawn.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 构建块 | 11 | 12_build_blocks\var_call_block.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 操作符 | 12 | 13_operators\compound_assign_more.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 指针与引用 | 13 | 14_pointers\box_rc_arc.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 生成器 | 14 | 15_generators\yield_demo.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |
| 测试框架 | 15 | 16_testing\test_suite.lz | 0 | PASS | LZ LINK RECIPE (BUG-10): rustc --edition 2021 --extern lz_bu… |

### SKIP 清单（无正例文件，不计入分母）

| 特性 | 期望正例（不存在） | 原因 |
|---|---|---|
| duck 关系约束 | 07_data_structures\duck_typing.lz | DEMO\07_data_structures\duck_typing.lz 不存在，无正例可编译 |
| 可变参数 | 04_functions\varargs.lz | DEMO\04_functions\varargs.lz 不存在，无正例可编译 |

## 反例（探针应失败）

- 反例探针数：25
- 按预期失败：25 / 25
- 意外结果：0
- 其中 `rc != 0` 但诊断不可读：0（本口径下为 0 表示「有失败就有可读诊断」）
- 意外结果清单：无

### 反例明细

| 探针 | rc | 诊断可读 | 结论 | 诊断首行 |
|---|---|---|---|---|
| neg_closure_bad_syntax.lz | 1 | 是 | 按预期失败 | Parse error: Unexpected token in expression: Newline |
| neg_ctrl_missing_else.lz | 1 | 是 | 按预期失败 | Parse error: Unexpected token in expression: Dedent |
| neg_dict_missing_colon.lz | 1 | 是 | 按预期失败 | Parse error: 字典/集合字面量元素之间缺少逗号（IntLit(1)）；字典请使用 `key: value` 形式 |
| neg_enum_dup_variant.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: 重复枚举变体: enum E 中变体 A 出现多次 |
| neg_error_uncaught_panic_ok.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: 函数 main 体内使用了 raise 但未在签名中声明 raises（raise 必须声明 raises 类型） |
| neg_expr_bad_op.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: 非法表达式: 对字面量解引用（如 `+*` 这类非法运算符） |
| neg_expr_dangling_caret.lz | 1 | 是 | 按预期失败 | Parse error: Unexpected token in expression: Newline |
| neg_fn_bad_call_arity.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: 调用参数个数不匹配: 函数 f 需要 1 个参数，但传入了 2 个 |
| neg_fn_dup_param.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: 重复参数名: x（函数 f) |
| neg_fn_missing_colon.lz | 1 | 是 | 按预期失败 | Parse error: Expected Colon, got Ident("int") at pos 5 |
| neg_gen_yield_outside.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: 函数 f 声明了返回类型但在函数体内使用 yield（yield 只允许用于无返回类型/iterator 的生成器） |
| neg_generic_missing_t.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: 泛型函数 make 调用缺少类型参数且无实参可推断（请使用 `make<...>` 显式指定 1 个类型参数） |
| neg_import_bad_path.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: import 路径不存在: no_such_module_xyz（在当前模块目录下找不到 no_such_module_xyz.lz） |
| neg_lexer_bad_number.lz | 1 | 是 | 按预期失败 | Parse error: 非法数字字面量: 12（后跟 `a`） |
| neg_lexer_unterminated_string.lz | 1 | 是 | 按预期失败 | Parse error: 未终止的字符串字面量: "unterminated |
| neg_loop_break_outside.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: break 语句出现在循环（for/while/loop）之外 |
| neg_macro_unclosed.lz | 1 | 是 | 按预期失败 | Macro definition error: unexpected token Eof in macro body at position 0 |
| neg_match_dup_case.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: match 分支重复: 模式 int:1 出现多次 |
| neg_ref_assign_immutable.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: error[E0384]: cannot assign twice to immutable variable `x` |
| neg_struct_missing_colon.lz | 1 | 是 | 按预期失败 | Parse error: 字段/变体 `Ident("int")` 缺少冒号 `:`（期望形如 `Ident("int"): Type`） |
| neg_trait_missing_fn.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: trait Greeter 要求实现抽象方法 farewell 但 impl Greeter for Dog 未提供 |
| neg_types_mismatch_ret.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: 返回类型不匹配: 函数 f 声明返回 Int，但返回了 str 字面量 |
| neg_types_unknown_type.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: 未知类型: NotAType（位于 参数 x 的类型） |
| neg_vars_double_mut.lz | 1 | 是 | 按预期失败 | Parse error: 重复修饰符 `mut`（`let mut mut x` 非法） |
| neg_vars_unbound.lz | 1 | 是 | 按预期失败 | IR build error: IR build error: Semantic error: 未绑定变量: undefined_var |

## 缺口清单

- 无正例的特性：2 条（duck 关系约束 (07_data_structures\duck_typing.lz); 可变参数 (04_functions\varargs.lz)）
- 无反例探针的特性：40 - 25 = 15 个（粗估：探针未与特性一一映射，只作覆盖差提示）
- 未修的次要缺陷：
  - 反例探针与特性清单未建立一一映射，因此“特性级反例覆盖”仍是粗估，不是精确矩阵。
  - 正例判定只看 rc，不校验生成物内容（如 .rs 是否含预期构造）；正例“通过”不等于语义正确。
  - 脚本仍按目录约定命名探针（neg_*.lz），命名不规范的新探针不会被收集。

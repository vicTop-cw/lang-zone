# lz 语法特性测试矩阵报告

- 日期：2026-10-29 15:58
- commit：c7c33e0
- 二进制：target/lane-matrix/debug/lang-zone.exe —— CARGO_TARGET_DIR=target/lane-matrix，构建于 2026-10-29 14:41，构建时工作树 src/ 无未提交改动（HEAD=0815856；其后的 c7c33e0 仅文档提交）

## 特性清单与正例（DEMO 编译）

- 总特性数：40
- 正例通过：38 / 38
- 失败清单：无
- 跳过（无正例文件，不计入分母）：duck 关系约束 (07_data_structures\duck_typing.lz); 可变参数 (04_functions\varargs.lz)

| 特性 | 规范文档 | 正例 |
|---|---|---|
| 词法-缩进块 | 00 | 01_basics\control_flow.lz |
| 词法-字符串/转义 | 00 | 01_basics\strings.lz |
| 词法-数字字面量 | 00 | 01_basics\literals.lz |
| 类型-基础类型 | 01 | 02_types\primitives.lz |
| 类型-容器 | 01 | 02_types\containers.lz |
| 类型-类型标注 | 01 | 03_variables\const.lz |
| duck 关系约束 | 01b | 07_data_structures\duck_typing.lz |
| 变量-不可变绑定 | 02 | 03_variables\const.lz |
| 变量-可变绑定 | 02 | 03_variables\mutable_let.lz |
| 变量-引用绑定 | 02 | 03_variables\ref_binding.lz |
| 变量-海象 | 02 | 03_variables\walrus.lz |
| 函数-基础 | 03 | 04_functions\basic.lz |
| 函数-泛型 | 03b | 04_functions\generics.lz |
| 函数-复合 | 03e | 04_functions\composite.lz |
| 检查站 checker | 03c | 06_control_flow\def_checker.lz |
| 可变参数 | 03d | 04_functions\varargs.lz |
| 闭包 | 03e | 01_basics\functions_advanced.lz |
| 表达式-管道 | 04 | 05_expressions\pipe.lz |
| 表达式-三元 | 04 | 05_expressions\ternary.lz |
| 表达式-推导式 | 04 | 05_expressions\comprehension.lz |
| 控制流-if/elif | 05 | 06_control_flow\if_elif_else.lz |
| 控制流-循环 | 05 | 06_control_flow\for_while_loop.lz |
| 控制流-break/continue | 05 | 06_control_flow\break_continue.lz |
| 控制流-guard | 05 | 06_control_flow\guard.lz |
| block 命名块 | 05b | 06_control_flow\block_demo.lz |
| 数据结构-struct | 06a | 07_data_structures\struct.lz |
| 数据结构-enum | 06b | 01_basics\enums.lz |
| trait/impl | 06c | 07_data_structures\trait_impl.lz |
| 魔法方法 | 06f | 07_data_structures\magic_methods.lz |
| 自引用 Self | 06h | 07_data_structures\self_recursive.lz |
| 模块与导入 | 07 | 08_modules\import_demo.lz |
| 宏 | 08 | 09_macros\macro_demo.lz |
| comptime | 08b | 09_macros\comptime_demo.lz |
| 错误处理 | 09 | 10_error_handling\panic_raise_try.lz |
| 并发异步 | 10 | 11_concurrency\async_spawn.lz |
| 构建块 | 11 | 12_build_blocks\var_call_block.lz |
| 操作符 | 12 | 13_operators\compound_assign_more.lz |
| 指针与引用 | 13 | 14_pointers\box_rc_arc.lz |
| 生成器 | 14 | 15_generators\yield_demo.lz |
| 测试框架 | 15 | 16_testing\test_suite.lz |

## 反例矩阵（应编译失败）

- 反例数：25
- 按预期失败：24 / 25
- 意外结果：neg_error_uncaught_panic_ok.lz

| 探针 | 说明 |
|---|---|
| neg_closure_bad_syntax.lz | 语法错误反例 |
| neg_ctrl_missing_else.lz | 语法错误反例 |
| neg_dict_missing_colon.lz | 语法错误反例 |
| neg_enum_dup_variant.lz | 语法错误反例 |
| neg_error_uncaught_panic_ok.lz | 语法错误反例 |
| neg_expr_bad_op.lz | 语法错误反例 |
| neg_expr_dangling_caret.lz | 语法错误反例 |
| neg_fn_bad_call_arity.lz | 语法错误反例 |
| neg_fn_dup_param.lz | 语法错误反例 |
| neg_fn_missing_colon.lz | 语法错误反例 |
| neg_gen_yield_outside.lz | 语法错误反例 |
| neg_generic_missing_t.lz | 语法错误反例 |
| neg_import_bad_path.lz | 语法错误反例 |
| neg_lexer_bad_number.lz | 语法错误反例 |
| neg_lexer_unterminated_string.lz | 语法错误反例 |
| neg_loop_break_outside.lz | 语法错误反例 |
| neg_macro_unclosed.lz | 语法错误反例 |
| neg_match_dup_case.lz | 语法错误反例 |
| neg_ref_assign_immutable.lz | 语法错误反例 |
| neg_struct_missing_colon.lz | 语法错误反例 |
| neg_trait_missing_fn.lz | 语法错误反例 |
| neg_types_mismatch_ret.lz | 语法错误反例 |
| neg_types_unknown_type.lz | 语法错误反例 |
| neg_vars_double_mut.lz | 语法错误反例 |
| neg_vars_unbound.lz | 语法错误反例 |

## 缺口清单

- 无正例的特性：duck 关系约束 (07_data_structures\duck_typing.lz); 可变参数 (04_functions\varargs.lz)
- 未覆盖反例的特性：True（反例覆盖核心特性子集）

## 重跑实测明细（本次重跑实测，非脚本模板产出）

判据同 `div-tools/syntax_matrix.ps1`：正例 rc==0；反例 rc!=0 且输出含 `error`。
下表额外记录退出码与首行诊断，用于让「按预期失败」可被复核。

| 探针 | rc | 结论 | 首行诊断 |
|---|---|---|---|
| neg_closure_bad_syntax.lz | 1 | 按预期失败 | Parse error: Unexpected token in expression: Newline |
| neg_ctrl_missing_else.lz | 1 | 按预期失败 | Parse error: Unexpected token in expression: Dedent |
| neg_dict_missing_colon.lz | 1 | 按预期失败 | Parse error: 字典/集合字面量元素之间缺少逗号（IntLit(1)）；字典请使用 `key: value` 形式 |
| neg_enum_dup_variant.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: 重复枚举变体: enum E 中变体 A 出现多次 |
| neg_error_uncaught_panic_ok.lz | 0 | 未按预期失败（rc=0） | Generated F:\AI\lang-zone\div-tools\syntax_probes\neg_error_uncaught_panic_ok.lz -> F:\AI\lang-zone\div-tools… |
| neg_expr_bad_op.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: 非法表达式: 对字面量解引用（如 `+*` 这类非法运算符） |
| neg_expr_dangling_caret.lz | 1 | 按预期失败 | Parse error: Unexpected token in expression: Newline |
| neg_fn_bad_call_arity.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: 调用参数个数不匹配: 函数 f 需要 1 个参数，但传入了 2 个 |
| neg_fn_dup_param.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: 重复参数名: x（函数 f) |
| neg_fn_missing_colon.lz | 1 | 按预期失败 | Parse error: Expected Colon, got Ident("int") at pos 5 |
| neg_gen_yield_outside.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: 函数 f 声明了返回类型但在函数体内使用 yield（yield 只允许用于无返回类型/iterator 的生成器） |
| neg_generic_missing_t.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: 泛型函数 make 调用缺少类型参数且无实参可推断（请使用 `make<...>` 显式指定 1 个类型参数） |
| neg_import_bad_path.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: import 路径不存在: no_such_module_xyz（在当前模块目录下找不到 no_such_module_x… |
| neg_lexer_bad_number.lz | 1 | 按预期失败 | Parse error: 非法数字字面量: 12（后跟 `a`） |
| neg_lexer_unterminated_string.lz | 1 | 按预期失败 | Parse error: 未终止的字符串字面量: "unterminated |
| neg_loop_break_outside.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: break 语句出现在循环（for/while/loop）之外 |
| neg_macro_unclosed.lz | 1 | 按预期失败 | Macro definition error: unexpected token Eof in macro body at position 0 |
| neg_match_dup_case.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: match 分支重复: 模式 int:1 出现多次 |
| neg_ref_assign_immutable.lz | 1 | 按预期失败 | IR build error: IR build error: error[E0384]: cannot assign twice to immutable variable `x` |
| neg_struct_missing_colon.lz | 1 | 按预期失败 | Parse error: 字段/变体 `Ident("int")` 缺少冒号 `:`（期望形如 `Ident("int"): Type`） |
| neg_trait_missing_fn.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: trait Greeter 要求实现抽象方法 farewell 但 impl Greeter for Dog 未提供 |
| neg_types_mismatch_ret.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: 返回类型不匹配: 函数 f 声明返回 Int，但返回了 str 字面量 |
| neg_types_unknown_type.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: 未知类型: NotAType（位于 参数 x 的类型） |
| neg_vars_double_mut.lz | 1 | 按预期失败 | Parse error: 重复修饰符 `mut`（`let mut mut x` 非法） |
| neg_vars_unbound.lz | 1 | 按预期失败 | IR build error: IR build error: Semantic error: 未绑定变量: undefined_var |

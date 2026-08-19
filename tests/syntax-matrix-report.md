# lz 语法特性测试矩阵报告

- 日期：2026-08-18 18:39
- commit：ec37e78

## 特性清单与正例（DEMO 编译）

- 总特性数：40
- 正例通过：38 / 38
- 失败清单：无

| 特性 | 规范文档 | 正例 |
|---|---|---|
| 词法-缩进块 | 00 | 01_basics\control_flow.lz | | 词法-字符串/转义 | 00 | 01_basics\strings.lz | | 词法-数字字面量 | 00 | 01_basics\literals.lz | | 类型-基础类型 | 01 | 02_types\primitives.lz | | 类型-容器 | 01 | 02_types\containers.lz | | 类型-类型标注 | 01 | 03_variables\const.lz | | duck 关系约束 | 01b | 07_data_structures\duck_typing.lz | | 变量-不可变绑定 | 02 | 03_variables\const.lz | | 变量-可变绑定 | 02 | 03_variables\mutable_let.lz | | 变量-引用绑定 | 02 | 03_variables\ref_binding.lz | | 变量-海象 | 02 | 03_variables\walrus.lz | | 函数-基础 | 03 | 04_functions\basic.lz | | 函数-泛型 | 03b | 04_functions\generics.lz | | 函数-复合 | 03e | 04_functions\composite.lz | | 检查站 checker | 03c | 06_control_flow\def_checker.lz | | 可变参数 | 03d | 04_functions\varargs.lz | | 闭包 | 03e | 01_basics\functions_advanced.lz | | 表达式-管道 | 04 | 05_expressions\pipe.lz | | 表达式-三元 | 04 | 05_expressions\ternary.lz | | 表达式-推导式 | 04 | 05_expressions\comprehension.lz | | 控制流-if/elif | 05 | 06_control_flow\if_elif_else.lz | | 控制流-循环 | 05 | 06_control_flow\for_while_loop.lz | | 控制流-break/continue | 05 | 06_control_flow\break_continue.lz | | 控制流-guard | 05 | 06_control_flow\guard.lz | | block 命名块 | 05b | 06_control_flow\block_demo.lz | | 数据结构-struct | 06a | 07_data_structures\struct.lz | | 数据结构-enum | 06b | 01_basics\enums.lz | | trait/impl | 06c | 07_data_structures\trait_impl.lz | | 魔法方法 | 06f | 07_data_structures\magic_methods.lz | | 自引用 Self | 06h | 07_data_structures\self_recursive.lz | | 模块与导入 | 07 | 08_modules\import_demo.lz | | 宏 | 08 | 09_macros\macro_demo.lz | | comptime | 08b | 09_macros\comptime_demo.lz | | 错误处理 | 09 | 10_error_handling\panic_raise_try.lz | | 并发异步 | 10 | 11_concurrency\async_spawn.lz | | 构建块 | 11 | 12_build_blocks\var_call_block.lz | | 操作符 | 12 | 13_operators\compound_assign_more.lz | | 指针与引用 | 13 | 14_pointers\box_rc_arc.lz | | 生成器 | 14 | 15_generators\yield_demo.lz | | 测试框架 | 15 | 16_testing\test_suite.lz |

## 反例矩阵（应编译失败）

- 反例数：25
- 按预期失败：8 / 25
- 意外结果：neg_dict_missing_colon.lz; neg_enum_dup_variant.lz; neg_error_uncaught_panic_ok.lz; neg_expr_bad_op.lz; neg_fn_bad_call_arity.lz; neg_fn_dup_param.lz; neg_gen_yield_outside.lz; neg_generic_missing_t.lz; neg_import_bad_path.lz; neg_lexer_bad_number.lz; neg_loop_break_outside.lz; neg_match_dup_case.lz; neg_struct_missing_colon.lz; neg_types_mismatch_ret.lz; neg_types_unknown_type.lz; neg_vars_double_mut.lz; neg_vars_unbound.lz

| 探针 | 说明 |
|---|---|
| neg_closure_bad_syntax.lz | 语法错误反例 | | neg_ctrl_missing_else.lz | 语法错误反例 | | neg_dict_missing_colon.lz | 语法错误反例 | | neg_enum_dup_variant.lz | 语法错误反例 | | neg_error_uncaught_panic_ok.lz | 语法错误反例 | | neg_expr_bad_op.lz | 语法错误反例 | | neg_expr_dangling_caret.lz | 语法错误反例 | | neg_fn_bad_call_arity.lz | 语法错误反例 | | neg_fn_dup_param.lz | 语法错误反例 | | neg_fn_missing_colon.lz | 语法错误反例 | | neg_gen_yield_outside.lz | 语法错误反例 | | neg_generic_missing_t.lz | 语法错误反例 | | neg_import_bad_path.lz | 语法错误反例 | | neg_lexer_bad_number.lz | 语法错误反例 | | neg_lexer_unterminated_string.lz | 语法错误反例 | | neg_loop_break_outside.lz | 语法错误反例 | | neg_macro_unclosed.lz | 语法错误反例 | | neg_match_dup_case.lz | 语法错误反例 | | neg_ref_assign_immutable.lz | 语法错误反例 | | neg_struct_missing_colon.lz | 语法错误反例 | | neg_trait_missing_fn.lz | 语法错误反例 | | neg_types_mismatch_ret.lz | 语法错误反例 | | neg_types_unknown_type.lz | 语法错误反例 | | neg_vars_double_mut.lz | 语法错误反例 | | neg_vars_unbound.lz | 语法错误反例 |

## 缺口清单

- 无正例的特性：见上 [SKIP] 行
- 未覆盖反例的特性：True（反例覆盖核心特性子集）

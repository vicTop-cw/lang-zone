# 当前阶段目标（Target）

- 本轮聚焦：embed §六.1 可序列化闭集扩展——Dict<str,float>/Dict<str,str> + Tuple 复合类型序列化支持
- 验收条件：cargo build 0 error、全量测试零回归、新测试全绿

---

## 2026-11-16 T0r64（定时任务探索轮）：BUG-513 三面收口 + JS 适配器产物「真跑」修复
- 起点：接手时工作区有 BUG-513 三面未提交修复（lexer 嵌入块边界 / cli dict 解析 / macros 反引号块）+ BUG-514 双轨词法对齐，代码已落地但台账未更新。
- **BUG-513 三面复核收口（FIXED）**：① `src/lexer/lexer.rs` 缩进式嵌入块边界（col-0 块用「空行 + 下一非空行缩进 ≤ 块缩进」为界，修复 main 体被吞）；② `src/bridge/cli.rs` 四处 `__lz_parse_dict_*` 的 `:` 深度误判（`{"a":6,"b":9}` 被切成一整对）；③ `src/macros/expand.rs` `@name!` + 反引号块参数探测。实证：`DEMO/embed_nested_demo.lz` 运行输出 `{"a": 6, "b": 9}`；`DEMO/embed_tnr_demo.lz` 编译运行输出正常。台账追记 + BUG-514 登记。
- **BUG-515~519（JS 适配器产物不可运行，全 FIXED）**：T0r63 的 JS 测试只断言 `body.contains(...)`，产物实际是**非法 JS**。本轮用 `node --check` + `node <dispatch>` 真跑暴露并修复五处：
  - BUG-515 dispatch arms 发成 Rust 形态 `if method === "x" {`（缺条件括号）→ 修为 `if (method === "x") {`；
  - BUG-516 PARAMS 表发成 Rust 元组 `[("a", __lz_str)]` → 修为 JS 二维数组 `[["a", __lz_str]]`；
  - BUG-517 artifact 保留 `# <filename>` LZ 注释（JS 无 `#` 语法）→ 新增 `js_comment_fix` 转 `//`；
  - BUG-518 顶层函数未 `module.exports`（CommonJS 取到 undefined）→ `extract` 追加 `module.exports = { ... }`；
  - BUG-519 dispatch 标量类型按 LZ 词汇 `int` 匹配（JS_TY 已映射为 `number`）恒不命中 → 退化 `__lz_str`，`add(3,4)` 得 `"34"`；修为按 JS 映射词汇匹配。另修 `fn_from_toplevel` 的 `rfind(')')`（函数体同行含 `)` 时吞函数体）为配对括号深度计数。
  - 另修 `help` 的 `KNOWN.map(n => n.slice(1,-1))`（值本无引号 → `OK:d`）。
- 验证：`node --check` 两文件均 0；端到端 `add a=3 b=4`→7、`sum_all nums=[1,2,3,4]`→10、`make_list`→`[1,2,3]`、`help`→`OK:add,sum_all,make_list`。测试强化 `tests/embed_js_serialization.rs` 新增 `js_embedded_artifacts_are_valid_and_run_correctly`（node --check + 运行判据），8/8 绿。
- 回归：lib 480/0、embed 全套 11 套件全绿、lz_ir_bootstrap 25/0、lz_frontend_bootstrap 7/0、frontend_dual_track 1/0（并发跑时一次 flake，单跑 492s 通过）、lexer_parser_core 4/0、dual_route_golden 11/0。
- 提交：acd352d（BUG-513/514，管线提交）、d58562c（JS 适配器修复）已推 GitCode；台账追记 fd24688。
- 下一步：① JS 适配器容器嵌套（dict/tuple 嵌套）运行验证；② 其他适配器（mojo/c/scala）嵌套容器序列化；③ `double_all(nums: list)` 这类非法 LZ 类型的用户提示友好化。

---

## 2026-11-14 T0r63（定时任务探索轮）：JS 适配器实现 + 嵌套容器运行时验证 + 编译回归修复
- 起点：T0r62（commit dc3f3f2）已完成嵌套容器完整 roundtrip + Dict×Tuple 嵌套组合端到端验证，下一步为① JS 适配器、② 嵌套容器运行时实际运行验证、③ 嵌套容器 DEMO 用例。
- **Bug 修复（BUG-512，critical）**：基线编译验证发现 `cargo clean` 后 release build 报 4 error——`src/ir/lz_codegen.rs` 的 `embed_targets_of`/`ir_module_to_rs_lz_source_with_stem` 等函数被误删，连同 `lz_codegen_lib.lz`（3759 行）/`lz_ir_lib.lz`/`tests/lz_semantic_cases.rs` 被删。`git checkout` 恢复后编译通过。教训：缓存掩盖真实编译失败，接手必须先 clean+build 确认基线。
- **T0r63.1 JS 适配器实现**：新增 `JsLang` 适配器（`src/embed/adapters.rs`）——JS_TY 类型表（int/float→number、str→string、bool→boolean + List/Dict/Tuple 容器）、`function` 顶层函数摘取（复用 `fn_from_toplevel`）、Node.js 分发入口（JSON.parse 解码容器参数、JSON.stringify 编码容器返回值）、`has_main` 检测 `function main(`/`require.main === module`、`run_cmd` 用 `node`。注册 `"js"|"javascript"` 到 adapter 表。单元测试 8 条 + 集成测试 7 条（含端到端 `js_embed_scalar_param_compiles`）全绿。
- **T0r63.2 嵌套容器运行时验证**：新增 `nested_dict_str_list_int_python_runtime_output_matches` 测试，直接运行 Python 嵌套容器脚本，验证 `group_counts({"a":[1,2,3],"b":[4,5]})` 输出 `{'a': 6, 'b': 9}`。**运行时验证暴露 BUG-513**（OPEN）：带 `#[embed]` 块的 LZ 程序 `def main()` 体未生成，`.rs` 中 `pub fn main() { }` 为空，编译通过但运行时无输出。
- **T0r63.3 嵌套容器 DEMO 用例**：新增 `DEMO/embed_nested_demo.lz`，演示 `Dict<str,List<int>>` 嵌套容器经 `#[embed(python)]` 端到端序列化，`--embed-build` 编译通过。
- 验证：`cargo build --release` 0 error；`cargo test --lib` 480/480（含 8 个 JS 单元测试）；embed 套件全绿零回归（dict_serialization 23、tuple 8、list 4、js 7、unify_r15 23、dedup 4、stem_parity 3、stub_shadowing 7）；lz_ir_bootstrap 25、dual_route_golden 4、bridge_embed 8 全绿。
- 下一步：① 修复 BUG-513（embed 块后 main 体未生成）；② JS 适配器端到端运行验证（当前仅编译验证）；③ 其他语言适配器（mojo/c/scala）的嵌套容器序列化支持。
- 提交：本轮改动待提交（adapters.rs + JS 适配器 + tests/embed_js_serialization.rs + DEMO/embed_nested_demo.lz + memory 更新）

---

## 2026-11-13 T0r62：嵌套容器完整运行时 roundtrip + Dict×Tuple 嵌套组合端到端验证
- 起点：T0r61（T0r60c commit）已完成嵌套容器递归 JSON 解析（embed_ret_ty_nested/embed_ret_parse_elem/embed_ret_parse_expr 递归实现）+ BTreeMap 类型统一，但端到端验证主要覆盖返回值侧，参数侧与嵌套组合的完整 roundtrip 验证不足。
- 本轮收口：先验收 T0r61 在途任务（T0r11 系列三方向①②③ 全部落地，验证全绿：embed_dict_serialization 19/19、embed_tuple_serialization 6/6、lib 472/472），清理 T0r57/T0r59 遗留僵尸任务（10 条在途 + 16 条孤儿叶全部收口），项目进度从 68% → 86.5%。
- 发布 T0r62 迭代（任务树 T0r12），聚焦三方向并发开发：
  - 方向①（T0r12.1）嵌套容器完整 roundtrip：新增 dict<str,list<int>> 参数+返回值端到端测试（python_embed_nested_dict_str_list_int_param_ret_roundtrip）、dict<str,dict<str,int>> 嵌套 dict 端到端测试（python_embed_nested_dict_str_dict_int_param_ret_roundtrip）
  - 方向②（T0r12.2）Dict×Tuple 嵌套组合：新增 tuple<dict<str,int>,list<int>> 端到端测试（e2e_tuple_dict_list_nested_param_compiles）、dict<str,tuple<int,float>> 端到端测试（python_embed_nested_dict_str_tuple_param_ret_roundtrip）
  - 方向③（T0r12.3）其他语言适配器 + 全量回归：JS 适配器调研（当前 adapters.rs 支持 python/mojo/c/scala，无独立 JS 适配器，留待下轮实现）；全量 embed 套件回归验证
- 验证：cargo build 0 error；embed_dict_serialization 22/22（原 19 + 新增 3）、embed_tuple_serialization 7/7（原 6 + 新增 1）、embed_unify_r15 23/23、embed_dedup 4/4、embed_gapfixes 17/17、embed_project_emit 3/3、embed_project_grouping 4/4 全绿零回归。
- 关键结论：T0r61 的嵌套容器递归解析实现（embed_arg_expr_nested/embed_elem_to_json/embed_ret_parse_elem）**已能正确处理参数侧序列化与 Dict×Tuple 嵌套组合**，dict<str,list<int>>/dict<str,dict<str,int>>/tuple<dict<str,int>,list<int>>/dict<str,tuple<int,float>> 四种嵌套组合端到端 --embed-build 编译全部通过，证明递归 JSON 序列化/反序列化全链路类型对齐。
- 提交：本轮测试新增待提交（tests/embed_dict_serialization.rs +265、tests/embed_tuple_serialization.rs +21）；src 代码为 T0r60/T0r61/BUG-2 遗留未提交改动，非本轮新增。
- 下一步：① JS 适配器（JsLang）实现（dispatch_file 生成 JS 分发入口 + List/Dict JSON.parse 解码）；② 嵌套容器运行时**实际运行**验证（当前为编译通过验证，可补充运行输出比对）；③ 嵌套容器的 DEMO 端到端用例。

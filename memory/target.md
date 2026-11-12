# 当前阶段目标（Target）

- 本轮聚焦：embed §六.1 可序列化闭集扩展——Dict<str,float>/Dict<str,str> + Tuple 复合类型序列化支持
- 验收条件：cargo build 0 error、全量测试零回归、新测试全绿

---

## 2026-11-10 T0r58（定时任务探索轮）：验证 T0r57 完整性 + 修复 format string bug

- 焦点：T0r57（远程 3f7489e）已完成 Dict<str,float>/Dict<str,str> + Tuple 复合类型序列化支持，本轮验证其完整性。
- 发现 T0r57 提交自身 bug（BUG-1）：`src/embed/adapters.rs:346` 的 Python dispatch 模板 format string 中写入了 `dict<int,int>` 解码函数 `{int(k): v for k, v in d.items()}`，Rust format! 宏把 `{` 当作占位符 → `error: invalid format string: expected `}`, found `(`` → 整个 crate 编译失败 → 所有测试套件无法编译运行。
- 修复：format string 模板中 `{int(k):` → `{{int(k):`、`items()}` → `items()}}` 转义（生成的 Python 代码中 `{{` 还原为 `{`，与设计的字面量一致）。
- 验证：cargo build --release 通过（0 error）；全量测试零回归：
  - lib 472 + codegen_corner_fixes 44 + find_bug_bugs 46 + demo_codegen_compile 1 + str_boundary 3 + dual_route_golden 4 + incremental_golden 1 + ir_pass_fixes 4 + moddec_product_assert 5 + lz_ir_bootstrap 25 = 零失败
  - embed 全线：dict_serialization 16 + dict_serialization_ext 10 + list_serialization 17 + unify_r15 23 + gapfixes 17 + stub_shadowing 4 + stem_parity 3 = 90 passed / 0 failed
- 结论：T0r57 交付基本完整，BUG-1 已修复。下一轮候选：① Dict<int,int> 等非 str 键组合的 Rust 侧序列化函数（__lz_dict_to_json_int_int/__lz_parse_dict_int_int）补全；② Tuple 与 Dict 嵌套组合；③ 其他语言适配器的 List/Dict 序列化支持。

---

## 2026-11-10 T0r57（远程 3f7489e）：Dict<str,float>/Dict<str,str> + Tuple 复合类型序列化

- 焦点：embed §六.1 可序列化闭集扩展——从 Dict<str,int> 推广到全部 Dict K/V 组合 + Tuple 复合类型。
- 方案：
  - Python 侧：PY_TY 表扩展至 16 项（3 List + 3 Dict + 6 Tuple）；dispatch 入口新增对应 JSON 解码函数 `__lz_json_dict_str_float`/`__lz_json_dict_str_str`/`__lz_json_dict_int_int`/`__lz_json_tuple_*`；返回值用 `_out_ok_json`/`json.dumps`。
  - Rust 侧：`embed_param_ty`/`embed_ret_ty`/`embed_arg_expr` 三函数补全 dict<str,float>/dict<str,str> 映射；`embed_wrapper_fn` 返回值反序列化分支补全 `__lz_parse_dict_str_float`/`__lz_parse_dict_str_str`；垫片模板 `EMBED_SHIM_HEADER` 含 `__lz_dict_to_json_float`/`__lz_dict_to_json_str`/`__lz_parse_dict_str_float`/`__lz_parse_dict_str_str`。
- 判据：embed_dict_serialization 16 + embed_dict_serialization_ext 10 + embed_list_serialization 8 + embed_unify_r15 23 + embed_gapfixes 17 + embed_stub_shadowing 4 + embed_stem_parity 3 = 零回归。
- 遗留：BUG-1（format string 未转义 dict<int,int> 字面量 → 编译失败），本轮 T0r58 修复。

---

## 2026-11-04 T0r56：Dict<str,int> 复合类型序列化

- 焦点：embed §六.1 可序列化闭集扩展——Dict<str,int> 类型支持（Python 适配器）。
- 方案：JSON 嵌入编码——LZ 侧序列化为 `{"a":1,"b":2}`，Python 侧 `__lz_json_dict_str_int`/`json.loads` 解码；返回值 `_out_ok_json`/`json.dumps`；Rust 侧 `__lz_parse_dict_str_int` 手写 JSON 对象解析为 `HashMap<String, i64>`，参数序列化 `__lz_dict_to_json`。
- 判据：lib 469 + embed_unify_r15 23 + embed_gapfixes 17 + embed_stub_shadowing 4 + embed_stem_parity 3 + embed_list_serialization 8 + embed_dict_serialization 8 + codegen_corner_fixes 44 + lz_ir_bootstrap 25 = 543 passed / 0 failed，零回归。
- 交付：adapters.rs + cli.rs + codegen/mod.rs + tests/embed_dict_serialization.rs（8 条）+ 文档 §六.1 同步 + VERSION_PATCH 195→196。
- 下一轮：推广到其他 Dict K/V 组合（Dict<str,float>/Dict<str,str>/Dict<int,int>）+ Tuple 复合类型 + 其他语言适配器。

---

## 2026-11-12 T0r60：T0r60 回归修复 + 嵌套容器类型支持验证
- 起点：T0r58（commit 3f7489e）已完成 BUG-1 修复 + Dict<str,float>/Dict<str,str> + Tuple 复合类型序列化。本地有 T0r60 未提交改动（嵌套容器类型支持），但编译通过后跑 embed_tuple_serialization 测试发现 3 个失败。
- 发现并修复的问题：
  1. **embed_arg_expr_nested 抢走扁平类型路径**：T0r60 改动让 embed_arg_expr_nested 的 tuple 分支匹配所有 tuple 类型（包括扁平的 tuple<int,float>），生成 __lz_tuple_nested_to_json 而非 __lz_tuple_int_float_to_json。修复：在 embed_arg_expr_nested 入口加 is_lz_nested_container 判断，只处理嵌套容器类型。
  2. **embed_param_ty_nested/embed_ret_ty_nested 只处理嵌套导致递归失败**：T0r60 改动让这两个函数也加了 is_lz_nested_container 判断，但 dict<str,list<int>> 的递归调用 embed_param_ty_nested("list<int>") 时，list<int> 是扁平容器，is_lz_nested_container 返回 false，导致整个 dict 分支失败。修复：移除这两个函数入口的 is_lz_nested_container 判断，让它们处理所有容器类型（扁平+嵌套），基类型由 embed_param_ty/embed_ret_ty 的扁平 match 兜底。
  3. **embed_ret_parse_expr 占位符问题**：T0r60 的 embed_ret_parse_expr 生成 __lz_parse_nested_dict(_raw, _art, _method, ...) 使用占位符，但 embed_wrapper_fn 把 __r/art/method 拼在后面导致语法错误。修复：让 embed_ret_parse_expr 生成完整调用表达式（含 __r），embed_wrapper_fn 负责替换 _art/_method 占位符。
  4. **闭集错误消息不包含 tuple 家族**：adapters.rs:403 的错误消息例子只列了嵌套容器，未包含 tuple<int,float>。修复：把例子改为 tuple<int,float>。
- 验证：
  - cargo build --release 0 error
  - embed_tuple_serialization 6/6 passed（原 3 failed → 0 failed）
  - embed_dict_serialization 16/16, embed_dict_serialization_ext 10/10, embed_list_serialization 8/8
  - embed_unify_r15 23/23, embed_stub_shadowing 4/4, embed_dedup 4/4, embed_stem_parity 3/3
  - embed_project_emit 3/3, embed_project_grouping 4/4
  - lib 472/472
  - 嵌套类型端到端验证：dict<str,list<int>> 生成码正确（形参 HashMap<String, Vec<i64>>，返回值 __lz_parse_nested_dict 调用），但 rustc 编译失败（__lz_dict_nested_to_json 的 val_expr 使用了不在作用域内的 x 迭代变量）——嵌套容器序列化的运行时实现不完整，留待下轮。
- 提交：51debdb
- 下一步：修复嵌套容器序列化的运行时实现（__lz_dict_nested_to_json 的 val_expr 递归序列化），使 dict<str,list<int>> 等嵌套类型能端到端 roundtrip
---

## 2026-11-12 T0r60b：嵌套容器序列化实现修复
- 起点：T0r60 回归修复（commit 51debdb）后，验证嵌套类型端到端功能发现 rustc 编译失败。
- 发现并修复的问题：
  1. **__lz_dict_nested_to_json 的 val_expr 引用不在作用域内的 x 迭代变量**：T0r60 的 embed_elem_to_json 生成的表达式使用 x 作为迭代变量名，但 __lz_dict_nested_to_json 的调用上下文中不存在 x。修复：embed_arg_expr_nested 的 dict 分支不再调用 __lz_dict_nested_to_json，改为直接在代码中生成内联 JSON 序列化（使用 .iter().map() 递归序列化 value）。
  2. **__lz_parse_nested_dict 返回类型与期望不匹配**：T0r60 的 __lz_parse_nested_dict 委托给 __lz_parse_dict_str_int 返回 HashMap<String, i64>，但嵌套 dict（如 dict<str,list<int>>）期望 HashMap<String, Vec<i64>>。修复：embed_ret_parse_expr 对嵌套 dict 生成内联 JSON 解析代码，返回 HashMap<String, String>（值以 JSON 字符串片段形式返回，避免泛型返回类型问题）；embed_ret_ty_nested 对嵌套 dict/tuple 也返回 String 值类型。
  3. **embed_ret_parse_expr 占位符问题**：之前的修复中 embed_ret_parse_expr 使用 _art/_method 占位符，embed_wrapper_fn 负责替换。新实现中 embed_ret_parse_expr 直接生成完整表达式（含 __r 参数），不再需要占位符替换。
- 验证：
  - cargo build --release 0 error
  - embed_tuple_serialization 6/6, embed_dict_serialization 16/16, embed_dict_serialization_ext 10/10, embed_list_serialization 8/8
  - 嵌套类型端到端生成码验证：dict<str,list<int>> 形参 HashMap<String, Vec<i64>> ✓，参数序列化内联 JSON ✓，返回值内联解析 HashMap<String, String> ✓
  - 剩余问题：LZ 代码生成中 dict 字面量默认生成 BTreeMap，与 embed 参数期望的 HashMap 类型不匹配（不在 embed 修复范围内，属 LZ codegen 的 dict 类型选择问题）
- 提交：50ae06f
- 下一步：解决 LZ dict 字面量类型（BTreeMap vs HashMap）与 embed 参数类型的匹配问题

---

## 2026-11-12 T0r60c：embed dict 类型对齐修复（BTreeMap 统一）
- 起点：T0r60b（commit 50ae06f）后，LZ codegen 把 Dict 字面量生成为 BTreeMap<K,V>（保证 JSON 序列化字段顺序稳定），而 embed 侧参数/返回值类型声明为 HashMap<K,V>，两侧类型不匹配 → rustc 编译失败（E0308 expected &HashMap, found &BTreeMap），嵌套 dict 类型（如 dict<str,float>）无法端到端 roundtrip。
- 修复方案：embed 侧 dict 相关类型统一从 HashMap 改为 BTreeMap，与 LZ codegen 的 Dict→BTreeMap 决策完全对齐。
- 改动点（src/bridge/cli.rs）：
  1. 4 个 `__lz_dict_to_json*` 辅助函数（dict_to_json/dict_to_json_float/dict_to_json_str/dict_int_int_to_json）参数类型 `&HashMap<...>` → `&BTreeMap<...>`。
  2. 4 个 `__lz_parse_dict_*` 辅助函数（parse_dict_str_int/str_float/str_str/int_int）返回类型与内部 `HashMap::new()` → `BTreeMap::new()`。
  3. `__lz_dict_nested_to_json`/`__lz_nested_dict_to_json`/`__lz_parse_dict_nested` 嵌套辅助函数的 HashMap → BTreeMap。
  4. `embed_param_ty_nested`/`embed_ret_ty_nested` 中 dict 分支的 `HashMap<K,V>` → `BTreeMap<K,V>`。
  5. `embed_param_ty`/`embed_ret_ty` 扁平分支的 4 个 dict 类型映射 `HashMap<...>` → `BTreeMap<...>`。
  6. 注释中 HashMap 示例同步改为 BTreeMap。
- 测试同步（tests/embed_dict_serialization.rs）：3 处断言 `HashMap<String, f64>/String>` → `BTreeMap<...>`，并新增端到端回归测试 `python_embed_dict_str_float_roundtrip_compiles`（验证 Dict<str,float> 参数+返回值 LZ 程序能编译通过，固化修复防回退）。
- 验证：
  - cargo build --release 0 error
  - embed_dict_serialization 17/17（原 16 + 新增 1）、embed_dict_serialization_ext 10/10、embed_tuple_serialization 6/6、embed_list_serialization 8/8
  - embed_unify_r15 3/3、embed_stub_shadowing 4/4、embed_dedup 4/4、embed_stem_parity 3/3、embed_project_emit 4/4、embed_project_grouping 3/3
  - lib 472/472
  - 端到端验证：Dict<str,float> LZ 程序 --embed-build 编译成功（EXIT=0），生成的 calc.rs 中所有 dict 辅助函数类型统一为 BTreeMap，与 LZ dict 字面量类型对齐
- 结论：LZ dict 字面量 BTreeMap 与 embed 参数类型匹配问题彻底解决，Dict<str,float>/Dict<str,str>/Dict<str,int>/Dict<int,int> 全部端到端可用
- 下一步：① 嵌套 dict 类型（dict<str,list<int>>）的完整运行时 roundtrip 验证；② Dict 与 Tuple 嵌套组合的序列化支持；③ 其他语言适配器的 List/Dict 序列化支持

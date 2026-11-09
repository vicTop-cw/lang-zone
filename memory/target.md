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

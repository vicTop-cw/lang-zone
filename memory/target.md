# 当前阶段目标（Target）

- 本轮聚焦：embed §六.1 可序列化闭集扩展——Dict<str,int> 复合类型序列化支持
- 验收条件：cargo build 0 error、全量测试零回归、新测试全绿

---

## 2026-11-04 T0r56：Dict<str,int> 复合类型序列化

- 焦点：embed §六.1 可序列化闭集扩展——Dict<str,int> 类型支持（Python 适配器）。
- 方案：JSON 嵌入编码——LZ 侧序列化为 `{"a":1,"b":2}`，Python 侧 `__lz_json_dict_str_int`/`json.loads` 解码；返回值 `_out_ok_json`/`json.dumps`；Rust 侧 `__lz_parse_dict_str_int` 手写 JSON 对象解析为 `HashMap<String, i64>`，参数序列化 `__lz_dict_to_json`。
- 判据：lib 469 + embed_unify_r15 23 + embed_gapfixes 17 + embed_stub_shadowing 4 + embed_stem_parity 3 + embed_list_serialization 8 + embed_dict_serialization 8 + codegen_corner_fixes 44 + lz_ir_bootstrap 25 = 543 passed / 0 failed，零回归。
- 交付：adapters.rs + cli.rs + codegen/mod.rs + tests/embed_dict_serialization.rs（8 条）+ 文档 §六.1 同步 + VERSION_PATCH 195→196。
- 下一轮：推广到其他 Dict K/V 组合（Dict<str,float>/Dict<str,str>/Dict<int,int>）+ Tuple 复合类型 + 其他语言适配器。

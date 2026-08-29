---
AIGC:
    Label: "1"
    ContentProducer: 001191440300708461136T1XGW3
    ProduceID: 9f2a11add43fbf12a546606fb2b962ab_206118709f7511f1a54f525400f8a581
    ReservedCode1: IMC3IDBmwjuTRFeF8+SVyfYi1JOCsLgFrl73/gEymwDkjL/FpSC8yxrOF22mLO2vYmEar7im2TbUCRzp8iSHMwMowt/MIp4+UfdwE+J84Fh2VZtsk1FwUcn3EboAdqDv5hMuP2xbk8cqLZpY+cYXn49TfrhPc3r6V20/4bhp+L2L9RmGDIEwibmsvNo=
    ContentPropagator: 001191440300708461136T1XGW3
    PropagateID: 9f2a11add43fbf12a546606fb2b962ab_206118709f7511f1a54f525400f8a581
    ReservedCode2: IMC3IDBmwjuTRFeF8+SVyfYi1JOCsLgFrl73/gEymwDkjL/FpSC8yxrOF22mLO2vYmEar7im2TbUCRzp8iSHMwMowt/MIp4+UfdwE+J84Fh2VZtsk1FwUcn3EboAdqDv5hMuP2xbk8cqLZpY+cYXn49TfrhPc3r6V20/4bhp+L2L9RmGDIEwibmsvNo=
---

# ERROR_BUG — 负向测试集（编译器漏报缺陷）

> 与 `FIND_BUG`（正向测试：合法代码被编译器错误拒绝）互为镜像。
> `ERROR_BUG` 收录**非法代码被编译器错误放行**的用例：lang-zone 应当报错、实际未报错。

## 收录标准

- 用例语法合法、**语义非法**（类型不匹配 / 未绑定变量 / 参数数量错误 / 模式非穷尽 / 约束缺失等）；
- 验证命令：`lang-zone.exe <file.lz>`，当前全部 `EXIT 0`（漏报）；
- 修复目标：以上用例应使 lang-zone 在语义检查阶段返回非 0 并给出明确错误。

## 严重级别

| 级别 | 含义 | 判定方式 | 数量 |
|------|------|----------|------|
| L1 | 完全漏放行 | lang-zone 放行，且生成的 `.rs` 经 rustc 亦通过（或仅 warning） | 1 |
| L2 | 前端漏检 | lang-zone 放行，生成的 `.rs` 被 rustc 拒绝（错误被推迟到 rustc 阶段） | 24 |

修复优先级：先 L1（完全漏放行），再按主题覆盖 L2。

## 复验方法

```bash
# 1) lang-zone 阶段：期望全部 EXIT 1 并报错，当前全部 EXIT 0（漏报）
lang-zone.exe ERROR_BUG/lib_sort/error_unconstrained_generic_cmp.lz

# 2) rustc 阶段（仅 L2 用例）：生成 .rs 后应报对应错误码
rustc --edition 2021 --extern lz_builtins=target/release/liblz_builtins.rlib -A warnings <generated.rs>
```

## 缺陷清单（25）

### L1 完全漏放行

| 用例 | 期望检查点 | 现状 |
|------|-----------|------|
| lib_pattern/error_duplicate_match_pattern.lz | match 重复模式应报错 | lang-zone 放行；rustc 仅 warning |

### L2 前端漏检（24）

| 主题 | 用例 | 期望检查点 | rustc 错误码 |
|------|------|-----------|-------------|
| lib_sort | error_unconstrained_generic_cmp | 泛型无 Ordered 约束却用 `>` | E0369 |
| lib_sort | error_unbound_var | 引用未定义变量 | E0425 |
| lib_sort | error_wrong_arity | 调用参数数量错误 | E0061 |
| lib_option | error_match_branch_type_mismatch | match 分支类型不一致 | E0308 |
| lib_option | error_non_exhaustive_match | match 非穷尽 | E0004 |
| lib_result | error_use_result_without_unwrap | Result 直接参与运算 | E0369 |
| lib_result | error_variant_type_mismatch | 枚举变体参数类型不符 | E0308 |
| lib_vector | error_mixed_list_type | List 混入不同类型 | E0308 |
| lib_vector | error_index_type | 索引类型错误 | E0277 |
| lib_closure | error_closure_param_type_mismatch | 闭包参数类型不匹配 | E0631 |
| lib_closure | error_closure_capture_unbound | 闭包捕获未定义变量 | E0425 |
| lib_pattern | error_match_subject_type_mismatch | 匹配主体类型错误 | E0308 |
| lib_linked_list | error_method_call_arity | 方法调用参数数量错误 | E0061,E0594 |
| lib_linked_list | error_field_not_found | 访问不存在字段 | E0609 |
| lib_string | error_str_plus_int | str+int 类型错误 | E0308 |
| lib_string | error_method_arg_type | 方法参数类型错误 | E0277,E0308 |
| lib_json | error_dict_key_type_mismatch | 字典键类型不匹配 | E0308 |
| lib_json | error_enum_field_type_mismatch | 枚举字段类型不符 | E0308 |
| lib_hashmap | error_method_not_found_put | 调用不存在方法 | E0308,E0599 |
| lib_hashmap | error_hash_key_type_mismatch | 字典键类型不匹配 | E0308 |
| lib_tree | error_closure_type_mismatch | 闭包类型不匹配 | E0425 |
| lib_tree | error_tree_insert_type_mismatch | 插入值类型不符 | E0308 |
| lib_iterator | error_return_type_mismatch | 函数返回类型不符 | E0308,E0425 |
| lib_iterator | error_return_type_mismatch2 | 函数返回类型不符 | E0308 |

## 说明

- 验证基线：`target/release/lang-zone.exe` + `target/release/liblz_builtins.rlib`，2026-08-24 复核；
- 用例文件头部注释含期望检查点与实测结果，可直接作为单测断言来源；
- 探针与原始复现过程位于 `temp/error_bug_probes_v1|v2`（中间产物，勿删除）。
*（内容由AI生成，仅供参考）*

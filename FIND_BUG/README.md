---
AIGC:
    Label: "1"
    ContentProducer: 001191440300708461136T1XGW3
    ProduceID: 9f2a11add43fbf12a546606fb2b962ab_dbbb5b469f7911f1a413525400287e28
    ReservedCode1: nEI9yX/lqyuow4X/BHwQ9sS73cR8VuQeQYzBy9ltRC1TRxwn72Gqhw4FC0BTNVBij5TlD1trGyw7LiU1p6Wno/Z6EVn/o1+QnFtNStvLZkWKoIfi9mMAiTN4VtoLti308o0bu1Mu/gbyEsswg7XiiZMJeIe7RYHOPi7+7FNVwvBSuKofE8YD+NyJw/I=
    ContentPropagator: 001191440300708461136T1XGW3
    PropagateID: 9f2a11add43fbf12a546606fb2b962ab_dbbb5b469f7911f1a413525400287e28
    ReservedCode2: nEI9yX/lqyuow4X/BHwQ9sS73cR8VuQeQYzBy9ltRC1TRxwn72Gqhw4FC0BTNVBij5TlD1trGyw7LiU1p6Wno/Z6EVn/o1+QnFtNStvLZkWKoIfi9mMAiTN4VtoLti308o0bu1Mu/gbyEsswg7XiiZMJeIe7RYHOPi7+7FNVwvBSuKofE8YD+NyJw/I=
---

# FIND_BUG — 正向测试集（编译器缺陷）

> 与 `ERROR_BUG`（负向测试：非法代码被编译器错误放行）互为镜像。
> `FIND_BUG` 收录**合法代码被编译器错误拒绝**的用例：lang-zone 应当编译通过（生成 `.rs` 且经 rustc 编译通过），实际未通过。

## 收录标准

- 用例语义合法、语法符合 lang-zone 设计（正常数据结构与算法实现：闭包/高阶函数、Option/Result、排序、链表、JSON、树等）；
- 验证命令：`lang-zone.exe <file.lz>` 生成 `.rs`，再 `rustc --edition 2021 --extern lz_builtins=<rlib> -A warnings` 编译；
- 当前状态：**12/12 全部未达到"生成 + rustc 通过"**（7 个生成成功但 rustc 拒绝，5 个 lang-zone 直接 Parse error）；
- 修复目标：以上用例应使 lang-zone 全链路编译通过。

## 严重级别

| 级别 | 含义 | 判定方式 | 数量 |
|------|------|----------|------|
| L1 | 解析层拒绝 | lang-zone 直接 Parse error，未进入 IR/codegen | 5 |
| L2 | codegen 层缺陷 | lang-zone 生成 `.rs` 成功，但 rustc 编译失败 | 7 |

修复优先级：先 L2（仅 codegen 侧修复即可闭环），再 L1（需 parser 支持函数类型注解等语法）。

## 复验方法

```bash
# 1) lang-zone 阶段：期望全部 EXIT 0 并生成 .rs，当前 7 成功 / 5 Parse error
lang-zone.exe FIND_BUG/lib_sort/sort.lz

# 2) rustc 阶段（L2 用例）：生成 .rs 后应编译通过，当前全部失败
rustc --edition 2021 --extern lz_builtins=target/release/liblz_builtins.rlib -A warnings <generated.rs>
```

## 缺陷清单（12）

### L1 解析层拒绝（5）— 共性触发点：函数类型注解 `fn(...) -> ...`

| 主题 | 用例 | 功能点 | 现状 |
|------|------|--------|------|
| lib_closure | closure.lz | 闭包与高阶函数（lambda、返回闭包、捕获、组合） | Parse error: Unexpected token in expression: Colon（`f: fn(int) -> int` 参数注解） |
| lib_option | option.lz | Option 类型与组合子（map/and_then/filter） | 同上（`f: fn(int) -> int` 参数注解） |
| lib_result | result.lz | 多泛型 Result 与错误传播组合子 | 同上（`f: fn(T) -> U` 参数注解） |
| lib_sort | sort.lz | 排序算法库（泛型 Ordered 约束、`sort_by` 高阶函数） | 同上（`cmp: fn(T, T) -> Ordering`） |
| lib_tree | tree.lz | 二叉树遍历与 fold/find 高阶函数 | 同上（`f: fn(int, int) -> int`） |

### L2 codegen 层缺陷（7）

| 主题 | 用例 | 功能点 | rustc 错误码 |
|------|------|--------|-------------|
| lib_hashmap | hashmap.lz | HashMap 简化版（struct 方法、while 循环） | E0255,E0425,E0107,E0308,E0560,E0599,E0116,E0782 |
| lib_iterator | iterator.lz | 迭代器 trait（trait 对象、关联类型、链式调用） | E0599,E0308,E0061 |
| lib_json | json.lz | JSON 子集解析（递归枚举、递归下降） | E0308,E0599,E0061,E0277,E0782 |
| lib_linked_list | linked_list.lz | 具体 int 链表 | E0308,E0599 |
| lib_pattern | pattern.lz | 模式匹配（嵌套模式、枚举解构、守卫） | E0425,E0308,E0618 |
| lib_string | string.lz | 字符串工具（while 循环、字符处理） | E0308,E0277,E0382,E0507,E0608 |
| lib_vector | vector.lz | 动态数组（具体类型） | E0599,E0308,E0596 |

## 典型缺陷摘录（rustc 阶段）

- `lib_hashmap`：`E0255 HashMap 定义多次` + `E0425 OptionInt 未定义`（codegen 重复导入/类型引用错误）
- `lib_iterator`：`E0599 no method named 'clone'/'f'/'pred'`（trait 方法丢失，闭包字段未生成）
- `lib_pattern`：`E0425 cannot find value 's2'`（模式绑定变量未正确生成）
- `lib_vector`：`E0599 no method named 'join'/'__len__'`（String/Vector 方法未生成）
- `lib_string`：`E0277 cannot add usize to i64`、`E0507`（借用/移动语义 codegen 错误）

## 历史对照

- 旧 `_compile_all.log`（迭代前）：11/12 Parse error（DotDot/RParen/Dot/Expected param name），仅 lib_pattern 通过；
- 本次实测（2026-08-24）：解析层已修复 6 个（hashmap/iterator/json/linked_list/string/vector 从 Parse error 转为生成成功），剩余 5 个函数类型注解用例仍卡解析层；但**已生成成功的 7 个 rustc 全部失败**，codegen 侧缺陷需系统修复。
*（内容由AI生成，仅供参考）*

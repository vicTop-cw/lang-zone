# DEMO 全面测试统计报告（2026-08-08）

> 生成方式：`lang-zone.exe <file.lz>` → IR codegen → `rustc --edition 2021` 编译 → 运行验证
> 排除项：`DEMO/99_errors/`（故意错误语法演示文件，预期报错，不计入失败）

## 一、总体结果

| 指标 | 数量 |
|------|------|
| DEMO 测试文件总数（排除 99_errors） | 204 |
| 通过（编译 + 运行成功） | 131 |
| 失败 | 73 |
| 通过率 | 64.2% |

## 二、失败分类

| 类别 | 数量 | 含义 |
|------|------|------|
| PARSE / IR build error | 17 | 无法生成 IR（语法错误或 IR 构建错误） |
| RUSTC（生成 rs 编译失败） | 55 | IR 生成成功但 Rust 编译错误 |
| RUN（运行失败） | 1 | 编译通过但运行崩溃 |

## 三、失败分布（按目录）

| 目录 | 失败数 | 主要错误 |
|------|--------|----------|
| boundary-coverage | 17 | 组合语法覆盖：PARSE 报错（struct/trait 表达式、闭包嵌套）、RUSTC（`__gen_vec` 未定义、类型不匹配、`_` 用法） |
| lz_std | 14 | 标准库自测：`__next__` 不属于 trait Iterator（E0407）、`Less_` 重复绑定、Parse 错误 |
| 06_control_flow | 11 | checker 块：`cannot find value`（counter/validate/depth/out 未找到）、`break` 在闭包内 |
| 04_functions | 5 | 闭包捕获、装饰器、spread 协议（T 未定义）、checker |
| 99_spec | 4 | `__gen_vec` 未定义、duck trait、guard_for |
| 07_data_structures | 4 | enum 算术、magic_methods 参数、模块魔法属性 |
| 08_modules | 3 | 宏定义解析、services 未找到 |
| 02_types | 2 | String→i64 cast、类型注解缺失 |
| 01_basics | 2 | `__str__` 未生成、`curly` 未定义 |
| 其他（Problems/combo/operators 等） | 11 | 分散问题 |

## 四、典型失败模式（按根因）

### 4.1 疑似编译器 bug（测试文件已确认语法正确）
- **`__gen_vec` 未定义**（99_spec/gen_block_star、boundary-coverage/combo-build-block、combo-iterator-generator）：构建块 `*:` 生成的 `__gen_vec` 作用域问题
- **checker 块变量未找到**（def_checker/def_checker2/block_demo/block_stack_test/block_tailrec）：`counter`/`validate`/`depth`/`out`/`result` 在块内引用解析失败
- **`__next__` 不属于 trait Iterator**（lz_std/list/option/result/set/string）：Iterator 关联类型改造后标准库 trait 方法签名不匹配
- **`__str__` 未生成**（01_basics/identifiers）：magic 方法 trait impl 生成问题

### 4.2 测试文件过时/语法不符合最新文档（需修正测试或剔除）
- **`std.io.print` 模块路径**（operators.lz）：`std` 被解析为 crate 名
- **`f"literal \{curly\}"`**（lexical_boundaries.lz）：转义花括号语法
- **`magic __str__` 旧写法**（identifiers.lz）
- **部分 boundary-coverage PARSE 错误**：组合语法超出现有 parser 支持

### 4.3 需要人工确认
- **lz_std 大量失败**：标准库 .lz 文件本身是否能独立编译（可能依赖 prelude 合并），需确认测试方式
- **99_spec**：专项规格测试，多为功能缺陷

## 五、建议下一步（按优先级）

1. **修复 `__gen_vec` 构建块作用域**（3+ 处失败，影响构建块功能）
2. **修复 checker 块变量解析**（5+ 处失败）
3. **对齐 lz_std 的 Iterator 关联类型**（6 处失败，与 trait 关联类型改造相关）
4. **修正/剔除过时测试文件**：identifiers、lexical_boundaries、operators、spread_protocol
5. **扩充 parser 支持 boundary-coverage 组合语法**（17 处失败中约一半）

## 六、失败清单

完整逐文件失败清单见同目录 `demo_test_failures_2026-08-08.txt`。

---
AIGC:
    Label: "1"
    ContentProducer: 001191440300708461136T1XGW3
    ProduceID: 9f2a11add43fbf12a546606fb2b962ab_d1133c179c3b11f1a98a525400f8a581
    ReservedCode1: bxfu1vV9/zf3BipcZW4e2TYNtBeVW+bHqK4PLgZ15h5rlYpmzg5gYQmbXmk8Fwgv2IkVjIWJL6xOPqTefv4R3SDy9zqdLM/9Smj1IZ5PwQF/DxZfqd/LcN/2BV8YJUdxQ+CcMPyvHkLc7tv5jDambxQuuLYh+RfJL0MFTNyrG4KJFQbqjnNAlnPZYtA=
    ContentPropagator: 001191440300708461136T1XGW3
    PropagateID: 9f2a11add43fbf12a546606fb2b962ab_d1133c179c3b11f1a98a525400f8a581
    ReservedCode2: bxfu1vV9/zf3BipcZW4e2TYNtBeVW+bHqK4PLgZ15h5rlYpmzg5gYQmbXmk8Fwgv2IkVjIWJL6xOPqTefv4R3SDy9zqdLM/9Smj1IZ5PwQF/DxZfqd/LcN/2BV8YJUdxQ+CcMPyvHkLc7tv5jDambxQuuLYh+RfJL0MFTNyrG4KJFQbqjnNAlnPZYtA=
---



# CHANGELOG — Lang-Zone (LZ)

本文件记录 LZ 编译器（`lzc` / `lzcyc`）的版本变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)。

## [v0.1.180] - 2026-08-29

### 阶段 A 收口 + Result/Option 泛型桥接 + lib_iterator 转正（J1–J4）
- builder.rs 跨表示桥接：`let` 注解 `Named("Result"/"Option")` 与函数签名 Result/Option 变体互通 → 泛型绑定不再零绑定（修 lib_result and_then）
- `infer_generic_binding` 对 Named(Result/Option) 与 IrType::Result/Option 变体做显式配对推断
- FIND_BUG/lib_iterator：`collect/sum` 参数 `RangeIter` → `Iterator` 放宽，用例全链路转正（ignored 12→10）
- version.rs bump v0.1.165 → **v0.1.180**（此前 version.rs 滞后于 git 版本）
- 清理根目录临时调试文件（err*.txt / build_*.txt / b3/b4.txt 等 16 个），`.diffwork/`、`*.lzcache` 纳入 .gitignore
- 回归 514 passed / 0 failed / 10 ignored（基线 512 + iterator 转正 +2）；批测 325/325 正例 + 3/3 反例 100%

## [v0.1.179] - 2026-08-28

### 扩展语义检查器 + 负向防线 25/25（ERROR_BUG）
- builder.rs 新增 check_extended_semantics / ExWalker 全套扩展语义检查：fn 形参逐参类型检查、闭包实参与 fn 形参匹配、泛型替换（ex_subst）、duck 约束、match 穷尽性与分支类型一致性、未绑定捕获等
- ERROR_BUG 负向测试集 25 用例（8/24 基线 0/25 全漏报）→ **25/25 全部拦截**
- 新增 tests/error_bug_libs.rs 回归守护（任一用例被放行即红）
- 变体载荷解析优先限定名（Enum.Variant）+ scrutinee 枚举消歧：修复跨枚举同名变体（IrType.Tuple vs Pattern.Tuple）导致的误报（.lzlz 自举库实测暴露）
- 参数类型报错补调用方函数名（诊断改进）

### codegen 修复与自举 gate 三连修
- 字符串字面量转义 escape_default（json.lz E0308）；字符串字面量参数直接生成 &str；下标 as usize 括号；字符串字面量 raise 豁免 raises 声明（消息式错误，语义见 reject_more 更新）
- collect_unknown_extern_fns 将 Item::Use 导入名记为已知：修复增量拼接产物中未知函数桩与缓存模块真身重复定义（E0428，incremental_golden 自 v178 起存量失败）
- LZ_BUILTIN_FN_NAMES 补 print_str / print_val：修复 .lzlz 自举 gate（--emit=ir-lz）被 i64 桩遮蔽内置函数（E0308）
- 上述三修使 lz_ir_bootstrap 5/5 转绿、incremental_golden 转绿

### 找缺推进与测试基建
- FIND_BUG 12 库用例入库：全链路 0/12（阶段推进：解析层 fn 类型注解 5/5 修复、lib_pattern 至运行关）
- tests/find_bug_libs.rs 修正路径约定（按目录实际 .lz 定位）+ rlib 查找兑底；12 用例按失败阶段 #[ignore] 分级，修复一个转正一个
- tests/reject_more.rs 口径对齐：字符串 raise 免 raises 声明入 ACCEPTED_CASES 锁定，类型化 raise 未声明仍拒绝
- DEMO 下 124 个 .pyx 生成产物清理；自举/stdlib 中间产物入库（lz_ir_lib.rs 等）
- **全量回归 512 passed / 0 failed / 12 ignored（12 个 ignore 为 12 库转正队列）**，v178 存量隐藏失败（incremental_golden / lz_ir_bootstrap×3 / reject_more 口径）全部清零

## [v0.1.165] - 2026-08-20

### IR→rustc 通过率提升（74.9% → 76.6%，本轮修复）
- E0308 类型不匹配（p22_str_index / p27_opt_elif 类）：
  - codegen/mod.rs 新增 `current_expected_ty`（RefCell<Option<IrType>>）+ `option_none_elem()`，`Option::None` 类型参数选择优先实参期望类型 → 函数返回类型 → 表达式自身类型 → 默认 i64
  - 字符串单字符索引按当前函数返回类型生成：返回 str/String 时生成 `String`（char 安全，越界 `'\0'`），否则生成 char 码点 i64（兼容 string_index_unicode 场景）
- E0599 方法不存在（p24_slice_method 类）：内置 str/List 接收者的 `.slice(a, b)` 映射为 `lz_slice`（用户自定义 struct 的 slice 保留原样）
- E0605/E0308 String→数值（p41_full_tokenize 类）：`as int` 对 String/Any 类型走 `.parse()` 而非 Rust `as`（`as` 不允许 String→数值）
- 新增 tests/ir_pass_fixes.rs：4 用例全链路（lz→rs→rustc→run）覆盖上述四类修复
- 全量回归 511 tests passed / 0 failed（基线 507 + 新增 4）；未触碰 Cy 后端
- 增量重跑原 21 个 rustc 失败用例：修复 6 个（含 2 个 diffwork 噪音），IR→rustc 通过率 266/355 → 272/355（76.6%）
- 遗留：E0425 use/import/extern 作用域注入（7 例）+ E0609 Option 模式解构（1 例）+ diffwork 内部噪音（7 例），下轮继续

## [v0.1.164] - 2026-08-20

### G6 D2 codegen 补缺（impl 块 / 列表推导 / 生成器 / match）
- codegen 补齐四类语法 IR→rustc 生成：impl 块（inherent / trait / 泛型）、列表推导（多 for / guard / 嵌套 / 函数调用 / 集合 / 字典）、生成器（yield / yield from）、match（值表达式 / 元组模式 / 范围模式 / 守卫）
- semantic_check.rs 新增 check_impls 语义校验：impl 未知 trait、impl 目标类型不存在、trait 抽象方法缺失、impl 多余方法均拒绝；match guard 变量先 bind 再检查；多 for 推导 extra 变量预注册后校验 output/cond/key/value
- 新增 DEMO/g6/ 四类 demo（g6_impl / g6_listcomp / g6_generator / g6_match），编译 + rustc 运行验证
- 新增 tests/g6_codegen.rs：17 用例（正向 10 + 拒绝 7），覆盖四类特性运行与语义拒绝
- 全量回归 507 tests passed / 0 failed（基线 490 + 新增 17）
- 未触碰 Cy 后端（codegen_cython.rs / CY/）与无关文件

### I6 收口（2026-08-20，无代码修复，仅测量与文档）
- 差异校验：docs/I6-差异校验-2026-08-20.md（12 例代表性语料 + 1 综合绑定项目；Rust codegen 产物 vs 绑定输出（registry/ledger/PyO3 结构基准）结构、行为、映射全部一致，无修复项）
- IR→rustc 通过率重测：docs/IR-rustc-通过率重测-2026-08-20.md（355 例 evaluable → 266 通过，**74.9%**，旧数据 56.6% 已刷新；rustc 失败 21：E0308×10 / E0425×7 / E0599×2 / E0609×1 / E0605×1；lz 失败 68）
- 提升方向：E0308 类型收敛 → E0425 use/extern 作用域 → builtins 方法别名 → 语义误报收敛

## [v0.1.163] - 2026-08-19

### 桥接缺口补全 + G2 语义检测 + Cy 后端
- G4 bridge 接线：python.rs 正向桥接（Mojo 式 from python import shim + resolve/gen + 单测）
- G5 调用台账：ledger.rs 追加式 TSV + `lzc emit-bridge-report` CLI 审计
- G8 PyO3 依赖注入：pyo3 0.22.6 + py-bridge feature 隔离 + docs/py-bridge-struct.md（Cy 对齐基准）
- G7 embed 属性宏完整落地：`#[embed(rust)]`/`#[embed(py)]` lexer/parser 解析 → builder 语义展开（IntrinsicKind::Embed + 代码段字面量提取 + 诊断）→ codegen 原样内嵌 + registry 登记；DEMO/embed_demo.lz 编译运行验证（`hello from embed` / 42）
- I3/I4 接线收尾：extern/export 在解析生成时自动注册 BridgeRegistry（generate_with_bridge + CLI 注入 registry + ledger flush）；extern_demo 2 symbols、export_demo 1 symbol 运行验证
- G2 错误检测推进：semantic_check.rs 修复 is_bound 漏判内置类型名、builtin_type_names 扩充、FnSig param_count_min（默认参数）、泛型/arity 放宽、Never 返回类型豁免 raise 声明、BuildBlock 标识符先绑定；builder.rs 泛型调用无法推断时拒绝（neg_generic_missing_t）；ir_snapshots 43/43 全绿；syntax_probes 反例 25/25 全部拒绝
- 新增 tests/bridge_embed.rs：embed 运行生效 / embed 缺代码段拒绝 / extern 登记 2 / export 登记 1
- Cy 后端完成：src/ir/codegen_cython.rs + CY/ 规范 + tests/cython_backend.rs + DEMO/*.pyx 生成物
- 全量回归 490 tests passed / 0 failed（基线 486 + 新增 4）
- docs/补缺计划-2026-08-19.md（单一事实源主计划）；过时文档归档 docs/obsolete/

## [v0.1.162] - 2026-08-18

### Ext 类型 + extern L1 + G3 字符串切片 + FIST T4
- `#[extern(lang)]` 外部声明（L1）：lexer/parser 通用装饰器、Ext 类型、`__lz_ext_call` 分发器、ExtHandle
- G3 字符串切片 char 安全（chars().collect()），非 ASCII 不 panic
- FIST T4：incr.rs 模块级缓存+依赖图+级联失效+rayon、hotreload.rs watch、lsp.rs；IR 序列化 IR_MAGIC+IR_VERSION+ModuleDep
- 提交 a4a6a25 / ec37e78；全量回归 392 tests

## [v0.1.161] - 2026-08-17

### 稳定自举 + CLI 子命令集成

- 完成稳定自举（三代收敛）：宿主编译器处理自举源集 13 个 .lz，连续两代 .rs manifest 与运行输出逐字节一致
- 前端自举链：LZ 写的 `frontend_self.lz` 编译自身源码，三代收敛（f2.rs == f3.rs）
- 新增 CLI 子命令：`lz create` / `lz build`（含 `--incremental`）/ `lz peek` / `lz check` / `lz push`
- 语法特性矩阵：36 份 SYNTAX/ 文档 → 40+ 特性清单；DEMO 261/261；bootstrap closed 13/13 RC=0
- cargo test 全量 320/0（基线不回归）

### 已知缺口

- D2 codegen 缺口（impl 块/列表推导/生成器/match 模式）
- 行级覆盖率报告（cargo llvm-cov）待网络安装工具
- 跨平台安装包超出当前范围

## [v0.1.160] - 2026-08-17

### 自举 100% 里程碑

- 达成自举 100%（v160）：cargo test 320/0、DEMO 261/261、bootstrap closed 13/13 RC=0
- `--emit=rs-lz` 与 Rust codegen 逐字符一致

## [自举 50% 里程碑] - 2026-08-17

- 自举进度过半，前端自举链收敛验证通过

## [v0.1.x] - 2026-07-31 起

### IR 路线决策

- 全力走 IR 中间表示路线：代码生成统一以 LZIR 为中间层（AST → LZIR → 目标语言）
- 旧 `src/codegen/`（AST → Rust 直接 codegen）视为遗留，逐步退役
- 双后端：Rust（`lzc`）与 Cython/Python（`lzcyc`）
*（内容由AI生成，仅供参考）*
*（内容由AI生成，仅供参考）*

---
AIGC:
    Label: "1"
    ContentProducer: 001191440300708461136T1XGW3
    ProduceID: 9f2a11add43fbf12a546606fb2b962ab_9f4bc76d9acc11f1a98a525400f8a581
    ReservedCode1: 5HXw5SYAkMeIB+u+ldEgMTCUo6Ob+mOUe8IurQRGCWpAuwZrunemlEPdctoHuy+ONvfBZGwSw+aiBmQ9yz/2OyHG7Lk5JLn27fv7goM2C9I7gc3/PxkJpYqpdaEvivdRIVS6CQKISQgk97M4yBu3nmZ65VAGjz0Xo4DNpH7ckNrf3yrrUAzwOUKOtSg=
    ContentPropagator: 001191440300708461136T1XGW3
    PropagateID: 9f2a11add43fbf12a546606fb2b962ab_9f4bc76d9acc11f1a98a525400f8a581
    ReservedCode2: 5HXw5SYAkMeIB+u+ldEgMTCUo6Ob+mOUe8IurQRGCWpAuwZrunemlEPdctoHuy+ONvfBZGwSw+aiBmQ9yz/2OyHG7Lk5JLn27fv7goM2C9I7gc3/PxkJpYqpdaEvivdRIVS6CQKISQgk97M4yBu3nmZ65VAGjz0Xo4DNpH7ckNrf3yrrUAzwOUKOtSg=
---

# CHANGELOG — Lang-Zone (LZ)

本文件记录 LZ 编译器（`lzc` / `lzcyc`）的版本变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)。

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

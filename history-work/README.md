# History Work — 项目工作记录

> 本目录记录各阶段「谁做了什么」，供 AI 接手与人工回顾。每条记录：日期 · 执行者 · 内容概要 · 产物位置。

---

## 2026-08-08 · AtomCode (deepseek-v4-flash)

### 一、管道机制重设计（v147，commit a628288）
- **内容**：`|>` 改为通用 callable 语义（左侧任意值 T，右侧单参 callable，默认首参预填充）；新增 `__lpipe__`（左侧数据变换，默认返回自身）与 `__rpipe__`（右侧处理函数工厂，优先于 `__call__`）；移除 `__pipe__` 魔法方法
- **覆盖**：AST/IR Pipe 节点带完整 callee 表达式；parser 用 parse_postfix 解析右侧；builder 类型感知分派（`__rpipe__` → `__call__` → 函数/构造预填充）；struct 位置参数构造按字段顺序映射（新增 struct_field_order 贯通各 ctx）
- **验证**：管道 DEMO 全场景 rustc 编译运行通过
- **产物**：src/ast/expr.rs、src/ir/node.rs、src/ir/builder.rs、src/ir/codegen.rs、src/parser/expr.rs、SYNTAX/04-表达式.md §十（文档同步）

### 二、项目规整与清理（v149，commit 54156ce）
- **内容**：删除全部缓存编译文件与临时文件（NUL.* 186 个、exe/pdb/o/rmeta 2000+、target 1.1G、_*.txt 44 个等）；建立 div-tools 收纳辅助脚本；整合有效测试进 DEMO、剔除占位测试；删除 DEMO 下 175 个生成 rs；全量测试 DEMO 并生成统计报告
- **测试结果**：131/204 通过（排除 99_errors 故意错误演示）
- **产物**：div-tools/、issue/demo-test-report-2026-08-08.md（后迁入 issue/）

### 三、builtins 内嵌 + 单一路线（v150，commit e56594a）
- **内容**：移除 `--ast-codegen` 老路子 CLI 选项，唯一 codegen 路径 = AST → LZIR → Rust；lz_builtins 加入 workspace 作为内部子库，生成代码 `use lz_builtins::*;` 替代 ~40 行内联 shims（__Params/__spawn_task/__block_on）
- **测试结果**：132/204 通过（排除 99_errors），失败 14 PARSE / 55 RUSTC / 1 RUN（lz_std 本轮不处理）
- **产物**：src/main.rs、src/ir/codegen.rs、Cargo.toml

### 四、目录规整（v151 前）
- **内容**：`issues/`（6 个文档）并入 `issue/`（README 补合并说明，冗余 test-report 快照清理保留最新）；统计报告移入 `issue/`；RUST/SYNTAX 下辅助脚本（fix_expr.ps1、fix_lexer2.ps1、check_doc_versions.py）归入 div-tools；清理 benchmark/_work 临时产物；创建 history-work 与 README-FOR-AI.md

---

## 历史记录索引

- 更早的 issue 记录（2026-07-29 ~ 2026-08-06）：见 `issue/`（AUDIT、decision-*、test-report、周期状态等）
- 设计决策汇总：`issue/README.md`

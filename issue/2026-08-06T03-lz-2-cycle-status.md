# lz-2 测试周期状态报告 — 2026-08-06T03:30:00Z

- **自动化**: lz-2 (IR 路线持续测试)
- **分支 / commit**: master @ 6897fe72b03216dcd01a939359b1786770e78a11 (HEAD 未变)
- **OS**: win32 (Windows / PowerShell)
- **路线约束**: 仅 IR 路径（LZ → AST→IR lowering → IR → Rust codegen）；未触碰任何 AST→RUST 测试逻辑

---

## Phase 0：环境与规范同步
- HEAD commit 未变（6897fe72）。计算 16 个快照对象真实 SHA256 与 `last_snapshot.json` 比对。
- **唯一变更**：`src/ir/builder.rs`（真实哈希 `f2e3d6...` → `b825e3...`）。
- 其余 15 个对象哈希全部一致（IR 文档 6 + 其余 IR 源 4 + tests 3 + Cargo.toml）。
- builder.rs 于 03:24 重新编辑，新增 IR lowering 匹配分支（纯增量，不影响解析阶段）。

→ 触发 **Phase 1**（IR 资产变更）。

## Phase 1：存量资产治理
- `builder.rs` 新增 `infer_expr_type`/`convert_ast_pattern`/`convert_expr`/`convert_stmt` 对 `BlockExpr`、`Pattern::List`/`Range`、`Stmt::WhileLet`、`Stmt::EnumDef`（inline enum 提升为模块 Item）的匹配分支。均为纯 IR lowering，合规。
- 治理动作：补录 `async_more.lz` 至 `known_parser_gaps_parse_error`（此前遗漏）。
- 既有 AST→RUST TECH_DEBT (`tests/deprecated/compile_demos.rs`) 仍隔离。更新 `last_snapshot.json`。

## Phase 2：全量测试执行（仅 IR 路径）
- **单元测试**：`cargo test --test mod` → **9 passed / 0 failed**（8 ir_snapshots + 1 reject_errors，纯 IR 路径）。
- **DEMO 全量 `--emit=ir` 扫描**（192 文件）：
  - 当前（03:24 builder）：**158 ok / 34 fail**。
  - 上一周期（02:30）：157 ok / 35 fail。
  - HEAD 基线对照（无工作树编辑）：155 ok / 37 fail。
  - 环比：+1 ok / −1 fail；相对 HEAD：+3 ok / −3 fail。
- **本周期 builder.rs 修复的 demo**（进入 IR 成功）：
  1. `DEMO/02_types/method_chains.lz`（新增 List/Range 模式 + BlockExpr 降级）
  2. `DEMO/06_control_flow/pattern_more.lz`
- 累积工作树修复：`closure_capture.lz`、`module_magic.lz`、`method_chains.lz`、`pattern_more.lz`。

## Phase 3：Bug 判定与分级
- **无 P0 / P1**。
- 34 个 fail 中 14 个为 `99_errors/*` 故意错误演示（合法失败）；20 个为已知能力缺口（含本周期补录的 `async_more.lz`）。
- **`async_more.lz`**：解析失败 `Parse error: Expected RBrack, got Comma at pos 60`（列表内 `await` 元素不被 parser 接受）。经对照实验确认：**parser 自 02:19 后未变更**，该解析错误位于 AST→IR lowering 之前的解析阶段，`builder.rs` 改动（增量 match 分支）**不可能**引发解析错误 → **非回归**，属既有 parser 缺口，定级 **P2**。已写 `DEMO/Problems/async-more-await-in-list-parse-failure.md`。
- `while_let.lz`：仍 P2（IR 构建期空列表元素推断 `error[E0282]`）。新增 `Stmt::WhileLet` 降级分支使其通过解析、进入 IR 构建期后失败（失败阶段由 parse → IR-build 迁移），与既有 `DEMO/Problems/while-let-empty-list-inference.md` 一致，非回归。
- 其余 19 个缺口与历史清单一致，无新增、无回归。

## Phase 4：报告与归档
- 周期状态报告：`issues/2026-08-06T03-lz-2-cycle-status.md`（本文件）。
- 新增 P2 Problem：`DEMO/Problems/async-more-await-in-list-parse-failure.md`。
- `while_let.lz` P2 报告沿用既有 `DEMO/Problems/`，不重复生成。
- 快照 `last_snapshot.json` 更新（真实哈希 + 新计数 + 补录 async_more）。

---

## 成功标准判定
- ✅ Phase 0–4 全部执行完毕
- ✅ 无 P0 / P1 未解决 Bug
- ✅ 所有变更已正确归档（快照更新 + 状态报告 + Problem 补录）

**本轮任务成功。**

## 建议（非阻塞）
- 修复 parser 以支持列表字面量内 `await` 表达式（`src/parser/expr.rs` 列表元素 grammar）。
- 为 `Pattern::List`/`Range`、`MagicMethod`、闭包块体、inline `EnumDef` 补 `ir_snapshots` 专项用例，巩固 IR round-trip 覆盖。
- 将 `99_errors/*` 从失败计数单独归类，避免与真实能力缺口混淆统计。

- 验证人：auto-sdet；验证时间：2026-08-06T03:30:00Z

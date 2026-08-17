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

---

## 2026-08-17 · AutoClaw（自举 50% 里程碑，v159）

### 一、C2：IR 构建 .lz 化（对齐 `--emit=ir` 与 `--emit=ir-lz` 双路输出）
- **内容**：`src/ir/lz_ir_lib.lz` 与 `src/ir/display.rs` 逐字符对齐——Expr `[ty]` 前缀、FnDef body 缩进式（弃 `{ }` 包裹）、Block `{ [ty] }` 展示、StructDef/EnumDef 泛型签名；`src/ir/lz_codegen.rs` 构造代码同步（`BlockIR.Block`、`Option.Some`、`print_str` 切换、`StructDef` generics 字段）
- **修复阻断缺陷**：struct 缺 PartialEq derive 致 enum derive 展开失败（E0369，`BlockIR` 改单变体 enum）；`print_str` 尾表达式与 `__lz_main -> i64` 类型不匹配（E0308，main 尾部补 `return 0`）；`lz_ir_lib.lz` UTF-8 BOM 致解析失败（去 BOM）
- **验证**：8 个关键 DEMO（literals/containers/const/ternary/comprehension/guard/struct/trait_impl）双路 diff 逐字符一致
- **产物**：src/ir/lz_ir_lib.lz、src/ir/lz_codegen.rs、bootstrap/work/lz_ir/README.md

### 二、C3：双路 diff 对照自动化
- **内容**：`bootstrap/work/lz_ir/diff_ir.ps1`（默认 8 输入集，Start-Process 纯净捕获 stdout，退出码 0/1/2/3）；`tests/lz_ir_bootstrap.rs` 新增 `lz_emit_ir_lz_matches_ir_byte_exact`（逐字节断言，覆盖泛型函数/struct/enum/match/循环/字典）
- **验证**：脚本 8/8 一致退出码 0；cargo test 全量 319/0（40% 基线 314 +5）；DEMO 全量 261/261（1 例 link.exe 1104 瞬时文件锁，单独重跑通过）；bootstrap closed 闭环退出码 0（两轮 manifest 一致 + PROMOTE）
- **产物**：bootstrap/work/lz_ir/diff_ir.ps1、tests/lz_ir_bootstrap.rs、bootstrap/05-自举里程碑台账.md（§2.1 50% 章节/§3/§4/§7）、bootstrap/06-自举交接续推文档.md（水位与路线图）、src/util/version.rs（v158→159）
- **口径**：40% 已计 49%（含 G1 缓冲）+ C2 6% + C3 6% = 61% ≥ 50%
- **异常记录**：本机 WSL bash 损坏（无法访问 localhost），`div-tools/regression.sh` 只扫到 2 个文件——改用 PowerShell 等价脚本完成 DEMO 全量回归，结论一致；未修改回归脚本本身

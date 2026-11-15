# lzc-loop-20260927 账本（lang-zone 主编译器 · 六环节 × 5 轮）

- run_id: `lzc-loop-20260927` · ns（server 侧）: `lzc-loop-20260927`
- 任务库目录: `E:\IDEProjects\AI\lang-zone\.fist-loop-20260927`（fist-mbt.db 落此目录）
- root: **T0**（已领取 → task_plan_deep 拆 T0.1/T0.2，各 2 叶）
- next: **打磨 R2**（第 11 环节）
- 单会话时间盒 120 分钟 / ≤2 环节；本会话完成 1 环节（推进 R1）

## 冷启动 Step0（实测定死）

- MCP 通道：会话内未挂载 `mcp__fist_mbt__*` → 走 **Node 直驱 STDIO** 脚本 `_mcp_call.js`（复用夜间循环做法，运行 FIST-Mbt 已构建 JS 产物，**未改 FIST-Mbt 仓库**）。
- `report_bug` 落点实测：server cwd = `.fist-loop-20260927`，`project_dir="."` → 落 `./memory/bugs.md`。
  **口径定死：真实缺陷入项目账本用 `project_dir=".."`**（→ `lang-zone/memory/bugs.md`，即现有 6 单所在账本）。
  试例 1 条留在本任务库本地账本（BUG-1 试例，不污染项目账本）。
- 启动快照：`git status --porcelain` → 43 项（28 M + 15 ??，含 `.codeartsdoer/` 工具态、`src/ir/**`、`tests/**`、`memory/` 等），存 `git_snapshot_start.txt`。**遗留集零触碰**。
- 规范底座已读：`project_standards`（R116，10 条）、`README-FOR-AI.md`、`SYNTAX/overview/缺失语法特性报告.md`、`tests/syntax-matrix-report.md`、`memory/bugs.md`（6 单）、`.fist-night/night_progress.md`（上轮夜间循环结转）。
- **工具参数坑（本轮实测）**：`task_plan_deep` 的 `spec` 必须是**合法 JSON**（本轮传 JSON 数组通过），且**不接受 `inject`**；`omega_spec_create` 必填 `content`；`omega_spec_review` 必填 `verdict`（"approve"）；`omega_result_verify` 必填 `verdict`；`verify` 必填 `verifier`。
- **编译/运行坑（本轮实测）**：手工 `rustc` 校核生成码必须带 `-L dependency=target/debug/deps`，否则 `--extern lz_builtins=...rlib` 报 E0463。

## done（环节级记账）

### R1-推进（1/30）— 2026-09-27 12:0x–13:0x

- 主题（自定，非 IR 域）：**为已支持但缺端到端闸门的路径补闸门** —— 内嵌函数（nested def）提升 + 列表推导式 over 列表变量。
  选型依据：IR 上游只读 → 语义/产物类推进基本被阻塞；先以探针实跑（`probe/`）确认两条路径当前**可用**，再将其固化为回归闸门，防止前端/IR 回归静默退化。
- 探针取证（`.fist-loop-20260927/probe/`，rustc+运行均过）：
  - `nested_def.lz` → `42`；`p8_nested2.lz`（两层内嵌）→ `8`
  - `p4_listcomp.lz` → `[2, 4, 6]`；`p6_setcomp.lz` → `{1, 2}`；`p7_lambda_comp.lz`（顶层）→ `[2, 3, 4]`
  - `p5_dictcomp.lz`（`{"k": x for x in xs}`）codegen 成功但 **rustc 编译失败** → 未确诊，转 **寻虫 R1** 优先复核（候选缺陷）。
- 交付：`tests/lzc_loop_nested_def.rs`（新文件，+4 用例，自带 lz→rs→rustc→run→assert 端到端 harness）
  - `nested_def_is_hoisted_and_callable`（42）/ `nested_def_two_levels`（8）
  - `list_comprehension_over_list_variable`（[2, 4, 6]）/ `list_comprehension_top_level`（[2, 3, 4]）
- 链路（FIST）：`publish T0 → claim T0 → task_plan_deep(omega_strong_verify=true, laya_auto=true, reinject_context=true) → T0.1.1/T0.1.2 [omega_spec_create → omega_spec_review(approve) → execute → submit → omega_result_verify(verdict=pass) → verify]`；T0.2.1/T0.2.2 未领（留作本轮后续环节的承载位）。
- laya：返回 `null` → **降级自决**（laya_auto 已显式开启，自决权由执行者行使，理由入账）。
- issue_up：本环节 0 单（无最小复现者不入账；dict 推导式候选待寻虫环节确诊）。
- 门禁：`cargo test --workspace --no-fail-fast -j 2` → **748 passed / 0 failed**（基线 744 → +4，只增不减 ✓）；golden 对账含在全量内（dual_route_golden 4、incremental_golden 1 均 ok）。
- changed：`tests/lzc_loop_nested_def.rs`（新增，+132 行）；未触碰 `src/ir/**`、lzcyz 域、遗留集。
- pending（转结）：① 字典推导式 over 列表变量 rustc 失败（寻虫 R1 确诊）；② BigInt/Complex 簇 BUG-1~6 复核（修复 R1，预计多数落 `src/ir/` → 转 `[ir-upstream]` 避让）；③ `src/bridge/cli.rs:159` JsonLines 未实现（需 serde 依赖决策，挂账待人工）；④ typer 对内嵌 def 不做推断（`src/typer/mod.rs:660/1620` TODO，挂账）。

### R1-寻虫（2/30）— 2026-09-28 09:5x–11:0x

- 主题：五路寻虫（审查报告复核 / 对抗样例 / 模式化读码 / golden+fuzz）。lzcyz 域与 `src/ir/**` 修改面全程排除。
- **H1 审查报告复核**：`memory/bugs.md` 存量 BUG-1~BUG-6 **全部实测已修**（用 `FIND_BUG/hunt-20260926` 原样例，
  lang-zone → rustc → run 全通过：bigint/complex/LazyLock、三层与四层嵌套空列表、bigint 注解位）。属「已修未销账」。
- **H2 对抗样例**：39 例探针（`.fist-loop-20260927/hunt_r1/probes/` + `mkprobes*.ps1`/`runall.ps1` harness）。
  转结项① 字典推导式 over 列表变量 **已转绿**（`{"k": 3}），不再转结。新确诊 5 单。
- **H3 模式化读码**：非 IR 侧 `eprintln!`/`dbg!` 全量排查（cli.rs/main.rs/hotreload.rs/typer/comptime）均为合法用户面输出；
  `lz_builtins/` 零命中；唯一未门控调试输出 = `src/ir/builder.rs:12825`。
- **H4 golden 差分 + fuzz**：本环节未开（时间盒内优先 H1~H3）→ 转 **验证 R1** 环节做 golden 对账；fuzz 变异转 R2-寻虫。
- issue_up：**7 单**（BUG-2~BUG-8 落任务库本地账本），其中 **ir-upstream 6 单**（超 i128 字面量 / bigint 形参 / 负号 bigint /
  字符串越界静默 0 / 未门控 DBG / 未知方法漏 E0599），非 IR 1 单（**BUG-2 未定义函数静默桩**，落 `src/semantic_check.rs`）。
- 交付：`memory/bugs-lzc-loop-20260927.md`（项目侧镜像，新文件）+ 任务库账本 BUG-2~8。
- changed：`memory/bugs-lzc-loop-20260927.md`（新增，+60 行）；未触碰 `src/**` 任何文件、遗留集、lzcyz 域。
- pending：① 修复 R1 闭环 BUG-2（`src/semantic_check.rs:1097`）；② 6 单 ir-upstream 转人工裁定避让；③ golden 对账 → 验证 R1。

### R1-修复（3/30）— 2026-09-28 02:0x–02:5x → **BLOCKED（外部并发改动回滚）**

- 修复目标：**BUG-2 未定义函数静默桩**（唯一非 IR 域落点 `src/semantic_check.rs`）；BUG-3~BUG-8 六单落 `src/ir/**`
  → 按红线标记「避让·待 IR 修复（人工裁定）」，保持 OPEN 转结。
- 实现：① `builtin_value_names()` 补 9 个内建名（`println/format/contains/push/sort/reverse/hash/Exception/__as__`，
  由 DEMO+corpus 312 文件全量扫描实测得出）；② `Expr::Call` 的 Ident callee 处加 `if !self.is_bound(name)` → `未定义函数`。
- 在位时验证（02:12–02:20）：312 文件 Semantic error **归零**；`foo()` 正确报 `未定义函数: foo`；
  反向用例（已定义函数 42 / 前向引用 7）通过，不误伤。新增闸门 `tests/lzc_loop_semantic_check.rs`（3 用例）全绿。
- **阻断**：02:43 `cargo test` 重编后行为回退 → 复查 `src/semantic_check.rs` 已被还原至 HEAD（`git diff` 空），
  同窗口 `src/ir/codegen_cython.rs`（lzcyz 域，红线零触碰）于 02:08:56 被外部修改，并一度造成
  `cargo build` 读到半写文件报 5 个 E0599（`IrType::List/Vec/Dict/Map/Set`），随后自愈。
  ⇒ **存在并发会话同时改本仓库 `src/**`，本循环的代码改动不可持久化**。
- 处置（红线：失败原样写账本、不伪造完成）：
  - 补丁 + 验证结论留档 `.fist-loop-20260927/hunt_r1/bug2_semantic_check.patch.md`（可原样重放）；
  - 闸门文件移出仓库 → `.fist-loop-20260927/hunt_r1/tests_lzc_loop_semantic_check.rs.pending`（避免留下红测）；
  - 本环节记 **blocked**，BUG-2 转人工裁定 / 下轮重放；本会话**未跑全量门禁**（补丁已不在位，跑无意义）。
- changed（净效果）：`src/semantic_check.rs` **0 行**（已被回滚）；仓库内新增文件 1：`memory/bugs-lzc-loop-20260927.md`。
- pending：① BUG-2 重放 + 全量门禁；② BUG-3~8（ir-upstream）人工裁定；③ 并发改动冲突需发起人协调。

### R1-验证（4/30）— 2026-09-28 05:5x–06:4x → **门禁 3 红 1 绿（均非本循环引入）**

- 门禁 1 `cargo test --workspace --no-fail-fast -j 2`：**733 passed / 2 failed**，exit=101，35 个测试目标。
  失败目标 `tests/str_boundary.rs`（`str_boundary_args_and_returns` / `str_boundary_ops_and_formatting`）：
  rustc `E0277 String: Pattern`（5 处）+ `E0308`（1 处）。→ BUG-9（ir-upstream）。证据 `_gate_ver_r1.txt`。
- 门禁 2 DEMO 全量回归（排除 `99_errors/`、`lz_std/`）：**total=312 / ok=285 / lzFail=1 / rustcFail=25 / runFail=0 / timeout=1**（4m50s）。
  脚本 `hunt_r1/demo_regression2.ps1`，证据 `_demo_regression_r1_v3.txt`。→ BUG-11 / BUG-12（ir-upstream）。
  **关键判定**：05:48 现场重建编译器后复核 `polish_07_closures`（`Box<dyn Fn>` clone E0599）、`iterator_demo`、
  `nesting-closure-lambda`（E0308）仍失败 —— 三者按记忆 28056391/79627747 已于 2026-09-27 转绿并移出
  `KNOWN_RUSTC_FAILURES` ⇒ 属**工作区回归**，不是历史欠账。
- 门禁 3 golden 对账：**绿** —— `dual_route_golden` 4/0、`incremental_golden` 1/0、`ir_snapshots` 8/0、
  `moddec_corpus` 2/0、`moddec_product_assert` 5/0；`git status --porcelain DEMO/99_spec/moddec/golden/` 为空。
- 门禁 4 测试数只增不减：**破** —— 基线 748 → 735。本循环唯一 commit `e371f0c1`（+4 用例）在册；
  差额源自 `tests/` 7 文件 M + `tests/str_boundary.rs` 未跟踪新增（外部并发改动）。→ BUG-10（待人工裁定基线口径）。
- FIST 链路：`publish T0r10 → claim → task_plan_deep(omega_strong_verify/laya_auto/reinject) → 四叶
  [spec_create → spec_review(approve) → execute → submit → omega_result_verify]`，G1/G2/G3 因门禁红走
  **verdict=reject**，`verify` 被 Omega 硬门正确拦下（`成果复验状态为 rejected，未通过前禁止验收`）；G4（报告交付）verify pass。
- 交付：`memory/reviews/20260928.06.40.00.md`（PROJECT-SPEC/04 进度报告，只增不删）。
- changed：仓库内**仅新增** `memory/reviews/20260928.06.40.00.md`；`src/**`、`tests/**`、DEMO、golden **零修改**（验证环节不顺手修）。
- pending：① BUG-2 补丁重放（R2-修复）；② BUG-9/11/12 IR 侧修复待人工裁定；③ BUG-10 基线口径待裁定。

### R1-打磨（5/30）— 2026-09-29 09:0x–09:4x

- 质量指标复测（`:polish:` `_gate_polish_r1.txt` / `_gate_polish_r1_revert.txt`）：
  `cargo test --workspace --no-fail-fast -j 2` **734 passed / 1 failed**（R1-验证 733/2 → 外部会话修好 1 例；
  余 `str_boundary_args_and_returns`，IR 域）；`cargo check --workspace` **0 warning / 0 error**；golden 仍全绿。
- **唯一非 IR 挂账项 BUG-2 结论修正（重要）**：重放补丁后门禁由 734/1 → **730/8**（新增 7 项失败），
  根因为 `tests/reject_more.rs:117 ACCEPTED_CASES::unknown_keyword` **显式锁定**「未知标识符按函数调用通过，交 rustc 阶段」，
  注释要求「若未来收紧须同步迁移至 REJECT_CASES」。⇒ 属**语义边界修订**而非缺陷，按红线「行为级契约只挂账不擅自改语义」
  + 打磨卡「不做语义变更」→ **回滚**，升级为 BUG-13 转人工裁定（含成套改动清单与 13 项白名单）。
  回滚后复测回到 **734/1**，无残留（`git diff src/semantic_check.rs` 仅余他人的 `type_name` 行）。
- 值得一提的合并细节：`src/semantic_check.rs` 在本环节前已被外部会话加入 `"type_name"`（BUG-EC-006，CY 后端），
  与其 White-list 扩展方向一致；本次回滚**精确保留该行**，未触碰他人改动。
- 复扫收敛盘点：寻虫 39 探针 → 7 确诊 → **BUG-1~6 已修**；BUG-2 → BUG-13（语义裁定）；
  BUG-3~8 中 6 单 ir-upstream 避让；验证 4 单（BUG-9/10/11/12）→ 3 单 ir-upstream 避让 + BUG-10 基线口径待裁定。
- changed：**净零**——`src/semantic_check.rs` 补丁已回滚（他人 `type_name` 行保留）；
  `tests/lzc_loop_semantic_check.rs` 已移回任务库（`.pending2`）；仓库内仅文档文件新增/追加。
- next：**推进 R1**（第 6 环节，轮内收口）｜pending：BUG-13 语义裁定、BUG-9/11/12 IR 修复裁定、BUG-10 基线口径裁定。

### R1-推进（6/30，轮内收口）— 2026-09-29 09:5x–10:2x

- 主题（承转结清单）：把 R1 各环节**实跑取证但无端到端闸门**的路径固化为回归测试（仅新增测试文件，非 IR 域）。
- 交付：`tests/lzc_loop_boundary_gates.rs`（**+160 行 / 4 用例**，commit `58f7eaa4`）
  - `malformed_inputs_are_rejected_with_parse_errors`（6 例：`{`、未终止字符串、孤立 `def`、空右值、未闭合列表、未闭合形参）
  - `undefined_variable_is_rejected` / `wrong_arity_call_is_rejected`（G2 既有契约锁死防退化）
  - `utf8_string_roundtrip_end_to_end`（21 / `"中文字符串 hello"`）
- **推进中发现新回归并立即入账**（不顺手修）：`bigint 四则/比较`、`bigint struct 字段`、`complex 四则`、
  `四层嵌套空列表` 四个候选闸门在 gdb-collect 阶段失败 —— 实测生成码**复发**裸 `use num_bigint::BigInt;` /
  `use num_complex::Complex64;`（E0432/E0433），以及 `ListLit` 期望类型下推只下一层（E0308）。
  两者落点 `src/ir/codegen/**` → **BUG-14（high）/ BUG-15（medium）** 入账避让；闸门文件内以注释留位待补。
- 门禁：`cargo test --workspace --no-fail-fast -j 2` **738 passed / 1 failed**（上环节 734/1 → **+4 无新增失败**）；
  唯一失败仍是外部 IR 域 `str_boundary_args_and_returns`。
- changed：`tests/lzc_loop_boundary_gates.rs`（新增 +160，已 commit）；`src/**` 零改动。
- next：**推进 R2**（7/30）｜本会话完成 2 环节（打磨 R1 + 推进 R1）。

## R1 轮转结清单（第 6 环节产出）

### 已完成（closed）

- 寻虫 H1：存量 `memory/bugs.md` **BUG-1~BUG-6 六单全部实测已修**（仅未销账，销账受限于 `report_bug` 禁 `..`）。
- 推进 R1（第 1 环节）：`tests/lzc_loop_nested_def.rs`（内嵌 def 提升 + 推导式 over 列表变量，4 用例，commit `e371f0c1`）。
- 推进 R1（第 6 环节）：`tests/lzc_loop_boundary_gates.rs`（畸形输入拒绝 + 语义拒绝 + UTF-8，4 用例，commit `58f7eaa4`）。
- 转结项「dict 推导式 over 列表变量 rustc 失败」→ 本轮**已转绿**，销项。

### 避让（ir-upstream，待 IR 修复人工裁定）

| id | 阻塞的开发面 |
| --- | --- |
| BUG-3 | 超 i128 整数字面量（`BigInt::from` 无后缀 → 字面量非法）：阻塞 BigInt 端到端闸门 |
| BUG-4 | bigint 形参不接受整数字面量实参：阻塞 bigint 函数接口面闸门 |
| BUG-5 | 一元负号 bigint 常量缺 `BigInt::from` 包装：阻塞 bigint 常量闸门 |
| BUG-6 | 字符串索引越界静默返回 `\0`：阻塞字符串语义闸门 |
| BUG-7 | `src/ir/builder.rs:12825` 未门控 `DBG fnarg` 输出：阻塞诊断输出洁净度 |
| BUG-8 | 未知方法漏 rustc E0599：阻塞错误体验改进 |
| BUG-9 | `str_boundary` 2/3（现存 1/3）红：阻塞门禁 1 转绿 |
| BUG-11 | DEMO 回归 25 例 rustc + 1 例 IR build 失败：阻塞门禁 2 转绿 |
| BUG-12 | `DEMO/99_spec/guard_for_3.lz` 挂起：阻塞回归自动化稳定性 |
| BUG-14 | 裸 `num_bigint`/`num_complex` import 复发：阻塞 bigint/complex 闸门补入 |
| BUG-15 | 四层嵌套空列表 E0308：阻塞容器下推闸门 |

### 待人工裁定

1. **BUG-13（语义边界）**：是否收紧「未知标识符按函数调用通过」——需成套改 `src/semantic_check.rs` +
   `builtin_value_names()` 补 13 名 + 迁移 `tests/reject_more.rs:117 unknown_keyword` 至 REJECT_CASES +
   复核 `lz_ir_bootstrap` 三用例。补丁留档 `hunt_r1/bug2_semantic_check.patch.md`。
2. **BUG-10（基线口径）**：门禁 4「只增不减」以哪个工作树快照为基线（当前基线 748 来自 09-27 快照，非 HEAD；
   现状 739 = 738+1，缺口源于他人删除/改动测试文件）。
3. **并发会话**：本项目存在并行修改 `src/**` 的会话（09-28 曾回滚本循环补丁，09-29 又引入 BUG-14/15 回归），
   建议明确独占窗口或串行化。
4. **工具限制**：FIST-Mbt `report_bug` 拒绝 `project_dir=".."`（禁穿越），缺陷无法直写项目 `memory/bugs.md`。

### OPEN 账本快照（任务库本地账本 `.fist-loop-20260927/memory/bugs.md`）

BUG-2（→ BUG-13 语义裁定）、BUG-3~BUG-15 全部 OPEN，共 **13 单**。

### R2-推进（7/30）— 2026-09-29 10:4x–11:1x

- 主题（承 R1 转结）：**清偿 BUG-6（bigint/complex 类型注解位不一致）——非 IR 域**。
  发现过程：R2 开局复跑 R1 探针集，发现 `struct Big = val: bigint` 与 `def f(a: bigint) -> bigint`
  报 `Semantic error: 未知类型: bigint`；追查 `src/semantic_check.rs:346 is_known_type` →
  其白名单 `src/ast/mod.rs::builtin_type_names()` **从未包含 bigint/complex**（`git diff src/ast/mod.rs` 为空，
  即 HEAD 如此），而 IR 侧 `src/ir/builder.rs:593` 早已支持 bigint/complex 映射。
  ⇒ **R1-寻虫「BUG-1~6 全部已修」的复核结论需修正**：该结论是在**临时工作树**上取得的（当时工作树含他人未提交改动），
  **HEAD 上 BUG-6 并未修**；正确表述为「BUG-1~5 已修，BUG-6 未修，本环节清偿」。
- 实现：`src/ast/mod.rs` `builtin_type_names()` 补入 `"bigint"` / `"complex"`（+5 行，含注释指向 BUG-6 与 IR 侧映射）。
- 验证：struct 字段位 / 函数参数+返回位 / complex 注解位三个探针 lang-zone 均零退出（修复前报未知类型）。
- 双向闸门（`tests/lzc_loop_boundary_gates.rs` +49 行）：
  `bigint_complex_annotations_are_accepted`（3 子例，仅锁前端——产物 rustc 阶段被 BUG-14 阻塞）
  + `unknown_type_annotation_is_still_rejected`（防白名单放宽过度）。
- 门禁：`cargo test --workspace --no-fail-fast -j 2` **740 passed / 1 failed**（上环节 738/1 → +2 无新增失败）；
  唯一失败仍为外部 IR 域 `str_boundary_args_and_returns`。
- changed + commit：`src/ast/mod.rs`（+5）与 `tests/lzc_loop_boundary_gates.rs`（+49）→ **`67b94d6`**
  `feat(ast): accept bigint/complex in type annotation positions`（2 files, +54）。
- pending（BUG-6 端到端仍未通，均落 IR）：① BUG-14 裸 `use num_bigint::BigInt` → E0432（`struct Big` 产物 rustc 实测仍红）；
  ② BUG-4 bigint 形参不接受整数字面量实参（`addbig(12345678901234567890, 1)` 仍 IR build error）。
- next：**寻虫 R2**（8/30）。

### R2-寻虫（8/30）— 2026-09-29 11:2x–11:5x

- 五路（本轮实做三路）：H2a 前端边界对抗样例（21 例）、H2b 模式化读码（非 IR）、H2c R2 开局回归比对。
  H4（golden 差分 + fuzz）仍留给后续验证/寻虫环节（时间盒优先）。
- **H2a 确诊 1 单：BUG-16（medium，非 IR 域）UTF-8 BOM 源文件被拒** ——
  最小复现 `println(1)` 以 UTF-8 带 BOM（EF BB BF）保存 → `Parse error: Unexpected token at top level: Unknown(U+FEFF)`；
  去 BOM 同内容正常。落点 `src/lexer/lexer.rs` 或源文件读取（`src/cli.rs` / `src/project.rs` / `src/util/source.rs`）→ 纳入修复 R2。
- H2a 其余结论（均正常，不入账）：CRLF、TAB 缩进、TAB/空格混排、`0x1F`/`0b1010`/`1_000`/`1e10`/`1.5e-3` 数字字面量、
  900 字符超长标识符、浮点除零（inf）、深嵌套 if、纯注释文件 —— 全部通过；
  `.5`/`5.` 前导尾随点浮点、分号语句、关键字作标识符、顶层 `return` —— 均为 LZ 层 Parse error 明确拒绝（设计如此，进观察区）。
- **误报自纠（不入账）**：`n05_escapes` 初判「转义不支持」，经 e1~e6 最小化复测（`\n` / `\"` / `\\` / `\t` 组合）
  **全部编译通过** —— 实为探针脚本把 `` `n ``/`` `t `` 替换成真实换行/TAB 导致字符串字面量跨行，属我自己的 harness 缺陷。
- **H2b**：非 IR 侧 `unwrap/expect/panic` 分布已采集（parser 三文件约 198、bridge/std 56、lexer 31、typer 11 等）。
  经验证：R1+R2 共 **54 例**畸形/边界输入**无一 panic**，全部给出 LZ 层 Parse/Semantic error ⇒ 未发现可达崩溃，不重复入账。
- **H2c**：R1 探针复跑发现的「bigint 注解位回归」已澄清 —— 根因是 R1-寻虫「BUG-1~6 全部已修」的复核结论
  取自**临时工作树**（当时含他人未提交改动），HEAD 上 BUG-6 并未修；已在 R2-推进 清偿（commit `67b94d6`）。
  教训入 notes：**复核「已修」必须以 HEAD 为准（`git diff` 为空 + 现场重建编译器），否则结论不可复现。**
- changed：仅探针/脚本（任务库内）；仓库零改动。next：**修复 R2**（9/30）—— 闭环 BUG-16（BOM）。

### R2-修复（9/30）— 2026-09-29 12:0x–12:3x

- **闭环 BUG-16（UTF-8 BOM，非 IR 域）**：根因 `src/main.rs` 单文件主路径用裸 `fs::read_to_string`，
  而仓库已有 `lang_zone::util::platform::strip_bom`（`src/util/source.rs::SourceCache::read` 在用）未被 CLI 复用。
  最小修：新增 `read_source_lossy()` 助手（`fs::read` + `strip_bom` + `from_utf8_lossy`），
  三处改用 —— 主编译路径 / 跨模块预加载 / 宏模块导入（+14/-3）。
- 验证：`n16_bom_min.lz`（`println(1)` + EF BB BF）与 `n01_bom.lz` 由 `Parse error: ... Unknown(U+FEFF)` 转为正常编译。
- 回归闸门：`tests/lzc_loop_boundary_gates.rs` 新增 `utf8_bom_source_is_accepted`（字节级写入 BOM，端到端跑出 `7`）；
  顺带把 `run_lz` 拆出 `run_lz_bytes` 供字节级用例复用（+17/-1）。
- 门禁：`cargo test --workspace --no-fail-fast -j 2` **741 passed / 1 failed**（740/1 → +1 无新增失败）。
- commit：**`ddf6e6b`** `fix(cli): strip UTF-8 BOM when reading .lz sources`（2 files, +30/-4）。
- 避让盘点（C2）：本轮可闭环 2 单（BUG-6 / BUG-16）；余下 BUG-3/4/5/7/8/9/11/12/14/15 与 BUG-6 的端到端残留
  全部落 `src/ir/**` → 标记「避让·待 IR 修复（人工裁定）」留 OPEN；BUG-2→BUG-13、BUG-10 亦待人工裁定。
- next：**验证 R2**（10/30）。

### R2-验证（10/30）— 2026-09-29 12:3x–12:5x

- 门禁 1：**741 passed / 1 failed**（`str_boundary_args_and_returns`，IR 域）。证据 `_gate_fix_r2.txt`。
- 门禁 2：DEMO 回归 **312 → ok295 / rustcFail16 / lzFail1 / runFail0 / timeout0**（6m13s）。
  较 R1（ok285/rc25/lz1/to1）**改善 +10**：walrus E0070 组 6 例、`magic_methods`、`spread_protocol` 转绿，
  `guard_for_3.lz` 挂起消失。证据 `_demo_regression_r2.txt`。
- 门禁 3：**绿**（golden 五目标全绿 + 受管 golden 目录 git 洁净）。
- 门禁 4：**748 → 742 仍破**；本循环累计 +7（R1 4 + R2 3），缺口 6 例源于他人删改 `tests/`。
- FIST：V1/V2 因门禁红走 `verdict=reject`（`verify` 被 Omega 硬门拦下，符合「门禁不绿不 verify」）；V3 报告叶 verify pass。
- 交付：`memory/reviews/20260929.12.45.00.md`（PROJECT-SPEC/04 进度报告）。本环节仓库零改码。
- next：**打磨 R2**（11/30）。本会话完成 2 环节（修复 R2 + 验证 R2）。

## notes

- **DEMO 回归 harness 必须有 per-case 超时**：`DEMO/99_spec/guard_for_3.lz` 生成程序挂起，首轮无超时的脚本被卡 ~9 分钟
  （kill PID 后继续）；v2 起加 `WaitForExit(20000)` + Kill。另：`Start-Process -PassThru` 后必须 `$p.Refresh()`
  再读 `ExitCode`，否则全量误判为 runFail（首轮 285 例假红）。
- **本地系统时钟与时区**：本机本地时间约 06:0x（UTC+8 即 06:0x+08:00）；本账本早期 `now` 参数曾按 11:xx+08:00 书写，
  与本地时钟不一致（不影响 FIST 侧排序），后续统一用本地时间。


- **工作区并发风险（本轮实测，重要）**：本会话期间 `src/ir/codegen_cython.rs`（02:08:56）与 `src/semantic_check.rs`
  （02:12–02:43 间被还原）均遭外部写入。后续环节在改 `src/**` 前应先用 `git diff --stat` 确认基线，
  并在改完**立刻**跑门禁；若改动被回滚，按本环节口径留档转结，不要反复重放。


- **FIST 工具口径变更（本轮实测，覆盖昨日定死口径）**：`report_bug` **拒绝 `project_dir=".."`**
  （`非法 project_dir（拒绝绝对路径/穿越/盘符）`）→ 缺陷无法直接写入项目 `memory/bugs.md`，本循环改落**任务库本地账本**
  （`project_dir="."` → `.fist-loop-20260927/memory/bugs.md`），并在项目侧新增镜像文件。报发起人裁决（FIST-Mbt 仓库只读未改）。
- **FIST 工具参数变更**：`verify` 必填参数由 `by` 变为 **`verifier`**（传 `by` 报 BUG-19 硬门缺参）。


- 基线实测（本循环启动）：**744 passed / 0 failed**（与夜间循环记录的 676/4 不同 —— 夜间为隔离副本口径；本库以仓库工作区实跑为准）。
- Windows 侧：`cargo test` 全量并行易触发 os error 1455，本循环统一 `-j 2`（已稳定跑通两次）。
- BUG-6 现状探查：`def makebig() -> bigint = <超大整数字面量>` 报 `IR build error: 函数 makebig 返回类型不匹配：期望 bigint，实际 int128` → 落点 `src/ir/builder.rs`（IR 域）→ **修复环节只能入账避让**；`src/ast/mod.rs:75-79` 的 `builtin_type_names()` 已含 bigint/complex（BUG-6 的 AST 半边似已修，待修复环节回归确认）。

## 遗留集（零触碰，不改/不 add/不 commit/不还原）

见 `git_snapshot_start.txt`（43 项）。

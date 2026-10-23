# 夜间循环进度（lang-zone · lz）

- run_id: `2026-10-23-night`
- round: **1/3**
- seq: 寻虫 → 修复 → 验证 → 打磨
- next: **R1-验证**（3/12）
- done:
  - `R1-修复`（2026-10-24 03:02–04:4x，2/12）→ 完成。根任务 **T0r2 已归档**；
    9 父 / 18 叶 DAG，BUG-8 链路（T0r2.1 + 2 叶）全 已完成；余 8 父挂账（见下）。
    - **手法**：隔离副本（`E:/IDEProjects/AI/_lz_night_scratch/lzcfix`）——仓库零改动，
      全部改动在副本上改 + 编 + 跑 + 出补丁。副本用 `CARGO_TARGET_DIR` + 目录联接
      `target → 原 target` 复用依赖缓存（首编 2m26s）；外部 workspace 依赖
      `tnr = { path = "../Tnr" }` 用目录联接 `_lz_night_scratch/Tnr → AI/Tnr` 解决。
    - **BUG-8 已修复并实跑验证**（high）：扩展 trait 声明处 Fn 形参按 impl 端同一规则补齐
      `+ Clone`（返回 fn 值时 `+ Send + Sync + Clone`，`new` 走 fn 指针）→ E0276 归零。
      实测：`set.lz` 4×E0276 → **编译通过并运行输出 all tests passed**；`dict.lz` 5×E0276 → 0；
      `list.lz` 6×E0276 → 0。落 `src/ir/codegen/decl_gen.rs`（净 +22 行）。
    - **DEMO 全量闸门**（`--ignored`，非门禁）：**327 文件 / 320 通过 / 7 失败**
      （上轮基线 319/8）→ **净 +1，零新增失败**。
    - **门禁**（隔离副本 + BUG-8 补丁，实测）：`cargo test --no-fail-fast` =
      **937 passed / 0 failed / 3 ignored**，41 target 全绿 —— **与上轮基线完全一致，零回归**；
      golden（`dual_route_golden` 等）全 ok → **零漂移，无需再对齐**。
    - **BUG-9…BUG-14 六单挂账待人工**（落点均在遗留集 `src/ir/codegen/*`，红线零触碰）。
    - **BUG-11 尝试修复 → 证伪 → 回退**：新增 `CodeGen.parent_lambda_nested_box` 标记并在
      闭包两分支强制 move、向 `CodeGen::new()` child 传播，产物内层**仍非 move**（E0373 依旧）。
      插桩发现该样例 `nested` 恒为 false，内层字符串由 `else` 分支**全新 child** 重建
      （expr_gen.rs ≈6570-6620），该重建路径只选择性复制少量字段 → 根因是
      **body_s 重建路径的上下文继承契约**，非「加标记」可解。改动已还原为仓库原版。
    - **新暴露缺陷入账 2 单**：BUG-15（dict 迭代解构 E0308，T0r20）、
      BUG-16（list 链式调用后移动 E0382，T0r21）——均为 BUG-8 修复后暴露的次生缺陷。
    - 产出：`.fist-night/patches/BUG-8_trait_decl_fn_bound.diff`（`git apply --check` 通过，未应用）、
      `.fist-night/memory/reviews/20261024.04.20.00.md`。
    - MCP 调用量：本次唤醒实际发起 **≈60 次**（`call_log` 为每进程窗口，无法跨进程累计）。
  - `R1-寻虫`（2026-10-24 00:38–01:5x，1/12）→ 完成。根任务 **T0 已归档**，5 父（T0.1~T0.5）+ 10 叶全 已完成，
    叶层 omega spec/result 全 approved（10/10）。
    - **门禁（实测，非引述）**：`cargo test --no-fail-fast` = **937 passed / 0 failed / 3 ignored / 940 total**，
      40 个 test target 全绿。较上夜（676/4/680 → 副本 680/0/680）**测试总数 +260 且全绿**。
    - **golden 对账**：`dual_route_golden` 4/4、`incremental_golden` 1/1 → **零漂移，无需再对齐**。
    - **DEMO 全量闸门**（`--ignored`，非门禁）：**327 文件 / 319 通过 / 8 失败**。
      上夜 17 条 allowlist 基线**已全部转通过**；**8 项为非基线新增失败**（上夜仅 2 项）。
    - 入账 **7 单**：BUG-8(E0276 +Clone, high) / BUG-9(Option<Box<T>> 未剥层, high) /
      BUG-10(E0599 Option<J>: Clone, medium) / BUG-11(E0373 嵌套闭包缺 move, medium) /
      BUG-12(E0308 &String, medium) / BUG-13(E0308 Result<i64,_>, medium) /
      BUG-14(i128⊕i64 混算, medium)；自动修复任务 `T0r13`…`T0r19`。
    - **对抗样例**（BigInt 边界，6 例，`FIND_BUG/hunt-20261023/`）：nb01/nb02/nb03/nb05 与 Python 校验一致通过；
      nb06 输出码点 104/101 与 `tests/str_boundary.rs` C4 口径一致 → **判非缺陷，不刷账本**；nb04 复现 E0308+E0277 → BUG-14。
    - 报告：`.fist-night/memory/reviews/20261024.01.25.00.md`；
      盘点基线 `.fist-night/scan/marker_inventory_20261023.md`（TODO 7 / unwrap 577 / panic! 92）。
    - MCP 调用量：本次唤醒实际发起 **≈212 次**（`call_log` 为**每进程窗口**，max seq 12，无法跨进程累计 →
      按实际批次计数登记）。
- report_bug_cwd: server cwd = `E:\IDEProjects\AI\lang-zone\.fist-night`；`project_dir="."` →
  落 `./memory/bugs.md`（= `.fist-night/memory/bugs.md`）。**沿用上夜实测口径，未重跑试例**（避免重复入账）。

## 遗留集清单（启动快照实测，夜间零触碰 / 不 add / 不 commit / 不还原）

`git status --porcelain` = **25 条（13 M + 12 ??）**，快照 `.fist-night/_start_snapshot_20261023.txt`。
**落点与上夜（2026-09-27）完全不同**，已换批次：

- M：`CY/TESTS/05_expressions/operators.exp`、`CY/TESTS/15_feature_matrix/builtin_methods.exp`、
  `docs/里程碑登记表-2026-09-29.md`、`memory/bugs.md`、`src/bridge/cli.rs`、`src/ir/builder/convert_expr.rs`、
  `src/ir/builder/infer.rs`、`src/ir/builder/mod.rs`、`src/ir/codegen/decl_gen.rs`、`src/ir/codegen/expr_gen.rs`、
  `src/ir/codegen/mod.rs`、`src/lib.rs`、`src/main.rs`、`src/semantic_check.rs`
- ??：`.fist-explore-cron/_fist_doctor_check.db`、`.fist-mbt/`、
  `CY/TESTS/05_expressions/operators.exp.staler15`、`CY/TESTS/12_build_blocks/checker_call_{lzerr,rcerr,run}.txt`、
  `CY/TESTS/15_feature_matrix/builtin_methods.exp.staler15`、`docs/嵌入式统一抽象-实现-2026-10-21.md`、
  `src/embed/`、`templates/`、`tests/embed_unify_r15.rs`

> 注：`memory/bugs.md` 为仓库根的**历史** bug 账（遗留），与本夜 `.fist-night/memory/bugs.md` 不是同一文件。

## notes（本轮新增口径，下轮接力必读）

0. **隔离副本最小集**（否则 60 例误报）：必须复制仓库顶层**除** `backup/`、`target/`、`.git/`、
   `.fist*` 外的**全部**目录。只复制 `Cargo.* / src / tests / DEMO / FIND_BUG / CY / templates`
   会因缺 `std/bridge.toml` 触发 `bridge::*` 59 例失败、缺 `ERROR_BUG/` 触发 `error_bug_libs` 1 例失败。
1. **server 新增 `ns_gate`**：写入类调用传 `namespace=default` 会被拒
   （`ns_gate: 拒绝写入 default 命名空间：请先 store_open(ns=...)`）。
   冷启动第一步改为 `store_open(namespace="lz-night-<窗口日>")`，其后**所有调用带该 ns**。
   db 落 `.fist-night/.fist-mbt/data`。
2. `tools/list` **不是工具**，是 JSON-RPC method：清单用 `node _list.js`，参数 schema 用 `node _schema.js <tool...>`。
3. **叶子上卷需先 `claim`**：`task_plan_deep` 拆出的叶初始为「待领取」，直接 `execute` 报「非法执行」。
   完整链路 = `claim → omega_spec_create → omega_spec_review(approve) → execute → omega_result_verify(pass) → submit → verify`。
4. **父层同样需 omega 语料**：`reject(verifier) → retry → omega_spec_create → omega_spec_review(approve) →
   execute → omega_result_verify(pass) → submit → verify`。缺 spec_create 会报「尚无执行前语料，不可复验成果」。
5. **根任务 T0 未开 omega**：直接 `execute → submit → verify → archive`（其时已处「待验收」，execute 报非法但
   verify/archive 成功 → 终态 已归档）。
6. `project_standards` 本轮**已拉取**（补上夜欠账）：通用 5 条 + FIST 专项 5 条 + checklist 7 项。
7. DEMO 复现须**先备份已存在 .rs、跑完还原**（`tests/demo_codegen_compile.rs` 同款机制），否则污染已提交的 DEMO/*.rs。
   复现脚本 `.fist-night/_repro_demo.sh`、单样例脚本 `.fist-night/_run_case.sh`（产物落 `.fist-night/_work/`）。

## notes（门禁与耗时基线）

- `cargo test --no-fail-fast` ≈ **2.5–3 min**（target 已热，40 target）；
  `cargo test --test demo_codegen_compile -- --ignored` ≈ **2 min**；
  单个 DEMO 文件 `lzc + rustc` ≈ 1–2 s。
- 100 分钟时间盒现实容量：**1 个环节**（门禁 3 min + DEMO 2 min + 复现 1 min + MCP 链路 ~20 min + 报告账本）。

## 挂账待人工（不可逆 / 越权 / 夜间不执行）

1. **BUG-8…BUG-16 的实际应用与修复**（落点 `src/ir/codegen/*` 属遗留集）→ 红线「遗留集零触碰」。
   补丁已备：`.fist-night/patches/BUG-8_trait_decl_fn_bound.diff`（`git apply --check` 通过）。
1b. **BUG-11 的 `body_s` 重建路径上下文继承契约重构**（expr_gen.rs ≈6570-6620 的
   `let mut c = CodeGen::new()` 只选择性复制字段）→ 需人工重构为显式上下文对象。
2. 8 项 DEMO 失败的**反向对照归因**（是否 2026-10-23 遗留批次引入）——需隔离副本或 `git worktree`，本环节时间盒不覆盖。
3. 上夜遗留：`report_bug` 历史重复单（BUG-4/5/6 等）清理需 `delete`（不可逆黑名单）。
4. **本夜本地 commit 已执行 1 次**（参数卡允许，仅夜间新改动）：
   `0decb68 findbug(night-2026-10-23): 新增 BigInt/str 边界对抗样例 6 例（R1-寻虫）`
   —— 显式 `git add` 6 个文件，**未用 `-A`/`.`**，未 push。
   仍未提交：`.fist-night/*`（任务库与日志，db/temp 产物不入库）+ 本夜报告，可次日人工择优提交。

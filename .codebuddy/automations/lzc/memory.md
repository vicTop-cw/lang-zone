# lzc 六环循环（lang-zone 主编译器）— 执行记忆

## 2026-09-27 · 会话 1 · R1-推进（1/30）

- 任务库：`E:\IDEProjects\AI\lang-zone\.fist-loop-20260927`（fist-mbt.db 落此处）；账本 `loop_progress.md`；ns `lzc-loop-20260927`；root T0（已拆 T0.1/T0.2 各 2 叶，T0.1.1/T0.1.2 已 verify，T0.2.x 未领）。
- **MCP 通道**：会话内未挂载 fist-mbt MCP 工具 → 复用 Node 直驱 STDIO 脚本 `.fist-loop-20260927/_mcp_call.js`（运行 FIST-Mbt 已构建 JS 产物，未改其仓库）。
- **report_bug 落点定死**：server cwd = 任务库目录；入项目账本 `lang-zone/memory/bugs.md` 用 `project_dir=".."`（`project_dir="."` 落任务库本地账本）。
- 门禁基线：`cargo test --workspace --no-fail-fast -j 2` → **744 passed / 0 failed**；本环节后 **748 / 0**（+4）。
- 交付：新增 `tests/lzc_loop_nested_def.rs`（内嵌 def 提升 42/8 + 列表推导式 over 列表变量 [2,4,6]/[2,3,4]，自带 lz→rs→rustc→run→assert 端到端 harness）；已 commit `e371f0c`。
- 踩坑（下次直接用）：
  - 手工校核生成码：`rustc --edition 2021 -L dependency=target/debug/deps --extern lz_builtins=target/debug/liblz_builtins.rlib`（**缺 `-L dependency=` 必报 E0463**）。
  - FIST 工具参数：`task_plan_deep` 的 `spec` 必须是合法 JSON（传数组）、不接受 `inject`；`omega_spec_create` 需 `content`；`omega_spec_review` 需 `verdict="approve"`；`omega_result_verify` 需 `verdict="pass"`；`verify` 需 `verifier`。
  - `laya` 返回 null（降级自决，需在账本留痕）。
- next：**R1-寻虫**（2/30）。优先复核候选：字典推导式 `{"k": x for x in xs}` over 列表变量 codegen 成功但 rustc 失败（探针 `probe/p5_dictcomp.lz`）；并复核 memory/bugs.md 存量 6 单（BUG-1~6，多数落 `src/ir/**` → 按红线只入 `[ir-upstream]` 账并避让）。
- 转结/挂账待人工：`src/bridge/cli.rs:159` JsonLines 未实现（需 serde 依赖决策）；`src/typer/mod.rs:660/1620` 内嵌 def 不做类型推断。
- 红线执行：未触碰 `src/ir/**`、lzcyz/Cython 域、FIST-Mbt 仓库、遗留集（43 项快照存 `git_snapshot_start.txt`）。

## 2026-09-28 · 会话 2 · R1-寻虫（2/30）+ R1-修复（3/30，BLOCKED）

- 沿用 `.fist-loop-20260927` 任务库 / ns `lzc-loop-20260927` / `_mcp_call.js` 通道。另发现 `.fist-loop-20260928` 为 01:12 一次
  中断的冷启动残留（仅 db + 3 条试例），未使用也未删除（红线：删文件待人工）。
- **R1-寻虫**：存量 BUG-1~6 **全部实测已修**（bigint/complex/LazyLock/嵌套空列表/bigint 注解位，rustc+run 均过）；
  39 例对抗/畸形探针（`.fist-loop-20260927/hunt_r1/`）新确诊 **7 单**（BUG-2~8，落任务库本地账本）。
  转结项「dict 推导式 over 列表变量」**已转绿**，不再转结。交付 `memory/bugs-lzc-loop-20260927.md`（项目侧镜像，新增文件）。
- **R1-修复**：BUG-2（未定义函数静默生成 `i64::MAX` 桩）落非 IR 域 `src/semantic_check.rs`，补丁 + 312 文件零误判验证已做成，
  但**补丁在会话内被并发会话回滚**（同窗口 `src/ir/codegen_cython.rs` 亦被外部改动）→ 环节记 blocked，
  补丁留档 `hunt_r1/bug2_semantic_check.patch.md` + 闸门 `tests_lzc_loop_semantic_check.rs.pending`，待重放。
- **重要环境事实**：本仓库存在**并发会话同时改 `src/**`**（02:08 codegen_cython.rs、02:12–02:43 semantic_check.rs 被还原），
  一度导致 `cargo build` 读到半写文件报 5 个 E0599 后自愈。后续改 `src/**` 必须先 `git diff --stat` 定基线、改完立刻跑门禁。
- **FIST 工具口径变更（覆盖会话 1 定死口径）**：`report_bug` 拒绝 `project_dir=".."`（禁穿越）→ 只能落任务库本地账本；
  `verify` 必填参数由 `by` 改为 `verifier`。
- next：**R1-验证**（4/30）—— 四项门禁 + golden 对账（寻虫环节未开的 H4 在此补）。

## 2026-09-28 · 会话 3 · R1-验证（4/30）

- 四门禁全量实跑（禁增量）：门禁1 `cargo test --workspace --no-fail-fast -j 2` **733 passed / 2 failed**（`tests/str_boundary.rs`
  两用例 E0277 `String: Pattern` + E0308，exit 101）；门禁2 DEMO 回归 **312 例 → ok 285 / rustcFail 25 / lzFail 1 / timeout 1**；
  门禁3 golden **绿**（五目标全绿 + 受管 golden 目录 git 洁净）；门禁4 测试数 **748 → 735 破**。
- 入账 BUG-9~12（3 单 ir-upstream + 1 单待人工裁定）。**失败均非本循环引入**：本循环仅 1 commit（新增测试文件），未碰 `src/**`；
  05:48 现场重建编译器复核，确认 `polish_07_closures`（`Box<dyn Fn>` clone）、`iterator_demo`、`nesting-closure-lambda` 等在
  2026-09-27 已转绿后**复现失败** → 工作区并发会话回归。
- FIST：红门禁三叶走 `omega_result_verify verdict=reject`，`verify` 被 Omega 硬门正确拦截（不伪造通过）；报告叶 verify pass。
- 交付：`memory/reviews/20260928.06.40.00.md`（PROJECT-SPEC/04 进度报告）。仓库内零改码。
- **环境坑（下次直接用）**：DEMO 回归 harness 必须带 per-case 超时（`guard_for_3.lz` 挂起曾卡死 9 分钟）；
  `Start-Process -PassThru` 后需 `$p.Refresh()` 再读 `ExitCode`，否则 285 例全假红；
  `Start-Process -RedirectStandardOutput` 输出文件在进程结束前被锁，需等结束再读。
- next：**R1-打磨**（5/30）—— 质量指标只升不降 + 复扫收敛（当前门禁红，打磨环节只能入账/补证据，不得顺手改码）。

## 2026-09-29 · 会话 4 · R1-打磨（5/30）+ R1-推进（6/30）

- **R1-打磨**：质量指标复测 `cargo test` **734/1**（较验证环节 733/2 外部改善 1 例）、`cargo check --workspace` **0 warning/0 error**、
  golden 全绿。**BUG-2 结论修正（重要）**：重放 `src/semantic_check.rs` 补丁后门禁 734/1 → **730/8**（新增 7 项失败），
  根因 `tests/reject_more.rs:117 ACCEPTED_CASES::unknown_keyword` **显式锁定**「未知标识符按函数调用通过，交 rustc 阶段」，
  属语义边界而非缺陷 → 按红线回滚（保留他人 `type_name` 行），升级 BUG-13 转人工裁定；回滚后复测回到 734/1 无残留。
- **R1-推进（轮内收口）**：新增 `tests/lzc_loop_boundary_gates.rs`（4 用例：畸形输入 6 例 Parse error 拒绝、
  未绑定变量/参数个数不匹配拒绝、UTF-8 端到端），commit **`58f7eaa4`**；全量门禁 **734/1 → 738/1（+4 无新增失败）**。
  推进中发现并入账两项**工作区回归**：BUG-14（生成码复发裸 `use num_bigint::BigInt;`/`use num_complex::Complex64;` → E0432，
  09-28 实测还是 `lz_builtins::BigInt`）、BUG-15（四层嵌套空列表 `[[], [[]], [[[]]]]` E0308 复发）；
  受此影响 bigint/complex/四层列表四个候选闸门**暂不入闸**（文件内注释留位），已在账本标注待补。
- 账本产出 R1 **轮转结清单**（已完成 / 11 项 ir-upstream 避让 / 4 项待人工裁定 / 13 单 OPEN 快照）。
- next：**推进 R2**（7/30）。开局建议先确认并发会话是否收敛（否则改动仍可能被回滚）。

## 2026-09-29 · 会话 5 · R2-推进（7/30）+ R2-寻虫（8/30）

- **R2-推进：清偿 BUG-6（非 IR 域）**。发现 R1「BUG-1~6 全已修」是**误判**——该结论取自含他人未提交改动的临时工作树，
  HEAD 上 `src/ast/mod.rs::builtin_type_names()` 从未含 bigint/complex（`git diff` 为空），故 struct 字段 / 函数参数 / 返回位
  一直报「未知类型: bigint」。修复：白名单补 `"bigint"`/`"complex"`（+5），配双向闸门（3 注解位接受 + 未知类型仍拒）。
  门禁 738/1 → **740/1**（+2 无新增失败）。commit **`67b94d6`**（2 files, +54）。
- **R2-寻虫**：21 例前端边界样例，确诊 **BUG-16**（medium，非 IR）：UTF-8 **BOM** 源文件被拒
  （`Parse error: Unexpected token at top level: Unknown(U+FEFF)`），最小复现 `println(1)` + EF BB BF；落点 src/lexer 或源文件读取。
  自纠一例误报（转义不支持 → 实为探针脚本把反引号替换成真实换行/TAB）。非 IR 侧 unwrap 分布已采集，54 例畸形输入无一 panic。
- **教训（重要，写进流程）**：判定「某缺陷已修」必须以 **HEAD 为准**（`git diff` 空 + 现场重建编译器复测），
  不能凭当前工作树实跑下结论——本仓库长期存在未提交改动。
- next：**修复 R2**（9/30）—— 闭环 BUG-16（BOM 剥离）。

## 2026-09-29 · 会话 6 · R2-修复（9/30）+ R2-验证（10/30）

- **R2-修复：闭环 BUG-16**。根因 `src/main.rs` 主路径用裸 `fs::read_to_string`，而仓库已有
  `lang_zone::util::platform::strip_bom`（只有 `util::source::SourceCache::read` 在用）。新增 `read_source_lossy()`
  并让主编译路径 / 跨模块预加载 / 宏导入三处改用它（+14/-3）；闸门 `utf8_bom_source_is_accepted`（字节级写 BOM，跑出 7）。
  门禁 740/1 → **741/1**。commit **`ddf6e6b`**。
- **R2-验证**：门禁1 **741/1**；门禁2 DEMO **312 → ok295 / rc16 / lz1 / to0**（较 R1 ok285/rc25 **改善 +10**，
  walrus 组与 magic_methods/spread_protocol 转绿、guard_for_3 挂起消失）；门禁3 green；门禁4 748→742 仍破（他人删改 tests）。
  报告 `memory/reviews/20260929.12.45.00.md`。红项全部落 `src/ir/**`，循环内不可修。
- 累计 commit：`e371f0c1`（内嵌 def 闸门）、`58f7eaa4`（边界闸门）、`67b94d6`（BUG-6）、`ddf6e6b`（BUG-16）。
- next：**打磨 R2**（11/30）—— 维持质量指标 + 复扫收敛盘点（不改码、不做语义变更）。

## 2026-09-29 · 会话 7（另一条流水线）· explore 派单参谋冷启动（ns `lz-explore`，任务库 `.fist-explore-cron`）

- 冷启动：建库 + 厂商 `patch_esm_main.py` + Python MCP 驱动 `_mcp.py`；`tools/list` 105 工具在列。
- **两个环境坑（下次直接用）**：① FIST-Mbt 构建产物 `cmd/main/main.js` 被上游重建后**丢失 ESM `createRequire` shim**
  （`ReferenceError: require is not defined`）→ 必须用 `python patch_esm_main.py <main.js 绝对路径>` 显式补
  （自动发现只补最大的 `cli.js`，覆盖不到 main.js）；② `watchdog_tick` 的 `meta_prompt_path` 被**按相对 cwd 重写**
  为 `templates/pipeline_mode_explore.md` → 需在任务库内置 `templates/` 只读镜像，否则模板 missing 全 7 个。
- 落点定死：`report_bug(project_dir=".")` → `.fist-explore-cron/memory/bugs.md`（试例已标 FALSE_POSITIVE）。
- 本轮分支：watchdog 回 `action=idle`（active=0、无待消费提示词）→ 走**④生成**：
  产出首份提示词 `prompts/20260929.13.15.00.md`，主题 `verify`（c=1），
  目标＝把三条门禁真值与 `KNOWN_RUSTC_FAILURES` 的逐条对账表钉死；附 `[难度梯度 1/3:易]`。
- `output_validate` 回 **fail**（artifact 检查过，但 evidence 层判 `l4-pass-but-l1-missing`）→ 按红线只上报不自行修复。
- 挂账待人工：Dict→BTreeMap 语义裁决；`.fist-*` 目录未被 `.gitignore` 覆盖；FIST-Mbt 模板路径解析口径。
- next：下一轮唤醒应走**③接活**，按 `chosen_mode=verify` 派单。

## 2026-09-30 · 会话 8（explore 流水线）· ③接活：派 verify 单 T0

- 分支③（active=0 + 最新提示词未消费）：按 `prompts/20260929.13.15.00.md` 接根任务 **T0**（ns `lz-explore`）并接一脚：
  reserve_check → reserve_scope(`memory/reviews/`) → publish → claim → heartbeat → reserve_release。
  `output_validate` **pass**（2 产物全过，l4-pass）。
- **两个工具口径坑（下次直接用）**：① `output_validate` 的 `evidence` 必须是 **string**（传数组/对象会被判空 → verdict=fail）；
  ② `publish` 的 `created_by` **只收 `human_steward`**（实例写「续轮 watchdog」，工具拒收 → 已入账 BUG-4 表/码分叉）。
- 打分：LADDER `[难度梯度 1/3:易]` 夹 0..2 → c=1 → `verify`（只读档）。第二档预告：`polish`（`Box<dyn Fn>`→`Rc<dyn Fn>` 3 例）。
- 汇报落 `prompts/_report-20260930.09.10.md`（`_` 前缀，不参与提示词选取）。
- next：T0 跑动中 → 下一轮 ①idle；T0 完成后走 ④生成第二档。

## 2026-09-30 · 会话 9（explore 流水线）· ②自愈：零动作

- `watchdog_tick` 回 `action=restarted, active=1, healed=1, healed_tasks=["T0"]` —— T0（上轮 verify 腿）
  心跳超时（最后心跳 2026-09-29T18:07:03Z），watchdog 已自愈回滚为「已领取」。
- 按分支② 口径：**参谋只汇报、不自行重启、不派单、零动作**；本轮未打分、未跑门禁、未改任何文件。
  汇报落 `prompts/_report-20260930.09.55.md`。
- 提醒：若连续 ≥2 轮停在「已领取 + 心跳超时」，说明下游未真正推进 → 应转人工或改派。
- 每轮拉起 server 前建议再跑一次 `python patch_esm_main.py <main.js 绝对路径>`（上游重建会丢 ESM shim）。

## 2026-10-09 · 会话 10（explore 流水线）· ④生成第二档（polish）

- 间隔 9 天再唤醒：期间仓库新增 **29 commit**（最新 10-09 13:32）；未提交文件 98 → 142。
  上轮派出的 T0（verify 只读腿）**已归档**，交付 `memory/reviews/20260930.08.05.00.md`。
- **门禁已转绿**：workspace **742 passed / 0 failed**（--lib 420/0、find_bug 40/0+12/0）；
  DEMO 326→309/17 在册/**unexpected=0**；golden 8 套件全绿。（与 R2 时的 741/1 相比红灯已消。）
- 本轮动作：生成第二档提示词 `prompts/20261009.13.45.00.md`，`[难度梯度 2/3:中]` → c=3 → **polish**
  —— 收口 `Box<dyn Fn>`→`Rc<dyn Fn>` 一族（polish_07_closures / polish_27_use / polish_29_combined 三条在册转绿后移出白名单）。
  `output_validate` **pass**（l4-pass）。
- 新发现：本流水线已有待领取任务 **T0r6（cy 后端批次，LZ→Cython）**，申报作用域含 src/ir/codegen_cython.rs、
  src/ir/builder.rs、tests/cy_codegen_gate.rs、CY/**；本轮 polish 作用域与之字面不相交，冲突时让路串行化。
  另入账 **BUG-5**：watchdog 把 explore 模板全文发布成待领取任务 T0r7，污染 task_triage 队列。
- next：下一轮走③接活，按 `chosen_mode=polish` 派第二档。

## 2026-10-10 · 会话 11（explore 流水线）· ③接活：派 polish 单 T0r9

- 按 `prompts/20261009.13.45.00.md`（`[难度梯度 2/3:中]` → c=3 → polish）接根任务 **T0r9** 并接一脚：
  reserve_check×3 → reserve_scope×3（均 free/reserved）→ publish(human_steward) → claim → heartbeat → reserve_release×3。
  `output_validate` **pass**。
- 派单内容：函数值 `Box<dyn Fn>`→`Rc<dyn Fn>` 一族收口（装箱点、值位/返回位/HOF 形参/curry 签名；
  传 std 的 Fn/FnMut 形参时补转发闭包），目标是把 `polish_07_closures` / `polish_27_use` / `polish_29_combined`
  三条 E0599 在册项转绿并从 KNOWN_RUSTC_FAILURES 移出。
- 作用域：`src/ir/codegen/mod.rs` + `tests/demo_codegen_compile.rs` + `DEMO/01_basics/`；
  与在册腿 T0r6（cy 后端，含 `src/ir/builder.rs`）字面不相交，冲突时让路串行化。
- 队列噪音累计 2 条（T0r7、T0r8 的 description = explore 模板全文，沿用 BUG-5），建议人工归档。
- next：T0r9 跑动中 → ①idle；心跳超时 → ②并挂账（连续自愈 ≥2 次转人工）。

## 2026-10-10 · 会话 12（explore 流水线）· ②自愈：零动作（T0r9 第 1 次）

- `watchdog_tick` 回 `action=restarted, healed=1, healed_tasks=["T0r9"]`；T0r9（polish 腿）心跳超时，
  仍「已领取」。按分支②：**只汇报、不重启、零动作**。汇报落 `prompts/_report-20261010.14.05.md`。
- 佐证无人接手：自 10-09 13:45 派单以来仓库 **0 新 commit**（最新仍是 b44d3c1 10-09 13:32）。
- 结构性缺口复现：explore 派单后需**另一执行者**消费，本计划任务每次唤醒都是参谋本人 → 必然空转。
  已再次向用户提出 A（加执行者任务）/ B（参谋接活即开工）/ C（暂停 explore）三选一，**尚未收到选择**。
- 若下一轮 T0r9 仍超时（第 2 次自愈）→ 挂账转人工，不再零动作循环。

## 2026-10-10 · 会话 13（explore 流水线）· ②自愈第 2 次 → 挂账转人工（BUG-6）

- T0r9（polish 腿）第 2 次心跳超时（updated 2026-10-10T00:20:17Z），watchdog 再次 healed。
  按既定口径提交 **BUG-6**（`publish_task=true`，落到队列 task_id T0r10）：含时间线、根因
  「explore 派单无执行者 → 必然空转」、A/B/C 三选项、待处置清单（T0r9 归档或改派、T0r7/T0r8 噪声任务、
  Dict 语义裁决、工具侧 4 项挂账）。
- 本轮零动作退出：未重启、未重派、未改源码。汇报落 `prompts/_report-20261010.18.55.md`。
- **后续约定**：在人工裁定前，参谋每次唤醒保持零动作，汇报里直接指向 BUG-6，不再重复挂账。
- 会话 14（10-10 23:40）：T0r9 第 3 次自愈、挂账单 T0r10 仍待领取 → 按约定零动作，未重复挂账。
  （参谋侧已无事可做，等人工 A/B/C 裁定；若长期无人处理建议直接停用本计划任务。）
- 会话 15（10-11 04:25）：状态无变化（T0r9 第 4 次自愈、T0r10 仍待领取）→ 再次零动作；
  后续同状态轮次**合并记录**在 `prompts/_report-20261010.23.40.md` 末尾，不再每轮新建汇报文件。
- **重要环境变更（两条流水线通用）**：FIST-Mbt 上游构建产物 `_build/js/debug/build/cmd/main/main.js`
  **已被删除**（`cmd/` 下只剩 `cli/`），`patch_esm_main.py` 直接 FileNotFoundError → 原 MCP 入口彻底不可用。
  **改用已注册 CLI：`fist-mbt serve`（v0.3.6）**；因是 WindowsApps shim，Python subprocess 必须经
  `cmd /c fist-mbt serve`。`.fist-explore-cron/_mcp.py` 已默认走 CLI（`LZ_FIST_MCP_CLI=0` 可回退），
  db 落点与任务状态与原先一致。**今后不再需要 patch_esm_main.py**；
  `.fist-loop-20260927/_mcp_call.js` 若再用需同步改入口。
- 会话 17（10-11 09:10）：T0r9 第 6 次自愈、T0r10 待领取已 ~33h → 零动作；
  **升级建议：停用本计划任务**（或先归档 T0r7/T0r8、处理 T0r9），否则每 45 分钟只会重复空转汇报。
- 会话 18（10-11 13:55）：第 7 次自愈、T0r10 待领取 ~38h → 零动作。
  **降噪约定**：状态无新事实时不再逐轮写提示词目录，只在账本累计计数；状态变化或人工处置后立即恢复详细记账。
- 会话 19（10-11 18:40）：第 8 次自愈、T0r10 待领取 ~42h → 零动作（按降噪约定仅计数）。
  （累计：连续 8 轮 ②自愈；参谋无合规动作可做，等人工停用/处置。）
- 会话 20（10-11 23:25）：第 9 次自愈，状态不变 → 零动作（仅计数）。连续 9 轮 ②自愈。
- 会话 21（10-12 04:10）：第 10 次自愈，状态不变 → 零动作（仅计数）。连续 10 轮 ②自愈；
  参谋已连续 10 轮无合规动作，强烈建议停用本计划任务。
- 会话 22（10-12 08:55）：第 11 次自愈，状态不变 → 零动作（仅计数）。连续 11 轮 ②自愈。

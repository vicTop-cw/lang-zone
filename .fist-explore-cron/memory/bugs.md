## BUG-1 [2026-09-29T15:00:09Z] [low] FALSE_POSITIVE
- summary: [explore 落点试例] 冷启动 Step0：测定 report_bug 在本 cwd 下的落盘位置（试例，不污染项目账本）
- detail: server cwd = E:\IDEProjects\AI\lang-zone\.fist-explore-cron；project_dir="." 。测定后按冷启动清单处理试例。

## BUG-2 [2026-09-29T15:02:50Z] [medium] OPEN
- summary: [explore] 工具侧：watchdog_tick 的 meta_prompt_path 按相对 cwd 解析（绝对路径被改写为 templates/pipeline_mode_explore.md）
- detail: 冷启动实测（ns=lz-explore，cwd=E:\IDEProjects\AI\lang-zone\.fist-explore-cron）：传入 meta_prompt_path=E:\IDEProjects\AI\FIST-Mbt\templates\pipeline_mode_explore.md，watchdog_tick 返回 -32000 `mode=explore 模板不存在: templates/pipeline_mode_explore.md`；回执 detail.meta_prompt_resolved 显示 `templates/pipeline_mode_explore.md`，即被按相对 server cwd 重写。同时 mode_templates 的 missing 为全部 7 个模式，判定基准同为 cwd 下的 templates/。绕过方式：在任务库目录内置 FIST-Mbt 模板的只读镜像（复制，未改 FIST-Mbt 仓库），之后 missing=[] 且 watchdog_tick 正常返回。建议上游改为绝对路径优先解析，或支持模板目录配置。属于 FIST-Mbt 工具缺陷，按红线只入账不动其仓库。

## BUG-3 [2026-09-29T15:02:50Z] [low] OPEN
- summary: [explore] 环境：任务库目录 .fist-explore-cron/ 未被 .gitignore 覆盖，且 FIST-Mbt 构建产物 main.js 缺 ESM createRequire shim
- detail: ① git check-ignore -v .fist-explore-cron 返回 1（未忽略）；既有 .fist-loop-20260927 / .fist-night / .fist-hunt-20260926 等同样未忽略，git status 中呈 ?? 。冷启动清单要求任务库被 .gitignore 覆盖，但参谋红线禁止改任何配置 ⇒ 挂账待人工（建议 .gitignore 增 .fist-*/ 或 .fist-explore-cron/）。② 2026-09-29 拉起 server 报 ReferenceError: require is not defined in ES module scope（_build/js/debug/build/cmd/main/main.js:1194），即 FIST-Mbt 产物被重建后丢失 ESM shim；用厂商脚本 python scripts/patch_esm_main.py <main.js 绝对路径> 幂等补回后 tools/list 正常（自动发现模式只补了最大的 cli.js，未覆盖 main.js）。两项均属环境/工具侧，只入账不改仓库。


## BUG-4 [2026-09-29T18:06:36Z] [medium] OPEN
- summary: [explore] 表/码口径分叉: publish 的 created_by 拒绝 watchdog，仅收 human_steward
- detail: 实例参数卡与第三节流程写「续轮 publish created_by="watchdog"（冷启动首轮 human_steward）」；实测 2026-09-30 ns=lz-explore：`publish(created_by="watchdog")` 返回 -32000 `publish 仅限人类指挥官（created_by=human_steward）`。改用 human_steward 后正常。另：watchdog_tick 传 next_created_by="watchdog" 未报冲突（本轮 active=0、未触发自动派单），两者口径是否需统一请人工裁定。按红线只入账，不动 FIST-Mbt 仓库；本轮参谋以 human_steward 落单以保证派单不空转。

## BUG-5 [2026-10-09T15:13:26Z] [low] OPEN
- summary: [explore] watchdog_tick 把 explore 模板全文发布成待领取任务（T0r7），污染 task_triage 派单队列
- detail: 2026-10-09 本轮 watchdog_tick（ns=lz-explore）回 action=advanced、new_task_id=T0r7；get T0r7 显示其 status=待领取、description 为 `templates/pipeline_mode_explore.md` 的**全文**（含「你是本项目的派单参谋…」等元提示词正文）。task_triage 随即将 T0r7 与真实任务 T0r6 一并返回（count=2），参谋需自行甄别哪条是活。建议：watchdog 发布下轮任务时应注入「本轮目标描述」而非模板全文，或对 meta 类任务打标并排除出 triage。属 FIST-Mbt 工具行为，按红线只入账不动其仓库；本轮已按 T0r6 为真实腿、T0r7 为噪音处理。

## BUG-6 [2026-10-10T00:20:41Z] [medium] OPEN
- summary: [explore] 转人工：T0r9（polish 腿）连续 2 次心跳超时、无人消费，流水线停在「派单→无执行者→自愈」空转
- detail: 时间线：2026-10-09 13:45 生成第二档提示词 prompts/20261009.13.45.00.md（[难度梯度 2/3:中] → c=3 → polish，Box<dyn Fn>→Rc<dyn Fn> 一族收口）；2026-10-10 09:10 参谋按该提示词派单 T0r9 并接一脚（reserve 3/3 → publish → claim → heartbeat → release），output_validate pass。此后 T0r9 连续两轮被 watchdog 判心跳超时并自愈回滚（10-10 14:05 第 1 次、10-10 18:55 第 2 次；最后更新 2026-10-10T00:20:17Z），期间仓库 **0 个新 commit**（最新仍为 b44d3c1 10-09 13:32）。\n根因（结构性，非偶发）：explore 是派单参谋，派出的单必须由**另一个执行者**消费；本计划任务每次唤醒的都是参谋本人，参谋开工即越权 ⇒ 派单后必然无人施工。\n参谋已采取的合规动作：分支② 两轮均零动作（不重启、不重新派单、不改源码）。按「连续自愈 ≥2 次转人工」的既定口径，本轮挂账。\n请人工三选一：A 加第二个计划任务当执行者（接活→按 templates/pipeline_mode_polish.md 施工→submit）；B 允许参谋接活即开工（突破 explore 角色边界）；C 暂停 explore、只保留 lzc 六环循环。另请处置：T0r9 归档或改派；队列噪音任务 T0r7/T0r8（description = 模板全文，见 BUG-5）归档。
- task_id: T0r10


# 重构回归质检报告 — 2026-10-30（T0r46 / lang-zone / atomcode-regress）

## 0. 范围与口径

对最近 4 次 move-only 大重构（helpers.rs 三切 ad05098、parser/expr.rs 拆 parse_pattern c74af9f、
macros/expand.rs 拆宏解析器、embed 测试临时目录修复 79f577c=HEAD）做**行为面重新量尺**，
只读 + 只写报告（`Reports/` 与 `temp/regress/`），不改产品源码、不 commit。

- HEAD：`79f577c` 2026-10-30 09:13:13（test(embed): 单测临时目录进程内唯一）
- 工作树：仅 `Cargo.lock`（M）与若干未跟踪工具文件；`src/parser/expr.rs` **无并行车道半拆中间态**
  （parse_pattern 已在独立 `src/parser/pattern.rs:16`），全量测试无需等待重试。

## 1. 构建（target/lane-sweep 对应 HEAD）

- 检查：`lang-zone.exe` mtime 08:55:24 晚于 HEAD 唯一改动文件 `src/embed/artifact.rs`（08:52:53），
  且 src/tests/benches 无文件比二进制新。
- `CARGO_TARGET_DIR=target/lane-sweep cargo build` 09:27:10 确认：**Finished in 0.15s，零重编译**
  ——既有二进制即对应 HEAD，可直接用于质检。
- 补建运行时：`cargo build -p lz_builtins`（09:34，deps 哈希 `7caf85a7`，顶层
  `liblz_builtins.rlib` nlink=2 hardlink 就位），lane-sweep 目录自此自足。

## 2. 121 件语料 sweep（temp/regress/fastsweep.sh，BIN/RLIB/DEPS 全部指向 lane-sweep，未改原脚本）

跑批 09:38:37 → 10:01:30（121/121 有结果行，COPY_FAIL=0，MISSING=0）。

| 分类 | 首轮 | r3 | G5~G8 修后基线 | **本轮** |
|---|---|---|---|---|
| RSLZ_OK | 37 | 67 | 77 | **78** |
| RSLZ_FAIL_NATIVE_OK | 40 | 10 | 0 | **0** |
| BOTH_FAIL | 44 | 44 | 44 | **43** |
| NATIVE_FAIL_RSLZ_OK | 0 | 0 | 0 | 0 |

日志：`temp/regress/sweep.log`；对照基线快照 `temp/regress/sweep_prev_baseline.log`（=r3 逐条 67/10/44）。

### 2.1 逐条差分（r3 基线 → 本轮，同 id 121/121 对齐）

- **红转绿 11 条**：
  - 9× `div-tools/probes/p{5,8,12,12b,12c,16,22,24}_*.lz`：RSLZ_FAIL_NATIVE_OK → RSLZ_OK（G5~G8 修复兑现，与基线预期一致）
  - 2× `div-tools/probe_release.lz`、`div-tools/probe_to_string.lz`：BOTH_FAIL → RSLZ_OK
    （两件同形：int/str/bool `.to_string()`，r3 双路 E0308；与 G5~G8 基线 77/0/44 对照，
    其中 1 条在那之后、本轮之前转绿——surviving 日志无法唯一区分发生在修复尾巴还是重构区间，
    判定见 §2.3）
- **绿转红 0 条**（RSLZ_OK → 任何非绿：无）。
- **分类迁移 1 条**（非重构破坏，见 §2.2）：
  `div-tools/syntax_probes/neg_error_uncaught_panic_ok.lz` RSLZ_FAIL_NATIVE_OK → BOTH_FAIL。

### 2.2 neg_error_uncaught_panic_ok 定性：探针变化，不是重构意外破坏

该样本是**反例探针**（设计期望 = 编译失败）。10-29 17:16 提交 `0ddb6ca` 把探针从
`raise "boom"`（字符串 raise，实现有意豁免 raises，恒 rc=0）改为
`raise AppError.Boom("boom")`（类型化 raise，应触发语义门禁）。实测：

- 新 HEAD 二进制：rc=1，诊断「函数 main 体内使用了 raise 但未在签名中声明 raises」——**恰是探针新设计期望的行为**。
- 旧 lane-redlist 二进制（重构前，10-28 09:20 build）跑同一探针：**同样 rc=1**。
  两代二进制对这条的行为一致 ⇒ 红来自**语料（探针文本）变化**，与 4 次重构无关。
- 旧 r3 基线跑的是旧探针（字符串 raise，native 恒 rc=0 假绿）；探针换新后 sweep
  正向口径把它记成 BOTH_FAIL 属预期归类，sweep.list 中另有 24 件同族 neg_ 反例。

### 2.3 新红清单

**0 条**。判据：
1. 无任何 RSLZ_OK → 红 的样本。
2. 本轮失败明细 E-code 集合 `{E0308×10, E0599×2}` ⊆ 基线集合，**无未见过的 E-code**；
   非 E-code 形态报错消息 0 新增。
3. probe_release/probe_to_string 属红转绿方向；唯一负向迁移（neg_error）有双二进制对照实证排除重构因素。

## 3. 行为差分 rs_lz_behdiff（16 条）

`cargo test --test rs_lz_behdiff`（lane-sweep）：10:12:01 → 10:15:25，
**16 passed / 0 failed / 0 skipped**（169.90s，exit=0）。
日志 `temp/regress/behdiff.log`。与基线（16 全绿）一致。

## 4. DEMO 门禁 demo_codegen_compile --ignored

`cargo test --test demo_codegen_compile -- --ignored`：10:17:17 → 10:20:23，
**`all_demos_generated_rust_compiles ... ok`（1 passed / 0 failed，180.60s）**。
覆盖件数经 `find DEMO -name '*.lz' -not -path '*99_errors*' | wc -l` = **327** 复核。
日志 `temp/regress/demo_gate.log`。与 r3 基线（同 1 闸全过）一致。

## 5. 全量 cargo test --no-fail-fast（串行）

10:23:17 → 10:45:10（约 22 分钟），exit=0。

| 指标 | 基线 | 本轮 |
|---|---|---|
| test targets（result 行数） | 50 | **50** |
| passed | 1033 | **1033** |
| failed | 0 | **0** |
| ignored | 3 | 3 |

日志 `temp/regress/full_test.log`。lib 单测 469 passed / 0 failed。
无并行车道中间态阻塞（§0），一次跑通，无需 3 分钟重试路径。

## 6. 结论

四项读数全部持平或优于基线：

- sweep **78/0/43**（基线 77/0/44，+1 绿 −1 双红；0 新红、0 新 E-code）
- behdiff **16/16**
- DEMO 闸 **327 件全编译通过**
- 全量 **50 targets / 1033 passed / 0 failed**

4 次 move-only 重构（helpers.rs 三切、parse_pattern 拆分、宏解析器拆分、embed 测试目录修复）
在行为面**未见接线层面的隐性破坏**；sha1 等价证明之外的实证回归成立。
唯一负向信号（neg_error 反例探针）已实证归因于 10-29 探针语义更新（0ddb6ca），非重构产物。

### 附：读数全部来自日志逐条 grep/解析计数，无估算。产物索引

| 项 | 路径 |
|---|---|
| sweep 明细/汇总 | `temp/regress/sweep.log`（+ `results/*.txt`、`sweep_prev_baseline.log`） |
| behdiff | `temp/regress/behdiff.log` |
| DEMO 闸 | `temp/regress/demo_gate.log` |
| 全量测试 | `temp/regress/full_test.log` |
| 复制改写脚本 | `temp/regress/fastsweep.sh`、`temp/regress/_worker.sh`（仅 BIN/RLIB/DEPS/工作目录改 lane-sweep/regress，原 `temp/redlist/` 未动） |

# 夜间循环缺陷账（lang-zone · lz）

- run_id: `2026-09-26-night`（窗口日 2026-09-26，时间窗 [23:00, 08:00)）
- 建账：2026-09-27 00:2x（R1-寻虫）
- 门禁基线：`cargo test --no-fail-fast` → **676 passed / 4 failed / 680 total**，2 个 target 全红
  （`tests/find_bug_bugs.rs` 3 failed、`tests/lz_ir_bootstrap.rs` 1 failed）
- 入账口径：无最小复现不入账；误报不刷账。全部 4 单均为门禁实失败（非静态推断）。

---

## BUG-N1 · PathAccess 哨兵 `\u{1}PATH\u{1}` 在 **Call** 发射点未剥离

| 项 | 值 |
|---|---|
| 严重度 | **critical**（产物不可编译，跨 2 个测试 target） |
| 落点 | `src/ir/codegen/mod.rs` **（★遗留集文件）** |
| 状态 | **OPEN · 挂账待人工**（夜间零触碰遗留集，补丁已备未应用） |
| 复现 1 | `cargo test --test find_bug_bugs ty004_params_type_erase` |
| 复现 2 | `cargo test --test lz_ir_bootstrap lz_emit_rs_lz_matches_native_codegen` |
| 原始报错 | `error: unknown start of token: \u{1}` → `__Params::<0x01>PATH<0x01>new()`、`String::<0x01>PATH<0x01>new()` |
| 根因 | `builder.rs:4974-4981` 将 `A::b`（PathAccess）降级为 `FieldAccess{ field: "\u{1}PATH\u{1}" + segment }`。<br>FieldAccess **表达式**发射点 `codegen/mod.rs:12704-12822` 已剥离（BUG-8 修复，12717-12822）；<br>但 **Call** 发射点 `codegen/mod.rs:10255-10347` 把含哨兵的 `field` 原样拼进 `format!("{}::{}({})", base_s, field, ...)`（10347）→ 哨兵泄漏 |
| 最小修复 | ≤20 行、不动语义：Call 分支入口剥离哨兵 + `is_path_access` 强制 `sep="::"` |
| 补丁 | `.fist-night/patches/BUG-N1_call_path_sentinel.patch`（`git apply --check` 通过，未应用） |
| 回归锁死 | 上述 2 个用例由 PASS 回归；需重跑 `cargo test --no-fail-fast` 全绿 |

## BUG-N2 · `time::Duration::fromMillis` 未映射（std 模块链拦截被哨兵绕过）

| 项 | 值 |
|---|---|
| 严重度 | high（E0433，产物不可编译） |
| 落点 | `src/ir/codegen/mod.rs:9628-9670` **（★遗留集文件）** |
| 状态 | **OPEN · 挂账待人工** |
| 复现 | `cargo test --test find_bug_bugs sb001_time_method` |
| 原始报错 | `error[E0433]: cannot find module or crate 'time' in this scope` → 产物 `time::Duration::fromMillis(1500i64)` |
| 根因 | `.lz` 源码写作 `time::Duration.fromMillis(1500)`，`time::Duration` 是 **PathAccess** → `builder.rs:4978` 给段名加 `\u{1}PATH\u{1}` 前缀。<br>拦截点 9640 用 `types.contains(&type_name.as_str())` 精确匹配白名单 `["Duration","Instant"]`，而 `type_name == "\u{1}PATH\u{1}Duration"` → **匹配失败** → 拦截不生效 → 走通用 Call 路径，`fromMillis` 未做 camelCase→snake_case、`time` 未加 `std::` 前缀 |
| 最小修复 | 9640 前 `let type_name_s = type_name.trim_start_matches(PATH_SENTINEL);`，匹配与输出（含 `type_name == "Duration"` 的 u64 后缀处理、最终 `std::{}::{}::{}`）统一改用 `type_name_s`。同源哨兵，与 BUG-N1 一并修 |
| 回归锁死 | `sb001_time_method` 转 PASS |

## BUG-N3 · 泛型 `compose` 闭包参数缺类型标注 → E0282

| 项 | 值 |
|---|---|
| 严重度 | medium |
| 落点 | `src/ir/codegen/mod.rs`（闭包参数发射 / 泛型调用点）**（★遗留集文件）** |
| 状态 | **OPEN · 挂账待人工（语义级，卡 C 禁区：只挂账不修）** |
| 复现 | `cargo test --test find_bug_bugs core_compose_ok` |
| 原始报错 | `error[E0282]: type annotations needed` @ `FIND_BUG/core/compose.rs:55` |
| 生成产物 | `compose<a,b,c>(f: impl Fn(b)->c, g: impl Fn(a)->b) -> Box<dyn Fn(a)->c>`；<br>调用点 `compose(move \|x\| { (x).clone() * 2i64 }, move \|x\| { LzAdd::__add__(x, 3i64) })` |
| 根因 | `.lz` 中 `\|x\| f(g(x))` 无参数类型标注；Rust 无法从 `impl Fn(a)->b` 反推 `a`（闭包参数无标注时不做双向推断） |
| 候选方案 | ① 调用点补 turbofish `compose::<i64,i64,i64>(...)`（需类型实参推断链路）；<br>② 生成闭包时按形参 `fn(a)->b` 反向标注参数类型；<br>③ 改签名为具名泛型 `F: Fn(b)->c, G: Fn(a)->b` 并靠实参闭包推断。<br>**属语义/设计决策，需人工定夺** |
| 回归锁死 | `core_compose_ok` 转 PASS |

---

## 汇总

| 编号 | 严重度 | 状态 | 是否落遗留集 |
|---|---|---|---|
| BUG-N1 | critical | OPEN · 待人工 | 是（codegen/mod.rs） |
| BUG-N2 | high | OPEN · 待人工 | 是（codegen/mod.rs） |
| BUG-N3 | medium | OPEN · 待人工 | 是（codegen/mod.rs） |
| BUG-N4 | — | **撤回**（与 BUG-N1 同根因同源，合并入 N1，不重复计账） | — |

> BUG-N4 原为 `lz_emit_rs_lz_matches_native_codegen`（rs-lz 路由 `String::<0x01>PATH<0x01>new()`），
> 与 BUG-N1 同一哨兵泄漏根因，合并记账，避免重复计数。
## BUG-1 [2026-09-26T16:45:35Z] [critical] OPEN
- summary: [夜巡R1] src/ir/codegen/mod.rs Call 发射点未剥离 PathAccess 哨兵，产物含 U+0001 不可编译
- detail: 复现: cargo test --test find_bug_bugs ty004_params_type_erase / cargo test --test lz_ir_bootstrap lz_emit_rs_lz_matches_native_codegen
报错: error: unknown start of token: \u{1} -> __Params::<U+0001>PATH<U+0001>new() / String::<U+0001>PATH<U+0001>new()
根因: src/ir/builder.rs:4974-4981 把 PathAccess 降级为 FieldAccess{field:\u{1}PATH\u{1}+segment}；codegen/mod.rs:12704-12822 表达式发射点已剥离，但 10255-10347 Call 发射点把含哨兵 field 原样拼进 format!("{}::{}({})",base_s,field,..) (10347)。
补丁: .fist-night/patches/BUG-N1_call_path_sentinel.patch（git apply --check 通过，未应用——落点属遗留集，夜间零触碰）
- reported_by: night-executor
- task_id: T0r2

## BUG-2 [2026-09-26T16:45:35Z] [high] OPEN
- summary: [夜巡R1] src/ir/codegen/mod.rs:9640 std 模块链拦截被 PathAccess 哨兵绕过，time::Duration::fromMillis 未映射
- detail: 复现: cargo test --test find_bug_bugs sb001_time_method
报错: error[E0433]: cannot find module or crate `time` in this scope -> 产物 time::Duration::fromMillis(1500i64)
根因: .lz 写 time::Duration.fromMillis(1500)，time::Duration 是 PathAccess -> builder.rs:4978 段名带 \u{1}PATH\u{1} 前缀；codegen/mod.rs:9640 types.contains(&type_name.as_str()) 与白名单 ["Duration","Instant"] 精确匹配失败 -> 拦截不生效，camelCase->snake_case 与 std:: 前缀均未施加。
修复: 9640 前 trim_start_matches(PATH_SENTINEL) 得到 type_name_s，匹配/输出/u64 后缀处理统一改用 type_name_s（与 BUG-N1 同源）。
- reported_by: night-executor
- task_id: T0r3

## BUG-3 [2026-09-26T16:45:35Z] [medium] OPEN
- summary: [夜巡R1] 泛型 compose 闭包参数缺类型标注 -> E0282（语义级，需人工定夺）
- detail: 复现: cargo test --test find_bug_bugs core_compose_ok
报错: error[E0282]: type annotations needed @ FIND_BUG/core/compose.rs:55
产物: compose<a,b,c>(f: impl Fn(b)->c, g: impl Fn(a)->b) -> Box<dyn Fn(a)->c)；调用点 compose(move |x| {(x).clone()*2i64}, move |x| {LzAdd::__add__(x,3i64)})
根因: .lz 中 |x| f(g(x)) 无参数类型标注，Rust 无法从 impl Fn(a)->b 反推 a。
候选: ① 调用点补 turbofish ② 生成闭包时按形参 fn(a)->b 反向标注 ③ 改具名泛型 F/G 靠实参推断。属语义/设计决策，卡 C 禁区只挂账不修。
- reported_by: night-executor
- task_id: T0r4

## BUG-4 [2026-09-26T16:50:46Z] [critical] OPEN
- summary: [夜巡R1] src/ir/codegen/mod.rs Call 发射点未剥离 PathAccess 哨兵，产物含 U+0001 不可编译
- detail: 复现: cargo test --test find_bug_bugs ty004_params_type_erase / cargo test --test lz_ir_bootstrap lz_emit_rs_lz_matches_native_codegen
报错: error: unknown start of token: \u{1} -> __Params::<U+0001>PATH<U+0001>new() / String::<U+0001>PATH<U+0001>new()
根因: src/ir/builder.rs:4974-4981 把 PathAccess 降级为 FieldAccess{field:\u{1}PATH\u{1}+segment}；codegen/mod.rs:12704-12822 表达式发射点已剥离，但 10255-10347 Call 发射点把含哨兵 field 原样拼进 format!("{}::{}({})",base_s,field,..) (10347)。
补丁: .fist-night/patches/BUG-N1_call_path_sentinel.patch（git apply --check 通过，未应用——落点属遗留集，夜间零触碰）
- reported_by: night-executor
- task_id: T0r5

## BUG-5 [2026-09-26T16:50:47Z] [high] OPEN
- summary: [夜巡R1] src/ir/codegen/mod.rs:9640 std 模块链拦截被 PathAccess 哨兵绕过，time::Duration::fromMillis 未映射
- detail: 复现: cargo test --test find_bug_bugs sb001_time_method
报错: error[E0433]: cannot find module or crate `time` in this scope -> 产物 time::Duration::fromMillis(1500i64)
根因: .lz 写 time::Duration.fromMillis(1500)，time::Duration 是 PathAccess -> builder.rs:4978 段名带 \u{1}PATH\u{1} 前缀；codegen/mod.rs:9640 types.contains(&type_name.as_str()) 与白名单 ["Duration","Instant"] 精确匹配失败 -> 拦截不生效，camelCase->snake_case 与 std:: 前缀均未施加。
修复: 9640 前 trim_start_matches(PATH_SENTINEL) 得到 type_name_s，匹配/输出/u64 后缀处理统一改用 type_name_s（与 BUG-N1 同源）。
- reported_by: night-executor
- task_id: T0r6

## BUG-6 [2026-09-26T16:50:47Z] [medium] OPEN
- summary: [夜巡R1] 泛型 compose 闭包参数缺类型标注 -> E0282（语义级，需人工定夺）
- detail: 复现: cargo test --test find_bug_bugs core_compose_ok
报错: error[E0282]: type annotations needed @ FIND_BUG/core/compose.rs:55
产物: compose<a,b,c>(f: impl Fn(b)->c, g: impl Fn(a)->b) -> Box<dyn Fn(a)->c)；调用点 compose(move |x| {(x).clone()*2i64}, move |x| {LzAdd::__add__(x,3i64)})
根因: .lz 中 |x| f(g(x)) 无参数类型标注，Rust 无法从 impl Fn(a)->b 反推 a。
候选: ① 调用点补 turbofish ② 生成闭包时按形参 fn(a)->b 反向标注 ③ 改具名泛型 F/G 靠实参推断。属语义/设计决策，卡 C 禁区只挂账不修。
- reported_by: night-executor
- task_id: T0r7


---

# R1-修复 进展（2026-09-27 03:02–03:40，根任务 T0r8）

> 遗留集零触碰 → 全部修复在仓库外隔离副本 `E:\IDEProjects\AI\_lz_night_scratch\lang-zone` 实跑验证。

| 编号 | 原状态 | 现状态 | 隔离副本实跑结论 |
|---|---|---|---|
| BUG-N1 | OPEN | **已验证修复 · 补丁待人工应用** | `ty004_params_type_erase` + `lz_emit_rs_lz_matches_native_codegen` 转 PASS |
| BUG-N2 | OPEN | **已验证修复 · 补丁待人工应用** | `sb001_time_method` 转 PASS（含次级缺陷：方法段同样带哨兵，一并剥离） |
| BUG-N3 | OPEN | **挂账不修（卡 C 禁区）· 已备验证补丁** | 候选④ 语料级调用点标注 `\|x: int\|` 实测转 PASS，是否采纳待人工 |

- 合并补丁：`.fist-night/patches/BUG-N1N2_path_sentinel.patch`（7 hunk / +34 −6，`git apply --check` 通过，落遗留集未应用）
- 语料补丁：`.fist-night/patches/BUG-N3_callsite_annotation.patch`（1 行 2 标注，`git apply --check` 通过，未应用）
- 隔离副本全量门禁：**679 passed / 1 failed / 680 total**（基线 676/4/680）；测试总数 680 不变；
  `dual_route_golden` 4/4、`incremental_golden` 1/1 全绿 → **golden 快照无需再对齐**
- 报告：`.fist-night/memory/reviews/20260927.03.37.02.md`
## BUG-7 [2026-09-26T22:31:29Z] [high] OPEN
- summary: [夜巡R1-验证] DEMO/lz_std/list.lz+set.lz filter 谓词闭包多发射解引用 -> E0614
- detail: 复现: cargo test --test demo_codegen_compile -- --ignored（全量 DEMO 闸门，非门禁）
产物: DEMO/lz_std/list.rs:679 / set.rs:663 -> move |x: &i64| -> bool { *(x).clone() % 2i64 == 0i64 }
报错: error[E0614]: type `i64` cannot be dereferenced
对照: map 闭包 (list.rs:675) 为 (x).clone() * 2i64 无误；仅 filter 谓词多出前缀 *
归因: 无本夜补丁对照组同 326/307/19 且同样 E0614 -> 非夜间引入，属 2026-09-26 遗留集批次遗留
- task_id: T0r10


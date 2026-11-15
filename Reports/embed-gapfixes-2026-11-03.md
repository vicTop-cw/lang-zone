# T0r50：T0r49 体检出的三处度量失真 + 一处静默丢弃 —— 落地记录（2026-11-03）

> 依据：`Reports/embed-gaps-2026-10-30.md`（T0r49）。
> 改动面：`src/embed/artifact.rs`(+63/-13 内)、`src/embed/mod.rs`、`src/embed/adapters.rs`、`src/bridge/cli.rs`，
> 新建 `tests/embed_gapfixes.rs`（17 条）。禁区未碰：`src/ir/**`、`tests/rs_lz_families.rs`、`tests/embed_unify_r15.rs`、`memory/bugs.md`。
> 探针与前后读数留在 `temp/gapfix/{before,after}/`（before 用 T0r49 车道遗留的
> `target/lane-embedgaps/debug/lang-zone.exe`，即修复前的二进制）。

## 一、四条逐条

### ① `toolchain_available` 量的是 run 命令，不是建库工具链 —— 已修

`src/embed/artifact.rs::manifest_text_at` 把原来的一行拆成两行：

| 行 | 新口径 |
|---|---|
| `toolchain_available=` | 只看 **build** 工具链在场性（`t.build.tool_available()`）；无建库步（解释型语言）= `n/a`，与「缺席(no)」分开 |
| `run_available=` | 只看 **运行命令/产物在位性**：含分隔符的相对程序名按**产物目录**解析（与垫片 `__lz_resolve_program` 同一把尺），裸名走 PATH，绝对路径看存在性 |

`write_artifacts` 落盘时传 `Some(产物目录)`；`manifest_text`（无目录）保留为兼容入口。
仍是确定性文本（无绝对路径、无时间戳）。

前后对比（`int add(int a,int b)` + 本机 gcc 在场）：

```
修前（默认路径 & 建库成功之后，两态都是）      toolchain_available=no   ← gcc 明明在 PATH 上
修后（默认路径）                             toolchain_available=yes  run_available=no
修后（--embed-build gcc 建库成功后复跑）       toolchain_available=yes  run_available=yes
python（解释型，无建库步）                    toolchain_available=n/a  run_available=yes
```

`build_artifact` 的 Skipped 分支本来就量 build 命令 ⇒ 这两行与建库判据从此同源，不再互相矛盾。

### ② scala `dispatch=none` 却照发可调用垫片 —— 已修（加门）

`src/embed/mod.rs::callable_methods()` 加第二道门：`Library && !has_dispatch_file()` ⇒ 全员不可调用；
`resolve()` 里被这道门挡下的方法就地记 `unsupported = NO_DISPATCH_SURFACE`
（`"该语言本版本不生成分发入口 ⇒ 无 CLI 调用面，只可被同语言导入"`），
⇒ MANIFEST 逐条点名 + 编译期 `embed 未覆盖: …` 回执照打，与代码注释「不出假垫片」的承诺对齐。

```
修前 scala MANIFEST   dispatch=none / method=add params=a:int,b:int ret=none callable
                     垫片: pub fn scala_calc_add(a: i64, b: i64) {   ← 调起来必 panic
修后 scala MANIFEST   dispatch=none / method=add params=a:int,b:int ret=none 该语言本版本不生成分发入口_⇒_…
                     垫片: 无 scala_calc_add；回执 embed 未覆盖: scala_calc::add —— …
python 不受牵连        method=add … callable 与 pub fn python_calc_add 一字不变
```

**残留（未修，属禁区）**：作者若仍调用被挡下的函数，编译期有点名，但运行期落到 codegen 的
unknown_extern 兜底桩（`i64::MAX`）。实测两侧对照（`temp/gapfix/{before,after}/p_scala_call`）：
修前 `rc=101 … 无法启动 scala（program not found）`；修后 `rc=0` 打印 `9223372036854775807`。
比「静默 Unit」更可归因，但**运行期仍不是编译错误**；收口要动 `src/ir/codegen/**`（T0r32 同族缺口），
本条按禁区未做，已在 `tests/embed_gapfixes.rs::author_still_calling_a_gated_method_gets_a_named_receipt`
里钉住「编译期必须点名」这一已兑现面（不把兜底桩行为写成正面契约）。

**另一处同源失真也未做**：报告 §2.3 提到的 `ret=none`（scala 的 `: Int` 返回注解解析不到）。
加门之后 scala 库方法整体不可调用、不再出垫片 ⇒ 这个读数已**惰性无害**，且任务书第②条只要求加门，
故未扩范围（避免在无收益处引入回归面）。

### ③ `.bat/.cmd` 形态工具链：读数说在场、spawn 说找不到 —— 已修（走 `cmd /C`）

判据换尺边界由本机实测划出（`temp/gapfix/quot2.rs` / `quot4.rs`，可复跑）：

```
Command::new("lztool")      PATH 上只有 lztool.bat/.cmd  ⇒ ERR program not found   ← 要换尺
Command::new("lztool.bat")  裸名带扩展名                ⇒ code=0 起得来          ← 不换
Command::new(<绝对 .bat>)                               ⇒ code=0 起得来          ← 不换
Command::new("rustc"/"python") 有同名 .exe 原生映像      ⇒ 不换（行为一字不动）
```

两侧同一把尺：
- 建库侧 `src/embed/artifact.rs::build_artifact` 用 `mod::cmd_route_shape`（新增
  `mod::needs_cmd_route`，Windows-only，非 Windows 恒 false）；
- 垫片侧 `src/bridge/cli.rs` 新增 `__lz_spawn_plan(dir, program)`，两个 runner 各用它拼
  `Command::new` + 前置段 ⇒ **每个 runner 仍恰好一处 `Command::new`**（`Command::new` 总数不变量守住，
  `embed_adds_exactly_one_spawn_site…` 与新增的 `shim_still_has_exactly_two_spawn_sites…` 双重钉）。

`tool_available` 继续认 `.bat/.cmd`（读数不许把「装了却用不了」谎报成「没装」）——修的是执行侧与读数对齐。

```
修前 scala --embed-build   embed 建库: scala_calc 失败: spawn 失败: program not found  rc=1（非零退出）
修后 scala --embed-build   embed 建库: scala_calc 完成 (scalac scala_calc.scala)       rc=0
                          落盘 scala_calc$package.class / .tasty（真产物）
```

垫片侧的 bat 运行面本机**没有端到端可用语言**（scala 无分发入口、python 有 .exe），
所以用隔离方式真跑生成代码：`generated_shim_plan_really_spawns_a_bat_only_tool` 把生成物里的
`__lz_spawn_plan` 原文取出编成小程序，面对只有 `.bat` 的工具链，断言计划是 `cmd /C` 且实际打印 `OK:42`。
**不谎称端到端验证过 scala 调用面**。

### ④ 无返回注解但体内有带值 `return` ⇒ 值被静默丢弃 —— 已修（判 unsupported + 逐条点名）

`src/embed/adapters.rs` 新增 `body_has_value_return` / `mark_discarded_return`（常量 `DISCARDED_RETURN`），
接入 python `methods()` 与共用的 `fn_from_toplevel`（mojo/scala）：
`ret == None` 且**已有更高优先级点名原因不覆盖**时置 unsupported。
体 = 定义行之后连续缩进行（含单行体 `: return [n]` 这一形态，探针 p1 报的正是它）；
注释行、裸 `return`、`returns` 同前缀标识符都不算。

```
修前 python MANIFEST   method=as_list params=n:int ret=none callable   ← 有返回值却标 callable
                     垫片: pub fn python_calc_as_list(n: i64) { … }    调用成功、值丢掉、零回执
修后 python MANIFEST   method=as_list params=n:int ret=none 无返回注解且函数体有_return_<expr>_⇒_结果会被静默丢弃
                     回执: embed 未覆盖: python_calc::as_list —— 无返回注解且函数体有 return <expr> ⇒ …
                     垫片里无 python_calc_as_list；python_calc_ok 不受牵连
```

不误伤面已钉（`genuine_unit_functions_are_not_flagged`）：`pass` / 裸 `return` / 注释里的 return / `returns=` 变量 / 有注解函数
五类全部保持 callable，`unsupported_methods()==0`。归因不覆盖已钉（`existing_signature_reason_is_not_overwritten`）。

## 二、判据（`tests/embed_gapfixes.rs`，17 条，全绿）

| 条 | 测试 |
|---|---|
| ① | `interpreted_lang_reports_na_for_build_toolchain` / `c_lines_separate_build_toolchain_from_run_artifact` / `absent_build_toolchain_is_no_not_na_and_does_not_leak_into_run_line` |
| ② | `scala_without_dispatch_surface_gets_no_callable_shim` / `langs_with_dispatch_surface_keep_their_callable_face` / `scala_embed_emits_no_wrapper_and_names_the_gap` / `author_still_calling_a_gated_method_gets_a_named_receipt` |
| ③ | `bare_bat_tool_is_seen_by_probe_and_routable_by_spawn`（含 PATH 上只有 .bat 的假工具链端到端 Built）/ `cmd_route_is_windows_only_and_never_wraps_plain_names_elsewhere` / `generated_shim_plan_really_spawns_a_bat_only_tool` / `shim_still_has_exactly_two_spawn_sites_after_the_bat_route` |
| ④ | `value_return_without_annotation_is_named_not_silently_dropped` / `one_line_value_return_body_is_also_named` / `genuine_unit_functions_are_not_flagged` / `toplevel_fn_shape_without_return_annotation_is_also_named` / `existing_signature_reason_is_not_overwritten` / `discarded_return_is_reported_on_the_receipt_and_absent_from_generated_code` |

每条都有负向对照（判据自己坏掉时必红）。`③` 里两条会临时改进程 PATH，加了 `PATH_LOCK` 串行锁。

## 三、读数

主树（`CARGO_TARGET_DIR=target/lane-embedfix`，串行）：

```
cargo build                       0 error / lib 告警 9（与基线逐条同 kind 同位置）
cargo test --test embed_gapfixes        17 passed; 0 failed      （新建）
cargo test --test embed_unify_r15       21 passed; 0 failed      （21 不回归）
cargo test --test embed_dedup            4 passed; 0 failed
cargo test --test embed_project_grouping 4 passed; 0 failed
cargo test --test embed_project_emit     3 passed; 0 failed
cargo test --lib                       469 passed; 0 failed
```

另跑（未在任务书里、但被本改动面波及的邻近门禁）：
`embed_stem_parity 3/0`、`embed_stub_shadowing 4/0`、`bridge_embed 8/0`、`bridge_e2e 9/0`、
`lz_ir_bootstrap 25/0`（逐字节门禁）、`lz_frontend_bootstrap 7/0`。

### 并发车道干扰（如实记录，非本车道破坏）

跑判据期间 `src/ir/builder/convert_stmt.rs`（T0r51 车道、本车道禁区）被改成半截，
`cargo build --lib` 一度报 `E0027 pattern does not mention field 'pattern'`，
`lz_frontend_bootstrap` 也随之编不过。为拿可归因读数，另开 HEAD worktree
（`F:/AI/iso-embedfix`，只叠本车道 4 个文件 + 测试）复跑：
`embed_gapfixes 17/0`、`embed_unify_r15 21/0`、`embed_dedup 4/0`、`embed_project_grouping 4/0`、
`embed_project_emit 3/0`、`--lib 469/0` 全绿 ⇒ 那一次红在别人的 builder 改动，与本车道无关。
稍后 T0r51 自己修好该文件，主树复跑即全绿（上表即主树最终读数）。工作树已清理该 worktree（`git worktree list` 只剩主树 + lz-head）。

## 四、未修清单（点名，不掩盖）

1. scala 的 `: <Type>` 返回注解解析（报告 §2.3 的另一半）：加门后该读数惰性无害，且超出任务书第②条口径，未做。
2. 被门挡下函数在**运行期**仍走 unknown_extern 兜底桩（`i64::MAX`）而非编译错误：需动 `src/ir/codegen/**`（禁区），另单。
3. `docs/嵌入式统一抽象-实现-2026-10-21.md` §六.2 那句「MANIFEST 里 `toolchain_available=no` 就是这个读数」现已过期（本条改的正是它），文档不在本车道范围。
4. T0r49 §四 的三条现场观测（c 形参名带声明处空白、`embed/mod.rs` 头注释陈旧、`embed_unify_r15` 的 `builtins_rlib()` 在 `CARGO_TARGET_DIR` 覆盖下取数路径失效）—— 其中第 3 条本车道实测撞到（隔离树 12 条红在这一行），仍不属本任务范围。

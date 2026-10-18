# lzc-loop-20260927 · 寻虫 R1 发现清单（主编译器）

> 生成时间：2026-09-28 · 环节 R1-寻虫（2/30）
> 复现物：`.fist-loop-20260927/hunt_r1/probes/*.lz`（ASCII/UTF-8 最小复现）+ `mkprobes*.ps1` / `runall.ps1` 批量 harness
> 说明：FIST-Mbt 的 `report_bug` 拒绝 `project_dir=".."`（报「非法 project_dir（拒绝绝对路径/穿越/盘符）」），
> 缺陷无法直接写入项目 `memory/bugs.md`；本循环缺陷以任务库账本 `.fist-loop-20260927/memory/bugs.md`（BUG-2~BUG-8）为准，
> 本文件为项目侧镜像（新增文件，不改存量账本）。该工具限制已记入账本 note，报发起人裁决。

## 一、存量账本复核（memory/bugs.md BUG-1~BUG-6）

结论：**6 单全部已修（代码已落地但未销账）**。实测：`.fist-loop-20260927/hunt_r1/` 下逐个
`lang-zone <x.lz>` → `rustc --edition 2021 -L dependency=target/debug/deps --extern lz_builtins=...` → 运行。

| 账本单 | 复现样例 | 现状 |
| --- | --- | --- |
| BUG-1 裸发 `use num_bigint::BigInt` E0432 | `bigint_min.lz` / `complex_min.lz` | 已修：生成码改用 `lz_builtins::BigInt` 绝对路径 |
| BUG-2 顶层 BigInt const 缺 `BigInt::from` E0308 | `bigint_min.lz` | 已修：`BigInt::from(12345678901234567890i128)` |
| BUG-3 BigInt 常量运算落 const 上下文 E0015 | `bigint_min.lz` | 已修：改 `std::sync::LazyLock<BigInt>` 惰性发射（输出 1234567890123456789000） |
| BUG-4 函数体 bigint 坍塌 i128 | `bigint_fnbody.lz` | 已修：rustc 通过，运行输出 1234567890123456789000 |
| BUG-5 三层嵌套空列表 E0308 | `rv_list3.lz` | 已修：rustc 通过，运行输出 `[[], [[]]]`；四层 `[[], [[]], [[[]]]]` 亦通过 |
| BUG-6 bigint 注解位不一致（struct 字段 / def 返回） | `bigint_annot.lz` | 已修：typer 接受，rustc 通过，运行输出 99999999999999999999999 / 123456789012345678901234567890 |

## 二、新增确诊缺陷（7 单）

| 编号 | 严重性 | 落点 | 一句话 |
| --- | --- | --- | --- |
| BUG-2 | high | `src/semantic_check.rs:1097`（**非 IR 域**） | 未定义函数调用静默通过 → codegen 生成 `fn foo(..) -> i64 { i64::MAX }` 桩，运行得到错误结果 9223372036854775807 |
| BUG-3 | high | `src/ir/codegen/mod.rs` | 超 i128 范围整数字面量发射 `BigInt::from(<无后缀字面量>)` → rustc `integer literal is too large` / `out of range for i32`（含带 `bigint` 注解场景） |
| BUG-4 | high | `src/ir/builder.rs`（+ typer 待定） | bigint 形参函数无法接受整数字面量实参：`IR build error … 期望 bigint，实际 int128` |
| BUG-5 | medium | `src/ir/codegen/mod.rs` | 一元负号 bigint 常量缺 `BigInt::from` 包装 → E0308 expected BigInt found i128 |
| BUG-6 | medium | `src/ir/codegen/mod.rs` | 字符串索引越界静默返回 `'\0'`(0)，与列表索引 panic 语义不一致 |
| BUG-7 | low | `src/ir/builder.rs:12825` | 无条件 `eprintln!("DBG fnarg: ...")` 调试输出污染用户可见 stderr |
| BUG-8 | low | `src/ir/codegen/mod.rs` / `duck_check.rs` | 未知方法调用不被 LZ 层拒绝，漏出 rustc E0599（暴露 Rust 内部结构） |

按红线：`src/ir/**` 落点 6 单（BUG-3~BUG-8）**只入账避让，不在循环内修复**，转人工裁定。
BUG-2 落点 `src/semantic_check.rs` 属非 IR 域，纳入 R1-修复 环节闭环。

### 最小复现（逐单）

- BUG-2：`def main() = println(foo())` → 编译+运行成功，输出 `9223372036854775807`（探针 `m19_undefined_fn.lz`）
- BUG-3：`let x = 170141183460469231731687303715884105728`（`m10`）/ 200 位 `9`（`m11`）/ `let x: bigint = <60 位 7>`（`m24`）
- BUG-4：`def addbig(a: bigint, b: bigint) -> bigint = a + b` + `addbig(12345678901234567890, 1)`（`p06`）
- BUG-5：`let x: bigint = -12345678901234567890`（`p03`）
- BUG-6：`let s = "abc"; println(s[10])` 与 `println(s[0 - 1])` 均输出 `0`（`m15` / `m23`）
- BUG-7：同 BUG-4 复现，stderr 首行即 `DBG fnarg: fn=addbig idx=1 want=Named { path: "bigint", ... } at=Int128 ...`
- BUG-8：`let s = "abc"; println(s.nosuch())`（`m21`）

## 二·补 R1-验证 环节入账（2026-09-28，任务库账本 BUG-9~BUG-12）

验证环节四门禁实跑。**结果：1 绿 3 红，且失败均非本循环引入**（本循环仅一个 commit `e371f0c1` 新增测试文件，未触碰 `src/**`）。

| 编号 | 严重性 | 归属 | 一句话 |
| --- | --- | --- | --- |
| BUG-9 | high | ir-upstream | 门禁1：`tests/str_boundary.rs` 2/3 红 —— `E0277 String: Pattern`（5 处）+ `E0308`（1 处）；全量 733 passed / 2 failed（基线 748/0） |
| BUG-10 | medium | 待人工裁定 | 门禁4「测试数只增不减」破：748 → 735；源头为工作区未提交的并发改动（`tests/` 7 文件 M + `tests/str_boundary.rs` 未跟踪新增） |
| BUG-11 | high | ir-upstream | 门禁2：DEMO 回归 312 例中 25 例 rustc 失败 + 1 例 IR build 失败；含 4 个 2026-09-27 已转绿 DEMO 复现（`Box<dyn Fn>` clone / E0308） |
| BUG-12 | medium | ir-upstream | `DEMO/99_spec/guard_for_3.lz` 生成程序挂起（20s 超时未退出），无超时的回归会整体挂死 |

门禁3 golden 对账**绿**：`dual_route_golden` 4/0、`incremental_golden` 1/0、`ir_snapshots` 8/0、`moddec_corpus` 2/0、
`moddec_product_assert` 5/0，受管 golden 目录（`DEMO/99_spec/moddec/golden/`）git 洁净无漂移。

进度报告：`memory/reviews/20260928.06.40.00.md`（PROJECT-SPEC/04 口径，只增不删）。

## 二·补2 R1-打磨 环节结论（2026-09-29）

### BUG-2 结论修正 —— 由「缺陷」升级为「语义边界修订请求」（任务库账本 BUG-13）

- 重放补丁后全量门禁由 **734/1 退化为 730/8**，新增 7 项失败：`find_bug_libs::{lib_hashmap,lib_string}`、
  `lz_ir_bootstrap::{lz_emit_ir_lz_matches_ir_byte_exact, lz_emit_ir_lz_roundtrip, lz_emit_rs_lz_matches_native_codegen}`、
  `mod::inline_error_boundaries_are_rejected`、`reject_more::inline_error_boundaries_are_rejected`。
- **根因**：`tests/reject_more.rs:97 ACCEPTED_CASES` 第 117 行 `unknown_keyword`
  （`def main() =\n    frobnicate(1)\n`，phase 注释「宽松：未知标识符按函数调用通过，交 rustc 阶段」）
  **显式锁定**该宽松语义，并要求「若未来收紧为拒绝，本表须同步迁移至 REJECT_CASES」。
  ⇒ 属行为级契约（语义边界）修订，按红线「只挂账不擅自改语义」+ 打磨卡「不做语义变更」，**循环内不得擅自收敛**。
- 若人工裁定收紧，需成套改动：① `src/semantic_check.rs` `Expr::Call` 的 Ident callee 加 `!self.is_bound(name)` 报错；
  ② `builtin_value_names()` 补白名单（DEMO+corpus 312 例与全量测试实测共需）：`println / format / contains / push / sort /
  reverse / hash / Exception / __as__ / ord / chr_unchecked / print_str / NegError`；③ `unknown_keyword` 由
  `ACCEPTED_CASES` 迁至 `REJECT_CASES`；④ 复核 `lz_ir_bootstrap` 三个 LZ 自举用例的跨模块调用。
- 已回滚，回滚后复测回到 **734/1**，无残留。补丁留档 `.fist-loop-20260927/hunt_r1/bug2_semantic_check.patch.md`。

### 质量指标（2026-09-29 实测）

| 指标 | R1-验证 | R1-打磨 | 结论 |
| --- | --- | --- | --- |
| `cargo test` 全量 | 733 p / 2 f | **734 p / 1 f** | 外部改善 1 例，仍未全绿（余 `str_boundary_args_and_returns`，IR 域） |
| `cargo check --workspace` 告警 | — | **0 warning / 0 error** | 达标 |
| golden 一致性 | 全绿 | 全绿（五目标 + 受管 golden 目录 git 洁净） | 达标 |
| DEMO 回归 | 312 → ok285 / rc25 / lz1 / to1 | 未复跑（打磨不改码，沿用验证结论） | 红（IR 域，BUG-11/12） |

## 三、未确诊观察区（本次不入账）

1. **除零/取模零**：`let a = 10; let b = 0; a / b`（`m12`）与 `a % b`（`m13`）触发 rustc `this operation will panic at runtime` 硬错误 ——
   生成码无法编译（rustc 的 `unconditional_panic` deny lint）。仅当两操作数都是字面量常量时触发；运行期变量为零则走 Rust panic。
   是否应改为 LZ 层 ZeroDivisionError 属语义设计问题，需发起人裁定。
2. **深嵌套列表字面量**：200 层 `[[[...1...]]]`（`m17`）触发 rustc `recursion limit reached while expanding vec!`。
   可在产物顶部加 `#![recursion_limit = "256"]` 缓解，属鲁棒性而非正确性。
3. **`str.len()` 返回字节数**：`"中文字符串 hello"` 的 `len()` = 21（字节）而非 11（字符）——与 Rust `str::len` 一致，
   但与 Python 语义不同，是否调整属语言设计裁定，暂不记账。
4. **畸形输入健壮性良好**：`{`、未终止字符串、孤立 `let`/`def`/`+`、空右值、缩进错乱、未闭合列表/形参（`m01`~`m09`）
   均给出 LZ 层 Parse error，**无 panic、无崩溃**；500 层嵌套括号（`m18`）、空文件（`p10`）、UTF-8 中文字符串（`p11`）均正确。

## BUG-1 [2026-09-27T14:57:07Z] [low] OPEN
- summary: [lzc-loop 落点试例] Step0 落点口径测定（测完即留档于任务库本地账本，不污染项目账本）
- detail: server cwd = .fist-loop-20260927；本条用于确认 project_dir 相对 cwd 的解析落点。

## BUG-2 [2026-09-27T17:58:04Z] [high] OPEN
- summary: [lzc-loop 寻虫R1] src/semantic_check.rs:1097 未定义函数调用静默通过 → codegen 生成 i64::MAX 桩，运行得到错误结果 9223372036854775807
- detail: 复现：`def main() = println(foo())`（探针 .fist-loop-20260927/hunt_r1/probes/m19_undefined_fn.lz）。lang-zone 编译成功、rustc 编译成功、运行打印 9223372036854775807（= i64::MAX）。生成码含 `#[allow(dead_code)] fn foo(__a0: i64) -> i64 { i64::MAX }`（src/ir/codegen/mod.rs:2512-2515 未知外部函数桩）。对照：未绑定**变量** `println(zzz)` 被 src/semantic_check.rs:1039 正确拒绝（Semantic error: 未绑定变量）；未定义**函数**因 check_expr 的 Expr::Call 分支显式跳过 func 位置 Ident 检查（1097 行注释「允许调用尚未显式绑定的函数/宏名」）而漏网。建议落点 src/semantic_check.rs（非 IR 域）：fn_names/imported_names/builtins/enum_variants 全量收集后，对 Expr::Call 的 Ident callee 做 is_bound 判定。严重性 high：静默错误结果远差于编译报错。
- task_id: T0r3

## BUG-3 [2026-09-27T17:58:04Z] [high] OPEN
- summary: [ir-upstream] [lzc-loop 寻虫R1] src/ir/codegen/mod.rs 超 i128 范围整数字面量发射 `BigInt::from(<无后缀字面量>)` → rustc integer literal is too large / out of range for i32
- detail: 复现（3 例）：`let x = 170141183460469231731687303715884105728`（2^127，probes/m10）→ rustc: literal out of range for `i32`；`let x = <200 个 9>`（m11）→ integer literal is too large；`let x: bigint = <60 个 7>`（m24，带 bigint 注解同样失败）→ integer literal is too large。生成码：`static x: LazyLock<lz_builtins::BigInt> = LazyLock::new(|| lz_builtins::BigInt::from(<裸字面量>));` —— 超过 i128 时无可用后缀，字面量无类型后缀 → Rust 整型默认 i32 / 超过 u128 直接非法。括号内为无后缀字面量时 `BigInt::from` 的 From impl 无法推断。建议：超 i128 范围时改走 `BigInt::parse_bytes(b"...", 10).unwrap()` 或 `"...".parse::<BigInt>().unwrap()`。落点 src/ir/codegen/mod.rs（IR 上游）→ 按红线只入账避让，不修。
- task_id: T0r4

## BUG-4 [2026-09-27T17:58:04Z] [medium] OPEN
- summary: [ir-upstream] [lzc-loop 寻虫R1] src/ir/codegen/mod.rs 一元负号 bigint 常量缺 BigInt::from 包装 → E0308 expected BigInt found i128
- detail: 复现：`let x: bigint = -12345678901234567890`（probes/p03_bigint_neg.lz）→ 生成 `static x: LazyLock<lz_builtins::BigInt> = LazyLock::new(|| -12345678901234567890i128);` → rustc E0308 mismatched types expected BigInt found i128（rustc 自带 help: call Into::into）。正号/无符号同位置（p03 对照 bigint_min.lz）已正确包 `lz_builtins::BigInt::from(...)`，说明 Unary Neg 路径未走 BigInt 字面量包装逻辑。落点 src/ir/codegen/mod.rs（IR 上游）→ 只入账避让。
- task_id: T0r5

## BUG-5 [2026-09-27T17:58:04Z] [high] OPEN
- summary: [ir-upstream] [lzc-loop 寻虫R1] bigint 形参函数无法接受整数字面量实参：IR build error 调用 addbig 的参数 1 类型不匹配：期望 bigint，实际 int128
- detail: 复现（probes/p06_bigint_fnparam.lz）：`def addbig(a: bigint, b: bigint) -> bigint = a + b` + `println(addbig(12345678901234567890, 1))` → IR build error: 调用 addbig 的参数 1 类型不匹配：期望 bigint，实际 int128（位于函数 main）。整数字面量一律推 Int128，bigint 形参位不做 int→bigint 隐式提升，导致 bigint 形参函数**完全无法调用**。疑似落点 src/ir/builder.rs:12800-12830（实参类型校验 want/at 分支，同处还有无条件 DBG 输出）。IR 上游 → 只入账避让。
- task_id: T0r6

## BUG-6 [2026-09-27T17:58:04Z] [medium] OPEN
- summary: [ir-upstream] [lzc-loop 寻虫R1] 字符串索引越界静默返回 '\0'(0)，与列表索引 panic 语义不一致
- detail: 复现（probes/m15_str_oob.lz / m23_str_idx_neg.lz）：`let s = "abc"; println(s[10])` 与 `println(s[0 - 1])` 均**正常运行并输出 0**。生成码：`let __cs: Vec<char> = (s).chars().collect(); let __i = ((10i64) as usize); if __i >= __cs.len() { '\0' as i64 } else { __cs[__i] as i64 }`。对照列表索引 `xs[10]`（m14）生成裸 `xs[(10i64) as usize]` → 运行时 panic index out of bounds。两者语义不一致，且字符串侧为**静默错误值**（越界读不报错），应统一为 LZ 层 IndexError/Option。落点 src/ir/codegen/mod.rs（IR 上游）→ 只入账避让。
- task_id: T0r7

## BUG-7 [2026-09-27T17:58:04Z] [low] OPEN
- summary: [ir-upstream] [lzc-loop 寻虫R1] src/ir/builder.rs:12825 无条件 eprintln!("DBG fnarg: ...") 调试输出污染用户可见 stderr
- detail: 复现：probes/p06_bigint_fnparam.lz 编译时 stderr 首行为 `DBG fnarg: fn=addbig idx=1 want=Named { path: "bigint", args: [] } at=Int128 gens=[] duck={}`，出现在正式错误信息之前。与本项目其余调试输出不同（src/ir/codegen/mod.rs 各 DBG 均以 std::env::var("LZ_DBG_*") 门控、src/comptime/mod.rs:653 的 DBG comptime for 亦未门控同族问题），该行无 env 门控，属遗留调试语句。落点 src/ir/builder.rs（IR 上游）→ 只入账避让。
- task_id: T0r8

## BUG-8 [2026-09-27T17:58:04Z] [low] OPEN
- summary: [ir-upstream] [lzc-loop 寻虫R1] 未知方法调用不被 LZ 层拒绝，漏出 rustc E0599（编译错误暴露 Rust 内部结构）
- detail: 复现（probes/m21_bad_method.lz）：`let s = "abc"; println(s.nosuch())` → lang-zone 编译**成功**，rustc 报 `error[E0599]: no method named nosuch found for struct String in the current scope`。对照未绑定变量/参数个数不匹配均有 LZ 层 Semantic error（src/semantic_check.rs）。用户侧应得到 LZ 语义错误而非 Rust 结构体名。落点涉及方法解析（src/ir/codegen/mod.rs / duck_check.rs，IR 上游）与可选的 src/semantic_check.rs 前置校验 → 只入账避让。
- task_id: T0r9

## BUG-9 [2026-09-27T22:40:07Z] [high] OPEN
- summary: [ir-upstream] [lzc-loop 验证R1] tests/str_boundary.rs 2/3 用例红：E0277 `String: Pattern` + E0308，门禁1 基线 748/0 → 733/2
- detail: 复现：`cargo test --workspace --no-fail-fast -j 2`（2026-09-28）→ `Running tests/str_boundary.rs`：str_boundary_args_and_returns FAILED、str_boundary_ops_and_formatting FAILED、str_boundary_bindings_and_containers ok。报错为 rustc `error[E0277]: the trait bound String: Pattern is not satisfied`（5 处）与 `error[E0308]: mismatched types`（1 处）。该文件在 git 中为**未跟踪新文件**（?? tests/str_boundary.rs），属工作区并发进行中改动，本循环未触碰。对照记忆 20622012：str 边界层（src/ir/codegen/str_boundary.rs 契约 + StrExt 只读形参族）此前为 3/0 全绿，E0277 `String: Pattern` 正是当时修掉的「pattern_methods 不生效致字面量被拥有化」同型问题。落点 src/ir/codegen/**（IR 上游）→ 按红线只入账避让，不在循环内修复，转人工裁定。raw: `error: test failed, to rerun pass -p lang-zone --test str_boundary`。
- task_id: T0r11

## BUG-10 [2026-09-27T22:40:07Z] [medium] OPEN
- summary: [lzc-loop 验证R1] 门禁4「测试数只增不减」破：基线 748 → 现 735（733 passed + 2 failed），源头为工作区未提交的并发改动
- detail: 实测：`cargo test --workspace --no-fail-fast -j 2` 共 35 个测试目标，sum passed=733 failed=2（合计 735），低于本循环冷启动基线 748。证据：本循环自 2026-09-27 22:50 起**仅一个 commit** e371f0c1（新增 tests/lzc_loop_nested_def.rs 4 用例，本次实跑 4 passed 在册）；而 `git status --porcelain -- tests/` 显示 tests/bridge_embed.rs、find_bug_bugs.rs、g6_codegen.rs、gen_build_block.rs、incremental_golden.rs、ir_pass_fixes.rs、moddec_product_assert.rs 共 7 个文件被改（M）且 tests/str_boundary.rs 为未跟踪新文件（??），均为外部并发会话的进行中改动。差异量级示例：find_bug_bugs 40（历史 47）、demo_codegen_compile 1（历史 2）。按红线「遗留集/他人进行中改动零触碰」，本循环不还原不修改，报发起人裁决：需明确门禁基线以哪个工作树快照为准。
- task_id: T0r12

## BUG-11 [2026-09-27T22:55:37Z] [high] OPEN
- summary: [ir-upstream] [lzc-loop 验证R1] DEMO 回归红：312 例中 25 例 rustc 失败 + 1 例 IR build 失败；含 4 个 2026-09-27 已转绿的 DEMO 复现（Box<dyn Fn> clone / E0308）
- detail: 复现：`target/debug/lang-zone.exe`（2026-09-28 05:48 现场重建）逐个 `.lz` → rustc --edition 2021 -L dependency=target/debug/deps --extern lz_builtins=...，排除 99_errors 与 lz_std。结果 total=312 / ok=285 / lzFail=1 / rustcFail=25 / runFail=0 / timeout=1（脚本 .fist-loop-20260927/hunt_r1/demo_regression2.ps1，输出 _demo_regression_r1_v3.txt）。\n重点回归项（对照记忆 28056391 / 79627747 记录的「已转绿并移出 KNOWN_RUSTC_FAILURES」）：01_basics/polish_07_closures.lz、01_basics/polish_27_use.lz、01_basics/polish_29_combined.lz → `error[E0599]: the method clone exists for struct Box<dyn Fn(i64) -> i64>, but its trait bounds were not satisfied`（应为 Rc<dyn Fn>）；99_spec/iterator_demo.lz、boundary-coverage/combo-iterator-generator.lz、boundary-coverage/nesting-closure-lambda.lz、boundary-coverage/combo-pipe-lambda.lz → E0308。其余：03_variables/walrus*.lz、99_spec/while_walrus_guard_1.lz、combo-syntax/combo_while_walrus.lz、boundary-coverage/edge-walrus-operator.lz、99_spec/combo-syntax/combo_while_guard_else.lz / combo_while_guard_walrus.lz → E0070 invalid left-hand side of assignment；04_functions/closures_more.lz → E0382；07_data_structures/magic_methods.lz → E0599 Config::new；05_expressions/operators.lz、pipe_semantics.lz、04_functions/spread_protocol.lz、09_macros/comptime_external_lib.lz、01_basics/polish_02_scope.lz（E0384）、polish_30_edge.lz、boundary-coverage/combo-defer-guard.lz、combo-error-control.lz、edge-values-boundary.lz。\nlzFail：99_spec/combo-syntax/combo_while_guard_try.lz → `IR build error: Result 值未经解包（unwrap/?）直接参与运算`。\n本循环未触碰 src/**，失败系工作区进行中改动所致。落点 src/ir/codegen/** 与 src/ir/builder.rs（IR 上游）→ 只入账避让，转人工裁定。
- task_id: T0r13

## BUG-12 [2026-09-27T22:55:37Z] [medium] OPEN
- summary: [ir-upstream] [lzc-loop 验证R1] DEMO/99_spec/guard_for_3.lz 生成程序挂起（20s 超时未退出），DEMO 回归被阻塞
- detail: 复现：`.fist-loop-20260927/hunt_r1/demo_regression2.ps1` 全量回归时，进程 guard_for_3_99_spec.exe（PID 44076，由 DEMO/99_spec/guard_for_3.lz 生成）启动后 20s+ 不退出，需外部 kill 才能继续；v3 复跑同样命中 1 例 timeout。未做进一步归零（是否等待 stdin 抑或死循环待定）。影响：无超时的回归脚本会整体挂死。建议：①查明 guard_for_3 的生成码；②回归 harness 必须带 per-case 超时。落点待定（疑似 src/ir/codegen 生成码，IR 上游）→ 只入账避让。
- task_id: T0r14

## BUG-13 [2026-09-28T17:12:29Z] [medium] OPEN
- summary: [lzc-loop 打磨R1] BUG-2（未定义函数静默 i64::MAX 桩）实为**已锁定的宽松语义边界**，非缺陷修复：tests/reject_more.rs:117 `unknown_keyword` 显式锁定「未知标识符按函数调用通过，交 rustc 阶段」
- detail: 结论修正（打磨 R1 实测，补丁已回滚）：R1-寻虫 将「未定义函数调用静默通过并生成 `fn NAME(__a0: i64) -> i64 { i64::MAX }` 桩」判为缺陷 BUG-2 并给出 `src/semantic_check.rs` 最小修。打磨环节重放该补丁后全量门禁由 734/1 退化为 **730/8**，新增 7 项失败：`find_bug_libs::{lib_hashmap,lib_string}`、`lz_ir_bootstrap::{lz_emit_ir_lz_matches_ir_byte_exact,lz_emit_ir_lz_roundtrip,lz_emit_rs_lz_matches_native_codegen}`、`mod::inline_error_boundaries_are_rejected`、`reject_more::inline_error_boundaries_are_rejected`。根因：`tests/reject_more.rs:97 ACCEPTED_CASES` 第 117 行 `unknown_keyword`（source `def main() =\n    frobnicate(1)\n`，phase 注释「宽松：未知标识符按函数调用通过，交 rustc 阶段」）**显式锁定该宽松语义**，注释并要求「若未来收紧为拒绝，本表须同步迁移至 REJECT_CASES」。故这属于**行为级契约（语义边界）修订**，按红线「行为级契约问题只挂账不擅自改语义」与打磨卡「不做语义变更」，循环内不得擅自收敛。\n若人工裁定要收紧，需成套改动：① `src/semantic_check.rs` 的 `Expr::Call` Ident callee 加 `!self.is_bound(name)` 报错；② `builtin_value_names()` 补白名单（312 个 DEMO+corpus 实测 + 全量测试实测共需）：println、format、contains、push、sort、reverse、hash、Exception、__as__、ord、chr_unchecked、print_str、NegError；③ 将 `unknown_keyword` 从 `ACCEPTED_CASES` 迁至 `REJECT_CASES` 并更新注释；④ 复核 lz_ir_bootstrap 三个 LZ 自举用例（跨模块调用是否被误判）。\n补丁与验证结论留档 `.fist-loop-20260927/hunt_r1/bug2_semantic_check.patch.md`；回滚后门禁已复测回到 734/1，无残留。
- task_id: T0r16

## BUG-14 [2026-09-28T17:17:03Z] [high] OPEN
- summary: [ir-upstream] [lzc-loop 推进R1] BUG-1 复发：生成码再次裸发 `use num_bigint::BigInt;` / `use num_complex::Complex64;`，孤立 crate 编译 E0432/E0433
- detail: 复现（2026-09-29，编译器 09:2x 现场重建）：`let x: bigint = 12345678901234567890; let y: bigint = x / 7; ...`（探针 .fist-loop-20260927/hunt_r1/probes/p04_bigint_divmod.lz）生成码首部 `use std::collections::{HashMap, HashSet};` 之后直接 `use num_bigint::BigInt;`。以 `rustc --edition 2021 -L dependency=target/debug/deps --extern lz_builtins=...`（孤立 crate、仅 --extern lz_builtins）编译 → `error[E0432]: unresolved import num_bigint` / E0433，rustc help: you might be missing a crate named `num_complex`。\n对照：R1-寻虫（2026-09-28）实测同一批复现样例（bigint_min/complex_min/bigint_fnbody/bigint_annot）**全部 rustc 通过并运行正确**，生成码用的是 `lz_builtins::BigInt` 绝对路径；本轮 09-29 复测退化为裸 import ⇒ **工作区回归**。\n影响：任何使用 bigint/complex 的 .lz 生成的孤立产物都无法编译（DEMO 回归与新增闸门 `tests/lzc_loop_boundary_gates.rs` 中 bigint/complex 用例因此不能入闸，已注释留位待补）。落点 src/ir/codegen/mod.rs（IR 上游）→ 只入账避让，转人工裁定。
- task_id: T0r17

## BUG-15 [2026-09-28T17:17:03Z] [medium] OPEN
- summary: [ir-upstream] [lzc-loop 推进R1] BUG-5 同类点未穷尽：四层嵌套空列表 `[[], [[]], [[[]]]]` 生成 E0308（三层已修）
- detail: 复现：`let a: List<List<List<List<int>>>> = [[], [[]], [[[]]]]`（探针 .fist-loop-20260927/hunt_r1/probes/p09_deep_list4.lz）生成 `std::sync::LazyLock::new(|| vec![Vec::<i64>::new(), vec![Vec::<i64>::new()], vec![vec![Vec::<i64>::new()]]])` → `error[E0308]: expected Vec<Vec<Vec<i64>>>, found Vec<i64>` 与 `expected Vec<Vec<i64>>, found Vec<i64>`（两处）。即 ListLit 期望类型下推仍只下推一层，第二/第三元素槽位未随外层类型递归实例化。\n对照：R1-寻虫（2026-09-28）实测四层用例**rustc 通过且运行输出 `[[], [[]], [[[]]]]`**；本轮 09-29 复测失败 ⇒ 工作区回归（与 BUG-11 同批）。落点 src/ir/codegen/mod.rs（IR 上游）→ 只入账避让，转人工裁定。
- task_id: T0r18

## BUG-16 [2026-09-28T20:34:28Z] [medium] OPEN
- summary: [lzc-loop 寻虫R2] src/lexer：UTF-8 BOM 源文件被拒 —— Parse error: Unexpected token at top level: Unknown(U+FEFF)
- detail: 复现：最小样例 println(1) 以 UTF-8 带 BOM 保存（首三字节 EF BB BF，探针 .fist-loop-20260927/hunt_r1/probes/n16_bom_min.lz），lang-zone 编译该文件报 Parse error: Unexpected token at top level: Unknown(U+FEFF)。同一内容去掉 BOM 即正常（对照 n02/n14）。影响：Windows 记事本等默认 UTF-8 带 BOM 保存，用户直接编译即失败，且错误信息不含 BOM 字样，排查成本高；LZ 自举与 DEMO 若混入 BOM 会静默卡住。预期：读取源文本时剥离 BOM（或词法层跳过 U+FEFF），无 BOM 文件行为不变。建议落点：源文件读取（src/cli.rs / src/project.rs / src/util/source.rs）或 src/lexer/lexer.rs 首字符跳过 U+FEFF —— 均非 IR 域，纳入 R2-修复 环节闭环。
- task_id: T0r22


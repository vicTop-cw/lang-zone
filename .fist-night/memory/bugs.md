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

## BUG-8 [2026-10-23T16:53:51Z] [high] OPEN
- summary: [夜巡R1-寻虫] DEMO/lz_std/{dict,list,set}.lz 泛型 impl 实现 trait 方法时闭包形参被追加 + Clone 约束 → E0276 impl has stricter requirements than trait
- detail: 复现：cargo test --test demo_codegen_compile -- --ignored（全量闸门，非门禁）。327 文件 / 319 通过 / 8 失败，其中 dict.lz(9 err)、list.lz(8 err)、set.lz(4 err) 首错均为 E0276。生成码形态：trait 声明 `fn retain(&mut self, predicate: impl Fn(&T) -> bool + 'static);`，impl 处生成为 `fn retain(&mut self, mut predicate: impl Fn(&T) -> bool + 'static + Clone)` —— 实现比 trait 多 `Clone` 约束。最小复现：lzc DEMO/lz_std/list.lz 后 rustc 产物。根因方向：codegen 为『闭包可复制』统一给 `impl Fn(..)` 追加 + Clone，未区分『trait 方法实现』与『独立函数/方法』，前者必须与 trait 签名逐字一致。修复须同步评估 golden 快照是否需再对齐。证据：.fist-night/_work/DEMO_lz_std_list.rustc.log 等。
- reported_by: night-executor
- task_id: T0r13

## BUG-9 [2026-10-23T16:53:51Z] [high] OPEN
- summary: [夜巡R1-寻虫] DEMO/07_data_structures/self_recursive.lz 自递归结构体 Option<Box<T>> 字段在返回元组处未剥 Box → E0308
- detail: 复现：lzc DEMO/07_data_structures/self_recursive.lz → rustc。产物 32 行 `return (self.value.clone(), self.next.clone());` 报 expected `Option<ListNode<T>>`, found `Option<Box<ListNode<T>>>`；76 行 TreeNode left/right 同族（共 3 处）。根因方向：自递归字段声明为 `Option<Box<Node<T>>>`（打破无限大小），但读取/克隆发射点按『字段声明类型即值类型』处理，缺 `*self.next.clone()` 之类的解箱。证据：.fist-night/_work/DEMO_07_data_structures_self_recursive.rustc.log。
- reported_by: night-executor
- task_id: T0r14

## BUG-10 [2026-10-23T16:53:51Z] [medium] OPEN
- summary: [夜巡R1-寻虫] DEMO/lz_std/traits.lz 泛型字段 Option<J> 调 .clone() 缺 Clone 约束传播 → E0599
- detail: 复现：lzc DEMO/lz_std/traits.lz → rustc。产物 117 行 `return (self.outer.clone(), self.func.clone(), self.inner.clone());` 报 `J: std::clone::Clone` which is required by `Option<J>: std::clone::Clone` 未满足；148 行 `Option<<I as Iterator>::Item>` 同族（共 2 处）。根因方向：结构体泛型参数 J/I 未把 Clone 约束带进生成的自动访问/元组返回路径（或应在 struct 声明处补 J: Clone）。证据：.fist-night/_work/DEMO_lz_std_traits.rustc.log（单独复现留存）。
- reported_by: night-executor
- task_id: T0r15

## BUG-11 [2026-10-23T16:53:51Z] [medium] OPEN
- summary: [夜巡R1-寻虫] DEMO/boundary-coverage/nesting-closure-lambda.lz 嵌套闭包返回闭包时内层闭包缺 move → E0373 + lifetime
- detail: 复现：lzc DEMO/boundary-coverage/nesting-closure-lambda.lz → rustc。产物 95-96 行 `return Arc::new(move |a: i64| { Arc::new(|b: i64| { .. a .. }) })`：外层已 move，内层借用外层变量 a，报 `closure may outlive the current function, but it borrows a`(E0373) 与 `lifetime may not live long enough`。根因方向：闭包返回闭包场景未对内层闭包加 move（外层变量按引用捕获即逃逸）。证据：.fist-night/_work/DEMO_boundary-coverage_nesting-closure-lambda.rustc.log。
- reported_by: night-executor
- task_id: T0r16

## BUG-12 [2026-10-23T16:53:51Z] [medium] OPEN
- summary: [夜巡R1-寻虫] DEMO/10_error_handling/panic_raise_try.lz 调用点引用层级不匹配 → E0308 expected String found &String
- detail: 复现：lzc DEMO/10_error_handling/panic_raise_try.lz → rustc。产物 255 行 `let content: String = (read_file(&path))?;` 报 arguments to this function are incorrect: expected `String`, found `&String`（被调签名生成为 `fn read_file(p: String)`，调用点却发射 `&path`）。根因方向：形参/实参的借用位（borrow bit）在函数声明与调用点两处判定不一致，try/? 包装路径尤甚。证据：.fist-night/_work/DEMO_10_error_handling_panic_raise_try.rustc.log。
- reported_by: night-executor
- task_id: T0r17

## BUG-13 [2026-10-23T16:53:51Z] [medium] OPEN
- summary: [夜巡R1-寻虫] DEMO/lz_std/result.lz or_else 闭包返回类型未传导外层标注 → E0308 Result<i64,_> vs Result<i64,String>
- detail: 复现：lzc DEMO/lz_std/result.lz → rustc。产物 413 行 `let c: Result<i64, String> = a.or_else(|e: String| -> Result<i64, String> { Ok((e.len() as i64)) });` 报 expected `Result<i64, String>`, found `Result<i64, _>`。根因方向：闭包体 `Ok(..)` 的错误型变体未从标注/被调泛型回推，生成了未定型的 `_`（或 Ok 包装缺类型标注 `Ok::<_, String>`）。证据：.fist-night/_work/DEMO_lz_std_result.rustc.log。
- reported_by: night-executor
- task_id: T0r18

## BUG-14 [2026-10-23T16:53:51Z] [medium] OPEN
- summary: [夜巡R1-寻虫] FIND_BUG/hunt-20261023/nb04 溢出字面量无标注时降为 i128 而非 BigInt，与 int 混算生成 i128 + i64 → E0308/E0277
- detail: 最小复现 FIND_BUG/hunt-20261023/nb04_bigint_int_mixed_arith.lz：`let x = 100000000000000000000000000000` + `let y: int = 3` + `println(x + y)`。产物 `return println!("{:?}", x + 3i64);` 报 expected `i128`, found `i64` 与 cannot add `i64` to `i128`。对照：显式标注 `let a: bigint = 10; let b: int = 3; println(a + b)`（nb05）正确输出 13 —— 说明『自动溢出提升』落到 i128、『显式 : bigint』落到 BigInt，两条路径不一致，且 i128 与 i64 混算未插入同型转换。期望：要么统一提升到 bigint，要么在混算点插入 `as i128` 同型转换。证据：.fist-night/_work/nb04_bigint_int_mixed_arith.rustc.log。
- reported_by: night-executor
- task_id: T0r19

## BUG-15 [2026-10-23T19:36:06Z] [medium] CLOSED
- summary: [夜巡R1-修复] DEMO/lz_std/dict.lz 迭代解构与 BTreeMap 迭代项类型不符 → E0308（3 处）
- detail: 复现（隔离副本实跑）：lzc DEMO/lz_std/dict.lz → rustc。BUG-8 的 5×E0276 消除后暴露的次生缺陷：产物 214/256 行报 expected this type parameter（impl<K: Clone+Debug+Ord, V: Clone+Debug> DictExt<K,V> for BTreeMap<K,V> 的方法返回类型与泛型参数不匹配）；517 行 `for (k, v) in (d).keys().cloned()` 报 this is an iterator with items of type `String`，无法按 (k,v) 元组解构。根因方向：dict 迭代（keys()/values()/items()）发射未区分『键迭代』与『键值对迭代』，`.keys().cloned()` 被当作键值对迭代解构；方法返回类型未跟随 impl 泛型参数。证据：E:/IDEProjects/AI/_lz_night_scratch/_work/dict.rustc.log。
- resolved: 2026-10-24 根因=BUG-349 将 Dict/HashMap 所有 for 迭代一律生成 `.keys().cloned()`（Python 键语义），覆盖了 `for (k, v) in dict` 键值对迭代意图。修复=stmt_gen.rs iter_is_dict 增加 `!var.contains(',')`：单变量 `for k in dict` 仍按键迭代（.keys().cloned()，保留 BUG-349 语义），元组模式 `for (k, v) in dict` 走 .iter()/into_iter() 产出 (K,V)。验证=cargo test --test demo_codegen_compile -- --ignored 全量 327 demo PASS（dict.lz 转绿）。
- reported_by: night-executor
- task_id: T0r20

## BUG-16 [2026-10-23T19:36:06Z] [medium] CLOSED
- summary: [夜巡R1-修复] DEMO/lz_std/list.lz 链式调用后原值被移动 → E0382 use of moved value `lst`
- detail: 复现（隔离副本实跑）：lzc DEMO/lz_std/list.lz → rustc。BUG-8 的 6×E0276 消除后暴露的次生缺陷：产物 518 行 `let mut lst: Vec<i64> = vec![...]`；520 行继续使用 lst 报 use of moved value `lst`（move occurs because `Vec<i64>` does not implement `Copy`）。根因方向：链式方法（sort/reverse/链式 HOF）按值接收 self 后未在使用点插入 `.clone()`，LZ 值语义与 Rust 移动语义在『同一变量链式调用后再次使用』场景未对齐。证据：E:/IDEProjects/AI/_lz_night_scratch/_work/list.rustc.log。
- resolved: 2026-10-24 根因=expr_gen.rs 裸 Vec 的 fold/reduce 迭代器化分支（BUG-359 系）对 receiver 直接 `.into_iter()` 消费，未走 clone_if_multiuse 值语义保护。修复=reduce(单参→functional::reduce)/reduce(双参→into_iter().fold)/fold(双参→into_iter().fold) 三处 receiver 改 `clone_if_multiuse(recv, &receiver)`：函数内多使用点克隆（cnt==1 时原样显式消费，不破坏既有语义）。验证=DEMO 全量 327 PASS（list.lz 转绿）+ FIND_BUG 12/12 零回归。
- reported_by: night-executor
- task_id: T0r21


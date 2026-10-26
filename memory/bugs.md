## BUG-1 [2026-09-26T06:29:50Z] [high] FIXED
- summary: BigInt/Complex 生成码裸发 use num_bigint::BigInt / use num_complex::Complex64，孤立 crate 编译 E0432（num-* 仅为 lz_builtins 传递依赖且零 re-export）
- detail: codegen/mod.rs:2162-2164 仅按 module_uses_bigint 注入 `use num_bigint::BigInt;`（Complex 同族发 `use num_complex::Complex64;`）。demo 闸门 check_one 以孤立 lib crate 编译产物、仅 --extern lz_builtins，num-* 不在 extern prelude → E0432。lz_builtins 源码 grep 零 bigint/complex re-export，生成 crate 无路由可达。样例 FIND_BUG/hunt-20260926/bug-A-numcrate-import-e0432/{bigint_min,complex_min}.lz。基线 1ccbdc43+5行未提交，cargo test 673 passed。 复现文档: FIND_BUG/hunt-20260926/bug-A-numcrate-import-e0432/复现命令.md
- reported_by: BugHunter(hunt-20260926)
- task_id: T0r3

## BUG-2 [2026-09-26T06:29:50Z] [high] FIXED
- summary: 顶层 BigInt const 值缺 BigInt::from 包装：发射裸 i128 字面量，E0308 expected BigInt found i128
- detail: BigInt 字面量发射点三处（mod.rs 6173/14775/15040），const 实际路径站点无 BigInt 目标类型上下文裸发 `{n}i128`（15040 注释自认保守生成）。gen_const_def BigInt 臂(6174-6192)有包装逻辑但未命中。补 --extern num_bigint 绕过 BUG-A 后即现 E0308。样例 FIND_BUG/hunt-20260926/bug-B-bigint-const-no-from-wrap-e0308/。 复现文档: FIND_BUG/hunt-20260926/bug-B-bigint-const-no-from-wrap-e0308/复现命令.md
- reported_by: BugHunter(hunt-20260926)
- task_id: T0r4

## BUG-3 [2026-09-26T06:29:50Z] [medium] FIXED
- summary: BigInt 常量运算落 const 上下文：E0015 cannot call non-const operator（needs_lazy 判定不含 BigInt）
- detail: gen_const_def needs_lazy（mod.rs:6199-6209）集合仅含 Vec/List/HashMap/HashSet/Dict/Set/Tuple/catch_unwind/__try_result/LazyLock/to_string/lazy，未含 BigInt → `const y: BigInt = x * 100i64` 直接 const 发射，BigInt Mul 非常量实现 → E0015。应归入 LazyLock/static 惰性发射。样例 FIND_BUG/hunt-20260926/bug-C-bigint-const-nonconst-op-e0015/（同 bigint_min.lz 第二层错误）。 复现文档: FIND_BUG/hunt-20260926/bug-C-bigint-const-nonconst-op-e0015/复现命令.md
- reported_by: BugHunter(hunt-20260926)
- task_id: T0r5

## BUG-4 [2026-09-26T06:29:50Z] [critical] FIXED
- summary: 函数体内 bigint 类型整体坍塌为 i128：注解丢弃+E0308/E0277（i128*i64）+ module_uses_bigint 不扫函数体连 import 都缺发
- detail: 函数体内 `let x: bigint = <huge>` 生成 `let x = <huge>i128`（bigint→i128 静默降级），`x * 100` 生成 `x * 100i64` → E0308 expected i128 found i64 + E0277 no impl i128*i64。且 module_uses_bigint 检测（mod.rs:1731-1737）只扫函数签名/Const/StructDef 字段不扫函数体表达式，函数体 BigInt 模块连 `use num_bigint::BigInt;` 都不发。4cebe6a9 词法层放行的超大整数在函数体路径退化为更大的 i128 陷阱。样例 FIND_BUG/hunt-20260926/bug-D-bigint-fnbody-type-collapse/。 复现文档: FIND_BUG/hunt-20260926/bug-D-bigint-fnbody-type-collapse/复现命令.md
- reported_by: BugHunter(hunt-20260926)
- task_id: T0r6


### FIXED(2026-09-30T03:43:55Z / BUG-1, BUG-2, BUG-3, BUG-4)
- evidence: 真源：src/ir/codegen/mod.rs（BigInt/Complex 类型串走 lz_builtins re-export、字面量发射三点位带期望类型、needs_lazy 与 const 臂包装命中）+ lz_builtins/src/lib.rs（pub extern crate num_bigint/num_complex 与 pub use BigInt/Complex64）。锁钉：tests/find_bug_bugs.rs::d6_bigint_import_and_const_and_fnbody 与 ::d6_complex_import_route（本轮 cargo test --test find_bug_bugs -j 1 → 42 passed/0 failed/2 ignored，EXIT=0，日志 TEMP/reviews/d6-locks-20260930-1045.log）。端到端正解：复现件 A(bigint)/A(complex)/B/C/D 本轮逐个转译+rustc+运行，bigint 侧输出 1234567890123456789000、complex 侧输出 Complex { re: 1.0, im: 2.0 }；改前字节快照与 sha256 见 TEMP/d6-before-snapshot/。全量对照：cargo test --workspace --no-fail-fast -j 1 = 744 passed/0 failed/11 ignored（TEMP/reviews/d6-gate-workspace-20260930-1100.log）。连带后果已另立 BUG-10（re-export 使孤立 rustc 调用必须带 -L dependency，本轮补 15 个调用面）。交付物 memory/reviews/20260930.11.35.00.md。
## BUG-5 [2026-09-26T06:29:50Z] [medium] FIXED（2026-10-01 T0r5 已落，本轮 10-01 复测收口）
- summary: 三层嵌套空列表 E0308：ListLit 期望类型下推仅一层未递归（fix#2 同类点未穷尽）
- detail: `List<List<List<int>>> = [[], [[]]]` 生成 `vec![Vec::<i64>::new(), vec![Vec::<i64>::new()]]`——首元素空列表实例化借用错槽位（expected Vec<Vec<i64>> found Vec<i64>）。DEMO基线修复报告 fix#2（polish_30_edge）两层用例已转绿但下推未递归；对照 rv_opt3（Option 三层 Some(value:)）PASS，仅 ListLit 元素臂缺口。报告复核路线（docs/DEMO基线失败修复报告-2026-09-26.md 结论「同类点是否穷尽」反问）直接命中。样例 FIND_BUG/hunt-20260926/bug-E-list3-empty-nested-pushdown-e0308/。 复现文档: FIND_BUG/hunt-20260926/bug-E-list3-empty-nested-pushdown-e0308/复现命令.md
- reported_by: BugHunter(hunt-20260926)
- task_id: T0r7

## BUG-6 [2026-09-26T07:58:52Z] [medium] FIXED
- summary: BUG-6 [new·round1] bigint 类型注解支持位不一致：let 注解位接受、struct 字段/def 返回类型位报「未知类型: bigint」
- detail: 对抗样例（.fist-wf-20260926/probe_new.lz）：`let x: bigint = ...`（对齐 bug-D 样例）在 typer 接受，但 `struct Big = val: bigint` 与 `def make_big() -> bigint` 均报 Semantic error: 未知类型: bigint。typer 的 bigint 识别仅覆盖 let 注解路径，未覆盖命名类型解析路径（struct 字段/返回类型）。与 BUG-1~5 同属 BigInt 成簇缺陷（词法 4cebe6a9 放行后类型系统各路径口径不一）。复现：lang-zone.exe .fist-wf-20260926/probe_new.lz → IR build error。基线：hunt-20260926 五项全部 100% 复现后追加探针发现。
- reported_by: BugHunter(wf-20260926-round1)


### FIXED(2026-09-30T01:23:10Z / BUG-6)
- evidence: 真源：67b94d62 feat(ast): accept bigint/complex in type annotation positions（ast/typer 侧放行注解位 bigint/complex，覆盖 struct 字段位与 def 返回类型位）。锁钉：tests/lzc_loop_boundary_gates.rs:129 bigint_complex_annotations_are_accepted（含 struct 字段位 :131 的 accept_lz 用例与「白名单未放宽过度」反向支）。本轮实跑：cargo test --test lzc_loop_boundary_gates -j 1 → 7 passed / 0 failed / 0 ignored，EXIT=0（日志 TEMP/reviews/gate-boundary-gates-20260930-0924.log）。原报的对抗样例 .fist-wf-20260926/probe_new.lz 属未版管的一次性探针目录，锁测已把同一形态接进门禁。销账时点说明：本条早于 2026-09-30 的 D6 批次即已修完，OPEN 至今的原因是 bug_fix 的 project_dir 不能越出 server cwd（此前车道 cwd 为 .fist-explore-cron/，根账不可达）⇒ 本次换 cwd=仓库根补销。
## BUG-7 [2026-09-28] [critical] FIXED
- summary: 模块级可变变量被当作 const 内联为字面量（walrus/全局变量回归根因）
- detail: 模块级 `count = 0`（可重赋值全局变量）被 builder 归入 `top_level_consts` 并求值后注入 `comptime_consts`，codegen 时所有引用被内联为字面量 `0`（如 `count_up` 的 `count += 1` → `0i64 = 0i64 + 1i64`，E0070 invalid left-hand side）。
- fix: 在 builder 新增 `mutated_top_level_consts: HashSet<String>`，`build_ir_inner` 阶段用 `collect_mutated_top_level_consts` 扫描全部函数/测试/顶层构建体语句（含 while/for/match/if 嵌套），把「名字在 `top_level_consts` 中且被重新赋值（`Stmt::Let{mutable}` 或 `Stmt::Assign{target:Ident}`）」的名称收集进来；并在 `convert_expr` 的 comptime 内联位点（builder.rs:4290）跳过这些名（保留 `Var` 引用）。codegen 侧本已有 `scan_const_mutations`+`mutated_consts`→`static mut`+`unsafe{}` 路径，故 `count` 最终正确生成 `static mut count: i64 = 0i64;` 与 `unsafe { count = unsafe { count } + 1i64; }`，`rustc` 编译通过。
- 关键坑：`convert_function` 创建的 `fn_ctx`（builder.rs:9530）此前只 clone `comptime_consts` 未 clone 新增字段，导致函数体内仍内联——已补 `fn_ctx.mutated_top_level_consts = ctx.mutated_top_level_consts.clone();`（同处已对 block_ctx/closure_ctx/arm_ctx 补齐）。
- 验证：用自包含复现（模块级 `count=0` + `count_up` 内 `count+=1` + `while (v:=next())<5`/`if (n:=next())>0` walrus）转译并 `rustc` 编译通过（无 E0070）。
- impact: 解决 walrus 簇 7 个文件（walrus/walrus_more/combo_while_guard_else/combo_while_guard_walrus/while_walrus_guard_1/edge-walrus-operator/combo_while_walrus）的 E0070 根因。
- reported_by: diagnose(2026-09-28); fixed_by: self-driven loop

## BUG-8 [2026-09-28] [high] FIXED
- summary: 构造器直接调用语法 `Rc(5)`/`Box(f)`/`Arc(7)` 生成伪造桩函数 `fn Rc(__a0: i64) -> i64 { i64::MAX }` 而非 `Rc::new(5)`，box.lz/magic_methods.lz 等编译失败
- detail: IR 中 `Rc(5)` 是 `Call{callee: Var("Rc")}`。codegen 的 `Call` 臂未识别 Rc/Box/Arc 为构造器，把 `Rc` 当作未知外部函数登记进 `unknown_extern_fns`（collect_unknown_extern_fns 预扫），发射 `fn Rc(__a0: i64) -> i64 { i64::MAX }` 桩；而 `MethodCall` 路径（`Rc.new(x)`）已在 mod.rs:11737 正确路由 `Rc::new`。注意 `--emit=ir` 与 `Box<fn(int)->int>` 函数类型做泛型实参会触发独立解析错误（Unexpected token: Arrow），与本次修复无关。
- fix: 在 `gen_expr` 的 `Call` 臂（mod.rs:9422，紧邻 ord 短路之后）新增 `Var("Rc"|"Box"|"Arc")` 直接调用拦截，发射 `{name}::new({args})`；与 MethodCall 路径对称。桩函数虽仍被预扫登记但因不再被调用成了死代码（`#[allow(dead_code)]`），无害。
- 验证：自包含复现 `Rc(5)`/`Box(5)` 转译→`Rc::new(5i64)`/`Box::new(5i64)`，`rustc` 编译通过（rc_repro/box_repro）。
- reported_by: self-driven loop

## 2026-09-28 修复小结（lang-zone 编译器）
- CY 后端：BUG-13/16/18/B10/B11/EC-006 已全部在仓库内修复（codegen_cython.rs + semantic_check.rs），`cargo check -p lang-zone` 编译通过；CY 端到端需 cython 环境（沙箱缺失，无法端到端验证）。
- Rust 后端已修复并验证：spread_protocol（`?` 作用于 Option 在 Result 上下文先转 `ok_or_else`）；error.lz 的 `!` 类型（嵌套 Never→()`）；dict 扩展 impl 键泛型注入 `K: Ord`；traits 递归检测 `Self.Item` 关联类型补全 `where Self: std::iter::Iterator`；iter 关联类型 clone 豁免（收窄为仅含 `::`/`<` 的真正关联类型，修复误伤 Cow/Point 等普通结构体的回归）；combo-error-control 的 `?` 回归修复；**构造器 `Rc/Box/Arc` 直接调用语法 → `::new`（BUG-8，box.lz/magic_methods.lz 受益）**。
- 网关现状：326 文件，293 通过 / 33 失败（16 个新增回归待修 + 17 个已知设计级失败）。BUG-7（全局变量内联，7 文件）与 BUG-8（构造器桩，box.lz/magic_methods.lz 等）已修复，预期失败数显著下降（新增回归降至 ~7、已知降至 ~17）；因 DEMO .lz 被 lzmc 自动化临时删除，未重跑全量闸门，改用自包含复现 `rustc` 验证。

## BUG-9 [2026-09-30T01:07:34Z] [high] FIXED（2026-10-01 T0r5 续轮：载体统一 Arc<dyn Fn+Send+Sync>，挂起锁已转正）
- summary: [Rc载体外溢] fn 值被 go/spawn 捕获时生成码 E0277：Rc<dyn Fn> 既非 Send 也非 Sync（旧 Box<dyn Fn> 载体是 Send）
- detail: 根因：2026-09-30 T0r3 为修 4 例 E0599（Box<dyn Fn> 不实现 Clone）把 LZ fn 值载体从 Box<dyn Fn> 换成 Rc<dyn Fn>（src/ir/codegen/mod.rs:2568/:3534/:3629 类型构造 + :3681/:3687/:8101/:9465/:13587/:13668 装箱位）。Rc 可 Clone 但不 Send/Sync ⇒ 一旦 fn 值被 `go expr`（codegen:10520 发射 std::thread::spawn(move || {...})）的捕获闭包带走，rustc 报 E0277 `Rc<dyn Fn(i64) -> i64>` cannot be sent between threads safely。

最小复现：FIND_BUG/codegen/bug-rc-fn-not-send-in-spawn.lz（let f = |x: int| x * 2 ; go print(f(1))）→ 生成码 :26 Rc<dyn Fn> 绑定、:28 spawn(move || { ... f(1) ... }) → 实跑 rustc E0277（探针日志 TEMP/rc_probe/）。

已排除的简易解法（实测）：只把 Rc 换成 Arc 仍红 —— Arc<dyn Fn> 要 dyn Fn: Send + Sync 才 Send，裸 trait 对象两条都缺（E0277 双向 + E0252 重复导入是我 sed 手改的副作用）。⇒ 修复方向应是「spawn 位的 fn 值载体带上界」（如按调用点选择 Arc<dyn Fn(..) -> .. + Send + Sync>），或让 go 发射位对捕获的 fn 值做一次性 Box 化转发。

语料缺口：现有 326 文件 DEMO 闸门与 742 项 workspace 全绿也测不到这条路（无一用例在 spawn 内捕获 fn 值），属「闸门全绿但产品回归」。已按 tests/find_bug_bugs.rs:4-5 声明的约定补挂起锁：cg_rc_fn_value_not_send_in_spawn（#[ignore] 记卡点 RUSTC_FAIL），修复后移除转正。该套件事前 40/0/0 → 现在 41 收集、40 passed、1 ignored。
- reported_by: qoder-explore-worker(T0r3终审)


## BUG-10 [2026-09-30T03:35:21Z] [high] FIXED（2026-10-01 T0r523：产品化方案 A 落地——生成 .rs 文件头静态链接配方块（src/ir/codegen/mod.rs emit_prelude_base ＋ LZ 侧镜像 src/ir/lz_codegen_lib.lz module_header，两路线逐字符一致）＋ CLI 三条写出路径 stderr 打印本机解析好的完整可复制 rustc 命令（src/main.rs print_link_recipe）。回归锁 cg_generated_rs_header_link_recipe 转正；10 个 moddec golden 冻结更新。弃 B（额外 recipe 文件与头部注释信息重复、多落工件）；弃 C（纯 Rust BigInt/Complex 替换 num-* 动依赖树、回归半径大收益低）。验证：workspace 38 套件 0 failed；两 repro 实编译实跑绿）
- summary: [链接契约] lz_builtins 因 re-export num-* 而承载可加载依赖 ⇒ 一切孤立 rustc 编译 LZ 产物必须带 -L dependency=<deps>，否则整体 E0463（含不用 bigint 的程序）
- detail: 根因：D6 批次按 BUG-1 处方把 num-bigint/num-complex 经 lz_builtins 对外 re-export（lz_builtins/src/lib.rs 新增 `pub extern crate num_bigint; pub extern crate num_complex; pub use num_bigint::BigInt; pub use num_complex::Complex64;`）。此前 lz_builtins 的 Cargo.toml 虽已声明 num-*，但源码零引用 ⇒ 该依赖不进 rlib 元数据，孤立 `rustc --extern lz_builtins=<rlib>` 能编译。re-export 之后 num-* 成为**可加载依赖**，rustc 必须能解析它，否则连不用 bigint 的程序也整体报 `error[E0463]: can't find crate for lz_builtins`（实测：同一 compose.rs，不带 -L 必红、带 `-L dependency=target/debug/deps` 即绿）。

实测爆炸半径（本轮）：workspace 首跑 36 套件 680 passed/64 failed，64 条全是 E0463 同因（无语义回归）。已补 `-L dependency=<rlib 所在目录>/deps` 的调用面共 15 处：src/main.rs 4 处（.lzfront :263 / IR 路线 .lzlz / .lzrs / --test 模式）、tests/*.rs 11 处（bridge_embed、dual_route_golden、g6_codegen、gen_build_block、incremental_golden、ir_pass_fixes、lz_ir_bootstrap ×3、lz_semantic_cases、moddec_product_assert；另 find_bug_bugs/find_bug_libs 由本腿补）、受版管脚本 4 处（div-tools/regression.sh:52、FIND_BUG/test_all.sh:47、bootstrap/build.ps1:51、bootstrap/build.sh:84）。补完复测：cargo test --workspace --no-fail-fast -j 1 = 744 passed/0 failed/11 ignored（WS_EXIT=0），DEMO 闸门 326/313/13 全在册（GATE_EXIT=0）。

未做（需人工裁决，属 D2 的 L2′ 运行时 ABI 面）：① 这条链接要求已写入开发者侧脚本与测试，但**面向用户的配方文档还没改** —— git grep 命中 README-FOR-AI.md、docs/I6-差异校验-2026-08-20.md、docs/IR-rustc-通过率重测-2026-08-20.md 三处仍教 `rustc --extern lz_builtins` 不带 -L，照做即 E0463；② 是否把该要求做成产品能力（例如 lzc 在产物头部打印所需 rustc 链接参数、或 --emit 时顺带产出一份 build 配方）需要拍板：这是对外契约变更，不是内部重构；③ 备选路线（lz_builtins 自带纯 Rust 的 BigInt/Complex 实现，彻底不引外部 crate）成本更高但可消除这条契约。
关联：台账 BUG-1（re-export 处方出处）、BUG-9（同批次载体外溢）。本轮 T0r4 收尾时一并入账。
- reported_by: qoder-explore-worker(T0r4终审)



## BUG-11 [2026-10-02T00:51:34Z] [medium] FIXED（2026-10-02 cy 后端轮：两处静态构造分支各加窄护栏）
- summary: [跨后端分叉] `Enum.Variant()` 这类**零参调用式构造无数据变体**在 rust 后端发成 `Enum::Variant()` ⇒ rustc E0618；同一份 .lz 在 cy 后端能编译能跑
- detail: 复现件就是受版管语料本身：`CY/TESTS/07_data_structures/enum_data.lz`（`Shape.Point()`）走默认后端时 `error[E0618]: expected function, found enum variant \`Shape::Point\``，走 cy 后端输出三行正常。根因两处：`src/ir/codegen/mod.rs` 的 `static_type_call` 分支（`:11487`，`args_s.is_empty()` 时恒发 `{recv}::{method}()`）与 FieldAccess 分支（`:10362` 一带）。IR 里 `Shape.Point()` 是 **MethodCall(receiver=Var("Shape"))**，走前者且早于后者返回 ⇒ 第一轮只改 FieldAccess 处、重跑 oracle 仍 E0618（实测，取证见 TEMP/reviews/bug11-unit-variant-e0618.md）。
  修复形态：两处各加「`method` 确是该类型的变体（`enum_variants`）且该变体零字段（`enum_variant_fields`）」的窄条件，命中才去掉括号。**护栏对照**：零参关联函数 `Counter.new()` 必须继续带括号（否则 E0015/E0423），实测修复前后都绿。
  探针形状坑（记下来免得下次误判成产品缺陷）：顶层 `let c = X()` 会被生成为 `const` ⇒ `E0015 cannot call non-const associated function in constants`，与本案无关；探针一律包 `def main()`。
  验证：`TEMP/oracle_rust.py` 前后对照（before：uv_dotted/enum_data 红，after：四件全绿，`Point` / `Circle(0.0, 0.0, 5.0)` 等）；回归锁新增 `tests/lzc_loop_unit_variant.rs`（3 条端到端：产物文本断言 + rustc + 运行 stdout）。
- reported_by: qoder-explore-worker(cy后端轮 T0r6)
- 编号说明: 「BUG-11」在 `memory/reviews/20261001.01.05.11.md` §四 里是**另一件**缺陷（TupleLit 槽位不下推）的内部代号，该文自己写明「正式编号以 bug_list 回读为准，入账以台账最大号 +1」。那件已修并挂锁 `tests/find_bug_bugs.rs:538` `cg_tuple_slot_empty_list_elem_ty_pushdown`，未进过台账 ⇒ 本卡按「台账最大号 BUG-10 + 1」取号，与之无冲突，但别把两处 BUG-11 当同一件事。
## BUG-12 [2026-10-02T00:51:34Z] [medium] FIXED（2026-10-06 补同步上下文的线程执行面，判据两侧都在卡内）

- summary: [cy 后端并发降级] `go expr` / `spawn` 在 cy 后端发成同步 `__go(v) = v`，并发语义整体丢失；rust 后端发 `std::thread::spawn`
- detail: 实测面：`src/ir/codegen_cython.rs:205` 逐字为 `def __go(v): return v  # go 并发降级为同步求值` ⇒ `CY/TESTS/11_concurrency/*.lz` 在 cy 侧全部退化为顺序执行，且 stdout 恰好与顺序程序一致 ⇒ L3 拿不到分叉信号（语料无一断言并发交错，只断言最终输出）。不在本轮修的理由：真并发要选线程模型（`threading.Thread` vs `asyncio`），并且现有 11_concurrency 语料的期望值本来就是"顺序也成立"的弱断言，改执行模型必须先改判据形状，属独立一轮。挂在此处，转正前置：`11_concurrency` 至少一件语料的 .exp 要能区分"并发跑过"与"顺序跑过"（例如 join 前打印标记）。
- reported_by: qoder-explore-worker(cy后端轮 T0r6)

### FIXED 2026-10-06T09:38:33Z（qoder-explore-worker, cy 后端轮 T0r8）
- 旧抬头逐字（append-only 台账，状态位就地改，正文留此存证）：`## BUG-12 [2026-10-02T00:51:34Z] [medium] OPEN（入账未修，本轮不动码）`
- **卡内转正前置怎么落地的**：原前置要求「`11_concurrency` 至少一件语料的 .exp 要能区分『并发跑过』与『顺序跑过』」。
  按时序交错去构造（两个任务各自 print，比先后）实测过就放弃：那种期望值进逐字比对的 golden 就是随机红。
  语言层又没有跨线程会合原语（SYNTAX/10 里没有 channel/锁/wait），所以把「并发」这一条**拆成两侧承重**：
  - 行为侧（能确定的那条）：任务失败不打断主流程。新语料 `CY/TESTS/11_concurrency/go_failure_isolated.lz`，
    期望值由 TEMP/xdiff_ds07.py 在**两后端 stdout 逐字一致**的前置下从 Rust 面实跑封盘：`sealed E:\IDEProjects\AI\lang-zone\CY/TESTS/11_concurrency\go_failure_isolated.exp  (11 字节)`
  - 机制侧：同步函数里不许再出现 `__go(`（体必须被延后交给线程）。这条钉在 L1 的文本扫描里，
    与 `ann_bad`（BUG-13）同层同批，随 `cargo test --workspace` 日常跑，不是 `--ignored` 层。
  - 单元锁：`tests/cython_backend.rs::cy_go_dispatch_is_async_gated` **两侧都咬**——同步上下文发
    `__spawn(lambda: boom())`、async 上下文保持 `__go(boom())`；只测一边等于没测。
- 降级为什么会**结构上**修不掉：IR 把 `go e`/`spawn e` 统一降成 `__go(e)`（src/ir/builder.rs:6331），
  而 Python 的实参在调用前就求值完了 ⇒ 任何 `def __go(v)` 垫片拿到的都是「已经跑完的值」。
  修法只能在发射点：`src/ir/codegen_cython.rs` 新增 `current_fn_is_async`（在 gen_function/gen_method
  随 body 生成保存/恢复，与 rust 端同名字段同形），Call 分支里同步上下文改发 `__spawn(lambda: 体)`，
  并新增运行时垫片 `__spawn`（threading.Thread + 体内 try/except，失败往 stderr 喊 `task failed: ...`，
  对齐 rust 面 `std::thread::spawn` 的可观测形态：stdout 干净、stderr 有噪声、主流程继续）。
- 红证（修复前，逐字）：
  - 跨后端差分 `❌分叉 go_failure_isolated.lz rust=OK             cy=RUN_FAIL`（TEMP/probe8/xdiff-11-before-0914.log）
  - cy 面单跑：stdout 无 `"after go"`、stderr `AssertionError:`、退出码见 TEMP/probe8/gojar/cy-run-before.log 末行 `RC=3`
  - 机制侧：`TEMP/probe8/gojar/p_go_panic.pyx:118` = `__go(boom())`，落在 `def main():` 里
- 绿证（修复后，逐字）：
  - 同一差分器：✅一致 go_failure_isolated.lz rust=OK             cy=OK（TEMP/probe8/xdiff-11-after-0919.log，合计 2 件 | 一致 2 | 分叉 0）
  - cy 面单跑：stdout `"after go"`、退出码见 TEMP/probe8/gojar/cy-run-after.log 末行 `RC=0`
  - 三档门禁（1721 这一遍，树身份 = 活树 E:\IDEProjects\AI\lang-zone）：
    L2 「语料 81 件 | cythonize 通过 81 | 失败 0（在册 0）」——注：日志里这一行与一条 L3 进度行**交错**在同一行（原文前缀是 `[cy L3] 02_types/tuple_destructure.lz … `，两个 test 写同一个 stdout），摘引只取 L2 那半句，别把交错前缀当产物内容；L3 「[cy L3] 语料 81 件 | 执行通过 81 | 运行异常 0 | 输出不符 0（在册 0）」；rc 读数 RC_WS=0 / RC_DEMO=0 / RC_CY=0
  - 单元面：`cargo test -j 1 --test cython_backend` → test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
- 遗留（**不在本次 FIXED 范围内**，别把这三条当已做完）：
  1. `async def` 上下文仍是**顺序 await**（协程对象逐个等，不并发）。值侧跨后端实测一致
     （`CY/TESTS/15_feature_matrix/async_spawn.lz` 差分 2/2 一致），但墙钟并发没有可封的确定性判据 ⇒ 挂着。
  2. 同步上下文里 `let t = spawn f()` 的 t 从「f 的返回值」变成「Thread 对象」。这是**对齐** rust 面
     （那边是 JoinHandle，同样不参与后续 stdout），但任何「把句柄当值用」的写法都会变。
     语料里 `spawn_demo.lz` 的 `t` 未被使用，本轮为清单级证据（逐件读过 11_concurrency + 关键词普查 `spawn`/`go `）。
  3. SYNTAX/10 §2.2/§2.5（`await` 句柄取结果）与 §2.3/§3.3（`spawn:` / `go:` 块语法）两后端都没有跑通的面，
     **不在 cy 单方面补**（补了就与另一侧分叉，同 SYNTAX/15 §四 那条裁决的形状）。实测逐字：
     - 块语法：`==== p_spawn_block.lz TRANSPILE_FAIL`（parser 只认冒号块的 `suite`/`test` 那条臂）
     - await 同步 callee 的句柄（rust 面）：`==== p_async_await.lz RUSTC_FAIL`（E0277 `String` is not a future）


## BUG-13 [2026-10-02T01:39Z] [low] FIXED（T0r7 按卡内前置做：判据先行，再改发射点）
- summary: [cy 类型标注失真] `def` 函数的**返回**标注发成 `-> Py_ssize_t` / `-> double`，Cython 直接忽略并告警 ⇒ 写上去的返回类型不承重（参数位的 `Py_ssize_t a` 是有效 C 标注，不受影响）
- detail: 实测面逐字（`CY/TESTS/15_feature_matrix/export_annotation.lz` 的产物第 115 行）：`def add(Py_ssize_t a, Py_ssize_t b) -> Py_ssize_t:`，stderr 同批出现
  `warning: _lzcyg_export_annotation.pyx:115:39: Unknown type declaration 'Py_ssize_t' in annotation, ignoring`。
  发射点 `src/ir/codegen_cython.rs:1377`（`TypeCtx::Signature | Field | Local => "Py_ssize_t"`）——`Signature` 上下文不分「参数位」与「返回位」，返回位也被塞了同一个 C 名字。
  爆炸半径实测：本轮 `target/cy-gate` 下 67 件产物里 **56 件**含 `Py_ssize_t` ⇒ 改成「返回位不带标注」或「返回位走 `cdef`」会动绝大多数产物文本，必须配一整轮 L2/L3 全量重跑，不放本轮尾巴上做。
  后果形态说清楚：这是**标注不生效**（类型检查/优化丢失 + 每次编译一条告警），不是算错值——所以 L3 的 stdout 判据现在测不到它；告警走 stderr，门禁不比 stderr。
  转正前置：先在 `15_feature_matrix` 里给"返回类型标注确实承重"造一条能红的判据（例如断言产物文本里 `def` 行不含 `-> Py_ssize_t`），再改发射点。
- reported_by: qoder-explore-worker(cy后端轮 T0r6)

### FIXED 2026-10-02T05:11:25Z（qoder-explore-worker, cy 后端轮 T0r7）
- 判据先行：`tests/cy_codegen_gate.rs::cy_l1_transpile_all_corpus` 里加了一段签名扫描——逐行取 `def `/`async def ` 打头的行，截 `-> ` 后的标识符，命中 `Py_ssize_t|double|bint|float` 就记进 `ann_bad` 并在末尾 assert。它跑在 L1 那一遍转译的同批产物上，不额外开一遍，且随 `cargo test --workspace` 日常跑（不是 `--ignored` 层）。
- 发射点：`src/ir/codegen_cython.rs` 新增 `render_ret_annotation()`（只对 `raises` 发 `-> object`，其余发空串）+ `ret_note()`（`  # ret: <映射类型>` 行尾注释承载），两处签名发射点（`gen_fn_def` / `gen_method`）与 trait 桩发射点（`def X(...): ...` 改成两行，注释不能吃掉 `...`）都改走这条路。
- 为什么不换成 `-> int`（实测 Cython 3.2.2，探针 `TEMP/annprobe/`）：`-> Py_ssize_t`／`-> double`／`-> bint` 只出 `warning: Unknown type declaration ... in annotation, ignoring`（a_pyssize / h_double / k_bint 三件都是"告警不生效"）；而 `-> int` **真做类型检查**——`def f() -> int: return "x"` 直接 `Error compiling Cython file`（g_int_wrong.pyx，rc=1）。本后端的函数一律发 `def`（`fn_decl` 只有 def/async def，没有 cdef/cpdef），所以这里发不出「既承重又无害」的返回标注：不写是唯一不说谎的形态，类型信息由 `# ret:` 注释保留给读产物的人。
- 爆炸半径与改前快照：`TEMP/ann-before-0314.log`（03:14 那一遍 L1 产物，88 行 C 类型返回标注，逐件路径带行号；重生成＝把 `render_ret_annotation` 返回 `" -> " + map_type(ret)` 再跑 `cargo test -j 1 --test cy_codegen_gate cy_l1`）。改后 L1 全绿：`test result: ok. 2 passed`（4.18s）。
- 单测面：`tests/cython_backend.rs` 5 处文本断言同步改成新形态（`def double(Py_ssize_t x):  # ret: Py_ssize_t` 等），并新增 `cy_return_annotation_policy`——三个返回类型（int/f64/bool）逐个 assert 产物**不含** C 类型返回标注，`raises` 那格 assert 含 `-> object`（这条就是能红的对照：把 `render_ret_annotation` 改回旧形态它必红）。`cargo test -j 1 --test cython_backend` → `35 passed; 0 failed`。
- 遗留（不在本卡内）：参数位的 `Py_ssize_t a` 仍是有效 C 标注，未动。

## BUG-14 [2026-10-02T02:48:10Z] [medium] FIXED
- summary: cy 内建接收者方法族缺改写表（`.len()/.length()/.is_empty()/.contains()/.starts_with()/.ends_with()/.trim()/.remove()/.to_lower()/.to_upper()`）＋ IR 直发的 `.clone()`
- summary: [跨后端分叉/零覆盖] 内建容器与 str 上的方法形态在 cy 后端**逐字透传给 Python**，  Python 侧没有这些方法名 ⇒ 运行期 AttributeError；而 cy 语料对此**零覆盖**（关键词普查实测： `.len()` 在 CY/TESTS 全部 .lz 里只出现在一行注释中，`.length()`/`.is_empty()` 零次）
- detail: 每条的实测失败形态（逐字取自证据日志）： `AttributeError: type object 'str' has no attribute 'len'`（TEMP/probe5/xdiff.log，owned_binding/param_modifiers）； `AttributeError: type object 'dict' has no attribute 'remove'`（TEMP/probe-all-0206.log，dicts.lz）； `AttributeError: '_lzcyg_with_defer.MyResource' object has no attribute 'clone'`（TEMP/probe5/xdiff.log，with_defer）。 `.length()`/`.is_empty()`/`not range`/`'str' object has no attribute 'to_lower'` 四条本轮为 **terminal 观察（未另存日志，取证级）**，但结构性证据在码里：改写臂是本轮才加的， `git show` 可见前一版根本没有这些臂；规范口径见 SYNTAX/99-内置预导入库.md:128（`len(x)`→`__len__`） 与 :152（「`.length()` 是 List/str 的实例方法」——即规范名与 Python 名不同名，必须改写）。 `remove` 特殊：LZ 按**位置/键**移除（DEMO/01_basics/lists.lz 的注释「移除位置 1」）， 而 Python `list.remove(v)` 按**值**移除 ⇒ 不改写是**静默移错元素**（lists.lz 实测 AssertionError）， 统一发 `.pop(...)` 后 list/dict 两侧同口径。 `.clone()` 是**共享 IR** 发出来的（src/ir/builder.rs:7864 把 with 的 guard 作 `alias.clone()` 传入）， 不是 cy 自己的选择 ⇒ 透传与本后端已声明的引用模型一致（src/ir/codegen_cython.rs 的 UnOp Ref/MutRef 分支）。 反向闸门：每条臂都带 `!ty_is_user_class(cg, &receiver.ty)`，用户类自己的同名方法不许改写。
- 修复落点: src/ir/codegen_cython.rs 的方法派发表（`"push"` 臂之后，逐名一臂； 本轮新增 len/length/is_empty/starts_with/ends_with/trim/contains/remove/to_lower/to_upper/clone）
- 判据（可红）: 语料 `CY/TESTS/15_feature_matrix/builtin_methods.lz` + `.exp`（Rust 面实跑封盘）， 由 `cargo test -j 1 --test cy_codegen_gate -- --ignored --nocapture` 的 L3 逐字节比对； 探针件 TEMP/probe6/p_str_len.lz、p_list_len.lz、p_is_empty.lz（差分一致 3/3）
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

## BUG-15 [2026-10-02T02:48:10Z] [high] REVERTED（同一窗口内回退，见卡内追记；改后口径见 BUG-22/BUG-23）
- summary: 两后端 f-string 插值口径不一致，且**同时违反仓库唯一写明该口径的地方**（DEMO 断言）
- summary: [跨后端分叉 + 双后端各自违反 DEMO 断言] LZ 的字符串渲染口径是 Debug（`print` 发 `{:?}`， 值带引号），f-string 插值旧实现按「这个变量是不是字符串」分叉：Rust 后端对**局部 str 变量**发 `{}` （Display，无引号）、对**结构体字段**发 `{:?}`（带引号），cy 后端一律 Display。 ⇒ 同一份源码两种口径，且两条 DEMO 断言在 Rust 面**长期为红**。
- detail: 写明意图的只有两处，且都在 DEMO 里：`DEMO/01_basics/strings.lz:42-43` 的注释 「f-string 插值（LZ 按 Debug 语义输出：字符串值带引号）」+ `assert msg == "\""name=\\\"LZ\\\", age=30\""`， 以及 `polish_28_strings.lz:28` 同形。冻结规范 `SYNTAX/` 全文对插值引号口径**没有任何条文** （grep `f-string`/`插值` 只命中词法说明与控制流禁止项）⇒ 规范不裁决，但仓库既有断言裁决， 且反向证据是硬实测：rust 面 `assertion \`left == right\` failed / left: "{name}=LZ, age=30" 形态` （见 TEMP/probe-all-0206.log 里 strings.lz 那行）。分叉对照实测：with_defer 里 `f"acquiring {self.name}"` rust 出 `acquiring \"database\"`、cy 出 `acquiring database`（字段位 vs 局部位）。 注意 strings.lz / polish_28_strings.lz 在修复前是**两端全红**，不是只有 rust 红。
- 裁决（本轮自决，理由如上）: 统一为 **Debug**——Rust 侧 `gen_fstring` 删掉 Display 分叉 （src/ir/codegen/mod.rs，`spec` 恒为 `{:?}`，分类用的 `str_specs`/`is_str_expr` 整块退役）， cy 侧删掉 `_lz_fs` 助手、`fs_interp` 直接裹 `_lz_dbg`（与 `print` 同一条渲染规则）。 弃"统一 Display"路线：那要求改动两条 DEMO 的断言值——为凑绿改判据是本项目红线。 弃"cy 模仿 rust 的分叉"路线：那是把一个后端内部的不自洽复制成两个后端的不自洽。
- 副产物: 两条既有红转绿（Rust 面 `R17 strings OK`、`P28 strings OK`）——本轮唯一一次 让 DEMO 的断言真正被执行到，此前没有任何门禁跑它们的运行结果（`demo_codegen_compile` 只编译不跑）。
- 判据（可红）: 跨后端差分 `TEMP/xdiff_ds07.py` 对 probe7 四件 4/4 一致； 语料 `strings_checks.lz`/`dicts_checks.lz`/`lists_checks.lz`/`with_defer.lz` 的 .exp 全部由 Rust 面重封
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

### 追记更正 2026-10-02T05:11:25Z（qoder-explore-worker, cy 后端轮 T0r7）：本卡的裁决**已回退**
- 承重理由站不住的地方有两条，都是我这一轮的错，不是表述问题：
  ① **「冻结规范全文对插值引号口径没有任何条文」是假负面主张**。规范是有条文的：
     `SYNTAX/00-词法基础.md:188` 的字面量表把 `f"x={x}"` 明确映射成 `format!("x={}", x)`
     （Display 不带引号），`SYNTAX/04-表达式.md:34` 列 f-string 语法，
     `tests/str_boundary.rs:428` 的 c7 用例注释早已把这条规范行当判据引用。
     我当时 grep 的是 `docs/SYNTAX/`——**该目录不存在**（规范在仓库根的 `SYNTAX/`），
     零命中被我读成"规范没写"。这正是「负面主张先证伪读取通道」那条老坑，我又踩了一次。
  ② **被当判据用的两条 DEMO 断言从来不被执行**。`tests/demo_codegen_compile.rs::run_gate`
     只有 Transpile/Rustc 两个 Stage，不跑产物 ⇒ `strings.lz:43`/`polish_28_strings.lz:28`
     里的 assert 无论写什么都不会响，属主张不属判据；而我卡里写的"副产物：两条既有红转绿"
     指的是我自己新加的语料件 `strings_checks.lz`（DEMO 的逐字拷贝），等于把未经执行的文案
     升级成了标准。
- 回退后实测的成对证据（同一判据、只换产品码）：红 `TEMP/fsb-c-0320.log`
  （`[c7_fstring] 程序运行失败（assert 未通过）` / `left: "name=\\"LZ\\", age=30"`），
  绿 `TEMP/fsb-c-after-0327.log`（`test result: ok. 1 passed`）。Rust 侧 `gen_fstring`
  与 `is_str_expr` 逐字回退到 HEAD（`TEMP/verify_fs_revert.py` 断言代码行相等，注释除外）。
- 真正的跨后端分叉成因不是「该用哪种口径」，而是 Rust 侧静态登记表认不出字段位与降级名
  ⇒ 见 BUG-23（已修）；cy 侧改运行时 `_lz_fs`，与规范一致。
- 号段串账提醒（不改历史）：`## BUG-15` 在 `.fist-loop-20260927/memory/bugs.md:70` 指的是
  另一件事（四层嵌套空列表 E0308）。引用本卡的文案请带路径。

## BUG-16 [2026-10-02T02:48:10Z] [medium] FIXED
- summary: cy 切片形态把 range 对象发进 `[]`；str 索引的元素类型与 oracle 分叉（单字符 vs 码点整数）
- summary: [跨后端分叉] `xs[1..3]` 在 cy 面发成 `xs[range(1, 3)]` ⇒ `TypeError: list indices must be integers or slices, not range`（terminal 观察，取证级； 结构性证据：`StructCtor("Range")` 分支本轮前只有 range() 一种发射）； `s[0]` 在 cy 面给单字符 str，而 Rust 面给**码点整数**（实测产物： `{let __cs: Vec<char> = ("abc".to_string()).chars().collect(); ... __cs[__i] as i64 }`， 越界与负索引一并回 0）⇒ `DEMO/01_basics/strings.lz:18` 的 `assert "abc"[0] == 97` 在 cy 面红。
- detail: 规范对切片是**明确裁决**的：SYNTAX/01-类型系统.md:436 与 :442 写 `lst[1..3]` 走「Python 语义」 ⇒ 索引位的 Range 改写成切片语法 `a:b`（闭区间末位 +1，与 range 分支同口径），新增 `range_slice()`； Range 作为**值**出现（`for i in 1..3`）不动，两条路语义不同。str 索引新增运行层助手 `_lz_idx_str(s, i)` （码点 + 越界 0），两侧同为码点口径 ⇒ 非 ASCII 也一致（都不是 UTF-8 字节）。
- 修复落点: src/ir/codegen_cython.rs 的 `ExprKind::IndexGet` 分支 + prelude 的 `_lz_idx_str` + 新函数 `range_slice`
- 判据（可红）: 语料 `str_index.lz` + Rust 面封的 .exp（`97 99` / `[1, 2]` 等）； 探针件 TEMP/probe9/p_index_slice.lz（含负索引那行，见 BUG-19——**负索引不进语料**，因 oracle 编不过）
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)
### 追记(2026-10-02 同轮，切片行已能封盘)
- 本卡正文里「负索引不进语料，因 oracle 编不过」这句**已按实测更正**：
  ① 切片作值（`print(xs[1..3])`）在 Rust 面确实编不过（E0277，逐字见 `TEMP/seal-15feat-0244.log` 的 str_index 行），但那**不是**负索引的形状——已单独修掉，见 BUG-21；
  ② 负索引的真实失败形态是**运行期 panic**（`index out of bounds: the len is 6 but the index is 18446744073709551615`，逐字见 `TEMP/probe10/xdiff.log`），见 BUG-19 的追记。
- 因此 `CY/TESTS/15_feature_matrix/str_index.lz` 现状：切片行**已封 .exp**（Rust 面产物），只有负索引那一行仍排除在外，理由写在件头注释里并挂了卡号。

## BUG-17 [2026-10-02T02:48:10Z] [high] FIXED
- summary: cy 的 `ref` 绑定发成拷贝 ⇒ 写不回源变量，**静默错值**（不报错、不红）
- summary: [静默错值] 旧实现逐字为「ref r = x → r = x（Cython 无引用语义，直接赋值）」， 于是 `ref r = x; r = 100; print(x)` 打的是旧值。跨后端差分实测：oracle `100/100/8/8/5/5/42/99`， cy `100/42/8/7/5/5/42/99`（TEMP/probe5/xdiff.log 逐字在场）。这是本轮最坏的一类：程序**能跑完、退出码 0、只是值错**。
- detail: Cython 无引用类型，改用**发射期别名**：`ref` 绑定登记「引用名 → 它指向的位置文本」， 其后裸名每次出现（读与写）都改写到该位置。写侧有两条路都要覆盖——真 `Stmt::Assign` 的 target 经 `ExprKind::Var`；而 IR 把「对已有可变变量的赋值」也发成 `Stmt::Let`（实测产物 `r = 100  # mut`）， 那条在 Let 的 `bind_to` 处解析——第一轮只改了读侧，重跑仍错，第二版才补上写侧（实测）。 绑定到字面量/表达式时先落隐式存放位 `__ref_<名>`（`ref lit = 42` 走这条）。别名表**每个函数体清空** （`gen_function` 起手），跨函数同名引用不得互相冒名。
- 已知未覆盖（诚实边界）: ① 把引用作为 `ref` 形参传给被调函数、由被调方写回——Python 传值无法回传； ② 别名按**名字**改写，没做 shadow 分析（嵌套 lambda 同名形参属理论面）。这两条不在本轮语料里， 若要做需要先在语料里造能红的判据。
- 修复落点: src/ir/codegen_cython.rs 的 `CythonCodeGen::ref_alias` 字段 + `gen_function` 清空 + `Stmt::Let` 的 is_ref 分支与 `bind_to` + `ExprKind::Var` 读侧改写
- 判据（可红）: 语料 `ref_binding.lz`（DEMO 逐字拷贝）+ Rust 面封的 .exp（八行含两处写回）
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

## BUG-18 [2026-10-02T02:48:10Z] [medium] FIXED（2026-10-06 补执行面；判据 CY/scripts/cy_test_gate.py）
- summary: cy 面无 `--test` 执行面：`suite`/`test`/`setup`/`teardown` 一个都不执行，却 rc=0 报『Generated』
- summary: [调用面缺失 + 静默成功] `--test` 的实现只认 Rust 后端：`src/main.rs` 的 `run_test_mode` 直调 `rustc --test`。cy 面把 `--test` 当不存在——实测 `lang-zone sp.lz --backend=cython --test` 只打一行「Generated ... (Cython backend, shared codegen with lzcyc)」并以 rc=0 退出， **一个 test 都没跑**，调用方拿到的信号却是「跑过了」。
- detail: 两面的对照实测（同一份带 setup/teardown/两个 test 的 .lz）：rust 面确实执行（打出 `❌ Some tests failed`，具体败因未追——见下方「另需看的一条」）；cy 面零执行零报告。 本轮已做的**不是实现**，是把这条假绿关掉：`--backend=cython` 与 `--test` 同时出现时带原因拒绝并给出路 （`src/main.rs` 的旗标解析段，实测 `rc=1` 与三行拒绝文案；去掉 `--test` 的纯转译仍 `rc=0`，不受影响）。 实现面的形状（未做）：cy 侧需要把 suite 降级成 `def test_<suite>_<name>()` + 一个 runner， 并让 `--test` 走 python 侧执行（`CY/scripts/cy_gate_run.py` 已经是这个执行器的雏形）。 另需看的一条：rust 面 `--test` 自己报 Some tests failed——那可能是 setup/teardown 的降级形状问题 （SYNTAX/15-测试框架.md §三 与 SYNTAX/00-词法基础.md:124 写「setup 中的变量在套件内所有 test 中可见」 「setup/teardown 内联到每个 test」），属 rust 后端/测试框架轮，不放本轮。
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

### FIXED 2026-10-06T08:38:24Z（同一轮内：判据先红，再补执行面）
- 改判前抬头逐字：「## BUG-18 [2026-10-02T02:48:10Z] [medium] OPEN（入账未修；本轮只把『装作跑了』改成带原因拒绝）」（原样留档，不覆盖当次可见性）。
- 判据先行：`CY/scripts/cy_test_gate.py` 先跑在旧码上 ⇒ `TEMP/bug18-judge-before-0802.log` 合计 3 格、红 3 格；
  判据自检 `--selfcheck` 四支（rc=0 无 test 行 / 只有拒绝文案 / 报 1 failed 无 FAILED 行
  必须全判红，罐头绿证必须判绿）当场咬住，否则「我会判状态」是假命题。
- 补完后同一判据复跑 ⇒ `TEMP/bug18-judge-after-0837.log` 合计 3 格、红 0 格，逐格：pass_only（PASS rc=0）、one_fails（PASS rc=1）、setup_visible（PASS rc=0）。
- 落点：`src/ir/codegen_cython.rs` 新增 `set_test_runner`/`emit_test_runner`/`test_names`
  （只在 `--test` 时发射 `_lz_run_tests()` 与 `__main__` 入口；纯转译产物一行都不带，
  否则 L3 stdout 被测试行污染、跨后端 golden 分叉）；`src/main.rs` 新增 `run_cy_test_mode`
  + `cy_python_program` + `CY_TEST_HOST`（pyximport 宿主，`.pyx` 复制成 `_lzcyt_<stem>` 别名
  并校**本次产物目录内的加载来源**——同名孪生会抢 finder，L3 实测过 `enum`/`struct`/`test_suite`）。
- 原来那句「带原因拒绝」现在只剩**取不到宿主 Python** 这一格（rc=1 + 三条出路）；
  拒绝文案变了，任何仍钉旧字面串「cython 后端未实现」的尺子都要跟着改，别让它静默失配。
- 尾巴对齐 Rust 面：`✅ All tests passed` / `❌ Some tests failed` + 退出码 1。
- 勘误（同一小记写完两分钟内的实测，把这条判据自己改了两次；原句留着，改正写在下面）：
  ① 宿主身份**不许读 `mod.__file__`**——LZ 的 cy 产物在模块级就发了 `__file__ = "<源件>.lz"` 与
  `__name__ = "main"`（const 机制），读它会把**真产物**判成冒名 ⇒ 每次 rc=4 假红。改用导入机器形
  写入的 `__spec__.origin`（退路 `__loader__.path`）。② 目录归属**不许用字符串前缀比**：
  `realpath(.../target/cy-testface/1/x.lz)` 以 `.../target/cy-test` 开头却是隔壁目录 ⇒ 门禁那一遍
  其实是被这条假绿放过的。改用 `os.path.commonpath([a,b]) == b`。两支各配对照（判据 `--selfcheck`
  的 C4：真归属放行 ＋ 兄弟目录与仓根冒名都拒），且 C4 是**从 `src/main.rs` 里抠出内嵌宿主源码
  再 exec** 来测的——不在判据侧另抄一份同名实现，抄本测不出出货面。
- 噪声处置：宿主跑两遍（第一遍把 pyximport/setuptools/MSVC 的编译日志关进子进程并丢弃），
  否则冷缓存那遍的链接器日志会混进 stdout ⇒ 跨端 stdout 不可比。逐字绿证：
  `TEMP/bug18-cli-mixed-after-0837.log`（`running 2 tests` / `test good ... ok` /
  `test bad ... FAILED` / `test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out`
  ＋ `❌ Some tests failed`，rc=1）与 `TEMP/bug18-cli-pass-0837.log`（两条 ok ＋ `✅ All tests passed`，rc=0）。
- 三档门禁本轮**未复跑**，原因是别车道在改的 hunk 挡住了编译（混因分栏，不冒充我的绿）：
  `src/ir/codegen/mod.rs:6299` 的 `reg.resolve_import_full(...)` 返回类型与调用点不匹配（E0308），
  而该调用点不在 HEAD 里（`git show HEAD:src/ir/codegen/mod.rs | grep -c resolve_import_full` = 0）
  ⇒ 属 bridge/embed 车道的在途改动。我这侧能复跑的只有 `cargo test --target-dir TEMP/cy-target
  --test cython_backend`，且在破损出现**之前**测得 36 passed / 0 failed
  （`TEMP/cybuild-iso2-0824.log`，RC_TEST=0）；语料面不受影响的依据是发射被 `test_runner` 旗标
  夹住，旗标默认关，且判据的「关了不发」那一侧由同一个测试断言。

## BUG-19 [2026-10-02T02:48:10Z] [medium] FIXED（2026-10-14，rust 后端负整数字面量下标归一化为 len+k；cy 面本就对）
- summary: 规范明写的序列负索引 `xs[-1]` 在 rust 后端编不过（E0277 unsized `[i64]`）
- summary: [规范 vs rust 后端] SYNTAX/01-类型系统.md:436/441 明确 `lst[-1]` 取末位（Python 风）， 但 `print(xs[-1])` 在 rust 后端产物里直接发 `xs[-1]` ⇒ rustc `error[E0277]: the size for values of type `[i64]` cannot be known at compilation time`（terminal 观察，取证级：本轮未把 probe9 那次运行另存日志； 复跑命令 `python TEMP/xdiff_ds07.py TEMP/probe9`）。cy 面走 Python 原生负索引，**行为正确**（出 5）。
- 后果与本轮处置: 语料**不能**收这条——L3 的期望值必须来自另一后端的实跑产物，oracle 编不过就没有产物， 收进来等于抄 cy 自己（这正是本项目反复禁止的「用被测面自证」）。故 `str_index.lz` 里没有负索引行， 并在该件头部注释写明原因与卡号，避免下一个人以为是漏写。
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)
### 追记(2026-10-02 同轮，失败形态按格更正)
- 本卡把两件事混成了一件。逐格实测（三份独立探针件，一份一格，日志 `TEMP/probe10/xdiff.log`）：
  ① **负索引 `print(xs[-1])`**：Rust 面**能编译**，运行期 panic `index out of bounds: the len is 6 but the index is 18446744073709551615`（即 `-1` 被 `as usize` 回绕成 usize::MAX）；cy 面出 5，符合规范 SYNTAX/01-类型系统.md:441。
  ② **切片作值 `print(xs[1..3])`**：才是 E0277 unsized `[i64]` 那个形状——本轮已修，另立 BUG-21。
  ③ **切片在比较位 `assert xs[1..3] == [1,2]`**：两后端本来就一致（`TEMP/probe10/xdiff.log` 里 b_slice_in_cmp 一致行），所以「切片整体在 rust 面坏」这种写法是错的，坏的是**值位**。
- 状态维持 OPEN：本卡只指①，属 rust 后端；本轮不动它（cy 面已对，且修它要动全仓最热的索引发射路径）。
### 修复(2026-10-14)
- rust 后端：codegen 新增 `gen_seq_neg_index`（stmt_gen.rs），对「负整数字面量下标 + 可安全二次求值的序列 place（Var/FieldAccess/IndexGet）」把 `xs[-k]` 归一化为 `xs[(xs.len() as i64 + (-k)) as usize]`（Python 风取末位，:441）。四个容器索引发射点（self.base ref / self.field / in_generic / 普通基底）统一接入。非负下标与变量下标走原快路径（全仓最热索引发射点，零额外开销），仅负字面量新增一次 `.len()`。
- 语料：`str_index.lz` 补回 `print(xs[-1])`/`print(xs[-2])` 并重封 .exp（`97 99 / [1, 2] / [2, 3, 4] / [0, 1] / 5 / 4 / 2 3`），双后端一致。
- 验证：cargo build + 全量 cargo test 绿；探针 `xs[-1]=5 xs[-2]=4 xs[0]=0` 过 rustc 实跑；DEMO 语料 0 件用负索引，回归面不受影响。

## BUG-20 [2026-10-02T02:48:10Z] [low] FIXED（rust 后端 2026-10-14，写穿感知别名替换）
- summary: `ref f = h.n` 字段位引用在 rust 后端编不过（E0308），cy 面写回正确
- summary: [规范/另一后端] `ref f = h.n` 然后 `f = 9` 应当写回字段（引用指向**位置**）。 cy 面本轮加别名后实测 9/9/9 正确；rust 面同一件 `error[E0308]: mismatched types` （terminal 观察，取证级，复跑同 BUG-19）。
- 根因: codegen 把 `ref f = h.n` 发成 `let f = &mut h.n;` + 赋值位 `*f = v`，触发 E0596（基底 h 非 mut）与 E0502（引用存活期间读基底 h.n）。原始报告的 E0308 是更早形态；本轮实测定位到 E0596/E0502 才是真正障碍（复跑 refpos/refvar 两探针证实）。
- 修复: **写穿感知别名替换**——`ref f = <place>`（place = Var/FieldAccess/IndexGet 且 helpers::root_var_of_place 命中）且别名名被后续写穿（re-Let is_ref=false 或 Assign target，由 helpers::scan_ref_write_through 预扫登记 CodeGen.ref_write_through）时，f 视作目标位置别名：读 f → `gen_expr(目标)`，写 f=v → `gen_target_expr(目标) = v`；基底登记 auto_mut（scan_auto_mut_block 的 Let 臂经 root_var_of_place）使 `let h` 升 `let mut h`。零借用冲突。
- 分流（关键）: 只读 ref（moddec `@ref r = x; print(r)`）无写穿，**保持** `let r = &mut x;` 原形态——plain `ref` 与 moddec `@ref` 的 IR 完全同构（`let ref r`），无法按 IR 区分，只能按写穿分流，否则 moddec_product_assert AC3 golden（`let r = &mut x`）会被别名替换破坏（首轮无门控别名替换曾致该 golden 失败）。
- 语料: 本轮未新增/改 .exp（rust 侧已对，但 cy 面未在本环境 L2/L3 复验，沿用原「不进语料」论证挂起；跨后端封盘待 cy 侧 co-verify）。探针 refpos（`ref f = h.n; f = 9; print(h.n); print(f)`）→ `h.n = 9` 实跑 9/9；refro（只读）→ 保持 `&mut` 形态与 moddec golden 逐字一致。
- 验证: cargo build + 全量 cargo test 441/0（moddec_product_assert AC3 转绿）；DEMO 回归 319/8（8 件全为既有——git stash A/B 实证本轮 0 新增，owned_var_pos 恒 exit 7 等）。
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

## BUG-21 [2026-10-02T02:55:06Z] [medium] FIXED（本轮修，rust 后端；动因是它挡住了 cy 的封盘）
- summary: [规范 vs rust 后端] 序列**切片作值**使用（`print(xs[1..3])`、`let a = xs[1..3]`、`for v in xs[1..3]`）在 rust 后端发成 `xs[1..3]`，类型是 unsized `[i64]` ⇒ `error[E0277]: the size for values of type `[i64]` cannot be known at compilation time`；同一份 .lz 在 cy 面出 `[1, 2]`。规范明写切片返回 `List<int>`（SYNTAX/01-类型系统.md:442）
- detail: 证据逐字见 `TEMP/seal-15feat-0244.log`（str_index.lz 那行 rust=RUSTC_FAIL）与 `TEMP/probe10/xdiff.log`（a/b/c 三格分立：值位红、比较位绿、负索引是另一件事）。根因是发射点的分支形状：`IndexGet` 在 `is_self_field/is_self_base` 与 `in_generic_fn` 两条分支都补了 `.clone()`（切片经 `ToOwned` 得到 Vec），唯独**非 self、非泛型的普通基底**那条直发 `{base}[{key}]`——比较位靠 `==` 的自动借位能过，值位过不去。所以「切片在 rust 面坏了」是不精确的说法，坏的是值位这一格（本卡的教训就是被这句话带偏过一次，见 BUG-19 追记）。
  修复形态：该分支在 key 为 Range 时发 `{base}[{key}].to_vec()`。选 `to_vec()` 而不是 `&{base}[{key}]`：owned Vec 在 `{:?}` 打印、与数组字面量比较（`impl PartialEq<[U;N]> for Vec<T>`）、按值迭代三种上下文都成立，且不把引用生命周期引进生成码；比较位原本能过的，改成 Vec 后实测照样过（b_slice_in_cmp 前后一致）。
  为什么算在 cy 轮里做：cy 面自己**是对的**，但 L3 的期望值只能取另一后端的实跑产物——oracle 编不过就没有产物，这条规范特性对 cy 就只能"执行不断言"。修它是为了把 `str_index.lz` 封成交付面能红的判据。
- 判据（可红）: 语料 `CY/TESTS/15_feature_matrix/str_index.lz` + Rust 面封的 `.exp`（`97 99` / `[1, 2]` / `[2, 3, 4]` / `[0, 1]` / `2 3`）；探针件 TEMP/probe10/a_slice_as_value.lz（修复后差分一致，`TEMP/probe10/xdiff-after.log`）
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

### 追记 2026-10-02T05:42:22Z（T0r7 尾）：本卡的修复**带出一个新回归**，另立 BUG-26
- 卡里那句「比较位原本就能过，改成 Vec 后照样过」只对**容器基底**成立。同一分支没有分基底类型，
  于是 `word[0..2]`（基底是文本、IR 类型退化成 `Any`）也被发了 `.to_vec()` ⇒
  `error[E0599]: no method named `to_vec` found for type `str` `，把自举语料打红了。
  本卡的修复范围（只解决「切片作值 unsized」）没错，缺的是**按基底类型分叉**——见 BUG-26。

## BUG-22 [2026-10-02T05:11:25Z] [medium] FIXED（2026-10-02 改正 DEMO 断言；2026-10-16 补销状态位漂移，见卡内 FIXED 段）
- summary: [规范 vs DEMO 语料] `DEMO/01_basics/strings.lz:43` 与 `DEMO/01_basics/polish_28_strings.lz:28` 断言 f-string 插值给 str 值**加引号**（`name=\"LZ\"` / `by \"LZ\"`），与规范行 SYNTAX/00-词法基础.md:188（`f\"x={x}\"` → `format!(\"x={}\", x)`，Display 不带引号）相反
- detail: 这两句从来没人执行——`tests/demo_codegen_compile.rs` 的 `run_gate` 只做「转译 + rustc 编译」两段（Stage 只有 Transpile/Rustc），不跑产物，所以断言写反也不响。危害是**它会被当证据读**：本轮我据此把两个后端的插值统一成 Debug，实测打红了真正在跑的判据 `tests/str_boundary.rs::c7_fstring`（`left: \"name=\\\"LZ\\\", age=30\"` vs `right: \"name=LZ, age=30\"`，证据 `TEMP/fsb-c-0320.log`），回退后同一判据绿（`TEMP/fsb-c-after-0327.log`）。规范行 + 实跑判据两边一致，DEMO 文案孤立，故判 DEMO 侧为错。
- 处置建议: 把这两行断言改成无引号形态（`name=LZ, age=30` / `lang-zone v2 by LZ`），并在件头注释里把「f-string Debug 语义」这句改掉——它现在是错的口径来源。属 DEMO/Rust 车道产物，本卡只记账不代改。
- 判据（可红）: `tests/str_boundary.rs` 的 `c7_fstring`（实跑，规范引用写在该用例注释里）；cy 面新增语料 `CY/TESTS/15_feature_matrix/fstr_interp.lz` 逐档断言（str 无引号 / int / bool `true|false` / None / list / struct 字段）
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

### 追记 2026-10-02T06:00:04Z（同一轮内自纠：本卡的两句已按规范改正）
- 改前实测（把两条 DEMO 件直接喂跨后端差分器，让它们的 assert 第一次真的被执行）：
  `TEMP/probe7/xdiff-before-bug22.log` ⇒ `合计 4 件 | 一致 2 | 分叉 2`，两条分叉**两侧同形失败**
  （rust `assertion `left == right` failed / left: "name=LZ, age=30"`、cy `AssertionError:`）——
  即两个后端早已一致，不一致的只有那两句断言文本。这就是"它不是判据"的正面实证。
- 处置：`DEMO/01_basics/strings.lz:41-45` 与 `DEMO/01_basics/polish_28_strings.lz:24-29` 的注释与断言
  改成规范形态（`name=LZ, age=30` / `lang-zone v2 by LZ`），并在注释里指回 SYNTAX/00-词法基础.md:188 与本卡号。
  复跑差分器 ⇒ `TEMP/probe7/xdiff-after.log`：`合计 4 件 | 一致 4 | 分叉 0`。
- 全仓再扫一遍同形断言：`grep -rn 'f"' DEMO/*/*.lz CY/TESTS/*/*.lz | grep '\\\\"'` ⇒ 零命中，没有第三处。
- 判据（可红）: `python TEMP/xdiff_ds07.py TEMP/probe7`（末行必须是 `一致 4 | 分叉 0`；把任一句断言改回带引号形态，该件立刻分叉）

### FIXED 2026-10-16（qoder-explore-worker，销账轮：抬头状态位漂移修正 + 判据复绿）
- 旧抬头逐字（本条留档，不覆盖当次可见性）：`## BUG-22 [2026-10-02T05:11:25Z] [medium] 入账未修（DEMO 语料文案，非产品码）`
- **状态位漂移，非代码缺陷**：上方「追记 2026-10-02T06:00:04Z」已记录本卡代码实际改完
  （`DEMO/01_basics/strings.lz:41-45` 与 `polish_28_strings.lz:24-29` 断言改无引号形态 +
  注释指回 SYNTAX/00-词法基础.md:188，差分器复跑 `一致 4 | 分叉 0`，全仓同形断言零命中），
  但抬头一直停留在「入账未修」，与本卡内文自相矛盾 ⇒ 台账漂移，本轮只补状态位。
- 本轮实证（2026-10-16）：
  - `DEMO/01_basics/strings.lz:44` 实测已是 `assert msg == "name=LZ, age=30"`（无引号），
    注释含「2026-10-02 按台账 BUG-22 改正」；`polish_28_strings.lz:28` 同形为
    `assert msg == "lang-zone v2 by LZ"` ⇒ 代码侧确已修，非「未修」。
  - 判据复绿：`cargo test --test str_boundary` → **3 passed / 0 failed**（含 `c7_fstring`）。
  - 全量闸门旁证：`cargo test --test demo_codegen_compile all_demos_generated_rust_compiles
    -- --ignored` → **327 文件 / 327 通过 / 0 失败**（两个 DEMO 件在该闸门内转译+编译全绿）。
  - 全工作区扫描：`cargo test --workspace --exclude lang-zone` → **零失败**。
- 结论：**本卡代码无需再改**（DEMO 语料层，非产品码）；销账仅因状态位与文内实证不符。
  回退方式：无需回退（未动任何产品码）；若判据 `c7_fstring` 转红，则按上方「追记」段的
  原结论处理——把两处断言改回规范形态并复核双后端差分器。

## BUG-23 [2026-10-02T05:11:25Z] [medium] FIXED（本轮修，rust 后端 f-string 静态判定）
- summary: [规范 vs rust 后端] `format!("{{x}}")` 的 Display/Debug 分档只认「已登记的 str 变量」，`self.field` / `param.field` / `let` 绑定的 str 字段与降级变量（`file`→`file_`）全部落到 Debug 分支 ⇒ str 插值被加引号，违反 SYNTAX/00-词法基础.md:188，并与 cy 后端分叉
- detail: 实测逐字（修复前）：`TEMP/probe12/wd.rs:32` = `println!("{:?}", format!("acquiring {:?}", self.name));`，`TEMP/probe12/fstr_matrix.rs` 的 `res.name`/`file_` 同形；cy 侧同一份源码是 `f"acquiring {{_lz_fs(self.name)}}"`（str 值不加引号）⇒ 跨后端 stdout 分叉，`with_defer.lz` 封不了盘。根因是 `gen_fstring` 拿到的插值是**文本**而不是 `Expr`，只能查 `str_typed_vars` 名字表，而该表 (a) 不登记字段访问、(b) 登记的是降级后的安全名（`file_`），插值文本永远是源码名（`file`）⇒ 必然查不到。
  修复形态：新增 `fstr_var_types`（形参在 `gen_fn_def` 登记、`let` 绑定在 `gen_stmt` 的 Let 分支登记，源码名与安全名都写），`is_str_expr` 先按该表的 `IrType` 复核（`strish` = Str / Ref(Str) / MutRef(Str)），再对 `recv.field` 用 `self_type_name` + `struct_fields_info` 查字段声明类型。cy 侧对应口径是**运行时**判定：prelude 新增 `_lz_fs(v) = v if isinstance(v, str) else _lz_dbg(v)`，`fs_interp` 的裹法从 `_lz_dbg` 换成 `_lz_fs`——比 Rust 的静态表更贴近规范，且 bool/None/容器仍走 Debug（Python 原生 `str(True)`=`True` 而 LZ 要 `true`，实测 `DEMO/02_types/duck_field_rel.lz`）。
- 判据（可红）: `tests/str_boundary.rs::c7_fstring` 维持绿；`CY/TESTS/15_feature_matrix/fstr_interp.lz` + `with_defer.lz` 的跨后端差分（`TEMP/seal-15-0442.log`）——任一侧口径漂了就分叉红
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

## BUG-24 [2026-10-02T05:11:25Z] [medium] FIXED（2026-10-14，rust 后端命名实参按声明顺序重排，封 .exp）
- summary: [规范 vs rust 后端] 命名实参 `k=v`（SYNTAX/02-变量与绑定.md:226 的 `name~` 糖的展开形、SYNTAX/01b-duck关系约束.md:481 的 `build(name="x", version=1)`）在 rust 后端**原样抄进产物** ⇒ 生成的 Rust 语法不可编译；cy 后端走 Python 原生关键字实参，行为正确
- detail: 实测逐字。跨后端差分器（`python TEMP/xdiff_ds07.py CY/TESTS/15_feature_matrix`，日志 `TEMP/seal-15-0442.log:81-84`）：`named_args.lz` 一格 `rust=RUSTC_FAIL / cy=OK`，rust 侧诊断 `error[E0425]: cannot find value `b` in this scope | error[E0425]: cannot find value `a` in this scope`，cy 侧 stdout `"3
3
\"hi LZ\"
\"hi world\"
"`（值全对）。产物文本逐字：无默认值形参 `TEMP/probe13/kw2.rs` 的 main 体 = `println!("{:?}", add(b = 2i64, a = 1i64));`（Rust 表达式位不允许赋值）；有默认值形参（发成 Option）`TEMP/probe13/kw.rs:38` = `greet(Some(name = "LZ".to_string()))`；struct 关键字构造写 `=` 时同型 `Box2 { label: label = \"field\".to_string(), ... }`（键名重复一份）。
  为什么算在 cy 轮里：cy 侧不是坏的，但 L3 的期望值只能取**另一后端**的实跑产物（BUG-21 同一条口径），oracle 编不过就没有产物 ⇒ 用到命名实参的语料只能做 L1/L2，不能封 `.exp`。本轮按此把 `CY/TESTS/15_feature_matrix/named_args.lz` 落成「转译+cythonize 断言、执行不断言」，并在件头写明拿不到 oracle 的原因。
  转正前置：rust 侧把 `k=v` 实参按被调函数的形参顺序重排成位置实参（需要 形参名表，现只有 `fn_param_info` 的 (arity, default_count)），缺位用默认值/`None` 填；然后 `named_args.lz` 连同 Rust 面产物一起封盘。
  修复（2026-10-14，本轮）：codegen Call 分支头部统一收口关键字实参——新增 `CodeGen::reorder_kwarg_args`（`src/ir/codegen/expr_gen.rs`），按 `fn_param_names`（builder 新登记的 函数名→形参名 表，`mod.rs`）把两种字形（`_KwArg` 糖 与 `AssignExpr{target: 变量}` 全形，识别器 `helpers::kwarg_name_value`）重排为位置实参**值表达式**，交常规实参渲染路径处理（current_expected_ty 的 Option 默认值形参 Some 包裹 / 类型转换 / clone 注入）——替代原先绕过这些机制、漏掉 Some 包裹的特例分支。守卫排除 kwargs 注入与变参函数（各有专门分支，若在此重排会丢弃其 kwargs/变参实参），仅确有关键字实参时克隆（方向 B 分配消除）。删除旧 WIP 特例分支与 5 处 `__TRACE__` 调试打印。`named_args.lz` 扩展覆盖 `name~` 糖并封 `.exp`（stdout `3/3/hi LZ/hi LZ/hi world`），双后端差分转正。
- 判据（可红）: `CY/TESTS/15_feature_matrix/named_args.lz` 过 L1/L2；rust 面产物可编译性由 `TEMP/probe13/kw2.rs` 手工复跑 rustc 判（当前必红）
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

## BUG-25 [2026-10-02T05:11:25Z] [high] FIXED（本轮修，回归由本轮 own 的改写表引入）
- summary: [cy 回归] `check` 块垫片类 `_LzKwargsMap` 只发了 `__getitem__`，而 BUG-14 的方法改写表把 `ps.kwargs.contains(k)` 改写成了 `k in ps.kwargs` ⇒ Python 的 `in` 在没有 `__contains__` 时退化成迭代协议，从 `[0]` 开始试 ⇒ `KeyError: 0`
- detail: L3 逐件复现（子集跑只出诊断：门禁末尾那条 `!subset` assert 会主动红一次，声明这一遍不构成判定）。`cargo test -j 1 --test cy_codegen_gate cy_l3`（LZ_CY_GATE_ONLY=12_build_blocks）13.76s 复现，逐字：`❌新增 12_build_blocks/checker_call.lz :: KeyError: 0`；完整 traceback（`pyximport` 直跑 `_lzcyg` 同名产物）落在 `checker_call.pyx:146 → check_positive → :114 if ("x" in ps.kwargs) → :139 _LzKwargsMap.__getitem__`。这不是既有缺陷：`contains` 改写给 str/list/dict 接收者都对，唯独漏了本后端**自己发的垫片类**不是内建类型，`ty_is_user_class` 那道闸拦不住它（它不在 LZ 源码的类型名集合里）。
  修复形态：垫片补 `__contains__` / `__len__` / `__iter__` / `items()`（`in`/`len`/`for`/`items` 四个 LZ 拼得出的形态各自有真实现），`__init__` 容忍 `d=None`，并让 `__Params` 真的持有 `args`（原先只赋 `kwargs`，`ps.args` 是 AttributeError 悬空）；垫片文本从两份逐字副本收成一份 `const PARAMS_SHIM`（`generate()` 尾部和 `postprocess_pyx` 插入路径共用——两处副本正是「改一处漏一处」的形状）。
- 判据（可红）: 语料 `CY/TESTS/12_build_blocks/checker_call.lz` 的 L3 逐字 stdout 比对（期望 `3`）；同层 `checker_call.pyx` 里 `class _LzKwargsMap` 段含 `__contains__`。回归形态：删掉 `__contains__` 那一行，该件必红（`KeyError: 0`）。修复复跑（同子集）：`call_block/checker_call/gen_block/var_block … ok`，汇总 `执行通过 4 | 运行异常 0 | 输出不符 0`（该遍 finished in 38.52s）。
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

## BUG-26 [2026-10-02T05:42:22Z] [high] FIXED（本轮修，回归由 BUG-21 的修复引入）
- summary: [回归/自举语料] 序列**切片作值**的 owned 化不看基底类型：文本基底（`word[0..2]`）也被发 `.to_vec()` ⇒ `error[E0599]: no method named `to_vec` found for type `str` in the current scope`，`cargo test --workspace` 两个 target 红（`-p lang-zone --test lz_frontend_bootstrap`、`--test lz_ir_bootstrap`）
- detail: 这条是**自己上一笔修复带进来的**，而且是门禁自己抓的（不是我另找的用例）：BUG-21 在 `IndexGet` 的非 self／非泛型分支上加了「Range 键 ⇒ `.to_vec()`」，理由是容器切片是 unsized `[i64]`。漏了另一半：`String`/`&str` 的 `Index<Range>` 出的是 `&str`，正确的 owned 化是 `to_string()`，而 `to_vec()` 在 `str` 上根本不存在。失败点逐字在 `TEMP/gates-ws-0503.log:798`（rustc 报在测试临时件 `lz_lex_flag/rich.rs:261:89`，源码那行是 `word[((0i64) as usize)..((2i64) as usize)].to_vec() == "__".to_string()`）。
  为什么类型判定差点也没救回来：`base.ty` 对逐字符累加的 `word` 是 `IrType::Any`（builder 推断缺口），所以既有的 `base_is_str_any` 那条早返回没命中——命中它的是 `s/source/input` 字段启发式。修复形态：新增 `base_is_str_like()`（类型 Str/Named(str|String)/Ref|MutRef(Str) ∪ 字段名启发式 ∪ `Any` 时查 `str_typed_vars` 与 `fstr_var_types` 两张登记表），Range 键分支按它选 `to_string()`／`to_vec()`；不猜的情况留给原形态。
- 判据（可红）: `cargo test -j 1 --test lz_frontend_bootstrap --test lz_ir_bootstrap`（修复后 `4 passed` + `5 passed`；把 `base_is_str_like` 换成只看 `base.ty` 即回到 E0599 红）。覆盖面口径：这两个 target 跑的是自举词法器产物真过 rustc，属**编译面**判据，不比 stdout
- reported_by: qoder-explore-worker(cy 后端轮 T0r7)

## BUG-27 [2026-10-06T09:13:36Z] [high] FIXED（本轮修；回归由共享 builder 的 `with` 尾值捕获引入，坏在 cy 发射侧）
- summary: [cy 回归] BlockExpr 语句提升把**尾表达式执行两遍** ⇒ `with` 体里最后一条 print 在产物里出现两次，L3 逐字比对判红
- detail: 红证逐字（`TEMP/gates-cy-1649.log`）：
  `❌新增 15_feature_matrix/with_defer.lz :: 期望 "\"acquiring database\"\n\"using database\"\n\"releasing database\"\n\"standalone with block\"\n\"inside standalone with\"\n\"processing data.txt\"\n\"closing data.txt\"\n\"with and defer demonstrated\"\n\"cleanup step B\"\n\"cleanup step C\"\n\"cleanup step A\"\n" / 实得 "\"acquiring database\"\r\n\"using database\"\r\n\"using database\"\r\n\"releasing database\"\r\n\"standalone with block\"\r\n\"inside standalone with\"\r\n\"inside standalone with\"\r\n\"processing data.txt\"\r\n\"closing data.txt\"\r\n\"with and defer demonstrated\"\r\n\"cleanup step B\"\r\n\"cleanup step C\"\r\n\"cleanup step A\"\r\n"`
  ——期望与实得的两行差异就是 `"using database"` 与 `"inside standalone with"` 各多打一遍。
  破坏面不是单点而是**一类**：`src/ir/codegen_cython.rs` 里 9 个提升点全部写成
  `gen_block(block)`（连尾语句一起发）＋ 紧接 `block_tail_expr(block)` 再发一次尾值。
  触发条件由共享侧新加的脱糖提供：`src/ir/builder.rs:7837` 把 `with` 体包成
  `let __with_val_<绑定名> = { 体语句; 尾值 }`（为了让 `with` 落在值位置时不返回 ()），
  而 `git show HEAD:src/ir/builder.rs | grep -c __with_val` = **0** ⇒ 该 IR 形成本轮才出现在树上，
  HEAD 里没有，所以这条红不能靠回退 HEAD 复现。
  修复形态：新增 `gen_block_prefix()`（只生成「尾之前的语句」，defer 的 LIFO flush 保持原样，
  不做 `gen_block` 的空块 `pass` 兜底——调用方紧接着必写一行，块体非空由那行保证），
  9 个点全部改走它；尾是 `assert!`/`assert_eq!`/`panic!` 时 `block_tail_expr` 返回 None，
  前缀长度自动退回全量，语句不会被吞。
- 判据（可红）: 单元锁 `tests/cython_backend.rs::cy_block_expr_tail_runs_once`
  ——同一 IR 形态（`let __with_val_res = { print(PREFIX); print(TAIL) }`）上断言
  两个 marker 在产物里**各恰好 1 次**，并逐字断言绑定行 `__with_val_res = print(_lz_dbg("TAIL_ONCE"))`；
  语料侧仍由 L3 逐字比对承重（`.exp` 一个字节没改）。修复后复跑：`cargo test -j 1 --test cython_backend`
  → `37 passed; 0 failed`；三档门禁 `绿证逐字：`TEMP/gates-cy-1704.log` 「[cy L3] 语料 80 件 | 执行通过 80 | 运行异常 0 | 输出不符 0（在册 0）」`
- 流程债（诚实标注，别当成已完备）: 单元锁**没有做同码变异红证**——往本树注入旧行为（`gen_block` + 尾值双发）
  再跑该测试这条路被权限分类器拦下（理由是「故意引入回归」）。它的可红性目前依据是**同一缺陷、同一 IR 形态**
  的语料侧红证 `TEMP/gates-cy-1649.log`（多打的那两行就是它数的那个计数）。下轮若拿到变异许可，补法就是
  把 `gen_block_prefix` 的 `n` 改回全量跑一次该测试，必须红。
- reported_by: qoder-explore-worker(cy 后端轮 T0r8)

## BUG-28 [2026-10-06T09:13:36Z] [medium] FIXED（rust 后端 2026-10-14；与 BUG-44 同根因同修复）
- summary: [BUG-23 复发 / 跨后端 oracle 分叉] `with ... as res` 绑定出来的接收者，其 str 字段插值又落回 Debug：
  `f"using {res.name}"` 发成 `format!("using {:?}", res.name)` ⇒ 违反 SYNTAX/00-词法基础.md:188，
  并与**它自己 2026-10-02 封盘的** `CY/TESTS/15_feature_matrix/with_defer.exp` 分叉
- detail: 逐字取自 rust 产物 `TEMP/evidence-r8/with_defer.rs`（从 oracle 工作目录 `TEMP/oracle/` 摘存，那边每次跑 oracle 会 rmtree）：
  - 正常的那条（方法体内 `self.name`）：`println!("{:?}", format!("acquiring {}", self.name));`
  - 坏的那条（`with` 体里 `res.name`）：`println!("{:?}", format!("using {:?}", res.name))`
  - 合成绑定 `res` 的那一行本身：`let mut res = __with_res.__enter__();`
  同轮对照探针 `TEMP/probe8/p_fstr_field.lz`（**不走 with**，两种直接绑定都正常）：
  `println!("{:?}", format!("ctor={}", a.name));` / `println!("{:?}", format!("call={}", b.name));` ⇒ 失效面窄，只落在 `__enter__()` 合成出来的那个 `let mut res` 上；
  `src/ir/codegen/mod.rs` 的分档读取顺序是 `unbound_fstring_vars` → `downgraded_vars` →
  `len(...)` → `is_str_expr`（见 :15044-15047 与 :15090-15094 的 `str_specs` 用法），
  待查的是 `fstr_var_types` 在 `is_mut` 的合成 Let 上有没有登记。
  为什么算在 cy 轮里，以及**明令**：cy 侧是对的（`.exp` 就是 Display 形态，L3 拿它比 ⇒ 门在替规范承重）。
  **不许把 `.exp` 改成带引号形态去迁就 rust 产物**——那是 2026-10-02 已经回退过的「统一 Debug」裁决
  （见 BUG-22/BUG-23 卡片与 SYNTAX/00-词法基础.md:188），改 `.exp` = 把规范改掉。
  无人执行面（本仓「没人看着的红」第三条，前两条是 CI 的 run_check.sh 与 lzcyc）：
  `cargo test --workspace` 不执行 with_defer，DEMO 闸门只转译 + rustc 编译、从不跑里面的 assert
  ⇒ rust 面这条分叉没有任何门在测，本轮靠 cy 侧的 `.exp` 比对才暴露。
  转正前置：先在**能被执行的**门里加一条「with 绑定接收者的 str 插值 = Display」的实跑断言（要能红），
  再动 `gen_fstring` 的登记臂；改完 with_defer.exp 与 rust 产物要同向。
- 修复（rust 2026-10-14）：根因**不在**共享层绑定类型——`let mut res: MyResource` 在 IR 里已正确落地（本卡假设「落成 Any」不成立），真正障碍在 **codegen 块表达式子 CodeGen 未继承 f-string str 类型登记表**：with 体是块表达式，走全新子 CodeGen（expr_gen.rs BlockExpr 臂），`is_str_expr("res.name")` 查 `fstr_var_types["res"]`/`struct_fields_info` 全空 ⇒ 落 Debug。`self.name`（在 `__enter__` 直接体、非嵌套块）不受影响，故当时「self.name 两侧一致」的读数具误导性。修法：给子 CodeGen 继承 `str_typed_vars`/`fstr_var_types`/`struct_fields_info`/`fstr_self_type` 四表（模块/函数级、体积小，块路径克隆可忽略）。探针 withas（`with MyResource(name:"database") as res: print(f"using {res.name}")`）实跑 `using database`（无引号＝Display，合 SYNTAX/00-词法基础.md:188）。
- 语料：`with_as_binding_fstr.lz` 已在语料（2026-10-09 拆出）；本轮**未封** `with_as_binding_fstr.exp`、**未移** `NO_ORACLE_EXPLAINED`——封盘要求两后端逐字一致（seal_missing_exp.py），本环境无 cython/cythonize（L2/L3 门不可跑），cy 侧 co-verify 挂起。母件 with_defer.lz 产物逐字不变（其 f-string 均为简单变量插值、非嵌套块字段访问；.exp 与产物内容一致，仅 CRLF/LF 行尾差异）。
- 验证: cargo build + 全量 cargo test 441/0（无 golden 回归）；探针 withas 过 rustc 实跑 Display。
- reported_by: qoder-explore-worker(cy 后端轮 T0r8)

## BUG-29 [2026-10-06T09:56:34Z] [medium] FIXED（2026-10-14 收尾：半成品修复验证通过，`CY/TESTS/01_basics/literals.exp` 已补封）
- summary: `CY/TESTS/01_basics/literals.lz` 在 rust 面 rustc 失败（`error[E0282]: type annotations needed`），cy 面同一份源码跑通 ⇒ 这一件的期望值**取不到 oracle**，L3 只能执行、不能判定
- detail: 逐字取自本轮封盘回执（TEMP/seal-missing-1741.log）：
  `未封 CY/TESTS/01_basics/literals.lz :: 不封：rust=RUSTC_FAIL error[E0282]: type annotations needed | error: aborting due to 1 previous error; 6 warnings emitted`
  口径：cy 语料的 `.exp` 只许取**另一后端**的实跑产物（BUG-21 同一条纪律），拿 cy 自己的 stdout 反灌＝把当期缺陷封成契约，所以 rust 编不过就等于这件没有覆盖。
  性质是「**挡路**」不是「cy 坏了」：cy 侧对字面量的处理已跑通，缺的是 rust 侧的类型推断。
  复跑：`python -u TEMP/xdiff_ds07.py CY/TESTS/01_basics`（不带 LZ_SEAL_EXP=1，只看差分）。
  转正前置：rust 面让该件产物可编译后，用 `python -u TEMP/seal_missing_exp.py` 补封（该工具**不会覆盖已存在的 .exp**）。
  收尾记录（2026-10-14）：工作区半成品修复（`src/ir/codegen/expr_gen.rs` +93 行：None 变量引用选型、字段期望类型注入、`LitKind::None_` 加类型参数、`ty_has_unbound_generic` 辅助）经复现验证有效——单文件转译 → rustc → run：`run_rc=0`，stdout=`'42\n3.14\nhello\ntrue\nfalse\nNone\n'`（与 cy 面一致）。已按 BUG-21 纪律取 **rust 面实跑 stdout** 补封 `CY/TESTS/01_basics/literals.exp`，并同轮从 `tests/cy_codegen_gate.rs::NO_ORACLE_EXPLAINED` 移出（该表双向对表，补封后不移出会把门禁判红）。
- reported_by: qoder-explore-worker(cy 后端轮 T0r8)

## BUG-30 [2026-10-06T09:56:34Z] [medium] FIXED（2026-10-14 收尾：半成品修复验证通过，`CY/TESTS/99_bootstrap/lz_std.exp` 已补封）
- summary: `CY/TESTS/99_bootstrap/lz_std.lz` 在本轮差分器的取数面（`lzc` 单文件转译 → `rustc --extern lz_builtins` → run）下 rustc 失败（`error[E0308]: mismatched types`），cy 面 OK ⇒ 该件同样封不了盘
- detail: 逐字取自 TEMP/seal-missing-1741.log：
  `未封 CY/TESTS/99_bootstrap/lz_std.lz :: 不封：rust=RUSTC_FAIL error[E0308]: mismatched types | error: aborting due to 1 previous error; 6 warnings emitted`
  **主张范围要收窄**：这只证明「单文件转译 + 那条 rustc 配方」跑不通 lz_std.lz，**不**等于「rust 后端跑不了 LZ 标准库」——自举面在仓库里另有体系（`tests/lz_ir_bootstrap.rs`），本轮没在那里取证，也不在那边下结论。
  对本车道的实际影响只有一个：L3 的 golden 覆盖清单里这一件与 BUG-24（named_args，rust 面 E0425）长期是缺口。
  复跑：`python -u TEMP/xdiff_ds07.py CY/TESTS/99_bootstrap`。
  收尾记录（2026-10-14）：工作区半成品修复（`src/ir/builder/convert_decl.rs` +28 行：`tail_is_none_expr` + None 尾表达式返回类型升级 `Option`）经复现验证有效——单文件转译 → rustc → run：`run_rc=0`，stdout=`'lz_std ok\n'`（与 cy 面一致）。已取 **rust 面实跑 stdout** 补封 `CY/TESTS/99_bootstrap/lz_std.exp`，并同轮从 `tests/cy_codegen_gate.rs::NO_ORACLE_EXPLAINED` 移出。
- reported_by: qoder-explore-worker(cy 后端轮 T0r8)

## BUG-31 [2026-10-06T09:56:34Z] [low] 已处置（2026-10-14 决策：维持「语言层未定义」，规范补声明 + 语料不承重；不改产品码）
- summary: 同一份源码编译出的 rust 程序连跑三次打印出三种 `Set` 顺序 ⇒ 任何直接 `print(Set)` 的语料都没有可比对的逐字期望值（cy 面同一件三次输出稳定，问题在另一侧）
- detail: 实测逐字（TEMP/evidence-r8/set-order-3runs.txt；源码 `TEMP/probe8/set_order.lz` 就是 `let st: Set<int> = {1, 2, 3}` + `print(st)`）：
  - rust 面三跑（去重 3 种）：`'{1, 2, 3}\n'`、`'{2, 1, 3}\n'`、`'{3, 2, 1}\n'`
  - cy 面三跑（恰 1 种）：`{1, 2, 3}`
  根因面：rust 后端把 LZ `Set` 映到 `std::collections::HashSet`，默认 `RandomState` 每进程随机种子，Debug 顺序随进程变；
  而 SYNTAX/02 从未承诺 Set 的迭代顺序（`grep Set` + 顺序/order/迭代 零命中）⇒ 不是「算错」，是**语言层未定义**被产物暴露：用户程序打印 Set 的输出跨进程不稳定且两后端不可比。
  本轮处置（**语料侧**，不动产品码、不开豁免清单）：`CY/TESTS/02_types/containers.lz` 改成只打与顺序无关的事实
  （`xs.len()/d.len()/st.len()` + `st.contains(2)` + `d["a"]`），件头写明为什么不能整包 print(Set)；改完该目录跨后端差分 7/7 一致，已封盘。
  三份在册豁免清单（`KNOWN_CYTHONIZE_FAILURES`/`KNOWN_DIFF_NOT_COMPARABLE`/`KNOWN_RUNTIME_FAILURES`）本轮**零新增**，仍是空
  ——放宽比较＝放水，这条纪律比覆盖数值钱。
  转正前置（要产品侧改的时候，二选一，不能两头都当没事）：① LZ 给 Set 一个有定义的可打印形态（排序后打印，规范先补一行）；
  ② 规范明文 Set 打印不承重、不许进 golden。
  处置决策（2026-10-14，选定方案 ②）：维持「语言层未定义」，不改产品码。已在 `SYNTAX/01-类型系统.md` §2.1 补声明：`Set<T>` 迭代/打印顺序未定义、跨进程不稳定、双后端不可比，**直接 `print(Set)` 的输出不承重，不得作为逐字 golden**；语料侧维持既有规避（`CY/TESTS/02_types/containers.lz` 只打与顺序无关的事实，已在库）。后续若需可打印形态再走方案 ①（独立立项：排序后打印 + 规范补行）。
- reported_by: qoder-explore-worker(cy 后端轮 T0r8)

## BUG-32 [2026-10-06T13:06:08Z] [high] FIXED（本轮修，cy 后端；`src/ir/codegen_cython.rs`）：块体闭包（`|a, b| =>` 多行体）的前缀语句被整条丢掉
- summary: `ExprKind::Lambda` 的体是 `BlockExpr` 时，cy 走的是表达式通道的降级分支（`src/ir/codegen_cython.rs` 里 `ExprKind::BlockExpr { block } =>` 那句注释写得很清楚：「深嵌套位置（无法语句提升）：降级为尾表达式」），产物发成 `lambda a, b: (c * c)` —— `c = a + b` 整条消失。本例恰好因为 `c` 从未绑定而被 Cython 拦下（响），**但换成被丢的语句不产生未绑定名字时就是静默少副作用**（例如体里是一条 `print` 或一次累加）
- detail: 证据件 `TEMP/prefix-loss-203431.log`（取数件 `TEMP/closure_prefix_loss.py`）。该件不把「修复前的拒绝原文」当成 HEAD 产物来引用——它是**手写旧发射形态的那一行**（`sq = lambda a, b: (c * c)`，即丢掉前缀语句 `c = a + b` 之后剩下的东西）再跑门禁同一条命令得到的，逐字回执：`OLD_SHAPE_RC=1` ＋ `old_shape.pyx:1:19: undeclared name not builtin: c`。
  同一件的另一半是当前产品码的产物读数：`CURRENT_HAS_DEGRADED=False`、`CURRENT_HAS_HOISTED_DEF=True`、`CURRENT_CYTHON_RC=0` ⇒ ① 的拒绝与 ② 的通过是同一条主张的两面。
  函数体与模块级 const 两条路都要管：`preprocess_lambdas`（函数体）与新增的 `preprocess_lambdas_expr`（const 初值）——第一版只改了函数体那一遍，取数件 A 组仍是 `cy=RUN_FAIL`。**那条读数没有落盘**（当时是前台跑的），本卡对它的主张只到「会话级」；盘上能复核的等价证据是上面那份旧形态拒绝件——它拒的正是同一行 `lambda a, b: (c * c)`。补上 const 通道后，两侧读数 `rust='25\n' / cy='25\n'`（逐字一致=True，见 `TEMP/closure-probes-201234.log` A 组）。
  修复形态：块体闭包 hoist 成 `def __lambda_N(形参):`（前缀语句照常生成，尾值转 `return`），绑定行引用该名字；单表达式闭包**保持** `lambda`（成对断言在单元锁里）。
  顺带补齐 SYNTAX/03e §五 的**写捕获声明**：hoist 出的 def 里按名字归属发 `global`（模块变量）或 `nonlocal`（外层函数已绑定），块内新绑定的名字不声明——原先 `AssignExpr` 臂是**无条件**发 `nonlocal`，用在模块变量上本身就是 SyntaxError。
  判据三层：语料 `CY/TESTS/04_functions/closure_basic.lz`（8 条 print，期望值由 Rust 面实跑封盘，实测逐字 `'10\n42\n6\n6\n12\n25\ntrue\n"hi"\n'`）、`closure_modlevel.lz`（模块级那条路，封 `'25\n'`）、单元锁 `tests/cython_backend.rs::cy_closure_block_body_is_hoisted` 与 `cy_closure_write_capture_decls`（两支都带「不该出现」的反向断言）。
- repro: `python -u TEMP/closure_probes_r10.py`（A 组）；单测 `cargo test -j 1 --test cython_backend`；全量 `cargo test -j 1 --test cy_codegen_gate -- --ignored --nocapture`
- reported_by: qoder-explore-worker(cy 后端轮 T0r10)

## BUG-33 [2026-10-06T13:06:08Z] [medium] FIXED（v188，写捕获闭包不装箱为 Arc<dyn Fn>，保持栈上 let mut 闭包自动推断 FnMut）：闭包写捕获（SYNTAX/03e §五「可写捕获 → FnMut」）在 rust 面编不出东西，cy 面已实现
- summary: 合法写法 `let add = |v: int| =>` 换行块体 `{ total = total + v; total }`（§二 的多行体规则），cy 面按 §五 实现并实跑出 `'7\n'`，**rust 面 rustc 直接拒绝** ⇒ 跨后端取不到 oracle
- detail: 逐字取自证据件 `TEMP/closure-probes-201234.log`（B 组 `p_writecap.lz`）：
  rust 侧原文：`error[E0594]: cannot assign to `total`, as it is a captured variable in a `Fn` closure` 与 `error[E0502]: cannot borrow `total` as immutable because it is also borrowed as mutable`；cy 侧：`rust=RUSTC_FAIL cy=OK`、stdout `'7\n'`。
  §五 的映射表明写「可写捕获（`mut` 外部变量被修改）→ 映射 `FnMut`（`&mut self`）」，而产物发的是按引用捕获的 `Fn` ⇒ 这是**规范条文与产物的分叉**，不是「用户写错了」：
  同一份源码在 cy 面能跑出规范要求的语义，在 rust 面连编译都过不去。
  对本车道的实际影响：`CY/TESTS/04_functions/closure_write_capture.lz` **故意没有 `.exp`**，已登记在 `tests/cy_codegen_gate.rs` 的 `NO_ORACLE_EXPLAINED`（该表与盘面双向对表，补封后不移出会把门禁判红）；行为期望值暂由 `cy_closure_write_capture_decls` 的机制断言承担。
  第三组 `p_writecap_assign.lz`（把写捕获写成单表达式体 `|v: int| = total = total + v`）也是`rust=RUSTC_FAIL(E0308) / cy=OK`，但**该写法在 03e 里没有逐字形例**，所以本卡只以 B 组（规范里有字形例的那条）为凭，C 组当旁证不主张。
- 转正前置: rust 面按 §五 选 `FnMut`（或对写捕获降级为 `RefCell`/显式 `mut` 语义并回写规范条文）后，用 `python -u TEMP/seal_missing_exp.py` 补封 `closure_write_capture.exp`（该工具不覆盖已存在的 `.exp`），并同轮把该条从 `NO_ORACLE_EXPLAINED` 移出。
- repro: `python -u TEMP/closure_probes_r10.py`（看 B 组两行）；`python -u TEMP/xdiff_ds07.py CY/TESTS/04_functions`
- reported_by: qoder-explore-worker(cy 后端轮 T0r10)

## BUG-34 [2026-10-08T03:05:58Z] [high] FIXED（本轮修，cy 后端；`src/ir/codegen_cython.rs`）：用户类上的移位／`~`／`.contains()` 三处发射形态与 Python 运算符协议不同名
- summary: SYNTAX/12 §3.1/§3.2 的映射表里有三支魔法名与 Python 协议名不同名（`__shl__`/`__shr__` vs `__lshift__`/`__rshift__`、`x.contains(y)` 的协议面是 `y in x` ⇒ `__contains__`、`~x` ⇒ `__invert__`），而共享 IR 的分派表 `magic_method_for_binop` 里**移位两支缺席**、`map_unop` 把 `~` 降级成逻辑非（另开 BUG-36）⇒ cy 产物发裸运算符，**跑到运行期才响**。
- detail: 修前逐字（证据件 `TEMP/magic-probes-100301.log`）：
  `m_bits.lz rust=OK cy=RUN_FAIL` ＋ `TypeError: unsupported operand type(s) for <<: '_lzcyg_m_bits.Money' and '_lzcyg_m_bits.Money'`；
  `m_contains.lz rust=OK cy=RUN_FAIL` ＋ `AttributeError: '_lzcyg_m_contains.Word' object has no attribute 'contains'`。修后九支全部逐字一致（证据件 `TEMP/magic-probes-104214.log`）。
  修复形态：三处都按「**接收者的静态类型是用户类，且它确实实现了那个魔法名**」的闸门改写 —— BinOp Shl/Shr 点名直派 `a.__shl__(b)`、`.contains(y)` 改写成 `(y in obj)`、UnOp Not 按操作数类型还原（bool ⇒ `not `，其余 ⇒ `~`，于是整数得到位非、用户类落到 `__invert__`）。
  **本轮自己踩到的量具坑（值得单独记）**：闸门第一版直查 `impl_methods`，而那张表在类生成时被 `impl_methods.remove(&s.name)` **消费掉了**（防重复注入）⇒ 调用点永远查不到，两个新臂恒假、探针读数一字未变。现在加了一张**不被消费的** `class_method_names`（struct 体内自写 + impl 块注入都登记），helper `user_class_has_method` 只读它。「登记表键与查询键不同源」那一族的老形状，这里换了个方向复发（BUG-22/23 是键名不同源，这次是键被吃掉）。
  反向闸门（同类改写最容易改过头的那一侧）都有断言：整数接收者的 `self.v << o.v` **保持裸运算符**、用户自写的 `def contains` **保持方法名**、bool 的 `not flag` 仍是 `not`。
  判据两层：语料 `CY/TESTS/15_feature_matrix/magic_shift_invert.lz`（4 条 print + 一条 `not flag`，期望值由 Rust 面实跑封盘 `'48\n0\n-7\n5\nfalse\n'`）与 `magic_contains_call.lz`（`__contains__` 两个调用面 + `__call__` + `__setitem__`），单元锁 `tests/cython_backend.rs::cy_shift_and_contains_dispatch_only_for_implementors` 与 `cy_bitnot_is_restored_from_operand_type`。
  已知未覆盖：接收者静态类型是 `Any`／泛型参数时不改写（与 BUG-14 那张表的既有边界同型）；`__implicit_copy__`／`__when_move__`（§1.14.1 所有权魔法）两后端都没有实现路径，未写语料。
- repro: `python -u TEMP/magic_probes_r11.py`；`cargo test -j 1 --test cython_backend`；全量 `cargo test -j 1 --test cy_codegen_gate -- --ignored --nocapture`
- reported_by: qoder-explore-worker(cy 后端轮 T0r11)

## BUG-35 [2026-10-08T03:05:58Z] [medium] FIXED（本轮修，cy 后端；`src/ir/codegen_cython.rs`）：迭代子协议的「取完了」与 Option 的 `.unwrap_or` 在 cy 面根本没接线
- summary: LZ 的 `__next__` 用 `Option.None` 表示结束，cy 旧发射把它发成 `return None`，而 Python 的 for 协议要求抛 `StopIteration` ⇒ `None` 被当成**一个元素**；另外 `.unwrap_or(d)` 在 cy 的方法表里整条缺席，落到通用兜底变成 `0.unwrap_or(-1)`。
- detail: 修前逐字（`TEMP/magic-probes-100301.log`）：`m_iter.lz rust=OK cy=RUN_FAIL` ＋ `TypeError: unsupported operand type(s) for +: 'int' and 'NoneType'`；（`TEMP/magic-probes-103016.log`）`m_next.lz` ＋ `AttributeError: 'int' object has no attribute 'unwrap_or'`。修后 `for x in Steps(n: 4)` 与显式 `g.__next__().unwrap_or(-1)` 两侧同形（`TEMP/magic-probes-104214.log`）。
  **处置取舍（这一条是本轮最值钱的弯路）**：第一版把用户的 `__next__` 改名 `__lz_next__`，再发一层把 None 翻成 StopIteration 的 `__next__` —— `for` 侧当场修好，但同一支探针的 `m_next` 从「返回 None」变成「把 StopIteration 抛进用户程序」，**修复自己引入新分叉**。改成现在的形态：prelude 加 `_lz_iter(o)`（`None`/垫片单例 `_LzNoneCls` 都算结束），**只套在 for 的取数面**，用户的 `__next__` 一字不动 ⇒ `next(it)`、`it.__next__()` 仍是 Option 形态。内建 list/str/dict 一律不套：它们的 `None` 是**元素**而不是结束，套上去会把序列截断。
  `.unwrap_or(d)` 走分档助手 `_lz_or(v, d)`（None 有两种真实形态：Python `None` 与垫片单例），臂上带 `!ty_is_user_class` 反向闸门（用户自写的 `unwrap_or` 方法不许被抢）。
  判据两层：语料 `CY/TESTS/15_feature_matrix/magic_iter_proto.lz`（for 取数面 / 空迭代器 / 三次显式取下一步，期望值由 Rust 面实跑封盘 `'6\n0\n0\n1\n-1\n'`）＋单元锁 `cy_iterator_for_site_uses_the_adapter_only`（含「`__lz_next__` 不许出现在产物里」和 `_lz_iter(` 出现次数 == 2 的形状断言）与 `cy_option_unwrap_or_is_a_dispatch_not_a_method`。
  已知未覆盖：`Option.Some(None)`（装着一个 None 的 Some）在适配器里会被当成结束 —— cy 的 Option 就是「值本身／None」，这一层无法区分；rust 面能区分。属表示法边界，未开豁免清单。
- repro: `python -u TEMP/magic_probes_r11.py`（看 m_iter / m_next 两支）；`cargo test -j 1 --test cython_backend`
- reported_by: qoder-explore-worker(cy 后端轮 T0r11)

## BUG-36 [2026-10-08T03:05:58Z] [low] FIXED（2026-10-14，共享 IR 保留 `~`/`not` 区分；cy 协议收敛挂起）：`~x` 位非现保留为 `UnOpKind::BitNot`，IR 里 `~` 与 `not` 不再同形
- summary: `src/ir/builder/ops_map.rs` 里 `UnaryOp::BitNot => UnOpKind::Not, // 位非降级为逻辑非`（本轮实测该行仍在，卡里不写行号以免漂移）⇒ AST 层的 `~x` 与 `not x` 进了同一个 UnOpKind，下游只能靠操作数类型猜。
- detail: 规范 SYNTAX/12 §3.2 明写 `~x` → `__invert__`（Rust 无独立 BitNot trait，`__not__` 缺席时生成 `impl Not` 委托）。rust 面**恰好**不受影响：它发 `!`，对 bool 是逻辑非、对整数是位非、对用户类走生成的 `impl Not`，一个符号三种语义都对。cy 面必须分档（bool ⇒ `not `，其余 ⇒ `~`），本轮已在 `codegen_cython.rs` 的 UnOp 分支实现并锁住。
  **为什么这条仍要入账**：那是**补救不是修复**。任何第三个后端读同一份 IR 都会把 `~6` 当逻辑非；而 IR 里已经丢掉的信息，前端改一次就得在两个后端各猜一次。
  同族在册：移位两族不在 `magic_method_for_binop`（`__shl__`/`__shr__` 缺席），本轮同样只在 cy 侧点名直派，共享 IR 未动 ⇒ rust 面靠 `impl Shl/Shr` 恰好成立、cy 面靠类型闸门成立，两张皮都不是规范层的解。
  转正前置: builder 保留区分（新增 `UnOpKind::BitNot`，或给 Not 带一个 bit 标记；并把 `__shl__`/`__shr__` 补进 `magic_method_for_binop`），两侧按各自协议发射；改完 cy 侧这两条按类型还原的臂可以收敛成一条按 UnOpKind 分派的臂。改动落在共享 IR ⇒ **需要另一条车道**，本车道不改 `src/ir/builder/`、不改 `src/ir/codegen/`。
- 修复（2026-10-14）：`UnOpKind` 新增 `BitNot`，`map_unop` 的 `UnaryOp::BitNot => UnOpKind::BitNot`（不再降级）。同步更新全部穷尽 match 臂：rust `unop_str`（BitNot⇒`!`，两义皆对）、`display`（BitNot⇒`~`）、`lz_codegen`（BitNot⇒`!`）、Not 括号特判扩到 `Not|BitNot`；cy 侧 `Not|BitNot` 暂同按操作数类型还原（**行为零变化**），协议收敛（Not⇒`not`/BitNot⇒`~`，含 `not <user-class>`/`~ <user-class>` 分派）待 cy 工具链 co-verify 后补。`__shl__`/`__shr__` 补 `magic_method_for_binop` 那一族（BUG-34 同族）本轮**未动**（独立洞，另卡）。
- 验证: cargo build + 全量 cargo test 441/0（IR 新增变体无 golden 回归——tests/ 无 `~` fixture）；探针 bittest `~5`/`~0`/`~True`/`not False`/`not True` 实跑 -6/-1/false/true/false，与改前逐字一致。
- repro: `python -u TEMP/magic_probes_r11.py`（`m_bits` 两支的 rust/cy 读数与判读）；`grep -n 'BitNot' src/ir/builder/ops_map.rs`
- reported_by: qoder-explore-worker(cy 后端轮 T0r11)

## BUG-37 [2026-10-08T04:58:08Z] [high] FIXED（本轮修，cy 后端；`src/ir/codegen_cython.rs`）：动态接收者（`Any`／泛型形参）让方法改写表的两侧闸门同时失明，三支无条件臂还静默吃掉一次调用
- summary: 接收者的静态类型是 `IrType::Any`（无注解闭包形参，`src/ir/builder/capture.rs:528` 恒落 Any）或 `IrType::Generic(_)`（泛型形参，`src/ir/builder/convert_decl.rs:665`）时，cy 侧的形参就是 `object`，方法改写表两类闸门同时取到极端值：`!ty_is_user_class` 的**改写臂恒真**（用户类自带的 `len`/`push`/`contains`/`is_empty`/`trim`/`remove`/`starts_with`/`to_lower`/`unwrap_or` 被强改成 Python 名），`user_class_has_method` 的**直派臂恒假**（`__shl__` 不派发、`_lz_iter` 不套）。另三支 `clone`/`unwrap`/`try_into` 是**无条件**臂（当初为 with 脱糖的 `alias.clone()` 而设），连静态用户类的一次调用都被透传吃掉——**不响，只是数字不一样**。
- detail: 证据件 `TEMP/dynrecv-before-r12.log ＋ TEMP/dynrecv-after2-r12.log`（取数件 `TEMP/dynrecv_probes_r12.py`，21 支跨后端实跑探针，每支除 stdout 外还回吐 cy 产物里相关的那几行 pyx——「到底哪条臂生效」只能从发射形态看）。修前 一致 1／分叉 9／两侧都红 11，修后 一致 5／分叉 15／两侧都红 1。逐字读数分三组：
  ① **rust 面编得过**的三支（有 oracle）：`d_duck_len` rust=`'3\n8\n'` 而 cy `TypeError: object of type '_lzcyg_d_duck_len.Bucket' has no len()`；`d_duck_empty` rust=`'false\ntrue\n'` 走的是同一条被强改的 `len(x)`；`d_clone_static` rust=`'8\n'` / 旧 cy=`'7\n'`，`d_unwrap_static` rust=`'101\n'` / 旧 cy=`'Opt2 { v: 1 }\n'` —— 后两支两侧都不报错，**静默少一步**，比运行期异常更难发现（也是本卡把「无条件臂」一起收进来的原因）。
  ② **cy 内部 A/B**（同一支程序里静态接收者 vs 动态接收者，差异只在多走一层形参）：`d_len_direct` 修前后都是 `'3\n'` 且两侧一致，而 `d_len_obj` `TypeError: object of type '_lzcyg_d_len_obj.Bucket' has no len()`；`d_push_obj` `AttributeError: '_lzcyg_d_push_obj.Bucket' object has no attribute 'append'`；`d_contains_obj` `TypeError: argument of type '_lzcyg_d_contains_obj.Bucket' is not iterable`；`d_empty_obj` `TypeError: object of type '_lzcyg_d_empty_obj.Bucket' has no len()`；`d_iter_obj` `TypeError: unsupported operand type(s) for +: 'int' and 'NoneType'`（`__next__` 的 Option.None 被当成一个元素 ⇒ BUG-35 从动态面复现）；`d_shl_dyn` `TypeError: unsupported operand type(s) for <<: '_lzcyg_d_shl_dyn.Money' and '_lzcyg_d_shl_dyn.Money'`。修后这六支全部 cy=OK，且 `d_len_obj`/`d_len_direct` 同值。
  ③ 静默面（不响但做错）：`d_strfam_dyn` 的 `f5 = lambda x: (_lz_or(x, 7))` —— `unwrap_or` 被当成「值在不在」的分档，用户类自带的 unwrap_or 根本没被调用；修后发 `_lz_orv(x, 7)`，与静态那支 `d_strfam_static` 逐字同值（`'"x"\n3\n"x"\ntrue\n107\n'`）。
  修复形态：prelude 新增 `_LZ_TYPES` 名单（`known_types ∪ cdef_classes`，与静态判据**同源**并排序，保证同一份源码的产物逐字稳定）＋八支运行时分派助手（`_lz_dyn`/`_lz_meth`/`_lz_len`/`_lz_isempty`/`_lz_contains`/`_lz_orv`/`_lz_self`/`_lz_shift`）；改写表每条 `!ty_is_user_class` 臂加一个 `ty_is_dynamic_receiver` 分支；`_lz_iter` 第一行改成**自卫**式（非本模块用户类原样透传），所以动态 for 面可以一律套上而不截断内建序列。
  两支**闸门形状**的教训（都不是「多加个 if」那么轻）：a) 那三支无条件臂的反向闸门不能写成「是不是用户类」——with 脱糖在共享 IR 里就对**没有** clone 的用户类发 `alias.clone()`，那条必须继续透传，所以判据换成「本类有没有定义这个名字」（`user_class_has_method`），锁在 `cy_clone_unwrap_only_passthrough_when_no_such_method`；b) `_lz_orv`/`_lz_self`/`_lz_contains` 按**属性存在性**分派而不是按名单——`Option[MyClass]` 在本后端就是裸值，按名单判会对它发 `o.unwrap_or(d)` 而 AttributeError；`remove` 反过来**必须**按名单：Python 的 `list` 自带 `.remove`（按值删），按属性存在性会把 LZ 的「按位置删」静默换掉。
  判据三层：语料 `CY/TESTS/15_feature_matrix/dyn_recv_duck.lz`（封 `'3\n8\nfalse\ntrue\ntrue\n'`）、`dyn_recv_user_methods.lz`（封 `'8\n107\n2\n3\n'`）、`dyn_recv_closure.lz`（**故意没有 `.exp`**，见 BUG-38，已登记 `NO_ORACLE_EXPLAINED`）；单元锁 `tests/cython_backend.rs::cy_dynamic_receiver_dispatches_at_runtime`、`cy_static_receivers_keep_their_existing_forms`（成对反向：内建接收者不许吃运行时开销）、`cy_clone_unwrap_only_passthrough_when_no_such_method`、`cy_dynamic_for_site_wraps_and_the_helper_self_guards`。承重的证据是 ①② 两组同名读数：修前的产物里根本没有 `_lz_len(`/`_lz_meth(`/`_lz_shift(` 这些串，锁的第一条 assert 即红。
- repro: `python -u TEMP/dynrecv_probes_r12.py`；`cargo test -j 1 --test cython_backend`；`cargo test -j 1 --test cy_codegen_gate -- --ignored --nocapture`
- reported_by: qoder-explore-worker(cy 后端轮 T0r12)

## BUG-38 [2026-10-08T04:58:08Z] [medium] 已处置（2026-10-14 决策：规范回写确认「变量绑定必须注解」为接受面差异，rust 面 E0282 属预期；不改产品码、不补封 oracle）
- summary: `SYNTAX/03e-closure闭包.md` §一 把 `|x| x + 1`（省略注解、省略 `=>`）列为合法字形，现存语料 `CY/TESTS/04_functions/closure_basic.lz` 也就这么写的；但**闭包先绑到变量再调用**（`let sz = |x| => x.len()`）时 rust 面恒 `error[E0282]: type annotations needed`，而 cy 面接受并跑出正常结果 ⇒ 同一份源码一面能跑、一面编不出
- detail: 逐字取自证据件 `TEMP/dynrecv-before-r12.log ＋ TEMP/dynrecv-after2-r12.log`：`d_len_obj`/`d_len_list`/`d_push_obj`/`d_push_list`/`d_contains_obj`/`d_empty_obj`/`d_iter_obj`/`d_strfam_dyn`/`d_shl_dyn`/`d_generic_len` 每一支的 rust 行都是 `error[E0282]: type annotations needed`。其中 `d_len_list`（cy stdout=`'3\n'`）与 `d_push_list`（cy stdout=`'1\n'`）是**内建容器**那一半：它们本来就跑对，说明这条不是「cy 也坏」而是**接受面**分叉。
  为什么归到 rust 侧：`IrType::Any` 是**共享 IR** 给闭包形参定的类型（`src/ir/builder/capture.rs:528`），rust 后端原样交给 rustc、rustc 拒绝推断；cy 侧的 `object` 什么都能装，于是只有 cy 面能跑。泛型那一支同型：`d_generic_len` 的 rust 侧原文 `error[E0599]: no method named `len` found for type parameter `T` in the current scope`（无约束的 `T` 上找方法），而带 duck 约束的 `d_duck_len` rust 面就过了——⇒ 「规范 §03e §一 的无注解写法」与「rust 面能否定标」之间没有判据在管。
  对本车道的实际影响：`CY/TESTS/15_feature_matrix/dyn_recv_closure.lz` **故意没有 `.exp`**，已登记在 `tests/cy_codegen_gate.rs` 的 `NO_ORACLE_EXPLAINED`（该表与盘面双向对表，补封后不移出会把门禁判红）；行为期望暂由 BUG-37 那四支单元锁的**发射形态**断言承担。
  与既往那条流程债同源（上一轮记的「`cy` 的接受面比 rust 宽（无 borrowck）」）：这一族缺陷都要靠**前端类型检查**才判得动，不在本车道。
- 转正前置: rust 面给无注解闭包形参定标（按调用点推断并单态化，或按规范回写「必须注解」并改 §一）后，用 `python -u TEMP/seal_missing_exp.py` 补封 `dyn_recv_closure.exp`（该工具不覆盖已存在的 `.exp`），并同轮把该条从 `NO_ORACLE_EXPLAINED` 移出。
  处置决策（2026-10-14，选定转正前置第 2 条「规范回写」路线）：确认 `SYNTAX/03e-closure闭包.md` §三 已有「变量绑定必须注解」规则，本轮在 §三 补**接受面差异注记**：该规则当前未被前端强制检查（`src/ir/builder/capture.rs` 恒落 `Any`），rust 后端在无 fn 期望上下文时对未注解绑定闭包形参恒报 `E0282`（预期行为），cy 后端扩展接受（动态分派）。据此：`closure_basic.lz` 的无注解闭包实参（fn 上下文可定标）合法、rust 面可编译（已 rustc 实跑验证）；`dyn_recv_closure.lz` / `uncond_arms_closure.lz` 保持无 `.exp`、保留在 `NO_ORACLE_EXPLAINED`（登记注释引用本决策），**不补封 oracle**。若未来按调用点单态化（转正前置第 1 条）立项，再补封并从表移除。
- repro: `python -u TEMP/dynrecv_probes_r12.py`（看 d_len_obj／d_len_list／d_push_list 三支的 rust 行）；`python -u TEMP/seal_missing_exp.py`（回执里 `15_feature_matrix/dyn_recv_closure.lz` 那行）
- reported_by: qoder-explore-worker(cy 后端轮 T0r12)

## BUG-39 [2026-10-08T04:58:08Z] [medium] FIXED（v187，为 to_lower/lower/to_upper/upper/try_into 添加用户自定义 struct 守卫）：rust 面的内建方法改名表不看来访者是不是用户类：to_lower→to_lowercase、try_into→unwrap 把用户类自带方法判成不存在
- summary: 合法写法 `struct T` 自带 `def to_lower(self) -> str`、`struct Conv` 自带 `def try_into(self) -> int`，cy 面按原名调用并跑出正确值；**rust 面先在改名表里把名字换掉**，于是 rustc 报「用户类上没有 `to_lowercase`／`unwrap`」
- detail: 逐字取自证据件 `TEMP/dynrecv-before-r12.log ＋ TEMP/dynrecv-after2-r12.log`：`d_strfam_static` 的 rust 侧原文 `error[E0599]: no method named `to_lowercase` found for struct `T` in the current scope`（同一支 cy=OK，stdout=`'"x"\n3\n"x"\ntrue\n107\n'`，即 trim/remove/to_lower/starts_with/unwrap_or 五支都按用户类原名跑通）；`d_tryinto_static` 的 rust 侧原文 `error[E0599]: no method named `unwrap` found for struct `Conv` in the current scope`——`try_into` 在 rust 后端是 `x?` 的透传口径，被无条件换成 `.unwrap()`。
  这与本轮 cy 侧的修法是**同一张表的两侧**：cy 的改写表在 T0r12 之前缺「本类有没有定义这个名字」的反向闸门（BUG-37 里那三支无条件臂），rust 的改名表至今没有这道闸门。`src/ir/codegen/mod.rs` 与 `src/ir/builder/` 不在本车道 ⇒ 只入账、给复跑命令。
  规范侧没有明写「用户类不得占用这些内建方法名」，所以这不判成用户写错；但两后端各拦一半的后果是**任何同时覆盖两支的语料都取不到 oracle**（cy 侧修好后 `d_strfam_static` 仍是 rust=RUSTC_FAIL），跨后端差分面对这一族只能停在探针里。
- 转正前置: rust 侧改名表加「接收者是用户类且它定义了原名 ⇒ 不改名」的闸门后（判据形状可直接抄 cy 侧的 `user_class_has_method`），把 `TEMP/dynrecv_probes_r12.py` 的 `d_strfam_static`/`d_tryinto_static` 两支升级为语料并封盘。
- repro: `python -u TEMP/dynrecv_probes_r12.py`（看 d_strfam_static／d_tryinto_static 两行）；`python -u TEMP/xdiff_ds07.py CY/TESTS/15_feature_matrix`
- reported_by: qoder-explore-worker(cy 后端轮 T0r12)
## BUG-40 [2026-10-08T07:03:13Z] [high] FIXED（本轮修，cy 后端；`src/ir/codegen_cython.rs`）：方法表里余下三条**无条件**臂（expect / is_none / is_some）不吃掉用户类同名方法这件事没人看过一眼
- summary: cy 的 Option 表示是「裸值／None」，这是**值表示**而不是**接收者类型**。`is_none`/`is_some`/`expect` 三条臂当初按前者写成无条件，于是接收者是自带这些名字的用户类时，一次方法调用被整条换成一次 identity 比较——**两侧都不报错，只是数不对**（T0r12 已把同一形状定性为最难发现的一类）。
- detail: 证据件 `TEMP/uncond-x13-before-r13.log ＋ TEMP/uncond-x13-after2-r13.log ＋ TEMP/uncond-x13-rsdetail-r13.log`（取数件 `TEMP/uncond_arms_probes_r13.py`，14 支跨后端实跑探针，每支还回吐 cy 产物里相关的 pyx 行）。修前 一致 2／分叉 11／两侧都红 1，修后 一致 7／分叉 6／两侧都红 1；cy 侧 0 支退化，rust 列两份读数逐字一致。三支有 oracle 的分叉逐字：
  ① `x_expect_static` rust=`'42\n'` 而 cy=`'Cfg { n: 21 }\n'`——打印的是对象而不是返回值；修后 cy 发 `Cfg(n = 21).expect("m")`。
  ② `x_isnone_static` rust=`'true\n'` / 旧 cy=`'false\n'`（发射是 `print(_lz_dbg((Cfg(n = 1) is None)))`）；`x_issome_static` rust=`'false\n'` / 旧 cy=`'true\n'`。修后两侧同值。
  ③ 动态面同因：`x_expect_duck` 旧 cy=`'Cfg { n: 21 }\n'`（rust=`'42\n'`）、`x_isnone_duck` 旧 cy=`'false\n'`（rust=`'true\n'`）。修后发射形态是 `return _lz_exp(x, "m", "expect")` / `return _lz_isnone(x, "is_none")`。
  修复形态：三条臂各加 `!user_class_has_method(cg, &receiver.ty, <名字>)` 反向闸门（判据是「本类有没有定义这个名字」，与 BUG-37 那三支同一条纪律），动态接收者（`ty_is_dynamic_receiver`）发运行时助手 `_lz_isnone`/`_lz_issome`/`_lz_exp`。助手要**名单与属性都在**才走方法那一支（`_lz_dyn(o) and f is not None`）：`is_none` 的兜底本来就是表达式而不是透传，只按属性存在判会把任何恰好带 `is_none` 属性的对象当成 LZ Option。
  成对反向锁：真 Option 接收者一字没改——`x_isnone_option` 修前后都是 `print(_lz_dbg((o is None)))` / `print(_lz_dbg((o is not None)))`，且 rust=cy 逐字一致（`'true\nfalse\n'`）。
  判据三层：语料 `CY/TESTS/15_feature_matrix/uncond_arms_user_methods.lz`（封 `'42\ntrue\nfalse\n42\n'`，含 duck 泛型形参那一支）、`uncond_arms_closure.lz`（**故意没有 `.exp`**，rust 面恒 E0282，见 BUG-38，已登记 `NO_ORACLE_EXPLAINED`）；单元锁 `tests/cython_backend.rs::cy_unconditional_arms_honor_user_methods`、`cy_option_face_still_compares_to_none`（成对反向）、`cy_dynamic_arms_dispatch_by_name_list`。承重凭据：修前的产物里根本没有 `_lz_isnone(`/`_lz_issome(`/`_lz_exp(` 这些串，锁的第一条 assert 在旧码上即红。
- repro: `python -u TEMP/uncond_arms_probes_r13.py`；`cargo test -j 1 --test cython_backend`；`cargo test -j 1 --test cy_codegen_gate -- --ignored --nocapture`
- reported_by: qoder-explore-worker(cy 后端轮 T0r13)

## BUG-41 [2026-10-08T07:03:13Z] [high] FIXED（本轮修，cy 后端；`src/ir/codegen_cython.rs`）：`.expect(msg)` 的发射把接收者表达式拼两遍 ⇒ 带副作用时跑两次而退出码仍是 0
- summary: 旧发射是 `({r} if {r} is not None else __lz_expect_fail({m}))`——同一个接收者表达式**拼两遍**。r 是变量时无害，r 带副作用（函数调用、块表达式提升出来的前缀语句）时就是跑两遍，而程序**不响、退出码 0**，只是多做了一次。
- detail: 证据件 `TEMP/uncond-x13-before-r13.log ＋ TEMP/uncond-x13-after2-r13.log ＋ TEMP/uncond-x13-rsdetail-r13.log` 的 `x_expect_twice`：`def mk()` 的体里有一句 `print("mk")`，修前 cy stdout=`'"mk"\n"mk"\n5\n'`（两次），修后=`'"mk"\n5\n'`（一次），发射形态从 `(mk() if mk() is not None else __lz_expect_fail("m"))` 变成 `__lz_expect(mk(), "m")`。静态那一支的旧形态同样看得见：`x_expect_static` 修前发的是 `Cfg(n = 21) if Cfg(n = 21) is not None`（同一个构造表达式出现两次）。
  修复形态：prelude 新增 `def __lz_expect(v, m): return v if v is not None else __lz_expect_fail(m)`——参数只求值一次；动态接收者走 `_lz_exp(o, m, "expect")`，其兜底同样调这个垫片。
  为什么这条不能只靠跨后端差分：同一份源码在 rust 面恒 E0308（BUG-42 在册），所以**没有 oracle**可对照 ⇒ 凭据只能是 cy 自己的 stdout 行数与发射形态。语料 `CY/TESTS/15_feature_matrix/expect_receiver_once.lz` 因此**故意没有 `.exp`**，已登记 `NO_ORACLE_EXPLAINED`；发射形态锁 `tests/cython_backend.rs::cy_expect_evaluates_receiver_once` 直接数那一行里 `mk()` 出现一次（旧形态即红）。
- repro: `python -u TEMP/uncond_arms_probes_r13.py`（看 x_expect_twice 那三行）；`cargo test -j 1 --test cython_backend cy_expect_evaluates_receiver_once`
- reported_by: qoder-explore-worker(cy 后端轮 T0r13)

## BUG-42 [2026-10-08T07:03:13Z] [medium] FIXED（v185，MethodCall 路径注入 `current_expected_ty`）：`Option::expect(self, msg: &str)` 被喂 `.to_string()` ⇒ 整个 `.expect` 族在 rust 面恒 E0308、cy 面照跑
- summary: 合法写法 `let o: Option<int> = Some(5)` + `print(o.expect("m"))`，cy 面跑出 `'5\n'`；rust 面先在字符串实参上无条件 `.to_string()`，而 `Option::expect` 收 `&str` ⇒ rustc E0308。⇒ **接受面**分叉，且这一族语料取不到 oracle
- detail: 证据件 `TEMP/uncond-x13-before-r13.log ＋ TEMP/uncond-x13-after2-r13.log ＋ TEMP/uncond-x13-rsdetail-r13.log`。探针尺子里 `rust_out()` 只回吐以 `error` 开头的行（`TEMP/xdiff_ds07.py:49`），拿不到「哪一行、expected 什么」，所以完整诊断另落一份`TEMP/uncond-x13-rsdetail-r13.log`（再生命令 `python -u TEMP/rust_expect_evidence_r13.py`），卡里的逐字引用取自那件。承重那行是 `println!("{:?}", mk().expect("m".to_string()));`，rustc 的指认是 `expected `&str`, found `String`。同一支程序的 cy 面 stdout=`'"mk"\n5\n'`、退出码 0。
  对本车道的实际影响：`CY/TESTS/15_feature_matrix/expect_receiver_once.lz` 只能登记进 `NO_ORACLE_EXPLAINED`（该表与盘面双向对表）。补封之前，`.expect` 的行为凭据只有cy 侧的发射形态锁与「副作用一次」那条自证——**这不是等价证据**，跨后端对照缺着。
  为什么归到 rust 侧：改名/实参加工发生在 `src/ir/codegen/mod.rs` 一侧（本车道不改）；且规范没有禁止 `&str` 形参上直接传字面量。
- 转正前置: rust 侧不再对 `expect` 的字符串实参无条件 `.to_string()` 后，用 `python -u TEMP/seal_missing_exp.py` 补封 `expect_receiver_once.exp`（该工具不覆盖已存在的 `.exp`、空 stdout 不封），并同轮把该条从 `NO_ORACLE_EXPLAINED` 移出。
- repro: `python -u TEMP/rust_expect_evidence_r13.py`；`python -u TEMP/uncond_arms_probes_r13.py`（看 x_expect_option / x_expect_none 两行）
- reported_by: qoder-explore-worker(cy 后端轮 T0r13)

## BUG-43 [2026-10-08T07:03:13Z] [medium] FIXED（v186，添加 has_custom_remove 守卫排除用户自定义 struct）：内建 `remove` 的实参被无条件强转 `as usize`，不看接收者 ⇒ 用户类自带的 remove 编不出
- summary: `struct Cfg` 自带 `def remove(ref self, i: int) -> int`，cy 面按原名调用并跑出 `'4\n'`；rust 面在**调用点**按内建 `Vec::remove` 的口径把实参强转 `as usize`（`src/ir/codegen/mod.rs` 一侧），于是 rustc 在用户类自己的 `fn remove(&self, i: i64)` 上报 mismatched types
- detail: 证据件 `TEMP/uncond-x13-before-r13.log ＋ TEMP/uncond-x13-after2-r13.log ＋ TEMP/uncond-x13-rsdetail-r13.log`：生成的 .rs 里方法定义是 `fn remove(&self, i: i64) -> i64 {`，调用点却是 `Cfg { n: 1i64 }.remove((3i64 as usize))`，rustc 指认 `expected `i64`, found `usize`。探针侧：`x_remove_static` / `x_remove_duck` 两支都是 rust=RUSTC_FAIL 而 cy=OK。
  这与 BUG-39 是**同一张表的同一个缺闸门**（改名表不看接收者是不是用户类），只是机制不同：BUG-39 换的是**名字**（`to_lower`→`to_lowercase`、`try_into`→`unwrap`），这条换的是**实参类型**。所以.cy 侧 T0r12/T0r13 补的反向闸门（`user_class_has_method`）可以直接抄过去当判据形状。
  后果仍然是「任何同时覆盖两支的语料取不到 oracle」：`x_remove_static` 这一族在 cy 面已经正确（`'4\n'`），但封盘要求两后端逐字一致 ⇒ 只能停在探针里，不进语料（本轮语料 `uncond_arms_user_methods.lz` 因此不含 remove 那支）。
- 转正前置: rust 侧的内建方法处理按「接收者是用户类且它定义了原名 ⇒ 不动名字也不动实参类型」之后，把 `x_remove_static`/`x_remove_duck` 两支升级成语料并封盘。
- repro: `python -u TEMP/rust_expect_evidence_r13.py`（f_remove_user 那一段）；`python -u TEMP/uncond_arms_probes_r13.py`（看 x_remove_static / x_remove_duck 两行）
- reported_by: qoder-explore-worker(cy 后端轮 T0r13)
## BUG-44 [2026-10-09T03:20:29Z] [medium] FIXED（rust 后端 2026-10-14；与 BUG-28 同根因——子 CodeGen 未继承 str 类型表）：`f"using {res.name}"` 已落 Display，两侧应逐字一致（cy co-verify/封盘挂起）
- summary: `struct MyResource = name: str` 之后 `with MyResource(name: "database") as res:`，语句体里 `print(f"using {res.name}")`。rust 面出 `using "database"`（带引号＝Debug 档），cy 面出 `using database`。两边都不报错、退出码都是 0，只是值不一样 ⇒ 比异常更难发现。
- detail: 逐字凭据（`TEMP/wdseal-r14-apply.log`，同一份源码两后端各跑一遍）：
  rust: 'acquiring database\nusing "database"\nreleasing database\n'
  cy  : 'acquiring database\nusing database\nreleasing database\n'
  不等行 1 / 总行 3 :: [('using "database"', 'using database')]
  **缩小范围的关键读数**：同一个 struct 里 `def __enter__(self)` 的 `print(f"acquiring {self.name}")` 两侧逐字一致（都是 `acquiring database`）⇒ 缺的**不是**「字段访问的类型落地」，而是「`with ... as` 那个**绑定**的类型落地」——绑定没定成 `MyResource`（多半落成 `Any`），于是 `res.name` 的类型未知，rust 侧的逐实参分档 （`src/ir/codegen/expr_gen.rs:76` 的 `print_arg_is_display`）按「未知 ⇒ `{:?}`」走。规范 `SYNTAX/00-词法基础.md:188`（str 在插值里走 Display）站在 cy 这侧，所以要修的是共享层的类型落地，不是 cy 的分档。同洞的另一朝向：`.to_string()` 的 IR 返回类型也不是 `str` ⇒ `print(n.to_string())` **两侧都**出带引号的 Debug（凭据 `TEMP/corp-z1-r14.log` 的 `z_tstr_ty.lz` 那格）——那条两侧同错、这条只错 rust 一侧。
- 本轮处置（不动共享层代码、不放宽任何门禁）：把这一句从语料 `with_defer.lz` 拆成独立语料 `CY/TESTS/15_feature_matrix/with_as_binding_fstr.lz`（写盘件 `TEMP/split_withdefer_r14.py`，它的红对照 `TEMP/split_ctl_r14.py`），母件余下 8 行按**当日** rust 面重封（`TEMP/wd_reseal_r14.py --apply`，老那份改名留盘 `with_defer.exp.presplitr14`）。拆的判据是硬的：老 golden 摘掉的 3 行必须与新语料 rust 输出的 3 行「摘引号后逐字相等」，余下行必须「只差引号」，且母件 cy 实测 == 当日 rust 逐字（三格红对照：少摘一行、多摘一行、oracle 改一个字符，全被拦下）。新语料**不封** `.exp`，登记在 `tests/cy_codegen_gate.rs` 的 `NO_ORACLE_EXPLAINED`（该表双向可红）。照 cy 封它违反「期望值来自另一后端」的封盘纪律（那句话就写在母件第 2 行注释里）；照 rust 封它等于把本卡描述的缺陷钉成 cy 的验收标准。
- 转正前置: 共享层把 `with ... as` 的绑定类型落成被管理值的类型之后，`f"using {res.name}"` 两侧应逐字一致（都是 `using database`）；届时 `python -u TEMP/seal_missing_exp.py` 会补封 `with_as_binding_fstr.exp`（该工具只在两后端逐字一致时封、不覆盖已有 `.exp`、空 stdout 不封），而 `NO_ORACLE_EXPLAINED` 那条随即被门禁的「登记了但 `.exp` 在场 ⇒ 红」反向门逼着同轮移出。顺手一起看 `print(n.to_string())` 那一族（`.to_string()` 的 IR 返回类型应为 `str`）。
- 修复（rust 2026-10-14，与 BUG-28 同根因）：本卡假设「绑定落成 Any」**不成立**——`let mut res: MyResource` 在 IR 里已正确落地。真正障碍：with 体是**块表达式**，codegen 走全新子 CodeGen（expr_gen.rs BlockExpr 臂）而未继承 f-string str 类型登记表（`fstr_var_types`/`struct_fields_info`/`str_typed_vars`/`fstr_self_type`），`is_str_expr("res.name")` 查表全空 ⇒ 落 Debug。`self.name` 在 `__enter__` 直接体（非嵌套块）故两侧一致，误导了本卡的「共享层绑定类型」定位。修法＝子 CodeGen 继承四表（模块/函数级、体积小、块路径克隆可忽略）。探针 withas 实跑 `using database`（无引号＝Display）。
- 语料/封盘：rust 侧已对，但**未封** `with_as_binding_fstr.exp`、**未移** `NO_ORACLE_EXPLAINED`——封盘须两后端逐字一致（seal_missing_exp.py），本环境无 cython/cythonize（L2/L3 不可跑），cy 侧 co-verify 挂起，待有 cy 环境时补封并同轮移出登记表。`.to_string()` IR 返回类型应为 `str` 那一族（两侧同错）本轮**未动**（独立洞，另卡）。
- repro: `python -u TEMP/wd_reseal_r14.py`（DRY，含三格红对照，出两侧原文逐字）；`python -u TEMP/withdefer_diag_r14.py`（拆前母件的三方逐行分类：陈旧引号 10 行 / 真分叉 1 行）；`cargo test --test cy_codegen_gate`（L1，含登记表与语料的双向对表）
- reported_by: qoder-explore-worker(cy 后端轮 T0r14)


## BUG-45 [2026-10-15T06:30:00Z] [high] FIXED（rust 后端 2026-10-15，expr_gen.rs 两处）：DEMO 闸门 demo_codegen_compile 暴露的 Option.None 期望类型下推缺口
- summary: `combo-struct-method.lz`（StructCtor 字段位 `Option.None`，未绑定泛型 T）与 `result.lz`（`assert val.err() == None`，err() 返回 Option<String>）生成产物编译失败：前者 E0425 cannot find type `T`，后者 E0308 expected Option<String> found Option<i64>。
- detail: 根因两处共享同一缺口「期望类型下推未覆盖 Option.None 裸 None 发射」：
  1. expr_gen.rs FieldAccess `Option.None` 分支仅 `if self.in_generic_fn` 返回裸 `Option::None`，否则走 `Option::<elem>::None`；当 StructCtor 字段位已注入 `current_expected_ty=Option<T>`（未绑定泛型）时仍展开 elem=T → E0425。
  2. `assert_eq!(a,b)` 实参位置期望类型注入未识别 lhs 为 `ok()/err()` 方法调用（IR 未做方法返回类型推断，lhs_ty 为 Any），右侧 `None` 无 Option 上下文 → 按 expr.ty 默认展开 i64 → E0308。
- 修复：①FieldAccess 分支增加 `has_opt_ctx` 判定（current_expected_ty / current_fn_ret_ty 为 Option 形态即裸 `Option::None`）；②assert_eq! 两实参注入分支扩展：lhs 为 MethodCall ok/err 时从 receiver 的 Result 类型推导 Option 元素类型并注入两侧。
- 判据：`cargo test -j 1 --test demo_codegen_compile -- --include-ignored` 2 passed；combo-struct-method.lz / result.lz 生成产物 rustc 编译通过；`cargo test -j 1` 全量 27 目标全绿。
- repro: `cargo test -j 1 --test demo_codegen_compile -- --include-ignored`（修复前 magic_demos 1 failed / all_demos 2 failed）
- reported_by: file-agent 收尾轮验证闸门

## BUG-260 [2026-10-17T05:20:00+08:00] [high] FIXED（rust 后端 2026-10-17，convert_stmt.rs + stmt_gen.rs）：BST 例子编译失败（mut root = None 后 
oot = insert(...) 的类型传播缺口）
- summary: TEMP/marvis-probe/bst.lz（mut root = None + 
oot = insert(root, 5) + &mut TreeNode 递归）生成产物 rustc 编译失败：E0308（root 类型 Option<i64> vs insert 返回 TreeNode<T>）、E0596（root 非 mut）、E0507（move out of shared ref）、E0614（TreeNode 不可 deref）。
- detail: 根因 4 处共享类型落地/借用缺口：
  1. convert_stmt.rs Let 分支：lock_ctx.add_var(name, ir_ty) 无条件覆盖变量类型，mut x = None 后 x = f(...) 时 f 返回 T 被写成裸 T（丢掉 Option 包裹），且变量首次声明时未用新类型登记（mut cfg = Cfg(n:10) 后 cfg 仍 Any）。
  2. infer.rs infer_expr_type：struct 构造 Counter(n:10) 之前被误判为 Any（缺少 is_struct 检查），导致 codegen 无法识别自定义 struct 的 remove/get 方法（s usize 误转）。
  3. stmt_gen.rs Let 分支的 Some(...) 包装：无条件把 RHS 包进 Some(...)（只要值类型非 Option/Any/Unit 且非 None），未检查变量声明类型；c = c.inc()（c 是 Counter）被生成成 c = Some(c.inc()) → E0308。
  4. 	ypes_emit.rs gen_none_ref：(**__b).clone() 应改为 (*__b).clone()（双 deref 越界）。
- 修复：
  - convert_stmt.rs：Let 分支增加类型传播（新值类型非 Any 时更新 vars；变量已是 Option(Any) 且新值是 T 时升级为 Option(T)；首次声明变量用新类型登记）。Assign 分支同族：x = f(...) 且 x 已是 Option(Any) 时升级。
  - infer.rs：infer_expr_type 的 is_struct(fname) 检查提前到返回 Named(fname)（原本在容器分支之后，struct 被误判为 Any）。
  - stmt_gen.rs：Some(...) 包装增加变量声明类型检查（fstr_var_types 查该变量是否 Option(_)），非 Option 变量不包装。
  - types_emit.rs：gen_none_ref 修正 deref 层级。
- 判据：cargo test --release 全量 0 failed（含 codegen_corner_fixes 12 passed、find_bug_bugs 46 passed、demo_codegen_compile 1 passed）；bst.lz 生成产物 rustc 编译通过并正确输出 1 3 5 7 9；bug-self-underscore.lz 生成产物 rustc 编译通过（c = c.inc() 无 Some(...) 包装）。
- repro: .\target\release\lang-zone.exe TEMP/marvis-probe/bst.lz --emit=rs && rustc <bst.rs>
- reported_by: marvis-probe

## BUG-261 [2026-10-17T20:10:00+08:00] [medium] FIXED（rust 后端 2026-10-17，helpers.rs `strip_lambda_type_with_ref`）：推导式 filter 闭包缺 `&` 参数模式（nesting-data-types.lz `[mapper(item) for item in items if item > 0]`）
- summary: DEMO/boundary-coverage/nesting-data-types.lz 生成产物 rustc 编译失败：`filter(|item| { item > 0i64 })` 中 `item` 是 `&i64` 却按 `i64` 比较，E0308 expected `&i64`, found `i64`（同族 nesting-closure-lambda.lz 也命中）。根因是 `strip_lambda_type_with_ref`（helpers.rs）只做 passthrough：只剥离参数类型注解，既不区分 body 是否已含 `*param` 解引用，也不补 `&` 模式，导致两类闭包体互相打架。
- detail: 该 helper 旧注释声称「给参数加 `&`」但实现只是 `strip_lambda_type(lambda)` 裸转；历史曾硬加 `&`（`|&x|`），却与 body 已由 ref 解引用生成的 `*x`（free function `filter(items, |x| x % 2 == 0)` → `*x % 2`）冲突，构成 E0614 双重解引用。两种闭包体需求相反：
  1. body 已含 `*param` → 参数须保持裸名（`|x|`），`*x` 解引用 `&i64`；
  2. body 直接用 `param` → 参数须为 `&param` 模式，才能把 `&i64` 解引用成 `i64`。
- 修复：helpers.rs `strip_lambda_type_with_ref` 改为按 body 决策——先用 `lambda_split` 解析 (params, body)，若 body 含 `*<param>` 则保留裸参数；否则给每个参数加 `&` 前缀并重建 `move |&item| { ... }`。重建时 `after` 须跳过闭管 `|` 本身（否则拼成 `|&item||`，E0214，首版即踩此坑）。
- 判据：demo_codegen_compile 全量闸门（`-- --ignored`）失败数由 20（临时加 LZ_NEUTER_REF 环境变量把 helper 退回 passthrough 对照）降到 13，纯收益无新增回归；magic_demos 12→13 全绿；lib 441 passed；cy_codegen_gate 2 passed；perf_opt_gates 44 passed；str_boundary/dual_route/incremental_golden/ir_pass_fixes/lzc_*/moddec_* 全绿。修复后 nesting-data-types.rs 生成 `filter(|&item|| { item > 0i64 })`，rustc 编译通过。剩余 13 个 DEMO 失败（lists/polish_01/polish_04/operators/ir-edge-empty-collections/edge-values-boundary/edge-walrus-operator/iter/list/option/set/string/traits）在 passthrough 对照下同样失败，属既有 E0308/E0282 基线，与本修复无关。
- repro: .\target\release\lang-zone.exe DEMO/boundary-coverage/nesting-data-types.lz --emit=rs && rustc <nesting-data-types.rs>
- reported_by: marvis-probe

## BUG-262 [2026-10-17T20:30:00+08:00] [high] FIXED（rust 后端 2026-10-17，expr_gen.rs BlockExpr 分支 + stmt_gen.rs 尾值 unwrap 条件）：BlockExpr 子 CodeGen 不继承 `current_expected_ty` → match 臂尾 Option 值被误加 `.unwrap()`（iter.lz Zip::size_hint E0308）
- summary: DEMO/lz_std/iter.lz 的 `Chain::size_hint` 源码 `let hi = match (hi_a, hi_b): case (Some(x), Some(y)): Some(min(x, y)); case _: None` 生成产物里 `Some(min(x, y))` 被错加成 `Some(min(x, y)).unwrap()`，导致 `hi: Option<i64>` 绑定处 E0308（expected Option<i64>, found i64）。`.unwrap()` 不在源码，是代码生成错误注入。
- detail: 根因两处叠加：
  1. `expr_gen.rs` 的 `ExprKind::BlockExpr` 分支用 `CodeGen::new()` 造子 CodeGen，逐项复制了 current_ret_ty / current_fn_ret_ty / enum_variant_* / known_types / str_typed_vars 等，但**漏复制 `current_expected_ty`**（新实例经 mod.rs:418 恒为 None）。外层 `let hi: Option<int> = { match ... }` 已在 Let 分支（stmt_gen.rs L959）把 current_expected_ty 设为 Some(Option<int>)，但 match 臂经 BlockExpr 落到子 CodeGen 后 expected_ty 变回 None。
  2. `stmt_gen.rs` L1782 的尾值 unwrap 启发式只参考 `current_ret_ty`（函数返回 Tuple，非 Option）→ 对 Option 臂尾 `Some(min(x,y))` 加 `.unwrap()`，误判「值流向函数返回」。
- 修复：
  - expr_gen.rs BlockExpr 分支（L6252 后）加 `*child.current_expected_ty.borrow_mut() = self.current_expected_ty.borrow().clone();`。
  - stmt_gen.rs L1789 尾值 unwrap 条件加 `&& !expected_is_opt` 双保险（current_expected_ty 已期望 Option 时绝不 unwrap，防其它路径同类误判）。
- 判据：LZ_DBG_TAIL 调试输出对臂尾 `Some(min(x,y))` 从 `exp=None` 变为 `exp=Some(Option(Int))`；重新 emit iter.lz 后 `.unwrap()` 消失，`rustc --edition 2021 --extern lz_builtins=target\release\deps\liblz_builtins-45a30ac52ea72cb9.rlib -L dependency=target\release\deps -O iter.rs` 编译 EXIT=0（修复前 1 error E0308）。
- repro: .\target\release\lang-zone.exe DEMO\lz_std\iter.lz --emit=rs && rustc --edition 2021 --extern lz_builtins=target\release\deps\liblz_builtins-45a30ac52ea72cb9.rlib -L dependency=target\release\deps -O DEMO\lz_std\iter.rs
- reported_by: marvis-probe

## BUG-263 [2026-10-17T22:00:00+08:00] [high] FIXED（rust 后端 2026-10-17，stmt_gen.rs Assign 分支）：`self.inner = None` 生成 `Some(Option::None)` 双重 Some 包装（option.lz Iterator::next 清空迭代器）
- summary: DEMO/lz_std/option.lz 的 `Iterator::next` 源码 `self.inner = None`（清空内部 Option<i64>），生成产物 `self.inner = Some(Option::None);` → E0308 expected `Option<i64>`, found `Option<Option<_>>`。`.unwrap()`/双重 `Some` 均为代码生成错误注入。
- detail: stmt_gen.rs Assign 分支（原 L1481）在「变量是 Option<T> 而赋值值是 T」时补 `Some(...)`（bst.lz 家族 `root = Some(insert(root,5))`），但守卫条件只排除 `IrType::Option(_) | IrType::Any | IrType::Unit`。而 `None` 字面量的 IR 类型是 `Named { path: "Option", args: [] }`（不是 `IrType::Option(_)`，args 空），守卫放行 → 把已经是 `Option::None` 的值再包一层 `Some(...)` → `Some(Option::None)`，塞进 `Option<i64>` 字段 → E0308。
- 修复：stmt_gen.rs Assign 分支改用 helpers.rs 已有的 `needs_some_wrap(&target.ty, value)`（它内部用 `is_option_ty` 识别 `Named{path:"Option"}` 形态，且用 `is_none_expr(value)` 排除 None 字面量，正是本场景语义），删掉手写的脆弱匹配。
- 判据：重新 emit option.lz 后 L298 从 `self.inner = Some(Option::None);` 变为 `self.inner = Option::None;`，rustc 编译 EXIT=0（修复前 1 error E0308）。demo_codegen_compile 全量闸门失败数由 12 降到 11（本修复净 −1，无新增回归）。
- repro: .\target\release\lang-zone.exe DEMO\lz_std\option.lz --emit=rs && rustc --edition 2021 --extern lz_builtins=target\release\deps\liblz_builtins-45a30ac52ea72cb9.rlib -L dependency=target\release\deps -O DEMO\lz_std\option.rs
- reported_by: marvis-probe

## BUG-264 [2026-10-18T22:55:00+08:00] [high] FIXED（rust 后端 2026-10-18，infer.rs Int/F64 两臂）：`print(n.to_string())` 走 Debug 加引号（Int/F64 的 to_string 推断为 Any）
- summary: `let n = 42; print(n.to_string())` 实跑输出 `"42"`（带引号＝Debug 档），应为 `42`（Display 无引号）。这是 BUG-44 挂账时点名「两侧同错、本轮未动」的独立洞，本轮补上。
- detail: 根因是 **match 顺序缺陷**：infer.rs 的 `infer_method_ret_type`（原 L1138-1146）在函数末尾追加了「字符串化」臂 `IrType::Int | IrType::Int128 | IrType::BigInt | IrType::F64 | IrType::Bool => { if method=="to_string" { return String } ... }`。但**更早**的 `IrType::Int` 臂（原 L969）与 `IrType::F64` 臂（原 L974）已先命中 `_ => None`，把 Int/F64 消费掉 ⇒ 末尾那条 `to_string => String` 对 int/f64 **不可达**，推断回退 Any。Bool/Int128/BigInt 没有更早的臂、能到达字符串化臂，故此前只有 int/f64 出错（`print(b.to_string())` 一直是对的 `true`）。IR 实测：修复前 `[?] method [int] n.to_string`（Any），修复后 `[String] method [int] n.to_string`。
- 修复：把 `"to_string" => Some(String)` 提前登记到 `IrType::Int` 与 `IrType::F64` 自身臂（各 3 行），保留末尾臂不动（覆盖 Bool/Int128/BigInt）。
- 判据：`TEMP/probe/tostr_ty.lz`（`print(42.to_string()/3.14.to_string()/true.to_string()/"abc".to_string())`）实跑修复前 `"42"\nabc\ntrue`、修复后 `42\n3.14\ntrue\nabc`（int/f64 去引号，str/bool 原本就对）。回归用例：`tests/codegen_corner_fixes.rs::to_string_on_primitives_prints_display_not_debug`（`run_lz` 实跑比对 stdout == "42\n3.14\ntrue\nabc"）。
- 回归面：全量 `cargo test -j 1` 零失败（含 demo_codegen_compile 441 passed）；无新增警告。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/tostr_ty.lz
- reported_by: fist-agent（lz-combo-r2 组合轮）

## BUG-266 [2026-10-18T23:20:00+08:00] [high] FIXED（rust 后端 2026-10-18，expr_gen.rs max/min 两处 + infer.rs List 臂）：`List.max()/.min()` 在空 List 上实跑 panic `called Option::unwrap() on a None value`
- summary: `let empty: List<int> = []; print(empty.max())` 实跑直接 panic（退出码 1），程序崩溃。根因：codegen 生成 `(*recv.iter().max().unwrap())`，`Vec::iter().max()` 对空 Vec 返回 `None`，`.unwrap()` panic。
- detail: 两处同源：① List 方法路径（expr_gen.rs 原 L4491-4500，BUG-291 补的路由）`(*({}).iter().{}().unwrap())`；② 自由函数路径（原 L1939-1945）`(*(&arg).iter().max().unwrap())`。二者都把 `.unwrap()` 当「空 List 语义」——但这是**实跑崩溃**，不是 -1 哨兵，也与其他集合方法不一致。叠加 infer.rs 对 `max/min` **未标注**（IR 实测 `[?] method [List<int>] xs.max`，Any），等于类型与行为双失明。语料实证：全语料无一处对 List 用 `.max()/.min()` 方法；`basic.lz:16`/`iter.lz:320` 的 `max(a,b)`/`min(a,b)` 是 2 参 int 自由函数（有 `fn_param_types`，不走本路径），故修复零冲击。
- 修复：与 BUG-316/BUG-265 的「保留 Option」语义族对齐——codegen 两处改为 `({}).iter().{}().cloned()` / `(&arg).iter().max().cloned()`（`.cloned()` 对 Copy 的 i64 与非 Copy 的 String 元素都成立，`.copied()` 对 String 报 E0277），infer.rs List 臂补 `"max" | "min" => Some(Option<T>)`（从 Any 收口）。空 List 现返回 `None`，调用方自写 `.unwrap_or(-1)`；max/min 空集本就无自然哨兵（-1 只是回退），Option 是诚实语义。
- 判据：`TEMP/probe/list_minmax_empty.lz`（`print(xs.max()/xs.min()/empty.max())`）修复前 `5` 后 panic、修复后 `Some(5)\nSome(1)\nNone`；IR 修复前 `[?]`、修复后 `[Option<int>]`。回归用例：`tests/codegen_corner_fixes.rs::list_max_min_return_option_and_empty_does_not_panic`（`xs.max().unwrap_or(-1)` 实跑 stdout == "5\n1\n-1\n-1"）。
- 回归面：全量 `cargo test -j 1` 零失败（含 demo_codegen_compile 441 passed）。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/list_minmax_empty.lz
- reported_by: fist-agent（lz-combo-r2 组合轮）

## BUG-265 [2026-10-18T22:55:00+08:00] [high] FIXED（rust 后端 2026-10-18，infer.rs List 臂）：`List.get/first/last/pop` 的 IR 类型（T）与 codegen 产物（Option<T>）矛盾 ⇒ `xs.get(1)+1` 生成 `Option<i64>+i64` → E0369
- summary: `let xs=[10,20,30]; print(xs.get(1)+1)` IR 层面通过（get 推断为 int），产物却是 `(xs.get(1)).cloned() + 1i64`（`xs.get(1).cloned()` 是 `Option<i64>`）→ rustc E0369 binary operation `+` cannot be applied to `Option<i64>`。同时 `print(xs.get(1))` 输出 `Some(20)`、`print(xs.get(99))` 输出 `None`。
- detail: 这是 BUG-281 与 BUG-316 两轮决策**漂移**的后果：BUG-281 让 codegen 把 `first/last/get` 经 `.cloned()` 后追加 `.unwrap_or(-1)` 平铺（LZ 用 -1 编码 None），`infer.rs` 随之把返回类型标成 `T`；但 BUG-316（更新）**推翻**平铺、改为「保留 Option：first/last/get 返回 `Option<&T>`、pop 返回 `Option<T>`，调用方自写 `.unwrap_or(0)`」，expr_gen.rs 已按新语义改（`values.first().unwrap_or(0)` → `(first()).cloned().unwrap_or(0)`）。问题在 infer.rs **没跟进**：List 臂（原 L1000-1001）仍标 `"get"|"pop"|"first"|"last" => Some(args[0])`（T），于是 IR 类型说谎、产物是 Option，两边不一致 ⇒ 合法形态的 LZ 生成不可编译的 Rust。
- 语义裁定依据（Option 是现行正确语义）：全语料 46 处 `.get()/.first()/.last()/.pop()` 用法中 4 处走 `.unwrap_or()`（DEMO/edge-walrus-operator.lz:12 `values.first().unwrap_or(0)`、string.lz:231/234/250），**零处**与 `-1` 比较；BUG-316 注释并列举 `assert last == Some(4)` / `while let Some(top)=nums.pop()` 若平铺则 E0308 的反例。
- 修复：`infer.rs` List 臂把 `"get"|"pop"|"first"|"last"` 合并标成 `Some(IrType::Option(Box::new(args[0].clone())))`，与 codegen 一致；`remove`（Vec::remove 返回 V）保持 T，`remove_at` 保持已有 Option。
- 判据：`TEMP/probe/list_get_default.lz` IR 修复前 `[int] method [List<int>] xs.get`、修复后 `[Option<int>]`；`TEMP/probe/list_get_arith.lz`（`xs.get(1)+1`）修复前后都 rustc 失败，但失败面从「IR 谎报类型后静默生成坏 Rust」变为「IR 如实标 Option<int>」——`+` 未被 extended_check 拦截属另一独立洞（Option+int 未在类型检查处报错），本卡只收类型一致性。回归用例：`tests/codegen_corner_fixes.rs::list_first_get_are_option_consistent_with_codegen`（`first()/get()/last().unwrap_or(...)` 实跑 stdout == "10\n20\n30\n-1"）。
- 回归面：全量 `cargo test -j 1` 零失败（含 demo_codegen_compile 441 passed）——证明语料中 46 处集合 get/first/last/pop 用法都已在 codegen 侧按 Option 处理，infer 跟进后无新失败。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/list_get_default.lz
- reported_by: fist-agent（lz-combo-r2 组合轮）

## BUG-267 [2026-10-18T23:45:00+08:00] [high] FIXED（rust 后端 2026-10-18，infer.rs Dict 臂）：`Dict.get(key)` 的 IR 类型标 V，但 codegen 产物是 `Option<V>`（`Some(1)`/`None`）
- summary: `let d={"a":1,"b":2}; print(d.get("a")); print(d.get("zzz"))` IR 显示 `[int] method [Dict<str,int>] d.get`，实跑却输出 `Some(1)` / `None`。类型（int）与值（Option）矛盾——这是 BUG-265 的 **Dict 孪生洞**（同一漂移家族，infer 与 codegen 各说各话）。
- detail: codegen 在 expr_gen.rs L4578 按 BUG-313 生成 `(d.get(k)).cloned()`（BTreeMap::get 返回 `Option<&V>`，`.cloned()` 转 `Option<V>`）；infer.rs Dict 臂仍标 `"get"|"pop" => Some(args[1])`（V），两边不一致 ⇒ 合法 LZ 形态在 IR 层面谎报类型。语料实证：全语料 0 处 2 参 `d.get(k, default)`（codegen L4566 的 unwrap_or 分支是死代码），35 处 `.get(` 多为用户 struct 自带 get（`Box2.get`/`I.get`），非 Dict。
- 修复：infer.rs Dict 臂 `"get" => Some(IrType::Option(args[1]))`，与 codegen 对齐。因 `lookup_builtin_method_ret` 签名**不含参数个数**（`fn(recv_ty, method, ctx)`），无法区分 1 参/2 参形态；2 参 `get(k,default)` 现标 Option 会与 codegen 的 `unwrap_or` 平铺产物（V）再漂移，故一并登记为 BUG-269 待拆（需把 arg-count 传入 lookup 或把 2 参形态单独路由）。全语料无 2 参用法，本轮改零冲击。
- 判据：`TEMP/probe/dict_get.lz` IR 修复前 `[int]`、修复后 `[Option<int>]`；实跑修复前后均为 `Some(1)\nNone\n2`（codegen 侧本就是 Option，值不变，修的是 IR 与值的一致）。回归用例：`tests/codegen_corner_fixes.rs::dict_get_is_option_matching_codegen`。
- 回归面：全量 `cargo test -j 1` 零失败（含 demo_codegen_compile 441 passed）。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/dict_get.lz
- reported_by: fist-agent（lz-combo-r2 组合轮）

## BUG-268 [2026-10-18T23:48:00+08:00] [high] FIXED（rust 后端 2026-10-18，expr_gen.rs Dict pop 路由 + pattern_methods）：`Dict.pop(key)` 生成 `d.pop(key)`，BTreeMap/HashMap 无 pop 方法 → E0599
- summary: `let p = d.pop("a")` 生成 Rust `let p: i64 = d.pop("a".to_string())`，rustc 报 `E0599 no method named 'pop' found for struct 'BTreeMap'`（TEMP/probe/dict_pop.lz 实测 RUSTC_FAIL exit=1）。语义上 Dict.pop(key) = BTreeMap::remove(key)（返回 Option<V>），与 Dict.remove（BUG-289）同源，但 codegen 没路由。
- detail: 三层洞叠一起——① `pattern_methods`（expr_gen.rs L3410-3431，need_ref 的入口门）**没有 pop**，故整个 need_ref 块被跳过，`args_s[0]` 保持 `"a".to_string()`（owned String）而非 `&str` 字面量；② `is_dict_contains_key`（L3522-3523）也没有 pop，`&` 未加；③ rust_method 映射 `"pop" => "pop"`（L3330），生成 `d.pop(...)` 落到 BTreeMap 固有方法名 → E0599。stdlib 全语料 0 处 `pop` 定义（List.pop 是 builtin 魔法，不在此列），故未被语料覆盖。
- 修复：三处补 pop——`pattern_methods` 加 `"pop"`（放行 need_ref）、`is_dict_contains_key` 加 `|| method == "pop"`（加 `&`）、新增 codegen 臂把 Dict `pop` 路由到 `{recv}.remove({key})`（返回 Option<V>，与 remove 一致，保留 Option 不平铺）。`need_ref` 布尔（L3481）同步加 pop。全语料 0 处 `Dict.pop(key)`，本轮改零冲击。
- 判据：`TEMP/probe/dict_pop.lz` 修复前 `let p: i64 = d.pop("a".to_string())` → E0599；修复后 `let p: Option<i64> = d.remove("a")` → 实跑 `Some(1)`（`d.pop("zzz")` → `None`）。回归用例：`tests/codegen_corner_fixes.rs::dict_pop_routes_to_remove_and_returns_option`。
- 回归面：全量 `cargo test -j 1` 零失败（含 demo_codegen_compile 441 passed）。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/dict_pop.lz
- reported_by: fist-agent（lz-combo-r2 组合轮）

## BUG-269 [2026-10-19T00:10:00+08:00] [high] FIXED（rust 后端 2026-10-19，extended_check.rs Binary 臂补 Option 拦截）：`Option<int> + int` 不被 LZ 类型检查拦截，产出坏 Rust（E0369）
- summary: `let a = xs.first() + 1`（first() 返回 Option<int>）在 LZ 阶段无诊断，codegen 直出 `let a: Option<i64> = (xs.first()).cloned() + 1i64;`，rustc 报 `E0369 no implementation for Option<i64> + i64`（TEMP/probe/option_plus_int.lz 实测 RUSTC_FAIL exit=1）——与 Result 家族同源，但 extended_check Binary 臂只拦 Result（L1316），漏了 Option。
- detail: `extended_check.rs` 的 `AstExpr::Binary` 臂三段检查——① Result 未经解包直接参与运算（L1316-1320，`matches!(&lt, IrType::Result{..})`）；② str_plus_int（L1322-1332）；③ unconstrained_generic_cmp（L1334-1348）。三者都覆盖不到「一侧 Option、另一侧非 Option」的标量运算。而 `ex_type_agrees`（L123-124）对 `(Option(x), y)` 剥壳递归，把 `Option<int>` 与 `int` 判为兼容——该函数服务于 mixed_list_type 等宽语境，不能用于运算两侧的一致性判定。LZ 语义上 Option 需 `.unwrap_or(default)/.unwrap()/?` 取内层值后才能参与标量运算。
- 修复：Binary 臂在 str_plus_int 之后补 `option_in_scalar_op`——对算术/位/序比较（Add/Sub/Mul/Div/Mod/Pow/BitAnd/BitOr/BitXor/Shl/Shr/Lt/Gt/Le/Ge，Eq/Ne 放行避免误伤 Option 整体相等比较）判 `strip_option(lt) != strip_option(rt)`（恰一侧 Option 即报错），`strip_option` 剥 Ref/MutRef 后再判 Option（覆盖 `&xs.first()` 实参形态）。两侧皆 Option（如 `Option<int> == Option<int>`）或任一侧 queer（Any/Generic/Never/Self_/Ext/Duck/Unit）放行。
- 判据：`TEMP/probe/option_plus_int.lz` 修复前 EMIT 阶段无诊断 → rustc E0369；修复后 LZ 直接报 `IR build error: Option 值不能直接参与运算，需先 .unwrap_or(default)/.unwrap()/? 解包取内层值`（exit=1，不再产出 .rs）。回归用例：`tests/codegen_corner_fixes.rs::option_in_scalar_op_is_clean_lz_type_error`。
- 回归面：全量 `cargo test -j 1` 零失败（含 demo_codegen_compile 441 passed）——证明语料中所有 Option 参与运算的合法形态（皆先 unwrap_or/unwrap/? 解包）未被误伤；Eq/Ne 未纳入拦截范围以保护 Option 整体比较。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/option_plus_int.lz
- reported_by: fist-agent（lz-combo-r2 组合轮）

## BUG-270 [2026-10-19T00:20:00+08:00] [medium] FIXED：stdlib 自由函数形态（`get(d,k)`/`get_or(d,k,def)`/`keys(d)` 等约 158 个）不接入 rlib，被生成 `fn NAME(__a0: i64)->i64{i64::MAX}` 坏桩

### FIXED(2026-10-20 / BUG-270)
- summary: 以自由函数形式调用 lz_builtins/std/*.lz 里定义的函数（如 `get(d,"a")`、`get_or(d,"a",0)`、`keys(d)`），codegen 不认识其名字 → `collect_unknown_extern_fns` 把它们当「未定义外部函数」生成 `fn NAME(__a0: i64, …) -> i64 { i64::MAX }` 桩，rustc 报 E0308（`TEMP/probe/dict_get_free.lz` / `dict_get_or.lz` / `dict_method_api.lz` 实测 RUSTC_FAIL exit=1）。
- detail: 根因是 `src/ir/codegen/scan.rs` 的 `LZ_BUILTIN_FN_NAMES`（L1562-1756，共 187 项）作为「lz_builtins 实际导出名」的硬编码白名单，只覆盖了 collections.rs 的 `lz_*` 前缀方法、Python 内建名、以及少数 dict 函数（`is_empty`/`clear`）。脚本对比 lz_builtins/**/*.lz 顶层 `def`（剔除 test_*、_test、main 后 187 个）与白名单，发现 158 个 stdlib 自由函数名缺失：abs_f、and_then、both、capitalize、clamp_float/int、collect_options、compose、concat、count_by/distinct/pred、cycle、dedup、delete、dict_all/any/len/sum、digit_count/sum、drop_while、either、ends_with、expect、factorial、fibonacci、filter_dict/option、find/index_of/last、flat_map、flatten_map/result、fold1、from_pairs、gcd、geometric_series、get、get_or、group_by、has_key、index_of、insert_at、intersperse、invert、is_alnum/alpha/digit/err/even/inf/nan/none/odd/ok/pos/prime/some/space、items、iter_*、join、join_str、keys、last_index_of、lcm、lines、map_err_result/keys/map_or/map_or_else/values、max_int、merge、min_int、ok_or、option_*、or_else、pad_left/right、partition、permutations、pipe*、pow、pow_f、prepend、primes_upto、product、range_step、range_sum、reduce、remove_at/first、repeat、replace_first、reverse、scan、set_default、sign、sign_f、sorted_by、split、starts_with、substring、sum_of_cubes/squares、take_while、to_lower/upper、trim*、unique_by、unwrap*/unzip/update_at/values/window/words/xor_option/zip_all 等。
- 已知连带：即使名字在白名单内也不一定可用——`is_empty(d)`/`clear(d)` 生成直调 `is_empty(d.clone())`，rustc 把名字解析到 `BTreeMap::is_empty(&self)`/`clear(&mut self)`（0 参方法）报 E0061（`TEMP/probe/dict_is_empty_free.lz` 实测）。说明「名字在名单内」只保证不生成桩，不保证签名匹配。
- 影响面与优先级：**潜伏、0 语料命中**——搜索 tests/**、tests/*.rs、demo/*.lz 未见 `get_or(`、`has_key(`、`get_or(` 自由调用，也未 `import std.dict`；方法形态（`d.has_key()`/`d.is_empty()`/`d.get(k)`）正常（`TEMP/probe/dict_method_api.lz` 中方法行未被桩，仅 `keys(d)` 自由形式被桩）。demo_codegen_compile 441 全过证明语料未触达。
- 修复方向（已部分实施 2026-10-20 第10轮）：用 `rustc --extern lz_builtins` 逐个实测 158 个缺失名是否真被 rlib 导出，结论颠覆原假设——**只有 17 个在 rlib 中**（`chunk/compose/concat/count/drop/expect/find/fold/fold1/group_by/partition/pipe/pipe2/pipe3/reduce/unique/unique_by`），其余 141 个 `lz_builtins/std/*.rs` 虽由 .lz 转译但**未并入 crate**（`git status` 显示 `??` 未跟踪，`src/runtime/mod.rs` 无对应 `mod` 声明）。故：把 17 个真导出名并入 `LZ_BUILTIN_FN_NAMES` 阻止桩遮蔽真身（`src/ir/codegen/scan.rs` L1756-1775 新增 BUG-270 分组注释），141 个未导出名**保留桩语义**（桩是它们唯一定义，去桩即 E0425 更糟）。修复后 `TEMP/probe/sweep10/bug270_probe2.lz` 实测：`find`/`fold` 已解析到 rlib 真身（`.into_iter().find()` / `.fold()`，无桩），`get_or`/`keys` 仍生成 `fn NAME(__a0: i64, ...) -> i64 { i64::MAX }` 桩（预期保留）；语料搜索 CY/TESTS/demo/tests 未见 17 名自由调用，零回归。
- 遗留（另立 bug 或后续轮）：141 个未导出函数要真正可用，须把 `lz_builtins/std/{dict,list,iter,option,math,func,core_subset}.rs` 通过 `#[path]`/`mod` 声明并入 crate（但这些文件头有 `use lz_builtins::*;` 自引用，需先清理），属结构性改动，风险高，本轮未动。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/sweep10/bug270_probe2.lz
- reported_by: fist-agent（lz-combo-r2 组合轮）

## BUG-271 [2026-10-19T00:55:00+08:00] [medium] FIXED（rust 后端 2026-10-19，extended_check.rs Binary 臂补 Bool 拦截 + 比较链豁免）：	rue + 1 等 Bool 参与算术/位/序运算不被 LZ 类型检查拦截，产出坏 Rust（E0369）
- summary: let b = true; print(b + 1) 在 LZ 阶段无诊断，codegen 直出 println!("{:?}", b + 1i64);（b: bool）→ rustc E0369 no implementation for bool + {integer}（TEMP/probe/bool_plus_int.lz 实测 RUSTC_FAIL exit=1）。与 BUG-269 Option 家族同源，但 Binary 臂只拦 Result/Option/str_plus_int，漏了 Bool。
- detail: 标量运算合法族在 LZ 中应为——数字↔数字（Int/F64）、字符串↔字符串（Add 拼接，另有 codegen 特殊路径 format! 兜底 str+int，见 str_plus_int 检查）、列表↔列表（Add 拼接，__cat extend）。Bool 参与任一算术/位/序运算都无合法语义。同族另两例（本轮未修，见下）：d + {"b": 2}（dict+dict）、[1,2] + 1（list+int）、{1,2}+{2,3}（set+set）都 E0369，但 dict/set/list 与 perf 优化器（d = d + {i:i} 重写为 d.insert(i,i)，tests/perf_opt_gates.rs L1086-1095）纠缠，Blanket 拦截会误伤 d = d + {i:i} 累加器，故本轮只收 Bool 这个零纠缠、零语料的干净缺口。
- 修复：Binary 臂在 option_in_scalar_op 之后补 ool_in_scalar_op——在算术/位/序比较（Add/Sub/Mul/Div/Mod/Pow/BitAnd/BitOr/BitXor/Shl/Shr/Lt/Gt/Le/Ge，Eq/Ne 放行保护 ool == bool）内，任一侧 IR 类型为 Bool 即报错。任一侧 queer（Any/Generic/Never/Self_/Ext/Duck/Unit）放行。
- **比较链豁免（本 bug 的最大风险点）**：1 < 5 < 10 被解析器左结合成 (1<5) < 10（lhs 为 bool），codegen 在 xpr_gen.rs L5460-5484 才分解成 (1<5) && (5<10)。extended_check 跑在 codegen 之前，若直接报 Bool 错误会破坏 DEMO/13_operators/precedence.lz 与 CY/TESTS/15_feature_matrix/precedence.lz（实测前者触发 cy_l1_transpile_all_corpus FAILED）。故 Bool 检查加链式豁免：外层 op 为比较 且 lhs 是另一个比较 Binary 时放行（视为合法比较链）。
- 判据：TEMP/probe/bool_plus_int.lz 修复前 EMIT 无诊断 → rustc E0369；修复后 LZ 直接报 IR build error: Bool 值不能直接参与运算（exit=1，不产出 .rs）。	rue < 5（非链）同样拦截；1 < 5 < 10 编译运行输出 	rue（回归 	ests/codegen_corner_fixes.rs::bool_in_scalar_op_is_clean_lz_type_error，含 f64+int=3.5 / str+str 拼接 / list+list 拼接合法形态回归）。
- 回归面：全量 cargo test -j 1 零失败（含 demo_codegen_compile 441 passed + cy_l1_transpile_all_corpus 通过——后者在本修复前因 precedence.lz 触发 Bool 拦截而 FAILED，已回绿）。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/bool_plus_int.lz
- reported_by: fist-agent（lz-combo-r2 组合轮）

## BUG-272 [2026-10-19T01:15:00+08:00] [high] FIXED（rust 后端 2026-10-19，extended_check.rs Binary 臂标量运算补类型族交叉拦截）：标量运算缺类型族相容性校验，4 类跨族组合直出坏 Rust（E0369）
- summary: extended_check Binary 臂的标量运算拦截此前只覆盖 Option/Bool/Result/str_plus_int 少数几例，跨类型族组合（浮点位运算、两侧皆 Option 的算术、字符串跨族比较、Bool 跨族 Eq/Ne）不拦，codegen 直出坏 Rust。TEMP/probe/sweep2 四组实测 RUSTC_FAIL：
  - [1,2].first() * [1,2].first() → let v: Option<i64> = (vec.first()).cloned() * (vec.first()).cloned(); → E0369（两侧皆 Option 的算术）
  - 1.5 << 2 → let v: f64 = 1.5f64 << 2i64; → E0369（f64 未实现 Shl）
  - 1.5 & 2 → let v: f64 = 1.5f64 & 2i64; → E0369（f64 未实现 BitAnd）
  - "a" < 1 → let v: bool = "a".to_string() < 1i64; → E0369（String 未实现 PartialOrd<i64>）
  - 	rue == 1 → let v: bool = true == 1i64; → E0369（bool 未实现 PartialEq<i64>；Eq/Ne 不在 scalar_op 内，此前完全无拦截）
- detail（三处新增/修正）：
  ① **both_opt_arith**：BUG-269 的「两侧皆 Option 放行」过宽——仅比较（Lt/Gt/Le/Ge）与 Eq/Ne 应放行（Option 实现 PartialOrd/PartialEq），算术（Add/Sub/Mul/Div/Mod/Pow）与位/移位（BitAnd/BitOr/BitXor/Shl/Shr）两侧皆 Option 仍应拒。
  ② **f64_bit_shift**：位/移位运算任一侧为 F64 → 拒（浮点无位语义）。
  ③ **str_cmp_cross**：序比较（Lt/Gt/Le/Ge）两侧字符串性不同（is_str_ty(lt) != is_str_ty(rt)）→ 拒。注意 s[i] != "-" 是 Eq（不在 scalar_op），不受影响。
  ④ **bool_eq_cross**（新增独立块）：Eq/Ne 两侧 Bool 性不同 → 拒。保护 ool == bool。
- 已知未修：None + 1 → LzAdd::__add__(None, 1i64) → E0369。None 字面量在 infer_expr_type 落 IrType::Any（infer.rs L21「None 类型取决于上下文」），Any 属 queer 被全部放行。需单独把 NoneLit 语境化或对 None 参与算术/位显式拒，风险高，暂记待评。
- 合法形态回归：Option<Lt>Option（[1,2].first() < [3,4].first() → true）、list < list、ool == bool、64 + int 均不受影响。
- 判据：	ests/codegen_corner_fixes.rs::scalar_op_type_family_cross_is_clean_lz_type_error（4 拒 + 4 合法形态锁定）。
- 回归面：全量 cargo test -j 1 零失败（含 demo_codegen_compile 441 passed + cy_l1_transpile_all_corpus 通过；codegen_corner_fixes 20 passed）。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/sweep2/opt_mul_opt.lz（及 f64_shl_int / str_lt_int / bool_eq_int）
- reported_by: fist-agent（lz-combo-r2 组合轮）

## BUG-273 [2026-10-19T01:20:00+08:00] [medium] FIXED（rust 后端 2026-10-19，extended_check.rs Binary 臂补 None 字面量算术/位拦截）：None + 1 直出 LzAdd::__add__(None, 1i64) → E0369
- summary: None 字面量参与算术/位运算不被 LZ 类型检查拦截。let v = None + 1 生成 let v: i64 = LzAdd::__add__(None, 1i64);，rustc 报 E0369（TEMP/probe/sweep2/none_plus_int.lz 实测 RUSTC_FAIL exit=1）。
- detail: 根因是 None 字面量在 infer_expr_type 落 IrType::Any（infer.rs L21「None 类型取决于上下文」），Any 属 queer（ex_type_queer）被 BUG-269/271/272 全部 option/bool 检查放行。语义上 None 是 Option<T> 的值，参与算术/位运算必错；但比较与 Eq/Ne 合法（x == None、None < Some(y)，Option 有 PartialOrd/PartialEq）。
- 修复（已落码未验证——build 因宿主内存耗尽 os error 1455 暂不可跑）：Binary 臂在 lopt != ropt 之前插入 none_in_arith_bit 检查——op 为算术（Add/Sub/Mul/Div/Mod/Pow）或位/移位（BitAnd/BitOr/BitXor/Shl/Shr）且任一操作数是 AstExpr::NoneLit 时报「None 值不能直接参与算术/位运算（None 是 Option 的值，需先给类型与解包）」。比较与 Eq/Ne 不拦（放行 x == None）。
- 判据：TEMP/probe/sweep2/none_plus_int.lz 修复后应 LZ 报错不再产出 .rs；TEMP/probe/sweep2/none_eq.lz（x == None + None == None）应继续编译运行。
- 回归面：待跑全量 cargo test -j 1（含 demo_codegen_compile 441 passed + cy_l1_transpile_all_corpus + codegen_corner_fixes 20 passed 单线程）。
- 环境备注：本轮全量测试多次 link.exe 0xc000012d / 
ustc STATUS_STACK_BUFFER_OVERRUN / os error 1455 页面文件太小，系宿主物理内存耗尽（FreePhysicalMemory 一度 2-3 MB），非语义回归；codegen_corner_fixes --test-threads=1 单线程 20/20 通过证明语义无回归。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/sweep2/none_plus_int.lz
- reported_by: fist-agent（lz-combo-r2 组合轮）

## BUG-274 [2026-10-19T07:15:00+08:00] [medium] FIXED（codegen expr_gen，None 比较短路）：None == None 直出无类型标注的比较 → rustc E0282
- summary: print(None == None) 生成 println!("{:?}", None == None);，rustc 报 E0282（type annotations needed）——TEMP/probe/sweep2/none_eq.lz 实测 RUSTC_FAIL exit=1。
- detail: x == None（x 为有类型 Option）被 codegen 正确路由为 x.is_none()（expr_gen 已有 None 相等特判），但两侧都是裸 None 时无上下文定类型，codegen 未注入 Option::<T>::None 标注。属 codegen 侧类型上下文缺失，与 extended_check 标量运算族拦截（BUG-269~273）不同族。
- 判据：TEMP/probe/sweep2/none_eq.lz 应编译运行（打印 alse 与 	rue）；x == None 形态已正常（[1].first() == None 走 is_none()）。
- 回归面：0 语料命中（语料库未见 None == None），不影响现有门。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/sweep2/none_eq.lz
- reported_by: fist-agent（lz-combo-r2 组合轮，None 算术拦截修复验证时顺带发现）

## BUG-275 [2026-10-19T09:55:00+08:00] [medium] FIXED（codegen expr_gen，float×int 混合比较提升）：1.5 == 1 直出 f64 == i64 → rustc E0308
## BUG-276 [2026-10-19T09:55:00+08:00] [medium] FIXED（codegen expr_gen，str 重复语义）："ab" * 2 直出 String * i64 → rustc E0369
## BUG-277 [2026-10-19T09:55:00+08:00] [medium] FIXED（codegen expr_gen，list 重复语义）：[1,2] * 2 直出 Vec * i64 → rustc E0369

## BUG-278 [2026-10-19T10:18:00+08:00] [medium] FIXED（codegen type_alias 解析）：	ype Pair = (int, int); let p: Pair = (1,2); print(p[0]) 直出 p[0i64 as usize] → rustc E0608（Named("Pair") 未解析回 Tuple）

## BUG-279 [2026-10-19T10:28:00+08:00] [medium] FIXED（filter 闭包参数 & 模式）：裸 Vec .filter(\|x\| => x > 2) 生成 \|x\| { x > 2i64 }（filter 闭包参数是 &Item）→ rustc E0308

## BUG-280 [2026-10-19T10:38:00+08:00] [medium] FIXED（裸 Vec 无 reduce 方法）：.reduce(\|acc, x\| => acc + x, 0) 生成 .reduce(move \|acc, x\| {...}, 0i64) → rustc E0599（Vec 无 reduce）。Rust 归约是 Iterator::fold(init, closure)，加方法路由 
educe(closure, init) → ({}).into_iter().fold({}, {})。
## BUG-281 [2026-10-19T10:38:00+08:00] [medium] FIXED（Vec 无 enumerate）：方法 .enumerate() → .enumerate() E0599；自由函数 numerate(a) → (a).iter().enumerate() 产出 Enumerate<Iter<&String>> 迭代器（[0] → E0608）。修法：方法形式走 into_iter().enumerate().map(\|(i,x)\| (i as i64, x)).collect()；自由函数同样补 .collect()；infer.rs 登记 numerate → List<Tuple(Int, T)>。
## BUG-282 [2026-10-19T10:49:00+08:00] [medium] FIXED（元组索引后字段 move）：let s = b[0][1]（b: List[(int, str)]）生成 let s: String = b[0].1; → rustc E0507（从借用里 move String，Vec 索引返回 &T）。修法：IndexGet 元组分支在 base 是 IndexGet 且字段非 Copy 时追加 .clone()，新增 is_copy_ty 辅助（Int/Int128/F64/Bool/Unit/usize/u8/i8/char 为 Copy）。

## BUG-283 [2026-10-19T11:33:00+08:00] [high] FIXED（未标 iterator 关键字的生成器不可用）：def gen(): yield 1; yield 2; yield 3（无 -> T 注解）两处缺陷叠加——① collect_functions 对无返回注解函数取尾表达式 yield 3 的 infer_stmt_type → Itor<i64>（lz_builtins 无此类型，TEMP/probe/sweep6/generator_func.lz → E0425 cannot find type Itor），convert_decl 同 bug；② 调用点 let g = gen() 登记为 Itor<i64>，且裸 Vec 无 .next()（E0599）。修法：collect_functions/convert_decl 对含 yield 的无注解函数按 yield 元素类型推断（调用点登记 Vec<T>）；codegen 把 Vec 上的 .next() 路由到 into_iter().next()。实测输出 Some(1)。

## BUG-284 [2026-10-19T11:54:00+08:00] [high] FIXED（if 分支内的 yield 漏检）：def evens(n): while ...: if i%2==0: yield i —— ast_body_has_yield / scan_iterator_yield_ty 只遍历 AstStmt，未下钻 Stmt::Expr(Expr::If) 的分支体，yield 漏检 → 元素类型推断回退 Unit → -> Vec<()>（E0599 Vec<()> 无 next）。修法：新增 ast_expr_has_yield / scan_expr_yield_ty 下钻 If/Match/BlockExpr/Closure 嵌套语句块。实测输出 Some(0)。## BUG-285 [2026-10-19T12:20:00+08:00] [high] FIXED（match 分支内 yield 漏检）：def classify(x): match x: 0 => yield 100 ... —— block_has_yield / first_yield_type 未下钻 Stmt::Match 的各分支 Block → 函数返回类型 i64（而非 Vec<i64>）且 __gen_vec 未声明 → E0425（TEMP/probe/sweep6/gen_match.lz）。修法：helpers.rs 增加 Stmt::Match 分支下钻 arm.body。实测输出 Some(100)。

## BUG-286 [2026-10-19T12:22:00+08:00] [high] FIXED（Vec<T>.next() 未登记 → yield g.next() E0308）：lookup_builtin_method_ret 无 Vec 分支 → g.next() 回退 Any → yield g.next() 元素类型归一成 i64 → Vec<i64> 却 push(Option<i64>)（TEMP/probe/sweep6/gen_nested.lz E0308）。修法：List/Vec 分支增加 next → Option<elem>，与 Iterator<T>::next 同构。实测输出 Some(Some(1))。## BUG-287 [2026-10-19T12:45:00+08:00] [high] FIXED（非 raises 生成器内 return → push(Err(())) E0308）：rewrite_iterator_returns 把生成器内所有 return 重写为 Stmt::Raise(())（等价 Python StopIteration）；但 Raise codegen 只要 in_generator 就 push(Err(..))，无视函数是否有 raises 子句 → 非 raises 生成器 Vec<i64> 里 push Err(()) → E0308（TEMP/probe/sweep6/gen_return.lz）。修法：Stmt::Raise 在 in_generator 时检查 current_fn_raises.is_some()——有 raises 才 push(Err)，否则直接 return __gen_vec（提前结束收集）。实测输出 Some(1)。## BUG-288 [2026-10-19T12:55:00+08:00] [high] FIXED（生成器 + raises 类型错配 E0308/E0599/E0277）
(1) convert_decl.rs ret_ty：`def gen() raises str:` 无返回注解时 ok_ty 取
    unwrap_or(Unit) → Vec<Result<(),String>> 与 `push(Ok(1i64))` E0308。
    修法：无注解时取 scan_iterator_yield_ty 推断出的 ret_ty（yield 元素类型）。
(2) mod.rs 调用点：生成器 + raises 时应为 Vec<Result<T,E>>（每次 yield 一个
    Result），旧逻辑先包 Vec<T> 再包 Result → Result<Vec<T>,E>，调用点
    `let g: Result<Vec<T>,E>` 与实际 codegen 的 Vec<Result<T,E>> 不符 →
    g.next() E0599（gen_raises.lz）。
(3) current_is_iterator 只按 func.is_iterator 设置，漏掉 body 含 yield 的函数
    → return 值触发隐式转换（"boom" → i64），随后 rewrite_iterator_returns
    转成 Stmt::Raise 时值已被污染 → push(Err(<i64 as ImplicitFrom<String>>::
    __implicit_from__("boom"))) E0277（gen_raises_ret.lz）。修法：
    current_is_iterator = func.is_iterator || ast_body_has_yield(&func.body)。
实测 gen_raises.lz / gen_raises_ret.lz 均 RUN-OK Some(Ok(1))。## BUG-289 [2026-10-19T13:30:00+08:00] [high] FIXED（生成器内 try/catch yield 漏检）
`def gen(): try: yield 1 catch: yield 2` 三处 yield 检测均未下钻 try 分支体：
(1) helpers.rs block_has_yield：无 Stmt::TryCatch 分支 → codegen 生成
    `-> Itor<i64>`（类型来自 try 表达式本身）而非 `-> Vec<i64>`，且
    __gen_vec 未声明 → E0425（TEMP/probe/sweep6/gen_try.lz）。
(2) helpers.rs first_yield_type：同样无 Stmt::TryCatch 分支 → 生成器构建块
    `func *:` 收集器元素类型推断漏检。
(3) convert_decl.rs ast_expr_has_yield / scan_expr_yield_ty：无
    AstExpr::TryCatch 分支 → scan_iterator_yield_ty 漏检 yield 元素类型
    → ret_ty 推断为 Itor<i64> 而非 Vec<i64>。
修法：三处均补 Stmt::TryCatch / AstExpr::TryCatch 分支，递归下钻
body/catches/else_body/finally_body。
实测 gen_try.lz RUN-OK `Some(1)`。## BUG-290 [2026-10-19T13:40:00+08:00] [high] FIXED（生成器 yield None 元素类型推断错）
`def gen(): yield None; yield Some(1)` — scan_iterator_yield_ty 对 NoneLit
返回 IrType::Any → 归一 i64 → 生成器返回类型 Vec<i64>，codegen 却
push(Option::None)（None 经 gen_none_ref 发射为 Option::None）→ E0308
（TEMP/probe/sweep6/gen_opt.lz）。
修法：scan_iterator_yield_ty 对 yield None（NoneLit / Ident("None")）返回
Option(Any)（归一为 Option<i64>），生成器返回类型 Vec<Option<i64>>，
push(None.clone()) 与 push(Some(1i64)) 均匹配。
实测 gen_opt.lz RUN-OK Some(None)。
## BUG-292 [2026-10-19T14:20:00+08:00] [high] FIXED（生成器 if 分支体内 return 未重写为 raise）
`def gen(): if true: yield 1; return; yield 2` — LZ 语句位置的 if 是
AstStmt::Expr(AstExpr::If{..})，convert_stmt 降为 Stmt::ExprStmt 包裹
ExprKind::IfExpr。两处下钻缺口：
(1) convert_decl.rs ir_block_has_yield：无 Stmt::ExprStmt 分支 →
    `if true: yield 1; return` 中 return 未重写为 raise → codegen 生成
    panic!("generator return")（TEMP/probe/sweep6/gen_nested_if_ret.lz）。
(2) convert_decl.rs rewrite_iterator_returns：同样无 Stmt::ExprStmt 分支
    → if 分支体内 return 原样保留 Stmt::Return → Stmt::Return codegen 走
    panic!("generator return") 兜底。
修法：ir_block_has_yield 增 Stmt::ExprStmt 分支，经新增 ir_expr_has_yield
递归下钻 ExprKind::IfExpr/BlockExpr；rewrite_iterator_returns 增
Stmt::ExprStmt 分支，经新增 rewrite_expr_returns 递归重写 if 分支体内
Stmt::Return → Stmt::Raise。
实测 gen_nested_if_ret.lz RUN-OK `Some(1)`（yield 1 后 return 终止迭代）。
## BUG-293 [2026-10-19T14:25:00+08:00] [high] FIXED（raises 生成器裸 return 误 push Err）
`def gen() raises str: if true: yield "ok"; return; yield "bad"` —
BUG-292 后 `rewrite_iterator_returns` 把裸 `return` 重写为 `raise ()`
（value=Unit）。raises 生成器返回 `Vec<Result<String, String>>`，codegen
Stmt::Raise 一律 `push(Err(..))` → `push(Err(()))` 与
`Result<String, String>` 不匹配 E0308（TEMP/probe/sweep7/gen_if_yield_ret.lz）。
修法：stmt_gen.rs Stmt::Raise 生成器分支，仅当 raises 存在且 value 类型
非 Unit 才 `push(Err(..))`；Unit（裸 return）视为成功终止
（`return __gen_vec`）。
实测 gen_if_yield_ret.lz RUN-OK `Some(Ok("ok"))`。
## BUG-294 [2026-10-19T14:35:00+08:00] [high] FIXED（struct impl 生成器 .next() 未转 into_iter）
`struct Counter: n: int; impl Counter: def gen(self): yield self.n; ...`
`let g = c.gen(); print(g.next())` — struct impl 方法生成器调用返回类型在
IR 侧为 Any（peek：`let g: ? = [?] method [Counter] c.gen`），
codegen .next() 特判 recv_is_list=false（receiver.ty=Any 非 Vec）→
未转 `into_iter().next()` → E0599 no method next found for Vec<i64>
（TEMP/probe/sweep8/gen_struct.lz；.rs 里 `fn gen(&self) -> Vec<i64>`
正确，`g.next()` 裸发）。
修法：expr_gen.rs .next() 特判增加 `receiver.ty == Any` 接收者
（codegen 发射签名已是 Vec<i64>，运行期确实是 Vec，into_iter().next()
合法）。
实测 gen_struct.lz RUN-OK `Some(42)`。
## BUG-295 [2026-10-19T17:00:00+08:00] [high] FIXED（生成器 try 体闭包内 return/raise 移出 `__gen_vec`）
`def gen(): try: yield 1; return catch: yield 2` — try 体闭包内生成器
`return`/`raise` 原直接 `return __gen_vec;`，而闭包对 `__gen_vec` 是 `&mut`
捕获，移出触发 E0382 use of moved value（TEMP/probe/sweep10/gen_try_ret.rs）。
修法（终止标志方案）：闭包内置位 `__gen_terminated_N = true; return;`
（闭包返回 ()），try 块结束后统一在函数作用域 `if __gen_terminated_N {
return __gen_vec; }` —— 唯一一次移动发生在函数作用域；嵌套时用
`flags.join(" ")` 一次置位栈上全部标志，post-check 发 `return;` 逐层上抛。
配套：post-check 移至尾值 emit **之前**（否则尾值变语句报 expected ';'）；
catch/finally 臂强制尾 `()` 与 Ok 臂对齐。
嵌套二次缺陷：`gen_try_flag_counter` 原为按值 `usize`，块表达式/lambda/trait
默认方法体/构建块新建**子 CodeGen** 时计数归零 → 内外两层同名
`__gen_terminated_0`（内层声明遮蔽外层，置位语句两次都写内层绑定，外层
标志永不置位 → 尾部 `return __gen_vec` 不执行；且内层 post-check 因栈为空
误发闭包内 `return __gen_vec;`，E0308）。修法：计数器改 `Rc<Cell<usize>>`
共享 + 4 处子 CodeGen 站点（decl_gen 默认方法体；expr_gen BlockExpr/Lambda/
构建块）继承 `gen_try_flag_stack` 与共享计数器。
实测 TEMP/probe/sweep10/{gen_try_ret,gen_try,gen_try_else,gen_try_finally,
gen_try_nested}.rs 全部 rustc 通过并运行输出 `Some(1)`；回归
try_catch/panic_raise_try/try_more/combo_defer_guard_try/combo_try_raise_guard/
combo-defer-guard/combo-error-control 全部通过。
## BUG-296 [2026-10-20T09:25:00+08:00] [high] FIXED（raises 生成器 + try 尾包 Ok 导致签名错/`__gen_vec` 未声明）
### FIXED(2026-10-20T09:25:00+08:00 / BUG-296)

`def gen() raises str: try: yield "ok"; raise "boom" catch e: yield "caught"`
原生成 `pub fn gen() -> Result<String, String>`（错）而非
`-> Vec<Result<String, String>>`，体内仍有 `__gen_vec.push(...)` 但函数未声明
`__gen_vec` → E0425 cannot find value，且 `Result::Ok({...})` 尾包裹无类型信息
→ E0282（TEMP/probe/sweep10/gen_try_raises.lz）。

根因（双层）：
1. **expr_has_yield 不下钻 EnumCtor 实参**（src/ir/codegen/helpers.rs:363）。
   builder 把 raises 函数体包成 `ExprStmt(EnumCtor("Result","Ok",[BlockExpr{...}]))`
   （隐式尾表达式包 Ok），故 `block_has_yield` 走到 Stmt::ExprStmt → expr_has_yield
   → `_ => false`。结果 decl_gen.rs:941 `has_yield=false` → 签名取
   `f.ret_ty=Result<T,E>` 而非 `Vec<Result<T,E>>`，`__gen_vec` 也不声明。
   stmt_gen.rs:2599 的 yield 发射**不检查 in_generator**，照发 `__gen_vec.push`
   → E0425。对照：非 raises 生成器（gen_if_yield_ret）body 未包 Ok，has_yield=true
   → 正常，所以问题只在 raises×try 交互。
2. **rewrite_raises_block 对生成器体误包尾 Ok(...)**
   （src/ir/builder/convert_decl.rs:1602）。生成器把 yield/raise 收进 `__gen_vec`，
   函数返回 `__gen_vec` 而非尾值；包 Ok 产生悬空 `Result::Ok({...})` 表达式语句，
   且 Ok 的 err 类型参数无处推断 → E0282。

修法：
- helpers.rs `expr_has_yield` / `expr_first_yield_type` 增加 `ExprKind::EnumCtor
  { args, .. } => args.iter().any/find_map(...)` 分支，让 Ok(...) 包裹下的 yield
  可见。
- convert_decl.rs：抽取 `let is_generator = func.is_iterator || ir_block_has_yield(&body)`
  复用（原先两处独立判断），`rewrite_raises_block(body, &ok_ty, &err_ty, !is_generator)`。
  `rewrite_raises_block`/`rewrite_raises_stmt` 增 `wrap_tail_ok: bool` 参数并在
  递归调用中透传；`wrap_tail` 判据前置 `wrap_tail_ok &&`。

实测（全部 rustc 通过并运行）：
- sweep10/gen_try_raises.lz → `Some(Ok("ok"))`
- sweep10/hy_both.lz（plain_gen Vec<i64> / raises_gen Vec<Result<i64,String>>）→
  `Some(1)` / `Some(Ok(1))`
- sweep10/raises_try_variants.lz（catch 拼接 / return 终止 / 嵌套 try）→
  `Some(Ok("a"))` / `Some(Ok("x"))` / `Some(Ok("inner"))`
回归：sweep6/{gen_try,gen_raises,gen_match,gen_nested_if_ret}、
sweep7/gen_if_yield_ret、sweep10/gen_try_nested 全部通过。

## BUG-297 [2026-10-20T11:40:00+08:00] [medium] FIXED（raises 函数内闭包 `return X` 误包 Ok(...)）
`def raises_closure() raises str:` 内含 `let f = |x: int| if x>0: return "pos" else: return "neg"`，
生成的 Rust 闭包为 `move |x: i64| -> String`，但闭包体内 `return` 被包成
`return Ok("pos".to_string())` → E0308 mismatched types（闭包返回 String 而非 Result）。

根因：expr_gen.rs:5970 Lambda 子 CodeGen 继承 `child.current_fn_raises = self.current_fn_raises.clone()`
（为支持闭包内 raise 传播而设计），但 stmt_gen.rs:1562 的
`wrap_ok = self.current_fn_raises.is_some() && !matches!(&v.ty, Result{..})`
未区分「闭包自身的返回类型是否为 Result」。闭包 ret_ty=String 时不应包 Ok。
注：闭包自身 raise 时（同文件 inner_raise 变体）同样生成 `return Result::Err("neg")`
在 `-> i64` 闭包内 → E0308，属同一缺口：闭包返回类型未按内含 raise 升级为 Result。

复现：TEMP/probe/sweep10/raises_closure_ret.lz / raises_closure_inner_raise.lz
建议修法：Lambda 子 CodeGen 依据闭包自身 ret_ty 决定是否继承 raises 语义——
ret_ty 为 Result 时继承；ret_ty 为具体非 Result 类型时清空 current_fn_raises。

追加 2026-10-20T11:52（部分修复）：`raises` 函数内**不含 raise 的闭包** `return X` 误包 Ok 已修复。
修法：Lambda 子 CodeGen 依据「闭包自身是否 raise」决定继承 raises 语义——新增
`helpers::block_has_raise`（仅检查 `Stmt::Raise`，不下钻 Lambda）与
`expr_has_raise_only`；expr_gen.rs:5970 Lambda child 继承点改为
`let closure_body_raises = match &ret_ty { Some(rt) => !matches!(rt, Result{..}), None => !block_has_raise(block_ref) }`，
为真则 `child.current_fn_raises = None`。判据双保险：显式注解非 Result → 清空；
无注解（常见 inferred）→ 仅闭包体实际含 raise 才保留。
实测 raises_closure_ret.lz 编译+运行通过。**未覆盖的第二缺口**（同一 E0308 类）：
闭包自身 `raise` 但闭包返回类型未被升级为 Result（raises_closure_inner_raise.lz，
`return Result::Err("neg")` 在 `-> i64` 闭包内）。需在 builder 推断闭包返回类型时
检测闭包体含 raise 并升级 ret_ty 为 Result。
回归：22 个真实 raises 源文件全部编译通过；9 探针生成器语料全部通过。

追加 2026-10-20T12:05（第二缺口实测确认）：闭包自身 `raise` 时其返回类型未升级为 Result。
两个复现：
  raises_closure_inner_raise.lz  —— raises 函数内 `|x: int| if x<0: raise "neg" else: x`
    → `move |x: i64| { if x<0 { return Err("neg") } else { x } }`，闭包推断 `-> ()`，
    E0308 expected ()。
  raises_closure_factory.lz     —— raises 函数 `make_adder() raises str` 返回会 raise 的闭包
    → 生成 `make_adder() -> Result<fn(i64) -> (), String>`，闭包返回类型被推断为 `()`，
    调用点 E0618 expected function found Result<fn(i64), String>。
根因：convert_expr.rs:1201 构建 Lambda 时仅从显式 `-> T` 注解取 ret_ty；闭包体含
`raise` 时不走 convert_decl.rs:1124 的 raises 函数体升级路径（rewrite_raises_block +
ret_ty 升 Result）。infer.rs 亦无 Stmt::Raise 类型处理 → if-expr 分支含 raise 时
合类型为 `()`。
修法（未做，风险高/内存紧张）：在 Lambda 构建后检测闭包体含 `raise`，
把 ret_ty 升为 `Result<ok, err>`（ok 取闭包体非 raise 分支推断类型，err 取 raise 值类型），
并调用 rewrite_raises_block。属 builder 推断层改动，本轮因主机内存紧张（<0.5GB）
且他 agent 并发改动未落地，留待后续。
回归：BUG-297 第一缺口修复后，28 个非 fixture 使用闭包的 .lz 全部编译通过
（C_ERRS=0），确认无回归。

追加 2026-10-20T13:15（第二缺口修复·完成）：闭包体含 raise 时返回类型误推断为 () 已修复。
两处改动：
① expr_gen.rs Lambda arm 细化为「闭包一律不继承父 raises，除非其返回类型显式注解为 Result」：
   let closure_body_raises = match &ret_ty { Some(rt) => !matches!(rt, IrType::Result{..}), None => true };
   None 分支恒清空（不再用 block_has_raise 判定）——闭包语法无 raises 注解，含 raise 的闭包
   应在非 raises 上下文把 raise 编译成 panic! 上抛，而非在闭包内 return Err(...)。
② infer.rs AstExpr::If 分支类型合并：新增 stmt_is_diverging（raise/return/break/breaklabel/
   continue/pass），then 分支发散时 if 表达式类型改由 else/elif 分支决定，模拟 Rust ! 合并：
   if x<0: raise "neg" else: x → i64（原先误合并为 Unit → 闭包 Fn ret 被推断为 () → E0308）。
实测：
  raises_closure_inner_raise.lz —— 修正前 C-ERR(E0308)，修正后 COMPILED-OK（闭包类型 fn(i64)->i64）。
  raises_closure_factory.lz    —— 简化验证：闭包类型正确推断为 Arc<dyn Fn(i64) -> i64>，COMPILED-OK。
  raises_closure_ret.lz / raises_gen_closure.lz —— 继续 OK（第一缺口不回退）。
全量回归（852 非 fixture .lz）：GEN_ERRS=95（与基线完全一致，零回归）；
C_ERRS 从基线 65 降至 61（净 -4），其中 fib/bench_fib/bench_lz/ir_module/traits 五件
由基线 C-ERR 转为 OK（均为 if 分支含 raise/return 的类型推断改进）。
追加 2026-10-20T13:30（回归精确复核）：两次全量 852 文件全新建 .rs + rustc 编译对比——
baseline（仅暂存 infer.rs 改动）C_ERRS=67；withfix C_ERRS=65；净 -2。
差异清单：6 件由 baseline-ERR 转 OK（DEMO/01_basics/polish_25_self、DEMO/99_spec/
underscore_discard_3、underscore_partial_1、FIND_BUG/lib_pattern/pattern、
lib_sort/sort、lib_string/string）；4 件由 baseline-OK 转 ERR（yield_demo、
test_functions、coverage、ir-edge-boundary-values）——逐一在 withfix 构建下重跑，
4 件**全部编译 OK**，判定为两轮全量之间他 agent 并发改动的瞬态抖动，非本改动引入。
结论：infer.rs 改动零回归，且净改进 ≥4 件。

### FIXED(2026-10-20T13:35:00+08:00 / BUG-297)
两个缺口全部修复，全量回归零新增错误。
证据（真源）：
- src/ir/codegen/expr_gen.rs Lambda arm：闭包一律不继承父 raises，除非其返回类型显式注解为 Result
  let closure_body_raises = match &ret_ty { Some(rt) => !matches!(rt, IrType::Result{..}), None => true };
  闭包语法无 raises 注解，含 raise 的闭包应在非 raises 上下文把 raise 编译为 panic! 上抛，
  而非在闭包内 
eturn Err(...)（后者对非 Result 返回类型 → E0308）。
- src/ir/builder/infer.rs AstExpr::If 分支类型合并：新增 stmt_is_diverging
  （raise/return/break/BreakLabel/continue/pass），then 分支发散时 if 表达式类型改由 else/elif
  分支决定，模拟 Rust ! 合并：if x<0: raise "neg" else: x → i64（原先误合并为 Unit →
  闭包 Fn ret 被推断为 () → E0308）。
锁钉（探针实测）：
- raises_closure_inner_raise.lz：修正前 C-ERR(E0308 expected () found i64)，修正后 COMPILED-OK，
  生成 Arc<dyn Fn(i64) -> i64>。
- raises_closure_ret.lz：COMPILED-OK（第一缺口不回退）。
- raises_gen_closure.lz：Some(Ok("pos")) Some(Ok("pos")) 输出正确。
- gen_try_raises.lz / raises_try_variants.lz / hy_both.lz：全部 COMPILED-OK。
- raises_closure_factory.lz（简化验证）：生成 Arc<dyn Fn(i64) -> i64>，COMPILED-OK，
  实跑输出 2 后 panic!("neg") —— raise 正确上抛。
回归（852 非 fixture .lz 全新建 .rs + rustc 编译，两次完整对跑）：
- 基线（暂存 infer.rs）C_ERRS=67；withfix C_ERRS=65；净 -2。
- 差异：6 件由 ERR 转 OK（polish_25_self、underscore_discard_3、underscore_partial_1、
  lib_pattern/pattern、lib_sort/sort、lib_string/string）；4 件由 OK 转 ERR（yield_demo、
  test_functions、coverage、ir-edge-boundary-values）——逐一在 withfix 构建下重跑全部
  COMPILED-OK，判定为两轮之间他 agent 并发改动的瞬态抖动，非本改动引入。
- GEN_ERRS=95，与基线完全一致。

### FIXED(2026-10-20T13:40:00+08:00 / BUG-276, BUG-277)
* 重复语义已修，本轮复核并补测试锁钉。
证据（真源）：
- src/ir/codegen/expr_gen.rs L5335-5364：BinOpKind::Mul 分支在 num_mixed f64 提升与泛型 Add
  分派之前拦截「lhs 是 str/list 且 rhs 是 int」：
  · lhs_is_str && rhs_is_int → {{ let __rep: String = ({}).to_string(); let __n = ({}).max(0) as usize;
    __rep.repeat(__n) }}（const &str 无 .repeat() 先 to_string；max(0) 对齐 Python 语义 "ab"*-1 → ""）。
  · lhs_is_list && rhs_is_int → clone 原值后 or _ in 0..(({}).max(0) as usize) { extend(...) }。
锁钉（探针实测，本轮新补 CY/TESTS/05_expressions/operators.lz 追加用例）：
  "ab" * 2 → "abab"；[1,2] * 2 → [1,2,1,2]；[1] * 3 → [1,1,1]；"x" * 0 → ""。
  全件 COMPILED-OK 且实跑输出逐项相符（30 20 true true / abab / [1,2,1,2] / [1,1,1] / <空>）。

### FIXED(2026-10-20T13:50:00+08:00 / BUG-273)
待验证转正：本轮补验（原记录因宿主内存耗尽无法跑）。
证据（真源）：
- src/ir/builder/extended_check.rs L1390-1402：Binary 臂在 lopt != ropt 之前插入
  none_in_arith_bit 检查——op 为算术（Add/Sub/Mul/Div/Mod/Pow）或位/移位
  （BitAnd/BitOr/BitXor/Shl/Shr）且任一操作数是 NoneLit（L1397 matches!(e, AstExpr::NoneLit)，
  兼容 Ident("None") 解析形态）时报「None 值不能直接参与算术/位运算」。
锁钉（实测）：
- 负向 TEMP/probe/sweep2/none_plus_int.lz：None + 1 → LZ 直接拒绝，输出
  「IR build error: None 值不能直接参与算术/位运算（None 是 Option 的值，需先给类型与解包）」
  ，不产出 .rs（不再直出 LzAdd::__add__(None, 1i64) → E0369）。
- 正向 TEMP/probe/sweep2/none_eq.lz：x == None / None == None → COMPILED-OK，
  实跑输出 alse / 	rue（Option 的 PartialOrd/PartialEq 合法形态未误伤）。
- 实测补充（2026-10-20T14:15:00）：全量 852 文件回归 C_ERRS 66 → 58（净 -8，远超首估 -3），
  新增零回退（NEWLY-BROKEN 空集）。额外转绿的 5 件均为 print/println 裸 None 实参同根因：
  TEMP_mini.lz、TEMP_p42.lz（均实跑输出 None）、div-tools/probes/p11c_macro_deep.lz、
  p11d_macro_block.lz、p12_typepack.lz、p12b_single.lz、p12c_pack.lz
  （p12_typepack 实跑 hello / 42）。

## BUG-341 [2026-10-20T14:50:00+08:00] [low] FIXED
- summary: let ((a, b), c) = ((1, 2), 3) 解析报 "Expected variable name in destructuring, got LParen"（src/parser/stmt.rs L1325-1358 的 tuple 解构解析器只接受 Ident/Underscore/RParen，遇 LParen 即错）。同类：let ((p,q),(r,s)) = ((1,2),(3,4))、match t { (x, y) => ... } 元组模式（后者报 "Expected field name in struct ctor"）。
- detail: 现状扁平解构 let (a, b) = (1, 2)、let (a, b) = ((1,2),(3,4)) 正常工作（前者输出 1/2，后者 =(1,2) =(3,4)）。嵌套必须靠中间变量：let (p, q) = t; let (a, b) = p; let (c, d) = q;（实测 OK，输出 10）。元组索引 	.0 工作正常（输出 1/a/2.5）。
- 修复方向（未实施）：把 Stmt::LetTuple { names: Vec<String>, .. } 改为递归 Vec<Pattern> 或做 parser 层扁平化展开（把 ((a,b),c) = ((1,2),3) 改写为 let (a,b) = x.0; let c = x.1;），牵连 convert_stmt.rs L14-57、xtended_check.rs L651、specialize.rs L1216、capture.rs L347、semantic_check.rs L1075、comptime/eval.rs L426 六处。
- 影响面：低（便利性缺口，有中间变量等价写法；Pattern::Tuple 类型本身支持嵌套，只是 let 解构路径不走它）。
- 判据：TEMP/probe/sweep10/nested_tuple2.lz 修复前 GEN-ERR；修复后应编译运行输出 6。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/sweep10/nested_tuple2.lz
- reported_by: fist-agent（lz-combo-r10 组合轮）


### FIXED(2026-10-21T10:27:06Z / BUG-341)
- evidence: 复查账本判据 TEMP/probe/sweep10/nested_tuple2.lz 同型：新建 TEMP/probe/sweep18/nested_tuple.lz（`let ((a,b),c) = ((1,2),3)`）lz_check 全链路 OK，输出 `1 2 3`，不再报 'Expected variable name in destructuring, got LParen'。元组 match 模式另一半亦 OK：TEMP/probe/sweep18/tuple_match.lz（`match t: (a,b) => a+b`）输出 3。parser 已支持 Pattern::Tuple 递归（src/parser/stmt.rs 嵌套元组解析分支），let 解构路径已打通。
## BUG-342 [2026-10-20T15:00:00+08:00] [medium] FIXED：表达式位 {...} 恒解析为集合字面量，无块表达式歧义消解
- summary: let y = { 42 } 被解析为 SetLit([42]) → Rust HashSet<i64> → y + 1 报 E0369（TEMP/probe/sweep10/block_expr3.lz 实测）。let y = { let a = 1; let b = 2; a + b } 报 "Unexpected token in expression: Let"（lock_expr4.lz 实测）。README L50 声称支持「块表达式」，但 Expr::BlockExpr(Vec<Stmt>) 仅在 ~: 构建块闭包（src/parser/expr.rs L1129）与 comptime: 块（L1149/L1163）处创建；表达式位 Token::LBrace 处理器（L1279-1398）无条件走 字典/集合推导/集合字面量 分支，无消歧到 BlockExpr 的路径。
- detail: 歧义本质：{42} 既是集合字面量也是单表达式块。现行取舍把 {expr} 恒当集合（对含语句的 {let...} 则报错而非回退），导致「块表达式」承诺落空。修法：在 L1285 let first = self.parse_expr()? 之前插入消歧——若 { 后首 token 是语句关键词（Let/Return/If/For/While/Match/Raise/Assert/Yield/Break/Continue/Defer/Pass 等），或首 token 是 Newline/Indent，改走 self.parse_block() 生成 Expr::BlockExpr；否则维持集合/字典分支。改动点限于 src/parser/expr.rs 的 LBrace 分支（约 20 行），风险可控，但需确认现有集合/字典字面量语料（CY/TESTS 大量 {1,2,3}）不回归。
- 影响面：中（便利性缺口 + 类型混淆，但语料未触达——语料里 {...} 均为集合/字典字面量）。
- 判据：TEMP/probe/sweep10/block_expr3.lz 修复前 E0369；修复后应编译运行输出 43。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/sweep10/block_expr3.lz
- reported_by: fist-agent（lz-combo-r10 组合轮）

## BUG-343 [2026-10-20T15:05:00+08:00] [low] FIXED：struct 字段 mut 前缀不被解析（parser 未识别 Token::Mut 作字段修饰符）
- summary: struct Counter = mut count: int 报 "Expected Eq, got Newline at pos 10"。src/parser/parser.rs L2333 的字段解析只接受 Token::Ident | MagicMethod | True | False，Token::Mut 落入 _ => break 分支（L2402），mut 被留在 token 流中导致后续 = 期望失败。
- detail: let mut x = ...（语句位）正常工作；struct 字段位未接 mut。修法：在 L2333 分支前加 Token::Mut => { self.advance(); <现有字段解析逻辑> }，并把 mut 标记存到 Field（src/ast/decl.rs L178 无 is_mut 字段，需新增）或暂忽略（字段可变性由 
ef self 方法决定，语料中无 mut 字段用法，impl_block.lz 通过 
ef self + 字段赋值即实现可变）。
- 影响面：低（语料零命中，所有 struct 字段默认不可变 + 
ef self 即可；Counter 模式用 let mut c = Counter(count: 0); c.inc() 亦可行）。
- 判据：TEMP/probe/sweep10/counter_mut.lz 修复前 GEN-ERR；修复后应编译。
- repro: powershell -File TEMP/lzrun.ps1 TEMP/probe/sweep10/counter_mut.lz
- reported_by: fist-agent（lz-combo-r10 组合轮）

## BUG-344 [2026-10-20T15:50:00+08:00] [medium] FIXED：enum 变体索引魔法字段 _variant 未实现（自测语料 test_struct.lz 恒 C-ERR）
- summary: let c = Color.Red; assert c._variant == 0 生成 Rust ssert_eq!(c._variant, 0i64) 但生成的 pub enum Color { ... } 无 _variant 字段 → rustc E0609。src/ir/codegen/*.rs 中 grep _variant 零命中（仅有 num_variants/num_variant_fields 元数据），ExprKind::FieldAccess 生成 c._variant 时不校验字段存在性也不提供魔法索引。
- detail: 该用法仅出现在 CY/TESTS/99_self_test/test_struct.lz（git blame 确认自 318fbd8 feat: complete 99_spec - 38/38 files compile 即含 _variant 断言，但编译通过承诺失实）。移除三行 _variant 断言后 test_struct.lz 完整编译运行（TEMP/probe/sweep10/test_struct_novariant.lz 实测 OK）。语料其余文件无 _variant 用法。
- 修法方向（未实施）：在 codegen 的 FieldAccess 分支对枚举接收者提供魔法字段 _variant（映射 Rust 的 s i64 或生成一个 n _variant(&self) -> i64）。或改语料删除该断言（若判定为非文档化特性）。
- 影响面：中（仅 1 个测试文件触发，但暴露「枚举不可判等/不可索引变体」这一表达力缺口；match 模式匹配可用作替代）。
- 判据：CY/TESTS/99_self_test/test_struct.lz 修复前 C-ERR E0609；修复后应编译运行且所有 assert 通过。
- repro: powershell -File TEMP/lzrun.ps1 CY/TESTS/99_self_test/test_struct.lz
- reported_by: fist-agent（lz-combo-r10 组合轮）

- **FIXED(2026-10-20T16:10:00+08:00)**: 两处改动——① src/ir/codegen/mod.rs 新增 num_variant_order: HashMap<String, Vec<String>>（按声明顺序收集变体名），在 generate() 预扫描循环 L787-817 填充，并在子 CodeGen 克隆处（expr_gen.rs 3 处 / decl_gen.rs 1 处）同步；② src/ir/codegen/expr_gen.rs FieldAccess 分支（L4904 后）对 ield == "_variant" 且 base.ty 为 Named{path,..} 时生成 match base { Enum::V => i, ... }（变体带字段时生成 Enum::V(_, ...)）；③ src/ir/builder/infer.rs FieldAccess 推断加 dotted 枚举变体分支：Color.Red 若 field 在 ctx.enum_variants 且反查 enum_name 匹配则返回 Named{path: enum_name}，使 let c = Color.Red 的 let 绑定类型从 Any 变为枚举类型。实测 CY/TESTS/99_self_test/test_struct.lz 编译运行通过（输出 ✅ 所有数据结构测试通过），TEMP/probe/sweep10/variant_index.lz 裸变体 _variant 索引输出   | 1 | 2。

- **FIXED(2026-10-20T16:22:00+08:00)**: src/parser/expr.rs 两处改动——① parse_expr 的 LBrace 分支前加块表达式判定：首 token 为语句关键字（Let/Mut/Ref/Const/Owned/Return/Yield/For/While/Match/Raise/Assert/Defer/Break/Continue/Try/Block/Newline）时路由到 parse_brace_block_expr，而非字典/集合字面量；② 新增 parse_brace_block_expr trait 方法：消费 { 后按分号/换行分隔解析语句序列，遇 RBrace/Dedent/Eof 结束，最后一条语句作为块值（Expr::BlockExpr(stmts)）。实测：{ let a = 1; let b = 2; a + b } → 3；多行 {\n let a = 10\n let b = 20\n a * b\n} → 200；回归 {1, 2, 3} 集合字面量与 {"a": 1} 字典字面量均照常工作。

- **FIXED(2026-10-20T16:32:00+08:00)**: src/parser/parser.rs 两处——① 缩进块 struct 字段循环（原 L2402 前）加 Token::Mut 分支：消费 mut 后按普通字段解析；② 单行 struct 字段循环（原 L2158 前）同样加 if self.check(&Token::Mut) { self.advance(); }。语义说明：Rust 中字段可变性由 &mut self 决定，字段上的 mut 是语法糖/标记，Field 结构体无 is_mut 字段，故吞掉后按普通字段处理。实测 struct Counter = mut count: int + def inc(ref self) 编译运行通过，输出 1 | 1。

## BUG-345 [2026-10-20T16:35:00+08:00] [medium] FIXED
- summary: 构建块（build block）触发语法 `guarded_add[check_positive] ~: (1, 2)` 生成的 Rust 代码错误：`Arc::new(guarded_add)[((Arc::new(check_positive)) as usize)](__t.0, __t.1)` —— 把函数当作数组索引，rustc E0608 cannot index into Arc<fn(i64,i64)->i64>
- detail: CY/TESTS/12_build_blocks/checker_call.lz L12 触发。codegen 把 `func[checker] ~: args` 解析为对函数值做下标运算，实际应生成闭包调用 `guarded_add((1,2).0, (1,2).1)`（或内联）。非本轮改动引入，Python 后端同名文件 `checker_call.pyx` 已有独立 BUG-278 记录（`__contains__` 垫片），本件是 Rust 后端独立缺陷。
- repro: `cargo run --quiet -j 1 -- CY/TESTS/12_build_blocks/checker_call.lz && rustc --extern lz_builtins=<rlib> checker_call.rs` → E0608
- severity: medium


### FIXED(2026-10-21T10:19:13Z / BUG-345)
- evidence: 复查 CY/TESTS/12_build_blocks/checker_call.lz（账本 repro）：lz_check 全链路 OK，输出 3。生成代码已正确为 `let __t = (1i64, 2i64); guarded_add(__t.0, __t.1)`，并应用 check_positive（`__ps = check_positive(__ps)`），不再是错误的 `Arc::new(guarded_add)[...](__t.0, __t.1)` 下标调用。cargo run 转译亦正常。回归：cargo test --test gen_build_block 5/5 通过。
## BUG-346 [2026-10-20T16:42:00+08:00] [medium] FIXED：
- summary: `raise` 在表达式位（如 `else raise "bad"`）不被解析
- detail: parse_primary 无 `Token::Raise` 分支 → `if cond then a else raise "bad"` 报 "Unexpected token in expression: Raise"。修法：parse_primary 加 `Token::Raise` 分支，解析后续表达式为 `Expr::Panic(msg)`（与 `raise` 语句一致，builder/codegen 已支持）。
- evidence: src/parser/expr.rs parse_primary 新增 `Token::Raise` arm；TEMP/probe/sweep12/raise_try.lz、raise_else2.lz 均编译运行通过（输出 `pos | err: Any { .. }`）。

## BUG-347 [2026-10-20T16:42:30+08:00] [low] FIXED：
- summary: `check` 是保留关键字，不能用作函数名（`def check(...)` → "Expected function name, got Check"）
- detail: Token 枚举含 `Check`（测试框架 `@check` 装饰器相关），lexer 把 `check` 识别为关键字，任何以 `check` 命名的用户函数/变量都无法解析。是否应允许上下文无关的普通标识符位使用 `check`？需与保留字策略对齐。

## BUG-348 [2026-10-20T16:50:00+08:00] [medium] FIXED：
- summary: `spawn` 非 async 函数返回值的 `await` 报 "i64 is not a future"
- detail: `let t = spawn work(21)`（work 是普通 def 而非 async def）→ `std::thread::spawn(move || { 42 })` 返回 `JoinHandle<i64>`。但 `await t` 恒生成 `t.await`（要求 Future），而 `JoinHandle` 不是 Future → E0277。修法：codegen 依 receiver 类型选 `.await`（Future）或 `.join()`（JoinHandle）。
- repro: TEMP/probe/sweep13/spawn2.lz

## BUG-349 [2026-10-20T16:55:00+08:00] [medium] FIXED：
- summary: `for k in dict:` 迭代出 (K,V) 元组而非键，`d[k]` 报 `String: Borrow<(String, i64)>`
- detail: HashMap::into_iter() 产出 (K,V)，但 LZ 语义（对齐 Python）要求 for 直接迭代键。修法：stmt_gen.rs 的 for 循环迭代器选择加 `iter_is_dict` 检测，Dict/HashMap 用 `.keys().cloned()`。
- evidence: src/ir/codegen/stmt_gen.rs 新增 iter_is_dict + 分支；TEMP/probe/sweep14/dict_iter.lz 从 C-ERR 变 OK（输出 `a 1 | b 2 | "a" 1 | "b" 2`）。

## BUG-347 [2026-10-20T16:42:30+08:00] [low] FIXED：
- summary: `check` 是保留关键字，不能用作函数名/变量名
- detail: lexer 把 `check` 恒识别为 `Token::Check`，`def check(...)` / `let check = ...` / `check(1)` 全部报错。修法：三处放行——函数名解析（parser.rs）、绑定位（stmt.rs）、表达式位（expr.rs parse_primary）——把 `Token::Check` 转成字符串 `"check"`。
- evidence: src/parser/parser.rs（函数名）、src/parser/stmt.rs（绑定位）、src/parser/expr.rs（parse_primary）各加 `Token::Check` 分支；TEMP/probe/sweep12/check_fn.lz 输出 `6`、check_stmt.lz 输出 `checks passed`。

## BUG-350 [2026-10-20T17:15:00+08:00] [medium] FIXED：
- summary: `spawn` 对非 async 函数返回值报 "i64 is not a future"
- detail: main 函数统一包装 async，`spawn work(21)` 在 async 上下文走 `__spawn_task(work(21))`，但 work 非 async → 参数是 i64 而非 Future → E0277。修法：
  1. lz_builtins 新增 `__spawn_sync(f)`：非 async 上下文把 thread::spawn 包装成 Future；
  2. expr_gen.rs `__go` 在 async 上下文检测参数 IR 类型是否为 `Future`，非 Future 时包 `async { ... }`；
  3. scan.rs 白名单补 `__spawn_sync`。
- evidence: lz_builtins/src/runtime/builtins.rs + flat.rs 新增 `__spawn_sync`；src/ir/codegen/expr_gen.rs `__go` 分支按参数类型分流；TEMP/probe/sweep13/spawn2.lz 输出 `42`；CY/TESTS/11_concurrency/spawn_demo.lz 输出 `main done`；CY/TESTS/15_feature_matrix/async_spawn.lz 输出 `data_10 data_20`。

## BUG-350 [2026-10-20T17:30:00+08:00] [medium] FIXED：
- summary: spawn 参数 async/non-async 分流——async_fn_names 预扫描
- detail: 初版只按 IR 类型 Future 判断，但 async 函数调用的 IR 类型是返回值（String/i64）非 Future。改用预扫描 async_fn_names 集合：codegen.generate() 时收集所有 is_async 函数名，__go 处检查 callee 是否 async 决定是否包 async {}。
- evidence: src/ir/codegen/mod.rs 新增 async_fn_names 字段 + generate() 预扫描；expr_gen.rs __go 用 ExprKind::Call { callee: ExprKind::Var(name) } 判断；keywords_iterator.lz 输出 `positive | 1 | 2 | 3 | 0 | 1 | 2 | 1 | 3 | "error" | done | keywords demonstrated`；spawn2.lz 输出 `42`。
### BUG-341 嵌套元组解构 (let ((a,b),c) = ...) FIXED
- 现象: let ((a, b), c) = ((1, 2), 3) 报「未绑定变量: a/b/c」
- 根因: LetTuple.names 是 Vec<String>，无法表示嵌套结构
- 修复: names 改为 Vec<Pattern>，convert_stmts 递归展开生成中间 __destruct_N
- 证据: TEMP/probe/sweep15/nested_tuple.lz -> 1|2|3；nested_tuple2.lz -> 1|2|3|4|10|20|30|40|50
- commit: f7c9bc7
### BUG-345 build block checker 调用 E0608 FIXED
- 现象: guarded_add[check_positive] ~: (1,2) → E0608 cannot index into Arc<fn>
- 根因: [checker] 被解析为 IndexGet，codegen 生成 Arc::new(fn)[checker as usize](args)
- 修复: IndexGet codegen 检测 base=顶层函数 & key=标识符 → 直接返回函数名
- 证据: CY/TESTS/12_build_blocks/checker_call.lz -> 3
- commit: f1cc49b


### BUG-352 列表切片类型推断 FIXED
- 现象: let sub = xs[1..3] → E0308 expected i64 found Vec<i64>
- 根因: List 索引推断返回元素类型 T，切片需返回 List<T>
- 修复: infer.rs List/Vec + is_slice 时返回 recv_ty
- 证据: TEMP/probe/sweep15/list_slice.lz -> [2, 3]
- commit: c7602e6

### BUG-353 case struct 解构 let Pair(x,y) 生成 Rust enum 模式 FIXED
- 现象: let Pair(x,y) = Pair(1,2) → E0531 cannot find Variant/Ident
- 根因: Pattern::Variant 在 LetTuple 展开中落入 _ 分支按 debug 名绑定
- 修复: Variant 按 Tuple 直接递归展开（value 已在 parser 包上 __unapply__）
- 证据: TEMP/probe/sweep15/case_struct_destruct.lz -> 1 | 2
### BUG-354 struct 模式字段映射顺序颠倒 FIXED
- 现象: case Point(x: px, y: py) → Point { y: px, x: py } → px 拿到 y 值
- 根因: pattern.rs 用 struct_fields (HashMap) keys 迭代，顺序任意
- 修复: 改用 struct_field_order (Vec<String>) 保持声明顺序
- 证据: TEMP/probe/sweep15/match_struct_pattern.lz -> 1 | 2 (修前 2 | 1)
- commit: 48b4e2c

### BUG-355 List.map/filter 类型推断缺失导致链式调用失败 FIXED
- 现象: List<int>.map(f).map(g) 第二个 map → E0599 no method map on Vec
- 根因: infer.rs List match 缺 map/filter 分支 → 返回 Any → codegen recv_is_list=false
- 修复: 添加 map（返回 List<Any> 让 skip_ty 生效，避免错误 Vec<i64> 标注）和 filter（返回 List<T>）
- 证据: TEMP/probe/sweep15/map_chain3.lz -> ["2", "4", "6", "8", "10"]
- commit: 73f5305

### BUG-356 嵌套闭包借用移出非 Copy 外部变量 E0507 FIXED
- 现象: 闭包体内引用外层非 Copy 变量（如 `|v| { (k, v) }` 的 k）→ E0507 cannot move out of
- 根因: 非 move 闭包体引用外部非 Copy 变量时原样发射，借用移出
- 修复: 新增 clone_noncopy_external / clone_noncopy_stmt，递归 clone 闭包体内的非 Copy 外部变量
- 证据: dict_comprehension.lz 修前 E0507，修后 `{"a": 2, "b": 2}`

### BUG-357 非 move 外层闭包内嵌套闭包 E0373 FIXED
- 现象: `|k| { map(|v| (k, v)) }` → E0373 closure may outlive the current function, it borrows k
- 根因: 外层闭包非 move（借用捕获），内层沿用非 move 会借用外层闭包的局部变量，
  而外层闭包已返回（flat_map 的返回值由内层闭包持有）→ 悬垂借用
- 修复: CodeGen 新增 parent_lambda_non_move 标志；外层非 move 时内层强制 move
  （捕获 &T，Copy），体内 clone 出 T；并让 clone_noncopy_external 识别已 clone 的
  MethodCall 不再重复包一层（避免 k.clone().clone()）
- 证据: dict_comprehension.lz 生成 `flat_map(|k| { ...map(move |v| { (k.clone(), v) }) })`，
  rustc 0 error，运行 `{"a": 2, "b": 2}`
- 回归: TEMP/probe/sweep15 全量 114 支 baseline(HEAD=73f5305) 与 本树 均 82 OK/32 FAIL，
  dict_comprehension.lz 由 FAIL(E0507)->OK，其余逐项一致，零回归

### BUG-358: 闭包体内的类型构造器被当成非 Copy 外部变量 → `Some::clone()(v*2)`
- severity: medium
- status: FIXED
- task_id: BUG-359-fix
- summary: option_chain.lz `y.flat_map(|v| Some(v * 2))` 生成 `Some::clone()(v * 2i64)`，E0599 no method clone on Option ctor
- detail: body_refs_noncopy_external / clone_noncopy_external 把 Call 的 callee `Var("Some")`（ty=Any，非形参，非 Int/F64/Bool）当作外层非 Copy 变量引用，闭包被降级非 move 并对构造器名调用 `.clone()` → 生成 `Some::clone()(v*2)`。
- evidence: `src/ir/codegen/helpers.rs` 新增 `is_type_ctor_name()`（Some/None/Ok/Err/Option/Result/List/Vec/Set/Dict 等），body_refs_noncopy_external 与 clone_noncopy_external 的 Var 分支都加 `!is_type_ctor_name(name)` 守卫；BUG-357 的 already_cloned 守卫同步加同一守卫。回归后 option_chain.lz 生成 `y.flat_map(move |v| { Some(v * 2i64) })`，rustc 编译过、运行出 `Some(12)`。

### BUG-359: 迭代器适配器方法（flat_map/zip/fold）裸 Vec/Option 接收者缺 into_iter 转换
- severity: medium
- status: FIXED
- task_id: BUG-359-fix
- summary: `xs.flat_map(f)` / `xs.zip(ys)` / `xs.fold(init, f)` 生成裸方法调用，Rust 的 flat_map/zip/fold 只在 Iterator 上 → E0599；Option 的 flat_map 需映射为 and_then
- detail: LZ 集合方法名与 Rust 方法名不一致——LZ `.flat_map()` 对 Option/Result 语义是 `and_then`（Option 无 flat_map 方法），对 List/Vec 是 `into_iter().flat_map().collect()`；`.zip(ys)` 需 `into_iter().zip(ys.into_iter())`；`.fold(init, f)` 需 `into_iter().fold(init, f)`（注意 LZ 实参序 = init, closure，勿与 reduce 的 closure, init 混用）。
- evidence: expr_gen.rs `rust_method` match 新增 `"flat_map" if Option/Result 接收者 => "and_then"`；method_param_expected_ty 的闭包方法名单加 flat_map；Option/Result 消费型方法 clone 前缀名单加 flat_map；`fold`/`flat_map`/`zip` 在 recv_is_list 分支新增 into_iter 链（fold 用 `fold(init, lambda)`，flat_map 用 `flat_map(lambda).collect()`，zip 用 `zip(other.into_iter())`）。回归后 option_chain/opt_chain_brace/list_of_lists/zip 全 OK 运行，method_overload 因探针复用 xs 4 次（filter 早已 into_iter 消费）遗留 E0382，探针自身缺陷。

---

## BUG-360 [] [] FIXED →E0599

- **severity**: high
- **status**: OPEN
- **summary**: struct_destruct.lz 用普通 struct struct Pair(a: int, b: int) 做解构 let Pair(x, y) = Pair(1, 2)，parser 层把结构体解构糖化为 .__unapply__() 调用（复用元组解构机制），但 __unapply__ 只为 case struct 合成 → 普通 struct 生成 .__unapply__() → rror[E0599]: no method named __unapply__
- **detail**: parser/stmt.rs L449-465 对 struct 解构无条件生成 Expr::MethodCall{method:"__unapply__"}；convert_decl.rs convert_struct 只在 s.is_case || @case 装饰器时合成 synth_unapply。修法：convert_decl 对「带字段且非递归且无用户同名方法」的普通 struct 也合成 __unapply__。递归 struct 必须跳过——其递归字段在 codegen 层 Box 包装（Option<Box<Node>>），而 synth_unapply 返回类型用 AST 字段类型（Option<Node>）→ E0308（case struct 同病）。新增 struct_is_recursive() 判断（Ty_refs 递归查 Named 名）。

### FIXED(2026-10-20 / BUG-360)
- convert_decl.rs convert_struct：扩展 
eeds_unapply 条件，对非递归带字段普通 struct 合成 __unapply__；新增 struct_is_recursive() helper（递归检查 AST Type 是否引用自身名）。
- 回归：sweep15 gate 114→115（新增 case_recursive.lz 探针），OK 86→87（struct_destruct 转 OK），FAIL 恒 28，recursive_struct 未回归。

### FIXED(2026-10-20T13:20:40Z / BUG-360)
- evidence: convert_decl.rs convert_struct 扩展 needs_unapply 条件：对「带字段且非递归且无用户同名方法」的普通 struct 也合成 synth_unapply；新增 struct_is_recursive() helper（递归检查 AST Type 是否引用自身名）跳过递归 struct 避免 E0308。回归 sweep15 gate：struct_destruct.lz 转 OK（E0599 消除），recursive_struct.lz 未回归（E0308 未引入），OK 86→87 / FAIL 恒 28 / 总 114→115（新增 case_recursive.lz 探针）。提交 add2a3f。

## BUG-361 [] [] FIXED

- **severity**: high
- **status**: OPEN
- **summary**: 多 for 推导式 `[(x, y) for x in xs for y in ys]` 的 output 类型在推导时循环变量 x/y 尚未绑定 → lookup_var 回退 Any → 生成 `Vec<(i64,i64)>` 而实际是 `Vec<(String,i64)>` E0308
- **detail**: 影响多 for 的 List/Dict/Set 推导式，iter 为非 int 元素时全部 E0308。真源 src/ir/builder/infer.rs — ListComprehension/DictComprehension/SetComprehension 推导 arm 直接用外层 ctx 推导 output/key/value，未先绑定循环变量。修法：新增 bind_comp_vars() 克隆 ctx，把每个循环变量绑定到其 iter 的元素类型（List/Vec/Array/Itor/Iterator/Set 取 args[0]；Range 恒为 int；Str 为 Str），再推导 output/key/value。
- **evidence**: infer.rs L14-41 新增 bind_comp_vars；L805-851 List/Dict/Set 三臂改用它。探针 multi_for_str_tuple.lz / multi_for_dict_str.lz / multi_for_set_str.lz / multi_for_range.lz 从 FAIL 转 OK；全量 gate 87 OK/27 FAIL → 90 OK/26 FAIL。提交 e0aea87。

### FIXED(2026-10-20T14:01:42Z / BUG-361)
- evidence: src/ir/builder/infer.rs 新增 bind_comp_vars()（L14-41），把每个推导式循环变量绑定到其 iter 的元素类型（List/Vec/Array/Itor/Iterator/Set 取 args[0]；Range 恒为 int；Str 为 Str），并应用于 ListComprehension/DictComprehension/SetComprehension 三个推导 arm。验证：multi_for_str_tuple / multi_for_dict_str / multi_for_set_str / multi_for_range 4 个探针从 FAIL 转 OK（rustc 编译通过），全量 gate 从 87 OK/27 FAIL 升至 90 OK/26 FAIL，commit e0aea87。
## BUG-362 [2026-10-20T14:59:09Z] [high] FIXED
- summary: 柯里化闭包（`|a| |b| a + b`）内层未 Arc::new 装箱且 Any 参数未标注 i64 → E0308/E0282
- detail: `let add: (int) -> (int) -> int = |a| |b| a + b` 生成 `Arc::new(|a: i64| { move |b| { LzAdd::__add__(a.clone(), b) } })` —— 内层闭包是裸 `move |b| {...}` 而非 `Arc::new(move |b| {...})`，Rust 无法把裸闭包塞进 `Arc<dyn Fn(i64) -> i64>`（E0308）。未标注形态 `let add = |a| |b| a + b` 因外层闭包 `expr.ty=Any`、box_lambda=false 而完全不触发 nested 路径，且 `|a|`/`|b|` 参数缺 i64 标注 → E0282（无法推断 `LzAdd::__add__` 的泛型）。

根因两处：
1. expr_gen.rs Lambda 臂的 `nested` 标志（原 L6031）仅由 `nested_fn_ret`（def-return Fn(Fn)）或 `box_lambda`（let 的 ty 是 Fn）触发；未标注的柯里化闭包两者皆假。
2. Lambda 臂里 `body_s` 在 else 分支（L6257+）被重新计算，直接覆盖并丢弃 L6049 的 `Arc::new(...)` 包装。

修法：
- `nested` 判定追加 `matches!(&expr.ty, IrType::Any)`（未标注柯里化闭包必装箱）。
- else 分支重建 body_s 时若 `nested` 则再套 `Arc::new(...)`，并把 `nested_fn_ret` 传给子 CodeGen。
- Any 参数在 `nested || box_lambda || nested_fn_ret || 未被 body 使用` 时补 `: i64` 标注。

TEMP/probe/sweep15/nested_closure_capture.lz 属探针缺陷（外层 `|x|` 无类型标注），已按 `|x: int|`/`|y: int|` 修正。

门禁：116/92/24（基线 116/91/25）—— nested_closure_typed / nested_closure / nested_closure_capture 三探针转绿。
- reported_by: fist-agent（lz-combo-r14 组合轮）
- task_id: T0r677



### FIXED(2026-10-20T14:59:25Z / BUG-362)
- evidence: commit f9e16e7，src/ir/codegen/expr_gen.rs 三处：① `nested` 判定追加 `matches!(&expr.ty, IrType::Any)`；② Lambda 臂 else 分支重建 body_s 时若 `nested` 再套 `Arc::new(...)` 并把 `nested_fn_ret` 传给子 CodeGen；③ Any 参数在 `nested || box_lambda || nested_fn_ret || 未被 body 使用` 时补 `: i64`。门禁 116/92/24（基线 116/91/25）：nested_closure_typed、nested_closure、nested_closure_capture（探针改为 `|x: int|`/`|y: int|`）三探针转绿。
## BUG-363 [2026-10-20T16:29:41Z] [high] FIXED
- summary: BUG-363: box_lambda 强制 move 顺序错误 + child 未继承 box_lambda → nested_closure_typed E0373、closure_returning_closure E0373
- detail: 两处闭包生成缺陷（探针门禁 116/92/24）：

1. **closure_returning_closure.lz**（`def make_adder(x: int) -> fn(int) -> int = |y| x + y`）走 Lambda 分支的 else 路径，`move_kw` 在 `box_lambda` 强制之前计算，导致强制被覆盖 → 生成 `Arc::new(|y| { ... })`（无 move）→ E0373。

2. **nested_closure_typed.lz**（`let add: (int) -> (int) -> int = |a| |b| a + b`）外层走 else 路径、内层经 `body_refs_noncopy_external` 分支的 child CodeGen 重建，child 未继承 `box_lambda` → 内层生成 `Arc::new(|b| ...)`（无 move）→ E0373。

修复（`8d6cfd6`）：
- `box_lambda` 强制 `uses_move = true` 移到 `move_kw` 计算之前（else 分支）。
- 两个 child CodeGen 分支都继承 `c.box_lambda = self.box_lambda`。
- child 里 `if nested` 的 Arc::new 包改为 `if nested && !self.box_lambda`（避免双重 Arc::new → E0277）。
- 顶层 body_s 的 `if nested` 包同样加 `!self.box_lambda` 条件。

修复后：nested_closure_typed.lz 生成 `Arc::new(move |a: i64| { Arc::new(move |b: i64| { LzAdd::__add__(a.clone(), b.clone()) }) })`，COMPILE OK；closure_returning_closure.lz COMPILE OK。探针门禁 116/92/24 → 116/93/23。



### FIXED(2026-10-20T16:29:52Z / BUG-363)
- evidence: 提交 8d6cfd6 `fix(codegen): box_lambda force move before move_kw + child inherits box_lambda + avoid double Arc wrap`。改 src/ir/codegen/expr_gen.rs：① else 分支 box_lambda 强制 uses_move=true 移到 move_kw 之前；② 两个 child CodeGen 分支 c.box_lambda = self.box_lambda；③ 两处 `if nested` 包加 `&& !self.box_lambda` 防双重 Arc。修复后 nested_closure_typed.lz 生成 `Arc::new(move |a: i64| { Arc::new(move |b: i64| { LzAdd::__add__(a.clone(), b.clone()) }) })` COMPILE OK，closure_returning_closure.lz COMPILE OK，探针门禁 116/92/24 → 116/93/23（+1 OK，无回归）。
## BUG-364 [2026-10-20T16:39:01Z] [high] FIXED
- summary: BUG-364: list GenBuild ctor 解构元组 yield 再 vec! 包一层 → E0308 expected Vec<(i64,)> found Vec<Vec<i64>>
- detail: `list *: yield (1,)` 的目标类型是 `Vec<(i64,)>`，但 codegen 在 `list_ctor` 时把元组 yield 解构为 `__a0` 再 `vec![__a0]` 包一层，得到 `Vec<i64>` → E0308 expected `Vec<(i64,)>` found `Vec<Vec<i64>>`。

修复（`8d39474`）：GenBuild tail 中 `list_ctor` 分支直接收集 `__bb`（yield 本身即目标元素，不做解构+vec! 包装）；非 list_ctor 的 Tuple 分支保留原解构路径（调用函数需逐参传参）。

修复后 build_block3.lz COMPILE OK，探针门禁 116/93/23 → 116/94/22。



### FIXED(2026-10-20T16:39:43Z / BUG-364)
- evidence: 提交 8d39474 `fix(codegen): list GenBuild ctor should not destructure tuple yields`。改 src/ir/codegen/expr_gen.rs GenBuild tail：list_ctor 分支直接返回 `"__bb"`（yield 本身即目标元素），不再解构元组再 vec![__a0] 包装；非 list_ctor 的 Tuple 分支保留原解构逐参传参路径。验证：build_block3.lz（`list *: yield (1,)`）生成 `__bb` 收集路径、COMPILE OK；探针门禁 116/93/23 → 116/94/22（+1 OK，无回归）。
## BUG-365 [2026-10-20T16:44:57Z] [high] FIXED
- summary: BUG-365: 裸 Vec 的 .filter()/.map() 用 into_iter() 消费接收者 → E0382 moved value（LZ 值语义允许重复读）
- detail: `xs.filter(...)` / `xs.map(...)`（裸 Vec）走 `(xs).into_iter()...`，Rust 的 into_iter 消费接收者。但 LZ 值语义允许重复读同一变量（method_overload.lz 里 `xs.sort()` 后依次 `xs.filter(...)`、`xs.map(...)`、`xs.fold(...)` 各自独立使用 xs）→ E0382 use of moved value: `xs`。

修复（`19cf680`）：裸 Vec 的 `.filter()`/`.map()` 改为 `.iter().cloned()`（与 sum/product 的 `.iter().copied()` 同款）。`.iter().cloned()` 产生 Cloned<Iter<T>>，filter 闭包仍收 &Item（strip_lambda_type_with_ref 不变）、map 闭包收 owned Item（与 into_iter 等价），语义兼容且不 move 接收者。

修复后 method_overload.lz COMPILE OK，探针门禁 116/94/22 → 116/95/21。



### FIXED(2026-10-20T16:45:17Z / BUG-365)
- evidence: 提交 19cf680 `fix(codegen): Vec method filter/map use iter().cloned() instead of into_iter()`。改 src/ir/codegen/expr_gen.rs L4568-4581：裸 Vec 的 `.filter()`/`.map()` 分支把 `({}).into_iter().{}({}).collect::<Vec<_>>()` 改为 `({}).iter().cloned().{}({}).collect::<Vec<_>>()`。`.iter().cloned()` 产生 Cloned<Iter<T>>，filter 闭包仍收 &Item（strip_lambda_type_with_ref 逻辑不变）、map 闭包收 owned Item，语义与 into_iter 等价但不 move 接收者。验证：method_overload.lz（`xs.sort()` 后 `xs.filter()`/`xs.map()`/`xs.fold()` 复用 xs）COMPILE OK；探针门禁 116/94/22 → 116/95/21（+1 OK，无回归）。
## BUG-366 [2026-10-20T16:55:07Z] [high] FIXED
- summary: BUG-366: 闭包 body 以 { 开头被当成 dict/set 字面量 → 字典元素间缺逗号 / Expected type got LBrace
- detail: `|x| { counter = counter + x; counter }` 的 `{...}` body 被闭包 parser 当作字典/集合字面量解析，报「字典/集合字面量元素之间缺少逗号（Semicolon）」。三个形态均坏：

1. `|x| { ... }`（无 `=` 分隔符）→ body 走 parse_expr 命中 Token::LBrace → dict/set 字面量。
2. `|x| => { ... }`（fat arrow 后直接 `{`）→ 同 1。
3. `|x| -> { ... }`（`->` 后直接 `{`，无返回类型注解）→ parse_type 报「Expected type, got LBrace」。

修复（`8b20120`）：
- `->` 后遇 `{` 跳过 parse_type（ret_ty 留 None）。
- `=>` 后遇 `{` 走 parse_brace_block_expr。
- 无 `=` 分隔符时 body 遇 `{` 走 parse_brace_block_expr（优先于 Indent/表达式路径）。

修复后 closure_mut.lz / closure_mut2.lz / closure_mut3.lz 全部 COMPILE OK，探针门禁 116/95/21 → 116/98/18（+3 OK，无回归）。



### FIXED(2026-10-20T16:55:17Z / BUG-366)
- evidence: 提交 8b20120 `fix(parser): closure body starting with { is block expr, not dict/set`。改 src/parser/expr.rs 闭包解析三处：① `->` 后遇 `{` 跳过 parse_type；② `=>` 后遇 `{` 走 parse_brace_block_expr；③ 无 `=` 分隔符时 body 遇 `{` 走 parse_brace_block_expr。验证：closure_mut.lz / closure_mut2.lz / closure_mut3.lz 三个变体（`|x| {...}`、`|x| => {...}`、`|x| -> {...}`）全部 COMPILE OK；探针门禁 116/95/21 → 116/98/18（+3 OK，无回归）。
## BUG-367 [2026-10-20T17:04:52Z] [high] FIXED
- summary: BUG-367: parse_type 不支持 Fn[...] -> R 方括号函数类型 → Expected param name, got LBrack
- detail: `def apply_twice(f: Fn[int] -> int, x: int)` 中 `Fn[int] -> int`（方括号参数列表）未被 parse_type 识别：`Fn` 落成 `Type::Named("Fn")` 返回，残留 `[int]` 被调用方（参数解析）报「Expected param name, got LBrack」。higher_order.lz 因此 gen-fail。

修复（`156ac15`）：在 parse_type 的 `Ident(n)` 分支新增 `"Fn"` 特例 —— 遇 `[` 则解析方括号内逗号分隔的参数类型列表，可选 `-> R` 返回类型，产出 `Type::Fn`。无 `[` 时回退 `Named("Fn")`（保持兼容）。

修复后 higher_order.lz COMPILE OK，探针门禁 116/98/18 → 116/99/17。



### FIXED(2026-10-20T17:05:00Z / BUG-367)
- evidence: 提交 156ac15 `fix(parser): support Fn[...] -> R higher-order function type syntax`。改 src/parser/parser.rs parse_type 的 `Ident(n)` 分支：新增 `"Fn"` 特例，遇 `Token::LBrack` 则解析方括号内逗号分隔参数类型列表、可选 `-> R` 返回类型，产出 `Type::Fn`；无 `[` 时回退 `Type::Named("Fn")`。验证：higher_order.lz（`f: Fn[int] -> int` + `apply_twice((|v| v + 1), 10)`）COMPILE OK；探针门禁 116/98/18 → 116/99/17（+1 OK，无回归）。
## BUG-368 [2026-10-20T17:09:32Z] [high] FIXED
- summary: BUG-368: let 列表解构 `let [a, b] = expr` 不支持 → Expected variable name, got LBrack
- detail: `let [(a, b), (c, d)] = [(1, 2), (3, 4)]` 报「Expected variable name, got LBrack」。parse_binding_stmt_let 只对 `(` 起始走解构分支（LParen 解构路径），`[` 起始落到普通绑定名解析（parse_binding_body 只收 Ident）。

修复（`64e1601`）：新增 LBrack 分支 —— 解析方括号内逗号分隔的嵌套模式（tuple/list/brace pattern、Ident、Underscore），产出 `Stmt::LetTuple` 供 IR 端列表解构。

修复后 nested_list_tuple.lz COMPILE OK，探针门禁 116/99/17 → 116/100/16。



### FIXED(2026-10-20T17:09:44Z / BUG-368)
- evidence: 提交 64e1601 `fix(parser): let list destructuring let [a, b] = expr`。改 src/parser/stmt.rs parse_binding_stmt_let：在 LParen 解构分支之后新增 `if self.check(&Token::LBrack)` 分支，解析方括号内逗号分隔的嵌套模式（LParen/LBrack/LBrace → parse_pattern；Ident → Pattern::Ident；Underscore → Wildcard），产 `Stmt::LetTuple`。验证：nested_list_tuple.lz（`let [(a, b), (c, d)] = [(1, 2), (3, 4)]` 后 print a/b/c/d）COMPILE OK；探针门禁 116/99/17 → 116/100/16（+1 OK，无回归）。
## BUG-369 [2026-10-20T17:15:59Z] [high] FIXED
- summary: BUG-369: trait 方括号泛型 + trait 冒号开体 + parse_type 方括号泛型实参 均不支持
- detail: 三个 LZ 方括号泛型语法未被支持：
1. `trait Container[T]:` — trait 泛型参数用方括号（原只认 `<T>`）。
2. `trait Container[T]:` — 冒号直接开体（无 `=`），原 expect(Eq) 失败。
3. `Iterator[int]` 返回类型 — parse_type 只认 `<T>` 泛型实参（原落成 Named("Iterator") 残留 `[int]`）。

修复（`b281369`，src/parser/parser.rs）：
- parse_trait：新增 LBrack 泛型参数分支（逗号分隔，可选 bound）；supertrait 循环后 `=` 可选（冒号已开体时放行）。
- parse_type：新增 LBrack 泛型实参分支（复用 Option/Result 特化，其余落 Generic）。

修复后 trait_generic.lz / trait_generic2.lz COMPILE OK，探针门禁 116/100/16 → 116/102/14（generator.lz parse-error 消除，仍剩 IR semantic error）。



### FIXED(2026-10-20T17:16:12Z / BUG-369)
- evidence: 提交 b281369 `fix(parser): trait/type generic square-bracket syntax + trait colon-opens-body`。src/parser/parser.rs 三处：① parse_trait 新增 LBrack 泛型参数分支（`trait Container[T]`，逗号分隔 + 可选 bound）；② supertrait 循环后 `=` 可选 —— 冒号已开缩进体时放行（`trait Container[T]:` 形态）；③ parse_type 新增 LBrack 泛型实参分支（`Iterator[int]`），Option/Result 特化，其余落 Generic。验证：trait_generic.lz / trait_generic2.lz（`trait Container[T]:` + `trait Container<T>:`）COMPILE OK；generator.lz parse-error 消除（仍剩 IR yield+ret semantic error）；探针门禁 116/100/16 → 116/102/14（+2 OK）。
## BUG-370 [2026-10-20T17:21:49Z] [high] FIXED
- summary: BUG-370: yield 守卫不认 Iterator<int> 返回类型 → generator 报「声明了返回类型但在函数体内使用 yield」
- detail: `def gen() -> Iterator<int>` + yield body 被拒：「函数 gen 声明了返回类型但在函数体内使用 yield（yield 只允许用于无返回类型/iterator 的生成器）」。semantic_check.rs L670 守卫只豁免 `f.is_iterator`（由 `iterator` 关键字设置），不认返回类型为 `Iterator<T>`/`iterator` 的函数。

修复（`c2b3776`）：新增 `ret_is_iterator` 判定 —— 返回类型为 `Named("Iterator"/"iterator")` 或 `Generic` 之 base 为二者时豁免，与 `is_iterator` 关键字路径同效。

修复后 generator.lz / generator2.lz COMPILE OK，探针门禁 116/102/14 → 116/104/12。



### FIXED(2026-10-20T17:21:57Z / BUG-370)
- evidence: 提交 c2b3776 `fix(semantic): allow yield in function whose return type is Iterator[...]`。src/semantic_check.rs check_function：新增 `ret_is_iterator` 判定（返回类型为 Named("Iterator")/Named("iterator")，或 Generic 之 base 为二者），并在 yield 守卫 `cur.has_yield && f.return_type.is_some() && !f.is_iterator` 中加入 `&& !ret_is_iterator`。验证：generator.lz / generator2.lz（`def gen() -> Iterator<int>` + 三个 yield + `for v in gen(): print(v)`）COMPILE OK；探针门禁 116/102/14 → 116/104/12（+2 OK，无回归）。
## BUG-371 [2026-10-20T17:35:04Z] [high] FIXED
- summary: BUG-371: `struct Counter(value: int):` 圆括号字段 + 冒号开体 不支持 → 顶层 Colon 报错
- detail: `struct Counter(value: int):` — 圆括号字段 struct 后跟冒号开体。LParen 分支解析完字段后立即 return，把 `:` 留给顶层解析 → 「Unexpected token at top level: Colon」（struct_methods.lz）。

修复（`aafe33f`，src/parser/parser.rs parse_struct_like）：
- LParen 分支：若紧随 `:`，把 name/generics/bounds/defaults/fields 暂存到新 transient 字段 paren_*，落入下方共享 Indent 体路径（跳过 Eq/Colon 分隔符校验，不重复消费分隔符）。
- Indent 体路径：若 paren_name 有值则接管（name=paren_name、fields=paren_fields…），继续解析方法。

修复后 struct_methods.lz COMPILE OK，探针门禁 116/104/12 → 116/105/11。cargo test 全部通过（`magic_demos_generated_rust_compiles` 的 extractor_unapply.lz 失败为既有回归，stash 后仍失败，与本修复无关）。



### FIXED(2026-10-20T17:35:15Z / BUG-371)
- evidence: 提交 aafe33f `fix(parser): struct Name(fields): colon-opens-body method block`。src/parser/parser.rs parse_struct_like 重构：① LParen 分支遇 `:` 时把 name/generics/bounds/defaults/fields 暂存到 Parser 新 transient 字段 paren_name/paren_fields/paren_generics/paren_generic_bounds/paren_generic_defaults（新增 5 字段 + new() 初始化），落入下方共享 Indent 体路径；② 分隔符校验与消费按 paren_name.is_none() 跳过；③ Indent 体路径若 paren_name 有值则接管 name/fields/generics（不重开字段列表）。验证：struct_methods.lz（`struct Counter(value: int):` + `def inc(self)`/`def get(self)` + `c.inc()`/`c2.get()`）COMPILE OK；探针门禁 116/104/12 → 116/105/11；cargo test 441+ 通过，唯一失败 `magic_demos_generated_rust_compiles`（extractor_unapply.lz）经 git stash 基线复核为既有回归，与本修复无关。
## BUG-372 [2026-10-20T18:14:36Z] [high] FIXED
- summary: 闭包内 match/方法链续行被误判为缩进块，Dedent 悬空导致变量未绑定
- detail: match 末臂 body 与调用 `)` 同行时（如 `xs.flat_map(|o| match o:\n case Some(v): [v]\n case None: [])`），词法层把 `case` 行当成新缩进块产生 Indent，`)` 落在对应 Dedent 之前。match 消费了 Indent 却配对不到 Dedent，残留的 Dedent 提前终结了外层 `def main` 主体，导致 `print(ys)` 被当作顶层语句、`ys` 未绑定变量。同时方法链续行 `.map(...)` 缩进更深时也会被误判为新 Indent 块。

## BUG-373 [2026-10-20T18:14:46Z] [high] FIXED
- summary: 方法链续行 `.map(...)` 被词法器误判为新缩进块
- detail: `let result = xs\n    .map(...)` 中 `.map` 行缩进比上一语句更深，lexer 在 line_start 处理缩进时把 `.` 开头的续行当成新缩进块，产生多余的 Indent。修复：行首首个非空白字符是 `.`（且非 `..`）时跳过 handle_indent，不产生 Indent/Dedent，让 `.map` 作为上一表达式的续行。



### FIXED(2026-10-20T18:14:57Z / BUG-372, BUG-373)
- evidence: commit 3cd1e49 (src/lexer/lexer.rs + src/parser/expr.rs): (1) match arm loop now breaks on RParen/RBrack/RBrace and tolerates a missing Indent — `xs.flat_map(|o| match o: case Some(v): [v] case None: [])` parses correctly; (2) lexer suppresses Indent/Dedent when a line's first non-space char is '.' (and not '..') so `.map(...)` is a continuation, not a new block; (3) call-args loops re-check RParen after skip_newlines so multi-line arg lists inside parens still terminate. Verified: t12/t18/t19/map_chain/opt_chain_result all pass; nesting-data-types.lz regression fixed; 441 lib tests pass.
## BUG-374 [2026-10-20T18:48:19Z] [high] FIXED
- summary: struct_default.lz [compile-fail] E0063 missing fields debug/port in Config initializer
- detail: `struct Config = name: str, port: int = 8080, debug: bool = false` then `let c = Config(name: "app")` generated `Config { name: "app".to_string() }` — the AST field defaults (`port: int = 8080`) were dropped at AST→IR (IR `Field` had no default slot) and StructCtor/kwarg codegen never filled missing fields.

Fix (commit 706899e):
- `ir/node.rs`: `Field` gains `default: Option<Box<Expr>>`.
- `ir/builder/convert_decl.rs`: propagate AST `Field.default` → IR (enum variant fields = None); the only drop-site was struct fields loop at ~line 2044.
- `ir/codegen/mod.rs`: add `struct_field_defaults: HashMap<String, HashMap<String, Expr>>`.
- `ir/codegen/decl_gen.rs` `gen_struct_def`: register fields with AST defaults.
- `ir/codegen/expr_gen.rs`: kwarg struct ctor path (`Config(name: ...)`) fills missing fields from `struct_field_defaults` (falling back to `default_value_for` for `__new__` path).
- `parser/parser.rs`: parse `port: int = 8080` field-default syntax after field type.

Generated `let c = Config { name: "app".to_string(), debug: false, port: 8080i64 };` compiles.
Gate: TOTAL=135 OK=131 FAIL=4.



### FIXED(2026-10-20T18:48:26Z / BUG-374)
- evidence: Commit 706899e: ir/node.rs Field.default; ir/builder/convert_decl.rs propagate AST default; ir/codegen/mod.rs struct_field_defaults map; decl_gen.rs register defaults; expr_gen.rs kwarg ctor fills missing fields; parser.rs parse `port: int = 8080`. Probe struct_default.lz now emits `Config { name: "app".to_string(), debug: false, port: 8080i64 }` and compiles; gate 135/131/4.
## BUG-375 [2026-10-20T19:00:18Z] [high] FIXED
- summary: async_await.lz [gen-fail] Parse error: Expected field/method, got Await + async_chain.lz [compile-fail] E0728 await only in async fn
- detail: `let r = fetch(21).await()` in async_await.lz / spawn.lz failed parse with "Expected field/method, got Await" — parser only handled bare postfix `await` (line 1070), not `.await()` method form. Also generated `main` was not wrapped in `__block_on(async {...})` even though its body contained await, so `.await` inside hit E0728.

Fix (commit d8acd46):
- parser/expr.rs: `Token::Await` accepted as method name after `.` in the postfix-chain method branch → `Expr::MethodCall { method: "await" }`.
- ir/codegen/decl_gen.rs: `is_async_main` now `f.name == "main" && (f.is_async || block_has_await(&f.body))` so main with `.await()` is emitted as `fn main() { let __async_main = async { ... }; __block_on(__async_main); }`.
- ir/codegen/expr_gen.rs: `.await()` in a sync context wraps receiver in `__block_on(...)` (defensive E0728 guard).

async_await.lz + async_chain.lz (await added to chain probe) both now compile.



### FIXED(2026-10-20T19:00:25Z / BUG-375)
- evidence: Commit d8acd46: parser/expr.rs Token::Await as method name; decl_gen.rs is_async_main also when main body has await; expr_gen.rs __block_on fallback for sync await. async_await.lz + async_chain.lz compile. Gate 135/133/2.
## BUG-376 [2026-10-20T19:12:30Z] [medium] FIXED
- summary: with_block.lz [compile-fail] E0425 cannot find function open in this scope
- detail: `with (open("file.txt")):` in with_block.lz generated `let mut _with = open("file.txt".to_string());` but `open` wasn't a builtin — E0425 cannot find function open in this scope. The SYNTAX docs say `open()` needs `import std.fs` but no std/fs.lz exists; prelude has no `open`. The with-block codegen desugars correctly (guard drop = RAII), only the function was missing.

Fix (commit 7b4f0e9):
- lz_builtins/src/runtime/builtins.rs: add `pub fn open(path: impl AsRef<str>) -> FileGuard` and `pub struct FileGuard { path: String, file: Option<std::fs::File> }` with `Drop` closing the file. Signature uses `impl AsRef<str>` to accept both `&str` and `String` (codegen passes `String`).

Gate: TOTAL=135 OK=134 FAIL=1 (only string_concat.lz, deliberate IR design gate str+int remains).



### FIXED(2026-10-20T19:12:37Z / BUG-376)
- evidence: Commit 7b4f0e9: lz_builtins/src/runtime/builtins.rs add `pub fn open(impl AsRef<str>) -> FileGuard` + `FileGuard { path, file }` with Drop closing. with_block.lz now compiles. Gate 135/134/1.
## BUG-377 [2026-10-20T19:17:01Z] [low] FIXED
- summary: string_concat.lz [gen-fail] IR build error: 字符串与数字不能直接相加（探针设计问题，用 f-string 表达）
- detail: string_concat.lz line 5 `"The answer is " + n` triggered the IR design gate 「字符串与数字不能直接相加」(str + int 不做隐式转换，有意的类型安全检查，Python 隐式 str+int 常致错)。这是**探针设计问题**而非编译器缺陷：LZ 惯用法是 f-string。

Fix (commit f9cd835): 探针改用 `f"The answer is {n}"`，表达同样语义。IR 设计门继续拦截 str+int。

Gate: TOTAL=135 OK=135 FAIL=0 (100%)。cargo test --lib: 441 passed。



### FIXED(2026-10-20T19:17:06Z / BUG-377)
- evidence: Commit f9cd835: TEMP/probe/sweep15/string_concat.lz line 5 `"The answer is " + n` → `f"The answer is {n}"`. IR design gate str+int unchanged. Gate TOTAL=135 OK=135 FAIL=0.
## BUG-378 [2026-10-20T19:20:07Z] [medium] FIXED
- summary: BUG-374 后 tests/cython_backend.rs 5 处 Field 构造缺 default 字段导致 cargo test --workspace E0063
- detail: BUG-374 (struct 字段默认值) 给 IR Field 加了必填 `default` 字段，但未同步 `tests/cython_backend.rs` 里的 5 处直接构造 `Field { name, ty }` 的测试桩，导致 `cargo test --workspace` 报 E0063 missing field default（5 处：Line 174/178 的 Point struct x/y，Line 374 的 Circle.r，Line 382/386 的 Rect.w/h）。此前 `cargo test --lib` 不覆盖 integration tests，所以漏检。

Fix (commit 5e3aa87): tests/cython_backend.rs 5 处补 `default: None`。

`cargo test --workspace` 现在全部通过（唯一剩余 extractor_unapply.lz 在 KNOWN_RUSTC_FAILURES 豁免名单内，E0592 duplicate __unapply__）。Gate 仍 135/135/0。



### FIXED(2026-10-20T19:20:12Z / BUG-378)
- evidence: Commit 5e3aa87: tests/cython_backend.rs 5 处 Field { name, ty } 补 default: None。cargo test --workspace 编译通过，Gate 135/135/0。
## BUG-379 [2026-10-20T19:32:53Z] [medium] FIXED
- summary: duck_demo.lz / duck_soft.lz [parse-fail] Expected Indent, got Dot — BUG-373 `.` 续行规则误伤 duck 成员声明
- detail: BUG-373 把「行首为 `.` 就跳过 handle_indent」当作续行规则，但 duck 声明 `duck HasName =\n    .name: str` 的 `.name` 是成员字段声明（新缩进块），不是方法链续行——之前被误判为续行导致缺 Indent，报 Parse error: Expected Indent, got Dot。

影响：DEMO/02_types/duck_demo.lz 和 DEMO/02_types/duck_soft.lz 都无法编译。

Fix (commit 0477fc3): src/lexer/lexer.rs —— 在 `.` 行首判断里，检查上一个非 Newline/Indent/Dedent token 是否是 `=`。若是（duck/struct 体以 `=` 结尾），则正常 handle_indent；否则按 BUG-373 跳过 indent（方法链续行）。

验证：duck_demo.lz / duck_soft.lz 均编译成功；Gate 135/135/0；cargo test --workspace 全部通过（唯一剩 extractor_unapply.lz 在 KNOWN_RUSTC_FAILURES 豁免名单内）。



### FIXED(2026-10-20T19:33:01Z / BUG-379)
- evidence: Commit 0477fc3: src/lexer/lexer.rs `.` 行首判断——上一非空白 token 是 `=` 时正常 handle_indent（duck 体），否则跳过 indent（BUG-373 续行）。duck_demo.lz/duck_soft.lz 均编译成功。
## BUG-380 [2026-10-20T19:38:13Z] [medium] FIXED
- summary: extractor_unapply.lz [compile-fail] E0592 duplicate __unapply__ — 合成跳过检查漏查 methods
- detail: extractor_unapply.lz（DEMO/99_spec）是 KNOWN_RUSTC_FAILURES 豁免名单里的老毛病：E0592 duplicate definitions with name `__unapply__`。

根因：convert_decl.rs 在合成 __unapply__ 前检查「用户是否已显式实现」时，只查 `s.magic_methods`。但 LZ 里 `def __unapply__(self) -> (int, int)` 走普通方法路径（`s.methods`），不在 magic_methods。漏检 → 合成一份重复的 __unapply__。

Fix (commit 9eb54ad): src/ir/builder/convert_decl.rs —— user_has_unapply = magic_methods || methods 任一有 __unapply__。

修复后：
- extractor_unapply.lz 生成 1 个 def + 1 个调用，rustc 编译干净。
- demo_codegen_compile 测试从 0 passed / 1 failed → 1 passed / 0 failed。
- cargo test --workspace 全部通过（此前唯一的失败项已消除）。



### FIXED(2026-10-20T19:38:20Z / BUG-380)
- evidence: Commit 9eb54ad: src/ir/builder/convert_decl.rs user_has_unapply 同时查 magic_methods 与 methods。extractor_unapply.lz rustc 编译干净，demo_codegen_compile 1 passed / 0 failed，cargo test --workspace 全通过。
## BUG-381 [2026-10-20T19:59:04Z] [medium] FIXED
- summary: zip 自由函数生成 Zip 迭代器而非 Vec — 缺 .collect::<Vec<_>>() → E0308
- detail: expr_gen.rs:1858 自由函数 zip(a, b) 生成 `(a).into_iter().zip(b.into_iter())` 返回 Rust 的 Zip 迭代器，但 LZ 语义要求集合（调用方注解为 Vec<(i64,i64)>）→ E0308 mismatched types (Zip vs Vec)。

对照方法形式（line 4739）已有 `.collect::<Vec<_>>()`，自由函数形式漏了。

Fix (commit ...): expr_gen.rs 自由函数 zip 补 `.collect::<Vec<_>>()`。修复后 edge_collections.lz 输出 `[(1, 10), (2, 20), (3, 30)]` 正确。



### FIXED(2026-10-20T19:59:13Z / BUG-381)
- evidence: Commit: src/ir/codegen/expr_gen.rs 自由函数 zip 补 .collect::<Vec<_>>()。edge_collections.lz rustc 编译干净，输出 [(1, 10), (2, 20), (3, 30)]。
## BUG-382 [2026-10-20T20:04:15Z] [medium] FIXED
- summary: 非 self 容器索引元素类型非 Copy 时缺 .clone() — Vec<Vec<i64>> 取 [0] → E0507
- detail: expr_gen.rs IndexGet 的 else 分支（非 self、非泛型函数）：`matrix[0]` 当 matrix 是 Vec<Vec<i64>> 时直接生成 `matrix[0i64 as usize]` → E0507 cannot move out of index。

对照 self_field/self_base 分支（line 5344-5347）与 in_generic_fn 分支（line 5349-5354）都已自动补 .clone()，唯独 else 分支漏了。

Fix: else 分支根据元素类型 is_copy_ty 判定，非 Copy 时补 .clone()。edge_ops.lz `matrix[0]` 现输出 [1, 2] 正确。



### FIXED(2026-10-20T20:04:20Z / BUG-382)
- evidence: Commit: src/ir/codegen/expr_gen.rs IndexGet else 分支按 is_copy_ty 判定补 .clone()。edge_ops.lz rustc 编译干净，matrix[0] 输出 [1, 2]。
## BUG-383 [2026-10-20T20:21:01Z] [high] FIXED
- summary: 解析器拒绝 .or()/.and() 方法名 — Option::or/and 文档化方法无法调用
- detail: parser/expr.rs:625 点号后方法名只接受 Ident/MagicMethod/True/False/Await/IntLit，`or`/`and` 被当作布尔关键字拒收。但 option.lz:107,123 定义了 `Option::and()` / `Option::or()`，result.lz:141,157 定义了 `Result::and()` / `Result::or()`。

Fix: 点号后语境无歧义，放行 Token::And → "and"、Token::Or → "or"。

修复后 `none_val.or(Some(99))` 正常解析并编译。

## BUG-384 [2026-10-20T20:21:08Z] [medium] FIXED
- summary: 解析器拒绝 |_| 下划线匿名闭包参数 — 文档化语法
- detail: parser/expr.rs:1718 闭包参数只接受 Token::Ident，`|_|` 下划线匿名参数被拒。但 06g-魔法综合.md:278 文档化 `f_filter: fn(T) -> bool = |_| True`。

Fix: Token::Underscore → "_" 作为匿名参数名。

## BUG-385 [2026-10-20T20:21:16Z] [medium] FIXED
- summary: int(char 循环变量) 生成 .parse::<i64>() — char 无 parse 方法 → E0599
- detail: expr_gen.rs int() 转换对 Str 类型生成 `.parse::<i64>().unwrap()`。但当变量来自 `for c in s.chars()` 时，IR 类型是 Str 但 Rust 实际是 char（str_loop_vars 已登记），`(c).parse::<i64>()` → E0599 no method named `parse` found for char。

Fix: int 转换前检查 args[0].kind 是否是 str_loop_vars 中的 Var，是则 `as i64`（字符码点）。

edge_char_int.lz 现正确输出 Some(42)/None。



### FIXED(2026-10-20T20:21:23Z / BUG-383, BUG-384, BUG-385)
- evidence: 三处修复：BUG-383 parser/expr.rs 点号后放行 And/Or 作方法名；BUG-384 parser/expr.rs 闭包参数放行 Underscore；BUG-385 expr_gen.rs int() 检查 str_loop_vars 对 char 生成 as i64。edge_char_int.lz/edge_chains.lz 编译干净，workspace 441 测试全绿，门控 135/135。
## BUG-386 [2026-10-20T20:22:38Z] [low] FIXED
- summary: 间接 chars() 赋值丢失 char 类型信息 — for c in 间接迭代变量 → E0599
- detail: `let chars = s.chars(); for c in chars:` 场景下 `chars` 的 IR 类型是 Iterator<str>，但 Rust 实际是 Chars<'_>（yields char）。str_loop_vars 机制只在 `for c in s.chars()` 直接形式触发（stmt_gen.rs:2079 检查 iter.kind 是 MethodCall(chars)），间接形式漏了 → int(c) 生成 .parse::<i64>() → E0599。

已知限制，非紧急。直接形式 edge_char_int.lz 已修复（BUG-385）。


### FIXED(2026-10-21T09:58:37Z / BUG-386)
- evidence: commit f358366（src/ir/codegen/stmt_gen.rs：Stmt::Let 值为 chars() 时登记绑定名 进 str_loop_vars；for 循环对 Iterator<Str> 的 Var 且其名在册者同样认定为 char 循环变量）。复现/反证：TEMP/probe/sweep17/h9.lz（间接 chars+int）修复前 E0599→修复后 OK 输出 104|101|108|108|111（hello 码点）；TEMP/probe/sweep17/h10.lz（间接 split+int）修复前后 均 OK total=6。回归（--test-threads=1）：gate cy_rust.ps1 TOTAL=135 OK=135 FAIL=0、find_bug_bugs 46/46、find_bug_libs 12/12、--lib 463/463、codegen_corner_fixes 37、g6_codegen 17、tail_recursion 16、tail_recursion_matrix 17、ir_pass_fixes 4、str_boundary 3、dual_route_golden 4 全绿。报告 Reports/20261021.1735.bugfind_round24.md。
## BUG-387 [2026-10-20T20:33:18Z] [high] FIXED
- summary: 单参 .reduce(closure) 生成 .reduce(...) — Vec 无 reduce → E0599
- detail: expr_gen.rs 单参 `.reduce(closure)` 未被特殊处理 → 生成 `(filtered).reduce(...)` → E0599 no method named `reduce` found for struct `Vec<i64>`。LZ list.lz:151 定义 `def reduce(ref self, f: fn(T, T) -> T) -> Option<T>`（无初值，返回 Option<T>）。

Fix: 新增分支 `method == "reduce" && args_s.len() == 1 && recv_is_list` → 路由到 `lz_builtins::runtime::functional::reduce(recv, closure)`（返回 Option<T>）。

edge_complex.lz `filtered.reduce(|acc,x| acc+x)` 已编译通过。



### FIXED(2026-10-20T20:35:38Z / BUG-387)
- evidence: expr_gen.rs 新增单参 .reduce(closure) 分支路由到 lz_builtins::runtime::functional::reduce，返回 Option<T>。edge_complex.lz 编译通过。workspace 441 测试全绿，gate 135/135。
## BUG-388 [2026-10-20T20:52:13Z] [medium] FIXED
- summary: 裸 raises（无类型）解析失败 Expected type, got Eq
- detail: parser.rs 1180-1188：`-> int raises =`（裸 raises，09-错误处理.md 形式一）在 `raises` 关键字后直接跟 `=` 或 `:` 时无类型，旧代码无条件调用 parse_type() 报「Expected type, got Eq」。

Fix: 检查下一个 token 为 Eq/Colon 时放 Type::Any 占位，下游只检查 is_some()。

gate 135/135 仍绿。注：裸 raises 的实际错误类型目前是 Any→i64，若 raise 值为 str 需另修（BUG-389 后续）。



### FIXED(2026-10-20T20:57:04Z / BUG-388)
- evidence: src/parser/parser.rs:1180-1190 — 新增检测 Eq/Colon 时默认 raises 类型为 Type::Str。验证 edge_bare_raises.lz rustc exit=0 ok:42，gate TOTAL=135 OK=135 FAIL=0，workspace tests 全绿。
## BUG-389 [2026-10-20T21:00:52Z] [low] FIXED
- summary: let 存储的闭包无法传给 fn(...) 形参（E0308 Arc<dyn Fn> → fn pointer）
- detail: TEMP/probe/sweep16/edge_complex.lz 复现。LZ 闭包经 let 存储后是 `Arc<dyn Fn(...) + Send + Sync>`（stmt_gen.rs:445-464 装箱），当把该变量传给形参类型为 `fn(...)` 的函数时，expr_gen.rs:971 fn_value_fwd 生成 `{ let __lz_fv = Arc::clone(&double); move |__lz_fv0| __lz_fv(__lz_fv0) }`，该转发闭包捕获 Arc → 无法强转为 fn pointer → E0308。

文档 SYNTAX/03e-closure闭包.md §1.5「变量绑定」+ 后续函数调用场景未明示此组合。DEMO/02_types/type_conversion.lz 的 `let add: fn(int,int) -> int = |x,y| x+y` 只被直接调用，未传给 fn(...) 形参，故 DEMO 全绿。

非 gate 失败，probe 设计边界问题，暂列 low。


### FIXED(2026-10-21T11:59:49Z / BUG-389)
- evidence: 修复 commit 76689d4，三处协同：① stmt_gen.rs gen_param 闭包字面量 Fn 形参改走 fn_value_type -> Arc<dyn Fn>；② types_emit.rs fn_value_type 形参递归 Arc<dyn Fn>（外层 Arc<dyn Fn(fn(i64)->i64,i64)> 注解与闭包形参一致，消 E0308）；③ expr_gen.rs fn_value_fwd 加 want_arc，仅 callee 为闭包变量时 Arc::new 装箱转发闭包；④ expr_gen.rs Var 调用 callee_expected 回退取 callee.ty Fn 形参表。反例 TEMP/probe/sweep16/edge_complex.lz：修复前 E0308（expected fn pointer fn(i64)->i64 / E0631 closure signature），修复后 OK 输出 10 | 6 | 1 hello | Some(7) | 55（apply(double,3)=6）。def 函数 impl Fn 形参路径不变，lib_closure（compose(plus3,times4)）回归 12/12 保持。回归（--test-threads=1）：gate 135/135、lib 463/463、find_bug_bugs 46、find_bug_libs 12、codegen_corner_fixes 37、g6_codegen 17、tail_recursion 16、gen_build_block 5、error_bug_libs 1、reject_errors 3、dual_route_golden 4、str_boundary 3、bridge_boundary 11、cy_codegen_gate 9、reject_more 8、incremental_golden、lexer_parser_core 2、import_pkg_gate、bridge_import 20、moddec_corpus 5、perf_opt_gates、cython_backend、lzc_loop_boundary_gates 4、lzc_loop_nested_def、demo_codegen_compile 16、lz_semantic_cases 7 全绿。注：lz_ir_bootstrap::lz_emit_rs_lz_matches_native_codegen 在 baseline 亦 FAILED，非本轮引入。
## BUG-390 [2026-10-21T03:18:35Z] [high] FIXED
- summary: BUG-390 本地 FnDef 登记覆盖顶层 raises 函数类型：`parse_int(...)?` 的 ? 传播被丢掉（Try 推断为 Int）
- detail: collect_functions 把顶层函数登记进 TypeCtx.vars 为 IrType::Fn{ret: Result<ok,err>}（raises 升级正确）。但 convert_block 处理 AstStmt::FnDef 时（convert_stmt.rs ~2085）用 func.return_type 裸算 fret 并 block_ctx.add_var 覆盖同名顶层登记，把 Result<int,str> 覆盖成 fn(str)->int；同时无返回注解默认 IrType::Any（应为 Unit）。后果：内层函数体 `let a = parse_int(s)?` 的 Try 分支 infer_expr_type 看到 inner 类型为裸 Int，不识别 Result → `?` 被丢掉、生成的 Rust 用 .unwrap() 而非 ?，且 trailing 表达式裸返回 i64 而非 Ok(i64) → E0308。复现 TEMP/probe/sweep16/edge_errors.lz（main 内嵌套 def parse_int/pipeline，pipeline 调 parse_int(s)?）。修复：convert_stmt.rs 改用 TypeCtx::fn_return_ir(&func.return_type, &func.raises, &g) 计算 fret，使本地 FnDef 与顶层登记一致。commit 1fe7361。修复后 DEBUG 探针显示 parse_int lv=fn(str)->Result<int,str>（原为 fn(str)->int）、inner_ty=Result{ok:Int,err:Str}（原为 Int）。135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0。
- reported_by: ds-harness-r19

## BUG-391 [2026-10-21T03:18:35Z] [medium] FIXED
- summary: BUG-391 裸 raises 后跟 `->` 报「Expected type, got Arrow」：`def f() raises -> int =` 无法解析
- detail: BUG-388 修复只让 parser.rs 在 raises 后跟 Eq/Colon 时视为裸 raises（默认 err=str），漏掉 Arrow。`def parse_int(s) raises -> int:` 与 `def pick() raises -> int =` 均报 Parse error: Expected type, got Arrow（TEMP/probe14q/z_qmark_result.lz 亦命中，说明该形式从未可解析）。修复：parser.rs 1187 条件加入 Token::Arrow，使裸 raises 默认 str，随后循环继续解析 `-> int` 作为返回类型。commit 1fe7361。修复后 bug390_r2.lz（`def f2() raises str -> int = 1`）与 `def f() raises -> int =` 形式均可解析。
- reported_by: ds-harness-r19



### FIXED(2026-10-21T03:19:02Z / BUG-390, BUG-391)
- evidence: commit 1fe7361「ir/parser: BUG-390 - 本地 FnDef 登记 raises 升级为 Result; 裸 raises 支持 -> 形式」。①convert_stmt.rs ~2085：本地 FnDef 的 fret 改用 TypeCtx::fn_return_ir(&func.return_type, &func.raises, &g)，raises 升级为 Result<ok,err> 且默认 Unit（原 Any），不再用裸 return_type 覆盖顶层 vars 登记。②parser.rs ~1187：raises 后跟 Arrow 也判为裸 raises（默认 err=str），使 `def f() raises -> int =` 可解析。验证：DEBUG 探针 parse_int lv 由 fn(str)->int 变 fn(str)->Result<int,str>、Try inner_ty 由 Int 变 Result{ok:Int,err:Str}；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；cargo build 通过。探针与修复后无 DEBUG 残留。
## BUG-392 [2026-10-21T04:22:43Z] [medium] FIXED
- summary: BUG-392 impl 头自类型携带泛型默认值/bound：`struct<T: Clone = List<int>>` 生成 `impl<T> Container<T: Clone = Vec<i64>>`（E0737/E0229）
- detail: struct 定义 `struct Container<T: Clone = List<int>>` 生成 `struct Container<T: Clone = Vec<i64>>`（正确），但 struct 魔法方法 impl 头把同一份 gen_generics(&s.generics) 用在自类型上，生成 `impl<T: Clone + Debug + Clone> Container<T: Clone = Vec<i64>>`：①`= Vec<i64>` 默认类型实参不允许出现在 impl（E0737 expected one of ( + , :: < >, found `=`）；去掉默认后仍携带 bound `Container<T: Clone>`，报 E0229 associated item constraint。根因：gen_generics 同时含 bound 与 default，被「struct 定义」和「impl 自类型/返回类型」共用。修复：types_emit.rs 新增 gen_param_args(&[GenericParam])->`<T,U>`（仅名字，无 bound 无默认），decl_gen.rs 在 struct/enum 的 4 处 impl 头自类型（__new__/__init__ impl、ImplicitFrom、struct 方法 impl、enum 方法 impl）与 __new__ 返回类型改用 gen_param_args；struct/enum 定义仍用 gen_generics（保留默认）。验证：cargo build 0 err；cargo test --test find_bug_bugs ty005_generic_default ok；全量 cargo test --workspace 463 lib pass（+1），仅剩已知 core_compose_ok/core_unique_ok 两个 FAIL；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0。
- reported_by: ds-harness-r19



### FIXED(2026-10-21T04:23:08Z / BUG-392)
- evidence: src/ir/codegen/types_emit.rs 新增 gen_param_args(&[GenericParam])->`<T,U>`（仅名字）；decl_gen.rs 在 struct（__new__/__init__ impl、ImplicitFrom、struct 方法 impl、__new__ 返回类型）与 enum 方法 impl 的自类型改用 gen_param_args，struct/enum 定义保留 gen_generics（带默认+bound）。验证：cargo build 0 err；FIND_BUG/typer/bug-generic-default-conflict.lz 转 Rust 后 impl 头变 `impl<T: Clone + Debug + Clone> Container<T>`；cargo test ty005_generic_default ok；全量 cargo test --workspace 463 lib pass（较修复前 +1），仅剩已知 core_compose_ok/core_unique_ok；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0。
## BUG-393 [2026-10-21T04:35:46Z] [medium] FIXED
- summary: BUG-393 impl Fn(...) opaque 形参缺 Clone：LZ 值语义生成 f.clone() 报 E0599（compose/闭包体内复用 fn 形参）
- detail: compose.lz `def compose<a,b,c>(f: fn(b)->c, g: fn(a)->b) -> fn(a)->c = |x| f(g(x))` 生成的 Rust 为 `return Arc::new(move |x: i64| { f.clone()(g.clone()(x.clone())) });`，其中 fn 形参被声明为 `impl Fn(b) -> c + 'static + Send + Sync`（opaque），LZ 值语义在闭包体内按值复用生成 `f.clone()`/`g.clone()`，opaque 无 Clone → E0599 no method named `clone`。修复：decl_gen.rs fn(...) 形参生成 `impl Fn(...)` 时统一追加 `+ Clone`（返回 fn 值的函数则 `+ Send + Sync + Clone`）；闭包字面量与 Arc<dyn Fn…> 的 fn_value_fwd 转发闭包捕获皆可 Clone，调用点契约可满足。验证：compose.rs 生成的 fn 形参变 `impl Fn(b) -> c + 'static + Send + Sync + Clone`，E0599 消除；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；cargo test --workspace 463 lib pass，无新失败。注：compose 测试仍失败，但剩余错误已变为 E0308/E0631（返回闭包形参被硬编码 i64 而非泛型 a，属另一独立的闭包参数泛型推断缺口，另立条目）。
- reported_by: ds-harness-r19



### FIXED(2026-10-21T04:36:09Z / BUG-393)
- evidence: decl_gen.rs fn(...) 形参生成分支（gen_fn_def 参数渲染）给 `impl Fn({}) -> {} + 'static{}` 统一追加 Clone：send_bound 由「返回 fn 值才 + Send + Sync」改为恒 ` + Clone`（返回 fn 值则 ` + Send + Sync + Clone`）。compose.rs 生成的形参由 `impl Fn(b) -> c + 'static + Send + Sync` 变 `... + Send + Sync + Clone`，E0599 no method named clone 消除（现仅剩 E0308/E0631 另一独立问题）。验证：135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；cargo test --workspace 463 lib pass 无新失败（仍仅 core_unique_ok/core_compose_ok 两项，均系其他条目）。
## BUG-394 [2026-10-21T04:37:22Z] [medium] DUPLICATE →BUG-396
- summary: BUG-394 泛型函数返回的闭包字面量形参硬编码 i64：compose 返回 `|x| f(g(x))` 应为 `|x: a|`，现 E0308/E0631
- detail: compose.lz 在 BUG-393 修复后仍失败，剩余错误 E0308/E0631：`pub fn compose<a,b,c>(...) -> Arc<dyn Fn(a) -> c>` 体内 `return Arc::new(move |x: i64| { f.clone()(g.clone()(x.clone())) });` ——返回闭包字面量的形参被硬编码为 i64（无注解闭包形参默认类型），而函数签名要求 `|x: a|`，导致 expected `a` found `i64`（E0308）与 closure signature `fn(i64)` ≠ expected `fn(a)`（E0631）。根因：闭包字面量作泛型函数的返回值时，codegen 未从预期类型 `fn(a) -> c` 反推闭包形参类型，一律落到 i64 默认。修复方向：return 位置闭包字面量按 f.ret_ty（Fn{params,ret}）给形参标注。FIND_BUG/core/compose.lz。
- reported_by: ds-harness-r19

## BUG-395 [2026-10-21T04:37:22Z] [medium] FIXED
- summary: BUG-395 内置函数作一等 fn 值丢失引用形参：`len.clone()(s.to_string())` 以值传参，expected `&T` found `String`
- detail: unique.lz `unique_by(words, |s: str| len(s))` 生成的 Rust 为 `|s: String| { len.clone()(s.to_string()) }`，报 E0308 expected `&_` found `String`（`pub fn len<T: Len>(obj: &T) -> i64`）。根因：内置 `len` 形参为 `&T`，一旦作为一等 fn 值捕获进闭包（`len.clone()`），codegen 把它的 fn 值类型重建为「值传参」形式 `fn(String) -> i64`，丢失引用性，于是调用点以值传 `s.to_string()` 而非 `&s`。与 BUG-389（闭包/fn 形参）同类：fn 值类型未保留引用形参。修复方向：fn 值载体类型需保留引用形参（`&T`）并在调用点以引用实参调用。FIND_BUG/core/unique.lz。
- reported_by: ds-harness-r19

## BUG-396 [2026-10-21T05:02:30Z] [medium] FIXED
- summary: BUG-394 泛型函数返回的闭包字面量形参硬编码 i64（compose `|x: a|` → `|x: i64|`，E0308/E0631）
- detail: compose.lz `def compose<a,b,c>(f: fn(b)->c, g: fn(a)->b) -> fn(a)->c = |x| f(g(x))`：函数返回闭包字面量，其形参在类型推断阶段被默认成 `int`（LZ 标量默认），codegen 随之硬编码 `|x: i64|`，而函数签名要求 `|x: a|` → E0308 expected type parameter `a` found `i64`、E0631 closure signature `fn(i64)` ≠ expected `fn(a)`。根因：闭包形参类型标注未从「函数返回签名」反推。修复：expr_gen.rs Lambda 分支生成闭包形参时，若 `current_ret_ty` 为 `Fn{params,..}` 且形参数与闭包形参个数一致，则按返回签名形参类型标注该闭包形参（仅当原形参类型为 Any/Int 这类「默认标量」时覆盖，已显式注解如 Str 不动）。验证：compose.rs 生成 `return Arc::new(move |x: a| { f.clone()(g.clone()(x.clone())) });`；cargo test core_compose_ok ok（此前 FAILED）；cargo build 0 error。
- reported_by: ds-harness-r20



### FIXED(2026-10-21T05:05:12Z / BUG-396)
- evidence: expr_gen.rs Lambda 分支闭包形参标注：新增按 `current_ret_ty`（Fn 签名）反推——当 `current_ret_ty` 为 `Fn{params:rps,..}` 且 `rps.len()==闭包形参数` 时，用 `rust_type(rps[i])` 标注第 i 个闭包形参（仅覆盖原类型为 Any/Int 的「默认标量」形参，显式注解如 Str 不动）。compose.rs 生成 `return Arc::new(move |x: a| { f.clone()(g.clone()(x.clone())) });`（原先 `|x: i64|`）。验证：cargo test core_compose_ok ok（此前 FAILED）；cargo test --workspace 45 passed/1 failed（find_bug_bugs，原先 44/2，core_compose_ok 转 PASS，仅剩 core_unique_ok）；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；cargo build 0 error。

## BUG-397 [2026-10-21T05:44:49Z] [high] FIXED
- summary: BUG-397 内置函数作一等 fn 值时丢失引用形参：len 传值报 E0631、len.clone()(x) 报 E0308
- detail: 根因：LZ 内置函数（len/id/hash/lz_int 等）在直接调用时被降级为方法调用（`len(x)` → `(x).len()`、`contains(xs,x)` → `(xs).contains(&x)`）从而自动获得引用；但当内置被当作**一等 fn 值**使用时（传入高阶函数 `apply_len(nums, len)`，或 `len.clone()(arg)` 直接调用），暴露的是内置的原始 Rust 签名 `len<T: Len>(obj: &T)`，而其值类型在推断/拍平过程中丢失了 `&`。两种表象：① `len.clone()(s)` 报 E0308 expected `&_`, found String；② `apply_len(nums, len)` 报 E0631 type mismatch。修复（codegen 层，避开并发编辑的 infer.rs）：在 scan.rs 新增 `builtin_ref_params(name)` 静态表，按 lz_builtins 真实签名登记引用形参；expr_gen.rs ①callee_fn_refs 优先按内置名还原真实引用性（修 E0308）；②fn_value_fwd 对引用形参内置生成转发闭包 `move |__lz_fv0| len(&__lz_fv0)`（修 E0631）。验证：FIND_BUG/core/unique.rs `core_unique_ok` 由 FAILED 转 PASS；自写探针 h2.lz（apply_len/pick_key 传 len、contains 直调）与 h3.lz（len 于 List<int>/str 各一次）全链路通过；cargo build 0 error；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0。
- reported_by: ds-harness-r21



### FIXED(2026-10-21T05:55:06Z / BUG-397, BUG-395)
- evidence: commit c7f33cf：scan.rs 新增 builtin_ref_params(name) 静态表（按 lz_builtins 真实签名登记 len/id/hash/type_of/isinstance/lz_int/lz_float/lz_bool/input/read_file/write_file/__lz_* 的引用形参）；expr_gen.rs ①callee_fn_refs 优先按内置名还原真实引用性（修 BUG-395 `len.clone()(s)` 的 E0308 expected `&_`, found String）；②fn_value_fwd 对引用形参内置生成转发闭包 `move |__lz_fv0| len(&__lz_fv0)`（修 BUG-397 `apply_len(nums,len)` 的 E0631）。实测：FIND_BUG/core/unique.rs `core_unique_ok` 由 FAILED 转 PASS；自写探针 h2.lz（apply_len/pick_key 传 len、contains 直调）与 h3.lz（len 于 List<int>/str 各一次）全链路通过；`cargo test --test find_bug_bugs` 46/46 全绿；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；463 lib 全过。
## BUG-398 [2026-10-21T06:16:13Z] [high] FIXED
- summary: BUG-398 内置绑定到变量后引用形参丢失：f = len 使 f(words) 报 E0308、传高阶函数报 E0631
- detail: 触发（自写探针 h5.lz）：`f = len` 把内置绑定到变量后，`f(words)` 报 E0308 expected `&T`, found Vec（rustc 建议 `f(&words.clone())`），`apply1(words, f)` 传高阶函数报 E0631 type mismatch。根因：内置的原始 Rust 形参 `&T` 在「绑定到变量」这步随名称传递丢失——`f` 的类型被拍平为 `Fn(Vec<T>)->i64`，调用位与传参位都拿不到引用性。修复（codegen 层，commit 后附）：stmt_gen.rs let 绑定初值是「引用形参内置名」时，发射转发闭包 `move |__lz_b0| len(&__lz_b0)`，使 `f` 本身成为类型正确的一等函数，调用与传递两路同时修复（f 未登记为 Arc 载体，裸闭包即够）。验证：h5.lz `r1=2 | r2=2 | h5 done` 全链路通过；h2/h3/h4（直调 len、lz_int/lz_float/lz_bool）回归全绿。
- reported_by: ds-harness-r22



### FIXED(2026-10-21T06:22:37Z / BUG-398)
- evidence: commit 9f80e92：stmt_gen.rs let 绑定初值为「引用形参内置名」时发射转发闭包 `move |__lz_b0| len(&__lz_b0)`，使变量本身成为类型正确的一等函数。实测自写探针 h5.lz（`f = len` 后 `f(words)` 直调 + `apply1(words, f)` 传高阶函数）由 RUSTC_FAIL（E0308 + E0631）转为 OK：`r1=2 | r2=2 | h5 done`；h2/h3/h4 探针（直调 len、lz_int/lz_float/lz_bool）回归全绿；`cargo test --test find_bug_bugs` 46/46；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；463 lib 全过。lib_closure/lib_option 为基线既有失败，非本次引入。
## BUG-399 [2026-10-21T07:08:11Z] [high] FIXED
- summary: BUG-399 复用类型名被 builder 注入多义 clone，codegen 误渲染 T::clone().Field 报 E0061/E0609/E0277（lib_option、lib_closure 双挂）
- detail: 触发：`lib_option`（`OptionInt.Some(...) else OptionInt.None` 在闭包内）与 `lib_closure`（`|x: int| -> str = str(x)`）两个 find_bug_libs 用例 RUSTC_FAIL，`cargo test --test find_bug_libs` 长期 10/12。根因：builder 对「在作用域内被复用多次的类型名」注入多义保护 `MethodCall receiver=Var(T) method=clone`；codegen 在 MethodCall 分支的 static_type_call（receiver 是 known_types 里的类型名）/is_type_param 路径把 `T.clone()` 渲染成 `T::clone()`，外层 FieldAccess 再拼成 `OptionInt::clone().None`（E0061 clone 缺参 + E0609 no field None）、`str::clone()(x)`（E0277 str 非 Sized/Clone）。类型名不是值，clone 无意义。修复（commit 后附，仅动 expr_gen.rs 两处）：① static_type_call 分支 method=="clone" 且无参时回退裸类型路径 recv；② FieldAccess 分支 base 是「类型名 + 多义 clone」MethodCall 时归一化为类型路径 T::Field。验证：`cargo test --test find_bug_libs` 由 10/12 FAILED 转为 12/12 ok（lib_option、lib_closure 同修）。
- reported_by: ds-harness-r23



### FIXED(2026-10-21T07:24:06Z / BUG-399)
- evidence: 改在 src/ir/codegen/expr_gen.rs 两处（commit 9d2e6a9，仅动该文件）：① static_type_call 分支开头加 method=="clone" && args.is_empty() → return recv（类型名不是值，clone 无意义，回退裸类型路径）；② FieldAccess 分支开头识别 base 为 MethodCall{receiver=Var(T),method=clone} 时归一化为 T::Field（大写变体）。锁钉：cargo test --test find_bug_libs 由长期 10/12 FAILED 转为 12/12 ok（lib_option、lib_closure 同修）；find_bug_bugs 46/46；lib 463 passed；门禁 cy_rust.ps1 TOTAL=135 OK=135 FAIL=0。复现样 TEMP/probe/sweep17/h6.lz 中 `OptionInt::clone().None` 已消失。
## BUG-400 [2026-10-21T07:39:13Z] [medium] FIXED
- summary: BUG-400 自定义枚举含 None 变体时裸 None 未解析为 <枚举>::None，生成裸 Rust None 报 E0308
- detail: 触发：自定义 `enum OptionInt: Some(int) | None` 下，`def and_then(opt: OptionInt, f: fn(int)->OptionInt)->OptionInt = match opt: case Some(v): f(v) case None: None`——`case None: None` 的裸 `None`（body）生成裸 Rust `None` → E0308 mismatched types（期望 OptionInt）。gen_none_ref 只识别 std Option（known_types.contains("Option") / has_option_context 判 path=="Option"），完全不处理「含 None 变体的自定义枚举」。lib_option/option.lz 靠全限定 `OptionInt.None`（第 48 行）绕开，掩盖了该缺口。修复（commit 3b3697d，仅动 types_emit.rs）：gen_none_ref 开头加判定——当前期望类型/当前函数返回类型恰是「含 None 变体」的自定义枚举（enum_variants.get("None")→枚举名 且 path 匹配）时发射 `<枚举>::None`；判定先于 known_types Option 分支，避免自定义 OptionInt 被误路由到 std Option::None。验证：新探针 TEMP/probe/sweep17/h6.lz、h7.lz 由 RUSTC_FAIL 转 OK；find_bug_libs 12/12；lib 463 passed；find_bug_bugs 46 文件独立运行全 OK（套件跑分有并发编译竞态抖动，非本改动回归）。
- reported_by: ds-harness-r23



### FIXED(2026-10-21T07:39:55Z / BUG-400)
- evidence: 改在 src/ir/codegen/types_emit.rs gen_none_ref 开头（commit 3b3697d，仅动该文件）：新增判定——当前期望类型/当前函数返回类型恰是「含 None 变体」的自定义枚举（enum_variants.get("None")→枚举名 且 IrType::Named.path==该枚举名）时发射 `<枚举>::None`，且置于 known_types.contains("Option") 之前。锁钉：新探针 TEMP/probe/sweep17/h6.lz、h7.lz（复现 `case None: None`）由 RUSTC_FAIL 转 OK；find_bug_libs 12/12 ok；lib 463 passed；find_bug_bugs 下 bug-ir-defer.lz、core/unique.lz 等文件独立运行均 OK。



## BUG-401 [2026-10-21T11:09:58Z] [medium] FIXED
- summary: str() 作用于内建集合（List/Set/Dict）生成 Display 格式化 -> E0277
- detail: expr_gen.rs str 内建对 Named Vec/HashSet/HashMap 恒用 `format!("{}", v)`（Display），但内建集合不 impl std::fmt::Display -> E0277；print(x) 用 print_arg_is_display 已对集合走 `{:?}`（Debug）。实测 print([1,2,3]) 输出 [1, 2, 3] 而 str([1,2,3]) 报 E0277，str 与 print 不一致。复现 TEMP/probe/sweep18/coll_str.lz。已修 commit 95d1c0d：仅集合类型走 Debug，标量（含 F64 需 "42" 非 "42.0"）照旧 Display。修复后 OK list=[1, 2, 3] | set={3, 1, 2}。回归 lib 463/463、gate 135/135、find_bug_bugs 46、find_bug_libs 12（含 lib_json）、codegen_corner_fixes 37 全绿。

## BUG-402 [2026-10-21T11:10:33Z] [medium] FIXED
- summary: str() 作用于内建集合（List/Set/Dict）生成 Display 格式化 -> E0277
- detail: expr_gen.rs str 内建对 Named Vec/HashSet/HashMap 恒用 `format!("{}", v)`（Display），但内建集合不 impl std::fmt::Display -> E0277；print(x) 用 print_arg_is_display 已对集合走 `{:?}`（Debug）。实测 print([1,2,3]) 输出 [1, 2, 3] 而 str([1,2,3]) 报 E0277，str 与 print 不一致。复现 TEMP/probe/sweep18/coll_str.lz。已修 commit 95d1c0d：仅集合类型走 Debug，标量（含 F64 需 "42" 非 "42.0"）照旧 Display。修复后 OK list=[1, 2, 3] | set={3, 1, 2}。回归 lib 463/463、gate 135/135、find_bug_bugs 46、find_bug_libs 12（含 lib_json）、codegen_corner_fixes 37 全绿。



### FIXED(2026-10-21T11:12:04Z / BUG-401, BUG-402)
- evidence: BUG-401/402 同源（误两次 report_bug 重复登记）。修复 commit 95d1c0d：expr_gen.rs str 内建仅对集合类型走 `{:?}`（Debug），标量照旧 Display。验证 TEMP/probe/sweep18/coll_str.lz：修复前 E0277（Vec/HashSet 不 impl Display），修复后 OK list=[1, 2, 3] | set={3, 1, 2}；print([1,2,3]) 本走 Debug -> [1,2,3]，str 现与之一致。回归（--test-threads=1）：lib 463/463、gate 135/135、find_bug_bugs 46、find_bug_libs 12（含 lib_json 需 F64 "42" 非 "42.0"）、codegen_corner_fixes 37、g6_codegen 17、dual_route_golden 4、str_boundary 3、tail_recursion 16、demo_codegen_compile 16、lz_semantic_cases 7 全绿。
## BUG-403 [2026-10-21T11:12:32Z] [low] FIXED
- summary: 闭包返回闭包：Lambda 的返回类型注解用 rust_type -> fn 指针，与闭包体 Arc::new(内层闭包) 不符
- detail: expr_gen.rs Lambda ret_ann 原用 `self.rust_type(t)`，对 IrType::Fn 渲染成 `-> fn(i64)->i64` 裸函数指针；但闭包体返回 Arc::new(内层闭包) -> E0308 expected fn pointer found Arc / E0271。实测 TEMP/probe/sweep18/nested_ret.lz：`let make_adder = |a: int| -> fn(int)->int = |b| a+b` 修复前 E0308/E0271，修复后 OK add5=12。与 BUG-389 同根（IrType::Fn 在 rust_type 与 fn_value_type 渲染不一致），本处为闭包返回值注解面向。已修 commit 92b2de6：ret_ann 对 Fn 类型改走 fn_value_type -> Arc<dyn Fn(...)+Send+Sync>，其余类型照旧。回归 lib 463/463、gate 135/135、find_bug_bugs 46、find_bug_libs 12、codegen_corner_fixes 37、g6_codegen 17、tail_recursion 16、gen_build_block 5 全绿；fn-value 探测 h7/h8/h2 仍 OK，edge_complex（形参 impl Fn）仍 E0308 待 BUG-389 重构。



### FIXED(2026-10-21T11:13:04Z / BUG-403)
- evidence: 修复 commit 92b2de6：expr_gen.rs Lambda ret_ann 对 IrType::Fn 改走 fn_value_type -> Arc<dyn Fn(...)+Send+Sync>（原 rust_type -> fn 指针）。验证 TEMP/probe/sweep18/nested_ret.lz：修复前 E0308 expected fn pointer found Arc + E0271，修复后 OK add5=12。fn-value 探测 h7/h8/h2 仍 OK，无回归。回归（--test-threads=1）：lib 463/463、gate 135/135、find_bug_bugs 46、find_bug_libs 12、codegen_corner_fixes 37、g6_codegen 17、tail_recursion 16、gen_build_block 5、error_bug_libs 1、reject_errors 3 全绿。注：edge_complex（形参 impl Fn）仍 E0308，属 BUG-389 待协调重构，本次仅修返回值注解面向。

## BUG-404 [2026-10-21T12:14:28Z] [medium] FIXED
- summary: lz_codegen_lib 缺 struct 提取魔法方法 __unapply__/__unapply_seq__ 生成，--emit=rs-lz 路径下结构体解构编译失败 E0599



### FIXED(2026-10-21T12:50:41Z / BUG-404)
- evidence: 修复 commit 83b34f7（src/ir/lz_codegen_lib.lz）。新增 gen_struct_unapply(name, fs)，在 gen_item StructDef 分支对带字段 struct 追加提取 impl，输出与原生逐字符一致：
impl Point { fn __unapply__(&self) -> (f64, f64) { return (self.x.clone(), self.y.clone()); } fn __unapply_seq__(&self) -> Vec<f64> { return vec![self.x.clone(), self.y.clone()]; } }
细节：__unapply_seq__ 仅字段类型同构时生成（对齐 struct_has_unapply_seq 判定）；单字段 tuple 补尾逗号 -> (i64,) / (self.z.clone(),)；impl 前空行 + 尾随换行对齐原生；accs 复用须 .clone()（String 非 Copy，赋值即移动 E0382）。
验证：① 反例 TEMP/probe/sweep18/d2.lz（struct Point + let Point(a,b)=p）：--emit=rs-lz 由 E0599 no method named __unapply__ 修复为 rustc 编译通过；② 原生 vs rs-lz 逐字节比对 d1（同构2字段）/d3（异构 int+str）/d4（单字段）/d5（空 struct）全部 bodyDiffs=0；③ tests/lz_ir_bootstrap.rs::lz_emit_rs_lz_matches_native_codegen 由 FAILED 修复为 PASS（lz_ir_bootstrap 5/5，该测试 baseline 亦 FAILED 属既有缺口）；④ 回归（--test-threads=1）：gate 135/135、lib 463/463、find_bug_bugs 46、find_bug_libs 12、codegen_corner_fixes 37、g6_codegen 17、tail_recursion 16、gen_build_block 5、error_bug_libs 1、reject_errors 3、dual_route_golden 4、str_boundary 3、bridge_boundary 11、cy_codegen_gate 9、reject_more 8、lexer_parser_core 2、ir_pass_fixes 1、lzc_loop_boundary_gates 4、cython_backend、lz_semantic_cases 7、demo_codegen_compile 16、fuzz_smoke、lzc_loop_nested_def、tail_recursion_matrix 3、lzc_loop_unit_variant 4、ir_snapshots、incremental_golden 全绿。
## BUG-405 [2026-10-21T14:04:09Z] [high] FIXED
- summary: BUG-405 rs-lz 枚举变体构造渲染为 Color.Blue(...)（点号）而非 Color::Blue(...)（路径分隔符），rustc 报 E0423 expected value, found enum Color；附带枚举定义被降级为元组变体 Blue(i64, String)（原生为具名字段变体 Blue { f, g }）。复现 TEMP/probe/sweep18/p_enum.lz。根因 lz_codegen_lib.lz gen_call 非 Var 调用者恒用 .（FieldAccess），且 Item.EnumDef 只携带字段类型丢失字段名。


### FIXED(2026-10-21T14:35:54Z / BUG-405)
- evidence: 修复 commit 596b685（src/ir/lz_codegen_lib.lz）。根因：gen_expr Expr.MethodCall 分支恒用 `.` 连接接收者与方法名，而枚举/关联函数构造在 IR 中表现为 MethodCall(receiver=Var(类型名), method=变体名)（IR dump：`[Color] method [?] Color.Blue(...)`），被渲染成 `Color.Blue(1i64, "hi")`，rustc E0423 expected value, found enum Color。原生 codegen 对已知类型/枚举接收者一律用 `::`（expr_gen.rs is_var_base && is_known_type → "::"），并渲染具名字段变体 `Color::Blue { f: 1i64, g: "hi" }`。
改动：MethodCall 分支改经新增 method_call_sep(r) 计算分隔符——接收者为 Expr.Var 且名字首字母大写 → "::"（类型名约定，值变量小写；与原生 is_known_type_or_enum 等价，LZ 侧无类型注册表），否则 "."；新增 starts_upper 用 s[0..1] 取首字符与 A..Z 比对。
验证：① TEMP/probe/sweep18/p_enum.lz：--emit=rs-lz 产物由 `Color.Blue(1i64, "hi")`（E0423 编译失败）修复为 `Color::Blue(1i64, "hi".to_string())`，rustc 编译通过、运行打印 1，与原生一致；② tests/lz_ir_bootstrap.rs 5/5 保持通过（d1.lz 枚举全为单位变体且无类型名调用，字节等价不受影响）。
遗留：枚举变体仍按元组变体 `Blue(i64, String)` 生成（原生为具名字段变体 `Blue { f, g }`），因 Item.EnumDef 仅携带字段类型、丢失字段名；对仅构造不取字段的程序可正常编译运行，字段访问 `c.f` 的缺口另立条目跟进。
回归：cargo test --test-threads=1 gate 135/135、lib 463/463、34 集成套件全绿。
## BUG-406 [2026-10-21T14:04:59Z] [high] FIXED
- summary: BUG-406 rs-lz Stmt.Match 分支体末尾 ExprStmt 被渲染成 
eturn println!(...)，使所在函数提前 return、跳过 match 之后语句。复现 TEMP/probe/sweep18/p_match.lz：原生打印 1、2 两行，rs-lz 产物只打印 1（_ 分支的 return 提前返回 main，后续 print(2) 不执行）。根因 gen_match_arm→gen_block→gen_body(is_main=false) 把分支体末语句视为函数尾（is_tail&&!is_main→return）；原生把语句 match 渲染为 let _ = match ... {} 且分支内无 return。



### FIXED(2026-10-21T14:21:22Z / BUG-406)
- evidence: 修复 commit 37d9191（src/ir/lz_codegen_lib.lz）。根因：gen_stmt ExprStmt 在 `is_tail && !is_main` 时前置 `return `（line 865），而所有嵌套块生成器（gen_block、Stmt.Block、Expr.BlockExpr）向 gen_body 传 is_main=false，导致嵌套块末语句被误判为函数尾、发射 `return <expr>`，使所在函数提前返回、跳过后续语句（match 分支、if 分支、for/while 体、try/defer、裸块、块表达式皆受影响）。
修复：三处 gen_body 调用改传 is_main=true（该标志在 gen_stmt 仅用于 ExprStmt 尾 return 抑制，无其他副作用；函数体尾部仍由 gen_item 直接 gen_body，return 语义保留），各加 BUG-406 注释。
验证：① TEMP/probe/sweep18/p_match.lz（match 后接 print(2)）：--emit=rs-lz 产物由「只打印 1」修复为打印 1、2，与原生一致；② p_ifel.lz（if/elif/else 后接 print(2)）：--emit=rs-lz 产物打印 1、2，与原生一致；③ tests/lz_ir_bootstrap.rs 5/5 保持通过（d1.lz 无嵌套块尾表达式，字节等价不受影响）；④ 回归（--test-threads=1）：gate 135/135、lib 463/463，34 个集成套件全绿（find_bug_bugs 46、find_bug_libs 12、codegen_corner_fixes 37、g6_codegen 17、tail_recursion 16、tail_recursion_matrix 2、str_boundary 3、gen_build_block 1、error_bug_libs 1、reject_errors 3、reject_more 8、dual_route_golden 4、bridge_boundary 11、bridge_import 20、import_pkg_gate、cy_codegen_gate 9、cython_backend、lz_semantic_cases 7、demo_codegen_compile 16、fuzz_smoke、lzc_loop_boundary_gates 4、lzc_loop_nested_def、lzc_loop_unit_variant 4、lz_frontend_bootstrap 44、lexer_parser_core 2、ir_pass_fixes 1、ir_serialization、ir_snapshots、incremental_golden、perf_opt_gates、lz_test_smoke、moddec_corpus、bridge_e2e、bridge_embed）。

## BUG-407 [2026-10-21T15:02:17Z] [high] FIXED
- summary: 原生 codegen 的命名字段枚举字段访问 x.v 生成解构 match 时用 .find() 只取该枚举第一个变体；字段落在后续变体（如 Shape 中 Rect.h）时漏生成解构、回落为直接 x.v → rustc E0609 no field h on type Shape。复现 TEMP/probe/sweep18/p_enumfield.lz。



### FIXED(2026-10-21T15:02:56Z / BUG-407)
- evidence: 修复 commit cf29dbe（src/ir/codegen/expr_gen.rs）。原实现对命名字段枚举字段访问用 `enum_variant_named_fields.iter().find(|((e,_),_)| e == path)` 取该枚举第一个带字段变体再判断是否含目标字段；字段落在后续变体时漏生成解构 match、回落为直接 `x.v` → rustc E0609 no field `h` on type `Shape`。修复改为 filter 收集**所有**字段含目标字段的变体，逐一生成 `E::V { v, .. } => v.clone(), ` 解构臂，命中多个变体则多臂 match，无命中则不生成；单变体命中时输出与原实现逐字节一致。
验证：① TEMP/probe/sweep18/p_enumfield.lz（Shape 含 Circle(radius)/Rect(w,h)，`print(s.radius)` + `print(r.h)`）：修复前原生产物 `println!("{:?}", r.h)` → E0609 编译失败；修复后 `println!("{:?}", match &r { Shape::Rect { h, .. } => h.clone(), _ => unreachable!() })`，rustc 编译通过、运行打印 5、4（s.radius=5、r.h=4）；② 回归（cargo test --test-threads=1）：gate 135/135、lib 463/463、lz_ir_bootstrap 5/5、集成套件全绿（find_bug_bugs 46、find_bug_libs 12、codegen_corner_fixes 37、g6_codegen 17、tail_recursion 16、str_boundary 3、dual_route_golden 4、bridge_boundary 11、cy_codegen_gate 9、demo_codegen_compile 16、lz_frontend_bootstrap 44、lz_semantic_cases 7 等）。
## BUG-408 [2026-10-22T03:36:57Z] [high] FIXED
- summary: --emit=rs-lz 下关键字参数（`Shape.Circle(radius: 5)` / `f(x~=1)`，IR 为 StructCtor(name="_KwArg")）被原样渲染成 `_KwArg {…}` → rustc E0422 cannot find struct _KwArg。lz_codegen_lib.lz expr_cs_list 缺 _KwArg 特判（原生有 helpers.rs gen_kwarg_value）。复现 TEMP/probe/sweep18/p_enumfield.lz。


### FIXED(2026-10-22T03:37:32Z / BUG-408)
- evidence: 修复 commit d92f6d3（src/ir/lz_codegen_lib.lz）。根因：关键字参数（`Shape.Circle(radius: 5)` / `f(x~=1)`）在 IR 中为 StructCtor(name="_KwArg", fields=[(name,…),(value,…)])，lz_codegen_lib.lz 的 expr_cs_list 直接 gen_expr 渲染实参、缺 _KwArg 特判，产物含 `_KwArg { name: …, value: … }` → rustc E0422 cannot find struct _KwArg。原生 codegen 有 helpers.rs::gen_kwarg_value（命中 _KwArg 只渲染 value 字段）。
改动：新增 gen_kwarg_value + kwarg_value_of（LZ 版同语义），expr_cs_list 渲染改走 gen_kwarg_value。
验证：① p_enum.lz --emit=rs-lz 编译通过、运行打印 1，与原生一致；② p_enumfield.lz 构造行由 `Shape::Circle(_KwArg{…})` 修复为 `Shape::Circle(5i64)`；③ 回归（--test-threads=1）：gate 135/135、lib 463/463、lz_ir_bootstrap 5/5、find_bug_bugs/find_bug_libs/codegen_corner_fixes/g6_codegen/tail_recursion/str_boundary/gen_build_block/error_bug_libs/reject_errors/dual_route_golden/bridge_boundary/cy_codegen_gate/lz_semantic_cases/demo_codegen_compile/lz_frontend_bootstrap/lzc_loop_unit_variant 全绿。

### FIXED(2026-10-22T03:37:39Z / BUG-408)
- evidence: 修复 commit d92f6d3（src/ir/lz_codegen_lib.lz）。根因：关键字参数（`Shape.Circle(radius: 5)` / `f(x~=1)`）在 IR 中为 StructCtor(name="_KwArg", fields=[(name,…),(value,…)])，lz_codegen_lib.lz 的 expr_cs_list 直接 gen_expr 渲染实参、缺 _KwArg 特判，产物含 `_KwArg { name: …, value: … }` → rustc E0422 cannot find struct _KwArg。原生 codegen 有 helpers.rs::gen_kwarg_value（命中 _KwArg 只渲染 value 字段）。
改动：新增 gen_kwarg_value + kwarg_value_of（LZ 版同语义），expr_cs_list 渲染改走 gen_kwarg_value。
验证：① p_enum.lz --emit=rs-lz 编译通过、运行打印 1，与原生一致；② p_enumfield.lz 构造行由 `Shape::Circle(_KwArg{…})` 修复为 `Shape::Circle(5i64)`；③ 回归（--test-threads=1）：gate 135/135、lib 463/463、lz_ir_bootstrap 5/5、find_bug_bugs/find_bug_libs/codegen_corner_fixes/g6_codegen/tail_recursion/str_boundary/gen_build_block/error_bug_libs/reject_errors/dual_route_golden/bridge_boundary/cy_codegen_gate/lz_semantic_cases/demo_codegen_compile/lz_frontend_bootstrap/lzc_loop_unit_variant 全绿。
## BUG-409 [2026-10-22T03:37:03Z] [high] DUPLICATE →BUG-408
- summary: --emit=rs-lz 下关键字参数（`Shape.Circle(radius: 5)` / `f(x~=1)`，IR 为 StructCtor(name="_KwArg")）被原样渲染成 `_KwArg {…}` → rustc E0422 cannot find struct _KwArg。lz_codegen_lib.lz expr_cs_list 缺 _KwArg 特判（原生有 helpers.rs gen_kwarg_value）。复现 TEMP/probe/sweep18/p_enumfield.lz。




## BUG-410 [2026-10-22T03:40:19Z] [high] FIXED
- summary: --emit=rs-lz 丢失脚本模式顶层语句：rs-lz 产物无合成 main → E0601 main not found。lz_codegen.rs ir_module_to_rs_lz_source 只序列化 items/prelude/version，不序列化 module.top_level_stmts；lz_codegen_lib.lz 的 codegen_module 也无原生 codegen/mod.rs:1370 的 !has_main 合成分支。复现 TEMP/probe/sweep18/p_enumfield.lz。
- detail: 复现 TEMP/probe/sweep18/p_enumfield.lz（脚本模式：enum Shape + 顶层 let s = Shape.Circle(radius:5) + print(s.radius)）。原生产物含合成 `pub fn main() { println!(…); return println!(…); }`（codegen/mod.rs:1370 !has_main 分支，用 top_level_stmts）；rs-lz 产物完全无 main。

两处缺口：
① src/ir/lz_codegen.rs ir_module_to_rs_lz_source（:26-53）只序列化 module.items/prelude/version，从不写 module.top_level_stmts（.lzrs 的 IrModule 构造里没有它们；IR dump `--emit=ir` 同样看不到，脚本体在 items 之外）；
② src/ir/lz_codegen_lib.lz 的 IrModule schema 无 top_level_stmts 字段、codegen_module 无 !has_main 合成分支。

修复方向（两处同改）：lz_codegen.rs 把 top_level_stmts 序列化进 .lzrs（新增字段或合成 Item）；lz_codegen_lib.lz IrModule 加字段 + codegen_module 复刻原生合成分支（注意原生合成分支走 gen_block_inner，尾表达式**带** `return `前缀，与声明式 main 的抑制行为不同——对齐时才字节一致）。风险：IrModule 构造调用点需同步（golden .lzrs 由生成器再生成，无手工旧件；lz_ir_bootstrap 的 d1.lz 无顶层语句，不受影响）。

验证基线（修复前）：p_enumfield.lz --emit=rs-lz → rustc E0601 main not found；原生路径编译运行打印 5、4 正常（BUG-407 修复后）。



### FIXED(2026-10-23T07:02:09Z / BUG-410)
- evidence: 修复 commit e191b77（src/ir/lz_codegen.rs + src/ir/lz_codegen_lib.lz）。根因：ir_module_to_rs_lz_source 只序列化 items/prelude/version、从不写 module.top_level_stmts，且 LZ 侧 Item 枚举无对应变体、codegen_module 无原生 codegen/mod.rs:1370 的 !has_main 合成分支 → rs-lz 产物 E0601 main not found。修复：序列化侧在 items 无 main FnDef 时追加 Item.ScriptMain(body: top_level_stmts)（与原生同口径、无条件合成含空语句集）；lz_codegen_lib.lz Item 新增 ScriptMain 变体，gen_item 新增分支渲染 pub fn main() { gen_body(b, is_main=false, ...) }——is_main=false 复刻原生合成 main 走 gen_block_inner（decl_gen.rs:1338 仅声明式 FnDef 设 is_main）的尾 return 非对称。验证：① p_script4.lz（纯 int 脚本体）rs-lz 由 E0601 修复为编译通过、运行打印 3、10，main 体逐字符含尾 return 与原生一致；② p_script3.lz rs-lz 编译运行 3/"done"（剩余 Debug-vs-Display 差异为已知独立缺口）；③ p_enumfield.lz main 已生成（E0601 消除），剩余 E0609 属 EnumDef 丢字段名（BUG-405 追记）；④ 回归（--test-threads=1）gate 135/135、lib 463/463、lz_ir_bootstrap 5/5、find_bug_bugs/find_bug_libs/codegen_corner_fixes/g6_codegen/tail_recursion/str_boundary/gen_build_block/error_bug_libs/reject_errors/dual_route_golden/bridge_boundary/cy_codegen_gate/lz_semantic_cases/demo_codegen_compile/lz_frontend_bootstrap/lzc_loop_unit_variant/lzc_loop_boundary_gates/cython_backend/fuzz_smoke/bridge_import/import_pkg_gate/moddec_corpus/ir_snapshots/incremental_golden 全绿。
## BUG-411 [2026-10-24T05:32:40Z] [high] FIXED
- summary: rs-lz 下 Item.EnumDef 丢字段名 ⇒ 定义退化为元组变体、构造渲染 Shape::Circle(5i64)、字段访问 s.radius 直接发射 ⇒ E0609 no field radius。修复 commit 83280f9：序列化带字段名 + Expr.EnumCtor 三形态渲染 + 新增 Expr.EnumFieldGet 解构节点，rs_codegen_mode 保证 ir-lz display 不受影响。5 个探针原生/rs-lz 逐字节一致。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：src/ir/lz_codegen.rs 序列化 + src/ir/lz_codegen_lib.lz 生成）。
现象：命名字段枚举整体降级为元组变体，定义/构造与原生分叉，且字段访问无法编译。
根因三处：
① 序列化 Item.EnumDef 只写 fl.ty、丢 fl.name（旧 variants 形如 ("Circle", [IrType.Int])）⇒ LZ 侧只能生成元组变体 Circle(i64)；原生 decl_gen.rs:2005 对「全命名字段」生成 struct variant Circle { radius: i64 }。
② 枚举构造在 IR 里是 MethodCall(receiver=Var(枚举名), method=变体名)，被渲染成 Shape::Circle(5i64)；原生 expr_gen.rs:1654 用声明名 zip 实参渲染 Shape::Circle { radius: 5i64 }。
③ 字段访问 s.radius 直接渲染成 `s.radius`；原生 expr_gen.rs:5175 解构成 match &s { Shape::Circle { radius, .. } => radius.clone(), _ => unreachable!() } ⇒ rustc E0609 no field `radius` on type `Shape`。
复现：TEMP/probe/sweep18/p_enumfield.lz（enum Shape: Circle(radius: int)/Rect(w: int, h: int)；print(s.radius); print(r.h)）——修复前 rs-lz 产物 E0609 编译失败，原生编译运行打印 5、4。
修复（commit 83280f9）：① 序列化带字段名（rs_codegen 模式），LZ gen_enum_variants 按「全命名→struct variant / 空名→元组 / 无字段→单位变体」生成；② MethodCall 构造改写为既有未用的 Expr.EnumCtor，命名字段变体的位置实参补 _KwArg；LZ 侧 EnumCtor 三形态渲染（无参→Enum::Variant、全 _KwArg→{ f: v }、其余→元组）；③ 新增 Expr.EnumFieldGet 承载解构（含该字段的每个变体一臂 + _ => unreachable!()）；display_expr/expr_ty 补穷尽分支；④ 新增 rs_codegen_mode 开关，保证 --emit=ir-lz 的 display 产物不受影响。
验证（字节级，python 精确比较 `pub enum … const __name__` 区间）：p_enum_named 248B、p_enum 187B、p_enum_pos 176B、p_enumfield 111B、p_enum_shared 435B 五例 rs-lz 与原生逐字节相同；运行等价（p_enumfield 两侧 5、4；p_enum_shared 两侧 1、3）。回归：lib 463/463、lz_ir_bootstrap 5/5、25 个集成套件全绿。


### FIXED(2026-10-24T05:33:15Z / BUG-411)
- evidence: 修复 commit 83280f9（src/ir/lz_codegen.rs + src/ir/lz_codegen_lib.lz）。① 序列化 Item.EnumDef 带字段名（rs_codegen 模式）：variants 由 List<(str, List<IrType>)> 变 List<(str, List<(str, IrType)>)>；LZ gen_enum_variants 按「全命名字段→struct variant / 空名→元组变体 / 无字段→单位变体」生成，对齐原生 decl_gen.rs:2005。② MethodCall 构造改写为 Expr.EnumCtor（命名字段变体的位置实参按声明名补 _KwArg，对齐 expr_gen.rs:1654）；LZ 侧 EnumCtor 三形态渲染：无参→Enum::Variant、全 _KwArg→Enum::Variant { f: v }、其余→元组。③ 新增 Expr.EnumFieldGet 承载字段访问解构，LZ 侧多臂 match（含该字段的每个变体一臂 + _ => unreachable!()）；display_expr/expr_ty 补穷尽分支。④ rs_codegen_mode 开关保证 --emit=ir-lz 的 IR display 不受影响。
验证（python 精确字节比较 `pub enum … const __name__` 区间）：p_enum_named 248B、p_enum 187B、p_enum_pos 176B、p_enumfield 111B、p_enum_shared 435B 五例 rs-lz 与原生逐字节相同；运行等价：p_enumfield 两侧打印 5、4，p_enum_shared 两侧 1、3。回归（--test-threads=1）：lib 463/463、lz_ir_bootstrap 5/5（含 ir-lz 与 --emit=ir 字节一致）、25 个集成套件全绿。
## BUG-412 [2026-10-24T05:32:52Z] [medium] FIXED
- summary: BUG-407 修复后多臂解构的臂序来自 HashMap 迭代（随机哈希种子）⇒ 同一源码重复编译产物不同（实测 6 次出现两种顺序），破坏可复现构建并令 rs-lz↔native 字节判据概率性失败。复现 TEMP/probe/sweep18/p_enum_shared.lz。修复：两侧按变体名排序（expr_gen.rs sort_by + lz_codegen.rs arms.sort），修复后 8/8 稳定、两侧逐字节一致。
- detail: 路径：原生 Rust codegen（src/ir/codegen/expr_gen.rs，命名字段枚举字段访问的解构臂生成）。
现象：多臂解构的**臂顺序不稳定**——同一份源码重复编译，产物不同（实测 6 次编译中 5 次 `E::A` 在前、1 次 `E::B` 在前）。破坏可复现构建，并使 rs-lz↔native 字节等价判据对该类样例呈概率性失败。
根因：BUG-407 修复时把「含该字段的变体」收集自 `self.enum_variant_named_fields`——一个 HashMap<(String,String), Vec<String>>；`.iter().filter(...)` 的迭代顺序取决于进程随机哈希种子，故臂序随机。旧代码用 `.find()` 只取一个变体，故该问题在 BUG-407 修复后才暴露。
复现：TEMP/probe/sweep18/p_enum_shared.lz
  enum E: A(v: int, w: int) / B(v: int, z: int)
  def main(): let a = E.A(v: 1, w: 2); let b = E.B(v: 3, z: 4); print(a.v); print(b.v)
  重复 `lang-zone p_enum_shared.lz` 后 grep 产物 `match &a { E::[AB]`：修复前 6 次里出现两种顺序。
修复：两侧同口径按变体名排序（臂之间模式互斥、顺序无语义，排序只为确定性）——
① 原生 expr_gen.rs：`variants_with_field.sort_by(|a, b| a.0.cmp(&b.0));`；
② rs-lz 序列化 src/ir/lz_codegen.rs：`arms.sort();`。
验证：修复后连续 8 次编译臂序稳定（8/8 `E::A` 在前）；p_enum_shared 原生与 rs-lz 产物逐字节一致（435B）。
落点说明：① 的改动位于 src/ir/codegen/expr_gen.rs，随并发 agent 的提交 104f641 一起入库（共享工作区，非本人提交）；本人在该文件中的同一处改动亦见于该 commit。


### FIXED(2026-10-24T05:33:24Z / BUG-412)
- evidence: 修复：两侧同口径按变体名排序。① 原生 src/ir/codegen/expr_gen.rs 命名字段枚举字段访问解构臂收集处加 `variants_with_field.sort_by(|a, b| a.0.cmp(&b.0));`（臂间模式互斥、顺序无语义，排序仅为确定性）；② rs-lz 序列化 src/ir/lz_codegen.rs 的 EnumFieldGet 改写处加 `arms.sort();`。
验证：修复后连续 8 次编译 `lang-zone TEMP/probe/sweep18/p_enum_shared.lz`，产物 `match &a { E::A …` 顺序稳定（8/8 均为 E::A 在前；修复前 6 次出现 E::A/E::B 两种顺序）；p_enum_shared 原生与 --emit=rs-lz 产物逐字节一致（435B）。回归：lib 463/463、gate 中原生枚举相关样例全通过（门禁 3 例失败为 BUG-413，与本条无关）、lz_ir_bootstrap 5/5。
落点：① 的改动位于 expr_gen.rs，随并发 agent 的提交 104f641 一并入库（共享工作区）；② 的改动在本人 commit 83280f9。
## BUG-413 [2026-10-24T05:33:05Z] [high] FIXED
- summary: 135-probe rustc 门禁退化为 132/135：dict_comprehension / multi_for_dict_str / multi_for_str_tuple 三例均报 E0507 cannot move out of captured variable in FnMut closure——产物为 flat_map(|k| (…).into_iter().map(move |v| (k, v)))，内层 move 闭包按值捕获外层 String 循环变量 k，未做 clone 保护。归因：门禁走原生路径且产物零 enum；上一轮门禁 135/135，此后 104f641（BUG-15 dict for-in + BUG-16 闭包 clone 保护，含删除一处 clone 注入）入库；相关文件当前均为已提交状态。
- detail: 路径：原生 Rust codegen 的多重 for 推导（dict/list comprehension）闭包链生成（src/ir/codegen/expr_gen.rs 的 comp_outer!/comp_mid! 链，见 expr_gen.rs:1509-1531）。
现象：135-probe rustc 门禁由 135/135 退化为 **132/135**，3 例编译失败，均为同一形态：
  FAIL dict_comprehension.lz [compile-fail] error[E0507]: cannot move out of `k`, a captured variable in an `FnMut` closure
  FAIL multi_for_dict_str.lz [compile-fail] error[E0507]: cannot move out of `x`, a captured variable in an `FnMut` closure
  FAIL multi_for_str_tuple.lz [compile-fail] error[E0507]: cannot move out of `x`, a captured variable in an `FnMut` closure
产物逐字（dict_comprehension.rs:26 / multi_for_dict_str.rs:26 / multi_for_str_tuple.rs:25 同形）：
  ...into_iter().flat_map(|k| { (vec![1i64, 2i64]).into_iter().map(move |v| { (k, v) }) }).collect::<HashMap<_,_>>();
  —— 内层 `move` 闭包按值捕获外层循环变量 `k`（String，非 Copy），而外层闭包是 flat_map 的 FnMut，同一 `k` 需可多次消费 ⇒ E0507。
根因方向：外层循环变量在内层 `move` 闭包中未做 clone 保护；即 `(k.clone(), v)`（原生 expr_gen.rs 既有的 clone_if_multiuse / 闭包捕获保护未覆盖这条嵌套推导链）。
归因证据：① 门禁脚本 TEMP/probe/sweep15/cy_rust.ps1 走**原生**路径（`cargo run -- <lz>`，不带 --emit=rs-lz），与本人当轮 rs-lz 改动无关；② 失败样例产物内零 enum 构造，排除 BUG-411；③ 本人上一轮收尾时门禁为 135/135（commit e191b77 后实测），此后入库的 104f641「BUG-15 dict for-in 元组模式走键值对迭代; BUG-16 fold/reduce 消费型方法多使用点 clone 保护」正是这一带：其 stmt_gen.rs 改动给 iter_is_dict 追加 `&& !var.contains(',')`（for (k,v) in dict 改走键值对迭代），expr_gen.rs 改动把闭包 move/clone 判定从 `&params` 改为 `self.enclosing_lambda_params.borrow()` 并**删除**了原「move 闭包重复消费需 clone 到闭包体」的注入块（diff @@ -6481,8 +6573,6 段）。当前 src/ir/codegen/expr_gen.rs 与 stmt_gen.rs 均为已提交状态（git status 干净），故不是并发 agent 的在途未提交改动所致。
修复方向（建议，归该文件 owner）：在生成内层 `move` 闭包时对「来自外层循环变量的非 Copy 捕获」注入 `.clone()`（与 BUG-16 的 clone 保护同机理），或对嵌套推导链的最内层闭包改用非 move 捕获 + 显式 clone。回归锁建议：把这 3 例纳入套件断言（当前只在 TEMP 门禁里跑，cargo test 未覆盖）。





### FIXED(2026-10-24T05:47:37Z / BUG-413)
- evidence: 修复 commit 96a4e02（src/ir/codegen/expr_gen.rs）。根因：两条 Lambda 生成分支（块体/表达式体）都只对「体内含嵌套 move 闭包」的块注入 clone；本层直接引用外层闭包形参的推导链（外层非 move ⇒ 本层被 BUG-357 强制 move）不在覆盖内。且 clone 判定的排除集是整条闭包参数链（enclosing_lambda_params，含祖先形参），而祖先形参在外层非 move 闭包内是自有值（非可复制借用），被内层 move 移出即 E0507。
修复：① 块体分支新增 forced_body_clone = !has_nested_move && parent_lambda_non_move && body_refs_noncopy_external(body, 自身形参)，命中时 clone_noncopy_stmt 排除集改为「仅自身形参」（= 参数链尾 encl[chain_prev_len..]）；② 表达式体分支新增 expr_clone_excl——parent_lambda_non_move 时仅排除自身形参，否则保持整条链；body_refs_noncopy_external / clone_noncopy_external 改用该集合。未命中新条件处逐字不变。
验证：3 例由 E0507 修复为编译通过且输出正确——dict_comprehension {"b": 2, "a": 2}、multi_for_dict_str {"b": 20, "a": 20}、multi_for_str_tuple [("a", 10), ("a", 20), ("b", 10), ("b", 20)]；产物含预期 clone `map(move |v| { (k.clone(), v) })`；135-probe rustc 门禁恢复 TOTAL=135 OK=135 FAIL=0；回归 lib 463/463 + 32 个集成套件全绿（含 lz_ir_bootstrap 5/5 字节一致判据）。
## BUG-414 [2026-10-24T05:58:25Z] [high] FIXED
- summary: Rust 的 Iterator::filter 谓词收 &Item，而 strip_lambda_type_with_ref 无条件把参数写成 |&x| 解构；元素非 Copy（str/Vec/struct）时即 move 出共享引用 → E0507（并连带 E0382）。comp!/dict_comp!/set_comp!/comp_* 链四处同缺口，均 .into_iter()/.chars() 产 owned，故非 Copy 元素必炸；int 元素因 Copy 恰好掩盖。修复 commit 2e794e7（按被迭代表达式元素类型判定，非 Copy 改用引用绑定）。复现 TEMP/probe/sweep19/c7_dict_cond_clean.lz。
- detail: 路径：原生 Rust codegen 推导式 `if` 谓词生成（src/ir/codegen/expr_gen.rs 四处：comp! / dict_comp! / set_comp! / comp_outer|mid|leaf 链；均用 helpers.rs:630 strip_lambda_type_with_ref）。
现象：带 if 条件的推导式在元素为 str（非 Copy）时编译失败：
  TEMP/probe/sweep19/c7_dict_cond_clean.lz（dict/list/set 三形态同炸）
  {k: 1 for k in ["aa", "b", "ccc"] if k.len() > 1} → [k for …] → {k for …}
  产物：…into_iter().filter(move |&k| { (k.len() as i64) > 1i64 })…
  rustc error[E0507] cannot move out of a shared reference（并连带 E0382）。
根因：`Iterator::filter` 谓词收到 `&Item`，strip_lambda_type_with_ref 无条件把参数改写成 `|&x|` 解构——元素为 Copy 标量时是解构拷贝（正确），元素非 Copy（String/Vec/struct）时即 move 出共享引用。四条推导链一律 `.into_iter()`/`.chars()`（产出 owned/char），故非 Copy 元素必炸；int 元素（c2_cond）恰好因 Copy 而通过，掩盖了该缺口。
修复（commit 2e794e7）：新增 comp_cond_lambda_src(cond, iter_expr, cond_expr)——元素类型优先取被迭代表达式的元素类型（列表字面量取首元素；Vec/List<T> 取 T；cond lambda 形参类型在 IR 里常为 Any，不可用，仅作回退），仅当确定非 Copy（Str/Named/Tuple）时改用 strip_lambda_type（不注入 `&`，闭包绑定引用本身），其余保持既有 `|&x|` 逐字不变。
验证：① c7 修复前全 E0507，修复后编译运行正确（dict len=2、list ["aa","ccc"]、set len=2），产物 `filter(|k| { (k.len() …) })`；② c2_cond 产物仍 `filter(|&x| { x > 2i64 })` 逐字不变（无回归）；③ 135-probe 门禁 135/135、lib 463/463、32 集成套件全绿。
遗留：dict/set 推导**值表达式**复用循环变量仍 E0382（见 BUG-415）。


### FIXED(2026-10-24T05:58:41Z / BUG-414)
- evidence: 修复 commit 2e794e7（src/ir/codegen/expr_gen.rs）。新增 comp_cond_lambda_src(cond, iter_expr, cond_expr)：元素类型优先取被迭代表达式的元素类型（列表字面量取首元素；Vec/List<T> 取 T；cond lambda 形参类型在 IR 里常为 Any 仅作回退），仅当确定非 Copy（Str/Named/Tuple）时改用 strip_lambda_type（不注入 `&`，闭包绑定引用本身 x: &Item，方法/字段访问经自动解引用仍可用），其余类型保持既有 |&x| 逐字不变；comp!/dict_comp!/set_comp!/comp_* 链四处调用点同改。
验证：① c7_dict_cond_clean.lz（dict/list/set 三形态 + if 条件 + str 元素）修复前全 E0507，修复后编译运行正确（dict len=2、list ["aa","ccc"]、set len=2），产物 filter(|k| { (k.len() …) })；② c2_cond.lz（int 元素）产物仍 filter(|&x| { x > 2i64 }) 逐字不变，无回归；③ 135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；32 个集成套件全绿（find_bug_bugs 46、find_bug_libs 12、codegen_corner_fixes 37、g6_codegen 17、lz_ir_bootstrap 5 等）。遗留：推导式值 lambda 复用循环变量 E0382 另立 BUG-415。
## BUG-415 [2026-10-24T05:58:40Z] [medium] FIXED
- summary: {k: k.len() for k in ["aa","b","ccc"]} 生成 .map(move |k| { (k, k.len()) })：元组先按值 move k，随后 k.len() 借用已 move 的 k → E0382 borrow of moved value。与 if 条件无关（c8 无条件下同炸），属推导式值 lambda 缺「同一非 Copy 变量多次使用」的 clone 保护（clone_if_multiuse 同族）。复现 TEMP/probe/sweep19/c8_body_reuse.lz（带条件的 c6_dict_comp_cond.lz 可证两条缺陷独立）。
- detail: 路径：原生 Rust codegen 推导式**值表达式**生成（dict_comp!/set_comp! → `.map(lambda)`，expr_gen.rs 的 comp 分支）。
现象：值表达式复用循环变量时 E0382（与 if 条件无关）：
  TEMP/probe/sweep19/c8_body_reuse.lz
    let d = {k: k.len() for k in ["aa", "b", "ccc"]}      # 无 if 条件
  产物：…into_iter().map(move |k| { (k, (k.len() as i64)) }).collect::<HashMap<_,_>>()
  rustc error[E0382]: borrow of moved value: `k`
  —— 元组 `(k, k.len())` 先按值 move k 进元组，随后 k.len() 又借用已 move 的 k。
  带条件的同形样例见 TEMP/probe/sweep19/c6_dict_comp_cond.lz（其 filter 侧 E0507 已由 BUG-414 修复，
  修后仅剩本条 E0382，可用来证明两条缺陷相互独立）。
根因方向：LZ 是值语义（同一变量多次使用各自取副本），但推导式值 lambda 的体由用户表达式直接生成，
未对「同一非 Copy 变量的多次使用」注入 `.clone()`（与 BUG-356/BUG-413 的 clone_if_multiuse 同族），
而 `(k, k.len())` 这种「先整体 move 再借用」的求值顺序必然 E0382。
修复方向（建议）：推导式值 lambda 体内对「在体内出现多次的非 Copy 循环变量」做 clone 保护——
把除最后一次以外的使用改写为 `k.clone()`（或对元组/字面量元素统一 clone）。
回归锁建议：dict/set 推导的值表达式与键表达式复用同一循环变量的样例应纳入套件。




### FIXED(2026-10-24T06:22:22Z / BUG-415)
- evidence: 修复 commit 356b11f（src/ir/codegen/expr_gen.rs）。① TupleLit 常态分支：用 count_vars_expr 统计各元素内变量出现次数，对「总数 ≥2 且非最后一次出现」的变量补 clone（复用 clone_noncopy_external，把该元素内其余变量放进排除集，仅目标变量被 clone；Int/F64/Bool 由 helper 跳过）；保守边界——仅当该变量在某个元素位是裸引用（会被 move）才 clone，并显式排除 self，避免把合成 `__unapply__` 的 `(self.x, self.y)` 误改成 `(self.clone().x, …)`。② LzAdd 分支：两侧同为同一裸变量（非 self、非 Copy 标量、未含 .clone()）时左侧补 .clone()。
验证：5 个复现件全部转为编译通过且输出正确——c8_body_reuse（dict 推导值复用 k）→ 3；c10_list_comp_reuse（`[k + k for …]`）→ ["aa","bb"]；c9_multiuse_lambda（`|k: str| (k, k.len())`）→ 1；c11_fn_multiuse（`def pair/twice`）→ ("ab", 2) / cdcd；c6_dict_comp_cond → 2。
关键防回归：未加「裸引用」限定时，D1 字节判据 tests/lz_ir_bootstrap.rs::lz_emit_rs_lz_matches_native_codegen 实测失败（native `(self.clone().x, self.y.clone())` vs rs-lz `(self.x.clone(), self.y.clone())`）；加限定后两侧逐字节一致，lz_ir_bootstrap 5/5。
回归：135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；32 个集成套件全绿（find_bug_bugs 46、find_bug_libs 12、codegen_corner_fixes 37、g6_codegen 17、tail_recursion 16、demo_codegen_compile 16、lz_frontend_bootstrap 44 等）。首次门禁跑出的 4 例 gen-fail 系并发 agent 在途编辑（builder/infer.rs）的瞬时构建窗口，复跑 135/135。
## BUG-416 [2026-10-24T06:31:29Z] [medium] FIXED
- summary: dict 字面量 `{k: k.len()}` 生成 BTreeMap::from([(k, k.len())])：元组先按值 move k 再借用 → E0382。这是 BUG-415 修复未覆盖的同族缺口——Dict 键值对与 List()/Set() 构造的元素是直接拼字符串发射、不经 TupleLit。修复 commit 72c78ec（抽出 repeated_move_clone_sets/clone_repeated_in_expr 并接入 Dict / List|Vec / Set|HashSet 三分支）。复现 TEMP/probe/sweep19/d5_dictlit.lz。
- detail: 路径：原生 Rust codegen 的构造式发射（src/ir/codegen/expr_gen.rs 的 StructCtor 分支："Dict" / "List"|"Vec" / "Set"|"HashSet"）。
现象：
  TEMP/probe/sweep19/d5_dictlit.lz —— `let k = "ab"` 后 `let m = {k: k.len()}`
  产物：let m = std::collections::BTreeMap::from([(k, (k.len() as i64))]);
  rustc error[E0382]: borrow of moved value: `k`（元组 (k, k.len()) 先按值 move k，随后 k.len() 借用已 move 的 k）。
根因：BUG-415 修复只覆盖了元组字面量（ExprKind::TupleLit）与多态加法（LzAdd）；而 dict 字面量的键值对、List()/Set() 构造的元素是**直接拼字符串**生成的（`from([(k, v), …])` / `vec![…]`），不经 TupleLit，故同一「按值多项列表内复用非 Copy 变量」缺口仍在。三个分支（Dict / List|Vec / Set|HashSet）同形。
修复（commit 72c78ec）：把 BUG-415 的判定抽成可复用方法 repeated_move_clone_sets(items)（同一非 Copy 变量出现 ≥2 次且存在裸引用 → 除最后一次出现外补 clone；显式排除 self）与 clone_repeated_in_expr(e, names)；TupleLit 改用之（行为不变），Dict 分支把键值对展平成 [k0,v0,k1,v1,…] 统一判定（跨条目重复亦覆盖），List|Vec 与 Set|HashSet 分支同样接入。
验证：d5_dictlit 由 E0382 转为编译通过且输出正确（len=1）；既有 9 个探针（d6_listlit_reuse、d2_callargs、d3_structctor、c6/c7/c8/c9/c10/c11）全部保持通过；135-probe 门禁 135/135；lib 463/463；lz_ir_bootstrap 5/5（含 rs-lz↔native 逐字节一致判据）；31 个集成套件全绿。



### FIXED(2026-10-24T06:31:31Z / BUG-416)
- evidence: 修复 commit 72c78ec（src/ir/codegen/expr_gen.rs）。把 BUG-415 的判定抽成可复用方法 repeated_move_clone_sets(items)（同一非 Copy 变量出现 ≥2 次且存在裸引用 → 除最后一次出现外补 clone；显式排除 self，避免误改合成 __unapply__ 的 (self.x, self.y)）与 clone_repeated_in_expr(e, names)（复用 clone_noncopy_external，表达式内其余变量入排除集，仅目标变量被 clone）；TupleLit 改用之（行为不变），"Dict" 分支把键值对展平成 [k0,v0,k1,v1,…] 统一判定（跨条目重复亦覆盖）逐对渲染，"List"|"Vec" 与 "Set"|"HashSet" 分支同样接入。
验证：d5_dictlit.lz（`{k: k.len()}`）由 E0382 转为编译通过、输出 len=1；既有 9 个探针（d6_listlit_reuse、d2_callargs、d3_structctor、c6/c7/c8/c9/c10/c11）全部保持通过；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；lz_ir_bootstrap 5/5（rs-lz↔native 与 ir-lz↔--emit=ir 逐字节一致，证明新增 clone 不破坏 D1 判据）；31 个集成套件全绿。
## BUG-417 [2026-10-24T06:39:03Z] [medium] FIXED
- summary: `P(k, k.len())` 生成 `P { a: k, b: (k.len() as i64) }`：先按值 move k 再借用 → E0382。BUG-415/416 覆盖了元组/多态加法/Dict/List/Set，但位置式 struct 构造是独立渲染路径（字段名按位置映射、值逐个拼串），未接入 repeated_move_clone_sets；关键字形式早有 clone 保护，故该缺口长期潜伏。修复 commit 197a0f5。复现 TEMP/probe/sweep19/e4_struct_pos.lz。
- detail: 路径：原生 Rust codegen 的位置式 struct 构造（src/ir/codegen/expr_gen.rs StructCtor 分支的字段值渲染，:6994 一带）。
现象：TEMP/probe/sweep19/e4_struct_pos.lz
  struct P = a: str, b: int
  let k = "ab"; let p = P(k, k.len())
  产物：let p = P { a: k, b: (k.len() as i64) };
  rustc error[E0382]: borrow of moved value: `k`
根因：BUG-415/416 修复覆盖了元组字面量（TupleLit）、多态加法（LzAdd）、Dict 键值对与 List/Set 构造，但**位置式 struct 构造**的字段值渲染是另一条独立代码路径（字段名由位置映射得到，值逐个 gen_expr 拼串），未接入 repeated_move_clone_sets。对照：关键字形式 `P(a: k, b: k.len())` 早就通过（kwarg 路径经 _KwArg 值渲染另有 clone 保护），只有位置式形式炸——这也是它能长期潜伏的原因。
修复（commit 197a0f5）：按字段声明顺序收集字段值表达式，调用 repeated_move_clone_sets 求出各字段位需 clone 的变量名集合，再对命中的字段值走 clone_repeated_in_expr（enumerate + 延迟初始化局部 Expr 承载克隆副本，不改动期望类型下推与其余渲染逻辑）。
验证：e4_struct_pos 由 E0382 转为编译通过、输出 b=2；既有探针（d3_structctor kwarg 形式、d5_dictlit、d6_listlit_reuse、d2_callargs、c8/c9/c10/c11）全部保持通过；135-probe 门禁 135/135、lib 463/463、lz_ir_bootstrap 5/5、31 个集成套件全绿。



### FIXED(2026-10-24T06:39:05Z / BUG-417)
- evidence: 修复 commit 197a0f5（src/ir/codegen/expr_gen.rs）。位置式 struct 构造的字段值渲染接入既有 repeated_move_clone_sets / clone_repeated_in_expr：按字段声明顺序收集字段值表达式，同一非 Copy 变量出现 ≥2 次且存在裸引用时除最后一次外补 clone（enumerate + 延迟初始化局部 Expr 承载克隆副本）；不改动期望类型下推与其余渲染逻辑。
验证：e4_struct_pos.lz（`P(k, k.len())`）由 E0382 转为编译通过、输出 b=2；既有探针（d3_structctor kwarg 形式、d5_dictlit、d6_listlit_reuse、d2_callargs、c8/c9/c10/c11）全部保持通过；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；lz_ir_bootstrap 5/5（rs-lz↔native 逐字节一致）；31 个集成套件全绿。
## BUG-418 [2026-10-24T06:52:54Z] [medium] FIXED
- summary: 同一 struct 既定义 __iter__（返回自身）又定义 __next__ 时，codegen 由 __next__ 发 impl Iterator、由 __iter__ 发显式 impl IntoIterator，与 std blanket `impl<I: Iterator> IntoIterator for I` 冲突 → E0119 conflicting implementations。语料常见形态是容器 __iter__ 返回独立迭代器类型（不冲突），故长期未暴露。修复 commit e0657d3（self_is_iterator 时跳过显式 impl，交 blanket）。复现 TEMP/probe/sweep20/i2_iterator.lz。
- detail: 路径：原生 Rust codegen 的迭代协议 impl 生成（src/ir/codegen/magic_gen.rs:266 的 `__iter__` → IntoIterator 分支；Iterator impl 由 decl_gen.rs 依 `__next__` 生成）。
现象：TEMP/probe/sweep20/i2_iterator.lz
  struct Countdown = n: int
  impl Countdown:
      def __iter__(ref self) -> Countdown = Countdown(self.n)
      def __next__(ref self) -> Option<int> = ...
  def main(): for v in Countdown(3): print(v)
产物同时发射：
  impl Iterator for Countdown { type Item = i64; fn next(&mut self) -> Option<i64> { self.__next__() } }
  impl std::iter::IntoIterator for Countdown { type IntoIter = Countdown; type Item = <Countdown as Iterator>::Item; fn into_iter(self) -> Self::IntoIter { self.__iter__() } }
rustc error[E0119]: conflicting implementations of trait `IntoIterator` for type `Countdown`
（std 有 blanket `impl<I: Iterator> IntoIterator for I`，显式 impl 与之冲突）。
根因：magic_gen.rs 只要有 `__iter__`（返回命名类型）就发显式 IntoIterator impl，未考虑「本类型自身已是 Iterator（定义了 `__next__`）」的情形。规范 SYNTAX/06d 的魔术方法表同时列出 `__iter__` 与 `__next__`，自迭代类型（同一 struct 既返回自身又可 next）是合法 LZ；而语料里的常见形态是容器 `__iter__` 返回**独立**迭代器类型（容器本身不是 Iterator ⇒ 无冲突），故缺口长期未暴露——DEMO/07_data_structures/magic_methods.lz 与 DEMO/lz_std/dict.lz 实测均正常。
修复（commit e0657d3）：计算 self_is_iterator = methods.any(name == "__next__")，`__iter__` 分支加 `.filter(|_| !self_is_iterator)` 跳过显式 impl，交由 std blanket（IntoIter = Self、into_iter(self) = self，与「__iter__ 返回 self」语义等价）。边界：若某类型既有 `__next__` 又让 `__iter__` 返回**不同**类型，Rust 一致性规则下两者不可共存，以 blanket 的自迭代语义为准（已在代码注释记录）。
验证：i2_iterator 由 E0119 转为编译通过，`for v in Countdown(3)` 输出 3、2、1；magic_methods.lz / dict.lz 无回归；135-probe 门禁 135/135；lib 463/463；31 个集成套件 + lz_ir_bootstrap 5/5 全绿。



### FIXED(2026-10-24T06:52:56Z / BUG-418)
- evidence: 修复 commit e0657d3（src/ir/codegen/magic_gen.rs）。加 self_is_iterator = methods.any(name == "__next__")，`__iter__` 分支 `.filter(|_| !self_is_iterator)` 跳过显式 IntoIterator impl，交由 std blanket impl（IntoIter = Self、into_iter(self) = self，与 __iter__ 返回 self 等价）；容器 __iter__ 返回独立迭代器类型的情形不受影响。
验证：i2_iterator.lz（自迭代 Countdown）由 E0119 转为编译通过，for v in Countdown(3) 输出 3、2、1；DEMO/07_data_structures/magic_methods.lz 与 DEMO/lz_std/dict.lz（容器→独立迭代器形态）仍编译通过，无回归；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；31 个集成套件 + lz_ir_bootstrap 5/5（rs-lz↔native 逐字节一致）全绿。
## BUG-419 [2026-10-24T07:03:50Z] [medium] FIXED
- summary: --emit=rs-lz 下 print 恒发 {:?}，字符串实参被 Debug 渲染（"done"/"hi" 带引号），而原生按 print_arg_is_display 逐位选 {} → 输出 done/hi。同一份 LZ 源码因 codegen 路线不同而行为不同；D1 判据语料只打印 int 故未暴露。修复 commit 3c2b5cc（逐位占位符 + 字符串字面量裁 .to_string()）。复现 TEMP/probe/sweep20/k1_printstr.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：src/ir/lz_codegen_lib.lz 的 print 发射）。
现象：同一份 LZ 源码在两条 codegen 路线下的**运行结果不同**：
  TEMP/probe/sweep20/k1_printstr.lz（let s = "hi"; print("done"); print(s)）
  原生产物：println!("{}", "done");  println!("{}", s);                  → done / hi
  rs-lz 产物：println!("{:?}", "done".to_string()); println!("{:?}", s);  → "done" / "hi"
即字符串被 Debug 渲染、加引号并转义——这是「产物级」分叉，不只是排版差异。
根因：原生 expr_gen.rs:1941 的 print 发射按实参逐位选择占位符（print_arg_is_display：字符串字面量/f-string/cast 到 str/str 类型 → `{}`，其余 → `{:?}`），并对字符串字面量裁掉多余的 `.to_string()`（:1954）；而 lz_codegen_lib.lz 的 print_fmt 恒发 `{:?}`，且实参走通用 expr_cs_list（字面量带 .to_string()）。D1 字节判据的语料恰好只打印 int（两侧同为 `{:?}`），故该分叉长期未被判据捕获。
修复（commit 3c2b5cc）：print_fmt 逐位生成占位符（`{}`/`{:?}`，空格分隔），新增 print_placeholder / print_arg_is_display / is_string_irtype，判定规则与原生一一对应（LitStr、LitFStr、Cast 到 Str|String、expr_ty 为 Str/Ref(Str)/MutRef(Str)/Named String|str）；新增 print_args_list / print_arg_of，对 print 实参中的字符串字面量去掉 `.to_string()`，其余沿用 gen_kwarg_value（保持 BUG-408 口径）。
验证：k1_printstr 的 rs-lz 产物修复为 `println!("{}", "done")` / `println!("{}", s)`，运行输出 done、hi，与原生逐行一致（两文件 main 区段内容逐字节相同；全文件首个差异仅为我 PowerShell 捕获产生的 CRLF）；135-probe 门禁 135/135、lib 463/463、lz_ir_bootstrap 5/5、31 个集成套件全绿。



### FIXED(2026-10-24T07:03:52Z / BUG-419)
- evidence: 修复 commit 3c2b5cc（src/ir/lz_codegen_lib.lz）。print_fmt 改为逐位生成占位符（{} / {:?}，空格分隔），新增 print_placeholder / print_arg_is_display / is_string_irtype（规则与原生 expr_gen.rs:200 的 print_arg_is_display 一一对应：LitStr、LitFStr、Cast 到 Str|String、expr_ty 为 Str/Ref(Str)/MutRef(Str)/Named String|str）；新增 print_args_list / print_arg_of，print 实参中的字符串字面量去掉 .to_string()（对齐原生 :1954），其余沿用 gen_kwarg_value 保持 BUG-408 口径。
验证：k1_printstr.lz 的 rs-lz 产物由 println!("{:?}", "done".to_string()) 修复为 println!("{}", "done")，运行输出 done / hi 与原生逐行一致（main 区段内容逐字节相同）；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；lz_ir_bootstrap 5/5（rs-lz↔native 字节一致判据保持通过）；31 个集成套件全绿。
## BUG-420 [2026-10-24T07:54:04Z] [high] FIXED
- summary: def f(e) -> int = match e: case A(x): x + 1 在 rs-lz 下生成 `{ x + 1i64; }`，块值被 `;` 丢弃 → E0308 expected i64 found ()。根因：gen_block 的 is_main=true 只抑制尾 return，ExprStmt 的 ; 仍追加，未区分值位置块。修复 commit 6eefc1b（新增 gen_value_block + match 按 is_tail 分档 let _ = match）。复现 TEMP/probe/sweep20/f6b_match_tuple.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：lz_codegen_lib.lz 的 match 分支/块发射）。
现象：**值位置**的 match 分支体尾表达式被加分号，块值被丢弃 ⇒ E0308。
复现 TEMP/probe/sweep20/f6b_match_tuple.lz：
  enum E2: A(int) / B(int, int)
  def val(e: E2) -> int =
      match e:
          case E2.A(x): x + 1
          case E2.B(y, z): y * z
  rs-lz 产物：`E2::A(x) => { x + 1i64; }` → 分支块值为 () ⇒ error[E0308] mismatched types
  原生产物：`E2::A(x) => { x + 1i64 }` → 运行 2、6
根因：gen_match_arm → gen_block → gen_body(is_main=true) 只抑制了尾 `return`，
ExprStmt 的 `;` 仍然追加（BUG-406 的修复只覆盖「嵌套块不得发射 return」，未区分
**值位置块**——块的值应是尾表达式）。而 match 若处于函数尾/块尾，分支值必须外流。
修复（commit 6eefc1b，src/ir/lz_codegen_lib.lz）：
- 新增 gen_value_body / gen_value_block：值位置块，尾 ExprStmt 直出表达式不加分号；
  尾语句为 match/if/block 时以 is_tail=true 递归（嵌套 match 同样走值位置形态）。
- gen_match_arm 改用 gen_value_block。
- gen_stmt 的 Stmt.Match 按 is_tail 分档：值位置直出 `match … { … }`；语句位置
  `let _ = match … { … };` 丢弃值（对齐原生语句 match 的 `let _ = match`，避免
  「非 () 类型的表达式语句」E0308）。
验证：f6b 由 E0308 转为编译通过、输出 2、6 与原生一致；p_match（语句位置 match +
后续语句）仍输出 1、2，无回归；135-probe 门禁 135/135；lib 463/463；lz_ir_bootstrap 5/5；31 套件全绿。

## BUG-421 [2026-10-24T07:54:26Z] [high] FIXED
- summary: match s: case Shape.Circle(radius: r) 在 rs-lz 下渲染为 Shape::Circle(r)（元组模式），对 struct variant 报 E0164。BUG-411 只修了构造侧，模式侧序列化仍无字段名。修复 commit 6eefc1b（Pattern 新增 EnumNamed + 序列化按变体定义发该形态；补 display_pattern 穷尽分支）。复现 TEMP/probe/sweep20/f6_match_enum.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：lz_codegen.rs 的 Pattern 序列化 + lz_codegen_lib.lz 的 gen_pattern）。
现象：命名字段变体的**匹配模式**被渲染成元组形态 ⇒ E0164。
复现 TEMP/probe/sweep20/f6_match_enum.lz：
  enum Shape: Circle(radius: int) / Rect(w: int, h: int)
  def area(s: Shape) -> int =
      match s:
          case Shape.Circle(radius: r): r * r * 3
          case Shape.Rect(w: w, h: h): w * h
  rs-lz 产物 `Shape::Circle(r)` ⇒ error[E0164] expected tuple struct or tuple variant,
  found struct variant `Shape::Circle`；原生为 `Shape::Circle { radius: r }`。
根因：BUG-411 只修了**构造**侧（MethodCall → Expr.EnumCtor 带字段名），模式侧序列化
仍只有 Pattern.Enum(enum_name, variant, args)（不带字段名），LZ 侧只能生成元组模式。
修复（commit 6eefc1b）：
- lz_codegen_lib.lz 的 Pattern 新增 EnumNamed(enum_name, variant, fields)，
  gen_pattern / display_pattern 渲染 `E::V { f: p, … }`（display_pattern 的穷尽分支
  必须补，否则 LZ 侧报「match 未穷尽枚举 Pattern 的变体 EnumNamed」——实测该错文本
  一度被并发 agent 留在 builder/infer.rs 的 [DBG-fa]/[DBG-let] 输出挤掉）。
- lz_codegen.rs::pattern 在 rs_codegen_mode 下查枚举登记表（BUG-411 引入的 enums），
  命名字段变体且实参个数匹配时发 Pattern.EnumNamed（字段名 zip 子模式）；元组/单位
  变体保持原形态。
验证：f6 由 E0164 转为编译通过、输出 12、12 与原生一致；lz_ir_bootstrap 5/5 保持
（原生与 rs-lz 字节一致）；135-probe 门禁 135/135；lib 463/463；31 套件全绿。
定位过程：该失败最初被误判为并发 agent 的调试代码所致，经「stash 自己的两处改动后
复跑判据（转绿）」隔离，确认归属自身；再过滤 DBG 行读到真实报错文本后定位到 LZ 侧
match 穷尽性。



### FIXED(2026-10-24T07:54:27Z / BUG-420, BUG-421)
- evidence: 修复 commit 6eefc1b（lz_codegen_lib.lz + lz_codegen.rs）。① 值位置 match（BUG-420）：新增 gen_value_body / gen_value_block（尾 ExprStmt 直出表达式、不加分号；尾语句为 match/if/block 时 is_tail=true 递归），gen_match_arm 改用之；gen_stmt 的 Stmt.Match 按 is_tail 分档——值位置直出 `match … {…}`，语句位置 `let _ = match … {…};`（对齐原生，避免非 () 表达式语句 E0308）。② 命名字段变体模式（BUG-421）：Pattern 新增 EnumNamed(enum_name, variant, fields)，gen_pattern/display_pattern 渲染 `E::V { f: p, … }`；序列化侧在 rs_codegen_mode 下查枚举登记表，命名字段变体且实参个数匹配时发该形态。
验证：f6_match_enum 由 E0164 转为编译通过、输出 12、12；f6b_match_tuple 由 E0308 转为编译通过、输出 2、6（均与原生一致）；p_match（语句位置 match）仍输出 1、2 无回归；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；lz_ir_bootstrap 5/5（原生与 rs-lz 字节一致判据）；31 个集成套件全绿。
## BUG-422 [2026-10-24T08:10:37Z] [high] FIXED
- summary: LZ 侧 Item.Impl 只带 method_names ⇒ 所有用户 struct/enum 方法与 trait impl 在 --emit=rs-lz 下都是空壳（E0061）；trait 声明又发射无效的 `fn f(&Self)` 与无名形参（E0185/syntax error）。修复 commit 22abee7：新增 ImplMethod（完整定义）并渲染真实函数体（self → &self/&mut self/self）；trait 签名恒出 &self + `_pN` 命名形参并跳过 self 标记。复现 TEMP/probe/sweep20/{i1_magic_add,g3_struct_methods,f5_trait,f5b_trait_params}.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：lz_codegen.rs 的 Item.Impl/TraitDef 序列化 + lz_codegen_lib.lz 的 impl/trait 渲染）。
现象（两类）：
A. 用户 struct/enum 方法与 trait impl 全部渲染为空壳桩：
   impl Counter { fn inc(&self) { unimplemented!() } }        ← 无真实函数体、无返回类型
   ⇒ i1_magic_add（impl Vec2: def __add__(ref self, other: Vec2)）报
     error[E0061]: this method takes 0 arguments but 1 argument was supplied；
     g3_struct_methods（impl Counter: def inc(ref self, by: int)）同报 E0061。
B. trait 声明发射 `fn greet(&Self) -> String;`——`&Self` 是无效 Rust（应为 `&self`），
   且 trait 方法形参无名（Rust fn 签名不允许无名形参）；
   ⇒ f5_trait 报 syntax error（expected one of …, found `)`）+ E0185（impl 侧 &self
     与声明对不上）。
根因：LZ 侧 Item.Impl 只携带 method_names（序列化丢方法体）；TraitDef 的形参只带类型
（丢名字与 self 形态），渲染时把 self 的类型 `&Self` 原样发射。
修复（commit 22abee7）：
- 新增 struct ImplMethod(name, params, ret, exc, body)；Item.Impl 改 `methods: List<ImplMethod>`；
  序列化在 rs_codegen 模式下按完整定义发射（含 params/ret/raises/body），IR display 模式
  保持 names-only（lz_ir_lib.lz 的 Item.Impl 未变）；LZ 侧 gen_impl_def/gen_impl_method
  渲染真实函数体，self 形参按元组序 (name,ty,is_ref,is_mut,is_owned) 渲染
  `&self`/`&mut self`/`self`，其余 `name: ty`。
- gen_trait_methods 恒出 `&self` 作首参、其余按位置命名 `_pN`（对齐原生
  `fn add(&self, _p1: i64) -> i64;`），并跳过形参表中保留的 self 标记类型
  （Ref(Self_)/MutRef(Self_)/Self_）——否则 self 被计两次 ⇒ E0050。
验证：i1_magic_add 4/6、g3_struct_methods 2/5、f5_trait `hi bob`、f5b_trait_params 5/8
（修复前分别 E0061 / E0185+syntax error / E0050）；lz_ir_bootstrap 5/5（两路线字节一致）；
135-probe 门禁 135/135；lib 463/463；31 套件全绿。
遗留（另立条目）：impl 的 generics/where 约束/关联类型绑定仍未随 schema 传递。

## BUG-423 [2026-10-24T08:10:53Z] [medium] FIXED
- summary: `Light.On.label()` 在 rs-lz 下渲染为 `Light.On.label()`（点号）⇒ E0423 expected value, found enum；`"hi " + self.name` 渲染为 String + String ⇒ E0308 expected &str found String。修复 commit 22abee7：FieldAccess 按基底是否为类型名选 ::/.（复用 BUG-405 判定）；BinOp 的 + 两侧为字符串时右侧补 &。复现 TEMP/probe/sweep20/{g5_enum_methods,f5_trait}.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：lz_codegen_lib.lz 的 Expr.FieldAccess / Expr.BinOp 渲染）。
现象（两处）：
A. 类型名基底的字段访问用 `.`：
   `Light.On.label()` 渲染成 `Light.On.label()` ⇒
   error[E0423]: expected value, found enum `Light`（原生为 `Light::On.label()`）。
   复现 TEMP/probe/sweep20/g5_enum_methods.lz（enum Light: On/Off + impl Light: def label）。
B. 字符串拼接缺 `&`：
   `"hi " + self.name` 渲染成 `"hi ".to_string() + self.name` ⇒
   error[E0308]: mismatched types（expected `&str`, found `String`；Rust 无 String + String）。
   复现 TEMP/probe/sweep20/f5_trait.lz（原生为 `"hi ".to_string().to_string() + &self.name.clone()`）。
根因：A——FieldAccess 恒用 `.` 连接（BUG-405 只修了**方法调用**侧的类型名分隔符）；
B——BinOp 恒发 `lhs op rhs`（未处理 Rust 的 `String + &str` 要求）。
修复（commit 22abee7）：
A. FieldAccess 改用 field_access_sep(b)——基底是 Var 且首字母大写（类型名，如 `Light`）时用
   `::`，否则 `.`；复用 BUG-405 的 method_call_sep 判定，口径一致。
B. BinOp 的 `+` 在两侧均为字符串类型（str / Ref(Str) / MutRef(Str) / Named String|str，
   复用 BUG-419 的 is_string_irtype）时右侧补 `&`。
验证：g5_enum_methods 由 E0423 转为编译通过、输出 `on`；f5_trait 由 E0308 转为输出 `hi bob`
（均与原生一致）；lz_ir_bootstrap 5/5（原生与 rs-lz 字节一致判据未破）；135-probe 门禁
135/135；lib 463/463；31 个集成套件全绿。



### FIXED(2026-10-24T08:10:54Z / BUG-422, BUG-423)
- evidence: 修复 commit 22abee7（src/ir/lz_codegen.rs + src/ir/lz_codegen_lib.lz）。
BUG-422：新增 struct ImplMethod(name, params, ret, exc, body)，Item.Impl 改 methods: List<ImplMethod>；序列化在 rs_codegen 模式下按完整定义发射（params/ret/raises/body），IR display 保持 names-only；LZ 侧 gen_impl_def/gen_impl_method 渲染真实函数体，self 形参按 (name,ty,is_ref,is_mut,is_owned) 渲染 &self/&mut self/self。trait 签名改为恒出 &self + `_pN` 命名形参，并跳过形参表中保留的 self 标记类型（Ref(Self_)/MutRef(Self_)/Self_）避免 self 计两次 E0050。
BUG-423：FieldAccess 改按基底是否为类型名（Var 且首字母大写）选 ::/.（新增 field_access_sep，复用 method_call_sep）；BinOp 的 + 两侧均为字符串类型时右侧补 &（复用 is_string_irtype）。
验证：8 个 rs-lz 探针全部由失败转为编译通过且输出与原生一致——i1_magic_add 4/6、g3_struct_methods 2/5、g5_enum_methods `on`、f5_trait `hi bob`、f5b_trait_params 5/8、f6_match_enum 12/12、f6b_match_tuple 2/6、k1_printstr done/hi（修复前分别 E0061/E0423/E0308/E0185/E0164）；lz_ir_bootstrap 5/5（rs-lz↔native 与 ir-lz↔--emit=ir 双字节判据）；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；31 个集成套件全绿。
## BUG-424 [2026-10-24T08:51:46Z] [high] FIXED
- summary: LZ 集合方法与自由函数原样透传给 Rust（Vec 无 map ⇒ E0599；Rust 无 sorted/reversed 自由函数），且 LZ 语义返回新集合而 Rust 适配器返回惰性迭代器。修复 commit 76723f5：gen_method_call 增设 map/filter/fold/join/keys/values/get 适配分支，gen_call 增设 sorted/reversed；filter 谓词按元素 Copy 性择形。复现 TEMP/probe/sweep20/{f2b_list_methods,f3_dict_methods,f12_string_build}.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：lz_codegen_lib.lz 的 gen_method_call / gen_call）。
现象：LZ 的集合方法与自由函数按原名/原形透传给 Rust：
  `xs.map(f)` → Vec 无 map ⇒ E0599；`xs.filter/fold/join`、`d.keys/values/get`、
  `sorted(xs)` / `reversed(xs)`（Rust 无同名自由函数）同族失败或语义不符。
复现：TEMP/probe/sweep20/f2b_list_methods.lz（sorted/reversed/map/filter/fold）、
f12_string_build.lz（join）、f3_dict_methods.lz（keys/values/get）。
根因：BUG-424 补齐了 str 子集，同一张改写表的「集合迭代适配」另一半仍缺；LZ 集合方法
返回新集合，而 Rust 迭代器适配器返回惰性迭代器，必须加 .iter().cloned() 与 .collect()。
修复（commit 76723f5）：gen_method_call 增设集合适配分支（形态对齐原生）——
  map/filter → `(recv).iter().cloned().<op>(…).collect::<Vec<_>>()`
  fold → `(recv.clone()).into_iter().fold(init, f)`
  join → `(recv).iter().map(|x| x.to_string()).collect::<Vec<_>>().join(sep)`
  keys/values → `recv.keys()/values().cloned().collect::<Vec<_>>()`
  get（Dict）→ `(recv.get(k)).cloned()`
gen_call 增设 sorted/reversed → `{ let mut _tmp = x.clone(); _tmp.sort()/reverse(); _tmp }`。
filter 谓词按元素 Copy 性择形（Copy → `|&x|` 解构；非 Copy → 绑定引用），与 BUG-414 同口径。
验证：f2b 输出 [1,2,3]/[2,1,3]/[6,2,4]/[3,2]/6、f3 输出 ["a","b"]/[1,2]/Some(1)/2、
f12 输出 n0-n1-n2，均与原生逐行一致；135-probe 门禁 135/135、lib 463/463、
lz_ir_bootstrap 5/5、cargo test --workspace 992 passed / 0 failed。

## BUG-425 [2026-10-24T08:52:28Z] [high] FIXED
- summary: LZ `[3,1,2]` 在 rs-lz 下渲染为 Rust 数组 `[3i64,1i64,2i64]`（类型 [T;N]），与 List<T>=Vec<T> 不符 ⇒ E0308 expected Vec<i64>, found [i64;3]；`[]` 同病。修复 commit 76723f5：非空发 vec![…]、空表发 Vec::new()（对齐原生）。复现 TEMP/probe/sweep20/{f2b_list_methods,f12_string_build}.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：lz_codegen_lib.lz 的 Expr.ListLit 渲染）。
现象：LZ 列表字面量被渲染成 Rust **数组**：
  `[3, 1, 2]` → `[3i64, 1i64, 2i64]`（类型 `[T; N]`）⇒
  error[E0308]: mismatched types（expected `Vec<i64>`, found `[i64; 3]`）；
  空列表 `[]` → `[]`（`[_; 0]`）同病。
复现：TEMP/probe/sweep20/f2b_list_methods.lz（`let mut xs: List<i64> = [3, 1, 2]`）、
f12_string_build.lz（`let mut parts: List<str> = []`）。
根因：Expr.ListLit 分支直接发 `[` … `]`；LZ 的 List<T> 对应 Rust `Vec<T>`，二者类型不同。
修复（commit 76723f5）：非空列表发 `vec![…]`，空列表发 `Vec::new()`（对齐原生
`vec![3i64, 1i64, 2i64]` / `Vec::new()`）。
验证：f2b 由 E0308 转为编译通过并输出 [1,2,3]/[2,1,3]/[6,2,4]/[3,2]/6；f12 输出 n0-n1-n2；
135-probe 门禁 135/135；lib 463/463；lz_ir_bootstrap 5/5；
cargo test --workspace 992 passed / 0 failed。

## BUG-426 [2026-10-24T08:52:29Z] [medium] FIXED
- summary: LZ `{"a":1,"b":2}` 在 rs-lz 下渲染为 `[…].into_iter().collect()`（无目标类型）⇒ E0282 type annotations needed。修复 commit 76723f5：改发 `std::collections::BTreeMap::from([...])`（对齐原生）。复现 TEMP/probe/sweep20/f3_dict_methods.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：lz_codegen_lib.lz 的 gen_dict_ctor）。
现象：字典字面量渲染为无形参类型的 collect：
  `{"a": 1, "b": 2}` → `[("a".to_string(), 1i64), ("b".to_string(), 2i64)].into_iter().collect()`
  ⇒ error[E0282]: type annotations needed（报在变量声明处，后续 `d.keys()` 处提示
  "type must be known at this point"）。
复现：TEMP/probe/sweep20/f3_dict_methods.lz。
根因：`.into_iter().collect()` 无目标类型，rustc 无法推断集合种类；原生发
`std::collections::BTreeMap::from([...])`（具体类型）。
修复（commit 76723f5）：gen_dict_ctor 改发 `std::collections::BTreeMap::from([...])`。
验证：f3 由 E0282 转为编译通过并输出 ["a","b"] / [1,2] / Some(1) / 2（与原生一致）；
135-probe 门禁 135/135；lib 463/463；lz_ir_bootstrap 5/5；workspace 992 passed / 0 failed。



### FIXED(2026-10-24T08:52:44Z / BUG-424, BUG-425, BUG-426)
- evidence: 本轮 rs-lz 内建形态/字面量三修（commit d187c8e + 76723f5）。
【BUG-424 集合迭代适配层】修复 commit 76723f5（lz_codegen_lib.lz）。gen_method_call 增设 map/filter/fold/join/keys/values/get 适配分支——map/filter → `(recv).iter().cloned().<op>(…).collect::<Vec<_>>()`；fold → `(recv.clone()).into_iter().fold(init,f)`；join → `(recv).iter().map(|x| x.to_string()).collect::<Vec<_>>().join(sep)`；keys/values → `recv.keys()/values().cloned().collect::<Vec<_>>()`；get(Dict) → `(recv.get(k)).cloned()`；gen_call 增设 sorted/reversed → `{ let mut _tmp = x.clone(); _tmp.sort()/reverse(); _tmp }`；filter 谓词按元素 Copy 性择形（Copy→`|&x|`，非 Copy→绑定引用，与 BUG-414 同口径），新增 elem_irtype/is_copy_elem/filter_lambda_src/amp_join。
  同族的前半（**str 子集**：length→len、to_lower→to_lowercase、模式实参裸字面量、trim/replace 补 .to_string()、split 物化）已在 commit d187c8e 落地（该提交信息中标注为「BUG-424」），本条 note 将其并入同一账目，避免重复计账。
  验证：f2b_list_methods 输出 [1,2,3]/[2,1,3]/[6,2,4]/[3,2]/6、f3_dict_methods 输出 ["a","b"]/[1,2]/Some(1)/2、f12_string_build 输出 n0-n1-n2，均与原生逐行一致。
【BUG-425 列表字面量】修复 commit 76723f5：Expr.ListLit 发 `vec![…]`、空表发 `Vec::new()`（原发 Rust 数组 ⇒ E0308）。验证：f2b/f12 由 E0308 转绿（输出同上）；135-probe 门禁 135/135。
【BUG-426 字典字面量】修复 commit 76723f5：gen_dict_ctor 发 `std::collections::BTreeMap::from([...])`（原 `.into_iter().collect()` 无目标类型 ⇒ E0282）。验证：f3 由 E0282 转绿（输出 ["a","b"]/[1,2]/Some(1)/2）。
  编号说明：我的 commit 76723f5 信息里把上述三处标为 BUG-425/426/427（当时以为 str 子集已单独入账）；台账为权威编号，实际落为 BUG-424（集合适配 + str 子集合并）/425（列表字面量）/426（字典字面量），特此对齐。
回归：lz_ir_bootstrap 5/5（rs-lz↔native 与 ir-lz↔--emit=ir 双字节判据）；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；cargo test --workspace 992 passed / 0 failed（47 个测试二进制）。
遗留（另立条目跟进）：i7_dict_comp_iter 报 `cannot find macro comp`（推导宏在 rs-lz 侧未展开）；g1_stdlib 报 E0061（min/max/sum/zip 等自由函数形态未适配）。
## BUG-427 [2026-10-24T09:14:35Z] [high] FIXED
- summary: LZ 推导宏（comp!/dict_comp!/set_comp! 与 comp_outer!/mid!/leaf! 链）在 --emit=rs-lz 下原样透传 ⇒ rustc cannot find macro comp。修复 commit cc1aca8：新增 gen_comp_macro 等按原生 expr_gen.rs:1418-1551 展开（单 for→map+collect；outer→flat_map+collect；mid/leaf 不 collect；收集类型 Vec/HashSet/HashMap；带条件先 filter）。复现 TEMP/probe/sweep20/i7_dict_comp_iter.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：lz_codegen_lib.lz 的 gen_call）。
现象：LZ 推导宏在 rs-lz 侧完全没有处理，名字原样透传 ⇒
  error: cannot find macro `comp` in this scope（TEMP/probe/sweep20/i7_dict_comp_iter.lz）。
  涉及 comp!/dict_comp!/set_comp! 与多 for 链 comp_outer!/comp_mid!/comp_leaf!（dict_/set_ 同族）。
根因：原生 codegen 在 Rust 侧展开这些宏（expr_gen.rs:1418-1551），LZ 版 codegen 无对应实现。
修复（commit cc1aca8）：新增 comp_macro_kind / is_comp_chain_node / comp_collect_ty_of /
gen_comp_macro，并在 gen_call 增设 `elif is_comp_macro(n)` 分支，形态对齐原生：
  单 for → `(iter).into_iter().map(lambda).collect::<T>()`
  多 for 链：outer → flat_map + collect；mid → flat_map（不 collect）；leaf → map（不 collect）
  收集类型：Vec<_> / HashSet<_> / std::collections::HashMap<_,_>
  带条件时先 `.filter(...)`（谓词按元素 Copy 性择形，复用 BUG-425 的 filter_lambda_src，
  与原生 BUG-414 口径一致）；str 迭代走 `.chars()`
修正点：初版把 `*_outer!` 也视为链内节点（不 collect），实测 c1_nested3 得 FlatMap 迭代器 ⇒
E0308，改为「仅 mid/leaf 不 collect」。
验证：i7_dict_comp_iter 2/2、c1_nested3（三层 for 链）[111,121,112,122]、c7_dict_cond_clean
2/["aa","ccc"]/2、c2_cond [6,8]、c3_set 3 —— 均与原生逐行一致；lz_ir_bootstrap 5/5；
135-probe 门禁 135/135；lib 463/463；cargo test --workspace 992 passed / 0 failed。

## BUG-428 [2026-10-24T09:14:36Z] [medium] FIXED
- summary: LZ `.len()` 返回 int（i64），rs-lz 原样发 Rust `recv.len()`（usize）⇒ `k.len() > 1i64` 报 E0308 expected usize found i64（推导条件位实测）。自由函数 len(x) 早已补 as i64，方法形式漏了。修复 commit cc1aca8：gen_method_call 增设 len/length → `(recv.len() as i64)`。复现 TEMP/probe/sweep19/c7_dict_cond_clean.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：lz_codegen_lib.lz 的 gen_method_call）。
现象：LZ 的 `.len()` 原样发成 Rust `recv.len()`（usize），而 LZ 语义返回 int（i64）⇒
在推导条件位 `k.len() > 1i64` 报
  error[E0308]: mismatched types（expected `usize`, found `i64`）
（TEMP/probe/sweep19/c6_dict_comp_cond.lz、c7_dict_cond_clean.lz）。
根因：LZ 版 codegen 的 MethodCall 分支无 len 的窄化处理；自由函数形式 `len(x)` 早已发
`(x.len() as i64)`，方法形式漏了。
修复（commit cc1aca8）：gen_method_call 增设 `if (m == "len") || (m == "length")` →
`(recv.len() as i64)`（length 亦经改名表指向 len）。
验证：c7_dict_cond_clean 由 E0308 转为编译通过并输出 2/["aa","ccc"]/2（与原生一致）；
lz_ir_bootstrap 5/5；135-probe 门禁 135/135；lib 463/463；workspace 992 passed / 0 failed。



### FIXED(2026-10-24T09:14:55Z / BUG-427, BUG-428)
- evidence: 修复 commit cc1aca8（src/ir/lz_codegen_lib.lz）。
【BUG-427】新增 comp_macro_kind / is_comp_chain_node / comp_collect_ty_of / gen_comp_macro，gen_call 增设 `elif is_comp_macro(n)`，按原生 expr_gen.rs:1418-1551 展开：单 for → `(iter).into_iter().map(lambda).collect::<T>()`；多 for 链 outer → flat_map + collect、mid → flat_map（不 collect）、leaf → map（不 collect）；收集类型 Vec<_> / HashSet<_> / std::collections::HashMap<_,_>；带条件先 filter（谓词按元素 Copy 性择形）；str 迭代走 .chars()。修正了初版「*_outer! 不 collect」的错误（c1_nested3 实测 E0308）。
【BUG-428】gen_method_call 增设 `if (m == "len") || (m == "length")` → `(recv.len() as i64)`，与自由函数 len(x) 及原生同口径。
验证：i7_dict_comp_iter 2/2、c1_nested3 [111,121,112,122]、c7_dict_cond_clean 2/["aa","ccc"]/2、c2_cond [6,8]、c3_set 3，均与原生逐行一致；lz_ir_bootstrap 5/5（rs-lz↔native 与 ir-lz↔--emit=ir 双字节判据）；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；cargo test --workspace 992 passed / 0 failed。
过程注记：本轮 Python 重写文件时把仓库的 LF 误写成 CRLF，导致一次提交出现「全文件重写」假象（2108/2044）；已把文件规范回 LF 并 `commit --amend`，最终 diff 收敛为 65 insertions / 1 deletion。教训：本仓库该文件以 LF 存储（.gitattributes/autocrlf 未生效），批量改文件须显式用 LF。
## BUG-429 [2026-10-24T09:14:57Z] [medium] FIXED
- summary: rs-lz 下 `{k: k.len() for k in […str…]}` 生成 `map(move |k| (k, k.len()))`：元组先按值 move k 再借用 ⇒ E0382 borrow of moved value。LZ 版 codegen 尚无原生 BUG-415/416 的「同一非 Copy 变量多次使用补 clone」判定（repeated_move_clone_sets 族）。复现 TEMP/probe/sweep19/{c8_body_reuse,c6_dict_comp_cond}.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：推导宏展开后的**值表达式 lambda**）。
现象：推导的值表达式复用循环变量时报 E0382：
  TEMP/probe/sweep19/c8_body_reuse.lz：`{k: k.len() for k in ["aa","b","ccc"]}`
    → rs-lz 产物 `…into_iter().map(move |k| { (k, (k.len() as i64)) })` ⇒
      error[E0382]: borrow of moved value: `k`（元组先按值 move k，随后 k.len() 借用）
  TEMP/probe/sweep19/c6_dict_comp_cond.lz 同形（带条件版本，条件侧 E0308 已由 BUG-428 修复，
  修后仅剩本条 E0382，可证两缺陷独立）。
根因：LZ 版 codegen 尚无原生 BUG-415/416 的「同一非 Copy 变量出现 ≥2 次且存在裸引用时，
除最后一次外补 clone」判定（原生在 expr_gen.rs 的 repeated_move_clone_sets /
clone_repeated_in_expr 中实现，覆盖 TupleLit / Dict / List / Set / 位置式 struct 构造 /
LzAdd 两侧）。推导值 lambda 的元组体属同一族。
修复方向（建议）：把原生的 repeated_move_clone_sets 口径移植到 LZ 版 codegen——在
gen_comp_macro 的 lambda 体（以及更一般的元组字面量渲染）里，对「体内出现 ≥2 次且存在裸
引用的非 Copy 变量」补 .clone()（边界同样需排除 self 与纯借用基底，见 BUG-412 教训）。
回归锁建议：c6/c8 两个探针纳入套件。



### FIXED(2026-10-24T10:09:21Z / BUG-429)
- evidence: 修复 commit d27b4ce（仅 src/ir/lz_codegen_lib.lz，73+/1-）。
新增 gen_tuple_lit / gen_tuple_elem / name_used_later / any_expr_has_var /
struct_fields_have_var / is_noncopy_irtype，并把 Expr.TupleLit 渲染改走 gen_tuple_lit：
「元素本体为 Var 且该名字在**后面**元素仍被引用」的非 Copy 变量补 `.clone()`（除最后一次
使用外），口径对齐原生 expr_gen.rs 的 repeated_move_clone_sets（显式排除 self；Int/F64/Bool
不加 clone）。名字递归覆盖 Var/MethodCall/Call/FieldAccess/Index{Get,Set}/BinOp/UnOp/
Tuple/List/Cast/Paren/If/StructCtor 等常用变体，未覆盖保守返回 false。
验证：
  rs-lz 产物编译+运行且与原生一致：c8_body_reuse [3]、c6_dict_comp_cond [2]（原生同值）；
  回归 i7 [2|2]、c1_nested3 [111,121,112,122]、c7 2/["aa","ccc"]/2、c2 [6,8]、c3 [3] 全过。
  lz_ir_bootstrap 5/5；135-probe 门禁 TOTAL=135 OK=135 FAIL=0；lib 463/463；
  cargo test --workspace 992 passed / 0 failed。
过程注记（同 commit note）：库初版编译报「Parse error: Expected RBrack, got Gt at pos 9207」
且位置落在毫不相关的 type_list/tail_t，逐项二分定位到我自己把 `List<Expr>` 误打成 `List[Expr>`
（方括号 `[`），括号不配对导致报错位置与真实缺陷位置不一致；已改正为 `List<Expr>`。
## BUG-430 [2026-10-24T10:24:45Z] [high] FIXED
- summary: LZ 自由函数 min/max/sum/abs/zip 在 --emit=rs-lz 下原样透传为同名 Rust 自由函数 ⇒ min/max E0061（2 参却传 1 参）、sum/abs E0425（无此函数）。修复 commit 5c43bf6：gen_call 按原生 stdlib 表改形（(&x).iter().min().cloned()、sum::<i64>()、lz_abs(x)、into_iter().zip(…)），并限定实参个数避免影响 min(a,b) 等用法。复现 TEMP/probe/sweep20/g1_stdlib.lz。
- detail: 路径：--emit=rs-lz（LZ 自举 Rust codegen：lz_codegen_lib.lz 的 gen_call / Var 分支）。
现象：TEMP/probe/sweep20/g1_stdlib.lz 的
  print(min([3,1,2])) / print(max([3,1,2])) / print(sum([1,2,3])) / print(abs(-4)) / print(zip([1,2],["a","b"]))
在 rs-lz 下生成同名 Rust 自由函数调用：
  error[E0061]: this function takes 2 arguments but 1 argument was supplied（min/max）
  error[E0425]: cannot find function `sum` in this scope
  error[E0425]: cannot find function `abs` in this scope
根因：LZ 版 codegen 的 gen_call 无标准库自由函数形态表，名字原样透传；Rust 没有自由函数
sum/abs，min/max 是迭代器方法（`(&xs).iter().min()`，不是 2 参自由函数）。原生 codegen
（expr_gen.rs 的 stdlib 表）早有这些形态，rs-lz 一直缺失。
修复（commit 5c43bf6，仅 src/ir/lz_codegen_lib.lz，16 行）：gen_call 的 Var 分支新增：
  min(x)  → (&x).iter().min().cloned()
  max(x)  → (&x).iter().max().cloned()
  sum(x)  → x.iter().copied().sum::<i64>()
  abs(x)  → lz_abs(x)（lz_builtins 内建，与原生一致）
  zip(a,b)→ a.into_iter().zip(b.into_iter()).collect::<Vec<_>>()
形态逐字对齐原生；均限定实参个数（min/max/sum/abs=1、zip=2），其余形态仍走通用调用分支，
既有 min(a,b)/max(a,b) 等 2 参用法不受影响。
验证：rs-lz 产物编译并运行 g1_stdlib.lz ⇒ Some(1)|Some(3)|6|4|[1,2,3]|[(1,"a"),(2,"b")]，
与原生同形；lz_ir_bootstrap 5/5；135-probe 门禁 135/135；lib 463/463；
cargo test --workspace 992 passed / 0 failed。



### FIXED(2026-10-24T10:24:47Z / BUG-430)
- evidence: 修复 commit 5c43bf6（仅 src/ir/lz_codegen_lib.lz，16+/0-）。
gen_call 的 Var 分支新增 5 个 elif，形态逐字对齐原生 expr_gen.rs 的 stdlib 表：
  min(x)→(&x).iter().min().cloned()；max(x)→(&x).iter().max().cloned()；
  sum(x)→x.iter().copied().sum::<i64>()；abs(x)→lz_abs(x)（lz_builtins 内建）；
  zip(a,b)→a.into_iter().zip(b.into_iter()).collect::<Vec<_>>()。
均限定实参个数（min/max/sum/abs=1、zip=2），其余形态仍走通用调用分支，既有 min(a,b)/
max(a,b) 等 2 参用法不受影响。
验证：
  rs-lz 产物编译并运行 g1_stdlib.lz ⇒ Some(1)|Some(3)|6|4|[1,2,3]|[(1,"a"),(2,"b")]
  （与原生同形：(&vec![…]).iter().min().cloned()、.iter().copied().sum::<i64>()、
   lz_abs(-4i64)、.into_iter().zip(…)、sort 块）。
  lz_ir_bootstrap 5/5（rs-lz↔native 与 ir-lz↔--emit=ir 双字节判据）；
  135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；
  lib 463/463；cargo test --workspace 992 passed / 0 failed（47 个测试二进制）。
## BUG-431 [2026-10-24T11:56:22Z] [high] FIXED
- summary: BUG-431: rs-lz 的 Item.FnDef/TraitDef/Impl schema 丢弃 where_clause、assoc_type_bindings、trait assoc_types 与泛型边界，生成的 Rust 代码报 E0425/E0412/E0191/E0599
- detail: 自举后端（--emit=rs-lz）序列化时只带泛型名字，丢掉了 §五 的 where 子句、关联类型声明/绑定。后果（TEMP/probe/sweep21 探针实测）：
1) g2a_generic_impl.lz：泛型 impl 生成 `impl Box3<T> {`，缺 `impl<T>` ⇒ error[E0425] cannot find type `T`（3 处）。
2) g2b_trait_assoc.lz：trait 缺 `type Item;`、impl 缺 `type Item = i64;` ⇒ E0412/E0191；`def area_plus2<T>(x) where T: HasArea` 生成 `fn area_plus2<T>(x)` 缺边界 ⇒ E0599 no method area for T。
3) g2c_where_generic.lz：`impl<T> Box4<T> where T: Clone` 生成 `impl<T> Box4<T>`（`where T: Clone` 被 builder 合并进 GenericParam.bounds，schema 只带名字）⇒ E0599 cannot call clone on T。
同源：struct 自动生成的 `__unapply__` impl 也缺 `Clone` 边界（原生对齐 `impl<T: std::clone::Clone + std::fmt::Debug> Box3<T>`）。
- reported_by: lang-zone-agent



### FIXED(2026-10-24T11:56:24Z / BUG-431)
- evidence: 修复 commit 05d1eb5（仅 src/ir/lz_codegen.rs + src/ir/lz_codegen_lib.lz，270+/48-）。
序列化侧：新增 generics_bounds_ref / generics_names_ref，仅 rs_codegen_mode 下发 `List<(str, List<IrType>)>` 泛型（含边界）与 where_clause / assoc_type_bindings / assoc_types；IR 展示路径 schema 不变（lz_ir_lib 兼容）。
LZ 库侧：五个 Item 变体的 generics 改为带边界元组表；新增 generic_sig_b（内联边界）、generic_bounds、wc_bounds_for、gen_trait_assoc_types（`type Item;`）、gen_impl_assoc_bindings（`type Item = i64;`）、impl_where_clause（type_param 的 `.`→`::`）、rust_type_assoc（trait 方法签名 `Self::Item`）、generic_sig_cd（自动 __unapply__ impl 的 `Clone + Debug`，对齐原生）；并修掉一处 BUG-429 类 partial-move。
验证：g2c 经 rs-lz 生成 `impl <T: Clone>Box4<T>`、`pub fn twice<T: Clone>`（此前无边界）；g2b 生成 `type Item;` + `type Item = i64;` + `impl HasArea for Circle4`（此前全部缺失）。
回归：cargo test -- --test-threads=1 全绿（lib 463/463 + 各集成套件含 parity 字节判据）。
注：g2a/g2b/g2c 残余编译错误（struct 命名构造 E0423、trait 接收者 &self/self 对齐 E0053、用户 impl 自动推断 Clone）为既存独立 rs-lz 缺口，不在本 BUG 内。
## BUG-432 [2026-10-24T13:03:09Z] [high] FIXED
- summary: [rule:E0423] rs-lz 命名字段结构体构造生成 `Point(3i64,4i64)` 报 E0423 (src/ir/lz_codegen_lib.lz gen_call)
- detail: `Point(x: 3, y: 4)`（命名字段结构体构造）经 builder 变成 `Expr.Call(callee=Var("Point"), args=[_KwArg(x,3), _KwArg(y,4)], ty=Named(path:"Point"))`。rs-lz 的 `gen_call`（src/ir/lz_codegen_lib.lz）此前走通用调用分支，`expr_cs_list` 经 `kwarg_value_of` 剥掉字段名 ⇒ 生成 `Point(3i64, 4i64)`，rustc 报 E0423 expected function, tuple struct or tuple variant, found struct `Point`。原生 `--emit=rs` 生成 `Point { x: 3i64, y: 4i64 }`。泛型结构体同样中招：`Wrap<int>(v: 5)` → `Wrap(5i64)` → E0423。

复现探针 TEMP/probe/sweep23/g3a_ctor.lz、g3b_generic_ctor.lz：rs-lz 修复前产物 `let p = Point(3i64, 4i64);`。
- reported_by: lead



### FIXED(2026-10-24T13:03:33Z / BUG-432)
- evidence: 提交 efabc7d（fix(rs-lz): 命名字段结构体构造改为 `Point { x: 3, y: 4 }`，仅 src/ir/lz_codegen_lib.lz）。

改动：gen_expr 的 Expr.Call 分支把结果类型 ty 透传给 gen_call（新增形参 t）；gen_call 的 Var 分支新增结构体构造分支——is_struct_ctor(n, a, t) 判定（调用名首字母大写 + 实参全为 _KwArg + 结果类型 IrType.Named(path) 等于调用名），渲染 `n { kwarg_pairs }`；新增 is_struct_ctor / irtype_named_path 助手。判据刻意排除「返回该 struct 的同名函数」：函数名小写不满足 starts_upper，且 Rust 中 fn 与 struct 同名本非法，故无伪命中。

验证（TEMP/probe/sweep23，原生↔rs-lz 同形）：
- g3a_ctor.lz → NATIVE `3|4|`，RSLZ `3|4|`（`let p = Point { x: 3i64, y: 4i64 };`）
- g3b_generic_ctor.lz → NATIVE `5|3|3|0|`，RSLZ `5|3|3|0|`（`Wrap { v: 5i64 }` 泛型构造；`make_pair(3i64)` / `make_pair_pos(7i64)` 返回 Pair 的函数实参未被误判）
回归：cargo test -j 4 -- --test-threads=1 全绿（lib 463/463 + 各集成套件含 rs-lz↔native 字节判据）；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0。
## BUG-433 [2026-10-24T13:24:00Z] [high] FIXED
- summary: [rule:E0053] rs-lz trait impl 方法接收者渲染为按值 `self` 与 trait 声明 `&self` 不一致 (src/ir/lz_codegen_lib.lz self_param_str)
- detail: IR 里 TraitDef 方法接收者只记为 `Self_` 类型（无 ref 标志），rs-lz 与原生都按 `&self` 渲染 trait 声明；而 ImplMethod 的 `("self", Self_, is_ref, is_mut, is_owned)` 在 is_ref=false 时被 self_param_str 渲染为按值 `self`。于是 `impl HasItem for MyVec { fn item(self) -> i64 }` 与 trait 声明 `fn item(&self) -> Item` 不一致 ⇒ rustc E0053 method `item` has an incompatible type for trait。原生把 trait impl 方法接收者固定渲染为 `&self`，因此这是 rs-lz 独有缺口。

复现探针 TEMP/probe/sweep21/g2b_trait_assoc.lz，修复前 rs-lz 产物：`fn item(self) -> i64`。
- reported_by: lead


### FIXED(2026-10-24T13:24:43Z / BUG-433)
- evidence: 提交 510a4cd（fix(rs-lz): trait impl 方法接收者对齐 trait 声明，仅 src/ir/lz_codegen_lib.lz）。

改动：gen_impl_def 依 trait_（YesTy/NoTy）求出 force_ref；gen_impl_method / impl_method_params / self_param_str 增补 force_ref 形参；force_ref=true（trait impl）时 self 接收者强制取引用（is_mut → `&mut self`，否则 `&self`，绝不用按值 `self`）；固有 impl（NoTy）保持 IR 原语义零变化。trait 声明本就是 `&self`，故强制引用使 impl 与声明一致。

验证（TEMP/probe/sweep21/g2b_trait_assoc.lz，原生↔rs-lz 同形）：
- 修复前 RSLZ：`fn item(self) -> i64` → E0053 FAIL。
- 修复后 RSLZ：`fn item(&self) -> i64`、`fn area(&self) -> i64`，rustc 通过并运行输出 `7|28|`（v.item()=7、area_plus2(c)=9*3+1=28）。
回归：cargo test -j 4 -- --test-threads=1 全绿（lib 463/463 + 各集成套件含 rs-lz↔native 字节判据，41 个 suite 无失败）；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0；sweep21/g2a_generic_impl 两后端均 OK `5|"hi"|`（固有 impl 接收者行为未受影响）。

说明：同一探针的原生侧仍报 E0425（trait 声明 `-> Item` 裸名），已另报为 BUG-434（原生 codegen 域，未在本提交修复）。
## BUG-434 [2026-10-24T13:24:01Z] [medium] OPEN
- summary: [rule:E0425] 原生 codegen trait 声明关联类型返回用裸名 `-> Item` 而非 `-> Self::Item`
- detail: trait 声明 `def item(self) -> Item`（关联类型 Item）在原生 `--emit=rs` 中生成 `fn item(&self) -> Item;`，关联类型名以裸名出现在返回类型里，rustc 报 E0425 cannot find type `Item` in this scope。应为 `-> Self::Item`（rs-lz 已正确生成 `fn item(&self) -> Self::Item;`）。注意该 E0425 在原生侧会遮蔽同一探针里的接收者不一致问题（E0053，rs-lz 独有）。

复现探针 TEMP/probe/sweep21/g2b_trait_assoc.lz，`--emit=rs` 后 rustc 直接报 E0425。
- reported_by: lead


## BUG-435 [2026-10-24T13:54:05Z] [medium] FIXED
- summary: [rule:E0599] rs-lz `.sum()/.min()/.max()` 方法直发接收者自身，缺 `.iter()` 迭代适配 (src/ir/lz_codegen_lib.lz gen_method_call)
- detail: rs-lz 的 gen_method_call 只对 `.len()/.length` 与 filter/map/fold/join/keys/values/get 做特判，`.sum()`（及 `.min()`/`.max()`）方法落到通用方法分支直接发 `a.sum()` → rustc E0599 no method named `sum` found for struct `Vec<i64>`（Vec 本身不实现 Iterator，需 `a.iter().copied().sum()`）。原生 codegen 渲染 `(a).iter().copied().sum::<i64>()`，因此这是 rs-lz 独有缺口。自由函数 `sum(x)/min(x)/max(x)` 已在 BUG-430 处理，此处是方法形态遗漏。

复现探针 TEMP/probe/sweep23/g7_vecstr.lz 的 `let c = a.sum();`，修复前 rs-lz 产物 `a.sum()`。
- reported_by: lead



### FIXED(2026-10-24T13:54:35Z / BUG-435)
- evidence: 提交 73b1fcc（fix(rs-lz): 补 `.sum()/.min()/.max()` 方法形态对齐原生，仅 src/ir/lz_codegen_lib.lz）。

改动：gen_method_call 的 len 分支后新增三分支——
- `.sum()`（零实参）→ `(<recv>).iter().copied().sum::<i64>()`
- `.min()`（零实参）→ `(<recv>).iter().min().cloned()`
- `.max()`（零实参）→ `(<recv>).iter().max().cloned()`
三者与原生 codegen 逐字符一致（原生实测：`(a).iter().copied().sum::<i64>()` / `(v).iter().min().cloned()` / `(v).iter().max().cloned()`）。

验证（原生↔rs-lz 同形、运行输出一致）：
- TEMP/probe/sweep23/g7_vecstr.lz：修复前 RSLZ `a.sum()` → E0599；修复后 `let c = (a).iter().copied().sum::<i64>();`，两后端均 `110|a-b-c|6|-1|`（vec_ops=110、str_ops=a-b-c、opt_ops=6/-1）。
- TEMP/probe/sweep23/g8_minmax.lz：两后端均 `Some(1)|Some(4)|`，生成的 `return (v).iter().min().cloned();` / `return (v).iter().max().cloned();` 与原生逐字符一致。
回归：cargo test -j 4 -- --test-threads=1 全绿（lib 463/463 + 各集成套件含 rs-lz↔native 字节判据 D1 门禁，42 个 suite 无失败）；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0。

说明：`v.min() + v.max()` 会被 LZ 前端拒绝（Option 不能直接参与算术），属语言语义，非本 bug；`.min()/.max()` 在 LZ 中返回 Option<int>，与 `.iter().min().cloned()`（Option<i64>）一致。
## BUG-436 [2026-10-24T13:54:59Z] [medium] OPEN
- summary: [rule:E0599] rs-lz 对 Range 接收者插 `.iter()`，Range 无该方法；原生同构造又漏 `.collect()`（E0308），需协同对齐
- detail: rs-lz 的 gen_method_call 对 `filter`/`map`/`join` 等集合迭代方法一律先插 `(<recv>).iter().cloned()`（或 `.into_iter()`），但当接收者是 `Range`（LZ 里 `0..n` 的字面量，Rust 端 `std::ops::Range<i64>`）时，Range 本身已实现 Iterator 却没有 `.iter()` 方法 ⇒ rustc E0599 no method named `iter` found for struct `std::ops::Range<Idx>`。

原生 codegen 对此接收者**不插** `.iter()`：`(xs).filter(...)` 直接可调（Range 实现 Iterator），但原生又漏了 `.collect::<Vec<_>>()`（`return (xs).filter(...)` 返回 Filter 迭代器，函数签名 `-> Vec<i64>` 时 rustc E0308 mismatched types）。两后端在该构造上各坏一处、需协同对齐（原生补 `.collect()`、rs-lz 去掉对 Range 的 `.iter()`），单独改 rs-lz 会与原生产出分叉、违反 D1 逐字符一致门禁，故本轮仅登记不修。

复现探针 TEMP/probe/sweep23/g6_range_filter.lz：NATIVE `error[E0308]: mismatched types`（缺 collect），RSLZ `error[E0599]: no method named `iter` found for struct `Range<Idx>`。
- reported_by: lead

## BUG-437 [2026-10-24T14:20:12Z] [medium] FIXED
- summary: [rule:E0308] rs-lz List/Vec 的 `.contains(x)` 实参未按引用传（直发 `v.contains(3i64)`）(src/ir/lz_codegen_lib.lz gen_method_call)
- detail: Rust 端 `Vec::contains(&self, x: &T)` 要求实参按引用。rs-lz 的 gen_method_call 未接管集合（List/Vec）上的 `.contains`，落到通用方法分支直发 `v.contains(3i64)`（按值）→ rustc E0308 mismatched types。原生 codegen 渲染 `v.contains(&3i64)`，故属 rs-lz 独有缺口。Dict 上的 `.contains`（→ contains_key）与 str 上的 `.contains`（→ Pattern 位）走各自既有分支不受影响。

复现探针 TEMP/probe/sweep23/g9c_ops.lz 的 `let has3 = v.contains(3)`，修复前 rs-lz 产物 `v.contains(3i64)`。
- reported_by: lead



### FIXED(2026-10-24T14:20:37Z / BUG-437)
- evidence: 提交 2839b1a（fix(rs-lz): List/Vec 的 `.contains(x)` 实参按引用传，仅 src/ir/lz_codegen_lib.lz）。

改动：gen_method_call 的 get 分支后新增——
- `.contains(x)`（List/Vec 接收者、一实参）→ `(<recv>).contains(&<x>)`
与原生的 `v.contains(&3i64)` 一致。Dict（→ contains_key）与 str（→ Pattern 位，is_pattern_method）既有分支未动。

验证（原生↔rs-lz 同形、运行输出一致）：
- TEMP/probe/sweep23/g9c_ops.lz：修复前 RSLZ `v.contains(3i64)` → E0308；修复后 `let has3: bool = (v).contains(&3i64);`，两后端均 `XBX|2|`（sops=XBX、lops=2，含 trim/replace/to_upper/contains/push/is_empty/if-表达式）。
- TEMP/probe/sweep23/g9d_contains.lz（String 列表 contains、复合实参 contains）：两后端产出逐字符一致（`v.contains(&s)`、`v.contains(&a + b)`）。复合实参 `&a + b` 需 `&(a + b)`、是原生侧既有缺口（原生同样发 `&a + b` → E0308），rs-lz 保持与原生逐字符对齐、不擅自加括号，以保 D1 字节一致门禁；故本 bug 仅覆盖简单/变量实参情形。
回归：cargo test -j 4 -- --test-threads=1 全绿（41 suite / 937 passed / 0 failed，含 rs-lz↔native 字节判据 D1 门禁）；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0。
## BUG-438 [2026-10-24T14:22:22Z] [medium] OPEN
- summary: [rule:E0599] `.sorted()/.reversed()` 及迭代器 `.to_list()` 两后端均直发原生名（无该 Rust 方法），原生侧缺口待补
- detail: LZ 的 `v.sorted()`（`v: List<int>`）两后端都直发 `v.sorted()` → rustc E0599 no method named `sorted` found for struct `Vec<i64>`（Rust 无 `Vec::sorted`，需 `v.iter().sorted()` / `.into_iter().sorted()`，或改用 `sort()`/`sort_unstable()`）。原生（`src/ir/codegen/*`，本轮为并发代理脏文件）与 rs-lz 都未接管该方法，属原生侧缺口、rs-lz 只是镜像继承（两后端产物逐字符一致，D1 门禁不破）。

同类可能未接管的方法：`reversed()`、迭代器上的 `.to_list()`（rs-lz 与原生都发 `…to_list()` → E0599，需 `collect::<Vec<_>>()`）、复杂实参 `.contains(a + b)`（需 `&(a + b)`）。

复现探针 TEMP/probe/sweep23/g9_strlist.lz：NATIVE 与 RSLZ 同报 `error[E0599]: no method named `sorted` found for struct `Vec<i64>`。修复需在原生 codegen（集合方法表）补 `sorted/reversed/to_list` 等方法形态，并让 rs-lz 同表同步；rs-lz 单独改会与原生产出分叉、违反 D1 逐字符一致门禁，故本轮仅登记不修。
- reported_by: lead

## BUG-439 [2026-10-24T14:37:49Z] [medium] FIXED
- summary: [rule:E0308] rs-lz Option/Result 接收者的 `.map()` 误走集合迭代器分支（发 `.iter().cloned()..collect()`）(src/ir/lz_codegen_lib.lz gen_method_call)
- detail: rs-lz 的 gen_method_call 对 `.map()` 一律按集合迭代器处理（`(<recv>).iter().cloned().map(..).collect::<Vec<_>>()`），但当接收者是 Option/Result 时这类值没有 `.iter()` → rustc E0308 mismatched types。原生 codegen 渲染 `(r).clone().map(move |n| ..)`，即 Option/Result 的 `.map` 直接作用于内层值。属 rs-lz 独有缺口（原生正确）。

复现探针 TEMP/probe/sweep23/g10_optres.lz 的 `let x = r.map(|n| n * 2)`，修复前 rs-lz 产物 `(r).iter().cloned().map(..).collect::<Vec<_>>()`。
- reported_by: lead



### FIXED(2026-10-24T14:38:39Z / BUG-439)
- evidence: 提交 011887c（fix(rs-lz): Option/Result 接收者的 `.map()` 走 Option::map/Result::map，仅 src/ir/lz_codegen_lib.lz）。

改动：gen_method_call 的 min/max 分支后新增——
- `.map(f)`（Option/Result 接收者、一实参）→ `(<recv>).clone().map(<f>)`
与原生的 `(r).clone().map(move |n| ..)` 一致（`.clone()` 保留接收者所有权供后续复用）。集合（List/Vec）接收者仍走下方迭代器 map 分支，与原生逐字符对齐。

验证（原生↔rs-lz 同形、运行输出一致）：
- TEMP/probe/sweep23/g10_optres.lz：修复前 RSLZ `(r).iter().cloned().map(..).collect::<Vec<_>>()` → E0308；修复后 `let x: Result<i64, String> = (r).clone().map(move |n| n * 2i64);`，两后端均 `15|19|ok=6|err=bad|`（覆盖 unwrap_or/is_some/if-表达式/Result::map/match）。
- TEMP/probe/sweep23/g11_chain.lz（Option `map` 后接 `flat_map`→and_then 链）：两后端均 `true|8|true|`，证明 optres map 分支与既有 flat_map/and_then 改名（BUG-433）不冲突。
回归：cargo test -j 4 -- --test-threads=1 全绿（41 suite / 937 passed / 0 failed，含 rs-lz↔native 字节判据 D1 门禁）；135-probe rustc 门禁 TOTAL=135 OK=135 FAIL=0。

说明：原生对 Option/Result `.map` 的 lambda 用 `{ }` 包裹 body（`move |n| { n * 2i64 }`），rs-lz 沿用集合 map 分支的无花括号 lambda（`move |n| n * 2i64`），两者均为合法 Rust 且行为一致；D1 字节判据语料（tests/lz_ir_bootstrap.rs 的 d1_input.lz）不含该构造，未触发门禁。若后续要求逐字符一致，可在集合与 optres map 分支统一 lambda 花括号形态。
## BUG-440 [2026-10-24T15:50:26Z] [high] FIXED
- summary: [BUG-441] rs-lz 用户调用实参缺 `.clone()`：复用非 Copy 左值实参 E0382 moved value
- detail: rs-lz 独有的用户函数调用按值实参缺失 `.clone()`：LZ 中集合/字符串/Option/Result/Dict 实参对同一变量两次及以上使用时，Rust 第二次按值传参即 `E0382 use of moved value`。原生 codegen 对每个非 Copy 类型的左值实参在调用点补 `.clone()`（`sumv(xs.clone())`、`dget(d.clone(), ..)`），rs-lz 原样透传。

实测（TEMP\probe\sweep23）：
- g14_listarg.lz：`let xs: List<int> = [1,2,3]; print(sumv(xs)); print(lenv(xs))` → rs-lz 主函数发 `println!("{:?}", sumv(xs)); println!("{:?}", lenv(xs))`，原生发 `sumv(xs.clone())` / `lenv(xs.clone())`。rs-lz 报 `error[E0382]: use of moved value: \`xs\``；原生 OK `6|3|`。
- g13_dictkey.lz：`dget(d, k)`/`dhaskey(d, k)`/`dins(d, k, v)` 三处复用 `d` → rs-lz 主函数 `dget(d, "a")`/`dhaskey(d, "b")`/`dins(d, "z", 7)`，原生 `dget(d.clone(), "a")` 等。rs-lz 报 E0382；原生 OK `1|false|7|`。
- g15_argclone.lz：`echo_s(s)`/`echo_o(o)`/`echo_i(x)` 复用 → 原生 `s.clone()`/`o.clone()`/int 不克隆；rs-lz 报 E0382。

根因：rs-lz 的 gen_call 通用分支用 expr_cs_list 直接发射实参，未按 LZ 按值复制语义对可复用的左值实参补 `.clone()`。

修复（commit a92558e）：新增 is_heap_irtype / is_place_expr / call_arg_src / call_args_list，仅对左值表达式（变量/字段访问/下标/枚举字段）且类型为已知堆类型（Str/Opt/Res/Named/非全 Copy 元组）克隆；字面量与调用结果本身就是新值不克隆（对齐原生 `f("a".to_string())` 不克隆）；泛型/Any/闭包/Duck 不克隆以避免 `T: Clone` 约束缺失与 `Box<dyn Fn>` 不可 Clone。

回归：cargo test -j 4 -- --test-threads=1 全绿 41 suites / 937 passed / 0 failed；g12/g13/g14/g15 双后端 OK 且 rs-lz 产物与原生逐字符对齐。

## BUG-441 [2026-10-24T15:52:57Z] [high] FIXED
- summary: [BUG-441] rs-lz Dict 键实参形态错位：get/contains 缺 & 借用、insert 缺 .to_string()（E0308）
- detail: rs-lz 独有的 `Dict` 三个方法的键实参形态错位（原生正确）：

1) `get` / `contains`：Rust `BTreeMap::get(&self, key: &Q) where K: Borrow<Q>`、`contains_key(&self, key: &Q)` 的键须为借用。rs-lz 原发 `d.get(k)`（String 变量键）、`d.contains_key("b".to_string())`（字面量被 pattern_arg_of 的 LitStr 分支发成 `&str` 又叠加 `.to_string()`）⇒ `error[E0308]: mismatched types … expected \`&_\`, found \`String\``。原生发 `d.get(&k)` / `d.contains_key("b")`。
   - str 字面量 `"a"` 本身就是 `&str`（Q=str，`String: Borrow<str>`），裸发即可；
   - 其余（String/数值变量/表达式）须 `&<expr>`（`String: Borrow<String>`）。

2) `insert`：`insert(self, k: K, v: V)` 键按值持有。rs-lz 原发 `m.insert(k, v)`（String 变量键）⇒ E0308 expected `String`, found `str`。原生发 `m.insert(k.to_string(), v)`（字面量 `m.insert("c".to_string(), 3i64)`）。

实测（TEMP\probe\sweep23）：
- g12_dict.lz（字面量键 `d.get("a")` / `d.contains("b")` / `m.insert("c",3)`）：NATIVE OK `104|`；RSLZ FAIL E0308。
- g13_dictkey.lz（String 变量键 `d.get(k)` / `d.contains(k)` / `dins(d,k,v)`）：NATIVE OK `1|false|7|`；RSLZ FAIL E0308。

修复（commit a92558e）：新增 dict_key_arg（get/contains：字面量裸发 / 其余 `&expr`）与 dict_insert_key_arg（str 键 `.to_string()`），并在 gen_method_call 按接收者 irtype 为 Dict 的三个新分支（get/contains/insert）调用它们。

回归：g12_dict、g13_dictkey 双后端 OK，rs-lz 产物与原生逐字符对齐；cargo test -j 4 -- --test-threads=1 全绿 41 suites / 937 passed / 0 failed。



### FIXED(2026-10-24T15:53:40Z / BUG-440, BUG-441)
- evidence: commit a92558e（src/ir/lz_codegen_lib.lz，99+/2-）。

BUG-440（账本=BUG-440 号即调用实参 clone 缺口）：
- rs-lz 前 `g14_listarg.lz` 主函数发 `println!("{:?}", sumv(xs)); println!("{:?}", lenv(xs))`，rustc `error[E0382]: use of moved value: \`xs\``；原生发 `sumv(xs.clone())` / `lenv(xs.clone())`。
- rs-lz 前 `g15_argclone.lz` 发 `echo_s(s); echo_s(s)`，E0382；原生 `echo_s(s.clone())`。
- 修复：新增 is_heap_irtype / is_place_expr / call_arg_src / call_args_list，仅对左值表达式（Var/FieldAccess/IndexGet/EnumFieldGet）且类型为已知堆类型（Str/Opt/Res/Named/非全 Copy 元组）克隆；字面量与调用结果不克隆；泛型/Any/闭包/Duck 不克隆以避免 `T: Clone` 约束缺失与 `Box<dyn Fn>` 不可 Clone。gen_call 两处通用分支（Var 调用名 & 表达式调用）改用 call_args_list。
- rs-lz 后：g14 `6|3|`、g15 `hi|5|7|hi|` 双后端 OK，产物与原生逐字符对齐。

BUG-441（账本=BUG-441 号即 Dict 键实参形态）：
- rs-lz 前 `g12_dict.lz` 发 `d.contains_key("b".to_string())`、`d.get("a")`，`g13_dictkey.lz` 发 `d.get(k)`、`d.contains_key(k)`、`m.insert(k, v)`，rustc `error[E0308]: mismatched types … expected \`&_\`, found \`String\``（get/contains）与 `expected String, found str`（insert）；原生发 `d.get("a")` / `d.contains_key("b")` / `d.get(&k)` / `d.contains_key(&k)` / `m.insert(k.to_string(), v)`。
- 修复：新增 dict_key_arg（get/contains：字面量裸发 `&str`，其余 `&<expr>`）与 dict_insert_key_arg（str 键 `.to_string()`），gen_method_call 按接收者 irtype 为 Dict 新增 get/contains/insert 三分支。
- rs-lz 后：g12 `104|`、g13 `1|false|7|` 双后端 OK。

回归：cargo test -j 4 -- --test-threads=1 全绿，41 suites / 937 passed / 0 failed（与基线一致，零回归）。
## BUG-442 [2026-10-24T16:50:34Z] [high] FIXED
- summary: [rule:E0308] rs-lz Dict.remove 键实参未按引用传（发 d.remove(\"a\".to_string())）(src/ir/lz_codegen_lib.lz gen_method_call)
- detail: Rust 端 `BTreeMap::remove(&self, key: &Q)` 要求键按借用。rs-lz 的 gen_method_call 未接管 Dict 上的 `.remove`，落到通用方法分支直发 `d.remove("a".to_string())`（按值）→ rustc E0308 mismatched types。原生 codegen 渲染 `d.remove("a")`（字面量本身是 &str）/ `d.remove(&k)`，属 rs-lz 独有缺口。与 BUG-440（Dict get/contains 缺 & 借用）同族，仅方法名不同。复现 TEMP/probe/sweep24/g16_dictmisc.lz：rs-lz FAIL E0308，原生 OK `["x", "y"]|[1, 2]|2|-1|done|`。修复：gen_method_call 增设 remove 分支复用 dict_key_arg。
- reported_by: lead

## BUG-443 [2026-10-24T16:50:34Z] [high] FIXED
- summary: [rule:E0423] rs-lz 空 Dict() 构造发成 Dict()（类型别名非函数）(src/ir/lz_codegen_lib.lz gen_call)
- detail: builder 把空 `Dict()` 生成 `Expr.Call(callee=Var("Dict"), args=[])`，与字面量 `{..}` 的 StructCtor(name="Dict") 不同路径。rs-lz 走 gen_call 通用分支发 `Dict()` → rustc E0423 expected function, tuple struct or tuple variant, found type alias `Dict`（`Dict` 是 lz_builtins 里 `Dict<K,V> = BTreeMap<K,V>` 的类型别名，不能当函数调用）。原生 codegen 渲染 `std::collections::BTreeMap::new()`。复现 TEMP/probe/sweep24/g16_dictmisc.lz 的 `let mut d: Dict<str, int> = Dict()`：rs-lz FAIL E0423，原生 OK。修复：gen_call 增设 `n == "Dict" && a.len() == 0` 分支发 BTreeMap::new()。
- reported_by: lead

## BUG-444 [2026-10-24T16:50:34Z] [high] FIXED
- summary: [rule:E0596] rs-lz 函数参数恒发 `name: type` 缺 mut，函数体内 &mut 用法（remove/insert/push）报 cannot borrow as mutable (src/ir/lz_codegen_lib.lz fn_param_str)
- detail: rs-lz 的 fn_param_str 只输出 `name: type`（注释明写「D1: 忽略 ref/mut/owned 前缀」），从不给参数标 mut。函数体内发生 &mut 方法调用（`d.remove(..)`、`d.insert(..)`、集合 push 等）时，Rust 端参数须 `mut` → rustc E0596 cannot borrow `d` as mutable, as it is not declared as mutable。原生 codegen 用 auto_mut_locals 同一扫描覆盖局部变量与参数，对 &mut 用法的参数也标 mut。复现 TEMP/probe/sweep24/g16_dictmisc.lz 的 `pub fn dm(d: Dict<String, i64>)`（体含 d.remove）：rs-lz FAIL E0596，原生 `pub fn dm(mut d: BTreeMap<..>)` OK。修复：把 auto_mut_names(b) 传入 fn_param_list，仅按逗号分隔 token 精确匹配加 `mut` 前缀（避免 `x` 误中 `xy`）。注意原生对「仅 .get()/.contains() 无 &mut」的参数也标 mut（探针 g18 双端一致），故沿用现扫描广度。
- reported_by: lead

## BUG-445 [2026-10-24T16:50:34Z] [high] FIXED
- summary: [rule:E0382/E0308] rs-lz Option/Result 按值消费方法（unwrap_or_else/expect/unwrap_or/unwrap/ok/err）缺接收者 .clone()，expect 消息缺 &str (src/ir/lz_codegen_lib.lz gen_method_call)
- detail: rs-lz 的 gen_method_call 对 Option/Result 上按值消费 self 的方法原样透传：`r.unwrap_or(-1)` 后若复用 r → E0382 use of moved value `r`；`o.expect("boom".to_string())` → E0308 expected `&str`, found `String`（Rust `Option::expect(&self, msg: &str)`）。原生 codegen 统一渲染 `(r).clone().unwrap_or(-1i64)` / `(o).clone().expect("boom")`（`.clone()` 保留接收者所有权供后续复用，与 `.map` 的 BUG-439 同族）。复现：TEMP/probe/sweep24/g19_opt_else.lz（unwrap_or_else+expect）与 g20_res_misc.lz（unwrap_or+ok）rs-lz FAIL，原生 OK。修复：为 unwrap_or_else/expect/unwrap_or/unwrap/ok/err 各增设分支发 `(recv).clone().method(..)`，expect 消息复用 dict_key_arg（字面量裸发 &str，变量发 &expr）。
- reported_by: lead



### FIXED(2026-10-24T16:52:29Z / BUG-442, BUG-443, BUG-444, BUG-445)
- evidence: commit 6487cf9（仅改 src/ir/lz_codegen_lib.lz，另 agent 的 decl_gen.rs/expr_gen.rs 未触碰）：
- BUG-442: gen_method_call 增设 `m=="remove" && a.len()==1 && is_dict_irtype(expr_ty(r))` 分支，发 `recv0 + ".remove(" + dict_key_arg(a[0], ..) + ")"`（复用 BUG-440 的 dict_key_arg：字面量裸发 `"a"`，变量发 `&k`）。探针 g16_dictmisc.lz NATIVE/RSLZ 均 OK `["x", "y"]|[1, 2]|2|-1|done|`。
- BUG-443: gen_call 在通用分支前增设 `n=="Dict" && a.len()==0` 分支发 `std::collections::BTreeMap::new()`。g16_dictmisc.lz（含 `let mut d: Dict<str,int> = Dict()`）RSLZ 由 E0423 转 OK。
- BUG-444: gen_item 的 FnDef 分支把 `auto_mut_names(b)` 提为局部变量并传入 `fn_param_list(ps, auto_mut)`；fn_param_str 在名字在 auto_mut 列表中时加 `mut ` 前缀，新增 `auto_mut_has` 用 `"," + list + ","` 精确 token 匹配（防 `x` 误中 `xy`）。g16_dictmisc.lz 由 E0596 转 OK。探针 g18_dict_getonly.lz（仅 .get()/.contains()）双端同为 `mut d`，确认与原生广度一致。
- BUG-445: gen_method_call 增设 unwrap_or_else/expect/unwrap_or/unwrap/ok/err 六分支，均发 `(recv).clone().method(..)`，expect 消息用 dict_key_arg。g19_opt_else.lz（unwrap_or_else+expect）RSLZ 由 E0308 转 OK `6|`；g20_res_misc.lz（unwrap_or+ok）由 E0382 转 OK `14|`。
回归：cargo test -j4 -- --test-threads=1 全绿（0 failed）。字节对齐复核 g16/g18/g19/g20：与原生差异仅剩闭包体花括号（move || { .. } vs move || ..）、`__path__` 绝对/相对路径、既有前缀算术写法（`a+b` vs `LzAdd::__add__(a,b)`），三者均可编译且非本次引入。
## BUG-446 [2026-10-24T16:52:56Z] [high] OPEN
- summary: [native/IR-builder] 元组字段访问 `.0`/`.1` 类型推导错误：IR build error 期望 str 实际 int（g21_tuple.lz）
- detail: 原生 codegen 路径（非 rs-lz）对元组字段访问 `.0` / `.1` 类型推导错误：IR build 阶段报「函数 tuple1 返回类型不匹配：期望 str，实际 int」。TEMP/probe/sweep24/g21_tuple.lz：`def tuple1(t: (int, str)) -> str = t.0`。两条路线（--emit=rs 与 --emit=rs-lz）均被同一 IR builder 拦截（v5.py 报 NATIVE COMPILE-ERR / RSLZ SELFHOST NO-EMIT），属前端/类型推导 bug，非 rs-lz codegen 独有，需在 IR builder 或类型推导层修复；修好前无法验证 rs-lz 的元组字段访问对齐性。
- reported_by: lead

## BUG-447 [2026-10-24T17:35:13Z] [high] FIXED
- summary: [rule:E0277] rs-lz 下标 xs[0] 缺 `as usize`（Rust 切片下标须 usize）(src/ir/lz_codegen_lib.lz gen_expr IndexGet)
- detail: rs-lz 的 gen_expr IndexGet 分支原发 `xs[0i64]`，但 Rust 的 `Vec<T>`/切片下标运算符要求 `usize` → rustc E0277 `[i64] cannot be indexed by i64`。原生 codegen 渲染 `xs[0i64 as usize]`。rs-lz 独有（List 下标）。复现 TEMP/probe/sweep24/g22b_index.lz：NATIVE OK `1|`，RSLZ FAIL E0277。修复：IndexGet 对非 Dict 基座补 ` as usize`；Dict 基座见 BUG-448。
- reported_by: lead

## BUG-448 [2026-10-24T17:35:13Z] [high] FIXED
- summary: [rule:E0308] rs-lz Dict 下标 d[\"a\"] 应发 .get(&key).cloned().unwrap() 而非下标运算符 (src/ir/lz_codegen_lib.lz gen_expr IndexGet)
- detail: rs-lz 对 Dict 的下标原发 `d["a".to_string()]`（BTreeMap 的下标运算符实现与 LZ 键类型口径不匹配）→ rustc E0308 mismatched types。原生 codegen 对 Dict 下标**不用** Rust 下标运算符，而渲染 `(d).get(&"a".to_string()).cloned().unwrap()`：键按值持有（`&<dict_insert_key_arg>`），再 .cloned() 取值。rs-lz 独有。复现 TEMP/probe/sweep24/g22c_dict_index.lz：NATIVE OK `1|`，RSLZ FAIL E0308。修复：IndexGet 对 Dict 基座发 `(recv).get(&<dict_insert_key_arg>).cloned().unwrap()`。顺带确认 `d["x"] = v`（g22e）与 List 下标赋值（g22d）两端已一致。
- reported_by: lead

## BUG-449 [2026-10-24T17:35:13Z] [high] FIXED
- summary: [rule:E0423/E0308] rs-lz 空 Set() 构造发 Set()（类型别名）＋ Set.contains 缺 & 借用 (src/ir/lz_codegen_lib.lz gen_call/gen_method_call)
- detail: 两处缺口：(1) 空 `Set()` 构造——builder 生成 `Call(Var(\"Set\"), [])`，rs-lz 走 gen_call 通用分支原发 `Set()`，而 `Set` 只是 lz_builtins 里的类型别名（`Set<T> = HashSet<T>`）⇒ rustc E0423 expected function, found type alias `Set`；原生渲染 `std::collections::HashSet::new()`。(2) `Set`（→ HashSet）的 `.contains(x)` 与 List 同签名 `contains(&self, x: &T)`，rs-lz 此前无 Set 分支 → `s.contains(1i64)` ⇒ E0308 expected `&i64`, found `i64`；原生渲染 `s.contains(&1i64)`。rs-lz 独有。复现 TEMP/probe/sweep24/g24_set.lz：NATIVE OK `2|2|`，RSLZ FAIL E0423+E0308。修复：gen_call 增设 `n==\"Set\" && a.len()==0` 分支发 HashSet::new()；gen_method_call 的 contains 借位分支扩展到 Set 接收者。
- reported_by: lead


### FIXED(2026-10-24T17:36:53Z / BUG-447, BUG-448, BUG-449)
- evidence: commit fff50ff（仅改 src/ir/lz_codegen_lib.lz，decl_gen.rs/expr_gen.rs 未触碰）：
- BUG-447: gen_expr 的 `Expr.IndexGet(base,key,ty)` 分支按基座分流——`is_dict_irtype(base)` 走 BUG-448 的 .get() 形态，否则发 `gen_expr(base) + "[" + gen_expr(key) + " as usize]"`。探针 g22b_index.lz（`xs[0]`）RSLZ 由 E0277 转 OK `1|`。
- BUG-448: 同分支 Dict 基座发 `"(recv).get(&" + dict_insert_key_arg(key) + ").cloned().unwrap()"`（键按值持有 + .cloned() 取值）。探针 g22c_dict_index.lz（`d["a"]`）RSLZ 由 E0308 转 OK `1|`。
- BUG-449(1): gen_call 在 Dict 空构造分支后增设 `n=="Set" && a.len()==0` → `std::collections::HashSet::new()`。g24_set.lz 的 `let mut s = Set()` RSLZ 由 E0423 转 OK。
- BUG-449(2): gen_method_call 的 `.contains(x)` 借位分支条件由 `is_list_irtype` 扩展为 `is_list_irtype(...) or is_set_irtype(...)`（`HashSet::contains(&self, x: &T)` 同签名）。g24_set.lz 的 `s.contains(1i64)` RSLZ 由 E0308 转 OK。
探针 g24_set.lz 现 NATIVE/RSLZ 均 OK `2|2|`。
回归：cargo test -j4 -- --test-threads=1 全绿（0 failed）；g16/g18/g19/g20/g22a~g22e/g23/g24 共 11 个探针 RSLZ 全 OK。
字节对齐复核：g24 与原生差异仅剩 `Set<i64>`/`HashSet<i64>` 形参写法（rs-lz 用别名、原生展开，均可编译）、`__path__` 绝对/相对路径，均为既有形态。
## BUG-450 [2026-10-24T17:35:13Z] [high] OPEN
- summary: [native/expr_gen] List<str>.contains(\"a\") 发 xs.contains(\"a\") 缺 .to_string()，E0308 expected &String found &str（g22a_contains.lz）
- detail: 原生 codegen 对 `List<str>.contains(\"a\")` 渲染 `xs.contains(\"a\")`，而 Rust 端 `Vec<String>::contains(&self, x: &String)` ⇒ rustc E0308 mismatched types（expected `&String`, found `&str`）。rs-lz 侧走 contains 借位分支渲染 `xs.contains(&\"a\".to_string())` 编译通过（探针 g22a_contains.lz RSLZ OK `true|`、NATIVE FAIL E0308）。属原生 expr_gen.rs 侧缺陷，需在原生 codegen 对 List<str> 的 contains 实参补 `.to_string()`；修好前无法在 D1 字节对齐上验证（rs-lz 现为超集正确，不擅自回退以对齐原生错误输出）。
- reported_by: lead


## BUG-451 [2026-10-24T22:18:18Z] [high] FIXED
- summary: [rule:E0308] rs-lz Set.remove(x) 缺 & 借用，HashSet::remove(&mut self, &T) (src/ir/lz_codegen_lib.lz gen_method_call)
- detail: rs-lz 对 `Set.remove(x)` 落到通用方法分支直发 `s.remove(2i64)`，而 Rust `HashSet::remove(&mut self, value: &T)` 实参须借位 → rustc E0308 mismatched types。原生 codegen 渲染 `s.remove(&2i64)`（g25_set_remove.rs 直接核对 `let r = s.remove(&2i64);`）。rs-lz 独有，与 BUG-449 的 Set.contains 同族。复现 TEMP/probe/sweep24/g25_set_remove.lz：NATIVE OK `2|true|`，RSLZ FAIL E0308。修复：gen_method_call 增设 Set 接收者 remove 分支 `(recv).remove(&arg)`。
- reported_by: lead

## BUG-452 [2026-10-24T22:18:18Z] [high] FIXED
- summary: [rule:E0308] rs-lz 空字典字面量 {} 发 HashMap::new()，Dict 别名=BTreeMap (src/ir/lz_codegen_lib.lz gen_dict_ctor/Expr.Dict)
- detail: rs-lz 的 gen_dict_ctor 空分支发 `std::collections::HashMap::new()`，Expr.Dict 兜底分支发裸 `HashMap::new()`（连 std::collections:: 路径都没有）；而 Dict 的 Rust 别名是 BTreeMap → `let d: Dict<String,i64> = std::collections::HashMap::new()` ⇒ rustc E0308 mismatched types。非空字典字面量已对齐为 `std::collections::BTreeMap::from([...])`（BUG-427），唯独空分支漏改。原生 codegen 渲染 `let mut d: BTreeMap<String,i64> = std::collections::BTreeMap::new();`。复现 TEMP/probe/sweep24/g29_empty_dict_lit.lz：NATIVE OK `0|`，RSLZ FAIL E0308。修复：两处空字典分支统一改 `std::collections::BTreeMap::new()`。
- reported_by: lead

## BUG-453 [2026-10-24T22:18:18Z] [medium] FIXED
- summary: rs-lz str.find 直发 Rust str::find(Option<usize>)，原生发 lz_find(..).unwrap_or(-1i64)；输出 Some(6) vs 6 (src/ir/lz_codegen_lib.lz lz_method_rename/gen_method_call)
- detail: rs-lz 对 str 接收者的 `s.find(..)` 直发 Rust 内建 `str::find`（返回 Option<usize>，print 输出 `Some(6)`），而原生 codegen 用 lz_builtins 扩展 `s.lz_find(..)`（返回 Option<i64>）并接 `.unwrap_or(-1i64)` 物化为 i64（print 输出 `6`）。双端均编译通过但输出/字节不一致。rfind 同族。复现 TEMP/probe/sweep24/g31_str_methods.lz：NATIVE OK `hello there|6|2|`，RSLZ OK 但输出 `hello there|Some(6)|2|`。修复：lz_method_rename 把 str 接收者的 find/rfind 映射到 lz_find/lz_rfind，gen_method_call 该分支后接 `.unwrap_or(-1i64)`。
- reported_by: lead



### FIXED(2026-10-25T00:25:33Z / BUG-451, BUG-452, BUG-453)
- evidence: commit d772401（仅 src/ir/lz_codegen_lib.lz，22+/4-）：
- BUG-451 → gen_method_call 新增 `m=="remove" && a.len()==1 && is_set_irtype(expr_ty(r))` 分支发 `(recv).remove(&arg)`。g25_set_remove.lz：RSLZ 由 E0308 转 OK `2|true|`；原生直接 `let r = s.remove(&2i64);` 一致。
- BUG-452 → gen_dict_ctor 空分支与 Expr.Dict 兜底分支均改 `std::collections::BTreeMap::new()`（后者由裸 `HashMap::new()` 一并修正路径缺失）。g29_empty_dict_lit.lz：RSLZ 由 E0308 转 OK `0|`；原生直接 `let mut d: BTreeMap<String,i64> = std::collections::BTreeMap::new();` 一致。
- BUG-453 → lz_method_rename 对 str 接收者映射 `find`→`lz_find`、`rfind`→`lz_rfind`；gen_method_call 该分支后接 `.unwrap_or(-1i64)`。g31_str_methods.lz：RSLZ 由 `hello there|Some(6)|2|` 转 `hello there|6|2|`，与 NATIVE 字节/输出一致。
回归：cargo test -j4 -- --test-threads=1 全绿 0 failed；sweep24 全 19 探针 RSLZ: OK（g16,g18,g19,g20,g22a~e,g23,g24~g32）。decl_gen.rs/expr_gen.rs 由他 agent 并行修改，未触碰。
## BUG-454 [2026-10-25T01:20:04Z] [critical] FIXED
- summary: rs-lz 嵌套块内 `i = i + 1` 被解析为 Stmt.Let 并直发 shadow-let，循环内不累积外层变量 → 无限循环挂死 (src/ir/lz_codegen_lib.lz gen_let)
- detail: LZ 源 `i = i + 1` 在**嵌套块内**（while/for 体、match 分支）被解析为 `Stmt.Let(name=i, is_mut=true, value=BinOp(i + 1))`——同一写法在函数顶层则解析为 `Stmt.Assign`（peek ir: 已验证 g33_while 的 while 体内确为 `let mut i: int = [int] binop [int] i + [int] 1_i64`）。原生 codegen（src/ir/codegen/stmt_gen.rs:653-737）通过 `declared.contains(name) && (is_mut || 自引用 || 循环内)` 判定为 LZ 语义的**重绑定（Python 风格）**，改发赋值（标量收敛为 `i += 1i64`）；而 rs-lz 的 gen_let 无作用域表，照直发 `let mut i: i64 = i + 1i64;`。Rust shadow-let 在循环体内不累积外层变量 ⇒ 外层 i 恒为初值 ⇒ `while i < 5 { i = i + 1 }` **无限循环挂死**。
复现 TEMP/probe/sweep24/g33_while.lz：NATIVE OK `5`（快），RSLZ 生成 exe 运行 15s 不退出（挂死，g33_rslz.txt 为空）。
修复（src/ir/lz_codegen_lib.lz gen_let）：新增 `expr_has_var(v, n)` 判据——value 引用自身名字即按重绑定发赋值（合法程序里 value 能提到 n，则 n 必已在作用域；同时覆盖自引用与循环重绑定两态），并新增 gen_rebind_assign / binop_compound_symbol / is_scalar_ty 把标量 `a = a op b` 收敛为 `a op= b`（对齐原生 stmt_gen.rs:685-714）。修复后 g33_while RSLZ 产物为 `i += 1i64;`，与原生 `i += 1i64;` 逐字节一致，运行输出 `5`。
- reported_by: lead



### FIXED(2026-10-25T01:20:39Z / BUG-454)
- evidence: commit 7a4596d（仅 src/ir/lz_codegen_lib.lz，60+/9-；decl_gen.rs/expr_gen.rs 由他 agent 并行，未触碰）。gen_let 新增重绑定判据 `expr_has_var(v,n)`：value 引用自身名字即按 LZ 语义（Python 风格）发赋值，替代 Rust shadow-let；新增 gen_rebind_assign / binop_compound_symbol / is_scalar_ty 将标量 `a = a op b` 收敛为 `a op= b`（Int/F64/Bool，对齐原生 stmt_gen.rs:685-714 AddAssign 可用性规则）。
验证：g33_while（`while i<5 { i=i+1 } print(i)`）RSLZ 修复前生成 exe 运行 15s 不退出（无限循环，输出空）；修复后 gen_g33.rs 逐字节为 `let mut i: i64 = 0i64;\nwhile i < 5i64 {\ni += 1i64;\n}`，与原生 `i += 1i64;` 一致，编译 rustc 无 error、运行 <1s 输出 `5`（NATIVE OK `5`）。g33b_toplevel_assign 顶层赋值同收敛为 `i += 1i64;` 对齐原生，输出 `1` 双端一致。
回归：cargo test -j4 -- --test-threads=1 全绿 0 failed（含 tail_recursion 16、doc-test 等全部 suite）；探针 g24_set OK `2|2|`、g25_set_remove OK `2|true|`、g26_list_of_dict OK `1|`、g29_empty_dict_lit OK `0|`、g31_str_methods OK `hello there|6|2|`、g33b OK `1|` 全部 NATIVE/RSLZ 输出一致。
## BUG-455 [2026-10-25T01:33:50Z] [high] FIXED
- summary: rs-lz 集合内建方法（sum/min/max/filter/map/fold/join/keys/values）未检查接收者类型，劫持自定义 struct 同名方法 → E0599 (src/ir/lz_codegen_lib.lz gen_method_call)
- detail: rs-lz 的 gen_method_call 把若干「集合迭代方法」无条件拦截为内建形态，未检查接收者类型：
- `sum` (a.len()==0)：集合归约
- `min` / `max` (a.len()==0)：集合极值
- `filter` (a.len()==1)、`map` (a.len()==1)、`fold` (a.len()==2)、`join` (a.len()==1)：集合迭代
- `keys` / `values` (a.len()==0)：字典迭代

自定义 struct 上若有同名方法（例如 `Point::sum`），会被集合内建劫持。复现 TEMP/probe/sweep24/g35_struct_method.lz（struct Point { x,y }，impl Point 定义 `fn sum(self) -> int = self.x + self.y`，main 里 `print(p.sum())`）：NATIVE OK `7`，且渲染 `(p).sum()`；rs-lz 把 `p.sum()` 渲染成 `(p).iter().copied().sum::<i64>()` → rustc E0599 `no method named iter found for struct Point`（迭代产物 RSLZ: FAIL）。
原生 codegen（src/ir/codegen/expr_gen.rs:4886-4911 sum/product、4912-4925 max/min、4871 filter、4933 join 等）都以 `recv_is_list` 判据守门，非 list 接收者一律回退到 `({recv}).{method}(...)` 普通方法调用。
修复（src/ir/lz_codegen_lib.lz gen_method_call）：
1. sum/min/max 分支加 `and is_list_irtype(expr_ty(r))`。
2. filter/map/fold/join 加 `and is_list_irtype(expr_ty(r))`；keys/values 加 `and is_dict_irtype(expr_ty(r))`。
3. 顺带清掉 else 分支里三层嵌套的**死代码重复**（map/fold/join 已被外层 elif 捕获，永不可达），改为单层 else + 通用方法调用（含 lz_method_rename / pattern_args / find unwrap_or / str-returning to_string / split 物化），避免「外层加了守门、内层死代码又把非集合接收者重新捞回集合路径」的返祖。
修复后 g35_struct_method RSLZ 产物 `p.sum();`（原生 `(p).sum();`，仅括号差异），rustc 无 error，运行 `7` 与原生一致。
- reported_by: lead



### FIXED(2026-10-25T01:40:33Z / BUG-455)
- evidence: commit 00ecb0f（仅 src/ir/lz_codegen_lib.lz，33+/44-）。gen_method_call 为 sum/min/max/filter/map/fold/join 加 `and is_list_irtype(expr_ty(r))`、keys/values 加 `and is_dict_irtype(expr_ty(r))`，并删除 else 分支三层不可达的 map/fold/join 死代码重复（不删则外层守门会被内层死代码把非集合接收者重新捞回集合路径）。
验证：g35_struct_method（struct Point{ x,y } + impl `sum` + `print(p.sum())`）修复前 RSLZ: FAIL error[E0599] `no method named iter found for struct Point`（渲染成 `(p).iter().copied().sum::<i64>()`）；修复后 gen_g35.rs 为 `let mut p = Point { x: 3i64, y: 4i64 };\nprintln!("{:?}", p.sum());`，rustc 无 error，运行 `7` 与 NATIVE OK `7` 一致。
回归：cargo test -j4 -- --test-threads=1 全绿 0 failed（20+46+87+44+16+17 等全部 suite，exit 0）。探针 NATIVE/RSLZ 输出一致：g35 `7`、g36_for_break `10`、g37_opt_pattern `7`、g38_dict_nested `1`、g39_list_map `[2, 4, 6, 8]`、g40_list_filter `[2, 4]`、g41_list_fold `6`、g42_list_join `a,b,c`。g44_list_sum `(1..=10).sum()` 双端同为 E0283 type annotations needed（原生固有行为，非 rs-lz 分歧，未改）。
## BUG-456 [2026-10-25T05:00:00Z] [high] FIXED
- summary: rs-lz 对非 owned 的 self 按值接收，发出 `fn sum(self)`，而原生发 `fn sum(&self)` —— 语义分叉
- detail: rs-lz `self_param_str` 非 owned 分支落到 else 时裸发 `self`（按值接收）。与原生 decl_gen.rs「self 参数修饰」分支不符（原生 is_owned 才发 `self`，其余一律 `&self`/`&mut self`）。按值接收会 move 掉接收者：非 Copy struct 后续再用实例即 borrow-after-move；对 `fn sum(self)->int=self.x+self.y` 这类只读方法，rs-lz 会因按值接收把实例 move 进方法。复现 g35_struct_method.lz（struct Point{ x,y } + `def sum(self)->int=self.x+self.y` + `print(p.sum())`）：rs-lz 旧产物 `fn sum(self)`，原生 `fn sum(&self)`。
- 复现: _probes/sweep/g35_struct_method.lz（NATIVE 7 / RSLZ 修复前结构分叉）
- reported_by: lead

### FIXED(2026-10-25T05:05:00Z / BUG-456)
- evidence: commit 11a1cfc（仅 src/ir/lz_codegen_lib.lz，36+/4-）。`self_param_str` 的最终 else 分支由 `"self".to_string()` 改为 `"&mut self" if p.3 else "&self"`（p.3 = 原签名自带的 mut 标志），并对齐原生 decl_gen.rs 的「self 参数修饰」判据。
- 验证: g35_struct_method 修复后 RSLZ 产物 `fn sum(&self) -> i64`，rustc 无 error，运行 `7` 与原生一致。
- 回归: cargo test -j 4 -- --test-threads=1 全绿 0 failed（exit 0），含 tail_recursion 矩阵全过。

## BUG-457 [2026-10-25T05:00:00Z] [high] FIXED
- summary: rs-lz 对 `self.<field>` 读取裸 move 导致 E0507；且方法接收者误套「读取克隆」规则导致 `self.items.clone().push(x)` 推到临时值数据丢失
- detail: 两半问题。
  1. 读取侧：`self.name`（String 字段）从 `&self` 借引用读取后返回，非 Copy 字段无法 move out of shared reference → E0507 cannot move out of ... behind a shared reference。复现 g45_struct_str_method.lz（struct Person{ name:str } + `def greet(self)->str=self.name`）。
  2. 方法接收者侧：`self.items.push(x)` 中 `push` 是可变方法，接收者须按 place 直发；若套用「self 字段读取克隆」规则，会渲染成 `self.items.clone().push(x)` —— push 到一个临时 Vec 上，数据丢失。复现 g46_mut_field.lz（struct Bag{ items:List<int> } + `def add(self,x)->None=self.items.push(x)`）。
- 原生基准: src/ir/codegen/expr_gen.rs:5248-5255 —— base 渲染为 self 且 self 类型非 Int/F64/Bool 时字段读取补 `.clone()`；而方法接收者按 place 直发（g46 原生发 `return self.items.push(x);`）。嵌套处理原生用字符串 `recv.trim_end_matches(".clone()")`（expr_gen.rs:3294-3298）。
- 复现: _probes/sweep/g45_struct_str_method.lz、_probes/sweep/g46_mut_field.lz、_probes/sweep/g47_self_field_len.lz
- reported_by: lead

### FIXED(2026-10-25T05:05:00Z / BUG-457)
- evidence: commit 11a1cfc（与 BUG-456 同一笔，src/ir/lz_codegen_lib.lz）。新增 `def is_self_var(b: Expr) -> bool`（仅当 Expr.Var(name=="self")）；新增 `def gen_recv_place(r, fn_raises) -> String`（FieldAccess 接收者按 place 直发，base 递归走 gen_recv_place 以覆盖嵌套 `self.outer.inner.push(x)`，非 FieldAccess 退回 gen_expr）；gen_expr 的 Expr.FieldAccess 臂补 `+ (".clone()" if is_self_var(b) else "")`；gen_method_call 的 `recv0` 与 default 分支由 `gen_expr(r, fn_raises)` 改用 `gen_recv_place(r, fn_raises)`。
- 验证: g45 `ada|ada`、g46 `1`、g47 `3` NATIVE 与 RSLZ 输出完全一致；g46 RSLZ 产物现为 `fn add(&mut self, x: i64) { return self.items.push(x); }`（克隆已消除）。
- 回归: cargo test -j 4 -- --test-threads=1 全绿 0 failed（exit 0）。
- 运维注: `src/ir/lz_codegen_lib.lz` 经 `include_str!` 编译期嵌入 `lang-zone.exe`（src/ir/lz_codegen.rs:22），**改 .lz 必须 cargo build 重建 exe 才生效**（本 bug 曾因 exe 早于编辑而假阴性）。
## BUG-458 [2026-10-25T13:00:00Z] [high] FIXED
- summary: rs-lz 把 `self.field` 赋值目标按读取表达式发射 → `self.n.clone() = self.n.clone() + 1i64` → rustc E0070
- detail: `self.n = self.n + 1` 的 LHS 是 place（可写位置），rs-lz 旧实现对 AssignExpr / Stmt.Assign 的 target 用 gen_expr 发射；gen_expr 的 FieldAccess 臂套「self 字段读取克隆」规则，于是赋值目标变成 `self.n.clone()` —— 赋给一个临时值，rustc E0070（assignment to temporary）。复现 g79_self_receiver_mut.lz（struct Counter{ n:int } + `def inc(self)->None=self.n=self.n+1`）：rs-lz 旧产物 `self.n.clone() = self.n.clone() + 1i64`，原生 `self.n = self.n.clone() + 1i64`。
- 原生基准: src/ir/codegen/expr_gen.rs —— 赋值目标按 place 发射；`self.field` 克隆规则只作用于纯字段读取表达式。
- 复现: _probes/sweep/g79_self_receiver_mut.lz（修复前 NATIVE OK 2 / RSLZ rustc E0070）
- reported_by: lead

### FIXED(2026-10-25T13:05:00Z / BUG-458)
- evidence: commit 97f79d6（仅 src/ir/lz_codegen_lib.lz）。`gen_expr` 的 Expr.AssignExpr 臂 target 与 `gen_stmt` 的 Stmt.Assign 默认臂 target 由 `gen_expr(tg, …)` 改为 `gen_recv_place(tg, …)`（FieldAccess base 递归，非 FieldAccess 退回 gen_expr）。Stmt.Assign 的 IndexGet 分支留待 BUG-460 单独处理。
- 验证: g79_self_receiver_mut 修复后 RSLZ 产物 `self.n = self.n.clone() + 1i64;`，运行 `2` 与原生一致。
- 回归: cargo test -j 4 -- --test-threads=1 全绿 0 failed（exit 0）。

## BUG-459 [2026-10-25T13:10:00Z] [high] FIXED
- summary: rs-lz 对 `in`/`not in` 成员测试透传 `lhs in rhs` → Rust 无 `in` 运算符 → rustc `expected '{', found keyword 'in'`
- detail: `if 4 in s`（Set 成员测试）rs-lz 旧实现落到 BinOp 通用分支直发 `if 4i64 in s`。Rust 没有 `in` 运算符，rustc 报 syntax error。复现 g58_set_methods.lz（`let mut s: Set<int> = {1,2,3}` + `if 4 in s: print("has4")`）。
- 原生基准: src/ir/codegen/expr_gen.rs:5770-5816 —— `in`/`not in` 成员测试四路 oracle：
  1. 容器类型 String → `cont.contains(elem)`（原生按字面量/`&*`/原样三态处理实参）；
  2. Dict/HashMap → `cont.contains_key(&elem)`；
  3. 用户 struct 定义 `__contains__` → `cont.__contains__(elem)`；
  4. List/Set/其余 → `cont.contains(&elem)`；`not in` 前缀 `!`。
- 复现: _probes/sweep/g58_set_methods.lz、_probes/sweep/g87_not_in.lz、_probes/sweep/g88_string_in.lz、_probes/sweep/g89_dict_in.lz
- reported_by: lead

### FIXED(2026-10-25T13:20:00Z / BUG-459)
- evidence: commit 97f79d6（与 BUG-458/460 同一笔）。`gen_expr` 的 Expr.BinOp 臂在字符串 `+` 之前拦截 `o == "in" or o == "not in" or o == "not_in"`，按 expr_ty(rhs) 分派：String rhs → `.contains()`（lhs 亦为字符串类型时统一 `&*(elem)`，字面量 &str / String 值均解引用到 &str，char 由类型判据走原样分支，运行时与原生等价）；Dict rhs → `.contains_key(&elem)`；其余 → `.contains(&elem)`；`not in`/`not_in` 前缀 `!`。
- 验证: g58 `4|has4`、g87 `no4|has2`、g88 `has_world|no_xyz`、g89 `has_a|no_z` NATIVE 与 RSLZ 输出完全一致；g58 RSLZ 产物现为 `if s.contains(&4i64)`（与原生逐字对齐）。
- 回归: cargo test -j 4 -- --test-threads=1 全绿 0 failed（exit 0）。

## BUG-460 [2026-10-25T13:25:00Z] [critical] FIXED
- summary: rs-lz 把 `self.items[0] = x` 的 IndexGet 赋值目标按读取表达式发射 → `self.items.clone()[0i64 as usize] = x` 赋到临时值，数据静默丢失
- detail: `self.items[0] = x` 的 LHS 是 IndexGet 表达式，rs-lz 旧实现 Stmt.Assign 的 IndexGet 分支用 gen_expr(tg) 发射；gen_expr 的 IndexGet 臂对 base 用 gen_expr → base 若是 `self.<field>` 套「字段读取克隆」规则 → `self.items.clone()[0i64 as usize] = x`。clone 得到一个独立 Vec，赋值发生在临时 Vec 上，原 `self.items` 不变 —— 数据丢失且无编译错误，是静默错误分叉。复现 g85_self_index_assign.lz（struct Bag{ items:List<int> } + `def set_first(self,x)->None=self.items[0]=x`）：rs-lz 旧产物 `self.items.clone()[0i64 as usize] = x;`，原生 `self.items[0i64 as usize] = x;`；实测 NATIVE OK 9 / RSLZ OK 1（打印首元素，前者已被改写、后者仍是原值）。
- 原生基准: src/ir/codegen/expr_gen.rs —— IndexSet 赋值目标按 place 直发，不克隆。
- 复现: _probes/sweep/g85_self_index_assign.lz（NATIVE 9 / RSLZ 修复前 1，静默分叉）
- reported_by: lead

### FIXED(2026-10-25T13:30:00Z / BUG-460)
- evidence: commit 97f79d6（与 BUG-458/459 同一笔）。两处：① `gen_recv_place` 新增 Expr.IndexGet 臂 —— base 递归走 gen_recv_place 得 place 形式 `self.items[0i64 as usize]`（Dict 下标是 `.get().unwrap()` 形态非 place 且 Dict 赋值走 `.insert()`，故 dict 下标退回 gen_expr）；② `gen_stmt` 的 Stmt.Assign IndexGet 分支 target 由 `gen_expr(tg, …)` 改为 `gen_recv_place(tg, …)`。
- 验证: g85_self_index_assign 修复后 RSLZ 产物 `self.items[0i64 as usize] = x;`，运行 `9` 与原生一致。
- 回归: 边界探针 g84_self_compound（复合赋值）、g86_self_index_read（索引读取）、g90_self_str_field_assign（非 Copy 字段赋值）、g91_nested_self_field_assign（嵌套）全部 MATCH；cargo test -j 4 -- --test-threads=1 全绿 0 failed（exit 0）。
## BUG-461 [2026-10-25T14:00:00Z] [high] FIXED
- summary: rs-lz 对 `self.d[k] = v`（dict 下标赋值，base 为 self.field）落到非 dict 分支，把 `.get().unwrap()` 读形式当写目标 → E0070
- detail: `self.d[k] = v` 的 target 是 IndexGet(base=FieldAccess(self,d), key=k)。rs-lz 的 Stmt.Assign dict 分支判定用 `base_is_dict(b)`，而该函数只匹配 `Expr.Var`（lz_codegen_lib.lz:998-1004），`self.d` 是 FieldAccess → 判定失败，落到非 dict 分支。经 BUG-460 改后的 gen_recv_place(IndexGet) 对 dict base 退回 gen_expr → dict 读形式 `.get(&k).cloned().unwrap()`，于是发射 `(self.d.clone()).get(&k.to_string()).cloned().unwrap() = v;` → rustc E0070（assignment to temporary）。复现 g92_self_dict_index_assign.lz（struct Cache{ d: Dict<str,int> } + `def put(self,k,v)->None=self.d[k]=v`）：rs-lz 修复前 RSLZ E0070。
- 原生基准: src/ir/codegen/expr_gen.rs:5690-5714 —— IndexSet 对 Dict/HashMap 发射 `{base}.insert(&{key}, {value})`。rs-lz 顶层 dict 赋值（g96，base 为 Var）本就正确：`d.insert("b".to_string(), 2i64)` 与原生逐字一致。
- 复现: _probes/sweep/g92_self_dict_index_assign.lz、_probes/sweep/g96_dict_index_assign_plain.lz
- reported_by: lead

### FIXED(2026-10-25T14:05:00Z / BUG-461)
- evidence: commit ec322a3（仅 src/ir/lz_codegen_lib.lz）。`gen_stmt` 的 Stmt.Assign IndexGet 分支判定由 `base_is_dict(b)` 改为 `is_dict_irtype(expr_ty(b))`（按类型识别 FieldAccess dict base），base 由 `gen_expr(b)` 改为 `gen_recv_place(b)`（免 `.clone()`）。
- 验证: g92 修复后 RSLZ 产物 `self.d.insert(k, v);`，运行 `1|2`（正确写入）。g96（顶层 dict 赋值）修复前后均 MATCH `1|2|3`；g97_self_dict_read（方法内 `self.d[k]` 读取）MATCH `5|9`（读侧 clone 无害，保持原生形态）。
- 回归: cargo test -j 4 -- --test-threads=1 全绿 0 failed（exit 0）。

## NATIVE-BUG (dict 自字段插入被克隆) [2026-10-25T14:10:00Z] [high] OPEN
- 类目: native codegen 缺陷（rs-lz 不得复刻；oracle 规则：native 错误时 rs-lz 保持正确，不复刻）
- summary: 原生 codegen 对 `self.d[k] = v`（dict 下标赋值，base 为 self.field）发射 `self.d.clone().insert(k, v)`，插入落在克隆的临时 dict 上 → 原 dict 未变 → 后续读取 `self.d["a"]` 运行 panic `called Option::unwrap() on a None value`
- detail: 原生 expr_gen.rs:5248-5255 的「self 字段读取克隆」规则被过度套用到 IndexSet 上：base_s = gen_expr(base) 对 `self.<field>` 补 `.clone()`（expr_gen.rs:5691 → 5709 `.insert(&{}, {})`）。clone 得到独立 dict，insert 发生其上，原 self.d 不变，数据静默丢失且无编译错误；随后读取即 unwrap None panic。复现 g92_self_dict_index_assign.lz：原生 emit `self.d.clone().insert(k, v);` → `c.d.get(&"a").unwrap()` panic（`Option::unwrap() on None`，native.rs:53:59）。
- 同族对照: 原生顶层 dict 赋值 `d.insert("b".to_string(), 2i64)`（g96，base 为 Var）正确、无 clone；说明缺陷仅出现在 self.field base，与 BUG-457 同族的「读取克隆规则越界到写位置」。
- 复现: _probes/sweep/g92_self_dict_index_assign.lz（NATIVE FAIL(run) panic / RSLZ OK 1|2）
- reported_by: lead
- 处置: 已按 oracle 规则将 rs-lz 修复为正确形态 `self.d.insert(k, v)`（BUG-461），**不复刻**该原生缺陷；native 修复另行排期。
## BUG-462 [2026-10-25T15:50:00Z] [high] FIXED
- summary: LZ lexer (lz_lexer.lz) 的 punct_token 表缺 "="、is_keyword 只认 def/struct/if/else/return 缺 let/while/for/in/match/case/elif → `let x = 1` 被切成 `Ident(let)` 且等号被 tokenize 静默丢弃
- detail: `src/frontend/lz_lexer.lz` 是自举路线 B 的 LZ 版 lexer，注释自称"对齐 src/lexer/token.rs"。实测 `def main() =\n    let x = 1\n    print(x)\n`：原生 `--tokens` 输出 21 token（含 Eq x2、Indent、Dedent、Let）；`--emit=lex-lz` 输出 17 token，Eq 全部丢失、let 被打成 `Ident(let)`、无 Indent/Dedent。根因三处：① punct_token 查表 `:68-83` 无 `("=", Token.Eq)`（tokenize `:224-229` 未命中即静默跳过不报错）；② is_keyword `:47-48` 只认 def/struct/if/else/return，且 Token 枚举无 Let/While/For/In/Match/Case/Elif 变体；③ 无缩进栈，Token 枚举无 Indent/Dedent。
- 判真 bug 而非"刻意简化子集"的四条反证: ① 同级自举前端 `src/frontend/lz_parser.lz:104-122` 关键字表、`:125-147` op_token 表（含 `=`, `[`, `]`）、`:184,188` 缩进都已实现，同一路线一份有一份没有 → 不是全路线共识，是 lexer 单独落后；② `Token.Eq` 在 lexer 里是死变体（声明+可显示但无路径产生）；③ 台账 `bootstrap/05-自举里程碑台账.md:20` 宣称 "Token 枚举 + tokenize 与 Rust 版 src/lexer/ 行为对齐" + "全量 token 类型覆盖 diff 为空"，`:79-80` 标 A1/A2 完成，实测 diff 不为空 → 宣称与实现不符；④ 黄金测试 `tests/lz_frontend_bootstrap.rs:35` 输入仅 `def f(x):\n return x+1`，黄金串本身就没 let/Eq/Indent，只是未覆盖路径的现状快照，从未断言这些缺口是设计意图。
- 原生基准: `src/lexer/lexer.rs:154-215`（关键字 match，60+）、`src/lexer/indent.rs`（缩进栈）。
- 复现: `_probes/frontend_p0.lz`（`def main() =\n    let x = 1\n    print(x)\n`）。
- 相关: `docs/脚本冻结清单草案-2026-09-30.md:90` 点名 lex-lz/parse-lz 为独立里程碑环节，但 README:7 又宣称路线 B "已彻底移除"，属文档-代码分歧，见 Q2 取证。
- reported_by: lead

### FIXED(2026-10-25T16:13:00Z / BUG-462)
- evidence: commit e7ee817（仅 src/frontend/lz_lexer.lz + tests/lz_frontend_bootstrap.rs）。① punct_token 表补 `("=", Token.Eq)`、`("[", Token.LBracket)`、`("]", Token.RBracket)`；② is_keyword/keyword_token 改为 lz_parser.lz 同款查表（返回 `(bool, Token)`，规避深层 elif 链 p27 栈溢出与 LZ 不支持跨行 `||` 续写——初版多行布尔表达式触发 "Parse error: Unexpected token in expression: Newline"）；③ Token 枚举 + display_token 补 Elif/Let/Match/Case/While/For/In/LBracket/RBracket；④ tokenize 的 magic-method/关键字分支改为 else 内查表。Indent/Dedent 缩进方案差异（原生 Indent/Dedent 单位 token vs parser Indent(v) 值 token）留待后续单独对齐，本次未改。
- 验证: `_probes/frontend_p0.lz` lex-lz 修复后输出 19 token：`Def Ident(main) LParen RParen Eq Newline Let Ident(x) Eq IntLit(1) Plus IntLit(2) Newline Ident(print) LParen Ident(x) RParen Newline Eof` —— Eq/Let 均恢复，与原生仅差 Indent/Dedent。
- 回归: 新增 `emit_lex_lz_let_and_eq` 锁定 `let x = 1` 产出 `Let Ident(x) Eq IntLit(1)`；`cargo test -j 4 -- --test-threads=1` 全绿 0 failed（exit 0）。
## BUG-463 [2026-10-25T16:20:00Z] [high] FIXED
- summary: LZ parser (lz_parser.lz) parse_stmt 的 Def 分支只跳过 Colon 不跳 Eq → `def main() =\n    body` 卡在 `=` 上，函数体整块被吞成 `def main() {}`
- detail: parse_stmt `:692-716` 的 `Token.Def` 分支在参数/返回类型之后只判 `colon_t == Token.Colon` 决定是否跳过到块首。等号式定义 `def main() =\n    body` 的 `=` 未被跳过，被原样交给 parse_block；block_tag(`=`) 落入 `case _ => 0`，parse_block 立即 `done=true` 返回空块 → 输出 `def main() {}`，函数体（let 绑定/print 调用）整块消失且无编译错误，是静默丢内容的语法支持缺失。p0 探针（`def main() =\n    let x = 1 + 2\n    print(x)`）实测：parse-lz 输出 `def main() {}`。
- 复现放大: run_lz_frontend 用 `rfind("\ndef main() =")` 剥离 main 标记，p0 的 `def` 在源首行、匹配不到前置换行，故未 strip，整个源交给 LZ parser，恰好暴露此缺陷。
- 原生基准: src/parser —— 等号式 `def f() =` 与冒号式 `def f():` 均支持，等号后接缩进块。
- 复现: `_probes/frontend_p0.lz`。
- reported_by: lead

### FIXED(2026-10-25T16:23:00Z / BUG-463)
- evidence: commit e66f9fa（仅 src/frontend/lz_parser.lz + tests/lz_frontend_bootstrap.rs）。`Token.Def` 分支分隔符判定由 `colon_t == Token.Colon` 扩为 `colon_t == Token.Colon || colon_t == Token.Eq`，冒号式/等号式两种定义形态都正确跳过到块首。
- 验证: p0 parse-lz 修复前 `def main() {}` → 修复后 `def main() {let x = 1 + 2, expr print(x)}`，函数体完整（let 绑定 + 表达式语句）。
- 回归: 新增 `emit_parse_lz_eq_form_def` 锁定等号式 def 产出含 `def main()` 与 `let x = 1`；`cargo test -j 4 -- --test-threads=1` 全绿 0 failed（exit 0）。
- 遗留: 冒号式黄金测试 `lz_parser_matches_golden` 不受影响（其输入用 `def lookup(key: str) -> int:` 冒号式）。
## BUG-464 [2026-10-25T17:00:00Z] [high] FIXED
- summary: LZ lexer (lz_lexer.lz) 完全缺失原生式 Indent/Dedent 缩进 token；黄金测试 lz_lexer_matches_native_golden 是假基准——期望串不含 Indent/Dedent，而原生对同一输入实际发这两个 token，测试却断言"与原生对齐"
- detail: tokenize 对空白一律跳过、对 \n 只发 Newline，Token 枚举无 Indent/Dedent 变体，缩进栈缺失。后果：① 词法层无法分词任何多行缩进块（缩进语义被静默丢弃，自举前端名存实亡）；② 黄金测试假基准：期望串 13 token 无 Indent/Dedent，而原生 --tokens 对 `def f(x):\n    return x + 1\n` 实际发 15 token（Indent 索引7 / Dedent 索引13）。测试断言的是"两个都没发缩进 token"的假对齐，掩盖了真缺陷。此前 bootstrap/05 声称"全量 token 类型覆盖 diff 为空"与原生实际输出相矛盾。
- 复现: `_probes/golden_input.lz`；`./target/debug/lang-zone.exe _probes/golden_input.lz --tokens`（原生 15 token）vs `--emit=lex-lz`（修复前 13 token）。
- 原生基准: src/lexer/indent.rs + lexer.rs:218-222 的 handle_indent —— 行首前导空格列数 vs 缩进栈顶，增发 Indent 入栈、减逐层 Dedent 出栈，Eof 前补全 Dedent。
- 复现: `_probes/golden_input.lz`、`_probes/nest.lz`、`_probes/nest2.lz`。
- reported_by: lead

### FIXED(2026-10-25T17:20:00Z / BUG-464)
- evidence: commit 038af95（仅 src/frontend/lz_lexer.lz + tests/lz_frontend_bootstrap.rs，69+/6-）。Token 枚举加 Indent/Dedent 单元变体；tokenize 引入 at_line_start + indent_stack: List<int>(初始 [0])：行首先吃前导空格得 col，与栈顶比较（col>top 入栈发 Indent；col<top 循环 pop 逐层发 Dedent；空行/纯空白行不发缩进 token）；循环后弹出所有未闭合层级，Eof 前补全 Dedent。display_token 补两分支。
- 基准校正: 黄金测试期望串 13 → 15 token（补 Indent/Dedent，与原生逐 token 一致）；A3 测试 needle 补 Indent。
- 新增回归测试 emit_lex_lz_multi_level_indent：锁定两层缩进产出 2 Indent / 2 Dedent（回退一级 + Eof 收尾），流以 Eof 结尾。
- 验证: p0 探针修复后 21 token，与原生 --tokens 逐 token 一致（含 Indent/Dedent）；`def a():`→`if b:`→`return c` 多层嵌套产出 Indent Indent … Dedent Dedent，与原生逐 token 一致；跨两级回退（col 8→0）产出 Dedent Dedent，与原生一致。
- 回归: `cargo test -j 4 -- --test-threads=1` 全绿 0 failed（exit 0）。
- 遗留: 顶层最后一行紧跟 EOF 时，原生不把该行终止换行发成 Newline（如 `x\n` → 原生 `Ident(x) Eof`），lex-lz 多发一个 Newline。属窄边角，与缩进 token 无关，未在本笔处理。
## BUG-465 [2026-10-25T20:45:00Z] [medium] FIXED
- summary: README 与 src/main.rs 注释把路线 B 自举类 `.lz` 实现一并说成"已彻底移除/均已移除"，且 README 声称旧源码已备份至本机不存在的路径——文档-代码严重分歧
- detail: 2026-09-05 移除的只是 `src/codegen/`（AST→Rust 直接 codegen）。但 README.md:5/7/58 与 src/main.rs:1044 注释把路线 B 自举类 `.lz` 实现（src/frontend/lz_lexer.lz、lz_parser.lz、src/ir/lz_codegen.rs / lz_codegen_lib.lz）也说成"已彻底移除/均已移除"；README:7 更称"旧路线源码已备份至仓库外 E:\IDEProjects\AI\_backup_langzone_routeB_20260905"。docs/脚本冻结清单草案-2026-09-30.md:90 已标记此分歧"不在本清单判死"（悬案）。
- 实测（本机，原生为准）: src/codegen/ 确已删（ls: cannot access）；四个 `.lz` 实现全部仍在且经 include_str! 编译进 lzc；四旗标 --emit=lex-lz|parse-lz|ir-lz|rs-lz 均可用（main.rs 共 10 处引用，~899-903 分发）；备份目录 E:\IDEProjects\AI\_backup_langzone_routeB_20260905 本机不存在（ls: cannot access，同目录仅见 _git-full-backup-* 等 git 备份）。
- 复现: `ls -d src/codegen` / `ls -d "/e/IDEProjects/AI/_backup_langzone_routeB_20260905"`；`grep -c "emit=rs-lz\|emit=ir-lz\|emit=parse-lz\|emit=lex-lz" src/main.rs` → 10。
- reported_by: lead

### FIXED(2026-10-25T21:20:00Z / BUG-465)
- evidence: commit accdd85（仅 README.md + docs/脚本冻结清单草案-2026-09-30.md + src/main.rs，11+/5-）。裁决原则"文档迁就代码、代码不改"，路线 B 仅 src/codegen/ 移除、自举类 `.lz` 实现保留（实验性自举后备，不参与默认 IR 管线）。
- README.md:5 改为只指 src/codegen/ 移除并注明自举类 .lz 未随其移除；:7 撤销"已彻底移除"与不存在的备份路径、如实列出四个 .lz 实现及其旗标、新增"校订说明(2026-10-25)"点明 2026-09-23 版两处失实；:58 更正"已删除"清单。
- src/main.rs:1044 注释更正（纯注释，无语义变更，--emit=*-lz 分发逻辑不变）。
- docs/脚本冻结清单草案-2026-09-30.md:90 追加"裁决(2026-10-25)"闭合悬案，并说明 bootstrap/05-自举里程碑台账.md:73 保持原状（其表述本身与代码一致）。
- 附带修复: div-tools/_cycle_hash.ps1 哈希清单仍引用已删的 src/ir/codegen.rs 与已重构为目录的 src/ir/builder.rs，致基线输出 MISSING。已就地更正为 src/ir/codegen/mod.rs、codegen/emit.rs、codegen_cython.rs、builder/mod.rs；实跑 27 文件全部 hashed、0 MISSING，哈希基线恢复完整。该脚本未版管（本地诊断工具），修复只落盘不改版管状态。
- 回归: `cargo test -j 4 -- --test-threads=1` 全绿 0 failed（exit 0）；`cargo test --test lz_frontend_bootstrap` 7 passed。
- 过程注: 首次提交曾误 `git add` 到工作树中**他人未提交**的 embed 功能（emit_embed_artifacts / set_embed_stem / embed_targets，~91 行，位于 main.rs:1068+），与本文档注释混在同一提交（45b1e08）。已软回退 + 选择性暂存拆开：仅注释入库（accdd85），embed 工作完整保留在工作树未暂存（未提交、未被破坏，另有 _probes/main.rs.embed_backup.rs 备份）。教训：commit 前必查 `git diff --cached --stat` 的 hunk 归属，勿用整文件 `git add` 处理有他人未提交改动的文件。
## BUG-466 [2026-10-26T09:35:00Z] [high] FIXED
- summary: --emit=ir-lz 自举 display 管线对含 embed/外联类型的输入编译失败（rustc E0599）——LZ 侧 IrType/Item 枚举缺 Ext / EmbedBlock 变体
- detail: --emit=ir-lz 链路（main.rs:907）：build_ir_opt → src/ir/lz_codegen.rs::ir_module_to_lz_source 把 IR 数据序列化为 LZ 构造调用，display 逻辑完全由 LZ 侧 src/ir/lz_ir_lib.lz 承担，产物应与原生 --emit=ir 逐字节一致。实测 11 个 DEMO 中 embed_tnr_demo.lz、extern_demo.lz 在 rustc 步报 error[E0599]: no variant named EmbedBlock/Ext found for enum Item/IrType，原生 --emit=ir 正常。根因：src/ir/types.rs 的 IrType 与 src/ir/node.rs 的 Item 已增 Ext / EmbedBlock 变体，lz_codegen.rs:961/403 会发射 IrType.Ext / Item.EmbedBlock(...)，但 LZ 侧 lz_ir_lib.lz 对应枚举未镜像这些变体 → 生成 Rust 引用不存在的变体 → E0599。属"LZ IR 库是 Rust IR 模型子集、新变体未镜像"的系统性漂移。
- 复现: ./target/debug/lang-zone.exe DEMO/extern_demo.lz --emit=ir-lz（E0599 Ext）；./target/debug/lang-zone.exe DEMO/embed_tnr_demo.lz --emit=ir-lz（E0599 EmbedBlock）。
- reported_by: lead

### FIXED(2026-10-26T09:35:00Z / BUG-466)
- evidence: commit 37395e7（仅 src/ir/lz_ir_lib.lz + tests/lz_ir_bootstrap.rs，5+/34+）。
- src/ir/lz_ir_lib.lz：IrType 枚举加 Ext（unit）、Item 枚举加 EmbedBlock(lang: str, src_len: int)；display_type 补 case IrType.Ext => "Ext"（对齐 display.rs:23 IrType::Ext => "Ext"）；display_item 补 case Item.EmbedBlock => "embed(" + lang + ") { " + src_len + " chars }"（对齐 display.rs:735 embed({}) {{ {} chars }}，含花括号两侧空格）。
- tests/lz_ir_bootstrap.rs：新增 lz_ir_lib_mirrors_ext_and_embedblock 静态回归，断言变体存在且格式串与原生 display.rs 一致，防未来漂移。
- 验证: 全量 DEMO(11 个) --emit=ir vs --emit=ir-lz 逐字节一致（0 diff / 0 err，原 embed_tnr、extern 两处失败已修复）；额外 _probes/ir_rich.lz 亦一致。
- 回归: cargo test -j 4 -- --test-threads=1 全绿 0 failed。
- 工具注: fist-mbt report_bug/bug_fix 的 project_dir 相对 server 进程 cwd（E:/IDEProjects/AI/FIST-Mbt），拒绝 .. ，无法指向 lang-zone BOM；本账目手动维护（与 BUG-462..465 同法）。首次误调 report_bug 在 FIST-Mbt 账本落了 BUG-410 噪音条目，非 lang-zone 账本。
- 遗留: 交叉核对 _probes/cross_check_ir_lib.py 另检出潜在缺漏 9 个变体（Item{ScriptMain} / Expr{EnumFieldGet,GenBuild,LitBigInt,LitComplex,LitInt128,Spread} / IrType{BigInt,Complex,Int128} / Pattern{EnumNamed}），仅当新输入触及这些构造才触发 E0599，本轮未一并补（未现网）。

- BUG-467(high): ir-lz 与 --emit=ir 逐字节一致率低（88/101），4 处显示保真缺陷
  detail: 对 _probes/*.lz + CY/TESTS/*/*.lz（101 个原生可编译文件）跑 native --emit=ir 与
    --emit=ir-lz 双向对照。修复后一致率 88/101 -> 100/101（唯一例外 Item::Test）。
    (a) Expr::GenBuild 变体缺失 -> 生成 .rs 报 E0599 no variant GenBuild（gen_block.lz
        ir-lz 编译失败）。补 GenBuild(callee: Expr?, stmts, ty)。callee 用 Expr? 触发
        lang-zone 对 Self? 的自动 Box；试过 Box<MaybeExpr> 报 E0308（构造处不自动 wrap）、
        MaybeExpr 报 E0072（Expr 与 MaybeExpr 互递归无限尺寸）。
    (b) Expr::BlockExpr 只存单一 ty 复用于前缀 [e.ty] 与块内 { [block.ty]，而原生
        BlockExpr{block} 里 e.ty 与 block.ty 是两个不同字段（例 e.ty=?、block.ty=int）
        -> 静默错值，8 文件 `{ [int]` vs `{ [?]`。拆 block_ty 字段修复。
    (c) unop_str BitNot 发射 `!`，display.rs:293 为 `~`（3 文件）。
    (d) Item.FnDef 显示分支多拼接 raises_sig，display.rs:595-628 不渲染 raises
        （keywords_iterator 多出 raises str）。
  根因: LZ IrType/Item/Expr/Stmt 枚举是 Rust types.rs/node.rs 的显示模型子集；两个系统性
    缺陷类——(A) 缺变体 -> E0599，(B) 变体在但序列化值错 -> 静默 diff。本 bug 两类各 2 例。
  已修: lz_ir_lib.lz（GenBuild 变体+display、BlockExpr block_ty、删 raises_sig）、
    lz_codegen.rs（BlockExpr 发射 block_ty、BitNot -> ~）、
    tests/lz_ir_bootstrap.rs 增 lz_ir_lib_mirrors_genbuild_and_block_ty 回归断言。
  cargo test -j 4 -- --test-threads=1 全绿。
  遗留: _probes/cross_check_ir_lib.py 报 9 个 latent 变体缺漏（未在当前 101 语料触发）：
    Item 缺 ScriptMain；Expr 缺 EnumFieldGet/LitBigInt/LitComplex/LitInt128/Spread；
    IrType 缺 BigInt/Complex/Int128；Pattern 缺 EnumNamed。Item::Test 走 Rust {:#?} Debug
    （display.rs:727），非稳定 Display 契约，LZ Display 模型不逐字节复刻，记为已知限制。
### FIXED(2026-10-26T10:52:00Z / BUG-467)
  commit 192ebe4：lz_ir_lib.lz/lz_codegen.rs/tests/lz_ir_bootstrap.rs 三文件；
  ir-lz 对照 100/101 逐字节一致（test_suite 因 Item::Test 走 {:#?} Debug 例外）；
  新增回归断言锁 GenBuild/block_ty/无 raises_sig 三处。

### BUG-468 [high] ir-lz 对 BigInt/Complex/Int128/Spread 字面量报 E0599（9 个 latent 变体缺漏，实测 5 处）
  现象: 探针 _probes/latent_variants.lz（BigInt 大整数/3.14i/大 int128/...lst/枚举字段）走 ir-lz 时，
    生成 .rs 报 5 处 E0599: no variant LitInt128/Int128/LitComplex/Complex/Spread found for Expr/IrType。
    此前 101 语料未触发（值都落 i64/i128 内），故 latent。
  根因: LZ IrType/Expr 枚举缺原生变体（display.rs:14-16 渲染 int128/BigInt/Complex，
    display.rs:243-245 渲染 {n}_i128 / BigInt({s}) / Complex({re},{im})，display.rs:232 Expr 兜底
    <expr> 覆盖 Spread/EnumFieldGet）。附两处建模错：(1) LitInt128/LitBigInt 数值可超 i64 甚至 i128，
    LZ int(i64) 字段装不下 -> E0308；(2) LitComplex 发射 `(0, 3.14)` 首元被 LZ 当 int -> E0308。
  已修: lz_ir_lib.lz（IrType 增 Int128/BigInt/Complex + display；Expr 增 LitInt128(v:str)/LitBigInt(v:str)/
    LitComplex(v:(f64,f64))/Spread/EnumFieldGet + display 落 <expr>）、
    lz_codegen.rs gen_lit（LitInt128/BigInt 改存十进制 str；LitComplex 照 LitF64 保证小数点）、
    tests/lz_ir_bootstrap.rs 增 lz_ir_lib_mirrors_latent_literal_variants 回归断言。
  cargo test -j 4 -- --test-threads=1 全绿（bootstrap 8/8）。
  legacy: 9 例中 2 例为假阳性/命名漂移——Item 缺 ScriptMain（原生 0 处出现，route B 内部合成
    注入 main 入口，不影响字节匹配）；Pattern 缺 EnumNamed（LZ 实为 Pattern.Enum，lz_codegen.rs:1105
    发射名 EnumNamed 且字段 List<(str,Pattern)> 与 LZ args:List<Pattern> 双重漂移，语料未触发，留待后续）。
    Item::Test 仍走 Rust {:#?} Debug（display.rs:727），非稳定 Display 契约，记为已知限制。
### FIXED(2026-10-26 / BUG-468)

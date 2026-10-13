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

## BUG-20 [2026-10-02T02:48:10Z] [low] OPEN（rust 后端；cy 面本轮别名后已对）
- summary: `ref f = h.n` 字段位引用在 rust 后端编不过（E0308），cy 面写回正确
- summary: [规范/另一后端] `ref f = h.n` 然后 `f = 9` 应当写回字段（引用指向**位置**）。 cy 面本轮加别名后实测 9/9/9 正确；rust 面同一件 `error[E0308]: mismatched types` （terminal 观察，取证级，复跑同 BUG-19）。
- 处置: 不进语料（同 BUG-19 的 oracle 论证）。挂此卡，转正前置：rust 侧把字段位引用发成 `let f = &mut h.n;` 并在赋值位解引用，再连同 cy 面一起封盘。
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

## BUG-22 [2026-10-02T05:11:25Z] [medium] 入账未修（DEMO 语料文案，非产品码）
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

## BUG-28 [2026-10-06T09:13:36Z] [medium] 入账未修（rust 后端；本车道不改 src/ir/codegen/mod.rs）
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
- reported_by: qoder-explore-worker(cy 后端轮 T0r8)

## BUG-29 [2026-10-06T09:56:34Z] [medium] 入账未修（rust 后端；挡 cy 语料封 golden）
- summary: `CY/TESTS/01_basics/literals.lz` 在 rust 面 rustc 失败（`error[E0282]: type annotations needed`），cy 面同一份源码跑通 ⇒ 这一件的期望值**取不到 oracle**，L3 只能执行、不能判定
- detail: 逐字取自本轮封盘回执（TEMP/seal-missing-1741.log）：
  `未封 CY/TESTS/01_basics/literals.lz :: 不封：rust=RUSTC_FAIL error[E0282]: type annotations needed | error: aborting due to 1 previous error; 6 warnings emitted`
  口径：cy 语料的 `.exp` 只许取**另一后端**的实跑产物（BUG-21 同一条纪律），拿 cy 自己的 stdout 反灌＝把当期缺陷封成契约，所以 rust 编不过就等于这件没有覆盖。
  性质是「**挡路**」不是「cy 坏了」：cy 侧对字面量的处理已跑通，缺的是 rust 侧的类型推断。
  复跑：`python -u TEMP/xdiff_ds07.py CY/TESTS/01_basics`（不带 LZ_SEAL_EXP=1，只看差分）。
  转正前置：rust 面让该件产物可编译后，用 `python -u TEMP/seal_missing_exp.py` 补封（该工具**不会覆盖已存在的 .exp**）。
- reported_by: qoder-explore-worker(cy 后端轮 T0r8)

## BUG-30 [2026-10-06T09:56:34Z] [medium] 入账未修（rust 后端；自举语料在单文件取数面上编不过）
- summary: `CY/TESTS/99_bootstrap/lz_std.lz` 在本轮差分器的取数面（`lzc` 单文件转译 → `rustc --extern lz_builtins` → run）下 rustc 失败（`error[E0308]: mismatched types`），cy 面 OK ⇒ 该件同样封不了盘
- detail: 逐字取自 TEMP/seal-missing-1741.log：
  `未封 CY/TESTS/99_bootstrap/lz_std.lz :: 不封：rust=RUSTC_FAIL error[E0308]: mismatched types | error: aborting due to 1 previous error; 6 warnings emitted`
  **主张范围要收窄**：这只证明「单文件转译 + 那条 rustc 配方」跑不通 lz_std.lz，**不**等于「rust 后端跑不了 LZ 标准库」——自举面在仓库里另有体系（`tests/lz_ir_bootstrap.rs`），本轮没在那里取证，也不在那边下结论。
  对本车道的实际影响只有一个：L3 的 golden 覆盖清单里这一件与 BUG-24（named_args，rust 面 E0425）长期是缺口。
  复跑：`python -u TEMP/xdiff_ds07.py CY/TESTS/99_bootstrap`。
- reported_by: qoder-explore-worker(cy 后端轮 T0r8)

## BUG-31 [2026-10-06T09:56:34Z] [low] 入账未修（rust 后端 Set 打印每进程随机；不是 cy 缺陷，是「Set 类语料无法封 golden」的根因）
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

## BUG-36 [2026-10-08T03:05:58Z] [low] 入账未修（共享 IR；本车道不改 `src/ir/builder/ops_map.rs`）：`~x` 的位非在 `map_unop` 里被降级成逻辑非，IR 里 `~` 与 `not` 同形
- summary: `src/ir/builder/ops_map.rs` 里 `UnaryOp::BitNot => UnOpKind::Not, // 位非降级为逻辑非`（本轮实测该行仍在，卡里不写行号以免漂移）⇒ AST 层的 `~x` 与 `not x` 进了同一个 UnOpKind，下游只能靠操作数类型猜。
- detail: 规范 SYNTAX/12 §3.2 明写 `~x` → `__invert__`（Rust 无独立 BitNot trait，`__not__` 缺席时生成 `impl Not` 委托）。rust 面**恰好**不受影响：它发 `!`，对 bool 是逻辑非、对整数是位非、对用户类走生成的 `impl Not`，一个符号三种语义都对。cy 面必须分档（bool ⇒ `not `，其余 ⇒ `~`），本轮已在 `codegen_cython.rs` 的 UnOp 分支实现并锁住。
  **为什么这条仍要入账**：那是**补救不是修复**。任何第三个后端读同一份 IR 都会把 `~6` 当逻辑非；而 IR 里已经丢掉的信息，前端改一次就得在两个后端各猜一次。
  同族在册：移位两族不在 `magic_method_for_binop`（`__shl__`/`__shr__` 缺席），本轮同样只在 cy 侧点名直派，共享 IR 未动 ⇒ rust 面靠 `impl Shl/Shr` 恰好成立、cy 面靠类型闸门成立，两张皮都不是规范层的解。
  转正前置: builder 保留区分（新增 `UnOpKind::BitNot`，或给 Not 带一个 bit 标记；并把 `__shl__`/`__shr__` 补进 `magic_method_for_binop`），两侧按各自协议发射；改完 cy 侧这两条按类型还原的臂可以收敛成一条按 UnOpKind 分派的臂。改动落在共享 IR ⇒ **需要另一条车道**，本车道不改 `src/ir/builder/`、不改 `src/ir/codegen/`。
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

## BUG-38 [2026-10-08T04:58:08Z] [medium] 入账未修（rust 后端；本车道不改 `src/ir/codegen/mod.rs`、`src/ir/builder/`）：无注解闭包形参在 rust 面恒 E0282，cy 面却接受并跑出结果 ⇒ 接受面分叉、oracle 取不到
- summary: `SYNTAX/03e-closure闭包.md` §一 把 `|x| x + 1`（省略注解、省略 `=>`）列为合法字形，现存语料 `CY/TESTS/04_functions/closure_basic.lz` 也就这么写的；但**闭包先绑到变量再调用**（`let sz = |x| => x.len()`）时 rust 面恒 `error[E0282]: type annotations needed`，而 cy 面接受并跑出正常结果 ⇒ 同一份源码一面能跑、一面编不出
- detail: 逐字取自证据件 `TEMP/dynrecv-before-r12.log ＋ TEMP/dynrecv-after2-r12.log`：`d_len_obj`/`d_len_list`/`d_push_obj`/`d_push_list`/`d_contains_obj`/`d_empty_obj`/`d_iter_obj`/`d_strfam_dyn`/`d_shl_dyn`/`d_generic_len` 每一支的 rust 行都是 `error[E0282]: type annotations needed`。其中 `d_len_list`（cy stdout=`'3\n'`）与 `d_push_list`（cy stdout=`'1\n'`）是**内建容器**那一半：它们本来就跑对，说明这条不是「cy 也坏」而是**接受面**分叉。
  为什么归到 rust 侧：`IrType::Any` 是**共享 IR** 给闭包形参定的类型（`src/ir/builder/capture.rs:528`），rust 后端原样交给 rustc、rustc 拒绝推断；cy 侧的 `object` 什么都能装，于是只有 cy 面能跑。泛型那一支同型：`d_generic_len` 的 rust 侧原文 `error[E0599]: no method named `len` found for type parameter `T` in the current scope`（无约束的 `T` 上找方法），而带 duck 约束的 `d_duck_len` rust 面就过了——⇒ 「规范 §03e §一 的无注解写法」与「rust 面能否定标」之间没有判据在管。
  对本车道的实际影响：`CY/TESTS/15_feature_matrix/dyn_recv_closure.lz` **故意没有 `.exp`**，已登记在 `tests/cy_codegen_gate.rs` 的 `NO_ORACLE_EXPLAINED`（该表与盘面双向对表，补封后不移出会把门禁判红）；行为期望暂由 BUG-37 那四支单元锁的**发射形态**断言承担。
  与既往那条流程债同源（上一轮记的「`cy` 的接受面比 rust 宽（无 borrowck）」）：这一族缺陷都要靠**前端类型检查**才判得动，不在本车道。
- 转正前置: rust 面给无注解闭包形参定标（按调用点推断并单态化，或按规范回写「必须注解」并改 §一）后，用 `python -u TEMP/seal_missing_exp.py` 补封 `dyn_recv_closure.exp`（该工具不覆盖已存在的 `.exp`），并同轮把该条从 `NO_ORACLE_EXPLAINED` 移出。
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
## BUG-44 [2026-10-09T03:20:29Z] [medium] 入账未修（共享层：`with ... as` 绑定的类型没落地；本车道不改 `src/ir/builder/`）：`f"using {res.name}"` 在 rust 面落 `{:?}`、cy 面按规范落 Display ⇒ 两侧逐字不等
- summary: `struct MyResource = name: str` 之后 `with MyResource(name: "database") as res:`，语句体里 `print(f"using {res.name}")`。rust 面出 `using "database"`（带引号＝Debug 档），cy 面出 `using database`。两边都不报错、退出码都是 0，只是值不一样 ⇒ 比异常更难发现。
- detail: 逐字凭据（`TEMP/wdseal-r14-apply.log`，同一份源码两后端各跑一遍）：
  rust: 'acquiring database\nusing "database"\nreleasing database\n'
  cy  : 'acquiring database\nusing database\nreleasing database\n'
  不等行 1 / 总行 3 :: [('using "database"', 'using database')]
  **缩小范围的关键读数**：同一个 struct 里 `def __enter__(self)` 的 `print(f"acquiring {self.name}")` 两侧逐字一致（都是 `acquiring database`）⇒ 缺的**不是**「字段访问的类型落地」，而是「`with ... as` 那个**绑定**的类型落地」——绑定没定成 `MyResource`（多半落成 `Any`），于是 `res.name` 的类型未知，rust 侧的逐实参分档 （`src/ir/codegen/expr_gen.rs:76` 的 `print_arg_is_display`）按「未知 ⇒ `{:?}`」走。规范 `SYNTAX/00-词法基础.md:188`（str 在插值里走 Display）站在 cy 这侧，所以要修的是共享层的类型落地，不是 cy 的分档。同洞的另一朝向：`.to_string()` 的 IR 返回类型也不是 `str` ⇒ `print(n.to_string())` **两侧都**出带引号的 Debug（凭据 `TEMP/corp-z1-r14.log` 的 `z_tstr_ty.lz` 那格）——那条两侧同错、这条只错 rust 一侧。
- 本轮处置（不动共享层代码、不放宽任何门禁）：把这一句从语料 `with_defer.lz` 拆成独立语料 `CY/TESTS/15_feature_matrix/with_as_binding_fstr.lz`（写盘件 `TEMP/split_withdefer_r14.py`，它的红对照 `TEMP/split_ctl_r14.py`），母件余下 8 行按**当日** rust 面重封（`TEMP/wd_reseal_r14.py --apply`，老那份改名留盘 `with_defer.exp.presplitr14`）。拆的判据是硬的：老 golden 摘掉的 3 行必须与新语料 rust 输出的 3 行「摘引号后逐字相等」，余下行必须「只差引号」，且母件 cy 实测 == 当日 rust 逐字（三格红对照：少摘一行、多摘一行、oracle 改一个字符，全被拦下）。新语料**不封** `.exp`，登记在 `tests/cy_codegen_gate.rs` 的 `NO_ORACLE_EXPLAINED`（该表双向可红）。照 cy 封它违反「期望值来自另一后端」的封盘纪律（那句话就写在母件第 2 行注释里）；照 rust 封它等于把本卡描述的缺陷钉成 cy 的验收标准。
- 转正前置: 共享层把 `with ... as` 的绑定类型落成被管理值的类型之后，`f"using {res.name}"` 两侧应逐字一致（都是 `using database`）；届时 `python -u TEMP/seal_missing_exp.py` 会补封 `with_as_binding_fstr.exp`（该工具只在两后端逐字一致时封、不覆盖已有 `.exp`、空 stdout 不封），而 `NO_ORACLE_EXPLAINED` 那条随即被门禁的「登记了但 `.exp` 在场 ⇒ 红」反向门逼着同轮移出。顺手一起看 `print(n.to_string())` 那一族（`.to_string()` 的 IR 返回类型应为 `str`）。
- repro: `python -u TEMP/wd_reseal_r14.py`（DRY，含三格红对照，出两侧原文逐字）；`python -u TEMP/withdefer_diag_r14.py`（拆前母件的三方逐行分类：陈旧引号 10 行 / 真分叉 1 行）；`cargo test --test cy_codegen_gate`（L1，含登记表与语料的双向对表）
- reported_by: qoder-explore-worker(cy 后端轮 T0r14)

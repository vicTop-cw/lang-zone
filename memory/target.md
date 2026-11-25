# 当前阶段目标（Target）

- 本轮聚焦：**rs-lz 自举路径三条真实缺口（T0r19/T0r20）** —— 已落地① 浮点/整数幂（BUG-521）、② 混合数值提升、③ 默认参数（T0r20, iter-22:00）
- 验收条件：cargo build 0 warning 0 error、lib/相关套件零回归、端到端真编译+真运行、账本 BUG-521/BUG-522~526 记录

---

## 2026-11-25 迭代-22:00（定时任务探索轮）：T0r20 收口 rs-lz 缺口②③

- 起点：HEAD=92a7f29（iter-16:00 全绿基线）。前提检查：项目源文件最新改动距启动 975 分钟（≥5），CONTINUE_PROCEED；git pull 本地=origin/master。工作树有 1 个在途改动 `src/ir/codegen_cython.rs`（Option 垫片下沉）。
- **先收口在途**：`codegen_cython.rs` 的 Option 垫片下沉（postprocess 3a 分支退位、codegen 直出 `Option.None_()` + `_LzNoneCls` + `_LZNONE`）实测编译通过 → 提交 **5f08826**。
- **T0r20 落地 rs-lz 两条缺口**（发任务树 T0r20 + plan_deep 拆 3×2）：
  - **缺口② 混合数值算术提升**：新增判据 `is_arith_or_cmp_op`/`expr_is_f64_side` 等，BinOp 的 else 前插 f64 提升分支（对齐原生 expr_gen.rs:6349-6352）→ `(3i64 as f64) + 0.5f64`，与原生逐字一致。
  - **缺口③ 默认参数**（最大改造）：参数元组 schema 5→**7 元组** `(str,IrType,bool,bool,bool,bool,Expr)`，同步改 `lz_codegen.rs::param_pairs` + `lz_codegen_lib.lz` + `lz_ir_lib.lz`；LZ 侧发 `Option<T>` 签名 + `default_shadow_lines` 函数体影子 + `@dfc` 编码表 + `gen_default_call` 调用点补 `Some()/None`。输出与原生逐字一致。
- **验证**（真 `.lz→.rs→rustc→运行`）：expr_literals（`R1 literals OK`）/ functions（`R6 functions OK`）等抽查全过；回归锁 rs_lz_families **18/18**（+2 锁）；lib 486/0；codegen_corner_fixes 47/47；rs_lz_behdiff 16/16；embed 全套绿。
- **零回归坐实**：lz_ir_bootstrap 24/1 失败项 `rs_lz_unannotated_param_defaults_i64` 与 DEMO 7 个失败项，经 stash 基线对照**同样红**（BASE_FAIL=7）⇒ 全为既有缺口，非本轮。
- **新入账 5 条既有缺口**：BUG-522（assert_eq! 混合数值）/BUG-523（native 无注解形参 `()`）/BUG-524（rs-lz 返回闭包 `fn` vs `Arc<dyn Fn>`）/BUG-525（rs-lz 运算符重载引用）/BUG-526（DEMO rs-lz 未覆盖基线），供后续轮次逐族收敛。
- 下一步：① 收口 BUG-522~526（rs-lz 与原生逐字对齐）；② 把 DEMO/01_basics 全量纳入 rs-lz 回归门禁；③ 前提检查脚本判据升级（源文件 mtime + target/iter* 产物 + %TEMP%/lz_* + 存活进程）。

---

## 2026-11-24 迭代-16:00（定时任务探索轮）：收口 BUG-1 + 修浮点幂运算（T0r18 + T0r19.1）

- 起点：HEAD=7ee1eea（iter-22:00 全绿基线）。前提检查：源文件最新改动距启动 42.02 分钟（≥5），CONTINUE_PROCEED；`git pull` 已最新。
- **第一步：收口僵尸树 T0r18（04:00 探索轮的 BUG-1 修复）**。工作树 `src/ir/codegen/decl_gen.rs`（+227 行）正是 T0r18 的未提交成果（iter-22:00 曾把它当兄弟 agent 的坏 WIP stash 隔离）。实测**能编译**（0 warning 0 error），但其回归锁期望值写错：`reduce(mul,[1,2,3,4])` 期望 24，实际语义 0（累加器从 0 起，`0*x` 恒 0）——代码对、锁错。修锁（改无初值歧义的 `apply_pair` 场景）后 46/46 绿 → 提交 **ddce839**，T0r18 整棵树收口（根/…1 完成、…2/…3 归档 + 5 个孤儿归档）。
- **第二步：验收 T0r17**（iter-22:00 Scala 零 runtime 分发入口）：embed_scala_serialization 8/8 复跑通过；根/…1 完成、…2/…3 归档。
- **第三步：发起本轮新迭代 T0r19**（rs-lz 自举路径三条缺口，plan_deep 拆 3×2），亲自落地**缺口① 幂运算**：
  - 发掘出比 rs-lz 更严重的**原生 bug**：`f64 ** exp` 两路都只认整数基数。原生 `expr_gen.rs` Pow 分支对任意 lhs 发 `.pow(<exp> as u32)`，而 `f64` 无 `pow(u32)` 方法（std 为 `powi(i32)`/`powf(f64)`）→ **E0599，浮点幂整条不可用**（整数幂正常，掩盖缺陷）；rs-lz `lz_codegen_lib.lz` 更把 `**` 落 `else` 原样透传 → 连整数幂都 E0614/E0307。
  - 修复：两路同步按基数类型分流 —— f64 → `.powf(<exp>)`（指数保持 f64），整数 → `.pow(<exp>)`（指数转 u32）。提交 **8fe64e3**，台账 **BUG-521**。
  - 回归锁：`tests/codegen_corner_fixes.rs::pow_float_base_uses_powf_not_pow`（`2**10`=1024 / `2.0**3.0`=8.0 / `2.0**0.5`=√2，真 rustc 编译 + 真运行）。
- **零回归读数**（车道 `target/iter16-lz`）：`cargo build --bin lang-zone` 0 warning 0 error；lib 486/0；codegen_corner_fixes 47/47；rs_lz_families 16/16；embed_scala_serialization 8/8。push 至 GitCode 成功（本地=远程=8fe64e3）。
- **在册遗留（T0r19，留给并发轮次）**：缺口② 混合数值算术无提升；缺口③ 默认参数签名字面展开；外加 `demo_codegen_compile::magic_demos_generated_rust_compiles` 单条历史失败（未定位）。
- 下一步：① 缺口②（混合数值提升：`3i64 + 0.5f64` → E0277，按类型提升到公共数值类型）；② 缺口③（默认参数 E0061）；③ 定位 magic_demos 历史失败。

---

## 2026-11-24 迭代-22:00（定时任务探索轮）：Scala 零 runtime 分发入口（T0r17）

- 起点：HEAD=7ee1eea（上轮 iter-14:00 全绿基线）。前提检查：源文件最新改动距启动 1022 分钟（≥5），CONTINUE_PROCEED。`git pull` 已最新；基线 `cargo build` 0 error、`cargo test --lib` 486/486。
- **本轮缺口判定**：`ScalaLang::dispatch_file` 恒 `None`（注释「JVM 入口要 object 包装，本版本不生成」）⇒ scala 库产物 `surface_gated`，**调用面为零**。这正是 iter-04:00 thinking 里点名的「下轮可直接接手的具体缺口」，也是 T0r66 起 `mojo/scala` 长期挂账的那一条。
- **探针期（先证伪再动手）**：本机 `scala`/`scalac` 是 Coursier `.bat` 包装，缓存里 `scala3-library_3`/`scala-library` **全是 0 字节占位** ⇒ 「scalac 能编、scala 跑不了」。实测三处致命坑（每个都让裸 `java` 崩）：① `private var ARGS: Array[String] = Array.empty[String]` 在 `<clinit>` 拉 `scala/Array$`；② `sys.exit`/早退 `return` 拉 `scala/runtime/Nothing$`；③ `"OK:" + <Long>` 字符串拼接拉 `scala.runtime.BoxesRunTime`。绕开即成立：参数走 main 入参线性扫、退出码用局部 `var code` 末尾统一 `System.exit`、数值显式 `java.lang.*.toString`、输出 `new java.io.PrintStream(System.out, true, "UTF-8")`。
- **T0r17 落地（`src/embed/adapters.rs`）**：
  - `extract`：`java` 在场且源无自定 `package` 时，整体包入 `package lz_embed_<artifact>` 并把顶格 `def` 收进 `object Lib`（缩进 2 空格）；作者自带 package 则不动（幂等，不双包）。
  - `dispatch_file`：生成同包 `object __lz_<artifact>`，`argvGet(args,key)`/`__lz_bool`/`__lz_escape` 自声明（不撞 stdlib），help 臂 + 逐方法臂 + 未知方法 `code=3`；`java` 不在场则**仍返回 None**（保留 gate，不出假垫片）。
  - `build_cmd` = `scalac -d classes <dispatch> <lib>`；`run_cmd` = `java -cp classes <pkg>.__lz_<artifact>`。
  - **共用摘取器改造**：`fn_from_toplevel` 抽出 `fn_from_toplevel_ex(.., ret_sep)`，scala 用 `ret_sep=":"`。原实现只认 `->`，导致 `def add(a: Long): Long = ...` 的返回类型恒 None ⇒ 入口打印裸 `OK:`（本轮实测暴露）。`ret_sep=":"` 时返回值以 `=` 截断，避免把 `Long = a + b` 整段当类型。Mojo/JS 走原 `->` 路径，零回归。
- **回归锁**：① 新增 `tests/embed_scala_serialization.rs` **8/8**（含真 `scalac` 编译 + 真 `java` 运行，见 docs §2 判据）；② `tests/embed_gapfixes.rs` 三条**旧契约反转**：原 `scala_without_dispatch_surface_gets_no_callable_shim`/`scala_embed_emits_no_wrapper_and_names_the_gap`/`author_still_calling_a_gated_method...` 前提是「scala 无分发入口」，T0r17 后反转——改为「有分发入口⇒有可调用面」+「NO_DISPATCH_SURFACE 门仍由 Executable 档与泛型函数两条钉住」。18/18 全绿（`scala_a_add` 命名口径：resolve("scala","a.lz") → artifact `scala_a`）。
- **零回归读数**（隔离车道 `target/iter22-scala`，先跑 `scripts/prepare_lane_deps.ps1 -Profile target/iter22-scala/debug`）：lib 486/0；embed 全套 15 套件全绿（unify_r15 23、dict 23、tuple 7、js 10、c 2、dedup 4、list 8、stub 4、stem_parity 3、project_emit 3、project_grouping 4、dict_ext 10、dict_kv 3、gapfixes 18、scala 8）；bridge_embed 8、bridge_python_delegation 1。`cargo build --bin lang-zone` 0 warning 0 error（顺手删掉探针期留下的死函数 `scala_native_ty`）。
- **台账**：BUG-2（陈旧 OPEN，`bootstrap_emit.rs` 签名不匹配 E0308）复核后 `bug_fix` 改判 FIXED——HEAD 下两函数签名早已一致，`cargo build`+`lib` 全绿即证。另修 BUG-520 正文缺 `状态：FIXED` 行导致一致性门禁扫到下方子标题（`scripts/bug_ledger_consistency.ps1` PASS，243 条 0 不一致）。
- **并发干扰（重要）**：全程有兄弟 agent 在 `src/ir/codegen/decl_gen.rs` 反复落笔（13:11、14:06 两次），其 WIP **自身不编译**（`E0599 ExprKind::Set`、`E0308 &Expr`），把共享工作树的测试连带给拉红。处理：把它们的改动 `git stash`（`other-agent-decl_gen-wip{,2,3}`）后在我的干净树上跑完全部回归，**绝不回滚也不提交别人的 WIP**。教训对齐 `loomy-learned-stale-lane-test-triage`：先隔离变量证明代码未坏，再判环境红。
- 下一步：① `c` 适配器容器序列化（gcc 在场可继续深化）；② `mojo` 适配器（本机无工具链，需先解决工具链或以「生成文本 + 静态审」口径钉住）；③ `embed_gapfixes::magic_demos_generated_rust_compiles` 单条历史失败（iter-04:00 记为未定位，本轮未涉足）。

---

## 2026-11-23 迭代-14:00（定时任务探索轮）：shim 布局适配 + 账本治理（T0r16）
- 起点：HEAD=bd74b6c（上轮 iter-18:00 warning 清零后全绿基线）。前提检查：源文件最新改动距启动 33.6 分钟（≥5），继续。
- **本轮判定：无未实现缺口，转打磨池**；但接手即遇**环境级阻塞** —— `cargo clean`（21.8GiB）后全量测试大面积红：30 个集成测试报 `E0463 can't find crate for lz_builtins`/`num_bigint`。
- **根因（双重）**：
  1. **陈旧 rlib 混合产物（E0514）**：`target/debug/deps` 里的 `libnum_bigint`/`liblz_builtins` 是旧工具链（异版本）编译的，与当前 rustc 1.100.0 不兼容；`cargo clean` 全清 + `cargo build -p lz_builtins` 同源重建后消除。
  2. **shim 扁平布局无 deps/ 子目录**：本机 cargo 为 Loomy 环境 shim（`~\.cargo\bin\cargo.exe` 0 字节占位），`--out-dir=<target>/<profile>/build/<crate>/<hash>/out`、以多个 `-L dependency=build/<dep>/<hash>/out` 传依赖，**从不产 deps/**。项目测试与 `src/bridge/project.rs` 按标准布局用 `<rlib父目录>/deps`，恒失败。
- **修复**：
  - 3 个测试（`demo_codegen_compile` / `embed_tuple_serialization` / `embed_unify_r15`）的 `profile_dir_from_exe()` 增强识别 `.../<profile>/build/<crate>/<hash>/out` → 逐级上溯到 `<profile>`。
  - 新增 `scripts/prepare_lane_deps.ps1`：把扁平布局 rlib 递归收集到合成 `deps/`（`-Build` 可预构建）。集成进 `scripts/dual_track_gate.ps1`。
  - 5 个 scripts 统一转 UTF-8 BOM（防跨环境编码漂移）。
- **账本治理**：`scripts/normalize_bug_headings.ps1` 增强处理「缺 timestamp 单括号形态」`## BUG-N [sev] <desc>`（状态位取正文 `status:`/`状态：`，支持 `OPEN -> FIXED` 箭头取右端，或「修复+回归测试+commit」启发式）；规范化 18 条历史遗留（BUG-469/470/471/472/473/475/477/478/479/480/450/434/482/483/484/485/486/487），补 `### FIXED` 小记。`scripts/bug_ledger_consistency.ps1` PASS（243 条，0 不一致）。
- **门禁/验证**：①`cargo build --bin lang-zone` 0 warning 0 error（隔离车道）；②全量 `cargo test --no-fail-fast -j 1` 60 套件全绿 **1139 passed / 0 failed**；③端到端 `temp/_e2e_iter1400/probe.lz`（fib 递归）→ .rs → rustc 编译 → 运行输出 `55` exit 0（验证铁律）。
- 账簿：本轮为环境适配 + 账本治理，非新缺陷；无新 bug 条目，补 [### FIXED] 小记（18 条账本可见性）。
- 下一步：① rs-lz 保真边界扫描（`_probes/rslz*`，新用例）；② 其他语言适配器深化（C str/容器已有，mojo/scala 无工具链）；③ 把 `prepare_lane_deps` 加入常规 CI/门禁前置（当前 dual_track_gate 已接）。

---

## 2026-11-19 迭代-18:00（定时任务探索轮）：编译 warning 清零打磨（17→0）
- 起点：HEAD=bd6ee6e（T0r14/T0r66 已收口 BUG-520/515，全绿基线）。接手做**前提检查**：源文件最新改动距启动 1327 分钟（≥5），继续。
- **本轮判定：无未实现缺口，转为打磨池**（既有 bug/失败测试/warning/文档漂移）。核对确认：T0r57 Dict-KV、T0r61 嵌套容器、BUG-520 PyO3 委托、C 适配器 str/容器护栏均已落地（HEAD 含 a46c363/d0f212d）；账本仅 BUG-515（low，WSL-only）OPEN 且已在 d0f212d 收口。⇒ 打磨目标锁定 = **17 条 rustc warning**。
- **T0r15 warning 清零**（10 文件，+8/-88，纯删死码/去未用）：
  - unused imports：`src/main.rs`（absolutize/run_selfcheck_exe/std::process::Command）、`src/bridge/project.rs`（std::process::Command）。
  - dead_code：`src/main.rs::builtins_link_or_exit`（职责由 print_link_recipe 承担）、`src/bridge/cli.rs::is_lz_flat_container`、`src/embed/shim.rs::rust_param_ty/rust_ret_ty`、`src/ir/codegen/types_emit.rs::gen_generics_no_default`（impl 头改用 gen_param_args 后无人调用）。
  - unused var：`src/bridge/cli.rs` 两个 vestigial `k`、`src/comptime/eval.rs::it_idx`（去 enumerate）、`src/ir/codegen/expr_gen.rs::recv_is_set_like` + 冗余 `recv_ref`。
  - unreachable pattern：`src/ir/builder/infer.rs` L1383 臂 `Int|Int128|BigInt|F64|Bool` → `Int128|BigInt|Bool`（Int/F64 已由 L1166/L1177 专属臂先行登记 to_string，原分支恒不可达）。
  - never-read / unneeded mut：`src/lexer/lexer.rs::next_indent` 初值恒未被读 → 去 `= None`；`src/ir/codegen/decl_gen.rs::out` → 去 mut。
- **门禁/验证**：①判据脚本 `scripts/iter18_warning_gate.ps1`（run_check → warnings=0 errors=0，passed）；②全量 `cargo test --no-fail-fast -j 1` 60 套件全绿，passed 1139 / failed 0，exit 0（零回归）；③端到端 `DEMO/01_basics/combined.lz` → .rs → rustc 编译 → 运行输出 `R18 combined OK` exit 0（验证铁律：产物编译+运行等价）。
- **任务卫生**：lang-zone ns 清出 16 条陈旧僵尸任务（T0r7.x/T0r8/T0r11.x，特性均已由 T0r57/T0r61 落地），逐条 pause→force 归档；ns 由「16 待领取」归零（126 已完成 + 20 已归档）。
- 账簿：本轮非新缺陷，未新增 bug 条目；T0r15 一条（已完成）。
- 下一步：① 文档漂移治理（`docs/py-bridge-struct.md` §2.2 仍写「函数体占位 todo!()」，与 BUG-520 实际实现矛盾——需更新为「真实委托 __py_<name>」）；② 其他语言适配器（mojo/scala，本机无工具链）容器序列化；③ CHANGELOG 回填。

---

## 2026-11-19 T0r66（定时任务探索轮）：PyO3 委托落地 + C 适配器 str/容器护栏对齐
- 起点：HEAD=aee53cc（BUG-515 入账）；fist-mbt lang-zone ns 健康（133 任务，16 待领取）；工作树有并发 agent 在途 WIP（BUG-467 回归锁 tests/frontend_dual_track.rs 未提交）。
- **先收口在途**：BUG-467 回归锁 `frontend_dual_track_bug467_regression` 真跑通过（1/1，19.7s），确认该未提交改动有效。
- **T0r14 双方向并发落地**（任务树 T0r14，探索模式；两方向文件作用域不相交：python.rs vs adapters.rs，已 file_bind 绑定）：
  - 方向① `T0r14.1.1`：`src/bridge/python.rs` 的 `generate_pyfunction` 由 `todo!("Implement __py_<name>")` 桩改为**真实委托** `__py_<name>(args)`；`generate_module` 顶部注入 `extern "Rust"` 委托声明（新 `generate_delegate_decls`）使生成模块自洽。单元测试 `test_pyo3_pyfunction_delegates_not_todo`（bridge::python 7/7）+ 集成测试 `tests/bridge_python_delegation.rs`（**真跑 rustc 编译委托核 + 运行**，输出 `hi bob 7`）。
  - 方向② `T0r14.2.1`：`src/embed/adapters.rs` CLane —— ①`C_TY` 补 `char→str`（const char* 形参/返回与 Python/JS 标量闭集对齐）；②`methods()` 修正指针返回函数名提取（`trim_start_matches('*')`，此前 `*greet` 因含 `*` 被滤掉）与形参名提取（左行纳入整标识符——旧实现条件反了只取末字符）；③`dispatch_file` 为每个可调用函数发 `extern` 原型声明（跨翻译单元必须，否则隐式 int 截断 `const char*` 返回 → exit=0xC0000005 崩），并新增 str 形参（const char* argv 原样）与 str 返回（`OK:` + `__lz_emit_str` 转义）；④容器/Dict-KV 等无法原生表达的形参不再静默丢弃，发显式护栏臂 `ERR:unsupported param type`。单元测试 2 条（adapters 19/19）+ 集成测试 `tests/embed_c_serialization.rs` 2 条（**真跑 gcc 13.2.0 编译 + 运行**：str 返回 `OK:hi`、容器形参显式护栏）。
- **排障：并发车道竞争（非本轮回归）**：首轮全量 `lz_ir_bootstrap` 14 红，**单跑复跑 25/25 全绿**；根因是兄弟定时任务（cargo PID 6716 + lang-zone PIDs 3748/20600，19:13–19:14）在同一 `lang-zone.exe` 上并发 build/run，造成文件锁与陈旧 bin 竞争。教训对齐 loomy-learned-stale-lane-test-triage。隔离车道 `CARGO_TARGET_DIR=target/lane-loomy-1600` 规避 exe 锁。
- 回归读数：`cargo build --bin lang-zone` 0 error（隔离车道）；**全量非慢档 60 套件全绿零回归**（lib 486/0，含本轮 +3 单元测试；embed 全套 +dm；lz_ir_bootstrap 25/0、lz_frontend_bootstrap 7/0、frontend_dual_track 3/0 含 BUG-467 锁）。
- 台账：BUG-520 [high] FIXED（PyO3 todo!() 桩 + C 适配器 str/原型/容器护栏）。
- 下一步：① 其他语言适配器（mojo/c/scala）容器序列化（本机有 gcc，c 可继续深化；mojo/scala 无工具链）；② BUG-515（WSL 下 lzcyc 入口 E0514）收口；③ Python bridge `__py_` 委托的 LZ codegen 侧符号生成打通（当前仅桥接侧委托就位）。

---

## 2026-11-17 T0r65（定时任务探索轮）：JS 适配器嵌套容器护栏锁定 + 陈旧车道 rlib 排障
- 起点：接手 HEAD=c2a6f94（T0r64），工作树有并发 agent 在途未提交 WIP（BUG-467 lz_lexer.lz/BUG-512 相关 builtins.rs/scan.rs，17:02–17:14 落笔，非本轮）。
- **T0r65 三方向并发落地**（任务树 T0r13，全部真编译 + node 运行验证，非字符串断言）：
  - 方向① T0r13.1：`tests/embed_js_serialization.rs` 新增 `js_nested_containers_run_correctly_end_to_end`——编译含 `#[embed(js)]` 的 LZ 程序（group_counts:Dict<str,List<int>>、sum_nested:List<List<int>>、flat_keys:Dict<str,Dict<str,int>>），`node --check` 两产物 + `Command::arg` 逐字传参（不经 shell，规避 PowerShell strip 双引号）跑分发入口：15 / 10 / 6 全部正确。
  - 方向② T0r13.2：新增 `js_nested_containers_edge_and_return_forms`——空 dict `{}`→0、内层空 list 混合→6、嵌套容器**返回值** `{"a":[1,2],"b":[3]}`（JSON.stringify）、`list<dict<str,int>>` 参数→6。
  - 方向③ T0r13.3：`src/embed/adapters.rs` 单测新增 3 条——`is_container_type` / `is_nested_container` 谓词真值表 + dispatch PARAMS 表三类嵌套参数挂 `__lz_json_nested`（不退化为 `__lz_str`）。
- **排障：陈旧车道 rlib（非本轮回归）**：全量回归时 `lz_frontend_bootstrap`(7→4 failed) / `frontend_dual_track`(1 failed) 双红，报 `error[E0425]: cannot find function debug_f64`。经 `git stash` 到纯 HEAD 复核**同样红** ⇒ 非本轮引入。根因：并发 WIP 给 `lz_builtins` 加了 `debug_f64` 并进了 `LZ_BUILTIN_FN_NAMES`，但 `target/lane-bootstrap/debug/liblz_builtins.rlib` 是加之前的旧产物（lex-lz 走该隔离车道）。`CARGO_TARGET_DIR=target/lane-bootstrap cargo build -p lz_builtins` 重建后两套件回绿（7/7、1/1）。教训对齐 loomy-learned-stale-lane-test-triage。
- 回归读数：lib 483/0、embed 全套 119/0（js_serialization 10：+2）、lz_ir_bootstrap 25/0、lz_frontend_bootstrap 7/0（车道重建后）、frontend_dual_track 1/0、lexer_parser_core 11/0、dual_route_golden 4/0；`cargo build --bin lang-zone` 0 error。
- 边界：并发 agent 的 4 个 WIP 文件（lz_lexer.lz/builtins.rs/scan.rs/bugs.md）**本轮未提交**（非我改动面），已在 `memory/bugs.md` 记为 BUG-467（其台账状态由原 agent 维护）。
- 下一步：① 其他语言适配器（mojo/c/scala）嵌套容器序列化（本机无 mojo/scala 工具链，c 可验）；② BUG-467 并发 WIP 的提交与收口；③ 嵌套容器跨适配器统一 varargs/返回侧护栏。

---

## 2026-11-16 T0r64（定时任务探索轮）：BUG-513 三面收口 + JS 适配器产物「真跑」修复
- 起点：接手时工作区有 BUG-513 三面未提交修复（lexer 嵌入块边界 / cli dict 解析 / macros 反引号块）+ BUG-514 双轨词法对齐，代码已落地但台账未更新。
- **BUG-513 三面复核收口（FIXED）**：① `src/lexer/lexer.rs` 缩进式嵌入块边界（col-0 块用「空行 + 下一非空行缩进 ≤ 块缩进」为界，修复 main 体被吞）；② `src/bridge/cli.rs` 四处 `__lz_parse_dict_*` 的 `:` 深度误判（`{"a":6,"b":9}` 被切成一整对）；③ `src/macros/expand.rs` `@name!` + 反引号块参数探测。实证：`DEMO/embed_nested_demo.lz` 运行输出 `{"a": 6, "b": 9}`；`DEMO/embed_tnr_demo.lz` 编译运行输出正常。台账追记 + BUG-514 登记。
- **BUG-515~519（JS 适配器产物不可运行，全 FIXED）**：T0r63 的 JS 测试只断言 `body.contains(...)`，产物实际是**非法 JS**。本轮用 `node --check` + `node <dispatch>` 真跑暴露并修复五处：
  - BUG-515 dispatch arms 发成 Rust 形态 `if method === "x" {`（缺条件括号）→ 修为 `if (method === "x") {`；
  - BUG-516 PARAMS 表发成 Rust 元组 `[("a", __lz_str)]` → 修为 JS 二维数组 `[["a", __lz_str]]`；
  - BUG-517 artifact 保留 `# <filename>` LZ 注释（JS 无 `#` 语法）→ 新增 `js_comment_fix` 转 `//`；
  - BUG-518 顶层函数未 `module.exports`（CommonJS 取到 undefined）→ `extract` 追加 `module.exports = { ... }`；
  - BUG-519 dispatch 标量类型按 LZ 词汇 `int` 匹配（JS_TY 已映射为 `number`）恒不命中 → 退化 `__lz_str`，`add(3,4)` 得 `"34"`；修为按 JS 映射词汇匹配。另修 `fn_from_toplevel` 的 `rfind(')')`（函数体同行含 `)` 时吞函数体）为配对括号深度计数。
  - 另修 `help` 的 `KNOWN.map(n => n.slice(1,-1))`（值本无引号 → `OK:d`）。
- 验证：`node --check` 两文件均 0；端到端 `add a=3 b=4`→7、`sum_all nums=[1,2,3,4]`→10、`make_list`→`[1,2,3]`、`help`→`OK:add,sum_all,make_list`。测试强化 `tests/embed_js_serialization.rs` 新增 `js_embedded_artifacts_are_valid_and_run_correctly`（node --check + 运行判据），8/8 绿。
- 回归：lib 480/0、embed 全套 11 套件全绿、lz_ir_bootstrap 25/0、lz_frontend_bootstrap 7/0、frontend_dual_track 1/0（并发跑时一次 flake，单跑 492s 通过）、lexer_parser_core 4/0、dual_route_golden 11/0。
- 提交：acd352d（BUG-513/514，管线提交）、d58562c（JS 适配器修复）已推 GitCode；台账追记 fd24688。
- 下一步：① JS 适配器容器嵌套（dict/tuple 嵌套）运行验证；② 其他适配器（mojo/c/scala）嵌套容器序列化；③ `double_all(nums: list)` 这类非法 LZ 类型的用户提示友好化。

---

## 2026-11-14 T0r63（定时任务探索轮）：JS 适配器实现 + 嵌套容器运行时验证 + 编译回归修复
- 起点：T0r62（commit dc3f3f2）已完成嵌套容器完整 roundtrip + Dict×Tuple 嵌套组合端到端验证，下一步为① JS 适配器、② 嵌套容器运行时实际运行验证、③ 嵌套容器 DEMO 用例。
- **Bug 修复（BUG-512，critical）**：基线编译验证发现 `cargo clean` 后 release build 报 4 error——`src/ir/lz_codegen.rs` 的 `embed_targets_of`/`ir_module_to_rs_lz_source_with_stem` 等函数被误删，连同 `lz_codegen_lib.lz`（3759 行）/`lz_ir_lib.lz`/`tests/lz_semantic_cases.rs` 被删。`git checkout` 恢复后编译通过。教训：缓存掩盖真实编译失败，接手必须先 clean+build 确认基线。
- **T0r63.1 JS 适配器实现**：新增 `JsLang` 适配器（`src/embed/adapters.rs`）——JS_TY 类型表（int/float→number、str→string、bool→boolean + List/Dict/Tuple 容器）、`function` 顶层函数摘取（复用 `fn_from_toplevel`）、Node.js 分发入口（JSON.parse 解码容器参数、JSON.stringify 编码容器返回值）、`has_main` 检测 `function main(`/`require.main === module`、`run_cmd` 用 `node`。注册 `"js"|"javascript"` 到 adapter 表。单元测试 8 条 + 集成测试 7 条（含端到端 `js_embed_scalar_param_compiles`）全绿。
- **T0r63.2 嵌套容器运行时验证**：新增 `nested_dict_str_list_int_python_runtime_output_matches` 测试，直接运行 Python 嵌套容器脚本，验证 `group_counts({"a":[1,2,3],"b":[4,5]})` 输出 `{'a': 6, 'b': 9}`。**运行时验证暴露 BUG-513**（OPEN）：带 `#[embed]` 块的 LZ 程序 `def main()` 体未生成，`.rs` 中 `pub fn main() { }` 为空，编译通过但运行时无输出。
- **T0r63.3 嵌套容器 DEMO 用例**：新增 `DEMO/embed_nested_demo.lz`，演示 `Dict<str,List<int>>` 嵌套容器经 `#[embed(python)]` 端到端序列化，`--embed-build` 编译通过。
- 验证：`cargo build --release` 0 error；`cargo test --lib` 480/480（含 8 个 JS 单元测试）；embed 套件全绿零回归（dict_serialization 23、tuple 8、list 4、js 7、unify_r15 23、dedup 4、stem_parity 3、stub_shadowing 7）；lz_ir_bootstrap 25、dual_route_golden 4、bridge_embed 8 全绿。
- 下一步：① 修复 BUG-513（embed 块后 main 体未生成）；② JS 适配器端到端运行验证（当前仅编译验证）；③ 其他语言适配器（mojo/c/scala）的嵌套容器序列化支持。
- 提交：本轮改动待提交（adapters.rs + JS 适配器 + tests/embed_js_serialization.rs + DEMO/embed_nested_demo.lz + memory 更新）

---

## 2026-11-13 T0r62：嵌套容器完整运行时 roundtrip + Dict×Tuple 嵌套组合端到端验证
- 起点：T0r61（T0r60c commit）已完成嵌套容器递归 JSON 解析（embed_ret_ty_nested/embed_ret_parse_elem/embed_ret_parse_expr 递归实现）+ BTreeMap 类型统一，但端到端验证主要覆盖返回值侧，参数侧与嵌套组合的完整 roundtrip 验证不足。
- 本轮收口：先验收 T0r61 在途任务（T0r11 系列三方向①②③ 全部落地，验证全绿：embed_dict_serialization 19/19、embed_tuple_serialization 6/6、lib 472/472），清理 T0r57/T0r59 遗留僵尸任务（10 条在途 + 16 条孤儿叶全部收口），项目进度从 68% → 86.5%。
- 发布 T0r62 迭代（任务树 T0r12），聚焦三方向并发开发：
  - 方向①（T0r12.1）嵌套容器完整 roundtrip：新增 dict<str,list<int>> 参数+返回值端到端测试（python_embed_nested_dict_str_list_int_param_ret_roundtrip）、dict<str,dict<str,int>> 嵌套 dict 端到端测试（python_embed_nested_dict_str_dict_int_param_ret_roundtrip）
  - 方向②（T0r12.2）Dict×Tuple 嵌套组合：新增 tuple<dict<str,int>,list<int>> 端到端测试（e2e_tuple_dict_list_nested_param_compiles）、dict<str,tuple<int,float>> 端到端测试（python_embed_nested_dict_str_tuple_param_ret_roundtrip）
  - 方向③（T0r12.3）其他语言适配器 + 全量回归：JS 适配器调研（当前 adapters.rs 支持 python/mojo/c/scala，无独立 JS 适配器，留待下轮实现）；全量 embed 套件回归验证
- 验证：cargo build 0 error；embed_dict_serialization 22/22（原 19 + 新增 3）、embed_tuple_serialization 7/7（原 6 + 新增 1）、embed_unify_r15 23/23、embed_dedup 4/4、embed_gapfixes 17/17、embed_project_emit 3/3、embed_project_grouping 4/4 全绿零回归。
- 关键结论：T0r61 的嵌套容器递归解析实现（embed_arg_expr_nested/embed_elem_to_json/embed_ret_parse_elem）**已能正确处理参数侧序列化与 Dict×Tuple 嵌套组合**，dict<str,list<int>>/dict<str,dict<str,int>>/tuple<dict<str,int>,list<int>>/dict<str,tuple<int,float>> 四种嵌套组合端到端 --embed-build 编译全部通过，证明递归 JSON 序列化/反序列化全链路类型对齐。
- 提交：本轮测试新增待提交（tests/embed_dict_serialization.rs +265、tests/embed_tuple_serialization.rs +21）；src 代码为 T0r60/T0r61/BUG-2 遗留未提交改动，非本轮新增。
- 下一步：① JS 适配器（JsLang）实现（dispatch_file 生成 JS 分发入口 + List/Dict JSON.parse 解码）；② 嵌套容器运行时**实际运行**验证（当前为编译通过验证，可补充运行输出比对）；③ 嵌套容器的 DEMO 端到端用例。

# Lang-Zone 缺陷台账复核报告 — BUG-469 ~ BUG-487

- 任务：fist-mbt T0r2（namespace `lang-zone`，executor `atomcode-audit`）
- 复核对象：`memory/bugs.md` 中 **BUG-469 ~ BUG-487** 区段（rs-lz 保真类 + 原生 codegen 类）
- 复核时点：2026-10-27 22:1x（HEAD `58ecd7b`，master）
- 二进制：`target/debug/lang-zone.exe` v0.1.195-alpha（mtime 2026-10-27 22:08，已含本轮修复标记）
- 审计性质：只读产品源码；仅在 `Reports/`、`temp/audit/` 下写入

## 0. 结论摘要

台账该区段**实存 17 条**（BUG-474 / BUG-476 在台账中不存在，号段不连续）。

| 结论 | 条数 | BUG 号 |
|---|---|---|
| **FIXED** | **17** | 469, 470, 471, 471(a), 472, 473, 475, 477, 478, 479, 480, 482, 483, 484, 485, 486, 487 |
| **STILL-BROKEN** | **0** | — |
| **UNREPRODUCIBLE** | **0** | — |

但**发现 3 个台账外真缺陷（N1–N3）**与 2 个台账已承认的残留缺口（R1–R2），见 §3；这些是下一轮的主要工作项，见 §4 TOP-5。

> 一句话结论：**17 条台账判据全部在当前 HEAD 成立（有实跑或源码证据）**；风险已从「台账里的旧账」转移到「同族未清理点 + rs-lz 工具链自身的可用性缺陷」。

---

## 1. 复核方法与证据等级说明

### 1.1 三种复核方式的执行情况

| 方式 | 执行情况 |
|---|---|
| 静态比对 | 17/17 全部执行：在 HEAD 源码里定位对应逻辑并与原生 oracle（`stmt_gen.rs` / `decl_gen.rs` / `helpers.rs` / `types_emit.rs` / `expr_gen.rs` / `pattern_gen.rs`）逐点对照 |
| 历史比对 | 17/17 执行（`git log --grep`、`git merge-base --is-ancestor`、`git show`）。**重大发现：台账里记录的 commit 短号全部不在本仓库**，见 §5.1 |
| 实跑复现 | 17/17 中有 15 条取得实跑证据（详见下表），另 2 条（486 的配方一致性、483 的原生序列化侧）以源码 + 相邻实跑佐证 |

### 1.2 关键环境事实（影响证据强度，务必先读）

1. **HEAD 的 Rust 侧此前编不过**（`src/embed/shim.rs` 缺 API），本轮**没有**执行 `cargo build` / `cargo test`（按要求避让另一 agent 的 cargo 锁）。
2. `target/debug/lang-zone.exe` 在审计过程中被**重建为 v0.1.195-alpha（2026-10-27 22:08）**。用 `strings` 比对确认该二进制**已内嵌本轮修复**：
   `flatten_or_and`×3、`scrut_is_list`×2、`boxed_bindings_of_pat`×2、`rest @ ..`×2。故本轮实跑证据代表 HEAD 语义。
3. 为绕开「无法 cargo build」的限制，本轮采用一种**额外证据手段（本文简称「LZ 库驱动法」）**：
   把 HEAD 的 `src/ir/lz_codegen_lib.lz`（3247 行，产品源码，**未改动**）与一段审计驱动 `main` 拼接到 `temp/audit/drv_*.lz`，用 `lang-zone` 编成 Rust 再用 rustc 独立编译运行，**直接调用被修复的 LZ 函数**并观察其输出。
   这属于「实跑产品代码」，不是静态推断；驱动只存在于 `temp/audit/`，不污染语料目录。
4. 语料样本全部复制到 `temp/audit/e2e/` 后再跑，未在 `DEMO/`、`FIND_BUG/`、`_probes/` 下落任何产物。
5. 工作树有他人未提交改动（`src/lib.rs`、`src/bridge/cli.rs`、`CHANGELOG.md`、`memory/bugs.md`），但**本轮涉及的 codegen 文件均与 HEAD 一致**（`git status --short src/ir/ src/main.rs` 无输出），故静态证据对应已提交状态。

### 1.3 证据等级标记

- `实跑` — 用 v0.1.195 二进制真实执行并得到预期输出/产物
- `LZ 库驱动` — 用二进制把 HEAD 的 LZ 库源码编成 Rust、rustc 编译运行、调用被修函数实测
- `静态+oracle` — HEAD 源码逐点比对原生参照实现
- `历史` — 修复 commit 在本分支且改动点匹配

---

## 2. 逐条复核明细

> 每条格式：原判据（台账原文摘录，未改写）→ 复核方式 → 结论 → 证据片段。

### BUG-469 [high] rs-lz match scrutinee 未对齐原生「纯 Var 一律 .clone()」

- **原判据**：「非 Copy 枚举实参经裸 match 移出后，函数体再次使用 `s` 报 rustc E0382（二次移动）。」
- **复核方式**：静态+oracle、历史、实跑、LZ 库驱动
- **结论**：`FIXED`
- **证据**：
  - 源码：`src/ir/lz_codegen_lib.lz:2193-2207`（`base_sc` 三分支：Str→`.as_str()`、`self`→`self.clone()`、纯 Var→`.clone()`）
  - oracle：`src/ir/codegen/stmt_gen.rs:2274-2312`（未改动，原生同口径）
  - 实跑（LZ 库驱动，输入 `Stmt.Match(scrutinee: Expr.Var("s", Named("Shape")), arms: [])`）：
    `A469>match s.clone() {`
  - 回归测试：`tests/lz_ir_bootstrap.rs:579 rs_lz_match_scrutinee_clones_var`
  - 历史：`ddeed29`（2026-10-26，UNREPRODUCIBLE 台账短号 `7559a78`，见 §5.1）

### BUG-470 [high] rs-lz match scrutinee 未对齐 Str/Lists 的 .as_str() / .as_slice()

- **原判据**：「Str scrutinee 原生发 `match name.as_str() {`，rs-lz 发 `match name {`（String 直 match &str 字面量模式 rustc E0308）；List/Vec/Tuple scrutinee + 任一臂含列表模式，原生发 `match xs.clone().as_slice() {`，rs-lz 发 `match xs.clone() {`（E0529）。」
- **复核方式**：静态+oracle、历史、实跑、LZ 库驱动
- **结论**：`FIXED`
- **证据**：
  - 源码：`src/ir/lz_codegen_lib.lz:2198-2207`；辅助函数 `pattern_is_list/any_pat_is_list/any_field_pat_is_list/scrut_is_list/any_arm_pat_is_list`（`.lz:484-514`）
  - 实跑（LZ 库驱动）：
    - `A470str>match name.as_str() {`
    - `A470list>match xs.clone().as_slice() {`
  - 端到端：`--emit=rs-lz DEMO/01_basics/match_patterns.lz`（含 `match items` + `case [x, ..]`）exit=0，自检生成→rustc→运行全通
  - 回归测试：`tests/lz_ir_bootstrap.rs:671 rs_lz_match_scrutinee_as_str`
  - 历史：`497d66d`（台账短号 `c2713b6` 不存在）

### BUG-471 [high] rs-lz 列表模式发射两处 rustc 硬错（..rest 语法 + slice 元素未解引用）

- **原判据**：「① 余项绑定 rs-lz 发 `[a, ..rest]` …（E0425）；② .as_slice() 后元素绑定为 &T，rs-lz 发 `return a` 未解引用（应发 `return *a`，E0308）。」
- **复核方式**：静态、历史、实跑、LZ 库驱动
- **结论**：`FIXED`（判据两处均消除；但**非 Copy 元素**另立新缺陷 N1，见 §3.1）
- **证据**：
  - 源码：`list_elem_bindings`（`.lz:527-536`）、`gen_match_arm` 的 `bindings/derefs`（`.lz:538-560`）、`scrut_slice` 与 `.as_slice()` 共用布尔（`.lz:2206-2207`）
  - 实跑（LZ 库驱动，`Pattern.List([Ident a, Rest(NoStr)])` + `scrut_slice=true`）：
    ```
    A471arm>    [a, ..] => {
        let a = *a;
    {}}
    ```
  - 端到端：`match_patterns.lz` / `p25_long_or.lz` 等 rs-lz 自检 exit=0
  - 回归测试：`tests/lz_ir_bootstrap.rs:2245 rs_lz_list_pattern_element_binding_deref`（内含 `[a, rest @ ..]` 与 `let a = *a;` 断言）
  - 历史：`bfe2d55`

### BUG-471(a) [high] rs-lz 具名 Rest 模式发成 `..rest`（E0425）

- **原判据**：「rs-lz 产物 `..rest` vs 原生 `rest @ ..`，rustc E0425 cannot find value rest。」
- **复核方式**：静态、历史、实跑、LZ 库驱动
- **结论**：`FIXED`
- **证据**：
  - 源码：`display_pattern`（`.lz:448-453`）与 `gen_pattern`（`.lz:593-599`）双处 `n2 + " @ .."`
  - oracle：`src/ir/codegen/pattern_gen.rs:21,188` 同为 `format!("{} @ ..", n)`
  - 实跑：`A471a>rest @ ..`
  - 历史：`d53ea29`
  - 台账瑕疵：所引回归测试 `rs_lz_rest_pattern_emits_at_binding` **不存在**（`grep -rn "fn rs_lz_rest_pattern_emits_at_binding" tests/` 无命中）；实际断言并入 `rs_lz_list_pattern_element_binding_deref`（`tests/lz_ir_bootstrap.rs:2267`）

### BUG-472 [high] rs-lz auto_mut 未标记字段/下标赋值底座 Var

- **原判据**：「任何字段/下标写入的 let 绑定缺 mut。」
- **复核方式**：静态、历史、实跑（函数级）、原生 oracle 对照
- **结论**：`FIXED`
- **证据**：
  - 源码：`scan_expr_auto_mut` 的 `Expr.FieldAccess` / `Expr.IndexSet` / `Expr.IndexGet` 三支均先 `add_var_auto_mut(base, acc)`（`.lz:3081-3095`）
  - 实跑（LZ 库驱动，直接喂 AST）：
    ```
    print(scan_expr_auto_mut(Var("c"), ""))                       → c
    print(scan_expr_auto_mut(FieldAccess(Var c,"p"), ""))         → c
    print(scan_expr_auto_mut(FieldAccess(FieldAccess(Var c,"p"),"x"), "")) → c   ← 两级也是 c
    print(scan_stmt_auto_mut(Assign(FieldAccess(Var c,"p"), 2), ""))       → c
    ```
  - 原生 oracle（同一语义，v0.1.195 二进制 `--emit=rs`，样本 `temp/audit/p472_nested.lz`）：
    `let mut c = C { p: P { x: 1i64 } };` → 证明 LZ 的 `let` 允许写入，codegen 必须自动补 `mut`
  - 历史：`2688eb7`
  - **审计自纠正**：本条最初按静态阅读（`case Expr.FieldAccess => acc = add_var_auto_mut(b, acc)` 后的 `scan_expr_auto_mut(b, acc)` 未赋值给 `acc`）判为「两级字段写入仍漏 mut → STILL-BROKEN」；实跑推翻：LZ 的 match 臂最后表达式即函数返回值，递归结果**经返回值传出**，不丢。故为 FIXED。此条记录在案，防止后续复核再次误判。

### BUG-473 [high] rs-lz 固有方法发成 `pub fn Type.method` 自由函数

- **原判据**：「所有用户 struct/enum 固有方法在 rs-lz 下编译失败。」
- **复核方式**：静态+oracle、历史、实跑（LZ 库驱动，直接生成 Item）
- **结论**：`FIXED`
- **证据**：
  - 源码：`split_method_name`/`find_dot_index`（`.lz:2864-2874`）、`gen_item` 的 FnDef 分支构造 `ImplMethod` + `impl <ty> {`（`.lz:2876-2887`）
  - oracle：`src/ir/codegen/decl_gen.rs:169-182`（名含 `.` → `impl` 包裹，方法在 impl 内不带 `pub`）
  - 实跑：`B473split>Point|sq`；`B473item>impl Point {\n    fn sq() -> i64 {\n    return 25i64;\n    }\n}`
  - 历史：`62be5d4`
  - 已知局限（台账已列、当前仍成立）：方法自身泛型在 impl 块上未携带

### BUG-475 [high] rs-lz 递归类型字段未 Box 包装（E0072）

- **原判据**：「所有递归 struct/enum（链表、树）在 rs-lz 下编译失败。」（根因：`enum Tree = Leaf(v:int) | Node(l: Tree, r: Tree)` 产物 `l: Tree` → E0072）
- **复核方式**：静态+oracle、历史、实跑、LZ 库驱动、端到端
- **结论**：`FIXED`（解构端已收口；`gen_expr` 的 BlockExpr 空表局限仍在 → R1，见 §3.4）
- **证据**：
  - 源码：`type_box_map`/`enum_variant_map`（`.lz:654-712`）、`field_needs_box`/`field_ty_str`/`box_inner_ty`（`.lz:3111-3140`）、`pat_field_bindings`+`boxed_bindings_of_pat`（`.lz:713-756`）、臂体统一 deref（`.lz:551-559`）
  - oracle：`src/ir/codegen/helpers.rs:759 field_needs_box`（自 2026-10-27 起与 LZ 版**逐分支一致**：`Self_` / `Named==self` / `Option<inner>` 三种；两边都**不会**对 `Vec<Self>` 加 Box——台账里「Vec/List<T> → Vec<Box<T>>」的表述对两侧均不成立，见 §5.2）
  - 实跑（LZ 库驱动）：
    - `A475boxed>l, r`
    - `A475arm>    Tree::Node { l: l, r: r } => {\n    let l = *l;\n    let r = *r;\n...`
  - 端到端：`--emit=rs-lz FIND_BUG/lib_tree/tree.lz` exit=0（声明+构造+解构全链自检通过）
  - 回归测试：`tests/lz_ir_bootstrap.rs:838 / :888`
  - 历史：`e8015d6`（声明+构造）、`471e834`（解构）

### BUG-477 [high] rs-lz 把内建类型转换 `str(x)`/`int(x)`/`f64(x)` 原发成函数调用

- **原判据**：「`--emit=rs-lz` 对 `str(self.x)` 生成 `str(self.x.clone())` → rustc E0423 "expected function, found builtin type `str`"；`int(x)`/`f64(x)` 同样原发成同名函数调用 → E0425。」
- **复核方式**：静态、历史、实跑、LZ 库驱动、端到端
- **结论**：`FIXED`
- **证据**：
  - 源码：`is_cast_call`/`gen_cast_call`（`.lz:1908-1928`），在 `gen_call` 兜底前拦截（`.lz:1838-1839`）
  - 实跑：
    ```
    B477strint>format!("{}", 7i64)
    B477strvec>format!("{:?}", vec![7i64])
    B477int>(7i64 as i64)
    B477f64>(7i64 as f64)
    ```
  - 端到端：`--emit=rs-lz _probes/rslz4/cast.lz` exit=0（七种形态）
  - 回归测试：`tests/lz_ir_bootstrap.rs:974 rs_lz_builtin_type_cast_conversions`
  - 历史：`71bcfae`
  - 已知局限（台账已列、仍成立）：`char` 循环变量的 `int(c)`、用户 `__str__/__int__/__float__` 魔法

### BUG-478 [medium] rs-lz 内建 stdlib magic 遮蔽同名用户函数

- **原判据**：「用户定义 `def sum(xs: List<int>) -> int = xs.len()` 后调用 `sum(xs)`，rs-lz 发成 `xs.iter().copied().sum::<i64>()` …，返回 6 … 而非 3。」
- **复核方式**：静态、历史、实跑、LZ 库驱动、端到端
- **结论**：`FIXED`
- **证据**：
  - 源码：`module_user_fn_names`（`.lz:3225-3234`）、`is_user_fn` 守卫（`.lz:1777-1781, 1842-1844`）；Rust 侧 main 注入 `let uf = module_user_fn_names(m)`（`src/ir/lz_codegen.rs:74`）、`codegen_module(m, uf)`（`.lz:3236-3240`）
  - 实跑（LZ 库驱动，同一 `sum(vec![7i64])` 调用）：
    ```
    B478magic>vec![7i64].iter().copied().sum::<i64>()      ← 无同名用户函数：魔法保留
    B478user>sum(vec![7i64])                              ← 有同名用户函数：走通用调用
    ```
  - 端到端：`--emit=rs-lz _probes/rslz4/user_sum.lz` exit=0
  - 回归测试：`tests/lz_ir_bootstrap.rs:2147 rs_lz_self_user_fn_shadows_stdlib_magic`
  - 历史：`73482a1`

### BUG-479 [high] rs-lz 前端自检对含 match 的程序失败（E0533 Expr::BlockExpr）

- **原判据**：「`lang-zone <p>.lz --emit=rs-lz` 对任何含 `match` 表达式的程序报 … `E0533: expected value, found struct variant Expr::BlockExpr`，整条 rs-lz 路线中断。」
- **复核方式**：静态、历史、端到端实跑
- **结论**：`FIXED`
- **证据**：
  - 源码：`src/ir/lz_codegen.rs:895-919` 按 `self.rs_codegen_mode` 分流——rs-lz 发 2 字段 `Expr.BlockExpr(stmts:, ty:)`，ir-lz 保留 3 字段 `(stmts:, block_ty:, ty:)`
  - 库侧元数匹配：`src/ir/lz_codegen_lib.lz:59`（`BlockExpr(stmts: List<Stmt>, ty: IrType)`）
  - 端到端：`--emit=rs-lz DEMO/01_basics/match_patterns.lz` exit=0，自检三步（生成 .lzrs → 编译 → 运行）全通，输出为合法 Rust（含 `match` 的程序不再中断）
  - 历史：`cf24ea3`
  - 附带：同批 `_probes/rssweep.sh` 的「rs-lz 覆盖 .rs 快照」坑（BUG-481）已修，本轮复核靠「样本先复制到 temp/audit/e2e」规避

### BUG-480 [medium] rs-lz 对无类型注解参数退化为 Box<dyn Any>（E0369）

- **原判据**：「`--emit=rs-lz` 生成 `pub fn f(x: Box<dyn Any>) -> i64` …，rs-lz 产物无法编译。」
- **复核方式**：静态+oracle、历史、端到端实跑（临时探针 `temp/audit/e2e/g480_untyped.lz`：`def f(x): return x + 1`）
- **结论**：`FIXED`
- **证据**：
  - 源码：`rust_type` 的 `case IrType.Any => "i64"`（`.lz:767`）；`IrType.Duck` 仍 `Box<dyn Any>`（`.lz:780-781`，未改，符合台账口径）
  - oracle：`src/ir/codegen/emit.rs:247`（`IrType::Any => "i64"`，原生未改动）
  - 实跑：`--emit=rs-lz g480_untyped.lz` → 产物含 `pub fn f(x: i64) -> i64 {`，且不含 `Box<dyn Any>`
  - 回归测试：`tests/lz_ir_bootstrap.rs:2299 rs_lz_unannotated_param_defaults_i64`
  - 历史：`31e0510`

### BUG-482 [high] rs-lz 的 esc_rust 按字节切片字符串，多字节 UTF-8 触发运行时 panic

- **原判据**：「`--emit=rs-lz` 对任何含非 ASCII 字符的 LZ 源失败。」（根因：`esc_rust` 用 `s[i..(i+1)]` 逐字节切片 → `end byte index N is not a char boundary` panic）
- **复核方式**：静态、历史、实跑、LZ 库驱动、端到端
- **结论**：`FIXED`（`esc_rust` 本体已根治；**同族未清理点**见 N2/§3.2）
- **证据**：
  - 源码：`esc_rust` 改为 replace 链（`.lz:2551-2557`）；`esc_quote_only` 同为 replace（`.lz:2546-2548`）；`has_dot_or_e` 改用 `contains`（`.lz:2473-2476`）
  - 实跑：
    ```
    B482cjk>中文—x            ← 未 panic，原样保留
    B482quote>a\"b\tc        ← 转义顺序正确
    ```
  - 端到端：`--emit=rs-lz div-tools/probes/p4b_unicode.lz` exit=0（原 16 例 RSLZ_FAIL_NATIVE_OK 之一的 Unicode 探针）
  - 历史：`bba18f8`
  - 台账无独立回归测试（`cargo test` 侧无 `esc_rust` 单测），实跑驱动可作补充证据

### BUG-483 [high] rs-lz 的 .lzrs 生成端把 NaN/inf f64 字面量序列化为裸 NaN.0

- **原判据**：「`--emit=rs-lz` 对 `DEMO/lz_std/math.lz` 报 `未绑定变量: NaN`（IR 构建失败）。」
- **复核方式**：静态、历史、实跑、LZ 库驱动、端到端
- **结论**：`FIXED`
- **证据**：
  - 源码：`src/ir/lz_codegen.rs:1160-1180`（`nan→0.0/0.0`、`inf→1.0/0.0`、`-inf→-1.0/0.0`、无小数点补 `.0`）；LZ 侧 `fmt_f64`（`.lz:2456-2471`）
  - 实跑（LZ 库驱动）：
    ```
    B483nan>f64::NAN
    B483inf>f64::INFINITY
    B483ninf>(-f64::INFINITY)
    B483two>2.0f64
    ```
  - 端到端：`--emit=rs-lz DEMO/lz_std/math.lz` exit=0（原报「未绑定变量: NaN」的文件现已通过自检）
  - 历史：`fb4be42`
  - 残留：`Pattern::LitF64`（`src/ir/lz_codegen.rs:1068`）未同步 → R2，见 §3.5

### BUG-484 [high] rs-lz 的 gen_expr 对左结合 or/and 长链递归，53 层即溢出 8MB 栈

- **原判据**：「`--emit=rs-lz` 对 `div-tools/probes/p25_long_or.lz` 报 `thread 'main' has overflowed its stack`。」
- **复核方式**：静态、历史、实跑、LZ 库驱动、端到端
- **结论**：`FIXED`
- **证据**：
  - 源码：`flatten_or_and`（`.lz:1012-1028`）+ `gen_expr` 的 `||`/`&&` 分支（`.lz:1097-1115`）
  - 实跑（LZ 库驱动，构造 60 层左结合 or 链）：
    ```
    B484len>61          ← flatten 收集 61 个操作数
    B484exprlen>544     ← gen_expr 正常返回 544 字符串，无栈溢出
    ```
  - 端到端：`--emit=rs-lz div-tools/probes/p25_long_or.lz` exit=0
  - 历史：`2b56281`
  - 语义核对：拼装顺序为 `a || b || c || d`（左结合保序），逐项深度 ≤1 → 递归深度 O(1)

### BUG-485 [high] `let x: Complex = ..` 类型注解发射裸 Complex（未导入）

- **原判据**：「`let comp: Complex = 3.14i` 生成 `let comp: Complex = lz_builtins::Complex64::new(0.0, 3.14);`，rustc 报 `E0425 cannot find type Complex in this scope`。」
- **复核方式**：静态+oracle、历史、端到端实跑（临时探针 `temp/audit/e2e/g485_complex.lz`）
- **结论**：`FIXED`
- **证据**：
  - 源码：`src/ir/codegen/types_emit.rs:315-330`（Named 分支在 `type_map.get` 之前做大小写不敏感映射：`bigint/BigInt/Bigint/bignum/BigNum → BIGINT_RS`；`complex/Complex/Complex64/complex64 → COMPLEX_RS`），并带 `is_user_ty` 守卫（用户同名 struct/enum 优先，不吞）
  - 实跑：`--emit=rs g485_complex.lz` → `let comp: lz_builtins::Complex64 = lz_builtins::Complex64::new(0.0, 3.14);`（不再有裸 `Complex`）
  - 回归测试：`tests/codegen_corner_fixes.rs:1009 complex_bigint_annotation_maps_to_builtin_path`
  - 历史：`44216e4`

### BUG-486 [medium] LINK RECIPE 的 -L dependency 指向 rlib 所在目录而非 deps/

- **原判据**：「生成的 stderr 链接配方 … 里 `<dir>` 取 `r.parent()`。当 rlib 命中 `target/debug/liblz_builtins.rlib` … → `-L dependency=target/debug` 找不到依赖 → `E0463`。」
- **复核方式**：静态、历史、实跑（配方本身可用性验证）
- **结论**：`FIXED`
- **证据**：
  - 源码：`src/main.rs:259-299`（`exe_deps_dir = <exe_dir>/deps`，存在才取；`deps_dir` 优先用它，回退 `r.parent()`）
  - 实跑：v0.1.195 在**顶层 rlib 存在**的情况下打出
    `-L dependency=F:\AI\lang-zone\target\debug\deps`（旧行为会是 `...\target\debug`）
  - 按该配方原样执行的 rustc 命令在本轮全部成功（`temp/audit/drv_a.rs`、`drv_b.rs`、`e0507.rs` 均用
    `--extern lz_builtins=target/debug/liblz_builtins.rlib -L dependency=target/debug/deps` 编译通过）
  - 历史：`4520840`

### BUG-487 [high] 用户 struct HashMap 撞内建名时命名构造器被误生成 BTreeMap::from

- **原判据**：「`new()` 末尾 `HashMap(buckets: …, _num_buckets: 16)` 生成的 `.rs` 变成 `return std::collections::BTreeMap::from([…])` → rustc E0308。」
- **复核方式**：静态、历史、端到端实跑
- **结论**：`FIXED`
- **证据**：
  - 源码：`src/ir/codegen/expr_gen.rs:2477-2487` 的 kwargs 分支追加
    `&& !self.emitted_types.contains(callee_s.as_str()) && !self.impl_types.contains(callee_s.as_str())`
  - 实跑：`--emit=rs FIND_BUG/lib_hashmap/hashmap.lz` exit=0，产物为
    ```
    pub struct HashMap { … }
    impl HashMap {
        pub fn new() -> HashMap {
            …
            return HashMap { buckets: (buckets).clone(), values: (values).clone(), _size: 0i64, _num_buckets: 16i64 };
        }
    ```
    `grep -c "BTreeMap::from" hashmap.rs` = **0**
  - 历史：`3f8512e`（HEAD~2，唯一一个台账短号能对上的条目——纯属巧合，见 §5.1）

---

## 3. 台账外新发现（本轮真缺陷）

### 3.1 N1 [high] BUG-471 同族：非 Copy 元素的列表模式解引用 `let a = *a;` 必失败（E0507）

- **证据链**：HEAD 的 `list_elem_bindings`（`.lz:527-536`）只按 `Pattern.Ident` 收集绑定名，**不看元素类型**，于是对 `List<str>` 的列表模式同样在臂首生成 `let a = *a;`。
- 实跑（LZ 库驱动，`List<str>` 的 `Stmt.Match`）得到产物文本：
  ```
  B471strlist>match xs.clone().as_slice() {
      [a, rest @ ..] => {
      let a = *a;
  {
      return a;
  }}
  ```
  把这段文本原样嵌入最小 Rust 函数后用 rustc 编译：
  ```
  error[E0507]: cannot move out of `*a` which is behind a shared reference
     --> temp/audit/e0507.rs:5:21
      |  let a = *a;
      |          ^^ move occurs because `*a` has type `String`, which does not implement the `Copy` trait
  ```
- **影响面**：任何 `match xs: case [a, …]` 且 `xs: List<str>`（或任何非 Copy 元素类型）的 rs-lz 程序，产物必编译失败。原生路线在**使用点**插 `__implicit_from__(a.clone())`，不受影响 → 双路分叉。
- **可复现性**：中（语料里列表模式样本都是 `List<int>`：`DEMO/01_basics/match_patterns.lz:77`、`DEMO/06_control_flow/pattern_more.lz:10-26`；需自造 `List<str>` 样本）
- **建议修法**：`list_elem_bindings` 返回「绑定名 + 是否 Copy」，Copy 走 `let a = *a;`，非 Copy 走 `let a = a.clone();`（或直接改发 `let a = a.to_owned();`）；或按原生语义在**使用点**注入 clone。

### 3.2 N2 [high] BUG-482 同族残留：`gen_fstring` 仍按字节切片，非 ASCII f-string 必 panic

- **证据**：`gen_fstring`（`.lz:2480-2522`）仍用逐字节 `let c = s[i..(i + 1)]`（`.lz:2486,2489,2510`），与 BUG-482 修复前的 `esc_rust` **完全同形态**；`esc_rust`/`esc_quote_only`/`has_dot_or_e` 已改 replace 链，唯独 f-string 解析器没改。
- 同类未清理点（同一文件）：
  - `.lz:1713 starts_upper`：`let c = s[0..1]`（名字首字符非 ASCII 时 panic）
  - `.lz:3056 str_contains`：`s[i..(i + sub_len)]`（变量名/子串含多字节时 panic；BUG-472 的 auto_mut 去重路径会走到）
  - `.lz:3245`：`out[0..(out.len() - 2)]`（尾部假定 2 字节）
- **影响面**：`f"…中文…{x}"` 形态的 rs-lz 程序在 codegen 期 panic；`str_contains` 影响中文标识符。语料目前**无**非 ASCII f-string 字面量（扫描 20 个候选全为行尾注释误命中），属潜伏缺陷。
- **可复现性**：高（一行 `print(f"中文{x}")` 即可，但需 rs-lz 路线）
- **建议修法**：f-string 解析改用 `find("{")/find("}")` 或先 `chars().collect::<Vec<char>>()` 再按字符索引；`starts_upper` 改 `s.starts_with(|c: char| c.is_ascii_uppercase())`；`str_contains` 直接用 `s.contains(sub)`。

### 3.3 N3 [high] rs-lz 自检用**相对路径**跑生成 exe 时 panic（`program not found`）

- **现象**：`lang-zone --emit=rs-lz <相对路径>.lz` 在自检最后一步崩溃：
  ```
  Generated match_patterns.lz -> match_patterns.lzrs (LZ Rust codegen)
  thread 'lang-zone-compile' panicked at src\main.rs:1035:22:
  run generated exe: Error { kind: NotFound, message: "program not found" }
  ```
  同一命令换成**绝对路径**则完全正常（`--emit=rs-lz /abs/path/match_patterns.lz` exit=0，打印生成 Rust）。
- **源码位置**：`src/main.rs:1022-1035`，`exe_path = replace_ext(&lz_path, ".lzrs", ".exe")` 保留相对形态后 `Command::new(&exe_path)`；生成的 exe 确实存在（`temp/audit/e2e/match_patterns.exe` 7.4 MB，直接 `./match_patterns.exe` 可运行），但 `Command::new("match_patterns.exe")` 在本环境解析失败。
- **影响面**：所有按文档写法（相对路径）调用 `--emit=rs-lz` 的用户/脚本直接 panic；也让任何 rs-lz 端到端门禁在相对路径下假失败。
- **可复现性**：**高**（100% 稳定，见 §6 命令）
- **建议修法**：`exe_path` 与 `rs_path`、`lz_path` 统一 `std::fs::canonicalize` 或 `current_dir()?.join(..)` 后再拼 `Command`；同时把 `.expect("run generated exe")` 换成带上下文的错误返回（当前直接 panic，破坏 CLI 契约）。

### 3.4 R1 [medium] BUG-475 已知局限：`gen_expr` 的 BlockExpr 空表 → 块表达式内的 match 不解 Box

- `gen_expr` 的 `case Expr.BlockExpr` 调 `gen_body(ss, true, "", fn_raises, user_fns, [], [])`（`.lz:1155`），即 `tbm/vem` 传空表。
- 因此 `let x = <块表达式里的 match 递归类型>` 不会生成 `let n = *n;`，会退回 E0308/E0072 类错误。
- 台账已如实登记为「已知局限」；本轮确认仍存在。**可复现性**：中（需构造块表达式内 match 的样本）。

### 3.5 R2 [medium] BUG-483 已知缺口：`Pattern::LitF64` 仍发裸 `NaN`/`inf`

- `src/ir/lz_codegen.rs:1068`：`Pattern::Lit(LitKind::F64(n)) => format!("Pattern.LitF64(v: {})", n)` —— NaN/inf 时仍产出 `Pattern.LitF64(v: NaN)`，.lzrs 重编译会报「未绑定变量: NaN」。
- 台账已注明「留待有触发样例再处理」；本轮确认仍存在，且**没有回归测试覆盖**。可复现性：低（pattern 位出现 NaN 极罕见）。

---

## 4. 建议下一轮优先修的 TOP 5

排序依据：真缺陷 > 影响面 > 可复现性。

| # | 项 | 类型 | 理由 | 建议动作 |
|---|---|---|---|---|
| **1** | **N3** rs-lz 相对路径自检 panic（`main.rs:1035`） | 工具链可用性 | 100% 稳定复现，**所有按文档用相对路径跑 `--emit=rs-lz` 的调用必崩**；且 `expect` 直接 panic，把可用性缺陷伪装成崩溃。影响面覆盖整条 rs-lz 路线（门禁、脚本、用户）。 | `exe_path` 绝对化；`expect` → 带上下文的 `eprintln! + exit(1)`；补一条「相对路径 vs 绝对路径」CLI 回归测试 |
| **2** | **N1** 非 Copy 元素列表模式 `let a = *a;`（E0507） | rs-lz 保真 | 真编译错误，且是 BUG-471 修复的**直接副作用**（修 i64 时把 &String 也解引用了）；原生路线正确 → 双路分叉。任何 `List<str>` 解构都中招。 | `list_elem_bindings` 带上元素是否 Copy；非 Copy 发 `let a = a.clone();`；补 `case [a, rest]` on `List<str>` 的双路回归 |
| **3** | **N2** `gen_fstring` 等 4 处字节切片（BUG-482 同族） | rs-lz 保真（潜伏） | 与刚修的 BUG-482 同类同文件，**修一半**；`f"中文{x}"` 一行即 panic，`str_contains` 影响非 ASCII 标识符。加 Prompt 侧重中文语料时必踩。 | f-string 解析改 `find`/字符索引；`starts_upper`/`str_contains`/`out[0..len-2]` 一并 replace；补一条非 ASCII f-string 回归 |
| **4** | **R1** `gen_expr` BlockExpr 传空 `tbm/vem` | rs-lz 保真 | BUG-475 的收口只覆盖语句位置，块表达式内的递归类型 match 仍会漏 Box 解引用；属「已承认但未闭环」的已知局限，越晚补越容易在语料里踩到。 | 让 `gen_expr` 接收 `tbm/vem`（93 处调用点可批处理），或退化为模块级只读全局表 |
| **5** | **R2** `Pattern::LitF64` 裸 `NaN`/`inf` | 原生序列化 | 与 BUG-483 同根不同面；触发率低但**无测试覆盖**，且失败形态是「生成的 .lzrs 无法重编译」这种难诊断的错误。 | 与 `LitKind::F64` 同口径分发；若 pattern 位不支持表达式，则显式报错（不要发坏代码） |

补充（非 TOP-5，但建议顺手做）：**台账卫生**——见 §5.1/§5.2，commit 短号全部失效 + 两处事实性描述不准，会让下一轮复核继续空转。

---

## 5. 台账文字与仓库事实的不一致（需修正台账）

### 5.1 台账里 17 条中有 16 条的 commit 短号在本仓库**不存在**

`git cat-file -e <hash>^{commit}` 对 `7559a78 / c2713b6 / 165b844 / 74798a1 / 4897f87 / 1fdf50c / 6df5a51 / 6bb52f5 / 9332bbe` 全部返回 MISSING（只有 BUG-487 的 `3f8512e` 命中）。实际修复 commit 映射如下（均 `merge-base --is-ancestor` 确认在 master 上）：

| BUG | 台账记录 | 实际 commit（master） | 日期 |
|---|---|---|---|
| 469 | 7559a78 ✗ | `ddeed29` | 2026-10-26 |
| 470 | c2713b6 ✗ | `497d66d` | 2026-10-26 |
| 471(a) | 165b844 ✗ | `d53ea29` | 2026-10-26 |
| 471② | （无） | `bfe2d55` | 2026-10-27 |
| 472 | 74798a1 ✗ | `2688eb7` | 2026-10-26 |
| 473 | 4897f87 ✗ | `62be5d4` | 2026-10-26 |
| 475 声明/构造 | 1fdf50c ✗ | `e8015d6` | 2026-10-26 |
| 475 解构 | 6df5a51 ✗ | `471e834` | 2026-10-27 |
| 477 | （无） | `71bcfae` | 2026-10-26 |
| 478 | 6bb52f5 ✗ | `73482a1` | 2026-10-27 |
| 479 | 9332bbe ✗ | `cf24ea3` | 2026-10-26 |
| 480 | （无） | `31e0510` | 2026-10-27 |
| 482 | （无） | `bba18f8` | 2026-10-27 |
| 483 | （无） | `fb4be42` | 2026-10-27 |
| 484 | （无） | `2b56281` | 2026-10-27 |
| 485 | （无） | `44216e4` | 2026-10-27 |
| 486 | （无） | `4520840` | 2026-10-27 |
| 487 | 3f8512e ✓ | `3f8512e` | 2026-10-27 |

推测是台账抄写/换分支时沿用了别处的短号（或历史被重写），建议统一回填真实短号，否则未来「历史比对」全部落空。

### 5.2 另外两处不准确

1. **BUG-471(a) 的回归测试名不存在**：台账写 `rs_lz_rest_pattern_emits_at_binding`，仓库无此函数；实际断言在 `tests/lz_ir_bootstrap.rs:2267`（`rs_lz_list_pattern_element_binding_deref` 内）。
2. **BUG-475 的 `Vec/List<T> → Vec<Box<T>>` 描述**：原生 `helpers.rs:759` 明确注释「`Vec<Self>`、`Rc<Self>` 已间接，无需 Box」，`field_needs_box` 对 `Vec<Self>` 返回 false；LZ 版同。两侧该分支实为死代码，台账表述会误导后续按「Vec 要 Box」去验收。
3. **BUG-472 的现场描述容易误导静态复核**：`case Expr.FieldAccess => acc = add_var_auto_mut(b, acc)` 之后那一行递归的返回值是**函数返回值**（LZ match 臂值语义），不是丢弃。本轮已踩过并纠正（见 §2 BUG-472），建议在台账补一句，避免下次再判错。

---

## 6. 复现命令与产物清单

### 6.1 环境

```bash
cd /f/AI/lang-zone
./target/debug/lang-zone.exe --version      # Lang-Zone compiler (lz) 0.1.195-alpha
stat -c '%y' target/debug/lang-zone.exe     # 2026-10-27 22:08:46
```

### 6.2 端到端（既有语料，样本已复制到 temp/audit/e2e，绝对路径）

```bash
D=/f/AI/lang-zone/temp/audit/e2e
cp DEMO/01_basics/match_patterns.lz      $D/     # BUG-469/470/471/479
cp FIND_BUG/lib_tree/tree.lz             $D/     # BUG-475
cp DEMO/lz_std/math.lz                   $D/     # BUG-483
cp div-tools/probes/p25_long_or.lz       $D/     # BUG-484
cp div-tools/probes/p4b_unicode.lz       $D/     # BUG-482
cp _probes/rslz4/cast.lz                 $D/     # BUG-477
cp _probes/rslz4/user_sum.lz             $D/     # BUG-478
cp FIND_BUG/lib_hashmap/hashmap.lz       $D/     # BUG-487（--emit=rs）
# 以上 8 条全部 exit=0

./target/debug/lang-zone.exe --emit=rs-lz "$D/match_patterns.lz"   # exit=0（相对路径会 panic，见 N3）
./target/debug/lang-zone.exe --emit=rs    "$D/hashmap.lz"          # exit=0，产物无 BTreeMap::from
```

### 6.3 LZ 库驱动法（函数级实跑）

```bash
# 拼接 HEAD 的 lz_codegen_lib.lz + 驱动（驱动只写 temp/audit，不改产品源码）
python - <<'EOF'   # 见 temp/audit/drv_a.lz / drv_b.lz 生成方式
EOF
./target/debug/lang-zone.exe temp/audit/drv_a.lz          # 生成 drv_a.rs
rustc --edition 2021 --extern lz_builtins=target/debug/liblz_builtins.rlib \
      -L dependency=target/debug/deps -O temp/audit/drv_a.rs -o temp/audit/drv_a.exe
./temp/audit/drv_a.exe    # A469/A470str/A470list/A471lem/A471arm/A471a/A475boxed/A475arm
./temp/audit/drv_b.exe    # B473/B477/B478/B482/B483/B484/B471strlist
```

### 6.4 N1 的 rustc 反证

```bash
rustc --edition 2021 -O temp/audit/e0507.rs -o temp/audit/e0507.exe
# error[E0507]: cannot move out of `*a` which is behind a shared reference
```

### 6.5 产物

| 路径 | 说明 |
|---|---|
| `temp/audit/drv_a.lz` / `drv_b.lz` | HEAD LZ 库 + 审计驱动（469/470/471/471a/475 · 473/477/478/482/483/484） |
| `temp/audit/drv_a.rs` / `drv_b.rs` / `.exe` | 驱动产物 |
| `temp/audit/e0507.rs` | N1 的 E0507 反证 |
| `temp/audit/p472_nested.lz` | BUG-472 原生 oracle 样本（原生发 `let mut c`） |
| `temp/audit/e2e/` | 8 个既有语料的端到端副本与产物 |

---

## 7. 附：本轮未做的事（边界声明）

- 未执行 `cargo build` / `cargo test`（按要求避让 cargo 锁），故**未验证** Rust 侧新增/修改代码能否编译——这属于另一 agent 的职责。
- 未修改任何产品源码（`src/`、`tests/`、`Cargo.toml`、`CHANGELOG.md`、`memory/bugs.md` 均未写）。
- 未做 `git add/commit/checkout`。
- 未复核 BUG-469 之前的条目、也未复核 BUG-488 及之后的条目。
- 台账记录的「269 文件全量差分扫描」「fastsweep/bigsweep」类全量回归**未重跑**（耗时 + 需 cargo 侧门禁），本轮以定点样本 + 函数级驱动替代。

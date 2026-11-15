# 大文件盘点与第二批拆分（move-only）· 2026-10-29

> 任务：fist-mbt **T0r35** / namespace `lang-zone` / assignee `atomcode-split`
> 仓库：`F:/AI/lang-zone`（Windows + bash）　基准 HEAD：**`c7c33e0`**
> 前置文档：`docs/大文件拆分计划-2026-10-06.md`（写于 10-06，当日已执行大部分）
> 口径：只拆 `src/` 一处 + 新建 1 个子文件 + 本报告；未改 `tests/`、`src/ir/lz_codegen*.lz`、`memory/bugs.md`；
> 所有 cargo 均带 `CARGO_TARGET_DIR=target/lane-split`；未 `git add` / `git commit` / `cargo fmt --all`。

---

## 0. 结论先行

1. **计划文档的文件级拆分已全部落地**（codegen/builder/typer/comptime/lexer 五处），10-06 那份表里「待拆」的大文件已经不存在；
   表中两处（`lz_codegen_lib.rs`、`lz_ir_lib.rs`）当日已**改判为不拆**（自举生成产物、未挂 `mod`、不参与编译）。
2. 剩余体积**不再是「文件没拆」，而是「单函数过大」**：`gen_expr` 一个函数 **7,328 行**（计划记录 5,100，已过时）、
   `gen_stmt` **2,772 行**（计划 1,850）、`convert_expr` **2,403 行**。纯移动（move-only）无法再降低这些文件的体积。
3. 本轮按「低风险 + 纯移动」判据真拆了 `src/ir/codegen/scan.rs` 的一个自洽段：
   **640 行 / 7 个函数 → `src/ir/codegen/scan/fstring_scan.rs`**，外部路径经 `pub(crate) use` 转发保持不变，
   逐字节等价（sha1 相同），三组测试全绿。
4. 下一批按风险排序见 §4；**最高风险项（expr_gen/stmt_gen 内部）必须单独立项**，不能混进 move-only 批次。

---

## 1. 盘点：`src/` 下最大的 10 个源文件（当前实测）

统计口径：`find src -name "*.rs" | xargs wc -l | sort -rn`。下表只列**参与编译**的 `.rs`（排除项见 §1.2）。

| # | 文件 | 行数 | 职责（一句话） | 公开面 | 相对 10-06 计划的状态 |
|---|------|------|----------------|--------|------------------------|
| 1 | `src/ir/codegen/expr_gen.rs` | 8,547 | 表达式 codegen（`impl CodeGen` 32 方法） | 25 个 `pub(crate) fn`，0 `pub` | P0-1 产物已拆出，**但仍待二次拆**：`gen_expr` 单函数 687–8014 行（7,328 行 = 文件 86%） |
| 2 | `src/ir/codegen_cython.rs` | 5,003 | LZIR → Cython 后端（子编译器） | 多 `pub` | **冻结**（项目铁律：Cython 后端不管不碰），仅记录不排期 |
| 3 | `src/ir/codegen/decl_gen.rs` | 3,438 | 声明级 codegen（item/fn/struct/enum/trait/impl） | 21 方法，17 个 `pub(crate)` | P0-1 产物已拆出，单函数 `gen_fn_def` 1,190 行仍大 |
| 4 | `src/ir/codegen/stmt_gen.rs` | 3,423 | 语句级 codegen（`gen_stmt`/块/模式→Rust pat） | 14 方法，14 个 `pub(crate)` | P0-1 产物已拆出，`gen_stmt` 单函数 2,772 行 |
| 5 | `src/parser/parser.rs` | 3,293 | `Parser` 核心 + 顶层 item 解析（结构体定义在此） | 1 impl / 3 `pub fn` + 11 私有 | **计划漏项**（10-06 表里没有 parser 三件套） |
| 6 | `src/ir/builder/convert_decl.rs` | 2,532 | 声明 AST→IR（duck/fn/impl/struct/trait） | 19 个 `pub(crate)` 自由函数 | P0-2 产物已拆出 |
| 7 | `src/macros/expand.rs` | 2,530 | 宏展开引擎（`@name!` 识别/递归展开/宏定义解析） | 5 impl / 21 `pub fn` / 6 `pub struct`，文件尾 10 条单测 | **计划漏项**，且是本轮看好的下一目标（见 §4-A） |
| 8 | `src/ir/builder/convert_stmt.rs` | 2,461 | 语句 AST→IR（含 walrus 收集、块转换） | 6 个 `pub(crate)` | P0-2 产物已拆出 |
| 9 | `src/ir/builder/convert_expr.rs` | 2,460 | 表达式 AST→IR | 3 个 `pub(crate)` 自由函数 | P0-2 产物已拆出；`convert_expr` 单函数 6–2408 行（2,403 行） |
| 10 | `src/parser/expr.rs` | 2,376 | 表达式解析（优先级递降）+ Pattern 解析 | `trait ParserExprExt`，48 方法 | **计划漏项** |
| — | `src/ir/codegen/helpers.rs` | 2,076 | codegen 辅助自由函数（54 个 `pub(crate)`，0 impl） | 54 个 `pub(crate)` | 计划第 8 条写「1,402 行保持不动」→ **计划数字已过时（+674）**，仍待拆 |
| — | `src/ir/codegen/scan.rs` | 2,060 | 模块级预扫描（比较约束/变量计数/type-pack/extern/dict-key） | 全 `pub(crate)` 自由函数 | **本轮拆出 640 行**（2,692 → 2,060） |
| — | `src/parser/stmt.rs` | 1,853 | 语句解析 + 构建块语义验证 | `trait ParserStmtExt`，28 方法 | 计划漏项 |
| — | `src/ir/codegen/mod.rs` | 1,826 | codegen 入口 + `CodeGen` 结构体 + 顶层 `generate*` | `CodeGen` + 6 `pub fn` | 计划 P0-1 主目标：17,702 → 1,826 ✅ |

**职责单一性判断**：上表 1–4、6、8、9 都是「一个 impl 或一族自由函数」，职责本身是单一的；
真正的问题是**单个函数体过大**（`gen_expr` 7,328 行、`gen_stmt` 2,772 行、`convert_expr` 2,403 行），
这不是拆分文件能解决的，必须做函数内部分解（= 改写，需另立项）。
`macros/expand.rs` 是唯一「多职责混装」的大文件：展开器 + 模板展开器 + 宏定义解析器三段，
其中宏定义解析段（1,131–2,201，约 1,071 行 / 18 个函数）边界干净，是下一批首选。

### 1.2 排除在排名外的文件（避免误读）

| 文件 | 行数 | 排除理由 |
|------|------|----------|
| `src/ir/lz_codegen_lib.rs` / `.lz` | 2,683 / 4,708 | 自举路线 B 的**生成产物**，`grep -rn "lz_codegen_lib" src` 在 `mod`/`use` 上零命中 → 不参与编译；改它下次重新生成即被覆盖 |
| `src/ir/lz_ir_lib.rs` / `.lz` | 1,206 / 644 | 同上（生成产物，未挂 `mod`） |
| `src/frontend/lz_lexer.lz` / `lz_parser.lz` | 351 / 897 | `.lz` 自举前端源码，非 Rust 源 |
| `src/ir/codegen_cython.rs` | 5,003 | 已列入上表但**按铁律冻结**（不排期） |

---

## 2. 与 10-06 计划对照

| 计划条目 | 计划（10-06） | 现状（10-29 实测） | 判定 |
|----------|---------------|--------------------|------|
| P0-1 `codegen/mod.rs` | 17,702 → 8 子模块 | mod.rs **1,826** + 12 子模块（expr_gen/decl_gen/stmt_gen/scan/magic_gen/types_emit/emit/pattern_gen/helpers/moddec_emit/str_boundary + 之后又加的 cap_hint/hide_ty/marks/owned_local） | **已完成**（并超出原计划） |
| P0-2 `builder.rs` | 13,861 → `builder/` × 10 | 删除单文件；`builder/mod.rs` **1,400** + 13 子模块（convert_expr 2,460 / convert_stmt 2,461 / convert_decl 2,532 / extended_check / specialize / infer / capture …） | **已完成**（`builder::build_ir`、`builder::IrBuildError` 路径零改动） |
| P1-3 `typer/mod.rs` | 2,637 → 6 子模块 | `typer/mod.rs` **448** + builtin/infer_fn/pattern/self_ty/subst/tests | **已完成**（448 vs 计划 405，正常增长） |
| P1-4 `lz_codegen_lib.rs` | 3,518 → `lzlib/` × 5 | 2,683，生成产物、未挂 `mod` | **计划已过时/改判**（不拆） |
| P2 `comptime/mod.rs` | 1,406 → 4 | **212** + eval/literal/frame | **已完成** |
| P2 `lexer/lexer.rs` | 1,372 → 内部拆 | **1,002** + numbers 203 / strings 226 | **已完成** |
| P2 `lz_ir_lib.rs` | 1,441 → `lzi/` × 3 | 1,206，生成产物 | **计划已过时/改判**（不拆） |
| P2 `codegen/helpers.rs` | 1,402「保持不动」 | **2,076**（+674） | **仍待拆**，且计划行数已过时 |
| §7.4 `moddec_emit`/`str_boundary` | 324 / 359「不动」 | 324 / 374 | 一致（`str_boundary` 已于 10-07 接线并跑单测） |
| 计划总目标 | 「全量拆分后最大单文件 ≤ ~4,000 行」 | 最大 **8,547**（`expr_gen.rs`） | **目标未达成**：`gen_expr` 单函数 7,328 行无法跨文件搬 |

**过时/漏项清单（计划文档应修订的 4 点）**

1. 计划第 1 行 `gen_expr` 记 5,100 行 → 现 **7,328 行**；第 4 行 `gen_stmt` 记 1,850 → 现 **2,772 行**。
2. 计划第 8 行 `helpers.rs` 记 1,402 行 → 现 **2,076 行**（「保持不动」的判断已失效）。
3. 计划**完全没覆盖** parser 三件套（`parser.rs` 3,293 / `expr.rs` 2,376 / `stmt.rs` 1,853）与 `macros/expand.rs` 2,530
   —— 合计 **10,052 行**，是当前最大的未规划存量。
4. 计划「≤4,000 行/文件」的指标本身不可由纯移动达成，应改为双指标：**文件 ≤ 2,500 行 + 单函数 ≤ 800 行**。

---

## 3. 本轮拆分记录

### 3.1 目标与选型依据

目标：**`src/ir/codegen/scan.rs`**（拆前 2,692 行）中的「f-string / lambda 绑定预扫描」段。

| 判据 | 实测 |
|------|------|
| 无其它车道占用 | `git status` 中 `src/` 仅 `Cargo.lock` 一处改动（非本文件），该文件自 `73f4274` 后未动 |
| 纯移动可行 | 段内 **7 个函数全部已是 `pub(crate)`** → **零可见性变更** |
| 依赖自洽 | 段内**不调用段外任何函数**（逐个比对 scan.rs 其余函数名，命中 0）；只用 `Block/Stmt/Expr/Pattern/Item/LitKind/IrType/IrModule` 与 `HashSet` |
| 外部接口面最小 | 仅 **1 个外部调用点**：`codegen/mod.rs:891 self.unbound_fstring_vars = collect_unbound_fstring_vars(module)` |
| 测试守护 | `cargo test --lib` 469 + `codegen_corner_fixes` 44 + `lz_ir_bootstrap` 25（codegen 最容易受影响的正是后两组） |

> 说明：`cargo test --lib` 的 469 条里没有直接针对 `scan.rs` 的单测（lib 单测集中在 bridge/simd/util/macros/moddec_emit），
> `scan.rs` 的行为由 **codegen 集成套件**（`codegen_corner_fixes` 44 条 + `lz_ir_bootstrap` 25 条 + `demo_codegen_compile`）守护。
> 这也是为什么本轮只挑「零可见性变更、零外部函数依赖」的段——把机械风险压到最低。

### 3.2 移动了什么

| 项 | 内容 |
|----|------|
| 移出文件 | `src/ir/codegen/scan.rs`（2,692 → **2,060** 行） |
| 迁入文件 | `src/ir/codegen/scan/fstring_scan.rs`（新建，**648** 行 = 8 行文件头 + 640 行原文） |
| 移动函数（7） | `fstring_var_interps` / `scan_expr_fstrings` / `scan_pattern_bindings` / `scan_block_fstrings` / `collect_lambda_params_expr` / `collect_lambda_params_block` / `collect_unbound_fstring_vars` |
| 行数账 | 2,060 + 648 = 2,708 = 原 2,692 + 头 8 + 接线 7 + 去向注记 1 ✅ |
| 原文件新增 | `mod fstring_scan;` + 6 行注释 + `pub(crate) use fstring_scan::collect_unbound_fstring_vars;`（共 7 行）+ 1 行「去向注记」 |

### 3.3 外部路径不变性

- `codegen/mod.rs` **零改动**：仍 `use scan::{…, collect_unbound_fstring_vars, …}`，经 `scan.rs` 的 `pub(crate) use` 转发到新子模块。
- 其余 6 个函数原本只在该段内部互相调用（外部 0 调用者），**不对外转发**——首版曾全量转发，实测多出 6 条 `unused imports` 警告，
  已收敛为只转发真实外部入口，警告数回到基线 9 条。
- `codegen::scan::<fn>` 路径对外行为不变；未新增 `pub`，未改任何签名。

### 3.4 等价性证明（move-only）

| 证明 | 结果 |
|------|------|
| 拆前往返校验（脚本内置） | `restored == raw` → **True**（按段切分再拼回，与原文件逐字节相同才允许落盘） |
| 迁入文件正文 vs 被剪段落 | sha1 均为 `82aa69bbb4274e8f4f54ac4b78d4ba1e8dfd1fd3`，24,114 bytes / 640 行 → **完全相同** |
| `scan.rs` 与原文的 diff opcode | 仅 2 处：`insert` 7 行（接线）、`replace` 640 行 → 1 行（注记）；其余全 `equal` |
| 行尾/编码 | 全 CRLF、UTF-8 无 BOM，移动段落逐行保持 CRLF（断言校验） |
| `cargo fmt` | **未运行**（也不需要对移动片段跑：内容逐字节未变，本就没有格式差异） |

### 3.5 验证读数（全部 `CARGO_TARGET_DIR=target/lane-split`）

| 项 | 拆分前 | 拆分后 | 判定 |
|----|--------|--------|------|
| `cargo build` | 通过（lib **9 warnings**） | 通过（lib **9 warnings**，逐条同名） | 零新增告警 ✅ |
| `cargo test --lib` | 469 passed / 0 failed | **469 passed / 0 failed** | 不回归 ✅ |
| `cargo test --test codegen_corner_fixes` | —（基线记录 44） | **44 passed / 0 failed**（125.25s） | 不回归 ✅ |
| `cargo test --test lz_ir_bootstrap` | —（基线记录 25） | **25 passed / 0 failed**（243.50s） | 不回归 ✅ |

> 三组套件串行执行（一次只跑一个重 rustc 套件，遵内存纪律）。

### 3.6 副作用与还原

- 跑 `codegen_corner_fixes` / `lz_ir_bootstrap` 时测试侧会重写 `tests/syntax-matrix-report.md`；
  本车道**未**编辑该文件，跑完已 `git checkout -- tests/syntax-matrix-report.md` 还原，工作树只留本轮真实改动：
  `M src/ir/codegen/scan.rs` + `?? src/ir/codegen/scan/`。
- 未触碰：`tests/`、`src/ir/lz_codegen*.lz`、`memory/bugs.md`、`cargo fmt`。

---

## 4. 下一批建议（按风险从低到高）

| 序 | 目标 | 预计收益 | 需要的改动 | 风险 |
|----|------|----------|------------|------|
| **A** | `src/macros/expand.rs` → `macros/parse.rs`：宏定义解析段（1,131–2,201，**1,071 行 / 18 fn**） | expand.rs 2,530 → ~1,470 | 段内自洽（0 个段外被调函数）；仅 `tokens_to_string`、`extract_macro_defs` 被段外引用 → 2 处 `pub(super)` + 1 个 `mod` + 转发；文件尾 10 条单测随展开器留在原地 | **低**（macros 是 lib 单测覆盖最好的大文件；建议下一轮就做） |
| **B** | `src/ir/codegen/scan.rs` 剩余三段：比较约束传播（~700 行）、type-pack 调用点（~880 行）、dict-key 泛型（~237 行） | scan.rs 2,060 → ~900（2 个新文件） | **与本轮完全同款**（全 `pub(crate)` 自由函数、段间零交叉），模式已验证一次 | **低** |
| **C** | `src/ir/codegen/helpers.rs`（2,076，54 个 `pub(crate)` 自由函数，0 impl 0 单测） | helpers.rs → ~700 × 3 文件 | 按族切：`block_has_*`/`expr_has_*` 谓词族、fstring/kwarg 渲染族、类型谓词族；全部 `pub(crate)`，转发即可 | **中低** |
| **D** | `src/parser/expr.rs`（2,376）：Pattern 解析拆 `parser/pattern.rs` | expr.rs → ~1,700 | `trait ParserExprExt` 需按职责拆两个 trait（或在 mod.rs 同时 `pub use`）；`Parser` 其余私有字段需升 `pub(super)`（`tokens/pos/pending_gt` 已是） | **中**（trait 拆分改的是 `parser/mod.rs` 的 re-export 面） |
| **E** | `src/parser/parser.rs`（3,293）：顶层 item 解析拆 `parser/item.rs` | parser.rs → ~2,000 | 同 D 的可见性代价，且该文件承载 `Parser` 结构体与其 `new/peek/advance` 基建——**动它等于动所有解析路径的地基** | **中偏高** |
| **F** | `builder/convert_expr.rs`（`convert_expr` 单函数 2,403 行）/ `convert_stmt.rs` / `convert_decl.rs`（`gen_fn_def` 1,190） | 文件级已无可拆 | 只能整体再分文件或**函数内部分解**；后者是行为改写 | **高** |
| **G** | `codegen/expr_gen.rs`（`gen_expr` **7,328 行**）/ `stmt_gen.rs`（`gen_stmt` 2,772）/ `decl_gen.rs`（`gen_fn_def` 1,190） | 单文件级无收益 | 必须按 `ExprKind`/`StmtKind` 分支提取子函数（语义等价但属改写），且正是 `codegen_corner_fixes` 44 条守护区 → **须单独立项 + 新旧发射对照** | **高（本轮明确不做）** |

**配套建议**

1. 修订 `docs/大文件拆分计划-2026-10-06.md`：把 A/B/C 三项补进清单，把 parser 三件套（10,052 行）纳入范围，
   并把指标从「≤4,000 行/文件」改为「**文件 ≤2,500 行 + 单函数 ≤800 行**」——否则计划永远显示「已达成」而实际最大函数仍在膨胀。
2. A/B 两项可以合并成一个小阶段（同一轮各拆一处），收益 ~1,700 行，风险仍属低档；
   做完后再评估 D/E，F/G 一律不进 move-only 批次。
3. 每批沿用本轮纪律：先 `git status` 确认无车道冲突 → 脚本化往返校验 → `build` + 三组测试 → 报告 + fist-mbt 记账。

---

## 5. 复算命令

```bash
cd /f/AI/lang-zone
# 盘点
find src -name "*.rs" | xargs wc -l | sort -rn | head -20
# 单函数体积（move-only 的真实边界）
grep -nE "^    (pub\(crate\) )?fn " src/ir/codegen/expr_gen.rs | head
# 本轮验证
CARGO_TARGET_DIR=target/lane-split cargo build
CARGO_TARGET_DIR=target/lane-split cargo test --lib
CARGO_TARGET_DIR=target/lane-split cargo test --test codegen_corner_fixes
CARGO_TARGET_DIR=target/lane-split cargo test --test lz_ir_bootstrap
# 等价性
PYTHONIOENCODING=utf-8 python temp/verify_move.py   # MOVE_ONLY_IDENTICAL: True
```

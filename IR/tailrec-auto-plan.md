# lz 尾递归自动优化计划（Auto TCO + `@tailrec` 严格标注）

> 状态：提案 · 2026-10-07
> 关联现状证据：`src/ir/builder/tail_call.rs`、`src/ir/builder/convert_decl.rs:945-955`、
> `src/ir/node.rs:280-291`（IntrinsicKind）、`src/ir/codegen_cython.rs:601`

## 0. 目标一句话

lz 编译器在**无标注**时自动检测「直接自调用全部处于尾位置」的函数并改写为循环
（参数重赋值 + `while true`）；新增 Scala 式 `@tailrec` 标注——**标注后若函数不是
尾递归 / 无法优化，编译报错**；标注可优化则**保证**改写为循环，不再依赖
「LLVM -O2 有机会优化」。

## 1. 现状与缺口

| 现状 | 证据 |
|---|---|
| 已有尾位置**结构校验**，但只服务手动 `#[tail_call]` 标注 | `tail_call.rs` `tail_call_positions_ok`（覆盖 if/for/while/match/try/block 尾表达式链）；`convert_decl.rs:949` 仅当 decorators 含 `tail_call` 时调用 |
| `IntrinsicKind::TailCall` 已入 IR | `node.rs:288` |
| Rust codegen 对尾递归**零转换**，注释自述「使 LLVM -O2+ 有机会优化为循环」 | `convert_decl.rs:945-946` |
| Cython 后端只 emit 一行 `# @tailcall` 注释 | `codegen_cython.rs:601` |

缺口：① 无自动发现（无标注 = 不检查、不改写）；② 无真正的循环改写（优化责任外包给 LLVM，无保证）。

## 2. 目标语义

### 2.1 自动发现（默认开启）

- 对每个 `FnDef`（含 impl/trait 方法）跑检测，**不依赖任何标注**。
- 判定枚举 `TcoVerdict`：
  - `NoSelfCall` — 无直接自调用；
  - `TailOptimizable` — 至少一处直接自调用，且全部处于尾位置、体结构可改写；
  - `NotTailPosition` — 存在非尾位置自调用；
  - `NotTransformable` — 尾位置 OK 但体结构超出 v1 改写能力（见 §4 边界）。
- `TailOptimizable` ⇒ 改写为循环；其余 ⇒ 原样保留（相互递归、非尾递归统一不转换、不报错）。
- 开关：环境变量 `LZ_TCO=0` 或 CLI `--no-tco` 关闭自动优化（标注的静态报错**不受开关影响**——标注是静态契约）。缺省开启。

### 2.2 尾位置规则

- 沿用并推广现有 `tail_call_positions_ok`：直接自调用只允许出现在「块尾语句的
  尾表达式链」上（if 两分支尾 / 块尾 / match 臂尾 / try 各段尾）；
  基本情形分支返回不含递归的值。
- **闭包边界截断**：嵌套闭包/λ 体内的自调用**不算**外层函数的尾调用
  （Scala 语义）。需核对现有 walk 是否会下钻 λ 体——若会，改为在 λ 边界停止
  （否则 `let g = || f(x)` 这类形态会被误判）。

### 2.3 `@tailrec` 标注（Scala 式，严格契约）

`@tailrec` 是保证性标注：标注 ⇒ 保证改写为循环。以下均为**编译错误**：

1. 函数体内无任何直接自调用（非递归函数套 `@tailrec`——Scala 同样报错：
   "method marked @tailrec neither recurses nor recurses via a nested function"）；
2. 存在自调用但非尾位置（`NotTailPosition`）；
3. 体结构超出 v1 改写能力（`NotTransformable`）。

错误文案约定（沿用 `ctx.report_error` 风格）：
`@tailrec 标注函数 'f' 不是可优化的尾递归：<原因>（无自调用 / 自调用不在尾位置 / 体含不可转换结构：<细节>）`

与既有 `#[tail_call]`（decorators 名 `tail_call`）的关系：`tail_call` 保留为
**弃用别名**，语义与 `@tailrec` 完全一致，附加 info 级弃用提示；既有语料零改动。
（备选：直接改名 + 全库语料清扫——不推荐，风险高收益低。）

## 3. 实现

### 3.1 检测（推广 `tail_call.rs`）

- 新增 `analyze(fn: &FnDef) -> TcoVerdict`：
  - 复用 `tail_call_positions_ok` 作「尾位置 OK」谓词；
  - 复用 `stmt_contains_call` / `expr_contains_call` 统计自调用数量与位置
    （报错时需行号：现有函数返回 bool，需扩展为返回违规位置列表）；
  - 递归 walk 改显式栈（防深体爆栈，参考 Ceres `tailrec.py` 的做法）。
- verdict 挂载 `FnDef`：`node.rs:398` 结构体加字段 `tco: Option<TcoVerdict>`
  （derive 已含 Clone/PartialEq/serde，新增枚举需同步 derive）。

### 3.2 改写（IR 层 desugar，后端无关）

- 位置：IR builder 产物收口处（`src/ir/builder/mod.rs` 组装 items 完成之后）
  新增 `rewrite_tco(module: &mut IrModule)`，对 verdict = `TailOptimizable` 的
  FnDef 就地改写。改写在 IR 层而非 codegen 层 ⇒ Rust 与 Cython 两个后端同时受益。
- 改写形态（两候选形态，M2 时按 IR 实际支持度定稿）：
  - **形态 A**（while 带值）：`let __res = while true { body' }`，body' 中尾自调用
    替换为「求参 → 重赋影子变量 → 落循环尾」；
  - **形态 B**（while + 结果变量 + break）：
    ```
    fn f(p1, p2) {
      let mut p1 = p1; let mut p2 = p2;      // 影子可变局部（let-mut，见 Stmt::Let 的 mods 字段）
      let mut __res = <默认值>;
      while true {
        <body'>    // 尾自调用 f(a1,a2) → 先求参进临时，再 p1=a1; p2=a2;（继续循环）
                   // 原 return v → __res = v; break;
      }
      __res
    }
    ```
- 关键细节：
  - **求参先行**：尾调用实参表达式先物化到临时槽再重赋值（`f(a+1, a)` 第二参用旧 a，
    防覆盖顺序 bug——专项测试）；
  - **参数影子命名风险**：同名影子可能与 codegen 变量表（`var_types`/alloca 命名）
    冲突；若冲突，改 `__tco_pN` 命名 + 体内参数引用整体改名（rename pass）。
    先试同名（生成物更可读），撞表再切。
  - 表达式尾形态的 body（块尾 `ExprStmt`）先按 codegen 既有尾值捕获机制
    （`stmt_gen.rs:116`）desugar 成 `let __t = <尾表达式>`，保证自调用恒在语句位置。
  - 体内非尾 `return x` 保持原样（直接返回，语义等价）。
- `convert_decl.rs:945-955` 现有 `#[tail_call]` 校验改为读 verdict（消除双份逻辑）：
  `tco != TailOptimizable` 且带标注 ⇒ 按 §2.3 报错。

### 3.3 后端

- Rust codegen（`codegen/decl_gen.rs` / `stmt_gen.rs`）：预期零改动（消费改写后的
  IR）；需回归 while 循环 + let-mut + 尾值捕获路径。
- Cython codegen（`codegen_cython.rs:601`）：`# @tailcall` 注释保留或移除均可
  （改写后 Cython 端已是显式循环，注释仅留作出处标记）。

## 4. 边界（v1 不转换；带 `@tailrec` ⇒ 编译错误，报 `NotTransformable`）

- 相互递归（a ↔ b）；
- 嵌套闭包内的自调用；
- 自调用被表达式包裹：`f(x) + 0`、方法链 `f(x).m()`、管道 `f(x) |> ...`；
- `match` 臂尾自调用：现有校验对 match 放行，但 v1 改写若不支持「重赋后落循环尾」
  的 match 臂 desugar，则判 `NotTransformable`（M2 实测后二选一：支持或降级）；
- 与其他 intrinsic 组合（`@memoize`/`@curry`/`@overload`）：自动路径不转换 +
  info 提示；`@tailrec` 组合 ⇒ 错误。

## 5. 改动文件清单（锚点）

| 文件 | 改动 |
|---|---|
| `src/ir/builder/tail_call.rs` | 推广为 `analyze()`：verdict + 自调用计数/位置；λ 边界截断；显式栈 |
| `src/ir/node.rs` | `TcoVerdict` 枚举 + `FnDef`（~L398）加 `tco` 字段 |
| `src/ir/builder/convert_decl.rs` | L945-955 标注分支改读 verdict 报错；intrinsic 名表注册 `tailrec` |
| `src/ir/builder/mod.rs`（组装收口处） | 调用 `rewrite_tco()`（新函数可放 `tail_call.rs` 或新 `tco_rewrite.rs`） |
| `src/moddec.rs:322` | 装饰器名表加 `"tailrec"` |
| `src/ir/codegen_cython.rs:601` | 注释去留（低优先） |
| `src/cli.rs` / 环境变量处理处 | `LZ_TCO` / `--no-tco` 开关 |

## 6. 测试与验收

**单元（builder 层）**
- verdict 五形态：无自调用 / 尾自调用 / 非尾自调用 / 闭包内自调用 / 相互递归；
- `@tailrec` 三类报错（无自调用 / 非尾位置 / 不可转换），断言文案与行号；
- 改写后 IR 断言：函数体内对 `f` 名的 `Call` 引用数 = 0，且存在 While 节点；
- 求参顺序：`f(a+1, a)` 改写后第二参取旧 `a`。

**e2e / 语料**
- 深尾递归语料：`fact_tr` / `sum_to` 量级 n = 100_000。改前行为：debug（-O0）构建
  下深递归栈溢出或超时；改后：O(1) 栈、正常输出。加**递归深度探针**：体内埋
  自增计数器，断言转换后 ≤ 2 层；
- 非尾递归 / 相互递归语料：输出与改前**逐字节一致**（golden 不漂移）；
- `@tail_call` 既有语料：别名路径全绿；
- `LZ_TCO=0`：尾形态深递归退化为普通递归（探针层数 > 2），行为开关生效。

**门禁**：全量 `cargo test` + 既有语料/golden 全绿；若有 perf bench（`BENCH/`）
跑 `fib` / `sum_to` 对照并记入报告。

## 7. 里程碑

| 里程碑 | 内容 | 验收 |
|---|---|---|
| M1 | `analyze()` + verdict 挂载（不改写、不报错） | 单元五形态全过 |
| M2 | `rewrite_tco()` 自动改写 + 开关 | 深用例 e2e 过；golden 不漂移 |
| M3 | `@tailrec` 标注 + 三类报错 + `tail_call` 别名 | 报错用例过；既有语料全绿 |
| M4 | 文档（SYNTAX/ 尾递归章节）+ CHANGELOG + bench 报告 | 文档与行为一致 |

---

## 八、实现记录（2026-10-07，M1–M3 已落地）

提交：`071aa1f feat(tailrec): Auto TCO + @tailrec strict contract`

### 实际落地形态
- **verdict 枚举**（`src/ir/node.rs`）：`NoSelfCall` / `TailOptimizable` /
  `NotTailPosition { sites }` / `NotTransformable { reason }`，挂在 `FnDef.tco`。
- **检测**（`src/ir/builder/tail_call.rs` 全面重写，`analyze_tail_recursion`）：
  显式栈扫描（防深体爆栈）；λ **与生成器体**边界截断；while / while-let / for /
  try-catch / defer / checker 体内的自调用一律记为**非尾位置**（循环回边语义）；
  arity 与默认值一致性校验。
- **改写**（`rewrite_tco`，挂在 `build_ir_inner` 收口，**IR 层 desugar** ⇒
  Rust / Cython 双后端受益、codegen 零改动）：
  `while true` + `__tco_res` 结果变量 + `break`；尾自调用 → 求参临时槽 → 形参重赋 → `continue`。
  改写后 `tco` 置 `NoSelfCall` 保证幂等。
- **开关**：`--no-tco`（单文件模式与 `build`/`check` 子命令）与 `LZ_TCO=0/off/false`
  关闭**自动**改写；`@tailrec` / `#[tail_call]` 的静态报错**不受开关影响**。
- **标注**：`@tailrec` 严格契约（无自调用 / 非尾位置 / 不可转换结构三类编译错误），
  `#[tail_call]` 保留为弃用别名（附 info 提示）；`moddec` 名表注册 `tailrec`。

### v1 边界（比计划 §4 更严格的四处，均因实测）
1. **默认形参**：Rust 侧形参是 `Option<T>`，真正的绑定是 codegen 发的
   `let x = x.unwrap_or(..)` 影子。改写后的重赋落在影子上，故
   `codegen/decl_gen.rs` 新增 `block_assigns_param()`：**仅当体内确有对该形参赋值时**
   才发 `let mut x`（否则 E0384；普通函数不多出 unused_mut 告警）。
2. **`raises` 函数**不转换：`return` 需包 `Ok(..)`，v1 改写会破坏该包装。
3. **引用 / owned / 变参 / comptime 形参**不转换：重赋值语义不等价。
4. **返回类型需可安全默认化**（Int/F64/Bool/Str/Unit/Option/List/Dict 等），
   否则无法构造 `__tco_res` 初值。

### 验收实测
- `cargo test -j 1`：**768 passed / 0 failed**（36 个测试二进制；较改动前 748 新增
  10 条 builder 单测 + 10 条 e2e）。
- `cargo test --test demo_codegen_compile -- --include-ignored`：**2 passed / 0 failed**
  （348 个 DEMO 全部转译 + rustc 编译通过，既有 `@tail_call` 语料
  `decorators_more.lz` 零改动转绿）。
- `tests/tail_recursion.rs` 10/10：100000 层深递归出结果（改写前 -O0 必栈溢出）、
  非尾递归开关前后输出逐字节一致、相互递归不转换、λ 边界不误判、
  `@tailrec` 三类报错、`tail_call` 别名、求参先行（`f(n+1,n)` 得 2 而非 3）。

### M4 未做
`SYNTAX/` 尾递归章节 + CHANGELOG + bench 报告尚未补；`BENCH/` 有无可用对照项待确认。

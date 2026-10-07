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

---

## 九、递归形态系统性检测矩阵（2026-10-07，`tests/tail_recursion_matrix.rs`）

### 9.1 检测方法
40 例数据表，每例同时断言两件事：
- **优化是否生效**：解析编译器自己的 `[tco] 自动改写 N 个` 计数行（**不能**只看
  产物里的 `__tco_res`——Unit 返回的函数改写后不引入结果变量），并与产物中的
  改写痕迹（`__tco_res` / `__tco_a`）交叉验证；
- **语义是否保持**：产物真编译（rustc）真运行，输出逐行比对。

判定口径：**「未优化」不是失败**（保守边界是设计的一部分），失败条件只有
「期望优化却没优化 / 期望不改写却改了 / 语义漂移 / 编译或运行失败」。
运行方式：`cargo test -j 1 --test tail_recursion_matrix -- --nocapture`（会打印全表）。

### 9.2 覆盖形态与结论
| 组 | 形态 | 结论 |
|---|---|---|
| A（17 例） | 直接尾递归：if/else、累加器形参、gcd 形参轮换、match 分支、match 守卫、`&&`/`||` 右操作数、let 后尾调用、提前 return、实参互依、Unit 返回、str 返回、10 万层深递归、实参含副作用、实参含非尾自调用、三形参轮换、实参来自 λ 形参 | 除「实参含副作用/含非尾自调用」两类本身非尾外，其余**全部优化且语义一致** |
| B（6 例） | 非尾递归：阶乘、朴素斐波那契、局部累加器、树形分叉、尾递归后在外层再运算、**10 万层非尾递归** | 全部不优化；深非尾递归**仍会栈溢出**（已固化为 `Run::Crash` 用例，明确记录 TCO 的边界） |
| C（3 例） | 互递归：偶奇尾互调、非尾互调、三函数环 | 全部不优化（v1 只改写直接自调用） |
| D（14 例） | 边界绕过：while 体调用方、for 体调用方、λ 内递归、内嵌 def 自调用、defer 函数、try 块内自调用、raises 函数、递归生成器、类方法 self 递归、经函数形参递归、无自调用、let 绑定非尾、bool 返回、Option 返回 | λ / try / raises / 生成器 / self / 函数形参 → 不优化；while·for 调用方、内嵌 def、bool/Option 返回 → 优化 |

汇总：**40 例 = 优化生效 22 + 未优化 18，问题 0**。

### 9.3 矩阵暴露并已修复的三处真缺陷
| # | 缺陷 | 症状 | 修复 |
|---|---|---|---|
| 1 | 改写器不处理 `Stmt::Match` | 分析器判尾位置、函数被裹进循环，但 match 变语句、分支值被丢弃 ⇒ **E0308** | 改写器补 `Match` / 尾位置 `Block` 分支，逐分支改写为「赋 `__tco_res` + break」或「形参重赋 + continue」 |
| 2 | Unit 返回仍声明结果变量 | 后端省略 `let x: () = ();`，循环内赋值 ⇒ **E0425** | Unit 走「裸表达式 + break」，不声明结果变量、不生成尾部 `return` |
| 3 | 含 `defer` 的函数被改写 | defer 是**逐次调用**的清理块，改写后只执行一次且落在循环体不可达分支 ⇒ 递归 4 层打印 4 次变 0 次 | 结构层新增 `find_defer` ⇒ `NotTransformable` |

三处均已补回归用例（`tests/tail_recursion.rs`：`match_arm_tail_call_is_rewritten_with_branch_values_kept`、
`unit_return_tail_call_emits_no_result_var`、`defer_in_tail_recursive_fn_is_not_transformed`）。

### 9.4 与 TCO 无关的既有缺陷（矩阵中显式标注，不计入 TCO 判定）
| 形态 | 症状 | 状态 |
|---|---|---|
| `try` + `enum` 异常 | 产物 E0308（`match` 分支类型不一致），`--no-tco` 同样失败 | 既有缺陷，未修 |
| `raises` + `enum` 异常 | 产物 E0308，`--no-tco` 同样失败 | 既有缺陷，未修 |
| 递归生成器 `yield from` | IR build 直接拒绝：`函数 X 返回类型不匹配：期望 int，实际 Itor<Vec<int>>` | 既有缺陷，未修 |

这三类在矩阵里以 `TranspileOnly` / `TranspileBlocked` 模式记录事实：既不掩盖，也不
把锅算到 TCO 头上。

### 9.5 提交与验收
- `f610849 fix(tailrec): match/unit/defer three defects found by recursion matrix`
- 隔离环境（`git worktree`，避开并行 agent 的在途改动）全量：
  `cargo test -j 1` = **772 passed / 0 failed**（37 个测试二进制）；
  `demo_codegen_compile -- --include-ignored` = **2 / 0**（348 DEMO 全部转译 + rustc 通过）。
- 同批把 `071aa1f` 误纳入的他人未完成改动从提交中剔除，使 **HEAD 可独立编译**。

---

## 十、第二轮覆盖审计（2026-10-07，`4cccc29`）

### 10.1 方法：从「逐例黑盒」升级为「结构性交叉核对」
矩阵是黑盒逐例，能发现**已想到**的形态；本轮补一层结构性核对：
1. **语句形态覆盖交叉核对**：把「分析器按尾位置传播的形态集合」与「改写器实际改写的
   形态集合」求差集——差集里的形态就是「判为可优化但改不动」的危险区。
2. **变体覆盖核对**：枚举 `ExprKind`（30 个）与 `Stmt`（23 个）全部变体，逐一确认
   被扫描器 / 改写器 / 通用子树收集器覆盖。

结果：`ExprKind` 30/30 覆盖；`Stmt` 23/23 全部归入四类之一
（尾传播且改写器已覆盖 / 非尾黑名单 / 逐次调用语义阻断 / 叶子）。

### 10.2 追加发现并修复的三处缺陷
| # | 缺陷 | 症状 | 修复 |
|---|---|---|---|
| ④ | 命名块（`block NAME:`）体在尾位置 | 分析器尾传播、改写器无分支 → 块内递归留在循环里 → **无限递归（实测挂死）**；`break/continue <label>` 标签语义亦冲突 | `find_defer` 泛化为 `find_per_call_semantics`（defer / 命名块 / 带标签跳转）；扫描器不再把命名块当尾位置容器 |
| ⑤ | **match 守卫**里的自调用是检测盲区 | 扫描器完全不遍历 `MatchArm.guard`（计数与改写都漏）；函数别处有真尾调用时被判可优化，守卫里的递归留在循环内 → **无限递归** | `scan()` 与 `collect_stmt_subtrees` 都补守卫（按非尾处理） |
| ⑥ | `Result` 返回类型初值取 `None` | Result 无 nil 值 → `let __tco_res: Result<T,E> = None;` ⇒ **E0308** | `default_value` 对 Result 返回 `None`（⇒ `NotTransformable`）；`Option` 保持 `None`；`Set` 仍可优化（`HashSet::new()`） |

三处各配一条回归用例（`labeled_block_tail_call_is_not_transformed`、
`match_guard_self_call_is_not_transformed`、`result_returning_tail_recursive_fn_is_not_transformed`）。

### 10.3 本轮全量验收（含并行 agent 的在途改动，主工作树）
| 闸门 | 结果 |
|---|---|
| `cargo build`（含双方改动） | 0 error |
| `cargo test -j 1` | **775 passed / 0 failed**（37 个测试二进制） |
| `demo_codegen_compile -- --include-ignored` | **2 / 0**，且此时 `KNOWN_TRANSPILE_FAILURES` 与 `KNOWN_RUSTC_FAILURES` **均为空清单** |
| `cy_codegen_gate -- --include-ignored`（Cython 三层） | **4 / 0**：L1 转译 84 件、L2 cythonize **84/84**、L3 编译运行 + `.exp` oracle 逐字比对全通过 |

Cython 后端未被触碰，且 IR 层改动（含 TCO 的 desugar）经L3 oracle 逐字比对确认
**未引入跨后端语义漂移**。

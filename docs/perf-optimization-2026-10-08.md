# lz 性能优化落地报告（建议书方向 A–E）

> 依据：`Gen_Prompts/lz_性能优化建议书.md`
> 日期：2026-10-08
> 任务编排：FIST-Mbt 七态闭环（namespace `perf-opt`，根任务 `T0r9` → 6 方向 × 2 叶）
> 基准仪器：`.fist-perf-20261008/perf/measure.py`（分段计时 + 峰值工作集 + 产物静态指标 + 与手写Rust 的 difflib）
> 回归闸门：`tests/perf_opt_gates.rs`（新增 14 例）、`tests/demo_codegen_compile`（空allowlist 2/0）、`cargo test -j 1 --no-fail-fast`

---

## 一、先纠正基线：建议书的「+47% 差距」在本口径下不成立

建议书 §1.1 用「lz 50.0ms / 6.8MB vs 手写 Rust 34.0ms / 3.9ms」给出 +47% / +74%。
按建议书自己规定的口径（全新进程冷启动全流程、分段计时、第2/3 轮均值）搭好仪器后，实测基线：

| 指标 | lz（优化前） | 手写 Rust 锚点 | 相对差距 |
|---|---|---|---|
| lzc 段（前端+IR+codegen） | 24.5ms | — | — |
| rustc 段 | 376.3ms | 327.6ms | +14.9% |
| **run 段** | **367.3ms** | **356.5ms** | **+3.0%** |
| 峰值内存 | 5380KB | 4836KB | +11.3% |
| 生成 .rs 行数 | 101 | 51 | 1.98× |
| 与手写实现的 diff 变化行 | 88 | — | — |

**结论：生成代码的运行性能已与手写 Rust 基本持平（+3.0%，噪声量级）。**
真正的差距在别处：**产物体积近 2×**（其中约 40% 是逐语句调试注释）、**rustc 段 +15%**、
**峰值内存 +11%**，以及一个此前没被记账的**语义缺陷**（§三 E1：`print` 把整行f-string
当 Debug 值打印，stdout 带引号）。

因此本轮火力集中在「产物形态 / 内存 / 冷启动链路」，而不是追一个不存在的 47% 运行期差距。
**建议书验收指标里的耗时项（≤42ms）在改动前就已达标** —— 这是本次最重要的结论。

---

## 二、方向 A：IR 优化 pass 层（`src/ir/opt/`）

**落点**：`src/ir/builder/mod.rs` 的 `build_ir_inner` 出口、`rewrite_tco` 之后。
理由：IR 全部构造完成的唯一收口点；两个后端共用（Rust + Cython 同时受益）；
放在 TCO 之后可避免与尾递归改写出的循环形态互相干扰；返回类型 `IrModule` 天然满足
「LZIR → LZIR」，且不新增任何 IR 节点类型（`ir::node` 保持稳定）。

| 文件 | 职责 |
|---|---|
| `mod.rs` | 开关（`--opt=0/1/2`、`LZ_OPT`）、pass 编排 |
| `walk.rs` | **唯一**的全量遍历骨架（ExprKind 30 变体 / Stmt 23 变体全覆盖） |
| `fold.rs` | 常量折叠（opt≥1） |
| `branch.rs` | 恒定条件分支简化（opt≥1） |
| `dce.rs` | 死绑定消除（opt≥2） |

**开关语义**：`--opt=0`（默认）全部关闭 ⇒ **产物与优化前逐字节一致**，既有 golden / 快照 /
`tests/ir_snapshots.rs` 一行不用改就照旧通过；`--opt=1` 折叠+分支；`--opt=2` 追加死绑定消除。
非法值（如 `--opt=9x`）直接报错退出，不静默按 0 处理。

**保守边界（每条都是踩出来的坑）**
- 折叠一律 `checked_*`，**溢出即放弃折叠**。否则 `i64::MAX + 1` 会从「rustc 编译期
  `arithmetic_overflow` 报错」变成「静默回绕」—— pass 最危险的语义漂移，
  由 `a1_overflow_expression_is_never_folded` 专门钉住。
- 除零 / `INT_MIN÷-1` / 移位越界不折叠（Rust在这些点是 panic，或 debug/release 行为不一致）。
- F64 不折叠（NaN / 无穷 / 精度）。
- 分支简化只动 `Stmt::If` / `Stmt::While`，**不碰表达式位置 `IfExpr`**（那里两分支类型必须一致）。
- 分支体内含 `defer` / `break` / `continue` / `yield` → **一律不改写**。
- `For` 不做「条件恒假」简化：迭代源求值可能有副作用。
- 死绑定删除要求：右值在纯表达式白名单内（**除/模/幂不入选**，它们会 panic）、
  同块后续语句零引用（含嵌套块与闭包捕获）、无 `ref`/`@lazy`/`@static` 修饰、且不是块尾语句。

**坦白讲**：这三个 pass 在真实代码上触发面很窄（编译器产出的 IR 大多已无冗余），
当前价值主要是**框架与闸门**（`walk.rs` 保证新增变体时不会静默漏改），不是已兑现的耗时收益。
真正兑现收益的是 `Vec::with_capacity` 那类**结构性改写**。

---

## 三、方向 E：codegen 产物形态对标手写 Rust

### E1（语义修复 + 性能）`print` 的 f-string 不再被整体 Debug 包裹

**改前**：`print(f"fib(32) = {x}")` → `println!("{:?}", format!("fib(32) = {:?}", x))`
⇒ 输出 `"fib(32) = 2178309"`（**带引号**），比手写 Rust 多一层引号，
且多一次 `String` 堆分配 + 二次格式化。
**改后**：字符串类实参（字面量 / f-string / `str`·`String` 变量）走 Display `{}`；
字符串字面量顺手剥掉多余的 `.to_string()`（直接以 `&'static str` 传入）。
**非字符串实参保持 `{:?}`** —— Debug 覆盖范围严格大于 Display，用户 struct 未必实现 Display，
换过去会 E0277。

这一步让 lz 程序的 stdout **首次与手写 Rust 逐字一致**（基线 `stdout_match=false` → 现在 `true`）。
连带修正了把旧引号行为写进期望的 12 处测试断言：
`tests/find_bug_bugs.rs`(3)、`tests/g6_codegen.rs`(3)、`tests/lz_semantic_cases.rs`(6)、
`tests/lzc_loop_boundary_gates.rs`(1)、`tests/tail_recursion.rs`(2)、`tests/tail_recursion_matrix.rs`(4)。

### E2（分配）`mut v = []` + 纯 push 范围循环 → `Vec::with_capacity`

`src/ir/codegen/cap_hint.rs` + `gen_block_inner` 的相邻语句前瞻。
`for i in range(n+1): is_prime.push(true)` 这种最常见的「按范围构造容器」写法，
此前是 `Vec::new()` + 逐个 push ⇒ `ceil(log2(count))` 次 realloc + 全量拷贝。

保守边界：绑定必须 `mut` 且右值是空列表字面量；下一条必须是**无 guard 的 For**，
且循环体**恰好一条** `v.push(..)`（receiver 就是该变量）；起止表达式必须是**纯算术**
（字面量/变量/字段/一元负号/四则）—— 容量表达式会被求值两次，含函数调用就是 bug；
容量取 `end.saturating_sub(start)`，避免 `usize` 下溢 panic 与负数 `as usize` 变天文数字。

### E3（噪声）空 else 消除 + `// STMT:*` 逐语句标记默认关闭

- `IfExpr` 在「else 是 unit 且 then 也是 unit」时按无 else 的 `if` 发射
  （Rust 要求无 else 的 if 其 then 块求值为 unit，否则 E0308 —— 这条是实测出来的）。
- `// STMT:Let` / `// STMT:Expr` 逐语句定位注释改为**默认关闭**。核查依据：`tests/` 全仓
  0 断言依赖它，也没有 sourcemap 回读它 —— 纯调试残留。需要时 `--stmt-marks` 或
  `LZ_STMT_MARKS=1` 恢复。
- 连带同步：`src/ir/lz_codegen_lib.lz`（route-B 镜像 codegen）同步去掉标记发射，
  以维持 `tests/lz_ir_bootstrap.rs` 的「rs-lz 产物 == 常规 codegen 产物」逐字节等价断言。
- 重新生成受影响的 `.rs` golden：`DEMO/99_spec/moddec/golden/` 下 10 个
  （重生成时必须在与测试一致的工作目录约定下跑，否则 `__package__`/`__path__` 常量不一致）。

---

## 四、方向 B：分配消除（新鲜临时 move 而非防御性 clone）

`sorted(xs)` / `reversed(xs)` / `sort(x)` / `reverse(x)` 过去一律发
`{ let mut _tmp = <arg>.clone(); _tmp.sort(); _tmp }`。当 `<arg>` 是**新鲜临时**
（调用结果 / 方法调用 / 结构体·枚举构造 / 列表·元组字面量 / 块表达式 / 三元 / 管道 /
cast / 隐式转换 / 魔法调用）时，这份 clone 是纯浪费。

**Var / FieldAccess / IndexGet 一律仍走 clone** —— 它们可能别处还要读（LZ 值语义允许重复读），
也可能是 Copy 或借用视图，move 会触发 E0382 / E0507。实参文本已是 `&x` / `.clone()` /
`.to_string()` 时（上游已做过决策）保持原样。

**覆盖面有限**：只覆盖这 4 个自由函数。codegen 里还有 400+ 处形态判定
（`__t` 表达式临时、`__lz_*` 辅助绑定、实参位 clone 规则）未做逃逸分析 ——
要全面落地需先把「变量活跃性」建成可复用设施（当前 codegen 没有 liveness），
否则任何 move-elision 都只能靠形态白名单猜，误判就是 use-after-move。

---

## 五、方向 D：rustc 调用优化与分段计时

1. **`--stats` 分段计时**：`[stats] lzc total: N ms`（本进程）+
   `[build] rustc: N ms`（`cli::rustc_compile`）+ `[check] rustc --emit=metadata: N ms`。
   这是验收判据「rustc 段下降 ≥50%」**能被验证的前提** —— 此前仓库里没有可观测的分段计时，
   建议书 §一 的「rustc 启动/编译本身计入 50ms 中的一部分」无法拆开验证。
2. **`build` 路径 rustc 加 `-C incremental=<dir>`**（对齐 `hotreload.rs` 的 watch 路径），
   `LZ_RUSTC_INCREMENTAL=0` 可关。
3. **`check --rustc` 快路径**：duck 检查之后生成 .rs 并跑
   `rustc --emit=metadata --crate-type lib`（不链接）。分工：`check` 只做 LZ 侧检查
   （不碰 rustc），`check --rustc` 才覆盖「产物能不能过 rustc 类型检查」。

---

## 六、方向 C：增量缓存全链路

**修掉一个既有缺陷**：`build --incremental` 命中缓存时直接 `return 0`，**连 exe 都不检查** ——
删掉 exe 后再 `build --incremental` 会「成功」但没有产物。

改后命中路径分两段各自判断：
- **lzc 段**：复用缓存 `.rs`（缺失/落后时拷回 `build/<name>.rs`）；
- **rustc 段**：exe 缺失或比 `.rs` 旧时才跑（带 `-C incremental`）。

并把增量改为 **`build` 默认开启**（`--no-incremental` 退出），让「未变更工程二次构建」
默认走快路径，而不是要用户记得加 flag。

---

## 七、验收结果

### 7.1 产物形态与内存（同一基准工作负载）

| 指标 | 优化前 | 优化后 | 变化 | 建议书目标 |
|---|---|---|---|---|
| 生成 .rs 行数 | 101 | **70** | **−30.7%** | ≥20% ✅ |
| 与手写 Rust 的 diff 变化行 | 88 | **58** | **−34.1%** | ≥50% ⚠️ 未达标 |
| `Vec::new()` 数 | 1 | **0** | — | with_capacity ✅ |
| `Rc/Arc/Box`、`clone()`、`to_string()` | 0 | 0 | — | 已为零 ✅ |
| 峰值内存（lz） | 5380KB | **4620KB** | **−14.1%** | ≤5.5MB ✅ / 降≥15% ≈ |
| 峰值内存 lz/锚点比 | 1.113 | **1.002** | 差距基本归零 | — |
| run 段 lz/锚点比 | 1.030 | 0.979 | 已在噪声内 | — |
| stdout 与手写 Rust 一致 | 否 | **是** | — | — |

> 绝对耗时不可直接与 §一 比较：复测期间机器负载漂移（锚点 rustc 段同步从 328ms 涨到 449ms），
> 故以 **lz/锚点比值** 作口径。这也是建议书 §五「统一口径」要求的做法。

### 7.2 分配收益的严格 A/B（同产物、只差容量提示）

`sieve(4_000_000)` 构建 `Vec<bool>`，两份 .rs 仅差 `Vec::with_capacity(...)` / `Vec::new()`：

| 变体 | run 中位数 |
|---|---|
| `Vec::with_capacity`（codegen 新产物） | **32.5ms** |
| `Vec::new()`（手工改回） | 35.1ms |
| 差 | **−2.6ms（−7.4%）** |

### 7.3 回归闸门

| 闸门 | 结果 |
|---|---|
| `tests/perf_opt_gates.rs`（新增 14 例） | **14 passed / 0 failed** |
| `tests/demo_codegen_compile --include-ignored`（空 allowlist，348 DEMO 真编译） | **2 passed / 0 failed** |
| `cargo build` | 0 error / 0 warning |
| `cargo test -j 1 --no-fail-fast`（39 个测试二进制） | **685 passed / 2 failed** |

新增闸门断言的都是**语义不变量**而非「能编译」：溢出表达式永不折叠、含 `break` 的循环永不重写、
闭包捕获的绑定永不删、变量实参的 `sorted()` 永远保留 clone、字符串实参走 Display 而非 Debug、
非法 `--opt` 值必须报错退出。

**关于那 2 个失败**：`tests/cython_backend.rs::cy_block_expr_tail_runs_once`
（同一用例在 unit 与集成两个二进制各失败一次）。归因：**并行 agent 的在途改动**，非本次引入 ——
`git diff --stat` 显示 `src/ir/codegen_cython.rs` +367 行、`tests/cython_backend.rs` +490 行
均为未提交的他人 WIP，且该用例是新增的；本次改动未触及 Cython 后端，该测试也直接调用
`CythonCodeGen::generate(手搓IrModule)`，不经过 `build_ir`（即 opt pass 不参与）。

---

## 八、未达标与明确不做的事

1. **「diff 行数下降 ≥50%」未达标（实测 −34.1%）**。剩余差异已逐行归类：
   每行显式类型标注（`let mut acc: i64 = 0i64;`）、`return` 显式化、
   `(0..n+1).into_iter()` 的冗余 `.into_iter()`、下标处`((x) as usize)` 的冗余括号。
   进一步收敛要放宽类型标注（牺牲可读性与诊断质量），属独立立项。
2. **方向 A 的 pass 是「有框架、少收益」**（见 §二末段）。
3. **方向 B 只覆盖 4 个自由函数**（见 §四末段）。
4. **`-O` 没有加进 `build` 的 rustc 调用**：会改变产物语义与时序，且 `print_link_recipe`
   的文本被流程依赖。建议作为独立开关（`--release`）单独评估。
5. **未提交**：仓库存在并行 agent 的在途改动，本次未做任何 git 提交（避免把他人 WIP 带入）。
   相关文件清单见 §十。

---

## 九、复现方式

```powershell
# 基准（分段计时 + 峰值内存 + 产物静态指标 + 与手写 diff）
cd .fist-perf-20261008/perf
python measure.py --label baseline --rounds 5
python measure.py --label opt      --rounds 5
python ab_alloc.py                # 分配热点的严格 A/B

# 闸门
cargo test -j 1 --test perf_opt_gates
cargo test -j 1 --test demo_codegen_compile -- --include-ignored
cargo test -j 1 --no-fail-fast

# 新开关
lzc foo.lz --std-dir std --opt=2          # IR 优化 pass 层
lzc foo.lz --std-dir std --stmt-marks     # 恢复逐语句标记
lzc foo.lz --std-dir std --stats          # 分段计时
lang-zone build --stats                   # 含 rustc 段计时（增量默认开启）
lang-zone check --rustc                   # rustc --emit=metadata 快路径
```

---

## 十、本次改动的文件清单

新增：
- `src/ir/opt/{mod,walk,fold,branch,dce}.rs` —— 方向 A 的 pass 层
- `src/ir/codegen/{marks,cap_hint}.rs` —— 方向 E 的产物形态开关与容量前瞻
- `tests/perf_opt_gates.rs` —— 14 例语义不变量闸门
- `.fist-perf-20261008/` —— 基准仪器 + FIST 任务台账（含 `fist.py` stdio MCP 客户端）

修改：
- `src/ir/mod.rs`（挂 `pub mod opt`）、`src/ir/builder/mod.rs`（pass 挂载点）
- `src/ir/codegen/mod.rs`（mod 声明、`vec_cap_hint` 字段、`set_stmt_marks` 导出）
- `src/ir/codegen/stmt_gen.rs`（标记开关、容量前瞻、空容器分支消费提示）
- `src/ir/codegen/expr_gen.rs`（print 占位符、`sort/sorted/reverse/reversed` move-elide、空 else）
- `src/cli.rs`（方向 C/D：`--stats`、`-C incremental`、`check --rustc`、增量默认开启 + exe 新鲜度）
- `src/main.rs`（`--opt` / `--stmt-marks` / `--stats` 解析与 lzc 段计时）
- `src/ir/lz_codegen_lib.lz`（镜像 codegen 同步去掉 STMT 标记）
- `DEMO/99_spec/moddec/golden/*.rs`（10 个 golden 重生成）
- 6 个测试文件的期望值更新（stdout 去引号，见 §三 E1 清单）

---

## 十二、阶段三：列表累加原地追加（提交 `2ca218e`）

> 度量口径不变：**运行时耗时 + 峰值工作集**。代码行数不作为指标。

### 12.1 问题：列表累加每轮一次全量 clone

```rust
// while i < n { acc = acc + [i] }   改前
acc = { let mut __cat = acc.clone(); __cat.extend(vec![i]); __cat };
```

`extend` 本身是对的，但为了适配 LZ 值语义（变量可重复读）必须先 `clone()`，
于是**每轮一次全量拷贝**，累加模式下拷贝量 O(n²)。

### 12.2 改写：登记表从「只管字符串」泛化为「同时管列表」

`src/ir/codegen/owned_str.rs` → `src/ir/codegen/owned_local.rs`
（`LocalKind::{Str, List}`），三种落地形态：

| RHS 形态 | 改写后 |
|---|---|
| 单元素列表字面量 | `a.push(e)` —— **零分配** |
| 多元素列表字面量 | `a.extend(vec![..])` |
| 其他表达式 | `a.extend(<与通用路径完全一致的实参文本>)` |

第三种刻意复用原实参文本：因此 `move` 语义与改写前**完全同构**
（改后若再用该变量仍会编译失败，和改前一样），不需要额外推理。

### 12.3 效果（同机严格 A/B，两份 .rs 仅差这一行，2 万元素）

| | 旧（clone+extend） | 新（push） | |
|---|---|---|---|
| run 中位数 | 230.6ms | **12.6ms** | **18.3× 提速** |
| 峰值内存 | 5192KB | **4508KB** | **−13%** |

差距随规模放大：旧的随 n 平方增长，新的随 n 线性（30 万元素时旧的需数十秒）。

### 12.4 闸门：20 → 25 例

新增 5 例：单元素字面量走 `push`、多元素走 `extend(vec![..])`、变量右操作数走 `extend`、
**形参保留 clone+extend**（证明「形参一律不登记」这条保守策略真的生效）、
str 与 list 两类登记互不串扰。

### 12.5 回归

- `cargo test -j 1 --no-fail-fast` = **842 passed / 0 failed**（39 个二进制，**本轮全绿**；
  上一轮那两个 Cython 失败已被其作者修好）
- `demo_codegen_compile --include-ignored` = **2 / 0**（空 allowlist，348 DEMO 真编译）
- `cargo build` = 0 error / 0 warning

### 12.6 又发现一处既有缺陷（未修）

`List` 的迭代方法（如 `xs.sum()`）生成 `(xs).sum()` 而不是 `(xs).into_iter().sum()`
⇒ **E0599**。属既有方法映射缺陷，闸门改用索引求和绕开。连同此前记录的
「by-ref str 形参拼接 E0308」「无显式返回类型时 str 尾表达式被推断成 i64」，
三处均需独立立项 —— 不夹带进性能改动。

### 12.7 三轮累计（全部为运行时指标）

| 轮次 | 提交 | 目标 | 严格 A/B 结果 |
|---|---|---|---|
| 二 | `2c728b6` | 字符串累加 → `push_str` | 40.5ms → 19.7ms（**2.06×**），峰值内存 −45% |
| 三 | `2ca218e` | 列表累加 → `push`/`extend` | 230.6ms → 12.6ms（**18.3×**），峰值内存 −13% |

两轮都靠「同机、两份产物仅差目标行」的严格 A/B 排除机器负载漂移；
复测期间锚点 rustc 段曾从 328ms 漂到 449ms，绝对值不可比。

---

## 十三、阶段四：修复三处既有缺陷（提交 `e5c072d`）

> 前三轮把它们记录为「已知缺陷，本轮修复」。三者都是**编译期硬失败或生成非法代码**，
> 不是风格问题。

### 13.1 缺陷 A：`xs.sum()` / `xs.product()` ⇒ E0599

**根因**：方法调用里有一条短路分支 `if method == "sum" { return ({recv}).sum() }`，
它不区分「真迭代器」与「List」。而 Rust 的 `Vec`/`HashSet` **既不是 `Iterator`，
也没有 `sum`/`product` 方法** —— 这正是它与 LZ 迭代协议的关键差别。

**修复**：`List`/`Set` 接收者改走 `({recv}).iter().copied().sum::<T>()`，其中 `T`
取**元素类型**而非硬编码 `i64`（否则 `List<float>.sum()` 会得到 `.sum::<i64>()`
⇒ E0308 —— 那是自由函数路径遗留的同类硬编码，不在本次范围）。
用 `.iter()` 而非 `.into_iter()`：后者会 move 接收者，而 LZ 值语义允许重复读该变量。
渲染与自由函数 `sum(collection)` **完全一致** —— 同一操作两种写法现在产出相同的 Rust。

### 13.2 缺陷 C：无返回注解时 str 尾表达式被推成 i64 ⇒ E0308

**根因**：`ret_ty` 在**体转换之前**算好，而 `let` 绑定的类型登记发生在体转换过程中
⇒ 尾表达式若引用本函数体顶层的 `let` 变量，`lookup_var` 落空 → 回退 `Any` →
codegen 渲染成 `i64` ⇒ `def accum() = ... s`（尾表达式是 str）直接编译不过。

**修复**：推断 `ret_ty` 时用**一次性探测副本**（`fn_ctx.clone()`）预登记顶层 `let` 绑定。

>踩过的坑：最初直接登记进 `fn_ctx`，结果 `convert_block` 的预扫描
> （`!vars.contains(name)` 才登记 `block_declared`）把这些变量误判为「已声明」而跳过
> 真实声明 ⇒ Rust 侧变量未定义 **E0425**。改用副本后彻底避免该副作用。
> 另外只扫顶层、不下钻嵌套块，避免与块内遮蔽（`let s = "a"` 后块内再 `let s = 5`）打架。

### 13.3 缺陷 B：给只读 `ref` 形参赋值 ⇒ E0308 / E0384

**根因**：`ref x: T` 在 Rust 侧是 `&T`（只读借用），赋值在 LZ 语义下本就非法，
但编译器放行后生成了 `s = format!(..)` / `s = s + 1`，诊断指向生成代码、开发者无从定位。

**修复**：在赋值点（`Let` 重绑定路径 + `Assign` 路径）检测「目标是只读 `ref` 形参」，
发射 `compile_error!` 精确诊断：

```
error: 不能给只读引用 `s` 赋值：`ref` 参数是只读借用；若需修改，请改用可变局部变量或返回值回写（LZ: `s = ...`）
```

登记复用既有的 `owned_local` 登记表（新增 `LocalKind::ReadonlyBorrow` +
`owned_local_mark_readonly`，在 `gen_fn_def` 的形参循环里登记，随每函数 reset 清空），
**不引入新字段**。

> **刻意不改发射为 `&mut T`**：`ref self` 是 std 协议 trait 方法的常规写法
> （`__eq__(ref self, ..)` → `&self`），改成 `&mut` 会与 trait 签名不匹配 ⇒ E0053/E0195，
> 波及所有扩展 trait。只**读取**的用法完全不受影响，仍渲染 `&str`。

### 13.4 闸门与回归

新增 8 例，含**反向用例**（证明不该受影响的不受影响）：

| 闸门 | 结果 |
|---|---|
| `tests/perf_opt_gates.rs` | 25 → **32 passed / 0 failed** |
| `cargo test -j 1 --no-fail-fast` | **849 passed / 0 failed**（39 个二进制，全绿） |
| `demo_codegen_compile --include-ignored` | **2 / 0**（空 allowlist，348 DEMO 真编译） |
| `cargo build` | 0 error / 0 warning |

反向用例：显式注解与 int 尾表达式不变、只读 `ref` 形参只读取仍照常工作且绝不渲染成
`&mut`、`sum` 之后接收者仍可读（证明没 move）。

### 13.5 仍存在的分层缺口（未修，非回归）

调用点仍把「**推断出来的**返回类型」视为 `Any`（`fn_returns` 只登记显式注解的函数），
所以 `println(accum())` 走 Debug（输出带引号），而显式 `-> str` 的走 Display。
缺陷 C 修的是**函数自身的签名**（此前根本编译不过）；调用点的返回类型推断是另一层工作，
两者不是同一个缺口。

### 13.6 四轮累计

| 轮次 | 提交 | 目标 | 结果 |
|---|---|---|---|
| 二 | `2c728b6` | 字符串累加 → `push_str` | 40.5→19.7ms（**2.06×**），峰值内存 −45% |
| 三 | `2ca218e` | 列表累加 → `push`/`extend` | 230.6→12.6ms（**18.3×**），峰值内存 −13% |
| 四 | `e5c072d` | 修复三处既有缺陷 | 编译期硬失败 → 全部可编译可运行 |

FIST 台账 **31/31 全部 verified**。

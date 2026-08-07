# Lang-Zone IR-only 特性补齐与文档修复计划

## 目标

围绕你给出的 3 条要求，形成一条**只走 IR 路线**的实施计划：

1. **实现项目中尚未完成的语法与特性**。
2. **修复 `.lz` 文件尚未正确翻译成 Rust 的相关文档**，把“现状 / 已知失败 / 已修复 / 未实现”说清楚。
3. **严格禁止新增 AST → Rust 直译逻辑**；所有功能都必须经由 `AST -> LZIR -> Rust` 管线落地。

---

## 当前仓库现状（基于代码与文档核对）

### 已确认的真实现状

- `src/main.rs` 当前**默认已经走 IR codegen**；只有显式传 `--ast-codegen` 才走旧路径。
- 活跃测试入口已切为 IR-only：`tests/mod.rs` 只挂 `ir_snapshots` 和 `reject_errors`。
- `tests/deprecated/compile_demos.rs` 已被隔离，并明确标注为违反 IR-only 约束的历史技术债。
- `src/ir/node.rs` / `src/ir/builder.rs` / `src/ir/codegen.rs` 已具备较完整骨架，但仍存在“节点有了、builder/codegen 语义未完全打通”的缺口。

### 已发现的文档/状态不一致

1. `README.md` 第 7 行仍写“默认输出还是老 AST 路线”，**与 `src/main.rs` 当前行为不符**。
2. `IR/migration-roadmap.md` 仍以“阶段 1 进行中、旧 compile_demos 77/77”为叙述核心，部分结论已过时。
3. `DEMO/README.md` 仍保留一些“99_spec 已纳入编译测试 / 文件数 / 覆盖状态”的旧表述，需要按**当前 IR-only 门禁**重写。
4. `issues/2026-08-06-block-demo-problems.md`、`DEMO/Problems/*.md` 这类文档记录了真实问题，但尚未形成**统一的 IR 翻译失败台账**。

### 当前缺口可分为两类

#### A. 前端 / IR 构建缺口（语法有 spec，但 parse / AST / builder 还没完全接上）

优先参考现有文档：`IR/frontend-gap-plan.md`、`issues/2026-08-06-block-demo-problems.md`。

当前高优先候选项：

- `for ... if ...` / `while ... if ...` 守卫及 `else` 子句
- Dict / Set 推导式
- 顶层构建块（尤其 `=:`）
- checker block / block trigger 相关语法（`[ps]` / `[chk]` / `^:` / `[(expr)]`）
- type alias 的完整前端 + IR 支持
- `_ = expr` 丢弃语句
- `~` 后缀命名参数糖
- 模块级 magic 属性 / shebang 属性宏 / comptime 块的补齐
- 泛型约束与 where bound 的前端承载

#### B. IR → Rust 降低缺口（IR 已能表示，但生成的 Rust 仍不完整或不正确）

优先参考现有文档：`README.md` 差距清单、`IR/migration-roadmap.md`、`src/ir/codegen.rs` 中 TODO/unsupported 分支。

当前高优先候选项：

- `__call__` 的 IR 降低与调用重写
- 魔法方法对应 trait / helper lowering
- `defer` 的 RAII 化，而不是仅展开/注释化
- variadic 参数与调用打包
- 顶层构建块的 Rust 发射
- bridge / export / nested fn lifting 的迁移
- `yield` / generator lowering 的稳定 Rust 方案
- checker block 到 Rust helper/fn 的发射一致性

---

## 红线 / 约束

### 1. 严格 IR-only

后续实现必须遵守：

- **允许改 AST**：仅作为前端承载语法的中间层。
- **不允许改旧 `src/codegen/**` 来补功能**。
- **不允许新增任何 AST → Rust 的测试、验证、旁路发射器**。
- 新语法的完成定义是：
  - lexer/parser/AST 能承载；
  - `src/ir/builder.rs` 能正确构造 LZIR；
  - `src/ir/codegen.rs` 能从 LZIR 正确生成 Rust；
  - 活跃测试门禁只看 IR 路线。

### 2. 文档以“IR 路线真实状态”为准

所有文档修复都必须以以下事实为基准：

- “默认编译路径是否为 IR” 以 `src/main.rs` 为准；
- “某语法是否已支持” 以 `--emit=ir`、IR 快照测试和定向 demo 为准；
- “Rust 翻译是否正确” 以 **IR codegen 产出的 `.rs` 是否能通过 `rustc` / 语义是否对齐** 为准；
- 一切过时的 AST 路线描述必须降级为 legacy / reference。

---

## 总体实施策略

分成 4 个阶段推进，每个阶段都只围绕 IR 管线做增量闭环。

### 阶段 0：建立单一事实源（先修文档和门禁，不先堆代码）

#### 目标

把“当前到底哪些功能未实现、哪些 demo 只是文档写错、哪些是 IR 能出但 Rust 降低错了”先分清。

#### 步骤

1. 建立一份**统一缺口总表**（建议放在 `IR/` 或 `issues/` 下）：
   - 类别：`frontend-missing` / `ir-build-missing` / `ir-rust-lowering-wrong` / `doc-stale`
   - 每项记录：来源 `.lz` 文件、触发命令、当前现象、期望行为、涉及源码文件
2. 核对并修正文档事实冲突：
   - `README.md`
   - `IR/migration-roadmap.md`
   - `DEMO/README.md`
   - `issues/2026-08-05-tech-debt-compile-demos-ast-rust.md`（补充“已隔离”后续状态）
3. 把 `DEMO/Problems/*.md` 和 `issues/*.md` 中分散的问题条目，归并到统一台账中，避免重复/冲突叙述。
4. 为“`.lz` 未正确翻译成 Rust”的问题单独定义状态机：
   - `ParseFail`
   - `IRBuildFail`
   - `RustEmitFail`
   - `RustCompileFail`
   - `RuntimeMismatch`
   - `DocOnly`（实际代码已修，文档未更新）

#### 预期产出

- 仓库内存在一份**可信、可持续维护**的 IR 路线总计划 / known-issues 文档。
- 所有后续开发按这份台账排序，不再凭记忆推进。

---

### 阶段 1：先补“前端 → IR”缺口，再补“IR → Rust”缺口

> 原则：**先让语法进入 IR，再谈 Rust 发射**。否则会在旧 AST 路径上产生补丁冲动。

#### 1A. 前端 / IR builder 优先级（P0）

优先补这些，因为它们同时影响 spec 文档、demo 表达能力和后续 lowering：

1. `for/while guard + else`
   - 入口：`src/parser/stmt.rs`
   - 承载：`src/ast/stmt.rs`
   - 降低：`src/ir/builder.rs`
   - 验证：`DEMO/99_spec/guard_for_*.lz` + 组合 demo 的 `--emit=ir`

2. 顶层构建块 `=:`
   - 入口：`src/parser/parser.rs` / 构建块解析逻辑
   - 承载：模块级 AST 项
   - 降低：`src/ir/builder.rs` → `Item::Const` / 顶层 let-like item / helper item
   - 说明：这是 README 差距清单中直接列出的关键缺口

3. checker block / trigger 语法
   - 入口：`src/parser/stmt.rs` / `src/parser/parser.rs`
   - 承载：`src/ast/stmt.rs`
   - 降低：`src/ir/builder.rs` 已有 `Item::CheckerBlock` 痕迹，需补齐完整语义
   - 文档来源：`issues/2026-08-06-block-demo-problems.md`

4. Dict / Set 推导式
   - 入口：`src/parser/expr.rs`
   - 承载：`src/ast/expr.rs`
   - 降低：`src/ir/builder.rs`

5. type alias 全链路
   - 入口：parser 顶层分支
   - 承载：AST module / stmt / item
   - 降低：`src/ir/builder.rs`
   - 发射：`src/ir/codegen.rs`（已有 `Item::TypeAlias` 生成逻辑，可继续完善）

#### 1B. 前端 / IR builder 次优先级（P1）

- `_ = expr` 丢弃语句
- `~` 后缀命名参数糖
- 模块级 magic 属性
- `#!` 属性宏
- `comptime:` 块
- 泛型 bounds / where 约束
- 多行 struct ctor / 复杂调用形态

#### 每个特性的固定实现模板

每补一个特性，都必须按同一模板完成：

1. 语法规范确认（先看 `SYNTAX/` 文档）
2. parser/AST 承载
3. IR 节点映射设计
4. `build_ir` 落地
5. `--emit=ir` 测试
6. 再进入 Rust lowering
7. 文档状态更新（从“未实现”改成“IR 已支持 / Rust lowering 已支持”）

---

### 阶段 2：补 IR → Rust 的关键语义降低

> 这一阶段解决“`.lz` 能进 IR，但生成的 Rust 不对”问题，是你第 2 条要求的核心。

#### 2A. P0：直接影响可编译性的 lowering

1. `__call__`
   - 现状：builder 已把部分实例调用改写为 `MethodCall(__call__)` 痕迹；codegen 还需要稳定发射规则
   - 目标：统一为 trait/helper 调用，不再依赖 Rust 原生 `()` 直接调用非函数值
   - 关键文件：`src/ir/builder.rs`、`src/ir/codegen.rs`

2. 魔法方法 lowering
   - 目标：把 `MagicCall` / `MethodCall("__xxx__")` 统一映射到稳定 Rust 写法
   - 包括：`__getitem__`、`__setitem__`、`__iter__`、`__next__`、`__str__`、`__eq__`、算术魔法等
   - 关键文件：`src/ir/codegen.rs`、必要时 `src/bridge/**` / `src/magic/**`

3. `yield` / generator lowering
   - 目标：禁止直接输出 Rust `yield` 关键字；改为稳定 Rust 可编译模型
   - 需要先选定统一策略：
     - 轻量 `Vec` / iterator wrapper 模拟
     - 或显式状态机结构
   - 约束：所有生成器语义必须由 IR codegen 完成，不能回退旧 codegen

4. `defer`
   - 目标：从“builder 内联 / codegen 注释占位”提升为可运行语义
   - 推荐方向：RAII guard / drop hook

5. variadic
   - 目标：参数定义、调用打包、默认参数/命名参数的交互一次理顺
   - 关键文件：`src/ir/node.rs`、`src/ir/builder.rs`、`src/ir/codegen.rs`

#### 2B. P1：结构性 lowering 缺口

- 顶层构建块的 Rust 发射
- bridge / std 映射迁移到 IR 后端
- `@export(...)` 到 Rust/Python 后端的 IR lowering
- 嵌套函数提升（nested fn lifting）
- checker block 到 Rust helper/fn 的可编译输出
- 装饰器类 intrinsic（`@parallel` / `@tail_call` / `@memoize` 等）的统一 lowering

#### 阶段 2 的实现原则

- 如果某特性 IR 已能表示，但 Rust 后端暂时无稳定语义，**宁可明确标为 known gap**，也不要偷偷补到旧 `src/codegen/**`。
- `src/ir/codegen.rs` 中的 `TODO` / `unsupported expr` / `<stmt todo>` 分支要逐步消零，并每次同步台账。

---

### 阶段 3：建立 IR-only 的 Rust 翻译验证与文档闭环

> 这一阶段专门解决“哪些 `.lz` 还没正确翻译成 Rust”这个问题的**持续维护机制**。

#### 目标

把“翻译是否正确”从零散手工判断，变成**固定测试 + 固定文档**。

#### 新增/调整验证方式

1. 保持现有 IR 门禁：
   - `tests/ir_snapshots.rs`
   - `tests/reject_errors.rs`

2. 新增一套 **IR-only Rust 发射验证**（不要复活 deprecated compile_demos）：
   - 建议新增：`tests/ir_rust_codegen.rs`
   - 流程：
     1. 调用 CLI（默认 IR 或 `--ir-codegen`）生成 `.rs`
     2. 用 `rustc` 编译生成物
     3. 记录失败分类：emit 失败 / rustc 失败 / 输出不符
   - 该测试只验证 **IR 生成的 Rust**，完全不碰 AST 路径

3. 对复杂 demo 增加“运行结果对齐”子集：
   - struct / enum / trait / magic / generator / guard / build-block 各至少挑 1~2 个
   - 只做精选样本，不必一开始就全量执行

#### 文档修复目标

把以下文档统一改为“IR-only 现状表达”：

- `README.md`
- `IR/README.md`
- `IR/migration-roadmap.md`
- `IR/frontend-gap-plan.md`（只保留仍未完成项，删掉已实现但未更新的误判）
- `DEMO/README.md`
- `DEMO/Problems/*.md`
- `issues/*.md`
- 必要时新增一份统一文档，例如：
  - `IR/known-rust-lowering-gaps.md`
  - 或 `issues/2026-08-xx-ir-rust-translation-status.md`

#### 统一文档格式建议

每个“翻译失败”的条目固定包含：

- `.lz` 文件路径
- 失败阶段（Parse / IR / Emit / rustc / runtime）
- 复现命令
- 当前现象
- 期望 Rust 语义
- 对应源码入口
- 状态（Open / Fixed / Doc stale）
- 修复版本/日期

这样文档才能真正服务第 2 项目标，而不是变成零散备忘录。

---

### 阶段 4：彻底收口 legacy 路径

> 只有在阶段 1~3 达到门槛后才做。

#### 进入条件

- 主要语法特性都已能稳定 `--emit=ir`
- IR → Rust 的主 demo 子集可通过 `rustc`
- 统一 gap 文档中的 P0/P1 项显著下降
- 团队不再依赖 AST codegen 做任何日常验证

#### 动作

1. 将 `--ast-codegen` 明确标为 legacy / deprecated。
2. README 和 CLI 帮助中移除“旧路径仍是主线”的残留话术。
3. 将 `src/codegen/**` 彻底降级为参考实现或迁移到归档目录。
4. 所有新特性开发约束写入文档：
   - “若未打通 IR builder + IR codegen，不算实现完成”。

---

## 建议的具体排期顺序

### 第一批（建议先做）

1. **文档事实校正**
   - 先修 `README.md` / `IR/migration-roadmap.md` / `DEMO/README.md`
   - 目的：让仓库先说真话

2. **前端 P0**
   - `for/while guard + else`
   - 顶层构建块
   - checker block / trigger
   - Dict / Set 推导式
   - type alias

3. **Rust lowering P0**
   - `__call__`
   - `yield` / generator
   - `defer`
   - 魔法方法核心集合

4. **IR-only Rust 验证测试**
   - 新增 `tests/ir_rust_codegen.rs`
   - 用它替代历史 `compile_demos` 价值

### 第二批（在第一批稳定后）

- `_ = expr`
- `~` 命名参数糖
- `comptime:`
- 模块级 magic / `#!` 属性宏
- 泛型 bounds / where
- variadic
- bridge / export / nested fn lifting

---

## 可能改动的核心文件

### 前端 / IR

- `src/parser/parser.rs`
- `src/parser/stmt.rs`
- `src/parser/expr.rs`
- `src/ast/stmt.rs`
- `src/ast/expr.rs`
- `src/ir/node.rs`
- `src/ir/builder.rs`
- `src/ir/display.rs`
- 必要时：`src/lexer/**`

### Rust 后端

- `src/ir/codegen.rs`
- 必要时：`src/ir/types.rs`
- 必要时：`src/bridge/**`
- 必要时：`src/export/**`
- `src/main.rs`（只做 CLI / 文案 / legacy 标识调整，不做 AST 回退增强）

### 测试

- `tests/ir_snapshots.rs`
- `tests/reject_errors.rs`
- 新增建议：`tests/ir_rust_codegen.rs`
- 保持：`tests/deprecated/compile_demos.rs` 继续隔离

### 文档

- `README.md`
- `IR/README.md`
- `IR/migration-roadmap.md`
- `IR/frontend-gap-plan.md`
- `DEMO/README.md`
- `DEMO/Problems/*.md`
- `issues/*.md`
- `fixed/*.md`

---

## 验证策略

### 每个特性最少 3 层验证

1. **Parse / IR 验证**
   - `cargo test --test mod`
   - 或对单个 `.lz` 执行 `cargo run -- <file> --emit=ir`

2. **Rust 发射验证（IR-only）**
   - `cargo run -- <file>`（默认 IR）或 `--ir-codegen`
   - 再用 `rustc` 编译生成的 `.rs`

3. **必要时运行结果验证**
   - 对关键 demo 执行目标二进制并比对输出

### 阶段门槛建议

- 阶段 1 完成门槛：对应 demo 全部能 `--emit=ir`
- 阶段 2 完成门槛：对应 demo 生成的 `.rs` 能通过 `rustc`
- 阶段 3 完成门槛：文档中的 known gaps 与测试结果一致，无“文档说已支持、代码实际未支持”现象

---

## 风险与取舍

### 风险 1：文档远比代码更“旧”

这不是坏事，但意味着**先做文档审计**能显著减少误判。当前仓库已有多份计划文档，部分内容互相冲突；如果不先统一，会不断重复走弯路。

### 风险 2：某些语法其实已部分实现，但只在 builder 或 codegen 某一侧断掉

因此必须把缺口拆成：

- parser/AST 未实现
- IR builder 未实现
- Rust lowering 未实现
- 文档陈述过时

不能笼统写成“该语法未实现”。

### 风险 3：为了尽快通过 demo，容易诱发旧 codegen 补丁

必须明确禁止。凡是只能通过 `src/codegen/**` 修好的问题，都不算本计划内的有效修复。

### 风险 4：生成器 / defer / magic / variadic 这类语义会牵动较多 lowering 规则

这几类建议按“先可编译，再语义对齐，再做优雅重构”的顺序推进，不要一开始就大改架构。

---

## 最终建议

如果按收益 / 风险比排序，我建议按下面的顺序执行：

1. **先统一文档事实**（尤其 `README.md` 与 IR 路线说明）
2. **补前端 P0 特性，让语法稳定进入 IR**
3. **补 IR → Rust 的 P0 lowering（`__call__` / generator / defer / magic）**
4. **新增 IR-only Rust 翻译测试，形成“代码 + 文档”双闭环**
5. **最后再收口 legacy AST 路径**

---

## 本计划对应的第一轮落地任务（建议）

如果下一步开始执行，建议第一轮只做这 5 件事：

1. 修 `README.md` 中“默认仍走 AST 路线”的错误陈述
2. 新建统一的 IR gap / Rust lowering status 文档
3. 实现 `for/while guard + else` 的 parser → AST → IR builder
4. 实现顶层构建块 `=:` 的 parser → IR builder → Rust lowering
5. 新增 `tests/ir_rust_codegen.rs`，只验证 IR 发射的 Rust

这 5 件完成后，项目就会从“方向正确但状态分散”，进入“IR-only 路线有明确门禁和推进顺序”的阶段。

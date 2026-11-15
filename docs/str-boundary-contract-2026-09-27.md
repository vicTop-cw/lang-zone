# str / &str / String 边界契约与收敛记录（2026-09-27）

> 设计记录（非规范文档）。规范仍是 `SYNTAX/*.md`：`01-类型系统` 规定 `str → String`，
> `00-词法基础` 规定 f-string → `format!("x={}", x)`。本文只记录 **codegen 侧的形态契约**
> 与本次收敛的范围、证据与遗留，不改语义、不改运行时 ABI。

## 一、问题定位

LZ 只有**一个**字符串类型 `str`，但它在 Rust 侧落在四种文本形态上，而
`lz_builtins` 的 ABI 是固定的一对：**入参 `&str`，出参 `String`**。
历史上每个发射点各自即兴处理形态转换，`src/ir/codegen/mod.rs` 内散出数百处
`to_string()` / `as_str()` / `&` 拼装，彼此不一致：

- `const str` 生成 `&'static str`，而普通绑定生成 `String`；
- `ref x: str` 形参生成 `&str`，但 IR 里其类型仍是 `Str`（多处据此判 `String` → E0308）；
- StrExt 实参同时存在「补 `.to_string()`」与「剥 `&`」两种相反处理；
- 值语义实参克隆对字符串统一 `.clone()`，对 `&str` 只得到 `&str`（E0308 隐患）。

结论：缺的是**唯一的形态归一化点**，而不是更多补丁。

## 二、契约（冻结）

| 来源形态 \ 目标要求 | `Owned` | `Borrow` | `ExactStrRef` | `Display` |
|---------------------|---------|----------|---------------|-----------|
| `Owned`（`String`：绑定/字段/调用结果/插值结果） | 原样 | `&x` | `x.as_str()` | 原样 |
| `StrRef`（`&str`：`ref str` 形参 / `Ref(Str)` 局部 / 切片 / 字面量以外的借用） | `x.to_string()` | 原样 | 原样 | 原样 |
| `StringRef`（`&String`：`ref self` 字段 / 自动加 `&` 的 String 实参） | `x.clone()` | 原样 | `x.as_str()` | 原样 |
| `Static`（`&'static str`：字符串字面量 / `const str`） | `x.to_string()` | 原样 | 原样 | 原样 |

三条铁律（实现见 `src/ir/codegen/str_boundary.rs` 模块级文档）：

1. **值域唯一**：LZ `str` ≡ Rust `String`；`&str` 只出现在「借用视图」与「Rust 侧 ABI」两处。
2. **方向唯一**：`&String → &str` 靠 Rust deref coercion（实参位零成本）；`&str → String` 只在
   `Owned` 目标处发生；不允许反向即兴处理。
3. **职责单一**：E0034 的方法解析强制限定（`<StrExt>::lz_*(self, ..)`）与「`{}` vs `{:?}`」
   选择不属形态层职责。

**保守借用假设（重要）**：IR 里 `Str` 表示 LZ 的 `str`，但其渲染文本可能是 `&str`/`&String`
（`ref` 形参、`&self` 字段、`const` 等）。凡「不确定渲染形态」的位置，调用方按 `StrRef`
传入（`to_string()` 对三种文本都正确）；只有**确认为拥有值绑定**时才用 `Owned`。

## 三、本次收敛（阶段①/②/③）

### 阶段① 契约层骨架（零行为变更）

- 新增 `src/ir/codegen/str_boundary.rs`（329 行）：契约文档 + `StrForm` / `StrWant` +
  `coerce()` + `form_of_ir` / `form_of_lit` / `form_of_expr` / `is_str_ir`；模块内 5 个单测
  （含 **4×4 契约矩阵**全格断言）。
- `mod.rs` 仅新增 `mod str_boundary;` 与 `use`（无调用点）→ 生成物逐字节不变。
- 顺带消除一处既有 `unreachable pattern` 警告：`Add` 约束统一映射到
  `lz_builtins::LzAdd`（与函数体 `LzAdd::__add__` 一致）。

### 阶段② 分批迁移（每批跑闸门）

已迁移到 `coerce()` 的 11 个接入点（`str_coerce(` 计数）：

| 位置 | 迁移内容 |
|------|----------|
| 返回位 `return self`（3 处） | str 方法的 `self` 是借用视图 → `to_string()` |
| `clone_if_multiuse` str 分支 | 复制形态由形态层给出（保守借用假设） |
| 方法 `clone` on str 接收者 | 同上 |
| 列表字面量 str 元素 | 容器元素位拥有化 |
| StrExt 实参拥有化 | **取代「以 `"` / `&` 文本前缀判断」的启发式**，改由 `form_of_expr` 判定 |
| 字符串比较两侧 | 两侧拥有化（保守借用假设，注释说明原因） |
| Pattern/借用位字符串实参 | `Owned → Borrow` 取引用（容器参数仍直接 `&`） |

**顺带修复的真实缺陷**（由专项闸门发现并锁定）：

- 值语义实参克隆对字符串统一 `.clone()`：`ref s: str` 形参（渲染为 `&str`）传
  `String` 形参时 `.clone()` 仍是 `&str` → **E0308**。已改为按形态层拥有化
  （`StrRef → Owned` = `to_string()`）。

### 阶段③ 专项闸门

- 新增 `tests/str_boundary.rs`：**27 个用例 / 3 个测试**（转译 + rustc 真编译 + 运行校验），
  覆盖：字面量→拥有形参、拥有→`ref str` 形参、`ref str`→拥有形参、`ref self` 字段返回、
  `__str__`、字段作实参、`raises` 的 `Result<str, str>`、str 字段读写、`Dict<str,_>` 键、
  `Set<str>`、`Option<str>`（Some/None 两路）、`List<str>` 索引、`join`、元组内 str、
  `const str`、`push`、`str+str`、`str(int)`、`==`/`<`、单字符索引码点、切片、`for-in`、
  f-string（Display 语义，与规范一致）、Pattern 方法（trim/starts_with/ends_with/replace/
  split/contains）、泛型 `T` 用 str 实例化、闭包取 str、`List<str>` 排序。

## 四、量化（`src/ir/codegen/mod.rs`，仅代码行、排除注释）

| 指标 | 迁移前 | 迁移后 |
|------|-------:|-------:|
| 形态判定接入点（`str_coerce(`） | 0 | **11** |
| StrExt 实参文本前缀启发式 | 1 处 | **0**（改 `form_of_expr`） |
| 文档化保留的启发式 | — | 7 类（见下） |

> 说明：`to_string()` / `as_str()` / `&str` 的**文本出现次数不降反平**（202/172/68），
> 因为本次收敛的是「**判定集中化**」而不是「文本替换」：产物文本本就必需这些调用。
> 判定点的数量（0 → 11）与启发式数量（StrExt 文本判定 1 → 0）才是收敛指标。

## 五、有意保留的启发式（改动前请先读）

| 保留项 | 原因 |
|--------|------|
| `str_typed_vars` / `is_str_expr`（f-string `{}` vs `{:?}`） | 需跨语句状态，输入是渲染后文本；且 Display/Debug 选择不属形态层 |
| `expr_is_clone_of_string` | 这类实参 IR 类型常退化为 `Any`（`form_of_expr` 返回 `None`） |
| `is_str_producing` | 需 `fn_returns` 表 + 方法名白名单的上下文推断 |
| `need_ref` 接收者归属判定 | 需要方法解析知识（std `Vec::contains` vs 自定义 `contains`、Dict `get`、kwargs） |
| 文本层剥离（`trim_end_matches(".to_string()")`） | 输入是已发射文本，且与管线顺序耦合 |
| 泛型/关联类型位的 clone 抑制（`I::Item` 无 `Clone`） | 依赖泛型约束，形态层不可见 |
| E0034 强制限定 UFCS | 决定「调用哪个方法」，不属形态层 |

**管线顺序注意**：文本可能已被前置 `&`（`need_ref` / 自动取引用先跑）或已带 `.clone()`。
迁移时必须在边界层之前先处理这两类「已归一」文本，否则会出现 `&String` 传给 `String`
形参（本次 StrExt 迁移中已踩到并按此处理）。

## 六、验收证据（2026-09-27）

| 闸门 | 结果 |
|------|------|
| `cargo test -j 1 --test str_boundary` | **3 passed / 0 failed**（27 用例） |
| `cargo test -j 1 --lib str_boundary` | **5 passed / 0 failed**（含 4×4 契约矩阵） |
| `cargo test -j 1 --test demo_codegen_compile -- --include-ignored` | **2 passed / 0 failed** |
| `cargo test -j 1 --test find_bug_bugs` | **47 passed / 0 failed** |
| `cargo test -j 1 --test moddec_corpus` / `moddec_product_assert` / `ir_snapshots` | 2 / 5 / 8 passed，0 failed |
| 生成物确定性 | 89 个 `DEMO/**/*.rs` 二次生成哈希零差异（基线清单 `.fist-wf-20260926/str_parity_baseline.txt`） |

> 所有 cargo 命令均 `-j 1`（全量并行会因 Windows 页文件不足报 os error 1455 假错）。

## 七、后续建议 —— 执行结果（2026-09-27 第二轮）

### ✅ 已执行 3：值语义与形态解耦

新增 `str_boundary::owned_copy(s)`：只回答「复制成什么形态」（字符串一律按保守借用假设
→ `to_string()`），「要不要复制」仍由调用方按 move 语义（`fn_use_count`/索引取出/元素绑定）
判定。原 5 处（`clone_if_multiuse` str 分支、值语义实参块、列表字面量元素、方法 `clone`
on str、字符串比较两侧）全部改为调用它，形态判定收敛为单一入口；配套单测
`owned_copy_is_form_only`。

### ✅ 已执行 2：f-string 的字符串判定归一边界层（部分）

`str_typed_vars` 的两处**登记**改用 `is_str_ir`（覆盖 `Str`/`String`/`&str`/`&String`），
取代原先手写的 `matches!(ty, Str | Ref(Str))`。**未做**的部分及其原因（实测）：
`gen_fstring(&self, s: &str)` 的入参是 **f-string 原始文本**（`LitKind::FStr(String)`），
插值槽位按变量名查表，**拿不到 `Expr`**，故 `{}` vs `{:?}` 无法由 `form_of_expr` 决定；
`is_str_producing`（`join`/`to_upper` 等链式产出，IR 类型退化为 `Any`）也仍需保留。
若要彻底退役 `str_typed_vars`，需先让 IR 的 FStr 带结构化插值槽位（`Vec<Expr>`），
属独立改造。

### ⛔ 已评估 4：Pattern 位**不做** blanket `as_str()`

实测证据：`let s: &str; s.as_str();` → **E0658 `use of unstable library feature str_as_str`**
（`as_str()` 只存在于 `String`/`&String`，`&str` 形态上不可用）。而该位点的实参形态可能是
`&str`（`ref str` 形参直接透传），改 `ExactStrRef` 会引入 E0658/E0599 回归。
故保持 `Borrow`（`&x`）：`&String: Pattern` 成立、`&str` 经 deref coercion 亦成立；
`ExactStrRef` 作为**形态层已备好但当前无安全落点**的目标保留（仅可用于「确认拥有值」的位点）。

### ✅ 已执行 1（纯 codegen 路线 b）：`StrExt` 只读字符串形参族按 `&str` 统一

在不改 LZ std 源码的前提下，对**生成的 `StrExt`**（`impl str`）把「只读字符串形参族」
渲染为借用视图 `&str`，与 `lz_builtins::StringExt` 的运行时 ABI 对齐：

- 白名单（逐个体检过方法体，形参仅读不 move）：`contains / starts_with / ends_with /
  find / rfind / replace / split`；未收录 `__add__(other: String)`（拼接需拥有值）、
  `__eq__(other: &str)`（源侧即 `ref str`）、`char_at/slice*/repeat/trim*`（无 owned `str` 形参）。
- 三处同步改动（保证 E0276 一致）：trait 声明侧（`gen_param`）、impl 侧（`gen_fn_def`
  形参渲染）、两处调用点（`<str as StrExt>::` 显式限定 + ext-trait 方法体内 `self.lz_*`
  自调用，统一经 `str_ext_borrow_args`）。
- 实参归一按**文本判定 + 形态还原**：借用视图的文本可能已被前置流程
  （`clone_if_multiuse` 的 str 分支）归一为 `.to_string()`，此时按形态层判定剥掉该
  拥有化，直接以借用视图传参。

生成物效果（`DEMO/lz_std/string.rs`）：

| 位置 | 统一前 | 统一后 |
|------|--------|--------|
| 声明 | `fn split(&self, delimiter: String)` | `fn split(&self, delimiter: &str)` |
| 字面量实参 | `split(self, "\n".to_string())` | `split(self, "\n")`（零分配） |
| 模式/谓词实参 | `find(&(..), "{}".to_string())` | `find(&(..), "{}")`（零分配） |
| 借用视图实参 | `starts_with(self, other.to_string())` | `starts_with(self, other)`（零分配） |
| owned 实参 | `find(self, substr)` | `find(self, &substr)` |

**顺带修复的既有缺口（由新用例 C12 暴露）**：接收者判定两处都漏了 `Ref(Str)`——
`ref s: str`（借用视图）接收者上调用 `contains/starts_with/find/replace/split` 时：
① `pattern_methods` 分支不生效 → 字面量实参被拥有化成 `String` → **E0277 `String: Pattern`**；
② StrExt 强制限定不生效（在含 StrExt 的编译单元里）→ 落到 std 固有方法，`find` 返回
`Option<usize>`、`replace` 需 `Pattern`，与 LZ std 语义不符（E0308）。
两处判定均补上 `Ref(Str) | MutRef(Str)`；新增锁定用例 C12。

验收：全量闸门 2/0、`str_boundary` 3/0（**28 用例**）、`find_bug` 47/0、`moddec_corpus` 2/0、
`moddec_product_assert` 5/0、`ir_snapshots` 8/0、lib 单测 6/0。

### ⏭ 遗留（第二轮之后的剩余项）：`StringExt(&str)` 与 `StrExt(String)` 双形态统一（需范围决策）

实测现状（证据齐备）：

| 侧 | 位置 | 形参 | 方法名 |
|----|------|------|--------|
| 运行时 | `lz_builtins::StringExt`（`flat.rs:809`、`runtime/collections.rs:309`） | `&str` | `lz_contains/lz_split/...`，impl for `String`+`str` |
| 生成物 | `trait StrExt`（由 LZ std 生成，见 `DEMO/lz_std/string.rs:25`） | `String`（`__eq__` 因 `ref str` 而是 `&str`） | `contains/starts_with/...`，impl for `str` |

LZ std 源码 `DEMO/lz_std/string.lz` 的签名是 owned：`def contains(ref self, substr: str) -> bool`
→ 生成 `String`；调用点由 `<str as StrExt>::` 强制限定，全仓生成物中 **22 处**。

两条可行路线：

- **(a) 改 LZ std 签名**（`substr: str` → `ref substr: str`）：生成 `&str`，调用点去掉
  `.to_string()` 改传 `&x`（省一次分配，且与运行时 ABI 一致）。需同步 trait 声明/impl 两端
  （E0276 一致性）+ 全部 22 个调用点 + `string.rs` 快照。**需要「允许改 `DEMO/lz_std/*.lz`」
  的范围授权**。
- **(b) 纯 codegen 映射**：对 **ext-trait 方法**的 owned `str` 形参渲染为 `&str`，并在调用点
  传 `&`。要求先做「形参在方法体内未被 move」的分析（否则 `return substr` 等位点会
  E0308），属中等风险批次，但不动 LZ 源码。

建议：优先 (a)（更直白、改动可审），(b) 作为不允许改 LZ std 时的替代。两者的验收口径与
本轮一致（全量闸门 + `str_boundary` 专项闸门 + 快照类）。

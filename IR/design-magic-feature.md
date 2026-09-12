# LZ `magic` 魔法特性 —— 设计（v1 · 待评审）

> 日期：2026-09-08
> 定位：**语言设计文档**。本文先确立 `magic` 的准确语义与联动模型，据此再修订 `SYNTAX/` 语法文档，最后落到实现。
> 相关：[`IR/magic-methods-completion-plan.md`](magic-methods-completion-plan.md)（补齐计划）、[`IR/design-magic-init-priority.md`](design-magic-init-priority.md)（构造器/隐式转换）

---

## 0. 为什么写这份文档

`magic` 的语义在现有文档里**自相矛盾**，并且已经传导到实现，导致实测出问题。

### 0.1 文档内部的三套说法

| 出处 | 说法 | 与本文语义 |
|---|---|---|
| `06f §四`、`06g §一/§六` | **顶层** `magic name<T> =` 定义契约（Trait + 全局函数），struct 内 `def __xxx__` 是实现 | ✅ 一致 |
| `06f §六`「内联 magic（struct 内）」 | `struct` 内可直接写 `magic __new__(...) = ...`，是 magic 块的"语法糖" | ❌ 冲突 |
| `06g §8.2`、`06d §九`、`06a §六` | 示例把 `magic __xxx__` 写在 struct 内当实现用 | ❌ 冲突 |

### 0.2 已造成的实现后果（实测）

- 按 `06f §六`/`06d` 在 struct 内写 `magic __eq__` → 只生成普通固有方法 `fn __eq__`，**不生成任何 trait impl**，运算符完全不接通。
- 按 DEMO 写法用 `impl X =` + `def __eq__` → 生成 `impl std::cmp::PartialEq for X`，`==`/`!=` 正常。
- 编译器里"魔法 → trait impl"目前只有 `__eq__` 一个**手写特例**（`src/ir/codegen/mod.rs:3958-4010`），其余魔法方法无 trait impl 生成分支。

**结论**：不是实现偷懒，是**设计没被表达清楚**。本文先把设计钉死。

---

## 1. `magic` 是什么

### 1.1 一句话定义

> **`magic` 是模块级顶层的「魔法特性声明」。一次 `magic` 声明同时完成三件事。**

### 1.2 三件事

```lz
magic Eq =
    def __eq__(ref self, ref other: Self) -> bool = ...
```

这一次声明产出：

| # | 产物 | 上例 |
|---|---|---|
| 1 | **定义一个 trait**，trait 内含魔法函数的签名 | `trait Eq { fn __eq__(...) -> bool }` |
| 2 | **拟定魔法名**（`__xxx__` 形式），作为实现侧的方法名 | `__eq__` |
| 3 | **拟定全局函数**，供统一调用 | `eq(a, b)` |

### 1.3 命名推导

| 元素 | 来源 | 例 |
|---|---|---|
| trait 名 | magic 块名（PascalCase） | `magic Eq` → `trait Eq` |
| 魔法名 | 块内 `def` 的方法名（必须 `__xxx__`） | `def __eq__` → `__eq__` |
| 全局函数名 | magic 块名（同名） | `magic Eq` → 全局函数 `eq` |
| 泛型参数 | 跟随 magic 块声明 | `magic Map<T, R>` → `map<T, R>(...)` |

> 声明式配置可显式覆盖：`trait = "..."` / `method = "..."`（见 §3.2）。

---

## 2. `magic` 不是什么（纠偏 · 强约束）

| 约束 | 说明 |
|---|---|
| **① `magic` 只在模块级顶层** | 不允许出现在 `struct` / `enum` / `impl` / `trait` / 函数体 / 任何缩进块内。写在别处 = 编译错误。 |
| **② `magic` 是声明，不是实现** | 实现一律用 `def __xxx__`（`impl` 块内 或 struct 内部均可）。 |
| **③ 不存在"内联 magic"** | 删除 `06f §六` 整套概念。struct 内写 `magic` 不是语法糖，是**错误**。 |
| **④ 一个 magic 定义一个魔法特性** | 不是"给这个 struct 加个魔法方法"。 |

**错误示例（应报编译错误）**：

```lz
struct Point =
    x: int
    y: int
    magic __new__(x: int, y: int) -> Point = ...   // ❌ magic 不得写在 struct 内
```

**正确写法**：

```lz
// 顶层声明魔法特性（通常来自标准库，用户一般不需要自己写）
magic New =
    def __new__(...) -> Self = ...

// struct 内 / impl 内实现
struct Point =
    x: int
    y: int
    def __new__(x: int, y: int) -> Point =
        Point(x: x, y: y)
```

---

## 3. 语法

### 3.1 方法定义式（推荐）

```lz
magic Name<泛型...> =
    def __magic__(self模式, 参数...) -> 返回类型
        where Self : 约束
        where 泛型 : 约束
        = ...          // 抽象：实现方必须提供
        // 或 = expr   // 默认实现
```

self 模式四种（沿用 `06g §一`）：

| 写法 | 模式 | Rust |
|---|---|---|
| `def __x__(self, ...)` | owned | `self` |
| `def __x__(ref self, ...)` | ref | `&self` |
| `def __x__(mut self, ...)` | refmut | `&mut self` |
| `def __x__(...)`（无 self） | none | 关联函数 |

### 3.2 声明式配置（覆盖推断）

```lz
magic __map__:
    trait = "Map"
    method = "map"
    self = "owned"
    dispatch = "by_ret"
    ret = "assoc"
    tuple = "false"
```

仅用于覆盖编译器推断；与方法定义式**不可混用**于同一块。

### 3.3 联动声明（新增，见 §5）

magic 块内可额外声明**编译器级联动**（trait 默认实现表达不了的部分）：

```lz
magic ImplicitConvert =
    chain = ["__implicit_from__", "__implicit_to__", "__from__", "__into__", "__default__"]

magic ImplicitTo =
    blanket = "ImplicitFrom"      // 由 ImplicitFrom 自动派生

magic IsOk =
    override = ["Option", "Result"]   // 覆盖内建判定（见 §6）
```

**用户级联动字段（见 §5.5 / §5.6）**——除编译器策略外，用户可声明自己的多方法协调单元：

```lz
magic MonitoringLinkage =
    mutex = ["__exec_fix__", "__exec_new__"]   // 互斥：同类型至多实现一个（§5.6）
    linkage:                                   // 用户定义的多方法联动链（§5.5）
        on   __on_anomaly__ -> __notify__
        gate __user_cmd__   -> __exec__
        else  __exec_new__
```

### 3.4 语法边界

- 魔法名必须 `__` 包裹
- magic 块名与全局函数名一一对应
- magic 出现在非顶层 → 编译错误：`E: magic 声明只能出现在模块顶层`
- 同一魔法名重复声明 → 编译错误

---

## 4. 声明 vs 实现：分工

```
┌─ ① magic（模块顶层）── 声明契约 ─────────────┐
│  magic Eq =                                   │
│      def __eq__(ref self, ref other: Self)    │
│          -> bool = ...                        │
│  产物：trait Eq + 魔法名 __eq__ + 全局函数 eq() │
└──────────────────┬──────────────────────────┘
                   ▼
┌─ ② def __eq__（impl 或 struct 内）── 实现 ────┐
│  impl Point =                                 │
│      def __eq__(ref self, ref other: Point)   │
│          -> bool = self.x == other.x          │
│  // 或写在 struct Point = ... 内部，等价        │
└──────────────────┬──────────────────────────┘
                   ▼
┌─ ③ 编译器匹配签名 → 自动生成 impl ────────────┐
│  impl Eq for Point { fn __eq__(...) {...} }   │
│  + 生成 Rust 对应 trait（std::cmp::PartialEq） │
└──────────────────┬──────────────────────────┘
                   ▼
┌─ ④ 全局函数 / 运算符可用 ─────────────────────┐
│  eq(p1, p2)      p1 == p2      p1 != p2      │
└──────────────────────────────────────────────┘
```

**关键点**：
1. `magic` 定义"魔法契约"；`def __xxx__` 提供实现。二者**位置与职责严格分离**。
2. struct 只需定义同签名的 `__xxx__`，编译器自动匹配并生成 impl。
3. `def __xxx__` 写在 `impl` 块内 **或** struct 内部，**语义等价**（现状 DEMO 以前者为主，二者都应支持）。

---

## 5. ★ 联动模型（核心）

> 联动 = 一个魔法特性声明后，编译器应**自动带动**的相关行为；或**一组魔法方法之间的配对/派生关系**。

用户明确指出：联动级 magic 可能需要**多个 trait**、或**一个 trait 要实现多个方法**、或**实现多个魔法方法之一**等等。本节把这几种形态逐一钉死。

### 5.1 联动的七种形态

| # | 形态 | 含义 | 例 |
|---|---|---|---|
| 1 | **单 trait 单方法** | 最简单的魔法 | `__len__` → `HasLen` |
| 2 | **单 trait 多方法（默认派生）** | trait 内多个魔法方法，其中若干有默认实现；实现方只需实现**抽象**的那些，其余自动获得 | `__eq__` 抽象 + `__ne__` 默认 `not __eq__` |
| 3 | **多 trait（继承链）** | 一个魔法特性展开为多个 trait，形成 supertrait 链 | `Ord : Eq + PartialOrd` |
| 4 | **多魔法方法之一** | 实现**任意一个**即可满足契约，其余由编译器合成 | 实现 `__lt__` → 自动得到 `__le__`/`__gt__`/`__ge__` |
| 5 | **配对（blanket 互推）** | 两个魔法互为派生，实现其一即得其二 | `__implicit_from__` ⇄ `__implicit_to__` |
| 6 | **优先级链** | 编译器在特定场景按固定顺序搜索候选 | 真值：`__bool__` → `__len__` → true；隐式转换 5 级链 |
| 7 | **覆盖** | 用户实现覆盖编译器内建行为 | `__is_ok__` 覆盖 `Option`/`Result` 的判定 |

### 5.2 各形态的表达方式

#### 形态 2 & 4：用 **trait 默认实现**表达（推荐，主要机制）

> 与 `DEMO/lz_std/traits.lz` 现有风格一致：**联动写在 trait 的默认方法里**。

```lz
magic PartialOrd =
    // 抽象：实现方必须提供（四选一即可，见下）
    def __lt__(ref self, ref other: Self) -> bool = ...

    // 默认实现：由 __lt__ 派生，实现方无需手写
    def __le__(ref self, ref other: Self) -> bool = not other.__lt__(self)
    def __gt__(ref self, ref other: Self) -> bool = other.__lt__(self)
    def __ge__(ref self, ref other: Self) -> bool = not self.__lt__(other)
```

**效果**（形态 4）：实现方只写 `def __lt__`，`<=` `>` `>=` 自动可用。

同理比较契约：

```lz
magic Eq =
    def __eq__(ref self, ref other: Self) -> bool = ...      // 抽象
    def __ne__(ref self, ref other: Self) -> bool = not self.__eq__(other)   // 派生
```

**编译器职责**：生成 Rust 侧对应 trait 时，把抽象方法映射到目标 trait 的必需方法，把派生方法合成为默认实现（如由 `__lt__` 合成 `PartialOrd::partial_cmp`）。

#### 形态 3：多 trait / 继承链

```lz
magic Ord =
    trait PartialOrd =
        def __lt__(ref self, ref other: Self) -> bool = ...
        def __le__(...) -> bool = not other.__lt__(self)
        def __gt__(...) -> bool = other.__lt__(self)
        def __ge__(...) -> bool = not self.__lt__(other)

    trait Ord: PartialOrd =            // supertrait 链
        def __cmp__(ref self, ref other: Self) -> Ordering = ...
```

一个 magic 块可声明**多个 trait**；用 `trait X: A + B` 表达继承。

#### 形态 5：配对（blanket 互推）

```lz
magic ImplicitFrom =
    def __implicit_from__(source: S) -> Self = ...

magic ImplicitTo =
    blanket = "ImplicitFrom"      // ← 编译器级声明：由 ImplicitFrom blanket 派生
    def __implicit_to__(self) -> T = ...
```

生成（对齐 Rust `From → Into`）：

```rust
impl<T, U: ImplicitFrom<T>> ImplicitTo<U> for T {
    fn __implicit_to__(self) -> U { U::__implicit_from__(self) }
}
```

**规则**：只需实现 `__implicit_from__`；二者**不可同时手写**（避免歧义）。

#### 形态 6：优先级链

```lz
magic ImplicitConvert =
    chain = ["__implicit_from__", "__implicit_to__", "__from__", "__into__", "__default__"]
```

编译器在类型不匹配处按序搜索，命中即止；**单步转换**（禁 `A→B→C` 链），并做**循环检测**。

真值判定链（规范 `06g §九`）：

```lz
magic Bool =
    chain = ["__bool__", "__len__", "true"]     // 三档：__bool__ → __len__>0 → 默认 true
```

#### 形态 7：覆盖（见 §6）

```lz
magic IsOk =
    override = ["Option", "Result"]
    def __is_ok__(ref self) -> bool = ...
```

### 5.3 联动该写在哪？

| 联动类型 | 表达位置 | 理由 |
|---|---|---|
| 方法间派生（`__ne__` from `__eq__`） | **trait 默认实现** | 与现有 `traits.lz` 风格一致，用户可读可覆盖 |
| 多 trait / 继承链 | magic 块内多个 `trait` 声明 | 需要声明式结构 |
| blanket 配对 | magic 块 `blanket = "..."` | 无法用默认实现表达 |
| 优先级链 | magic 块 `chain = [...]` | 属于编译器搜索策略 |
| 覆盖内建 | magic 块 `override = [...]` | 属于编译器内建行为的替换点 |

> **原则**：能用 trait 默认实现表达的，就写在 trait 里（对用户透明、可覆盖）；只有编译器策略层面的（blanket / chain / override）才放进 magic 块的声明字段。

### 5.4 策略型联动（Strategy Family）★

> 前面遗漏的一类：**策略**。策略型联动的本质是——**同一个语义目的，有多种可选的执行方式，由类型作者声明"我支持哪些"，由编译器按优先级/调用点需求择一。**

规范依据：`99-内置预导入库.md §3.2 策略系统`、`06d §十五 守卫策略`、`06d §十八 迭代策略`。

#### 5.4.1 统一抽象

```lz
trait Strategy =
    type Host                    // 策略宿主类型
```

各具体策略 trait 继承 `Strategy`，并**按"是否出现对应魔法符号"条件注入**：

| 策略 trait | 触发魔法 | 签名 |
|---|---|---|
| `CloneStrategy` | `__clone__` | `fn clone_of(&self, src: &Host) -> Host` |
| `DestroyStrategy` | `__drop__` | `fn destroy(&self, target: Host)` |
| `SerializeStrategy` | 序列化魔法 | `fn encode` / `fn decode` |
| `IterStrategy` | `__iter_strategy__` | `fn iterate(&self, src: &Host) -> Box<dyn Iterator<Item = Item>>` |
| `GuardedStrategy` | `__guarded_pred__` + `__guarded_action__` | 见 §5.4.3 |

`StrategyKind` 为策略解析枚举：`Clone` / `Destroy` / `Serialize` / `Iter` / ...

**联动形态**：策略属于 §5.1 形态 2/4 的复合——trait 内多个方法，实现其一（或若干）即可，其余有默认；**且策略的选择是运行/编译期按优先级解析的**（`__iter_resolve()`）。

---

#### 5.4.2 迭代策略 ★（重点补齐）

##### (a) 要解决的问题

Rust 的 `for i in xs` **只有一种默认语义**（`into_iter`）。调用点想换语义只能靠 `&xs` / `&mut xs`，且**类型作者无法声明"我这个类型支持哪些迭代方式"**。具体缺失：

| 需求 | 含义 |
|---|---|
| **修改原有值** | 迭代时拿到可变引用，写回原容器 |
| **一次性** | 消费式迭代，迭代后原值不可再用 |
| **持久** | 可重复迭代，迭代后原值仍可用、可再迭代一次 |
| **引用即可** | 只读借用，不转移所有权 |

**这些正是迭代策略要解决的问题。**

##### (b) 两个正交维度（关键区分）

现有规范 `99-内置预导入库.md:222-234` 的 `IterStrategyKind` 枚举有 9 个变体，但**全部属于"迭代方式"维度**：

| 变体 | 说明 |
|---|---|
| `Forward` | 正向迭代（保持惰性） |
| `Reverse` | 反向迭代（collect 后 `.rev()`） |
| `Strided(i64)` | 步进迭代（`.step_by()`） |
| `Controlled` | 可控迭代（包裹 `Itor`，暂停/跳转） |
| `Collected` | 收集迭代（非惰性，collect 为 Vec） |
| `Historized` | 历史迭代（记录已迭代元素） |
| `Indexed` | 索引迭代（按索引访问） |
| `Chunked(usize)` | 分块迭代 |
| `Infinite` | 无限迭代 |

**缺失的是"所有权 / 持久性"维度**。故拆为两个正交维度：

**维度 B（新增）：`IterBorrow` — 所有权 / 持久性**

| 变体 | Item | 消费原值 | 可重复（持久） | 可改原值 | 对应需求 |
|---|---|:---:|:---:|:---:|---|
| `Owned` | `T` | ✅ 一次性 | ❌ | — | **一次性** |
| `Ref` | `&T` | ❌ | ✅ **持久** | ❌ | **引用即可** |
| `MutRef` | `&mut T` | ❌ | ✅ **持久** | ✅ | **修改原有值** |
| `Controlled` | `T`（可配置） | 可配置 | ✅ | 可配置 | 暂停/恢复/跳转（`Itor`） |

> 说明：**持久性不是独立变体，而是 `consuming` 的推论**——`Owned`（consuming）天然一次性；`Ref`/`MutRef`（借用）天然持久可重复。故策略条目可用四元组描述：`(borrow, kind, consuming, repeatable)`。

**一个迭代策略 = 维度 A（方式）× 维度 B（所有权）的组合**，例如：
- `(Forward, Ref)` — 正向、只读借用（默认首选）
- `(Reverse, Ref)` — 反向、只读借用
- `(Forward, MutRef)` — 正向、可变借用（**可修改原值**）
- `(Forward, Owned)` — 正向、消费（**一次性**）
- `(Controlled, Ref)` — 可控 `Itor`、只读

##### (c) 声明与解析

```lz
magic IterStrategy =
    def __iter_strategy__(ref self) -> List<IterSpec> = ...
```

类型作者按**优先级从高到低**声明自己支持的策略（草案）：

```lz
struct MyVec<T> =
    items: List<T>

    def __iter__(ref self) -> BaseIter<T> = ...          // base 迭代器（既有）

    def __iter_strategy__(ref self) -> List<IterSpec> =
        [ IterSpec(borrow: MutRef, kind: Forward)        // ① 首选：可改原值
        , IterSpec(borrow: Ref,    kind: Forward)        // ② 次之：引用即可
        , IterSpec(borrow: Owned,  kind: Forward)        // ③ 兜底：一次性消费
        ]
```

**解析**：`for-in` 时由 `__iter_resolve(...)` 取**第一个满足调用点需求**的策略，包裹 `__iter__()` 产出的 base 迭代器。

调用点需求来源：
1. 循环变量的绑定模式（是否要可变 / 是否要引用）
2. 循环体内是否对元素赋值（推断需要 `MutRef`）
3. 迭代后原值是否被再次使用（若再用 → 排除 `Owned`）
4. 显式指定（见 (d)）

##### (d) for 语法选择（★已裁定：方案 A + C）

现状 `for x in xs:` 无策略修饰符，且 `05-控制流.md:158` 规定"**循环变量默认可变**"——因此**不能**直接复用 `mut` 关键字表达迭代模式（会与该语义冲突）。

| 方案 | 写法 | 裁定 |
|---|---|---|
| A. 显式模式注解 | `for x in xs @MutRef:` / `@Ref` / `@Owned` / `@Controlled` | ✅ **已采纳** |
| B. 绑定关键字 | `for ref x in xs:` / `for ref mut x in xs:` | ❌ 否决（与"循环变量默认可变"撞语义） |
| C. 纯推断 | 只靠 (c) 的需求推断 | ✅ 作为**默认行为**保留（无注解时） |

**确定语法（A 为主、C 为辅）**：

```lz
for x in xs:              // 无注解 → 推断，取首个适用策略（通常 Ref）
for x in xs @Ref:         // 只读借用（引用即可）
for x in xs @MutRef:      // 可变借用（可修改原有值）
for x in xs @Owned:       // 消费（一次性，迭代后原值不可再用）
for x in xs @Controlled:  // Itor 可控（暂停/恢复/跳转）

for x in xs @MutRef if cond:   // 与守卫共存：@Mode 在前，if 守卫在后
```

> **`@` 与装饰器的冲突**：`@` 已用于装饰器（`@intrinsics`），但语法位置可区分——装饰器在定义/表达式**前**，`@Mode` 在 for 迭代表达式**后**。若实现期发现冲突，退路为等价组合子写法 `xs.mut_iter()` / `xs.ref_iter()`。

方式维度（维度 A）可用既有/新增的组合子：
```lz
for x in xs.reversed():          // Reverse
for x in xs.strided(2):          // Strided(2)
for x in itor(xs):               // Controlled（Itor，支持 pause/resume/jump）
```

##### (e) 与 `Itor` 可控迭代器的关系

`Itor<T>`（`itor(iter)` / `__itor_from(iter)`，线程安全）是 `Controlled` 策略的**载体**，提供 `pause()` / `resume()` / `stop()` / `restart()` / `jump(n)`，并有 `history_strategy`（保留并重放迭代历史）。

即：`Controlled` 不是"另一种 for"，而是**把迭代过程本身对象化**，使暂停/恢复/跳转成为一等能力。`__iter_strategy__` 只需声明 `Controlled`，`__iter_resolve` 即用 `Itor` 包裹 base 迭代器。

##### (f) 待确认
1. `IterSpec` 是新增结构体，还是直接扩展 `IterStrategyKind` 使其携带 borrow 信息？
2. 维度 A 与维度 B 是否允许任意组合，还是需白名单（如 `Infinite, Owned` 是否合理）？
3. 策略不匹配时（调用点要 `MutRef` 但类型只声明 `Ref`）是编译错误还是静默降级？**建议编译错误**。

---

#### 5.4.3 守卫策略

```lz
magic GuardedStrategy =
    def __guarded_pred__(ref self, input: Input) -> bool = ...     // 判定：是否需要兜底
    def __guarded_action__(self, input: Input) -> Output = ...     // 执行：兜底行为并返回
```

- **配对生成** `GuardedStrategy` impl（§5.1 形态 2/4 的配对版）
- 二者**均支持按输入参数类型多分派**
- 语义：把"**前置条件不满足时的兜底行为**"做成类型可定制的策略

**典型用法**（`99-内置预导入库.md §3.7`）：

```lz
struct Div = b: int
    def __guarded_pred__(self, input: int) -> bool = self.b != 0
    def __guarded_action__(self, input: int) -> int = 0

guard div.__guarded_pred__(42) else:
    return div.__guarded_action__(42)      // 失败路径走兜底
```

> 注意区分：`guard` / `guard let` 是**控制流语句**（`05-控制流.md §7`）；`__guarded_pred__` / `__guarded_action__` 是**策略魔法**，二者联动——`guard` 的条件可委托给 `__guarded_pred__`，失败路径委托给 `__guarded_action__`。

**联动形态**：属于**配对型**，但与 §5.1 形态 5（blanket 互推，实现其一得其二）**不同**——守卫策略要求**成对实现**才生成 impl（pred 判定、action 执行，职责不可互推）。

**缺省规则（裁定 #11）**：

| 魔法 | 是否必选 | 说明 |
|---|:---:|---|
| `__guarded_pred__` | **必选** | 不实现则不存在守卫 |
| `__guarded_action__` | **可选** | 不实现则**无兜底** |

- 未实现 `__guarded_action__` 时：`guard` 的失败路径**必须由用户手写 `else` 体**；此时在 `guard` 中调用 `__guarded_action__` → **编译错误**。
- 理由：兜底行为是**业务决策**（返回 0？`None`？`raise`？），编译器不应猜测——猜错的兜底比没有兜底更危险。

---

#### 5.4.4 其它策略

| 策略 | 触发 | 说明 |
|---|---|---|
| `CloneStrategy` | `__clone__` | 克隆策略：`clone_of(&self, src: &Host) -> Host` |
| `DestroyStrategy` | `__drop__` | 析构策略：`destroy(&self, target: Host)` |
| `SerializeStrategy` | 序列化魔法 | `encode` / `decode` |
| `history_strategy` | `History` 策略 | 保留并重放迭代历史（`Itor` 侧） |
| 内存预算守卫 | `MemBudget` / `std/memguard.lz` | 编译期内存预算守卫（与 `guard` 体系同源，待并入） |

**条件注入规则**（`99 §3.2`）：仅当模块中出现对应魔法符号时，才注入该策略 trait 定义。这本身是一种**联动**：写魔法方法 → 自动引入对应策略 trait。

---

#### 5.4.5 策略型联动小结

```
类型作者：def __iter_strategy__ / __guarded_pred__ / __clone__ ...
                    │
                    ├─ 条件注入：出现对应符号 → 注入 Strategy trait
                    ├─ 声明：类型显式列出"我支持哪些策略"（按优先级）
                    └─ 解析：__iter_resolve() 按调用点需求取首个适用策略
                                    │
调用点：for x in xs  ──▶  选中策略  ──▶  包裹 base 迭代器（__iter__）
                                    └─▶  Itor（Controlled 时）
```

- 策略是**联动的最高形态**：一个语义目的 × N 种执行方式 × 按上下文择一。
- 与 §5.1 的关系：策略 = 形态 2（trait 多方法）+ 形态 4（实现其一）+ 形态 6（优先级链）的复合。

### 5.5 用户定义联动级魔法（Linkage Magic）★ 新增

> 用户诉求：除编译器策略型联动（§5.4）外，还希望**在用户层**把多个魔法方法组装成一个**协调的行为单元**——它可跨多个 trait、含多个魔法方法、方法间可互斥，并能把若干方法**按业务顺序连成一条链**（例："检测到异常 → 自动发邮件 → 等待用户指令 → 按指令执行"）。
>
> 这不是运行时状态机库，而是**编译期校验的魔法方法接线契约**：用户声明"谁触发谁、谁等用户决策、谁兜底"，编译器据此生成接线与调用点级联，类型不匹配即编译错误。

#### 5.5.1 四个诉求的落点

| 用户诉求 | 机制 | 落点 |
|---|---|---|
| 一个 magic 需要多个 trait | **多 trait 块**（§5.1 形态 3） | `magic X = { trait A = ...; trait B = ... }` |
| 一个 trait 里需要多个魔法方法 | **trait 内多 `def`**（§5.1 形态 2） | `trait A = def __m1__ = ...; def __m2__ = ...` |
| 魔法方法互斥 | **`mutex` 字段**（§5.6） | `mutex = ["__a__", "__b__"]` |
| 多个方法连成业务链 | **`linkage` 块**（本节） | `linkage: on … gate … else …` |

> 前两者 §5 已覆盖；本节补后两者，并说明如何**组合**成用户级联动单元。

#### 5.5.2 `linkage` 接线语义

`linkage` 块由若干**接线规则**组成，把本 magic 单元内的魔法方法（含跨 trait 方法）串成有向链：

| 规则 | 写法 | 语义 |
|---|---|---|
| 触发→反应 | `on A -> B` | 调用 `A` 返回 `v` 后自动调用 `B(v)`（或 `B(self, v)`）；`ret(A)` 须可赋 `arg(B)`，否则编译错误 |
| 用户决策门 | `gate C -> D` | 前一步后调用 `C` 取决策值 `d`，再以 `d` 调用 `D`；`C` 为**用户必须实现的钩子**（§5.5.3），即"用户下达指令"的暂停点 |
| 兜底 | `else E` | `gate` 返回 `None`/无决策时走 `E` |
| 多源串联 | 多行 `on`/`then` | 组成 DAG；多条规则可共享触发源 |

**类型校验（编译期）**：每条 `X -> Y` 要求 `ret(X)` 可赋给 `arg0(Y)`（若 `Y` 带 `self` 则为 `arg1(Y)`）；否则 `E: 联动接线类型不匹配`。保证联动不在运行时才崩。

**级联触发**：触发方法 `A` 被调用时，编译器在调用点**注入后续级联**（on→gate→else）；用户无需手写胶水。也可经生成的 `run_linkage()` 入口统一驱动。

#### 5.5.3 用户决策门（human-in-the-loop 钩子）

`gate C -> D` 中的 `C` 是**类型作者必须提供**的魔法方法（或注册到魔法名表的决策函数）。编译器**不生成默认实现**——理由同 §5.4.3 守卫策略：决策是业务行为，编译器不猜。

- `C` 未实现 → 编译错误 `E: linkage 决策门 __user_cmd__ 未实现`。
- `ret(C)` 须匹配 `arg(D)`（由 §5.5.2 校验保证）。
- 恰表达"监控→发邮件→**用户指令**→执行"：`__notify__` 自动发，`__user_cmd__` 是用户取指令的钩子，`__exec__` 按指令执行。

#### 5.5.4 组合示例（用户级联动单元）

```lz
// 用户层声明"监控→通知→指令→执行"联动魔法
magic MonitoringLinkage =
    trait AlertSource = def __on_anomaly__(ref self, a: Alert) -> Option<Cmd>   // 跨多 trait（诉求①）
    trait Notifier    = def __notify__(ref self, m: Mail) -> ()
    trait Executor    = def __exec__(ref self, c: Cmd) -> Result<(), Err>

    mutex = ["__exec_fix__", "__exec_new__"]   // 互斥：两种执行策略不可并存（诉求③，§5.6）

    linkage:                                   // 联动接线（诉求④）
        on   __on_anomaly__ -> __notify__
        gate __user_cmd__   -> __exec__
        else  __exec_new__

// 用户实现：决策门与执行由用户提供
struct MyMonitor =
    def __on_anomaly__(ref self, a: Alert) -> Option<Cmd> = detect(a)
    def __notify__(ref self, m: Mail) -> () = send_mail(m)          // 自动
    def __user_cmd__(ref self) -> Option<Cmd> = wait_for_user_command()   // ★用户钩子（暂停点）
    def __exec__(ref self, c: Cmd) -> Result<(), Err> = apply(c)    // 按指令执行
    def __exec_new__(ref self, c: Cmd) -> Result<(), Err> = deploy(c)    // 兜底
```

> 上例仅展示**语言机制**如何表达该业务形态；监控/邮件/指令通道等运行时组件由用户自实现（不在编译器范畴）。`magic` 只把"触发—通知—决策—执行"的**接线与互斥约束**钉成可静态校验的契约。

#### 5.5.5 与既有联动的关系

- `linkage` 是**用户声明**的接线（按业务顺序）；`chain`/`blanket`/`override`（§3.3）是**编译器策略**（按固定优先级/配对/覆盖）。二者正交：`chain` 决定"同一语义的候选搜索顺序"，`linkage` 决定"多个语义间的业务级联"。
- `linkage` 节点可引用 `chain` 选出的方法（如 `on __iter_resolve__ -> ...`）。
- `mutex` 可作用于 `linkage` 节点，保证链中互斥分支不会并存。

### 5.6 互斥（Mutual Exclusion）★ 新增

> 诉求③：某些魔法方法**不应同时出现**于同一类型（如两种互斥执行策略），同时实现应报编译错误。

#### 5.6.1 声明

```lz
magic Repair =
    def __exec_fix__(self, p: Patch) -> Result<(), Err> = ...
    def __exec_new__(self, c: Cmd) -> Result<(), Err> = ...
    mutex = ["__exec_fix__", "__exec_new__"]   // 互斥组
```

#### 5.6.2 语义

| 情形 | 结果 |
|---|---|
| 类型只实现互斥组中**一个** | ✅ 正常 |
| 类型同时实现互斥组中**多个** | ❌ 编译错误 `E: 互斥魔法方法 [__exec_fix__, __exec_new__] 不可同时实现` |
| 互斥组方法一个都没实现 | ✅ 允许（除非该 magic 另要求必选，见 §5.4.3 守卫式 `pred` 必选规则） |

#### 5.6.3 与"形态 4 实现其一即可"的区别

- 形态 4（`__lt__`/`__cmp__` 任选其一，另由默认派生）是**软可选**：实现一个即满足，实现多个时 `__cmp__` 优先并 warning。
- `mutex` 是**硬禁止**：互斥组内**严禁同时实现**，同时实现直接编译错误（无"优先"可言）。
- 选用原则：方法间**可派生/可共存**（如比较运算符）→ 形态 4 默认实现；方法间**语义冲突/资源互斥**（如两种执行策略）→ `mutex` 硬互斥。

#### 5.6.4 互斥作用域

- `mutex` 仅约束**同一 magic 单元内**声明的互斥组（与 magic 契约唯一性一致，裁定 #3）。
- 跨 magic 的互斥须在各 magic 内分别声明；需全局唯一则重新声明一个 magic（参考裁定 #4"重新声明而非局部覆盖"）。

---

## 6. `Option` / `Result` 相关魔法方法：归属与覆盖语义

### 6.1 归属

`__is_ok__` / `__unwrap__` / `__err__` **属于魔法方法**，归到 `Option` / `Result` 体系（`12-操作符.md:223` 称其为"编译器内建协议，非魔法方法"的说法 **作废**，以本文为准）。

```lz
magic IsOk =
    override = ["Option", "Result"]
    def __is_ok__(ref self) -> bool = ...

magic Unwrap =
    override = ["Option", "Result"]
    def __unwrap__(self) -> T = ...

magic Err =
    override = ["Result"]
    def __err__(self) -> E = ...
```

### 6.2 覆盖语义（关键）

> 若某类型实现了这些魔法方法，则该类型参与 `Option`/`Result` 判定时，**使用用户实现的方法，等同于覆盖内建行为**。

```lz
struct MyInt =
    v: int
    def __is_ok__(ref self) -> bool =
        self.v > 0            // MyInt 自己定义"什么算成功"

// Result<MyInt, Err> 实例判定时：
//   → 使用 MyInt 的 __is_ok__（而非 Result 内建判定）
```

**语义要点**：
1. 覆盖是**按元素类型**生效的：`Result<MyInt, E>` 的成功判定委托给 `MyInt::__is_ok__`。
2. 与 `?` 运算符联动：`obj?` 的成功分支走 `__unwrap__`，错误分支走 `__err__`，判定走 `__is_ok__`。
3. 若元素类型未实现，回退到 `Option`/`Result` 的内建判定。

### 6.3 E 侧覆盖规则（裁定 #2）

**两侧均可覆盖，但职责分离**：

| 侧 | 提供的魔法 | 覆盖对象 |
|---|---|---|
| **T 侧**（成功值类型） | `__is_ok__`（判定）、`__unwrap__`（取成功值） | `Option<T>` 的 Some 分支、`Result<T, E>` 的 Ok 分支 |
| **E 侧**（错误类型） | `__err__`（取错误值） | `Result<T, E>` 的 Err 分支 |

**冲突规则**：若同一类型同时充当 T 与 E（如 `Result<MyErr, MyErr>`），`__is_ok__` 判定**以 T 侧为准**；E 侧只认 `__err__`，E 上定义的 `__is_ok__` 被忽略。

```lz
struct MyErr =
    code: int
    def __err__(self) -> str = f"E{self.code}"    // E 侧：自定义错误取值

// Result<MyInt, MyErr> 上的 ? ：
//   判定 → MyInt::__is_ok__（T 侧）
//   成功 → MyInt::__unwrap__（T 侧）
//   错误 → MyErr::__err__（E 侧）
```

---

## 7. trait 的存放位置

**一律放 `lz_builtins`**。

- 自定义 trait：`HasLen`、`Contains`、`Pow`、`Cast`、`TryCast`、`HasBool`、`HasAbs`、`Callable`、`ImplicitFrom`/`ImplicitTo`、`ImplicitCopy`、`ImplicitDefault`、`WhenMove`、`GuardedStrategy`、`SpreadOk`/`SpreadErr`、`Enter`/`Exit`、`New`/`Init` 等。
- **移动所有权相关魔法**（详见 `SYNTAX/12-操作符.md` §1.14.1）：`__implicit_copy__`（移动时复制，原变量仍有效，等价 `Copy`）、`__when_move__<R>(self, before?: fn() -> R = None, after?: fn() -> R = None) -> bool`（移动前后钩子，返回是否真移动）。
- 与标准库 trait 有对应关系的（`Eq`→`PartialEq`、`Ord`→`Ord`、`Add`→`std::ops::Add` …）由编译器做**映射**，trait **定义**仍在 `lz_builtins`。
- 生成的 Rust 产物 `use lz_builtins::*;` 即可见（现状产物已含此行）。

---

## 8. 反射运算符

**暂不引入** `__radd__` / `__rsub__` 等反射运算符。待魔法方法体系完善后再单独议题评估。

（`06d`/`12-操作符.md` 中若出现相关描述，标注为"未实现 / 规划中"。）

---

## 9. 与现状实现的差距 & 落地步骤

### 9.1 现状

| 项 | 现状 |
|---|---|
| magic 位置 | 解析器支持顶层 `magic_blocks`（`parser.rs:425/442`），**也**支持 struct 内 `magic` + `MagicMethod`（`parser.rs:1877`）← 应移除 |
| magic 消费 | `builder.rs:8436` 遍历 `ast_module.magic_blocks` |
| 魔法 → trait impl | 仅 `__eq__` 手写特例（`codegen/mod.rs:3958-4010`），其余无 |
| 魔法注册表 | `src/magic/engine.rs` 33 条（含 trait_path/trait_method），**未被 codegen 泛化消费** |
| 联动 | 除 `__eq__→PartialEq` 外基本缺失（实测 `__contains__`/`__len__`/`__iadd__`/`__new__` 四处断裂） |

### 9.2 落地步骤（依赖顺序）

| 步 | 内容 | 涉及 |
|---|---|---|
| **S1** | **本文评审定稿** | — |
| **S2** | 据本文修订 SYNTAX 文档（清单见 §10） | `06f`/`06g`/`06d`/`06a`/`附录B` |
| **S3** | 解析器：禁止 struct/impl 内 `magic`（报错 `magic 只能出现在模块顶层`）；移除 `parser.rs:1877` 分支 | `src/parser/parser.rs` |
| **S4** | `MagicDef`（`src/ast/decl.rs:30`）扩展：支持多 trait、`blanket`、`chain`、`override` 字段 | `src/ast/decl.rs`、`src/parser` |
| **S5** | builder 消费 magic_blocks：生成 trait 定义 + 全局函数 + 魔法名注册表 | `src/ir/builder.rs:8436` |
| **S6** | **codegen 由 magic 注册表驱动生成 trait impl**（替代 `__eq__` 特例） | `src/ir/codegen/mod.rs:3958`、`src/magic/engine.rs` |
| **S7** | 联动实现：trait 默认实现合成 + `chain`/`blanket`/`override` 三个编译器级策略 | builder + codegen |
| **S7b** | **策略系统**：`IterBorrow` 所有权维度 + `IterSpec` + `__iter_resolve()` 策略解析；`Controlled`→`Itor` 包裹；守卫策略配对校验 | builder + codegen + `lz_builtins`（`Itor`/`IterSpec`） |
| **S8** | 标准库 trait 定义入库 | `lz_builtins` |
| **S9** | 回归：现有 DEMO（`traits.lz`/`box.lz`/`magic_methods.lz` 等）不劣化 | tests |

> S6 与 `IR/magic-methods-completion-plan.md` 的 **P1-0** 是同一件事；本文 S3–S7 是该计划的上游设计依据。

---

## 10. 文档修订清单（S2，据本文执行）

| 文件 | 修订内容 |
|---|---|
| `SYNTAX/06f-magic用法.md` | **删除 §六「内联 magic（struct 内）」整节**；§一 补强"仅模块级顶层"；新增联动声明字段（`blanket`/`chain`/`override`/`mutex`/`linkage`，见 §3.3/§5.5/§5.6） |
| `SYNTAX/06g-魔法综合.md` | §8.2 的 `struct Int = ... magic __implicit_from__` 示例改为顶层 magic + struct 内 `def`；§六 工作流图与本文 §4 对齐；新增"联动模型"章节与 §5.5 用户联动 `linkage`、§5.6 `mutex` 章节（含监控→通知→指令→执行示例） |
| `SYNTAX/06d-内置魔法trait和全局函数.md` | §九 `__new__/__init__` 示例移出 struct 内 magic；§十一 标注 `__is_ok__`/`__unwrap__`/`__err__` **归属魔法方法**且可覆盖 Option/Result；§十四 隐式策略补 blanket/chain 声明 |
| `SYNTAX/06a-struct.md` | §六 构造器魔法方法示例改为 `def __new__` |
| `SYNTAX/12-操作符.md` | `:223` "非魔法方法"表述作废，改指向魔法方法 + 覆盖语义 |
| `SYNTAX/附录B-关键字保留字符号语法边界.md` | `magic` 条目补"**仅模块级顶层**，struct 内不可" |
| `SYNTAX/06e-trait定义.md` | 如涉及，与 magic 的 trait 产出对齐 |
| `SYNTAX/99-内置预导入库.md` | §3.2 策略系统补 **`IterBorrow`（所有权维度）** 与 `IterSpec`，说明维度 A×B 组合；§3.3 明确 `Itor` = `Controlled` 策略载体；§3.7 守卫策略与 magic 声明的对应关系 |
| `SYNTAX/05-控制流.md` | §3.1 for 迭代协议补"策略选择（`__iter_resolve`）"；明确"循环变量默认可变"与**迭代模式注解**的分工（避免与 `mut` 冲突）；§7 `guard` 补委托式精简写法（`guard <obj> with <input>`，见 §5.4.3.1）与失败动作沿用规则（**§7.4 已落**） |
| `SYNTAX/06d-内置魔法trait和全局函数.md` | §十八 迭代策略补所有权/持久性维度；§十五 守卫策略补"配对实现、不可互推"约束 |

---

## 11. 裁定记录（Decisions）

> 原 §11 为"待决项"，现逐条裁定如下。**每条均给出理由与影响；如与预期不符，可单条推翻。**

| # | 议题 | 裁定 |
|---|---|---|
| 1 | `def __xxx__` 实现位置 | **两者都支持，语义等价** |
| 2 | `Result` E 侧覆盖 | **两侧都可覆盖，职责分离：T 侧管成功、E 侧管错误** |
| 3 | magic 跨文件拆分 | **不允许**（契约唯一，实现可分散） |
| 4 | `chain` 块外覆盖 | **不可**（编译器策略固定，仅 magic 块可声明） |
| 5 | 形态 4 歧义（`__lt__` vs `__cmp__`） | **`__cmp__` 优先**，手写 `__lt__` 报冗余 warning |
| 6 | magic 内 trait vs 独立 trait | **互补不等价**：magic 才产出魔法绑定 |
| 7 | 迭代策略语法 | **A（`@Mode` 注解）为主 + C（推断）为辅**，否决 B |
| 8 | `IterSpec` 载体 | **新增 `IterBorrow` 枚举 + `IterSpec` 结构体**，`IterStrategyKind` 不变 |
| 9 | 维度 A×B 组合 | **默认全允许 + 黑名单**（仅禁语义矛盾组合） |
| 10 | 策略不匹配 | **编译错误**，不静默降级 |
| 11 | 守卫策略缺省 | `__guarded_pred__` **必选**，`__guarded_action__` **可选** |
| 12 | `MemBudget` 并入策略体系 | **暂不并入** |
| 13 | 用户联动 `linkage` 决策门 | **必选**：`gate C -> D` 的 `C` 未实现即编译错误（同守卫策略精神，决策不猜测） |
| 14 | 互斥 `mutex` | **硬禁止**：同类型同时实现互斥组内多个方法 = 编译错误；实现其一或零个均允许 |
| 15 | 委托式 `guard` 关键字 | **`guard <obj> with <input>`**（方案 A）；冲突退路 `guard <obj> by <input>`；`__guarded_action__` 缺失时该写法编译错误 |

---

### 1. `def __xxx__` 的实现位置 → **两者都支持，语义等价**

**裁定**：`impl` 块内 与 struct 内部**都支持**，语义完全等价；同一魔法方法不可重复定义（重复 = 编译错误）。推荐 `impl` 块为首选写法。

**理由**：
- DEMO 与测试全用 `impl` + `def`（已实测 `__eq__` 可生成 `PartialEq`）；文档（`06d`/`06a`）示例多用 struct 内。两者都合法才不会与既有代码/文档冲突。
- struct 内 `def __xxx__` 本质是"struct 的固有方法"，编译器按**魔法名**匹配，与 `impl` 块无差别。

**影响**：实现需保证两条路径等价 → 需补 struct 内 `def __xxx__` 的回归探针（现状只验过 `impl` 路径）。

---

### 2. `Result<T, E>` 的 E 侧覆盖 → **两侧都可覆盖，职责分离**

**裁定**：
- **T 侧**（成功值类型）：提供 `__is_ok__`（判定）与 `__unwrap__`（取成功值）→ 覆盖 `Option<T>` 的 Some 分支与 `Result<T, E>` 的 Ok 分支。
- **E 侧**（错误类型）：提供 `__err__`（取错误值）→ 覆盖 `Result<T, E>` 的 Err 分支。
- **冲突规则**：若同一类型同时充当 T 与 E（如 `Result<MyErr, MyErr>`），`__is_ok__` 判定**以 T 侧为准**；E 侧只认 `__err__`，E 上定义的 `__is_ok__` 被忽略。

**理由**：`__err__` 语义是"从错误中取值"，错误值是 E 类型，由 E 提供最自然；判定"算不算成功"则是成功值的属性，归 T。职责分离可避免二义。

**影响**：§6 需按此补 E 侧说明（已同步）。

---

### 3. magic 跨文件拆分 → **不允许**

**裁定**：**magic 声明（契约）必须在单一样式、单一位置完整声明，不可跨文件/模块拆分**；但**实现**（`def __xxx__`）可分散在任意 `impl` 块。

**理由**：magic 的产出是"契约"（trait + 魔法名 + 全局函数）。契约若可拆分，会出现"同一魔法在不同文件有不同契约"，违背 magic 作为**唯一权威声明**的定位，也让全局函数签名无法确定。类 Rust：trait **定义**唯一，impl 可分散。

**影响**：标准库的 `Ord` 等需在**一处**完整声明（含 supertrait 链）。用户想扩展既有魔法 → 用普通 `trait` + `impl`，不产生新魔法名。

---

### 4. `chain` 优先级链可否块外覆盖 → **不可**

**裁定**：**不可在 magic 块外覆盖**。优先级链是编译器策略，只在 magic 块内声明；内置链（`__bool__` → `__len__` → true、隐式转换 5 级链）由编译器固定。

**理由**：若允许模块级覆盖，同一类型在不同模块的隐式行为会不同，破坏可预测性，且让"为什么这里转了那里没转"无法静态审计。

**影响**：需要新的优先级语义时，必须**重新声明一个 magic**（全局唯一），而非局部覆盖。

---

### 5. 形态 4 歧义：`__lt__` 与 `__cmp__` 并存 → **`__cmp__` 优先**

**裁定**：同时实现时**以 `__cmp__` 为准**，由 `__cmp__` 合成 `__lt__`/`__le__`/`__gt__`/`__ge__`；用户手写的这些方法与 `__cmp__` 共存时**不生效**，编译器报 **warning（冗余定义，已被 `__cmp__` 派生覆盖）**。

**理由**：全序（`Ordering`）蕴含偏序，信息更强、派生更一致；同一运算符若有两个来源，一旦不一致会产生极难排查的 bug。对齐 Rust：`Ord` 要求 `cmp`，`PartialOrd` 可由 `cmp` 派生。

**影响**：`__cmp__` 成为比较契约的"首选实现"；只实现 `__lt__` 的场景仍完全有效。

---

### 6. magic 内 trait vs 独立 `trait` → **互补，不等价**

**裁定**：
- **magic 块声明的 trait** = 魔法 trait（产出 trait + 魔法名 + 全局函数三件事）。
- **独立 `trait` 关键字定义的 trait** = 普通 trait，**不因含 `__xxx__` 方法就自动成为魔法**。
- 但二者**互补**：独立 trait 可为**已注册的魔法名**提供默认实现与约束（这正是 `DEMO/lz_std/traits.lz` 中 `trait PartialEq = def __eq__ = ...` + `__ne__` 默认实现生效的原因——`__eq__` 已在编译器魔法名注册表中）。

**理由**："什么是魔法"应由**魔法名注册表**（编译器内置 33 个 + magic 声明）决定，而非由"方法名长得像魔法"决定。否则标准库 trait 与 magic 声明会重复/冲突。

**影响**：`traits.lz` 的现有写法继续有效；新增魔法必须走 `magic` 声明（或加入内置注册表）。

---

### 7. 迭代策略语法 → **A（`@Mode` 注解）为主，C（推断）为辅；否决 B**

**裁定**：
```lz
for x in xs:              // 推断：取首个适用策略（通常 Ref）
for x in xs @Ref:         // 强制只读借用（引用即可）
for x in xs @MutRef:      // 强制可变借用（可修改原有值）
for x in xs @Owned:       // 强制消费（一次性，迭代后原值不可再用）
for x in xs @Controlled:  // Itor 可控（暂停/恢复/跳转）

for x in xs @MutRef if cond:   // 与守卫共存，@Mode 在前
```

维度 A（方式）**不进 for 语法**，用既有/新增组合子：`xs.reversed()` / `xs.strided(2)` / `itor(xs)`。

**否决 B（`for ref mut x`）的理由**：`05-控制流.md:158` 规定"循环变量**默认可变**"，且 `ref`/`mut` 在模式中已有绑定含义（`case Some(ref mut c)`），复用会让"变量可变"与"迭代模式"两套语义撞车。

**`@` 与装饰器冲突的风险与退路**：`@` 已用于装饰器（`@intrinsics`）。但二者**语法位置可区分**——装饰器在定义/表达式**前**，`@Mode` 在 for 迭代表达式**后**、冒号/守卫前。若实现期发现冲突，退路是组合子写法 `xs.mut_iter()` / `xs.ref_iter()`（仍可用，作为等价显式形式保留）。

---

### 8. `IterSpec` 载体 → **新增 `IterBorrow` 枚举 + `IterSpec` 结构体**

**裁定**：
- `IterStrategyKind` **保持 9 种不变**（方式维度）。
- **新增 `IterBorrow` 枚举**：`Owned` / `Ref` / `MutRef` / `Controlled`。
- **新增 `IterSpec` 结构体**：`{ borrow: IterBorrow, kind: IterStrategyKind }`（+ 推导字段 `consuming` / `repeatable`）。
- `__iter_strategy__` 返回 `List<IterSpec>`（按优先级降序）。

**理由**：若把所有权并入 `IterStrategyKind`，变体会从 9 个爆炸到 9×4=36 个，语义混杂、难维护。两个正交枚举 + 组合结构体最清晰，且允许未来独立扩展任一维度。

---

### 9. 维度 A×B 组合 → **默认全允许 + 黑名单（仅禁语义矛盾）**

**裁定**：**默认允许全部组合**；仅**黑名单**禁止语义上自相矛盾的组合。当前黑名单唯一条目：

| 禁止组合 | 原因 |
|---|---|
| `Infinite` × `Collected` | 无限迭代器 collect 为 Vec → 必然 OOM |

（`Infinite` × `Owned` **允许**——无限消费迭代器如生成器是合理的。）

**理由**：白名单维护成本高（每次新增维度都要补表），且会无谓限制表达力；真正矛盾的组合极少，用黑名单精准禁止即可。新增组合时走评审。

---

### 10. 策略不匹配 → **编译错误（不静默降级）**

**裁定**：调用点要求 `MutRef` 而类型只声明了 `Ref`（或反之）→ **编译错误**，不静默降级。

**理由**：静默降级会让"我要修改原值"悄悄退化成"只读引用"，写操作无声失败或被丢弃，是极难排查的逻辑 bug。显式报错 + 错误信息列出"该类型声明了哪些策略"，用户可立即定位。

**错误信息应含**：期望模式、类型实际声明的策略列表、如何修正（改声明 或 改用 `@Ref`）。

---

### 11. 守卫策略缺省 → `pred` 必选，`action` 可选

**裁定**：
- `__guarded_pred__` **必须实现**（否则不存在守卫）。
- `__guarded_action__` **可选**。
- 若未实现 `__guarded_action__`：`guard` 的失败路径**必须由用户手写 `else` 体**；此时在 guard 中调用 `__guarded_action__` → 编译错误。

**理由**：兜底行为是**业务决策**（返回 0？返回 `None`？`raise`？），编译器不应猜测——猜错的兜底比没有兜底更危险（静默返回错误值）。要求用户显式给出，语义可审计。

**影响**：§5.4.3 同步此规则。

#### 5.4.3.1 精简 `guard` 语法（委托式）★ 新增

> 痛点：上一节典型用法 `guard div.__guarded_pred__(42) else: return div.__guarded_action__(42)` 把 pred/action 的调用写两遍，且 `__guarded_action__` 的返回值还要手写 `return`，冗长且易错。

**机制**：当 `guard` 的操作数是一个**实现了 `GuardedStrategy` 的类型实例**时，可用委托式写法，让编译器自动把条件委托给 `__guarded_pred__`、把失败兜底委托给 `__guarded_action__`：

```lz
// 精简写法（推荐）
guard div with 42
// 编译器展开为：
guard div.__guarded_pred__(42) else:
    return div.__guarded_action__(42)

// 循环体内：失败动作沿用 §7.1，须覆盖默认 return
for d in divs:
    guard d with x else continue    // 不满足 → 跳下一轮，兜底走 __guarded_action__
// 生成器内
iterator safe(xs) -> int =
    for d in xs:
        guard d with x else yield -1   // 不满足 → 产出 -1
```

**语法（推荐 · 方案 A）**：`guard <guarded-obj> with <input>`

| 要素 | 含义 |
|---|---|
| `<guarded-obj>` | 实现了 `GuardedStrategy` 的实例（pred/action 的 `self`） |
| `with <input>` | 传给 `__guarded_pred__` / `__guarded_action__` 的输入参数 |

**语义**：
1. 条件 = `<obj>.__guarded_pred__(<input>)`；为 False 时失败路径 = `return <obj>.__guarded_action__(<input>)`（默认 `return`，与 §7.1 一致）。
2. 失败动作沿用 §7.1 规则：循环体内须 `else break`/`else continue`；生成器内 `else yield`；块/顶层须显式 `raise`/`break NAME`。
3. `<input>` 可为任意表达式；pred/action 签名须一致（同一 `Input` 类型），否则编译错误。

**约束**：
- `__guarded_action__` 未实现（裁定 #11 可选）→ `guard div with 42` **编译错误**：无法自动委派兜底，须改回显式 `guard ... else:` 手写失败体。
- 想自定义失败动作（不走 `__guarded_action__`）时，仍用原有显式 `guard <cond> else ...` 形式。

**与 `with` 关键字冲突**：`with` 亦作资源管理块（`with <expr>:`）。二者**语法位置可区分**——`guard` 之后的 `with` 位于表达式中间、后接输入且无尾随冒号；资源管理 `with` 位于语句开头、后接冒号起块（同 §5.4.2(d) 的 `@Mode` 位置区分思路）。若实现期发现歧义，退路为 `guard div by 42`（`by` 引入输入，且不与 `guard let` 的 `=` 冲突）。

---

### 12. `MemBudget` / 内存预算守卫 → **暂不并入策略体系**

**裁定**：**不并入**。`MemBudget` / `std/memguard.lz` 保持为独立的**编译期**机制。

**理由**：它是**编译期**内存预算守卫，作用于编译过程而非类型行为，不具备策略系统的本质特征（"类型作者声明多种方式 + 调用点按需求择一"）。并入会稀释策略语义，且把编译期概念混入运行期魔法体系。

**影响**：文档需在两处互相注明"**内存预算守卫 ≠ 守卫策略（`__guarded_pred__`/`__guarded_action__`）**"，避免混淆。未来若出现**运行期**内存策略需求（如 `__mem_budget__`），再单独评估。

---

### 裁定的连带修订

| 裁定 | 已同步到 |
|---|---|
| #2（E 侧覆盖） | §6 已补 E 侧规则 |
| #7（`@Mode`） | §5.4.2(d) 标注为已裁定 |
| #11（action 可选） | §5.4.3 标注为已裁定 |

---

## 附：一图流

```
magic（模块顶层 · 声明）
   │
   ├── trait <Name>  …  魔法函数签名（抽象 = ...  或 默认实现 = expr）
   ├── 魔法名 __xxx__
   └── 全局函数 xxx()
            │
            │  [联动]
            ├── trait 默认实现   → 方法间派生（__ne__ ← __eq__）
            ├── 多 trait/继承链  → trait A: B + C
            ├── blanket = "..."  → 配对互推（From ⇄ To）
            ├── chain = [...]    → 优先级链（__bool__ → __len__ → true）
            ├── override = [...] → 覆盖内建（Option/Result 判定）
            ├── mutex = [...]   → 硬互斥（同类型不可同时实现，§5.6）
            └── linkage: …      → 用户级多方法接线（on/gate/else，§5.5）

def __xxx__（impl 块内 或 struct 内 · 实现）
            │
            ▼
   编译器匹配签名 → 生成 impl + Rust 侧 trait 映射
            │
            ▼
   运算符 / 全局函数 / ? / with / if … 可用
```

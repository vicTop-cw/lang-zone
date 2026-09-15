# LZ 魔法方法 — magic 用法

> 规范版本: 3.3 · 基于编译器源码 · 最后校订: 2026-09-14

本文档详细说明如何使用 `magic` 块注册自定义魔法方法。

---

## 零、定位：`magic` 是**声明**，不是实现

> **本节为规范性约束，优先于本文档及 `06d` / `06a` / `06g` 中的任何历史示例。**

`magic` 是**模块级顶层的魔法特性声明**。一次 `magic` 声明同时完成三件事：

| # | 产物 | 例（`magic Eq`） |
|---|---|---|
| 1 | 定义一个 **trait**，trait 内含魔法函数签名 | `trait Eq { fn __eq__(...) -> bool }` |
| 2 | 拟定 **魔法名**（`__xxx__` 形式） | `__eq__` |
| 3 | 拟定 **全局函数** | `eq(a, b)` |

> **三产物实现状态（2026-09-13 对照编译器源码核实）**：
>
> | 产物 | 状态 | 当前形态 |
> |---|:---:|------|
> | trait | ✅ | **无需** `magic` 声明：struct/impl 定义 `def __xxx__` 时编译器自动生成对应 trait impl（std trait / lz_builtins trait，见 [06d](06d-内置魔法trait和全局函数.md) §〇 三分法 A/C 类） |
> | 魔法名 | ✅ | `__xxx__` 方法经双下划线命名识别，运算符/语法位置在调用点直派方法调用（B 类直派） |
> | 全局函数 | ❌ 规划中 | `fn <块名>(args)` 全局函数**尚未生成**；期间用户需以普通 `def` 显式书写（推导规则正式规范见 §七） |
>
> 即：当前 `magic` 声明块仅实现"把魔法方法注册到目标类型"（`magic __xxx__:` 块与 struct 内 `magic __xxx__(...)` 两种形式，见 §一）；
> 06g §六"声明 magic → 统一全局函数调用"的工作流**尚未可用**。

### 硬性约束

| 约束 | 说明 |
|---|---|
| ① **只能在模块级顶层** | `struct` / `impl` / `trait` / 函数体 / 任何缩进块内写 `magic` 均为**编译错误**。**不存在"内联 magic"这一语法糖** |
| ② `magic` 是**声明/契约**，不是实现 | 实现一律用 `def __xxx__`（写在 `impl` 块内或 struct 内部，二者语义等价） |
| ③ 声明**可以带默认实现** | 块内 `= expr` 为默认实现，`= ...` 为抽象（实现方必须提供） |

### 错误 vs 正确

```lz
// ❌ 错误：magic 写在 struct 内（编译错误）
struct Point =
    x: int
    y: int
    magic __new__(x: int, y: int) -> Point =
        Point(x, y)
```

```lz
// ✅ 正确：顶层声明魔法特性（通常来自标准库，用户一般无需自己写）
magic New =
    def __new__(...) -> Self = ...

// ✅ 正确：实现用 def（struct 内或 impl 块内均可）
struct Point =
    x: int
    y: int
    def __new__(x: int, y: int) -> Point =
        Point(x: x, y: y)
```

---

## 一、magic 声明块语法

magic 块有两种等价书写形式：方法定义式（推荐）和声明式配置。

### 方法定义式（推荐）

```lz
// 完整语法（方法定义式）
magic map<T, R> =
    def __map__(self, f: fn(T) -> R) -> Iterable<R>
        where Self : Iterable<T>
        = ...                              // 抽象，struct 必须提供实现
```

这种形式直接包含魔法方法的完整签名，编译器可从中提取所有元信息（trait 名、方法名、self 模式等），无需额外声明。

> **实现状态：❌ 尚未支持（规划中）**。当前解析器仅识别顶层 `magic __xxx__:` 形式（双下划线方法名 + 冒号）；
> 本节方法定义式（块名可与方法名不同、带泛型参数表、`=` 接块体）**解析器尚未支持**——直接书写会报解析错误。
> 本节语法为**规范目标形式**，产物定义见 §七，实现后需同步更新本注。

### 声明式配置

```lz
// 声明式配置
magic __map__:
    trait = "Map"
    method = "map"
    self = "owned"
    dispatch = "by_ret"
```

此形式仅指定元数据键值对，不包含方法签名。**适用场景**：当编译器自动推断的默认值无法满足需求时，通过声明式配置覆盖特定字段。

> **实现状态：🔸 部分支持**。`magic __xxx__:` 已被解析器支持：块内 `def` 会注册为目标类型（self 参数类型）的魔法方法，并自动生成对应 trait impl（06d §〇 A/C 类）；
> 但配置键值对（`trait = ...` / `dispatch = ...` 等）**当前被解析器跳过、未消费**——§三 表所列字段均为"解析但忽略"状态。
> struct 内 `def __xxx__` 与 `magic __xxx__:` 内 `def __xxx__` 语义等价（后者是前者的语法糖）。

---

## 二、magic 块名规则

magic 块的命名直接决定了自动生成产物的名称：

| 元素 | 来源 | 示例 |
|------|------|------|
| 全局函数名 | magic 块名 | `magic map<T,R> =` → 全局函数 `map()` |
| 内部方法名 | `def` 中的方法名 | `def __map__(...)` → 魔法方法 `__map__` |
| trait 名 | 魔法方法名去掉 `__`，首字母大写 | `__map__` → `Map` |
| 泛型参数 | 跟随 magic 块声明 | `magic map<T, R>` → 全局函数为 `map<T, R>(self, f)` |

**命名约束**：
- magic 块名必须为有效标识符，与全局函数名一一对应
- 魔法方法名必须以双下划线 `__` 包裹（如 `__map__`、`__filter__`）
- trait 名由编译器自动推导（去掉 `__` 后 PascalCase），除非声明式中显式指定

---

## 三、配置字段表（声明式）

以下字段用于声明式 `magic` 配置，每个字段均对应方法定义式中的一个语义概念。

| 字段 | 可选值 | 默认值 | 说明 | 实现状态 |
|------|--------|--------|------|:---:|
| `trait` | 字符串字面量 | `__方法名__` 去掉 `__` 的 PascalCase | 生成的 trait 名称 | ❌ 未消费（当前 trait 名由魔法方法名推导，本字段解析后忽略） |
| `method` | 字符串字面量 | `__方法名__` 去掉 `__` | 全局函数的对外方法名 | ❌ 未消费（全局函数尚未生成，见 §零） |
| `self` | `"owned"` / `"ref"` / `"refmut"` / `"none"` | `"owned"` | self 参数的所有权模式 | 🔸 self 模式当前由编译器按方法签名自动推断（`self`/`ref self`/`mut self`），本字段未消费 |
| `dispatch` | `"none"` / `"by_ret"` / `"by_params"` / `"by_arg(N)"` | `"none"` | 多分派策略 | ❌ 未消费（多分派未实现，见 06g §二；当前魔法方法按单签名分派） |
| `ret` | `"assoc"` / `"generic"` | `"assoc"` | 返回类型：关联类型 vs 泛型 | ❌ 未消费（关联类型/泛型返回选择未实现；当前 std trait impl 均按 Rust trait 固定形态生成） |
| `tuple` | `"true"` / `"false"` | `"false"` | 是否支持元组自动解包 | ❌ 未消费 |

### 字段详解

**`self`**：控制魔法方法接收 self 的方式。

| 值 | self 类型 | 适用场景 |
|----|-----------|----------|
| `"owned"` | `self: Self` | 消费型操作（如 `+`、迭代） |
| `"ref"` | `self: &Self` | 只读操作（如 `==`、`str`） |
| `"refmut"` | `self: &mut Self` | 可变操作（如迭代器 `next`） |
| `"none"` | 无 self 参数 | 关联函数（如 `default`、`new`） |

**`dispatch`**：多分派策略，详见 §三 配置字段表。

**`ret`**：返回类型模式。
- `"assoc"`：在 trait 上定义 `type Output` 关联类型
- `"generic"`：返回类型作为 trait 的泛型参数 `<R>`

**`tuple`**：设为 `"true"` 时，调用 `map(xs, f)` 时若 `xs` 为元组类型则自动按元素解包。

---

## 四、完整示例

以下展示从定义到使用的完整流程。

> **实现状态（❌ 规划中）**：本示例的 `magic map<T, R> = ...` 方法定义式与"编译器自动生成全局函数 `map`"两步**均未实现**（解析器不支持该形式；全局函数生成是下一阶段工作，产物推导规则见 §七）。
> **当前可用的等价流程**：struct 内直接 `def __map__` + 用户手写 `def map(...)` 全局包装函数。

### 步骤 1：定义 magic 块

```lz
magic map<T, R> =
    def __map__(self, f: fn(T) -> R) -> Iterable<R>
        where Self : Iterable<T>
        = ...
```

编译器自动生成：
- Trait：`trait Map<T, R> { ... }`（含 `__map__` 签名的 trait 定义）
- 全局函数：`fn map<T, R, S>(self: S, f: fn(T) -> R) -> Iterable<R> where S: Map<T, R>`

### 步骤 2：struct 实现魔法方法

```lz
struct MyList<T> =
    items: List<T>

    // 实现魔法方法 __map__ → 自动获得 Map trait
    def __map__(self, f: fn(T) -> R) -> Iterable<R> =
        result = []
        for item in self.items:
            result.push(f(item))
        result
```

只要 struct 定义了与魔法方法同签名的 `__map__` 方法，编译器即自动为其生成 `impl Map<T, R> for MyList<T>`。

### 步骤 3：调用全局函数

```lz
def main() =
    let xs = MyList(items: [1, 2, 3])
    let doubled = map(xs, |x| x * 2)      // 全局函数调用
    for x in doubled:
        print(x)                            // 2 4 6
```

全局函数 `map` 自动接受任何实现了 `Map` trait 的类型。

---

## 五、语法边界

```lz
// ✓ 正确：方法定义式（❌ 当前未实现，规范目标形式）
magic map<T, R> =
    def __map__(self, f: fn(T) -> R) -> Iterable<R>
        where Self : Iterable<T>
        = ...

// ✓ 正确：声明式配置（✅ 当前已解析；配置键值对未消费，见 §三 实现状态列）
magic __map__:
    trait = "Map"
    method = "map"

// ✗ 错误：方法名不是 __xxx__ 形式
magic map =
    def map(self, f: fn(T) -> R) -> R = ...   // 方法名缺少 __

// ✗ 错误：magic 块名与方法名不匹配
magic filter<T> =
    def __map__(self, f: fn(T) -> bool) -> bool = ...
    // magic 名为 filter，但定义的方法为 __map__（应为 __filter__）

// ✓ 合法但无实际意义：声明式全部字段都有默认值（§三 表格），
//    仅覆盖个别字段且不改变语义的配置不报错，只是冗余
magic __map__:
    self = "owned"   // 单独指定 self 与默认值相同 → 冗余但不报错
```

**关键规则**：
1. 魔法方法名必须以 `__` 开头和结尾
2. magic 块名 = 全局函数名（方法定义式），需与方法语义一致
3. 声明式和方法定义式不可混用在同一个 magic 块内
4. `where Self : Trait` 约束可写于方法签名之后（多约束可逐行列出）

---

## 六、`magic` 的位置约束与常见误用

> **勘误复核（2026-09-13，实测修正）**：本章早期版本称"struct 内 `magic __方法名__` 为编译错误、语法糖已废除"。
> 经对照解析器源码（`src/parser/parser.rs` struct 体内 `magic` 分支）与探针实测（p62：`struct P` 内 `magic __eq__(...)` 转译成功，
> 生成 `fn __eq__` + 调用点直派 `a.__eq__(b)`），**struct 内 `magic __xxx__(...)` 形式是合法的**，与 struct 内 `def __xxx__` 语义等价（均注册为 struct 方法，
> 触发 06d §〇 A/C 类 trait impl 自动生成）。本节按实测修正。
> 早期勘误所指的"错误"，实际针对的是**方法定义式** `magic Name = def ...`（块名不带双下划线、`=` 接签名体）在 struct 内的写法——
> 该形式整体尚未实现（见 §一），并非"struct 内 magic 一律非法"。

### 6.1 位置支持矩阵（实测状态）

| 位置 | 形式 | 状态 |
|------|------|:---:|
| 模块级顶层 | `magic __xxx__:` + 缩进块（内 `def __xxx__(self: T) -> ...`） | ✅ 支持（p63 实测：注册到 self 参数指定类型 + trait impl 自动生成） |
| 模块级顶层 | `magic Name<T> = def __xxx__(...) ... = .../expr`（方法定义式） | ❌ 未实现（规范目标形式，见 §一/§七） |
| struct 体内 | `magic __xxx__(params) -> R = ...`（声明式签名） | ✅ 支持（p62 实测：等价于 struct 内 `def __xxx__`） |
| impl 块内 | `magic __xxx__(...) = ...` | 🔸 未发现解析分支（`def __xxx__` 在 impl 块内是主要写法） |
| 函数体内 / 任何缩进块 | 任意 | ❌ 报错（非声明位置） |

### 6.2 声明可以提供默认实现

`magic` 是**声明**，但**允许**给出默认实现（详见 §三 / §五）：

```lz
// ❌ 当前未实现（方法定义式，见 §一）：
magic PartialOrd =
    def __lt__(ref self, ref other: Self) -> bool = ...     // 抽象：实现方必须提供
    def __le__(ref self, ref other: Self) -> bool =         // 默认实现：由 __lt__ 派生
        not other.__lt__(self)
```

实现方只需写 `def __lt__`，`<=` 即自动可用（联动派生）。

> **当前实现边界**：默认实现（`= expr` 提供方法体）与"实现方缺省时自动补齐"的联动派生**尚未实现**。
> 当前编译器对魔法方法的处理是"struct/impl 定义了 `def __xxx__` 才生成对应 trait impl"，
> 不存在"声明侧默认方法体被实现方复用"的机制。本小节为规范目标行为。

### 6.3 书写形式对照

| 需求 | 写法 | 状态 |
|------|------|:---:|
| 给 struct 定义魔法方法 | struct 内 `def __xxx__(...)` 或 `magic __xxx__(...) = ...` | ✅ |
| 模块级声明魔法方法（指定目标类型） | 顶层 `magic __xxx__:` 块 + `def __xxx__(self: T) -> ...` | ✅ |
| 声明独立 trait + 全局函数 | 顶层 `magic Name<T> = def __xxx__ ...`（方法定义式） | ❌ 规划中（产物规则见 §七） |
| 声明式覆盖 trait 名/多分派策略 | 顶层 `magic __xxx__:` 块 + 配置键值对 | 🔸 已解析、未消费 |

> 设计依据见 `IR/design-magic-feature.md` §2（`magic` 不是什么）与 §5.4（策略型联动）。

---

## 七、声明产物正式规范（方法定义式 → trait + 全局函数）

> 本章是 `magic Name<T, ...> = def __xxx__ ...` 方法定义式（§一）的**产物定义**，为后续实现与
> 06g §六工作流的规范性依据。当前状态：**❌ 未实现**（见 §零 三产物实现状态）。

### 7.1 声明语法（完整形式）

```
magic <Name><<泛型参数表>?> =
    def <__方法名__><<泛型参数表>?>(<self 参数>, <非 self 参数>...) -> <返回类型>
        where <约束> [where <约束>]*
        = <expr> | ...
```

- `<Name>`：块名 = 全局函数名（与魔法方法语义一致，如 `map` ↔ `__map__`）
- `<self 参数>`：`self` / `ref self` / `mut self`（决定调用端所有权语义，见 06g §一）；无 self 参数时为关联函数（如 `__default__`）
- `= ...`：抽象（实现方必须提供方法体）；`= expr`：默认实现（实现方缺省时使用）
- `where` 约束：`Self : Trait`（限制可实现类型）或 `<T> : Trait`（泛型参数约束）

### 7.2 产物 ①：trait 定义

```
输入（LZ）:
magic map<T, R> =
    def __map__(self, f: fn(T) -> R) -> Iterable<R>
        where Self : Iterable<T>
        = ...

产物（Rust）:
pub trait Map<T, R> where Self: IntoIterator<Item = T> {
    fn __map__(self, f: fn(T) -> R) -> Vec<R>;   // self 模式按 06g §一 映射
}
```

推导规则：
1. **trait 名**：`__方法名__` 去 `__` 后 PascalCase（`__map__` → `Map`）；声明式 `trait = "X"` 可覆盖
2. **trait 泛型**：方法定义式方法签名中的未绑定类型参数（`T`、`R`）提升为 trait 泛型；`ret = "generic"` 时返回类型参数（`R`）显式进入泛型表，`ret = "assoc"`（默认）时生成关联类型 `type Output;` 并以 `Self::Output` 替换返回类型
3. **self 模式**：`self` → `self`（owned）；`ref self` → `&self`；`mut self` → `&mut self`（与 06g §一 表一致）
4. **where 约束**：`Self : Trait` → trait 方法约束（实现方 impl 时校验）；`<T> : Trait` → trait 泛型 bounds
5. **默认实现**：`= expr` 时 trait 方法带方法体（Rust trait 默认方法）；`= ...` 时为抽象方法

### 7.3 产物 ②：全局函数

```
输入（LZ）:  同上 magic map 声明

产物（Rust）:
pub fn map<T, R, S>(arg0: S, f: fn(T) -> R) -> Vec<R>
    where S: Map<T, R>, S: IntoIterator<Item = T>
{
    arg0.__map__(f)
}
```

推导规则：
1. **函数名** = 块名（`map`）
2. **参数列表**：self 参数转为**第一个位置参数**（命名 `arg0`，类型 `S` = 自类型泛型），其余非 self 参数按签名原序跟随
3. **自类型泛型**：追加 `S` 作为函数泛型参数（与 trait 泛型并列），约束 `S: <Trait><trait 泛型实参>`
4. **返回类型** = 方法返回类型（经 Self 泛型化替换）
5. **函数体**：`arg0.__方法名__(<其余实参>)`
6. **where Self : Trait 约束**：对全局函数降级为 `S: Trait`（自类型 S 必须满足）

### 7.4 产物 ③：struct 自动 impl 匹配

struct 定义与魔法方法**同签名**的 `def __xxx__` 时，编译器自动生成
`impl <Trait> for <Struct>` 委托 impl（此能力已实现，见 06d §〇 A 类——
当前对内置魔法方法表生效；方法定义式实现后，用户自定义 magic 声明的方法同样触发）：

```
struct MyList<T> = items: List<T>
    def __map__(self, f: fn(T) -> R) -> Iterable<R> = ...

产物:
impl<T, R> Map<T, R> for MyList<T> where MyList<T>: IntoIterator<Item = T> {
    fn __map__(self, f: fn(T) -> R) -> Vec<R> { self.__map__(f) }
}
```

匹配规则：
1. **签名匹配**：方法名相同；非 self 参数的类型序列一致（含泛型参数对齐）；返回类型一致
2. **self 模式匹配**：struct 方法 self 模式须与声明一致（或更弱：声明 `self` 时 struct 可 `ref self`，编译器按 06g §一 语义提升）
3. **约束校验**：struct 必须满足声明的 `where Self : Trait`（复用 duck_check 结构匹配机制报 `E0600`）
4. **多分派**（`dispatch = "by_ret"` / `"by_arg(N)"`）：struct 的多个同名方法按分派键区分，各生成一个 trait impl（**未实现**，当前按单签名分派）

### 7.5 端到端示例（规范目标）

```lz
// ① 声明（模块顶层，通常由标准库提供）
magic map<T, R> =
    def __map__(self, f: fn(T) -> R) -> Iterable<R>
        where Self : Iterable<T>
        = ...

// ② 实现（struct 内 def，签名匹配）
struct MyList<T> =
    items: List<T>
    def __map__(self, f: fn(T) -> R) -> Iterable<R> =
        result = []
        for item in self.items:
            result.push(f(item))
        result

// ③ 使用（全局函数，接受任何 Map 实现类型）
def main() =
    let xs = MyList(items: [1, 2, 3])
    let doubled = map(xs, |x| x * 2)
    for x in doubled:
        print(x)      // 2 4 6
```

---

*上一章：[06e-模块级魔法属性](06e-模块级魔法属性.md)* · *下一章：[06g-魔法综合](06g-魔法综合.md)*

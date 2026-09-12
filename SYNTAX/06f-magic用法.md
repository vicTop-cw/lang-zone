# LZ 魔法方法 — magic 用法

> 规范版本: 3.3 · 基于编译器源码 · 最后校订: 2026-08-04

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

| 字段 | 可选值 | 默认值 | 说明 |
|------|--------|--------|------|
| `trait` | 字符串字面量 | `__方法名__` 去掉 `__` 的 PascalCase | 生成的 trait 名称 |
| `method` | 字符串字面量 | `__方法名__` 去掉 `__` | 全局函数的对外方法名 |
| `self` | `"owned"` / `"ref"` / `"refmut"` / `"none"` | `"owned"` | self 参数的所有权模式 |
| `dispatch` | `"none"` / `"by_ret"` / `"by_params"` / `"by_arg(N)"` | `"none"` | 多分派策略 |
| `ret` | `"assoc"` / `"generic"` | `"assoc"` | 返回类型：关联类型 vs 泛型 |
| `tuple` | `"true"` / `"false"` | `"false"` | 是否支持元组自动解包 |

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

以下展示从定义到使用的完整流程：

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
// ✓ 正确：方法定义式
magic map<T, R> =
    def __map__(self, f: fn(T) -> R) -> Iterable<R>
        where Self : Iterable<T>
        = ...

// ✓ 正确：声明式配置
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
2. magic 块名 = 全局函数名，需与方法语义一致
3. 声明式和方法定义式不可混用在同一个 magic 块内
4. `where Self : Trait` 约束可写于方法签名之后（多约束可逐行列出）

---

## 六、`magic` 的位置约束与常见误用

> **历史勘误（重要）**：本章早期版本曾描述"struct 内部可使用 `magic __方法名__` 作为 magic 块的语法糖"，
> 并给出 `struct Point = ... magic __new__(...) = ...` 一类示例。
> 该说法**错误，已废除**——`magic` 不允许出现在 struct 内，也**不存在**这一语法糖。
> 该错误表述已扩散到 `06d §九`、`06a §六`、`06g §8.2` 以及若干 DEMO，均需按本章修正。

### 6.1 唯一合法位置：模块级顶层

```lz
// ✅ 模块级顶层
magic New =
    def __new__(...) -> Self = ...

// ❌ struct 内
struct Point =
    magic __new__(...) = ...

// ❌ impl 块内（如 DEMO/lz_std/box.lz 的历史写法）
impl<T> Box<T> =
    magic __new__(value: T) -> Box<T> = ...

// ❌ 函数体内 / 任何缩进块内
def f() =
    magic __str__(...) = ...
```

编译器应在以上非法位置报错：`magic 声明只能出现在模块顶层`。

### 6.2 声明可以提供默认实现

`magic` 是**声明**，但**允许**给出默认实现（详见 §三 / §五）：

```lz
magic PartialOrd =
    def __lt__(ref self, ref other: Self) -> bool = ...     // 抽象：实现方必须提供
    def __le__(ref self, ref other: Self) -> bool =         // 默认实现：由 __lt__ 派生
        not other.__lt__(self)
```

实现方只需写 `def __lt__`，`<=` 即自动可用（联动派生）。

### 6.3 迁移对照

| 历史写法（错误） | 现行写法 |
|---|---|
| `struct P = ... magic __new__(...) = ...` | 顶层 `magic New = def __new__(...) = ...` + struct 内 `def __new__(...) = ...` |
| `impl<T> Box<T> = ... magic __new__(...) = ...` | 顶层 `magic New = ...` + `impl<T> Box<T> = def __new__(...) = ...` |
| `struct P = ... magic __implicit_from__(...) = ...` | 顶层 `magic ImplicitFrom = ...` + struct 内 `def __implicit_from__(...) = ...` |

> 设计依据见 `IR/design-magic-feature.md` §2（`magic` 不是什么）与 §5.4（策略型联动）。

---

*上一章：[06e-模块级魔法属性](06e-模块级魔法属性.md)* · *下一章：[06g-魔法综合](06g-魔法综合.md)*

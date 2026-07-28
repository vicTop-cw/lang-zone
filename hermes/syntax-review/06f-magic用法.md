# LZ 魔法方法 — magic 用法

> 版本: 3.1 · 基于编译器源码 · 2026-07-27

本文档详细说明如何使用 `magic` 块注册自定义魔法方法。

---

## 一、magic 声明块语法

magic 块有两种等价书写形式：方法定义式（推荐）和声明式配置（向后兼容）。

### 方法定义式（推荐）

```lz
// 完整语法（方法定义式）
magic map<T, R> =
    def __map__(self, f: fn(T) -> R) -> Iterable<R>
        where Self <: Iterable<T>
        = ...                              // 抽象，struct 必须提供实现
```

这种形式直接包含魔法方法的完整签名，编译器可从中提取所有元信息（trait 名、方法名、self 模式等），无需额外声明。

### 声明式配置（向后兼容）

```lz
// 声明式配置（向后兼容）
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

**`dispatch`**：多分派策略，详见 §二。

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
        where Self <: Iterable<T>
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
        where Self <: Iterable<T>
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

// ✗ 错误：声明式缺少必要字段
magic __map__:      // 至少需要 trait 或 method 之一（虽然有默认值）
    self = "owned"   // 单独指定 self 无实际意义
```

**关键规则**：
1. 魔法方法名必须以 `__` 开头和结尾
2. magic 块名 = 全局函数名，需与方法语义一致
3. 声明式和方法定义式不可混用在同一个 magic 块内
4. `where Self <: Trait` 约束可写于方法签名之后（多约束可逐行列出）

---

*上一章：[06e-模块级魔法属性](06e-模块级魔法属性.md)* · *下一章：[06g-魔法综合](06g-魔法综合.md)*

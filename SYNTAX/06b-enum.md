# LZ enum（枚举）

> 规范版本: 3.3 · 基于编译器源码 · 最后校订: 2026-08-05

本文档定义 Lang-Zone 的枚举类型语法。

---

## 一、基本定义

```lz
enum Color:
    Red
    Green
    Blue
```

- 使用 `enum Name:` 语法（冒号 + 换行缩进）
- **注意**：enum 用 `:` 而非 `=`，与 struct 不同
- 变体可以是**无字段**（单元变体）或**带字段**（元组变体）

---

## 二、无数据变体

```lz
enum Color:
    Red
    Green
    Blue
```

- 每个变体单独一行，用缩进表示属于该枚举
- 无数据的变体在 LZ 中称为"单元变体"（unit variant）
- 构造时使用 `Color.Red` 语法

---

## 三、带数据变体

```lz
enum Shape:
    Circle(x: f64, y: f64, radius: f64)
    Rectangle(width: f64, height: f64)
    Square(side: f64)
```

- 变体可以携带**具名字段**（类似 struct 的字段语法）
- 每个字段**必须**带类型注解
- 不同变体可以携带不同数量和类型的字段

---

## 四、构造枚举值

```lz
let c = Color.Red                     // 无字段变体
let s = Shape.Circle(x: 0.0, y: 0.0, radius: 5.0)  // 有字段变体
```

- 无字段变体：`EnumName.Variant`
- 有字段变体：`EnumName.Variant(field: value, ...)` — 使用**关键字参数**构造
- **预导入变体简写**：`Option` 的 `Some`/`None` 与 `Result` 的 `Ok`/`Err` 支持直接构造简写，`Some(42)` ≡ `Option.Some(42)`（见 [01-类型系统.md](01-类型系统.md) §2.2）；自定义枚举变体一律用路径构造。

---

## 五、泛型枚举

```lz
enum Result<T, E>:
    Ok(value: T)
    Err(error: E)

enum Option<T>:
    Some(value: T)
    None
```

- `Option<T>` — 标准可选值类型，`Some` 携带值，`None` 表示空
- `Result<T, E>` — 标准结果类型，`Ok` 表示成功，`Err` 表示错误
- 泛型参数紧跟枚举名，使用尖括号 `<T, E>`

---

## 六、模式匹配

枚举值通常与 `match` 表达式配合使用以解构变体：

```lz
match shape:
    Shape.Circle(x: x, y: y, radius: r) =>
        print(f"Circle at ({x}, {y}) with radius {r}")
    Shape.Rectangle(width: w, height: h) =>
        print(f"Rectangle {w}x{h}")
    Shape.Square(side: s) =>
        print(f"Square side {s}")
```

> 详细 match 语法见 **[05-控制流](05-控制流.md)**。

```lz
// 带位置模式匹配（元组变体，按顺序取字段）
match c:
    case Circle(x, y, r) =>
        print(f"Circle at ({x},{y}) radius {r}")
    case Rect(x, y, w, h) =>
        print(f"Rect at ({x},{y}) {w}x{h}")
```

---

## 七、枚举方法

enum 可定义方法，语法与 struct 方法完全一致：

```lz
enum Option<T>:
    Some(T)
    None

    def is_some(self) -> bool =
        match self:
            case Some(_) => True
            case None => False

    def unwrap_or(self, default: T) -> T =
        match self:
            case Some(v) => v
            case None => default
```

enum 方法自动在所有变体上可用。使用 `self` 在方法中匹配当前变体。

> 方法语法详见 **[06a-struct.md §三](06a-struct.md)**。

---

## 八、错误语法边界

```lz
// ❌ enum 用 = 而非 :
enum Bad =           // 错误：enum 需用 : 而非 =
    Red

// ❌ 变体字段缺少类型注解
enum Bad:
    Circle(x, y, radius)  // 错误：缺少类型注解
```

---

*上一章：[06a-struct](06a-struct.md)* · *下一章：[06c-trait和impl](06c-trait和impl.md)*

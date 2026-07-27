# struct / enum / trait / impl

## 1. struct

```
struct Point =
    x: f64
    y: f64

struct Rectangle =
    width: f64
    height: f64

    def area(self: Rectangle)-> f64 =
        self.width * self.height

    def square(size: f64)-> Rectangle =
        Rectangle(size, size)

# 构造
p = Point(1.0, 2.0)
r = Rectangle.square(5.0)
```

## 2. 泛型 struct

```
struct Box<T> =
    value: T

    def map<U>(self: Box<T>, f: (T) -> U)-> Box<U> =
        Box(f(self.value))
```

## 3. enum

```
enum Option<T> =
    None
    Some(T)

enum Result<T, E> =
    Ok(T)
    Err(E)

enum Color =
    Red
    Green
    Blue

# 带数据的枚举（元组变体）
enum Message =
    Quit
    Move(int, int)
    Write(str)
```

## 4. 模式匹配 (解构)

```
match msg:
    case Quit => "bye"
    case Move(x, y) => f"move to ({x}, {y})"
    case Write(text) => f"write: {text}"
```

## 5. trait

```
trait Drawable =
    def draw(self: Self) = ...          # 抽象方法："= ..." 表示待实现（省略号不可省略）

trait Summary =
    def summarize(self: Self)-> str = ...  # 抽象方法必须有 "= ..."
    def default_method(self: Self) =      # 默认实现：有 "=" + 缩进体
        print("default impl")
```

## 6. impl

```
struct Circle =
    x: f64
    y: f64
    radius: f64

impl Drawable for Circle =
    def draw(self: Circle) =              # 实现 trait 抽象方法
        print(f"Circle at ({self.x}, {self.y})")

# 自动 trait（字段匹配）
struct Point =
    x: f64
    y: f64

trait HasPosition =
    x: f64
    y: f64

impl HasPosition for Point   # 字段自动匹配，无需手写

> ⚠️ 未实现（规划中）：当前编译器不会在 `impl Trait for Type` 时自动按字段名匹配 trait，上述 `impl HasPosition for Point` 字段自动匹配暂无法通过 `lzc` 编译。当前需手写对应方法实现 trait 抽象成员。
```

## 7. 关联类型

> ⚠️ 未实现（规划中）：`type Item` 关联类型当前编译器不解析，下列示例无法通过 `lzc` 编译。

```
trait Iterator =
    type Item
    def next(self: Self)-> Option<Self.Item>

impl Iterator for Counter =
    type Item = int
    def next(self: Counter)-> Option<int> = ...
```

## 8. 与 Rust 对照

| Lang-Zong | Rust |
|-----------|------|
| `struct Point = x: f64, y: f64` | `struct Point { x: f64, y: f64 }` |
| `enum Option<T> = None, Some(T)` | `enum Option<T> { None, Some(T) }` |
| `trait Drawable = def draw(self)` | `trait Drawable { fn draw(&self); }` |
| `impl Drawable for Circle =` | `impl Drawable for Circle { }` |
| `type Item` | `type Item;` |

## 语法边界

- ✅ `struct`/`enum`/`trait`/`impl` 体用 `=` 开启并缩进 4 空格：`struct Point =` 后换行缩进字段与方法。
  ❌ 用 `{}` 包裹体（`struct Point { x: f64 }` 非法）。
- ✅ 枚举仅支持元组变体：`Variant` / `Variant(T)` / `Variant(T1, T2)`。
  ❌ 具名字段变体：`Move(x: int, y: int)` 应写为 `Move(int, int)`。
- ✅ 方法内 `self` 默认 `&self`；方法体缩进于 `struct`/`impl` 之下。
  ❌ 方法不缩进或漏 `:`。
- ⚠️ 未实现：`impl Trait for Type` 的字段自动匹配（需手写方法）、关联类型 `type Item`（见上文标注）。

# LZ struct（结构体）

> 规范版本: 3.3 · 基于编译器源码 · 最后校订: 2026-08-05

本文档详细定义 Lang-Zone 的 struct（结构体）语法：基本定义、字段注解、方法、构造实例、泛型结构体、`__new__`/`__init__` 魔法方法，以及错误语法边界。enum、trait、impl 等类型见 **[06-数据结构](06-数据结构.md)**。

---

## 一、基本定义

struct 使用 `struct Name =` 语法定义，等号后换行缩进书写字段列表。

```lz
struct Point =
    x: f64
    y: f64
```

- 关键字 `struct` 后跟结构体名称（大写开头，PascalCase）
- `=` 后换行缩进（4 空格）
- 每个字段独占一行
- 所有 struct 自动 `#[derive(Clone)]`（编译器内置派生）

### 空结构体

允许定义不含字段的 struct（零大小类型，Zero-Sized Type）：

```lz
struct Marker =
// Marker 没有任何字段，但仍是有效的类型标识
```

---

## 二、字段

### 2.1 必须带类型注解

struct 的每个字段**必须**带类型注解，这与函数参数不同（函数参数允许省略类型注解依赖推断）。

```lz
struct Person =
    name: str       // ✅ 字段有类型注解
    age: int        // ✅ 字段有类型注解

struct Bad =
    x               // ❌ 编译错误：缺少类型注解
    y               // ❌ 编译错误：缺少类型注解
```

**与函数参数的区别**：

```lz
// 函数参数：类型注解可选
//def foo(x) = x + 1           // x 完全没有注解
//def bar(x: int) = x + 1      // X 返回值没有注解
def bar(x: int) -> int = x + 1      // ✅ 合法

// struct 字段：类型注解强制
struct Data =
    value: int               // ✅ 合法
//  value                  // ❌ 编译器报错
```

### 2.2 `__slots__` 内存布局控制

struct 可使用 `__slots__` 属性控制内存布局，映射为 Rust 的 `#[repr(...)]`：

```lz
struct MyStruct =
    __slots__ = "C"           // #[repr(C)] — C 兼容布局
    x: int
    y: int
```

有效值：`"C"` / `"packed"` / `"transparent"` / `"align(N)"`。详见 [06e-模块级魔法属性](06e-模块级魔法属性.md) §六。

### 2.3 字段类型支持

字段类型可以是任意合法类型：基础类型、复合类型、泛型参数、其他 struct、Self 引用等。

```lz
struct Example<T> =
    id: int
    name: str
    flag: bool
    data: T
    children: List<Option<T>>
    parent: Option<Self>    // 自引用：自动 Box → Option<Box<Self>>（见 06h §二）
```

**自引用字段**（`Self` / 类型全名直接引用自身）由编译器**自动插入独占间接层 `Box`**：`parent: Option<Self>` ≡ `Option<Box<Self>>`，构造自动 `Box::new`、访问透明。共享所有权须显式写 `Rc<Self>`/`Arc<Self>`。完整规则见 **[06h-自引用与Self关键字.md](06h-自引用与Self关键字.md)**。

### 2.4 字段访问

字段通过 `.` 操作符访问，语法与大多数语言一致：

```lz
let p = Point(x: 3.0, y: 4.0)
print(p.x)       // 3.0
print(p.y)       // 4.0
```

### 2.5 字段可见性（公开 / 私有）

LZ **没有 `pub` 关键字**。字段可见性完全由字段名前缀表达，这是一个全局约定，适用于 struct 定义、duck 约束以及所有类型字段：

| 写法 | 含义 | 外部模块是否可见 |
|------|------|------------------|
| `.name` | 公开字段 `name`（显式写法） | 是（公开 API 的一部分） |
| `._name` | 私有字段 `_name` | 否（仅本模块内可访问） |
| `name`（无前缀） | 公开字段（默认）；与 `.name` 等价 | 是 |

```lz
struct User =
    .id: int          // ✅ 公开字段：可被其他模块访问
    .name: str        // ✅ 公开字段
    ._token: str      // ✅ 私有字段：仅本模块内可见，外部不可访问
```

要点：

- **前缀即可见性**：`.field` 就是公开，`._field` 就是私有。你不需要、也不能写 `pub` 修饰符——`pub` 是 Rust 关键字，不属于 LZ 语法。
- **无前缀 = 公开（默认）**：`struct Point = x: f64` 中的 `x` 即公开字段，与显式 `.x` 完全等价。前缀 `.` 只是把"公开"写得更显眼，并不改变语义；只有 `._` 前缀才表示私有。因此下文所有无前缀示例（如 §2.1 的 `Point`、§2.3 的 `Example`）均为公开字段。
- **同一模块内无差别**：私有字段 `._x` 在定义它的模块内部可以正常读写，可见性限制只作用于**跨模块**访问。
- **与 duck 约束一致**：在 `duck` 中约束目标字段时同样用前缀表达——`.field: T` 要求目标有公开字段，`._field: T` 要求目标有私有字段。详见 [01b-duck关系约束.md §8.3.1](01b-duck关系约束.md)。

> 不要写：`struct S = pub id: int   // ❌ pub 不是 LZ 关键字`
> 应该写：`struct S = .id: int      // ✅ 公开字段用 . 前缀`

---

## 三、方法

### 3.1 定义方法

方法在 struct 体内使用 `def` 关键字定义，与字段处于相同缩进级别。

```lz
struct Rectangle =
    width: f64
    height: f64

    def area(self) -> f64 =
        self.width * self.height

    def scale(self, factor: f64) =
        self.width *= factor
        self.height *= factor
```

- 方法紧跟字段定义，缩进层级与字段相同
- `def` 关键字 + 方法名 + 参数列表 + 返回类型（可选）+ `=` + 函数体
- 函数体为表达式或缩进代码块

### 3.2 self 参数

方法的第一个参数为 `self`，表示调用实例的引用。

| 形式 | 语义 | 说明 |
|------|------|------|
| `self` | 不可变引用 `&Self` | 只读访问字段，不可修改 |
| `self: Self` | 不可变引用（显式） | 与 `self` 等价，显式写出类型 |
| `mut self` | 可变引用 `&mut Self` | 可修改字段 |

> **self 语义区分 — 普通方法 vs 魔法方法：**
>
> | 上下文 | `self` 默认语义 | 写法 |
> |--------|:---:|------|
> | 普通方法 | `&Self`（不可变引用） | `def m(self) = ...` |
> | 普通方法（可变） | `&mut Self` | `def m(mut self) = ...` |
> | 魔法方法 | `Self`（消费/owned） | `def __add__(self, other: T) -> T` |
> | 魔法方法（借用） | `&Self` | `def __str__(ref self) -> str = ...` |

```lz
struct Counter =
    count: int

    def value(self) -> int =
        self.count              // 只读访问

    def increment(mut self) =
        self.count += 1         // 修改字段

    def reset(mut self) =
        self.count = 0          // 修改字段
```

### 3.3 静态方法（无 self）

不需要实例即可调用的方法，参数列表中省略 `self`：

```lz
struct MathUtils =

    def pi() -> f64 =
        3.1415926535

    def double(x: int) -> int =
        x * 2
```

### 3.4 方法调用语法

```lz
let r = Rectangle(width: 10.0, height: 5.0)
let a = r.area()                    // 50.0
r.scale(2.0)                        // 修改 r
let pi = MathUtils.pi()             // 静态方法调用
```

---

## 四、构造实例

### 4.1 关键字参数构造（默认）

struct 实例通过**关键字参数**语法构造，字段名作为参数名，按名称匹配：

```lz
let p = Point(x: 3.0, y: 4.0)
let r = Rectangle(width: 10.0, height: 5.0)
let person = Person(name: "Alice", age: 30)
```

- 必须为每个字段提供值（struct 没有默认字段值）
- 参数顺序无关（关键字匹配字段名）
- 构造表达式的类型推断：编译器从 struct 名称推导返回类型

### 4.2 嵌套构造

```lz
struct Address =
    city: str
    street: str

struct User =
    name: str
    addr: Address

let u = User(
    name: "Bob",
    addr: Address(city: "Beijing", street: "ChangAn Ave")
)
```

### 4.3 与函数构造模式的对比

| 特性 | struct 关键字构造 | impl 工厂方法 |
|------|------------------|---------------|
| 语法 | `Name(field: val)` | `Name.new(args)` |
| 可自定义逻辑 | 否 | 是 |
| 隐藏字段 | 否 | 是 |
| 默认值 | 否 | 可实现 |

```lz
impl Rectangle =
    def new(width: f64, height: f64) -> Rectangle =
        Rectangle(width: width, height: height)

    def square(side: f64) -> Rectangle =
        Rectangle(width: side, height: side)

let r1 = Rectangle(width: 10.0, height: 5.0)   // 关键字构造
let r2 = Rectangle.new(10.0, 5.0)              // 工厂方法
let r3 = Rectangle.square(7.0)                 // 便捷构造器
```

---

## 五、泛型结构体

### 5.1 定义

struct 支持泛型参数，使用 `<T, U, ...>` 尖括号语法声明，泛型参数名紧跟 struct 名称。

```lz
struct Pair<T, U> =
    first: T
    second: U
```

- 泛型参数在编译期单态化（monomorphization）
- 同一泛型 struct 的不同具体类型为不同的独立类型

### 5.2 构造泛型实例

泛型 struct 的构造与普通 struct 相同，类型参数由字段值自动推断：

```lz
let p1 = Pair(first: 1, second: "hello")       // Pair<int, str>
let p2 = Pair(first: True, second: 3.14)       // Pair<bool, f64>
```

### 5.3 带泛型参数的方法

```lz
struct Box<T> =
    value: T

    def unwrap(self) -> T =
        self.value

    def replace(mut self, new_value: T) =
        self.value = new_value
```

### 5.4 泛型约束

```lz
struct OrderedPair<T: Ordered> =
    left: T
    right: T

    def larger(self) -> T =
        if self.left > self.right:
            self.left
        else:
            self.right
```

### 5.5 多约束与 where 子句

```lz
struct CloneablePair<T>
    where T: Clone =
    a: T
    b: T

    def clone_both(self) -> (T, T) =
        (self.a.clone(), self.b.clone())
```

---

## 六、`__new__` 和 `__init__`

**（设计考虑 — 当前默认行为 + 扩展规划）**

### 6.1 当前默认

当前编译器为每个 struct 自动生成关键字参数构造器 `Name(field: val, ...)`。这是纯粹的字段——值映射，不涉及任何自定义逻辑。

```lz
struct Point =
    x: f64
    y: f64

let p = Point(x: 3.0, y: 4.0)    // 编译器自动生成的构造
```

### 6.2 `__new__` 和 `__init__` 设计

struct 可选择性地实现 `__new__` 和 `__init__` 魔法方法，以定制实例化过程。这借鉴了 Python 的对象创建二分法，但适配 LZ 的值语义和不可变性倾向。

#### `__new__` — 分配器

```lz
struct Point =
    x: f64
    y: f64

    magic __new__(x: f64, y: f64) -> Self =
        Self(x: x, y: y)
```

- `__new__` 是**静态**方法（无 self 参数），负责**分配和构造**实例
- 返回类型为 `Self`，即当前 struct 类型
- 语义上承担"创建实例"的全部责任
- 如果 struct 实现了 `__new__`，则优先使用自定义逻辑（而非编译器默认的关键字构造）

> **`magic __new__` 内联语法糖**：在 struct 体内使用 `magic __new__` 是 `magic __new__: def __new__(...)` 声明的内联语法糖。等价于在 struct 外写独立的 magic 块。详见 [06f-magic用法.md](06f-magic用法.md)。

#### `__init__` — 初始化器

```lz
struct Point =
    x: f64
    y: f64

    magic __init__(mut self, x: f64, y: f64) =
        self.x = x
        self.y = y
```

- `__init__` 是**实例方法**，在 `__new__` **之后**被调用
- 接收已分配但未初始化的 `self`，负责字段赋值
- 调用链：`__new__` 分配实例 → `__init__` 初始化字段 → 返回完整实例

#### 二者协作

当 struct 同时定义了 `__new__` 和 `__init__`：

1. 编译器调用 `__new__` → 获得 `Self` 类型的实例
2. 编译器将该实例传给 `__init__` 进行初始化
3. 最终返回初始化后的实例

```lz
struct Config =
    host: str
    port: int
    debug: bool

    magic __new__(host: str, port: int) -> Self =
        Self(host: host, port: port, debug: false)

    magic __init__(mut self, host: str, port: int) =
        // __new__ 已设置 host 和 port
        // __init__ 可做额外验证或日志
        if self.port <= 0 or self.port > 65535:
            panic("invalid port")

let cfg = Config(host: "localhost", port: 8080)
```

#### `.new()` 语法

如果 struct 实现了 `__new__`（或 `__init__`），则 `Name.new(...)` 调用语法变为有效：

```lz
struct Database =
    url: str
    pool_size: int

    magic __new__(url: str) -> Self =
        Self(url: url, pool_size: 10)

let db = Database.new("postgres://localhost/test")
```

### 6.3 设计理由

| 方面 | 说明 |
|------|------|
| **默认简单** | 90% 的 struct 只需关键字构造，不必写 `__new__` |
| **可选定制** | 需要验证、默认值、副作用的 struct 可实现 `__new__` |
| **分离关注** | `__new__` 负责分配策略，`__init__` 负责字段级初始化 |
| **与 trait 兼容** | `__new__` 可作为 `Constructible` trait 的一部分 |

### 6.4 `__implicit_from__` — 隐式构造

当 struct 实现了 `__implicit_from__`，编译器可在类型不匹配时自动插入转换：

```lz
struct Port =
    value: int

    // 从 str 隐式构造
    magic __implicit_from__(s: str) -> Self =
        Self(value: s.parse().unwrap_or(8080))

    // 从 int 隐式构造
    magic __implicit_from__(n: int) -> Self =
        Self(value: n)
```

触发场景：

```lz
s = "3000"
let p: Port = s           // str → Port，自动转换

let n = 8080
let p2: Port = n          // int → Port，自动转换

def serve(port: Port) ->
    print(port.value)
serve("9090")             // 参数传递，自动转换
```

**与 `__new__` 的区别**：
- `__new__` → 覆盖默认构造器，`Port(n: 8080)` 调用时触发
- `__implicit_from__` → 编译器自动插入，`let p: Port = 8080` 时触发

### 6.5 注意

- `__new__` 和 `__init__` 的签名由用户自定义，不强制匹配字段列表
- 实现了 `__new__` 后，编译器默认的关键字构造是否仍可用有待确定
- 与 trait `Constructible` 的交互仍在设计阶段

---

## 七、错误语法边界

以下为常见的 struct 语法错误：

```lz
// ❌ struct 使用 : 而非 =
struct Bad:
    x: int              // 错误：struct 定义必须用 = 而非 :

// ❌ 字段缺少类型注解
struct Bad =
    x                   // 错误：每个字段必须带类型注解
    y: int              //   (x 缺少类型注解)

// ❌ 构造时遗漏字段
struct Point =
    x: f64
    y: f64

let p = Point(x: 1.0)   // 错误：缺少字段 y

// ❌ 构造时多余的字段
let p = Point(x: 1.0, y: 2.0, z: 3.0)  // 错误：Point 没有字段 z

// ❌ 泛型参数声明语法错误
struct Box T =           // 错误：应为 Box<T>
    value: T

// ❌ 字段类型引用不存在的泛型参数
struct Box<T> =
    value: U            // 错误：U 未声明（应为 T 或声明 U）

// ❌ self 参数类型标注错误
struct Counter =
    count: int
    def inc(self: int) =  // 错误：self 类型应为 Self，不是 int
        self.count += 1
```

### 正确对照

```lz
// ✅ struct 用 = 定义
struct Point =
    x: f64
    y: f64

// ✅ 所有字段带注解
struct Person =
    name: str
    age: int

// ✅ 完整的关键字构造
let p = Point(x: 1.0, y: 2.0)

// ✅ 泛型 struct 声明
struct Box<T> =
    value: T

// ✅ 泛型字段
struct Pair<T, U> =
    first: T
    second: U
```

---

*上一章：[05-控制流](05-控制流.md)* · *下一章：[06b-enum](06b-enum.md)*

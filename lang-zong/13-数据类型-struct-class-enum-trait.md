# 数据类型 — struct / class / enum / trait / impl

所有类型定义使用 `=` 开启定义体（替代 v2 的 `:`）。

## 1. struct — 不可变数据

```
struct Point<T> =
    x : T
    y : T

    // 构造函数
    def new(x: T, y: T) -> Point<T> =
        Point { x, y }

    // 方法（必须有实现）
    def length(Self) -> f64 =
        (self.x * self.x + self.y * self.y).sqrt()

    // 私有方法（_ 前缀）
    def _helper(Self) -> T =
        self.x + self.y
```

### 1.1 Trait 组合

`struct` 支持 trait 组合语法，`()` 内列出的 trait 的抽象方法必须在 struct 内实现：

```
struct Point<T>(Display, Clone) =
    x : T
    y : T

    def display(Self) -> str =
        f"Point({self.x}, {self.y})"

    def clone(Self) -> Self =
        Point(self.x, self.y)
```

| 特性 | 规则 |
|------|------|
| 字段 | 全部不可变（类似默认 `val` 语义） |
| 继承 | 不支持 |
| 抽象方法 | 不支持，trait 抽象方法必须全部实现 |
| 私有 | `_` 前缀 |
| 魔法方法 | 始终公开 |

### 1.2 空 struct

```
struct demo = ()
```

## 2. class — 可变数据，单继承

```
class 名称<泛型>(父类?, trait1, trait2, ...) =
    ...
```

父类必须放在 `()` 内**第一个**位置，trait 跟随其后：

```
class Animal =
    name : mut str

    def new(name: str) =
        self.name = name

    def speak(Self) -> str =
        "..."

class Dog(Animal, Display, Clone) =
    breed : mut str

    def new(name: str, breed: str) =
        self.name = name
        self.breed = breed

    def speak(Self) -> str =
        f"Woof! I'm {self.name}"

    def display(Self) -> str =
        f"Dog({self.name}, {self.breed})"

    def clone(Self) -> Self =
        Dog(self.name, self.breed)
```

转译为目标语言的**组合 + 委托**（如 Rust 的 `Deref`、Python 的属性委托）。

## 3. enum — 带标签的代数数据类型

```
enum Option<T> =
    Some(T)
    None

enum Result<T, E> =
    Ok(T)
    Err(E)

enum Color =
    Red(u8, u8, u8)
    Green(u8, u8, u8)
    Blue(u8, u8, u8)

enum Status =
    Active
    Inactive
    Banned
```

### 3.1 方法

```
enum HttpStatus =
    Ok(u16)
    NotFound(str)
    ServerError(u16, str)

    def code(Self) -> u16 =
        match self:
            case Ok(c):
                c
            case NotFound(_):
                404
            case ServerError(c, _):
                c
```

### 3.2 Trait 组合

```
enum Order(Display, Clone) =
    Pending
    Shipped(str)

    def display(Self) -> str =
        match self:
            case Pending:
                "Pending"
            case Shipped(id):
                f"Shipped({id})"

    def clone(Self) -> Self =
        match self:
            case Pending:
                Order.Pending
            case Shipped(id):
                Order.Shipped(id)
```

## 4. trait — 名义类型接口

trait 仅支持方法（抽象或默认实现），**不支持字段**：

```
trait Iterator<T> =
    // 抽象方法（无函数体）
    def next(mut Self) -> T?

    // 默认实现
    def collect(Self) -> [T] =
        items : mut [T] = []
        loop:
            match self.next():
                case Some(v):
                    items.append(v)
                case None:
                    break items

    // 类方法（首参 Type）
    def empty(Type) -> Self

    // 静态方法（首参非 Self/Type）
    def helper(x: int) -> bool =
        x > 0
```

### 4.1 方法类型由首参数决定

| 首参 | 含义 | 调用方式 |
|------|------|----------|
| `Self` | 实例方法 | `instance.method()` |
| `Type` | 类方法 | `TypeName::method()` |
| 其他 | 静态方法 | `TypeName::method()` |

### 4.2 Trait 组合 `&`

```
trait Read =
    def read(Self) -> str

trait Write =
    def write(mut Self, data: str)

ReadWrite = Read & Write

impl ReadWrite for File =
    def read(Self) -> str =
        ...
    def write(mut Self, data: str) =
        ...
```

## 5. impl — 为类型实现 trait

```
impl TraitName for TypeName =
    def method(Self) -> T =
        ...
```

```
struct Counter =
    n : mut int

impl Iterator<int> for Counter =
    def next(mut Self) -> int? =
        current = self.n
        self.n += 1
        Some(current)
```

## 6. 魔法方法

通过实现魔法方法定义操作符语义：

```
struct Vector =
    x : f64
    y : f64

    def __add__(Self, other: Vector) -> Vector =
        Vector(self.x + other.x, self.y + other.y)

    def __getitem__(Self, index: int) -> f64 =
        match index:
            case 0:
                self.x
            case 1:
                self.y
            case _:
                panic("out of bounds")

    def __iter__(Self) =
        yield self.x
        yield self.y

    def __eq__(Self, other: Vector) -> bool =
        self.x == other.x and self.y == other.y

    def __call__(Self, t: f64) -> Vector =
        Vector(self.x * t, self.y * t)

    def __enter__(Self) =
        self

    def __exit__(Self) =
        pass
```

完整魔法方法列表见 [01-附录-关键字与保留字.md](01-附录-关键字与保留字.md)。

## 7. 上下文管理器

`with` 语句绑定资源到作用域，退出时自动释放：

```
with open(path) as f:
    process(f.read())
```

`__enter__` 返回资源，`__exit__` 负责清理（无异常参数）：

```
struct Connection =
    host : mut str
    port : mut int

    def __enter__(Self) -> Connection =
        self.connect()
        self

    def __exit__(Self) =
        self.disconnect()
```

## 8. 已移除的特性

| 特性 | 状态 | 说明 |
|------|------|------|
| `extend` 借用方法 | ❌ 移除 | 用 `impl` 或在 trait 中定义 |
| `omit` 移除方法 | ❌ 移除 | 不支持运行时移除方法 |
| `define` 结构类型约束 | ❌ 移除 | 用 `trait` + `where` |
| `extend Enum` 增加枚举值 | ❌ 移除 | 枚举值在定义时固定 |
| `*fix` 自定义操作符 | ❌ 移除 | 不支持自定义符号 |

## 9. 类型对比

| | struct | enum | class |
|---|---|---|---|
| 字段 | 固定、不可变 | 变体各自携带数据 | 固定、可变 |
| 继承 | 无 | 无 | 单继承 |
| 方法 | 支持 | 支持 | 支持 |
| trait 实现 | 支持 | 支持 | 支持 |
| 适用 | 数据聚合 | 多种可能性 | 可变状态 + 继承 |

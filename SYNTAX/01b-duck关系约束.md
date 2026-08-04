# Duck 关系约束——多类型间结构化关系

> 规范版本: 3.3 · 基于编译器源码 · 最后校订: 2026-08-04

> **参考**: Nim `concept` · Rust `trait` bounds · Go interface 结构匹配

---

## 一、基本动机

现有 `duck`（[01-类型系统.md](01-类型系统.md) §九）描述的是"单类型满足什么结构"——属性型约束。扩展后的 `duck` 要解决的是"**多类型之间存在怎样的结构关系**"——**关系型约束**。

### 两种约束的对比

```lz
// 属性型（见 01-类型系统.md §九）—— "T 是什么"
duck Quackable =
    def quack(self) -> ()

// 关系型（本章）—— "T 和 R 之间有什么关系"
duck Mapper<T, R> =
    def T.map(self) -> R
    def R.unmap(self) -> T
```

### 为什么需要关系型约束

| 场景 | 单类型 duck | 多类型关系 duck |
|------|:-----------:|:---------------:|
| "T 有 map 方法" | ✅ | — |
| "T.map 返回的类型就是 R" | ❌ | ✅ |
| "T 和 R 有对称的方法" | ❌ | ✅ |
| "T 的元素类型与 R 一致" | ❌ | ✅ |
| "T 和 R 是协变的" | ❌ | ✅ |

---

## 二、语法设计

### 2.0 泛型与约束语法

`duck` 的泛型参数使用尖括号 `<>` 声明。约束支持两种等价写法（与 Rust 一致）：

| 形式 | 语法 | 说明 |
|------|------|------|
| 尖括号内联 | `<T: Quackable>` | Rust 风格，推荐；约束直接写在参数声明处 |
| where 子句 | `where T: Quackable` | 函数 / 类型体之前的等价写法，约束较多时更清晰 |

> **注意**：`[]` 是**运行时检查站**语法，只用于运行期断言，**不用于泛型约束**——泛型约束一律走尖括号或 `where`。

> **注解运算符速记**：约束用 `:`（如 `<T: Clone>` / `where T: Clone`）；类型等同用 `==`（如 `A.id == B.id`）。LZ 是结构类型系统、**无名义继承**，型变（协变/逆变/不变）由编译器按位置**自动推断**（与 Rust 一致），不提供也不需 `<:` / `>: ` 标注。完整对照见 [§3.2 注解运算符](#32-注解运算符--)。

```lz
// 1. 尖括号内联（推荐，对齐 Rust）—— 约束写在参数处
def make_quack<T: Quackable>(x: T) = x.quack()

// 2. where 子句 —— 等价写法
def make_quack<T>(x: T)
    where T: Quackable =
    x.quack()

// 混合：T 为带约束泛型，R 为普通泛型
def mix<T: HasArea, R>(shape: T, extra: R) = ...
```

### 2.1 多泛型参数 + 类型前缀

```lz
duck Mapper<T, R> =
    def T.map(self) -> R          // T 有 map 方法，返回值类型为 R
    def R.unmap(self) -> T        // R 有 unmap 方法，返回值类型为 T
```

当 `duck` 有**多个泛型参数**时，方法声明前需加上 `TypeName.` 前缀指明**所属类型**。

**规则**：多参数时方法前缀**必须**（歧义消除）；单参数时可省略（退化为 [01-类型系统.md](01-类型系统.md) §九 的单参数 duck 语法）。

```
duck Q<T> =                        // 单参数：兼容 [01-类型系统.md](01-类型系统.md) §九 语法
    def quack(self) -> ()

duck Q<T, R> =                     // 多参数：必须前缀
    def T.map(self) -> R
    def R.unmap(self) -> T
```

### 2.2 字段级关系

```lz
duck SameFields<A, B> =
    A.x: f64                       // A 有 x: f64 字段
    B.x: f64                       // B 也有 x: f64 字段（类型相同但不必联系）

duck LinkedFields<A, B> =
    A.id == B.id                   // A.id 的类型必须等于 B.id 的类型
    A.name: B.name                 // A.name 类型必须等于 B.name 类型（简写）
```

### 2.3 关联类型关系

```lz
duck IterPair<T, I> =
    type I.Item                     // I 有关联类型 Item
    def T.__iter__(self) -> I       // T.__iter__ 返回 I
    def I.__next__(mut self) -> T   // I.__next__ 返回 T
```

### 2.4 嵌套 duck 约束

```lz
duck DoubleMap<T, R> where T: Iterable, R: Iterable =
    def T.map(self) -> R            // T.map 返回另一个 Iterable
    
duck CrossCheck<A, B, Eq> where Eq: Equals =
    def Eq.equals(self, a: A, b: B) -> bool   // Eq 提供 equals(a: A, b: B) -> bool
```

---

## 三、关系运算符

### 3.1 类型投影运算符

| 运算符 | 含义 | 示例 |
|--------|------|------|
| `T.method -> R` | T.method 的返回类型为 R | `def T.produce() -> R` |
| `T.method ↦ R` | T.method 的参数类型为 R（简写 `T.method(R)`） | `def T.consume(R)` |
| `T.field : R` | T.field 的类型等于 R | `T.name: str` |

实际语法中使用 `->` 箭头（与函数签名一致）：

```lz
// 返回类型投影
def T.produce(self) -> R          // produce 的返回类型 = R

// 参数类型投影
def T.consume(self, x: R) -> ()   // consume 的参数类型 = R
```

### 3.2 注解运算符：` : ` / ` == `（约束 vs 类型等同）

`duck` 与泛型约束里会用到三种"注解"运算符，含义各不相同——**混用是文档里最常见的错误来源**。下面一次说清：

| 运算符 | 名称 | 含义 | 何时用 | 示例 |
|--------|------|------|--------|------|
| `:` | 约束 / bound | **T 满足约束 X**：实现某个 trait，或满足某个 duck 的结构要求 | 给泛型参数加"能力"约束（最常用） | `<T: Clone>`、`where T: Quackable` |
| `==` | 类型等同 | 两侧类型必须完全相同 | 字段类型必须一致时 | `A.id == B.id` |

> **型变（协变 / 逆变 / 不变）由编译器自动推断，无需标注**
> LZ 是结构类型系统、**没有名义继承**，因此也没有"子类型运算符"。泛型参数的型变方向由它在签名中的**位置**决定，编译器在类型检查阶段自动算出（与 Rust 一致）：
> - 只出现在**返回类型**（产出）→ **协变**
> - 只出现在**参数**（消费）→ **逆变**
> - 同时出现在输入与输出 → **不变**
>
> 因此你**不需要、也不能**写 `<:` / `>: ` 来标注型变——这是有意为之的简化，也消除了此前文档里"`:` / `<:` / `>:` 三者混淆"的问题。

> **关键区别（避免混淆）**
> - `:` 说的是"**能力**"：T 必须能提供 X 要求的方法/字段。这是 99% 场景该用的。
> - `==` 说的是"**类型等同**"：两侧类型必须完全一致（见 §2.2 / §2.3 的 `A.id == B.id`、`A.name: B.name`）。
> - 协变 / 逆变**不靠运算符表达**，而由编译器按位置自动推断（见上方说明框）。
>
> 注：`:` 在泛型声明处是约束（`<T: X>`），在 duck 字段行 `A.x: f64` 是字段类型标注——都是"X 具备类型/约束 Y"的同一语义，不会冲突。

### 3.3 协变 / 逆变（编译器自动推断，无需标注）

型变方向由泛型参数出现在**产出**还是**消费**位置决定——但**你不用写任何标注**，编译器在类型检查阶段自动算出（与 Rust 一致）：

```lz
// 协变：T 只出现在「产出」位置（返回类型）→ 编译器自动判定为协变
duck Producer<T> =
    def make(self) -> T

// 逆变：T 只出现在「消费」位置（参数）→ 编译器自动判定为逆变
duck Consumer<T> =
    def eat(self, x: T) -> ()

// 不变：T 同时出现在输入与输出位置 → 编译器自动判定为不变
duck Cell<T> =
    def get(self) -> T
    def set(self, x: T) -> ()
```

判断口诀：

- **协变**：方法只返回 T（产出）→ 不用写任何东西，编译器自动处理。
- **逆变**：方法只接收 T（消费）→ 同上，自动处理。
- **不变**：既接收又返回 T → 自动处理。

> 因为 LZ 是结构类型系统、没有名义继承，所以不存在"子类型运算符"。`Producer` / `Consumer` / `Cell` 只是用来说明**位置决定型变**的三种结构形态；具体类型之间的相容性由 `:` 约束（trait / duck）在实例化点检查。

---

## 四、关系运算符的编译期检查

### 4.1 检查时机

与 `duck` 的基本检查一致——在**泛型实例化时**（monomorphization）：

```lz
def process<T, R>(x: T, y: R)
    where Mapper<T, R> =
    let mapped = x.map()              // 检查：T.map → R
    let restored = y.unmap(mapped)    // 检查：R.unmap → T
```

### 4.2 类型投影解析规则

投影 `T.method -> R` 的检查：

```
1. 在 T 的实际类型中查找 method
2. 提取 method 的返回类型 actual_return
3. 检查 actual_return 是否等于（或满足）R
   - 如果 R 是具体类型：actual_return == R
   - 如果 R 是 duck 约束：actual_return : R（满足该 duck 约束）
   - 如果 R 是泛型参数：actual_return 的类型与 R 的参数类型一致
```

### 4.3 结构相容性检查

当某类型 `Sub` 被要求满足约束 `Super`（trait / duck）时，编译器在**泛型实例化时**做结构相容性检查：

```
1. 检查 Sub 的实际类型是否满足 Super 的结构要求（约束 / duck）
2. 满足的定义：
   - Sub == Super ✅
   - Sub 实现了 Super 的 trait ✅
   - Sub 满足 Super 的所有 duck 约束 ✅
   - Sub 与 Super 结构匹配（无需名义继承）✅
```

> LZ 没有名义继承，因此"子类型"在这里就是"结构匹配"——只要 Sub 提供了 Super 要求的方法/字段即视为相容，不需要声明继承关系。

---

## 五、典型场景

### 5.1 函数式：映射器模式

```lz
duck Mapper<T, R> =
    def T.map(self) -> R

// 用法：任何有 map() 方法且返回 R 的类型都满足
def transform<T, R>(items: List<T>) -> List<R>
    where Mapper<T, R> =
    let result: List<R> = []
    for x in items:
        result.push(x.map())        // 检查：x.map() → R
    result

struct Wrapper<T> =
    value: T
    def map(self) -> T = self.value   // Wrapper<T>.map → T

let ws = [Wrapper(value: 10), Wrapper(value: 20)]
let nums = transform(ws)          // 推断：T = Wrapper<int>, R = int
```

### 5.2 双端迭代器

```lz
duck DoubleEnded<T, I> =
    def T.__iter__(self) -> I
    def I.__next__(mut self) -> T
    def I.__rev__(mut self) -> I          // 反向迭代器返回同类型
```

### 5.3 验证器配对

```lz
duck Validator<T, E> =
    def T.validate(self) -> Result<T, E>  // T.validate 返回 T 或 E
    def E.message(self) -> str            // E 必须有 message 方法

// 不相关的类型自动满足
struct Email =
    value: str
struct ValidationError =
    value: str

impl Email =
    def validate(self) -> Result<Email, ValidationError> =
        if "@" in self.value:
            Ok(self)
        else:
            Err(ValidationError(value: "invalid email"))

impl ValidationError =
    def message(self) -> str = self.value

// 这里 Email 和 ValidationError 自动满足 Validator<T=Email, E=ValidationError>
// 不需要 impl Validator for ...
```

### 5.4 型变容器（型变由编译器自动推断）

容器的型变方向**不靠标注**，而由泛型参数在签名中的位置决定。下面三个 duck 分别展示了协变 / 逆变 / 不变三种结构，编译器在类型检查时自动识别：

```lz
// 协变：In（经 Out 间接）只出现在「产出」位置 → 编译器自动判定协变
duck CovariantBox<In, Out> =
    def get(self) -> Out

// 逆变：Out 只出现在「消费」位置（参数）→ 编译器自动判定逆变
duck ContravariantBox<Out, In> =
    def put(self, x: Out) -> ()

// 不变：T 同时出现在输入与输出 → 编译器自动判定不变
duck Cell<T> =
    def get(self) -> T
    def set(self, x: T) -> ()
```

> 使用这些 duck 时，具体类型之间的相容性仍由 `:` 约束在实例化点检查（LZ 是结构类型系统，没有名义继承，因此不存在 `Dog <: Animal` 这类子类型声明）。需要"产出 Dog 的盒子可当作产出 Animal 的盒子"时，让 `Animal` 成为 duck / trait，`Dog` 在结构上满足它即可。

### 5.5 编解码对

```lz
duck Codec<T, Encoded> =
    def T.encode(self) -> Encoded
    def Encoded.decode(self) -> T

// 示例：Json 编解码
struct User =
    id: int
    name: str
struct Json =
    value: str

impl User =
    def encode(self) -> Json = Json(value: f"id:{self.id},name:{self.name}")

impl Json =
    def decode(self) -> User = User(id: 1, name: "parsed")  // 简化

// 自动满足 Codec<T=User, Encoded=Json>
```

---

## 六、与 trait 的本质区别

```
│ 维度          │ trait（名义属性）  │ duck 关系约束（关系型）      │
│──────────────│───────────────────│───────────────────────────│
│ 作用对象       │ 单个类型           │ 多个类型之间的关系          │
│ 匹配方式       │ T: Xxx            │ T, R: Xxx<T, R>          │
│ 跨类型方法     │ ❌ 无法表达        │ ✅ T.f → R 投影           │
│ 类型间关系     │ ❌ 无法表达        │ ✅ == 类型等同 / 结构约束匹配  │
│ 协变/逆变      │ ❌ 不支持          │ ✅ 编译器按位置自动推断    │
│ 第三方类型     │ ❌ 需要 impl       │ ✅ 结构匹配自动满足        │
│ 运行时开销     │ 零（静态分发）      │ 零（编译期展开）           │
```

**一句话区分**：
- `trait Drawable` = "**某个类型**可以干什么"
- `duck Mapper<T, R>` = "**两个类型之间**有什么结构关系"

---

## 七、语法边界

```lz
// ❌ 多参数 duck 必须使用类型前缀
duck Bad<T, R> =
    def map(self) -> R            // 错误：歧义，map 是 T 的还是 R 的？

// ✅ 正确
duck Good<T, R> =
    def T.map(self) -> R

// ❌ duck 体内不能有字段实现
duck Bad2<T, R> =
    def T.map(self) -> R = ...   // 错误：duck 不能有方法体

// ✅ 只能有签名
duck Good2<T, R> =
    def T.map(self) -> R          // 仅签名，无实现

// ❌ duck 不能实例化（不能当构造器使用）
let x = Mapper<int, str>(...)   // 错误：duck 是约束，不可实例化

// ✅ duck 可用作类型注解（与 trait 相同）
let x: Mapper<int, str> = ...     // 关系型 duck 主要用作泛型约束；此处仅示意可作类型注解

// ✅ 作为泛型约束
def f<T, R>(...) where Mapper<T, R>   // 正确

// ❌ 类型投影不能指向不存在的成员
duck Bad3<T, R> =
    def T.nonexistent(self) -> R  // 编译时错误（在实例化点检查）
```

---

## 八、编译期静态检查（TypeScript 级别安全）

### 8.1 方法签名约束系统

`duck` 对方法签名的校验分为三级，每级提供不同的精度：

| 级别 | 语法 | 说明 |
|------|------|------|
| **精确匹配** | `def foo(self) -> int` | 返回类型必须为 `int`，参数类型精确相等 |
| **约束匹配** | `def foo(self) -> T where T: Numeric` | 返回类型满足某个 trait/duck 约束即可 |
| **链式推断** | `def foo(self) -> R; def R.bar(self) -> T` | R 是中间类型，其 `bar()` 的返回类型再约束 T |

**精确匹配**（已有）：

```lz
duck ExactMatch =
    def process(self, x: int) -> str       // x 必须 int，返回必须 str
```

**约束匹配**：

```lz
duck ConstrainedMatch<T> =
    def process(self) -> T where T: Display  // 返回类型只要实现了 Display 就 OK
    def handle(self, x: T) -> () where T: Clone  // 参数只要实现了 Clone 就 OK
```

**链式推断**：

```lz
duck Chained<I, R> =
    def get_items(self) -> I                // get_items 返回 I
    def I.process(self) -> R                // I 必须有 process() 返回 R
    def R.to_string(ref self) -> str        // R 必须有 to_string() 返回 str

// 编译期自动推断链：T.get_items → I → I.process() → R → R.to_string() → str
// 实例化时整条链一步校验，任一环节不满足即报错
```

### 8.2 参数约束系统

#### 8.2.1 位置参数数量约束

| 约束语法 | 含义 | 示例 |
|----------|------|------|
| `exact(N)` | 必须有恰好 N 个位置参数 | `def f(self, exact(2))` |
| `max(N)` | 位置参数不超过 N 个 | `def f(self, max(3))` |
| `min(N)` | 位置参数至少 N 个 | `def g(self, min(1))` |
| `range(L, R)` | 位置参数在 L~R 之间 | `def h(self, range(1, 5))` |
| `..` | 任意数量（已有） | `def f(self, ..)` |

```lz
duck ParamConstrained<T> =
    // 方法必须恰好接受 2 个位置参数
    def set_pair(self, a: int, b: str) -> ()

    // 方法共可接受 1~3 个位置参数（key 必填 1 个；range(0,2) 再允许 0~2 个）
    def configure(self, key: str, range(0, 2)) -> ()

    // 方法至少需要 1 个参数
    def process(self, min(1)) -> T
```

#### 8.2.2 命名参数约束

| 约束语法 | 含义 |
|----------|------|
| `require(a, b)` | 目标方法调用时必须提供命名参数 a, b |
| `optional(a, b)` | 目标方法的命名参数 a, b 可选（有默认值） |

`require` / `optional` 作为 duck 体内的**独立约束行**书写（紧跟在被约束的方法之后）：

```lz
duck Buildable<T> =
    def build(self) -> T
    require(name: str, version: int)        // build 必须能接受 name 和 version 命名参数
    def init(self, key: str) -> ()
    optional(timeout: int)                  // init 可额外接受可选的 timeout 命名参数

duck Sendable =
    def send(self, ..) -> ()
    require(to: str, body: str)             // send 必须提供 to 和 body 命名参数
```

编译期检查：调用 `build(name="x")` 缺少 `version` 报错；`build(name="x", version=1, extra=true)` 检查 `extra` 是否为已知命名参数（否则报错，除非 duck 显式允许额外参数）。

#### 8.2.3 形参类型约束

限制参数只能为**栈类型（StackType）**或**引用类型（RefType）**：

```lz
// 类型分类
// StackType: int, f64, bool, (), str, (T), struct（无 self 引用字段）
// RefType:   List<T>, Dict<K,V>, Set<T>, struct(含引用字段), Box<T>, Iterator<T>

duck LowLevel =
    // 参数只能为栈类型（零开销，无堆分配）
    def compute(self, a: StackType, b: StackType) -> StackType

    // 参数只能为引用类型（含堆分配）
    def store(self, data: RefType) -> ()

    // 混合：按场景区分
    def transform(self, id: StackType, payload: RefType) -> StackType
```

编译期规则：
- `StackType` 参数：实际类型必须是 Copy 语义（`int`/`f64`/`bool`/`()` 等）
- `RefType` 参数：实际类型必须是 Move 语义（`List`/`Dict`/`Box` 等）
- 若实际类型的类别与声明不匹配，编译期报错

### 8.3 字段与方法修饰符规则

#### 8.3.1 字段可见性约束

LZ 字段可见性由字段名前缀表达：`.name` 即公开字段，`._name` 即私有字段。duck 约束中要求目标类型存在公开字段，直接写 `.field` 即可（无 `pub` 关键字）。

```lz
duck PublicFields =
    .name: str                    // .name 即公开字段，要求目标有公开 name
    .version: int

duck InternalState =
    .counter: int                 // 只要存在该字段即可
```

编译期规则：
- `.x: T`：要求目标类型有**公开**字段 `x: T`（`.x` 即公开）
- `._x: T`：要求目标类型有**私有**字段 `_x: T`（`._x` 即私有）

#### 8.3.2 方法 self 修饰符匹配

| duck 声明 | 匹配规则 | 示例实际方法 |
|-----------|----------|-------------|
| `def foo(self)` | 匹配任何 self 形态 | `fn foo(self)` / `fn foo(&self)` |
| `def foo(ref self)` | 只能匹配 `&self` | `fn foo(&self)` |
| `def foo(mut self)` | 只能匹配 `&mut self` | `fn foo(&mut self)` |
| `def foo(owned self)` | 只能匹配消费 self | `fn foo(self)` |

```lz
duck RefMethods =
    def reader(ref self) -> str        // 必须是 &self
    def writer(mut self) -> ()         // 必须是 &mut self
    def consumer(owned self) -> ()     // 必须是 self（消费）
```

#### 8.3.3 修饰符编译期校验表

| 修饰符 | duck 中声明 | 编译期校验规则 |
|--------|------------|--------------|
| `.`（公开） | `.field: T` | 目标必须有公开字段 `field`，否则报错 |
| `._`（私有） | `._field: T` | 目标必须有私有字段 `_field`，否则报错 |
| `ref` | `def foo(ref self)` | 目标方法 self 必须为 `&self` |
| `mut` | `def foo(mut self)` | 目标方法 self 必须为 `&mut self` |
| `owned` | `def foo(owned self)` | 目标方法 self 必须为 `self`（move） |
| `comptime` | `def foo(comptime self)` | 目标方法必须为编译期纯函数 |
| `unsafe` | `def foo(unsafe self)` | 目标方法必须标记 `unsafe` |

### 8.4 方法名正则模式匹配

`duck` 中方法名支持正则表达式，用 `/pattern/flags` 括起：

```lz
duck PatternMatched<T> =
    // 精确方法名（已有）
    def reset(self) -> ()

    // 正则模式：匹配 get_ 开头的任何方法，返回 int
    def /get_\w+/ (ref self) -> int

    // 正则模式：匹配 set_ 前缀，参数为 int
    def /set_\w+/ (mut self, value: int) -> ()

    // 正则模式：匹配 process_ 前缀且返回类型为实现 Display 的类型
    def /process_\w+/ (self, input: str) -> T where T: Display

    // 数量限定：至少有 1 个匹配 get_\d+ 的方法
    match /get_\d+/ at_least(1)
```

**匹配规则**：

| 正则模式 | 匹配示例（实际类型的方法名） |
|----------|---------------------------|
| `/get_\w+/` | `get_name`, `get_value`, `get_count` ✅ |
| | `getter` ❌（没有下划线） |
| `/set_\w+/` | `set_name`, `set_version` ✅ |
| `/is_\w+/` | `is_valid`, `is_ready`, `is_empty` ✅ |
| `/to_\w+/` | `to_string`, `to_json`, `to_list` ✅ |
| `/(add|remove|update)_\w+/` | `add_item`, `remove_item`, `update_record` ✅ |

**数量约束**：

| 约束 | 含义 |
|------|------|
| `at_least(N)` | 实际类型中至少有 N 个方法匹配该模式 |
| `at_most(N)` | 最多 N 个 |
| `exact(N)` | 恰好 N 个 |

```lz
duck HasMultipleGetters<T> =
    // 至少有 1 个 get_ 方法
    def /get_\w+/ (ref self) -> T where T: Display
    match /get_\w+/ at_least(1)

duck CRUDService =
    // 必须有 create, read, update, delete 四个方法
    def /(create|read|update|delete)_\w+/ (self, ..) -> ()
    match /(create|read|update|delete)_\w+/ exact(4)
```

### 8.5 整体编译期绑定流程

`duck` 的编译期静态检查分为三个步骤：

```
阶段 1：定义期（Parse duck 块时）
├── 解析方法签名（精确/约束/正则）
├── 解析参数约束（位置数/命名/类型类别）
├── 解析字段可见性（`.` 公开 / `._` 私有）与修饰符（ref/mut/comptime）
├── 构建约束表（ConstraintTable）
└── 注意：此时不报错，duck 块不绑定任何具体类型

阶段 2：实例化期（泛型函数调用时）
├── 获取实际类型的方法列表（反射＋AST 扫描）
├── 逐条约束匹配：
│   ├── 精确匹配 → 方法名 + 签名精确相等
│   ├── 约束匹配 → 方法名 + 返回类型满足约束
│   ├── 正则匹配 → 方法名正则 + 参数签名 + 数量约束
│   └── 修饰符匹配 → 字段可见性（`.`/`._`）与 ref/mut/comptime 校验
├── 链式推断 → 跨方法类型跟踪
├── 收集所有错误（非短路，全部上报）
└── 全部通过 → monomorphization 生成代码

阶段 3：派发期（函数体编译）
├── 使用约束表中的类型信息进行代码生成
├── 零运行时开销 —— 检查全部在阶段 2 完成
└── 运行时：直接调用对应方法，无反射/动态派发
```

**错误报告示例**：

```lz
def process<T, R>(x: T, y: R)
    where Mapper<T, R> =
    let mapped = x.map()              // ✅ T.map() → R
    let back = y.unmap(mapped)        // ❌ R 没有 unmap 方法
```

编译期输出：

```
error[DC001]: duck constraint violation at instantiation point
  ──> demo.lz:10
   │
10 │     let back = y.unmap(mapped)
   │                ^^^^^^^^^^^^^^^
   │
   = duck `Mapper<T, R>` requires `R.unmap(self) -> T`
   = actual type `str` has no method `unmap`
   = bound at: demo.lz:5  where Mapper<T, R>
```

### 8.6 完整示例：TypeScript 级别类型安全的列表转换器

```lz
// ── 1. 定义关系约束 ──

// 关系约束：IterablePair 描述 T 与 I 之间的迭代关系
duck IterablePair<T, I> =
    type I.Item                              // I 有关联类型 Item
    def T.__iter__(self) -> I                // T.__iter__ 返回 I
    def I.__next__(mut self) -> T            // I.__next__ 返回 T
    match /__\w+/ exact(2)                   // 恰好 2 个 dunder 方法（__iter__ / __next__）

// 转换器约束：T 与 R 之间有 map 关系（I 为迭代器类型）
duck Transform<T, R, I> where T: IterablePair<T, I> =
    def T.map(self, f: fn(I.Item) -> R) -> R   // T.map 接受闭包 fn(I.Item) -> R，返回 R
    match /map\w*/ at_least(1)                  // 至少有 1 个 map 前缀方法（map 本身也匹配）

// ── 2. 实现类型（不相关 struct，结构匹配自动生效）──

struct MyIter<T> =
    items: List<T>
    index: int
    def __next__(mut self) -> T =
        if self.index < self.items.length():
            let val = self.items[self.index]
            self.index += 1
            val
        else:
            raise StopIteration

struct MyList<T> =
    items: List<T>
    def __iter__(self) -> MyIter<T> =
        MyIter(items: self.items, index: 0)
    // 自动满足 IterablePair<T=MyList, I=MyIter<T>>

// ── 3. 泛型函数 ──

def transform_list[T, R, I](items: List<T>) -> List<R>
    where Transform<T, R, I> =                 // 编译期静态检查
    let result: List<R> = []
    for item in items:
        result.push(item.map(f: |x| x as R))
    result

// ── 4. 使用（编译期检查所有约束） ──

let nums = MyList(items: [1, 2, 3])
let strs = transform_list(nums)
// 编译期检查链：
//   MyList → __iter__() → MyIter<T> → __next__() → T (int)
//   ✅ 2 dunder methods: __iter__, __next__
//   ✅ map 方法至少 1 个（map 匹配 /map\w*/ at_least(1)）
//   ✅ map() 接受 fn(I.Item) -> R 形式的闭包
```

---

## 九、编译期检查错误码表

| 错误码 | 条件 | 触发场景 |
|--------|------|----------|
| `DC001` | Duck 约束方法不存在 | 实际类型缺少 duck 中声明的方法 |
| `DC002` | 参数数量不匹配 | `exact(N)`/`max(N)`/`range(L,R)` 约束违反 |
| `DC003` | 命名参数缺失 | `require(name, version)` 中某个参数未提供 |
| `DC004` | 类型类别不匹配 | `StackType` 参数传入了 `RefType` 类型 |
| `DC005` | self 修饰符不匹配 | `ref self` 要求 `&self` 但实际为 `self` |
| `DC006` | 字段可见性不匹配 | 约束要求 `.name`（公开）但目标字段不可见（私有或不存在） |
| `DC007` | 正则模式不匹配 | 所有方法名都不匹配 `/pattern/` |
| `DC008` | 数量约束违反 | `at_least(N)`/`exact(N)` 不满足 |
| `DC009` | 关联类型缺失 | `type Item` 但实际类型没有 |
| `DC010` | 类型链推断失败 | `T.foo → R → R.bar → S` 中某环不满足 |

---

## 十、进阶模式（组合语法集锦）

### 10.1 `..` 位置通配符

`..` 在 duck 块中表示"此处允许任意数量字段/方法"，用于表达灵活的位置约束：

```lz
// ..在尾部：前面字段固定，后面可扩展
duck Extensible =
    .id: int
    .name: str
    ..                              // 允许额外字段

// ..在头部：尾部字段固定，前面字段任意
duck Appendable =
    ..
    .tail: str

// ..在中间：两端固定，中间可变形
duck Framed =
    .prefix: str
    ..
    .suffix: str

// ..与泛型结合
duck Container<T> =
    .tag: str
    .payload: T
    ..
```

**编译期规则**：

1. 一个 duck 块中允许**最多一个 `..`**
2. `..` 处匹配实际类型的对应位置——不检查"被 `..` 覆盖"的字段/方法
3. `..` 前后的精确声明必须满足，`..` 覆盖的部分忽略

### 10.2 `_` 字段/方法占位符

`_` 在 duck 块中表示"该位置有一个字段/方法，但不约束其名称"，用于表达位置敏感的部分约束：

```lz
// 第一个字段任意，第二个必须是 .name: str
duck Named =
    _: Any
    .name: str

// 前两个字段任意，第三个必须是 .timestamp: int
duck Timestamped =
    _: Any
    _: Any
    .timestamp: int

// 必须字段穿插在占位之间
duck Credential =
    _: Any
    .username: str
    _: Any
    .password: str
    _: Any

// _ + .. 组合：前两个任意，后面任意扩展，必须有 .id: int
duck Identifiable =
    _: Any
    _: Any
    .id: int
    ..
```

**方法签名占位**：`_(self)` 表示"该位置有一个方法，不约束方法名"：

```lz
duck Stringifiable =
    def _(self) -> Any                 // 第一个方法任意
    def toString(self) -> str          // 第二个必须是 toString

duck LayoutAware =
    def _(self) -> Any
    def _(self) -> Any
    def render(self) -> ()             // 第三个必须是 render
    def _(self) -> Any

duck Service =
    def init(self) -> ()               // 位置0: 初始化
    def _(self) -> Any                 // 位置1: 任意
    def process(self) -> ()            // 位置2: 核心处理
    def _(self) -> Any                 // 位置3: 任意
    def destroy(self) -> ()            // 位置4: 清理
```

**编译期规则**：

1. `_` 匹配位置对应的字段/方法（按声明顺序匹配）
2. `_` 不检查名称，只检查类型签名
3. `_: Any` 匹配任何类型的字段（完全通配）

> 注：`Any` 在此是"任意类型"的位置通配符（仅用于 `_: Any` / `def _(self) -> Any` 这类占位），与 `Box<dyn Any>` 的**运行时类型擦除**无关——duck 的所有检查都是编译期静态完成的。
4. `_(): RetType` 匹配任何名称、返回 `RetType` 的方法
5. `_` 不会跳过字段——必须逐个位置计数

### 10.3 签名继承与扩展 `T, ..`

duck 可以通过 `T, ..` 语法"继承"另一个类型参数的签名并扩展：

```lz
duck Enhanced<T> =
    T, ..                           // 包含 T 的所有字段/方法
    .enhanced: bool                 // 额外字段

// 等价于: "T 有的我都有，外加 enhanced: bool"
```

### 10.4 泛型族约束

多个类型参数共享同一基础约束：

```lz
duck Family<Base, T1, T2, T3> =
    .members: (T1, T2, T3)
    where T1: Base, T2: Base, T3: Base

// 用法：要求 T1/T2/T3 都满足 Base 的 duck 约束
```

### 10.5 交叉约束 `A + B`

duck 约束可以通过 `+` 组合多个约束（与 `where T: Clone + Display` 写法一致）：

```lz
duck Merge<A, B, C> =
    .left: A
    .right: B
    .merged: C
    where C: A + B                  // C 必须同时满足 A 和 B 的约束
```

### 10.6 递归泛型约束

```lz
duck Tree<T> =
    .value: T
    .children: List<Tree<T>>         // 递归引用自身
```

### 10.7 位置参数数量约束 (`..` 在签名中)

`..` 在 `duck` 的方法签名中用于表示该位置可以接受不定数量的参数：

```lz
duck Flexible =
    def process(self, ..) -> ()
    def log(self, msg: str, ..) -> ()
    def format(self, prefix: str, .., suffix: str) -> ()
```

已在 §8.2 参数约束系统中定义，此处为组合示意。

### 10.8 完整组合示例：带生命周期的服务契约

```lz
duck LifecycleService =
    def init(self) -> ()                 // 必须第一个
    def _(self) -> Any                   // 任意
    def start(self) -> bool              // 必须第三个
    ..
    def _(self) -> Any
    def shutdown(self) -> ()             // 必须倒数第二个
    def destroy(self) -> ()              // 必须最后一个
```

编译期检查：

```lz
struct MyService =
    def init(self) = pass
    def setup(self) = pass           // 匹配 _(self)
    def start(self) -> bool = True
    def log(self) = pass             // 匹配 ..
    def cleanup(self) = pass         // 匹配 _(self)
    def shutdown(self) = pass
    def destroy(self) = pass

// ✅ MyService 满足 LifecycleService
// ❌ 缺少 shutdown() 或顺序不对 → 编译期报错
```

---

## 十一、duck 专用软关键字（soft keywords）

### 11.1 机制：什么是「软关键字」

**duck 专用软关键字（soft keyword）** 是一类在 `duck` 块内按上下文识别为专用 token、在 `duck` 块外恢复为普通标识符的词（与 Rust 的 `union`、Swift 的 `some` 同机制）。它们**不是全局保留字**——不占用标识符命名空间，可用作变量名/字段名/方法名；仅在 `duck` 体内特定语法位置具有专用含义。

### 11.2 软关键字分类表

| 软关键字 | 类别 | duck 体内含义 | 语法位置 |
|----------|------|---------------|----------|
| `require(a, b)` | 约束行 | 目标方法调用时必须提供命名参数 `a, b` | 方法签名后独立行（§8.2.2） |
| `optional(a, b)` | 约束行 | 目标方法的命名参数 `a, b` 可选（有默认值） | 方法签名后独立行（§8.2.2） |
| `exact(N)` | 签名内嵌 | 位置参数恰好 N 个 / 正则匹配恰好 N 个 | 参数位 / `match` 行（§8.2.1/8.4） |
| `min(N)` | 签名内嵌 | 位置参数至少 N 个 | 参数位（§8.2.1） |
| `max(N)` | 签名内嵌 | 位置参数不超过 N 个 | 参数位（§8.2.1） |
| `range(L, R)` | 签名内嵌 | 位置参数在 L~R 个 | 参数位（§8.2.1） |
| `at_least(N)` | match 行 | 至少有 N 个方法匹配正则模式 | `match` 行（§8.4） |
| `at_most(N)` | match 行 | 最多 N 个方法匹配正则模式 | `match` 行（§8.4） |
| `match` | 约束行 | 引入正则模式匹配约束 | duck 体内独立行（§8.4） |
| `StackType` | 伪类型 | 参数必须为栈类型（Copy 语义，零堆分配） | 参数类型位（§8.2.3） |
| `RefType` | 伪类型 | 参数必须为引用类型（Move 语义，含堆分配） | 参数类型位（§8.2.3） |
| `Any` | 伪类型 | 位置通配（`_: Any` / `def _(self) -> Any`） | 字段/方法占位（§10.2） |
| `satisfies` | 约束行 | 要求目标类型同时满足另一 duck（`satisfies Base`） | duck 体内独立行 |
| `sealed` | 约束行 | 闭合约束：目标类型不得有额外成员（与 `..` 相反） | duck 体内独立行 |
| `default` | 签名修饰 | 该成员可选：目标类型可不实现（缺省跳过） | 方法签名行 |

### 11.3 约束行 / 签名内嵌 / 伪类型（第 1~12 行）

前 12 个软关键字在 `duck` 体内按其语法位置识别（§11.5 解析规则），duck 体外恢复为普通标识符：

```lz
duck Buildable<T> =
    def build(self) -> T
    require(name: str, version: int)     // 软关键字：require 行
    def init(self, key: str, range(0, 2)) -> ()   // range 内嵌
    optional(timeout: int)               // 软关键字：optional 行

// 离开 duck 体后恢复普通标识符：
let require = 1                          // ✅ 合法：require 是普通变量名
```

> **解析规则**：这些词在 `duck` 体内被词法器按上下文稳定识别；在 duck 体外（普通代码）它们是普通标识符，可用作变量名等，不因保留字占用而报错。

### 11.4 组合约束 / 闭合 / 可选成员（第 13~15 行）

```lz
// ① satisfies —— 显式「本 duck 要求目标类型还满足另一个 duck」，
//    与 where T: Base 等价，但可写在 duck 体内作约束行（更内聚）
duck Resource<T> where T: Iterable =
    satisfies Iterable                 // 等价 where T: Iterable（二选一即可）
    def fetch(self) -> T

// ② sealed —— 闭合约束：目标类型不得有额外成员。
//    与 `..`（开放，允许额外成员）相反，默认 duck 是开放的
duck ExactShape =
    sealed                             // 目标类型只能有 .w / .h，多一个字段即报错
    .w: int
    .h: int

// ③ default —— 该成员可选：目标类型可不实现，编译器缺省跳过。
//    与 `optional`（命名参数可选）互补：default 是整个成员可选
duck Renderable =
    def render(self) -> ()
    default def fallback(self) -> ()   // 目标类型可不实现 fallback
```

### 11.5 解析规则与语法边界

1. **上下文识别**：软关键字仅在 `duck` 块体内被识别；识别依据是所在行/参数位的语法结构（约束行引导词、`match` 行、参数位 `exact/min/max/range`、类型位 `StackType/RefType/Any`）。
2. **duck 体外完全普通**：`let require = 1`、`struct X = x: StackType`、`def optional() = ...` 均合法——这些词不是全局保留字。
3. **不嵌套识别**：`duck` 体内的 `def`/`struct` 等普通语法里不识别软关键字（嵌套的 duck 块除外）。
4. **冲突处理**：若目标类型真的定义了名为 `require` 的字段/方法，duck 体内用 `def require(self)` 正常声明；软关键字 `require(...)` 只出现在「无 `def` 前缀的独立调用行」，两者由行首 token 区分，无歧义。
5. **错误提示**：软关键字用错位置（如 duck 体外写 `require(a)`）→ 按普通函数调用解析（若 `require` 未定义则报「未定义函数」），不产生「保留字」错误。

### 11.6 与「保留字」的边界

| 类别 | 示例 | 是否占用标识符 |
|------|------|:--------------:|
| 硬关键字（全局保留） | `def` / `if` / `for` / `duck` | ✅ 占用，不可作标识符 |
| 软关键字（duck 体内专用） | `require` / `optional` / `exact` / `satisfies` / `sealed` / `default` | ❌ 不占用，duck 体外可用 |
| 内建/伪类型（prelude） | `StackType` / `RefType` / `Any` | ❌ 不占用（库内容） |

> 详见 [附录B-关键字保留字符号语法边界.md](附录B-关键字保留字符号语法边界.md) §1.13 的 duck 软关键字总表。

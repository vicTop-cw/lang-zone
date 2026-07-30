# Duck 关系约束——多类型间结构化关系

> **版本**: 3.1 · 2026-07-30  
> **参考**: Nim `concept` · Rust `trait` bounds · Go interface 结构匹配  
> **状态**: 🟡 规范设计（语法冻结，待实现）

---

## 一、基本动机

现有 `duck`（§六）描述的是"单类型满足什么结构"——属性型约束。扩展后的 `duck` 要解决的是"**多类型之间存在怎样的结构关系**"——**关系型约束**。

### 两种约束的对比

```
// 属性型（当前 §六）—— "T 是什么"
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

### 2.0 检查站约束语法（Checkpoint）

`duck` 约束在函数签名中通过 **方括号检查站 `[]`** 声明。这是 LZ 统一的**类型约束语法**：

| 符号 | 含义 | 示例 |
|------|------|------|
| `[]` | **检查站** — 声明有约束的泛型参数 | `[T: Quackable]` |
| `<>` | **普通泛型** — 声明无约束的泛型参数 | `<T>` |
| 混合 | `[]` 声明有约束的，`<>` 声明无约束的 | `[T: Iterable]<R>` |

```lz
// 只有检查站（所有类型参数都有约束）
def quack[T: Quackable](x: T) = x.quack()

// 只有普通泛型（无约束）
def identity<T>(x: T) = x

// 混合：T 有约束，R 无约束
def process[T: Iterable]<R>(items: T) -> List<R> = ...
```

> **设计渊源**：`[]` 检查站最初在 `iterator` 关键字设计中以 `[检查站]` 占位符出现，现通过 `duck` 给出具体语义——方括号内声明类型参数并附加约束。

### 2.1 多泛型参数 + 类型前缀

```lz
duck Mapper<T, R> =
    def T.map(self) -> R          // T 有 map 方法，返回值类型为 R
    def R.unmap(self) -> T        // R 有 unmap 方法，返回值类型为 T
```

当 `duck` 有**多个泛型参数**时，方法声明前需加上 `TypeName.` 前缀指明**所属类型**。

**规则**：多参数时方法前缀**必须**（歧义消除）；单参数时可省略（退化为 §六 语法）。

```
duck Q<T> =                        // 单参数：兼容 §六
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
    Eq.equals(A, B) -> bool         // Eq.compare 可接受 A 和 B
```

---

## 三、关系运算符

### 3.1 类型投影运算符

| 运算符 | 含义 | 示例 |
|--------|------|------|
| `T.method → R` | T.method 的返回类型为 R | `def T.produce() → R` |
| `T.method ↦ R` | T.method 的参数类型为 R（简写 `T.method(R)`） | `def T.consume(R)` |
| `T.field : R` | T.field 的类型等于 R | `T.name: str` |

实际语法中使用 `->` 箭头（与函数签名一致）：

```lz
// 返回类型投影
def T.produce(self) -> R          // produce 的返回类型 = R

// 参数类型投影
def T.consume(self, x: R) -> ()   // consume 的参数类型 = R
```

### 3.2 类型关系运算符（在 where 子句中）

| 运算符 | 含义 | 示例 |
|--------|------|------|
| `<:` | 子类型关系（T 是 R 的子类型） | `Sub <: Super` |
| `:>` | 超类型关系（T 是 R 的超类型） | `Super :> Sub` |
| `==` | 类型等同 | `A.field == B.field` |
| `::` | duck 约束满足 | `T :: Iterable` |

### 3.3 协变/逆变声明

```lz
duck Covariant<Sub, Super> where Sub <: Super =
    def Sub.produce(self) -> Super     // Sub 产出 Super（协变：产出方向）

duck Contravariant<Super, Sub> where Sub <: Super =
    def Super.consume(self, x: Sub)    // Super 消费 Sub（逆变：消费方向）

duck Invariant<T> =
    def T.read(self) -> T              // T 既是输入也是输出（不变）
    def T.write(self, x: T)
```

---

## 四、关系运算符的编译期检查

### 4.1 检查时机

与 `duck` 的基本检查一致——在**泛型实例化时**（monomorphization）：

```lz
def process[T, R: Mapper<T, R>](x: T, y: R) =
    let mapped = x.map()              // 检查：T.map → R
    let restored = y.unmap(mapped)    // 检查：R.unmap → T
```

### 4.2 类型投影解析规则

投影 `T.method → R` 的检查：

```
1. 在 T 的实际类型中查找 method
2. 提取 method 的返回类型 actual_return
3. 检查 actual_return 是否等于（或满足）R
   - 如果 R 是具体类型：actual_return == R
   - 如果 R 是 duck 约束：actual_return :: R
   - 如果 R 是泛型参数：actual_return 的类型与 R 的参数类型一致
```

### 4.3 子类型关系检查

`Sub <: Super` 的检查：

```
1. 在泛型实例化时，检查 Sub 的实际类型是否可以向上转型为 Super
2. 可转型的定义：
   - Sub == Super ✅
   - Sub 实现了 Super 的 trait ✅
   - Sub 满足 Super 的所有 duck 约束 ✅
   - Sub 是 Super 的子 struct 或变体 ✅
```

---

## 五、典型场景

### 5.1 函数式：映射器模式

```lz
duck Mapper<T, R> =
    def T.map(self) -> R

// 用法：任何有 map() 方法且返回 R 的类型都满足
def transform[T, R: Mapper<T, R>](items: List<T>) -> List<R> =
    let result: List<R> = []
    for x in items:
        result.push(x.map())
    result

struct Wrapper<T> =
    value: T
    def map(self) -> T = self.value

let ws = [Wrapper(value: 10), Wrapper(value: 20)]
let nums = transform(ws)          // Type inference: T = Wrapper<int>, R = int
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
struct Email(String)
struct ValidationError(String)

impl Email =
    def validate(self) -> Result<Email, ValidationError> =
        if "@" in self.0:
            Ok(self)
        else:
            Err(ValidationError("invalid email"))

impl ValidationError =
    def message(self) -> str = self.0

// 这里 Email 和 ValidationError 自动满足 Validator<T=Email, E=ValidationError>
// 不需要 impl Validator for ...
```

### 5.4 协变容器

```lz
duck CovariantBox<In, Out> where In <: Out =
    def In.get(self) -> Out               // In.get 返回 Out（或其子类型）

struct Animal(name: str)
struct Dog(Animal)                        // Dog 是 Animal 的子类型

struct Box<T>(value: T)
    def get(self) -> T = self.value

// Box<Dog> 的 get 返回 Dog，Dog <: Animal
// 满足 CovariantBox<In=Box<Dog>, Out=Animal>
```

### 5.5 编解码对

```lz
duck Codec<T, Encoded> =
    def T.encode(self) -> Encoded
    def Encoded.decode(self) -> T

// 示例：Json 编解码
struct User(id: int, name: str)
struct Json(str)

impl User =
    def encode(self) -> Json = Json(f'{{"id": {self.id}, "name": "{self.name}"}}')

impl Json =
    def decode(self) -> User = User(1, "parsed")  // 简化

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
│ 类型间关系     │ ❌ 无法表达        │ ✅ <: / :> / ==          │
│ 协变/逆变      │ ❌ 不支持          │ ✅ 通过投影推导            │
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

// ❌ duck 不能实例化
let x: Mapper<int, str> = ...    // 错误：duck 不是类型，是约束

// ✅ 作为泛型约束
def f[T, R: Mapper<T, R>](...)   // 正确

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
duck ConstrainedMatch =
    def process(self) -> T where T: Display  // 返回类型只要实现了 Display 就 OK
    def handle(self, x: T) -> () where T: Clone  // 参数只要实现了 Clone 就 OK
```

**链式推断**：

```lz
duck Chained =
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
duck ParamConstrained =
    // 方法必须恰好接受 2 个位置参数
    def set_pair(self, a: int, b: str) -> ()

    // 方法可以接受 1-3 个位置参数
    def configure(self, key: str, range(0, 2)) -> ()

    // 方法至少需要 1 个参数
    def process(self, min(1)) -> T
```

#### 8.2.2 命名参数约束

| 约束语法 | 含义 |
|----------|------|
| `require(a, b)` | 调用时必须提供命名参数 a, b |
| `optional(a, b)` | 命名参数 a, b 可选（有默认值） |

```lz
duck Buildable =
    def build(self) -> T require(name: str, version: int)  // 必须有 name 和 version 命名参数
    def init(self, key: str) -> () optional(timeout: int = 30)  // timeout 可选

    // 组合：必须有以下命名参数
    def send(self, ..) -> () require(to: str, body: str)
```

编译期检查：调用 `build(name="x")` 缺少 `version` 报错。
调用 `build(name="x", version=1, extra=true)` 编译器必须检查是否存在额外的可选命名参数。

#### 8.2.3 形参类型约束

限制参数只能为**栈类型（StackType）**或**引用类型（RefType）**：

```lz
// 类型分类
// StackType: int, f64, bool, (), str*, (T), struct(无 self 引用)
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

```lz
duck PublicFields =
    pub .name: str                 // 要求 name 字段为公开（pub）
    pub .version: int

duck InternalState =
    .counter: int                  // 不指定 pub → 只要存在即可（不限制可见性）
```

编译期规则：
- `pub .x: T`：要求类型必须有 `pub x: T`（Rust 侧对应 `pub x: T`）
- `.x: T`：只要类型有 `x: T` 字段（不限可见性）

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
| `pub` | `pub .field: T` | 目标字段必须 `pub`，否则报错 |
| `ref` | `def foo(ref self)` | 目标方法 self 必须为 `&self` |
| `mut` | `def foo(mut self)` | 目标方法 self 必须为 `&mut self` |
| `owned` | `def foo(owned self)` | 目标方法 self 必须为 `self`（move） |
| `comptime` | `def foo(comptime self)` | 目标方法必须为编译期纯函数 |
| `unsafe` | `def foo(unsafe self)` | 目标方法必须标记 `unsafe` |

### 8.4 方法名正则模式匹配

`duck` 中方法名支持正则表达式，用 `/pattern/flags` 括起：

```lz
duck PatternMatched =
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
duck HasMultipleGetters =
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
├── 解析修饰符规则（pub/ref/mut/comptime）
├── 构建约束表（ConstraintTable）
└── 注意：此时不报错，duck 块不绑定任何具体类型

阶段 2：实例化期（泛型函数调用时）
├── 获取实际类型的方法列表（反射＋AST 扫描）
├── 逐条约束匹配：
│   ├── 精确匹配 → 方法名 + 签名精确相等
│   ├── 约束匹配 → 方法名 + 返回类型满足约束
│   ├── 正则匹配 → 方法名正则 + 参数签名 + 数量约束
│   └── 修饰符匹配 → pub/ref/mut/comptime 校验
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
def process[T, R: Mapper<T, R>](x: T, y: R) =
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
   = actual type `String` has no method `unmap`
   = bound at: demo.lz:5  where T: Mapper<T, R>
```

### 8.6 完整示例：TypeScript 级别类型安全的列表转换器

```lz
// ── 1. 定义 duck 约束 ──

// 关系约束：IterablePair 描述了 T 与 I 之间的迭代关系
duck IterablePair<T, I> =
    type I.Item                              // I 有关联类型 Item
    def T.__iter__(self) -> I                // T.__iter__ 返回 I
    def I.__next__(mut self) -> T            // I.__next__ 返回 T
    match /__\w+/ exact(3)                  // 必须恰好有 3 个 dunder 方法

// 转化器约束：T 和 R 之间有 map 关系
duck Transform<T, R> where T: IterablePair<T, I> =
    def T.map(self, f: StackType) -> R       // T.map 的参数必须为栈类型
    require(func: Callable)                  // 必须有关键字参数 func
    match /map_\w+/ at_least(1)             // 至少有 1 个 map_ 前缀方法

// ── 2. 实现类型（不相关 struct，结构匹配自动生效）──

struct MyIter<T>
    items: List<T>
    index: int
    def __next__(mut self) -> T =
        if self.index < self.items.length():
            let val = self.items[self.index]
            self.index += 1
            val
        else:
            raise StopIteration

struct MyList<T>
    items: List<T>
    def __iter__(self) -> MyIter<T> =
        MyIter(items: self.items, index: 0)
    // 自动满足 IterablePair<T=MyList, I=MyIter<T>>

// ── 3. 泛型函数 ──

def transform_list[T, R](items: List<T>) -> List<R>
    where T: Transform<T, R> =              // 编译期静态检查
    let result: List<R> = []
    for item in items:
        result.push(item.map(func: |x| x as R))
    result

// ── 4. 使用（编译期检查所有约束） ──

let nums = MyList(items: [1, 2, 3])
let strs = transform_list(nums)
// 编译期检查链：
//   MyList → __iter__() → MyIter → __next__() → T (int)
//   ✅ 3 dunder methods: __iter__, __next__, __init__
//   ✅ getters at_least(1)
//   ✅ map() params StackType
//   ✅ func 关键字参数存在
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
| `DC006` | 字段可见性不匹配 | `pub .name` 但实际字段不是 pub |
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
    id: int
    name: str
    ..                              // 允许额外字段

// ..在头部：尾部字段固定，前面字段任意
duck Appendable =
    ..
    tail: str

// ..在中间：两端固定，中间可变形
duck Framed =
    prefix: str
    ..
    suffix: str

// ..与泛型结合
duck Container<T> =
    tag: str
    payload: T
    ..

// ..在嵌套签名中
duck NestedFlex =
    outer: { x: int, .. }
    inner: { .., y: int }
```

**编译期规则**：

1. 一个 duck 块中允许**最多一个 `..`**
2. `..` 处匹配实际类型的对应位置——不检查"被 `..` 覆盖"的字段/方法
3. `..` 前后的精确声明必须满足，`..` 覆盖的部分忽略

### 10.2 `_` 字段/方法占位符

`_` 在 duck 块中表示"该位置有一个字段/方法，但不约束其名称"，用于表达位置敏感的部分约束：

```lz
// 第一个字段任意，第二个必须是 name: str
duck Named =
    _: Any
    name: str

// 前两个字段任意，第三个必须是 timestamp: int
duck Timestamped =
    _: Any
    _: Any
    timestamp: int

// 必须字段穿插在占位之间
duck Credential =
    _: Any
    username: str
    _: Any
    password: str
    _: Any

// _ + .. 组合：前两个任意，后面任意扩展，必须有 id: int
duck Identifiable =
    _: Any
    _: Any
    id: int
    ..
```

**方法签名占位**：`_()` 表示"该位置有一个方法，不约束方法名"：

```lz
duck Stringifiable =
    _(): Any                       // 第一个方法任意
    toString(): str                // 第二个必须是 toString

duck LayoutAware =
    _(): Any
    _(): Any
    render(): void                 // 第三个必须是 render
    _(): Any

duck Service =
    init(): void                   // 位置0: 初始化
    _(): Any                       // 位置1: 任意
    process(): Result              // 位置2: 核心处理
    _(): Any                       // 位置3: 任意
    destroy(): void                // 位置4: 清理
```

**编译期规则**：

1. `_` 匹配位置对应的字段/方法（按声明顺序匹配）
2. `_` 不检查名称，只检查类型签名
3. `_: Any` 匹配任何类型的字段（完全通配）
4. `_(): RetType` 匹配任何名称、返回 `RetType` 的方法
5. `_` 不会跳过字段——必须逐个位置计数

### 10.3 签名继承与扩展 `T, ..`

duck 可以通过 `T, ..` 语法"继承"另一个类型参数的签名并扩展：

```lz
duck Enhanced<T> =
    T, ..                           // 包含 T 的所有字段/方法
    enhanced: bool                  // 额外字段

// 等价于: "T 有的我都有，外加 enhanced: bool"
```

### 10.4 泛型族约束

多个类型参数共享同一基础约束：

```lz
duck Family<Base, T1, T2, T3> =
    members: (T1, T2, T3)
    where T1: Base, T2: Base, T3: Base

// 用法：要求 T1/T2/T3 都满足 Base 的 duck 约束
```

### 10.5 交叉约束 `A & B`

duck 约束可以通过 `&` 组合多个约束：

```lz
duck Merge<A, B, C> =
    left: A
    right: B
    merged: C
    where C: A & B                  // C 必须同时满足 A 和 B 的约束
```

### 10.6 递归泛型约束

```lz
duck Tree<T> =
    value: T
    children: List<Tree<T>>         // 递归引用自身
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
    init(): void                     // 必须第一个
    _(): Any                         // 任意
    start(): Result                  // 必须第三个
    ..
    _(): Any
    shutdown(): void                 // 必须倒数第二个
    destroy(): void                  // 必须最后一个
```

编译期检查：

```lz
struct MyService
    def init(self) = pass
    def setup(self) = pass           // 匹配 _(): Any
    def start(self) -> bool = True
    def log(self) = pass             // 匹配 ..
    def cleanup(self) = pass         // 匹配 _
    def shutdown(self) = pass
    def destroy(self) = pass

// ✅ MyService 满足 LifecycleService
// ❌ 缺少 shutdown() 或顺序不对 → 编译期报错
```

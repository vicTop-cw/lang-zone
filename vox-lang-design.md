# Vox 语言设计文档 v2

> *"Write like Python, run like Rust."*

## 0. 设计哲学 -- 用超甜的糖，做更甜的糖！

Vox 是一门**转译型**语言，编译为 Rust 源码再交给 rustc。不碰底层，只做语法糖。

核心原则：
1. **函数式与命令式并存** —— 支持递归/高阶函数，也支持 `for`/`while`/`loop` 循环
2. **缩进即语法** —— 省去 `{}`，视觉干净
3. **Rust 扛底层** —— 所有权、生命周期、内存管理全部由 Rust 编译器自动推导
4. **符号即语义** —— 魔法方法定义操作符，自定义符号定义 DSL
5. **不暴露底层** —— 无指针、无 `unsafe`、无生命周期标注

---

## 1. 词法基础

### 1.1 缩进规则

```
// 4 空格为一个缩进层级
def factorial(n: int) -> int:
    if n <= 1:
        1
    else:
        n * factorial(n - 1)
```

### 1.2 注释

```
// 单行注释
/* 
   多行注释
   可以嵌套
*/
```

### 1.3 语句分隔

```
// 换行即分隔，分号可选
val x = 1
val y = 2

// 一行多语句用分号
val a = 1; val b = 2
```

---

## 2. 变量声明

只有三种：`val` / `var` / `const`。

| 关键字 | 可重新赋值 | 可用于类型位置 |
|--------|:---:|:---:|
| `val` | 否 | 否 |
| `var` | 是 | 否 |
| `const` | 否 | 是 |

### 2.1 基本用法

```
val x = 42              // 不可变绑定
var y = 42              // 可变绑定
y = 43                  // ✅ 允许

const PI = 3.14159      // 编译期常量
const MAX = 1024

// 类型注解 + 自动转换
val a: f64 = 1          // ✅ int → f64
val b: int = "a"        // ❌ 编译错误

// 类型推断
val c = 1               // c: int
val d = 3.14            // d: f64
```

### 2.2 解构绑定

```
// 元组解构
val (x, y) = (1, 2)

// 枚举解构
val Some(v) = maybe_value

// 列表解构
val [first, ...rest] = [1, 2, 3]
```

---

## 3. 函数

### 3.1 语法

```
def 函数名 <泛型> (参数) -> 返回值 raises 异常
    where 约束 = 类型
    else:
    函数体
```

- `where` — 编译期泛型类型约束

### 3.2 示例

```
// 最简
def hello():
    print("hi")

// 单表达式简写（= 后直接接表达式）
def add1(x: int) -> int = x + 1
def mul2(x: int) -> int = x * 2

// 返回值
def add(a: int, b: int) -> int:
    a + b                          // 最后一行是返回值

// 泛型
def identity<T>(x: T) -> T:
    x

// 泛型约束
def max<T: Comparable>(a: T, b: T) -> T:
    a if a > b else b

// 异常（函数体内有 raise 就必须写 raises）
def divide(a: int, b: int) -> int raises DivError:
    if b == 0:
        raise DivError("zero")
    a / b

// where 类型别名
def transform<T>(x: T, f: F) -> T
    where F = (T) -> T:
    f(x)

// 完整形态
def demo_func<T: Intable>(ix: T, f: F) -> String raises MyError
    where F = (int) -> bool:
    val i = int(ix)??
    if f(i):
        str(i + 1)
    else:
        str(i - 1)
```

### 3.3 参数传递

| 修饰 | 含义 | 调用方要求 |
|------|------|-----------|
| （默认） | 借用（只读） | 无限制 |
| `owned` | 所有权转移 | 无限制，之后不可用 |
| `mut` | 可变借用 | 必须是 `var` |

```
def inspect(data: Data):                    // 默认借用
    print(data.payload)

def consume(data: owned Data):              // 拿走所有权
    print(data.payload)

def modify(data: mut Data):                 // 可变借用
    data.payload = "changed"

def main():
    val a = Data("hello")
    var b = Data("world")

    inspect(a)       // ✅
    inspect(b)       // ✅

    consume(a)       // ✅ 所有权转移，之后 a 不可用

    // modify(a)     // ❌ a 是 val，不能可变借用
    modify(b)        // ✅ b 是 var
    inspect(b)       // ✅ b.payload = "changed"
```

### 3.4 匿名函数

```
val double = |x: int| -> int: x * 2
val add = |a: int, b: int| -> int: a + b
```

#### 3.4.1 `_` 占位符语法（Scala 风格）

在函数调用中，`_` 可作为匿名函数参数的占位符，编译器自动生成闭包。每个 `_` 对应一个独立的位置参数（从左到右）。

```
// 单参数
[1, 2, 3].map(_ * 2)          // 等价于 .map(|x| x * 2)
[1, 2, 3].filter(_ > 1)       // 等价于 .filter(|x| x > 1)

// 多参数
[1, 2, 3].reduce(_ + _)       // 等价于 .reduce(|a, b| a + b)
[1, 2, 3].fold(0, _ + _)      // 等价于 .fold(0, |a, b| a + b)

// 嵌套调用
[1, 2, 3].map(_.to_str())     // 等价于 .map(|x| x.to_str())
```

**限制**：`_` 只能在函数调用参数位置使用，不能用于独立创建匿名函数：

```
// ✅ 允许：作为函数参数
val result = data.map(_ * 2)

// ❌ 禁止：无类型上下文，编译器无法推断
val f = _ + _                  // 编译错误！
val g = _ * 2                  // 编译错误！
```

**语义**：
- `_` 的作用域是最近的函数调用括号
- 多个 `_` 按出现顺序对应参数位置（第一个 `_` = 第 1 个参数，第二个 `_` = 第 2 个参数）
- 类型由目标函数的参数签名推断

转译示例：

```
Vox                           Rust
───                           ────
.map(_ * 2)                   .map(|x| x * 2)
.reduce(_ + _)                .reduce(|a, b| a + b)
.filter(_.len() > 0)          .filter(|x| x.len() > 0)
```

### 3.5 可变参数 `*args`

`*args: T` 收集同类型的可变数量参数，对标 Python 的 `*args`。

```
// 同类型可变参数
def sum_all(*args: int) -> int:
    args.fold(0, |acc, x| acc + x)

sum_all(1, 2, 3, 4)       // 10
sum_all()                  // 0

// 泛型可变参数
def join<T: Display>(sep: str, *args: T) -> str:
    args.map(|x| x.display()).join(sep)

join(", ", 1, 2, 3)       // "1, 2, 3"
join(" | ", "a", "b")     // "a | b"
```

转译成 Rust 的切片：

```rust
fn sum_all(args: &[i32]) -> i32 {
    args.iter().fold(0, |acc, x| acc + x)
}

// 调用时编译器自动收集为切片
sum_all(&[1, 2, 3, 4]);
```

**限制**：`*args` 必须是同类型（`T`），不支持异类型可变参数。如需不同类型，使用元组重载。

### 3.6 参数解包 `*`

`*` 在调用处展开集合为位置参数，编译器期检查长度匹配。

```
val nums = [1, 2, 3]
sum_all(*nums)             // 等价于 sum_all(1, 2, 3)

val pair = (3.0, 4.0)
def distance(x: f64, y: f64) -> f64:
    (x * x + y * y).sqrt()
distance(*pair)            // 等价于 distance(3.0, 4.0)
```

转译成 Rust 的元组/数组解构：

```rust
let nums = [1, 2, 3];
sum_all(&nums);            // 直接传切片引用

let pair = (3.0, 4.0);
distance(pair.0, pair.1);  // 编译期展开为逐个参数
```

### 3.7 不支持 `**kwargs`

Vox **不支持** Python 的 `**kwargs`（可变关键字参数）。原因：

| | Python `**kwargs` | Rust |
|---|---|---|
| 类型 | `Dict[str, Any]` 运行时反射 | 无运行时反射 |
| 开销 | 堆分配 + 哈希查找 | — |
| 类型安全 | 无编译期检查 | — |

**替代方案**：使用 struct 或默认参数。

```
// ❌ 不支持的写法
def config(**kwargs): ...

// ✅ 替代：struct 参数
struct Config:
    host: str = "localhost"
    port: int = 8080
    debug: bool = false

def connect(cfg: Config):
    ...

connect(Config(port=3000))           // 只覆盖部分字段
connect(Config(host="0.0.0.0"))      // 其余字段用默认值
```

### 3.8 参数特性对比

| 特性 | Vox | Python | Rust 映射 |
|------|:---:|:---:|------|
| 默认参数 | ✅ | ✅ | 不支持（用 struct 替代） |
| `*args` 同类型 | ✅ | ✅ | `&[T]` 切片 |
| `*args` 异类型 | ❌ | ✅ | — |
| `**kwargs` | ❌ | ✅ | — |
| `*` 解包调用 | ✅ | ✅ | 编译期展开 |

### 3.9 多分派（Multi-dispatch）

Vox 使用 **SQLite3 数据库** 在转译期管理多分派映射。非匿名函数的多分派数据存储于 `{:K: List[Row]}` 结构中，映射维度包括：

| 维度 | 说明 |
|------|------|
| 模块名 | 函数所在模块 |
| 函数名 | 函数标识符 |
| 函数类型 | `def` / 生成器 / 匿名函数 |
| 是否魔法方法 | `__xxx__` 双下划线方法 |
| 公开否 | 公开 / `_` 前缀私有 |
| 块层级 | 函数定义所在的嵌套层级 |
| 序列 | 同作用域同名的序号 |
| 签名 | 参数类型签名（泛型 + 参数表达） |
| 目标语言函数名 | 转译后 Rust 侧的函数名 |

#### 核心规则

1. **匿名函数不参与多分派** —— 匿名函数不存入数据库，无多分派功能。
2. **同名同作用域** —— 两个函数作用域相同、函数名相同、签名不同时，维护为列表。
3. **签名比较** —— 只管参数的类型签名（泛型和参数表达），不比较参数名、返回值类型。

#### 签名等价性判断（由 `std.typing` 实现）

```
// 错误示例 1：参数名不同但签名相同 → 转译期报错
def add(a: int, b: int):
    ...
def add(c: int, d: int):
    ...
// 以上两个函数签名完全一致（都是 (int, int)），应报错

// 错误示例 2：类型别名展开后签名一致 → 转译期报错
define:
    F<T>: |int| -> T

def add(a: |int| -> T, b: int):
    ...
def add(a: F<T>, b: int):
    ...
// 以上两个函数参数约束实际完全一致（duck 类型），应报错
```

`std.typing` 模块负责实现签名等价性判断算法，包括：
- 泛型参数归一化
- 类型别名展开
- Duck 类型等价性比较
- 函数类型签名标准化

#### 存储结构

```sql
-- SQLite3 表结构（转译期内部使用）
CREATE TABLE multimethod (
    id INTEGER PRIMARY KEY,
    module TEXT NOT NULL,           -- 模块名
    func_name TEXT NOT NULL,        -- 函数名
    func_type TEXT NOT NULL,        -- def | generator
    is_magic INTEGER DEFAULT 0,     -- 是否魔法方法
    is_public INTEGER DEFAULT 1,    -- 公开否
    block_level INTEGER DEFAULT 0,  -- 块层级
    seq INTEGER DEFAULT 0,          -- 序列号
    signature TEXT NOT NULL,        -- 归一化签名
    target_name TEXT NOT NULL,      -- 目标语言函数名
    UNIQUE(module, func_name, block_level, signature)
);
```

#### `when` `then` — 关键字

`when`、`then` 为 `case` 表达式专用关键字，支持 SQL 风格的条件分支（见 4.1.2）。

---

## 4. 控制流

### 4.1 条件

```
val x = 10

// if-elif-else 表达式
val category = if x < 0:
    "negative"
elif x == 0:
    "zero"
else:
    "positive"

// 三元运算符（Python 风格，无冒号）
val result = "positive" if x > 0 else "non-positive"

// 后续可通过 nthfix 实现 ?-> 条件链语法糖（锦上添花）
// val result = x > 0 ? "positive" : "non-positive"
```

#### 4.1.1 条件链表达式（`?` `->` `!` `=?`）

条件链表达式有三种形式，无歧义：

| 形式 | 语法 | 用途 |
|------|------|------|
| 简单三元 | `data ? true_val : false_val` | 布尔/ImplicitBoolable 快速分支 |
| 求值链 | `data ? _>=90 -> "A" -> _>=80 -> "B" ! "F"` | 立即对 data 求值 |
| 构建器 | `val f: int =? _>=90 -> "A" -> _>=80 -> "B" ! "F"` | 创建 Iif 实例（延迟求值） |

**符号**：
- `_` — 占位符，代表条件是**对 data 的比较**（如 `_ >= 90` 等价于 `data >= 90`）
- `->` — 条件 → 结果分隔符
- `!` — 默认值分隔符（左侧是最后一个结果，右侧是默认值）
- `=?` — 构建器声明（创建 Iif 实例而非立即求值）

#### 形式 1：简单三元

```
val grade = score ? "过" : "卡"
```

`score` 须为 `bool` 或实现了 `ImplicitBoolable` trait 的类型。等价于：

```
val grade = if score: "过" else: "卡"
```

#### 形式 2：求值链（立即求值）

```
val grade = score ? _ >= 90 -> "A" -> _ >= 80 -> "B" -> _ >= 60 -> "C" ! "F"
```

`_` 在条件中代表 `score`。等价于：

```
val grade = if score >= 90: "A"
            elif score >= 80: "B"
            elif score >= 60: "C"
            else: "F"
```

多行写法：

```
val grade = score ?
    _ >= 90 -> "A"
    -> _ >= 80 -> "B"
    -> _ >= 60 -> "C"
    ! "F"
```

`_` 可省略（当条件是需要 data 参与的比较时）：

```
val result = data ?
    _ is int -> "integer"
    -> _ is str -> "string"
    ! "unknown"
```

#### 形式 3：构建器（延迟求值，SQL CASE WHEN 风格）

```
val grade_checker: int =? _ >= 90 -> "A" -> _ >= 80 -> "B" -> _ >= 60 -> "C" ! "F"
```

`=?` 创建一个 **Iif 实例**（实现了 `Callable` trait），不立即求值。之后可反复调用：

```
grade_checker(85)    // "B"
grade_checker(95)    // "A"
grade_checker(55)    // "F"
```

等价于 SQL 的：
```sql
CASE WHEN _ >= 90 THEN 'A' WHEN _ >= 80 THEN 'B' WHEN _ >= 60 THEN 'C' ELSE 'F' END
```

#### 规则

- `?` 前是数据，`?` 后是条件链（求值链）
- `=?` 前无数据，`=?` 后是条件链（构建器）
- `_` 是占位符，代表数据参数
- `->` 左条件右结果，必须成对
- `!` 是默认值分隔符，无歧义（上下文为条件链尾部，非前缀否定）
- 构建器返回的 Iif 实现了 `Callable`，可直接调用
- 构建器可赋值给变量、作为参数传递、组合使用

#### 构建器组合

```
val is_positive =? _ > 0 ! False
val is_even =? _ % 2 == 0 ! False

// 调用
is_positive(5)     // True
is_even(5)         // False

// 组合使用
val is_positive_even =? _ > 0 and is_even(_) ! False

// 作为参数
val result = items.filter(is_positive)
```

---

#### 4.1.2 `case` — SQL 风格分支

`case` 表达式支持两种形式，完全兼容 SQL 的 `CASE WHEN` 语法。块由**缩进回退**闭合，无需 `end`。

**形式 A：简单 case（值匹配）**

```
case data
    when 1 then "one"
    when 2 then "two"
    else "other"
```

等价于：

```
match data:
    | 1 => "one"
    | 2 => "two"
    | _ => "other"
```

**形式 B：搜索 case（条件匹配）**

```
case
    when data > 0 then "positive"
    when data < 0 then "negative"
    else "zero"
```

等价于：

```
if data > 0: "positive"
elif data < 0: "negative"
else: "zero"
```

**赋值使用**：

```
val grade = case score
    when 90..=100 then "A"
    when 80..<90  then "B"
    when 60..<80  then "C"
    else "F"

val sign = case
    when x > 0 then 1
    when x < 0 then -1
    else 0
```

**多行表达式**：

```
// case 是表达式，可直接用于任何需要值的地方
print(case x when 1 then "one" else "other")

// 单行简写（所有内容在一行）
val result = case x when 1 then "yes" else "no"
```

**规则**：
- `case` 后可跟数据（简单 case）或直接 `when`（搜索 case）
- 每个 `when` 后跟 `then`
- `else` 可选，省略时默认返回 `()`
- 块由**缩进回退**闭合：当缩进回到 `case` 同级或更浅时，`case` 块结束
- `case` 是表达式，有返回值
- 与 `?` 条件链的区别：`case` 更适合多行结构化场景，`?` 链更适合紧凑内联场景

---

### 4.2 模式匹配

```
// match 表达式
def describe(x: int) -> str:
    match x:
        | 0 => "zero"
        | 1 => "one"
        | 2 => "two"
        | _ => "many"

// 带 if 守卫
def classify(x: int) -> str:
    match x:
        | n if n < 0  => "negative"
        | 0           => "zero"
        | n if n < 10 => "small"
        | _           => "large"

// 解构匹配
struct Point:
    x: f64
    y: f64

def quadrant(p: Point) -> str:
    match p:
        | Point(0, 0)                        => "origin"
        | Point(x, y) if x > 0 and y > 0     => "Q1"
        | Point(x, y) if x < 0 and y > 0     => "Q2"
        | Point(x, y) if x < 0 and y < 0     => "Q3"
        | Point(x, y) if x > 0 and y < 0     => "Q4"
        | _                                  => "axis"

// 枚举匹配
enum Option<T>:
    Some(T)
    None

def unwrap_or<T>(opt: Option<T>, default: T) -> T:
    match opt:
        | Some(v) => v
        | None    => default

// 列表匹配
def sum_list(xs: [int]) -> int:
    match xs:
        | []          => 0
        | [x, ...rest] => x + sum_list(rest)

// 注意：match x: 后必须换行缩进（与 Python 一致）
// 单行写法：match x: | [] => 0 | [x, ...] => x  // 也是合法的

// 轻量级 case
def weekday(day: int) -> str:
    case day:
        of 1 => "Monday"
        of 2 => "Tuesday"
        of 3 => "Wednesday"
        of 4 => "Thursday"
        of 5 => "Friday"
        else => "Weekend"
```

### 4.3 guard 语句

`guard` **仅支持应用于函数内部**（包括匿名函数），不支持赋值语句。`guard` 支持**连级**（chaining）——即 `guard` 的 `else` 分支中可嵌套另一个 `guard`。

> **`let` 约束**：`let` 是绑定关键字，**不能单独使用**。必须与 `if`、`while`、`guard`、`until` 等控制流关键字配合使用（如 `guard let`、`if let` 等）。

#### 基本语法

```
guard let pattern = expr else else_expr
```

或使用缩进块：

```
guard let pattern = expr else:
    else_body
```

#### 连级 guard

`guard` 支持连级嵌套，`else` 后的 `:` 表示缩进块开始，块内可以包含另一个 `guard`：

```
def add(a: int?, b: int?) -> int:
    guard let Some(v) = a else:
        guard let Some(t) = b else 0
    v + t
// OK — 连级 guard：a 为 None 时尝试 b
```

#### 匿名函数中的 guard

```
// 单行匿名函数 guard
val f = |a: int?, b: int?| => guard let Some(v) = a else guard let Some(t) = b else 0 ; v + t
// OK

// 多行匿名函数 guard（=> 后不加 ：）
val f = |a: int?, b: int?| =>
    guard let Some(v) = a else:
        guard let Some(t) = b else 0
    v + t
// OK

// 注意：=> 后不加 ：，加了就是画蛇添足
```

#### 并行 guard（非连级）

两个并列的 `guard` 各自独立，不是连级关系：

```
val f = |a: int?, b: int?| =>
    guard let Some(v) = a else 0  // OK
    guard let Some(t) = b else 0  // OK
    v + t
// OK — 两个 guard 是并行的，各自独立处理
```

#### 错误示例

```
// 错误：缺少 else_expr 且未用 ： 换行缩进
val f = |a: int?, b: int?| =>
    guard let Some(v) = a else    // Error
    guard let Some(t) = b else 0
    v + t
// 两个 guard 是并行的，第一个 guard 缺少 else 分支

// 错误：guard 不支持赋值
val i: int? = 1
val t = guard let Some(v) = i else 0
// Error — guard 不能用于赋值表达式，只能写在函数体内
```

#### 转译

`guard` 转译成 Rust 的 `let ... else`：

```rust
// Vox: guard let Some(v) = a else 0
// Rust: let Some(v) = a else { return 0; };
```

连级 guard 转译成嵌套的 `let ... else`：

```rust
// Vox:
// guard let Some(v) = a else:
//     guard let Some(t) = b else 0
// v + t
//
// Rust:
let Some(v) = a else {
    let Some(t) = b else { return 0; };
    return t;
};
v + t
```

### 4.4 异常处理

```
// 定义异常
enum MyError:
    NotFound(str)
    PermissionDenied(str)

// 抛出
def might_fail(x: int) -> int raises MyError:
    if x < 0:
        raise MyError.NotFound("x < 0")
    100 / x

// 捕获
def safe_call(x: int) -> Result<int, MyError>:
    try:
        val result = might_fail(x)
    catch MyError.NotFound(msg):
        print(f"Not found: {msg}")
        Err(MyError.NotFound(msg))
    catch e:
        panic(f"Unexpected: {e}")
    else:
        // 没有异常时才执行
        print(f"Success: {result}")
        Ok(result)
    finally:
        cleanup()
```

`try-else` 语义与 Python 一致：`else` 块仅在 `try` 块**没有发生任何异常**时执行，且在 `finally` 之前执行。

| 块 | 执行时机 |
|------|------|
| `try` | 始终先执行 |
| `catch` | 匹配到异常时执行 |
| `else` | `try` 无异常时执行 |
| `finally` | 始终最后执行（无论是否异常） |

### 4.5 defer

```
def file_operation():
    val file = open("data.txt")
    defer: file.close()
    process(file.read())
```

### 4.6 with 上下文管理器

`with` 语句绑定资源到作用域，退出时自动释放。

#### 4.6.1 核心思想：Rust 不需要 with

Rust 通过 **RAII（资源获取即初始化）** + `Drop` trait 天然实现了上下文管理。变量离开作用域时，`drop()` 自动调用——不需要 `with` 关键字。

```
// Rust 原生写法（C++ 也如此）
{
    let mut f = File::open("data.txt")?;
    let content = f.read_to_string()?;
    process(content);
}   // ← f 在这里自动关闭，即使中间抛异常
```

Vox 的 `with` 是**语法糖**——让习惯 Python 的开发者用熟悉的写法，底层转译成 Rust 的块作用域。

#### 4.6.2 基础转译

```
Vox                                  Rust
───                                  ────
with open(path) as f:                {
    body(f)                              let f = open(path)?;
                                         body(&f);
                                     }   // f.drop() 自动调用
```

转译步骤：
1. `with expr as name:` → `{ let name = expr;`
2. `with` 块体 → 直接放入 Rust 块中
3. 缩进结束 → `}`（Rust 在此自动调用 `drop()`）

#### 4.6.3 `__enter__` 和 `__exit__` 的 Rust 映射

与 Python 不同，Vox 的上下文管理器**简化了语义**：

| Python | Vox | Rust |
|--------|-----|------|
| `__enter__(self)` → 返回资源 | `__enter__(Self)` → 返回资源 | 内联到 `with` 表达式中 |
| `__exit__(self, exc_type, exc_val, exc_tb)` → 可返回 True 抑制异常 | `__exit__(Self)` **无异常参数** | `Drop::drop(&mut self)` |

**关键区别**：Python 的 `__exit__` 接收异常信息并可以返回 `True` 来吞掉异常。Vox 去掉了这个能力——异常处理交给 `try/catch`，`__exit__` 只负责清理。

```
// Vox 中 __exit__ 不接收异常参数
struct Connection:
    var host: str
    var port: int

    def __enter__(Self) -> Connection:
        self.connect()
        self

    def __exit__(Self):          // ← 无参数，纯清理
        self.disconnect()
```

转译成 Rust：

```rust
struct Connection {
    host: String,
    port: i32,
}

impl Connection {
    fn enter(&mut self) -> &mut Self {  // __enter__ 展开
        self.connect();
        self
    }
}

impl Drop for Connection {
    fn drop(&mut self) {                // __exit__ → Drop::drop
        self.disconnect();
    }
}

// with Connection("localhost", 8080) as conn:
//     conn.send("hello")
//
// 转译为：
{
    let mut conn = Connection::new("localhost", 8080);
    conn.enter();
    conn.send("hello");
}   // conn.drop() → disconnect()
```

#### 4.6.4 异常安全

Rust 的 `Drop` 保证：**即使 panic 或 `?` 提前返回，`drop()` 也会执行**。这是编译器级别的保证，不依赖 `finally` 块。

```
def safe_read(path: str) -> str raises IOError:
    with open(path) as f:
        val content = f.read()
        if content.is_empty():
            raise IOError("empty file")  // ← 即使这里抛出
        content
    // f 仍然会自动关闭（Drop 保证）
```

转译成 Rust 后，`?` 提前返回时 `f` 的 `drop()` 自动触发：

```rust
fn safe_read(path: &str) -> Result<String, IOError> {
    {
        let mut f = File::open(path)?;
        let content = f.read_to_string()?;
        if content.is_empty() {
            return Err(IOError::new("empty file"));
            // ↑ return 之前，f.drop() 自动执行
        }
        Ok(content)
    }
}
```

#### 4.6.5 不需要 `__enter__` 的类型（零开销）

很多 Rust 类型**自带 `Drop`**，不需要实现 `__enter__`/`__exit__` 就能直接用于 `with`：

```
// File 本身就有 Drop，不需要包装
with open("data.txt") as f:         // 直接工作
    print(f.read())

// 互斥锁也不需要包装
with mutex.lock() as guard:         // guard 的 Drop 自动解锁
    modify_shared_data()
```

转译时，Vox 编译器检测类型是否实现了 `Drop`：
- 有 `Drop` → 直接用于 `with`，零开销
- 无 `Drop` 但有 `__enter__`/`__exit__` → 生成 `Drop` impl
- 都没有 → 编译错误

#### 4.6.6 嵌套 with

```
with open("a.txt") as a:
    with open("b.txt") as b:
        val combined = a.read() + b.read()
        print(combined)
```

转译成 Rust 的嵌套块，利用 `Drop` 的**后进先出**（LIFO）语义：

```rust
{
    let mut a = File::open("a.txt")?;
    {
        let mut b = File::open("b.txt")?;
        let combined = a.read()? + b.read()?;
        println!("{combined}");
    }   // b.drop() 先执行
}   // a.drop() 后执行
```

#### 4.6.7 完整示例：数据库事务

```
struct Transaction:
    var conn: Connection
    var committed: bool

    def __enter__(Self) -> Transaction:
        self.conn.execute("BEGIN")
        self.committed = false
        self

    def __exit__(Self):
        if not self.committed:
            self.conn.execute("ROLLBACK")

    def commit(mut Self):
        self.conn.execute("COMMIT")
        self.committed = true

def transfer(from_id: int, to_id: int, amount: f64) raises DbError:
    with conn.begin() as tx:           // 开始事务
        debit(tx, from_id, amount)
        credit(tx, to_id, amount)
        tx.commit()                    // 成功则提交
    // 如果中间抛异常，__exit__ 自动 ROLLBACK
```

转译成 Rust：

```rust
struct Transaction {
    conn: Connection,
    committed: bool,
}

impl Transaction {
    fn enter(&mut self) -> &mut Self {
        self.conn.execute("BEGIN");
        self.committed = false;
        self
    }
    fn commit(&mut self) {
        self.conn.execute("COMMIT");
        self.committed = true;
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.committed {
            self.conn.execute("ROLLBACK");
        }
    }
}

fn transfer(from_id: i32, to_id: i32, amount: f64) -> Result<(), DbError> {
    {
        let mut tx = conn.begin()?;
        tx.enter();
        debit(&mut tx, from_id, amount)?;
        credit(&mut tx, to_id, amount)?;
        tx.commit();
        Ok(())
    }   // 异常时 tx.drop() → ROLLBACK
}
```

#### 4.6.8 with vs defer

| | `with` | `defer` |
|---|---|---|
| 作用域 | 绑定到 `with` 块 | 绑定到当前函数 |
| 依赖 | `__enter__`/`__exit__`（可选） | 任意闭包 |
| 适合 | 资源生命周期 = 代码块 | 函数退出时清理 |
| 开销 | 零（Rust 原生 Drop） | 一个闭包 |
| 示例 | `with open(f) as f:` | `defer: file.close()` |
| 嵌套 | 后进先出（LIFO） | 后进先出（LIFO） |

选择建议：
- 需要**块级作用域**（如事务、锁）→ `with`
- 需要**函数级清理**（如日志、计时）→ `defer`

### 4.7 标签块（Labeled Block）

`block #label_name:` 创建一个带标签的作用域，`break #label_name` 可从任意深度跳出，`break #label_name with expr` 可跳出并返回值。`#label_name` 在同作用域下必须唯一，否则报错。

#### 基础用法

```
block:
    // 无标签的普通块
    val x = 1
    val y = 2

block #init:
    if skip_all:
        break #init
    step1()
    if skip_rest:
        break #init
    step2()
```

转译成 Rust 的 labeled block：

```rust
{
    let x = 1;
    let y = 2;
}

'init: {
    if skip_all { break 'init; }
    step1();
    if skip_rest { break 'init; }
    step2();
}
```

#### 带返回值

```
val result = block #compute:
    if cache_hit:
        break #compute with cached_value
    val fresh = expensive()
    if fresh == 0:
        break #compute with default_value
    fresh
```

转译：

```rust
let result = 'compute: {
    if cache_hit { break 'compute cached_value; }
    let fresh = expensive();
    if fresh == 0 { break 'compute default_value; }
    fresh
};
```

#### 适用场景

| 场景 | 不用 block | 用 block |
|------|-----------|---------|
| 多条件提前退出 | 嵌套 `if-elif-else` | 扁平 `break #label` |
| 复杂初始化 | 提取函数 + `return` | 内联 `block` + `break` |
| 错误时回退默认值 | `match`/`if` 链 | `break #label with default` |

### 4.8 循环

Vox 提供三种循环：`loop`、`while`、`for`。支持标签跳转和守卫模式。

#### 4.8.1 loop — 无限循环

```
loop:
    val msg = recv()
    if msg == :quit:
        break
    process(msg)
```

转译成 Rust 的 `loop { ... }`：

```rust
loop {
    let msg = recv();
    if msg == Quit { break; }
    process(msg);
}
```

#### 4.8.2 while — 条件循环

```
var i = 0
while i < 10:
    print(i)
    i = i + 1
```

#### 4.8.3 for — 迭代器循环

```
for x in [1, 2, 3]:
    print(x)

for (k, v) in map:
    print(f"{k}: {v}")

for line in open("data.txt"):
    process(line)
```

#### 4.8.4 守卫模式（Guard）

**过滤守卫**：`for ... in ... if condition`

```
for x in 0..100 if x % 2 == 0:
    print(x)                        // 只打印偶数

for line in open("log.txt") if line.starts_with("ERROR"):
    print(line)                     // 只处理 ERROR 行
```

转译成 Rust 的 `if` 守卫（零开销，不创建中间集合）：

```rust
for x in 0..100 {
    if !(x % 2 == 0) { continue; }
    println!("{x}");
}
```

**模式守卫**：`while let pattern = expr`

```
while let Some(x) = iter.next():
    process(x)

while let Ok(data) = recv_result():
    handle(data)
```

转译成 Rust 的 `while let`：

```rust
while let Some(x) = iter.next() {
    process(x);
}
```

#### 4.8.5 标签跳转：`break #label` / `continue #label`

`#label:` 前缀给循环命名，`break #label` 跳出指定层，`continue #label` 跳到指定层的下一次迭代。

```
// 二维搜索
#outer: for row in 0..matrix.rows:
    for col in 0..matrix.cols:
        if matrix.get(row, col) == 0:
            continue #outer      // 跳过当前行，继续下一行
        if matrix.get(row, col) == target:
            print(f"Found at ({row}, {col})")
            break #outer         // 跳出外层循环
```

转译成 Rust 的 labeled loop：

```rust
'outer: for row in 0..matrix.rows {
    for col in 0..matrix.cols {
        if matrix.get(row, col) == 0 {
            continue 'outer;
        }
        if matrix.get(row, col) == target {
            println!("Found at ({row}, {col})");
            break 'outer;
        }
    }
}
```

#### 4.8.6 break 带返回值

`loop` 和 `for` 可作为表达式，`break with expr` 跳出并返回值：

```
val found = loop:
    val msg = recv()
    if msg.kind == :data:
        break with msg.payload
    // 继续等待

val first_even = for x in 0..100:
    if x % 2 == 0 and x > 10:
        break with x
    // 隐式 continue
```

转译：

```rust
let found = loop {
    let msg = recv();
    if msg.kind == Data { break msg.payload; }
};

let first_even = 'for_loop: {
    for x in 0..100 {
        if x % 2 == 0 && x > 10 { break 'for_loop x; }
    }
    panic!("not found")  // 兜底（如果 for 可能无结果则必须处理）
};
```

#### 4.8.7 循环对比

| | `loop` | `while` | `for` |
|---|---|---|---|
| 条件 | 无（手动 break） | 前置条件 | 迭代器耗尽 |
| 守卫 | — | `while let` | `if` 过滤 |
| `else` | — | ✅ | ✅ |
| 返回值 | `break with expr` | 不支持（始终 `()`） | `break with expr` |
| 适用 | 事件循环、重试 | 条件循环 | 遍历集合 |
| Rust 映射 | `loop { }` | `while { }` | `for in { }` |

#### 4.8.8 `else` 子句 — 无 break 时触发

`for` 和 `while` 支持 `else` 子句，与 Python 语义一致：循环**正常结束**（非 `break` 跳出）时执行 `else` 块。

```
// for-else：查找元素
def find_index(items: [int], target: int) -> int?:
    for (i, v) in items.enumerate():
        if v == target:
            print(f"Found at {i}")
            break
    else:
        print("Not found")
        return None
    i

// while-else：重试逻辑
def retry_connect(max_retries: int) -> Connection:
    var attempts = 0
    while attempts < max_retries:
        match connect():
        | Ok(conn) => return conn
        | Err(_)    => attempts = attempts + 1
    else:
        panic("All retries exhausted")
```

**守卫模式 + else**：`for ... if ... else` 的 `else` 始终触发。因为守卫转换为 `continue`（跳过元素），而非 `break`：

```
// 守卫模式的 else 总会执行
for x in 0..100 if x % 2 == 0:
    print(x)                // 只打印偶数
else:
    print("Done!")          // 遍历完 100 个元素后必定执行
```

语义总结：

| 情况 | `else` 是否执行 |
|------|:---:|
| 循环正常结束（迭代器耗尽/条件为假） | ✅ |
| `break` 跳出 | ❌ |
| `return` 退出函数 | ❌（函数已结束） |
| `raise` 抛出异常 | ❌（异常传播） |
| 守卫 `if` 过滤元素 | 不影响（守卫 = `continue`） |

---

### 4.9 `return` / `break` / `continue`

#### 4.9.1 `return` — 函数返回

```
def foo() -> int:
    return 42

def bar() -> (int, str):
    return 1, "hello"           // 隐式打包为元组
    return (1, "hello")         // 显式元组，等价
```

**元组返回**：`return 1, 2` 自动打包为 `(1, 2)`，与 Python 行为一致。括号可选。

**空返回**：`return` 等价于 `return ()`（空元组），函数返回类型为 `()`。

```
def maybe(x: bool) -> int?:
    if x:
        return 42
    return                      // 等价于 return None
```

#### 4.9.2 `break` — 跳出循环

```
loop:
    if done:
        break
```

`break` 可带标签跳出嵌套循环（见 4.8.5），也可带返回值（见 4.8.6）。

#### 4.9.3 `continue` — 跳过本次迭代

```
for x in items:
    if skip(x):
        continue
    process(x)
```

---

## 5. 函数式编程

Vox 的函数式特性与循环并存，按场景选择合适风格。

### 5.1 高阶函数与循环对照

```
// 替代 for i in range(10):
def loop(i: int, n: int, f: |int|):
    if i >= n:
        return
    f(i)
    loop(i + 1, n, f)

loop(0, 10, |i: int|: print(i))

// 高阶函数
val doubled = [1, 2, 3].map(|x| x * 2)           // [2, 4, 6]
val evens   = [1, 2, 3, 4].filter(|x| x % 2 == 0) // [2, 4]
val sum     = [1, 2, 3, 4].fold(0, |acc, x| acc + x) // 10
val found   = [1, 2, 3, 4].find(|x| x > 3)       // Some(4)
val all_ok  = [2, 4, 6].all(|x| x % 2 == 0)      // true
val any_odd = [1, 2, 3].any(|x| x % 2 != 0)     // true

// 惰性求值
val lazy = (1..).iter().map(|x| x * x).take(5)
val result = lazy.collect()  // [1, 4, 9, 16, 25]
```

### 5.2 迭代器 yield / yield from

```
// 生成器
def fibs() -> [int]:
    yield 0
    yield 1
    yield from _fib_loop(0, 1)

def _fib_loop(a: int, b: int) -> [int]:
    yield a + b
    yield from _fib_loop(b, a + b)

val first_10 = fibs().take(10).collect()
// [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]

// 合并多个生成器
def all_items() -> [int]:
    yield from [1, 2, 3]
    yield from fibs().take(3)
    yield from [10, 20, 30]
```

### 5.3 管道 `|>` 和函数组合 `<~` / `~>`

```
// 管道：数据从左到右流动
val result = [1, 2, 3, 4, 5]
    |> filter(|x| x % 2 == 0)
    |> map(|x| x * x)
    |> sum()

// 函数组合：右结合
def add1(x: int) -> int = x + 1
def mul2(x: int) -> int = x * 2
def add3(x: int) -> int = x + 3

val pipeline = add1 <~ mul2 <~ add3
// pipeline(5) = add1(mul2(add3(5))) = 17

// ~> 左结合（与 <~ 等价）
val g = add3 ~> mul2 ~> add1
// g(5) = add1(mul2(add3(5))) = 17
```

### 5.4 推导式（Comprehension）

与 Python 语法一致，支持列表、集合、字典三种推导式。`if` 守卫可选。

```
// 列表推导式
val squares = [x * x for x in 0..10]                    // [0, 1, 4, 9, ..., 81]
val evens   = [x for x in 0..10 if x % 2 == 0]          // 带守卫
val pairs   = [(x, y) for x in 0..3 for y in 0..3]      // 多重循环

// 集合推导式
val unique = {x % 3 for x in 0..10}                     // {0, 1, 2}

// 字典推导式
val squares_map = {x: x * x for x in 0..5}              // {: 0: 0, 1: 1, 2: 4, ...}
val filtered     = {k: v for k, v in pairs() if v > 0}  // 带守卫

// 嵌套推导式
val matrix = [[i * j for j in 0..3] for i in 0..3]
// [[0, 0, 0, 0], [0, 1, 2, 3], [0, 2, 4, 6], [0, 3, 6, 9]]
```

**转译**：列表推导式 → Rust 的 `.map()` + `.filter()` + `.collect()` 链式调用，零额外开销。

```
Vox                                      Rust
───                                      ────
[x * 2 for x in 0..10]                   (0..10).map(|x| x * 2).collect()
[x for x in items if x > 0]              items.iter().filter(|x| *x > 0).cloned().collect()
[(x, y) for x in 0..3 for y in 0..3]     (0..3).flat_map(|x| (0..3).map(move |y| (x, y))).collect()
{x: x * x for x in 0..5}                 (0..5).map(|x| (x, x * x)).collect()
```

**与 `_` 占位符协同**：

```
// 推导式内使用 _ 占位符
val doubled = [x * 2 for x in items]     // 推导式写法
val doubled = items.map(_ * 2)           // 等价于 _ 占位符写法
```

---

## 6. 类型系统

### 6.1 基本类型

```
val name: str = "Vox"
val age: int = 42
val pi: f64 = 3.14159
val flag: bool = true
val data: bytes = b"hello"
val nothing: None = None
```

### 6.2 Option 类型 (`?`)

```
// int? 等价于 Option<int>
def find_user(id: int) -> str?:
    if id == 0:
        return None
    "User {id}"

// 安全访问
val name = user?.profile?.name

// 强制解包
val ensured = risky_call() ??          // None 时 panic
val name = user?.profile?.name ?? "Anonymous"
```

### 6.2.1 所有权转移 (`^`)

`^` 是**后缀操作符**，表示显式所有权转移。对标 Rust 的 `std::mem::take` 或 `std::mem::replace`。

```
val s = 1
val s2 = s^          // s 的所有权转移给 s2，之后 s 不可用
```

**语义**：`^` 将变量的所有权移出，原变量变为未初始化状态。编译期检查，确保转移后原变量不再被使用。

**规则**：
- `^` 紧贴变量名，是后缀操作符
- 只能用于 `val` / `var` 声明的变量
- 所有权转移后，原变量不可再访问（编译期错误）

```
val a = 42
val b = a^           // ✅ a 的所有权转移给 b
print(b)             // ✅ 42
// print(a)          // ❌ 编译错误：a 已被移动

var c = 10
c = c^ + 1           // ✅ 先移出 c 再赋新值
```

**转译**：`s^` 转译成 Rust 的 `std::mem::take(&mut s)` 或直接 `s`（利用 Rust 的 move 语义）。

```
Vox                     Rust
───                     ────
val s2 = s^             let s2 = s;  // s 被 move
val s2 = s^             let s2 = std::mem::take(&mut s);  // 或显式 take
```

### 6.2.2 自增/自减（`++` / `--`）

`++` 和 `--` 既是前缀也是后缀操作符，语义如下：

| 形式 | 语义 | 返回值 |
|------|------|--------|
| `++cnt` | 前缀自增 | 返回**更新前**的值 |
| `cnt++` | 后缀自增 | 返回**更新后**的值 |
| `--cnt` | 前缀自减 | 返回**更新前**的值 |
| `cnt--` | 后缀自减 | 返回**更新后**的值 |

```
var cnt = 0
val a = ++cnt   // a = 0, cnt = 1
val b = cnt++   // b = 2, cnt = 2
val c = --cnt   // c = 1, cnt = 0
val d = cnt--   // d = -1, cnt = -1
```

> 此语义同样适用于用户自定义的 `prefix` / `suffix` 操作符。

### 6.3 容器类型

```
val list: [int] = [1, 2, 3, 4, 5]
val tuple: (int, str, bool) = (42, "hello", true)
val dict: {:str: int} = {: "a": 1, "b": 2, "c": 3}
val set: {int} = {1, 2, 3, 4, 5}
```

**字典语法**：`{:` 是字典字面量的**起始标记**（双字符 token），后续 `:` 是键值分隔符。

```
{:}                 // 空字典
{: "a": 1}          // 单键值对
{: "a": 1, "b": 2}  // 多键值对
```

**集合语法**：`{}` 是集合字面量。

```
{}                  // 空集合
{1, 2, 3}           // 非空集合
```

**区分规则**：

| 开头 | 内容 | 结果 |
|------|------|------|
| `{:` | `}` 直接闭合 | 空字典 |
| `{:` | `key: val, ...` | 字典 |
| `{` | `}` 直接闭合 | 空集合 |
| `{` | `elem, elem, ...` | 集合 |
| `{` | `expr for ...` | 集合推导式 |
| `{` | `key: val for ...` | 字典推导式 |

> **设计说明**：`{:}` 作为字典开头的 `{:` token 设计与 `{}` 集合区分，避免空字典/空集合的歧义。`{: "a": 1}` 中两个 `:` 职责不同——第一个属于 `{:` token（字典标记），第二个是键值分隔符。解析器在 `{` 后向前看一个字符即可区分。类型标注 `{:str: int}` 与值字面量 `{: "a": 1}` 使用相同的 `{:` 前缀，语义一致。

### 6.4 `define` — 类型约束定义

`define` 定义结构化的**鸭子类型约束**，**仅用于函数参数**。不同于 `trait`（名义类型），`define` 是**结构类型**——只要满足结构要求即可匹配。

> **核心规则：`define` 中定义的约束名只能出现在函数参数的圆括号里，不能作为实际类型使用。**

```
define:
    F0<T>: || -> T
    Callback<T>: |T| -> bool

// ✅ 正确：函数参数约束
def transform(f: F0<int>) -> int:
    f()

// ❌ 错误：define 定义的鸭子约束不是真正的类型
val f: F0 = ...               // Error — F0 未定义为真实类型
val f: |t: F0| -> bool = ...  // Error — 赋值语句中 F0 类型未定义
```

**`define` 内定义的 enum/struct/class 可复用其他 define 约束**：

```
define:
    F0<T>: || -> T
    T:
        enum(F0):     // 复用 F0 约束
            ...
```

**`define` 定义的名称与泛型参数同名时**：函数定义的泛型参数优先，遮蔽 `define` 中的名称：

```
def add<F0>(f: F0):    // 这个 F0 是泛型参数，不是 define 中的 F0
    ...
```

**总结**：`define` 中定义的约束**仅能用作函数圆括号内的参数类型约束**，减少 `where` 语句的冗长写法。

#### 语法

```
define:
    约束名 <泛型>: 约束体
```

#### 简单类型别名

```
define:
    F0<T>: | | -> T                      // 无参函数，返回 T
    F1<T, R>: |T| -> R                   // 单参函数
    Callback<T>: |T| -> bool             // 回调签名
    IntPair: (int, int)                  // 元组别名

def transform(f: F0<int>) -> int:
    f()

def filter_by(data: [int], pred: Callback<int>) -> [int]:
    data.filter(pred)
```

#### 结构类型约束：class

```
define:
    class Drawable:
        props = {x: f64, y: f64}         // 必须包含这些属性
        instancemethods = {               // 必须包含这些实例方法
            draw: |Self| -> None,
            area: |Self| -> f64,
        }

def render(obj: Drawable):
    print(f"Area: {obj.area()}")
    obj.draw()

// 任何有 x, y 属性和 draw, area 方法的类型都能传入
```

#### 结构类型约束：完整形态

```
define:
    class MyClass:
        props = {name: str, age: int}                 // 属性约束
        staticmethods = {factory: || -> Self}          // 静态方法约束
        typemethods = {from_str: |str| -> Self}        // 类方法约束
        instancemethods = {                             // 实例方法约束
            greet: |Self| -> str,
            update: |mut Self, int| -> None,
        }

        // 自定义检查（替代上述所有约束）
        def __check__(cls, other_cls: Type) -> bool:
            // 返回 true 表示 other_cls 匹配此约束
            ...
```

`__check__` 的优先级：如果定义了 `__check__`，则忽略 `props`/`staticmethods`/`typemethods`/`instancemethods`，完全由 `__check__` 决定匹配。

#### 结构类型约束：enum / trait / struct

```
define:
    enum MyEnum:
        // 必须包含这些变体
        variants = {Success, Failure}

    trait MyTrait:
        // 必须包含这些方法
        instancemethods = {process: |Self| -> int}

    struct MyStruct:
        // 必须包含这些字段
        props = {id: int, label: str}
```

#### 使用

```
def func(f: F0<int>, cls: MyClass, e: MyEnum):
    val result = f()
    cls.greet()
    match e:
        | Success => print("ok")
        | Failure => print("fail")
```

#### 预定义类型别名

`define` 可定义常用参数模式，替代 `*args` 异类型和 `**kwargs`：

`KWargsType<K, V>` 是内置泛型类型，用于模拟关键字参数。`K` 是键类型（字面量字符串），`V` 是值类型。配合 `Include` 约束限定允许的键集合。

```
define:
    // 固定数量异类型参数（替代 *args 异类型）
    Args2<T1, T2>: (T1, T2)
    Args3<T1, T2, T3>: (T1, T2, T3)

    // 关键字参数（替代 **kwargs）
    // Include 约束 K 只能是指定的字面量
    KWargs3: KWargsType<K, str> where K Include [name, age, city]

def create_user(args: Args3<str, int, str>) -> User:
    val (name, age, city) = args
    User(name, age, city)

def query(filter: KWargs3) -> [User]:
    // filter.name, filter.age, filter.city 可用
    ...

// 使用
create_user(("Alice", 25, "Beijing"))
query(KWargs3(name="Alice", city="Beijing"))
```

`Include` 约束键类型为有限字面量集合，转译时生成 struct 或 enum：

```rust
// Vox: KWargs3: KWargsType<K, str> where K Include [name, age, city]
// Rust:
struct KWargs3 {
    name: Option<String>,
    age: Option<String>,
    city: Option<String>,
}
```

#### 可见性

`define` **只能写在模块顶层**，不能写在函数或局部作用域内。使用 `_` 前缀控制可见性：

```
// 模块顶层 — 公开约束
define: Sortable<T>: {
    props = {cmp: (T, T) -> int}
}

// 模块顶层 — 私有约束（_ 前缀）
define: _InternalHelper<T>: {
    props = {validate: (T) -> bool}
}
```

**公开 API 签名约束**：公开函数的签名中不能出现私有 `define` 约束，否则编译报错。

```
// OK — 私有函数使用私有约束
def _internal_sort(data: [_InternalHelper]): [_InternalHelper]:
    ...

// 编译错误！公开函数签名暴露了私有约束 _InternalHelper
def public_sort(data: [_InternalHelper]): [_InternalHelper]:
    // Error: 公开 API 中不能使用私有类型约束 `_InternalHelper`

// OK — 改用公开约束
def public_sort(data: [Sortable]): [Sortable]:
    ...

// OK — 内部辅助函数，不受此限
def _private_helper(data: [_InternalHelper]): bool:
    ...
```

**设计理由**：与 Rust 的"公开类型不能暴露私有类型"规则一致。如果公开 API 引用了私有约束，外部调用者无法理解该类型，破坏了封装。私有函数内部使用私有约束则完全自由。

#### define vs trait

| | `define` | `trait` |
|---|---|---|
| 类型系统 | 结构类型 | 名义类型 |
| 匹配方式 | 自动（满足结构即可） | 显式 `impl` |
| 默认实现 | 不支持 | 支持 |
| 适用 | 临时约束、函数参数 | 公共接口、多态 |
| 可见性 | 仅模块级（`_` 前缀私有） | 仅模块级 |

---

## 6.5 参数包构建符号

Vox 提供三种参数包构建符号，将代码块生成的值作为参数应用于可调用/可索引/可括取对象。

### 6.5.1 `<:` — Callable 调用（`__call__`）

`<:` 符号表示：**先执行右侧代码块，再将块内最后表达式的返回值作为参数包，调用左侧的 callable 对象**。

```
callable_obj <:
    // 参数生成块（先执行）
    // 最后一行作为参数包传递给 callable_obj
    (arg1, arg2, ...)
```

**语义**：`<:` 右侧块先执行，块内最后一行（元组或字典）作为参数包，自动解包后调用左侧函数。

```
var cnt = 0
val f = | | => 
    print(cnt)
    cnt += 1
    cnt++

val c = f <:
   print(f())   // ① f() 执行：cnt 0→2，打印 0 2
   cnt += 1     // ② cnt 2→3
   print(f())   // ③ f() 执行：cnt 3→5，打印 3 5
   ()           // ④ 块返回 ()，作为参数包调用 f(())
                //    f() 执行：cnt 5→7，返回 7
// c = 7，打印：0 2 3 5 5 7
```

#### 连级 `<:`

`<:` 支持连级调用，返回的依旧是 callable 对象时可继续使用 `<:`：

```
val lst3 = lst.map <:
    val f = |x: int| => x * 2
    (f,) <:                      // 连级，继续应用于 __call__
        val f = |i: int| => i > 10
        {: "pred": f}            // 返回字典，自动解包
```

- 连级 `<:` 需要缩进
- 上层返回的必须是 callable 对象
- 支持元组（位置参数）和字典（关键字参数）

#### 生命周期

`<:` 块内创建的变量，Vox 转译器自动处理生命周期问题，确保变量在 callable 调用期间有效。

### 6.5.2 Callable trait

```
trait Callable:
    def __call__(Self, ..) -> Any:
        ...
```

实现了 `__call__` 实例方法的类型即可使用 `<:` 语法。不对参数和返回值做具体约束（duck typing）。

### 6.5.3 `^:` — Indexable 索引（`__getitem__`）

`^:` 对应方括号 `[]` 索引，构建参数包传递给 `__getitem__`。

```
trait Indexable:
    def __getitem__(Self, ..) -> Any:
        ...
```

```
// 使用方式
val result = obj ^:
    val key = compute_key()
    (key,)           // 等价于 obj[key]
```

### 6.5.4 `@:` — Braceable 括取（`__brace__`）

`@:` 对应花括号 `{}` 括取，构建参数包传递给 `__brace__`。

```
trait Braceable:
    def __brace__(Self, ..) -> Any:
        ...
```

```
// 使用方式
val result = obj @:
    val config = build_config()
    {: "key": config}  // 等价于 obj{key: config}
```

### 6.5.5 三种参数包符号对比

| 符号 | 对应 trait | 对应方法 | 等价语法 |
|------|-----------|---------|---------|
| `<:` | `Callable` | `__call__` | `obj(...)` |
| `^:` | `Indexable` | `__getitem__` | `obj[...]` |
| `@:` | `Braceable` | `__brace__` | `obj{...}` |

---

### 6.6 Atom 字面量

Atom 是**轻量级不可变标识符**，以 `:` 前缀表示。对标 Ruby 的 Symbol 和 Elixir 的 Atom。

```
:inc
:dec
:get
:reset
:ok
:error
```

**语义**：Atom 是编译期确定的唯一标识符，同一名称的 Atom 在全局共享同一实例。本质是 `u64` 哈希值，比较操作是 O(1) 整数比较。

**用途**：
- 消息传递（Actor 模式匹配）
- 枚举替代（轻量标签）
- 字典 key（比字符串快）

```
// 消息匹配
def handle(msg):
    match msg:
        | :inc => counter += 1
        | :dec => counter -= 1
        | :reset => counter = 0

// 作为字典 key
val config = {: :host: "localhost", :port: 8080}
val host = config[: :host]    // 字典 key 场景下 `:` 前需空格
```

**转译**：Atom 编译为 `u64` 常量（编译期哈希），不是字符串。

```
// Vox                              // Rust
val a = :inc                        const A: u64 = hash("inc");
```

---

### 6.7 类型作为一等值

Vox 中类型可以作为**运行时值**使用，传递、存储、检查。

#### 6.7.1 类型引用

```
val t = Enum3         // t 是类型引用，类型为 Type
val s = Struct1       // s 是类型引用，类型为 Type
```

类型引用本身是值，不是实例。`Enum3` 是类型，`Enum3.Variant` 是实例。

#### 6.7.2 类型列表

```
val types = [Struct1, Class2, Enum3, Copyable]  // 类型为 [Type]
```

类型列表可用于注册表、工厂模式、运行时检查。

#### 6.7.3 `type()` 函数

```
val t = Enum3
val cat = type(t)      // 返回 TypeCategory::Enum
val name = type(t).name()  // 返回 "Enum3"
```

`type()` 返回 `TypeCategory` 枚举：

| 值 | 说明 |
|------|------|
| `TypeCategory::Struct` | struct 类型 |
| `TypeCategory::Class` | class 类型 |
| `TypeCategory::Enum` | enum 类型 |
| `TypeCategory::Trait` | trait 类型 |

#### 6.7.4 使用场景

```
// 类型注册表
val registry = [Struct1, Class2, Enum3]

// 运行时类型检查
for t in registry:
    if type(t) == TypeCategory::Struct:
        print("struct: " + t.name())

// 工厂模式
def create_instance(t: Type) -> Any:
    match type(t):
        | TypeCategory::Struct => t()
        | TypeCategory::Enum => t.default()
        | _ => panic("unsupported")
```

#### 6.7.5 限制

- 类型引用是**不可变**的（编译期确定）
- 不能动态创建新类型
- 泛型类型必须完全实例化：`List<int>` 可用，`List` 不可用（缺少类型参数）

---

## 7. struct

不可变数据，无继承。

### 7.1 定义

```
struct Point<T>:
    x: T
    y: T

    // 构造函数
    def new(x: T, y: T) -> Point<T>:
        Point { x, y }

    // 方法（必须有实现）
    def length(Self) -> f64:
        (self.x * self.x + self.y * self.y).sqrt()

    // 私有方法（_ 前缀）
    def _helper(Self) -> T:
        self.x + self.y
```

### 7.1.1 Trait 组合

`struct` 支持 trait 组合语法，`()` 内列出的 trait 的抽象方法必须在 struct 内实现：

```
struct Point<T>(Display, Clone):
    x: T
    y: T

    // Display trait 的抽象方法必须实现
    def display(Self) -> str:
        f"Point({self.x}, {self.y})"

    // Clone trait 的抽象方法必须实现
    def clone(Self) -> Self:
        Point(self.x, self.y)
```

> **注意**：struct 内部没有抽象方法——所有来自 trait 的抽象方法必须在 struct 内实现。struct **不支持继承**。

| 特性 | 规则 |
|------|------|
| 字段 | 全部 `val`，不能 `var` |
| 继承 | 不支持 |
| 抽象方法 | 不支持，全部有实现 |
| 私有 | `_` 前缀 | 方法和字段以下划线开头为私有 |
| 魔法方法 | 始终公开 | `__xxx__` 始终公开 |
### 7.2 扩展方法

```
// 新增方法
impl Point:
    def magnitude(Self) -> f64:
        (self.x * self.x + self.y * self.y).sqrt()

// 删除方法（核心魔法方法不可移除）
omit Point._helper
```

---

## 8. class

可变数据，单继承，支持 trait 组合。转译成 Rust 组合 + Deref。

### 8.1 定义与继承

```
class 名称 <泛型> (父类?, trait1, trait2, ...):
    ...
```

**继承规则**：需要继承时，父类必须放在 `()` 内**第一个**位置，trait 跟随其后。

```
class Animal:
    var name: str

    def new(name: str):
        self.name = name

    def speak(Self) -> str:
        "..."

class Dog(Animal, Display, Clone):
    var breed: str

    def new(name: str, breed: str):
        self.name = name
        self.breed = breed

    def speak(Self) -> str:
        f"Woof! I'm {self.name}"

    def fetch(Self):
        print(f"{self.name} fetches ball")

    // Display trait 抽象方法必须实现
    def display(Self) -> str:
        f"Dog({self.name}, {self.breed})"

    // Clone trait 抽象方法必须实现
    def clone(Self) -> Self:
        Dog(self.name, self.breed)
```

> **注意**：class 内部没有抽象方法——所有来自 trait 的抽象方法必须在 class 内实现。

### 8.2 转译方式

```
class Dog(Animal)  →  struct Dog { base: Animal, breed: String }
                     + impl Deref/DerefMut for Dog

self.name          →  Deref 自动访问父类字段
self.base.method() →  显式调用父类方法
```

---

## 9. trait

### 9.1 定义

trait 仅支持方法（抽象或默认实现），**不支持字段**，与 Rust 对齐。trait 支持组合语法 `trait Name<Generic>(Parent1, Parent2, ...)`，组合的 trait 中的抽象方法**可自行实现或不实现**。

```
trait 名称 <T> (Parent1, Parent2, ...):
    // 抽象方法（无函数体）
    def next(mut Self) -> Option<T>

    // 默认实现（有函数体）
    def has_next(Self) -> bool:
        self.next().is_some()

    // 类方法（首个参数 Type）
    def from_str(Type, s: str) -> Self:
        ...

    // 静态方法（首个参数不是 Self 也不是 Type）
    def helper(x: int) -> bool:
        ...
```

**使用 trait 的类型负责实现**：当 `enum`、`struct`、`class` 使用 trait 组合时，它们必须实现所有未实现的抽象方法。

`Self`、`Type` 是关键字，方法类型由第一个参数决定：

| 首个参数 | 含义 | 调用方式 |
|----------|------|----------|
| `Self` | 实例方法 | `instance.method()` |
| `Type` | 类方法 | `TypeName::method()` |
| 其他 | 静态方法 | `TypeName::method()` |

### 9.2 示例

```
trait Iterator<T>:
    def next(mut Self) -> Option<T>           // 抽象

    def nth(mut Self, n: int) -> Option<T>:   // 默认实现
        def _loop(i: int):
            if i >= n:
                return None
            self.next()
            _loop(i + 1)
        _loop(0)

    def collect(Self) -> [T]:                 // 默认实现
        val mut items = []
        def _loop():
            match self.next():
            | Some(v) => items.append(v); _loop()
            | None    => return items
        _loop()

    def empty(Type) -> Self:                  // 静态工厂
        EmptyIterator()
```

### 9.3 实现 trait

```
struct Counter:
    var n: int

impl Iterator<int> for Counter:
    def next(mut Self) -> Option<int>:
        val current = self.n
        self.n = self.n + 1
        Some(current)

val iter = Counter(0)
val first_5 = iter.take(5).collect()  // [0, 1, 2, 3, 4]
```

### 9.4 Trait 组合：`&`

```
trait Read:
    def read(Self) -> str

trait Write:
    def write(mut Self, data: str)

val ReadWrite = Read & Write
// 等价于手写 trait ReadWrite: Read, Write

impl ReadWrite for File:
    def read(Self) -> str:
        // ...
    def write(mut Self, data: str):
        // ...
```

#### 同名方法冲突规则

| 情况 | 结果 |
|------|------|
| 同名 + 同参数签名 + 同返回类型 | 合并（视为同一方法） |
| 同名 + 同参数签名 + 不同返回类型 | **冲突** → 编译错误 |
| 同名 + 不同参数签名 | **合法** → 多分派 |

**规则 1：同签名不同返回 → 冲突**

```
trait A:
    def foo(Self) -> str: "A"

trait B:
    def foo(Self) -> bool: False

val C = A & B
// ❌ 编译错误：foo 签名相同但返回类型不同
```

**规则 2：不同参数签名 → 多分派（合法）**

```
trait A:
    def foo(Self, int) -> bool: True

trait B:
    def foo(Self) -> bool: False

val C = A & B
// ✅ 合法 — 参数签名不同，满足多分派
```

**规则 3：多分派抽象 → 全部实现**

```
trait A:
    def foo(Self) -> bool          // 抽象

trait B:
    def foo(Self, int) -> bool     // 抽象

val C = A & B
// C 有 2 个抽象 foo 方法

impl C for MyType:
    // 必须全部实现
    def foo(Self) -> bool: True
    def foo(Self, n: int) -> bool: n > 0
```

**规则 4：部分有默认实现**

```
trait A:
    def foo(Self) -> bool: True    // 有默认实现

trait B:
    def foo(Self, int) -> bool     // 抽象

val C = A & B
// C 只有 1 个抽象 foo（B 的），A 的默认实现保留

impl C for MyType:
    def foo(Self, n: int) -> bool: n > 0
    // A 的 foo(Self) 默认实现自动继承
```

---

## 10. impl / extend / omit

### 10.1 `impl` — 为类型实现整个 trait

```
impl TraitName for TypeName:
    // 必须实现 trait 中的所有抽象方法
    def method(Self) -> T:
        ...
```

### 10.2 `extend` — 将单个方法借给另一个类型

#### 语法

```
extend func_name [as new_name] for TargetType
```

#### 从 trait/struct/class 借用方法

方法自身已有类型（实例/类/静态），`extend` 时**不需要**加 `Self|Type|Static` 前缀：

```
trait Show:
    def show(Self) -> str

struct Point:
    x: f64
    y: f64

impl Show for Point:
    def show(Self) -> str:
        f"Point({self.x}, {self.y})"

extend show for Vector
// Vector 现在也有 show() 方法了，复用 Point 的实现

// 支持换名
extend show as to_string for Vector
// Vector.to_string() 等价于原来的 show()
```

#### 从模块级函数借用

模块级函数**没有** `Self` 参数，必须显式指定方法类型：

```
def double(x: int) -> int:
    x * 2

extend Static double for int
// int.double() 可用

def from_str(s: str) -> MyStruct:
    parse(s)

extend Type from_str for MyStruct
// MyStruct.from_str("...") 可用
```

#### 规则

1. trait/struct/class 的方法 → 不加 `Self|Type|Static`，类型自动继承
2. 模块级函数 → 必须加 `Self|Type|Static` 指定方法类型
3. 支持 `as` 换名：`extend func_name as new_name for TargetType`
4. 编译器检查方法签名兼容

### 10.3 `omit` — 移除方法或枚举值

```
// 移除方法
omit Point._helper
omit HashMap.keys
omit Iterator.collect

// 移除枚举值
omit Color.Green
omit Status.Inactive
```

**核心魔法方法不可移除**（编译报错）：

```
__new__, __init__, __del__, __copy__, __move__,
__eq__, __ne__, __hash__,
__add__, __sub__, __mul__, __div__, __mod__,
__getitem__, __setitem__, __len__, __iter__, __next__,
__bool__, __int__, __float__, __str__, __repr__,
__call__, __enter__, __exit__,
__getattr__, __setattr__
```

### 10.4 三者对比

| | `impl` | `extend` | `omit` |
|---|---|---|---|
| 作用对象 | trait → 类型 | 方法/枚举值 → 类型 | 类型上的方法/枚举值 |
| 粒度 | 整个 trait | 单个方法/枚举值 | 单个方法/枚举值 |
| 要求 | 抽象方法全实现 | 签名兼容 | 非核心魔法方法 |
| 示例 | `impl Show for Point` | `extend show for Vector` | `omit Point._helper` |

### 10.5 enum 的 impl / extend / omit

`impl`、`extend`、`omit` 对枚举同样适用。

#### extend 增加枚举值

```
enum Color:
    Red(u8, u8, u8)
    Green(u8, u8, u8)
    Blue(u8, u8, u8)

extend Enum Yellow(u8, u8, u8) for Color
extend Enum Cyan(u8, u8, u8) for Color
// Color 现在有 Red, Green, Blue, Yellow, Cyan
```

#### omit 移除枚举值

```
omit Color.Green
// Color 现在有 Red, Blue, Yellow, Cyan
```

#### impl 为枚举实现 trait

```
enum Order:
    Pending
    Shipped(str)

impl Display for Order:
    def display(Self) -> str:
        match self:
            | Pending    => "Pending"
            | Shipped(s) => f"Shipped({s})"
```

转译：Vox 编译器收集所有 `extend` 追加的枚举值，生成最终的完整枚举定义。

### 10.6 `ignore` — 忽略冲突

`ignore` 用于在 `extend` / `omit` / `test` 场景中抑制冲突报错。

#### ignore test

忽略当前模块的指定测试集合。测试运行时打印被忽略的测试个数，使用 `tests` 模块可展示具体跳过了哪些测试。

```
ignore test:
    // 必须返回 List[str] 对象
    ["test_slow", "test_flaky", "test_experimental"]

test:
    // 无返回值
    ...
```

#### ignore extend

若目标 Type 已有同名且同参数签名的方法，正常情况会报错。加上 `ignore extend` 后，编译直接通过，目标将拥有该方法：

```
ignore extend func_name for TargetType
// 不管目标是否已有同名同签名方法，编译通过，目标拥有该方法
```

#### ignore omit

若目标 Type 没有要移除的同名且同签名方法，正常情况会报错。加上 `ignore omit` 后，编译直接通过，目标将去掉该方法（如果存在）：

```
ignore omit TargetType.method_name
// 不管目标是否有该方法，编译通过，目标去掉该方法（如果存在）
```

---

## 11. 魔法方法

```
struct Vector:
    x: f64
    y: f64

    // 算术
    def __add__(Self, other: Vector) -> Vector:
        Vector(self.x + other.x, self.y + other.y)

    def __sub__(Self, other: Vector) -> Vector:
        Vector(self.x - other.x, self.y - other.y)

    def __mul__(Self, scalar: f64) -> Vector:
        Vector(self.x * scalar, self.y * scalar)

    def __neg__(Self) -> Vector:
        Vector(-self.x, -self.y)

    // 索引
    def __getitem__(Self, index: int) -> f64:
        match index:
            | 0 => self.x
            | 1 => self.y
            | _ => panic("out of bounds")

    // 迭代
    def __iter__(Self) -> [f64]:
        yield self.x
        yield self.y

    // 比较
    def __eq__(Self, other: Vector) -> bool:
        self.x == other.x and self.y == other.y

    def __lt__(Self, other: Vector) -> bool:
        self.magnitude() < other.magnitude()

    // 表示
    def __repr__(Self) -> str:
        f"Vector({self.x}, {self.y})"

    // 可调用
    def __call__(Self, t: f64) -> Vector:
        Vector(self.x * t, self.y * t)

    // 上下文管理
    def __enter__(Self):
        self

    def __exit__(Self):
        pass
```

完整魔法方法列表：

| 类别 | 方法 |
|------|------|
| 算术 | `__add__` `__sub__` `__mul__` `__div__` `__mod__` `__pow__` |
| 带赋值 | `__iadd__` `__isub__` `__imul__` `__idiv__` |
| 一元 | `__neg__` `__pos__` `__abs__` `__invert__` |
| 比较 | `__eq__` `__ne__` `__lt__` `__le__` `__gt__` `__ge__` |
| 容器 | `__getitem__` `__setitem__` `__delitem__` `__len__` `__contains__` |
| 迭代 | `__iter__` `__next__` |
| 转换 | `__bool__` `__int__` `__float__` `__str__` `__repr__` |
| 调用 | `__call__` |
| 上下文 | `__enter__` `__exit__` |
| 属性 | `__getattr__` `__setattr__` `__delattr__` |
| 生命周期 | `__init__` `__del__` `__copy__` `__move__` |

---

## 12. 宏与装饰器

### 12.1 装饰器

**任何带参数的函数都可以作为装饰器**。支持柯里化求值，可传部分关键字参数。

**装饰器语法**：`@name` 作为装饰器，`@name!` 作为宏装饰器。

| 语法 | 含义 | 位置要求 |
|------|------|---------|
| `@name` | 装饰器 | 函数/类定义前 |
| `@name!` | 宏装饰器 | **必须独占一行** |

```
// 无参装饰器：直接包裹
def log(f: |int| -> int) -> |int| -> int:
    |x: int| -> int:
        print("Calling...")
        val result = f(x)
        print(f"Result: {result}")
        result

@log
def compute(x: int) -> int:
    x * x

// 等价于：compute = log(compute)

// 有参装饰器：柯里化
def cache(ttl: int, max_size: int)(f: |int| -> int) -> |int| -> int:
    // ttl, max_size 先绑定，f 后传入
    |x: int| -> int:
        // 缓存逻辑
        f(x)

@cache(ttl=60, max_size=100)
def expensive(x: int) -> int:
    x * x

// 等价于：expensive = cache(ttl=60, max_size=100)(expensive)

// 混合传参
@cache(60, max_size=100)
def heavy(x: int) -> int:
    x * x
```

规则：
- 装饰器本质是函数调用，`@f` 等价于 `target = f(target)`
- `@f(args)` 等价于 `target = f(args)(target)`（柯里化）
- 支持位置参数和关键字参数混用
- 多个装饰器从下往上求值：`@a @b` 等价于 `target = a(b(target))`

#### 宏装饰器 (`@name!`)

`@name!` 是宏装饰器，必须**独占一行**。`!` 紧贴装饰器名，表示编译期展开。

```
// 宏装饰器：编译期展开
@derive!
struct Point:
    x: float
    y: float

// 等价于调用 derive 宏（编译期执行）
// derive 宏生成 Debug、Clone、PartialEq 等实现

// 多个宏装饰器（每个独占一行）
@derive!
@mark!
struct Config:
    host: str
    port: int
```

**规则**：
- `@name!` 表示宏调用，`name` 是注册的宏名
- 必须独占一行，不能与其他代码同行
- `!` 和 `name` 之间不能有空格

### 12.2 宏 (`macro`)

宏是**编译期操作 Token 流**的特殊函数，使用 `macro` 关键字替代 `def`。

#### 声明

```
macro 名称(ts: Tokens) -> Tokens:
    ...
```

- 参数必须是 `ts: Tokens`（接收后续语句块的 Token 流）
- 返回值必须是 `Tokens`（替换原始 Token 流）
- 签名固定，不可自定义

#### 使用

```
@名称
def|val|var|struct|trait|class|enum|impl|extend|macro|...
    语句块
```

宏必须**独占一行**，后跟任意关键字开头的语句块。

#### 示例

```
// 宏：自动打印调用
macro log_call(ts: Tokens) -> Tokens:
    // ts 是后面整个语句块的 Token 流
    // 返回修改后的 Token 流
    return quote:
        print("Calling...")
        $(ts)
        print("Done.")

@log_call
def heavy_computation(x: int) -> int:
    x * x

// 展开后等价于：
// def heavy_computation(x: int) -> int:
//     print("Calling...")
//     val result = x * x
//     print("Done.")
//     result
```

```
// 宏：条件编译
macro debug_only(ts: Tokens) -> Tokens:
    if DEBUG:
        return ts
    else:
        return Tokens::empty()

@debug_only
val verbose = compute_expensive_debug_info()
```

```
// 宏：自动生成 trait 实现
macro derive_debug(ts: Tokens) -> Tokens:
    // ts 是 struct 定义
    // 为 struct 自动生成 Debug trait 实现
    return quote:
        $(ts)
        impl Debug for /* 提取 struct 名 */:
            def debug(Self) -> str:
                // 自动拼接所有字段
                ...
```

#### 宏 vs 装饰器

| | 装饰器 | 宏 |
|---|---|---|
| 作用时机 | 运行时 | 编译期 |
| 操作对象 | 函数值 | Token 流 |
| 关键字 | `def` | `macro` |
| 签名 | 自由 | `(ts: Tokens) -> Tokens` |
| 能力 | 包装函数 | 生成/修改任意代码 |

### 12.3 模板 (`template`)

模板是**编译期代码替换**——调用处直接展开，不需要手动操作 Token 流。

**应用模板**使用 `template_name!(...)` 语法，`!` 紧贴模板名，**区分模板调用和普通函数调用**。

#### 声明

```
template 名称(参数) -> 返回类型:
    函数体
```

#### 示例

```
// 基础模板：消除重复代码
template times(n: int, body: |int|):
    def _loop(i: int):
        if i >= n: return
        body(i)
        _loop(i + 1)
    _loop(0)

times!(5, |i: int|: print(i))
times!(10, |i: int|: print(i * 2))
times!(3, |i: int|: heavy_work(i))

// 模板：自动 defer
template with_file(path: str, body: |File|):
    val f = open(path)
    defer: f.close()
    body(f)

with_file!("data.txt", |f: File|:
    print(f.read())
)

// 模板：benchmark
template bench(name: str, body: | |):
    val start = Time.now()
    body()
    val elapsed = Time.now() - start
    print(f"{name}: {elapsed}ms")

bench!("heavy", | |:
    heavy_computation()
)
```

**注意**：`| |` 中间有空格，表示无参函数。`||` 是二元逻辑或运算符。

#### `untyped` 参数

`untyped` 标记的参数**不进行求值**，直接把代码块原样插入：

```
// 用 untyped 实现惰性求值
template log_if(cond: bool, expr: untyped):
    if cond:
        print(f"debug: {$(expr)} = {expr}")

val debug_mode = false
log_if(debug_mode, expensive_call())  // debug_mode=false 时 expensive_call 不执行
```

#### 模板 vs 宏 vs 装饰器

| | 装饰器 | 模板 | 宏 |
|---|---|---|---|
| 时机 | 运行时 | 编译期 | 编译期 |
| 操作 | 函数值 | 代码替换 | Token 流 |
| 关键字 | `def` | `template` | `macro` |
| 复杂度 | 低 | 低 | 高 |
| 适合 | 包装函数 | 消除重复 | 生成/修改代码 |

### 12.4 自定义操作符（*fix）

自定义操作符分两步：**声明**（模块顶层）→ **实现**（trait/struct/class 内或模块内）。

#### 声明（模块顶层，抽象）

```
// prefix: 前缀操作符（一元，操作符在操作数前）
prefix `++` inc<T>(T) where T: Add -> T

// infix: 双元操作符（二元，操作符在中间）
infix `++` concat<T>(T, T) where T: Add -> T

// suffix: 后缀操作符（一元，操作符在操作数后）
suffix `!` factorial<T>(T) where T: Int -> T

// nthfix: 多元操作符（n 个符号，n+1 个操作数，n >= 2）
nthfix `..`, `^` range_step<T>(start: T, end: T, step: T) where T: Num -> T

// pairfix: 成对操作符（2 个符号包裹 1 个操作数，至少一个参数）
pairfix `《`, `》` book_name<T>(a: T) -> str
```

#### 多名称与多签名

同一个符号可以有**多个名字**，同名字也可以有**多个签名**（签名必须不同）：

```
// 同一个符号 <| 有多个名字和签名
infix `<|` name<T1, T2>(T1, T2) -> bool
infix `<|` name<T, R>(T, T) -> R
infix `<|` name1<T1, T2>(T1, T2) -> bool
infix `<|` name2<T, R>(T, T) -> R
// 以上都是合理的
```

#### 空白规则

| 操作符类型 | 空白规则 | 示例 |
|-----------|---------|------|
| `prefix` / `suffix` | 符号**紧贴**对象，不能有空格 | `@name` `#xxx` `5!` |
| `infix` / `nthfix` | 符号**必须有空格**分隔 | `a @ b` `a ^ b` `cond ? a : b` |
| `pairfix` | 符号紧贴对象 | `《三体》` |

> **注意**：`#` 和 `@` 禁止作为 prefix 声明，`:` 禁止作为 suffix 声明，这些符号已被语言内置使用。

#### 规则
- 符号用反引号 `` ` `` 包裹，长度不限，不能含空格、字母、数字
- `#` 和 `@` 禁止做 prefix，`:` 禁止做 suffix（已被语言内置使用）
- 不能重定义已有符号（如 `|>` 已定义就不能再定义）
- `name` 开头和结尾都不能有下划线
- `*fix` 定义的符号全部公开，没有私有的
- 声明必须写在模块顶层

#### 实现（trait/struct/class 内，或模块内 + extend）

```
// 方式一：在 trait/struct/class 内实现
struct Point:
    x: f64
    y: f64

    def __scale__(Self, scalar: f64) -> Point:
        Point(self.x * scalar, self.y * scalar)

    def __concat__(Self, other: Point) -> Point:
        Point(self.x + other.x, self.y + other.y)

// 方式二：模块内定义，再 extend 给目标
def __double__(x: int) -> int:
    x * 2

extend Static __double__ for int

// 方式三：trait 内定义抽象，impl 实现
trait Scalable:
    def __scale__(Self, f64) -> Self

impl Scalable for Point:
    def __scale__(Self, scalar: f64) -> Point:
        Point(self.x * scalar, self.y * scalar)
```

#### 完整示例

```
// === 模块 math_ops.vox ===

// 1. 声明操作符
prefix `++` inc<T>(T) where T: Add -> T
infix `++` concat<T>(T, T) where T: Add -> T
suffix `!` factorial<T>(T) where T: Int -> T
nthfix `..`, `^` range_step<T>(start: T, end: T, step: T) where T: Num -> T
pairfix `《`, `》` book_name<T>(a: T) -> str

// 2. 模块级实现
def __inc__(x: int) -> int:
    x + 1

def __factorial__(n: int) -> int:
    1 if n <= 1 else n * __factorial__(n - 1)

def __book_name__(a: str) -> str:
    f"《{a}》"

// 3. extend 给类型
extend Static __inc__ for int
extend Static __factorial__ for int
extend Static __book_name__ for str

// === 使用 ===
import math_ops

val a = ++5          // 6   (prefix)
val b = "a" ++ "b"   // "ab" (infix，如果 String 实现了)
val c = 5!           // 120 (suffix)
val d = 0 .. 10 ^ 2  // range_step(0, 10, 2) (nthfix)
val e = 《"三体"》    // "《三体》" (pairfix)
```

#### nthfix 详解

`nthfix` 是 n 个符号分割 n+1 个操作数：

```
// 范围步进：.. ^   →  2 符号，3 操作数
nthfix `..`, `^` range_step<T>(start: T, end: T, step: T) where T: Num -> T
// 用法: start .. end ^ step

// 带默认值的索引：  [ ]   →  2 符号，3 操作数
nthfix `[`, `]` get_or<T>(list: [T], idx: int, default: T) where T -> T
// 用法: list [ idx ] default

// 自定义区间：  .. ^   →  2 符号，3 操作数
nthfix `..`, `^` range<T>(start: T, end: T, step: T) where T: Num -> T
// 用法: 0 .. 10 ^ 2
```

> **锦上添花**：`?:` 三元运算符是 nthfix 的经典应用，可在后期实现为语法糖：
> ```
> nthfix `?`, `:` choose<T>(cond: bool, a: T, b: T) where T -> T
> // 用法: cond ? a : b
> // 等价于 a if cond else b
> ```

符号数 = n，操作数 = n+1，n >= 2。表达式呈现为：

```
a1 符号1 a2 符号2 a3 ... 符号n a_{n+1}
```

#### pairfix 详解

`pairfix` 是成对符号包裹单个操作数，固定 2 个符号、1 个操作数：

```
// 中文书名号：《 》
pairfix `《`, `》` book_name<T>(a: T) -> str
// 用法: 《"三体"》 → book_name("三体")

// 日文引号：「 」
pairfix `「`, `」` quote<T>(a: T) -> str
// 用法: 「hello」 → quote("hello")

// 方括号包裹（自定义，不同于内置 []）
pairfix `【`, `】` highlight<T>(a: T) -> str
// 用法: 【warning】 → highlight("warning")
```

`pairfix` 与 `nthfix` 的区别：
- `nthfix`：符号在操作数**之间**，`a1 s1 a2 s2 a3`
- `pairfix`：符号在操作数**两侧**，`s1 a s2`

#### 完整操作符对比

| 类型 | 符号数 | 操作数 | 表达式 | 示例 |
|------|:---:|:---:|------|------|
| `prefix` | 1 | 1 | `sym a` | `++5` |
| `infix` | 1 | 2 | `a sym b` | `a ++ b` |
| `suffix` | 1 | 1 | `a sym` | `5!` |
| `nthfix` | n | n+1 | `a1 s1 a2 s2 ... sn a_{n+1}` | `0 .. 10 ^ 2` |
| `pairfix` | 2 | 1 | `s1 a s2` | `《x》` |

### 12.5 `lazy` — 惰性求值

`lazy` 支持惰性声明变量、导入和块语句。

#### lazy val / lazy var

```
lazy val x = expensive_computation()   // 首次访问时才计算
lazy var y = heavy_init()              // 惰性可变变量
```

#### lazy import / lazy from ... import ...

```
lazy import std::heavy_module           // 首次使用模块时才加载
lazy from std::net import TcpStream     // 惰性导入指定符号
```

#### lazy block #block_name

`lazy block #block_name` 表示只在转译期将源码转译到缓存，不立即生成目标代码。`#block_name` 必须唯一。

```
lazy block #init_logger:
    // 这些代码只转译到缓存，不立即生成 Rust 代码
    val logger = Logger::new("app.log")
    logger.set_level(Level::Debug)
```

#### transtime block 复用

在可见到 `lazy block #block_name` 定义的作用域内，可通过单行语句复用缓存的转译代码：

```
def main():
    transtime block #init_logger      // 将缓存的转译代码插入此处
    logger.info("Application started")
```

#### 规则
- `lazy val` / `lazy var`：首次访问时求值，之后缓存结果
- `lazy import`：首次使用模块时加载
- `lazy block`：转译期缓存，`transtime block` 复用
- `lazy block #block_name` 的 `#block_name` 同作用域下必须唯一

### 12.6 `transtime`、`comptime`、`define` — 编译期与转译期

#### 单行语法

`comptime` 和 `define` 支持单行形式：

```
comptime: 40 + 2
define: F0<T>: || -> T
```

#### `transtime` — 转译期执行（仅模块顶层）

`transtime` 产生**构建副作用**（修改 Cargo.toml、注入依赖、生成额外文件、缓存复用等），**不产生运行时值**。

```
transtime:
    // 转译期执行，修改构建产物
    toml.dependency("serde", "1.0")
    toml.feature("serde", ["derive"])
    config.set("edition", "2021")
```

- **位置限制**：仅模块顶层，不能写在函数或局部作用域内
- **不可赋值**：`val x = transtime: ...` 是错误用法
- **不可单行**：`transtime` 不支持单行（转译期操作通常是多行配置）

```
// ❌ 错误：transtime 在函数内
def foo():
    transtime: ...   // Error

// ❌ 错误：transtime 赋值
val x = transtime: ...  // Error
```

#### `comptime` — 编译期求值（任意位置）

`comptime` 产生一个**编译期计算的值**，可放在任何需要值的位置——变量初始化、函数实参、返回值等。

```
// 模块级
val PI = comptime: 3.141592653589793

// 函数内
def area(r: f64) -> f64:
    val table = comptime:
        var t: [256]f64 = [0.0; 256]
        for i in 0..256:
            t[i] = (i as f64).sqrt()
        t
    table[r as usize] * r * PI

// 函数实参
def draw(scale: f64):
    sprite.render(comptime: calc_scale())

// 单行
val answer = comptime: 40 + 2
```

- **位置限制**：任意位置（模块级、函数内、块内、表达式内）
- **可赋值**：`val x = comptime: expr` 是合法用法
- **支持单行**：`comptime: expr`

#### 示例

```
// comptime: 编译期生成查找表
val sqrt_table = comptime:
    var table: [256]f64 = [0.0; 256]
    for i in 0..256:
        table[i] = (i as f64).sqrt()
    table

// transtime: 转译期条件配置
transtime:
    if PLATFORM == "windows":
        toml.dependency("winapi", "0.3")
    else:
        toml.dependency("libc", "0.2")

// lazy block 缓存复用
transtime:
    lazy block #init_logger  // 仅转译，不生成代码
```

#### `define` — 类型约束定义

`define` **必须放在模块顶层**，不能写在函数或局部作用域内。支持单行和多行：

```
// 单行 define
define: F0<T>: || -> T
define: Callback<T>: |T| -> bool

// 多行 define
define:
    F0<T>: || -> T                    // 类型约束（非 trait）
    trait t0:
        props = {x: f64, y: f64}
        instancemethods = {
            draw: |Self| -> None,
            area: |Self| -> f64,
        }
    enum name(t0):                    // 复用 t0 约束（须为 trait）
        case A
        case B
    trait BG(ImplicitStringable & Copyable):  // 合法 — 多约束
        ...
    BG2 = BG & Writeable              // 类型别名 — 组合约束
```

**约束复用规则**：

| 声明 | 可复用约束类型 |
|------|--------------|
| `enum X(t)` | 仅 `trait` |
| `class X(c)` | `class` 或 `trait` |
| `struct X(s)` | 仅 `struct` |
| `trait X(t)` | 仅 `trait` |

**类型约束 vs trait**：

```
define:
    F0<T>: || -> T                    // 类型约束，不是 trait
    enum name(F0):                    // ❌ 错误！F0 不是 trait
        ...

    trait t0:                         // ✅ 定义 trait
        ...
    enum name(t0):                    // ✅ 合法 — t0 是 trait
        ...
```

**约束组合（`&`）**：

```
define:
    trait BG(ImplicitStringable & Copyable):   // 多重约束
        ...
    BG2 = BG & Writeable                        // 取并集
    BG3 = BG2 & Readable & Hashable             // 继续组合
```

#### 与 Rust 的衔接

`comptime` 块不直接生成 Rust 代码——Vox 编译器在编译期执行块内代码，将其结果内联到生成的 Rust 源码中。

```
comptime:                          // Vox 编译器执行
    val table = [0; 256]           // 计算
    for i in 0..256:               // 生成
        table[i] = i * i           // 数据
                                   // ↓
val squares = table                // 生成的 Rust 代码中包含内联的数组
```

#### 三者对比

| | `transtime` | `comptime` | `define` |
|---|---|---|---|
| 执行时机 | Vox 转译期 | Rust 编译期 | 转译期（类型注册） |
| 位置限制 | **仅模块顶层** | **任意位置** | 仅模块顶层 |
| 产生值 | 否（副作用） | 是（值） | 否（类型约束） |
| 可赋值 | 否 | 是 | 否 |
| 单行支持 | 否 | 是 | 是 |
| 用途 | 构建配置、缓存复用 | 编译期计算 | 类型约束定义 |

---

### 12.7 `buildfix` — 自定义参数包构建操作符

`buildfix` 声明自定义的**参数包构建操作符**，提供三种符号形式来调用 `__demoname__` 方法。

#### 声明

```
buildfix `~:`, `【`, `】` demoname<R>(参数表) -> R
```

需要提供**三个符号**：
- `~:` — 参数包构建符号（类比 `<:`）
- `【` — 左括号（类比 `(`）
- `】` — 右括号（类比 `)`）

#### 实现

```
def __demoname__<R>(参数表) -> R:
    ...

extend Self __demoname__ for TargetType
```

#### 三种调用方式

```
// 方式 1：直接调用魔法方法
demo.__demoname__(args)

// 方式 2：括号语法
demo【args】

// 方式 3：参数包构建语法
demo ~:
    val s = ...
    ...
    (args)  // 返回的参数包传给 __demoname__
```

#### 完整示例

```
// 声明 buildfix
buildfix `~:`, `【`, `】` sum_pair<R>(a: int, b: int) -> R

// 实现
def __sum_pair__(a: int, b: int) -> int:
    a + b

extend Self __sum_pair__ for Calculator

// 使用
val calc = Calculator()
val r1 = calc.__sum_pair__(1, 2)   // 3
val r2 = calc【1, 2】               // 3
val r3 = calc ~:
    val x = 10
    val y = 20
    (x, y)                         // 3
```

#### 规则

- `buildfix` 声明必须在模块顶层
- 三个符号必须全部提供，且不能与已有操作符冲突
- `__demoname__` 实现可放在 trait/struct/class 内，或通过 `extend` 绑定
- 符号紧贴规则同 `prefix`/`suffix`/`pairfix`

---

### 12.8 `quote` — Token 构建

`quote` 关键字用于构建 `Tokens` 对象，返回类型始终为 `Tokens`。可以在任何地方使用，但最常用于宏中。

#### 语法

```
quote:
    // 多行 Token 构建
    ...
```

#### 宏中使用

```
macro name(ts: Tokens) -> Tokens:
    val a = ...
    val b = ...
    val c = quote:
        // 中间 Token 构建
        ...
    quote:
        $| ... {ts}...  {c}...
        #| ....
        &| ....
```

- `$|` 行：支持插值 `{...}`，表达式结果嵌入为 Token
- `#|` 行：支持转义，无插值
- `&|` 行：原始字符串，一切原样

#### 其他位置使用

```
// 模块级 Token 构建
val tokens = quote:
    $| def hello():
    $|     print("Hello from Vox!")
    &| // 这行原样保留

// 函数内构建 Token
def generate_code() -> Tokens:
    quote:
        $| struct Point:
        $|     x: f64
        $|     y: f64
```

#### 与宏的关系

`quote` 是构建 Token 的通用机制，宏是其主要应用场景，但 `quote` 不限于宏——任何需要构建 `Tokens` 的地方都可以使用。

---

## 13. 并发模型

### 13.1 Goroutine (`go`)

```
def worker(id: int, ch: Chan[str]):
    go:
        ch.send(f"Worker {id} started")
        ch.send(f"Worker {id} done")

def main():
    val ch = Chan::new()
    go: worker(1, ch.clone())
    go: worker(2, ch.clone())
    go: worker(3, ch.clone())

    print(ch.recv())
    print(ch.recv())
    print(ch.recv())
```

### 13.2 Channel

```
def pipeline():
    val ch1 = Chan::new()
    val ch2 = Chan::buffered(10)

    go:
        ch1.send(1)
        ch1.send(2)
        ch1.close()

    go:
        for msg in ch1:
            ch2.send(msg * 2)
        ch2.close()

    val results = ch2.collect()  // [2, 4]
```

### 13.3 Actor 模型

```
spawn Counter:
    var state = 0

    def handle(msg):
        match msg:
            | :inc   => state = state + 1; state
            | :dec   => state = state - 1; state
            | :get   => state
            | :reset => state = 0; 0

def main():
    val counter = spawn Counter
    print(counter.send(:inc))    // 1
    print(counter.send(:inc))    // 2
    print(counter.send(:get))    // 2
    print(counter.send(:reset))  // 0
```

### 13.4 async / await

```
async def fetch_data(url: str) -> Result<str, NetError>:
    val response = await http.get(url)
    if response.status != 200:
        return Err(NetError(f"HTTP {response.status}"))
    Ok(response.body)

async def fetch_all(urls: [str]) -> [str]:
    val tasks = urls.map(|url| fetch_data(url))
    val results = await gather(tasks)
    results.filter_map(|r| r.ok())
```

---

## 14. 字符串

### 14.1 单行字符串：`"..."` / `f"..."` / `r"..."` / `'...'` / `f'...'` / `r'...'`

| 前缀 | 转义 `\n` | 插值 `{...}` | 说明 |
|------|:---:|:---:|------|
| `"..."` | 是 | 否 | 普通双引号字符串 |
| `'...'` | 是 | 否 | 普通单引号字符串 |
| `f"..."` | 是 | 是 | 插值双引号字符串 |
| `f'...'` | 是 | 是 | 插值单引号字符串 |
| `r"..."` | 否 | 否 | 原始双引号字符串 |
| `r'...'` | 否 | 否 | 原始单引号字符串 |

```
val name = "Vox"
val version = 2

val s1 = "Hello, World!"              // 普通双引号字符串
val s1b = 'Hello, World!'             // 普通单引号字符串
val s2 = f"Hello, {name} v{version}"  // 插值双引号
val s2b = f'Hello, {name} v{version}' // 插值单引号
val s3 = r"C:\Users\{name}\data"      // 原始双引号
val s3b = r'C:\Users\{name}\data'     // 原始单引号
```

### 14.1.1 三引号字符串：`"""..."""` / `'''...'''` 及带前缀版本

| 前缀 | 转义 `\n` | 插值 `{...}` | 说明 |
|------|:---:|:---:|------|
| `"""..."""` | 是 | 否 | 普通三双引号 |
| `'''...'''` | 是 | 否 | 普通三单引号 |
| `f"""..."""` | 是 | 是 | 插值三双引号 |
| `f'''...'''` | 是 | 是 | 插值三单引号 |
| `r"""..."""` | 否 | 否 | 原始三双引号 |
| `r'''...'''` | 否 | 否 | 原始三单引号 |

```
val doc1 = """多行
字符串
示例"""

val doc2 = f"""Hello, {name}!
Version: {version}"""

val doc3 = r'''C:\Users\
{name}\data'''  // 原始三单引号，{name} 原样输出
```

与 Python 的关键区别：`"..."` 不加 `f` 就是**纯字符串**，`{name}` 原样输出，不会插值。

### 14.2 多行字符串 (`#|` / `$|` / `&|`)

每行以 `#|`、`$|` 或 `&|` 开头，三者可混用在同一字符串中。

| 前缀 | 转义 `\n` | 插值 `{...}` | 说明 |
|------|:---:|:---:|------|
| `#|` | 是 | 否 | 支持转义，无插值 |
| `$|` | 是 | 是 | 支持转义 + 插值 |
| `&|` | 否 | 否 | 原始字符串，一切原样 |

```
val s = 
    #|这个\n 有效，会换行，\\ 这个注释有效
    $|这个\n 有效，会换行，{val t = "hello"; t + " world"}  \\ 这个注释有效
    &|这个\n 无效，保持原样，{val t = "hello"; t + " world"}  前面花括号无效，保持原样，\\ 这个注释有效
```

实际效果（`s` 的值）：

```
这个
 有效，会换行，\
hello world
这个\n 无效，保持原样，{val t = "hello"; t + " world"}  前面花括号无效，保持原样，\
```

规则：
- `//` 行注释在三种模式下都有效
- `#|` 和 `$|` 中 `\n` `\t` `\\` 等转义生效
- `$|` 中 `{...}` 作为表达式求值，结果转为字符串嵌入
- `&|` 中一切字符原样保留，不转义、不插值

### 14.3 字符串操作

```
val s = "Hello, Vox!"

val first = s[0]             // 'H'
val slice = s[0..5]          // "Hello"
val combined = "Hello" + " " + "World"
val upper = s.to_upper()     // "HELLO, VOX!"
val len = s.len()            // 11
val has = s.contains("Vox")  // true
```

### 14.4 日期字面量 (`#...#`)

日期/时间字面量使用 `#...#` 包围：

```
val today = #2024-01-15#
val datetime = #2024-01-15 13:30:00#
val time = #13:30:00#
```

**规则**：
- `#` 和内容之间不需要空格
- 内容必须符合合法的日期/时间格式
- 编译期验证日期格式正确性
- 转译成 Rust 的 `chrono::NaiveDate` / `chrono::NaiveDateTime` / `chrono::NaiveTime`

```
Vox                            Rust
───                            ────
val d = #2024-01-15#           let d = chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
```

---

## 15. GPU 与 Tensor（Mojo 风格）

```
// 内建 Tensor
val a = Tensor::zeros([2, 3])
val b = Tensor::ones([2, 3])
val c = Tensor::rand([2, 3])

// 运算
val d = a + b
val e = a @ b.transpose()
val f = c.relu()

// GPU 执行
@gpu
def gpu_computation():
    val x = Tensor::rand([1000, 1000])
    val y = Tensor::rand([1000, 1000])
    val z = x @ y
    z

// 自动微分
val w = Tensor::variable([1.0, 2.0, 3.0])
val loss = w.sum().backward()
```

---

## 16. 事件

事件**只能在 `define` 块中声明**，不能写在 class/struct/enum/trait 内部。先定义、后使用，事件无返回值。

### 16.1 定义事件

```
define:
    event Click(args: EventArgs)
    event ValueChanged<T>(old: T, new: T)
    event DataReceived(data: bytes)
    event Closed
```

### 16.2 使用事件

事件在 `define` 中声明后，编译器自动将其注入到**所有**类型中，通过 `self.EventName` 访问：

```
define:
    event Click(args: EventArgs)
    event ValueChanged<T>(old: T, new: T)

// 编译器自动为所有类型生成事件字段：
// struct/class/enum 内部 → self.Click, self.ValueChanged 可用

class Button:
    var text: str

    def new(text: str):
        self.text = text

    def simulate_click(Self):
        self.Click.fire(EventArgs(self))   // 触发 Click 事件

class Slider:
    var value: f64

    def set_value(mut Self, v: f64):
        val old = self.value
        self.value = v
        self.ValueChanged.fire(old, v)      // 触发泛型事件

def main():
    val btn = Button("Save")

    btn.Click.subscribe(|args: EventArgs|:
        print("Button clicked!")
    )

    val slider = Slider(0.0)
    slider.ValueChanged.subscribe(|old: f64, new: f64|:
        print(f"Slider: {old} -> {new}")
    )

    btn.simulate_click()
    slider.set_value(5.0)
```

### 16.3 事件 API

| 方法 | 说明 |
|------|------|
| `event.fire(args)` | 触发事件，通知所有订阅者 |
| `event.subscribe(handler)` | 订阅事件，返回取消订阅的句柄 |
| `event.unsubscribe(handle)` | 取消订阅 |

### 16.4 转译到 Rust

事件转译成 Rust 的回调列表：

```rust
// Vox: event Click(args: EventArgs)
// Rust:
struct ClickEvent {
    handlers: Vec<Box<dyn Fn(&EventArgs)>>,
}
impl ClickEvent {
    fn fire(&self, args: &EventArgs) { /* 调用所有 handler */ }
    fn subscribe(&mut self, f: impl Fn(&EventArgs) + 'static) -> usize { /* ... */ }
    fn unsubscribe(&mut self, id: usize) { /* ... */ }
}
```

### 16.5 设计理由

| 决策 | 理由 |
|------|------|
| 只能在 `define` 中声明 | 事件是独立类型，不属于任何特定 class/struct |
| 无返回值 | 事件是通知机制，不产生结果 |
| 支持泛型 | `ValueChanged<T>` 可复用于不同类型 |
| `fire` / `subscribe` 模式 | 对标 C# / VB.NET 风格 |

---

## 17. enum（枚举）

枚举是带标签的代数数据类型，每个变体可携带数据。支持泛型、方法、trait 实现和 trait 组合。

### 17.1 定义

```
// 泛型枚举
enum Option<T>:
    Some(T)
    None

enum Result<T, E>:
    Ok(T)
    Err(E)

enum Either<L, R>:
    Left(L)
    Right(R)

// 带数据的枚举
enum Color:
    Red(u8, u8, u8)
    Green(u8, u8, u8)
    Blue(u8, u8, u8)

// 简单枚举（无数据）
enum Status:
    Active
    Inactive
    Banned
```

### 17.1.1 Trait 组合

`enum` 支持 trait 组合语法 `enum Name<Generic>(trait1, trait2, ...)`，来自 trait 的抽象方法必须在 enum 内实现：

```
enum Order(Display, Clone):
    Pending
    Shipped(str)
    Delivered

    // Display trait 抽象方法必须实现
    def display(Self) -> str:
        match self:
            | Pending     => "Pending"
            | Shipped(id) => f"Shipped({id})"
            | Delivered   => "Delivered"

    // Clone trait 抽象方法必须实现
    def clone(Self) -> Self:
        match self:
        | Pending     => Order.Pending
        | Shipped(id) => Order.Shipped(id)
        | Delivered   => Order.Delivered
```

> **注意**：enum 内部没有抽象方法——所有来自 trait 的抽象方法必须在 enum 内实现。

转译成 Rust 的 `enum`：

```rust
enum Option<T> { Some(T), None }
enum Result<T, E> { Ok(T), Err(E) }
enum Color { Red(u8, u8, u8), Green(u8, u8, u8), Blue(u8, u8, u8) }
enum Status { Active, Inactive, Banned }
```

### 17.2 构造与匹配

```
val x = Some(42)
val y: Option<int> = None

def describe(opt: Option<int>) -> str:
    match opt:
    | Some(v) => f"Got {v}"
    | None    => "Nothing"

def to_rgb(c: Color) -> (u8, u8, u8):
    match c:
    | Red(r, g, b)   => (r, g, b)
    | Green(r, g, b) => (r, g, b)
    | Blue(r, g, b)  => (r, g, b)
```

### 17.3 方法

枚举可以定义方法，直接写在枚举体内：

```
enum HttpStatus:
    Ok(u16)
    NotFound(str)
    ServerError(u16, str)

    def code(Self) -> u16:
        match self:
        | Ok(c)             => c
        | NotFound(_)       => 404
        | ServerError(c, _) => c

    def is_success(Self) -> bool:
        match self:
        | Ok(_) => true
        | _     => false

    def message(Self) -> str:
        match self:
        | Ok(_)               => "OK"
        | NotFound(msg)       => msg
        | ServerError(_, msg) => msg
```

转译成 Rust 的 `enum` + `impl` 块：

```rust
enum HttpStatus {
    Ok(u16),
    NotFound(String),
    ServerError(u16, String),
}

impl HttpStatus {
    fn code(&self) -> u16 { /* match self */ }
    fn is_success(&self) -> bool { /* match self */ }
    fn message(&self) -> String { /* match self */ }
}
```

### 17.4 实现 trait

```
enum Order:
    Pending
    Shipped(str)     // tracking_id
    Delivered

impl Display for Order:
    def display(Self) -> str:
        match self:
        | Pending          => "Pending"
        | Shipped(id)      => f"Shipped ({id})"
        | Delivered        => "Delivered"
```

### 17.5 与 struct/class 的对比

| | struct | enum | class |
|---|---|---|---|
| 字段 | 固定、不可变 | 变体各自携带数据 | 固定、可变 |
| 继承 | 无 | 无 | 单继承 |
| 方法 | 支持 | 支持 | 支持 |
| trait 实现 | 支持 | 支持 | 支持 |
| 适用 | 数据聚合 | 多种可能性 | 可变状态 + 继承 |

---

## 18. 包与模块

### 18.1 可见性规则

**无 `pub` 关键字**。不是下划线开头的，全部公开。双下划线开头结尾的魔法方法也是公开的。

```
// 公开：不以 _ 开头
def factorial(n: int) -> int:
    ...

// 私有：以 _ 开头
def _internal(n: int) -> int:
    ...

// 公开：魔法方法
def __add__(Self, other: Point) -> Point:
    ...
```

**`define` 约束的可见性传染**：公开函数的签名中不能出现 `_` 前缀的私有 `define` 约束，否则编译报错。这与 Rust 的"公开类型不能暴露私有类型"规则一致。

```
define: _Sortable<T>: { props = {cmp: (T, T) -> int} }

// 编译错误 — 公开签名暴露了私有约束
def sort(data: [_Sortable]): [_Sortable]: ...   // ❌

// OK — 私有函数无此限制
def _sort(data: [_Sortable]): [_Sortable]: ...  // ✓
```

### 18.2 包管理（类 Python）

**无 `package` 关键字**。包含 `__init__.vox` 文件的文件夹即为一个包，与 Python 一致。

```
// 目录结构
myapp/
  __init__.vox              // 标记 myapp 为包（可为空文件）
  main.vox                  // 入口模块
  utils/
    __init__.vox            // 标记 utils 为子包
    math.vox
    strings.vox
  models/
    __init__.vox
    user.vox

// 导入（使用 . 分隔路径）
import utils.math                        // 导入整个模块
import utils.math::{add, sub}            // 导入指定符号
import utils.strings as strutil          // 导入并别名
from utils.math import add               // 从模块导入单个符号

// 命名空间访问（使用 :: 分隔）
val result = utils.math::add(1, 2)
val s = strutil::trim("  hello  ")
```

#### 与 Rust 的衔接

Vox 编译器扫描 `__init__.vox` 确定包结构，转译时生成 Rust 的 `mod` 层级：

```
Vox 文件结构                          Rust 生成的模块结构
───                                  ───
myapp/                               // crate root
  __init__.vox           →           // lib.rs (crate root)
  main.vox               →           // main.rs (如果有 main 入口)
  utils/
    __init__.vox         →           mod utils { }  (utils/mod.rs)
    math.vox             →           mod utils { pub mod math; }
  models/
    __init__.vox         →           mod models { }
    user.vox             →           mod models { pub mod user; }
```

转译规则：
1. `__init__.vox` → 目录对应的 `mod` 声明
2. `import utils.math` → `use crate::utils::math;`
3. `utils.math::add(1, 2)` → `utils::math::add(1, 2)`（`::` 直接映射）

#### 外部依赖声明（`toml`）

`toml` 关键字用于在 Vox 源文件中内联声明项目元数据和外部包依赖，编译时生成 `Cargo.toml`。

```
// 在 __init__.vox 或 main.vox 中声明
toml:
    [dependencies]
    numpy = "1.24"
    pandas = "2.0"
    requests = ">=2.28, <3.0"
    
    [dev-dependencies]
    pytest = "7.0"
    
    [vox]
    version = "0.1.0"
    target = "rust"
    edition = "2024"
```

| 节 | 说明 |
|----|------|
| `[dependencies]` | 运行时依赖，映射到 `Cargo.toml` 的 `[dependencies]` |
| `[dev-dependencies]` | 仅测试时依赖 |
| `[vox]` | Vox 编译器元数据（版本、目标语言、edition） |

**设计原则**：
- 一个项目只需一个 `toml` 块（通常在 `__init__.vox`）
- 语法与 TOML 格式一致，编译器直接解析并合并到生成的 `Cargo.toml`
- 支持语义化版本约束（`>=`, `<`, `^`, `~`）
- 多目标语言时，`[vox].target` 可指定为 `rust` / `nim` / `python` 等

---

### 18.3 外部语言互操作（`external`）

`external` 声明外部语言的函数接口。标准库仅实现 `py`（Python 桥接），用户可通过 `template`/`macro` 自行扩展其他语言。

#### 语法

```
// 声明 Python 外部函数（标准库提供）
external "py":
    def numpy_array(data: [f64]): PyObject
    def pandas_read_csv(path: str): PyObject

// 声明 Nim 外部函数（用户自定义）
external "nim":
    def fast_sort(data: [int]): [int]
    def sha256_hash(input: str): str
```

#### 设计理念

| 原则 | 说明 |
|------|------|
| 声明即契约 | `external` 块声明外部函数的签名，编译器做类型检查 |
| 标准库仅实现 py | Python 桥接作为一等公民内置（基于 PyO3） |
| 用户可扩展 | 其他语言通过 `template`/`macro` 生成桥接代码 |
| 与 `define` 配合 | `define` 可约束外部语言模块的结构 |

#### 转译示例

```
// Vox
external "py":
    def np_array(data: [f64]): PyObject

val arr = np_array([1.0, 2.0, 3.0])
```

```
// → Rust (使用 PyO3)
use pyo3::prelude::*;
let arr = py_ffi::np_array(vec![1.0_f64, 2.0, 3.0])?;
```

#### 用户自定义外部语言

```
// 用户使用 template 实现 nim 桥接
template nim_bridge(name: str, lib: str):
    external "__language__":
        def __name__(__args__): __ret__

// 使用模板声明 Nim 函数
nim_bridge("fast_sort", "libsort.so"):
    def fast_sort(data: [int]): [int]

// 直接调用，如同本地函数
val sorted = fast_sort([3, 1, 2])
// → 编译时展开为 Nim 的 FFI 调用代码
```

#### `external` 与 `extern` 保留字的区别

| 关键字 | 层级 | 用途 |
|--------|------|------|
| `external`（关键字） | 高层 | 语言互操作，通过宏/模板生成桥接代码 |
| `extern`（保留字） | 低层 | 预留给未来可能的 C ABI 直接调用 |

#### 与宏系统的协作

`external` 块本质上是一个**声明式契约**，实际代码生成由宏系统完成：

```
// 标准库中 py 外部语言的实现（简化）
template py_external(spec: untyped):
    // 为每个声明的函数生成 PyO3 桥接代码
    for func in spec.functions:
        generate_pyo3_wrapper(func)

// 用户声明的 external "py" 触发此模板
```

**为什么标准库只实现 py**：
- Python 是数据科学/ML 生态的事实标准
- 其他语言桥接需求多样，无法穷举
- 宏/模板系统提供了足够的扩展能力，用户自行实现 `external "nim"` / `external "julia"` 等

---

## 19. 测试

### 19.0 语法

`test` 关键字后跟测试名称，以 `:` 结尾。测试名称支持**两种写法**：

**引号形式**：
```
test "add works" :
    assert add(1, 2) == 3
```

**无引号形式**（自由文本，直到本行最后一个 `:` 为止）：
```
test add works :
    assert add(1, 2) == 3
```

两种形式**完全等价**。无引号形式中，测试名称 = `test` 之后、本行最后一个 `:` 之前的所有内容（trim 后）。

**`:` 规则**：取**本行注释前最后的 `:`**。`//` 注释及其后的 `:` 不参与匹配。

```
test 名称包含 ：冒号 ： // 这里的冒号不行 被注释了 //
//          ↑ 这个 ：是终止符   ↑ 注释内的 ：被忽略
```

**错误示例**：
```
test 没有冒号                  // ❌ 编译错误：缺少终止符 `:`
```

### 19.1 示例

```
// 单个测试
test factorial should work :
    assert factorial(0) == 1
    assert factorial(5) == 120
    assert factorial(10) == 3628800

test edge cases :
    assert throws(|: factorial(-1))

// 测试套件（suite 用于分组）
suite "math tests":
    test add :
        assert add(1, 2) == 3
    test sub :
        assert sub(5, 3) == 2

suite "string tests":
    test trim :
        assert trim("  hi  ") == "hi"
    test split :
        assert split("a,b,c", ",") == ["a", "b", "c"]
```

### 19.2 测试断言

| 断言 | 说明 |
|------|------|
| `assert expr` | expr 为 true |
| `assert expr == expected` | 相等比较 |
| `assert throws(lambda)` | lambda 抛出异常 |
| `assert not throws(lambda)` | lambda 不抛异常 |

### 19.3 与 Rust 的衔接

`test` 和 `suite` 转译成 Rust 的 `#[test]` 和 `mod tests`：

```
// Vox                         // Rust
test add works :               #[test]
    assert add(1, 2) == 3      fn add_works() { assert_eq!(add(1, 2), 3); }

suite "math tests":            #[cfg(test)]
    test add : ...             mod math_tests {
    test sub : ...                 #[test] fn add() { ... }
                               }
```

### 19.4 词法规则

`test` 关键字触发**特殊词法模式**：
1. 跳过 `test` 后的空白
2. 若下一个字符是 `"`，进入**引号模式**：读取 `"..."` 字符串作为测试名称
3. 否则进入**自由文本模式**：从 `test` 后到本行最后一个 `:` 之前的所有字符为测试名称（trim 后去除首尾空白）
4. `//` 注释及其后的内容不参与 `:` 的查找
5. 行末必须有 `:` 作为终止符，否则编译错误

```
词法分解示例（│ 为边界）：
test │ add works │ : │ // comment
  ↑        ↑       ↑       ↑
 关键字   测试名称  终止符   注释（忽略）
```

---

## 20. 完整示例

### 20.1 斐波那契

```
// 递归
def fib(n: int) -> int:
    match n:
    | 0 => 0
    | 1 => 1
    | _ => fib(n - 1) + fib(n - 2)

// 尾递归
def fib_tail(n: int) -> int:
    _fib(n, 0, 1)

def _fib(n: int, a: int, b: int) -> int:
    match n:
    | 0 => a
    | _ => _fib(n - 1, b, a + b)

// 循环
def fib_loop(n: int) -> int:
    if n <= 1:
        return n
    var a = 0
    var b = 1
    for _ in 2..=n:
        val next = a + b
        a = b
        b = next
    b

// 惰性序列
def fib_seq() -> [int]:
    yield 0
    yield 1
    yield from _fib_step(0, 1)

def _fib_step(a: int, b: int) -> [int]:
    yield a + b
    yield from _fib_step(b, a + b)
```

### 20.2 Web 服务器

```
import std::net::{TcpListener, TcpStream}
import std::io::{read_to_string, write}

def handle_client(stream: TcpStream):
    defer: stream.close()
    val request = read_to_string(stream)
    val response = $|
        HTTP/1.1 200 OK
        Content-Type: text/plain

        Hello, Vox!
        {request}
        $|
    write(stream, response)

async def main():
    val listener = TcpListener::bind("0.0.0.0:8080")?
    print("Server on port 8080")
    for conn in listener.incoming():
        go: handle_client(conn?)
```

### 20.3 机器学习流水线

```
import std::ml::{Tensor, Model, Optimizer, Dataset}
import std::ml::layers::{Linear, Conv2d}

struct CNN(Model):
    conv1: Conv2d
    conv2: Conv2d
    fc: Linear

    def new():
        self.conv1 = Conv2d(1, 32, 3)
        self.conv2 = Conv2d(32, 64, 3)
        self.fc = Linear(64 * 7 * 7, 10)

    def forward(Self, x: Tensor) -> Tensor:
        x
        |> self.conv1.forward
        |> Tensor.relu
        |> self.conv2.forward
        |> Tensor.relu
        |> Tensor.flatten
        |> self.fc.forward

@gpu
def train(model: mut CNN, dataset: Dataset, epochs: int):
    val optimizer = Optimizer::adam(model.parameters(), lr=0.001)

    def train_epoch(epoch: int):
        val (data, labels) = dataset.next_batch(32)
        val pred = model.forward(data)
        val loss = Tensor.cross_entropy(pred, labels)
        optimizer.zero_grad()
        loss.backward()
        optimizer.step()
        loss

    val losses = (0..epochs).iter()
        .map(|e| train_epoch(e))
        .collect()
    losses
```

---

## 21. 关键字汇总

| 类别 | 关键字 |
|------|--------|
| 声明 | `def` `struct` `enum` `trait` `class` `val` `var` `const` `type` `define` `event` `buildfix` |
| 导入 | `import` `from` `as` |
| 外部 | `external` `toml` |
| 控制流 | `if` `elif` `else` `otherwise` `match` `case` `when` `then` `of` `where` `loop` `while` `for` `block` `do` `until` |
| 异常 | `try` `catch` `finally` `raise` `raises` |
| 并发 | `go` `async` `await` `spawn` |
| 迭代 | `yield` `in` |
| 模块 | `impl` `extend` `omit` `Self` `Type` `Enum` `Static` |
| 宏 | `macro` `template` `untyped` `quote` |
| 操作符 | `prefix` `infix` `suffix` `nthfix` `pairfix` `buildfix` |
| 类型约束 | `Include` |
| 编译期 | `comptime` `transtime` `lazy` |
| 其他 | `return` `break` `continue` `defer` `guard` `assert` `test` `suite` `owned` `mut` `with` `super` `ignore` |
| 字面量 | `True` `False` `None` |
| 逻辑 | `and` `or` `not` `is` |
| 特殊 | `...`（占位符） `_`（匿名参数占位） |

### 21.1 完整关键字列表（按字母排序）

```
and         as          assert      async       await
block       break       buildfix    case        catch       class
comptime    const       continue    defer       def
define      do          elif        else        enum        Enum
event       extend      external    False       finally
for         from        go          guard       if
ignore      impl        import      in          Include     infix
is          lazy        loop        macro       match       mut
not         None        nthfix      of          omit        otherwise
owned       pairfix     prefix      quote       raise       raises
return      Self        spawn       Static      struct
suffix      suite       super       template    test        then
toml        trait       transtime   True        try
Type        untyped     until       val         var
when        where       while       with        yield
```

### 21.2 保留字（预留给未来使用）

```
abstract    base        end         extern      internal    override
pass        protected   sealed      unsafe
```

### 21.3 魔法方法前缀

```
__init__    __new__     __del__     __copy__    __move__
__eq__      __ne__      __hash__    __str__     __repr__
__bool__    __int__     __float__   __call__
__add__     __sub__     __mul__     __div__     __mod__     __pow__
__neg__     __pos__     __abs__     __invert__
__iadd__    __isub__    __imul__    __idiv__
__getitem__ __setitem__ __delitem__ __len__     __contains__
__iter__    __next__
__enter__   __exit__
__getattr__ __setattr__ __delattr__
__check__
```

---

## 22. 操作符规范

### 22.1 字符规则

#### 22.1.1 允许字符及权重

以下 ASCII 可见标点符号可参与操作符构建，每个字符有预设权重分：

| 字符 | 权重 | 依据 |
|:---:|:----:|------|
| `.` | 10.0 | 成员访问，绑定最紧 |
| `?` | 9.0 | 安全导航、空合并 |
| `:` | 8.5 | 类型注解、作用域 |
| `@` `[` `]` | 8.0 | 装饰、括取、buildfix |
| `#` | 7.5 | 特殊标记 |
| `$` | 7.0 | 插值标记 |
| `_` | 6.5 | 占位 |
| `*` `/` `%` | 6.0 | 乘除模 |
| `+` `-` | 5.0 | 加减 |
| `!` `=` `<` `>` | 4.0 | 比较、等值、否定 |
| `&` | 3.5 | 逻辑与、归约 |
| `^` `;` | 3.0 | 所有权、异或 |
| `\|` | 2.5 | 管道、逻辑或 |
| `~` | 2.0 | 组合 |
| `\` | 1.5 | 转义 |

#### 22.1.2 禁止字符

以下字符**不能**参与自定义操作符：

| 字符 | 原因 |
|:---:|------|
| `,` | 参数分隔符 |
| `'` `"` | 字符串界定符 |
| `(` `)` | 分组、调用括号 |

> 中文字符 `【` `】` 仅用于 `buildfix`，不能用于其他操作符。

---

### 22.2 *fix 类型体系

#### 22.2.1 统一声明语法

```vox
*fix[l|r|n] [n级] `符号列表` name<泛型R>(参数列表) -> R
```

| 部分 | 说明 |
|------|------|
| `*fix` | `prefix` / `suffix` / `infix` / `nthfix` / `pairfix` / `buildfix` |
| `l\|r\|n` | `l`=左结合 `r`=右结合 `n`=不可结合（仅 `infix`/`nthfix` 需要） |
| `n级` | 可选，优先级数值（字面量或 `const`） |
| `符号列表` | 反引号括起的操作符符号 |

#### 22.2.2 类型详解

| 类型 | 参数 | 结合性 | 示例 |
|------|:---:|:---:|------|
| `prefix` | 1 | 无 | `-x` `!x` `++x` |
| `suffix` | 1 | 无 | `x^` `x++` `x!` |
| `infix` | 2 | l/r/n | `a + b` `a \|> f` |
| `nthfix` | 3+ | l/r/n | `a ? b : c` `a ?? b1 : b2 : b3` |
| `pairfix` | 2段 | 无 | `if/else` `while/do` |
| `buildfix` | 1段 | 无 | `obj ~: body` `obj【arg】` |

#### 22.2.3 *fix 类型权重偏移

| 类型 | 偏移 | 说明 |
|------|:---:|------|
| `prefix` | +2.0 | 前缀最紧 |
| `suffix` | +1.0 | 后缀次之 |
| `infix` | 0.0 | 二元基准 |
| `nthfix` | -0.5 | 多元稍松 |
| `pairfix` | -1.0 | 配对更松 |
| `buildfix` | -1.5 | 构建最松 |

---

### 22.3 优先级计算

#### 22.3.1 公式

```
precedence = first_char_weight + fix_type_offset + (len - 1) × 0.05
```

- `first_char_weight`：操作符**第一个字符**的权重
- `fix_type_offset`：*fix 类型偏移
- `len`：操作符字符个数
- 镜像操作符（如 `<|` 和 `|>`）首字符不同导致优先级不同时，标准库应**显式声明**优先级

#### 22.3.2 结合性默认规则

未声明 `l|r|n` 时，按首字符自动判断：

| 首字符 | 默认结合性 |
|:---:|:---:|
| `\|` `~` `&` `+` `-` `*` `/` `%` `@` `.` `:` `?` `[` `^` `#` `$` `\` | **左结合** |
| `=` `!` `<` `>` | **不可结合** |

---

### 22.4 内置操作符总表

#### 22.4.1 按优先级排序

```
优先级   操作符          类型      结合性
──────────────────────────────────────────
10.00    .               infixn    l
 9.05    ?.  ??          infixl    l
 8.55    ::              infixl    l
 8.05    @:              infixl    l
 7.05    ++  --          prefix    ─
 7.00    -  !  ~         prefix    ─
 6.50    【  】           buildfix  ─
 6.05    ++  --          suffix    ─
 6.00    *  /  %         infixl    l
 5.05    ! (suffix)      suffix    ─
 5.00    +  -            infixl    l
 4.05    ==  !=  <=  >=  infixn    n
 4.00    <  >            infixn    n
 3.55    &&              infixl    l
 3.05    ^:              infixl    l
 3.00    ^  ;            infixl    l
 2.60    |>>  |?>  |*>   infixl    l
         |&>  |@>
 2.55    ||  |>          infixl    l
 2.05    ~>              infixl    l
 0.55    ~:              buildfix  ─
```

#### 22.4.2 标准库镜像操作符（需显式声明优先级）

```
优先级   操作符   镜像于   声明
────────────────────────────────────────────────
 2.55    <|       |>       infixl 2.55 `<|`
 2.60    <<|      |>>      infixl 2.60 `<<|`
 2.60    <?|      |?>      infixl 2.60 `<?|`
 2.60    <*|      |*>      infixl 2.60 `<*|`
 2.60    <&|      |&>      infixl 2.60 `<&|`
 2.60    <@|      |@>      infixl 2.60 `<@|`
 2.05    <~       ~>       infixl 2.05 `<~`
```

---

### 22.5 列表推导式

Haskell 风格，`for`/`if` 子句组合。

#### 基本语法

```vox
[表达式 for 模式 in 可迭代对象 if 条件, ...]
```

#### 示例

```vox
// 基本推导
val squares = [x * x for x in [1, 2, 3, 4, 5]]          // [1, 4, 9, 16, 25]

// 带过滤
val evens = [x for x in [1..10] if x % 2 == 0]          // [2, 4, 6, 8, 10]

// 多重生成器（笛卡尔积）
val pairs = [(x, y) for x in [1, 2, 3] for y in ['a', 'b']]
// [(1, 'a'), (1, 'b'), (2, 'a'), (2, 'b'), (3, 'a'), (3, 'b')]

// 依赖生成器（后面的依赖前面的）
val triples = [(x, y, z) for x in [1..3] for y in [x..3] for z in [y..3]]
// [(1,1,1), (1,1,2), (1,1,3), (1,2,2), (1,2,3), (1,3,3), (2,2,2), ...]

// 多重过滤
val result = [x * y for x in [1..10] if x > 3 for y in [1..5] if y != x]

// 字典推导
val dict = {: "k" + str(x): x * 2 for x in [1, 2, 3]}
// {"k1": 2, "k2": 4, "k3": 6}

// 集合推导
val set = {x % 3 for x in [1..10]}
// {0, 1, 2}
```

#### 推导式中的模式匹配

```vox
// 解构推导
val keys = [k for (k, v) in [("a", 1), ("b", 2)] if v > 1]  // ["b"]

// of 模式
val tagged = [x for x of Some(v) in [Some(1), None, Some(2)]]  // [1, 2]
```

---

### 22.6 并行操作符

以 `||` 为前缀，与管道操作符组合，表示**数据并行**操作（多线程/多进程）。

#### 并行操作符表

| 操作符 | 名称 | 说明 |
|:---:|------|------|
| `\|\|>` | par_pipe | 并行传入函数 |
| `\|\|>>` | par_map | 并行 map |
| `\|\|?>` | par_filter | 并行 filter |
| `\|\|*>` | par_flat_map | 并行 flatMap |
| `\|\|&>` | par_reduce | 并行 reduce |
| `\|\|@>` | par_fold | 并行 fold |
| `\|\|\|` | par_join | 等待所有并行任务完成 |

#### 示例

```vox
// 并行 map — 每个元素在不同线程中处理
val results = large_list ||>> |x| => heavy_compute(x)

// 并行 pipe — 将整个数据分片后并行处理
val results = large_list ||> process_chunk

// 并行 filter + 并行 map 链式
val results = large_list
    ||?> |x| => x.is_valid()
    ||>> |x| => x.transform()

// 并行 reduce — 分片归约后合并
val total = large_list ||&> |a, b| => a + b

// 显式等待
val results = large_list ||>> |x| => async_work(x)
    |||                            // 等待所有完成
```

#### 并行策略

```vox
// 默认：自动检测 CPU 核心数
val r = data ||>> heavy_work

// 显式指定线程数
val r = data ||>> (threads: 4) heavy_work

// 使用进程池（适合 CPU 密集）
val r = data ||>> (process: 8) heavy_work

// 使用 GPU
val r = data ||>> (gpu) heavy_work
```

#### 权重

并行操作符以 `||` 开头，视为**双字符前缀** `|` 权重 2.5 + `||` 权重 2.55：

```
优先级   操作符          类型
──────────────────────────────
 2.65    ||>>  ||?>  ||*>  ||&>  ||@>   infixl
 2.60    ||>               infixl
 2.55    |||               infixl
```

---

## 23. 转译引擎

```
vox source.vox    →    source.rs    →    rustc source.rs    →    binary
```

架构：

```
vox/
├── src/
│   ├── lexer.rs       # 缩进敏感词法分析
│   ├── parser.rs      # 生成 AST
│   ├── ast.rs         # AST 定义
│   ├── codegen.rs     # AST → Rust 源码
│   └── main.rs        # CLI 入口
└── Cargo.toml
```

---

### 23.1 Rust 转译策略

#### 23.1.1 类型作为一等值的转译

Vox 类型引用 → Rust 生成的 `Type` 枚举：

```rust
// 编译器自动生成（根据所有用户定义的类型）
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    Struct1,
    Struct2,
    Class1,
    Class2,
    Enum1,
    Enum2,
    Enum3,
    TraitCopyable,
    TraitDisplay,
    // ... 每个用户定义的类型一个 variant
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeCategory {
    Struct,
    Class,
    Enum,
    Trait,
}

impl Type {
    pub fn category(&self) -> TypeCategory {
        match self {
            Type::Struct1 | Type::Struct2 => TypeCategory::Struct,
            Type::Class1 | Type::Class2 => TypeCategory::Class,
            Type::Enum1 | Type::Enum2 | Type::Enum3 => TypeCategory::Enum,
            Type::TraitCopyable | Type::TraitDisplay => TypeCategory::Trait,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Type::Struct1 => "Struct1",
            Type::Class2 => "Class2",
            // ...
        }
    }
}
```

**Vox → Rust 对照表**：

| Vox | Rust |
|-----|------|
| `val t = Enum3` | `let t = Type::Enum3;` |
| `[Struct1, Class2, Enum3]` | `vec![Type::Struct1, Type::Class2, Type::Enum3]` |
| `type(t)` | `t.category()` |
| `t.name()` | `t.name()` |

#### 23.1.2 Atom 字面量的转译

Atom 编译为 `u64` 常量（编译期 SipHash 哈希）：

```
// Vox                              // Rust 生成
val a = :inc                        const A: u64 = 0x1a2b3c4d5e6f7890_u64;
val b = :inc                        // 同一 Atom 哈希相同，复用常量
```

#### 23.1.3 条件表达式的转译

```
// 简单三元
val g = score ? 过 : 卡             let g = if score { "过" } else { "卡" };

// 链式条件
val g = score ? _ >= 90 -> A        let g = if score >= 90 { "A" }
                _ >= 80 -> B               else if score >= 80 { "B" }
                _ >= 60 -> C               else if score >= 60 { "C" }
                ! F                        else { "F" };

// 构建式
val g: int =? _ >= 90 -> A          let g = (|| {
                _ >= 80 -> B            if score >= 90 { return "A"; }
                _ >= 60 -> C            if score >= 80 { return "B"; }
                ! F                     if score >= 60 { return "C"; }
                                        "F"
                                    })();
```

#### 23.1.4 `case` 的转译

```
// Vox                              // Rust
case x when 1 then 3               match x {
    when 2 then 4                       1 => 3,
    else 0                              2 => 4,
                                        _ => 0,
                                    };
```
`case` 块由缩进回退闭合，无需 `end`。多行时 `when`/`then`/`else` 分支缩进在 `case` 下，单行可直接写在一行。

#### 23.1.5 `*args` 可变参数的转译

```
// Vox                              // Rust
def sum(*args: int) -> int:         fn sum(args: &[i32]) -> i32 {
    args.fold(0, |a, b| => a + b)       args.iter().fold(0, |a, b| a + b)
                                    }

sum(1, 2, 3)                        sum(&[1, 2, 3])
```

#### 23.1.6 列表推导式的转译

```
// 基本推导
[x * x for x in [1, 2, 3]]         [1, 2, 3].iter().map(|x| x * x).collect()

// 带过滤
[x for x in [1..10] if x % 2 == 0] (1..=10).filter(|x| x % 2 == 0).collect()

// 多重生成器（笛卡尔积）
[(x, y) for x in [1, 2]            (1..=2).flat_map(|x|
    for y in ['a', 'b']]                ['a', 'b'].iter().map(move |&y|
                                        (x, y))).collect()

// 字典推导
{: "k" + str(x): x * 2             (1..=3).map(|x|
    for x in [1, 2, 3]}                 (format!("k{}", x), x * 2))
                                        .collect()
```

#### 23.1.7 并行操作符的转译

```
// Vox                              // Rust (使用 rayon)
data ||>> |x| => heavy(x)           data.par_iter().map(|x| heavy(x)).collect()

data ||?> |x| => x.is_valid()       data.par_iter().filter(|x| x.is_valid()).collect()
    ||>> |x| => x.transform()           .map(|x| x.transform()).collect()
```

`||` 前缀 → 使用 `rayon` 的 `.par_iter()` 并行迭代器。

#### 23.1.8 `spawn` Actor 的转译

```
// Vox                              // Rust
spawn Counter:                      let counter = tokio::spawn(async move {
    loop:                               loop {
        match recv():                       match rx.recv().await {
            | :inc => cnt += 1                 Some(:inc) => cnt += 1,
            | :dec => cnt -= 1                 Some(:dec) => cnt -= 1,
            | :reset => cnt = 0                Some(:reset) => cnt = 0,
                                        }
                                    });
```

Actor 转译为 `tokio::spawn` + channel 通信。Atom 消息 `:inc` 是 `u64` 哈希。

---

## 24. 符号多义性分析

Vox 中部分符号承担多种职责，以下逐一分析是否存在歧义。

### 24.1 `:` — 冒号（职责最多）

| 职责 | 示例 | 上下文 | 歧义风险 |
|------|------|------|:---:|
| 语句块开始 | `if x > 0:` | 关键字后 | 无 |
| 类型注解 | `x: int` | 变量/参数声明中 | 无 |
| 返回类型 | `-> int:` | `->` 后 | 无 |
| 字典键值 | `{: "a": 1}` | 字典字面量内 | 无 |
| 结构体字段 | `props = {x: f64}` | `define` 内 `{}` 块 | 低 |

**结论**：`:` 的所有职责都可通过上下文明确区分，无需消歧义。`{}` 为空集合，`{:` 开头为字典。

### 24.2 `::` — 双冒号

| 职责 | 示例 | 上下文 |
|------|------|------|
| 模块路径 | `utils::math::add` | `import` / 调用路径 |
| 命名空间访问 | `String::from("hi")` | 类型后 |

**无歧义**，始终表示路径分隔，与 Rust 一致。

### 24.3 `?` — 问号

| 职责 | 示例 | 上下文 | 歧义风险 |
|------|------|------|:---:|
| Option 类型 | `int?` | 类型位置 | 无 |
| 安全导航 | `obj?.field` | 表达式 | 无 |
| null 合并 | `a ?? b` | 双写 `??` | 无 |

**结论**：`?` 三重职责均无歧义——`T?` 是类型级别，`?.` 和 `??` 是表达式级别，且符号组合不同。

### 24.4 `|` — 竖线

| 职责 | 示例 | 上下文 | 歧义风险 |
|------|------|------|:---:|
| match 分支分隔 | `match x: \| A => \| B =>` | `match` 内 | 无 |
| 按位或 | `a \| b` | 表达式 | 无 |
| 管道 | `a \|> f()` | 后跟 `>` | 无 |
| lambda 参数 | `\|x\| -> T:` | 成对 `\|` 包围 | 无 |
| 逻辑或 | `a \|\| b` | 双写 `\|\|` | 无 |

**结论**：`|` 职责最多，但每种都有明确的上下文或符号组合区分。`|` 和 `||` 是不同符号，`|>` 是三字符，`|x|` 是成对包围。

### 24.5 `_` — 下划线

| 职责 | 示例 | 上下文 | 歧义风险 |
|------|------|------|:---:|
| 匿名参数占位 | `lst.map(_ * 2)` | 函数调用参数 | 无 |
| 私有前缀 | `_helper()` | 标识符开头 | 无 |
| 丢弃模式 | `match x: \| Some(_) =>` | `match` 模式 | 无 |
| 忽略循环变量 | `for _ in 0..10:` | `for` 循环 | 无 |

**结论**：`_` 四种职责——作为独立 token 时是占位/丢弃，作为前缀时是私有标记，上下文清晰。

### 24.6 `#` — 井号

| 职责 | 示例 | 上下文 |
|------|------|------|
| 单行注释 | `// 注释` | 行首/行中 |
| 标签 | `#label: block:` | 标识符前 |

**无歧义**：`#` 后跟标识符 = 标签，`//` 后跟文本 = 注释（不同符号）。

### 24.7 `{` `}` — 花括号

| 职责 | 示例 | 上下文 | 歧义风险 |
|------|------|------|:---:|
| 字典字面量 | `{: "a": 1}` | `{:` 开头 | 无 |
| 集合字面量 | `{1, 2, 3}` | `{` 开头无 `:` | 无 |
| 字符串插值 | `f"x={x}"` | `f"..."` 内 | 无 |
| 推导式 | `{x for x in ...}` | 包含 `for` | 无 |
| `define` 约束 | `props = {x: f64}` | `define` 内 | 低 |

**结论**：`{:` 开头 = 字典，`{` 开头（无 `:`）= 集合/推导式。`{}` = 空集合，`{:}` = 空字典。

### 24.8 `(` `)` — 圆括号

| 职责 | 示例 | 上下文 |
|------|------|------|
| 函数调用 | `f(x, y)` | 标识符后 |
| 元组 | `(1, "hi")` | 多元素逗号分隔 |
| 分组 | `(a + b) * c` | 表达式包围 |
| 参数列表 | `def f(x, y):` | `def` 声明 |
| 泛型参数 | `List<T>` | 类型上下文 |

**无歧义**：上下文完全区分。单元素元组 `(x,)` 需要逗号。

### 24.9 `[` `]` — 方括号

| 职责 | 示例 | 上下文 |
|------|------|------|
| 列表字面量 | `[1, 2, 3]` | 独立表达式 |
| 索引 | `arr[0]` | 标识符后 |
| 泛型类型 | `[int]` | 类型位置 |
| 推导式 | `[x for x in ...]` | 包含 `for` |

**无歧义**：`[T]` 作为类型 = 列表类型，`[expr]` 作为表达式 = 列表字面量。

### 24.10 `<` `>` — 尖括号

| 职责 | 示例 | 上下文 | 歧义风险 |
|------|------|------|:---:|
| 泛型参数 | `List<T>` | 类型位置 | 无 |
| 比较 | `a < b` | 表达式 | 低 |
| 函数组合 | `f <~ g` | 后跟 `~` | 无 |

**风险点**：`a < b > c` 在 Vox 中永远是**两个比较**（`a < b` 和 `b > c`），不会解析为泛型。泛型只出现在类型上下文（`def`/`val`/`class` 声明中）。

### 24.11 `@` — at 符号

| 职责 | 示例 | 上下文 |
|------|------|------|
| 装饰器 | `@decorator` | 函数/类定义前 |

**唯一职责**，无歧义。

### 24.12 `->` `=>` — 箭头

`->` 和 `=>` 是**多用途**符号，上下文决定含义：

| 符号 | 上下文 | 含义 | 示例 |
|------|------|------|------|
| `->` | 函数声明 | 返回类型 | `def f(x: int) -> str:` |
| `->` | 条件表达式 | 条件→结果 | `x ? cond1 -> a -> cond2 -> b` |
| `->` | lambda 体 | 分隔参数与体 | `\|x\| -> x * 2` |
| `=>` | match 分支 | 模式→结果 | `match x: \| 1 => "one"` |
| `=>` | lambda 体 | 分隔参数与体 | `\|x\| => x * 2` |
| `=>` | of 分支 | 模式→结果 | `of Some(v) => v` |

**不是专用**：`->` 不限于函数声明，`=>` 不限于 match。编译器根据上下文消歧。

### 24.13 对比：Python vs Rust vs Vox 歧义符号

| 符号 | Python | Rust | Vox | Vox 风险 |
|------|------|------|------|:---:|
| `:` | dict + slice + lambda + block | 类型 + block | dict + 类型 + block | 低 |
| `{}` | dict + set + f-string | block | set + f-string + define | 低 |
| `[]` | list + index + comprehension | array + index | list + index + type + comprehension | 低 |
| `()` | call + tuple + group | call + tuple + group | call + tuple + group + generic | 低 |
| `<>` | compare | compare + generic | compare + generic + compose | 低 |
| `\|` | or + union + dict merge | or + closure + match | or + closure + match + pipe | 低 |
| `?` | — | Option + error + chain | Option + chain + null | 低 |

### 24.14 `:=` — 海象运算符

| 职责 | 示例 | 上下文 |
|------|------|------|
| 表达式内赋值 | `while (val n = read()) != 0:` | 表达式上下文 |

**设计说明**：`:=` 是双字符 token，在表达式位置进行赋值并返回该值。对标 Python 的 `:=` 和 Go 的 `:=`。

### 24.15 条件表达式 `?` `->` — 多元操作符

```
val f = |x: int| -> x ? cond1 -> rs1
    cond2 -> rs2
    cond3 -> rs3
    other_wise
```

条件表达式由 `?` 开启，`->` 连接条件与结果。`other_wise` 是兜底分支。`?` 和 `->` 之间有空格要求（多元操作符）。`other_wise` 是关键字。

### 24.16 新增符号

| 符号 | 职责 | 上下文 |
|------|------|------|
| `:=` | 表达式内赋值 | `while (val n := read()) != 0:` |
| `?.` | 安全导航 | `obj?.field` |
| `??` | null 合并 | `a ?? b` |
| `?` | 条件表达式开启 | `x ? cond -> val` |
| `->` | 条件表达式箭头 | `cond -> result` |

**总结**：Vox 的符号多义性与 Python 和 Rust 处于同一水平，所有歧义都可以通过上下文或符号组合消除。没有引入新的歧义源。

---

## 25. Token 四级分类体系

### 25.1 设计原则

Token 分类采用**四级递进**体系，从最宽泛到最精细：

| 级别 | 名称 | 含义 | 示例 |
|------|------|------|------|
| 1 | **Category**（种类） | 最宽泛的 token 大类 | `Keyword`、`Operator`、`Literal`、`Punct`、`Delimiter` |
| 2 | **Type**（大类） | token 的语义类型 | `Name`、`Number`、`String`、`Punct` |
| 3 | **Genre**（小类） | 用法层面的大类 | `StatementStart`、`InfixOperator`、`ExpressionStart`、`BlockOpener` |
| 4 | **Kind**（细类） | 具体职责/角色 | `IfKeyword`、`ForKeyword`、`ColonBlock`、`ColonType` |

### 25.2 用户自定义 Category

用户可以通过 `*fix` 声明**自定义操作符**，编译器自动注册对应的 `Category`。当用户定义：

```
prefix `++` inc<T>(T) where T: Add -> T
infix `++` concat<T>(T, T) where T: Add -> T
```

编译器自动创建：
- `Category`: `Operator`（用户自定义符号一概归入 Operator）
- `Type`: `Punct`（自定义符号是标点）
- `Genre`: `PrefixOperator` / `InfixOperator`（取决于声明）
- `Kind`: `UserDefined("++".to_string())`（动态 kind）

所有用户自定义的 `*fix` 符号，`Kind` 都是 `UserDefined(symbol)`，其中 `symbol` 是原始符号字符串。

### 25.3 操作符空白规则

#### 25.3.1 总规则

| 操作符类型 | 空白规则 | 示例 |
|-----------|---------|------|
| `prefix` / `suffix` | 符号**紧贴**操作数 | `++5`、`5!`、`@name` |
| `infix` / `nthfix` | 符号**必须两侧有空格** | `a + b`、`val s = 1`、`cond ? a : b` |
| `pairfix` | 符号紧贴操作数 | `《三体》` |

#### 25.3.2 多元操作符（infix / nthfix）不可紧贴

```
val s = 1        // ✅ 正确
val s=1          // ❌ 错误：= 操作符两侧必须空格

a + b            // ✅ 正确
a+b              // ❌ 错误：+ 操作符两侧必须空格

cond ? a : b     // ✅ 正确（nthfix）
cond ? a: b      // ❌ 错误：: 操作符两侧必须空格
```

#### 25.3.3 前缀/后缀操作符必须紧贴

```
++5              // ✅ 正确
++ 5             // ❌ 错误：前缀操作符不能有空格

5!               // ✅ 正确
5 !              // ❌ 错误：后缀操作符不能有空格
```

#### 25.3.4 冒号 `:` 空白规则

| 场景 | 规则 | 示例 |
|------|------|------|
| 语句块开始 | 可松可紧 | `if x > 0 :` 或 `if x > 0:` 均可 |
| 类型注解 | **前紧后松** | `val s: int = 1` |
| 字典键值 | 可松可紧 | `{: "a": 1}` 或 `{: "a" : 1}` |
| 字典开头 `{:` | `{` 和 `:` 必须紧贴 | `{:}`、`{: "a": 1}` |
| 函数返回类型 | 可松可紧 | `def f(x) -> int :` 或 `def f(x) -> int:` |
| `<:` 调用应用 | `<:` 前需空格 | `lst.map <:` （注意空格） |

**冒号换行规则**：`:` 后若换行，下一行**必须缩进**（表示一个整块）。不换行则效果相同。

```
if x > 0 :         // ✅ 换行后缩进
    print("ok")

if x > 0 : print("ok")  // ✅ 不换行，效果相同

val s = if a : b else : c  // ✅ 单行，合法
```

**注意**：只有类型注解 `:` 严格要求前紧后松，其他场景 `:` 可松可紧。

```
val s: int = 1     // ✅ 正确：类型注解前紧后松
val s : int = 1    // ❌ 错误：类型注解 `:` 不能前松
val s:  int = 1    // ❌ 错误：类型注解 `:` 不能后紧（需要空格）

if x > 0 :         // ✅ 正确
else :             // ✅ 正确
else:              // ✅ 也正确
```

#### 25.3.5 问号 `?` 紧贴规则

`?` 作为类型后缀或安全导航时**必须紧贴**前一个 token：

```
int?               // ✅ 正确：Option 类型
s?.field           // ✅ 正确：安全导航
s??                // ✅ 正确：强制解包
a ?? b             // ✅ 正确：null 合并（?? 是双字符，作为 infix 需要空格）

int ?              // ❌ 错误：类型问号不能有空格
s ?. field         // ❌ 错误：安全导航不能有空格
```

#### 25.3.6 泛型 `<>` 紧贴规则

```
name<T>()          // ✅ 正确：泛型尖括号紧贴类型名
List<int>          // ✅ 正确
name < T > ()      // ❌ 错误：泛型尖括号不能有空格

a < b              // ✅ 正确：比较运算符（infix 需要空格）
```

#### 25.3.7 逗号 `,` 规则

逗号可松可紧，无严格要求。创建元组、列表时，最后一个元素后可加或不加逗号：

```
[1, 2, 3]          // ✅ 正确
[1,2,3]            // ✅ 正确（紧贴也允许）
[1, 2, 3,]         // ✅ 正确：尾部逗号允许
(1, 2, 3)          // ✅ 正确
(1,)               // ✅ 正确：单元素元组
```

#### 25.3.8 分号 `;` 规则

分号用于一行多语句，**紧贴前一个 token**，与后一个 token 之间**需要空格**。分号**不能作为行尾**：

```
val s = 1; val t = 2    // ✅ 正确：; 紧贴 1，与 val 有空格
val s = 1 ; val t = 2   // ❌ 错误：; 前面有空格
val s = 1;val t = 2     // ❌ 错误：; 后面没有空格
val s = 1;              // ❌ 错误：分号不能作为行尾
```

### 25.4 空白规则速查表

| Token | 前空格 | 后空格 | 备注 |
|-------|:---:|:---:|------|
| `=`（赋值） | 必须 | 必须 | `val s = 1` |
| `:`（语句块） | 任意 | 换行缩进 | `if x > 0 :` 或 `if x > 0:` |
| `:`（类型注解） | 紧贴 | 必须 | `val s: int` |
| `:`（字典键值） | 任意 | 任意 | `{: "a": 1}` |
| `{:`（字典开头） | 紧贴 | 任意 | `{:}` |
| `^`（所有权转移） | 紧贴 | 任意 | `s^` 后缀 |
| `|`（match 分支） | 必须 | 必须 | `\| [] => 0` |
| `|`（位或） | 必须 | 必须 | `a \| b` |
| `\|\|`（逻辑或） | 必须 | 必须 | `a \|\| b` |
| `\| \|`（无参函数） | 空格在中间 | 任意 | `\| \|` 中间有空格 |
| `=>` / `->` 换行 | 任意 | 换行缩进 | 不换行效果相同 |
| `=` 换行 | 任意 | 换行缩进 | 不换行效果相同 |
| `!`（模板应用） | 紧贴 | 任意 | `times!(5)` |
| `?`（类型后缀） | 紧贴 | 紧贴或空格 | `int?`、`s?.` |
| `?.`（安全导航） | 紧贴 | 紧贴 | `obj?.field` |
| `??`（null合并） | 必须 | 必须 | `a ?? b` |
| `<` `>`（泛型） | 紧贴 | 紧贴 | `List<int>` |
| `<` `>`（比较） | 必须 | 必须 | `a < b` |
| `,`（逗号） | 任意 | 任意 | `[1,2,3]` |
| `;`（分号） | 紧贴（前） | 必须（后） | `a; b` |
| `:=`（海象） | 必须 | 必须 | `val n := read()` |
| 所有 infix/nthfix | 必须 | 必须 | `a + b` |
| 所有 prefix/suffix | 紧贴 | 紧贴 | `++5` |

---

> 本设计文档描述 Vox 语言的核心特性。Vox 是 Rust 的语法糖皮，所有 Rust 的库、性能、安全都保留，只是写法更舒服。
> ⚠️ **[OBSOLETE — 已过时]** 本文件为 Lang-Zong **旧版设计规范**，含大量已被 `hermes/00-最终语法规范.md` 与编译器 `src/*.rs` 删除的语法（`[int]`、`->` 返回类型、`raises`、`fn(T)->U`、`{:...}` 字典、`async def`、`class` 等）。**以 `src/*.rs` 实现代码为唯一运行时权威基线**；本文件仅作历史参考，请勿据此编写新代码。修正清单见 `workbuddy/plan/语法一致性审查报告.md`。

# Lang-Zong 语言设计规范 v1

> 静态类型、编译型、转译到 Rust 的通用编程语言

---

## 目录

1. [语言定位](#1-语言定位)
2. [关键字](#2-关键字)
3. [词法元素](#3-词法元素)
4. [类型系统](#4-类型系统)
5. [变量与绑定](#5-变量与绑定)
6. [函数与调用块](#6-函数与调用块)
7. [数据类型](#7-数据类型)
8. [控制流](#8-控制流)
9. [操作符与表达式](#9-操作符与表达式)
10. [泛型与 where](#10-泛型与-where)
11. [错误处理](#11-错误处理)
12. [并发](#12-并发)
13. [宏与编译期](#13-宏与编译期)
14. [模块与导入](#14-模块与导入)
15. [测试](#15-测试)
16. [转译引擎](#16-转译引擎)
17. [字符串与容器](#17-字符串与容器)
18. [完整示例](#18-完整示例)

---

## 1. 语言定位

Lang-Zong 是一门静态类型、编译型、转译到 Rust 的通用编程语言。

### 核心特性

| 特性 | 说明 |
|------|------|
| 缩进作用域 | 4 空格缩进决定作用域，无 `end` 关键字 |
| `=` 定义体 | 函数、类型、模块均用 `=` 开启定义体 |
| 构建块 | `=:` 变量构建块 / `~:` 调用构建块 / `*:` 生成器调用构建块（无参闭包，块内默认 unsafe） |
| 自动解包 | 元组 → 位置实参、字典 → 命名实参（经 `__Pack` 类型擦除），BuildParams 对象整体传入 |
| 仓颉式可变参数 | 末参数类型 `[T]` 时自动收集 |
| 单一 Rust 后端 | 源码转译为 Rust，编译为原生可执行文件 |
| 命名约定可见性 | 无 `pub`/`private` 关键字，通过命名控制 |
| trait 支持字段 | trait 可定义字段，`extend` 扩展字段 |

### 已移除的旧设计

| 移除项 | 原因 |
|--------|------|
| `class` / `super` | 使用 struct + trait 组合替代 |
| `Type` 关键字 | 方法区分通过首参数类型判断 |
| `Args` / `KwArgs` / `out` | 已移除，参数包改用 `=:`/`~:`/`*:` 构建块 |
| `defer` 关键字 | 用户自定义 `__enter__` / `__exit__` |
| `quote:` 关键字 | 改用三反引号代码字面量 |
| `pub` / `private` | 命名约定控制可见性 |
| `*args` / `**kwargs` | 改用仓颉式可变参数 |

---

## 2. 关键字

### 完整列表

```
and         as          assert      async       await
block       break       case        catch
comptime    const       continue    def
elif        else        enum        extend
finally     for         from        guard       if
impl        import      in          is
let         loop        macro       match       mut
not         None        owned
raise       raises      return
Self        select      spawn       struct      suite
test        trait       True        False       try
where       while       with        yield
```

### 按类别分组

| 类别 | 关键字 |
|------|--------|
| 声明 | `def` `struct` `enum` `trait` `impl` `const` `let` |
| 修饰符 | `mut` `owned` |
| 导入 | `import` `from` `as` |
| 控制流 | `if` `elif` `else` `match` `case` `loop` `while` `for` `block` `break` `continue` `return` |
| 异常 | `try` `catch` `finally` `raise` `raises` |
| 并发 | `async` `await` `spawn` `select` |
| 迭代 | `in` `yield` |
| 扩展 | `extend` |
| 类型/泛型 | `Self` `where` |
| 宏/编译期 | `macro` `comptime` |
| 其他 | `guard` `assert` `test` `suite` `with` `is` `not` |
| 字面量 | `True` `False` `None` |
| 逻辑 | `and` `or` |

---

## 3. 词法元素

### 3.1 文件编码

源文件使用 UTF-8 编码。

### 3.2 注释

```lz
# 单行注释

##
多行注释
第二行
##
```

### 3.3 缩进规则

- 4 空格缩进
- 无 Tab
- 缩进决定作用域，无 `end` 关键字

### 3.4 标识符

| 命名 | 可见性 |
|------|:------:|
| `foo` | 公开 |
| `_foo` | 私有 |
| `__foo__` | 魔法方法 |

### 3.5 字面量

```lz
# 整数
42
0b1010
0o52
0x2A

# 浮点数
3.14
6.02e23

# 布尔
True
False

# None
None

# 字符串
"hello"
'hello'
r"raw string"
f"hello {name}"

# 多行字符串
"""
line 1
line 2
"""

# 容器
[1, 2, 3]           # 数组
{"a": 1, "b": 2}    # 字典
{1, 2, 3}           # 集合
(1, "a")            # 元组
```

### 3.6 代码字面量

三反引号包裹代码，作为编译期 Token 字面量：

```lz
# 基本形式
```
    code here
    $(expr)  # 插值
```

# f 前缀（支持插值）
f```
    print("before")
    $(expr)
    print("after")
```

# r 前缀（原始，不插值）
r```
    re = r"^\d+$"
```
```

### 3.7 运算符

| 类别 | 运算符 |
|------|--------|
| 算术 | `+` `-` `*` `/` `%` `**` |
| 比较 | `==` `!=` `<` `>` `<=` `>=` |
| 逻辑 | `and` `or` `not` |
| 位运算 | `&` `|` `^` `<<` `>>` |
| 赋值 | `=` `+=` `-=` `*=` `/=` `%=` |
| 成员 | `in` `is` |
| 管道 | `|>` `<|` |
| 所有权 | `^` |

---

## 4. 类型系统

### 4.1 基本类型

| 类型 | 说明 | Rust 映射 |
|------|------|-----------|
| `int` | 64 位整数 | `i64` |
| `float` / `f64` | 64 位浮点数 | `f64` |
| `bool` | 布尔值 | `bool` |
| `str` | 字符串 | `String` |

### 4.2 复合类型

```lz
# 数组
[int]       # 动态数组
[int; 5]    # 固定长度数组

# 元组
(int, str)
struct Point = (x: int, y: int)  # 命名元组

# 字典
{str: int}

# 集合
{int}
```

### 4.3 引用类型

```lz
&int       # 不可变引用
&mut int   # 可变引用
```

### 4.4 Option 类型

```lz
enum Option<T> =
    None
    Some(T)

def find(arr: [int], target: int) -> Option<int> =
    for i, v in arr.iter().enumerate():
        if v == target:
            return Some(i)
    return None
```

### 4.5 Result 类型

```lz
enum Result<T, E> =
    Ok(T)
    Err(E)

def divide(a: int, b: int) -> Result<int, str> =
    if b == 0:
        return Err("division by zero")
    return Ok(a / b)
```

### 4.6 类型推导

```lz
x = 42           # int
y = "hello"      # str
z = [1, 2, 3]    # [int]

# 显式标注
x : int = 42
```

### 4.7 类型转换

```lz
x : int = 42
y : f64 = x as f64
```

---

## 5. 变量与绑定

### 5.1 绑定形式

| 形式 | 可变性 | 转移性 |
|------|:---:|:---:|
| `x = e` | 不可变 | 可转移 |
| `x : mut = e` | 可变 | 可转移 |
| `x : owned T = e` | 可变 | 不可转移 |
| `x : const T = e` | 编译期常量 | 编译期求值 |

### 5.2 不可变绑定

```lz
x = 42
x : int = 42
```

### 5.3 可变绑定

```lz
x : mut = 42
x : mut int = 42
x += 1
```

### 5.4 owned 绑定

`owned` 表示可变但**不可转移**所有权：

```lz
z : owned int = 1
z += 1  # ✅ 可修改
# k = z^  # ❌ 编译错误：owned 从不转移
```

### 5.5 const 绑定

```lz
PI : const f64 = 3.1415926
MAX_SIZE : const int = 1024
```

### 5.6 所有权转移

```lz
y : mut int = 5
t = y^  # y 的所有权转移给 t，之后 y 不可用
```

### 5.7 解构绑定

```lz
# 元组解构
(x, y) = (1, 2)

# 枚举解构
Some(v) = maybe_value

# 数组解构
[first, ...rest] = [1, 2, 3]
```

---

## 6. 函数与调用块

### 6.1 函数定义

```lz
def hello() =
    print("hello")

def add(a: int, b: int) -> int =
    return a + b

def add1(x: int) -> int = x + 1
```

### 6.2 完整签名

```lz
def name<泛型>(参数) -> 返回类型 raises 异常 where 约束 = 函数体
```

### 6.3 匿名函数

```lz
double = |x: int| -> int = x * 2
add = |a: int, b: int| -> int = a + b

# 类型推导
sq = |x| x * x
```

### 6.4 `_` 占位符

```lz
[1, 2, 3].map(_ * 2)       # 等价于 .map(|x| x * 2)
[1, 2, 3].reduce(_ + _)    # 等价于 .reduce(|a, b| a + b)
```

### 6.5 参数传递

| 修饰 | 含义 |
|------|------|
| 默认 | 借用（只读） |
| `owned` | 所有权转移 |
| `mut` | 可变借用 |

```lz
def inspect(data: Data) =           # 默认借用
def consume(data: owned Data) =     # 拿走所有权
def modify(data: mut Data) =        # 可变借用
```

### 6.6 可变参数（仓颉式）

末参数类型为 `[T]` 时自动收集：

```lz
def sum(nums: [int]) -> int =
    total = 0
    for n in nums:
        total += n
    return total

sum(1, 2, 3, 4)  # 自动收集为 [1, 2, 3, 4]
```

### 6.7 调用构建块 `~:`

`callee ~:` 后换行缩进，为可调用对象构造参数包并应用：

```lz
def hello(age: int, name: str) =
    print(f"Hi! my name is {name}, i'm {age} years old")

hello ~:
    (22, "Alice")          # 元组 → 位置参数
```

### 6.8 变量构建块 `=:`

`name =:` 后换行缩进，用无参闭包计算值并绑定到变量（块内默认 unsafe）：

```lz
safe =:
    let p = &raw as *const i32
    *p                      # 指针语法作用域限定块内
```

### 6.9 生成器调用构建块 `*: `

`callee *:` 后换行缩进，逐步产出参数包（惰性迭代器）：

```lz
for r in hello *:
    yield (22, "Alice")
    return                  # IterStopException 停止信号
```

### 6.10 接收返回值

```lz
result = hello ~:
    (22, "Alice")
```

### 6.11 自动解包

**编译期解包**（字面量 / const 字典）：

```lz
const DEFAULT = { x: 0, y: 0 }
p = Point(DEFAULT)  # 编译期展开为 Point(x=0, y=0)
```

**运行时解包**（非 const 变量，发 warning）：

```lz
d = some_dict()
p = Point(d)  # warning: runtime unpack, type not checked
```

**支持的解包类型**：
- `tuple` → 位置参数
- `namedtuple` → 关键字参数
- 实现 `BuildParams` 的类型 → 自定义构造

### 6.12 方法区分

| 首参数类型 | 方法类型 |
|------------|----------|
| 定义类型 | 实例方法 |
| 其他 | 静态方法 |

```lz
struct Point =
    x : f64
    y : f64

    # 实例方法
    def distance(self: Point, other: Point) -> f64 =
        ...

    # 静态方法
    def origin() -> Point =
        Point(0.0, 0.0)
```

### 6.13 高阶函数与管道

```lz
doubled = [1, 2, 3].map(|x| x * 2)
evens = [1, 2, 3, 4].filter(|x| x % 2 == 0)

result = [1, 2, 3, 4, 5]
    |> filter(|x| x % 2 == 0)
    |> map(|x| x * x)
    |> sum()
```

### 6.14 生成器

```lz
def fibs() =
    yield 0
    yield 1
    a, b = 0, 1
    loop:
        c = a + b
        yield c
        a, b = b, c
```

---

## 7. 数据类型

### 7.1 struct

```lz
struct Point =
    x : f64
    y : f64

    def distance(self: Point, other: Point) -> f64 =
        dx = self.x - other.x
        dy = self.y - other.y
        (dx * dx + dy * dy).sqrt()
```

**构造**：

```lz
p = Point(1.0, 2.0)
p2 = Point(x=3.0, y=4.0)
```

**默认值**：

```lz
struct Point =
    x : f64 = 0.0
    y : f64 = 0.0
```

**泛型**：

```lz
struct Box<T> =
    value : T

b = Box<int>(42)
```

**扩展字段**：

```lz
trait HasPosition =
    x : f64
    y : f64

struct Point =
    extend HasPosition
```

### 7.2 enum

```lz
enum Option<T> =
    None
    Some(T)

enum Result<T, E> =
    Ok(T)
    Err(E)

enum Color =
    Red(f64, f64, f64)
    Green(f64, f64, f64)
    Blue(f64, f64, f64)

enum Status =
    Active
    Inactive
    Banned
```

**方法**：

```lz
enum HttpStatus =
    Ok(u16)
    NotFound(str)
    ServerError(u16, str)

    def code(self: HttpStatus) -> u16 =
        match self:
            case Ok(c):
                c
            case NotFound(_):
                404
            case ServerError(c, _):
                c
```

### 7.3 trait

trait 支持字段和方法：

```lz
trait HasPosition =
    x : f64
    y : f64

    def distance(self: Self, other: Self) -> f64
```

**默认实现**：

```lz
trait Iterator =
    Item : type
    def next(self: Self) -> Option<Self.Item>

    def collect(self: Self) -> [Self.Item] =
        items : mut [Self.Item] = []
        loop:
            match self.next():
                case Some(v):
                    items.push(v)
                case None:
                    break items
```

**方法类型**：

| 首参 | 含义 | 调用方式 |
|------|------|----------|
| `Self` | 实例方法 | `instance.method()` |
| 其他 | 静态方法 | `TypeName::method()` |

**trait 组合**：

```lz
trait Read =
    def read(self: Self) -> str

trait Write =
    def write(mut Self, data: str)

ReadWrite = Read & Write
```

### 7.4 impl

**扩展方法**：

```lz
impl HasPosition for Point =
    def distance(self: Point, other: Point) -> f64 =
        dx = self.x - other.x
        dy = self.y - other.y
        (dx * dx + dy * dy).sqrt()
```

**为泛型类型实现**：

```lz
impl<T> Box<T> =
    def new(value: T) -> Box<T> =
        return Box<T>(value)
```

**带约束的 impl**：

```lz
impl<T> Box<T>
where T : Display =
    def print(self: Box<T>) =
        print(self.value)
```

### 7.5 魔法方法

```lz
struct Vector =
    x : f64
    y : f64

    def __add__(self: Vector, other: Vector) -> Vector =
        Vector(self.x + other.x, self.y + other.y)

    def __eq__(self: Vector, other: Vector) -> bool =
        self.x == other.x and self.y == other.y

    def __enter__(self: Self) -> Self =
        self

    def __exit__(self: Self, error: Option<Error>) -> bool =
        false
```

**完整列表**：

```
__init__    __del__     __copy__    __move__
__eq__      __ne__      __hash__    __str__     __repr__
__bool__    __int__     __float__   __call__
__add__     __sub__     __mul__     __div__     __mod__     __pow__
__neg__     __pos__     __abs__     __invert__
__iadd__    __isub__    __imul__    __idiv__
__getitem__ __setitem__ __delitem__ __len__     __contains__
__iter__    __next__
__enter__   __exit__
__getattr__ __setattr__ __delattr__
```

### 7.6 上下文管理器

```lz
with Connection("localhost", 8080) as conn:
    conn.send("hello")
```

---

## 8. 控制流

### 8.1 if / elif / else

```lz
if x > 0:
    print("positive")
elif x < 0:
    print("negative")
else:
    print("zero")
```

**表达式形式**：

```lz
result = if x > 0:
    "positive"
else:
    "non-positive"
```

### 8.2 match

```lz
match x:
    case 1:
        print("one")
    case 2:
        print("two")
    case _:
        print("other")
```

**模式匹配**：

```lz
match opt:
    case Some(v):
        print(f"value: {v}")
    case None:
        print("none")
```

**带条件**：

```lz
match num:
    case n if n > 0:
        print(f"positive: {n}")
    case n if n < 0:
        print(f"negative: {n}")
    case _:
        print("zero")
```

**元组模式**：

```lz
match pair:
    case (0, y):
        print(f"first is 0, second is {y}")
    case (x, 0):
        print(f"first is {x}, second is 0")
    case (x, y):
        print(f"both non-zero: {x}, {y}")
```

### 8.3 loop

```lz
n = 0
loop:
    n += 1
    if n > 5:
        break
    print(n)
```

**break 返回值**：

```lz
result = loop:
    for x in arr:
        if x > 10:
            break x
    break 0
```

### 8.4 while

```lz
n = 0
while n < 5:
    print(n)
    n += 1
```

### 8.5 for

```lz
# 遍历容器
for v in [1, 2, 3]:
    print(v)

# 遍历范围
for i in 0..10:
    print(i)

# 枚举
for i, v in arr.iter().enumerate():
    print(f"{i}: {v}")

# 遍历字典
for k, v in {"a": 1, "b": 2}:
    print(f"{k}: {v}")
```

### 8.6 break / continue

```lz
for i in 0..10:
    if i == 5:
        break
    if i % 2 == 0:
        continue
    print(i)
```

### 8.7 return

```lz
def add(a: int, b: int) -> int =
    return a + b
```

### 8.8 guard

```lz
# 基本 guard：else 后直接写表达式值（不写 return）
def safe_divide(a: int, b: int): int? =
    guard b != 0 else: None
    Some(a / b)

# guard + 绑定
def process(opt: int?) =
    guard let Some(v) = opt else: ()
    print(v)

def check_age(age: int) =
    guard age >= 0 else: raise ValueError("negative age")
```

> **规则：`guard cond else: expr` 中 `else:` 后直接写表达式值，不写 `return`。**
> 编译器会将 `guard cond else: expr` 转译为 `if !cond { return expr; }`。

### 8.9 block

```lz
result = block:
    x = 1
    y = 2
    x + y
```

### 8.10 单行多语句

用 `;` 分隔，**分号后不能留白**：

```lz
def hello(age: int, name: str) = print(f"Hi {name}"); "done"
```

---

## 9. 操作符与表达式

### 9.1 算术运算符

| 运算符 | 含义 |
|--------|------|
| `+` | 加法 |
| `-` | 减法 |
| `*` | 乘法 |
| `/` | 除法 |
| `%` | 取模 |
| `**` | 幂运算 |

### 9.2 比较运算符

| 运算符 | 含义 |
|--------|------|
| `==` | 等于 |
| `!=` | 不等于 |
| `<` | 小于 |
| `>` | 大于 |
| `<=` | 小于等于 |
| `>=` | 大于等于 |

### 9.3 逻辑运算符

| 运算符 | 含义 |
|--------|------|
| `and` | 逻辑与 |
| `or` | 逻辑或 |
| `not` | 逻辑非 |

### 9.4 位运算符

| 运算符 | 含义 |
|--------|------|
| `&` | 按位与 |
| `|` | 按位或 |
| `^` | 按位异或 |
| `<<` | 左移 |
| `>>` | 右移 |

### 9.5 赋值运算符

| 运算符 | 含义 |
|--------|------|
| `=` | 赋值 |
| `+=` | 加赋值 |
| `-=` | 减赋值 |
| `*=` | 乘赋值 |
| `/=` | 除赋值 |
| `%=` | 模赋值 |

### 9.6 成员运算符

| 运算符 | 含义 |
|--------|------|
| `in` | 成员检测 |
| `is` | 身份检测 |

### 9.7 管道运算符

| 运算符 | 含义 |
|--------|------|
| `|>` | 左管道 |
| `<|` | 右管道 |

### 9.8 所有权运算符

| 运算符 | 含义 |
|--------|------|
| `^` | 显式转移所有权 |

### 9.9 表达式优先级

从高到低：

1. 括号 `()`
2. 幂运算 `**`
3. 一元运算符 `-` `not`
4. 乘法/除法/取模 `*` `/` `%`
5. 加法/减法 `+` `-`
6. 位运算符 `<<` `>>` `&` `^` `|`
7. 比较运算符
8. `in` `is`
9. `not`
10. `and`
11. `or`

---

## 10. 泛型与 where

### 10.1 泛型函数

```lz
def identity<T>(x: T) -> T =
    return x

# 调用
a = identity(42)
b = identity<int>(42)
```

### 10.2 多类型参数

```lz
def pair<T, U>(x: T, y: U) -> (T, U) =
    return (x, y)
```

### 10.3 泛型方法

```lz
struct Box<T> =
    value : T

    def map<U>(self: Box<T>, f: fn(T) -> U) -> Box<U> =
        return Box<U>(f(self.value))
```

### 10.4 泛型类型

```lz
struct Point<T> =
    x : T
    y : T

p = Point<int>(1, 2)

enum Option<T> =
    None
    Some(T)
```

### 10.5 where 子句

```lz
def compare<T>(a: T, b: T) -> int
where T : Ord =
    if a < b:
        return -1
    elif a > b:
        return 1
    else:
        return 0

def print_and_clone<T>(x: T) -> T
where T : Debug + Clone =
    print(x)
    return x.clone()
```

### 10.6 关联类型约束

```lz
trait Iterator =
    Item : type
    def next(self: Self) -> Option<Self.Item>

def collect<I>(iter: I) -> List<I.Item>
where I : Iterator =
    ...
```

### 10.7 impl 泛型

```lz
impl<T> Box<T> =
    def new(value: T) -> Box<T> =
        return Box<T>(value)

impl<T> Box<T>
where T : Display =
    def print(self: Box<T>) =
        print(self.value)
```

### 10.8 const 泛型

```lz
struct Array<T, N: int> =
    data : [T; N]

a = Array<int, 5>([1, 2, 3, 4, 5])
```

### 10.9 泛型推导

```lz
v : List<int> = List::new()
x = first([1, 2, 3])  # 从参数推导

# 显式标注
v = List::<int>.new()
```

---

## 11. 错误处理

### 11.1 Result 类型

```lz
def divide(a: int, b: int) -> Result<int, str> =
    if b == 0:
        return Err("division by zero")
    return Ok(a / b)

r = divide(10, 2)
match r:
    case Ok(v):
        print(f"result: {v}")
    case Err(e):
        print(f"error: {e}")
```

### 11.2 Option 类型

```lz
def find(arr: [int], target: int) -> Option<int> =
    for i, v in arr.iter().enumerate():
        if v == target:
            return Some(i)
    return None
```

### 11.3 try / catch / finally

```lz
try:
    f = File.open("data.txt")
    content = f.read()
catch e: IOError:
    print(f"IO error: {e}")
catch e:
    print(f"unknown error: {e}")
finally:
    cleanup()
```

### 11.4 raise

```lz
def validate_age(age: int) =
    if age < 0:
        raise ValueError("age cannot be negative")
```

### 11.5 raises 标注

```lz
def read_file(path: str) -> str
raises IOError, ValueError =
    ...
```

### 11.6 ? 运算符

```lz
def process() -> Result<int, str> =
    f = File.open("data.txt")?
    content = f.read()?
    return Ok(content.len())

def get_first(arr: [[int]]) -> Option<int> =
    first = arr.first()?
    return first.first()
```

### 11.7 with 语句

```lz
with File.open("data.txt") as f:
    content = f.read()
```

### 11.8 断言

```lz
def divide(a: int, b: int) -> int =
    assert b != 0, "division by zero"
    return a / b

debug_assert(x.is_valid())  # 仅 debug 模式
```

### 11.9 不可恢复错误

```lz
panic!("critical failure: {}", msg)

match x:
    case 1:
        ...
    case _:
        unreachable!()
```

---

## 12. 并发

### 12.1 spawn

```lz
handle = spawn:
    print("hello from thread")
handle.join()
```

### 12.2 async / await

```lz
async def fetch(url: str) -> str =
    return await http.get(url)

async def main() =
    result = await fetch("https://example.com")
    print(result)
```

### 12.3 并发执行

```lz
async def main() =
    a = async fetch("https://a.com")
    b = async fetch("https://b.com")
    (ra, rb) = await (a, b)
```

### 12.4 select

```lz
async def race() =
    select:
        r = await task1():
            print(f"task1 won: {r}")
        r = await task2():
            print(f"task2 won: {r}")
```

### 12.5 通道

```lz
(tx, rx) = channel<int>()

spawn:
    tx.send(1)
    tx.send(2)

for v in rx:
    print(v)
```

### 12.6 互斥锁

```lz
counter = Mutex<int>(0)

def increment() =
    guard = counter.lock()
    guard* += 1
```

### 12.7 原子类型

```lz
counter = Atomic<int>(0)
counter.fetch_add(1)
```

---

## 13. 宏与编译期

### 13.1 代码字面量

```lz
# 基本形式
macro hello() -> Tokens =
    return ```
        print("hello")
    ```

# f 前缀（插值）
macro log(expr: Tokens) -> Tokens =
    return f```
        print("before")
        $(expr)
        print("after")
    ```

# r 前缀（原始）
macro regex_pattern() -> Tokens =
    return r```
        re = r"^\d+$"
    ```
```

### 13.2 宏调用

**函数式调用**：

```lz
println!("hello {}", name)
vec![1, 2, 3]
assert!(x > 0)
```

**装饰器调用**：

```lz
@derive(Clone, Debug)
struct Point =
    x : int
    y : int

@test
def test_add() =
    assert 1 + 1 == 2
```

### 13.3 const

```lz
const MAX_SIZE : int = 1024
const PI : f64 = 3.1415926535

const def fib(n: int) -> int =
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

const FIB_10 : int = fib(10)
```

### 13.4 comptime 块

```lz
comptime:
    computed = complex_calculation()

const RESULT : int = computed
```

### 13.5 元变量

| 分类符 | 匹配内容 |
|:------:|----------|
| `ident` | 标识符 |
| `expr` | 表达式 |
| `ty` | 类型 |
| `pat` | 模式 |
| `stmt` | 语句 |
| `block` | 代码块 |
| `tt` | 单 token 树 |

### 13.6 常用内置宏

| 宏 | 用途 |
|----|------|
| `println!` | 打印并换行 |
| `format!` | 格式化字符串 |
| `vec!` | 创建向量 |
| `assert!` | 断言 |
| `derive` | 自动派生 trait |
| `test` | 标记测试函数 |

---

## 14. 模块与导入

### 14.1 文件即模块

```
src/
├── main.lz          # main 模块
├── utils.lz         # utils 模块
└── math/
    ├── mod.lz       # math 模块入口
    ├── vector.lz    # math::vector
    └── matrix.lz    # math::matrix
```

### 14.2 import 导入

```lz
import std.io
import std.collections.list

import std.io as io
from std.io import println
from std.collections import List, HashMap
```

### 14.3 可见性规则

| 名称 | 可见性 |
|------|:------:|
| `foo` | 公开 |
| `_foo` | 私有 |
| `__foo__` | 魔法方法 |

### 14.4 包配置

```toml
# project.toml
[package]
name = "my_project"
version = "0.1.0"

[dependencies]
serde = "1.0"
```

### 14.5 相对路径

```lz
from .utils import helper
from ..math import Vec2
```

---

## 15. 测试

### 15.1 测试函数

```lz
@test
def test_add() =
    assert 1 + 1 == 2

@test(should_fail)
def test_should_fail() =
    assert 1 == 2
```

### 15.2 测试模块

```lz
@test
suite math_tests =

    @test
    def test_add() =
        assert 1 + 1 == 2

    @test
    def test_sub() =
        assert 2 - 1 == 1
```

### 15.3 断言函数

| 函数 | 用途 |
|------|------|
| `assert(cond)` | 条件为真 |
| `assert_eq(a, b)` | 相等 |
| `assert_ne(a, b)` | 不等 |
| `assert_raises(f, E)` | 抛出异常 E |

### 15.4 测试属性

```lz
@test(ignore)
def test_expensive() =
    ...

@test(timeout=1000)
def test_fast() =
    ...
```

### 15.5 基准测试

```lz
@bench
def bench_sort() =
    arr = [3, 1, 4, 1, 5, 9, 2, 6]
    arr.sort()
```

---

## 16. 转译引擎

### 16.1 编译流程

```
.lz 源码 → 词法分析 → 语法分析 → 语义分析 → 代码生成 → .rs → rustc → 二进制
```

### 16.2 类型映射

| Lang-Zong | Rust |
|-----------|------|
| `int` | `i64` |
| `float` | `f64` |
| `bool` | `bool` |
| `str` | `String` |
| `[T]` | `Vec<T>` |
| `(T, U)` | `(T, U)` |

### 16.3 语法映射

```lz
# Lang-Zong
def foo(x: int) -> int =
    if x > 0:
        return x
    else:
        return -x
```

```rust
// Rust
fn foo(x: i64) -> i64 {
    if x > 0 {
        return x;
    } else {
        return -x;
    }
}
```

### 16.4 构建命令

```bash
$ lz build           # 构建
$ lz build --release # 发布构建
$ lz run             # 运行
$ lz test            # 测试
$ lz check           # 类型检查
```

---

## 17. 字符串与容器

### 17.1 字符串

```lz
s = "hello world"
path = r"C:\Users\name"
name = "world"
greeting = f"hello {name}"

# 多行
s = """
    line 1
    line 2
    """

# 操作
s.len()
s.contains("ell")
s.to_upper()
s.trim()
s.split(" ")
```

### 17.2 数组

```lz
arr = [1, 2, 3, 4, 5]
arr : mut = [1, 2, 3]

arr.push(4)
arr.pop()
arr[0]
arr[1:3]

# 推导
squares = [x * x for x in range(10)]
evens = [x for x in range(20) if x % 2 == 0]
```

### 17.3 字典

```lz
d = {"a": 1, "b": 2}
d : mut = {"a": 1}

d["b"] = 2
d.get("a")
d.remove("a")

for k, v in d:
    print(f"{k}: {v}")
```

### 17.4 集合

```lz
s = {1, 2, 3}
s : mut = {1, 2, 3}

s.add(4)
s.remove(3)
s.contains(2)

# 集合运算
a = {1, 2, 3}
b = {3, 4, 5}
a + b   # 并集
a & b   # 交集
a - b   # 差集
```

### 17.5 元组

```lz
t = (1, "hello", 3.14)
x, y, z = (1, 2, 3)
t.0  # 1
```

### 17.6 范围

```lz
0..10    # 0 到 9
0..=10   # 0 到 10

for i in 0..10:
    print(i)
```

### 17.7 迭代器

```lz
arr.iter()
    .map(|x| x * 2)
    .filter(|x| x > 4)
    .collect()
```

### 17.8 生成器

```lz
def count(n: int) =
    for i in 0..n:
        yield i

for v in count(5):
    print(v)
```

---

## 18. 完整示例

```lz
# Lang-Zong 完整示例
# 演示语言的主要特性

import std.io
from std.io import println

## 1. 变量与绑定

x = 42
y : mut = 0
y += 10

z : owned int = 100
z += 1

const MAX_SIZE : int = 1024

## 2. 函数

def add(a: int, b: int) -> int =
    return a + b

def sum(nums: [int]) -> int =
    total = 0
    for n in nums:
        total += n
    return total

result = sum(1, 2, 3, 4, 5)

## 3. 结构体构造与方法示例

trait HasPosition =
    x : f64
    y : f64

struct Point =
    extend HasPosition

    def distance(self: Point, other: Point) -> f64 =
        dx = self.x - other.x
        dy = self.y - other.y
        return (dx * dx + dy * dy).sqrt()

p1 = Point:
    x = 1.0
    y = 2.0

p2 = Point:
    x = 4.0
    y = 6.0

d = p1.distance(p2)

> 注：可调用对象的参数包构造已改为 `~:` 调用构建块 / `*: ` 生成器调用构建块（见 [构建块.md](构建块.md)）；`Point:` 此处为结构体字面量构造（字段赋值块），非旧式 `name :` 调用块。

## 4. return 提前返回构建参数包

def make_point(x: int, y: int) =
    if x < 0 or y < 0:
        return Point(0.0, 0.0)
    Point(x as f64, y as f64)

p3 = make_point(-1, 5)

## 5. 结构体与方法

struct Rectangle =
    width : f64
    height : f64

    def area(self: Rectangle) -> f64 =
        return self.width * self.height

    def square(size: f64) -> Rectangle =
        return Rectangle(size, size)

rect = Rectangle(3.0, 4.0)
a = rect.area()
sq = Rectangle.square(5.0)

## 6. 枚举与模式匹配

enum Option<T> =
    None
    Some(T)

enum Result<T, E> =
    Ok(T)
    Err(E)

def divide(a: int, b: int) -> Result<int, str> =
    if b == 0:
        return Err("division by zero")
    return Ok(a / b)

r = divide(10, 2)
match r:
    case Ok(v):
        println(f"result: {v}")
    case Err(e):
        println(f"error: {e}")

## 7. trait 与 impl

trait Drawable =
    def draw(self: Self)

impl Drawable for Circle =
    def draw(self: Circle) =
        println(f"Drawing circle at ({self.x}, {self.y})")

struct Circle =
    extend HasPosition
    radius : f64

## 8. 泛型

struct Box<T> =
    value : T

    def map<U>(self: Box<T>, f: fn(T) -> U) -> Box<U> =
        return Box<U>(f(self.value))

def compare<T>(a: T, b: T) -> int
where T : Ord =
    if a < b:
        return -1
    elif a > b:
        return 1
    else:
        return 0

## 9. 控制流

def fib(n: int) -> int =
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

for i in 0..10:
    println(f"fib({i}) = {fib(i)}")

n = 0
loop:
    n += 1
    if n > 5:
        break

## 10. 字符串与容器

name = "world"
greeting = f"hello {name}"

numbers = [1, 2, 3, 4, 5]
doubled = [x * 2 for x in numbers]

scores = {"Alice": 95, "Bob": 87, "Charlie": 92}
for name, score in scores:
    println(f"{name}: {score}")

unique = {1, 2, 3, 2, 1}

## 11. 迭代器与生成器

def count_up_to(n: int) =
    for i in 1..=n:
        yield i

for v in count_up_to(5):
    println(v)

squares = numbers.iter()
    .map(|x| x * x)
    .filter(|x| x > 10)
    .collect()

## 12. 错误处理

def read_config(path: str) -> str
raises IOError =
    try:
        f = File.open(path)
        return f.read()
    catch e: IOError:
        raise e
    finally:
        println("done")

def safe_divide(a: int, b: int): int? =
    guard b != 0 else: None          # 不写 return，直接写值
    Some(a / b)

## 13. 宏

macro log_call(expr: Tokens) -> Tokens =
    return f```
        println("Calling...")
        result = $(expr)
        println("Done.")
        result
    ```

val = log_call!(add(1, 2))

## 14. 并发

async def fetch_data(url: str) -> str =
    return await http.get(url)

async def main_async() =
    a = async fetch_data("https://a.com")
    b = async fetch_data("https://b.com")
    (ra, rb) = await (a, b)
    println(ra)
    println(rb)

def main_threads() =
    h1 = spawn:
        println("from thread 1")
    h2 = spawn:
        println("from thread 2")
    h1.join()
    h2.join()

## 15. 测试

@test
def test_add() =
    assert add(2, 3) == 5

@test
def test_fib() =
    assert fib(0) == 0
    assert fib(1) == 1
    assert fib(10) == 55

## 16. 所有权转移

struct Resource =
    data : str

    def __enter__(self: Self) -> Self =
        println("acquiring resource")
        return self

    def __exit__(self: Self, error: Option<Error>) -> bool =
        println("releasing resource")
        return false

def use_resource() =
    with Resource("data") as r:
        println(f"using {r.data}")

## 程序入口

def main() =
    println("Hello, Lang-Zong!")
    println(f"1 + 2 = {add(1, 2)}")
    println(f"sum = {sum(1, 2, 3, 4, 5)}")
    println(f"distance = {d}")
    println(f"area = {a}")

    for v in count_up_to(3):
        println(f"count: {v}")

    use_resource()
```

---

## 附录：设计原则

1. **零成本抽象**：高级语法转译后无运行时开销
2. **显式优于隐式**：所有权转移、生命周期等必须显式表达
3. **命名即约定**：通过命名约定控制可见性，无需关键字
4. **单一真相源**：设计文档是语言定义的唯一权威
5. **渐进式复杂度**：核心语法简单，高级特性按需使用
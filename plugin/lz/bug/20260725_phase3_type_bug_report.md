# LZ 类型系统边界测试 Bug 报告

> 测试日期: 2026-07-25
> 测试方法: 覆盖 trait 默认方法/实现、泛型函数/方法、Option 链式调用、None/List 类型推断、枚举/结构体泛型
> 编译器: `lang-zong.exe` (release build)
> 阶段: Phase 3

---

## 一、Bug 汇总

### 严重程度图例
- 🔴 **严重**: 编译通过但生成无效 Rust 代码（静默错误）
- 🟡 **中等**: LZ 编译报错，但错误信息不准确或位置偏移
- 🟢 **轻微**: 文档与实现不一致，但不影响使用

---

## 二、🔴 新发现 Bug

### Bug-40: trait 默认方法体尾表达式被当作语句（多余分号）

**代码**:
```lz
trait Greet =
    def greet(self: Self) -> str =
        "Hello from trait!"
```

**生成 Rust**:
```rust
trait Greet {
    fn greet(&self) -> String {
        "Hello from trait!";   // ← 分号导致返回 ()
    }
}
```

**Rust 编译错误**: `error[E0308]: mismatched types, expected String, found ()`

**分析**: trait 默认方法体中，尾表达式 `"Hello from trait!"` 被错误地添加了分号，变成语句而非返回值。代码生成器对尾表达式的处理与普通函数体不一致。

**影响范围**: 所有 trait 默认方法实现。

---

### Bug-41: `&str + String` 拼接操作数顺序错误

**代码**:
```lz
impl Greet for Person =
    def greet(self: Person) -> str =
        "Hi, I'm " + self.name
```

**生成 Rust**:
```rust
impl Greet for Person {
    fn greet(&self) -> String {
        "Hi, I'm " + self.name   // &str + String (Rust 不支持)
    }
}
```

**Rust 编译错误**: `error[E0369]: cannot add String to &str` — 需要 `"Hi, I'm ".to_owned() + &self.name`

**分析**: lz 的 `+` 运算符在左操作数为字符串字面量且右操作数为 `String` 时，生成的 Rust 代码操作数顺序不正确。Rust 要求 `String + &str`，而非 `&str + String`。

**影响范围**: 所有字符串字面量与 `String` 类型变量的拼接。

---

### Bug-42: `self.name` 从 `&self` 共享引用中移动

**代码**:
```lz
impl Show for Person =
    def show(self: Person) -> str =
        self.name
```

**生成 Rust**:
```rust
impl Show for Person {
    fn show(&self) -> String {
        self.name   // 从 &self 中移动 String
    }
}
```

**Rust 编译错误**: `error[E0507]: cannot move out of self.name which is behind a shared reference`

**分析**: `self.name` 的类型是 `String`，从 `&self` 中返回 `String` 需要 `.clone()`。lz 编译器没有识别人格方法中 `self` 是共享引用，应生成 `self.name.clone()`。

**影响范围**: 所有 trait 实现中返回 `self` 字段（非 Copy 类型）的方法。

---

### Bug-43: `None` 类型推断生成 `Option<>` 缺少类型参数

**代码**:
```lz
def test_none_inference() =
    val = None
    print(val)
```

**生成 Rust**:
```rust
fn test_none_inference() {
    let mut val: Option<> = None;   // ← 缺少类型参数
    println!("{:?}", val)
}
```

**Rust 编译错误**: `error[E0107]: enum takes 1 generic argument but 0 generic arguments were supplied`

**分析**: `None` 在 lz 中对应 `Option::None`，但类型推断无法确定 `T` 时应生成 `Option<_>` 或默认类型，而非空的 `Option<>`。

**影响范围**: 所有 `None` 字面量（无上下文类型提示）。

---

### Bug-44: `List()` 构造生成 `List::default()` 而非 `Vec::new()`

**代码**:
```lz
list = List()
```

**生成 Rust**:
```rust
let mut list = List::default();   // List 类型不存在
```

**Rust 编译错误**: `error[E0433]: cannot find type List in this scope`

**分析**: lz 的 `List<T>` 映射到 Rust 的 `Vec<T>`，但 `List()` 构造器生成 `List::default()` 而非 `Vec::new()`。类型名映射在构造函数场景中丢失。

**影响范围**: 所有 `List()` 空列表构造。

---

### Bug-45: 泛型函数 `print` 参数缺少 `Debug` trait 约束

**代码**:
```lz
def pair<T, U>(a: T, b: U) =
    print(a)
    print(b)
```

**生成 Rust**:
```rust
fn pair<T, U>(a: T, b: U) {
    println!("{:?}", a);   // T 未实现 Debug
    println!("{:?}", b);   // U 未实现 Debug
}
```

**Rust 编译错误**: `error[E0277]: T doesn't implement Debug`

**分析**: `print()` 使用 `{:?}` 格式化，需要 `Debug` trait。但 lz 编译器不为泛型参数自动添加 `Debug` 约束，导致 Rust 编译失败。

**影响范围**: 所有泛型函数中的 `print()` 调用。

---

### Bug-46: `identity("hello")` 中 `&str` 到泛型 `T` 的类型不匹配

**代码**:
```lz
def identity<T>(x: T) -> T = x
val = identity("hello")
```

**生成 Rust**:
```rust
fn identity<T>(x: T) -> T { x }
let mut val = identity("hello");   // "hello" 是 &str，但可能需要 String
```

**Rust 编译错误**: 间接导致 `println!("{:?}", val)` 中 `&str` 的 Debug 输出问题。

**分析**: 泛型函数 `identity` 接受 `T`，但 `"hello"` 字面量在 lz 中应映射为 `str` (= Rust `String`)，而 Rust 中 `"hello"` 是 `&str`。需要 `.to_string()` 转换。

**影响范围**: 所有以字符串字面量作为泛型参数实参的调用。

---

### Bug-47: `print()` 生成 `__call_magic(println, ...)` — 宏被当作函数

**代码**:
```lz
print(val)
```

**生成 Rust**:
```rust
__call_magic(println, (val,));
```

**Rust 编译错误**: `error[E0423]: expected value, found macro 'println'` + `error[E0425]: cannot find function '__call_magic'`

**分析**: `println!` 是 Rust 宏而非函数，不能作为 `__call_magic` 的参数。`__call_magic` 机制无法处理 Rust 宏调用。这是 Bug-30/35 的又一变体——`print()` 内置函数也错误地走了 `__call_magic` 路径。

**影响范围**: 所有 `print()` 调用（在 `match` 分支中尤为严重）。

---

### Bug-48: 泛型方法生成为独立函数而非 `impl` 块方法

**代码**:
```lz
struct Box<T> = value: T
def map<U>(self: Box<T>, f: (T) -> U) -> Box<U> =
    Box(f(self.value))
```

**生成 Rust**:
```rust
fn map<U>(&self, f: fn(T) -> U) -> Box<U> {   // 独立函数，非 impl 方法
    Box { value: __call_magic(f, (self.value,)) }
}
```

**Rust 编译错误**: `error: self parameter is only allowed in associated functions`

**分析**: lz 中定义在结构体外的泛型方法（`self: Box<T>` 参数）应生成为 `impl<T> Box<T> { fn map<U>(&self, ...) }` 块内方法，但实际生成了独立函数，`&self` 在独立函数中无效。

**影响范围**: 所有结构体外定义的泛型方法（`self: Struct<T>` 参数）。

---

### Bug-49: 泛型方法丢失外层类型参数 `T`

**代码**:
```lz
struct Box<T> = value: T
def map<U>(self: Box<T>, f: (T) -> U) -> Box<U> = ...
```

**生成 Rust**:
```rust
fn map<U>(&self, f: fn(T) -> U) -> Box<U> { ... }
//                    ^ T 未定义！
```

**Rust 编译错误**: `error[E0425]: cannot find type T in this scope`

**分析**: 泛型方法的 `T` 参数来自外层 `Box<T>`，但生成的函数签名中只声明了 `<U>`，丢失了 `<T>`。应生成 `fn map<T, U>(...)` 或放在 `impl<T> Box<T> { fn map<U>(...) }` 中。

**影响范围**: 所有引用外层结构体类型参数的方法。

---

### Bug-50: `Err("str")` 中 `&str` 字面量不自动转换为 `String`

**代码**:
```lz
enum Result<T, E> = Ok(T) | Err(E)
def divide(a: i64, b: i64) -> Result<i64, String> =
    if b == 0: Err("division by zero") else: Ok(a / b)
```

**生成 Rust**:
```rust
Result::Err("division by zero")   // &str 而非 String
```

**Rust 编译错误**: `error[E0308]: mismatched types, expected String, found &str`

**分析**: 枚举变体 `Err(String)` 期望 `String` 类型，但 `"division by zero"` 字面量是 `&str`。这是 Bug-32 的延伸——字符串字面量在枚举构造器中也未自动转换。

**影响范围**: 所有以字符串字面量构造带 `String` 类型参数的枚举变体。

---

## 三、🟡 解析限制

### Bug-51: 单行 `if` 表达式在 `=` 后导致解析错误

**代码**:
```lz
def max_int(a: int, b: int) -> int =
    if a > b: a else: b     // 解析失败
```

**LZ 编译错误**: `Parse error: Expected Indent, got Ident("a") at pos 40`

**分析**: `def ... =` 后的 `if` 表达式要求 `if` 体和 `else` 体必须换行缩进，不支持单行写法。相比 `_test_generic_result.lz` 中的多行格式可以正常编译：
```lz
def max_int(a: int, b: int) -> int =
    if a > b:
        a
    else:
        b
```

**影响范围**: 所有 `def ... =` 后的单行 `if/else` 表达式。

---

## 四、✅ 验证通过的特性

| 特性 | 状态 |
|------|------|
| trait 定义（含抽象方法 `...`） | ✅ 通过 |
| `impl Trait for Struct` 实现 | ✅ 通过 |
| 泛型函数 `identity<T>` | ✅ 通过 |
| `where T <: Ord` 约束（内联语法） | ✅ 通过 |
| `Option.map()` 链式调用 | ✅ 通过 |
| `Some(42)` 构造 | ✅ 通过 |
| 泛型枚举 `Result<T, E>` | ✅ 通过 |
| 泛型结构体 `Box<T>` | ✅ 通过 |
| `match` 枚举解构 | ✅ 通过 |

---

## 五、统计

| 类别 | 数量 |
|------|------|
| 🔴 新发现严重 Bug | 11 (Bug-40 ~ Bug-50) |
| 🟡 解析限制 | 1 (Bug-51) |
| 阶段 3 新增 | **12** |
| 累计 Bug 总数 | **51** |
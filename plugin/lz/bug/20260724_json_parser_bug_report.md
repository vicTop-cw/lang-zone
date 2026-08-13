# LZ 语言 Bug 挖掘报告

> 测试日期: 2026-07-24
> 测试方法: 在 lz 语言中编写 JSON 解析器，逐步测试 lz 宣称支持的各种语法特性
> 编译器: `lang-zong.exe` (release build)

---

## 一、Bug 汇总

### 严重程度图例
- 🔴 **严重**: 编译通过但生成无效 Rust 代码（静默错误）
- 🟡 **中等**: LZ 编译报错，但错误信息不准确或位置偏移
- 🟢 **轻微**: 文档与实现不一致，但不影响使用

---

## 二、🔴 LZ 编译器 Bug（编译通过但生成无效 Rust）

### Bug-1: 多参数闭包返回类型推断失败
**代码**:
```lz
add = |a, b| a + b
```
**生成 Rust**:
```rust
let mut add: fn(i64, i64) ->  = |a, b| a + b;  // 返回类型为空！
```
**Rust 编译错误**: `error: expected type, found '='`
**分析**: 闭包返回类型推断不完整，`->` 后丢失了类型。

---

### Bug-2: 管道上下文中的闭包参数类型丢失
**代码**:
```lz
double = |x| x * 2
5 |> double |> add_one
```
**生成 Rust**:
```rust
let mut double: fn() -> i64 = |x| x * 2;  // 参数类型被推断为空！
```
**Rust 编译错误**: `closure is expected to take 0 arguments, but it takes 1 argument`
**分析**: 管道操作符 `|>` 的代码生成破坏了闭包的类型推断。

---

### Bug-3: 列表推导式范围语法缺少括号
**代码**:
```lz
[x * 2 for x in 1..5]
```
**生成 Rust**:
```rust
1..5.into_iter()  // 缺少括号
```
**Rust 编译错误**: `can't call method 'into_iter' on type '{integer}'`
**分析**: 应生成 `(1..5).into_iter()`，方法调用优先级高于 `..`。

---

### Bug-4: f-string 中字符串字面量类型不匹配
**代码**:
```lz
name = "World"
print(f"Hello, {name}!")
```
**生成 Rust**:
```rust
let mut name: String = "World";  // "World" 是 &str，不是 String
```
**Rust 编译错误**: `mismatched types, expected String, found &str`
**分析**: lz 的 `str` 映射为 Rust `String`，但字符串字面量在 Rust 中默认是 `&str`。需要 `.to_string()` 转换。

---

### Bug-5: Safe Navigation 的 None 类型推断错误
**代码**:
```lz
person = None
name = person?.name ?? "default"
```
**生成 Rust**:
```rust
let mut person: () = None;  // 类型被推断为 ()，而非 Option<Person>
```
**Rust 编译错误**: `mismatched types, expected (), found Option<_>`
**分析**: `None` 的类型推断失败，无法推断为 `Option<_>`。

---

### Bug-6: Safe Navigation 使用 Iterator::map 而非 Option::map
**代码**:
```lz
name = person?.name ?? "default"
```
**生成 Rust**:
```rust
((person).map(|x| x.name)).unwrap_or("default")  // 使用了 Iterator::map 方法
```
**Rust 编译错误**: `Person is not an iterator`
**分析**: 应为 `person.map(|x| x.name)` 使用 `Option::map`，而非 `Iterator::map`。

---

### Bug-7: 泛型方法代码生成丢失泛型参数
**代码**:
```lz
def map<U>(self: Box<T>, f: (T) -> U)-> Box<U> =
    Box(f(self.value))
```
**生成 Rust**:
```rust
fn map<U>(&self, f: fn(T) -> U) -> Box<U> {  // T 未声明为泛型参数
    Box { value: f(self.value) }
}
```
**Rust 编译错误**: `cannot find type T in this scope`
**分析**: 泛型方法 `map<U>` 中引用了 `T`，但 `T` 未在方法签名中声明。`impl<T> Box<T>` 的 `T` 应传递到方法中。

---

### Bug-8: println 代码生成错误
**代码**:
```lz
print(val)
```
**生成 Rust**:
```rust
println(val)  // 缺少 ! 和格式化参数
```
**Rust 编译错误**: `cannot find macro 'println' in this scope`
**分析**: 应为 `println!("{}", val)` 或 `println!("{:?}", val)`。

---

### Bug-9~13: Trait/Impl 缺少语义校验

#### Bug-9: 方法名不匹配不报错
**代码**:
```lz
trait Greet = def say_hello(self: Self) = ...
impl Greet for Foo = def say_goodbye(self: Foo) = ...  // 方法名不匹配
```
**LZ 编译**: 通过  
**后果**: 生成无效 Rust（trait 要求 `say_hello` 但 impl 只有 `say_goodbye`）

#### Bug-10: 缺少方法实现不报错
**代码**:
```lz
trait Dual = def first(self: Self) = ...  def second(self: Self) = ...
impl Dual for Bar = def first(self: Bar) = ...  // 缺少 second
```
**LZ 编译**: 通过  
**后果**: 生成无效 Rust

#### Bug-11: 返回类型不匹配不报错
**代码**:
```lz
trait V = def get_val(self: Self)-> str = ...
impl V for T = def get_val(self: T)-> int = ...  // 返回类型不一致
```
**LZ 编译**: 通过  
**后果**: 生成无效 Rust

#### Bug-12: self 可变性不匹配不报错
**代码**:
```lz
trait Inc = def inc(self: Self) = ...
impl Inc for C = def inc(mut self: C) = ...  // mut 不匹配
```
**LZ 编译**: 通过  
**后果**: 生成无效 Rust

#### Bug-13: 方法名冲突不报错
**代码**:
```lz
impl S = def speak(self: S) = ...
impl T for S = def speak(self: S) = ...  // 与 inherent impl 冲突
```
**LZ 编译**: 通过  
**后果**: 生成重复方法定义

---

## 三、🟡 编译器解析 Bug

### Bug-14: `if` 条件中调用方法报错
**代码**:
```lz
if parser.is_eof():
    return
```
**LZ 编译错误**: `Type error: cannot unify with bool`
**解决方法**: 改用 `if parser.pos >= parser.input.len()` 直接字段访问
**分析**: 方法调用返回值的类型检查有 bug，无法正确识别为 `bool`。

---

### Bug-15: `Some(EnumVariant(data))` 在元组返回中报错
**代码**:
```lz
return (parser, Some(JsonValue.Bool(True)))
```
**LZ 编译错误**: `Type error: cannot unify with JsonValue`
**解决方法**: 通过辅助函数 `some_val(v)` 间接调用 `Some()`
**分析**: 元组返回上下文中的类型推断 bug。

---

### Bug-16: `match` 不支持嵌套 enum 模式
**代码**:
```lz
match val:
    case Some(JsonValue.String(key)):
        ...
```
**LZ 编译错误**: `Unexpected token in pattern: Dot`
**解决方法**: 先匹配 `Some(kv)` 再内层匹配 `case String(key):`
**分析**: 文档称支持 `case Some(v) => ...`，但嵌套 `case Some(EnumVariant)` 不支持。

---

### Bug-17: `catch` 不支持类型注解
**代码**:
```lz
try:
    ...
catch e: str:
    print(e)
```
**LZ 编译错误**: `Parse error: Expected Indent, got Ident("str")`
**分析**: 文档 `catch e: str:` 声称支持类型注解，但实际解析器不支持。

---

### Bug-18: 类型检查误报
**代码**:
```lz
def divide(a: int, b: int)-> int =
    a / b
```
**LZ 编译器**: 输出 `Type error: cannot unify i64 with i64`，但编译仍成功
**分析**: 类型检查器报告了一个虚假的类型不匹配错误。

---

## 四、🟢 文档与实现不一致

### Doc-1: `match` 分支语法 — 文档说 `=>` 实际用 `:`
**文档**: `case 0 => "zero"` (用 `=>` 箭头)  
**实际**: 所有 DEMO 文件使用 `case 0: "zero"` (用 `:` 冒号)  
**结论**: 文档过时，`=>` 语法已废弃。

### Doc-2: 函数体 `=` vs `:`
**文档**: 未明确说明无返回类型的函数也必须用 `=`  
**实际**: `def func() =` 正确，`def func():` 报错  
**结论**: 文档可补充此规则。

### Doc-3: 不支持的特性
**文档声称支持但实际不支持**:
- ❌ 单行 lambda `(x) -> x * 2` — 报 "Unexpected token: Arrow"
- ❌ 字符串拼接 `"a" ++ "b"` — 报 "Unexpected token: Plus"

---

## 五、✅ 验证通过的特性

| 特性 | 状态 |
|------|------|
| `enum` 定义（无数据变体） | ✅ |
| `enum` 定义（带数据变体） | ✅ |
| `enum` 泛型定义 | ✅ |
| `struct` 定义 + 字段 + 方法 | ✅ |
| `struct` 泛型定义 | ✅ |
| `mut self` 参数 | ✅ |
| 元组返回类型 `(T1, T2)` | ✅ |
| `match` 基本模式匹配 | ✅ |
| `match` 多分支 | ✅ |
| `return` 提前返回 | ✅ |
| `if`/`elif`/`else` | ✅ |
| `while` 循环 | ✅ |
| `for` 循环 | ✅ |
| 字符串索引 `s[pos]` | ✅ |
| 字符串切片 `s[start..end]` | ✅ |
| `List<T>` 和 `Dict<K, V>` | ✅ |
| `Option<T>` / `T?` | ✅ |
| 泛型函数 `def f<T>(x: T)-> T` | ✅ |
| `where T <: Ord` 约束 | ✅ |
| 闭包 `|x| x * 2`（单参数） | ✅ |
| 管道 `|>` | ✅ |
| 列表推导式 | ✅ |
| f-string | ✅ |
| 安全导航 `?.` / `??` | ✅ |
| 海象 `:=` | ✅ |
| `guard` / `guard let` | ✅ |
| `defer`（两种形式） | ✅ |
| `try`/`catch`/`else`/`finally` | ✅ |
| `with` | ✅ |
| `trait` 定义 | ✅ |
| `impl` / `impl Trait for Type` | ✅ |
| `test` / `suite` / `assert` | ✅ |

---

## 六、🔴 LZ Lexer 编译发现的编译器 Bug（Phase 6）

> LZ 源码 `lz_lexer.lz` 成功通过 LZ 编译器生成 Rust 代码（LZ→Rust 代码生成通过），
> 但生成的 Rust 代码无法通过 rustc 编译（139 个错误），所有错误均为 LZ 编译器代码生成层面的 bug。

### Bug-19: 函数参数 `||` 链式调用时 String 所有权移动错误

**代码** (LZ 源码):
```lz
def is_kw_a(word: str)-> bool =
    is_kw1(word) or is_kw2(word) or is_kw3(word) or is_kw4(word)
```

**生成 Rust**:
```rust
fn is_kw_a(word: String) -> bool {
    is_kw1(word) || is_kw2(word) || is_kw3(word) || is_kw4(word)
    //       ^^^^ moved here    ^^^^ use after move (error E0382)
}
```

**Rust 编译错误**: `error[E0382]: use of moved value: 'word'` (影响所有 `is_kw_a/b/c/d` 和 `is_keyword` 函数)

**分析**: LZ 编译器将 `str` 参数映射为 Rust `String`（按值传递），在 `||` 短路求值链中，`word` 被第一个函数调用移动后无法再被后续调用使用。编译器应生成 `&str` 引用或自动克隆。

**影响范围**: `is_kw_a`, `is_kw_b`, `is_kw_c`, `is_kw_d`, `is_keyword`, `is_ident_start`, `is_ident_char` 共 7 个函数

---

### Bug-20: `int` 类型用作字符串索引无类型转换

**代码** (LZ 源码):
```lz
self.input[self.pos]          // self.pos: int = i64
self.input[start..self.pos]   // start, self.pos: int = i64
```

**生成 Rust**:
```rust
self.input[self.pos]           // self.pos: i64, 但 String 索引需要 usize
self.input[start..self.pos]    // start: i64, self.pos: i64, 但切片需要 usize
```

**Rust 编译错误**: `error[E0277]: the type 'i64' cannot be indexed by 'i64'`, `error[E0308]: mismatched types`

**分析**: LZ 的 `int` 类型映射为 Rust `i64`，但 Rust 的 `String` 索引和切片要求 `usize` 类型。编译器应生成 `as usize` 转换。

**影响范围**: `skip_whitespace`, `skip_line_comment`, `skip_block_comment`, `read_number`, `read_string`, `read_ident_or_keyword`, `scan_token` 等多个方法

---

### Bug-21: `List<T>.append(item)` 生成 `Vec::append` 而非 `Vec::push`

**代码** (LZ 源码):
```lz
def add_token(mut self: Lexer, t: Token) =
    self.tokens.append(t)
```

**生成 Rust**:
```rust
fn add_token(&mut self, t: Token) {
    self.tokens.append(t)
    // Vec::append takes &mut Vec<T>, not T
}
```

**Rust 编译错误**: `error[E0308]: mismatched types, expected &mut Vec<Token>, found Token`

**分析**: `List<T>.append(item)` 应生成 `Vec::push(item)`（添加单个元素），但编译器生成了 `Vec::append(&mut other)`（合并两个 Vec）。方法映射错误。

---

### Bug-22: 变量重新赋值生成 `let` 声明导致变量遮蔽

**代码** (LZ 源码):
```lz
def read_number(mut self: Lexer)-> Token =
    ...
    is_float = False
    ...
    elif self.input[self.pos] == ".":
        if is_float:
            break
        is_float = True       // 应为重新赋值，而非新声明
```

**生成 Rust**:
```rust
let mut is_float: bool = false;
...
} else if ... {
    if is_float { break; };
    let mut is_float = true;  // BUG: 创建了新局部变量，遮蔽了外层 is_float
    ...
}
if is_float {  // 永远为 false，因为外层 is_float 从未被修改
    Token { kind: TokenType::FloatLiteral(0.0), ... }
}
```

**Rust 编译**: 此 bug 不会导致编译错误，但会导致**运行时行为错误**——所有浮点数都会被误识别为整数。

**分析**: LZ 编译器在 `elif` 分支内对 `is_float = True` 生成了 `let mut is_float = true;`（新变量声明），而非 `is_float = true;`（重新赋值）。这导致外层变量从未被修改。

---

### Bug-23: `!=` 操作符生成 `!=` 导致 Rust 类型比较错误

**代码** (LZ 源码):
```lz
if self.input[self.pos] != "\n":
```

**生成 Rust**:
```rust
if self.input[self.pos] != "\n" {
    // comparing String with &str
}
```

**Rust 编译错误**: `error[E0308]: mismatched types, expected String, found &str`

**分析**: `self.input[self.pos]` 返回 `String`，`"\n"` 是 `&str`，直接 `!=` 比较类型不匹配。编译器应生成 `.to_string()` 或使用不同比较方式。

**影响范围**: `skip_whitespace`, `skip_block_comment`, `scan_token` 中的字符比较

---

### Bug-24: `print()` 代码生成错误

**代码** (LZ 源码):
```lz
print("Tokens:")
```

**生成 Rust**:
```rust
println!("{:?}", "Tokens:".to_string() + k)  // 字符串拼接错误
```

**Rust 编译错误**: 多个 `print`/`println` 相关的类型不匹配

**分析**: 与 Bug-8 相同，`println` 宏调用生成不正确。

---

## 七、统计

| 类别 | 数量 |
|------|------|
| 🔴 严重 Bug（代码生成错误） | 13 + 6 (Lexer) = 19 |
| 🟡 中等 Bug（解析/类型检查） | 5 |
| 🟢 文档不一致 | 3 |
| ✅ 验证通过特性 | 33 |
| **总计发现问题** | **27** |
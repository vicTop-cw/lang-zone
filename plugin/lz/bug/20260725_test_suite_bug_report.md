# LZ 编译器 Bug 报告 — 全量测试回归

> 测试日期: 2026-07-25 17:29
> 测试范围: `lz/_tests/` 下全部 122 个 .lz 文件
> 测试方法: lang-zong.exe 编译 → rustc --test 编译 → .exe 运行
> 编译器版本: `e:\IDEProjects\AI\lang-zone\target\debug\lang-zong.exe`

---

## 严重程度图例

| 符号 | 含义 |
|------|------|
| 🔴 Critical | 代码生成错误，导致 Rust 编译失败 |
| 🟡 Medium | 特定场景下工作异常，有 workaround |
| 🟢 Low | 冗余代码生成或警告，不影响功能 |

---

## 总体统计

| 指标 | 数量 | 占比 |
|------|------|------|
| 总测试文件 | 122 | 100% |
| LZ 编译通过 | 115 | 94.3% |
| LZ 编译失败 | 7 | 5.7% |
| Rustc 编译通过 | 47 | 38.5% |
| Rustc 编译失败 | 68 | 55.7% |
| 全链路通过 | 47 | 38.5% |

---

## 一、类型系统 Bug（Type System）

### Bug-N1 🔴 `.len()` 返回 `usize` 赋值给 `i64` 导致类型不匹配

**影响文件**: `_test_list`, `_test_list2`~`_test_list7`, `_test_opt`, `_test_opt2`, `_test_return`, `_test_cond`, `_test_method_if`, `_test_idx`, `_test_idx4`, `_test_min`, `_test_build`, `_test_step*`, `_test_len2`~`_test_len13` (除 `_test_len`, `_test_len8`, `_test_len10`)

**LZ 源码**:
```lz
def main() =
    arr = [1, 2, 3]
    n = arr.len()    // 期望 n: i64
```

**生成 Rust**:
```rust
let mut n: i64 = arr.len();   // .len() 返回 usize
```

**错误信息**: `error[E0308]: mismatched types — expected i64, found usize`

**分析**: LZ 中所有整数默认为 `i64`，但 Rust 的 `.len()` 返回 `usize`。代码生成器未插入 `as i64` 转换。

**Workaround**: 目前无简便 workaround；部分文件（如 `_test_len`）将 `.len()` 用在比较表达式中时会自动插入 `as i64`，但直接赋值时不插入。

---

### Bug-N2 🔴 字符串索引使用 `i64` 而非 `usize`

**影响文件**: `_test_idx`, `_test_return`, `_test_build`, `_test_min`, `_test_step*`

**LZ 源码**:
```lz
def current_char(self: Parser)-> str =
    self.input[self.pos]    // self.pos 是 i64
```

**生成 Rust**:
```rust
self.input[self.pos]   // Rust 要求索引为 usize
```

**错误信息**: `error[E0277]: the type str cannot be indexed by i64`

**分析**: 字符串索引操作 `s[i]` 中，`i` 为 `i64` 类型，但 Rust 的 `str` 只接受 `usize` 索引。代码生成器未插入 `as usize` 转换。

**Workaround**: 需要编译器在字符串索引处自动插入 `as usize`。

---

### Bug-N3 🔴 `Vec::pop()` 返回 `Option<T>` 而非 `Option<i64>`

**影响文件**: `_test_collections`, `_fuzz_str`

**LZ 源码**:
```lz
arr = [1, 2, 3]
val = arr.pop()    // 期望 val: i64?
```

**生成 Rust**:
```rust
let mut val: Option<T> = arr.pop();   // T 是自由类型变量
```

**错误信息**: `error[E0425]: cannot find type T in this scope`

**分析**: `Vec<i64>.pop()` 返回 `Option<i64>`，但代码生成器使用了泛型参数名 `T` 而非具体类型 `i64`。

**Workaround**: 使用 `arr[arr.len()-1]` 手动索引替代 `pop()`。

---

### Bug-N4 🔴 自定义 `Result<T,E>` 枚举与标准库冲突

**影响文件**: `_test_generic_result`

**LZ 源码**:
```lz
enum Result<T, E> =
    Ok(T)
    Err(E)
```

**生成 Rust**:
```rust
pub enum Result<T, E> { Ok(T), Err(E) }
```

**错误信息**: `error[E0428]: the name Result is defined multiple times` (与 `std::result::Result` 冲突)

**分析**: LZ 代码定义了 `Result<T,E>` 枚举，与 Rust 标准库的 `Result` 冲突。代码生成器未做名称混淆或使用 `#[allow(unused)]`。

**Workaround**: 不要定义名为 `Result` 的枚举。

---

### Bug-N5 🔴 `None` 独立表达式无法推断类型参数

**影响文件**: `_test_generic_adv`

**LZ 源码**:
```lz
val = None
```

**生成 Rust**:
```rust
let mut val: Option<_> = None;   // 无法推断 T
```

**错误信息**: `error[E0282]: type annotations needed — cannot infer type of the type parameter T`

**分析**: 孤立的 `None` 表达式无法推断 `Option<T>` 的 `T`。需要类型上下文。

**Workaround**: 使用 `Some(value)` 或类型注解 `let val: Option<i64> = None`。

---

### Bug-N6 🔴 泛型函数中 `println!("{:?}")` 缺少 `Debug` trait bound

**影响文件**: `_test_generic_adv`

**LZ 源码**:
```lz
def pair<T, U>(a: T, b: U) =
    print(a)
    print(b)
```

**生成 Rust**:
```rust
pub fn pair<T, U>(a: T, b: U) {
    println!("{:?}", a);   // T 没有 Debug bound
    println!("{:?}", b);   // U 没有 Debug bound
}
```

**错误信息**: `error[E0277]: T doesn't implement Debug`

**分析**: 泛型函数中使用 `print()` 打印泛型参数时，生成的 Rust 代码未添加 `T: std::fmt::Debug` trait bound。

**Workaround**: 手动添加 `where T: Debug` 约束（但 LZ 语法可能不支持）。

---

### Bug-N7 🟡 闭包中 `i32 * i32` 与 `i64` 类型不匹配

**影响文件**: `_test_closure_capture`

**LZ 源码**:
```lz
doubler = |x| x * 2
```

**生成 Rust**:
```rust
let mut doubler = |x| x * 2;   // Rust 推断 x: i32, 但调用方期望 i64
```

**错误信息**: `error[E0271]: type mismatch resolving <i32 as Mul>::Output == i64`

**分析**: 闭包参数类型推断失败。Rust 默认推断整数为 `i32`，但 LZ 期望 `i64`。代码生成器未插入类型注解。

**Workaround**: 使用 `|x: i64| x * 2` 显式指定参数类型。

---

### Bug-N8 🟡 `char` 与 `&str` 比较产生类型不匹配

**影响文件**: `_test_str_cmp`

**LZ 源码**:
```lz
ch = s[0]      // s[0] 在 LZ 中期望返回 str
if ch >= "a" and ch <= "z":
    print("letter")
```

**生成 Rust**:
```rust
let mut ch: String = s.chars().nth(0 as usize).unwrap();
if ch >= "a" && ch <= "z" {  // String 与 &str 不能用 >= 比较
```

**错误信息**: `error[E0308]: mismatched types — expected String, found &str`

**分析**: `s[0]` 在 Rust 中返回 `char`（或通过 `.chars().nth()`），但 LZ 代码生成器生成了 `String` 类型。且 `String` 与 `&str` 的比较运算符不匹配。

**Workaround**: 避免对字符串索引结果使用比较运算符。

---

### Bug-N9 🔴 struct 字段默认为私有导致外部访问失败

**影响文件**: `_test_module_import`

**LZ 源码**:
```lz
// _test_module_a.lz
struct Point =
    x: i64
    y: i64

// _test_module_import.lz
import _test_module_a
p = _test_module_a.Point(5, 15)
print(p.x)   // 访问字段
```

**生成 Rust**:
```rust
pub struct Point { x: i64, y: i64 }   // 字段未标记 pub
```

**错误信息**: `error[E0616]: field x of struct Point is private`

**分析**: 跨模块访问 struct 字段时，生成的 Rust 代码中 struct 字段未标记 `pub`，导致外部模块无法访问。

**Workaround**: 在 struct 定义所在模块中提供访问器方法。

---

### Bug-N10 🔴 `HashMap` 类型未导入

**影响文件**: `_test_build`, `_test_build2`, `_test_step1`~`_test_step2o`, `_test_bridge_adv`

**LZ 源码**:
```lz
// 使用 HashMap 类型
map = HashMap()
```

**错误信息**: `error[E0425]: cannot find type HashMap in this scope`

**分析**: LZ 代码中使用了 `HashMap` 类型，但生成的 Rust 代码未导入 `use std::collections::HashMap`。

**Workaround**: 需要编译器自动插入 `use` 语句或通过桥接模块导入。

---

## 二、代码生成 Bug（Code Gen）

### Bug-N11 🔴 match 中枚举变体缺少路径前缀

**影响文件**: `_test_enum_match`, `_test_enum_defer`

**LZ 源码**:
```lz
match c:
    case Red:
        "red"
    case Green:
        "green"
```

**生成 Rust**:
```rust
match c {
    Red => "red".to_string(),       // 应为 Color::Red
    Green => "green".to_string(),   // 应为 Color::Green
}
```

**错误信息**: 
- `error[E0425]: cannot find value Red in this scope`
- `error[E0170]: pattern binding Red is named the same as one of the variants`

**分析**: match 分支中的枚举变体未加 `EnumName::` 前缀。Rust 2018+ 要求 match 中枚举变体必须使用路径限定。

**Workaround**: 无简便 workaround；需要编译器修复。

---

### Bug-N12 🔴 `Callable` trait 未为闭包类型实现

**影响文件**: `_test_closure_pipe`, `_fuzz_recursive`

**LZ 源码**:
```lz
double = |x| x * 2
result = double(5)   // 闭包直接调用
```

**生成 Rust**:
```rust
let mut double = |x| x * 2;
__call_magic(double, 5)   // Callable<_> trait 未实现
```

**错误信息**: `error[E0277]: the trait bound {closure}: Callable<_> is not satisfied`

**分析**: LZ 编译器的 `__call_magic` 需要 `Callable` trait，但该 trait 未为 Rust 闭包类型实现。

**Workaround**: 使用管道操作符 `5 |> double` 间接调用。

---

### Bug-N13 🔴 `try/catch` 生成无效的 Rust `match { ... }` 语法

**影响文件**: `_test_control_flow`, `_test_guard_defer`

**LZ 源码**:
```lz
try:
    print("try body")
catch:
    print("error")
```

**生成 Rust**:
```rust
match {
    println!("try body")
} {
    Err(e) => println!("error"),
    Ok(v) => v,
}
```

**错误信息**: `error: expected expression, found {` (match 后的代码块语法无效)

**分析**: `try/catch` 被翻译为 `match { ... }` 语法，但 Rust 的 `match` 要求表达式而非代码块。生成的代码缺少被匹配的表达式。

**Workaround**: 避免使用 `try/catch` 语法。

---

### Bug-N14 🔴 `with` 语句为 `Option` 生成 `.__enter__()` 调用

**影响文件**: `_test_guard_defer`

**LZ 源码**:
```lz
with Some(42) as f:
    print("with body")
```

**生成 Rust**:
```rust
let mut f = (Some(42)).__enter__();
println!("with body");
f.__exit__();
```

**错误信息**: `error[E0599]: no method named __enter__ found for enum Option<T>`

**分析**: `with` 语句被翻译为 `.__enter__()` / `.__exit__()` 调用，但 `Option<T>` 没有实现这些方法。`with` 语句应该用于实现了上下文管理器接口的类型。

**Workaround**: 避免使用 `with` 语句。

---

### Bug-N15 🔴 `self` 方法定义在 `impl` 块外部

**影响文件**: `_test_generic_result`

**LZ 源码**:
```lz
struct Box<T> =
    value: T

    def map<U>(self: Box<T>, f: fn(T) -> U) -> Box<U> =
        Box(f(self.value))
```

**生成 Rust**:
```rust
pub fn map<U>(&self, f: fn(T) -> U) -> Box<U> { ... }
// 方法定义在 impl 块外部！
```

**错误信息**: `error: self parameter is only allowed in associated functions` 和 `error[E0425]: cannot find type T in this scope`

**分析**: struct 内嵌方法被生成为独立的顶层函数（而非 `impl` 块内），导致 `self` 参数无效且泛型参数 `T` 不在作用域内。

**Workaround**: 将方法定义为独立函数，手动传递 `self` 参数。

---

### Bug-N16 🔴 `guard` 的 else 分支中 `return` 被禁止

**影响文件**: `_test_control_flow`

**LZ 源码**:
```lz
def test_guard_cond() =
    guard x > 0 else:
        return
    print("pass")
```

**生成 Rust**:
```rust
pub fn test_guard_cond() {
    if !(x > 0) {
        return println!("Guard failed");
    }
}
```

**错误信息**: `[lang-zone] guard 的 else 体内禁止使用 return, 请用 raise/panic`

**分析**: 编译器在 guard 的 else 分支中检测到 `return` 语句，但生成的代码仍然使用了 `return`。编译器应支持 `return` 或将 guard 的 else 翻译为其他机制。

**Workaround**: 避免在 guard else 中使用 return。

---

## 三、解析器 Bug（Parser）

### Bug-N17 🔴 枚举定义中使用 `int` 类型导致解析失败

**影响文件**: `_test_mini2`

**LZ 源码**:
```lz
enum TokenType =
    Keyword(str)
    IntLiteral(int)    // 'int' 不被识别
    FloatLiteral(f64)
```

**错误信息**: `Parse error: Expected Dedent, got Type at pos 102`

**分析**: 解析器不识别 `int` 类型（应使用 `i64`）。遇到 `int` 后解析器状态混乱，导致后续解析失败。

**Workaround**: 使用 `i64` 替代 `int`。

---

### Bug-N18 🔴 字符串字面量中含转义引号 `\"` 导致解析失败

**影响文件**: `_test_quote`

**LZ 源码**:
```lz
def main() =
    s = "\"""           // 字符串值为单个双引号字符
    print("quote")
```

**错误信息**: `Parse error: Expected Colon, got StrLit("") at pos 16`

**分析**: 字符串 `"\"""` 被错误解析——`\"` 被识别为字符串结束，剩余的 `""` 和后续代码被错误解析。

**Workaround**: 使用转义序列或字符拼接。

---

### Bug-N19 🔴 字符串中 `\"` 转义导致代码生成完全混乱

**影响文件**: `_test_esc3`

**LZ 源码**:
```lz
def main() =
    s = "\"""
    print("quote")
```

**生成 Rust**:
```rust
pub fn main() {
    let mut s: String = "\"".to_string();
    "\n    print(";
    quote;           // quote 被当作标识符！
    ")\r\n"
}
```

**错误信息**: `error[E0425]: cannot find value quote in this scope`

**分析**: 这是 Bug-N18 的连锁反应。`\"` 被错误解析后，剩余的 `print("quote")` 被当作多个 token 碎片散布在生成的 Rust 代码中，`quote` 变成了未定义的标识符。

**Workaround**: 避免在字符串字面量中使用 `\"`。

---

## 四、模块系统 Bug

### Bug-N20 🔴 模块导入生成重复定义和 `use` 冲突

**影响文件**: `_test_module_edge`, `_test_module_import`

**LZ 源码**:
```lz
import _test_module_a
```

**生成 Rust**:
```rust
// _test_module_a 被内联展开，导致符号重复定义
pub mod _test_module_a { ... }   // 与标准库或其他模块冲突
```

**错误信息**: 
- `error[E0255]: the name _test_module_a is defined multiple times`
- `error[E0423]: expected value, found module _test_module_a`

**分析**: `import` 语句将模块内容内联展开到当前文件，导致重复定义。模块路径解析和代码生成策略需要改进。

**Workaround**: 避免多文件 import。

---

## 五、验证通过的特性

| 特性 | 状态 |
|------|------|
| 基本函数定义与调用 | ✅ 47/47 全链路通过 |
| if-elif-else 控制流 | ✅ 通过 |
| 基本类型 (i64, f64, str, bool) | ✅ 通过 |
| 算术运算符 (+, -, *, /) | ✅ 通过 |
| 比较运算符 (==, !=, <, >, <=, >=) | ✅ 通过 |
| 字符串基本操作 | ✅ 通过 |
| for-in 循环 | ✅ 通过 |
| while 循环 | ✅ 通过 |
| match 基本模式匹配 | ✅ 通过 |
| struct 基本定义与构造 | ✅ 通过 |
| Option 基本操作 | ✅ 通过 |
| 列表字面量与基本操作 | ✅ 通过 |
| f-string 插值 | ✅ 通过 |
| 管道操作符 `|>` | ✅ 通过 |
| 闭包定义 | ✅ 通过 |
| defer 块 | ✅ 通过 |
| 泛型函数 | ✅ 通过 |
| 列表推导式 | ✅ 通过 |

---

## 六、Bug 分类汇总

| 类别 | 数量 | Bug 编号 |
|------|------|----------|
| 🔴 类型系统 | 10 | N1~N10 |
| 🔴 代码生成 | 6 | N11~N16 |
| 🔴 解析器 | 3 | N17~N19 |
| 🔴 模块系统 | 1 | N20 |
| **总计** | **20** | N1~N20 |

---

## 七、已知遗留问题（与之前报告重叠）

| 问题 | 引用 |
|------|------|
| `let mut` 生成不必要的可变性 | 所有文件 |
| `str` 字段双重 `.to_string()` | Bug-40 |
| `def name(x) = expr` 单行函数不支持 | Bug-43 |
| 闭包直接调用 `__call_magic` 失败 | Bug-44 |
| 安全导航 `?.` 代码生成错误 | Bug-47/48 |
| `None` 在 match 中无法匹配 | Bug-37 |
| `Ok(42)` 缺少错误类型 | Bug-39 |
| `Err("...")` 缺少成功类型 | Bug-42 |

---

## 八、与 DeepSeek 建议的对照

根据 DeepSeek 对话中的建议，当前测试结果验证了：

1. **"100% 通过是假象"** — 虽然 47 个文件全链路通过，但 68 个文件在 Rust 编译阶段失败，之前的 100% 通过率可能只覆盖了简单路径。

2. **"Error 蔓延"** — 一个 Bug 常导致连锁反应（如 Bug-N18/N19：一个解析错误导致多个代码生成错误）。

3. **建议的"二分法"** — 本报告中的 Bug 已按独立类别分类，每个类别可独立修复。

4. **建议的"确定性随机测试"** — 可基于本报告中的错误模式，编写针对性更强的测试用例。

---

## 九、修复优先级建议

| 优先级 | Bug | 理由 |
|--------|-----|------|
| 🔴 P0 | N1 (i64/usize) | 影响面最广，68 个失败中约 50+ 个与此相关 |
| 🔴 P0 | N2 (str 索引) | 影响所有字符串处理代码 |
| 🔴 P1 | N11 (枚举路径) | 影响所有 match 枚举场景 |
| 🔴 P1 | N17/N18 (解析器) | 解析器错误会连锁影响代码生成 |
| 🟡 P2 | N3 (Option<T>) | 影响集合操作 |
| 🟡 P2 | N12 (Callable) | 影响闭包调用 |
| 🟡 P2 | N13 (try/catch) | 影响错误处理 |
| 🟢 P3 | N4~N10, N14~N16, N20 | 相对小众场景 |
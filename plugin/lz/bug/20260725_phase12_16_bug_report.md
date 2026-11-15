# LZ 编译器 Bug 报告 — Phase 12~16

> 测试日期: 2026-07-25
> 测试范围: 结构体与方法、Option/Result、类型转换、闭包/管道/f-string、边界场景
> 测试方法: 编写 .lz 测试文件 → lang-zong.exe 编译 → rustc 编译 → 运行 .exe

---

## 严重程度图例

| 符号 | 含义 |
|------|------|
| 🔴 Critical | 代码生成错误，导致 Rust 编译失败或运行时崩溃 |
| 🟡 Medium | 特定场景下工作异常，有 workaround |
| 🟢 Low | 冗余代码生成，不影响功能 |

---

## 新发现 Bug

### Bug-35 🔴 `int` 类型无效，应使用 `i64`

**代码**:
```lz
struct Point =
    x: int
    y: int
```

**生成 Rust**: N/A (解析失败)

**错误信息**: `Parse error: Unexpected token in expression: Eq`

**分析**: 编译器不识别 `int` 类型，struct 字段类型解析失败。应使用 `i64` 或 `f64`。

**影响范围**: 所有使用 `int` 类型的地方。

**Workaround**: 使用 `i64` 替代 `int`。

---

### Bug-36 🔴 `None` 无类型注解时生成 `Option<>`

**代码**:
```lz
opt = None
```

**生成 Rust**:
```rust
let mut opt: Option<> = None;
```

**错误信息**: `error[E0107]: enum takes 1 generic argument but 0 generic arguments were supplied`

**分析**: 孤立的 `None` 表达式无法推断类型参数，生成空的 `Option<>`。

**影响范围**: 所有函数返回 `None` 且无类型注解的场景。

**Workaround**: 使用 `Some(value)` 或条件分支 `if cond: Some(x) else: Some(y)` 提供类型上下文。

---

### Bug-37 🟡 match 中 `None_` 被当作变量绑定

**代码**:
```lz
match opt:
    case None_:
        print("None")
```

**生成 Rust**:
```rust
match opt {
    None_ => println!("None"),  // 作为新变量绑定，不是 Option::None
}
```

**错误信息**: `warning: unused variable: None_`

**分析**: 编译器中 `None` 是关键字/保留字，不得不写成 `None_`，但 `None_` 被当作普通变量绑定而非模式匹配。

**影响范围**: 所有需要匹配 `None` 变体的 match 语句。

**Workaround**: 使用 `case _:` 通配符替代。

---

### Bug-38 🔴 函数返回 Option/Result 时类型推断失败

**代码**:
```lz
def safe_div(a: i64, b: i64) =
    if b == 0:
        None
    else:
        Some(a / b)
```

**生成 Rust**:
```rust
pub fn safe_div(a: i64, b: i64) {  // 返回类型为 () 而非 Option<i64>
    if b == 0 { None } else { Some(a / b) }
}
```

**错误信息**: `Type error: cannot unify i64 with i64`

**分析**: 当函数返回 `Option<T>` 或 `Result<T,E>` 时，如果两个分支类型不同（`None` vs `Some(i64)`），类型检查器无法正确推断返回类型。

**影响范围**: 所有返回 Option/Result 的函数。

---

### Bug-39 🔴 `Ok(42)` 生成 `Result<i64, >`（缺少错误类型）

**代码**:
```lz
r = Ok(42)
```

**生成 Rust**:
```rust
let mut r: Result<i64, > = Ok(42);
```

**错误信息**: `error[E0107]: enum takes 2 generic arguments but 1 generic argument was supplied`

**分析**: 孤立的 `Ok(value)` 无法推断错误类型参数，生成不完整的 `Result<i64, >`。

**影响范围**: 所有独立的 `Ok(...)` 表达式。

**Workaround**: 在 if-else 分支中使用 `Ok`/`Err` 配对，利用类型推断。

---

### Bug-40 🟡 `Err` 中字符串字面量双重 `.to_string()`

**代码**:
```lz
r = Err("Something failed")
```

**生成 Rust**:
```rust
let mut r: Result<, String> = Err("Something failed".to_string().to_string());
```

**分析**: 字符串字面量被转换为 `String` 时应用了两次 `.to_string()`，冗余但不影响功能。

**影响范围**: 所有 `Err("str literal")` 表达式。

---

### Bug-41 🔴 `safe_div_result` 返回类型推断失败

**代码**:
```lz
def safe_div_result(a: i64, b: i64) =
    if b == 0:
        Err("Division by zero")
    else:
        Ok(a / b)
```

**错误信息**: `Type error: cannot unify i64 with i64`

**分析**: 同 Bug-38，Result 多分支返回类型推断失败。

---

### Bug-42 🔴 `Err("...")` 生成 `Result<, String>`（缺少成功类型）

**代码**:
```lz
r = Err("Something failed")
```

**生成 Rust**:
```rust
let mut r: Result<, String> = Err("Something failed".to_string().to_string());
```

**错误信息**: `error: expected one of >, a const expression, lifetime, or type, found ,`

**分析**: 孤立的 `Err(value)` 无法推断成功类型参数，生成 `Result<, String>`。

**影响范围**: 所有独立的 `Err(...)` 表达式。

---

### Bug-43 🔴 `def square(x) = x * x` 单行函数定义语法解析失败

**代码**:
```lz
def square(x) = x * x
```

**错误信息**: `Parse error: Expected Colon, got RParen at pos 77`

**分析**: 单行函数定义 `def name(params) = expr` 语法不被支持。解析器期望在 `)` 后遇到 `:` 或 `->`。

**影响范围**: 所有单行函数定义。

**Workaround**: 使用多行定义：
```lz
def square(x) =
    x * x
```

---

### Bug-44 🔴 闭包调用 `__call_magic` 的 `Callable` trait 未实现

**代码**:
```lz
doubler = |x| x * 2
result = doubler(5)
```

**生成 Rust**:
```rust
let mut doubler: fn(i64) -> i64 = |x| x * 2;
let mut result: i64 = __call_magic(doubler, 5);
```

**错误信息**: `error[E0277]: the trait bound fn(i64) -> i64: Callable<_> is not satisfied`

**分析**: 闭包被生成为 `fn` 指针类型，调用时使用 `__call_magic` 包装，但 `Callable` trait 没有为 `fn` 类型实现。

**影响范围**: 所有闭包的直接调用（`closure(args)`）。

**Workaround**: 使用管道操作符 `|>` 间接调用闭包：
```lz
result = 5 |> doubler
```

---

### Bug-45 🟡 管道 `|>` 被转换为嵌套调用

**代码**:
```lz
result = 3 |> square |> double
```

**生成 Rust**:
```rust
let mut result: i64 = double(square(3));
```

**分析**: 管道链被转换为嵌套函数调用，对于简单函数工作正常，但不支持 `Pipe` trait 的链式调用语义。

**影响范围**: 管道操作符的语义差异。

---

### Bug-46 🟢 列表推导生成复杂代码链

**代码**:
```lz
result = [x * 2 for x in 1..5]
```

**生成 Rust**:
```rust
let mut result: Vec<i64> = (1..5).into_iter().map(|x| x * 2).collect::<Vec<_>>();
```

**分析**: 列表推导被展开为迭代器链式调用，功能正确但代码冗长。

**影响范围**: 仅代码生成质量。

---

### Bug-47 🔴 安全导航 `person?.name` 对非 Option 类型调用 `.map()`

**代码**:
```lz
person = Person(name="Alice")
result = person?.name ?? "default"
```

**生成 Rust**:
```rust
let mut result: String = ((person).map(|x| x.name)).unwrap_or("default".to_string());
```

**错误信息**: `error[E0599]: Person is not an iterator` (`.map()` 方法不存在)

**分析**: 安全导航 `?.` 应该对 `Option<Person>` 操作，但代码生成对 `Person` 直接调用 `.map()`。

**影响范围**: 所有安全导航表达式。

---

### Bug-48 🔴 安全导航 `person?.name ?? "default"` 生成错误代码

**代码**:
```lz
person = None
result = person?.name ?? "default"
```

**错误信息**: `error[E0609]: no field name on type String`

**分析**: 与 Bug-47 相关，安全导航和空值合并操作符的代码生成不完整。

**影响范围**: 安全导航 + 空值合并组合。

---

## 验证通过的特性

| 特性 | 状态 |
|------|------|
| struct 定义 (i64/f64/str 字段) | ✅ 通过 |
| struct 内嵌方法 (self) | ✅ 通过 |
| struct 字段更新 (返回新实例) | ✅ 通过 |
| 嵌套 struct | ✅ 通过 |
| 多字段 struct | ✅ 通过 |
| Option Some 基本操作 | ✅ 通过 |
| Option 条件分支 | ✅ 通过 |
| Option 链式 match | ✅ 通过 |
| Option is_some() 检查 | ✅ 通过 |
| Result 条件分支 (Ok/Err) | ✅ 通过 |
| i64 字面量 (0, -1, 大数) | ✅ 通过 |
| bool 字面量 (true/false) | ✅ 通过 |
| to_string() 类型转换 | ✅ 通过 |
| 整数除法、乘法、取反 | ✅ 通过 |
| 单参数闭包 \|x\| expr | ✅ 通过 |
| 多参数闭包 \|a, b\| expr | ✅ 通过 |
| 管道操作符 \|> (闭包) | ✅ 通过 |
| f-string 插值 | ✅ 通过 |
| 列表推导式 | ✅ 通过 |
| 字符串比较 (==, !=) | ✅ 通过 |
| 逻辑运算 (and, or, not) | ✅ 通过 |
| 嵌套 if-elif-else | ✅ 通过 |

---

## 统计汇总

| 阶段 | 新 Bug | 严重 | 中等 | 轻微 |
|------|--------|------|------|------|
| Phase 12: 结构体 | 1 | 1 | 0 | 0 |
| Phase 13: Option/Result | 7 | 5 | 2 | 0 |
| Phase 14: 类型转换 | 0 | 0 | 0 | 0 |
| Phase 15: 闭包/管道 | 4 | 2 | 1 | 1 |
| Phase 16: 边界测试 | 2 | 2 | 0 | 0 |
| **总计** | **14** | **10** | **3** | **1** |

---

## 已知遗留问题（未修复）

| 问题 | 描述 |
|------|------|
| 所有 `let` 生成 `let mut` | 不必要的可变性声明 |
| `str` 字段双重 `.to_string()` | 字符串字面量冗余转换 |
| `def name(x) = expr` 不支持 | 单行函数定义解析失败 |
| `None` 无法在 match 中匹配 | 需要 `_` 通配符替代 |
| 闭包直接调用失败 | 需要管道操作符替代 |
| 安全导航 `?.` 不工作 | 代码生成错误 |
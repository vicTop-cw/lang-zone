# LZ 代码生成鲁棒性 + 控制流测试 Bug 报告

> 测试日期: 2026-07-25
> 测试方法: 覆盖字符串处理、所有权/移动、索引/切片、变量遮蔽、match/guard/try-catch/闭包/for
> 编译器: `lang-zong.exe` (release build)
> 阶段: Phase 4 + Phase 5

---

## 一、Bug 汇总

### 严重程度图例
- 🔴 **严重**: 编译通过但生成无效 Rust 代码
- 🟡 **中等**: LZ 编译报错，但错误信息不准确
- 🟢 **轻微**: 警告/风格问题

---

## 二、🔴 Phase 4: 代码生成 Bug

### Bug-52: `String + String + String` 链式拼接失败

**代码**:
```lz
a = "Hello"; b = " "; c = "World"
result = a + b + c
```

**生成 Rust**:
```rust
let mut result: String = a + b + c;
// (a + b) 返回 String，然后 String + c 期望 &str 但得到 String
```

**Rust 编译错误**: `error[E0308]: mismatched types, expected &str, found String`

**分析**: Rust 的 `+` 运算符签名是 `String + &str`。`a + b` 返回 `String`，链式 `(a + b) + c` 中 `c` 是 `String`，需要自动添加 `&` 借用。

**影响范围**: 所有链式字符串拼接（超过 2 个操作数）。

---

### Bug-53: `&str + String` 拼接顺序错误（Bug-41 复现）

**代码**:
```lz
result = "Name: " + name + ", Age: " + age.to_string()
```

**生成 Rust**:
```rust
let mut result: String = "Name: " + name + ", Age: " + age.to_string();
// &str + String 不合法
```

**Rust 编译错误**: `error[E0369]: cannot add String to &str`

**分析**: 与 Bug-41 相同，`&str` 字面量在 `+` 左侧时，右侧 `String` 需要 `&` 借用。

---

### Bug-54: `s[0]` 字符串索引不支持

**代码**:
```lz
s = "hello"
ch = s[0]
```

**生成 Rust**:
```rust
let mut ch = s[0 as usize];
```

**Rust 编译错误**: `error[E0277]: the type str cannot be indexed by usize`

**分析**: Rust 的 `String` 不支持 `usize` 索引（只支持范围切片）。应生成 `s.chars().nth(0)` 或 `s.as_bytes()[0]`。

**影响范围**: 所有字符串索引操作。

---

## 三、🔴 Phase 5: 控制流 Bug

### Bug-55: `try/catch` 生成错误的 `match` 语法

**代码**:
```lz
try:
    print("Try block")
catch e:
    print("Catch: ")
    print(e)
```

**生成 Rust**:
```rust
match {
    println!("Try block")
} {
    Err(e) => { println!("Catch: "); println!("{:?}", e) },
    Ok(v) => v,
}
```

**Rust 编译错误**: `error[E0308]: mismatched types, expected (), found Result<_, _>`

**分析**: `try/catch` 的代码生成逻辑完全错误。`match` 的第一个参数是表达式，但代码块 `{ ... }` 返回 `()`。应将 try 体包装为 `Result` 或使用闭包。

**影响范围**: 所有 `try/catch` 结构。

---

### Bug-56: `guard let ... else` 中 `return` 被禁止

**代码**:
```lz
guard let Some(val) = opt else:
    print("None")
    return
```

**生成 Rust**:
```rust
compile_error!("[lang-zone] guard 的 else 体内禁止使用 return, 请用 raise/panic");
let Some(val) = opt else {
    println!("None");
    return;
};
```

**Rust 编译错误**: `compile_error!` 宏强制编译失败

**分析**: 编译器检测到 `guard` 的 `else` 体中使用了 `return`，但处理方式不当：(1) 生成了 `compile_error!` 导致 Rust 必定编译失败；(2) 同时仍生成了 `let else` 代码。应该要么在 LZ 编译阶段报错，要么正确处理 `return`。

**影响范围**: 所有 `guard let ... else` 中使用 `return` 的场景。

---

### Bug-57: 捕获变量的闭包类型推断为 `fn` 指针

**代码**:
```lz
factor = 10
multiply = |x| x * factor
```

**生成 Rust**:
```rust
let mut multiply: fn(i64) -> i64 = |x| x * factor;
```

**Rust 编译错误**: `closures can only be coerced to fn types if they do not capture any variables`

**分析**: 捕获外部变量 `factor` 的闭包不能转换为 `fn` 指针。应生成 `impl Fn(i64) -> i64` 或使用 `Box<dyn Fn>`。

**影响范围**: 所有捕获外部变量的闭包。

---

### Bug-58: 闭包调用生成 `__call_magic`

**代码**:
```lz
result = multiply(5)
```

**生成 Rust**:
```rust
let mut result: i64 = __call_magic(multiply, (5,));
```

**Rust 编译错误**: `error[E0425]: cannot find function '__call_magic'`

**分析**: 闭包调用也被误识别为"可调用对象"，走了 `__call_magic` 路径。这是 Bug-30/35/47 的延续——`__call_magic` 机制在多个场景中错误触发。

**影响范围**: 所有闭包/函数指针调用。

---

## 四、✅ 验证通过的特性

| 特性 | 状态 |
|------|------|
| `match` 整数字面量匹配 | ✅ 通过 |
| `match` bool 匹配 | ✅ 通过 |
| `match` 变量绑定 | ✅ 通过 |
| `guard x > 0 else:` 条件 guard | ✅ 通过 |
| `for i in 0..3` 循环 | ✅ 通过 |
| `while` 循环重赋值 | ✅ 通过 |
| `if` 分支重赋值 | ✅ 通过 |
| `to_string()` 方法 | ✅ 通过 |
| `str()` 类型转换 | ✅ 通过 |
| `x = y` 移动语义 | ✅ 通过 |

---

## 五、统计

| 类别 | 数量 |
|------|------|
| 🔴 Phase 4 新 Bug | 3 (Bug-52 ~ Bug-54) |
| 🔴 Phase 5 新 Bug | 4 (Bug-55 ~ Bug-58) |
| 阶段 4+5 新增 | **7** |
| 累计 Bug 总数 | **58** |
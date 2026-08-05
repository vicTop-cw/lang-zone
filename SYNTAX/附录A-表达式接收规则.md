# 语法接收规则 — 哪些表达式可赋值给变量

> 规范版本: 3.3 · 基于编译器源码 · 最后校订: 2026-08-05

---

## 一、接收分类

| 类别 | 含义 |
|:---:|------|
| **必须接收** | 有返回值且**必须**绑定到变量，否则编译警告/错误 |
| **可选接收** | 有返回值，可绑定也可丢弃 |
| **拒绝接收** | 无返回值或为语句，`let x = ...` 编译错误 |

---

## 二、分类表

| 语法 | 示例 | 接收 | 须写注解 | 说明 |
|------|------|:---:|:---:|------|
| 字面量 | `x = 42` | 可选 | 否 | 类型自动推断 |
| 二元表达式 | `x = a + b` | 可选 | 否 | |
| 函数调用 | `x = fn(a)` | 可选 | 否 | 返回 `()` 时绑定为 `Unit` |
| 三元条件 | `x = a if cond else b` | 可选 | 否 | 两分支类型必须一致 |
| `loop` / `for` / `while` | `x = for i in xs: if c: break with i` | 可选 | 否 | 均为表达式，值由 `break [NAME] [v]` 决定；正常完成 → `()` |
| `block` | `x = block NAME: ...` | 拒绝 | — | 命名作用域，**无返回值**；`break NAME` value-less |
| `if/elif/else` 块 | `x = if cond: ... else: ...` | 可选 | 否 | 作为表达式时须有 `else` |
| `match` 块 | `x = match v: case p => e` | 可选 | 否 | 全部分支类型一致 |
| `try` 块 | `x = try: ... catch e: ...` | 可选 | 否 | 值 = try 体或 catch 分支；`finally` 不产生值（见 09-错误处理.md §3.2）；`else` 成功分支可为空表达式 |
| 闭包 | `f = \|x\| x + 1` | 可选 | 是 | 参数/返回类型需注解 |
| 构建块 `=:` | `x =: y + 2` | 可选 | 否 | |
| 管道 `\|>` | `x = data \|> fn` | 可选 | 否 | |
| 安全导航 `?.` | `x = obj?.field` | 可选 | 否 | 返回 `Option<T>` |
| 空值合并 `??` | `x = a ?? b` | 可选 | 否 | |
| `struct` 构造 | `p = Point(1, 2)` | 可选 | 否 | |
| `enum` 变体 | `c = Color.Red` | 可选 | 否 | |
| `Box.new` / `Rc.new` 等 | `b = Box.new(42)` | 可选 | 否 | |
| `ref` 引用 | `ref r = x` | 可选 | 否 | |
| `return` | `return expr` | 拒绝 | — | 语句，不可赋值 |
| `yield` | `yield expr` | 拒绝 | — | 语句，不可赋值 |
| `break` | `break [NAME] [value]`；`break [NAME] with value` | 拒绝* | — | 跨层由 `block` 标签提供；`break NAME v` 仅循环合法，block 不能带值；`break NAME with v` 为 block 复用（v=`__Params`），非返回值 |
| `continue` | `continue [NAME]` | 拒绝 | — | 语句；`continue NAME` 续跑标签所指循环 |
| `defer` | `defer: cleanup()` | 拒绝 | — | 语句 |
| `with` | `with f as x: ...` | 拒绝 | — | 语句 |
| `guard` | `guard cond else expr` | 拒绝 | — | `guard` 是语句，整体不可赋值；单行 `guard cond else expr` 中 `expr` 是失败动作（函数内默认按 `return expr` 处理，见 05-控制流.md §7.1） |
| `pass` | `x = pass` | 可选 | — | 求值为 `()` |
| `comptime` 表达式 | `x = comptime expr` | 可选 | 否 | 编译期求值，值固化到常量 |
| `comptime:` 块 | `x = comptime: expr` | 可选 | 否 | 编译期执行块，末尾表达式值固化 |
| `test` / `suite` | — | 拒绝 | — | 声明，不可赋值 |
| `import` | — | 拒绝 | — | 声明，不可赋值 |

> * `break [NAME] [value]` 的值由**循环表达式**接收（`let x = loop: ... break with v` / `let r = for i in xs: if c: break with i`），不可单独赋值 `let x = break v`；`break NAME v` 仅在 `NAME` 为循环时合法，block 的 `break NAME` 不带值；`break NAME with v` 属于 block 复用（v 为 `__Params`），不进入循环返回值通道（见 05b-block命名块.md §4.3）。  
> * `guard` 是语句（见 05-控制流.md §7.1），整体不可赋值 `let x = guard ...`；单行 `guard cond else expr` 中 `expr` 是失败动作——在函数体内等价 `return expr`，`break`/`continue`/`yield` 用于对应作用域。

---

## 三、注解规则

| 场景 | 是否必须注解 |
|------|:---:|
| 字面量初始化 | 否 |
| 表达式初始化 | 否 |
| 闭包参数 | **变量绑定/装饰器实参必须；作函数实参时可省略**（见 03e-closure闭包.md §三） |
| 闭包返回类型 | 可省略，由体推断 |
| struct 字段 | **是** |
| 函数参数（有类型注解） | 已注解 |
| 函数参数（无注解） | 默认 `int` 或 `@math` 泛型 |
| 函数返回类型 | 可省略（默认 `-> int`，main 默认 `-> ()`） |

```lz
// ✅ 不需要注解
x = 42
y = fib(10)
z = [1, 2, 3]

// ✅ 闭包参数必须注解
f = |x: int, y: int| x + y

// ❌ 闭包参数无注解
// f = |x, y| x + y          // 错误
```

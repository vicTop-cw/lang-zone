# LZ 编译期求值（comptime）

> 版本: 3.1 · 基于编译器源码 · 2026-07-27

`comptime` 是 LZ 中难度最高的关键字之一。它让一段代码在**编译期**执行并将**结果内联**到最终产物中，运行时无需重复计算。

---

## 一、核心机制

`comptime` 的工作流程：

```
① comptime 块/表达式 → ② 编译器将其转译为 Rust 代码
    → ③ Rust 编译成功 → ④ 启动编译产物、执行代码
    → ⑤ 捕获执行结果 → ⑥ 将结果替换回 comptime 所在位置
```

**不是"常量折叠"**。是整个 comptime 代码被单独编译成可执行文件、运行、然后把结果"烧"回源码位置。

```
运行时：     [  result  ]  ← 已经是硬编码常量，无需计算
编译期：     ① 翻译 → ② rustc → ③ 运行 → ④ 取结果
```

### 1.1 结果不一定需要变量接收

```lz
// comptime 结果直接替换到所在位置
let x = 10 + comptime 5 * 5    // 编译后：let x = 10 + 25
let s = f"build-{comptime sha256("src/")}"  // 编译后：let s = "build-a1b2c3..."

// 不需要变量接收 — 结果直接内联
let y = 42
if y > comptime:
    let data = load_config()
    data["min_value"]
    // 编译后：if y > 100 { ... }
```

---

## 二、语法形式

### 2.1 块形式

```lz
comptime:
    // 这里的代码在编译期执行
    // 结果替换 comptime 块
```

```lz
let threshold = comptime:
    let config = load_json("config.json")
    let base = config["base_size"]
    let scale = config["scale_factor"]
    base * scale
// 编译后：let threshold = 250;
```

- `comptime:` 冒号后必须**换行 + 缩进**
- 块尾表达式的值即为 comptime 结果，内联到该位置

### 2.2 表达式形式

```lz
comptime <expr>
```

```lz
let hash = comptime sha256("hello")         // 编译期算哈希
let port = comptime read_port_from_env()     // 编译期读环境变量
```

`comptime` 修饰后面的整个表达式，该表达式在编译期执行，结果内联替换。

---

## 三、变量可见性 — 核心难点

comptime 块内能访问的变量必须**在编译期已知**。运行时的变量（函数参数、局部 `let` 绑定等）在 comptime 执行时尚不存在，不可用。

### 3.1 可用 ✅

| 来源 | 示例 | 原因 |
|------|------|------|
| 字面量 | `42`, `"hello"` | 编译期自然已知 |
| `const` 常量 | `const MAX = 100` | 在编译期已固化 |
| 模块级 `const` | 跨模块常量 | 编译期全局可见 |
| 其他 comptime 结果 | `comptime 5 * 5` | 同为编译期产物 |
| `inspect::*` API | `inspect.module_info()` | 编译器内省，编译期专有 |
| 编译期函数 | `comptime def f() = ...` | 标注了 comptime 的函数 |

### 3.2 不可用 ❌

| 来源 | 示例 | 原因 |
|------|------|------|
| 函数参数 | `def f(x: int) = comptime x * 2` | `x` 运行时才传入 |
| 运行时 `let` | `let y = rand()` → `comptime y + 1` | `y` 运行时变量 |
| 运行时返回值 | `comptime some_runtime_fn()` | 函数内部可能依赖运行时状态 |
| 闭包捕获 | `comptime: let caps = captured_var` | 捕获变量运行时才存在 |

### 3.3 典型场景

```lz
// ✅ 正确：const 在编译期已知
const LIMIT = 100
let arr = [0; comptime LIMIT / 2]    // 编译后：[0; 50]

// ✅ 正确：comptime 内使用 const
comptime:
    let doubles = []
    for i in 0..LIMIT:
        doubles.push(i * 2)
    doubles  // 结果直接内联为数组

// ❌ 错误：运行时参数不可用于 comptime
def bad(x: int) -> int =
    comptime:         // 错误！x 在编译期不存在
        x * 2

// ❌ 错误：运行时变量不可用于 comptime
def also_bad() =
    let runtime_val = read_sensor()
    comptime:         // 错误！runtime_val 编译期不可见
        runtime_val + 1
```

---

## 四、comptime 块求值规则

块内支持的语法（当前编译器实现）：

### 4.1 支持 ✅

| 语法 | 说明 |
|------|------|
| 字面量 | `int`/`f64`/`bool`/`str`/`None`/`list`/`tuple`/`dict` |
| 运算 | 算术、比较、逻辑、位运算（二元+一元） |
| 条件 | `if`/`elif`/`else` 表达式 |
| 循环 | `for`（遍历 list/tuple）、`while`（有步数上限保护） |
| 变量绑定 | `let`/`const`/裸绑定，存储在 comptime 符号表 |
| 赋值 | 简单标识符赋值 |
| `return` | 提前退出 comptime 块，返回值 |
| `guard` | 守卫条件 |
| `assert`/`check` | 编译期断言 |
| `inspect::*` | 编译器内省 API（14 个函数） |
| `print` | 编译期输出（调试用） |
| `break`/`continue` | 循环控制 |

### 4.2 不支持 ❌（当前实现）

| 语法 | 原因 |
|------|------|
| 方法调用 `obj.method()` | 编译期不支持 |
| 索引 `arr[0]` | 编译期不支持 |
| `match` 表达式 | 编译期不支持 |
| 构建块 `=:`/`~:`/`*:` | 编译期不支持 |
| 路径访问 `a.b.c` | 编译期不支持 |
| 运行时函数调用 | 函数内部依赖运行时状态 |
| 访问运行时变量 | 编译期不存在 |
| 闭包 | 编译期不支持 |
| 列表推导 | comptime 不支持 |

> **注意**：以上限制部分将在自举后放宽（JIT 编译执行模型可调用任意已转译的 Rust 函数）。

### 4.5 comptime def — 编译期函数

`comptime def` 声明一个仅在编译期存在的函数，运行时不可调用：

```lz
comptime def build_table(size: int) -> List<int> =
    let table = []
    for i in 0..size:
        table.push(i * i)
    table

const LOOKUP = build_table(256)
```

与普通 `def` 的区别：
- `comptime def` 只能在 `comptime:` 块或 `const X = comptime expr` 中被调用
- 编译期函数的参数同样只能使用编译期可知的值（`const`、字面量等）
- 运行时代码中调用 `comptime def` 会触发编译错误

---

## 五、comptime 结果内联

comptime 求值成功后，结果被转换为 Rust 字面量嵌入到 .rs 文件中：

```
comptime 值         →  Rust 字面量
42                 →  42
3.14               →  3.14
true               →  true
"hello"            →  "hello"
[1, 2, 3]          →  vec![1, 2, 3]
(1, "a")           →  (1, "a")
```

**求值失败 → 编译失败**（`compile_error!`），不是运行时错误。

---

## 六、inspect 内省 API

`inspect` 是编译期专有的内省命名空间，**仅在 comptime 上下文中可用**。运行时不可调用、不生成代码。

| 函数 | 说明 |
|------|------|
| `inspect.module_info()` | 返回当前模块的结构化信息 |
| `inspect.getmodulename()` | 当前模块名 |
| `inspect.ismodule(name)` | 是否指定模块 |
| `inspect.isclass(name)` | 是否 struct/enum |
| `inspect.isfunction(name)` | 是否函数 |
| `inspect.ismethod(cls, method)` | 是否方法 |
| `inspect.signature(fn)` | 函数签名信息 |
| `inspect.function_info(fn)` | 函数详细信息 |
| `inspect.getmembers()` | 所有成员列表 |
| `inspect.getmro(cls)` | 方法解析顺序 |
| `inspect.getabstracts(cls)` | 抽象方法列表 |
| `inspect.has_field(s, f)` | 结构体字段检查 |
| `inspect.assert_module_has(target)` | 编译期成员存在断言 |
| `inspect.getsource(fn)` | 获取源码文本（需注入） |

---

## 七、典型场景

### 7.1 编译期读取配置

```lz
comptime:
    let config = load_json("app_config.json")
    assert config.contains("database"), "缺少 database 配置"
    config["database"]

// 编译后直接是 {"host": "localhost", "port": 5432} 的字面量
```

### 7.2 编译期生成查找表

```lz
const SIN_TABLE = comptime:
    let table = []
    for i in 0..360:
        table.push(sin(to_radians(i)))
    table
// 编译后 SIN_TABLE 是硬编码的 [0.0, 0.0174, 0.0348, ...]
```

### 7.3 编译期验证 + 生成

```lz
comptime:
    let members = inspect.getmembers()
    guard members.len() > 0
        else:
            print("警告：模块为空")

    for (kind, name) in members:
        print(f"  [{kind}] {name}")
```

### 7.4 comptime 表达式内联

```lz
// 编译期哈希 → 运行时无需重复计算
let checksum = f"data-{comptime sha256_file("data.bin")}.dat"

// comptime 任意表达式
let buffer = [0u8; comptime 1024 * 64]     // 编译后：[0u8; 65536]
let flag = comptime cfg("feature_x")         // 编译期条件编译
```

---

## 八、语法边界

```lz
// ❌ comptime 块冒号后不换行
comptime: x = 42             // 错误：必须换行缩进

// ❌ 访问运行时变量
def f(x: int) =
    comptime:                // 错误：x 在编译期不存在
        x * 2

// ❌ 访问运行时 let
def g() =
    let val = runtime_call()
    comptime val + 1         // 错误：val 运行时变量

// ❌ comptime 内调用运行时函数
comptime:
    let r = rand()            // 错误：rand() 运行时函数，编译期不可用

// ✅ 正确：comptime 块使用 const 和字面量
const BUFFER = 4096
comptime:
    let doubled = BUFFER * 2
    doubled                   // 结果内联为 8192
```

---

*上一章：[08-宏与编译期](08-宏与编译期.md)* · *下一章：[09-错误处理](09-错误处理.md)*

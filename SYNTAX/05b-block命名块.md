# LZ 命名块 `block` — 细则

> 规范版本: 3.3 · 设计讨论定稿 · 最后校订: 2026-08-05
> 关联文档：[05-控制流.md](05-控制流.md)、[03c-检查站.md](03c-检查站.md)、[07-模块与导入.md](07-模块与导入.md)、[01-类型系统.md](01-类型系统.md) §7.1、[附录A-表达式接收规则.md](附录A-表达式接收规则.md)、[附录B-关键字保留字符号语法边界.md](附录B-关键字保留字符号语法边界.md)

## 〇、定位速览

`block` 是 LZ 的**统一命名作用域原语**：给一段代码一个名字，使其可被 `break NAME` 跨层跳出，并可作为检查站作用域。它有**两种定义形态**：

- **plain 块** `block NAME:` —— **定义即执行**的命名作用域，名字作 `break NAME` 的标签目标。
- **checker 块** `block NAME[ps]/[chk]:` —— **惰性登记**的「函数式」逻辑单元，定义时不执行体；经四种**统一触发途径**（`^:` 标准调用 / `[(expr)]` 单行调用 / `break NAME with v` 循环体内调用 / 挂 `[chk]` 随宿主执行）运行，可跨模块导入调用，编译为 Rust `fn`。

三条铁律（贯穿全文）：

1. **block 不是表达式、无返回值**。它纯为控制流/作用域服务，`break NAME`（无 `with` 的标签式跳出）一律 value-less；`break NAME with v` 是「启动运行语句」（重入块一次并注入 `ps`），非跳出、亦非返回值，见 §4.3。
2. **带值的退出只属于循环**（loop / for / while）。`break [NAME] [v]`（无 `with`）中 `v` 是循环表达式的值；block 不能 `break NAME v`。block 的「带参复用」用 `break NAME with v`（`v` 为 `__Params`，重入块一次，不跳出循环，见 §4.3），与循环带值形式正交。
3. **`:` 不被征作标签前缀**。`block NAME:` 的 `NAME` 是普通标识符；标签命名空间与变量命名空间独立。

**一句话模型**：

> **block = 能被打标签跳出、又能当带闭包的检查站用的命名代码段。**
> **定义（2 种）**：`block NAME:`（plain，定义即执行）、`block NAME[ps]:`（checker，懒惰登记）。
> **触发（统一为「调用已定义单元」，4 种写法等价）**：`block NAME ^: (实参)`、`block NAME[(实参)]`、`break NAME with (实参)`、挂 `[chk]`。

---

## 一、概念与定位

### 1.1 它是什么

`block` 把任意语句序列包成一个**可命名、可标签、可闭包捕获**的作用域：

```lz
block A:                 // A 是块标签
    let x = 1
    block B:             // 嵌套块
        ...
        break A          // 跳出 A（及其中所有嵌套块）
```

用途：

- **多层跳出**：替代 flag 变量 / `return` 退外层。
- **检查站作用域**：`block NAME[ps: __Params]` 复用函数检查站机制（见 §五）。
- **结构化标注**：给一段逻辑起名，提升可读性。

### 1.2 它不是什么（边界）

| 不是 | 原因 / 正确替代 |
|------|----------------|
| 不是表达式（无返回值） | 想要「块产出值」用 `loop` + `break v`，或推导式 `[f(x) for x in xs]`；block 要带数据出去用捕获变量或 `ps` |
| 不是函数 | 不接收入口参数（除 `ps` 检查站通道）；**plain 块内 `return` 编译错误**（退出它用 `break NAME`），**checker 块内可 `return`**（终止本次运行，见 §4.6）；要正式传参/返回用 `def` |
| 不是 `label` 关键字 | 不引入独立 `label` 关键字；标签由 `block NAME:` 直接给出（见 §1.3） |
| 不占用 `:` 作标签前缀 | 标签由 `block NAME:` 给出；引用用 `break NAME`（见 §1.3 / §四） |

### 1.3 标签的命名形式

标签由 **`block NAME:`** 直接给出：`NAME` 即标签（普通标识符），`:` 表示块体起始（与 `for:` / `if:` 一致）。引用标签用 `break NAME` / `continue NAME`，`NAME` 是普通标识符：

```lz
block outer:
    for x in xs:
        guard !bad else break outer   // 引用块名 outer 作标签
```

- `block` 关键字同时承担「命名作用域」与「可标签跳出」。
- 不存在独立 `label` 关键字，也不使用 `:label` 前缀形式；`:` 的职责只有：块体起始（`block NAME:` / `if:` / `for:` 等）、构建块 `=:` / `^:` / `~:` / `*:`、类型注解 `x: T`、三元 `?:`。

---

## 二、语法

### 2.1 基本语法

```ebnf
block_def    := "block" [NAME] [checker_clause] [raises_clause] ":"
                block_body                       // NAME 可省略 → 无名 plain 块（匿名作用域）
block_call   := "block" NAME "^:" expr           // 标准调用：注入 __Params（^: 单值块体）
             |  "block" NAME "[" expr "]"        // 单行调用：等价 ^: 的紧凑写法
raises_clause := "raises" error_type             // 仅 checker 块可声明（plain 块不可 raises）
checker_clause := "[" NAME ":" "__Params" "]"    // 消费 ps（定义检查站参数）
                | "[" NAME "]"                   // 带检查站 NAME（引用已有检查站）
                | "[" "None" "]"                  // 显式无检查站
break_stmt   := "break" [NAME] [expr]            // 循环带值：break | break NAME | break [NAME] v（v=循环表达式值）
               | "break" [NAME] "with" expr      // with 值：无 NAME→循环体返回值；有 NAME→复用命名块，expr 为 __Params（不跳出循环）
continue_stmt := "continue" [NAME]
```

**关键规则：定义与启动分离。**

- `block_def`（带 `:` 与体）是**定义**，且只有 `block_def` 是定义。
  - **plain 块**（`block [NAME]:`，无 checker 子句；`NAME` 可省略为匿名块）：定义时**立即执行体一次**（控制流/标签用途）。
  - **checker 块**（`block NAME[ps: __Params]` / `block NAME[ps]` / `block NAME[chk]`）：**惰性登记**——定义时**不执行体**，仅在触发时运行。
- **触发 = 调用已定义单元**，无冒号、无体、无重新定义，四种写法等价（§2.4）：

| 触发写法 | 说明 |
|----------|------|
| `block NAME ^: (实参)` | 标准调用；`^:` 单值块体是入口实参，脱糖为 `__Params` 注入 |
| `block NAME[(实参)]` | 单行调用；`(实参)` 脱糖为 `__Params`（等价 `^:` 紧凑写法） |
| `break NAME with (实参)` | 循环体内调用；`(实参)` 注入，执行一次 `NAME` 的体，不跳出循环（§4.3） |
| 挂 `[chk]`（`def f[NAME]` / `block B[NAME]`） | 宿主被调用时自动执行（§5.8） |

- **`block NAME[ps] ^:`（带方括号入口）非法**：定义与启动是两种形态，不能在同一行合并；`[ps]` 属于定义（声明检查站通道），`^:` 属于触发（调用已定义单元），两者必须拆成两行：
  ```lz
  block A[ps: __Params]:     // ① 定义（惰性登记）
      print(ps.args[0])
  block A ^:                 // ② 启动（调用已定义单元）
      (0, 10)
  ```
- **`^:` 单值块体**：`^:` 后必须换行缩进写入口实参（一个表达式，如元组 `(10, 20)`），其后的缩进行才是被调用的块体——遵循构建块全局空白规则（[11-构建块.md](11-构建块.md)、[12-操作符.md](12-操作符.md) §1.18）：符号内无空格、前后须留白、冒号后换行缩进。
  ```lz
  block A ^:            // 标准调用：首行缩进的是入口实参
      (10, 20)
  block A[(30, 40)]     // 单行调用：等价 block A ^: (30, 40)
  ```
- **`^:` 与 `~:` 的分工**：`^:` 是索引构建块，对应 block 的 `[...]`（中括号）；`~:` 是调用构建块，对应函数调用的 `(...)`（圆括号）。故**函数检查站入口用 `~:`**（如 `f ~: (1, 2)` 脱糖 `__Params`），**block 检查站入口用 `^:`**，二者不混用。
- block 自身的 `break NAME` 不带 `expr`（block 无值）。带 `expr` 的 `break NAME expr` 仅当 `NAME` 标注的是循环时合法（见 §4.3）；block 的「带参复用」用 `break NAME with expr`（`expr` 为 `__Params`，重入该块一次，不跳出循环）。

### 2.2 命名与标签唯一性

- **同作用域内标签名唯一**：同一词法作用域下两个 `block` 不能同名，否则编译错误。
- **跨作用域允许重名**：不同（非嵌套或不同分支）作用域可用同名，按「最近闭合」解析。
- **标签命名空间独立**：`block A` 的 `A` 与变量 `A`、函数 `A` 互不冲突。
- **标签必须是编译期字面标识符**，不能是表达式或变量。

```lz
block A:            // OK
    block A:        // ❌ 同作用域内重名（A 已在当前作用域）
        ...
block A:            // OK：不同作用域（上一个 block A 已闭合）
    ...
```

### 2.3 缩进与作用域边界

- block 体缩进 4 空格，与 `if`/`for` 一致。
- block 内的 `let` 绑定在 block 结束时销毁（除非被外层捕获引用）。
- `break NAME` 可出现在 block 内任意深度，但 `NAME` 必须是其**词法祖先**块（见 §4.5）。

### 2.4 形态判定：定义 vs 触发（核心）

`block` 的形态由**两样东西**组合决定：**冒号体 `:` / 方括号 `[...]` 内容**。一条判定规则贯穿所有形态：

> **`:` 冒号体 = 定义；方括号内放「标识符」（`ps` / `chk`）= 声明检查站通道，放「字面量/元组」= 实参；无冒号、无重定义、仅对已定义单元注入实参 = 触发。**

| 写法 | 判定 | 语义 |
|------|------|------|
| `block :`（无名） | `:` → 定义 | 匿名 plain 块：隔离作用域，执行一次，无名字可复用 |
| `block NAME:` | `:` → 定义 | plain 块：执行一次，名字作 `break NAME` 目标；无检查站不可复用 |
| `block NAME[ps]:` / `[ps: __Params]:` / `[chk]:` | `:` + 方括号**声明** → 定义 | 懒惰登记 checker 块；体不执行，经四种触发途径运行 |
| `block NAME ^: (expr)` | 无冒号 → **触发** | 标准调用已定义块，`(expr)` 脱糖为 `__Params` 注入 |
| `block NAME[(expr)]` | 方括号内是**实参** → **触发** | 单行调用已定义块，等价 `^:` 紧凑写法 |
| `break NAME with (expr)` | 无冒号 → **触发** | 循环体内调用已定义块（不跳出），§4.3 |
| 挂 `[chk]` | 定义处引用 → **触发** | 宿主被调用时自动执行该块，§5.8 |
| `block NAME[ps] ^: (expr)` | `:` 与无冒号**混用** → **非法** | 编译错误：定义与启动分离，`[ps]` 属定义、`^:` 属触发，拆两行（§2.1） |

```lz
block :                                 // ① 定义：无名 plain 块（定义即执行一次）
    ...
block Scan:                             // ② 定义：有名 plain 块（定义即执行一次，可 break Scan）
    ...
block CountUp[ps: __Params]:            // ③ 定义：懒惰登记 checker 块（带冒号，体不执行）
    ...
block CountUp[(0,)]                     // ④ 触发：单行调用（方括号内是实参）
block CountUp ^:                        // ⑤ 触发：标准调用（^: 单值块体是入口实参）
    (0,)
```

**要点**：

- **定义只有两种**：`block [NAME]:`（plain，立即执行）与 `block NAME[ps]/[chk]:`（checker，懒惰登记）。一切**无冒号**形态都是「**调用已定义单元**」，不重新定义、不产生新标签。
- **触发四途径等价**：`^:`、`[(expr)]`、`break NAME with`、挂 `[chk]` 都是对同一已定义 checker 块的「启动运行」；写哪个取决于语境（顶层/单行/循环体内/挂宿主）。
- **`block NAME ^: (expr)` 时 `NAME` 未定义** → 编译错误（无法调用不存在的块）；必须先有 `block NAME[ps]:` 定义。
- **`break NAME with v`** 是触发的一种（重入已定义块，§4.3），与 `^:` / `[(expr)]` 同类。
- **`block NAME[ps] ^:` 非法**：定义与触发不可合并于一行，拆成 `block NAME[ps]:` + `block NAME ^: (expr)` 两行。

> **一句话记忆**：plain 块「定义即执行、仅作标签」；checker 块「**先 `:` 登记（懒惰），后四途径触发（`^:` / `[(expr)]` / `break with` / 挂 `[chk]`）**」；**定义与触发永不合并在一行**。

---

## 三、作用域与词法捕获（闭包语义）

`block` 以**闭包**方式捕获外层变量，因此无需入口参数即可读写外部环境：

```lz
let total = 0
block Scan:
    for x in items:
        if x < 0:
            break Scan         // 提前结束整个扫描
        total += x             // 捕获并修改外层 total
// total 在此可见，已被更新
```

- **捕获可读写**：被捕获的 `let`/`mut` 变量，block 内可读、可改（与函数闭包一致）。
- **不引入入口参数**：block 不接收 `(a, b)` 式形参——那会重复函数语义。唯一的「入口」是检查站通道 `ps`（见 §五），且 `ps` 仅用于 checker 场景。
- **捕获 vs 传参**：若逻辑需要显式输入/输出，用 `def` + `return`；block 的定位是「就地作用域 + 跳出」，不是计算单元。

---

## 四、break / continue 规则（核心细则）

### 4.1 无标签 `break`（退最内层循环/块）

```lz
for x in xs:
    if x > 100:
        break            // 退最内层 for
block A:
    break A             // 退 block A（命名跳出，与 §1.3 形式一致）
```

- 无 `NAME` 时，退**当前最内层**的可跳出构造（for / while / loop / block）。
- 出现在 block 内则退该 block；出现在 loop 内则退该 loop。

### 4.2 `break NAME`（命名跳出，value-less）

```lz
block A:
    for i in 0..10:
        block B:
            if bad:
                break A        // 跳出 A 及其全部嵌套（B、for 一并结束）
```

- `NAME` 须为**词法祖先**的块标签（或循环标签，见 §4.3）。
- block 的 `break NAME` **不带值**（block 无返回值）。
- 跳出后，block 内所有嵌套构造立即终止，控制流回到 `block NAME:` 之后的语句。

### 4.3 `break` 带值 / 带参的统一规则

循环（loop / for / while）是**表达式**，其值是 `break` 携带的循环值；block 是**无返回值**的命名作用域。`break` 通过两种形态携带「值 / 参」，由 `with` 关键字判别：

```lz
// 1) 循环带值（loop return value）—— 与 block 无关
let r = for i in xs:
    guard i <= 5 else break with i    // r = i（i>5 时带值跳出）；guard 单行形式，替代 if 单行；循环体内必须 else break 覆盖默认 return
let s = loop outer:
    for j in ys:
        guard !done else break outer j   // s = j 并跳出 outer（外层 loop 标签，带值合法）

// 2) break with value（无 NAME）—— 等价循环返回值，显式强调「带值」
let t = for k in zs:
    guard k >= 0 else break with -1      // t = -1（取最内层循环）

// 3) break NAME with value —— 触发已定义的 checker 块，value 是 __Params（不跳出循环）
block A[ps: __Params]:
    print(ps.args[0])
for n in 0..3:
    break A with (n,)            // 复用 A 一次：(n,) 作为 __Params 注入 ps，跑一次 A 的体
```

| 语句 | 形态 | 语义 |
|------|------|------|
| loop / for / while | `break [NAME] [v]` | `v` 是该**循环表达式**的值；正常完成 → `()`；`NAME` 指定目标循环 |
| loop / for / while | `break with v`（无 NAME） | 等价于 `break v`，显式强调循环带值，取最内层循环 |
| block NAME: | `break [NAME]` | **无值**跳出（block 无返回值）；数据靠捕获变量或 `ps` 带出 |
| block NAME（checker 块） | `break NAME with v` | **触发该命名块一次**：`v`（元组/字典/record）作为 `__Params` 注入块的 `ps`，执行一次块体。**不跳出**外层循环 |

> **铁律**：
> - `break NAME v`（无 `with`）中若 `NAME` 是 block → **编译错误**（block 无值；复用块用 `break NAME with v` 触发）。
> - `break NAME with v` 中若 `NAME` 不是已定义的 checker 块 → **编译错误**。
> - `break NAME with v` **不终止**外层循环，只是重入该命名块一次——这正是「在循环体内复用 block」的写法。
> - 要「带值跳出外层循环」，把外层写成 `loop NAME:`：`let r = loop outer: ... break outer i`。
> - `break NAME with v` 本质是**「启动运行语句」**：它不跳出、而是**再跑一次** `NAME` 的体（把 `v` 注入其 `ps`）。这与顶层的调用 `block NAME[(v)]`（§2.4）是同一类「触发块运行」动作，与循环带值语义正交。自递归（`NAME` 恰为当前块）是其合法特例，不触碰 §4.5 祖先约束。

#### 4.3.1 `break with i` vs `break i`（关键辨析）

在循环表达式里**给最内层循环带值跳出**，用 **`break with v`**（`with` 显式标记），**不要写 `break v`**：

```lz
let t = for i in 1..10:
    guard i < 5 else break with i    // ✅ t = i（i>=5 时带值跳出最内层 for）
    // break i                      // ❌ 会被解释为「退出名为 i 的 block」——
    //                                  //     break NAME 的 NAME 是标签（block/loop），不是变量
    //                                  //     i 不是 block 名 → 编译错误「找不到标签 i」
```

- **`break with v`（无 NAME）**：`v` 是**循环表达式值**，取最内层循环——这是「带值退出循环」的正规写法。
- **`break v`（无 with、无 NAME）**：语法上 `v` 会被当作 `NAME` 解析（标签引用），因此**不是**「带值退出」；若 `v` 恰好不是任何标签名 → 编译错误。要带值必须写 `break with v`。
- 例外：`break outer i`（`NAME` 是 `loop NAME:` 标签）才是「带值跳出外层循环」——此时 `NAME` 是**循环标签**而非变量。
- 嵌套 block + 循环的 break 逐层解析见 §7.6.1 综合示例。

### 4.4 `continue` / `continue NAME`

- `continue` 仅对**循环**有意义，对 block 非法（block 不是迭代器）。
- `continue NAME`：续跑标签 `NAME` 所指的循环（下一轮）。`NAME` 为 block 时编译错误。

```lz
block Outer:
    for i in 0..10:
        if i % 2 == 0:
            continue           // 续 for 下一轮
        process(i)
```

### 4.5 解析规则：最近闭合 + 祖先约束

- `break NAME` 在符号表向上查找，取**最近的同名标签**。
- `NAME` 必须处于 break 点的**词法祖先链**上；跳「兄弟块」非法：
  ```lz
  block A: ...          // A 先执行完
  block B:
      break A           // ❌ A 不是 B 的祖先，运行时早已结束
  ```
  要让 `break A` 生效，`A` 必须包住 `B`（即 `block A: block B: break A`）。
- 标签重名时按最近闭合解析，不报错（除非同作用域重名，见 §2.2）。

### 4.6 `return` 语句在 block 中的处理

`return` 终止**最近的可返回边界**（函数 `def` / 构建块 / checker 块）。plain 块不创建可返回边界。两条规则：

1. **checker 块（调用行为）体内 `return` 合法**：终止**本次块的运行/触发**（无值；block 无返回值），不穿透到外层函数/块。自递归终止依赖它（见 §7.6）：
   ```lz
   block A[ps: __Params]:
       if len(ps.args) == 0:
           return                 // 结束本次运行：ps 不再处理，直接返回
       ps.args[0] = (ps.args[0] as int) + 1
   block A ^:
       ()
   ```
2. **plain 块（定义行为）体内 `return` 编译错误**：plain 块定义即执行、不是函数边界，退出它只能用 `break NAME`；要提前退出**外层函数**，在函数体层写条件 + `return`，或用 `break NAME` + 块后判断：
   ```lz
   def process(xs):
       block scan:
           for x in xs:
               if bad(x):
                   break scan      // ✅ 退出 block：用 break
                   // return      // ❌ 编译错误：plain 块内不允许 return
       ...
   ```

边界与检查规则：

- **`return` 不带值**；`return expr` 在 checker 块内也编译错误（block 无返回值，数据靠原地改 `ps` / 捕获变量带出，见 §5.1/§5.6）。
- **checker 块内 `return` 只结束自己**：嵌套在 plain 块 / 外层函数内时，`return` 不会穿透到它们——它是 checker 块自身的边界（Rust 侧等价函数返回，见 §7.8）。
- **顶层脚本直接写 `return` 非法**（见 [附录B](附录B-关键字保留字符号语法边界.md) §四 return 行）；但**顶层定义的 checker 块**被触发时，其体内 `return` 合法（那是 checker 块自己的边界，不属于顶层脚本语句，见 [附录B](附录B-关键字保留字符号语法边界.md) §四 return 行）。

---

## 五、block 与检查站（复用 [chk] / [ps: __Params]）

「检查站」从函数专属升格为「任何能接收 `__Params` 的可命名执行作用域」。复用 [03c-检查站.md](03c-检查站.md) 的边界脱糖与 `__Params` 结构。

### 5.1 checker 块定义：`block NAME[ps: __Params]:`

块声明自己**消费** `__Params`（即「checker 块」），`ps` 是它的**定义期通道**。定义是**懒惰**的（体不执行）；**触发时** `ps` 作为 **`mut __Params`** 注入，注入来源见 §5.3：

```lz
block validate[ps: __Params]:
    if len(ps.args) == 0:
        raise ValueError("need >=1 arg")
    ps.args[0] = (ps.args[0] as int) + 1   // 原地改写（block 无返回值，故原地改 ps）
```

- `ps` 类型为 `mut __Params`；block 体可校验 / 改写 `ps` 字段。
- 省略注解 `block NAME[ps]:` 也合法——`[ps]` 出现在块上，编译器已知这是检查站通道，`ps` 推断为 `__Params`（`[ps]` ≡ `[ps: __Params]`，见 §2.4）。
- **定义即惰性**：`block NAME[ps]:` 只登记、不执行；触发一律走 §2.4 四途径，**不存在「定义并启动」合并形态**（`block NAME[ps] ^:` 非法，见 §2.1）。
- 因 block 无返回值，**用「原地修改 ps」代替函数的 `return __Params`**（见 §5.6）。

**方括号 `[ ]` 三种形态判定（与函数 `def` 一致，见 [03c-检查站.md](03c-检查站.md) §一）：**

| 写法 | 语义 | `ps_name` | `default_checker` |
|------|------|-----------|-------------------|
| `block NAME[ps: __Params]:` | 定义检查站参数，惰性登记 | `Some("ps")` | `None` |
| `block NAME[cache]:` | 引用已有检查站 `cache`，触发时先跑 `cache` 再进体 | `None` | `Some("cache")` |
| `block NAME[None]:` | 显式无检查站 | `None` | `None` |
| `block NAME:`（无 `[ ]`） | plain 块（非 checker），立即执行 | — | — |

> `block NAME[ps]:`（省略类型注解）等价于 `block NAME[ps: __Params]:`，编译器推断 `ps` 类型。

### 5.2 block 带检查站：`block NAME[chk]:`（精确写法）

与函数 `[chk]` 对称：块「带」一个检查站（函数或 checker 块），触发块时先跑它改写 `__Params`，再跑体。

```lz
def my_check(ps: __Params) -> __Params raises ValueError:
    if (ps.args[0] as int) < 0:
        raise ValueError("neg")
    ps

block A[my_check]:            // 定义（延迟）：登记一个「带 my_check 的 checker 块」
    print(ps.args[0])         // ps 已是 my_check 处理后的结果
block A ^:                    // 触发（标准调用）：先跑 my_check，再进 A 体
    (10, 20)
block A ^:                    // 再触发：喂新参数
    (30, 40)

// [None] 显式无检查站
block Fast[None]:              // 显式声明不使用检查站
    print(ps.args[0])
```

**要点**：

- `block A[my_check]:`（带 `:` 与体）是**定义（延迟）**——定义时**不执行体**、也无参数入口（`ps` 不存在）。触发（`^:` / `[(expr)]` / `break with` / 挂 `[chk]`）时才注入 `ps` 并运行。
- `[None]` 用于显式禁用检查站——当模块存在默认检查站，某个特定块需要绕过时使用。
- `block A[my_check] ^: (10, 20):`（在同一行用 `^:` 接参数并带 `:` 体）**非法**——`^:` 冒号后必须换行缩进，且「定义与触发不得合并」（§2.1/§2.4）。**正确写法**：`block A[my_check]:` 定义 + `block A ^:` 触发，两行。
- 叠加消费与带站：`block A[ps: __Params][my_check]:` + `block A ^:\n    (10, 20)` —— 先 `my_check` 处理，再进 A 体（A 体还能继续改 ps；`^:` 首行缩进的是入口实参，其后块体属已定义的 A）。

### 5.3 如何获取 Block 中的 ps（注入机制）

`ps` 的注入有**四种触发途径**（统一脱糖，[03c §4.3](03c-检查站.md)）：

| 触发途径 | `__Params` 内容 | 典型写法 |
|----------|-----------------|----------|
| `^:` 标准调用 | `^:` 单值块体（元组/字典/裸值/record 按 §4.3 打包） | `block A ^:\n    (10, 20)` |
| `[(expr)]` 单行调用 | `(expr)` 实参元组脱糖为 `__Params`（等价 `^:`，紧凑写法，§2.4） | `block A[(0,)]` |
| `break NAME with v` | `v` 作为 `__Params` 注入 `NAME` 的 `ps`（启动运行语句，§4.3） | `break A with (1,)` |
| 作为 `[chk]` 挂在宿主 | 宿主的调用实参打包成 `__Params` | `def f[A](a, b)` → A 的 ps = f 的实参 |

补充规则：

- **定义不注入**：`block A[ps]:` 只是登记，无 `__Params` 注入、体不运行；注入只发生在上述四种触发时。
- **`ps` 是 `mut __Params` 局部变量**：在 checker 块内可直接读写 `ps.args` / `ps.kwargs`。
- **嵌套块可捕获父块的 `ps`**：子 checker 块按词法闭包捕获父作用域的 `ps` 变量（它只是个变量），因此子块既能用**自己的** `ps`（由触发注入），也能直接读**父块**的 `ps`。
- **`ps` 不自动沿嵌套传递**：父块的 `ps` 不会自动成为子块的 `ps`；若要让子块基于父 `ps` 工作，要么用 `^:` 显式传 `ps`，要么在子块内通过捕获直接读父 `ps`。

```lz
block Parent[ps: __Params]:
    let parent_ps = ps                // 父 ps 可见
    block Child[ps: __Params]:        // 子块有独立 ps
        let p = parent_ps             // 同时能捕获父 ps（闭包）
        print(p.args[0], ps.args[0])  // 父 ps ; 子 ps
    block Child ^:                    // 给子块传独立 ps
        (99,)
```

**检查站实参的三种形态**（凡是「进入 checker 块 / 调用带 `[chk]` 的函数」的位置，实参必须是以下之一，编译器统一脱糖为 `__Params`）：

| 实参形态 | 说明 | 示例 |
|----------|------|------|
| `__Params` 值 | 直接构造 `__Params`（元组/字典/record/裸值，按 §4.3 打包规则） | `block A[(0, 10)]` / `block A ^:\n    (0, 10)` / `break A with (0, 10)` |
| 带检查站的 block | 一个已定义的 checker 块名作实参——把**它的运行结果**（原地改写的 `ps`）作为本次实参 | `block A[B]`（`B` 先跑、改写 `ps`，再进 A） |
| `__Params -> __Params` 函数 | 一个签名 `def f(ps: __Params) -> __Params` 的普通函数作实参 | `block A[f]` / `def g[A]`（`f` 作为检查站先处理 `ps`，见 §5.2/§5.6） |

> 三种形态都要求「可接收 `__Params` 并（可选）产出新 `__Params`」：`__Params` 值直接注入；带站 block 与 `__Params -> __Params` 函数先执行再注入其产物（checker 语义，见 §5.2 与 [03c-检查站.md](03c-检查站.md) §四）。

### 5.4 block 语句如何复用

block 不是表达式、无返回值，但其**作为检查站角色可被复用**。三种复用形态：

1. **作为 `[chk]` 挂在多处（主复用）**：一个 checker 块定义一次，可挂到任意函数/块：
   ```lz
   block norm[ps: __Params] raises ValueError:
       ps.args[0] = (ps.args[0] as int).abs()
   def div[norm](a: int, b: int) -> int = a / b
   def mul[norm](a: int, b: int) -> int = a * b   // 同一 norm 复用
   block Work[norm]:                               // 也可挂到另一个块
       ...
   ```
2. **`^:` 触发（就地复用）**：已定义的 checker 块可用 `block NAME ^:`（实参写于下一行缩进）反复执行其体（复用校验逻辑，喂新参数）：
   ```lz
   block A[ps: __Params]:             // 定义（惰性）
       print(ps.args[0])
   block A ^:                         // 触发：喂 (0,)
       (0,)
   block A ^:                         // 再触发：喂 (1,)
       (1,)
   block A[(2,)]                      // 单行触发：等价
   ```
3. **作为 break 标签（控制流复用）**：plain `block A:` 不「调用」，但其名字被多次 `break A` 引用，实现跨层跳出（见 §四）。

> 注意：内联 plain `block A:`（无 `[ps]`/`[chk]`）**只执行一次**，不是可被调用的单元；它的「复用」仅指作为跳转目标。可被「触发/重入」的只有 checker 块（`[ps]`/`[chk]` 形态），通过 `^:` / `[(expr)]` / `break NAME with` / 挂 `[chk]`（见 §2.4 四种途径）。
> 另：`^:` 触发主要用于 checker 块；若原定义内含 `break NAME`（**无 `with` 的标签式跳出**），触发时该标签不作为跳出目标——被触发的 checker 块不应依赖**标签式** `break NAME`。`break NAME with v`（§4.3）是「启动运行语句」，即触发/自递归机制本身（见 §7.6），不属于此限制。

### 5.5 嵌套 block 的复用：仅复用父块中的子块

子块在词法上属于父块，因此**只能在父块内部被 `^:` 触发或作为 `[chk]` 使用**；父块外部无法引用子块名（词法作用域）。「仅复用子块而不重跑父块」正是 checker 块的本地触发语义：

```lz
block Outer:                       // 父块，普通标签作用域，只跑一次
    block Child[ps: __Params]:     // 子 checker 块，定义在 Outer 内
        print("child got", ps.args[0])
    // 仅触发 Child（不重跑 Outer）：
    block Child ^:
        (1,)
    block Child ^:
        (2,)
    block Inner[Child]:            // 也可以把 Child 当 Inner 的检查站
        ...
// 此处 Child / Inner 均不可见（已离开 Outer 作用域）
```

要点：

- **可见性**：子块名仅在父块词法作用域内有效；要在父块外复用子块，需把子块定义提到外层作用域，或将其作为 `[chk]` 挂到外层可见的函数。
- **复用不重跑父块**：`block Child ^:` 只执行 Child 的体，Outer 的其余语句不会重跑——这正是「局部触发」。
- **父块自身的 checker 与子块独立**：若 Outer 也是 checker 块（`block Outer[ps:__Params]`），其 `ps` 与 Child 的 `ps` 是两份独立变量，子块通过捕获读父 `ps`（见 §5.3）。

### 5.6 与函数 checker 的对称与差异

| | 函数 checker | block checker |
|---|---|---|
| 声明 | `def f[chk](..)` / `def chker(ps: __Params) -> __Params` | `block A[chk]:` / `block A[ps: __Params]:` |
| 入口 `ps` | 伪参数（函数签名注入） | `mut __Params` 注入（触发时） |
| 产出参数 | `return __Params{...}` | **原地 `ps.args[..] = ...`**（无 return） |
| 复用为 `[chk]` | 是 | 是（函数/块均可挂 `[chk]`） |
| 额外能力 | 仅看参数 | 还能校验**捕获的外层变量**（闭包） |
| 触发 | 调用即触发 | `block NAME ^:` 触发 |

> block checker 比函数 checker 更灵活：它同时看到传入的 `__Params` 与捕获的外部环境，适合「进入某段逻辑前一并校验环境 + 参数」的场景。

### 5.7 编译期验证

| 检查项 | 行为 |
|--------|------|
| `block NAME[ps: T]` 且 `T != __Params` | 编译错误（检查站参数类型必须为 `__Params`） |
| `block NAME[ps]` 但体未使用 `ps` | 警告（未使用的检查站通道） |
| `block NAME[chk]` 的 `chk` 签名非 `(__Params) -> __Params` 或不是 checker 块 | 编译错误 |
| `block NAME[None] ^:` 触发 | 合法：显式跳过检查站 |
| `block NAME[ps] ^:`（定义与触发合并一行） | 编译错误（定义与触发分离，见 §2.1/§2.4） |
| `block NAME ^:` / `block NAME[(expr)]` 但 `NAME` 未定义 / 非 checker 块 | 编译错误（无法调用不存在的块） |
| `block NAME[chk]` 的 `chk` 签名非 `(__Params) -> __Params` 或不是 checker 块 | 编译错误 |
| `^:` 单值块体不是 `__Params` 形态 | 编译错误（入口实参必须可打包为 `__Params`） |
| 同作用域内 `block NAME:` 重定义（带 `:` 体） | 编译错误（同名重定义，见 §2.2） |
| 子块在父块外被引用 | 编译错误（词法作用域） |
| `break NAME v`（非 `with` 形式）且 `NAME` 是 block | 编译错误（block 无值；复用块用 `break NAME with v`） |
| `block NAME[ps: __Params]` 但 `ps` 被当作不可变却改写 | 编译错误 |
| plain 块体内出现 `return` | 编译错误（plain 块非可返回边界；退出用 `break NAME`，见 §4.6） |
| checker 块体内 `return expr`（带值） | 编译错误（block 无返回值；数据靠原地改 `ps` / 捕获变量带出，见 §4.6） |

### 5.8 block 作为 `[chk]` 挂在宿主：去除惰性、直接调用

当 block 作为检查站 `[chk]` 挂到函数 / 另一块上时，**宿主一被直接调用，检查站即同步执行**（在宿主体之前 / 之内），触发时机完全由宿主的调用决定：

```lz
block norm[ps: __Params] raises ValueError:
    ps.args[0] = (ps.args[0] as int).abs()

def div[norm](a: int, b: int) -> int = a / b   // norm 作 [chk]
// div(-3, 2) 被直接调用时，norm 同步直接运行（进入 div 体前已 abs 化）→ 惰性被去除
print(div(-3, 2))                              // 直算 -3/2（norm 已生效）
```

对比：

| 形态 | 是否懒惰 | 触发方式 |
|------|----------|----------|
| `block NAME[ps: __Params]:`（孤立 checker 块） | **懒惰** | 须 `^:` / `[(expr)]` / `break NAME with` 显式触发，否则永不运行 |
| `block NAME[chk]` 挂到 `def f[NAME]` | **直接调用（去惰性）** | `f(...)` 一调用，`NAME` 立即随宿主运行 |

> 即：block 作为「可复用检查站」挂在宿主上时，其运行完全跟随宿主调用，**懒惰性被宿主的直接调用消除**。

---

## 六、与 for 守卫、loop 值语义的分工

三者正交，互不替代：

| 机制 | 语义 | 典型写法 |
|------|------|----------|
| **for 守卫** `for x in xs if cond:` | 跳过本轮（continue 语义），不污染体 | `for x in xs if x > 0: use(x)` |
| **block + break NAME** | 多层跳出、带标签控制流（无值） | `block A: ... break A` |
| **loop/for/while + break v** | 循环产出聚合值（带值退出） | `let r = loop: ... break acc` |

- 守卫吸收「条件跳过」，block 吸收「多层跳出」，loop 吸收「带值聚合」——常见「乘积+阈值停」可纯守卫化，连 `break` 都不需要。
- block 要带数据出去：用捕获变量（推荐）或写入 `ps`（checker 块场景），不能用 `break NAME v`（但可用 `break NAME with v` 触发块并注入 `__Params`，见 §4.3）。

---

## 七、完整示例

### 7.1 用户原例改写（block 无值 + checker 块）

原伪代码 `break A {"name":"Bob"}` 因 block 无值需调整：

```lz
cond = False
let out: Dict<str, str> = {}        // 捕获变量带出数据

block A1:                            // ① 普通命名作用域（plain，定义即执行）
    ...

block A2[ps]:                        // ② checker 作用域定义（ps 推断为 __Params，惰性）
    ...

block A[ps: __Params]:               // ②' checker 作用域定义（显式注解，惰性）
    if len(ps.kwargs) == 0:
        cond = True
    else:
        cond = !cond
    block B:
        ...
        block C:
            if cond:
                out = {"name": "Bob"} // 数据写入捕获变量
                break A               // 跳出 A/B/C，value-less
// out 在此可见
```

### 7.2 多层跳出（纯控制流）

```lz
block outer:
    for i in 0..10:
        for j in 0..10:
            if i * j > 50:
                break outer          // 跳出 outer（含两层 for）
            print(i, j)
```

### 7.3 带值跳出外层循环

```lz
let found = loop outer:              // 外层用 loop 才能带值
    for i in matrix:
        for j in row:
            if j == target:
                break outer (i, j)   // found = (i, j)
```

### 7.4 checker 块改写参数

```lz
block norm[ps: __Params] raises ValueError:
    if (ps.args[0] as int) < 0:
        raise ValueError("neg")
    ps.args[0] = (ps.args[0] as int).abs()   // 原地归一化

def div[norm](a: int, b: int) -> int = a / b   // 复用 block 作检查站
```

### 7.5 复用与嵌套复用的完整示例

结合 §5.4 / §5.5：一个 checker 块挂到多个宿主，并在父块内被其子块 `^:` 触发。

```lz
// ① 定义一个 checker 块（归一化第一个参数），可多处复用
block norm[ps: __Params] raises ValueError:
    if (ps.args[0] as int) < 0:
        raise ValueError("neg")
    ps.args[0] = (ps.args[0] as int).abs()

// ② 挂到多个函数 —— 同一 norm 复用
def div[norm](a: int, b: int) -> int = a / b
def mul[norm](a: int, b: int) -> int = a * b

// ③ ^: 触发（复用校验逻辑，喂新参数）
block norm ^:
    (-5,)        // 打印/使用 abs 后的 5
block norm ^:
    (3,)         // 复用第二次

// ④ 父块 + 仅复用子块（不重跑父块）
block Outer:
    block Child[ps: __Params]:
        print("child:", ps.args[0])
    block OuterBody[Child]:   // Outer 自己的逻辑也用 Child 作检查站
        print("outer done")
    block Child ^:
        (1,)       // 仅跑 Child，Outer 其余语句不重跑
    block Child ^:
        (2,)
// Child / OuterBody 在 Outer 外不可见
```

### 7.6 自递归 block（压栈递归，演示「启动运行语句」）

checker 块可在体内用 `break NAME with v` 重入自己，配合顶层的单行调用 `block NAME[(v)]` 即可形成递归。本质是**压栈递归**（每轮 `break NAME with` 压一层栈帧），终止靠某轮不再重入并 `return`，随后**整条栈逐层解开**——它不是常量空间迭代：

```lz
out = []
block CountUp[ps: __Params]:                // ① 定义（checker 块，懒惰，可重入）
    n = ps.args[0] as int
    if n > 5:
        return                              // 终止：停止重入，整条栈逐层返回
    out.append(n)
    break CountUp with (n + 1,)             // ② 启动运行语句：重入自己（压栈），(n+1,) 注入新 ps
block CountUp[(0,)]                         // ③ 触发（单行调用）：等价 block CountUp ^: (0,)
// 执行后 out = [0, 1, 2, 3, 4, 5]
```

执行流与注意：

- 启动 `CountUp[(0,)]` → `n=0` 入 `out`，`break CountUp with (1,)` 重入（`n=1` 压栈）→ … → `n=6` 时 `return`，整条递归栈逐层解开。
- **压栈代价**：每轮 `break CountUp with` 叠一层调用栈，`N` 很大（如百万级）时会**栈溢出**；真循环 `loop`/`for`/`while` 是常量空间迭代，不叠栈。
- **自递归合法且干净**：`break CountUp with v` 只是 `break NAME with v`（§4.3）的自指特例（`NAME` 恰为当前块），不触碰 §4.5「祖先约束」（当前块必在作用域内）；互递归（A 跳兄弟 B）才存在 §4.5 歧义。
- **`return` 是「块级 return」**：checker 块体内的 `return` 表示**结束当前这次块的运行/触发**（整条递归栈逐层解开），不是退出外围函数、也不是顶层脚本退出（顶层脚本直接写 `return` 仍非法，见 [附录B](附录B-关键字保留字符号语法边界.md) §四 return 行）——块内 `return` 只作用于该块自身（规则见 §4.6）。
- **`break CountUp with (n + 1,)` 即「启动运行语句」**：它不跳出、而是再跑一次 `CountUp` 并把 `(n+1,)` 注入其 `ps`——与顶层的 `block CountUp[(0,)]`（§2.4）是同一类「触发块运行」动作。

### 7.6.1 综合嵌套示例（定义 + 触发 + 内层 for 带值）

把 §2.4 的定义/触发形态、§4.3 的 break 解析、§5.3 的检查站实参组合在一个嵌套例子里：

```lz
block outer_block_name[ps1: __Params]:     // ① 定义：checker 块（懒惰登记）
    for i in 1..10:
        break outer_block_name             // ② 直接退外层块：终止 outer_block_name 体，后续不运行

    block inner_block:                     // ③ 内层 plain 块（仅隔离/标签）
        for i in 1..10:
            break                          // ④ 仅退最内层 for

    t = for i in 1..10:
        if i < 5:
            break with i                   // ⑤ 仅退最内层 for，带值 i（不能写 break i——会被当标签）
        elif i == 8:
            break outer_block_name with (0, 1)  // ⑥ 触发 outer_block_name：(0,1) 作 ps1
        else:
            break inner_block              // ⑦ 退到 inner_block
        else:
            10                              // ⑧ for 正常走完（无 break）→ t = 10

block outer_block_name ^:                  // ⑨ 触发（标准调用）：入口实参喂 ps1
    (0,)
```

执行要点（逐条解析 break 的层级）：

- ② `break outer_block_name`：value-less 跳出——**终止外层块体**，连 ③ 的 `inner_block` 也不会执行。
- ④ `break`（无标签）：取**最内层** for。
- ⑤ `break with i`：给最内层 for 带值；`break i` 不合法（`i` 不是标签名，见 §4.3.1）。
- ⑥ `break outer_block_name with (0, 1)`：**触发**外层块（不跳出循环），`(0,1)` 脱糖为 `ps1`——复用该 checker 块。
- ⑧ `for ... else:`：循环正常完成时 `else` 表达式作循环值。
- ⑨ 顶层再触发一次：喂 `(0,)`，重跑 `outer_block_name` 体。

### 7.7 跨模块：导入 checker 块、另一模块调用

checker 块可像函数一样**跨模块复用**。分两个层次：

- **LZ 模块间调用（默认，无需 `@export`）**：模块顶层定义的 checker 块与其 `def` 一样**默认对 `import` / `from ... import` 可见**（见 [07-模块与导入.md](07-模块与导入.md) §二/§三）。调用模块引入后按 §2.4 的四种触发途径使用。
- **FFI 导出（`@export(Rust)` / `@export(Python)`，可选）**：若要暴露给**外部 Rust crate / Python** 直接调用，才需在定义上加 `@export` 标注（复用 [07-模块与导入.md](07-模块与导入.md) §4.3 机制——本规范将其扩展至 checker 块，因 checker 块在 codegen 层与函数等价，见 §7.8）。`@export` 裸用非法，必须指定目标（见 [03e-复合综合.md](03e-复合综合.md) §四）。

```lz
// checks.lz —— 定义模块（顶层）
block norm[ps: __Params] raises ValueError:
    ps.args[0] = (ps.args[0] as int).abs()

block clamp01[ps: __Params]:
    let v = ps.args[0] as int
    guard v >= 0 else:
        ps.args[0] = 0
        return
    guard v <= 100 else:
        ps.args[0] = 100
        return
    ps.args[0] = v
```

```lz
// app.lz —— 调用模块
import checks                       // 或 from checks import norm, clamp01

// ① 作 [chk] 挂到函数/块（自动去惰，见 §5.8）
def div[norm](a: int, b: int) -> int = a / b
print(div(-3, 2))                   // norm 随宿主调用自动运行

// ② 显式触发：^: 或单行调用
block norm ^:                       // 标准调用：喂 (-5,)，ps.args[0] → 5
    (-5,)
block clamp01[(101,)]               // 单行调用：ps.args[0] → 100

// ③ 启动运行语句：循环体内复用
for n in 0..3:
    break clamp01 with (n * 50,)    // 每次重入 clamp01 归一化一次
```

跨模块要点：

- **可见性**：模块顶层的 checker 块对 `import`/`from` 默认可见（与 `def` 同规则），无需 `@export`；`@export` 仅用于暴露给外部 Rust/Python。
- **惰性跨模块保持**：导入的 checker 块**依然是惰性的**——`import` 不会触发它；只有调用方显式触发（`^:` / `[(expr)]` / `break with`）或挂 `[chk]` 被宿主调用时才运行（见 §5.8 去惰）。
- **子块不可跨模块**：嵌套在父块体内的子 checker 块受词法作用域约束，**不能**被其它模块引用（§5.5）；要跨模块复用必须定义在模块顶层。
- **同名冲突**：`from checks import norm` 与本模块其他 `norm` 冲突时编译错误（与函数导入同规则，见 07 §2.3）。

### 7.8 Rust 侧行为：block 编译为 Rust `fn`

checker 块在 codegen 层的定位**等价于一个 Rust 函数**——「惰性」只是 LZ 源语言层面的语法糖（登记/触发），编译后无「惰性」残留：

- **plain 块** → 编译为 Rust 内联块 `{ ... }`（定义即执行，无函数边界）。
- **checker 块** → 编译为 **Rust `fn`**，签名形如：
  ```rust
  // block norm[ps: __Params] raises ValueError: 生成（以下为编译器生成的 Rust 代码，非 LZ）：
  fn norm(ps: &mut __Params) -> () {
      // 体：改写 ps.args[0] 等
  }
  ```
  因 `__Params` 可变借用（`mut __Params`），body 对 `ps` 的原地改写直接作用于入参。

四种触发途径对应的 Rust 代码形态：

| LZ 触发 | Rust 侧等价物 |
|---------|---------------|
| `block NAME[ps: __Params]:`（定义） | 生成 `fn name(ps: &mut __Params)`（不立即调用） |
| `block NAME ^:` / `block NAME[(expr)]` | 生成 `name(&mut __params)`（显式调用一次） |
| 挂 `[chk]`（`def f[norm]` / `block B[norm]`） | 函数签名注入检查站指针 `ps: Option<fn(&mut __Params)>`，宿主调用时 `norm` 作为 `Some(norm)` 传入并执行（见 [03c-检查站.md](03c-检查站.md) §五） |
| `break NAME with v` | 等价 `name(&mut __params)`（重入调用） |

因此：

- **从 Rust 侧看，checker 块就是一个普通函数**：可被其它 LZ 函数/块调用、可挂作检查站、也可经 `@export`（§7.7）暴露给外部 crate 直接调用。
- **「惰性」是源语言概念，不是运行时概念**：编译产物里只有 `fn` 与调用点，没有「延迟执行/登记表」之类的运行时机制。
- **自递归压栈（§7.6）在 Rust 侧就是普通函数递归**：`break CountUp with (n+1,)` 编译为 `count_up(&mut params)` 自调用，压栈语义与 Rust 函数递归一致（大 N 会栈溢出）。

---

## 八、编译期验证汇总

| 检查项 | 行为 |
|--------|------|
| 同作用域 `block` 同名 | 编译错误 |
| `break NAME` 的 `NAME` 非词法祖先 | 编译错误（含跳兄弟块） |
| `break NAME v`（非 `with` 形式）且 `NAME` 是 block | 编译错误（复用块用 `break NAME with v`） |
| `continue` / `continue NAME` 出现在 block（非循环） | 编译错误 |
| `block NAME[ps: __Params]` 但 `ps` 未声明 `mut` 却改写 | 编译错误 |
| `block NAME[chk]` 的 chk 签名不符 | 编译错误 |
| `block NAME[ps] ^:`（定义与触发合并一行） | 编译错误（定义与触发分离，见 §2.1/§2.4） |
| 触发 `block NAME ^:` / `block NAME[(expr)]` 但 `NAME` 未定义/非 checker 块 | 编译错误 |
| `^:` 单值块体不是 `__Params` 形态 | 编译错误 |
| 同作用域内 `block NAME:` 带 `:` 体重定义 | 编译错误 |
| 子块在父块外被引用 | 编译错误（词法作用域） |
| plain 块体内出现 `return` | 编译错误（plain 块非可返回边界；退出用 `break NAME`，见 §4.6） |
| checker 块体内 `return expr`（带值） | 编译错误（block 无返回值；数据靠原地改 `ps` / 捕获变量带出，见 §4.6） |
| `@export(Rust/Python)` 标注非 checker 块（plain 块/嵌套子块） | 编译错误（FFI 导出仅限模块顶层 checker 块，§7.7）；`@export` 裸用非法（见 03e §四） |
| `import`/`from` 引入不存在的导出 checker 块 | 编译错误（与函数导入同规则，07 §五） |
| 惰性 checker 块从未被任何途径触发 | 编译警告（死代码提示；`^:` / `[(expr)]` / `[chk]` / `break with` 四种途径均未出现） |

---

## 九、`:` 的职责清单（呼应附录 B）

`:` 的语法职责如下，标签引用不使用 `:` 前缀（见 §1.3）：

| `:` 用法 | 说明 |
|----------|------|
| 块体 `for ..:` / `if ..:` / `block NAME:` | 块体起始 |
| 构建块 `=:` `^:` `~:` `*:` | 构建块标记 |
| 类型注解 `x: T` | 类型注解 |
| 三元 `a if cond else b` | 三元表达式 |
| **`break :label` 标签前缀** | ❌ 改 `block NAME:` + `break NAME` |

---

## 十、常见错误与修正

```lz
// ❌ 跳兄弟块
block A: ...
block B: break A
// ✅ A 必须包住 B
block A: block B: break A

// ❌ block 带值跳出
break A {"name": "Bob"}
// ✅ 捕获变量 + value-less break
out = {"name": "Bob"}; break A

// ❌ 定义与触发合并一行
block A[ps: __Params] ^:
    (0,)
    print(ps.args[0])
// ✅ 拆两行：先定义（惰性登记），后触发（标准调用）
block A[ps: __Params]:
    print(ps.args[0])
block A ^:
    (0,)

// ❌ 触发不存在的块
block A ^:
    (0,)
// ✅ 先定义 checker 块，再触发
block A[ps: __Params]:
    ...
block A ^:
    (0,)

// ❌ :label 前缀
:outer for ...: break :outer
// ✅ block 标签（内层 for 用 guard 单行早退，for 冒号须换行缩进）
block outer:
    for x in xs:
        guard !bad else break outer

// ❌ 在 block 里 continue
block A: continue
// ✅ continue 仅用于循环（guard 单行早退，替代 if 单行）
for x in xs:
    guard !bad else continue
```

# LZ 命名块 `block` — 细则

> 规范版本: 1.0 · 设计讨论定稿 · 最后校订: 2026-08-04
> 关联文档：[05-控制流.md](05-控制流.md)、[03c-检查站.md](03c-检查站.md)、[01-类型系统.md](01-类型系统.md) §7.1、[附录A-表达式接收规则.md](附录A-表达式接收规则.md)、[附录B-关键字保留字符号语法边界.md](附录B-关键字保留字符号语法边界.md)

## 〇、定位速览

`block` 是 LZ 的**统一命名作用域原语**：给一段代码一个名字，使其可被 `break NAME` / `continue NAME` 跨层跳出/续跑，并可作为检查站作用域。

三条铁律（贯穿全文）：

1. **block 不是表达式、无返回值**。它纯为控制流/作用域服务，`break NAME` 一律 value-less。
2. **带值的退出只属于循环**（loop / for / while）。`break [NAME] [v]` 中 `v` 是循环表达式的值；block 不能 `break NAME v`。
3. **`:` 不被征作标签前缀**。`block NAME:` 的 `NAME` 是普通标识符；标签命名空间与变量命名空间独立。

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
| 不是函数 | 不接收入口参数（除 `ps` 检查站通道）、不 `return`；要传参/返回用 `def` |
| 不是 `label` 关键字 | 不引入独立 `label` 关键字；标签由 `block NAME:` 直接给出（见 §1.3） |
| 不占用 `:` 作标签前缀 | `break :label` 语法废弃，改 `break NAME`（见 §四、附录 B） |

### 1.3 为什么用 `block NAME:` 而非 `label NAME:` 或 `:label`

| 方案 | 问题 |
|------|------|
| `:outer for ...: break :outer` | `:` 已一身兼数职（块体、构建块 `=:`/`^:`/`~:`/`*:`、类型注解 `x: T`、三元 `?:`），再当标签前缀超载；且与构建块 `=:` 视觉撞车 |
| `label outer: for ...: break outer` | 多一个关键字；`label` 只服务于「贴标签」，语义单薄 |
| **`block outer: for ...: break outer`** ✅ | 复用统一作用域关键字 `block`；标签即块名；`break NAME` 的 `NAME` 是普通标识符，`:` 仅在 `block NAME:` 表示块体起始（与 `for:`/`if:` 一致） |

结论：`block` 关键字自身承担「命名作用域 + 可标签跳出」，无需新关键字、不挤占 `:`。

---

## 二、语法

### 2.1 基本语法

```ebnf
block_def      := "block" NAME [checker_clause] [entry_clause] ":"
                  block_body
block_invoke   := "block" NAME entry_clause     // 复用重入：无体，不重新定义
checker_clause := "[" NAME ":" "__Params" "]"   // 消费 ps（省略注解写为 [ps]）
                | "[" NAME "]"                  // 带检查站 NAME（函数或 checker 块）
                | checker_clause checker_clause // 可叠加，如 [ps: __Params][my_chk]
entry_clause   := "^:" expr                     // 传参进入（^: 对应 block 的 [...] 中括号）
break_stmt     := "break" [NAME] [expr]         // NAME: 命名跳出；expr: 仅当 NAME 指向循环
continue_stmt  := "continue" [NAME]
```

注意：
- `block_def` 带 `:` 与体，是**定义**。**plain 块**（`block NAME:`，无 checker 子句）定义时**立即执行体一次**（控制流/标签用途）；**checker 块**（`block NAME[ps: __Params]` 或 `block NAME[chk]`）是**延迟定义**——定义时**不执行体**，仅在通过 `^:` 进入或作为 `[chk]` 挂到宿主被触发时才执行。`block_invoke`（`block NAME ^:`，无体）是**复用重入**已存在的 checker 块。
- **构建块符号 `^:` 遵循全局空白规则（[11-构建块.md](11-构建块.md)、[12-操作符.md](12-操作符.md) §1.18）**：符号内无空格、前后须留白，且**冒号后必须换行缩进**——入口参数作为 `^:` 的**单值块体**（一个表达式，如元组 `(10, 20)`）写在缩进行。
  - **解析规则（定义并进入时）**：`block NAME[...] ^:` 后，缩进块的**首行**是 `^:` 的入口参数（单值表达式），其后的缩进行才是 block 体；`^:` 只取首行单值，不吞掉其余语句。
  - **重入（无体）**：`block NAME ^:` 后只写入口参数，无 block 体（块已定义）。
  ```lz
  block A[ps: __Params] ^:     // 定义并立即进入
      (10, 20)                  // ← ^: 单值块体 = 入口参数，脱糖为 __Params 注入 ps
      print(ps.args[0])         // ← 以下为 block A 的块体
  block A ^:                    // 复用重入（已定义，无体）
      (30, 40)
  ```
- **`^:` 与 `~:` 的分工**：`^:` 是索引构建块，对应 block 的 `[...]`（中括号）；`~:` 是调用构建块，对应函数调用的 `(...)`（圆括号）。故**函数检查站入口用 `~:`**（如 `f ~: (1, 2)` 脱糖 `__Params`），**block 检查站入口用 `^:`**，二者不混用。
- block 自身的 `break NAME` 不带 `expr`（block 无值）。带 `expr` 的 `break NAME expr` 仅当 `NAME` 标注的是循环时合法（见 §4.3）。

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
    break               // 退最内层 block A
```

- 无 `NAME` 时，退**当前最内层**的可跳出构造（loop 或 block）。
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

### 4.3 与循环带值的关系（统一规则）

循环（loop / for / while）都是**表达式**，其值是 `break [NAME] [v]` 的 `v`；正常跑完 → `()`：

```lz
let r = for i in xs:          // for 是表达式
    if i > 5: break i         // r = i；若循环正常结束则 r = ()
```

| 语句 | `break` 形态 | 值的去向 |
|------|-------------|----------|
| `loop` / `for` / `while` | `break [NAME] [v]` | `v` 是**循环表达式**的值，可 `let` 绑定或丢弃；正常完成 → `()` |
| `block NAME:` | `break [NAME]` | **无值**；数据靠捕获变量或 `ps` 带出 |

> **铁律**：`break NAME v` 中若 `NAME` 是 block → **编译错误**（block 无值）；若 `NAME` 是 loop/for/while → `v` 成为该循环的值。
>
> 要「带值跳出外层」时，把外层写成 `loop NAME:`（而非 `block NAME:`）：
> ```lz
> let r = loop outer:             // outer 是 loop，可带值
>     for i in xs:
>         if done: break outer i  // r = i，并跳出 outer
> ```

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

---

## 五、block 与检查站（复用 [chk] / [ps: __Params]）

「检查站」从函数专属升格为「任何能接收 `__Params` 的可命名执行作用域」。复用 [03c-检查站.md](03c-检查站.md) 的边界脱糖与 `__Params` 结构。

### 5.1 checker 作用域块：`block NAME[ps: __Params]`

块声明自己**消费** `__Params`（即「checker 块」）。进入该块时（通过 `block NAME ^:` 传参，参数写于下一行缩进；或作为某函数/块的 `[chk]`），`ps` 作为 **`mut __Params`** 注入：

```lz
block validate[ps: __Params]:
    if len(ps.args) == 0:
        raise ValueError("need >=1 arg")
    ps.args[0] = (ps.args[0] as int) + 1   // 原地改写（block 无返回值，故原地改 ps）
```

- `ps` 类型为 `mut __Params`；block 体可校验 / 改写 `ps` 字段。
- 省略注解 `block NAME[ps]:` 也合法——`[ps]` 出现在块上，编译器已知这是检查站通道，`ps` 推断为 `__Params`（对应 `block A2[ps]:`）。
- 因 block 无返回值，**用「原地修改 ps」代替函数的 `return __Params`**（见 §5.6）。

### 5.2 block 带检查站：`block NAME[chk]`（精确写法）

与函数 `[chk]` 对称：块「带」一个检查站（函数或 checker 块），进入块先跑它改写 `__Params`，再跑体。

```lz
def my_check(ps: __Params) -> __Params raises ValueError:
    if (ps.args[0] as int) < 0:
        raise ValueError("neg")
    ps

block A[my_check] ^:             // ① 定义并立即进入 A
    (10, 20)                     // 入口参数，脱糖为 __Params 交给 my_check
    print(ps.args[0])            // ps 已是 my_check 处理后的结果
block A ^:                       // ② 复用：再次进入已定义的 A（无体，不重新定义）
    (30, 40)
```

**关键澄清（你提到的写法问题）**：

- `block A[my_check]:`（带 `:` 与体）是**定义（延迟）**——定义时**不执行体**、也无参数入口（`ps` 不存在）。需通过 `^:` 进入或作为 `[chk]` 挂载才会运行，届时才注入 `ps`。若想定义即带参进入，参数必须写在**定义行的 `^:`** 上，如 ①。
- `block A[my_check] ^: (10, 20):`（在同一行用 `^:` 接参数并带 `:` 体）**违反构建块空白规则**——`^:` 冒号后必须换行缩进，且「带 `:` 体」会被解析为重新定义 A（同名重定义，编译错误，§2.2）。**正确写法**：`^:` 后换行缩进写参数，如 ①；复用写 ②（无体）。
- 叠加消费与带站：`block A[ps: __Params][my_check] ^:\n    (10, 20)\n    print(ps.args[0])` —— 先 `my_check` 处理，再进 A 体（A 体还能继续改 ps；首行 `(10, 20)` 是 `^:` 入口参数，其后为块体）。

### 5.3 如何获取 Block 中的 ps（注入机制）

`ps` 的出现有三种**来源**，统一走编译器脱糖（[03c §4.3](03c-检查站.md)）：

| 进入方式 | `__Params` 内容 | 典型写法 |
|----------|-----------------|----------|
| `^:` 显式传参 | 元组/字典/裸值/record 按 §4.3 打包（单值块体） | `block A[ps] ^:\n    (10, 20)` |
| 作为 `[chk]` 挂在宿主 | 宿主的调用实参打包成 `__Params` | `def f[A](a, b)` → A 的 ps = f 的实参 |
| checker 块定义无 `^:` | 仅**延迟定义**、不执行（无 `__Params` 注入）；体在 `^:` 进入或 `[chk]` 触发时运行 | `block A[ps]:`（延迟定义，未进入） |

补充规则：

- **`ps` 是 `mut __Params` 局部变量**：在 checker 块内可直接读写 `ps.args` / `ps.kwargs`。
- **嵌套块可捕获父块的 `ps`**：子 checker 块按词法闭包捕获父作用域的 `ps` 变量（它只是个变量），因此子块既能用**自己的** `ps`（由 `^:`/宿主传入），也能直接读**父块**的 `ps`。
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
2. **`^:` 重新进入（就地复用）**：已定义的 checker 块可用 `block NAME ^:`（参数写于下一行缩进）执行其体（复用校验逻辑，喂新参数）：
   ```lz
   block A[ps: __Params] ^:       // 定义并首次进入（注入 ps=(0,)）
       (0,)
       print(ps.args[0])
   block A ^:                     // 复用重入（不重跑定义，只跑体 + 注入新 ps）
       (1,)                       // 复用 A 的体：打印 1
   block A ^:
       (2,)          // 复用 A 的体：打印 2
   ```
3. **作为 break 标签（控制流复用）**：plain `block A:` 不「调用」，但其名字被多次 `break A` 引用，实现跨层跳出（见 §四）。

> 注意：内联 plain `block A:`（无 `[ps]`/`[chk]`）**只执行一次**，不是可被调用的单元；它的「复用」仅指作为跳转目标。可被「调用/重入」的只有 checker 块（`[ps]`/`[chk]` 形态）通过 `^:` 与 `[chk]`。
> 另：`^:` 重入主要用于 checker 块；若原定义内含 `break NAME`（标签语义），重入时该标签不再作为跳出目标——重入的 checker 块不应依赖 `break NAME`。

### 5.5 嵌套 block 的复用：仅复用父块中的子块

子块在词法上属于父块，因此**只能在父块内部被 `^:` 重入或作为 `[chk]` 使用**；父块外部无法引用子块名（词法作用域）。「仅复用子块而不重跑父块」正是 checker 块的本地重入语义：

```lz
block Outer:                       // 父块，普通标签作用域，只跑一次
    block Child[ps: __Params]:     // 子 checker 块，定义在 Outer 内
        print("child got", ps.args[0])
    // 仅复用 Child，Outer 不再重跑：
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
- **复用不重跑父块**：`block Child ^:` 只执行 Child 的体，Outer 的其余语句不会重跑——这正是「局部重入」。
- **父块自身的 checker 与子块独立**：若 Outer 也是 checker 块（`block Outer[ps:__Params]`），其 `ps` 与 Child 的 `ps` 是两份独立变量，子块通过捕获读父 `ps`（见 §5.3）。

### 5.6 与函数 checker 的对称与差异

| | 函数 checker | block checker |
|---|---|---|
| 声明 | `def f[chk](..)` / `def chker(ps: __Params) -> __Params` | `block A[chk]` / `block A[ps: __Params]` |
| 入口 `ps` | 伪参数（函数签名注入） | `mut __Params` 注入 |
| 产出参数 | `return __Params{...}` | **原地 `ps.args[..] = ...`**（无 return） |
| 复用为 `[chk]` | 是 | 是（函数/块均可挂 `[chk]`） |
| 额外能力 | 仅看参数 | 还能校验**捕获的外层变量**（闭包） |
| 重入 | 调用即重入 | `block NAME ^:` 重入 |

> block checker 比函数 checker 更灵活：它同时看到传入的 `__Params` 与捕获的外部环境，适合「进入某段逻辑前一并校验环境 + 参数」的场景。

### 5.7 编译期验证

| 检查项 | 行为 |
|--------|------|
| `block NAME[ps]` 但体未使用 `ps` | 警告（未使用的检查站通道） |
| `block NAME[chk]` 的 `chk` 签名非 `(__Params) -> __Params` 或不是 checker 块 | 编译错误 |
| 复用重入 `block NAME ^:` 但 `NAME` 未定义 / 非 checker 块 | 编译错误 |
| 同作用域内 `block NAME ^:` 重定义（带 `:` 体） | 编译错误（同名重定义，见 §2.2） |
| 子块在父块外被引用 | 编译错误（词法作用域） |
| `break NAME v` 且 `NAME` 是 block | 编译错误（block 无值） |
| `block NAME[ps: __Params]` 但 `ps` 被当作不可变却改写 | 编译错误 |

---

## 六、与 for 守卫、loop 值语义的分工

三者正交，互不替代：

| 机制 | 语义 | 典型写法 |
|------|------|----------|
| **for 守卫** `for x in xs if cond:` | 跳过本轮（continue 语义），不污染体 | `for x in xs if x > 0: use(x)` |
| **block + break NAME** | 多层跳出、带标签控制流（无值） | `block A: ... break A` |
| **loop/for/while + break v** | 循环产出聚合值（带值退出） | `let r = loop: ... break acc` |

- 守卫吸收「条件跳过」，block 吸收「多层跳出」，loop 吸收「带值聚合」——常见「乘积+阈值停」可纯守卫化，连 `break` 都不需要。
- block 要带数据出去：用捕获变量（推荐）或写入 `ps`（checker 块场景），不能用 `break NAME v`。

---

## 七、完整示例

### 7.1 用户原例改写（block 无值 + checker 块）

原伪代码 `break A {"name":"Bob"}` 因 block 无值需调整：

```lz
cond = False
let out: Dict<str, str> = {}        // 捕获变量带出数据

block A1:                            // ① 普通命名作用域
    ...

block A2[ps]:                        // ② checker 作用域（ps 推断为 __Params）
    ...

block A[ps: __Params]:               // ②' checker 作用域（显式注解）
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

结合 §5.4 / §5.5：一个 checker 块挂到多个宿主，并在父块内被其子块 `^:` 重入。

```lz
// ① 定义一个 checker 块（归一化第一个参数），可多处复用
block norm[ps: __Params] raises ValueError:
    if (ps.args[0] as int) < 0:
        raise ValueError("neg")
    ps.args[0] = (ps.args[0] as int).abs()

// ② 挂到多个函数 —— 同一 norm 复用
def div[norm](a: int, b: int) -> int = a / b
def mul[norm](a: int, b: int) -> int = a * b

// ③ ^: 就地重入（复用校验逻辑，喂新参数）
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

---

## 八、编译期验证汇总

| 检查项 | 行为 |
|--------|------|
| 同作用域 `block` 同名 | 编译错误 |
| `break NAME` 的 `NAME` 非词法祖先 | 编译错误（含跳兄弟块） |
| `break NAME v` 且 `NAME` 是 block | 编译错误 |
| `continue` / `continue NAME` 出现在 block（非循环） | 编译错误 |
| `block NAME[ps: __Params]` 但 `ps` 未声明 `mut` 却改写 | 编译错误 |
| `block NAME[chk]` 的 chk 签名不符 | 编译错误 |
| 复用重入 `block NAME ^:` 但 `NAME` 未定义/非 checker 块 | 编译错误 |
| 同作用域内 `block NAME ^:` 带 `:` 体重定义 | 编译错误 |
| 子块在父块外被引用 | 编译错误（词法作用域） |

---

## 九、与 `:` 职责收敛（呼应附录 B）

引入 `block` 后，`break :label` / `:label` 前缀语法**废弃**。`:` 职责收敛为：

| `:` 用法 | 保留？ |
|----------|--------|
| 块体 `for ..:` / `if ..:` / `block NAME:` | ✅ |
| 构建块 `=:` `^:` `~:` `*: ` | ✅ |
| 类型注解 `x: T` | ✅ |
| 三元 `?:` | ✅ |
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

// ❌ :label 前缀
:outer for ...: break :outer
// ✅ block 标签
block outer: for ...: break outer

// ❌ 在 block 里 continue
block A: continue
// ✅ continue 仅用于循环
for x in xs: if bad: continue
```

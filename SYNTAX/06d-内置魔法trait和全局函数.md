# LZ 魔法方法 — 内置魔法 Trait 和全局函数

> 规范版本: 3.3 · 基于编译器源码 · 最后校订: 2026-09-14

本文档列举 Lang-Zone 编译器内置的全部魔法方法（`__xxx__`），以及它们自动生成的 trait 与对应的 Rust trait。

当 struct 实现了某个魔法方法时，编译器自动为该 struct 生成对应的 trait impl，同时生成一个全局函数供直接调用。

**实现状态图例**（与编译器实际行为同步，速查索引已逐条标注）：

- ✅ **已接通**：运算符/语法调用点直派该方法，产物经 rustc 编译运行验证
- 🔸 **部分支持**：调用点直派可用，但对应 trait impl 未生成（泛型/库代码场景不可用）或仅部分场景覆盖
- ❌ **未实现**：仅规范声明，编译器无对应处理（写了不报错但不联动）

---

## 〇、实现机制三分法（重要：读表前先看）

"生成 Trait"列描述的是该魔法方法**逻辑上归属的 trait**，但**并非所有方法都会真的生成 `impl`**。
编译器对魔法方法的落地分三类，速查索引表用 `impl` 列逐条标注：

| 类 | 落地方式 | 产物 | 典型方法 |
|:--:|---------|------|----------|
| **A** | 映射到 **std 标准库 trait**，编译器生成 `impl <StdTrait> for <Type>` | `impl std::ops::Add for P` 等 | `__add__`/`__sub__`/`__mul__`/`__div__`/`__rem__`、位运算族、`__eq__`/`__ne__`/`__lt__`/`__le__`/`__gt__`/`__ge__`/`__cmp__`/`__hash__`、`__neg__`/`__not__`/`__invert__`、`__iadd__`/`__isub__`/`__imul__`/`__idiv__`、`__str__`/`__repr__`、`__from__`/`__into__`/`__try_from__`/`__try_into__`、`__int__`/`__float__`、`__drop__`/`__clone__`/`__default__`、`__next__`/`__iter__`/`__rev__` |
| **B** | **仅调用点直派**：语法位置直接展开为 `recv.__method__(args)`，**不生成任何 `impl`** | 无 impl，调用点即魔法方法调用 | `__getitem__`/`__setitem__`、`__len__`/`__contains__`/`__bool__`、`__abs__`、`__cast__`/`__try_cast__`、`__pos__`/`__deref__`、`__lpipe__`/`__rpipe__`、`__is_ok__`/`__unwrap__`/`__err__`、`__size_hint__`（Iterator impl 内映射 `size_hint`，非独立 trait） |
| **C** | 映射到 **LZ 运行时自定义 trait**（`lz_builtins`），编译器生成 `impl <LzTrait> for <Type>` | `impl ImplicitFrom<i64> for P` 等 | `__implicit_from__`/`__implicit_to__`/`__implicit_copy__`/`__implicit_default__`、`__call__`（`Callable`）、`__buildparams__`（`BuildParams`）、`__guarded_pred__`/`__guarded_action__`（`GuardedStrategy`）、`__pow__`（`LzPow`） |

> **关键澄清**：`__getitem__`/`__setitem__`/`__len__`/`__contains__`/`__bool__`/`__abs__`/`__cast__`/`__try_cast__`/`__pos__`/`__deref__` 这些方法**只走调用点直派（B 类）**，
> 即使本文各分类表格"生成 Trait"列写了 `std::ops::Index`/`HasLen`/`Cast` 等 trait 名，编译器**当前也不会生成对应 `impl`**——
> 它们由语法位置（`[]`/`in`/`if`/`abs()`/`as`/`+`/`*`/`**`）在调用点直接展开为固有方法调用。
> B 类方法要参与泛型/库代码（如 `fn f<T: Index>`）时**无法**经 trait 约束，这是当前实现边界。

### self 模式速查（各分类典型 self 接收方式）

self 参数决定调用方式与所有权语义，四种模式定义详见 [06g](06g-魔法综合.md) §一。各分类典型值：

| 分类 | 典型 self | 说明 |
|------|-----------|------|
| 一/二 算术·位运算 | `self`（owned 消费） | self 与 rhs 均消费，Output 来自返回类型 |
| 三 一元 | `self`（owned 消费） | 不分派 |
| 四 复合赋值 | `mut self`（&mut） | rhs 借入 |
| 五 比较 | `ref self`（&self） | `__eq__`/`__ne__` 支持按 rhs 多分派 |
| 六 类型转换 | `__from__`/`__try_from__` 无 self（关联函数）；其余 `self` | 前者按参数分派，后者按返回类型分派 |
| 七 显示/调试 | `ref self`（&self） | 不分派 |
| 八 容器/迭代 | `__next__`/`__rev__` `mut self`；`__iter__`/`__into_iter__` `self`；`__size_hint__`/`__len__`/`__contains__` `ref self` | |
| 九 构造 | `__new__` 无 self（类方法）；`__init__` `self`（实例方法） | |
| 十 生命周期 | `__drop__` `mut self`；`__clone__` `ref self`；`__default__` 无 self | |
| 十一 调用/索引 | `__call__` `self`；`__getitem__` `ref self`；`__setitem__` `mut self` | |
| 十二 布尔/数学 | `__bool__` `ref self`；`__abs__` `self` | |
| 十四 隐式策略 | `__implicit_from__`/`__implicit_default__` 无 self；`__implicit_to__`/`__implicit_copy__` `ref self` | |
| 十六 类型缺口 | `__int__`/`__float__`/`__pos__` `self` | |
| 十七 上下文 | `__enter__` `self`（owned，消费后返回 guard）；`__exit__` `mut self` | |

> 注：上表为**规范典型值**（用户声明时通常这样写）。编译器对 `def __xxx__(self, ...)` 有 **auto-ref/mut 推断**：
> 方法体只读访问时自动降级为 `&self`，需修改时升级为 `&mut self`（见 `src/ir/codegen/mod.rs` self 参数修饰逻辑）。

### 全局函数（当前实现状态）

按 [06f](06f-magic用法.md) §零，一次 `magic` 声明应同时产出 **trait + 魔法名 + 全局函数**。
**当前实现状态**：

- **trait 生成**：✅ 已实现（A 类 std trait impl + C 类 lz_builtins impl，struct/impl 定义 `def __xxx__` 时自动生成，无需显式 `magic` 声明）。
- **调用点直派**：✅ 已实现（运算符/语法位置直接展开为 `__xxx__` 调用）。
- **独立 trait 定义 + 全局函数生成**：❌ **尚未实现**（`magic name<T> = def __xxx__ ...` 声明块产出独立 trait 与 `fn name(args)` 全局函数的能力，为下一阶段工作，规范定义见 06f §一/§四）。
  在实现前，"全局函数"（如 `map(xs, f)`）需用户以普通 `def` 显式书写，编译器不自动生成。

---

## 一、算术运算符

魔法方法 → 生成的 LZ trait → Rust trait → 用途。

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__add__` | `Add` | `std::ops::Add` | 加法 `a + b` |
| `__sub__` | `Sub` | `std::ops::Sub` | 减法 `a - b` |
| `__mul__` | `Mul` | `std::ops::Mul` | 乘法 `a * b` |
| `__div__` | `Div` | `std::ops::Div` | 除法 `a / b` |
| `__rem__` | `Rem` | `std::ops::Rem` | 取余 `a % b` |
| `__pow__` | `LzPow` | `lz_builtins::LzPow`（C 类，impl 生成） | 幂运算 `a ** b`（std::ops 无对应 trait，LZ 自定义；trait 方法名 `pow` 与固有魔法名 `__pow__` 不同名，仿 std::ops::Add::add 惯例避免同名递归） |

均为二元运算符（`BinaryOp`），self 和 rhs 均消费所有权，Output 来自返回类型。`__add__`~`__rem__` 映射到 `std::ops`（A 类，生成 `impl`）；`__pow__` 映射到 `lz_builtins::LzPow`（C 类，用户 struct 定义时生成 `impl LzPow<Rhs>`，数值 `**` 仍走内建 `.pow()`）。

> `__rem__` 注意事项：`%` 是 remainder（取余，Rust `Rem` trait），非 Python 的 modulo（取模）。对负数，`-7 % 3` = `-1`（remainder）而非 `2`（modulo）。

---

## 二、位运算符

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__bitand__` | `BitAnd` | `std::ops::BitAnd` | 按位与 `a & b` |
| `__bitor__` | `BitOr` | `std::ops::BitOr` | 按位或 `a \| b` |
| `__bitxor__` | `BitXor` | `std::ops::BitXor` | 按位异或 `a ^ b` |
| `__shl__` | `Shl` | `std::ops::Shl` | 左移 `a << b` |
| `__shr__` | `Shr` | `std::ops::Shr` | 右移 `a >> b` |

均为二元运算符（`BinaryOp`），self 和 rhs 均消费，支持按 rhs 类型多分派。

---

## 三、一元运算符

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__neg__` | `Neg` | `std::ops::Neg` | 取负 `-a` |
| `__not__` | `Not` | `std::ops::Not` | 逻辑非 `not a` / `!a` |
| `__invert__` | `std::ops::Not`（复用） | `std::ops::Not` | 按位取反 `~a`（Rust 无独立 BitNot trait，`Not` 为位非/逻辑非统一入口，按语法 `~`/`!`/`not` 分派；A 类，`__not__` 缺席时生成 `impl Not`） |
| `__deref__` | `Deref` | `std::ops::Deref` | 解引用 `*a` |

均为一元运算符（`UnaryOp`），self 消费，Output 来自返回类型。不分派。

---

## 四、复合赋值

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__iadd__` | `AddAssign` | `std::ops::AddAssign` | 自加 `a += b` |
| `__isub__` | `SubAssign` | `std::ops::SubAssign` | 自减 `a -= b` |
| `__imul__` | `MulAssign` | `std::ops::MulAssign` | 自乘 `a *= b` |
| `__idiv__` | `DivAssign` | `std::ops::DivAssign` | 自除 `a /= b` |

均为复合赋值二元运算符（`BinaryAssign`），self 为可变引用 `&mut self`，rhs 借入。支持按 rhs 类型多分派。

---

## 五、比较

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__eq__` | `PartialEq` | `std::cmp::PartialEq` | 相等 `a == b` |
| `__ne__` | `PartialEq` | `std::cmp::PartialEq` | 不等 `a != b` |
| `__lt__` | `PartialOrd` | `std::cmp::PartialOrd` | 小于 `a < b` |
| `__le__` | `PartialOrd` | `std::cmp::PartialOrd` | 小于等于 `a <= b` |
| `__gt__` | `PartialOrd` | `std::cmp::PartialOrd` | 大于 `a > b` |
| `__ge__` | `PartialOrd` | `std::cmp::PartialOrd` | 大于等于 `a >= b` |
| `__cmp__` | `Ord` | `std::cmp::Ord` | 全序比较（返回 `Ordering`） |
| `__hash__` | `Hash` | `std::hash::Hash` | 哈希值计算 |

比较魔法方法均以 `&self` 借用模式接收 self。`__eq__`/`__ne__` 支持按 rhs 类型多分派；`__cmp__`/`__hash__` 不分派。

`__lt__`/`__le__`/`__gt__`/`__ge__` 四个方法均映射到 `PartialOrd::partial_cmp`，返回 `Option<Ordering>`。

---

## 六、类型转换

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__from__` | `From` | `std::convert::From` | 从已知类型构造自身 `T::from(x)` |
| `__into__` | `Into` | `std::convert::Into` | 转换为目标类型 `x.into()` |
| `__cast__` | `Cast`（规划） | `Cast`（自定义，未生成） | Mojo 风格直接类型转换（B 类：`x as T` 调用点直派 `x.__cast__()`，无 Cast impl） |
| `__try_cast__` | `TryCast`（规划） | `TryCast`（自定义，未生成） | 可失败类型转换 `Result<T, Error>`（B 类：仅定义它时 `x as T` → `x.__try_cast__().unwrap()`，无 TryCast impl） |
| `__try_from__` | `TryFrom` | `std::convert::TryFrom` | 可失败构造 |
| `__try_into__` | `TryInto` | `std::convert::TryInto` | 可失败转换 |

`__from__`/`__try_from__` 按参数类型多分派；`__into__`/`__cast__`/`__try_cast__`/`__try_into__` 按返回类型多分派。

`__cast__<T>(self) -> T` 和 `__try_cast__<T>(self) -> Result<T, Error>` 对齐 Mojo 风格的泛型类型转换。

**`as` 操作符分发规则**：
1. 优先调用 `__cast__`（强转）
2. 若类型未实现 `__cast__` 但实现了 `__try_cast__`，则调用 `__try_cast__`，失败时抛出 `CastError`
3. 两者均未实现 → 编译期错误
4. 数值基本类型互转由编译器内置实现，不依赖 `__cast__`/`__try_cast__`

---

## 七、显示/调试

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__str__` | `Display` | `std::fmt::Display` | 用户友好的字符串表示 |
| `__repr__` | `Debug` | `std::fmt::Debug` | 调试用的详细字符串表示 |

均以 `&self` 借用模式接收。不分派。方法签名均为 `fn fmt(&self, f: &mut Formatter) -> fmt::Result`。

---

## 八、容器/迭代

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__next__` | `Iterator` | `std::iter::Iterator` | 获取下一个元素 `Option<Item>` |
| `__iter__` | `IntoIterator` | `std::iter::IntoIterator` | 转换为迭代器 |
| `__into_iter__` | `IntoIterator` | `std::iter::IntoIterator` | struct 自身即迭代器（如生成器） |
| `__rev__` | `DoubleEndedIterator` | `std::iter::DoubleEndedIterator` | 反向获取元素 `next_back` |
| `__size_hint__` | `Iterator` | `std::iter::Iterator` | 剩余元素数量范围提示 |
| `__len__` | `HasLen`（规划） | `HasLen`（自定义，未生成） | 容器长度（B 类：`len(x)`/真值链 `x.__len__() != 0` 调用点直派，无 HasLen impl） |
| `__contains__` | `Contains`（规划） | `Contains`（自定义，未生成） | 成员检查 `x in obj`（B 类：调用点直派 `obj.__contains__(x)`，无 Contains impl） |

- `__next__` 使用 `&mut self` 可变借用，`__iter__`/`__into_iter__` 消费 self
- `__rev__` 与 `__next__` 共存时自动生成 `DoubleEndedIterator` impl（`DoubleEndedIterator` 继承 `Iterator`）
- `__size_hint__` 使用 `&self` 借用，返回 `(usize, Option<usize>)`，用于优化 `collect()`/`partition()` 等操作

---

## 九、构造与初始化

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__new__` | `New`（规划） | （自定义 trait，未生成） | 分配并构造实例，返回 `Self`（B 类：构造调用点 `P(...)` 分派 `P::__new__(...)`，无 New impl） |
| `__init__` | `Init`（规划） | （自定义 trait，未生成） | 初始化已分配的实例（B 类：`let mut x = P(...)` 后自动注入 `x.__init__()`，仅 `__init__` 只含 self 参数时触发，无 Init impl） |

- `__new__(...) -> Self` — 类方法（第一个参数不是 self），负责内存分配和构造，返回新实例。实现后可覆盖默认构造行为
- `__init__(self, ...)` — 实例方法，构造后立即调用，负责字段初始化

```lz
struct Point =
    x: int
    y: int
    def __new__(x: int, y: int) -> Point =
        Point(x, y)
    def __init__(self: Point, x: int, y: int) =
        self.x = x
        self.y = y
```

> 详见 [06a-struct.md](06a-struct.md) §六 构造器魔法方法。

---

## 九点五、提取器模式

| 魔法方法 | 参数 | 返回 | 用途 |
|----------|------|------|------|
| `__unapply__` | `self` | `(T1, T2, ...)` | 提取解构：将 struct 分解为元组，用于 `case Point(x,y)` / `let Point(x,y)=p` / `for Point(x,y) in pts` |

```lz
// 普通 struct 显式实现：
struct Point =
    x: int
    y: int
    def __unapply__(self) -> (int, int) = (self.x, self.y)

match p:
    case Point(px, py) => print(px + py)
// let 提取绑定：
let Point(a, b) = p        // 糖化为 let (a, b) = p.__unapply__()
// for 循环变量提取：
for Point(a, b) in pts:
    print(a, b)
```

- **case struct 自动配**：声明为 `case struct PointEx(...)` 时，编译器自动生成 `__unapply__`（按字段声明顺序返回元组），无需手写（见 [06a-struct.md](06a-struct.md) case struct 章节）；普通 struct 才需上述显式 `def __unapply__`。
- `__unapply__` 使用 `self`（owned），返回元组。
- 返回元组的各元素按位置绑定到模式中的子变量。
- 同一提取协议在三处通用：`case Point(px,py)` / `let Point(px,py)=p` / `for Point(px,py) in pts` 最终都展开为 `let (px, py) = p.__unapply__()`（再由 IR 复用元组解构）。
- 一个 struct 只能定义一个 `__unapply__`。

> 详见 [06a-struct.md](06a-struct.md) 与 [05-控制流.md](05-控制流.md) §二 match / case。

---

## 十、生命周期

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__drop__` | `Drop` | `std::ops::Drop` | 析构时清理资源 |
| `__clone__` | `Clone` | `std::clone::Clone` | 克隆对象 |
| `__default__` | `Default` | `std::default::Default` | 创建默认值 |

- `__drop__` 使用 `&mut self` 可变借用
- `__clone__` 使用 `&self` 借用，返回 `Self`
- `__default__` 无 self（关联函数），返回 `Self`

---

## 十一、调用/索引

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__call__` | `Callable` | `lz_builtins::Callable`（C 类，impl 生成） | 可调用对象 `obj(args)`（调用点直派 + `impl Callable<Args>` 生成） |
| `__getitem__` | `Index`（规划） | `std::ops::Index`（未生成） | 索引读取 `obj[key]`（B 类：调用点直派 `obj.__getitem__(key)`，无 Index impl） |
| `__setitem__` | `IndexMut`（规划） | `std::ops::IndexMut`（未生成） | 索引写入 `obj[key] = val`（B 类：调用点直派 `obj.__setitem__(key, v)`，无 IndexMut impl） |
| `__lpipe__` | — | — | 管道左侧数据变换 `a \|> b`：先调 `a.__lpipe__()` 产出数据（默认返回自身）（B 类：调用点直派） |
| `__rpipe__` | — | — | 管道右侧处理工厂 `a \|> b`：`b.__rpipe__(a)` 返回单参函数处理数据（优先于 `__call__`）（B 类：调用点直派） |
| `__is_ok__` | `SpreadOk`（规划） | `SpreadOk`（自定义，未生成） | 可传播判定 `obj?`：`True`=成功值（B 类：`?` 展开直派；`Option`/`Result` 为编译器内建判定，见 09 §4.3） |
| `__unwrap__` | `SpreadOk`（规划） | `SpreadOk`（自定义，未生成） | 可传播成功值解包：`obj?` 成功分支的结果（B 类：调用点直派） |
| `__err__` | `SpreadErr`（规划） | `SpreadErr`（自定义，未生成） | 可传播错误值提取：`obj?` 错误分支向上传播的内容（B 类：调用点直派） |

- `__call__` 使用 `Custom(MagicDesc)` 通用分支生成，trait 方法签名为 `fn call(self, args: (A, B)) -> R`，入参打包为元组，返回类型为关联类型 `Output`
- `__getitem__` 使用 `&self` 借用，`__setitem__` 使用 `&mut self` 可变借用
- `__getitem__`/`__setitem__` 支持按索引类型多分派

> **可调用性规则**：struct 作为构造器始终可调用；struct 实例**仅当定义了 `__call__`** 时才可调用；enum / trait / duck 均不可调用。详见 [99-内置预导入库.md](99-内置预导入库.md) 可调用性规则表。

---

## 十二、布尔 / 数学

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__bool__` | `HasBool`（规划） | `HasBool`（自定义，未生成） | 布尔测试 `if obj:`。判定链：`__bool__` → `__len__ != 0` → 默认 true（B 类：调用点直派，无 HasBool impl） |
| `__abs__` | `HasAbs`（规划） | `HasAbs`（自定义，未生成） | 绝对值 `abs(x)`（B 类：调用点直派 `x.__abs__()`，数值走内建 `lz_abs`，无 HasAbs impl） |
| `__hash__` | `Hash` | `std::hash::Hash` | 哈希值计算（见比较） |

- `__bool__` 使用 `&self` 借用，返回 `bool`
- `__abs__` 使用 self 消费，返回 `Self`

> `__hash__` 在比较分类中已列出，此处为交叉引用。

---

## 十三、构建块

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__buildparams__` | `BuildParams` | `BuildParams`（自定义） | 构建块参数协议 `into_args` |

构建块协议允许将结构体参数打包为参数形式，用于 DSL 构建块场景。

---

## 十四、隐式策略

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__implicit_from__` | `ImplicitFrom` | `ImplicitFrom`（自定义） | 隐式从源类型构造：`T` 可从 `S` 自动构造 |
| `__implicit_to__` | `ImplicitInto` | `ImplicitInto`（自定义） | 隐式转换为目标类型：`S` 可自动转为 `T`（由 `__implicit_from__` blanket impl 派生） |
| `__implicit_copy__` | `ImplicitCopy` | `ImplicitCopy`（自定义） | Mojo 风格隐式复制（move 后复用） |
| `__implicit_default__` | `ImplicitDefault` | `ImplicitDefault`（自定义） | 隐式默认值填充 |

**`__implicit_from__` 与 `__implicit_to__` 的配对关系**：

```
实现 __implicit_from__  → blanket impl 自动提供 __implicit_to__
（类似 Rust From<T> → Into<U> 的自动推导）
```

**规则**：
1. 只需实现 `__implicit_from__`，对应的 `__implicit_to__` 自动获得
2. `__implicit_from__` 和 `__implicit_to__` 不可同时手动实现（避免歧义）
3. 每次隐式转换最多执行一步（禁止链式 A→B→C 自动转换）
4. 隐式转换与显式 `as` / `.into()` 互不干扰

```lz
// 示例：让 int 可以从 str 隐式构造
struct Int =
    value: i64
    def __implicit_from__(s: str) -> Self =
        Self(value: s.parse().unwrap_or(0))

s = "1"
x: int = s   // str → int，编译器自动插入 __implicit_from__
```

**初始化优先级链**（类型不匹配时的尝试顺序）：

```
S → T (隐式)
  1. T::__implicit_from__(v)     // 优先：隐式 From
  2. v.__implicit_to__::<T>()    // 次之：隐式 Into（由步骤 1 的 blanket impl 派生）
  3. T::__from__(v)              // 回退：显式 From
  4. v.__into__::<T>()           // 回退：显式 Into
  5. T::__default__()            // 兜底：默认值（仅当 v 是 default 关键字时）
```

**返回值隐式转换**：

```lz
def calculate() -> int =
    let result: str = "42"
    return result  // str → int，编译器自动插入 __implicit_from__<int>(result)
```

- `__implicit_from__` 按源类型多分派，签名 `__implicit_from__(source: S) -> Self`
- `__implicit_to__<T>` 按返回类型多分派，签名 `__implicit_to__(self) -> T`
- `__implicit_copy__` 使用 `&self`（`ref self`）借用，返回 `Self`。**两处触发点**（实测 p61 均生效）：
  1. **复制路径**：`let b = a`（`a` 为实现 `__implicit_copy__` 的同类型变量引用）→ 桥接为 `<T as ImplicitCopy>::__implicit_copy__(&a)`，`a` 继续有效（Mojo 风格隐式复制）；
  2. **move 路径**：`d = a^`（所有权转移时）→ 同样触发 `__implicit_copy__`，原变量 `a` 转移后仍有效。
  两处产物形态相同；区别仅在触发语法位置（let 绑定 vs `^` 移动）。详见 [12-操作符.md](12-操作符.md) §1.14。
- `__implicit_default__` 无 self（关联函数），触发点为 `let x: T = default`

---

## 十五、守卫策略

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__guarded_pred__` | `GuardedStrategy` | `GuardedStrategy`（自定义） | 兜底守卫判定 `fn pred(&self, &Input) -> bool` |
| `__guarded_action__` | `GuardedStrategy` | `GuardedStrategy`（自定义） | 兜底守卫执行 `fn action(self, Input) -> Output` |

两者配对生成 `GuardedStrategy` impl。`__guarded_pred__` 判定是否执行兜底行为，`__guarded_action__` 执行兜底行为并返回结果。均支持按输入参数类型多分派。

---

## 十六、类型缺口

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__int__` | `From` | `std::convert::From` | 数值/字符串 → `i64` 整数转换 |
| `__float__` | `From` | `std::convert::From` | 数值/字符串 → `f64` 浮点转换 |
| `__pos__` | `Pos`（规划） | `Pos`（自定义，未生成） | 一元正号 `+x`（B 类：`+a` 调用点直派 `a.__pos__()`，无 Pos impl；无魔术方法时数值恒等） |

`__int__` 和 `__float__` 是"缺口魔法"：编译器为实现了它们的类型自动生成 `impl From<SelfTy> for i64` 和 `impl From<SelfTy> for f64`（A 类），并直派 `int(x)`/`float(x)` 调用点。

`__pos__` 为 B 类：调用点直派 `a.__pos__()`，当前不生成 `Pos` trait impl（泛型场景不可用）。

---

## 十七、上下文管理器

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__enter__` | `Enter`（规划） | `Enter`（自定义，未生成） | 进入 `with` 块（上下文管理器入口）（B 类：with 语句展开直派，无 Enter impl） |
| `__exit__` | `Exit`（规划） | `Exit`（自定义，未生成） | 退出 `with` 块（上下文管理器出口）（B 类：未定义时跳过调用，无 Exit impl） |

`__enter__` 消费 self，返回一个 guard 对象（由 `with ... as x` 中的 `x` 绑定）；`__exit__` 接收 `mut self` 与该 guard 对象为参数（`def __exit__(mut self, _guard: File)`），在其上执行清理逻辑。

```lz
struct MyFile =
    path: str
    def __enter__(self) -> File =
        open(self.path)                     // self 被消费, 返回 File guard
    def __exit__(mut self, _guard: File) =
        _guard.close()                       // 在 guard 上清理
```

> 与 Python 协议的关键区别: `__exit__` 接收 guard 值而非 `self`（因为 `__enter__` 已将 `self` 消费），且不接收异常三元组（LZ 用 `try/catch` 替代 `with` 内的异常处理）。

---

## 十八、迭代策略

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__iter_strategy__` | `IterStrategy`（规划） | `IterStrategy`（自定义，未生成） | 返回按优先级排序的迭代策略列表 |

> **实现状态（🔸 未接通）**：当前编译器仅在 `MagicEngine` 注册表中登记了 `__iter_strategy__`（→ `IterStrategy::resolve`），codegen 侧只有 `MagicKind::IterStrategy` 直调占位（原样返回）——
> `__iter_resolve` 的"取首个适用策略"选择机制、`IterStrategy` trait 定义、for-in 循环消费点**均未实现**。
> 规范目标语义：`__iter_strategy__` 在 for-in 循环中通过 `__iter_resolve` 取第一个适用策略包裹 base 迭代器（来自 `__iter__`），对齐可控迭代器（Itor）的迭代策略选择机制。
> 本节保留为**规划规范**，实现后此处状态需同步为 ✅。

---

## 速查索引

状态依据 2026-09-12 编译器源码核实 + 探针（转译 → rustc 编译 → 运行）实测：

| 魔法方法 | Trait | 分类 | impl | 状态 |
|----------|-------|------|:----:|:----:|
| `__add__` | `std::ops::Add` | 算术 | A | ✅ |
| `__sub__` | `std::ops::Sub` | 算术 | A | ✅ |
| `__mul__` | `std::ops::Mul` | 算术 | A | ✅ |
| `__div__` | `std::ops::Div` | 算术 | A | ✅ |
| `__rem__` | `std::ops::Rem` | 算术 | A | ✅ |
| `__pow__` | `LzPow` | 算术 | C | ✅ 用户 struct `a ** b` 直派 `a.__pow__(b)` + 生成 `impl lz_builtins::LzPow<Rhs>`（泛型场景可约束 `T: LzPow<Rhs>`）；数值 `**` 走内建 `.pow()` |
| `__bitand__` | `std::ops::BitAnd` | 位运算 | A | ✅ |
| `__bitor__` | `std::ops::BitOr` | 位运算 | A | ✅ |
| `__bitxor__` | `std::ops::BitXor` | 位运算 | A | ✅ |
| `__shl__` | `std::ops::Shl` | 位运算 | A | ✅ |
| `__shr__` | `std::ops::Shr` | 位运算 | A | ✅ |
| `__neg__` | `std::ops::Neg` | 一元 | A | ✅ |
| `__not__` | `std::ops::Not` | 一元 | A | ✅ |
| `__invert__` | `std::ops::Not`（复用） | 一元 | A | ✅ `~a`/`!a`/`not a` 分派，`__not__` 缺席时回退 |
| `__iadd__` | `std::ops::AddAssign` | 复合赋值 | A | ✅ |
| `__isub__` | `std::ops::SubAssign` | 复合赋值 | A | ✅ |
| `__imul__` | `std::ops::MulAssign` | 复合赋值 | A | ✅ |
| `__idiv__` | `std::ops::DivAssign` | 复合赋值 | A | ✅ |
| `__eq__` | `std::cmp::PartialEq` | 比较 | A | ✅ |
| `__ne__` | `std::cmp::PartialEq` | 比较 | A | ✅ 未定义时由 `!__eq__` 派生 |
| `__lt__` | `std::cmp::PartialOrd` | 比较 | A | ✅ `if a < b` 直派 + PartialOrd 由 `__eq__`+`__lt__` 推导 |
| `__le__` | `std::cmp::PartialOrd` | 比较 | A | ✅ |
| `__gt__` | `std::cmp::PartialOrd` | 比较 | A | ✅ |
| `__ge__` | `std::cmp::PartialOrd` | 比较 | A | ✅ |
| `__cmp__` | `std::cmp::Ord` | 比较 | A | ✅ `__eq__`+`__lt__` 同存时生成 Ord impl（int -1/0/1 → Ordering）+ Eq 标记 impl |
| `__hash__` | `std::hash::Hash` | 比较 | A | ✅ Hash impl 委托 `__hash__()` |
| `__from__` | `std::convert::From` | 类型转换 | A | ✅ From impl 生成 + `let`/实参/返回值三触发点隐式转换；A↔B 双向 `__from__` 编译期报循环错误 |
| `__into__` | `std::convert::Into` | 类型转换 | A | ✅ Into impl 生成；显式 `.__into__()` 调用 |
| `__cast__` | `Cast`（规划） | 类型转换 | B | ✅ `x as T` 调用点直派 `x.__cast__()`（原生类型间仍走内置 `as`）；无 Cast trait impl |
| `__try_cast__` | `TryCast`（规划） | 类型转换 | B | ✅ 仅定义 `__try_cast__` 时 `x as T` → `x.__try_cast__().unwrap()`（失败 panic）；无 TryCast trait impl |
| `__try_from__` | `std::convert::TryFrom` | 类型转换 | A | ✅ TryFrom impl 生成（`type Error = E` + try_from 委托） |
| `__try_into__` | `std::convert::TryInto` | 类型转换 | A | ✅ TryInto impl 生成（`type Error = E` + try_into 委托） |
| `__str__` | `std::fmt::Display` | 显示/调试 | A | ✅ |
| `__repr__` | `std::fmt::Debug` | 显示/调试 | A | ✅ |
| `__next__` | `std::iter::Iterator` | 容器/迭代 | A | ✅ 返回 `Option<T>` 时生成 |
| `__iter__` | `std::iter::IntoIterator` | 容器/迭代 | A | ✅ 返回命名迭代器时生成 |
| `__into_iter__` | `std::iter::IntoIterator` | 容器/迭代 | B | ✅ for-in 分派 `iter.__into_iter__()`（返回 List 复用 Vec 迭代）；无独立 impl（迭代器类型自身已有 Iterator impl） |
| `__rev__` | `std::iter::DoubleEndedIterator` | 容器/迭代 | A | ✅ 与 __next__ 共存时生成 DoubleEndedIterator impl（next_back 委托） |
| `__size_hint__` | `std::iter::Iterator` | 容器/迭代 | B | ✅ `impl Iterator` 块内方法名映射为 `size_hint`（非独立 trait impl）；LZ 签名 `(int, Option<int>)` → `(usize, Option<usize>)` |
| `__len__` | HasLen（规划） | 容器/迭代 | B | ✅ `len(x)`/真值链 `__len__ != 0` 调用点直派；无 HasLen trait impl |
| `__contains__` | Contains（规划） | 容器/迭代 | B | ✅ `x in obj` 调用点直派；无 Contains trait impl |
| `__drop__` | `std::ops::Drop` | 生命周期 | A | ✅ Drop impl 委托 `__drop__()`（方法与泛型串均无约束时生成） |
| `__clone__` | `std::clone::Clone` | 生命周期 | A | ✅ 手动 Clone impl 委托 `__clone__()`（derive 兜底已让位，E0119 规避） |
| `__default__` | `std::default::Default` | 生命周期 | A | ✅ Default impl 委托 `__default__()` |
| `__call__` | `Callable` | 调用/索引 | C | ✅ `obj(args)` 直调分派 + `impl lz_builtins::Callable<Args>` 生成（探针验证） |
| `__getitem__` | `std::ops::Index`（规划） | 调用/索引 | B | ✅ `obj[key]` 调用点直派 `obj.__getitem__(key)`；无 Index trait impl |
| `__setitem__` | `std::ops::IndexMut`（规划） | 调用/索引 | B | ✅ `obj[key] = v` 调用点直派 `obj.__setitem__(key, v)`；无 IndexMut trait impl |
| `__lpipe__`/`__rpipe__` | — | 管道 | B | ✅ 管道由通用 callable 语义驱动（2026-08-08 决策），调用点直派 |
| `__is_ok__`/`__unwrap__` | `SpreadOk`（规划） | 错误传播 `?` | B | ✅ 编译器内建协议（`?` 传播三件套），调用点直派 |
| `__err__` | `SpreadErr`（规划） | 错误传播 `?` | B | ✅ 同上 |
| `__bool__` | HasBool（规划） | 布尔/数学 | B | ✅ 真值判定链 `__bool__` → `__len__ != 0` → 默认 true（调用点直派）；无 HasBool trait impl |
| `__buildparams__` | `BuildParams` | 构建块 | C | ✅ `task ~: cfg` 构建块协议（`cfg.into_args()` → 元组解包传参）；trait impl 生成 |
| `__implicit_copy__` | `ImplicitCopy` | 隐式策略 | C | ✅ 两处触发：① `let b = a`（同类型变量引用）② `d = a^`（move 时复制），均桥接为 `<T as ImplicitCopy>::__implicit_copy__(&a)`；trait impl 生成（探针 p61 实测两处均触发） |
| `__implicit_to__` | ImplicitInto | 隐式策略 | C | ✅ `let b: B = a` 类型不匹配回退桥接；trait impl 生成 |
| `__implicit_from__` | ImplicitFrom | 隐式策略 | C | ✅ 用户定义 `def __implicit_from__(raw: SrcTy) → Self` 生成 `impl ImplicitFrom<SrcTy>` 委托；内建首字段构造路径见 StructDef 声明式 |
| `__implicit_default__` | ImplicitDefault | 隐式策略 | C | ✅ `let x: T = default` 桥接为 `<T as ImplicitDefault>::__implicit_default__()`；trait impl 生成 |
| `__guarded_pred__` | `GuardedStrategy` | 守卫策略 | C | ✅ `guard obj with input` 委托语法；trait impl 生成 |
| `__guarded_action__` | `GuardedStrategy` | 守卫策略 | C | ✅ 与 pred 配对；false 分支自动调用 |
| `__int__` | `std::convert::From` | 类型缺口 | A | ✅ `From<SelfTy> for i64` impl 生成 + `int(x)` 调用点直派 |
| `__float__` | `std::convert::From` | 类型缺口 | A | ✅ `From<SelfTy> for f64` impl 生成 + `float(x)` 调用点直派 |
| `__pos__` | Pos（规划） | 类型缺口 | B | ✅ `+a` 分派 `a.__pos__()`；无 Pos trait impl；无魔术方法时数值恒等 |
| `__deref__` | `std::ops::Deref`（规划） | 运算符 | B | ✅ `*a` 对用户 struct 分派 `a.__deref__()`（调用点直派）；无 Deref trait impl；真实引用类型保留裸解引用 |
| `__unapply__` | — | 提取器 | B | ✅ case struct 自动配；普通 struct 显式定义可用（`let Point(a,b)=p` / `case Point(x,y)` / `for Point(a,b) in pts` 三处脱糖为 `__unapply__()`） |
| `__enter__` | Enter（规划） | 上下文 | B | ✅ with 构造链：enter → 体 → exit（with 语句展开，调用点直派）；无 Enter trait impl |
| `__exit__` | Exit（规划） | 上下文 | B | ✅ 未定义时跳过调用（E0599 防护）；无 Exit trait impl |
| `__iter_strategy__` | `IterStrategy`（规划） | 迭代策略 | B | 🔸 仅注册表 + 直调占位；`__iter_resolve` 策略选择机制未实现（`lz_builtins` 无 `IterStrategy` 家族、无 for-in 消费点、`@Mode` 注解不可用）——规范目标见 §十八 注，05-控制流 §3.1、99 §3.2 已同步标注 ❌ 规划中 |
| `__new__` | New（规划） | 构造 | B | ✅ 构造调用点 `P(...)` 分派 `P::__new__(...)`（`struct_has_new` 或方法集合检测）；未定义时走字段构造 + 默认值补齐；无 New trait impl |
| `__init__` | Init（规划） | 构造 | B | ✅ 函数体生成正确（`self.x = ...`）；`let mut x = Struct(...)` 后自动注入 `x.__init__()`（仅 `__init__` 只含 self 参数时触发，Call / StructCtor 两种构造表示均支持）；无 Init trait impl |
| `__abs__` | HasAbs（规划） | 缺口魔法 | B | ✅ `abs(x)` 调用点直派 `x.__abs__()`（数值走内建 `lz_abs`）；无 HasAbs trait impl |

---

*上一章：[06c-trait和impl](06c-trait和impl.md)* · *下一章：[06e-模块级魔法属性](06e-模块级魔法属性.md)*

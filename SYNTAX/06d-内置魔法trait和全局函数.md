# LZ 魔法方法 — 内置魔法 Trait 和全局函数

> 规范版本: 3.4 · 基于编译器源码 · 最后校订: 2026-09-12

本文档列举 Lang-Zone 编译器内置的全部魔法方法（`__xxx__`），以及它们自动生成的 trait 与对应的 Rust trait。

当 struct 实现了某个魔法方法时，编译器自动为该 struct 生成对应的 trait impl，同时生成一个全局函数供直接调用。

**实现状态图例**（与编译器实际行为同步，速查索引已逐条标注）：

- ✅ **已接通**：运算符/语法调用点直派该方法，trait impl 自动生成，产物经 rustc 编译运行验证
- 🔸 **部分支持**：已注册或调用点可用，但 trait impl / 全部场景未覆盖
- ❌ **未实现**：仅规范声明，编译器无对应处理（写了不报错但不联动）

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
| `__pow__` | `Pow` | `Pow`（自定义） | 幂运算 `a ** b` |

均为二元运算符（`BinaryOp`），self 和 rhs 均消费所有权，Output 来自返回类型。除 `__pow__` 外均映射到 `std::ops`。

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
| `__invert__` | `HasInvert` | `HasInvert`（自定义） | 按位取反 `~a` |
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
| `__cast__` | `Cast` | `Cast`（自定义） | Mojo 风格直接类型转换 |
| `__try_cast__` | `TryCast` | `TryCast`（自定义） | 可失败类型转换 `Result<T, Error>` |
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
| `__len__` | `HasLen` | `HasLen`（自定义） | 容器长度 |
| `__contains__` | `Contains` | `Contains`（自定义） | 成员检查 `x in obj` |

- `__next__` 使用 `&mut self` 可变借用，`__iter__`/`__into_iter__` 消费 self
- `__rev__` 与 `__next__` 共存时自动生成 `DoubleEndedIterator` impl（`DoubleEndedIterator` 继承 `Iterator`）
- `__size_hint__` 使用 `&self` 借用，返回 `(usize, Option<usize>)`，用于优化 `collect()`/`partition()` 等操作

---

## 九、构造与初始化

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__new__` | `New` | （自定义 trait） | 分配并构造实例，返回 `Self` |
| `__init__` | `Init` | （自定义 trait） | 初始化已分配的实例 |

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
| `__call__` | `Callable` | `Callable`（自定义） | 可调用对象 `obj(args)` |
| `__getitem__` | `Index` | `std::ops::Index` | 索引读取 `obj[key]` |
| `__setitem__` | `IndexMut` | `std::ops::IndexMut` | 索引写入 `obj[key] = val` |
| `__lpipe__` | — | — | 管道左侧数据变换 `a \|> b`：先调 `a.__lpipe__()` 产出数据（默认返回自身） |
| `__rpipe__` | — | — | 管道右侧处理工厂 `a \|> b`：`b.__rpipe__(a)` 返回单参函数处理数据（优先于 `__call__`） |
| `__is_ok__` | `SpreadOk` | `SpreadOk`（自定义） | 可传播判定 `obj?`：`True`=成功值（见 09 §4.3） |
| `__unwrap__` | `SpreadOk` | `SpreadOk`（自定义） | 可传播成功值解包：`obj?` 成功分支的结果 |
| `__err__` | `SpreadErr` | `SpreadErr`（自定义） | 可传播错误值提取：`obj?` 错误分支向上传播的内容 |

- `__call__` 使用 `Custom(MagicDesc)` 通用分支生成，trait 方法签名为 `fn call(self, args: (A, B)) -> R`，入参打包为元组，返回类型为关联类型 `Output`
- `__getitem__` 使用 `&self` 借用，`__setitem__` 使用 `&mut self` 可变借用
- `__getitem__`/`__setitem__` 支持按索引类型多分派

> **可调用性规则**：struct 作为构造器始终可调用；struct 实例**仅当定义了 `__call__`** 时才可调用；enum / trait / duck 均不可调用。详见 [99-内置预导入库.md](99-内置预导入库.md) 可调用性规则表。

---

## 十二、布尔 / 数学

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__bool__` | `HasBool` | `HasBool`（自定义） | 布尔测试 `if obj:`。判定链：`__bool__` → `__len__` → 默认 true |
| `__abs__` | `HasAbs` | `HasAbs`（自定义） | 绝对值 `abs(x)` |
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
- `__implicit_copy__` 使用 `&self` 借用，返回 `Self`，move 后自动植入复用
- `__implicit_default__` 无 self（关联函数）

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
| `__pos__` | `Pos` | `Pos`（自定义） | 一元正号 `+x` |

`__int__` 和 `__float__` 是"缺口魔法"：编译器为实现了它们的类型自动生成 `impl From<SelfTy> for i64` 和 `impl From<SelfTy> for f64`。

`__pos__` 通过 `Custom(MagicDesc)` 通用分支生成，关联类型 `Output` 按返回类型分派。

---

## 十七、上下文管理器

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__enter__` | `Enter` | `Enter`（自定义） | 进入 `with` 块（上下文管理器入口） |
| `__exit__` | `Exit` | `Exit`（自定义） | 退出 `with` 块（上下文管理器出口） |

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
| `__iter_strategy__` | `IntoIterator` | `std::iter::IntoIterator` | 返回按优先级排序的迭代策略列表 |

`__iter_strategy__` 在 for-in 循环中通过 `__iter_resolve` 取第一个适用策略包裹 base 迭代器（来自 `__iter__`）。对齐可控迭代器（Itor）的迭代策略选择机制。

---

## 速查索引

状态依据 2026-09-12 编译器源码核实 + 探针（转译 → rustc 编译 → 运行）实测：

| 魔法方法 | Trait | 分类 | 状态 |
|----------|-------|------|:----:|
| `__add__` | `std::ops::Add` | 算术 | ✅ |
| `__sub__` | `std::ops::Sub` | 算术 | ✅ |
| `__mul__` | `std::ops::Mul` | 算术 | ✅ |
| `__div__` | `std::ops::Div` | 算术 | ✅ |
| `__rem__` | `std::ops::Rem` | 算术 | ✅ |
| `__pow__` | `Pow` | 算术 | 🔸 已注册；数值 `**` 走内建 `.pow()`，用户 struct 无 Pow impl 生成 |
| `__bitand__` | `std::ops::BitAnd` | 位运算 | ✅ |
| `__bitor__` | `std::ops::BitOr` | 位运算 | ✅ |
| `__bitxor__` | `std::ops::BitXor` | 位运算 | ✅ |
| `__shl__` | `std::ops::Shl` | 位运算 | ✅ |
| `__shr__` | `std::ops::Shr` | 位运算 | ✅ |
| `__neg__` | `std::ops::Neg` | 一元 | ✅ |
| `__not__` | `std::ops::Not` | 一元 | ✅ |
| `__invert__` | `std::ops::Not`（复用） | 一元 | ✅ `~a`/`!a`/`not a` 分派，`__not__` 缺席时回退 |
| `__iadd__` | `std::ops::AddAssign` | 复合赋值 | ✅ |
| `__isub__` | `std::ops::SubAssign` | 复合赋值 | ✅ |
| `__imul__` | `std::ops::MulAssign` | 复合赋值 | ✅ |
| `__idiv__` | `std::ops::DivAssign` | 复合赋值 | ✅ |
| `__eq__` | `std::cmp::PartialEq` | 比较 | ✅ |
| `__ne__` | `std::cmp::PartialEq` | 比较 | ✅ 未定义时由 `!__eq__` 派生 |
| `__lt__` | `std::cmp::PartialOrd` | 比较 | ✅ `if a < b` 直派 + PartialOrd 由 `__eq__`+`__lt__` 推导 |
| `__le__` | `std::cmp::PartialOrd` | 比较 | ✅ |
| `__gt__` | `std::cmp::PartialOrd` | 比较 | ✅ |
| `__ge__` | `std::cmp::PartialOrd` | 比较 | ✅ |
| `__cmp__` | `std::cmp::Ord` | 比较 | ✅ `__eq__`+`__lt__` 同存时生成 Ord impl（int -1/0/1 → Ordering）+ Eq 标记 impl |
| `__hash__` | `std::hash::Hash` | 比较 | ✅ Hash impl 委托 `__hash__()` |
| `__from__` | `std::convert::From` | 类型转换 | ✅ From impl 生成 + `let`/实参/返回值三触发点隐式转换；A↔B 双向 `__from__` 编译期报循环错误 |
| `__into__` | `std::convert::Into` | 类型转换 | ✅ Into impl 生成；显式 `.__into__()` 调用 |
| `__cast__` | `Cast` | 类型转换 | ✅ `x as T` 调用点直派 `x.__cast__()`（原生类型间仍走内置 `as`） |
| `__try_cast__` | `TryCast` | 类型转换 | ✅ 仅定义 `__try_cast__` 时 `x as T` → `x.__try_cast__().unwrap()`（失败 panic） |
| `__try_from__` | `std::convert::TryFrom` | 类型转换 | ✅ TryFrom impl 生成（`type Error = E` + try_from 委托） |
| `__try_into__` | `std::convert::TryInto` | 类型转换 | ✅ TryInto impl 生成（`type Error = E` + try_into 委托） |
| `__str__` | `std::fmt::Display` | 显示/调试 | ✅ |
| `__repr__` | `std::fmt::Debug` | 显示/调试 | ✅ |
| `__next__` | `std::iter::Iterator` | 容器/迭代 | ✅ 返回 `Option<T>` 时生成 |
| `__iter__` | `std::iter::IntoIterator` | 容器/迭代 | ✅ 返回命名迭代器时生成 |
| `__into_iter__` | `std::iter::IntoIterator` | 容器/迭代 | ✅ for-in 分派 `iter.__into_iter__()`（返回 List 复用 Vec 迭代） |
| `__rev__` | `std::iter::DoubleEndedIterator` | 容器/迭代 | ✅ 与 __next__ 共存时生成 DoubleEndedIterator impl（next_back 委托） |
| `__size_hint__` | `std::iter::Iterator` | 容器/迭代 | ✅ impl Iterator 场景映射 `size_hint` |
| `__len__` | HasLen | 容器/迭代 | ✅ `len()`/真值链 `__len__ != 0`；无 HasLen trait 生成 |
| `__contains__` | Contains | 容器/迭代 | ✅ `x in obj` 直派；无 Contains trait 生成 |
| `__drop__` | `std::ops::Drop` | 生命周期 | ✅ Drop impl 委托 `__drop__()` |
| `__clone__` | `std::clone::Clone` | 生命周期 | ✅ 手动 Clone impl 委托 `__clone__()`（derive 兜底已让位，E0119 规避） |
| `__default__` | `std::default::Default` | 生命周期 | ✅ Default impl 委托 `__default__()` |
| `__call__` | `Callable` | 调用/索引 | ✅ `obj(args)` 直调分派 `obj.__call__(args)`（探针验证） |
| `__getitem__` | `std::ops::Index` | 调用/索引 | ✅ `obj[key]` 直派 |
| `__setitem__` | `std::ops::IndexMut` | 调用/索引 | ✅ `obj[key] = v` 直派；无 IndexMut impl 生成 |
| `__lpipe__`/`__rpipe__` | — | 管道 | ✅ 管道由通用 callable 语义驱动（2026-08-08 决策） |
| `__is_ok__`/`__unwrap__` | `SpreadOk` | 错误传播 `?` | ✅ 编译器内建协议（`?` 传播三件套） |
| `__err__` | `SpreadErr` | 错误传播 `?` | ✅ 同上 |
| `__bool__` | HasBool | 布尔/数学 | ✅ 判定链首环 `__bool__` → `__len__` → 内建 `!is_empty()`；无 HasBool trait 生成 |
| `__buildparams__` | `BuildParams` | 构建块 | ✅ `task ~: cfg` 构建块协议（`cfg.into_args()` → 元组解包传参） |
| `__implicit_copy__` | `ImplicitCopy` | 隐式策略 | ✅ `let b = a` 桥接为 `<T as ImplicitCopy>::__implicit_copy__(&a)`；trait impl 生成 |
| `__implicit_to__` | ImplicitInto | 隐式策略 | ✅ `let b: B = a` 回退桥接；trait impl 生成 |
| `__implicit_from__` | ImplicitFrom | 隐式策略 | ✅ 用户定义 `def __implicit_from__(raw: SrcTy) → Self` 生成 impl ImplicitFrom<SrcTy> 委托；内建首字段构造路径见 StructDef 声明式 |
| `__implicit_default__` | ImplicitDefault | 隐式策略 | ✅ `let x: T = default` 桥接为 `<T as ImplicitDefault>::__implicit_default__()`；trait impl 生成 |
| `__guarded_pred__` | `GuardedStrategy` | 守卫策略 | ✅ `guard obj with input` 委托语法；trait impl 生成 |
| `__guarded_action__` | `GuardedStrategy` | 守卫策略 | ✅ 与 pred 配对；false 分支自动调用 |
| `__int__` | `std::convert::From` | 类型缺口 | ✅ From<SelfTy> for i64 impl 生成 + `int(x)` 调用点直派 |
| `__float__` | `std::convert::From` | 类型缺口 | ✅ From<SelfTy> for f64 impl 生成 + `float(x)` 调用点直派 |
| `__pos__` | Pos | 类型缺口 | ✅ `+a` 分派 `a.__pos__()`；无魔术方法时数值恒等 |
| `__deref__` | `std::ops::Deref` | 运算符 | ✅ `*a` 对用户 struct 分派 `a.__deref__()`；真实引用类型保留裸解引用 |
| `__unapply__` | — | 提取器 | ✅ case struct 自动配；普通 struct 显式定义可用（`let Point(a,b)=p` 脱糖为 `__unapply__()`） |
| `__enter__` | Enter | 上下文 | ✅ with 构造链：enter → 体 → exit |
| `__exit__` | Exit | 上下文 | ✅ 未定义时跳过调用（E0599 防护） |
| `__iter_strategy__` | `std::iter::IntoIterator` | 迭代策略 | 🔸 |
| `__new__` | New | 构造 | ✅ `__lz_new` 命名统一 + 体透传 |
| `__init__` | Init | 构造 | ✅ 构造后 `__lz_init` 调用 |
| `__abs__` | — | 缺口魔法 | ✅ `abs(x)` 调用点直派 `x.__abs__()`；数值仍走内建 |

---

*上一章：[06c-trait和impl](06c-trait和impl.md)* · *下一章：[06e-模块级魔法属性](06e-模块级魔法属性.md)*

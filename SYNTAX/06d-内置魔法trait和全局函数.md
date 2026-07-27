# LZ 魔法方法 — 内置魔法 Trait 和全局函数

> 版本: 3.1 · 基于编译器源码 · 2026-07-27

本文档列举 Lang-Zong 编译器内置的全部魔法方法（`__xxx__`），以及它们自动生成的 trait 与对应的 Rust trait。

当 struct 实现了某个魔法方法时，编译器自动为该 struct 生成对应的 trait impl，同时生成一个全局函数供直接调用。

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

`__cast__[T](self) -> T` 和 `__try_cast__[T](self) -> Result<T, Error>` 对齐 Mojo 风格的泛型类型转换。

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

## 九、生命周期

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__drop__` | `Drop` | `std::ops::Drop` | 析构时清理资源 |
| `__clone__` | `Clone` | `std::clone::Clone` | 克隆对象 |
| `__default__` | `Default` | `std::default::Default` | 创建默认值 |

- `__drop__` 使用 `&mut self` 可变借用
- `__clone__` 使用 `&self` 借用，返回 `Self`
- `__default__` 无 self（关联函数），返回 `Self`

---

## 十、调用/索引

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__call__` | `Callable` | `Callable`（自定义） | 可调用对象 `obj(args)` |
| `__getitem__` | `Index` | `std::ops::Index` | 索引读取 `obj[key]` |
| `__setitem__` | `IndexMut` | `std::ops::IndexMut` | 索引写入 `obj[key] = val` |
| `__pipe__` | `Pipe` | `Pipe`（自定义） | 管道操作 `a \|> b` |

- `__call__` 使用 `Custom(MagicDesc)` 通用分支生成，trait 方法签名为 `fn call(self, args: (A, B)) -> R`，入参打包为元组，返回类型为关联类型 `Output`
- `__getitem__` 使用 `&self` 借用，`__setitem__` 使用 `&mut self` 可变借用
- `__getitem__`/`__setitem__` 支持按索引类型多分派

---

## 十一、布尔 / 数学

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__bool__` | `HasBool` | `HasBool`（自定义） | 布尔测试 `if obj:` |
| `__abs__` | `HasAbs` | `HasAbs`（自定义） | 绝对值 `abs(x)` |
| `__hash__` | `Hash` | `std::hash::Hash` | 哈希值计算（见比较） |

- `__bool__` 使用 `&self` 借用，返回 `bool`
- `__abs__` 使用 self 消费，返回 `Self`

> `__hash__` 在比较分类中已列出，此处为交叉引用。

---

## 十二、构建块

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__buildparams__` | `BuildParams` | `BuildParams`（自定义） | 构建块参数协议 `into_args` |

构建块协议允许将结构体参数打包为参数形式，用于 DSL 构建块场景。

---

## 十三、隐式策略

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__implicit_copy__` | `ImplicitCopy` | `ImplicitCopy`（自定义） | Mojo 风格隐式复制（move 后复用） |
| `__implicit_to__` | `ImplicitInto` | `ImplicitInto`（自定义） | Scala 风格隐式转换（类型不匹配时自动搜索） |
| `__implicit_default__` | `ImplicitDefault` | `ImplicitDefault`（自定义） | 隐式默认值填充 |

- `__implicit_copy__` 使用 `&self` 借用，返回 `Self`，move 后自动植入复用
- `__implicit_to__[T](self) -> T` 按返回类型多分派
- `__implicit_default__` 无 self（关联函数）

---

## 十四、守卫策略

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__guarded_pred__` | `GuardedStrategy` | `GuardedStrategy`（自定义） | 兜底守卫判定 `fn pred(&self, &Input) -> bool` |
| `__guarded_action__` | `GuardedStrategy` | `GuardedStrategy`（自定义） | 兜底守卫执行 `fn action(self, Input) -> Output` |

两者配对生成 `GuardedStrategy` impl。`__guarded_pred__` 判定是否执行兜底行为，`__guarded_action__` 执行兜底行为并返回结果。均支持按输入参数类型多分派。

---

## 十五、类型缺口

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__int__` | `From` | `std::convert::From` | 数值/字符串 → `i64` 整数转换 |
| `__float__` | `From` | `std::convert::From` | 数值/字符串 → `f64` 浮点转换 |
| `__pos__` | `Pos` | `Pos`（自定义） | 一元正号 `+x` |

`__int__` 和 `__float__` 是"缺口魔法"：编译器为实现了它们的类型自动生成 `impl From<SelfTy> for i64` 和 `impl From<SelfTy> for f64`。

`__pos__` 通过 `Custom(MagicDesc)` 通用分支生成，关联类型 `Output` 按返回类型分派。

---

## 十六、上下文管理器

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__enter__` | `Enter` | `Enter`（自定义） | 进入 `with` 块（上下文管理器入口） |
| `__exit__` | `Exit` | `Exit`（自定义） | 退出 `with` 块（上下文管理器出口） |

`__enter__` 消费 self，返回 `Guard` 类型；`__exit__` 使用 `&mut self` 可变借用，与 `__enter__` 配对实现 RAII 风格的资源管理。

---

## 十七、迭代策略

| 魔法方法 | 生成 Trait | Rust Trait | 用途 |
|----------|-----------|-----------|------|
| `__iter_strategy__` | `IntoIterator` | `std::iter::IntoIterator` | 返回按优先级排序的迭代策略列表 |

`__iter_strategy__` 在 for-in 循环中通过 `__iter_resolve` 取第一个适用策略包裹 base 迭代器（来自 `__iter__`）。对齐可控迭代器（Itor）的迭代策略选择机制。

---

## 速查索引

| 魔法方法 | Trait | 分类 |
|----------|-------|------|
| `__add__` | `std::ops::Add` | 算术 |
| `__sub__` | `std::ops::Sub` | 算术 |
| `__mul__` | `std::ops::Mul` | 算术 |
| `__div__` | `std::ops::Div` | 算术 |
| `__rem__` | `std::ops::Rem` | 算术 |
| `__pow__` | `Pow` | 算术 |
| `__bitand__` | `std::ops::BitAnd` | 位运算 |
| `__bitor__` | `std::ops::BitOr` | 位运算 |
| `__bitxor__` | `std::ops::BitXor` | 位运算 |
| `__shl__` | `std::ops::Shl` | 位运算 |
| `__shr__` | `std::ops::Shr` | 位运算 |
| `__neg__` | `std::ops::Neg` | 一元 |
| `__not__` | `std::ops::Not` | 一元 |
| `__invert__` | `HasInvert` | 一元 |
| `__iadd__` | `std::ops::AddAssign` | 复合赋值 |
| `__isub__` | `std::ops::SubAssign` | 复合赋值 |
| `__imul__` | `std::ops::MulAssign` | 复合赋值 |
| `__idiv__` | `std::ops::DivAssign` | 复合赋值 |
| `__eq__` | `std::cmp::PartialEq` | 比较 |
| `__ne__` | `std::cmp::PartialEq` | 比较 |
| `__lt__` | `std::cmp::PartialOrd` | 比较 |
| `__le__` | `std::cmp::PartialOrd` | 比较 |
| `__gt__` | `std::cmp::PartialOrd` | 比较 |
| `__ge__` | `std::cmp::PartialOrd` | 比较 |
| `__cmp__` | `std::cmp::Ord` | 比较 |
| `__hash__` | `std::hash::Hash` | 比较 |
| `__from__` | `std::convert::From` | 类型转换 |
| `__into__` | `std::convert::Into` | 类型转换 |
| `__cast__` | `Cast` | 类型转换 |
| `__try_cast__` | `TryCast` | 类型转换 |
| `__try_from__` | `std::convert::TryFrom` | 类型转换 |
| `__try_into__` | `std::convert::TryInto` | 类型转换 |
| `__str__` | `std::fmt::Display` | 显示/调试 |
| `__repr__` | `std::fmt::Debug` | 显示/调试 |
| `__next__` | `std::iter::Iterator` | 容器/迭代 |
| `__iter__` | `std::iter::IntoIterator` | 容器/迭代 |
| `__into_iter__` | `std::iter::IntoIterator` | 容器/迭代 |
| `__rev__` | `std::iter::DoubleEndedIterator` | 容器/迭代 |
| `__size_hint__` | `std::iter::Iterator` | 容器/迭代 |
| `__len__` | `HasLen` | 容器/迭代 |
| `__contains__` | `Contains` | 容器/迭代 |
| `__drop__` | `std::ops::Drop` | 生命周期 |
| `__clone__` | `std::clone::Clone` | 生命周期 |
| `__default__` | `std::default::Default` | 生命周期 |
| `__call__` | `Callable` | 调用/索引 |
| `__getitem__` | `std::ops::Index` | 调用/索引 |
| `__setitem__` | `std::ops::IndexMut` | 调用/索引 |
| `__pipe__` | `Pipe` | 调用/索引 |
| `__bool__` | `HasBool` | 布尔/数学 |
| `__abs__` | `HasAbs` | 布尔/数学 |
| `__buildparams__` | `BuildParams` | 构建块 |
| `__implicit_copy__` | `ImplicitCopy` | 隐式策略 |
| `__implicit_to__` | `ImplicitInto` | 隐式策略 |
| `__implicit_default__` | `ImplicitDefault` | 隐式策略 |
| `__guarded_pred__` | `GuardedStrategy` | 守卫策略 |
| `__guarded_action__` | `GuardedStrategy` | 守卫策略 |
| `__int__` | `std::convert::From` | 类型缺口 |
| `__float__` | `std::convert::From` | 类型缺口 |
| `__pos__` | `Pos` | 类型缺口 |
| `__enter__` | `Enter` | 上下文 |
| `__exit__` | `Exit` | 上下文 |
| `__iter_strategy__` | `std::iter::IntoIterator` | 迭代策略 |

---

*上一章：[06c-trait和impl](06c-trait和impl.md)* · *下一章：[06e-模块级魔法属性](06e-模块级魔法属性.md)*

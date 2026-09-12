// lz_builtins::ops — LZ 特有运算符 / 魔法方法 trait
// 注意：Rust std 已有的 trait (Clone/Default/Drop/PartialEq/Iterator 等)
// 直接使用 std 版本，不在此重复定义以避免命名冲突。

// ══════════════════════════════════════════════════════════════
// ImplicitFrom — 隐式类型转换 (LZ 特有)
// ══════════════════════════════════════════════════════════════

pub trait ImplicitFrom<T> {
    fn __implicit_from__(value: T) -> Self;
}

impl<T> ImplicitFrom<T> for T {
    fn __implicit_from__(value: T) -> Self {
        value
    }
}

// ══════════════════════════════════════════════════════════════
// ImplicitInto — 隐式类型转换目标端（06d §十四，由 ImplicitFrom blanket 派生）
// ══════════════════════════════════════════════════════════════

pub trait ImplicitInto<T> {
    fn __implicit_into__(self) -> T;
}

impl<S, T> ImplicitInto<T> for S where S: crate::runtime::ImplicitFrom<T> {
    fn __implicit_into__(self) -> T {
        // 注意：实际触发点由编译器决策，此处为 trait 占位
        <S as crate::runtime::ImplicitFrom<T>>::__implicit_from__(self)
    }
}

// ══════════════════════════════════════════════════════════════
// ImplicitCopy — Mojo 风格隐式复制（06d §十四）
// ══════════════════════════════════════════════════════════════

pub trait ImplicitCopy {
    fn __implicit_copy__(&self) -> Self;
}

// ══════════════════════════════════════════════════════════════
// ImplicitDefault — 隐式默认值填充（06d §十四）
// ══════════════════════════════════════════════════════════════

pub trait ImplicitDefault {
    fn __implicit_default__() -> Self;
}

// ══════════════════════════════════════════════════════════════
// LZ 显示 / 调试 trait (不与 std::fmt 冲突)
// ══════════════════════════════════════════════════════════════

pub trait LzStr {
    fn __str__(&self) -> String;
}

pub trait LzRepr {
    fn __repr__(&self) -> String;
}

// ══════════════════════════════════════════════════════════════
// 可调用 trait (LZ 特有)
// ══════════════════════════════════════════════════════════════

pub trait Callable<Args> {
    type Output;
    fn __call__(&self, args: Args) -> Self::Output;
}

// ══════════════════════════════════════════════════════════════
// 构建块参数 trait（LZ 特有，06d §十三）
// ══════════════════════════════════════════════════════════════

/// 构建块参数协议（06d §十三）：实现此 trait 的结构体可作为构建块
/// (~: / *:) 的载荷，`into_args` 返回参数元组供 DSL 构建块调用。
pub trait BuildParams {
    type Args: 'static;
    fn into_args(&self) -> Self::Args;
}

// ══════════════════════════════════════════════════════════════
// 索引 trait (LZ 特有，不同于 std::ops::Index)
// ══════════════════════════════════════════════════════════════

pub trait LzIndex<Idx> {
    type Output;
    fn __getitem__(&self, index: Idx) -> &Self::Output;
}

pub trait LzIndexMut<Idx>: LzIndex<Idx> {
    fn __setitem__(&mut self, index: Idx, value: Self::Output);
}

// ══════════════════════════════════════════════════════════════
// LZ 迭代器扩展 trait (补充 std::iter::Iterator)
// ══════════════════════════════════════════════════════════════

pub trait LzIterable {
    type Item;
    fn __iter__(&self) -> Box<dyn Iterator<Item = Self::Item>>;
}

// ══════════════════════════════════════════════════════════════
// LZ 算术 trait — 当需要自定义 __add__ 等方法时使用
// 这些不和 std::ops 冲突，因为有 Lz 前缀
// ══════════════════════════════════════════════════════════════

pub trait LzAdd<Rhs = Self> {
    type Output;
    fn __add__(self, rhs: Rhs) -> Self::Output;
}

pub trait LzSub<Rhs = Self> {
    type Output;
    fn __sub__(self, rhs: Rhs) -> Self::Output;
}

pub trait LzMul<Rhs = Self> {
    type Output;
    fn __mul__(self, rhs: Rhs) -> Self::Output;
}

pub trait LzDiv<Rhs = Self> {
    type Output;
    fn __div__(self, rhs: Rhs) -> Self::Output;
}

pub trait LzNeg {
    type Output;
    fn __neg__(self) -> Self::Output;
}

// 整数默认实现
impl LzAdd for i64 {
    type Output = i64;
    fn __add__(self, rhs: i64) -> i64 {
        self + rhs
    }
}
impl LzSub for i64 {
    type Output = i64;
    fn __sub__(self, rhs: i64) -> i64 {
        self - rhs
    }
}
impl LzMul for i64 {
    type Output = i64;
    fn __mul__(self, rhs: i64) -> i64 {
        self * rhs
    }
}
impl LzDiv for i64 {
    type Output = i64;
    fn __div__(self, rhs: i64) -> i64 {
        self / rhs
    }
}
impl LzNeg for i64 {
    type Output = i64;
    fn __neg__(self) -> i64 {
        -self
    }
}

impl LzAdd for f64 {
    type Output = f64;
    fn __add__(self, rhs: f64) -> f64 {
        self + rhs
    }
}
impl LzSub for f64 {
    type Output = f64;
    fn __sub__(self, rhs: f64) -> f64 {
        self - rhs
    }
}
impl LzMul for f64 {
    type Output = f64;
    fn __mul__(self, rhs: f64) -> f64 {
        self * rhs
    }
}
impl LzDiv for f64 {
    type Output = f64;
    fn __div__(self, rhs: f64) -> f64 {
        self / rhs
    }
}
impl LzNeg for f64 {
    type Output = f64;
    fn __neg__(self) -> f64 {
        -self
    }
}

// 字符串拼接：LZ 的 `a + b` 对字符串执行拼接（语义同 Python str + str）。
// Rust 原生 String 未实现 Add<String>，故经 LzAdd 分派；泛型闭包（如
// fold 的 `acc + s`）凭此对 i64 与 String 同时合法。
impl LzAdd for String {
    type Output = String;
    fn __add__(self, rhs: String) -> String {
        format!("{}{}", self, rhs)
    }
}

// ══════════════════════════════════════════════════════════════
// 类型转换 trait
// ══════════════════════════════════════════════════════════════

pub trait LzFrom<T> {
    fn __from__(value: T) -> Self;
}

pub trait LzTryFrom<T>: Sized {
    type Error;
    fn try_from(value: T) -> Result<Self, Self::Error>;
}

impl LzFrom<i64> for f64 {
    fn __from__(v: i64) -> f64 {
        v as f64
    }
}
impl LzFrom<f64> for i64 {
    fn __from__(v: f64) -> i64 {
        v as i64
    }
}
impl LzFrom<i64> for String {
    fn __from__(v: i64) -> String {
        v.to_string()
    }
}
impl LzFrom<bool> for String {
    fn __from__(v: bool) -> String {
        v.to_string()
    }
}

// ══════════════════════════════════════════════════════════════
// 守卫策略 trait（LZ 特有，06d §十）
// ══════════════════════════════════════════════════════════════

/// 守卫策略协议（06d §十）：
/// - `pred(&self, &Input) -> bool` — 判定是否执行兜底行为
/// - `action(self, Input) -> Output` — 执行兜底行为
/// 用户定义 `__guarded_pred__` / `__guarded_action__` 方法，
/// codegen 自动生成 impl GuardedStrategy for Struct。
pub trait GuardedStrategy<Input> {
    type Output;
    fn pred(&self, input: &Input) -> bool;
    fn action(self, input: Input) -> Self::Output;
}

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

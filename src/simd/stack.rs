// Lang-Zong SIMD — simd/stack.rs
// SimdStack: 栈分配定长 SIMD 向量 → 对标 Rust [T; N] 数组
//
// 适用场景：向量宽度在编译期已知，无需堆分配
// 零开销：直接映射到 CPU 寄存器或栈内存

use super::dtype::DType;
use super::Simd;
use std::fmt;

/// 栈 SIMD — 编译期固定宽度，栈上分配
///
/// 对标 Mojo `SIMD[DType.float32, 8]` → Rust `[f32; 8]`
#[derive(Clone, Copy)]
pub struct SimdStack<const N: usize> {
    dtype: DType,
    data: [f64; N],
}

impl<const N: usize> SimdStack<N> {
    /// 零初始化
    pub fn zero(dtype: DType) -> Self {
        Self { dtype, data: [0.0; N] }
    }

    /// 从元素列表构造
    pub fn from_elements(dtype: DType, elements: &[f64]) -> Self {
        let mut data = [0.0; N];
        let len = elements.len().min(N);
        data[..len].copy_from_slice(&elements[..len]);
        Self { dtype, data }
    }

    /// 标量广播到所有 lane（对标 Mojo splat）
    pub fn splat(dtype: DType, value: f64) -> Self {
        Self { dtype, data: [value; N] }
    }

    /// 获取底层数组引用
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    /// 获取底层数组可变引用
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut self.data
    }

    /// 读写 lane
    pub fn get(&self, idx: usize) -> f64 { self.data[idx] }
    pub fn set(&mut self, idx: usize, val: f64) { self.data[idx] = val; }

    /// 位宽（字节）
    pub fn byte_width(&self) -> usize { N * 8 }

    // ── 元素级运算 ──

    pub fn add(&self, other: &Self) -> Self {
        let mut r = Self::zero(self.dtype);
        for i in 0..N { r.data[i] = self.data[i] + other.data[i]; }
        r
    }

    pub fn sub(&self, other: &Self) -> Self {
        let mut r = Self::zero(self.dtype);
        for i in 0..N { r.data[i] = self.data[i] - other.data[i]; }
        r
    }

    pub fn mul(&self, other: &Self) -> Self {
        let mut r = Self::zero(self.dtype);
        for i in 0..N { r.data[i] = self.data[i] * other.data[i]; }
        r
    }

    pub fn scale(&self, scalar: f64) -> Self {
        let mut r = Self::zero(self.dtype);
        for i in 0..N { r.data[i] = self.data[i] * scalar; }
        r
    }

    // ── 归约 ──

    pub fn reduce_add(&self) -> f64 { self.data.iter().sum() }
    pub fn reduce_mul(&self) -> f64 { self.data.iter().product() }
    pub fn reduce_max(&self) -> f64 { self.data.iter().cloned().fold(f64::NEG_INFINITY, f64::max) }
    pub fn reduce_min(&self) -> f64 { self.data.iter().cloned().fold(f64::INFINITY, f64::min) }

    // ── 混洗 / 重排 ──

    /// 对标 Mojo shuffle：按索引数组重排 lane
    pub fn shuffle(&self, indices: &[usize]) -> Self {
        let mut r = Self::zero(self.dtype);
        for (i, &idx) in indices.iter().enumerate() {
            if i < N && idx < N { r.data[i] = self.data[idx]; }
        }
        r
    }

    /// 向量比较 (相等)
    pub fn eq(&self, other: &Self) -> [bool; N] {
        let mut r = [false; N];
        for i in 0..N { r[i] = self.data[i] == other.data[i]; }
        r
    }

    /// 向量比较 (大于)
    pub fn gt(&self, other: &Self) -> [bool; N] {
        let mut r = [false; N];
        for i in 0..N { r[i] = self.data[i] > other.data[i]; }
        r
    }

    /// 条件选择（对标 Mojo select）
    pub fn select(mask: &[bool; N], true_val: &Self, false_val: &Self) -> Self {
        let mut r = Self::zero(true_val.dtype);
        for i in 0..N {
            r.data[i] = if mask[i] { true_val.data[i] } else { false_val.data[i] };
        }
        r
    }
}

impl<const N: usize> Simd for SimdStack<N> {
    fn dtype(&self) -> DType { self.dtype }
    fn len(&self) -> usize { N }
    fn reduce_sum(&self) -> f64 { self.reduce_add() }
    fn reduce_max(&self) -> f64 { self.reduce_max() }
    fn reduce_min(&self) -> f64 { self.reduce_min() }
    fn lane(&self, idx: usize) -> f64 { self.get(idx) }

    fn map(&self, f: &dyn Fn(f64) -> f64) -> Vec<f64> {
        self.data.iter().map(|&x| f(x)).collect()
    }
}

impl<const N: usize> fmt::Debug for SimdStack<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.data.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_splat() {
        let v = SimdStack::<4>::splat(DType::F32, 3.0);
        assert_eq!(v.as_slice(), &[3.0; 4]);
    }

    #[test]
    fn test_stack_add() {
        let a = SimdStack::<4>::from_elements(DType::F32, &[1.0, 2.0, 3.0, 4.0]);
        let b = SimdStack::<4>::splat(DType::F32, 1.0);
        let c = a.add(&b);
        assert_eq!(c.as_slice(), &[2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_stack_reduce() {
        let v = SimdStack::<8>::from_elements(DType::I32, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        assert_eq!(v.reduce_add(), 36.0);
        assert_eq!(v.reduce_max(), 8.0);
        assert_eq!(v.reduce_min(), 1.0);
    }

    #[test]
    fn test_stack_shuffle() {
        let v = SimdStack::<4>::from_elements(DType::F32, &[1.0, 2.0, 3.0, 4.0]);
        let shuffled = v.shuffle(&[3, 2, 1, 0]); // reverse
        assert_eq!(shuffled.as_slice(), &[4.0, 3.0, 2.0, 1.0]);
    }

    #[test]
    fn test_select() {
        let t = SimdStack::<4>::from_elements(DType::F32, &[10.0, 20.0, 30.0, 40.0]);
        let f = SimdStack::<4>::splat(DType::F32, -1.0);
        let mask = [true, false, true, false];
        let r = SimdStack::<4>::select(&mask, &t, &f);
        assert_eq!(r.as_slice(), &[10.0, -1.0, 30.0, -1.0]);
    }
}

// Lang-Zong SIMD — simd/vector.rs
// SimdVec: 动态扩容 SIMD 向量 → 对标 Rust Vec<T>

use super::dtype::DType;
use super::Simd;
use std::fmt;

pub struct SimdVec {
    dtype: DType,
    data: Vec<f64>,
}

impl SimdVec {
    pub fn new(dtype: DType) -> Self { Self { dtype, data: Vec::new() } }
    pub fn with_capacity(dtype: DType, cap: usize) -> Self { Self { dtype, data: Vec::with_capacity(cap) } }
    pub fn splat(dtype: DType, len: usize, value: f64) -> Self { Self { dtype, data: vec![value; len] } }
    pub fn from_slice(dtype: DType, s: &[f64]) -> Self { Self { dtype, data: s.to_vec() } }

    pub fn len(&self) -> usize { self.data.len() }
    pub fn as_slice(&self) -> &[f64] { &self.data }
    pub fn get(&self, idx: usize) -> f64 { self.data[idx] }

    pub fn push(&mut self, v: f64) { self.data.push(v); }
    pub fn extend(&mut self, iter: impl IntoIterator<Item = f64>) { self.data.extend(iter); }

    // 运算
    pub fn add_assign(&mut self, other: &SimdVec) {
        for i in 0..self.len().min(other.len()) { self.data[i] += other.data[i]; }
    }
    pub fn scale_assign(&mut self, s: f64) {
        for x in &mut self.data { *x *= s; }
    }
    pub fn reduce_add(&self) -> f64 { self.data.iter().sum() }
    pub fn reduce_max(&self) -> f64 { self.data.iter().cloned().fold(f64::NEG_INFINITY, f64::max) }
    pub fn reduce_min(&self) -> f64 { self.data.iter().cloned().fold(f64::INFINITY, f64::min) }
    /// 转换为 SimdBox（冻结长度）
    pub fn into_boxed(self) -> super::SimdBox { super::SimdBox::from_slice(self.dtype, &self.data) }
}

impl Simd for SimdVec {
    fn dtype(&self) -> DType { self.dtype }
    fn len(&self) -> usize { self.data.len() }
    fn reduce_sum(&self) -> f64 { self.reduce_add() }
    fn reduce_max(&self) -> f64 { self.reduce_max() }
    fn reduce_min(&self) -> f64 { self.reduce_min() }
    fn lane(&self, idx: usize) -> f64 { self.get(idx) }
    fn map(&self, f: &dyn Fn(f64) -> f64) -> Vec<f64> { self.data.iter().map(|&x| f(x)).collect() }
    
}

impl fmt::Debug for SimdVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.data.iter()).finish()
    }
}

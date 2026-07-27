// Lang-Zong SIMD — simd/boxed.rs
// SimdBox: 堆分配定长 SIMD → 对标 Rust Box<[T]> (独占所有权)

use super::dtype::DType;
use super::Simd;
use std::fmt;

/// 堆分配 SIMD — 独占所有权，大小在运行时确定但不可变
pub struct SimdBox {
    dtype: DType,
    data: Box<[f64]>,
}

impl SimdBox {
    pub fn new(dtype: DType, len: usize) -> Self {
        Self { dtype, data: vec![0.0; len].into_boxed_slice() }
    }

    pub fn splat(dtype: DType, len: usize, value: f64) -> Self {
        Self { dtype, data: vec![value; len].into_boxed_slice() }
    }

    pub fn from_slice(dtype: DType, slice: &[f64]) -> Self {
        Self { dtype, data: slice.to_vec().into_boxed_slice() }
    }

    pub fn len(&self) -> usize { self.data.len() }
    pub fn as_slice(&self) -> &[f64] { &self.data }
    pub fn as_mut_slice(&mut self) -> &mut [f64] { &mut self.data }
    pub fn get(&self, idx: usize) -> f64 { self.data[idx] }
    pub fn set(&mut self, idx: usize, v: f64) { self.data[idx] = v; }
    pub fn byte_width(&self) -> usize { self.data.len() * 8 }

    // 运算
    pub fn add(&self, other: &SimdBox) -> SimdBox {
        let mut r = SimdBox::new(self.dtype, self.len());
        for i in 0..self.len() { r.data[i] = self.data[i] + other.data[i]; }
        r
    }
    pub fn scale(&self, s: f64) -> SimdBox {
        let mut r = SimdBox::new(self.dtype, self.len());
        for i in 0..self.len() { r.data[i] = self.data[i] * s; }
        r
    }
    pub fn reduce_add(&self) -> f64 { self.data.iter().sum() }
    pub fn reduce_max(&self) -> f64 { self.data.iter().cloned().fold(f64::NEG_INFINITY, f64::max) }
    pub fn reduce_min(&self) -> f64 { self.data.iter().cloned().fold(f64::INFINITY, f64::min) }
}

impl Simd for SimdBox {
    fn dtype(&self) -> DType { self.dtype }
    fn len(&self) -> usize { self.data.len() }
    fn reduce_sum(&self) -> f64 { self.reduce_add() }
    fn reduce_max(&self) -> f64 { self.reduce_max() }
    fn reduce_min(&self) -> f64 { self.reduce_min() }
    fn lane(&self, idx: usize) -> f64 { self.get(idx) }
    fn map(&self, f: &dyn Fn(f64) -> f64) -> Vec<f64> { self.data.iter().map(|&x| f(x)).collect() }
    
}

impl fmt::Debug for SimdBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.data.iter()).finish()
    }
}

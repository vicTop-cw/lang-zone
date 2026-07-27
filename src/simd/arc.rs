// Lang-Zong SIMD — simd/arc.rs
// SimdArc: 共享所有权 SIMD → 对标 Rust Arc<[T]>, 线程安全

use super::dtype::DType;
use super::Simd;
use std::sync::Arc;
use std::fmt;

pub struct SimdArc {
    dtype: DType,
    data: Arc<[f64]>,
}

impl SimdArc {
    pub fn new(dtype: DType, len: usize) -> Self {
        Self { dtype, data: vec![0.0; len].into() }
    }
    pub fn splat(dtype: DType, len: usize, value: f64) -> Self {
        Self { dtype, data: vec![value; len].into() }
    }
    pub fn from_slice(dtype: DType, s: &[f64]) -> Self {
        Self { dtype, data: s.to_vec().into() }
    }

    pub fn len(&self) -> usize { self.data.len() }
    pub fn as_slice(&self) -> &[f64] { &self.data }
    pub fn get(&self, idx: usize) -> f64 { self.data[idx] }

    /// 克隆 Arc（共享所有权）
    pub fn share(&self) -> Self {
        Self { dtype: self.dtype, data: Arc::clone(&self.data) }
    }
    /// 引用计数
    pub fn ref_count(&self) -> usize { Arc::strong_count(&self.data) }

    // 运算（返回新 SimdArc）
    pub fn add(&self, other: &SimdArc) -> SimdArc {
        let mut v = vec![0.0; self.len()];
        for i in 0..self.len() { v[i] = self.data[i] + other.data[i]; }
        SimdArc { dtype: self.dtype, data: v.into() }
    }
    pub fn scale(&self, s: f64) -> SimdArc {
        let mut v = vec![0.0; self.len()];
        for i in 0..self.len() { v[i] = self.data[i] * s; }
        SimdArc { dtype: self.dtype, data: v.into() }
    }
    pub fn reduce_add(&self) -> f64 { self.data.iter().sum() }
    pub fn reduce_max(&self) -> f64 { self.data.iter().cloned().fold(f64::NEG_INFINITY, f64::max) }
    pub fn reduce_min(&self) -> f64 { self.data.iter().cloned().fold(f64::INFINITY, f64::min) }
}

impl Clone for SimdArc {
    fn clone(&self) -> Self { self.share() }
}

impl Simd for SimdArc {
    fn dtype(&self) -> DType { self.dtype }
    fn len(&self) -> usize { self.data.len() }
    fn reduce_sum(&self) -> f64 { self.reduce_add() }
    fn reduce_max(&self) -> f64 { self.reduce_max() }
    fn reduce_min(&self) -> f64 { self.reduce_min() }
    fn lane(&self, idx: usize) -> f64 { self.get(idx) }
    fn map(&self, f: &dyn Fn(f64) -> f64) -> Vec<f64> { self.data.iter().map(|&x| f(x)).collect() }
    
}

// Send + Sync — Arc 天然线程安全
unsafe impl Send for SimdArc {}
unsafe impl Sync for SimdArc {}

impl fmt::Debug for SimdArc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimdArc").field("len", &self.data.len()).field("refs", &self.ref_count()).finish()
    }
}

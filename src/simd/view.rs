// Lang-Zong SIMD — simd/view.rs
// SimdView: 借用视图 → 对标 Rust &[T] / &mut [T], 零拷贝

use super::dtype::DType;
use super::Simd;
use std::fmt;

/// 不可变借用视图
pub struct SimdView<'a> {
    dtype: DType,
    data: &'a [f64],
}

impl<'a> SimdView<'a> {
    pub fn new(dtype: DType, data: &'a [f64]) -> Self { Self { dtype, data } }
    pub fn len(&self) -> usize { self.data.len() }
    pub fn as_slice(&self) -> &[f64] { self.data }
    pub fn get(&self, idx: usize) -> f64 { self.data[idx] }
    pub fn reduce_add(&self) -> f64 { self.data.iter().sum() }
    pub fn reduce_max(&self) -> f64 { self.data.iter().cloned().fold(f64::NEG_INFINITY, f64::max) }
    pub fn reduce_min(&self) -> f64 { self.data.iter().cloned().fold(f64::INFINITY, f64::min) }

    /// 转换成拥有的 SimdBox
    pub fn to_owned(&self) -> super::SimdBox { super::SimdBox::from_slice(self.dtype, self.data) }
}

impl<'a> Simd for SimdView<'a> {
    fn dtype(&self) -> DType { self.dtype }
    fn len(&self) -> usize { self.data.len() }
    fn reduce_sum(&self) -> f64 { self.reduce_add() }
    fn reduce_max(&self) -> f64 { self.reduce_max() }
    fn reduce_min(&self) -> f64 { self.reduce_min() }
    fn lane(&self, idx: usize) -> f64 { self.get(idx) }
    fn map(&self, f: &dyn Fn(f64) -> f64) -> Vec<f64> { self.data.iter().map(|&x| f(x)).collect() }
    
}

impl<'a> fmt::Debug for SimdView<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.data.iter()).finish()
    }
}

// Lang-Zong SIMD — simd/mod.rs
// 包入口：Simd trait + 五种智能指针分类重导出
//
// 对标 Mojo SIMD[DType, size] — 按 Rust 所有权模型分化：
//
//   SimdStack<D, N>    →  [T; N]    栈分配，编译期定长
//   SimdBox<D>          →  Box<[T]>  堆分配，独占所有权
//   SimdVec<D>          →  Vec<T>    堆分配，动态扩容
//   SimdArc<D>          →  Arc<[T]>  共享所有权，并发安全
//   SimdView<'a, D>     →  &[T]      借用视图，零拷贝

pub mod dtype;
pub mod layout;
pub mod ops;
#[cfg(test)]
mod bench;
mod stack;
mod boxed;
mod vector;
mod arc;
mod view;

pub use dtype::{DType, SimdWidth};
pub use layout::{SimdLayout, AlignedAlloc, CACHE_LINE, PAGE_SIZE};
pub use ops::SimdOps;
pub use stack::SimdStack;
pub use boxed::SimdBox;
pub use vector::SimdVec;
pub use arc::SimdArc;
pub use view::SimdView;

// ──────────────── Simd 核心 trait ────────────────

/// SIMD 向量统一接口
///
/// 对标 Mojo `SIMD[dtype, size]` 的方法集：
/// 元素级运算、归约、混洗、类型转换
pub trait Simd {
    /// 元素类型
    fn dtype(&self) -> DType;
    /// 元素数量（向量宽度）
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }

    // ── 归约 ──
    fn reduce_sum(&self) -> f64;
    fn reduce_max(&self) -> f64;
    fn reduce_min(&self) -> f64;

    // ── 元素级 ──
    fn map(&self, f: &dyn Fn(f64) -> f64) -> Vec<f64>;

    // ── 访问 ──
    fn lane(&self, idx: usize) -> f64;
}

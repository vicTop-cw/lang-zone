// Lang-Zong SIMD — simd/layout.rs
// 内存布局与对齐：cache-line 优化、对齐分配、SIMD 友好布局
//
// 对标 Mojo 的内存语义 + Rust 的 `#[repr(align(N))]`

use super::dtype::DType;

/// SIMD 向量的内存布局描述
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimdLayout {
    /// 元素类型
    pub dtype: DType,
    /// 元素数量
    pub len: usize,
    /// 对齐要求（字节）
    pub alignment: usize,
    /// 总字节数
    pub byte_size: usize,
}

impl SimdLayout {
    /// 创建布局描述
    pub const fn new(dtype: DType, len: usize) -> Self {
        let elem_bytes = dtype.byte_width();
        let alignment = Self::compute_alignment(dtype, len);
        Self {
            dtype,
            len,
            alignment,
            byte_size: len * elem_bytes,
        }
    }

    /// 计算给定 SIMD 向量的最优对齐
    ///
    /// 规则：
    /// - 总字节数 ≤ 16 → 16 字节对齐（SSE）
    /// - 总字节数 ≤ 32 → 32 字节对齐（AVX2）
    /// - 总字节数 ≤ 64 → 64 字节对齐（AVX-512）
    /// - 更大 → cache-line 对齐（64 字节）
    pub const fn compute_alignment(dtype: DType, len: usize) -> usize {
        let total = len * dtype.byte_width();
        if total <= 16 { 16 }
        else if total <= 32 { 32 }
        else { 64 } // AVX-512 / cache line
    }

    /// 是否为 cache-line 对齐（64 字节）
    pub const fn is_cacheline_aligned(&self) -> bool {
        self.alignment >= 64
    }

    /// 对齐后的总字节数（含 padding）
    pub const fn padded_byte_size(&self) -> usize {
        let raw = self.byte_size;
        let align = self.alignment;
        ((raw + align - 1) / align) * align
    }

    /// 计算给定内存地址是否对齐
    pub fn is_aligned(ptr: *const u8, alignment: usize) -> bool {
        (ptr as usize) % alignment == 0
    }
}

/// CPU Cache line 常量
pub const CACHE_LINE: usize = 64;

/// 页面大小（4KB，标准 x86/ARM）
pub const PAGE_SIZE: usize = 4096;

/// 对齐到 cache line 边界
pub const fn align_to_cache_line(size: usize) -> usize {
    ((size + CACHE_LINE - 1) / CACHE_LINE) * CACHE_LINE
}

/// 对齐到指定边界
pub const fn align_to(size: usize, alignment: usize) -> usize {
    ((size + alignment - 1) / alignment) * alignment
}

/// SIMD 友好的内存分配器封装
/// 对齐分配 + 释放，对标 `std::alloc` 但强制指定对齐
pub struct AlignedAlloc;

impl AlignedAlloc {
    /// 分配对齐内存块（返回裸指针）
    pub fn alloc(byte_size: usize, alignment: usize) -> *mut u8 {
        let layout = std::alloc::Layout::from_size_align(byte_size, alignment)
            .expect("AlignedAlloc: invalid layout");
        unsafe { std::alloc::alloc(layout) }
    }

    /// 分配零初始化对齐内存块
    pub fn alloc_zeroed(byte_size: usize, alignment: usize) -> *mut u8 {
        let layout = std::alloc::Layout::from_size_align(byte_size, alignment)
            .expect("AlignedAlloc: invalid layout");
        unsafe { std::alloc::alloc_zeroed(layout) }
    }

    /// 释放对齐内存
    pub fn free(ptr: *mut u8, byte_size: usize, alignment: usize) {
        let layout = std::alloc::Layout::from_size_align(byte_size, alignment)
            .expect("AlignedAlloc: invalid layout");
        unsafe { std::alloc::dealloc(ptr, layout) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_computation() {
        let l = SimdLayout::new(DType::F32, 8); // 8 × f32 = 32 bytes
        assert_eq!(l.byte_size, 32);
        assert_eq!(l.alignment, 32); // ≤32 → 32

        let l2 = SimdLayout::new(DType::F64, 8); // 8 × f64 = 64 bytes
        assert_eq!(l2.byte_size, 64);
        assert_eq!(l2.alignment, 64);
    }

    #[test]
    fn test_align_math() {
        assert_eq!(align_to_cache_line(100), 128);
        assert_eq!(align_to_cache_line(64), 64);
        assert_eq!(align_to(17, 16), 32);
    }

    #[test]
    fn test_aligned_alloc() {
        let ptr = AlignedAlloc::alloc_zeroed(128, 64);
        assert!(SimdLayout::is_aligned(ptr, 64));
        AlignedAlloc::free(ptr, 128, 64);
    }
}

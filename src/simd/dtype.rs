// Lang-Zong SIMD — simd/dtype.rs
// 对标 Mojo DType：SIMD 支持的元素类型枚举
//
// 设计原则：
//   每种类型有固定的位宽，编译期确定
//   覆盖有符号整数、无符号整数、浮点数、布尔值

use std::fmt;

/// SIMD 元素类型（对标 Mojo `DType`）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DType {
    // 有符号整数
    I8,
    I16,
    I32,
    I64,
    // 无符号整数
    U8,
    U16,
    U32,
    U64,
    // 浮点数
    F32,
    F64,
    // 布尔（位向量）
    Bool,
}

impl DType {
    /// 位宽
    pub const fn bit_width(self) -> usize {
        match self {
            DType::I8 | DType::U8 | DType::Bool => 8,
            DType::I16 | DType::U16 => 16,
            DType::I32 | DType::U32 | DType::F32 => 32,
            DType::I64 | DType::U64 | DType::F64 => 64,
        }
    }

    /// 字节宽度
    pub const fn byte_width(self) -> usize {
        self.bit_width() / 8
    }

    /// 是否为有符号整数
    pub const fn is_signed_int(self) -> bool {
        matches!(self, DType::I8 | DType::I16 | DType::I32 | DType::I64)
    }

    /// 是否为无符号整数
    pub const fn is_unsigned_int(self) -> bool {
        matches!(self, DType::U8 | DType::U16 | DType::U32 | DType::U64)
    }

    /// 是否为整数
    pub const fn is_integer(self) -> bool {
        self.is_signed_int() || self.is_unsigned_int()
    }

    /// 是否为浮点数
    pub const fn is_float(self) -> bool {
        matches!(self, DType::F32 | DType::F64)
    }

    /// Rust 类型名（用于文档 / 代码生成）
    pub fn rust_name(self) -> &'static str {
        match self {
            DType::I8 => "i8",
            DType::I16 => "i16",
            DType::I32 => "i32",
            DType::I64 => "i64",
            DType::U8 => "u8",
            DType::U16 => "u16",
            DType::U32 => "u32",
            DType::U64 => "u64",
            DType::F32 => "f32",
            DType::F64 => "f64",
            DType::Bool => "bool",
        }
    }
}

impl fmt::Display for DType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.rust_name())
    }
}

/// 硬件 SIMD 寄存器宽度（位）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdWidth {
    B128 = 128,
    B256 = 256,
    B512 = 512,
}

impl SimdWidth {
    /// 给定元素类型，最优 SIMD 宽度
    /// 对标 Mojo `simdwidthof[DType]()`
    pub const fn detect() -> Self {
        if cfg!(target_feature = "avx512f") {
            SimdWidth::B512
        } else if cfg!(target_feature = "avx2") {
            SimdWidth::B256
        } else {
            SimdWidth::B128 // SSE2 / NEON baseline
        }
    }

    /// 该宽度下可容纳的元素数量
    pub const fn element_count(self, dtype: DType) -> usize {
        self as usize / dtype.bit_width()
    }

    /// 字节宽度
    pub const fn byte_width(self) -> usize {
        self as usize / 8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_widths() {
        assert_eq!(DType::I8.bit_width(), 8);
        assert_eq!(DType::F32.bit_width(), 32);
        assert_eq!(DType::F64.bit_width(), 64);
    }

    #[test]
    fn test_classification() {
        assert!(DType::I32.is_signed_int());
        assert!(DType::U32.is_unsigned_int());
        assert!(DType::F32.is_float());
        assert!(!DType::Bool.is_integer());
    }

    #[test]
    fn test_simd_width_elements() {
        let w = SimdWidth::B256;
        assert_eq!(w.element_count(DType::F32), 8);  // 256/32 = 8
        assert_eq!(w.element_count(DType::F64), 4);  // 256/64 = 4
        assert_eq!(w.element_count(DType::I8), 32);  // 256/8 = 32
    }
}

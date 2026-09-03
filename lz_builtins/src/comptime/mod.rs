// lz_builtins::comptime — 仅编译期可用的工具
//
// 这些 API 仅在 LZ 编译期（comptime）上下文中可用。
// 在运行时调用会导致编译错误或 panic。

// ══════════════════════════════════════════════════════════════
// [CT] type_name — 编译期类型名
// ══════════════════════════════════════════════════════════════

/// 获取类型的编译期名称字符串
pub fn type_name<T>() -> &'static str {
    std::any::type_name::<T>()
}

/// 从引用获取类型名
pub fn type_of<T>(_val: &T) -> &'static str {
    std::any::type_name::<T>()
}

/// 获取类型的编译期唯一标识符（TypeId）
pub fn type_id<T: 'static>() -> std::any::TypeId {
    std::any::TypeId::of::<T>()
}

/// 获取类型的编译期大小（字节数）
pub fn size_of<T>() -> usize {
    std::mem::size_of::<T>()
}

/// 获取类型的编译期对齐要求（字节数）
pub fn align_of<T>() -> usize {
    std::mem::align_of::<T>()
}

/// 判断两个类型是否相同（编译期恒真/恒假）
pub fn is_same_type<T: 'static, U: 'static>() -> bool {
    std::any::TypeId::of::<T>() == std::any::TypeId::of::<U>()
}

/// 编译期警告
pub fn compile_warn(_msg: &str) {}

/// 字段内省（预留编译器实现）
pub fn fields_of<T>() -> Vec<&'static str> {
    Vec::new()
}

/// 字段数量（预留编译器实现）
pub fn field_count<T>() -> usize {
    fields_of::<T>().len()
}

// ══════════════════════════════════════════════════════════════
// 编译期 inspect 数据类型
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub name: String,
    pub params: Vec<ParameterInfo>,
    pub return_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: String,
    pub kind: ParameterKind,
    pub annotation: Option<String>,
    pub has_default: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    PositionalOnly,
    PositionalOrKeyword,
    VarPositional,
    KeywordOnly,
    VarKeyword,
}

#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub function: Option<String>,
    pub filename: String,
    pub lineno: i64,
}

// ══════════════════════════════════════════════════════════════
// inspect 子模块
// ══════════════════════════════════════════════════════════════

pub mod inspect;

pub use inspect::*;

// ══════════════════════════════════════════════════════════════
// 单元测试
// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_name() {
        assert_eq!(type_name::<i64>(), "i64");
        assert_eq!(type_name::<i32>(), "i32");
    }

    #[test]
    fn test_type_id() {
        let a = type_id::<i64>();
        let b = type_id::<i64>();
        let c = type_id::<i32>();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_size_of() {
        assert_eq!(size_of::<i8>(), 1);
        assert_eq!(size_of::<i64>(), 8);
    }

    #[test]
    fn test_is_same_type() {
        assert!(is_same_type::<i32, i32>());
        assert!(!is_same_type::<i32, i64>());
    }

    #[test]
    fn test_fields_of_empty() {
        let f: Vec<&str> = fields_of::<i64>();
        assert!(f.is_empty());
    }
}

// lz_builtins::error — LZ 错误类型与 Result 泛型
// 对齐 Rust std::error::Error + Result 模式，提供 LZ 友好的错误处理。
// 零外部依赖，纯 Rust std。

use std::fmt::{Debug, Display};

// ══════════════════════════════════════════════════════════════
// Result — 错误处理的核心类型
// ══════════════════════════════════════════════════════════════

pub type Result<T, E = LzError> = std::result::Result<T, E>;

// ══════════════════════════════════════════════════════════════
// LzError — LZ 标准错误类型
// ══════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct LzError {
    /// 错误类型标签（与 ErrorKind 对应）
    pub kind: ErrorKind,
    /// 人类可读的错误消息
    pub message: String,
    /// 底层错误来源（可选）
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
    /// 额外的错误上下文（模块特定数据）
    pub context: Vec<(String, String)>,
}

// Manual Clone impl because `dyn Error + Send + Sync` doesn't implement Clone
impl Clone for LzError {
    fn clone(&self) -> Self {
        LzError {
            kind: self.kind,
            message: self.message.clone(),
            source: None, // Can't clone trait objects; preserve kind + message
            context: self.context.clone(),
        }
    }
}

impl LzError {
    /// 从消息构造一个 LzError（kind 默认为 Generic）
    pub fn new(message: impl Into<String>) -> Self {
        LzError {
            kind: ErrorKind::Generic,
            message: message.into(),
            source: None,
            context: Vec::new(),
        }
    }

    /// 从 kind 和消息构造
    pub fn kind(kind: ErrorKind, message: impl Into<String>) -> Self {
        LzError {
            kind,
            message: message.into(),
            source: None,
            context: Vec::new(),
        }
    }

    /// 从 kind、消息和底层错误构造
    pub fn with_source<E: std::error::Error + Send + Sync + 'static>(
        kind: ErrorKind,
        message: impl Into<String>,
        source: E,
    ) -> Self {
        LzError {
            kind,
            message: message.into(),
            source: Some(Box::new(source)),
            context: Vec::new(),
        }
    }

    /// 添加上下文键值对
    pub fn context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.push((key.into(), value.into()));
        self.context.sort();
        self
    }

    /// 获取错误类型的标签
    pub fn kind_name(&self) -> &'static str {
        self.kind.name()
    }

    /// 检查是否是特定错误类型
    pub fn is(&self, kind: ErrorKind) -> bool {
        self.kind == kind
    }

    /// 转换为 io::Error（若 kind 是 IO 相关）
    pub fn into_io_error(self) -> std::io::Error {
        use std::io::ErrorKind as IoKind;
        match self.kind {
            ErrorKind::IO => std::io::Error::new(IoKind::Other, self.message.clone()),
            ErrorKind::NotFound => std::io::Error::new(IoKind::NotFound, self.message.clone()),
            ErrorKind::PermissionDenied => {
                std::io::Error::new(IoKind::PermissionDenied, self.message.clone())
            }
            ErrorKind::AlreadyExists => {
                std::io::Error::new(IoKind::AlreadyExists, self.message.clone())
            }
            ErrorKind::InvalidInput => {
                std::io::Error::new(IoKind::InvalidInput, self.message.clone())
            }
            ErrorKind::TimedOut => std::io::Error::new(IoKind::TimedOut, self.message.clone()),
            ErrorKind::ConnectionRefused => {
                std::io::Error::new(IoKind::ConnectionRefused, self.message.clone())
            }
            _ => std::io::Error::new(IoKind::Other, self.message),
        }
    }
}

impl Display for LzError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind.name(), self.message)?;
        if !self.context.is_empty() {
            write!(f, " [")?;
            for (i, (k, v)) in self.context.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}={}", k, v)?;
            }
            write!(f, "]")?;
        }
        Ok(())
    }
}

impl std::error::Error for LzError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

// ══════════════════════════════════════════════════════════════
// ErrorKind — 错误类型枚举
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    Generic,
    IO,
    NotFound,
    PermissionDenied,
    AlreadyExists,
    InvalidInput,
    TypeMismatch,
    IndexOutOfBounds,
    KeyNotFound,
    ParseError,
    JsonError,
    AssertionFailed,
    NullPointer,
    StackOverflow,
    ArithmeticError,
    TimedOut,
    ConnectionRefused,
    SerializationError,
    DeserializationError,
    ImportError,
    SyntaxError,
    Cancelled,
    Unimplemented,
    Internal,
}

impl ErrorKind {
    pub fn name(&self) -> &'static str {
        match self {
            ErrorKind::Generic => "Error",
            ErrorKind::IO => "IOError",
            ErrorKind::NotFound => "NotFoundError",
            ErrorKind::PermissionDenied => "PermissionError",
            ErrorKind::AlreadyExists => "AlreadyExistsError",
            ErrorKind::InvalidInput => "ValueError",
            ErrorKind::TypeMismatch => "TypeError",
            ErrorKind::IndexOutOfBounds => "IndexError",
            ErrorKind::KeyNotFound => "KeyError",
            ErrorKind::ParseError => "ParseError",
            ErrorKind::JsonError => "JSONDecodeError",
            ErrorKind::AssertionFailed => "AssertionError",
            ErrorKind::NullPointer => "NullError",
            ErrorKind::StackOverflow => "RecursionError",
            ErrorKind::ArithmeticError => "ArithmeticError",
            ErrorKind::TimedOut => "TimeoutError",
            ErrorKind::ConnectionRefused => "ConnectionError",
            ErrorKind::SerializationError => "SerializationError",
            ErrorKind::DeserializationError => "DeserializationError",
            ErrorKind::ImportError => "ImportError",
            ErrorKind::SyntaxError => "SyntaxError",
            ErrorKind::Cancelled => "CancelledError",
            ErrorKind::Unimplemented => "NotImplementedError",
            ErrorKind::Internal => "InternalError",
        }
    }

    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            ErrorKind::StackOverflow | ErrorKind::AssertionFailed | ErrorKind::Internal
        )
    }

    pub fn is_io(&self) -> bool {
        matches!(
            self,
            ErrorKind::IO
                | ErrorKind::NotFound
                | ErrorKind::PermissionDenied
                | ErrorKind::AlreadyExists
                | ErrorKind::TimedOut
                | ErrorKind::ConnectionRefused
        )
    }
}

// ══════════════════════════════════════════════════════════════
// 便捷构造函数
// ══════════════════════════════════════════════════════════════

pub fn error(msg: impl Into<String>) -> LzError { LzError::new(msg) }
pub fn io_error(msg: impl Into<String>) -> LzError { LzError::kind(ErrorKind::IO, msg) }
pub fn type_error(msg: impl Into<String>) -> LzError { LzError::kind(ErrorKind::TypeMismatch, msg) }
pub fn index_error(msg: impl Into<String>) -> LzError { LzError::kind(ErrorKind::IndexOutOfBounds, msg) }
pub fn value_error(msg: impl Into<String>) -> LzError { LzError::kind(ErrorKind::InvalidInput, msg) }
pub fn parse_error(msg: impl Into<String>) -> LzError { LzError::kind(ErrorKind::ParseError, msg) }
pub fn not_implemented(msg: impl Into<String>) -> LzError { LzError::kind(ErrorKind::Unimplemented, msg) }
pub fn internal_error(msg: impl Into<String>) -> LzError { LzError::kind(ErrorKind::Internal, msg) }
pub fn json_error(msg: impl Into<String>) -> LzError { LzError::kind(ErrorKind::JsonError, msg) }

// ══════════════════════════════════════════════════════════════
// Result 扩展 trait
// ══════════════════════════════════════════════════════════════

pub trait ResultExt<T, E> {
    fn ok(self) -> Option<T>;
    fn unwrap_or(self, default: T) -> T;
    fn unwrap_or_else<F: FnOnce(E) -> T>(self, f: F) -> T;
    fn expect(self, msg: &str) -> T;
    fn unwrap(self) -> T where E: Debug;
    fn map_err<F, F2>(self, op: F) -> Result<T, F2> where F: FnOnce(E) -> F2;
    fn is_ok(&self) -> bool;
    fn is_err(&self) -> bool;
}

impl<T, E: Debug> ResultExt<T, E> for Result<T, E> {
    fn ok(self) -> Option<T> { self.ok() }
    fn unwrap_or(self, default: T) -> T { self.unwrap_or(default) }
    fn unwrap_or_else<F: FnOnce(E) -> T>(self, f: F) -> T { self.unwrap_or_else(f) }
    fn expect(self, msg: &str) -> T { self.expect(msg) }
    fn unwrap(self) -> T where E: Debug { self.unwrap() }
    fn map_err<F, F2>(self, op: F) -> Result<T, F2> where F: FnOnce(E) -> F2 {
        self.map_err(op)
    }
    fn is_ok(&self) -> bool { self.is_ok() }
    fn is_err(&self) -> bool { self.is_err() }
}

// ══════════════════════════════════════════════════════════════
// Option 扩展
// ══════════════════════════════════════════════════════════════

pub trait OptionExt<T> {
    fn unwrap_or(self, default: T) -> T;
    fn unwrap_or_else<F: FnOnce() -> T>(self, f: F) -> T;
    fn expect(self, msg: &str) -> T;
    fn unwrap(self) -> T;
    fn is_some(&self) -> bool;
    fn is_none(&self) -> bool;
    fn ok_or<E: Debug>(self, err: E) -> Result<T, E>;
    fn and_then<U, F: FnOnce(T) -> Option<U>>(self, f: F) -> Option<U>;
    fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Option<U>;
}

impl<T> OptionExt<T> for Option<T> {
    fn unwrap_or(self, default: T) -> T { self.unwrap_or(default) }
    fn unwrap_or_else<F: FnOnce() -> T>(self, f: F) -> T { self.unwrap_or_else(f) }
    fn expect(self, msg: &str) -> T { self.expect(msg) }
    fn unwrap(self) -> T { self.unwrap() }
    fn is_some(&self) -> bool { self.is_some() }
    fn is_none(&self) -> bool { self.is_none() }
    fn ok_or<E: Debug>(self, err: E) -> Result<T, E> { self.ok_or(err) }
    fn and_then<U, F: FnOnce(T) -> Option<U>>(self, f: F) -> Option<U> { self.and_then(f) }
    fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Option<U> { self.map(f) }
}

// ══════════════════════════════════════════════════════════════
// 断言宏（编译期/运行期）
// ══════════════════════════════════════════════════════════════

pub fn assert_true(condition: bool, message: &str) -> Result<(), LzError> {
    if condition { Ok(()) } else { Err(LzError::kind(ErrorKind::AssertionFailed, message)) }
}

pub fn assert_eq_vals<T: PartialEq + Debug>(a: T, b: T, msg: &str) -> Result<(), LzError> {
    if a == b { Ok(()) } else {
        Err(LzError::kind(ErrorKind::AssertionFailed, format!("{}: {:?} != {:?}", msg, a, b)))
    }
}

pub fn panic_lz(msg: &str) -> ! {
    panic!("{}", msg);
}

pub fn require<T>(condition: bool, msg: &str) -> Result<T, LzError> {
    if condition {
        Err(LzError::kind(ErrorKind::AssertionFailed, msg))
    } else {
        Ok(unsafe { std::mem::zeroed() })
    }
}

// ══════════════════════════════════════════════════════════════
// 单元测试
// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_new() {
        let e = LzError::new("test error");
        assert_eq!(e.kind, ErrorKind::Generic);
        assert_eq!(e.message, "test error");
    }

    #[test]
    fn test_error_clone() {
        let e = LzError::new("clone me").context("key", "val");
        let e2 = e.clone();
        assert_eq!(e2.kind, ErrorKind::Generic);
        assert_eq!(e2.message, "clone me");
        assert_eq!(e2.context.len(), 1);
    }

    #[test]
    fn test_error_display() {
        let e = LzError::kind(ErrorKind::NotFound, "file not found");
        let s = format!("{}", e);
        assert!(s.contains("NotFoundError"));
        assert!(s.contains("file not found"));
    }

    #[test]
    fn test_kind_name() {
        assert_eq!(ErrorKind::Generic.name(), "Error");
        assert_eq!(ErrorKind::TypeMismatch.name(), "TypeError");
    }

    #[test]
    fn test_convenience_constructors() {
        let e1 = io_error("disk full");
        assert_eq!(e1.kind, ErrorKind::IO);
        let e2 = type_error("expected int");
        assert_eq!(e2.kind, ErrorKind::TypeMismatch);
    }

    #[test]
    fn test_result_ext() {
        let ok: Result<i32, LzError> = Ok(42);
        let err: Result<i32, LzError> = Err(LzError::new("fail"));
        assert!(ok.is_ok());
        assert!(err.is_err());
        assert_eq!(ok.unwrap_or(0), 42);
        assert_eq!(err.unwrap_or(0), 0);
    }

    #[test]
    fn test_option_ext() {
        let some: Option<i32> = Some(42);
        let none: Option<i32> = None;
        assert!(some.is_some());
        assert!(none.is_none());
        assert_eq!(some.unwrap_or(0), 42);
        assert_eq!(none.unwrap_or(99), 99);
    }

    #[test]
    fn test_assert_eq_vals() {
        let ok = assert_eq_vals(1 + 1, 2, "math broken");
        assert!(ok.is_ok());
    }
}

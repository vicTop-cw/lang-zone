// Lang-Zong 编译器 — util/error.rs
// 统一编译器错误类型：词法/语法/语义/IO 四大类别
//
// 对标 Rust `std::error::Error` trait + Python SyntaxError/ImportError 分层设计

use std::fmt;
use std::io;
use crate::lexer::Span;

/// 编译器错误类别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// 词法错误：非法字符、未闭合字符串等
    Lex,
    /// 语法错误：意外 token、缺少冒号/缩进等
    Parse,
    /// 语义错误：类型不匹配、未定义变量等
    Semantic,
    /// 类型错误
    Type,
    /// 导入/模块错误
    Import,
    /// IO 错误：文件不存在、权限不足等
    Io,
    /// 内部错误：不应出现的编译器 bug
    Internal,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Lex => write!(f, "Lex error"),
            ErrorKind::Parse => write!(f, "Parse error"),
            ErrorKind::Semantic => write!(f, "Semantic error"),
            ErrorKind::Type => write!(f, "Type error"),
            ErrorKind::Import => write!(f, "Import error"),
            ErrorKind::Io => write!(f, "IO error"),
            ErrorKind::Internal => write!(f, "Internal compiler error"),
        }
    }
}

/// 编译器统一错误
#[derive(Debug)]
pub struct CompilerError {
    pub kind: ErrorKind,
    pub message: String,
    /// 源码位置（可选）
    pub span: Option<Span>,
    /// 文件路径（可选）
    pub file: Option<String>,
    /// 源代码行（用于错误展示）
    pub source_line: Option<String>,
    /// 底层 IO 错误
    pub io_error: Option<io::Error>,
}

impl CompilerError {
    pub fn lex(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Lex,
            message: message.into(),
            span: None,
            file: None,
            source_line: None,
            io_error: None,
        }
    }

    pub fn parse(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Parse,
            message: message.into(),
            span: None,
            file: None,
            source_line: None,
            io_error: None,
        }
    }

    pub fn semantic(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Semantic,
            message: message.into(),
            span: None,
            file: None,
            source_line: None,
            io_error: None,
        }
    }

    pub fn io(message: impl Into<String>, err: io::Error) -> Self {
        Self {
            kind: ErrorKind::Io,
            message: message.into(),
            span: None,
            file: None,
            source_line: None,
            io_error: Some(err),
        }
    }

    pub fn import(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Import,
            message: message.into(),
            span: None,
            file: None,
            source_line: None,
            io_error: None,
        }
    }

    /// 附加源码位置
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// 附加文件路径
    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// 附加源代码行
    pub fn with_source(mut self, line: impl Into<String>) -> Self {
        self.source_line = Some(line.into());
        self
    }
}

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 文件:行:列: kind: message
        if let Some(ref file) = self.file {
            write!(f, "{}", file)?;
            if let Some(ref span) = self.span {
                write!(f, ":{}", span)?;
            }
            write!(f, ": ")?;
        }
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for CompilerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.io_error.as_ref().map(|e| e as &dyn std::error::Error)
    }
}

impl From<io::Error> for CompilerError {
    fn from(err: io::Error) -> Self {
        CompilerError::io(err.to_string(), err)
    }
}

/// 编译器执行结果类型别名
pub type Result<T> = std::result::Result<T, CompilerError>;

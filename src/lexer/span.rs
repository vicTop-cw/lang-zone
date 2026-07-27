// Lang-Zong 编译器 — lexer/span.rs
// 源码位置信息：文件、行、列，用于错误报告和诊断
//
// 设计对标 Python 3.13 tokenize.py — TokenInfo (type, string, start, end, line)

/// 源码中的行列位置
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourcePos {
    /// 行号（从 1 开始）
    pub line: usize,
    /// 列号（从 1 开始，按 UTF-8 字节计数）
    pub col: usize,
}

impl SourcePos {
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

impl std::fmt::Display for SourcePos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// 源码片段的位置范围（起始 → 结束）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// 起始位置
    pub start: SourcePos,
    /// 结束位置
    pub end: SourcePos,
}

impl Span {
    pub const fn new(start: SourcePos, end: SourcePos) -> Self {
        Self { start, end }
    }

    /// 单字符跨度（起点和终点相同）
    pub fn point(pos: SourcePos) -> Self {
        Self { start: pos, end: pos }
    }

    /// 合并两个跨度
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
        }
    }
}

impl Default for Span {
    fn default() -> Self {
        Span {
            start: SourcePos::default(),
            end: SourcePos::default(),
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.start.line == self.end.line && self.start.col == self.end.col {
            write!(f, "{}", self.start)
        } else {
            write!(f, "{}-{}", self.start, self.end)
        }
    }
}

/// 带源码位置信息的泛型容器
/// 对标 Rust `Spanned<T>` 模式 — 将位置信息附加到任意 AST 节点上
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

// Lang-Zong 编译器 — sourcemap 模块
// ────────────────────────────────────────────────────────
// LZ→Rust 源码位置映射 + 双层错误追踪
//
// 动机：lz 编译器生成 Rust 代码后 `rustc` 二次编译，错误可能来自两层。
//  sourcemap 将 rustc 报错的行号/列号反向映射回 lz 源位置，
//  使开发者看到的始终是 lz 层面的错误信息。
//
// 设计对标：TypeScript `sourceMap` / Elm `--debug` 行号注释 /
//          Zig `@src()` builtin（编译期 source location）

use crate::lexer::{Span};
use std::fmt;

// ═══════════════════════════════════════════════════════
// LineRange
// ═══════════════════════════════════════════════════════

/// 生成代码中的行范围（start_line..end_line, 1-based）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    /// 起始行（1-based, 闭区间）
    pub start: usize,
    /// 结束行（1-based, 开区间，即不包含此行）
    pub end: usize,
}

impl LineRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn point(line: usize) -> Self {
        Self { start: line, end: line + 1 }
    }

    /// 此范围是否包含指定行
    pub fn contains(&self, line: usize) -> bool {
        line >= self.start && line < self.end
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

impl fmt::Display for LineRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.len() <= 1 {
            write!(f, "line {}", self.start)
        } else {
            write!(f, "lines {}-{}", self.start, self.end - 1)
        }
    }
}

// ═══════════════════════════════════════════════════════
// MappingEntry
// ═══════════════════════════════════════════════════════

/// 一条映射记录：lz 源位置 → 生成的 Rust 行范围
#[derive(Debug, Clone)]
pub struct MappingEntry {
    /// lz 源码位置
    pub lz_span: Span,
    /// 当前映射对应的 lz 顶层声明名（可选，用于更友好的错误信息 eg. "在函数 foo 中"）
    pub context: Option<String>,
    /// 生成的 Rust 代码行范围
    pub rust_range: LineRange,
}

// ═══════════════════════════════════════════════════════
// SourceMap
// ═══════════════════════════════════════════════════════

/// 完整的源码映射表。
/// 按 rust_range.start 升序排列，支持二分查找。
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    entries: Vec<MappingEntry>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// 添加一条映射（调用方保证按 rust 行号升序添加）
    pub fn push(&mut self, entry: MappingEntry) {
        self.entries.push(entry);
    }

    /// 记录一条 simple 映射
    pub fn record(&mut self, lz_span: Span, rust_range: LineRange, context: Option<String>) {
        self.push(MappingEntry { lz_span, context, rust_range });
    }

    /// 给定 Rust 行号，反查 lz 源位置。
    /// 返回 (lz 源位置, 上下文描述)
    pub fn rust_line_to_lz(&self, rust_line: usize) -> Option<(&Span, Option<&str>)> {
        // 二分查找第一个 start > rust_line 的条目，取前一个
        let idx = self.entries.partition_point(|e| e.rust_range.start <= rust_line);
        if idx == 0 {
            return None;
        }
        let entry = &self.entries[idx - 1];
        if entry.rust_range.contains(rust_line) {
            Some((&entry.lz_span, entry.context.as_deref()))
        } else {
            None
        }
    }

    /// 遍历所有映射
    pub fn iter(&self) -> impl Iterator<Item = &MappingEntry> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

// ═══════════════════════════════════════════════════════
// SourceBuilder — 带位置追踪的代码拼接器
// ═══════════════════════════════════════════════════════

/// 带源码位置追踪的字符串构建器。
/// 替代 codegen 中直接使用 `String`，在拼接时自动记录
/// lz 源位置 → 生成 Rust 行号的映射。
pub struct SourceBuilder {
    buf: String,
    /// 当前 Rust 行号（1-based）
    current_line: usize,
    /// 当前映射：lz span
    current_lz: Option<Span>,
    /// 当前映射：起始 Rust 行号
    current_rust_start: usize,
    /// 当前上下文
    current_ctx: Option<String>,
    /// 已排出的映射
    map: SourceMap,
}

impl SourceBuilder {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            current_line: 1,
            current_lz: None,
            current_rust_start: 1,
            current_ctx: None,
            map: SourceMap::new(),
        }
    }

    // ── 位置追踪 ──

    /// 标记接下来的输出来自指定 lz 源位置。
    /// 如果之前有未关闭的映射，自动关闭。
    pub fn mark_source(&mut self, span: Span, context: Option<String>) {
        self.flush_current();
        self.current_lz = Some(span);
        self.current_rust_start = self.current_line;
        self.current_ctx = context;
    }

    /// 关闭当前映射段（如果有）
    fn flush_current(&mut self) {
        if let Some(lz_span) = self.current_lz.take() {
            let range = LineRange::new(self.current_rust_start, self.current_line);
            self.map.record(lz_span, range, self.current_ctx.take());
        }
    }

    // ── 写入方法 ──

    /// 追加字符串，自动跟踪换行
    pub fn push_str(&mut self, s: &str) {
        self.buf.push_str(s);
        self.current_line += s.bytes().filter(|&b| b == b'\n').count();
    }

    /// 追加字符
    pub fn push(&mut self, ch: char) {
        self.buf.push(ch);
        if ch == '\n' {
            self.current_line += 1;
        }
    }

    /// 追加格式化的内容
    pub fn push_fmt(&mut self, args: fmt::Arguments<'_>) {
        // 用 std::fmt::write 写入内部 buf，并统计换行
        use std::fmt::Write;
        let before = self.buf.len();
        self.buf.write_fmt(args).expect("write to String never fails");
        let after = self.buf.len();
        let added = &self.buf[before..after];
        self.current_line += added.bytes().filter(|&b| b == b'\n').count();
    }

    /// 返回当前 Rust 行号
    pub fn line(&self) -> usize {
        self.current_line
    }

    /// 返回已构建的字符串
    pub fn as_str(&self) -> &str {
        &self.buf
    }

    // ── 完成 ──

    /// 完成构建，返回 (生成的代码, 源映射)
    pub fn finish(mut self) -> (String, SourceMap) {
        self.flush_current();
        (self.buf, self.map)
    }

    /// 仅获取生成的代码
    pub fn into_string(mut self) -> String {
        self.flush_current();
        self.buf
    }
}

impl Default for SourceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Write for SourceBuilder {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }
}

impl fmt::Display for SourceBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.buf)
    }
}

// ═══════════════════════════════════════════════════════
// DoubleError — 双层错误类型
// ═══════════════════════════════════════════════════════

/// 双层编译器错误：lz 编译错误 + 可选 rustc 编译错误
#[derive(Debug)]
pub struct DoubleError {
    /// lz 侧错误（解析/类型/语义）
    pub lz_error: Option<crate::util::error::CompilerError>,
    /// rustc 原始 stderr
    pub rustc_stderr: Option<String>,
    /// 源映射（用于翻译 rustc 行号）
    pub source_map: Option<SourceMap>,
}

impl DoubleError {
    /// lz 层面的错误
    pub fn from_lz(err: crate::util::error::CompilerError) -> Self {
        Self { lz_error: Some(err), rustc_stderr: None, source_map: None }
    }

    /// rustc 编译失败（需要 sourcemap 翻译行号）
    pub fn from_rustc(stderr: String, map: SourceMap) -> Self {
        Self { lz_error: None, rustc_stderr: Some(stderr), source_map: Some(map) }
    }

    /// 使用源映射翻译 rustc 错误行号
    /// 将 rustc stderr 中的 "--> path.rs:42:15" 行号替换为 lz 源位置
    pub fn translate_rustc_errors(&self) -> String {
        let stderr = match &self.rustc_stderr {
            Some(s) => s,
            None => return String::from("(no rustc errors)"),
        };
        let map = match &self.source_map {
            Some(m) => m,
            None => return stderr.clone(),
        };

        let mut out = String::new();
        for line in stderr.lines() {
            // 尝试匹配 rustc 的行号格式: " --> file.rs:42:15"
            if let Some(translated) = Self::try_translate_line(line, map) {
                out.push_str(&translated);
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        out
    }

    /// 尝试翻译单行 rustc 错误
    fn try_translate_line(line: &str, map: &SourceMap) -> Option<String> {
        // 匹配模式: "  --> some/path.rs:42:15"
        let arrow_pos = line.find("-->")?;
        let after_arrow = &line[arrow_pos + 3..].trim();
        // 格式: "path:line:col" 或 "line:col"
        let colon1 = after_arrow.rfind(':')?; // last colon is before column
        let before_col = &after_arrow[..colon1];
        let colon2 = before_col.rfind(':')?; // second-to-last colon is before line
        let line_str = &before_col[colon2 + 1..];
        let rust_line: usize = line_str.parse().ok()?;

        let (lz_span, ctx) = map.rust_line_to_lz(rust_line)?;
        let prefix = &line[..arrow_pos + 3];

        let mut translated = format!(
            "{} [lz] line {}:{}",
            prefix, lz_span.start.line, lz_span.start.col
        );
        if let Some(c) = ctx {
            translated.push_str(&format!(" (in {})", c));
        }
        translated.push_str(&format!("  (was rustc line {})", rust_line));
        Some(translated)
    }
}

impl fmt::Display for DoubleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref lz) = self.lz_error {
            write!(f, "{}", lz)?;
        }
        if self.lz_error.is_some() && self.rustc_stderr.is_some() {
            writeln!(f)?;
        }
        if self.rustc_stderr.is_some() {
            write!(f, "{}", self.translate_rustc_errors())?;
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::SourcePos;

    #[test]
    fn test_builder_basic_line_tracking() {
        let mut sb = SourceBuilder::new();
        assert_eq!(sb.line(), 1);
        sb.push_str("hello\n");
        assert_eq!(sb.line(), 2);
        sb.push_str("world\n\n");
        assert_eq!(sb.line(), 4);
        let (code, _map) = sb.finish();
        assert_eq!(code, "hello\nworld\n\n");
    }

    #[test]
    fn test_mapping_single_span() {
        let mut sb = SourceBuilder::new();
        sb.mark_source(
            Span::new(SourcePos::new(1, 1), SourcePos::new(1, 10)),
            Some("test_fn".into()),
        );
        sb.push_str("fn foo() {\n");
        sb.push_str("    let x = 1;\n");
        sb.push_str("}\n");
        let (code, map) = sb.finish();

        assert_eq!(code, "fn foo() {\n    let x = 1;\n}\n");
        assert_eq!(map.len(), 1);

        // 查 Rust line 1 → lz line 1
        let (span, ctx) = map.rust_line_to_lz(1).unwrap();
        assert_eq!(span.start.line, 1);
        assert_eq!(ctx, Some("test_fn"));

        // 查 Rust line 3 → lz line 1
        let (span, _) = map.rust_line_to_lz(3).unwrap();
        assert_eq!(span.start.line, 1);

        // 查 Rust line 4 → none (out of range)
        assert!(map.rust_line_to_lz(4).is_none());
    }

    #[test]
    fn test_mapping_multiple_spans() {
        let mut sb = SourceBuilder::new();

        sb.mark_source(Span::point(SourcePos::new(1, 1)), Some("a".into()));
        sb.push_str("// from line 1\n");
        sb.mark_source(Span::point(SourcePos::new(3, 1)), Some("b".into()));
        sb.push_str("// from line 3\n");
        sb.mark_source(Span::point(SourcePos::new(5, 1)), Some("c".into()));
        sb.push_str("// from line 5\n");

        let (_code, map) = sb.finish();

        assert_eq!(map.len(), 3);
        let (_, ctx) = map.rust_line_to_lz(1).unwrap();
        assert_eq!(ctx, Some("a"));
        let (_, ctx) = map.rust_line_to_lz(2).unwrap();
        assert_eq!(ctx, Some("b"));
        let (_, ctx) = map.rust_line_to_lz(3).unwrap();
        assert_eq!(ctx, Some("c"));
    }

    #[test]
    fn test_rustc_error_translation() {
        let mut map = SourceMap::new();
        map.record(
            Span::new(SourcePos::new(5, 1), SourcePos::new(5, 20)),
            LineRange::new(12, 15),
            Some("consume".into()),
        );

        let de = DoubleError {
            lz_error: None,
            rustc_stderr: Some(String::from(
                "error[E0382]: use of moved value\n  --> test.rs:13:5\n   |\n12 |     let x = foo();\n   |         - move occurs\n13 |     foo(x);\n   |         ^ value used here after move\n",
            )),
            source_map: Some(map),
        };

        let translated = de.translate_rustc_errors();
        // 13 should map back to lz line 5
        assert!(translated.contains("[lz] line 5:1"));
        assert!(translated.contains("(in consume)"));
    }
}

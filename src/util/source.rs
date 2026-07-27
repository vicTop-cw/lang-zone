// Lang-Zong 编译器 — util/source.rs
// 源码文件读写：读取文件 + BOM 处理 + 行缓存 + 错误片段提取
//
// 对标 Python tokenize.py 的 open/readline 层

use std::fs;
use std::io;
use std::path::Path;

/// 源码缓存：按行存储，支持错误诊断时的上下文提取
#[derive(Debug, Clone)]
pub struct SourceCache {
    /// 文件路径
    pub path: String,
    /// 原始内容
    pub content: String,
    /// 按行切分的缓存（从第 1 行起，索引 = 行号-1）
    lines: Vec<String>,
}

impl SourceCache {
    /// 从文件路径读取源码（自动处理 UTF-8 BOM + 行尾归一化）
    pub fn read(path: &Path) -> io::Result<Self> {
        let raw = fs::read(path)?;
        let stripped = crate::util::platform::strip_bom(&raw);
        let content = String::from_utf8_lossy(stripped).into_owned();
        let normalized = crate::util::platform::normalize_line_endings(&content);
        let lines: Vec<String> = normalized.lines().map(|s| s.to_string()).collect();

        Ok(Self {
            path: path.to_string_lossy().into_owned(),
            content: normalized,
            lines,
        })
    }

    /// 从字符串创建（测试/内联用）
    pub fn from_string(name: &str, source: &str) -> Self {
        let normalized = crate::util::platform::normalize_line_endings(source);
        let lines: Vec<String> = normalized.lines().map(|s| s.to_string()).collect();
        Self {
            path: name.to_string(),
            content: normalized,
            lines,
        }
    }

    /// 获取指定行（行号从 1 开始）
    pub fn line(&self, line_no: usize) -> Option<&str> {
        if line_no == 0 { return None; }
        self.lines.get(line_no - 1).map(|s| s.as_str())
    }

    /// 总行数
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// 获取指定行范围的源码片段（用于错误报告，行号从 1 开始）
    ///
    /// 返回带行号标注的多行字符串：
    /// ```text
    ///  42 |     let x = foo
    ///     |              ^^^
    ///  43 |     bar()
    /// ```
    pub fn snippet(&self, start_line: usize, end_line: usize, highlight_start_col: usize, highlight_end_col: usize) -> String {
        let mut out = String::new();
        let start = start_line.max(1);
        let end = end_line.min(self.lines.len());

        for line_no in start..=end {
            if let Some(line) = self.line(line_no) {
                let prefix = format!("{:>4} | ", line_no);
                out.push_str(&prefix);
                out.push_str(line);
                out.push('\n');

                // 高亮行
                if line_no >= start_line && line_no <= end_line {
                    let col_start = if line_no == start_line { highlight_start_col.max(1) } else { 1 };
                    let col_end = if line_no == end_line { highlight_end_col } else { line.len() + 1 };
                    let spaces = " ".repeat(prefix.len());
                    out.push_str(&spaces);
                    let carets_len = (col_end.saturating_sub(col_start)).max(1);
                    out.push_str(&" ".repeat(col_start - 1));
                    out.push_str(&"^".repeat(carets_len));
                    out.push('\n');
                }
            }
        }

        out
    }
}

/// 将生成的 Rust 代码写入文件
pub fn write_output(path: &Path, code: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, code)
}

/// 生成输出文件路径（输入 .lz → 输出 .rs）
pub fn output_path(input: &Path) -> std::path::PathBuf {
    let stem = input.file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| "output".into());
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{}.rs", stem))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_string() {
        let src = SourceCache::from_string("test.lz", "line1\nline2\nline3");
        assert_eq!(src.line(1), Some("line1"));
        assert_eq!(src.line(3), Some("line3"));
        assert_eq!(src.line(4), None);
        assert_eq!(src.line_count(), 3);
    }

    #[test]
    fn test_crlf_normalized() {
        let src = SourceCache::from_string("test.lz", "a\r\nb");
        assert_eq!(src.line(1), Some("a"));
        assert_eq!(src.line(2), Some("b"));
    }

    #[test]
    fn test_snippet() {
        let src = SourceCache::from_string("test.lz", "def foo() =\n    let x = 42\n    x");
        let snippet = src.snippet(1, 2, 9, 10);
        assert!(snippet.contains("def foo()"));
        assert!(snippet.contains("let x"));
    }

    #[test]
    fn test_output_path() {
        let path = Path::new("foo/bar/test.lz");
        let out = output_path(path);
        assert_eq!(out.to_str().unwrap().replace('\\', "/"), "foo/bar/test.rs");
    }
}

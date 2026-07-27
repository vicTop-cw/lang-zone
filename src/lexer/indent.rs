// Lang-Zong 编译器 — lexer/indent.rs
// 缩进状态机：Indent/Dedent 虚拟 Token 生成逻辑
//
// 从 lexer 中抽出，保持词法分析器的关注点分离
// 对标 Python tokenize.py 的 INDENT/DEDENT 注入逻辑

use super::token::Token;

/// 缩进栈：跟踪嵌套深度的栈式结构
///
/// 栈顶始终是当前缩进级别（列号），初始为 0（顶级）
#[derive(Debug, Clone)]
pub struct IndentStack {
    stack: Vec<usize>,
}

impl IndentStack {
    /// 创建新的缩进栈（顶级 = 列 0）
    pub fn new() -> Self {
        Self { stack: vec![0] }
    }

    /// 当前缩进级别（列号）
    pub fn current(&self) -> usize {
        *self.stack.last().unwrap_or(&0)
    }

    /// 栈深度（嵌套层数，不含顶级）
    pub fn depth(&self) -> usize {
        self.stack.len().saturating_sub(1)
    }

    /// 处理新行的缩进级别，返回要注入的虚拟 Token
    ///
    /// - col > current → Indent（进入更深嵌套）
    /// - col < current → 一个或多个 Dedent（退出嵌套）
    /// - col == current → 无操作
    /// - col 不在栈中任何一个级别 → 返回 None（缩进错误）
    pub fn handle(&mut self, col: usize) -> Option<Vec<Token>> {
        let current = self.current();
        let mut tokens = Vec::new();

        if col > current {
            // 加深缩进
            self.stack.push(col);
            tokens.push(Token::Indent);
        } else if col < current {
            // 减少缩进：持续弹出直到找到匹配级别
            while self.stack.len() > 1 && col < self.current() {
                self.stack.pop();
                tokens.push(Token::Dedent);
            }

            // 检查缩进级别是否有效
            if col != self.current() && !self.stack.is_empty() {
                // 缩进到了不存在的级别
                return None;
            }
        }
        // col == current → 无操作

        Some(tokens)
    }

    /// 在文件末尾弹出所有剩余缩进（生成 EOF 前的 Dedent）
    pub fn drain(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while self.stack.len() > 1 {
            self.stack.pop();
            tokens.push(Token::Dedent);
        }
        tokens
    }
}

impl Default for IndentStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_indent(tokens: &[Token], expected_count: usize) {
        let indent_count = tokens.iter().filter(|t| matches!(t, Token::Indent)).count();
        assert_eq!(indent_count, expected_count, "Expected {} Indent(s)", expected_count);
    }

    fn assert_dedent(tokens: &[Token], expected_count: usize) {
        let dedent_count = tokens.iter().filter(|t| matches!(t, Token::Dedent)).count();
        assert_eq!(dedent_count, expected_count, "Expected {} Dedent(s)", expected_count);
    }

    #[test]
    fn test_initial_state() {
        let s = IndentStack::new();
        assert_eq!(s.current(), 0);
        assert_eq!(s.depth(), 0);
    }

    #[test]
    fn test_single_indent() {
        let mut s = IndentStack::new();
        let tokens = s.handle(4).unwrap();
        assert_indent(&tokens, 1);
        assert_eq!(s.current(), 4);
        assert_eq!(s.depth(), 1);
    }

    #[test]
    fn test_single_dedent() {
        let mut s = IndentStack::new();
        s.handle(4).unwrap();
        let tokens = s.handle(0).unwrap();
        assert_dedent(&tokens, 1);
        assert_eq!(s.current(), 0);
    }

    #[test]
    fn test_multi_level() {
        let mut s = IndentStack::new();
        // 0 → 4
        let t1 = s.handle(4).unwrap();
        assert_indent(&t1, 1);
        // 4 → 8
        let t2 = s.handle(8).unwrap();
        assert_indent(&t2, 1);
        // 8 → 4
        let t3 = s.handle(4).unwrap();
        assert_dedent(&t3, 1);
        // 4 → 0
        let t4 = s.handle(0).unwrap();
        assert_dedent(&t4, 1);
        assert_eq!(s.current(), 0);
    }

    #[test]
    fn test_invalid_dedent() {
        let mut s = IndentStack::new();
        s.handle(4).unwrap();
        // 尝试缩进到不存在的级别 2（当前在 4，栈中有 [0, 4]）
        let result = s.handle(2);
        assert!(result.is_none()); // 缩进错误
    }

    #[test]
    fn test_drain_at_eof() {
        let mut s = IndentStack::new();
        s.handle(4).unwrap();
        s.handle(8).unwrap();
        let tokens = s.drain();
        assert_dedent(&tokens, 2);
        assert_eq!(s.depth(), 0);
    }
}

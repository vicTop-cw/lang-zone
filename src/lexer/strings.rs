// Lang-Zone 编译器 — lexer/strings.rs
// （由 lexer/lexer.rs move-only 拆出，逻辑零改动）

use super::*;

impl Lexer {
    /// 解析反斜杠转义序列（调用时当前字符为 `\`，尚未 advance）。
    /// 返回要写入字符串的内容；Err 为非法转义错误（00-词法基础 §2.1）。
    /// allow_braces: f-string 中允许 `\{` `\}`（字面花括号 → `{{`/`}}`，与 format! 一致）。
    pub(crate) fn lex_escape(&mut self, allow_braces: bool) -> Result<String, String> {
        self.advance(); // skip '\'
        match self.peek() {
            None => Err("字符串以反斜杠结尾".into()),
            Some('n') => {
                self.advance();
                Ok("\n".into())
            }
            Some('t') => {
                self.advance();
                Ok("\t".into())
            }
            Some('r') => {
                self.advance();
                Ok("\r".into())
            }
            Some('\\') => {
                self.advance();
                Ok("\\".into())
            }
            Some('"') => {
                self.advance();
                Ok("\"".into())
            }
            Some('\'') => {
                self.advance();
                Ok("'".into())
            }
            Some('0') => {
                self.advance();
                Ok("\0".into())
            }
            Some('{') if allow_braces => {
                self.advance();
                Ok("{{".into())
            }
            Some('}') if allow_braces => {
                self.advance();
                Ok("}}".into())
            }
            Some('u') => {
                self.advance(); // skip 'u'
                if self.peek() != Some('{') {
                    return Err("非法转义序列: \\u 后应跟 {".into());
                }
                self.advance(); // skip '{'
                let mut hex = String::new();
                while let Some(h) = self.peek() {
                    if h == '}' {
                        break;
                    }
                    if !h.is_ascii_hexdigit() || hex.len() >= 6 {
                        return Err(format!("非法 Unicode 转义: \\u{{{}}}", hex));
                    }
                    hex.push(h);
                    self.advance();
                }
                if self.peek() != Some('}') || hex.is_empty() {
                    return Err(format!("非法 Unicode 转义: \\u{{{}}}", hex));
                }
                self.advance(); // skip '}'
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => Ok(ch.to_string()),
                    None => Err(format!("非法 Unicode 码点: \\u{{{}}}", hex)),
                }
            }
            Some(other) => Err(format!("非法转义序列: \\{}", other)),
        }
    }

    pub(crate) fn read_string(&mut self) -> Token {
        self.advance(); // skip opening "
        let mut s = String::new();
        let mut closed = false;
        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance();
                closed = true;
                break;
            } else if c == '\\' {
                match self.lex_escape(false) {
                    Ok(part) => s.push_str(&part),
                    Err(e) => return Token::LexError(e),
                }
            } else {
                s.push(self.advance().unwrap());
            }
        }
        if closed {
            Token::StrLit(s)
        } else {
            Token::LexError(format!("未终止的字符串字面量: \"{}\"", s))
        }
    }

    pub(crate) fn read_triple_string(&mut self) -> Token {
        // 已经在 '"""' 的第一个 " 处
        self.advance();
        self.advance();
        self.advance(); // skip """
        let mut s = String::new();
        loop {
            match self.peek() {
                None => break,
                Some('"') if self.peek_n(1) == Some('"') && self.peek_n(2) == Some('"') => {
                    self.advance();
                    self.advance();
                    self.advance();
                    break;
                }
                Some(_c) => s.push(self.advance().unwrap()),
            }
        }
        // 去除公共缩进
        let lines: Vec<&str> = s.lines().collect();
        if lines.len() > 1 {
            let min_indent = lines[1..]
                .iter()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.len() - l.trim_start().len())
                .min()
                .unwrap_or(0);
            let trimmed: Vec<String> = lines
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    if i == 0 || l.trim().is_empty() {
                        l.to_string()
                    } else {
                        if l.len() >= min_indent {
                            l[min_indent..].to_string()
                        } else {
                            l.to_string()
                        }
                    }
                })
                .collect();
            Token::StrLit(trimmed.join("\n"))
        } else {
            Token::StrLit(s)
        }
    }

    pub(crate) fn read_fstring(&mut self) -> Token {
        self.advance(); // skip f
                        // Check for triple-quoted f-string
        if self.peek() == Some('"') && self.peek_n(1) == Some('"') && self.peek_n(2) == Some('"') {
            self.advance();
            self.advance();
            self.advance();
            let mut s = String::new();
            loop {
                match self.peek() {
                    None => break,
                    Some('"') if self.peek_n(1) == Some('"') && self.peek_n(2) == Some('"') => {
                        self.advance();
                        self.advance();
                        self.advance();
                        break;
                    }
                    Some(_c) => s.push(self.advance().unwrap()),
                }
            }
            return Token::FStrLit(s);
        }
        self.advance(); // skip opening "
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance();
                break;
            } else if c == '\\' {
                match self.lex_escape(true) {
                    Ok(part) => s.push_str(&part),
                    Err(e) => return Token::LexError(e),
                }
            } else {
                s.push(self.advance().unwrap());
            }
        }
        Token::FStrLit(s)
    }

    pub(crate) fn read_raw_string(&mut self) -> Token {
        self.advance(); // skip r
                        // r"""...""" 三引号原始字符串（00-词法基础 §2.1）：不处理转义，读到 """ 结束
        if self.peek() == Some('"') && self.peek_n(1) == Some('"') && self.peek_n(2) == Some('"') {
            self.advance();
            self.advance();
            self.advance();
            let mut s = String::new();
            loop {
                match self.peek() {
                    None => break,
                    Some('"') if self.peek_n(1) == Some('"') && self.peek_n(2) == Some('"') => {
                        self.advance();
                        self.advance();
                        self.advance();
                        break;
                    }
                    Some(_c) => s.push(self.advance().unwrap()),
                }
            }
            return Token::RawStrLit(s);
        }
        self.advance(); // skip opening "
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance();
                break;
            }
            s.push(self.advance().unwrap());
        }
        Token::RawStrLit(s)
    }
}

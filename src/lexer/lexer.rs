// Lang-Zong 编译器 — lexer/lexer.rs
// 词法分析器: 源码 → Token 流
use super::token::Token;
use crate::util::chars::is_build_ws;
use super::indent::IndentStack;
pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    indent: IndentStack,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            indent: IndentStack::new(),
            line: 1,
            col: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_n(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }

    /// 返回当前位置的前一个字符（pos==0 时返回 None，视作输入边界）
    fn prev_char(&self) -> Option<char> {
        if self.pos == 0 {
            None
        } else {
            self.chars.get(self.pos - 1).copied()
        }
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 0;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn skip_inline_whitespace(&mut self) -> usize {
        let mut spaces = 0;
        while let Some(c) = self.peek() {
            if c == ' ' {
                spaces += 1;
                self.advance();
            } else if c == '\t' {
                spaces += 4;
                self.advance();
            } else {
                break;
            }
        }
        spaces
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' { break; }
            self.advance();
        }
    }

    fn skip_block_comment(&mut self) {
        // /* ... */ 多行注释（Java/Rust 体系）；# 预留给宏语法，不再作注释
        loop {
            match self.peek() {
                None => break,
                Some('*') if self.peek_n(1) == Some('/') => {
                    self.advance();
                    self.advance();
                    break;
                }
                Some(_) => { self.advance(); }
            }
        }
    }

    fn read_number(&mut self, first: char) -> Token {
        let mut num = String::from(first);
        let mut is_float = false;

        // 处理进制前缀 0x 0o 0b
        if first == '0' {
            match self.peek() {
                Some('x') | Some('X') => {
                    num.push(self.advance().unwrap());
                    while let Some(c) = self.peek() {
                        if c.is_ascii_hexdigit() {
                            num.push(self.advance().unwrap());
                        } else if c == '_' {
                            self.advance();
                        } else { break; }
                    }
                    match i64::from_str_radix(&num[2..].replace('_', ""), 16) {
                        Ok(val) => return Token::IntLit(val),
                        Err(_) => {
                            let hex_str = &num[2..].replace('_', "");
                            // 如果值在 u64 范围内，作为 i64 返回（允许负数表示）
                            if let Ok(val) = u64::from_str_radix(hex_str, 16) {
                                return Token::IntLit(val as i64);
                            }
                            return Token::LexError(format!("无效的十六进制数字: {}", num));
                        }
                    }
                }
                Some('o') | Some('O') => {
                    num.push(self.advance().unwrap());
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() && c < '8' {
                            num.push(self.advance().unwrap());
                        } else if c == '_' {
                            self.advance();
                        } else { break; }
                    }
                    match i64::from_str_radix(&num[2..].replace('_', ""), 8) {
                        Ok(val) => return Token::IntLit(val),
                        Err(_) => return Token::LexError(format!("八进制值溢出 i64 范围: {}", num)),
                    }
                }
                Some('b') | Some('B') => {
                    num.push(self.advance().unwrap());
                    while let Some(c) = self.peek() {
                        if c == '0' || c == '1' {
                            num.push(self.advance().unwrap());
                        } else if c == '_' {
                            self.advance();
                        } else { break; }
                    }
                    match i64::from_str_radix(&num[2..].replace('_', ""), 2) {
                        Ok(val) => return Token::IntLit(val),
                        Err(_) => return Token::LexError(format!("二进制值溢出 i64 范围: {}", num)),
                    }
                }
                _ => {}
            }
        }

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                num.push(self.advance().unwrap());
            } else if c == '_' {
                self.advance();
            } else if c == '.' && !is_float && self.peek_n(1).map_or(false, |c| c.is_ascii_digit()) {
                is_float = true;
                num.push(self.advance().unwrap());
            } else if (c == 'e' || c == 'E') && !is_float {
                is_float = true;
                num.push(self.advance().unwrap());
                if let Some(sign) = self.peek() {
                    if sign == '+' || sign == '-' {
                        num.push(self.advance().unwrap());
                    }
                }
            } else if c == 'e' || c == 'E' {
                num.push(self.advance().unwrap());
                if let Some(sign) = self.peek() {
                    if sign == '+' || sign == '-' {
                        num.push(self.advance().unwrap());
                    }
                }
            } else {
                break;
            }
        }
        if is_float {
            match num.parse::<f64>() {
                Ok(v) => Token::FloatLit(v),
                Err(_) => {
                    // 检查是否形如 "123e"（指数无尾数）
                    if num.ends_with('e') || num.ends_with('E')
                        || num.ends_with("e+") || num.ends_with("E+")
                        || num.ends_with("e-") || num.ends_with("E-") {
                        Token::LexError(format!("科学计数法缺少指数: {}", num))
                    } else {
                        Token::LexError(format!("无效的浮点数: {}", num))
                    }
                }
            }
        } else {
            match num.parse::<i64>() {
                Ok(v) => Token::IntLit(v),
                Err(_) => {
                    // i64::MIN = -9223372036854775808，其绝对值 9223372036854775808 超出 i64 正数范围
                    // 允许该特殊值通过并以 wrapping 方式存储，由 parser/codegen 处理一元负号
                    if num == "9223372036854775808" {
                        Token::IntLit(i64::MIN)
                    } else {
                        Token::LexError(format!("无效的整数（可能溢出）: {}", num))
                    }
                }
            }
        }
    }

    fn read_string(&mut self) -> Token {
        self.advance(); // skip opening "
        let mut s = String::new();
        let mut closed = false;
        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance();
                closed = true;
                break;
            } else if c == '\\' {
                self.advance();
                if let Some(esc) = self.peek() {
                    s.push(match esc {
                        'n' => '\n', 't' => '\t', 'r' => '\r',
                        '\\' => '\\', '"' => '"', '\'' => '\'',
                        '0' => '\0',
                        _ => esc,
                    });
                    self.advance();
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

    fn read_triple_string(&mut self) -> Token {
        // 已经在 '"""' 的第一个 " 处
        self.advance(); self.advance(); self.advance(); // skip """
        let mut s = String::new();
        loop {
            match self.peek() {
                None => break,
                Some('"') if self.peek_n(1) == Some('"') && self.peek_n(2) == Some('"') => {
                    self.advance(); self.advance(); self.advance();
                    break;
                }
                Some(_c) => s.push(self.advance().unwrap()),
            }
        }
        // 去除公共缩进
        let lines: Vec<&str> = s.lines().collect();
        if lines.len() > 1 {
            let min_indent = lines[1..].iter()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.len() - l.trim_start().len())
                .min().unwrap_or(0);
            let trimmed: Vec<String> = lines.iter().enumerate()
                .map(|(i, l)| {
                    if i == 0 || l.trim().is_empty() {
                        l.to_string()
                    } else {
                        if l.len() >= min_indent { l[min_indent..].to_string() } else { l.to_string() }
                    }
                })
                .collect();
            Token::StrLit(trimmed.join("\n"))
        } else {
            Token::StrLit(s)
        }
    }

    fn read_fstring(&mut self) -> Token {
        self.advance(); // skip f
        // Check for triple-quoted f-string
        if self.peek() == Some('"') && self.peek_n(1) == Some('"') && self.peek_n(2) == Some('"') {
            self.advance(); self.advance(); self.advance();
            let mut s = String::new();
            loop {
                match self.peek() {
                    None => break,
                    Some('"') if self.peek_n(1) == Some('"') && self.peek_n(2) == Some('"') => {
                        self.advance(); self.advance(); self.advance();
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
                self.advance();
                if let Some(esc) = self.peek() {
                    s.push(match esc { 'n' => '\n', 't' => '\t', _ => esc });
                    self.advance();
                }
            } else {
                s.push(self.advance().unwrap());
            }
        }
        Token::FStrLit(s)
    }

    fn read_raw_string(&mut self) -> Token {
        self.advance(); // skip r
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

    fn read_ident_or_keyword(&mut self, first: char) -> Token {
        let mut s = String::from(first);
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(self.advance().unwrap());
            } else {
                break;
            }
        }

        // 魔法方法 __xxx__
        if s.starts_with("__") && s.ends_with("__") && s.len() > 4 {
            return Token::MagicMethod(s);
        }

        match s.as_str() {
            "def" => Token::Def,
            "iterator" => Token::Iterator,
            "mut" => Token::Mut,
            "ref" => Token::Ref,
            "const" => Token::Const,
            "let" => Token::Let,
            "owned" => Token::Owned,
            "owend" => Token::Owned,
            "return" => Token::Return,
            "yield" => Token::Yield,
            "if" => Token::If,
            "elif" => Token::Elif,
            "else" => Token::Else,
            "match" => Token::Match,
            "case" => Token::Case,
            "guard" => Token::Guard,
            "for" => Token::For,
            "in" => Token::In,
            "while" => Token::While,
            "loop" => Token::Loop,
            "break" => Token::Break,
            "continue" => Token::Continue,
            "struct" => Token::Struct,
            "enum" => Token::Enum,
            "trait" => Token::Trait,
            "impl" => Token::Impl,
            "where" => Token::Where,
            "Self" => Token::Self_,
            "try" => Token::Try,
            "catch" => Token::Catch,
            "finally" => Token::Finally,
            "with" => Token::With,
            "defer" => Token::Defer,
            "as" => Token::As,
            "import" => Token::Import,
            "from" => Token::From,
            "async" => Token::Async,
            "await" => Token::Await,
            "spawn" => Token::Spawn,
            "go" => Token::Go,
            "select" => Token::Select,
            "macro" => Token::Macro,
            "template" => Token::Template,
            "comptime" => Token::Comptime,
            "raise" => Token::Raise,
            "raises" => Token::Raises,
            "test" => Token::Test,
            "assert" => Token::Assert,
            "check" => Token::Check,
            "suite" => Token::Suite,
            "setup" => Token::Setup,
            "teardown" => Token::Teardown,
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            "is" => Token::Is,
            "duck" => Token::Duck,
            "block" => Token::Block,
            "True" => Token::True,
            "False" => Token::False,
            _ => Token::Ident(s),
        }
    }

    fn handle_indent(&mut self, col: usize, tokens: &mut Vec<Token>) {
        if let Some(mut virtual_tokens) = self.indent.handle(col) {
            tokens.append(&mut virtual_tokens);
        }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut line_start = true;

        loop {
            let c = match self.peek() {
                Some(c) => c,
                None => break,
            };

            // 行首非空白字符：处理 col 0 的 Dedent
            if line_start && c != ' ' && c != '\t' && c != '\n' && c != '\r' {
                self.handle_indent(0, &mut tokens);
                line_start = false;
            }

            match c {
                '\n' => {
                    self.advance();
                    // 连续空行不产生 Newline
                    if !tokens.is_empty() && tokens.last() != Some(&Token::Newline) {
                        tokens.push(Token::Newline);
                    }
                    line_start = true;
                }
                ' ' | '\t' if line_start => {
                    let col = self.skip_inline_whitespace();
                    // 空行或注释行不处理缩进
                    match self.peek() {
                        None => break,
                        Some('\n') => continue,
                        Some('/') if self.peek_n(1) == Some('/') => {
                            self.advance(); self.advance();
                            self.skip_line_comment();
                            continue;
                        }
                        Some('/') if self.peek_n(1) == Some('*') => {
                            self.advance(); self.advance();
                            self.skip_block_comment();
                            continue;
                        }
                        _ => {}
                    }
                    self.handle_indent(col, &mut tokens);
                    line_start = false;
                }
                ' ' | '\t' => { self.advance(); }
                '\r' => { self.advance(); }
                // # 不再作为注释：预留给 Rust 风格宏语法，交由下方 _ 兜底跳过
                '0'..='9' => {
                    let first = self.advance().unwrap();
                    tokens.push(self.read_number(first));
                    line_start = false;
                }
                '"' if self.peek_n(1) == Some('"') && self.peek_n(2) == Some('"') => {
                    tokens.push(self.read_triple_string());
                    line_start = false;
                }
                '"' => {
                    tokens.push(self.read_string());
                    line_start = false;
                }
                'f' if self.peek_n(1) == Some('"') => {
                    tokens.push(self.read_fstring());
                    line_start = false;
                }
                'r' if self.peek_n(1) == Some('"') => {
                    tokens.push(self.read_raw_string());
                    line_start = false;
                }
                '`' => {
                    self.advance();
                    tokens.push(Token::Backtick);
                    line_start = false;
                }
                c if c.is_alphabetic() || c == '_' => {
                    let first = self.advance().unwrap();
                    tokens.push(self.read_ident_or_keyword(first));
                    line_start = false;
                }
                // 构建块符号 =: 变量构建块（前后必须留白，其后必须换行缩进）
                '=' if self.peek_n(1) == Some(':') => {
                    if is_build_ws(self.prev_char()) && is_build_ws(self.peek_n(2)) {
                        self.advance(); self.advance();
                        tokens.push(Token::BuildAssign);
                    } else {
                        self.advance(); self.advance();
                        tokens.push(Token::LexError(
                            "构建块符号 '=:' 前后必须留白（符号前需空格，符号后需换行缩进）".into()));
                    }
                    line_start = false;
                }
                // 赋值/比较
                '=' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::EqEq);
                    line_start = false;
                }
                '=' if self.peek_n(1) == Some('>') => {
                    self.advance(); self.advance();
                    tokens.push(Token::FatArrow);
                    line_start = false;
                }
                '=' => { self.advance(); tokens.push(Token::Eq); line_start = false; }
                '!' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::NotEq);
                    line_start = false;
                }
                '!' => { self.advance(); tokens.push(Token::Exclamation); line_start = false; }

                // 比较/约束
                '<' => {
                    self.advance();
                    if self.peek() == Some('|') {
                        self.advance();
                        tokens.push(Token::BackPipe);
                    } else if self.peek() == Some('<') {
                        self.advance();
                        if self.peek() == Some('=') {
                            self.advance();
                            tokens.push(Token::ShlEq);
                        } else {
                            tokens.push(Token::Shl);
                        }
                    } else if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Token::Le);
                    } else {
                        tokens.push(Token::Lt);
                    }
                    line_start = false;
                }
                '>' => {
                    self.advance();
                    if self.peek() == Some('>') {
                        self.advance();
                        if self.peek() == Some('=') {
                            self.advance();
                            tokens.push(Token::ShrEq);
                        } else {
                            tokens.push(Token::Shr);
                        }
                    } else if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Token::Ge);
                    } else {
                        tokens.push(Token::Gt);
                    }
                    line_start = false;
                }

                // 构建块符号 *: 生成器调用构建块（前后必须留白，其后必须换行缩进）
                '*' if self.peek_n(1) == Some(':') => {
                    if is_build_ws(self.prev_char()) && is_build_ws(self.peek_n(2)) {
                        self.advance(); self.advance();
                        tokens.push(Token::BuildGen);
                    } else {
                        self.advance(); self.advance();
                        tokens.push(Token::LexError(
                            "构建块符号 '*:' 前后必须留白（符号前需空格，符号后需换行缩进）".into()));
                    }
                    line_start = false;
                }
                // 算术
                '+' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::PlusEq);
                    line_start = false;
                }
                '+' => { self.advance(); tokens.push(Token::Plus); line_start = false; }
                '-' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::MinusEq);
                    line_start = false;
                }
                '-' if self.peek_n(1) == Some('>') => {
                    self.advance(); self.advance();
                    tokens.push(Token::Arrow);
                    line_start = false;
                }
                '-' => { self.advance(); tokens.push(Token::Minus); line_start = false; }
                '*' if self.peek_n(1) == Some('*') => {
                    if self.peek_n(2) == Some('=') {
                        self.advance(); self.advance(); self.advance();
                        tokens.push(Token::PowEq);
                    } else {
                        self.advance(); self.advance();
                        tokens.push(Token::StarStar);
                    }
                    line_start = false;
                }
                '*' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::StarEq);
                    line_start = false;
                }
                '*' => { self.advance(); tokens.push(Token::Star); line_start = false; }
                '/' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::SlashEq);
                    line_start = false;
                }
                '/' if self.peek_n(1) == Some('/') => {
                    // 行注释（Java/Rust 体系），可出现在行尾
                    self.advance(); self.advance();
                    self.skip_line_comment();
                }
                '/' if self.peek_n(1) == Some('*') => {
                    // 块注释（Java/Rust 体系）
                    self.advance(); self.advance();
                    self.skip_block_comment();
                }
                '/' => { self.advance(); tokens.push(Token::Slash); line_start = false; }
                '%' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::PercentEq);
                    line_start = false;
                }
                '%' => { self.advance(); tokens.push(Token::Percent); line_start = false; }
                // 构建块符号 ~: 调用构建块（前后必须留白，其后必须换行缩进）
                '~' if self.peek_n(1) == Some(':') => {
                    if is_build_ws(self.prev_char()) && is_build_ws(self.peek_n(2)) {
                        self.advance(); self.advance();
                        tokens.push(Token::BuildCall);
                    } else {
                        self.advance(); self.advance();
                        tokens.push(Token::LexError(
                            "构建块符号 '~:' 前后必须留白（符号前需空格，符号后需换行缩进）".into()));
                    }
                    line_start = false;
                }
                '~' => {
                    // 后缀命名参数糖: x~ → 仅在紧贴标识符后时产生 Tilde
                    // 前缀 ~x (位非) → 产生 Token::Exclamation (等价于 !x)
                    // 先检查前置字符（advance 之前），再 advance
                    let prev_before_tilde = self.prev_char();
                    self.advance();
                    if prev_before_tilde.map_or(false, |c| c.is_alphanumeric() || c == '_' || c == ')') {
                        tokens.push(Token::Tilde);
                    } else {
                        tokens.push(Token::Exclamation);
                    }
                    line_start = false;
                }
                '^' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::XorEq);
                    line_start = false;
                }
                '^' => {
                    // `^:` → BuildIndex（索引构建块）
                    if self.peek_n(1) == Some(':') {
                        self.advance(); self.advance();
                        tokens.push(Token::BuildIndex);
                    } else {
                        let spaced = is_build_ws(self.prev_char());
                        self.advance();
                        tokens.push(if spaced { Token::CaretInfix } else { Token::CaretOp });
                    }
                    line_start = false;
                }

                // 位/逻辑
                '&' if self.peek_n(1) == Some('&') => {
                    self.advance(); self.advance();
                    tokens.push(Token::AmpAmp);
                    line_start = false;
                }
                '&' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::AndEq);
                    line_start = false;
                }
                '&' => { self.advance(); tokens.push(Token::Amp); line_start = false; }
                '|' if self.peek_n(1) == Some('|') => {
                    self.advance(); self.advance();
                    tokens.push(Token::PipePipe);
                    line_start = false;
                }
                '|' if self.peek_n(1) == Some('>') => {
                    self.advance(); self.advance();
                    tokens.push(Token::Pipe);
                    line_start = false;
                }
                '|' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::OrEq);
                    line_start = false;
                }
                '|' => { self.advance(); tokens.push(Token::Pipe_); line_start = false; }

                // 标点
                ':' => {
                    if self.peek_n(1) == Some(':') {
                        self.advance(); self.advance();
                        tokens.push(Token::PathSep);
                    } else if self.peek_n(1) == Some('=') {
                        self.advance(); self.advance();
                        tokens.push(Token::ColonEq);
                    } else {
                        self.advance(); tokens.push(Token::Colon);
                    }
                    line_start = false;
                }
                ',' => { self.advance(); tokens.push(Token::Comma); line_start = false; }
                ';' => { self.advance(); tokens.push(Token::Semicolon); line_start = false; }
                '.' => {
                    self.advance();
                    if self.peek() == Some('.') {
                        self.advance();
                        if self.peek() == Some('=') {
                            self.advance();
                            tokens.push(Token::DotDotEq);
                        } else {
                            tokens.push(Token::DotDot);
                        }
                    } else {
                        tokens.push(Token::Dot);
                    }
                    line_start = false;
                }
                '(' => { self.advance(); tokens.push(Token::LParen); line_start = false; }
                ')' => { self.advance(); tokens.push(Token::RParen); line_start = false; }
                '[' => { self.advance(); tokens.push(Token::LBrack); line_start = false; }
                ']' => { self.advance(); tokens.push(Token::RBrack); line_start = false; }
                '{' => { self.advance(); tokens.push(Token::LBrace); line_start = false; }
                '}' => { self.advance(); tokens.push(Token::RBrace); line_start = false; }
                '@' => { self.advance(); tokens.push(Token::At); line_start = false; }
                '$' => { self.advance(); tokens.push(Token::Dollar); line_start = false; }

                // 特殊
                '?' if self.peek_n(1) == Some('?') => {
                    self.advance(); self.advance();
                    tokens.push(Token::QuestionQuestion);
                    line_start = false;
                }
                '?' if self.peek_n(1) == Some('.') => {
                    self.advance(); self.advance();
                    tokens.push(Token::SafeNav);
                    line_start = false;
                }
                '?' => { self.advance(); tokens.push(Token::Question); line_start = false; }
                '_' if self.peek_n(1).map_or(true, |c| !c.is_alphanumeric() && c != '_') => {
                    self.advance();
                    tokens.push(Token::Underscore);
                    line_start = false;
                }

                // #! shebang / # attribute macro
                '#' if self.peek_n(1) == Some('!') => {
                    self.advance(); self.advance();
                    while self.pos < self.chars.len() && self.chars[self.pos] != '\n' {
                        self.pos += 1;
                    }
                }
                '#' => {
                    self.advance();
                    tokens.push(Token::At);
                    line_start = false;
                }

                _ => { self.advance(); } // skip unknown
            }
        }

        // EOF 结尾排空所有剩余缩进
        tokens.append(&mut self.indent.drain());
        // 清理尾部 Newline
        while tokens.last() == Some(&Token::Newline) {
            tokens.pop();
        }
        tokens.push(Token::Eof);
        tokens
    }
}


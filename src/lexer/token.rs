// Lang-Zong 编译器 — token.rs
// 词法分析: 源码 → Token 流, 处理缩进
// 对齐 hermes/00-最终语法规范.md

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ── 声明关键字 ──
    Def, Struct, Enum, Trait, Impl, Const, Mut, Ref, Owned, Let,
    Iterator,      // iterator 关键字（生成器函数定义）

    // ── 控制流 ──
    If, Elif, Else, Match, Case, Guard,
    For, In, While, Loop, Break, Continue, Return, With,
    Defer,

    // ── 异常 ──
    Try, Catch, Finally, Raise, Raises,

    // ── 测试 ──
    Test, Assert, Suite, Setup, Teardown,

    // ── 并发 ──
    Async, Await, Spawn, Go, Select,

    // ── 迭代 ──
    Yield,

    // ── 导入 ──
    Import, From, As,

    // ── 类型/泛型 ──
    Where, Self_, Duck,

    // ── 宏/编译期 ──
    Macro, Comptime,

    // ── 逻辑关键字 ──
    And, Or, Not, Is,

    // ── 字面量 ──
    True, False,

    // ── 字面量值 ──
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    FStrLit(String),       // f"..." 字符串插值
    RawStrLit(String),     // r"..." 原始字符串
    TripleStrLit(String),  // """..."""

    // ── 标识符 ──
    Ident(String),
    MagicMethod(String),   // __xxx__ 魔法方法

    // ── 赋值/比较 ──
    Eq,            // =
    EqEq,          // ==
    NotEq,         // !=
    Lt, Gt, Le, Ge,

    // ── 算术 ──
    Plus, Minus, Star, Slash, Percent, StarStar, // + - * / % **

    // ── 复合赋值 ──
    PlusEq, MinusEq, StarEq, SlashEq, PercentEq, // += -= *= /= %=
    AndEq, OrEq, XorEq, ShlEq, ShrEq, PowEq,    // &= |= ^= <<= >>= **=

    // ── 位运算 ──
    Amp, Pipe_, Caret, Shl, Shr, // & | ^ << >>
    AmpAmp, PipePipe,            // && ||

    // ── 标点 ──
    Colon, Comma, Dot, DotDot, DotDotEq, DotDotDot, Semicolon,
    PathSep,       // :: 命名空间路径分隔
    Arrow,         // -> 函数返回类型 / 函数类型注解 (T) -> U
    FatArrow,      // => match case 箭头（允许内联或换行缩进）
    Pipe,          // |>
    BackPipe,      // <|
    ColonEq,       // := 海象运算符
    LParen, RParen,
    LBrack, RBrack,
    LBrace, RBrace,

    // ── 特殊符号 ──
    At,            // @ 装饰器
    Question,      // ?
    QuestionQuestion, // ??
    SafeNav,       // ?.
    Exclamation,   // !
    Underscore,    // _
    CaretOp,       // ^ 所有权转移（紧贴前导 token：后缀 move / 紧贴 XOR）
    CaretInfix,    // ^ 前置留白：强制中缀 XOR，必须带右操作数（悬空报错）
    Backtick,      // ` 代码字面量（三反引号用于宏 quote 块）
    Dollar,        // $ 宏插值符号
    Tilde,         // ~ 命名参数糖 f(x~) -> f(x = x)

    // ── 构建块专用符号 ──
    LexError(String), // 词法错误（构建块符号留白违规等），由 parse_module 拒绝
    BuildAssign,   // =: 变量构建块
    BuildIndex,    // ^: 索引构建块
    BuildCall,     // ~: 调用构建块
    BuildGen,      // *: 生成器调用构建块

    // ── 缩进虚拟 Token ──
    Indent, Dedent, Newline,

    Eof,
}

/// 判断字符是否为构建块符号所需的"留白边界"：
// 保留向后兼容：规范定义在 src/util/chars.rs
use crate::util::chars;
/// 空格 / 制表符 / 换行 / 回车，或输入边界（None）。
/// 用于强制 `=:` `~:` `*: ` 前后必须留白。
/// 
/// 规范实现在 util::chars::is_build_ws，此处为兼容性重导出。
pub fn is_build_ws(c: Option<char>) -> bool {
    chars::is_build_ws(c)
}


pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    indent_stack: Vec<usize>,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            indent_stack: vec![0],
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
                    let val = i64::from_str_radix(&num[2..].replace('_', ""), 16).unwrap_or(0);
                    return Token::IntLit(val);
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
                    let val = i64::from_str_radix(&num[2..].replace('_', ""), 8).unwrap_or(0);
                    return Token::IntLit(val);
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
                    let val = i64::from_str_radix(&num[2..].replace('_', ""), 2).unwrap_or(0);
                    return Token::IntLit(val);
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
            Token::FloatLit(num.parse().unwrap_or(0.0))
        } else {
            Token::IntLit(num.parse().unwrap_or(0))
        }
    }

    fn read_string(&mut self) -> Token {
        self.advance(); // skip opening "
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance();
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
        Token::StrLit(s)
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
            "comptime" => Token::Comptime,
            "raise" => Token::Raise,
            "raises" => Token::Raises,
            "test" => Token::Test,
            "assert" => Token::Assert,
            "suite" => Token::Suite,
            "setup" => Token::Setup,
            "teardown" => Token::Teardown,
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            "is" => Token::Is,
            "duck" => Token::Duck,
            "True" => Token::True,
            "False" => Token::False,
            _ => Token::Ident(s),
        }
    }

    fn handle_indent(&mut self, col: usize, tokens: &mut Vec<Token>) {
        let last = *self.indent_stack.last().unwrap();
        if col > last {
            self.indent_stack.push(col);
            tokens.push(Token::Indent);
        } else if col < last {
            while self.indent_stack.len() > 1 && col < *self.indent_stack.last().unwrap() {
                self.indent_stack.pop();
                tokens.push(Token::Dedent);
            }
            // 验证缩进对齐
            if col != *self.indent_stack.last().unwrap() && self.indent_stack.len() > 1 {
                // 不匹配的缩进，静默处理（后续可报错）
            }
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
                    if self.peek() == Some('<') {
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
                    self.advance();
                    let prev = self.prev_char();
                    if prev.map_or(false, |c| c.is_alphanumeric() || c == '_' || c == ')') {
                        tokens.push(Token::Tilde);
                    }
                    line_start = false;
                }
                '^' if self.peek_n(1) == Some('=') => {
                    self.advance(); self.advance();
                    tokens.push(Token::XorEq);
                    line_start = false;
                }
                '^' => {
                    // 前置留白消歧：`a ^` (带空格) → CaretInfix（强制中缀 XOR，须带右操作数，
                    // 悬空报错）；`a^` (紧贴) → CaretOp（后缀 move / 紧贴 XOR，原位置消歧）。
                    let spaced = is_build_ws(self.prev_char());
                    self.advance();
                    tokens.push(if spaced { Token::CaretInfix } else { Token::CaretOp });
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
                        } else if self.peek() == Some('.') {
                            self.advance();
                            tokens.push(Token::DotDotDot);
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
                '<' if self.peek_n(1) == Some('|') => {
                    self.advance(); self.advance();
                    tokens.push(Token::BackPipe);
                    line_start = false;
                }
                '_' if self.peek_n(1).map_or(true, |c| !c.is_alphanumeric() && c != '_') => {
                    self.advance();
                    tokens.push(Token::Underscore);
                    line_start = false;
                }

                // #! shebang / # attribute macro
                // #! 整行 → 跳过（shebang 行），#ident → @ident
                '#' if self.peek_n(1) == Some('!') => {
                    // shebang 行：跳过整行
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

        // Dedent remaining at EOF
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            tokens.push(Token::Dedent);
        }
        // 清理尾部 Newline
        while tokens.last() == Some(&Token::Newline) {
            tokens.pop();
        }
        tokens.push(Token::Eof);
        tokens
    }
}

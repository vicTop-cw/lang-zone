$c = @'
// Minimal lexer: tokens needed for import/export/function signatures

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Str(String),
    Int(i64),
    KwImport,
    KwFrom,
    KwAs,
    KwDef,
    KwLet,
    KwTrue,
    KwFalse,
    Dot,
    Comma,
    LParen,
    RParen,
    Star,
    Slash,
    Plus,
    Minus,
    At,
    Arrow,     // ->
    Colon,
    Eq,        // =
    Lt,        // <
    Gt,        // >
    Question,  // ?
    Ampersand, // &
    LBracket,  // [
    RBracket,  // ]
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}

pub struct Lexer<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    line: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
            line: 1,
        }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token();
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        tokens
    }

    fn next_token(&mut self) -> Token {
        self.skip_whitespace();
        self.skip_comments();
        self.skip_whitespace();

        match self.peek_char() {
            None => self.tok(TokenKind::Eof),
            Some((_, '\n')) => {
                self.line += 1;
                self.advance();
                self.tok(TokenKind::Newline)
            }
            Some((_, '\r')) => {
                self.advance();
                if self.peek_char().map(|(_, c)| c) == Some('\n') {
                    self.advance();
                    self.line += 1;
                }
                self.tok(TokenKind::Newline)
            }
            Some((_, '.')) => {
                self.advance();
                self.tok(TokenKind::Dot)
            }
            Some((_, ',')) => {
                self.advance();
                self.tok(TokenKind::Comma)
            }
            Some((_, '(')) => {
                self.advance();
                self.tok(TokenKind::LParen)
            }
            Some((_, ')')) => {
                self.advance();
                self.tok(TokenKind::RParen)
            }
            Some((_, '*')) => {
                self.advance();
                self.tok(TokenKind::Star)
            }
            Some((_, '/')) => {
                self.advance();
                self.tok(TokenKind::Slash)
            }
            Some((_, '+')) => {
                self.advance();
                self.tok(TokenKind::Plus)
            }
            Some((_, '-')) => {
                self.advance();
                if self.peek_char().map(|(_, c)| c) == Some('>') {
                    self.advance();
                    self.tok(TokenKind::Arrow)
                } else {
                    self.tok(TokenKind::Minus)
                }
            }
            Some((_, '@')) => {
                self.advance();
                self.tok(TokenKind::At)
            }
            Some((_, ':')) => {
                self.advance();
                self.tok(TokenKind::Colon)
            }
            Some((_, '=')) => {
                self.advance();
                self.tok(TokenKind::Eq)
            }
            Some((_, '<')) => {
                self.advance();
                self.tok(TokenKind::Lt)
            }
            Some((_, '>')) => {
                self.advance();
                self.tok(TokenKind::Gt)
            }
            Some((_, '?')) => {
                self.advance();
                self.tok(TokenKind::Question)
            }
            Some((_, '&')) => {
                self.advance();
                self.tok(TokenKind::Ampersand)
            }
            Some((_, '[')) => {
                self.advance();
                self.tok(TokenKind::LBracket)
            }
            Some((_, ']')) => {
                self.advance();
                self.tok(TokenKind::RBracket)
            }
            Some((_, '"')) => self.read_string(),
            Some((_, c)) if c.is_ascii_digit() => self.read_number(),
            Some((_, c)) if is_ident_start(c) => self.read_ident_or_kw(),
            Some((_, _c)) => {
                let _ = self.advance();
                self.next_token()
            }
        }
    }

    fn tok(&self, kind: TokenKind) -> Token {
        Token { kind, line: self.line }
    }

    fn skip_whitespace(&mut self) {
        while let Some((_, c)) = self.peek_char() {
            if c == ' ' || c == '\t' {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_comments(&mut self) {
        if self.peek_str("//") {
            while let Some((_, c)) = self.peek_char() {
                if c == '\n' {
                    break;
                }
                self.advance();
            }
        } else if self.peek_str("/*") {
            self.advance();
            self.advance();
            while let Some((_, c)) = self.peek_char() {
                if c == '\n' {
                    self.line += 1;
                }
                self.advance();
                if self.peek_str("*/") {
                    self.advance();
                    self.advance();
                    break;
                }
            }
        }
    }

    fn read_string(&mut self) -> Token {
        let _start = self.advance().unwrap().0 + 1;
        let mut value = String::new();
        loop {
            match self.peek_char() {
                None => break,
                Some((_, '\n')) => break,
                Some((_, '\\')) => {
                    self.advance();
                    match self.peek_char() {
                        Some((_, 'n')) => { value.push('\n'); self.advance(); }
                        Some((_, 't')) => { value.push('\t'); self.advance(); }
                        Some((_, 'r')) => { value.push('\r'); self.advance(); }
                        Some((_, '\\')) => { value.push('\\'); self.advance(); }
                        Some((_, '"')) => { value.push('"'); self.advance(); }
                        Some((_, c)) => { value.push(c); self.advance(); }
                        None => break,
                    }
                }
                Some((_, '"')) => {
                    self.advance();
                    break;
                }
                Some((_, c)) => {
                    value.push(c);
                    self.advance();
                }
            }
        }
        self.tok(TokenKind::Str(value))
    }

    fn read_number(&mut self) -> Token {
        let start = self.peek_char().unwrap().0;
        let mut saw_dot = false;
        while let Some((_, c)) = self.peek_char() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.' && !saw_dot {
                // Could be a float or a method call like 1.to_string().
                // For simplicity, treat as part of number if followed by digit.
                if let Some((_, next)) = self.peek_next_char() {
                    if next.is_ascii_digit() {
                        saw_dot = true;
                        self.advance();
                        continue;
                    }
                }
                break;
            } else {
                break;
            }
        }
        let text = &self.source[start..self.cur_pos()];
        if saw_dot {
            self.tok(TokenKind::Float(text.parse().unwrap_or(0.0)))
        } else {
            self.tok(TokenKind::Int(text.parse().unwrap_or(0)))
        }
    }

    fn read_ident_or_kw(&mut self) -> Token {
        let start = self.peek_char().unwrap().0;
        while let Some((_i, c)) = self.peek_char() {
            if is_ident_continue(c) {
                self.advance();
            } else {
                break;
            }
        }
        let text = &self.source[start..self.cur_pos()];
        let kind = match text {
            "import" => TokenKind::KwImport,
            "from" => TokenKind::KwFrom,
            "as" => TokenKind::KwAs,
            "def" => TokenKind::KwDef,
            "let" => TokenKind::KwLet,
            "true" => TokenKind::KwTrue,
            "false" => TokenKind::KwFalse,
            _ => TokenKind::Ident(text.to_string()),
        };
        self.tok(kind)
    }

    fn peek_char(&mut self) -> Option<(usize, char)> {
        self.chars.peek().copied()
    }

    fn peek_next_char(&mut self) -> Option<(usize, char)> {
        let mut it = self.chars.clone();
        it.next()?;
        it.peek().copied()
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        self.chars.next()
    }

    fn cur_pos(&mut self) -> usize {
        self.peek_char().map(|(i, _)| i).unwrap_or(self.source.len())
    }

    fn peek_str(&mut self, s: &str) -> bool {
        let start = self.cur_pos();
        self.source[start..].starts_with(s)
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_import() {
        let mut lex = Lexer::new("import std.io");
        let toks = lex.tokenize();
        assert_eq!(toks[0].kind, TokenKind::KwImport);
        assert_eq!(toks[1].kind, TokenKind::Ident("std".into()));
        assert_eq!(toks[2].kind, TokenKind::Dot);
        assert_eq!(toks[3].kind, TokenKind::Ident("io".into()));
    }

    #[test]
    fn test_from_import_star() {
        let mut lex = Lexer::new("from std.io import *");
        let toks = lex.tokenize();
        assert_eq!(toks[0].kind, TokenKind::KwFrom);
        assert_eq!(toks[5].kind, TokenKind::Star);
    }

    #[test]
    fn test_export_decorator() {
        let mut lex = Lexer::new("@export(Rust)\ndef add() = 1");
        let toks = lex.tokenize();
        assert_eq!(toks[0].kind, TokenKind::At);
        assert_eq!(toks[1].kind, TokenKind::Ident("export".into()));
        assert_eq!(toks[2].kind, TokenKind::LParen);
        assert_eq!(toks[3].kind, TokenKind::Ident("Rust".into()));
        assert_eq!(toks[4].kind, TokenKind::RParen);
        assert_eq!(toks[6].kind, TokenKind::KwDef);
    }

    #[test]
    fn test_eq_token() {
        let mut lex = Lexer::new("def add() = 1");
        let toks = lex.tokenize();
        assert_eq!(toks[3].kind, TokenKind::RParen);
        assert_eq!(toks[4].kind, TokenKind::Eq);
        assert_eq!(toks[5].kind, TokenKind::Int(1));
    }

    #[test]
    fn test_operators() {
        let mut lex = Lexer::new("a + b - c * d / e");
        let toks = lex.tokenize();
        assert_eq!(toks[1].kind, TokenKind::Plus);
        assert_eq!(toks[3].kind, TokenKind::Minus);
        assert_eq!(toks[5].kind, TokenKind::Star);
        assert_eq!(toks[7].kind, TokenKind::Slash);
    }

    #[test]
    fn test_number_and_string() {
        let mut lex = Lexer::new(r#"42 3.14 "hi""#);
        let toks = lex.tokenize();
        assert_eq!(toks[0].kind, TokenKind::Int(42));
        assert_eq!(toks[2].kind, TokenKind::Float(3.14));
        assert_eq!(toks[4].kind, TokenKind::Str("hi".into()));
    }
}
'@
Set-Content -Path "src/lexer.rs" -Value $c -Encoding UTF8

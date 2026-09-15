
#[allow(unused_imports)]
#[allow(unused_variables)]
#[allow(dead_code)]
#[allow(non_snake_case)]

use std::collections::{HashMap, HashSet};
use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;
use std::fmt::Debug;
use std::fmt::Display;

use lz_builtins::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Def,
    Struct,
    If,
    Else,
    Return,
    IntLit {
        v: i64,
    },
    StrLit {
        s: String,
    },
    Ident {
        name: String,
    },
    MagicMethod {
        name: String,
    },
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    StarStar,
    Eq,
    EqEq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    AmpAmp,
    PipePipe,
    Colon,
    ColonColon,
    Comma,
    Dot,
    Arrow,
    FatArrow,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Newline,
    Eof,
}

pub fn is_keyword(s: String) -> bool {
    // STMT:Expr
    return s == "def".to_string() || s == "struct".to_string() || s == "if".to_string() || s == "else".to_string() || s == "return".to_string();
}

pub fn keyword_token(s: String) -> Token {
    // STMT:Expr
    return if s == "def".to_string() { Token::Def } else { if s == "struct".to_string() { Token::Struct } else { if s == "if".to_string() { Token::If } else { if s == "else".to_string() { Token::Else } else { Token::Return } } } };
}

pub fn punct_token(c: String) -> (bool, Token) {
    // STMT:Let
    let mut table: Vec<(String, Token)> = vec![("+".to_string(), Token::Plus), ("-".to_string(), Token::Minus), ("*".to_string(), Token::Star), ("/".to_string(), Token::Slash), ("%".to_string(), Token::Percent), (":".to_string(), Token::Colon), (",".to_string(), Token::Comma), (".".to_string(), Token::Dot), ("(".to_string(), Token::LParen), (")".to_string(), Token::RParen), ("{".to_string(), Token::LBrace), ("}".to_string(), Token::RBrace), ("<".to_string(), Token::Lt), (">".to_string(), Token::Gt)];
    // STMT:Let
    let mut result: (bool, Token) = (false, Token::Plus);
    // STMT:For
    for idx in (0i64..(table.len() as i64)).into_iter() {
        // STMT:Let
        let pair = table[((idx) as usize)].clone();
        // STMT:Expr
        if pair.0 == c {
            // STMT:Other
            result = (true, pair.1);
        } else { ()};
    }
    // STMT:Expr
    return result;
}

pub fn two_char_token(c1: String, c2: String) -> (bool, Token) {
    // STMT:Let
    let key: String = c1 + &c2[..];
    // STMT:Let
    let mut table: Vec<(String, Token)> = vec![("==".to_string(), Token::EqEq), ("!=".to_string(), Token::NotEq), ("<=".to_string(), Token::Le), (">=".to_string(), Token::Ge), ("&&".to_string(), Token::AmpAmp), ("||".to_string(), Token::PipePipe), ("::".to_string(), Token::ColonColon), ("->".to_string(), Token::Arrow), ("=>".to_string(), Token::FatArrow), ("**".to_string(), Token::StarStar)];
    // STMT:Let
    let mut result: (bool, Token) = (false, Token::Plus);
    // STMT:For
    for idx in (0i64..(table.len() as i64)).into_iter() {
        // STMT:Let
        let pair = table[((idx) as usize)].clone();
        // STMT:Expr
        if pair.0 == key {
            // STMT:Other
            result = (true, pair.1);
        } else { ()};
    }
    // STMT:Expr
    return result;
}

pub fn is_digit(c: String) -> bool {
    // STMT:Expr
    return c >= "0".to_string() && c <= "9".to_string();
}

pub fn is_alpha(c: String) -> bool {
    // STMT:Expr
    return (c >= "a".to_string() && c <= "z".to_string()) || (c >= "A".to_string() && c <= "Z".to_string()) || c == "_".to_string();
}

pub fn is_ident_char(c: String) -> bool {
    // STMT:Expr
    return is_alpha(c.clone()) || is_digit(c.clone());
}

pub fn char_at(s: String, idx: i64) -> String {
    // STMT:Expr
    return s[((idx) as usize)..(((idx + 1i64)) as usize)].to_string();
}

pub fn str_len(s: String) -> i64 {
    // STMT:Expr
    return (s.len() as i64);
}

pub fn scan_ident(src: String, start: i64) -> (String, i64) {
    // STMT:Let
    let mut i: i64 = start;
    // STMT:Other
    while i < str_len(src.clone()) && is_ident_char(char_at(src.clone(), i)) {
        // STMT:Let
        i = i + 1i64;
    }
    // STMT:Expr
    return (src[((start) as usize)..((i) as usize)].to_string(), i);
}

pub fn scan_int(src: String, start: i64) -> (String, i64) {
    // STMT:Let
    let mut i: i64 = start;
    // STMT:Other
    while i < str_len(src.clone()) && is_digit(char_at(src.clone(), i)) {
        // STMT:Let
        i = i + 1i64;
    }
    // STMT:Expr
    return (src[((start) as usize)..((i) as usize)].to_string(), i);
}

pub fn scan_punct(src: String, i: i64) -> ((bool, Token), i64) {
    // STMT:Let
    let c: String = char_at(src.clone(), i);
    // STMT:Let
    let r: (bool, Token) = punct_token(c.clone());
    // STMT:Expr
    return if r.0 { (r, i + 1i64)} else { if i + 1i64 < str_len(src.clone()) {
        // STMT:Let
        let two: (bool, Token) = two_char_token(c.clone(), char_at(src.clone(), i + 1i64));
        // STMT:Expr
        if two.0 { ((true, two.1), i + 2i64) } else { (r, i + 1i64) }
    } else { (r, i + 1i64)}};
}

pub fn scan_string(src: String, start: i64) -> (String, i64) {
    // STMT:Let
    let mut i: i64 = start + 1i64;
    // STMT:Other
    while i < str_len(src.clone()) && char_at(src.clone(), i) != "\"".to_string() {
        // STMT:Let
        i = i + 1i64;
    }
    // STMT:Expr
    return (src[(((start + 1i64)) as usize)..((i) as usize)].to_string(), i + 1i64);
}

pub fn digit_val(c: String) -> i64 {
    // STMT:Let
    let mut table: Vec<(String, i64)> = vec![("0".to_string(), 0i64), ("1".to_string(), 1i64), ("2".to_string(), 2i64), ("3".to_string(), 3i64), ("4".to_string(), 4i64), ("5".to_string(), 5i64), ("6".to_string(), 6i64), ("7".to_string(), 7i64), ("8".to_string(), 8i64), ("9".to_string(), 9i64)];
    // STMT:Let
    let mut result: i64 = 0i64;
    // STMT:For
    for idx in (0i64..(table.len() as i64)).into_iter() {
        // STMT:Let
        let pair = table[((idx) as usize)].clone();
        // STMT:Expr
        if pair.0 == c {
            // STMT:Other
            result = pair.1;
        } else { ()};
    }
    // STMT:Expr
    return result;
}

pub fn str_to_int(s: String) -> i64 {
    // STMT:Let
    let mut v: i64 = 0i64;
    // STMT:For
    for idx in (0i64..(s.len() as i64)).into_iter() {
        // STMT:Other
        v = LzAdd::__add__(v * 10i64, digit_val(char_at(s.clone(), idx)));
    }
    // STMT:Expr
    return v;
}

pub fn tokenize(src: String) -> Vec<Token> {
    // STMT:Let
    let mut tokens: Vec<Token> = Vec::new();
    // STMT:Let
    let mut i: i64 = 0i64;
    // STMT:Other
    while i < str_len(src.clone()) {
        // STMT:Let
        let c: String = char_at(src.clone(), i);
        // STMT:Expr
        if c == " ".to_string() || c == "\t".to_string() {
            // STMT:Let
            i = i + 1i64;
        } else { if c == "\n".to_string() {
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Newline]); __cat };
            // STMT:Let
            i = i + 1i64;
        } else { if c == "\"".to_string() {
            // STMT:Let
            let r: (String, i64) = scan_string(src.clone(), i);
            // STMT:Let
            i = r.1;
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::StrLit { s: r.0 }]); __cat };
        } else { if is_alpha(c.clone()) {
            // STMT:Let
            let r: (String, i64) = scan_ident(src.clone(), i);
            // STMT:Let
            i = r.1;
            // STMT:Let
            let word: String = r.0;
            // STMT:Expr
            if (word.len() as i64) > 4i64 && word[((0i64) as usize)..((2i64) as usize)] == "__".to_string() && word[((((word.len() as i64) - 2i64)) as usize)..(((word.len() as i64)) as usize)] == "__".to_string() {
                // STMT:Let
                tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::MagicMethod { name: word }]); __cat };
            } else { if is_keyword(word.clone()) {
                // STMT:Let
                tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![keyword_token(word.clone())]); __cat };
            } else {
                // STMT:Let
                tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Ident { name: word }]); __cat };
            }}
        } else { if is_digit(c.clone()) {
            // STMT:Let
            let r: (String, i64) = scan_int(src.clone(), i);
            // STMT:Let
            i = r.1;
            // STMT:Let
            let num: String = r.0;
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::IntLit { v: str_to_int(num.clone()) }]); __cat };
        } else {
            // STMT:Let
            let r: ((bool, Token), i64) = scan_punct(src.clone(), i);
            // STMT:Let
            i = r.1;
            // STMT:Let
            let pr = r.0;
            // STMT:Expr
            if pr.0 {
                // STMT:Let
                tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![pr.1]); __cat };
            } else { ()}
        }}}}}
    }
    // STMT:Let
    tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Eof]); __cat };
    // STMT:Expr
    return tokens;
}

pub fn display_token(t: Token) -> String {
    // STMT:Other
    match t.clone() {
        Token::Def => {
            // STMT:Expr
            "Def".to_string()
        }
        Token::Struct => {
            // STMT:Expr
            "Struct".to_string()
        }
        Token::If => {
            // STMT:Expr
            "If".to_string()
        }
        Token::Else => {
            // STMT:Expr
            "Else".to_string()
        }
        Token::Return => {
            // STMT:Expr
            "Return".to_string()
        }
        Token::IntLit { v: n } => {
            // STMT:Expr
            LzAdd::__add__("IntLit(".to_string(), n.to_string()) + &")".to_string()[..]
        }
        Token::StrLit { s: s } => {
            // STMT:Expr
            "StrLit(".to_string().to_string() + &s[..] + &")".to_string()[..]
        }
        Token::Ident { name: n } => {
            // STMT:Expr
            "Ident(".to_string().to_string() + &n[..] + &")".to_string()[..]
        }
        Token::MagicMethod { name: n } => {
            // STMT:Expr
            "MagicMethod(".to_string().to_string() + &n[..] + &")".to_string()[..]
        }
        Token::Plus => {
            // STMT:Expr
            "Plus".to_string()
        }
        Token::Minus => {
            // STMT:Expr
            "Minus".to_string()
        }
        Token::Star => {
            // STMT:Expr
            "Star".to_string()
        }
        Token::Slash => {
            // STMT:Expr
            "Slash".to_string()
        }
        Token::Percent => {
            // STMT:Expr
            "Percent".to_string()
        }
        Token::StarStar => {
            // STMT:Expr
            "StarStar".to_string()
        }
        Token::Eq => {
            // STMT:Expr
            "Eq".to_string()
        }
        Token::EqEq => {
            // STMT:Expr
            "EqEq".to_string()
        }
        Token::NotEq => {
            // STMT:Expr
            "NotEq".to_string()
        }
        Token::Lt => {
            // STMT:Expr
            "Lt".to_string()
        }
        Token::Gt => {
            // STMT:Expr
            "Gt".to_string()
        }
        Token::Le => {
            // STMT:Expr
            "Le".to_string()
        }
        Token::Ge => {
            // STMT:Expr
            "Ge".to_string()
        }
        Token::AmpAmp => {
            // STMT:Expr
            "AmpAmp".to_string()
        }
        Token::PipePipe => {
            // STMT:Expr
            "PipePipe".to_string()
        }
        Token::Colon => {
            // STMT:Expr
            "Colon".to_string()
        }
        Token::ColonColon => {
            // STMT:Expr
            "ColonColon".to_string()
        }
        Token::Comma => {
            // STMT:Expr
            "Comma".to_string()
        }
        Token::Dot => {
            // STMT:Expr
            "Dot".to_string()
        }
        Token::Arrow => {
            // STMT:Expr
            "Arrow".to_string()
        }
        Token::FatArrow => {
            // STMT:Expr
            "FatArrow".to_string()
        }
        Token::LParen => {
            // STMT:Expr
            "LParen".to_string()
        }
        Token::RParen => {
            // STMT:Expr
            "RParen".to_string()
        }
        Token::LBrace => {
            // STMT:Expr
            "LBrace".to_string()
        }
        Token::RBrace => {
            // STMT:Expr
            "RBrace".to_string()
        }
        Token::Newline => {
            // STMT:Expr
            "Newline".to_string()
        }
        Token::Eof => {
            // STMT:Expr
            "Eof".to_string()
        }
    }
}

pub fn main() {
    // STMT:Let
    let src: String = "def greet(name):\n    return \"hello \" + name\nif a >= 2 && b <= 3:\n    x.__len__()".to_string();
    // STMT:Let
    let mut toks: Vec<Token> = tokenize(src.clone());
    // STMT:For
    for idx in (0i64..(toks.len() as i64)).into_iter() {
        // STMT:Expr
        println!("{:?}", display_token(toks[((idx) as usize)].clone()));
    }
}

const __name__: &str = "main";

const __file__: &str = "F:\\AI\\lang-zone\\src\\frontend\\lz_lexer.lz";

const __package__: &str = "frontend";

const __path__: &str = "F:\\AI\\lang-zone\\src\\frontend";

const __doc__: &str = "";

const __is_macro__: bool = false;


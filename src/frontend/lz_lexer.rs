
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
    IntLit(i64),
    StrLit(String),
    Ident(String),
    MagicMethod(String),
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
    return s == "def".to_string() || s == "struct".to_string() || s == "if".to_string() || s == "else".to_string() || s == "return".to_string();
}

pub fn keyword_token(s: String) -> Token {
    return if s == "def".to_string() { Token::Def } else { if s == "struct".to_string() { Token::Struct } else { if s == "if".to_string() { Token::If } else { if s == "else".to_string() { Token::Else } else { Token::Return } } } };
}

pub fn punct_token(c: String) -> (bool, Token) {
    let table: Vec<(String, Token)> = vec![("+".to_string(), Token::Plus), ("-".to_string(), Token::Minus), ("*".to_string(), Token::Star), ("/".to_string(), Token::Slash), ("%".to_string(), Token::Percent), (":".to_string(), Token::Colon), (",".to_string(), Token::Comma), (".".to_string(), Token::Dot), ("(".to_string(), Token::LParen), (")".to_string(), Token::RParen), ("{".to_string(), Token::LBrace), ("}".to_string(), Token::RBrace), ("<".to_string(), Token::Lt), (">".to_string(), Token::Gt)];
    let mut result: (bool, Token) = (false, Token::Plus);
    for idx in (0i64..(table.len() as i64)).into_iter() {
        let pair = table[((idx) as usize)].clone();
        if pair.0 == c {
            result = (true, pair.1);
        } else { ()};
    }
    return result;
}

pub fn two_char_token(c1: String, c2: String) -> (bool, Token) {
    let key: String = c1 + &c2[..];
    let table: Vec<(String, Token)> = vec![("==".to_string(), Token::EqEq), ("!=".to_string(), Token::NotEq), ("<=".to_string(), Token::Le), (">=".to_string(), Token::Ge), ("&&".to_string(), Token::AmpAmp), ("||".to_string(), Token::PipePipe), ("::".to_string(), Token::ColonColon), ("->".to_string(), Token::Arrow), ("=>".to_string(), Token::FatArrow), ("**".to_string(), Token::StarStar)];
    let mut result: (bool, Token) = (false, Token::Plus);
    for idx in (0i64..(table.len() as i64)).into_iter() {
        let pair = table[((idx) as usize)].clone();
        if pair.0 == key {
            result = (true, pair.1);
        } else { ()};
    }
    return result;
}

pub fn is_digit(c: String) -> bool {
    return c >= "0".to_string() && c <= "9".to_string();
}

pub fn is_alpha(c: String) -> bool {
    return (c >= "a".to_string() && c <= "z".to_string()) || (c >= "A".to_string() && c <= "Z".to_string()) || c == "_".to_string();
}

pub fn is_ident_char(c: String) -> bool {
    return is_alpha(c.clone()) || is_digit(c.clone());
}

pub fn char_at(s: String, idx: i64) -> String {
    return s[(idx as usize)..((idx + 1i64) as usize)].to_string();
}

pub fn str_len(s: String) -> i64 {
    return (s.len() as i64);
}

pub fn scan_ident(src: String, start: i64) -> (String, i64) {
    let mut i: i64 = start;
    while i < str_len(src.clone()) && is_ident_char(char_at(src.clone(), i)) {
        i = i + 1i64;
    }
    return (src[(start as usize)..(i as usize)].to_string(), i);
}

pub fn scan_int(src: String, start: i64) -> (String, i64) {
    let mut i: i64 = start;
    while i < str_len(src.clone()) && is_digit(char_at(src.clone(), i)) {
        i = i + 1i64;
    }
    return (src[(start as usize)..(i as usize)].to_string(), i);
}

pub fn scan_punct(src: String, i: i64) -> ((bool, Token), i64) {
    let c: String = char_at(src.clone(), i);
    let r: (bool, Token) = punct_token(c.clone());
    return if r.0 { (r, i + 1i64)} else { if i + 1i64 < str_len(src.clone()) {
        let two: (bool, Token) = two_char_token(c.clone(), char_at(src.clone(), i + 1i64));
        if two.0 { ((true, two.1), i + 2i64) } else { (r, i + 1i64) }
    } else { (r, i + 1i64)}};
}

pub fn scan_string(src: String, start: i64) -> (String, i64) {
    let mut i: i64 = start + 1i64;
    while i < str_len(src.clone()) && char_at(src.clone(), i) != "\"".to_string() {
        i = i + 1i64;
    }
    return (src[((start + 1i64) as usize)..(i as usize)].to_string(), i + 1i64);
}

pub fn digit_val(c: String) -> i64 {
    let table: Vec<(String, i64)> = vec![("0".to_string(), 0i64), ("1".to_string(), 1i64), ("2".to_string(), 2i64), ("3".to_string(), 3i64), ("4".to_string(), 4i64), ("5".to_string(), 5i64), ("6".to_string(), 6i64), ("7".to_string(), 7i64), ("8".to_string(), 8i64), ("9".to_string(), 9i64)];
    let mut result: i64 = 0i64;
    for idx in (0i64..(table.len() as i64)).into_iter() {
        let pair = table[((idx) as usize)].clone();
        if pair.0 == c {
            result = pair.1;
        } else { ()};
    }
    return result;
}

pub fn str_to_int(s: String) -> i64 {
    let mut v: i64 = 0i64;
    for idx in (0i64..(s.len() as i64)).into_iter() {
        v = v * 10i64 + digit_val(char_at(s.clone(), idx));
    }
    return v;
}

pub fn tokenize(src: String) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut i: i64 = 0i64;
    while i < str_len(src.clone()) {
        let c: String = char_at(src.clone(), i);
        if c == " ".to_string() || c == "\t".to_string() {
            i = i + 1i64;
        } else { if c == "\n".to_string() {
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Newline]); __lz_cat };
            i = i + 1i64;
        } else { if c == "\"".to_string() {
            let r: (String, i64) = scan_string(src.clone(), i);
            i = r.1;
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::StrLit(r.0)]); __lz_cat };
        } else { if is_alpha(c.clone()) {
            let r: (String, i64) = scan_ident(src.clone(), i);
            i = r.1;
            let word: String = r.0;
            if (word.len() as i64) > 4i64 && word[(0i64 as usize)..(2i64 as usize)] == "__".to_string() && word[(((word.len() as i64) - 2i64) as usize)..((word.len() as i64) as usize)] == "__".to_string() {
                tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::MagicMethod(word)]); __lz_cat };
            } else { if is_keyword(word.clone()) {
                tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![keyword_token(word.clone())]); __lz_cat };
            } else {
                tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Ident(word)]); __lz_cat };
            }}
        } else { if is_digit(c.clone()) {
            let r: (String, i64) = scan_int(src.clone(), i);
            i = r.1;
            let num: String = r.0;
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::IntLit(str_to_int(num.clone()))]); __lz_cat };
        } else {
            let r: ((bool, Token), i64) = scan_punct(src.clone(), i);
            i = r.1;
            let pr = r.0;
            if pr.0 {
                tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![pr.1]); __lz_cat };
            } else { ()}
        }}}}}
    }
    tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Eof]); __lz_cat };
    return tokens;
}

pub fn display_token(t: Token) -> String {
    match t.clone() {
        Token::Def => {
            "Def".to_string()
        }
        Token::Struct => {
            "Struct".to_string()
        }
        Token::If => {
            "If".to_string()
        }
        Token::Else => {
            "Else".to_string()
        }
        Token::Return => {
            "Return".to_string()
        }
        Token::IntLit(n) => {
            "IntLit(".to_string().to_string() + &n.to_string()[..] + &")".to_string()[..]
        }
        Token::StrLit(s) => {
            "StrLit(".to_string().to_string() + &s[..] + &")".to_string()[..]
        }
        Token::Ident(n) => {
            "Ident(".to_string().to_string() + &n[..] + &")".to_string()[..]
        }
        Token::MagicMethod(n) => {
            "MagicMethod(".to_string().to_string() + &n[..] + &")".to_string()[..]
        }
        Token::Plus => {
            "Plus".to_string()
        }
        Token::Minus => {
            "Minus".to_string()
        }
        Token::Star => {
            "Star".to_string()
        }
        Token::Slash => {
            "Slash".to_string()
        }
        Token::Percent => {
            "Percent".to_string()
        }
        Token::StarStar => {
            "StarStar".to_string()
        }
        Token::Eq => {
            "Eq".to_string()
        }
        Token::EqEq => {
            "EqEq".to_string()
        }
        Token::NotEq => {
            "NotEq".to_string()
        }
        Token::Lt => {
            "Lt".to_string()
        }
        Token::Gt => {
            "Gt".to_string()
        }
        Token::Le => {
            "Le".to_string()
        }
        Token::Ge => {
            "Ge".to_string()
        }
        Token::AmpAmp => {
            "AmpAmp".to_string()
        }
        Token::PipePipe => {
            "PipePipe".to_string()
        }
        Token::Colon => {
            "Colon".to_string()
        }
        Token::ColonColon => {
            "ColonColon".to_string()
        }
        Token::Comma => {
            "Comma".to_string()
        }
        Token::Dot => {
            "Dot".to_string()
        }
        Token::Arrow => {
            "Arrow".to_string()
        }
        Token::FatArrow => {
            "FatArrow".to_string()
        }
        Token::LParen => {
            "LParen".to_string()
        }
        Token::RParen => {
            "RParen".to_string()
        }
        Token::LBrace => {
            "LBrace".to_string()
        }
        Token::RBrace => {
            "RBrace".to_string()
        }
        Token::Newline => {
            "Newline".to_string()
        }
        Token::Eof => {
            "Eof".to_string()
        }
    }
}

pub fn main() {
    let src: String = "def greet(name):\n    return \"hello \" + name\nif a >= 2 && b <= 3:\n    x.__len__()".to_string();
    let toks: Vec<Token> = tokenize(src.clone());
    for idx in (0i64..(toks.len() as i64)).into_iter() {
        println!("{:?}", display_token(toks[((idx) as usize)].clone()));
    }
}

const __name__: &str = "main";

const __file__: &str = "src\\frontend\\lz_lexer.lz";

const __package__: &str = "frontend";

const __path__: &str = "src\\frontend";

const __doc__: &str = "";

const __is_macro__: bool = false;


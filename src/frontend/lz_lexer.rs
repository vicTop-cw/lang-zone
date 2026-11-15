
// ── LZ rustc link recipe (BUG-10: lz_builtins re-exports num-bigint/num-complex) ──
// To compile & run this generated file in isolation:
//   rustc --edition 2021 --extern lz_builtins=<rlib> -L dependency=<deps_dir> -O <this_file>.rs
//   <rlib>     = liblz_builtins.rlib (build it: `cargo build -p lz_builtins`)
//   <deps_dir> = the directory containing that rlib (e.g. <workspace>/target/debug/deps)
// Omitting -L gives error[E0463] even if this program never uses BigInt/Complex.
// ─────────────────────────────────────────────────────────────────────────────────────

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
    return (s).to_string() == ("def".to_string()).to_string() || (s).to_string() == ("struct".to_string()).to_string() || (s).to_string() == ("if".to_string()).to_string() || (s).to_string() == ("else".to_string()).to_string() || (s).to_string() == ("return".to_string()).to_string();
}

pub fn keyword_token(s: String) -> Token {
    return if (s).to_string() == ("def".to_string()).to_string() { Token::Def } else { if (s).to_string() == ("struct".to_string()).to_string() { Token::Struct } else { if (s).to_string() == ("if".to_string()).to_string() { Token::If } else { if (s).to_string() == ("else".to_string()).to_string() { Token::Else } else { Token::Return } } } };
}

pub fn punct_token(c: String) -> (bool, Token) {
    let mut table: Vec<(String, Token)> = vec![("+".to_string(), Token::Plus), ("-".to_string(), Token::Minus), ("*".to_string(), Token::Star), ("/".to_string(), Token::Slash), ("%".to_string(), Token::Percent), (":".to_string(), Token::Colon), (",".to_string(), Token::Comma), (".".to_string(), Token::Dot), ("(".to_string(), Token::LParen), (")".to_string(), Token::RParen), ("{".to_string(), Token::LBrace), ("}".to_string(), Token::RBrace), ("<".to_string(), Token::Lt), (">".to_string(), Token::Gt)];
    let mut result: (bool, Token) = (false, Token::Plus);
    for idx in 0i64..(table.len() as i64) {
        let pair = table[idx as usize].clone();
        if pair.0 == c {
            result = (true, pair.1);
        }
    }
    return result;
}

pub fn two_char_token(c1: String, c2: String) -> (bool, Token) {
    let key: String = c1.to_string() + &c2[..];
    let mut table: Vec<(String, Token)> = vec![("==".to_string(), Token::EqEq), ("!=".to_string(), Token::NotEq), ("<=".to_string(), Token::Le), (">=".to_string(), Token::Ge), ("&&".to_string(), Token::AmpAmp), ("||".to_string(), Token::PipePipe), ("::".to_string(), Token::ColonColon), ("->".to_string(), Token::Arrow), ("=>".to_string(), Token::FatArrow), ("**".to_string(), Token::StarStar)];
    let mut result: (bool, Token) = (false, Token::Plus);
    for idx in 0i64..(table.len() as i64) {
        let pair = table[idx as usize].clone();
        if pair.0 == key {
            result = (true, pair.1);
        }
    }
    return result;
}

pub fn is_digit(c: String) -> bool {
    return (c).to_string() >= ("0".to_string()).to_string() && (c).to_string() <= ("9".to_string()).to_string();
}

pub fn is_alpha(c: String) -> bool {
    return ((c).to_string() >= ("a".to_string()).to_string() && (c).to_string() <= ("z".to_string()).to_string()) || ((c).to_string() >= ("A".to_string()).to_string() && (c).to_string() <= ("Z".to_string()).to_string()) || (c).to_string() == ("_".to_string()).to_string();
}

pub fn is_ident_char(c: String) -> bool {
    return is_alpha(c.clone()) || is_digit(c.clone());
}

pub fn char_at(s: String, idx: i64) -> String {
    return s[((idx) as usize)..(((idx + 1i64)) as usize)].to_string();
}

pub fn str_len(mut s: String) -> i64 {
    return (s.len() as i64);
}

pub fn scan_ident(src: String, start: i64) -> (String, i64) {
    let mut i: i64 = start;
    while i < str_len(src.clone()) && is_ident_char(char_at(src.clone(), i)) {
        i += 1i64;
    }
    return (src[((start) as usize)..((i) as usize)].to_string(), i);
}

pub fn scan_int(src: String, start: i64) -> (String, i64) {
    let mut i: i64 = start;
    while i < str_len(src.clone()) && is_digit(char_at(src.clone(), i)) {
        i += 1i64;
    }
    return (src[((start) as usize)..((i) as usize)].to_string(), i);
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
    while i < str_len(src.clone()) && (char_at(src.clone(), i)).to_string() != ("\"".to_string()).to_string() {
        i += 1i64;
    }
    return (src[(((start + 1i64)) as usize)..((i) as usize)].to_string(), i + 1i64);
}

pub fn digit_val(c: String) -> i64 {
    let mut table: Vec<(String, i64)> = vec![("0".to_string(), 0i64), ("1".to_string(), 1i64), ("2".to_string(), 2i64), ("3".to_string(), 3i64), ("4".to_string(), 4i64), ("5".to_string(), 5i64), ("6".to_string(), 6i64), ("7".to_string(), 7i64), ("8".to_string(), 8i64), ("9".to_string(), 9i64)];
    let mut result: i64 = 0i64;
    for idx in 0i64..(table.len() as i64) {
        let pair = table[idx as usize].clone();
        if pair.0 == c {
            result = pair.1;
        }
    }
    return result;
}

pub fn str_to_int(mut s: String) -> i64 {
    let mut v: i64 = 0i64;
    for idx in 0i64..(s.len() as i64) {
        v = LzAdd::__add__(v * 10i64, digit_val(char_at(s.clone(), idx)));
    }
    return v;
}

pub fn tokenize(src: String) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut i: i64 = 0i64;
    while i < str_len(src.clone()) {
        let c: String = char_at(src.clone(), i);
        if (c).to_string() == (" ".to_string()).to_string() || (c).to_string() == ("\t".to_string()).to_string() {
            i += 1i64;
        } else { if (c).to_string() == ("\n".to_string()).to_string() {
            tokens.push(Token::Newline);
            i += 1i64;
        } else { if (c).to_string() == ("\"".to_string()).to_string() {
            let r: (String, i64) = scan_string(src.clone(), i);
            i = r.1;
            tokens.push(Token::StrLit { s: r.0 });
        } else { if is_alpha(c.clone()) {
            let r: (String, i64) = scan_ident(src.clone(), i);
            i = r.1;
            let word: String = r.0;
            if (word.len() as i64) > 4i64 && word[((0i64) as usize)..((2i64) as usize)].to_string() == "__".to_string() && word[((((word.len() as i64) - 2i64)) as usize)..(((word.len() as i64)) as usize)].to_string() == "__".to_string() {
                tokens.push(Token::MagicMethod { name: word });
            } else { if is_keyword(word.clone()) {
                tokens.push(keyword_token(word.clone()));
            } else {
                tokens.push(Token::Ident { name: word });
            }}
        } else { if is_digit(c.clone()) {
            let r: (String, i64) = scan_int(src.clone(), i);
            i = r.1;
            let num: String = r.0;
            tokens.push(Token::IntLit { v: str_to_int(num.clone()) });
        } else {
            let r: ((bool, Token), i64) = scan_punct(src.clone(), i);
            i = r.1;
            let pr = r.0;
            if pr.0 {
                tokens.push(pr.1);
            }
        }}}}}
    }
    tokens.push(Token::Eof);
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
        Token::IntLit { v: n } => {
            "IntLit(".to_string().to_string() + &n.to_string()[..] + &")".to_string()[..]
        }
        Token::StrLit { s: s } => {
            "StrLit(".to_string().to_string() + &s[..] + &")".to_string()[..]
        }
        Token::Ident { name: n } => {
            "Ident(".to_string().to_string() + &n[..] + &")".to_string()[..]
        }
        Token::MagicMethod { name: n } => {
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
    let mut toks: Vec<Token> = tokenize(src.clone());
    for idx in 0i64..(toks.len() as i64) {
        println!("{:?}", display_token(toks[idx as usize].clone()));
    }
}

const __name__: &str = "main";

const __file__: &str = "lz_lexer.lz";

const __package__: &str = "frontend";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\src\\frontend";

const __doc__: &str = "";

const __is_macro__: bool = false;


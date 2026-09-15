
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
    IntLit {
        v: i64,
    },
    StrLit {
        s: String,
    },
    Ident {
        name: String,
    },
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Arrow,
    FatArrow,
    EqEq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    AmpAmp,
    PipePipe,
    Eq,
    Let,
    Dot,
    Match,
    Case,
    While,
    For,
    In,
    Return,
    If,
    Else,
    Def,
    Indent {
        v: i64,
    },
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit {
        v: i64,
    },
    StrLit {
        v: String,
    },
    Ident {
        name: String,
    },
    Bin {
        op: String,
        l: Box<Expr>,
        r: Box<Expr>,
    },
    Cmp {
        op: String,
        l: Box<Expr>,
        r: Box<Expr>,
    },
    Logic {
        op: String,
        l: Box<Expr>,
        r: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Box<Vec<Expr>>,
    },
    Get {
        recv: Box<Expr>,
        name: String,
    },
    Index {
        recv: Box<Expr>,
        idx: Box<Expr>,
    },
    ListLit {
        elems: Box<Vec<Expr>>,
    },
    TupleLit {
        elems: Box<Vec<Expr>>,
    },
    DictLit {
        pairs: Box<Vec<(Expr, Expr)>>,
    },
}

pub fn char_at(s: String, idx: i64) -> String {
    // STMT:Expr
    return s[((idx) as usize)..(((idx + 1i64)) as usize)].to_string();
}

pub fn str_len(s: String) -> i64 {
    // STMT:Expr
    return (s.len() as i64);
}

pub fn is_digit(c: String) -> bool {
    // STMT:Expr
    return c >= "0".to_string() && c <= "9".to_string();
}

pub fn is_alpha(c: String) -> bool {
    // STMT:Expr
    return (c >= "a".to_string() && c <= "z".to_string()) || (c >= "A".to_string() && c <= "Z".to_string()) || c == "_".to_string();
}

pub fn scan_int(src: String, start: i64) -> (i64, i64) {
    // STMT:Let
    let mut i: i64 = start;
    // STMT:Other
    while i < str_len(src.clone()) && is_digit(char_at(src.clone(), i)) {
        // STMT:Let
        i = i + 1i64;
    }
    // STMT:Let
    let v: i64 = (src[((start) as usize)..((i) as usize)].to_string()).parse::<i64>().unwrap();
    // STMT:Let
    let res: (i64, i64) = (v, i);
    // STMT:Expr
    return res;
}

pub fn scan_indent(src: String, start: i64) -> (i64, i64) {
    // STMT:Let
    let mut j: i64 = start;
    // STMT:Other
    while j < str_len(src.clone()) && (char_at(src.clone(), j) == " ".to_string() || char_at(src.clone(), j) == "\t".to_string()) {
        // STMT:Let
        j = j + 1i64;
    }
    // STMT:Expr
    return (j - start, j);
}

pub fn scan_ident(src: String, start: i64) -> (String, i64) {
    // STMT:Let
    let mut i: i64 = start;
    // STMT:Other
    while i < str_len(src.clone()) && (is_alpha(char_at(src.clone(), i)) || is_digit(char_at(src.clone(), i))) {
        // STMT:Let
        i = i + 1i64;
    }
    // STMT:Expr
    return (src[((start) as usize)..((i) as usize)].to_string(), i);
}

pub fn scan_string(src: String, start: i64) -> (String, i64) {
    // STMT:Let
    let mut i: i64 = start + 1i64;
    // STMT:Other
    while i < str_len(src.clone()) && char_at(src.clone(), i) != "\"".to_string() {
        // STMT:Let
        i = i + 1i64;
    }
    // STMT:Let
    let res: (String, i64) = (src[(((start + 1i64)) as usize)..((i) as usize)].to_string(), i + 1i64);
    // STMT:Expr
    return res;
}

pub fn keyword_token(w: String) -> (bool, Token) {
    // STMT:Let
    let mut table: Vec<(String, Token)> = vec![("return".to_string(), Token::Return), ("if".to_string(), Token::If), ("else".to_string(), Token::Else), ("def".to_string(), Token::Def), ("let".to_string(), Token::Let), ("match".to_string(), Token::Match), ("case".to_string(), Token::Case), ("while".to_string(), Token::While), ("for".to_string(), Token::For), ("in".to_string(), Token::In)];
    // STMT:Let
    let mut result: (bool, Token) = (false, Token::Ident { name: w.clone() });
    // STMT:For
    for idx in (0i64..(table.len() as i64)).into_iter() {
        // STMT:Let
        let pair = table[((idx) as usize)].clone();
        // STMT:Expr
        if pair.0 == w {
            // STMT:Other
            result = (true, pair.1);
        } else { ()};
    }
    // STMT:Expr
    return result;
}

pub fn op_token(c: String) -> (bool, Token) {
    // STMT:Let
    let mut table: Vec<(String, Token)> = vec![("+".to_string(), Token::Plus), ("-".to_string(), Token::Minus), ("*".to_string(), Token::Star), ("/".to_string(), Token::Slash), ("(".to_string(), Token::LParen), (")".to_string(), Token::RParen), ("[".to_string(), Token::LBracket), ("]".to_string(), Token::RBracket), ("{".to_string(), Token::LBrace), ("}".to_string(), Token::RBrace), ("<".to_string(), Token::Lt), (">".to_string(), Token::Gt), ("=".to_string(), Token::Eq), (".".to_string(), Token::Dot)];
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

pub fn two_char_op(c1: String, c2: String) -> (bool, Token) {
    // STMT:Let
    let key: String = c1 + &c2[..];
    // STMT:Let
    let mut table: Vec<(String, Token)> = vec![("==".to_string(), Token::EqEq), ("!=".to_string(), Token::Ne), ("<=".to_string(), Token::Le), (">=".to_string(), Token::Ge), ("&&".to_string(), Token::AmpAmp), ("||".to_string(), Token::PipePipe), ("=>".to_string(), Token::FatArrow)];
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

pub fn tokenize(src: String) -> Vec<Token> {
    // STMT:Let
    let mut tokens: Vec<Token> = Vec::new();
    // STMT:Let
    let mut i: i64 = 0i64;
    // STMT:Let
    let mut line_start: bool = true;
    // STMT:Other
    while i < str_len(src.clone()) {
        // STMT:Let
        let c: String = char_at(src.clone(), i);
        // STMT:Expr
        if c == "\n".to_string() {
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Newline]); __cat };
            // STMT:Let
            i = i + 1i64;
            // STMT:Let
            line_start = true;
        } else { if line_start && c == " ".to_string() {
            // STMT:Let
            let r: (i64, i64) = scan_indent(src.clone(), i);
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Indent { v: r.0 }]); __cat };
            // STMT:Let
            i = r.1;
            // STMT:Let
            line_start = false;
        } else { if line_start {
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Indent { v: 0i64 }]); __cat };
            // STMT:Let
            line_start = false;
        } else { if c == " ".to_string() {
            // STMT:Let
            i = i + 1i64;
        } else { if c == "\"".to_string() {
            // STMT:Let
            let r: (String, i64) = scan_string(src.clone(), i);
            // STMT:Let
            i = r.1;
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::StrLit { s: r.0 }]); __cat };
        } else { if is_digit(c.clone()) {
            // STMT:Let
            let r: (i64, i64) = scan_int(src.clone(), i);
            // STMT:Let
            i = r.1;
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::IntLit { v: r.0 }]); __cat };
        } else { if is_alpha(c.clone()) {
            // STMT:Let
            let r: (String, i64) = scan_ident(src.clone(), i);
            // STMT:Let
            i = r.1;
            // STMT:Let
            let kw: (bool, Token) = keyword_token(r.0.clone());
            // STMT:Expr
            if kw.0 {
                // STMT:Let
                tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![kw.1]); __cat };
            } else {
                // STMT:Let
                tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Ident { name: r.0 }]); __cat };
            }
        } else { if c == ":".to_string() {
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Colon]); __cat };
            // STMT:Let
            i = i + 1i64;
        } else { if c == ",".to_string() {
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Comma]); __cat };
            // STMT:Let
            i = i + 1i64;
        } else { if c == "-".to_string() && i + 1i64 < str_len(src.clone()) && char_at(src.clone(), i + 1i64) == ">".to_string() {
            // STMT:Let
            tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Arrow]); __cat };
            // STMT:Let
            i = i + 2i64;
        } else {
            // STMT:Let
            let mut handled: bool = false;
            // STMT:Expr
            if i + 1i64 < str_len(src.clone()) {
                // STMT:Let
                let r2: (bool, Token) = two_char_op(c.clone(), char_at(src.clone(), i + 1i64));
                // STMT:Expr
                if r2.0 {
                    // STMT:Let
                    tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![r2.1]); __cat };
                    // STMT:Let
                    i = i + 2i64;
                    // STMT:Let
                    handled = true;
                } else { ()}
            } else { ()};
            // STMT:Expr
            if !(handled) {
                // STMT:Let
                let r: (bool, Token) = op_token(c.clone());
                // STMT:Let
                let found = r.0;
                // STMT:Let
                let tok = r.1;
                // STMT:Expr
                if found {
                    // STMT:Let
                    tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![tok.clone()]); __cat };
                    // STMT:Let
                    i = i + 1i64;
                } else {
                    // STMT:Let
                    i = i + 1i64;
                }
            } else { ()}
        }}}}}}}}}}
    }
    // STMT:Let
    tokens = { let mut __cat = tokens.clone(); __cat.extend(vec![Token::Eof]); __cat };
    // STMT:Expr
    return tokens;
}

pub fn tok_at(toks: Vec<Token>, idx: i64) -> Token {
    // STMT:Expr
    return if idx >= (toks.len() as i64) { Token::Eof } else { toks[((idx) as usize)].clone() };
}

pub fn parse_atom(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    // STMT:Let
    let t = tok_at(toks.clone(), pos);
    // STMT:Let
    let res: (Expr, i64) = {
// STMT:Other
match t.clone() {
    Token::IntLit { v: n } => {
        // STMT:Expr
        (Expr::IntLit { v: n }, LzAdd::__add__(pos, 1i64))
    }
    Token::StrLit { s: sv } => {
        // STMT:Expr
        (Expr::StrLit { v: sv }, LzAdd::__add__(pos, 1i64))
    }
    Token::Ident { name: nm } => {
        // STMT:Expr
        (Expr::Ident { name: nm }, LzAdd::__add__(pos, 1i64))
    }
    Token::LParen => {
        // STMT:Let
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let rv = r.0;
        // STMT:Let
        let rp = r.1;
        // STMT:Let
        let sep = tok_at(toks.clone(), rp);
        // STMT:Expr
        if sep == Token::Comma {
            // STMT:Let
            let mut elems: Vec<Expr> = vec![rv.clone()];
            // STMT:Let
            let mut p: i64 = LzAdd::__add__(rp, 1i64);
            // STMT:Let
            let mut done: bool = false;
            // STMT:Other
            while !done {
                // STMT:Let
                let t2 = tok_at(toks.clone(), p);
                // STMT:Expr
                if t2 == Token::RParen {
                    // STMT:Let
                    done = true;
                    // STMT:Let
                    p = LzAdd::__add__(p, 1i64);
                } else {
                    // STMT:Let
                    let r2 = parse_logic(toks.clone(), p);
                    // STMT:Let
                    let rv2 = r2.0;
                    // STMT:Let
                    let rp2 = r2.1;
                    // STMT:Let
                    elems = { let mut __cat = elems.clone(); __cat.extend(vec![rv2.clone()]); __cat };
                    // STMT:Let
                    let sep2 = tok_at(toks.clone(), rp2);
                    // STMT:Expr
                    if sep2 == Token::Comma {
                        // STMT:Let
                        p = LzAdd::__add__(rp2, 1i64);
                    } else {
                        // STMT:Let
                        p = rp2;
                    }
                }
            }
            // STMT:Let
            let pr: (Expr, i64) = (Expr::TupleLit { elems: Box::new(elems) }, p);
            // STMT:Expr
            pr
        } else {
            // STMT:Let
            let pr: (Expr, i64) = (rv, LzAdd::__add__(rp, 1i64));
            // STMT:Expr
            pr
        }
    }
    Token::LBracket => {
        // STMT:Let
        let r = parse_list_elems(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let elems = r.0;
        // STMT:Let
        let rp = r.1;
        // STMT:Let
        let pr: (Expr, i64) = (Expr::ListLit { elems: Box::new(elems) }, LzAdd::__add__(rp, 1i64));
        // STMT:Expr
        pr
    }
    Token::LBrace => {
        // STMT:Let
        let r = parse_dict_pairs(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let pairs = r.0;
        // STMT:Let
        let rp = r.1;
        // STMT:Let
        let pr: (Expr, i64) = (Expr::DictLit { pairs: Box::new(pairs) }, LzAdd::__add__(rp, 1i64));
        // STMT:Expr
        pr
    }
    _ => {
        // STMT:Expr
        (Expr::Ident { name: "?".to_string() }, LzAdd::__add__(pos, 1i64))
    }
}
    };
    // STMT:Expr
    return res;
}

pub fn parse_dict_pairs(toks: Vec<Token>, pos: i64) -> (Vec<(Expr, Expr)>, i64) {
    // STMT:Let
    let mut out: Vec<(Expr, Expr)> = Vec::new();
    // STMT:Let
    let mut p: i64 = pos;
    // STMT:Let
    let mut done: bool = false;
    // STMT:Other
    while !done {
        // STMT:Let
        let t = tok_at(toks.clone(), p);
        // STMT:Expr
        if t == Token::RBrace {
            // STMT:Let
            done = true;
            // STMT:Let
            p = p + 1i64;
        } else { if t == Token::Eof {
            // STMT:Let
            done = true;
        } else {
            // STMT:Let
            let rk: (Expr, i64) = parse_logic(toks.clone(), p);
            // STMT:Let
            let k = rk.0;
            // STMT:Let
            let after_key = rk.1;
            // STMT:Let
            let rv: (Expr, i64) = parse_logic(toks.clone(), LzAdd::__add__(after_key, 1i64));
            // STMT:Let
            let v = rv.0;
            // STMT:Let
            let after_val = rv.1;
            // STMT:Let
            let pair: (Expr, Expr) = (k, v);
            // STMT:Let
            out = { let mut __cat = out.clone(); __cat.extend(vec![pair.clone()]); __cat };
            // STMT:Let
            let sep = tok_at(toks.clone(), after_val);
            // STMT:Expr
            if sep == Token::Comma {
                // STMT:Let
                p = LzAdd::__add__(after_val, 1i64);
            } else {
                // STMT:Let
                p = after_val;
            }
        }}
    }
    // STMT:Let
    let res: (Vec<(Expr, Expr)>, i64) = (out, p);
    // STMT:Expr
    return res;
}

pub fn parse_list_elems(toks: Vec<Token>, pos: i64) -> (Vec<Expr>, i64) {
    // STMT:Let
    let mut out: Vec<Expr> = Vec::new();
    // STMT:Let
    let mut p: i64 = pos;
    // STMT:Let
    let mut done: bool = false;
    // STMT:Other
    while !done {
        // STMT:Let
        let t = tok_at(toks.clone(), p);
        // STMT:Expr
        if t == Token::RBracket {
            // STMT:Let
            done = true;
            // STMT:Let
            p = p + 1i64;
        } else {
            // STMT:Let
            let r: (Expr, i64) = parse_logic(toks.clone(), p);
            // STMT:Let
            let rv = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            out = { let mut __cat = out.clone(); __cat.extend(vec![rv.clone()]); __cat };
            // STMT:Let
            let sep = tok_at(toks.clone(), rp);
            // STMT:Expr
            if sep == Token::Comma {
                // STMT:Let
                p = LzAdd::__add__(rp, 1i64);
            } else {
                // STMT:Let
                p = rp;
            }
        }
    }
    // STMT:Let
    let res: (Vec<Expr>, i64) = (out, p);
    // STMT:Expr
    return res;
}

pub fn parse_args(toks: Vec<Token>, pos: i64) -> (Vec<Expr>, i64) {
    // STMT:Let
    let mut out: Vec<Expr> = Vec::new();
    // STMT:Let
    let mut p: i64 = pos;
    // STMT:Let
    let mut done: bool = false;
    // STMT:Other
    while !done {
        // STMT:Let
        let t = tok_at(toks.clone(), p);
        // STMT:Expr
        if t == Token::RParen {
            // STMT:Let
            done = true;
            // STMT:Let
            p = p + 1i64;
        } else {
            // STMT:Let
            let r: (Expr, i64) = parse_logic(toks.clone(), p);
            // STMT:Let
            let rv = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            out = { let mut __cat = out.clone(); __cat.extend(vec![rv.clone()]); __cat };
            // STMT:Let
            let sep = tok_at(toks.clone(), rp);
            // STMT:Expr
            if sep == Token::Comma {
                // STMT:Let
                p = LzAdd::__add__(rp, 1i64);
            } else {
                // STMT:Let
                p = rp;
            }
        }
    }
    // STMT:Let
    let res: (Vec<Expr>, i64) = (out, p);
    // STMT:Expr
    return res;
}

pub fn parse_postfix(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    // STMT:Let
    let first: (Expr, i64) = parse_atom(toks.clone(), pos);
    // STMT:Let
    let mut value = first.0;
    // STMT:Let
    let mut p = first.1;
    // STMT:Let
    let mut done: bool = false;
    // STMT:Other
    while !done {
        // STMT:Let
        let t = tok_at(toks.clone(), p);
        // STMT:Expr
        if t == Token::LParen {
            // STMT:Let
            let r: (Vec<Expr>, i64) = parse_args(toks.clone(), LzAdd::__add__(p, 1i64));
            // STMT:Let
            let args_list = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            let ne = Expr::Call { callee: Box::new(value.clone()), args: Box::new(args_list) };
            // STMT:Let
            value = ne;
            // STMT:Let
            p = rp;
        } else { if t == Token::Dot {
            // STMT:Let
            let name_t = tok_at(toks.clone(), LzAdd::__add__(p, 1i64));
            // STMT:Let
            let nm: String = field_desc(name_t.clone());
            // STMT:Let
            let ne = Expr::Get { recv: Box::new(value.clone()), name: nm };
            // STMT:Let
            value = ne;
            // STMT:Let
            p = LzAdd::__add__(p, 2i64);
        } else { if t == Token::LBracket {
            // STMT:Let
            let r: (Expr, i64) = parse_logic(toks.clone(), LzAdd::__add__(p, 1i64));
            // STMT:Let
            let idx_expr = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            let ne = Expr::Index { recv: Box::new(value.clone()), idx: Box::new(idx_expr) };
            // STMT:Let
            value = ne;
            // STMT:Let
            p = (LzAdd::__add__(rp, 1i64));
        } else {
            // STMT:Let
            done = true;
        }}}
    }
    // STMT:Let
    let res: (Expr, i64) = (value, p);
    // STMT:Expr
    return res;
}

pub fn parse_term(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    // STMT:Let
    let first: (Expr, i64) = parse_postfix(toks.clone(), pos);
    // STMT:Let
    let mut value = first.0;
    // STMT:Let
    let mut p = first.1;
    // STMT:Let
    let mut done: bool = false;
    // STMT:Other
    while !done {
        // STMT:Let
        let t = tok_at(toks.clone(), p);
        // STMT:Expr
        if t == Token::Star {
            // STMT:Let
            let r: (Expr, i64) = parse_postfix(toks.clone(), LzAdd::__add__(p, 1i64));
            // STMT:Let
            let rv = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            let ne = Expr::Bin { op: "*".to_string(), l: Box::new(value.clone()), r: Box::new(rv) };
            // STMT:Let
            value = ne;
            // STMT:Let
            p = rp;
        } else { if t == Token::Slash {
            // STMT:Let
            let r: (Expr, i64) = parse_postfix(toks.clone(), LzAdd::__add__(p, 1i64));
            // STMT:Let
            let rv = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            let ne = Expr::Bin { op: "/".to_string(), l: Box::new(value.clone()), r: Box::new(rv) };
            // STMT:Let
            value = ne;
            // STMT:Let
            p = rp;
        } else {
            // STMT:Let
            done = true;
        }}
    }
    // STMT:Let
    let res: (Expr, i64) = (value, p);
    // STMT:Expr
    return res;
}

pub fn parse_expr(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    // STMT:Let
    let first: (Expr, i64) = parse_term(toks.clone(), pos);
    // STMT:Let
    let mut value = first.0;
    // STMT:Let
    let mut p = first.1;
    // STMT:Let
    let mut done: bool = false;
    // STMT:Other
    while !done {
        // STMT:Let
        let t = tok_at(toks.clone(), p);
        // STMT:Expr
        if t == Token::Plus {
            // STMT:Let
            let r: (Expr, i64) = parse_term(toks.clone(), LzAdd::__add__(p, 1i64));
            // STMT:Let
            let rv = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            let ne = Expr::Bin { op: "+".to_string(), l: Box::new(value.clone()), r: Box::new(rv) };
            // STMT:Let
            value = ne;
            // STMT:Let
            p = rp;
        } else { if t == Token::Minus {
            // STMT:Let
            let r: (Expr, i64) = parse_term(toks.clone(), LzAdd::__add__(p, 1i64));
            // STMT:Let
            let rv = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            let ne = Expr::Bin { op: "-".to_string(), l: Box::new(value.clone()), r: Box::new(rv) };
            // STMT:Let
            value = ne;
            // STMT:Let
            p = rp;
        } else {
            // STMT:Let
            done = true;
        }}
    }
    // STMT:Let
    let res: (Expr, i64) = (value, p);
    // STMT:Expr
    return res;
}

pub fn cmp_op(t: Token) -> (bool, String) {
    // STMT:Let
    let res: (bool, String) = {
// STMT:Other
match t.clone() {
    Token::EqEq => {
        // STMT:Expr
        (true, "==".to_string())
    }
    Token::Ne => {
        // STMT:Expr
        (true, "!=".to_string())
    }
    Token::Lt => {
        // STMT:Expr
        (true, "<".to_string())
    }
    Token::Le => {
        // STMT:Expr
        (true, "<=".to_string())
    }
    Token::Gt => {
        // STMT:Expr
        (true, ">".to_string())
    }
    Token::Ge => {
        // STMT:Expr
        (true, ">=".to_string())
    }
    _ => {
        // STMT:Expr
        (false, "?".to_string())
    }
}
    };
    // STMT:Expr
    return res;
}

pub fn logic_op(t: Token) -> (bool, String) {
    // STMT:Let
    let res: (bool, String) = {
// STMT:Other
match t.clone() {
    Token::AmpAmp => {
        // STMT:Expr
        (true, "&&".to_string())
    }
    Token::PipePipe => {
        // STMT:Expr
        (true, "||".to_string())
    }
    _ => {
        // STMT:Expr
        (false, "?".to_string())
    }
}
    };
    // STMT:Expr
    return res;
}

pub fn parse_cmp(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    // STMT:Let
    let first: (Expr, i64) = parse_expr(toks.clone(), pos);
    // STMT:Let
    let mut value = first.0;
    // STMT:Let
    let mut p = first.1;
    // STMT:Let
    let mut done: bool = false;
    // STMT:Other
    while !done {
        // STMT:Let
        let t = tok_at(toks.clone(), p);
        // STMT:Let
        let co: (bool, String) = cmp_op(t.clone());
        // STMT:Expr
        if co.0 {
            // STMT:Let
            let r: (Expr, i64) = parse_expr(toks.clone(), LzAdd::__add__(p, 1i64));
            // STMT:Let
            let rv = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            let ne = Expr::Cmp { op: co.1, l: Box::new(value.clone()), r: Box::new(rv) };
            // STMT:Let
            value = ne;
            // STMT:Let
            p = rp;
        } else {
            // STMT:Let
            done = true;
        }
    }
    // STMT:Let
    let res: (Expr, i64) = (value, p);
    // STMT:Expr
    return res;
}

pub fn parse_logic(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    // STMT:Let
    let first: (Expr, i64) = parse_cmp(toks.clone(), pos);
    // STMT:Let
    let mut value = first.0;
    // STMT:Let
    let mut p = first.1;
    // STMT:Let
    let mut done: bool = false;
    // STMT:Other
    while !done {
        // STMT:Let
        let t = tok_at(toks.clone(), p);
        // STMT:Let
        let lo: (bool, String) = logic_op(t.clone());
        // STMT:Expr
        if lo.0 {
            // STMT:Let
            let r: (Expr, i64) = parse_cmp(toks.clone(), LzAdd::__add__(p, 1i64));
            // STMT:Let
            let rv = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            let ne = Expr::Logic { op: lo.1, l: Box::new(value.clone()), r: Box::new(rv) };
            // STMT:Let
            value = ne;
            // STMT:Let
            p = rp;
        } else {
            // STMT:Let
            done = true;
        }
    }
    // STMT:Let
    let res: (Expr, i64) = (value, p);
    // STMT:Expr
    return res;
}

pub fn expr_list_summary(xs: Vec<Expr>) -> String {
    // STMT:Let
    let mut out: String = "".to_string();
    // STMT:For
    for idx in (0i64..(xs.len() as i64)).into_iter() {
        // STMT:Expr
        if idx > 0i64 {
            // STMT:Other
            out = out + &", ".to_string()[..];
        } else { ()};
        // STMT:Other
        out = LzAdd::__add__(out, display_expr(xs[((idx) as usize)].clone()));
    }
    // STMT:Expr
    return out;
}

pub fn expr_pair_summary(xs: Vec<(Expr, Expr)>) -> String {
    // STMT:Let
    let mut out: String = "".to_string();
    // STMT:For
    for idx in (0i64..(xs.len() as i64)).into_iter() {
        // STMT:Expr
        if idx > 0i64 {
            // STMT:Other
            out = out + &", ".to_string()[..];
        } else { ()};
        // STMT:Let
        let pair = xs[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(out, display_expr(pair.0.clone())) + &": ".to_string()[..], display_expr(pair.1.clone()));
    }
    // STMT:Expr
    return out;
}

pub fn display_expr(e: Expr) -> String {
    // STMT:Let
    let res: String = {
// STMT:Other
match e.clone() {
    Expr::IntLit { v: n } => {
        // STMT:Expr
        n.to_string()
    }
    Expr::StrLit { v: sv } => {
        // STMT:Expr
        "\"".to_string().to_string() + &sv[..] + &"\"".to_string()[..]
    }
    Expr::Ident { name: nm } => {
        // STMT:Expr
        nm
    }
    Expr::Bin { op: op, l: lv, r: rv } => {
        let lv = *lv;
        let rv = *rv;
        // STMT:Expr
        LzAdd::__add__(LzAdd::__add__(display_expr(lv.clone()), " ".to_string()) + &op[..] + &" ".to_string()[..], display_expr(rv.clone()))
    }
    Expr::Cmp { op: op, l: lv, r: rv } => {
        let lv = *lv;
        let rv = *rv;
        // STMT:Expr
        LzAdd::__add__(LzAdd::__add__(display_expr(lv.clone()), " ".to_string()) + &op[..] + &" ".to_string()[..], display_expr(rv.clone()))
    }
    Expr::Logic { op: op, l: lv, r: rv } => {
        let lv = *lv;
        let rv = *rv;
        // STMT:Expr
        LzAdd::__add__(LzAdd::__add__(display_expr(lv.clone()), " ".to_string()) + &op[..] + &" ".to_string()[..], display_expr(rv.clone()))
    }
    Expr::Call { callee: cv, args: av } => {
        let cv = *cv;
        let av = *av;
        // STMT:Expr
        LzAdd::__add__(LzAdd::__add__(display_expr(cv.clone()), "(".to_string()), expr_list_summary(av.clone())) + &")".to_string()[..]
    }
    Expr::Get { recv: rv, name: nm } => {
        let rv = *rv;
        // STMT:Expr
        LzAdd::__add__(display_expr(rv.clone()), ".".to_string()) + &nm[..]
    }
    Expr::Index { recv: rv, idx: iv } => {
        let rv = *rv;
        let iv = *iv;
        // STMT:Expr
        LzAdd::__add__(LzAdd::__add__(display_expr(rv.clone()), "[".to_string()), display_expr(iv.clone())) + &"]".to_string()[..]
    }
    Expr::ListLit { elems: ev } => {
        let ev = *ev;
        // STMT:Expr
        LzAdd::__add__("[".to_string(), expr_list_summary(ev.clone())) + &"]".to_string()[..]
    }
    Expr::TupleLit { elems: ev } => {
        let ev = *ev;
        // STMT:Expr
        LzAdd::__add__("(".to_string(), expr_list_summary(ev.clone())) + &")".to_string()[..]
    }
    Expr::DictLit { pairs: pv } => {
        let pv = *pv;
        // STMT:Expr
        LzAdd::__add__("{".to_string(), expr_pair_summary(pv.clone())) + &"}".to_string()[..]
    }
}
    };
    // STMT:Expr
    return res;
}

pub fn parse_stmt(toks: Vec<Token>, pos: i64, indent: i64) -> (String, i64) {
    // STMT:Let
    let t = tok_at(toks.clone(), pos);
    // STMT:Let
    let res: (String, i64) = {
// STMT:Other
match t.clone() {
    Token::Return => {
        // STMT:Let
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let rv = r.0;
        // STMT:Let
        let rp = r.1;
        // STMT:Let
        let rr: (String, i64) = (LzAdd::__add__("return ".to_string(), display_expr(rv.clone())), rp);
        // STMT:Expr
        rr
    }
    Token::If => {
        // STMT:Let
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let rv = r.0;
        // STMT:Let
        let rp = r.1;
        // STMT:Let
        let after = tok_at(toks.clone(), rp);
        // STMT:Let
        let npos: i64 = if after == Token::Colon { (LzAdd::__add__(rp, 1i64)) } else { rp };
        // STMT:Let
        let blk = parse_block(toks.clone(), npos, indent);
        // STMT:Let
        let body = blk.0;
        // STMT:Let
        let blk_end = blk.1;
        // STMT:Let
        let body_s = list_summary(body.clone());
        // STMT:Let
        let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__("if ".to_string(), display_expr(rv.clone())) + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
        // STMT:Expr
        rr
    }
    Token::Else => {
        // STMT:Let
        let rr: (String, i64) = ("else".to_string(), LzAdd::__add__(pos, 1i64));
        // STMT:Expr
        rr
    }
    Token::Match => {
        // STMT:Let
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let rv = r.0;
        // STMT:Let
        let rp = r.1;
        // STMT:Let
        let after = tok_at(toks.clone(), rp);
        // STMT:Let
        let npos: i64 = if after == Token::Colon { (LzAdd::__add__(rp, 1i64)) } else { rp };
        // STMT:Let
        let blk = parse_block(toks.clone(), npos, indent);
        // STMT:Let
        let body = blk.0;
        // STMT:Let
        let blk_end = blk.1;
        // STMT:Let
        let body_s = list_summary(body.clone());
        // STMT:Let
        let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__("match ".to_string(), display_expr(rv.clone())) + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
        // STMT:Expr
        rr
    }
    Token::Case => {
        // STMT:Let
        let pat_t = tok_at(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let pat_s = pattern_desc(pat_t.clone());
        // STMT:Let
        let arrow_t = tok_at(toks.clone(), LzAdd::__add__(pos, 2i64));
        // STMT:Expr
        if arrow_t == Token::FatArrow {
            // STMT:Let
            let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 3i64));
            // STMT:Let
            let rv = r.0;
            // STMT:Let
            let rp = r.1;
            // STMT:Let
            let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__("case ".to_string(), pat_s) + &" => ".to_string()[..], display_expr(rv.clone())), rp);
            // STMT:Expr
            rr
        } else {
            // STMT:Let
            let colon_t = tok_at(toks.clone(), LzAdd::__add__(pos, 2i64));
            // STMT:Let
            let npos: i64 = if colon_t == Token::Colon { (LzAdd::__add__(pos, 3i64)) } else { LzAdd::__add__(pos, 2i64) };
            // STMT:Let
            let blk = parse_block(toks.clone(), npos, indent);
            // STMT:Let
            let body = blk.0;
            // STMT:Let
            let blk_end = blk.1;
            // STMT:Let
            let body_s = list_summary(body.clone());
            // STMT:Let
            let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__("case ".to_string(), pat_s) + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
            // STMT:Expr
            rr
        }
    }
    Token::While => {
        // STMT:Let
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let rv = r.0;
        // STMT:Let
        let rp = r.1;
        // STMT:Let
        let after = tok_at(toks.clone(), rp);
        // STMT:Let
        let npos: i64 = if after == Token::Colon { (LzAdd::__add__(rp, 1i64)) } else { rp };
        // STMT:Let
        let blk = parse_block(toks.clone(), npos, indent);
        // STMT:Let
        let body = blk.0;
        // STMT:Let
        let blk_end = blk.1;
        // STMT:Let
        let body_s = list_summary(body.clone());
        // STMT:Let
        let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__("while ".to_string(), display_expr(rv.clone())) + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
        // STMT:Expr
        rr
    }
    Token::For => {
        // STMT:Let
        let name_t = tok_at(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let name_s = ident_name(name_t.clone());
        // STMT:Let
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 3i64));
        // STMT:Let
        let rv = r.0;
        // STMT:Let
        let rp = r.1;
        // STMT:Let
        let after = tok_at(toks.clone(), rp);
        // STMT:Let
        let npos: i64 = if after == Token::Colon { (LzAdd::__add__(rp, 1i64)) } else { rp };
        // STMT:Let
        let blk = parse_block(toks.clone(), npos, indent);
        // STMT:Let
        let body = blk.0;
        // STMT:Let
        let blk_end = blk.1;
        // STMT:Let
        let body_s = list_summary(body.clone());
        // STMT:Let
        let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("for ".to_string(), name_s) + &" in ".to_string()[..], display_expr(rv.clone())) + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
        // STMT:Expr
        rr
    }
    Token::Def => {
        // STMT:Let
        let name_t = tok_at(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let name_s = ident_name(name_t.clone());
        // STMT:Let
        let mut sig: String = LzAdd::__add__(name_s, "".to_string());
        // STMT:Let
        let mut ap: i64 = LzAdd::__add__(pos, 2i64);
        // STMT:Let
        let lp_t = tok_at(toks.clone(), ap);
        // STMT:Expr
        if lp_t == Token::LParen {
            // STMT:Let
            let pr = parse_params(toks.clone(), ap);
            // STMT:Let
            sig = LzAdd::__add__(sig, pr.0);
            // STMT:Let
            ap = pr.1;
        } else { ()};
        // STMT:Let
        let rf = ret_suffix(toks.clone(), ap);
        // STMT:Let
        sig = LzAdd::__add__(sig, rf.0);
        // STMT:Let
        ap = rf.1;
        // STMT:Let
        let colon_t = tok_at(toks.clone(), ap);
        // STMT:Let
        let npos: i64 = if colon_t == Token::Colon { (LzAdd::__add__(ap, 1i64)) } else { ap };
        // STMT:Let
        let blk = parse_block(toks.clone(), npos, indent);
        // STMT:Let
        let body = blk.0;
        // STMT:Let
        let blk_end = blk.1;
        // STMT:Let
        let body_s = list_summary(body.clone());
        // STMT:Let
        let rr: (String, i64) = (LzAdd::__add__("def ".to_string().to_string() + &sig[..] + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
        // STMT:Expr
        rr
    }
    Token::Let => {
        // STMT:Let
        let name_t = tok_at(toks.clone(), LzAdd::__add__(pos, 1i64));
        // STMT:Let
        let name_s = ident_name(name_t.clone());
        // STMT:Let
        let mut ap: i64 = LzAdd::__add__(pos, 2i64);
        // STMT:Let
        let mut ty_s: String = "".to_string();
        // STMT:Let
        let colon_t = tok_at(toks.clone(), ap);
        // STMT:Expr
        if colon_t == Token::Colon {
            // STMT:Let
            let ty_t = tok_at(toks.clone(), ap + 1i64);
            // STMT:Let
            ty_s = ident_name(ty_t.clone());
            // STMT:Let
            ap = ap + 2i64;
        } else { ()};
        // STMT:Let
        let eq_t = tok_at(toks.clone(), ap);
        // STMT:Let
        let rp: i64 = if eq_t == Token::Eq { (ap + 1i64) } else { ap };
        // STMT:Let
        let r = parse_logic(toks.clone(), rp);
        // STMT:Let
        let rv = r.0;
        // STMT:Let
        let rend = r.1;
        // STMT:Let
        let full: String = LzAdd::__add__(LzAdd::__add__(name_s, (if (ty_s.len() as i64) > 0i64 { " : ".to_string().to_string() + &ty_s[..] } else { "".to_string() })) + &" = ".to_string()[..], display_expr(rv.clone()));
        // STMT:Let
        let rr: (String, i64) = ("let ".to_string().to_string() + &full[..], rend);
        // STMT:Expr
        rr
    }
    Token::Newline => {
        // STMT:Let
        let rr: (String, i64) = ("".to_string(), LzAdd::__add__(pos, 1i64));
        // STMT:Expr
        rr
    }
    _ => {
        // STMT:Let
        let r = parse_logic(toks.clone(), pos);
        // STMT:Let
        let rv = r.0;
        // STMT:Let
        let rp = r.1;
        // STMT:Let
        let eq_t = tok_at(toks.clone(), rp);
        // STMT:Expr
        if eq_t == Token::Eq {
            // STMT:Let
            let r2 = parse_logic(toks.clone(), LzAdd::__add__(rp, 1i64));
            // STMT:Let
            let rv2 = r2.0;
            // STMT:Let
            let rend = r2.1;
            // STMT:Let
            let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__(display_expr(rv.clone()), " = ".to_string()), display_expr(rv2.clone())), rend);
            // STMT:Expr
            rr
        } else {
            // STMT:Let
            let rr: (String, i64) = (LzAdd::__add__("expr ".to_string(), display_expr(rv.clone())), rp);
            // STMT:Expr
            rr
        }
    }
}
    };
    // STMT:Expr
    return res;
}

pub fn ident_name(t: Token) -> String {
    // STMT:Let
    let res: String = {
// STMT:Other
match t.clone() {
    Token::Ident { name: nm } => {
        // STMT:Expr
        nm
    }
    _ => {
        // STMT:Expr
        "?".to_string()
    }
}
    };
    // STMT:Expr
    return res;
}

pub fn pattern_desc(t: Token) -> String {
    // STMT:Let
    let res: String = {
// STMT:Other
match t.clone() {
    Token::IntLit { v: n } => {
        // STMT:Expr
        n.to_string()
    }
    Token::Ident { name: nm } => {
        // STMT:Expr
        nm
    }
    _ => {
        // STMT:Expr
        "?".to_string()
    }
}
    };
    // STMT:Expr
    return res;
}

pub fn field_desc(t: Token) -> String {
    // STMT:Let
    let res: String = {
// STMT:Other
match t.clone() {
    Token::IntLit { v: n } => {
        // STMT:Expr
        n.to_string()
    }
    Token::Ident { name: nm } => {
        // STMT:Expr
        nm
    }
    _ => {
        // STMT:Expr
        "?".to_string()
    }
}
    };
    // STMT:Expr
    return res;
}

pub fn list_summary(xs: Vec<String>) -> String {
    // STMT:Let
    let mut out: String = "".to_string();
    // STMT:For
    for idx in (0i64..(xs.len() as i64)).into_iter() {
        // STMT:Expr
        if idx > 0i64 {
            // STMT:Other
            out = out + &", ".to_string()[..];
        } else { ()};
        // STMT:Other
        out = out + &xs[((idx) as usize)];
    }
    // STMT:Expr
    return out;
}

pub fn parse_params(toks: Vec<Token>, pos: i64) -> (String, i64) {
    // STMT:Let
    let mut out: Vec<String> = Vec::new();
    // STMT:Let
    let mut p: i64 = pos + 1i64;
    // STMT:Let
    let mut done: bool = false;
    // STMT:Other
    while !done {
        // STMT:Let
        let t = tok_at(toks.clone(), p);
        // STMT:Expr
        if t == Token::RParen {
            // STMT:Let
            done = true;
            // STMT:Let
            p = p + 1i64;
        } else { if t == Token::Eof {
            // STMT:Let
            done = true;
        } else {
            // STMT:Let
            let nm: String = ident_name(t.clone());
            // STMT:Let
            let colon_t = tok_at(toks.clone(), p + 1i64);
            // STMT:Expr
            if colon_t == Token::Colon {
                // STMT:Let
                let ty_t = tok_at(toks.clone(), p + 2i64);
                // STMT:Let
                let ty: String = ident_name(ty_t.clone());
                // STMT:Let
                let item: String = LzAdd::__add__(LzAdd::__add__(nm, ": ".to_string()), ty);
                // STMT:Let
                out = { let mut __cat = out.clone(); __cat.extend(vec![item.clone()]); __cat };
                // STMT:Let
                let sep = tok_at(toks.clone(), p + 3i64);
                // STMT:Expr
                if sep == Token::Comma {
                    // STMT:Let
                    p = p + 4i64;
                } else {
                    // STMT:Let
                    p = p + 3i64;
                }
            } else {
                // STMT:Let
                out = { let mut __cat = out.clone(); __cat.extend(vec![nm.clone()]); __cat };
                // STMT:Let
                let sep = tok_at(toks.clone(), p + 1i64);
                // STMT:Expr
                if sep == Token::Comma {
                    // STMT:Let
                    p = p + 2i64;
                } else {
                    // STMT:Let
                    p = p + 1i64;
                }
            }
        }}
    }
    // STMT:Let
    let params_s: String = "(".to_string().to_string() + &list_summary(out.clone())[..] + &")".to_string()[..];
    // STMT:Let
    let res: (String, i64) = (params_s, p);
    // STMT:Expr
    return res;
}

pub fn ret_suffix(toks: Vec<Token>, pos: i64) -> (String, i64) {
    // STMT:Let
    let t = tok_at(toks.clone(), pos);
    // STMT:Expr
    return if t == Token::Arrow {
        // STMT:Let
        let ty_t = tok_at(toks.clone(), pos + 1i64);
        // STMT:Let
        let ty: String = ident_name(ty_t.clone());
        // STMT:Let
        let res: (String, i64) = (LzAdd::__add__(" -> ".to_string(), ty), pos + 2i64);
        // STMT:Expr
        res
    } else {
        // STMT:Let
        let res: (String, i64) = ("".to_string(), pos);
        // STMT:Expr
        res
    };
}

pub fn parse_block(toks: Vec<Token>, pos: i64, indent: i64) -> (Vec<String>, i64) {
    // STMT:Let
    let mut out: Vec<String> = Vec::new();
    // STMT:Let
    let mut p: i64 = pos;
    // STMT:Let
    let mut done: bool = false;
    // STMT:Other
    while !done {
        // STMT:Let
        let t = tok_at(toks.clone(), p);
        // STMT:Let
        let tag: i64 = block_tag(t.clone());
        // STMT:Expr
        if tag == 0i64 {
            // STMT:Let
            done = true;
        } else { if tag == 1i64 {
            // STMT:Let
            p = p + 1i64;
        } else { if tag == 2i64 {
            // STMT:Let
            let n: i64 = block_indent(t.clone());
            // STMT:Expr
            if n > indent {
                // STMT:Let
                let r: (String, i64) = parse_stmt(toks.clone(), p + 1i64, n);
                // STMT:Let
                let s = r.0;
                // STMT:Expr
                if (s.len() as i64) > 0i64 {
                    // STMT:Let
                    out = { let mut __cat = out.clone(); __cat.extend(vec![s.clone()]); __cat };
                } else { ()};
                // STMT:Let
                p = r.1;
            } else {
                // STMT:Let
                done = true;
            }
        } else {
            // STMT:Let
            done = true;
        }}}
    }
    // STMT:Expr
    return (out, p);
}

pub fn block_tag(t: Token) -> i64 {
    // STMT:Let
    let res: i64 = {
// STMT:Other
match t.clone() {
    Token::Eof => {
        // STMT:Expr
        0i64
    }
    Token::Newline => {
        // STMT:Expr
        1i64
    }
    Token::Indent { v: n } => {
        // STMT:Expr
        2i64
    }
    _ => {
        // STMT:Expr
        0i64
    }
}
    };
    // STMT:Expr
    return res;
}

pub fn block_indent(t: Token) -> i64 {
    // STMT:Let
    let res: i64 = {
// STMT:Other
match t.clone() {
    Token::Indent { v: n } => {
        // STMT:Expr
        n
    }
    _ => {
        // STMT:Expr
        -1i64
    }
}
    };
    // STMT:Expr
    return res;
}

pub fn parse_program(toks: Vec<Token>, pos: i64) -> (Vec<String>, i64) {
    // STMT:Expr
    return parse_block(toks.clone(), pos, -1i64);
}

pub fn main() {
    // STMT:Let
    let src: String = "def lookup(key: str) -> int:\n    let m: str = {\"a\": 1, \"b\": 2}\n    return m[\"a\"]\nelse".to_string();
    // STMT:Let
    let toks: Vec<Token> = tokenize(src.clone());
    // STMT:Let
    let r: (Vec<String>, i64) = parse_program(toks.clone(), 0i64);
    // STMT:Let
    let mut stmts = r.0;
    // STMT:For
    for idx in (0i64..(stmts.len() as i64)).into_iter() {
        // STMT:Expr
        println!("{:?}", stmts[((idx) as usize)]);
    }
}

const __name__: &str = "main";

const __file__: &str = "F:\\AI\\lang-zone\\src\\frontend\\lz_parser.lz";

const __package__: &str = "frontend";

const __path__: &str = "F:\\AI\\lang-zone\\src\\frontend";

const __doc__: &str = "";

const __is_macro__: bool = false;


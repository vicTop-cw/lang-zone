
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
    IntLit(i64),
    StrLit(String),
    Ident(String),
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
    Indent(i64),
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    StrLit(String),
    Ident(String),
    Bin(String, Box<Expr>, Box<Expr>),
    Cmp(String, Box<Expr>, Box<Expr>),
    Logic(String, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Box<Vec<Expr>>),
    Get(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    ListLit(Box<Vec<Expr>>),
    TupleLit(Box<Vec<Expr>>),
    DictLit(Box<Vec<(Expr, Expr)>>),
}

pub fn char_at(s: String, idx: i64) -> String {
    return s[(idx as usize)..((idx + 1i64) as usize)].to_string();
}

pub fn str_len(s: String) -> i64 {
    return (s.len() as i64);
}

pub fn is_digit(c: String) -> bool {
    return c >= "0".to_string() && c <= "9".to_string();
}

pub fn is_alpha(c: String) -> bool {
    return (c >= "a".to_string() && c <= "z".to_string()) || (c >= "A".to_string() && c <= "Z".to_string()) || c == "_".to_string();
}

pub fn scan_int(src: String, start: i64) -> (i64, i64) {
    let mut i: i64 = start;
    while i < str_len(src.clone()) && is_digit(char_at(src.clone(), i)) {
        i = i + 1i64;
    }
    let v: i64 = (src[(start as usize)..(i as usize)].to_string()).parse::<i64>().unwrap();
    let res: (i64, i64) = (v, i);
    return res;
}

pub fn scan_indent(src: String, start: i64) -> (i64, i64) {
    let mut j: i64 = start;
    while j < str_len(src.clone()) && (char_at(src.clone(), j) == " ".to_string() || char_at(src.clone(), j) == "\t".to_string()) {
        j = j + 1i64;
    }
    return (j - start, j);
}

pub fn scan_ident(src: String, start: i64) -> (String, i64) {
    let mut i: i64 = start;
    while i < str_len(src.clone()) && (is_alpha(char_at(src.clone(), i)) || is_digit(char_at(src.clone(), i))) {
        i = i + 1i64;
    }
    return (src[(start as usize)..(i as usize)].to_string(), i);
}

pub fn scan_string(src: String, start: i64) -> (String, i64) {
    let mut i: i64 = start + 1i64;
    while i < str_len(src.clone()) && char_at(src.clone(), i) != "\"".to_string() {
        i = i + 1i64;
    }
    let res: (String, i64) = (src[((start + 1i64) as usize)..(i as usize)].to_string(), i + 1i64);
    return res;
}

pub fn keyword_token(w: String) -> (bool, Token) {
    let table: Vec<(String, Token)> = vec![("return".to_string(), Token::Return), ("if".to_string(), Token::If), ("else".to_string(), Token::Else), ("def".to_string(), Token::Def), ("let".to_string(), Token::Let), ("match".to_string(), Token::Match), ("case".to_string(), Token::Case), ("while".to_string(), Token::While), ("for".to_string(), Token::For), ("in".to_string(), Token::In)];
    let mut result: (bool, Token) = (false, Token::Ident(w.clone()));
    for idx in (0i64..(table.len() as i64)).into_iter() {
        let pair = table[((idx) as usize)].clone();
        if pair.0 == w {
            result = (true, pair.1);
        } else { ()};
    }
    return result;
}

pub fn op_token(c: String) -> (bool, Token) {
    let table: Vec<(String, Token)> = vec![("+".to_string(), Token::Plus), ("-".to_string(), Token::Minus), ("*".to_string(), Token::Star), ("/".to_string(), Token::Slash), ("(".to_string(), Token::LParen), (")".to_string(), Token::RParen), ("[".to_string(), Token::LBracket), ("]".to_string(), Token::RBracket), ("{".to_string(), Token::LBrace), ("}".to_string(), Token::RBrace), ("<".to_string(), Token::Lt), (">".to_string(), Token::Gt), ("=".to_string(), Token::Eq), (".".to_string(), Token::Dot)];
    let mut result: (bool, Token) = (false, Token::Plus);
    for idx in (0i64..(table.len() as i64)).into_iter() {
        let pair = table[((idx) as usize)].clone();
        if pair.0 == c {
            result = (true, pair.1);
        } else { ()};
    }
    return result;
}

pub fn two_char_op(c1: String, c2: String) -> (bool, Token) {
    let key: String = c1 + &c2[..];
    let table: Vec<(String, Token)> = vec![("==".to_string(), Token::EqEq), ("!=".to_string(), Token::Ne), ("<=".to_string(), Token::Le), (">=".to_string(), Token::Ge), ("&&".to_string(), Token::AmpAmp), ("||".to_string(), Token::PipePipe), ("=>".to_string(), Token::FatArrow)];
    let mut result: (bool, Token) = (false, Token::Plus);
    for idx in (0i64..(table.len() as i64)).into_iter() {
        let pair = table[((idx) as usize)].clone();
        if pair.0 == key {
            result = (true, pair.1);
        } else { ()};
    }
    return result;
}

pub fn tokenize(src: String) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut i: i64 = 0i64;
    let mut line_start = true;
    while i < str_len(src.clone()) {
        let c: String = char_at(src.clone(), i);
        if c == "\n".to_string() {
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Newline]); __lz_cat };
            i = i + 1i64;
            line_start = true;
        } else { if line_start && c == " ".to_string() {
            let r: (i64, i64) = scan_indent(src.clone(), i);
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Indent(r.0)]); __lz_cat };
            i = r.1;
            line_start = false;
        } else { if line_start {
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Indent(0i64)]); __lz_cat };
            line_start = false;
        } else { if c == " ".to_string() {
            i = i + 1i64;
        } else { if c == "\"".to_string() {
            let r: (String, i64) = scan_string(src.clone(), i);
            i = r.1;
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::StrLit(r.0)]); __lz_cat };
        } else { if is_digit(c.clone()) {
            let r: (i64, i64) = scan_int(src.clone(), i);
            i = r.1;
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::IntLit(r.0)]); __lz_cat };
        } else { if is_alpha(c.clone()) {
            let r: (String, i64) = scan_ident(src.clone(), i);
            i = r.1;
            let kw: (bool, Token) = keyword_token(r.0.clone());
            if kw.0 {
                tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![kw.1]); __lz_cat };
            } else {
                tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Ident(r.0)]); __lz_cat };
            }
        } else { if c == ":".to_string() {
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Colon]); __lz_cat };
            i = i + 1i64;
        } else { if c == ",".to_string() {
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Comma]); __lz_cat };
            i = i + 1i64;
        } else { if c == "-".to_string() && i + 1i64 < str_len(src.clone()) && char_at(src.clone(), i + 1i64) == ">".to_string() {
            tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Arrow]); __lz_cat };
            i = i + 2i64;
        } else {
            let mut handled = false;
            if i + 1i64 < str_len(src.clone()) {
                let r2: (bool, Token) = two_char_op(c.clone(), char_at(src.clone(), i + 1i64));
                if r2.0 {
                    tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![r2.1]); __lz_cat };
                    i = i + 2i64;
                    handled = true;
                } else { ()}
            } else { ()};
            if !(handled) {
                let r: (bool, Token) = op_token(c.clone());
                let found = r.0;
                let tok = r.1;
                if found {
                    tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![tok.clone()]); __lz_cat };
                    i = i + 1i64;
                } else {
                    i = i + 1i64;
                }
            } else { ()}
        }}}}}}}}}}
    }
    tokens = { let mut __lz_cat = tokens; __lz_cat.extend(vec![Token::Eof]); __lz_cat };
    return tokens;
}

pub fn tok_at(toks: Vec<Token>, idx: i64) -> Token {
    return if idx >= (toks.len() as i64) { Token::Eof } else { toks[((idx) as usize)].clone() };
}

pub fn parse_atom(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    let t = tok_at(toks.clone(), pos);
    let res: (Expr, i64) = {
match t.clone() {
    Token::IntLit(n) => {
        (Expr::IntLit(n), pos + 1i64)
    }
    Token::StrLit(sv) => {
        (Expr::StrLit(sv), pos + 1i64)
    }
    Token::Ident(nm) => {
        (Expr::Ident(nm), pos + 1i64)
    }
    Token::LParen => {
        let r = parse_logic(toks.clone(), pos + 1i64);
        let rv = r.0;
        let rp = r.1;
        let sep = tok_at(toks.clone(), rp);
        if sep == Token::Comma {
            let mut elems: Vec<Expr> = vec![rv.clone()];
            let mut p: i64 = rp + 1i64;
            let mut done = false;
            while !done {
                let t2 = tok_at(toks.clone(), p);
                if t2 == Token::RParen {
                    done = true;
                    p = p + 1i64;
                } else {
                    let r2 = parse_logic(toks.clone(), p);
                    let rv2 = r2.0;
                    let rp2 = r2.1;
                    elems = { let mut __lz_cat = elems; __lz_cat.extend(vec![rv2.clone()]); __lz_cat };
                    let sep2 = tok_at(toks.clone(), rp2);
                    if sep2 == Token::Comma {
                        p = rp2 + 1i64;
                    } else {
                        p = rp2;
                    }
                }
            }
            let pr: (Expr, i64) = (Expr::TupleLit(Box::new(elems)), p);
            pr
        } else {
            let pr: (Expr, i64) = (rv, rp + 1i64);
            pr
        }
    }
    Token::LBracket => {
        let r = parse_list_elems(toks.clone(), pos + 1i64);
        let elems = r.0;
        let rp = r.1;
        let pr: (Expr, i64) = (Expr::ListLit(Box::new(elems)), rp + 1i64);
        pr
    }
    Token::LBrace => {
        let r = parse_dict_pairs(toks.clone(), pos + 1i64);
        let pairs = r.0;
        let rp = r.1;
        let pr: (Expr, i64) = (Expr::DictLit(Box::new(pairs)), rp + 1i64);
        pr
    }
    _ => {
        (Expr::Ident("?".to_string()), pos + 1i64)
    }
}
    };
    return res;
}

pub fn parse_dict_pairs(toks: Vec<Token>, pos: i64) -> (Vec<(Expr, Expr)>, i64) {
    let mut out: Vec<(Expr, Expr)> = Vec::new();
    let mut p: i64 = pos;
    let mut done = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::RBrace {
            done = true;
            p = p + 1i64;
        } else { if t == Token::Eof {
            done = true;
        } else {
            let rk: (Expr, i64) = parse_logic(toks.clone(), p);
            let k = rk.0;
            let after_key = rk.1;
            let rv: (Expr, i64) = parse_logic(toks.clone(), after_key + 1i64);
            let v = rv.0;
            let after_val = rv.1;
            let pair: (Expr, Expr) = (k, v);
            out = { let mut __lz_cat = out; __lz_cat.extend(vec![pair.clone()]); __lz_cat };
            let sep = tok_at(toks.clone(), after_val);
            if sep == Token::Comma {
                p = after_val + 1i64;
            } else {
                p = after_val;
            }
        }}
    }
    let res: (Vec<(Expr, Expr)>, i64) = (out, p);
    return res;
}

pub fn parse_list_elems(toks: Vec<Token>, pos: i64) -> (Vec<Expr>, i64) {
    let mut out: Vec<Expr> = Vec::new();
    let mut p: i64 = pos;
    let mut done = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::RBracket {
            done = true;
            p = p + 1i64;
        } else {
            let r: (Expr, i64) = parse_logic(toks.clone(), p);
            let rv = r.0;
            let rp = r.1;
            out = { let mut __lz_cat = out; __lz_cat.extend(vec![rv.clone()]); __lz_cat };
            let sep = tok_at(toks.clone(), rp);
            if sep == Token::Comma {
                p = rp + 1i64;
            } else {
                p = rp;
            }
        }
    }
    let res: (Vec<Expr>, i64) = (out, p);
    return res;
}

pub fn parse_args(toks: Vec<Token>, pos: i64) -> (Vec<Expr>, i64) {
    let mut out: Vec<Expr> = Vec::new();
    let mut p: i64 = pos;
    let mut done = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::RParen {
            done = true;
            p = p + 1i64;
        } else {
            let r: (Expr, i64) = parse_logic(toks.clone(), p);
            let rv = r.0;
            let rp = r.1;
            out = { let mut __lz_cat = out; __lz_cat.extend(vec![rv.clone()]); __lz_cat };
            let sep = tok_at(toks.clone(), rp);
            if sep == Token::Comma {
                p = rp + 1i64;
            } else {
                p = rp;
            }
        }
    }
    let res: (Vec<Expr>, i64) = (out, p);
    return res;
}

pub fn parse_postfix(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    let first: (Expr, i64) = parse_atom(toks.clone(), pos);
    let mut value = first.0;
    let mut p = first.1;
    let mut done = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::LParen {
            let r: (Vec<Expr>, i64) = parse_args(toks.clone(), p + 1i64);
            let args_list = r.0;
            let rp = r.1;
            let ne = Expr::Call(Box::new(value.clone()), Box::new(args_list));
            value = ne;
            p = rp;
        } else { if t == Token::Dot {
            let name_t = tok_at(toks.clone(), p + 1i64);
            let nm: String = field_desc(name_t.clone());
            let ne = Expr::Get(Box::new(value.clone()), nm);
            value = ne;
            p = p + 2i64;
        } else { if t == Token::LBracket {
            let r: (Expr, i64) = parse_logic(toks.clone(), p + 1i64);
            let idx_expr = r.0;
            let rp = r.1;
            let ne = Expr::Index(Box::new(value.clone()), Box::new(idx_expr));
            value = ne;
            p = (rp + 1i64);
        } else {
            done = true;
        }}}
    }
    let res: (Expr, i64) = (value, p);
    return res;
}

pub fn parse_term(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    let first: (Expr, i64) = parse_postfix(toks.clone(), pos);
    let mut value = first.0;
    let mut p = first.1;
    let mut done = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::Star {
            let r: (Expr, i64) = parse_postfix(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Bin("*".to_string(), Box::new(value.clone()), Box::new(rv));
            value = ne;
            p = rp;
        } else { if t == Token::Slash {
            let r: (Expr, i64) = parse_postfix(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Bin("/".to_string(), Box::new(value.clone()), Box::new(rv));
            value = ne;
            p = rp;
        } else {
            done = true;
        }}
    }
    let res: (Expr, i64) = (value, p);
    return res;
}

pub fn parse_expr(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    let first: (Expr, i64) = parse_term(toks.clone(), pos);
    let mut value = first.0;
    let mut p = first.1;
    let mut done = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::Plus {
            let r: (Expr, i64) = parse_term(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Bin("+".to_string(), Box::new(value.clone()), Box::new(rv));
            value = ne;
            p = rp;
        } else { if t == Token::Minus {
            let r: (Expr, i64) = parse_term(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Bin("-".to_string(), Box::new(value.clone()), Box::new(rv));
            value = ne;
            p = rp;
        } else {
            done = true;
        }}
    }
    let res: (Expr, i64) = (value, p);
    return res;
}

pub fn cmp_op(t: Token) -> (bool, String) {
    let res: (bool, String) = {
match t.clone() {
    Token::EqEq => {
        (true, "==".to_string())
    }
    Token::Ne => {
        (true, "!=".to_string())
    }
    Token::Lt => {
        (true, "<".to_string())
    }
    Token::Le => {
        (true, "<=".to_string())
    }
    Token::Gt => {
        (true, ">".to_string())
    }
    Token::Ge => {
        (true, ">=".to_string())
    }
    _ => {
        (false, "?".to_string())
    }
}
    };
    return res;
}

pub fn logic_op(t: Token) -> (bool, String) {
    let res: (bool, String) = {
match t.clone() {
    Token::AmpAmp => {
        (true, "&&".to_string())
    }
    Token::PipePipe => {
        (true, "||".to_string())
    }
    _ => {
        (false, "?".to_string())
    }
}
    };
    return res;
}

pub fn parse_cmp(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    let first: (Expr, i64) = parse_expr(toks.clone(), pos);
    let mut value = first.0;
    let mut p = first.1;
    let mut done = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        let co: (bool, String) = cmp_op(t.clone());
        if co.0 {
            let r: (Expr, i64) = parse_expr(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Cmp(co.1, Box::new(value.clone()), Box::new(rv));
            value = ne;
            p = rp;
        } else {
            done = true;
        }
    }
    let res: (Expr, i64) = (value, p);
    return res;
}

pub fn parse_logic(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    let first: (Expr, i64) = parse_cmp(toks.clone(), pos);
    let mut value = first.0;
    let mut p = first.1;
    let mut done = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        let lo: (bool, String) = logic_op(t.clone());
        if lo.0 {
            let r: (Expr, i64) = parse_cmp(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Logic(lo.1, Box::new(value.clone()), Box::new(rv));
            value = ne;
            p = rp;
        } else {
            done = true;
        }
    }
    let res: (Expr, i64) = (value, p);
    return res;
}

pub fn expr_list_summary(xs: Vec<Expr>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(xs.len() as i64)).into_iter() {
        if idx > 0i64 {
            out = out + &", ".to_string()[..];
        } else { ()};
        out = out + &display_expr(xs[((idx) as usize)].clone())[..];
    }
    return out;
}

pub fn expr_pair_summary(xs: Vec<(Expr, Expr)>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(xs.len() as i64)).into_iter() {
        if idx > 0i64 {
            out = out + &", ".to_string()[..];
        } else { ()};
        let pair = xs[((idx) as usize)].clone();
        out = out + &display_expr(pair.0.clone())[..] + &": ".to_string()[..] + &display_expr(pair.1.clone())[..];
    }
    return out;
}

pub fn display_expr(e: Expr) -> String {
    let res: String = {
match e.clone() {
    Expr::IntLit(n) => {
        n.to_string()
    }
    Expr::StrLit(sv) => {
        "\"".to_string().to_string() + &sv[..] + &"\"".to_string()[..]
    }
    Expr::Ident(nm) => {
        nm
    }
    Expr::Bin(op, lv, rv) => {
        let lv = *lv;
        let rv = *rv;
        display_expr(lv.clone()) + &" ".to_string()[..] + &op[..] + &" ".to_string()[..] + &display_expr(rv.clone())[..]
    }
    Expr::Cmp(op, lv, rv) => {
        let lv = *lv;
        let rv = *rv;
        display_expr(lv.clone()) + &" ".to_string()[..] + &op[..] + &" ".to_string()[..] + &display_expr(rv.clone())[..]
    }
    Expr::Logic(op, lv, rv) => {
        let lv = *lv;
        let rv = *rv;
        display_expr(lv.clone()) + &" ".to_string()[..] + &op[..] + &" ".to_string()[..] + &display_expr(rv.clone())[..]
    }
    Expr::Call(cv, av) => {
        let cv = *cv;
        let av = *av;
        display_expr(cv.clone()) + &"(".to_string()[..] + &expr_list_summary(av.clone())[..] + &")".to_string()[..]
    }
    Expr::Get(rv, nm) => {
        let rv = *rv;
        display_expr(rv.clone()) + &".".to_string()[..] + &nm[..]
    }
    Expr::Index(rv, iv) => {
        let rv = *rv;
        let iv = *iv;
        display_expr(rv.clone()) + &"[".to_string()[..] + &display_expr(iv.clone())[..] + &"]".to_string()[..]
    }
    Expr::ListLit(ev) => {
        let ev = *ev;
        "[".to_string().to_string() + &expr_list_summary(ev.clone())[..] + &"]".to_string()[..]
    }
    Expr::TupleLit(ev) => {
        let ev = *ev;
        "(".to_string().to_string() + &expr_list_summary(ev.clone())[..] + &")".to_string()[..]
    }
    Expr::DictLit(pv) => {
        let pv = *pv;
        "{".to_string().to_string() + &expr_pair_summary(pv.clone())[..] + &"}".to_string()[..]
    }
}
    };
    return res;
}

pub fn parse_stmt(toks: Vec<Token>, pos: i64, indent: i64) -> (String, i64) {
    let t = tok_at(toks.clone(), pos);
    let res: (String, i64) = {
match t.clone() {
    Token::Return => {
        let r = parse_logic(toks.clone(), pos + 1i64);
        let rv = r.0;
        let rp = r.1;
        let rr: (String, i64) = ("return ".to_string().to_string() + &display_expr(rv.clone())[..], rp);
        rr
    }
    Token::If => {
        let r = parse_logic(toks.clone(), pos + 1i64);
        let rv = r.0;
        let rp = r.1;
        let after = tok_at(toks.clone(), rp);
        let npos: i64 = if after == Token::Colon { (rp + 1i64) } else { rp };
        let blk = parse_block(toks.clone(), npos, indent);
        let body = blk.0;
        let blk_end = blk.1;
        let body_s = list_summary(body.clone());
        let rr: (String, i64) = ("if ".to_string().to_string() + &display_expr(rv.clone())[..] + &" {".to_string()[..] + &body_s[..] + &"}".to_string()[..], blk_end);
        rr
    }
    Token::Else => {
        let rr: (String, i64) = ("else".to_string(), pos + 1i64);
        rr
    }
    Token::Match => {
        let r = parse_logic(toks.clone(), pos + 1i64);
        let rv = r.0;
        let rp = r.1;
        let after = tok_at(toks.clone(), rp);
        let npos: i64 = if after == Token::Colon { (rp + 1i64) } else { rp };
        let blk = parse_block(toks.clone(), npos, indent);
        let body = blk.0;
        let blk_end = blk.1;
        let body_s = list_summary(body.clone());
        let rr: (String, i64) = ("match ".to_string().to_string() + &display_expr(rv.clone())[..] + &" {".to_string()[..] + &body_s[..] + &"}".to_string()[..], blk_end);
        rr
    }
    Token::Case => {
        let pat_t = tok_at(toks.clone(), pos + 1i64);
        let pat_s = pattern_desc(pat_t.clone());
        let arrow_t = tok_at(toks.clone(), pos + 2i64);
        if arrow_t == Token::FatArrow {
            let r = parse_logic(toks.clone(), pos + 3i64);
            let rv = r.0;
            let rp = r.1;
            let rr: (String, i64) = ("case ".to_string().to_string() + &pat_s[..] + &" => ".to_string()[..] + &display_expr(rv.clone())[..], rp);
            rr
        } else {
            let colon_t = tok_at(toks.clone(), pos + 2i64);
            let npos: i64 = if colon_t == Token::Colon { (pos + 3i64) } else { pos + 2i64 };
            let blk = parse_block(toks.clone(), npos, indent);
            let body = blk.0;
            let blk_end = blk.1;
            let body_s = list_summary(body.clone());
            let rr: (String, i64) = ("case ".to_string().to_string() + &pat_s[..] + &" {".to_string()[..] + &body_s[..] + &"}".to_string()[..], blk_end);
            rr
        }
    }
    Token::While => {
        let r = parse_logic(toks.clone(), pos + 1i64);
        let rv = r.0;
        let rp = r.1;
        let after = tok_at(toks.clone(), rp);
        let npos: i64 = if after == Token::Colon { (rp + 1i64) } else { rp };
        let blk = parse_block(toks.clone(), npos, indent);
        let body = blk.0;
        let blk_end = blk.1;
        let body_s = list_summary(body.clone());
        let rr: (String, i64) = ("while ".to_string().to_string() + &display_expr(rv.clone())[..] + &" {".to_string()[..] + &body_s[..] + &"}".to_string()[..], blk_end);
        rr
    }
    Token::For => {
        let name_t = tok_at(toks.clone(), pos + 1i64);
        let name_s = ident_name(name_t.clone());
        let r = parse_logic(toks.clone(), pos + 3i64);
        let rv = r.0;
        let rp = r.1;
        let after = tok_at(toks.clone(), rp);
        let npos: i64 = if after == Token::Colon { (rp + 1i64) } else { rp };
        let blk = parse_block(toks.clone(), npos, indent);
        let body = blk.0;
        let blk_end = blk.1;
        let body_s = list_summary(body.clone());
        let rr: (String, i64) = ("for ".to_string().to_string() + &name_s[..] + &" in ".to_string()[..] + &display_expr(rv.clone())[..] + &" {".to_string()[..] + &body_s[..] + &"}".to_string()[..], blk_end);
        rr
    }
    Token::Def => {
        let name_t = tok_at(toks.clone(), pos + 1i64);
        let name_s = ident_name(name_t.clone());
        let mut sig: String = name_s + &"".to_string()[..];
        let mut ap: i64 = pos + 2i64;
        let lp_t = tok_at(toks.clone(), ap);
        if lp_t == Token::LParen {
            let pr = parse_params(toks.clone(), ap);
            sig = sig + &pr.0;
            ap = pr.1;
        } else { ()};
        let rf = ret_suffix(toks.clone(), ap);
        sig = sig + &rf.0;
        ap = rf.1;
        let colon_t = tok_at(toks.clone(), ap);
        let npos: i64 = if colon_t == Token::Colon { (ap + 1i64) } else { ap };
        let blk = parse_block(toks.clone(), npos, indent);
        let body = blk.0;
        let blk_end = blk.1;
        let body_s = list_summary(body.clone());
        let rr: (String, i64) = ("def ".to_string().to_string() + &sig[..] + &" {".to_string()[..] + &body_s[..] + &"}".to_string()[..], blk_end);
        rr
    }
    Token::Let => {
        let name_t = tok_at(toks.clone(), pos + 1i64);
        let name_s = ident_name(name_t.clone());
        let mut ap: i64 = pos + 2i64;
        let mut ty_s: String = "".to_string();
        let colon_t = tok_at(toks.clone(), ap);
        if colon_t == Token::Colon {
            let ty_t = tok_at(toks.clone(), ap + 1i64);
            ty_s = ident_name(ty_t.clone());
            ap = ap + 2i64;
        } else { ()};
        let eq_t = tok_at(toks.clone(), ap);
        let rp: i64 = if eq_t == Token::Eq { (ap + 1i64) } else { ap };
        let r = parse_logic(toks.clone(), rp);
        let rv = r.0;
        let rend = r.1;
        let full: String = name_s + &(if (ty_s.len() as i64) > 0i64 { " : ".to_string().to_string() + &ty_s[..] } else { "".to_string() }) + &" = ".to_string()[..] + &display_expr(rv.clone())[..];
        let rr: (String, i64) = ("let ".to_string().to_string() + &full[..], rend);
        rr
    }
    Token::Newline => {
        let rr: (String, i64) = ("".to_string(), pos + 1i64);
        rr
    }
    _ => {
        let r = parse_logic(toks.clone(), pos);
        let rv = r.0;
        let rp = r.1;
        let eq_t = tok_at(toks.clone(), rp);
        if eq_t == Token::Eq {
            let r2 = parse_logic(toks.clone(), rp + 1i64);
            let rv2 = r2.0;
            let rend = r2.1;
            let rr: (String, i64) = (display_expr(rv.clone()) + &" = ".to_string()[..] + &display_expr(rv2.clone())[..], rend);
            rr
        } else {
            let rr: (String, i64) = ("expr ".to_string().to_string() + &display_expr(rv.clone())[..], rp);
            rr
        }
    }
}
    };
    return res;
}

pub fn ident_name(t: Token) -> String {
    let res: String = {
match t.clone() {
    Token::Ident(nm) => {
        nm
    }
    _ => {
        "?".to_string()
    }
}
    };
    return res;
}

pub fn pattern_desc(t: Token) -> String {
    let res: String = {
match t.clone() {
    Token::IntLit(n) => {
        n.to_string()
    }
    Token::Ident(nm) => {
        nm
    }
    _ => {
        "?".to_string()
    }
}
    };
    return res;
}

pub fn field_desc(t: Token) -> String {
    let res: String = {
match t.clone() {
    Token::IntLit(n) => {
        n.to_string()
    }
    Token::Ident(nm) => {
        nm
    }
    _ => {
        "?".to_string()
    }
}
    };
    return res;
}

pub fn list_summary(xs: Vec<String>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(xs.len() as i64)).into_iter() {
        if idx > 0i64 {
            out = out + &", ".to_string()[..];
        } else { ()};
        out = out + &xs[((idx) as usize)];
    }
    return out;
}

pub fn parse_params(toks: Vec<Token>, pos: i64) -> (String, i64) {
    let mut out: Vec<String> = Vec::new();
    let mut p: i64 = pos + 1i64;
    let mut done = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::RParen {
            done = true;
            p = p + 1i64;
        } else { if t == Token::Eof {
            done = true;
        } else {
            let nm: String = ident_name(t.clone());
            let colon_t = tok_at(toks.clone(), p + 1i64);
            if colon_t == Token::Colon {
                let ty_t = tok_at(toks.clone(), p + 2i64);
                let ty: String = ident_name(ty_t.clone());
                let item: String = nm + &": ".to_string()[..] + &ty[..];
                out = { let mut __lz_cat = out; __lz_cat.extend(vec![item.clone()]); __lz_cat };
                let sep = tok_at(toks.clone(), p + 3i64);
                if sep == Token::Comma {
                    p = p + 4i64;
                } else {
                    p = p + 3i64;
                }
            } else {
                out = { let mut __lz_cat = out; __lz_cat.extend(vec![nm.clone()]); __lz_cat };
                let sep = tok_at(toks.clone(), p + 1i64);
                if sep == Token::Comma {
                    p = p + 2i64;
                } else {
                    p = p + 1i64;
                }
            }
        }}
    }
    let params_s: String = "(".to_string().to_string() + &list_summary(out.clone())[..] + &")".to_string()[..];
    let res: (String, i64) = (params_s, p);
    return res;
}

pub fn ret_suffix(toks: Vec<Token>, pos: i64) -> (String, i64) {
    let t = tok_at(toks.clone(), pos);
    return if t == Token::Arrow {
        let ty_t = tok_at(toks.clone(), pos + 1i64);
        let ty: String = ident_name(ty_t.clone());
        let res: (String, i64) = (" -> ".to_string().to_string() + &ty[..], pos + 2i64);
        res
    } else {
        let res: (String, i64) = ("".to_string(), pos);
        res
    };
}

pub fn parse_block(toks: Vec<Token>, pos: i64, indent: i64) -> (Vec<String>, i64) {
    let mut out: Vec<String> = Vec::new();
    let mut p: i64 = pos;
    let mut done = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        let tag: i64 = block_tag(t.clone());
        if tag == 0i64 {
            done = true;
        } else { if tag == 1i64 {
            p = p + 1i64;
        } else { if tag == 2i64 {
            let n: i64 = block_indent(t.clone());
            if n > indent {
                let r: (String, i64) = parse_stmt(toks.clone(), p + 1i64, n);
                let s = r.0;
                if (s.len() as i64) > 0i64 {
                    out = { let mut __lz_cat = out; __lz_cat.extend(vec![s.clone()]); __lz_cat };
                } else { ()};
                p = r.1;
            } else {
                done = true;
            }
        } else {
            done = true;
        }}}
    }
    return (out, p);
}

pub fn block_tag(t: Token) -> i64 {
    let res: i64 = {
match t.clone() {
    Token::Eof => {
        0i64
    }
    Token::Newline => {
        1i64
    }
    Token::Indent(n) => {
        2i64
    }
    _ => {
        0i64
    }
}
    };
    return res;
}

pub fn block_indent(t: Token) -> i64 {
    let res: i64 = {
match t.clone() {
    Token::Indent(n) => {
        n
    }
    _ => {
        -1i64
    }
}
    };
    return res;
}

pub fn parse_program(toks: Vec<Token>, pos: i64) -> (Vec<String>, i64) {
    return parse_block(toks.clone(), pos, -1i64);
}

pub fn main() {
    let src: String = "def lookup(key: str) -> int:\n    let m: str = {\"a\": 1, \"b\": 2}\n    return m[\"a\"]\nelse".to_string();
    let toks: Vec<Token> = tokenize(src.clone());
    let r: (Vec<String>, i64) = parse_program(toks.clone(), 0i64);
    let stmts = r.0;
    for idx in (0i64..(stmts.len() as i64)).into_iter() {
        println!("{:?}", stmts[((idx) as usize)]);
    }
}

const __name__: &str = "main";

const __file__: &str = "src\\frontend\\lz_parser.lz";

const __package__: &str = "frontend";

const __path__: &str = "src\\frontend";

const __doc__: &str = "";

const __is_macro__: bool = false;


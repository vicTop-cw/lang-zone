
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
    return s[((idx) as usize)..(((idx + 1i64)) as usize)].to_string();
}

pub fn str_len(mut s: String) -> i64 {
    return (s.len() as i64);
}

pub fn is_digit(c: String) -> bool {
    return (c).to_string() >= ("0".to_string()).to_string() && (c).to_string() <= ("9".to_string()).to_string();
}

pub fn is_alpha(c: String) -> bool {
    return ((c).to_string() >= ("a".to_string()).to_string() && (c).to_string() <= ("z".to_string()).to_string()) || ((c).to_string() >= ("A".to_string()).to_string() && (c).to_string() <= ("Z".to_string()).to_string()) || (c).to_string() == ("_".to_string()).to_string();
}

pub fn scan_int(src: String, start: i64) -> (i64, i64) {
    let mut i: i64 = start;
    while i < str_len(src.clone()) && is_digit(char_at(src.clone(), i)) {
        i += 1i64;
    }
    let v: i64 = (src[((start) as usize)..((i) as usize)].to_string()).parse::<i64>().unwrap();
    let res: (i64, i64) = (v, i);
    return res;
}

pub fn scan_indent(src: String, start: i64) -> (i64, i64) {
    let mut j: i64 = start;
    while j < str_len(src.clone()) && ((char_at(src.clone(), j)).to_string() == (" ".to_string()).to_string() || (char_at(src.clone(), j)).to_string() == ("\t".to_string()).to_string()) {
        j += 1i64;
    }
    return (j - start, j);
}

pub fn scan_ident(src: String, start: i64) -> (String, i64) {
    let mut i: i64 = start;
    while i < str_len(src.clone()) && (is_alpha(char_at(src.clone(), i)) || is_digit(char_at(src.clone(), i))) {
        i += 1i64;
    }
    return (src[((start) as usize)..((i) as usize)].to_string(), i);
}

pub fn scan_string(src: String, start: i64) -> (String, i64) {
    let mut i: i64 = start + 1i64;
    while i < str_len(src.clone()) && (char_at(src.clone(), i)).to_string() != ("\"".to_string()).to_string() {
        i += 1i64;
    }
    let res: (String, i64) = (src[(((start + 1i64)) as usize)..((i) as usize)].to_string(), i + 1i64);
    return res;
}

pub fn keyword_token(mut w: String) -> (bool, Token) {
    let mut table: Vec<(String, Token)> = vec![("return".to_string(), Token::Return), ("if".to_string(), Token::If), ("else".to_string(), Token::Else), ("def".to_string(), Token::Def), ("let".to_string(), Token::Let), ("match".to_string(), Token::Match), ("case".to_string(), Token::Case), ("while".to_string(), Token::While), ("for".to_string(), Token::For), ("in".to_string(), Token::In)];
    let mut result: (bool, Token) = (false, Token::Ident { name: w.to_string() });
    for idx in 0i64..(table.len() as i64) {
        let pair = table[idx as usize].clone();
        if pair.0 == w {
            result = (true, pair.1);
        }
    }
    return result;
}

pub fn op_token(c: String) -> (bool, Token) {
    let mut table: Vec<(String, Token)> = vec![("+".to_string(), Token::Plus), ("-".to_string(), Token::Minus), ("*".to_string(), Token::Star), ("/".to_string(), Token::Slash), ("(".to_string(), Token::LParen), (")".to_string(), Token::RParen), ("[".to_string(), Token::LBracket), ("]".to_string(), Token::RBracket), ("{".to_string(), Token::LBrace), ("}".to_string(), Token::RBrace), ("<".to_string(), Token::Lt), (">".to_string(), Token::Gt), ("=".to_string(), Token::Eq), (".".to_string(), Token::Dot)];
    let mut result: (bool, Token) = (false, Token::Plus);
    for idx in 0i64..(table.len() as i64) {
        let pair = table[idx as usize].clone();
        if pair.0 == c {
            result = (true, pair.1);
        }
    }
    return result;
}

pub fn two_char_op(c1: String, c2: String) -> (bool, Token) {
    let key: String = c1.to_string() + &c2[..];
    let mut table: Vec<(String, Token)> = vec![("==".to_string(), Token::EqEq), ("!=".to_string(), Token::Ne), ("<=".to_string(), Token::Le), (">=".to_string(), Token::Ge), ("&&".to_string(), Token::AmpAmp), ("||".to_string(), Token::PipePipe), ("=>".to_string(), Token::FatArrow)];
    let mut result: (bool, Token) = (false, Token::Plus);
    for idx in 0i64..(table.len() as i64) {
        let pair = table[idx as usize].clone();
        if pair.0 == key {
            result = (true, pair.1);
        }
    }
    return result;
}

pub fn tokenize(src: String) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut i: i64 = 0i64;
    let mut line_start: bool = true;
    while i < str_len(src.clone()) {
        let c: String = char_at(src.clone(), i);
        if (c).to_string() == ("\n".to_string()).to_string() {
            tokens.push(Token::Newline);
            i += 1i64;
            line_start = true;
        } else { if line_start && (c).to_string() == (" ".to_string()).to_string() {
            let r: (i64, i64) = scan_indent(src.clone(), i);
            tokens.push(Token::Indent { v: r.0 });
            i = r.1;
            line_start = false;
        } else { if line_start {
            tokens.push(Token::Indent { v: 0i64 });
            line_start = false;
        } else { if (c).to_string() == (" ".to_string()).to_string() {
            i += 1i64;
        } else { if (c).to_string() == ("\"".to_string()).to_string() {
            let r: (String, i64) = scan_string(src.clone(), i);
            i = r.1;
            tokens.push(Token::StrLit { s: r.0 });
        } else { if is_digit(c.clone()) {
            let r: (i64, i64) = scan_int(src.clone(), i);
            i = r.1;
            tokens.push(Token::IntLit { v: r.0 });
        } else { if is_alpha(c.clone()) {
            let r: (String, i64) = scan_ident(src.clone(), i);
            i = r.1;
            let kw: (bool, Token) = keyword_token(r.0.clone());
            if kw.0 {
                tokens.push(kw.1);
            } else {
                tokens.push(Token::Ident { name: r.0 });
            }
        } else { if (c).to_string() == (":".to_string()).to_string() {
            tokens.push(Token::Colon);
            i += 1i64;
        } else { if (c).to_string() == (",".to_string()).to_string() {
            tokens.push(Token::Comma);
            i += 1i64;
        } else { if (c).to_string() == ("-".to_string()).to_string() && i + 1i64 < str_len(src.clone()) && (char_at(src.clone(), i + 1i64)).to_string() == (">".to_string()).to_string() {
            tokens.push(Token::Arrow);
            i += 2i64;
        } else {
            let mut handled: bool = false;
            if i + 1i64 < str_len(src.clone()) {
                let r2: (bool, Token) = two_char_op(c.clone(), char_at(src.clone(), i + 1i64));
                if r2.0 {
                    tokens.push(r2.1);
                    i += 2i64;
                    handled = true;
                }
            }
            if !(handled) {
                let r: (bool, Token) = op_token(c.clone());
                let found = r.0;
                let tok = r.1;
                if found {
                    tokens.push(tok.clone());
                    i += 1i64;
                } else {
                    i += 1i64;
                }
            }
        }}}}}}}}}}
    }
    tokens.push(Token::Eof);
    return tokens;
}

pub fn tok_at(mut toks: Vec<Token>, idx: i64) -> Token {
    return if idx >= (toks.len() as i64) { Token::Eof } else { toks[idx as usize].clone() };
}

pub fn parse_atom(toks: Vec<Token>, pos: i64) -> (Expr, i64) {
    let t = tok_at(toks.clone(), pos);
    let res: (Expr, i64) = {
match t.clone() {
    Token::IntLit { v: n } => {
        (Expr::IntLit { v: n }, LzAdd::__add__(pos, 1i64))
    }
    Token::StrLit { s: sv } => {
        (Expr::StrLit { v: sv }, LzAdd::__add__(pos, 1i64))
    }
    Token::Ident { name: nm } => {
        (Expr::Ident { name: nm }, LzAdd::__add__(pos, 1i64))
    }
    Token::LParen => {
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 1i64));
        let rv = r.0;
        let rp = r.1;
        let sep = tok_at(toks.clone(), rp);
        if sep == Token::Comma {
            let mut elems: Vec<Expr> = vec![rv.clone()];
            let mut p: i64 = LzAdd::__add__(rp, 1i64);
            let mut done: bool = false;
            while !done {
                let t2 = tok_at(toks.clone(), p);
                if t2 == Token::RParen {
                    done = true;
                    p += 1i64;
                } else {
                    let r2 = parse_logic(toks.clone(), p);
                    let rv2 = r2.0;
                    let rp2 = r2.1;
                    elems = { let mut __cat = elems.clone(); __cat.extend(vec![rv2.clone()]); __cat };
                    let sep2 = tok_at(toks.clone(), rp2);
                    if sep2 == Token::Comma {
                        p = LzAdd::__add__(rp2, 1i64);
                    } else {
                        p = rp2;
                    }
                }
            }
            let pr: (Expr, i64) = (Expr::TupleLit { elems: Box::new(elems) }, p);
            pr
        } else {
            let pr: (Expr, i64) = (rv, LzAdd::__add__(rp, 1i64));
            pr
        }
    }
    Token::LBracket => {
        let r = parse_list_elems(toks.clone(), LzAdd::__add__(pos, 1i64));
        let elems = r.0;
        let rp = r.1;
        let pr: (Expr, i64) = (Expr::ListLit { elems: Box::new(elems) }, LzAdd::__add__(rp, 1i64));
        pr
    }
    Token::LBrace => {
        let r = parse_dict_pairs(toks.clone(), LzAdd::__add__(pos, 1i64));
        let pairs = r.0;
        let rp = r.1;
        let pr: (Expr, i64) = (Expr::DictLit { pairs: Box::new(pairs) }, LzAdd::__add__(rp, 1i64));
        pr
    }
    _ => {
        (Expr::Ident { name: "?".to_string() }, LzAdd::__add__(pos, 1i64))
    }
}
    };
    return res;
}

pub fn parse_dict_pairs(toks: Vec<Token>, pos: i64) -> (Vec<(Expr, Expr)>, i64) {
    let mut out: Vec<(Expr, Expr)> = Vec::new();
    let mut p: i64 = pos;
    let mut done: bool = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::RBrace {
            done = true;
            p += 1i64;
        } else { if t == Token::Eof {
            done = true;
        } else {
            let rk: (Expr, i64) = parse_logic(toks.clone(), p);
            let k = rk.0;
            let after_key = rk.1;
            let rv: (Expr, i64) = parse_logic(toks.clone(), LzAdd::__add__(after_key, 1i64));
            let v = rv.0;
            let after_val = rv.1;
            let pair: (Expr, Expr) = (k, v);
            out.push(pair.clone());
            let sep = tok_at(toks.clone(), after_val);
            if sep == Token::Comma {
                p = LzAdd::__add__(after_val, 1i64);
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
    let mut done: bool = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::RBracket {
            done = true;
            p += 1i64;
        } else {
            let r: (Expr, i64) = parse_logic(toks.clone(), p);
            let rv = r.0;
            let rp = r.1;
            out.push(rv.clone());
            let sep = tok_at(toks.clone(), rp);
            if sep == Token::Comma {
                p = LzAdd::__add__(rp, 1i64);
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
    let mut done: bool = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::RParen {
            done = true;
            p += 1i64;
        } else {
            let r: (Expr, i64) = parse_logic(toks.clone(), p);
            let rv = r.0;
            let rp = r.1;
            out.push(rv.clone());
            let sep = tok_at(toks.clone(), rp);
            if sep == Token::Comma {
                p = LzAdd::__add__(rp, 1i64);
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
    let mut p: i64 = first.1;
    let mut done: bool = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::LParen {
            let r: (Vec<Expr>, i64) = parse_args(toks.clone(), p + 1i64);
            let args_list = r.0;
            let rp = r.1;
            let ne = Expr::Call { callee: Box::new(value.clone()), args: Box::new(args_list) };
            value = ne;
            p = rp;
        } else { if t == Token::Dot {
            let name_t = tok_at(toks.clone(), p + 1i64);
            let nm: String = field_desc(name_t.clone());
            let ne = Expr::Get { recv: Box::new(value.clone()), name: nm };
            value = ne;
            p += 2i64;
        } else { if t == Token::LBracket {
            let r: (Expr, i64) = parse_logic(toks.clone(), p + 1i64);
            let idx_expr = r.0;
            let rp = r.1;
            let ne = Expr::Index { recv: Box::new(value.clone()), idx: Box::new(idx_expr) };
            value = ne;
            p = (LzAdd::__add__(rp, 1i64));
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
    let mut p: i64 = first.1;
    let mut done: bool = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::Star {
            let r: (Expr, i64) = parse_postfix(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Bin { op: "*".to_string(), l: Box::new(value.clone()), r: Box::new(rv) };
            value = ne;
            p = rp;
        } else { if t == Token::Slash {
            let r: (Expr, i64) = parse_postfix(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Bin { op: "/".to_string(), l: Box::new(value.clone()), r: Box::new(rv) };
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
    let mut p: i64 = first.1;
    let mut done: bool = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::Plus {
            let r: (Expr, i64) = parse_term(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Bin { op: "+".to_string(), l: Box::new(value.clone()), r: Box::new(rv) };
            value = ne;
            p = rp;
        } else { if t == Token::Minus {
            let r: (Expr, i64) = parse_term(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Bin { op: "-".to_string(), l: Box::new(value.clone()), r: Box::new(rv) };
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
    let mut p: i64 = first.1;
    let mut done: bool = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        let co: (bool, String) = cmp_op(t.clone());
        if co.0 {
            let r: (Expr, i64) = parse_expr(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Cmp { op: co.1, l: Box::new(value.clone()), r: Box::new(rv) };
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
    let mut p: i64 = first.1;
    let mut done: bool = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        let lo: (bool, String) = logic_op(t.clone());
        if lo.0 {
            let r: (Expr, i64) = parse_cmp(toks.clone(), p + 1i64);
            let rv = r.0;
            let rp = r.1;
            let ne = Expr::Logic { op: lo.1, l: Box::new(value.clone()), r: Box::new(rv) };
            value = ne;
            p = rp;
        } else {
            done = true;
        }
    }
    let res: (Expr, i64) = (value, p);
    return res;
}

pub fn expr_list_summary(mut xs: Vec<Expr>) -> String {
    let mut out: String = "".to_string();
    for idx in 0i64..(xs.len() as i64) {
        if idx > 0i64 {
            out = out.to_string() + &", ".to_string()[..];
        }
        out = LzAdd::__add__(out, display_expr(xs[idx as usize].clone()));
    }
    return out;
}

pub fn expr_pair_summary(mut xs: Vec<(Expr, Expr)>) -> String {
    let mut out: String = "".to_string();
    for idx in 0i64..(xs.len() as i64) {
        if idx > 0i64 {
            out = out.to_string() + &", ".to_string()[..];
        }
        let pair = xs[idx as usize].clone();
        out = LzAdd::__add__(LzAdd::__add__(out, display_expr(pair.0.clone())) + &": ".to_string()[..], display_expr(pair.1.clone()));
    }
    return out;
}

pub fn display_expr(e: Expr) -> String {
    let res: String = {
match e.clone() {
    Expr::IntLit { v: n } => {
        n.to_string()
    }
    Expr::StrLit { v: sv } => {
        "\"".to_string().to_string() + &sv[..] + &"\"".to_string()[..]
    }
    Expr::Ident { name: nm } => {
        nm
    }
    Expr::Bin { op: op, l: lv, r: rv } => {
        let lv = *lv;
        let rv = *rv;
        LzAdd::__add__(LzAdd::__add__(display_expr(lv.clone()), " ".to_string()) + &op[..] + &" ".to_string()[..], display_expr(rv.clone()))
    }
    Expr::Cmp { op: op, l: lv, r: rv } => {
        let lv = *lv;
        let rv = *rv;
        LzAdd::__add__(LzAdd::__add__(display_expr(lv.clone()), " ".to_string()) + &op[..] + &" ".to_string()[..], display_expr(rv.clone()))
    }
    Expr::Logic { op: op, l: lv, r: rv } => {
        let lv = *lv;
        let rv = *rv;
        LzAdd::__add__(LzAdd::__add__(display_expr(lv.clone()), " ".to_string()) + &op[..] + &" ".to_string()[..], display_expr(rv.clone()))
    }
    Expr::Call { callee: cv, args: av } => {
        let cv = *cv;
        let av = *av;
        LzAdd::__add__(LzAdd::__add__(display_expr(cv.clone()), "(".to_string()), expr_list_summary(av.clone())) + &")".to_string()[..]
    }
    Expr::Get { recv: rv, name: nm } => {
        let rv = *rv;
        LzAdd::__add__(display_expr(rv.clone()), ".".to_string()) + &nm[..]
    }
    Expr::Index { recv: rv, idx: iv } => {
        let rv = *rv;
        let iv = *iv;
        LzAdd::__add__(LzAdd::__add__(display_expr(rv.clone()), "[".to_string()), display_expr(iv.clone())) + &"]".to_string()[..]
    }
    Expr::ListLit { elems: ev } => {
        let ev = *ev;
        LzAdd::__add__("[".to_string(), expr_list_summary(ev.clone())) + &"]".to_string()[..]
    }
    Expr::TupleLit { elems: ev } => {
        let ev = *ev;
        LzAdd::__add__("(".to_string(), expr_list_summary(ev.clone())) + &")".to_string()[..]
    }
    Expr::DictLit { pairs: pv } => {
        let pv = *pv;
        LzAdd::__add__("{".to_string(), expr_pair_summary(pv.clone())) + &"}".to_string()[..]
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
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 1i64));
        let rv = r.0;
        let rp = r.1;
        let rr: (String, i64) = (LzAdd::__add__("return ".to_string(), display_expr(rv.clone())), rp);
        rr
    }
    Token::If => {
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 1i64));
        let rv = r.0;
        let rp = r.1;
        let after = tok_at(toks.clone(), rp);
        let npos: i64 = if after == Token::Colon { (LzAdd::__add__(rp, 1i64)) } else { rp };
        let blk = parse_block(toks.clone(), npos, indent);
        let body = blk.0;
        let blk_end = blk.1;
        let body_s = list_summary(body.clone());
        let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__("if ".to_string(), display_expr(rv.clone())) + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
        rr
    }
    Token::Else => {
        let rr: (String, i64) = ("else".to_string(), LzAdd::__add__(pos, 1i64));
        rr
    }
    Token::Match => {
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 1i64));
        let rv = r.0;
        let rp = r.1;
        let after = tok_at(toks.clone(), rp);
        let npos: i64 = if after == Token::Colon { (LzAdd::__add__(rp, 1i64)) } else { rp };
        let blk = parse_block(toks.clone(), npos, indent);
        let body = blk.0;
        let blk_end = blk.1;
        let body_s = list_summary(body.clone());
        let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__("match ".to_string(), display_expr(rv.clone())) + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
        rr
    }
    Token::Case => {
        let pat_t = tok_at(toks.clone(), LzAdd::__add__(pos, 1i64));
        let pat_s = pattern_desc(pat_t.clone());
        let arrow_t = tok_at(toks.clone(), LzAdd::__add__(pos, 2i64));
        if arrow_t == Token::FatArrow {
            let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 3i64));
            let rv = r.0;
            let rp = r.1;
            let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__("case ".to_string(), pat_s) + &" => ".to_string()[..], display_expr(rv.clone())), rp);
            rr
        } else {
            let colon_t = tok_at(toks.clone(), LzAdd::__add__(pos, 2i64));
            let npos: i64 = if colon_t == Token::Colon { (LzAdd::__add__(pos, 3i64)) } else { LzAdd::__add__(pos, 2i64) };
            let blk = parse_block(toks.clone(), npos, indent);
            let body = blk.0;
            let blk_end = blk.1;
            let body_s = list_summary(body.clone());
            let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__("case ".to_string(), pat_s) + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
            rr
        }
    }
    Token::While => {
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 1i64));
        let rv = r.0;
        let rp = r.1;
        let after = tok_at(toks.clone(), rp);
        let npos: i64 = if after == Token::Colon { (LzAdd::__add__(rp, 1i64)) } else { rp };
        let blk = parse_block(toks.clone(), npos, indent);
        let body = blk.0;
        let blk_end = blk.1;
        let body_s = list_summary(body.clone());
        let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__("while ".to_string(), display_expr(rv.clone())) + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
        rr
    }
    Token::For => {
        let name_t = tok_at(toks.clone(), LzAdd::__add__(pos, 1i64));
        let name_s = ident_name(name_t.clone());
        let r = parse_logic(toks.clone(), LzAdd::__add__(pos, 3i64));
        let rv = r.0;
        let rp = r.1;
        let after = tok_at(toks.clone(), rp);
        let npos: i64 = if after == Token::Colon { (LzAdd::__add__(rp, 1i64)) } else { rp };
        let blk = parse_block(toks.clone(), npos, indent);
        let body = blk.0;
        let blk_end = blk.1;
        let body_s = list_summary(body.clone());
        let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("for ".to_string(), name_s) + &" in ".to_string()[..], display_expr(rv.clone())) + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
        rr
    }
    Token::Def => {
        let name_t = tok_at(toks.clone(), LzAdd::__add__(pos, 1i64));
        let name_s = ident_name(name_t.clone());
        let mut sig: String = LzAdd::__add__(name_s, "".to_string());
        let mut ap: i64 = LzAdd::__add__(pos, 2i64);
        let lp_t = tok_at(toks.clone(), ap);
        if lp_t == Token::LParen {
            let pr = parse_params(toks.clone(), ap);
            sig = LzAdd::__add__(sig, pr.0);
            ap = pr.1;
        }
        let rf = ret_suffix(toks.clone(), ap);
        sig = LzAdd::__add__(sig, rf.0);
        ap = rf.1;
        let colon_t = tok_at(toks.clone(), ap);
        let npos: i64 = if colon_t == Token::Colon { (ap + 1i64) } else { ap };
        let blk = parse_block(toks.clone(), npos, indent);
        let body = blk.0;
        let blk_end = blk.1;
        let body_s = list_summary(body.clone());
        let rr: (String, i64) = (LzAdd::__add__("def ".to_string().to_string() + &sig[..] + &" {".to_string()[..], body_s) + &"}".to_string()[..], blk_end);
        rr
    }
    Token::Let => {
        let name_t = tok_at(toks.clone(), LzAdd::__add__(pos, 1i64));
        let name_s = ident_name(name_t.clone());
        let mut ap: i64 = LzAdd::__add__(pos, 2i64);
        let mut ty_s: String = "".to_string();
        let colon_t = tok_at(toks.clone(), ap);
        if colon_t == Token::Colon {
            let ty_t = tok_at(toks.clone(), ap + 1i64);
            ty_s = ident_name(ty_t.clone());
            ap += 2i64;
        }
        let eq_t = tok_at(toks.clone(), ap);
        let rp: i64 = if eq_t == Token::Eq { (ap + 1i64) } else { ap };
        let r = parse_logic(toks.clone(), rp);
        let rv = r.0;
        let rend = r.1;
        let full: String = LzAdd::__add__(LzAdd::__add__(name_s, (if (ty_s.len() as i64) > 0i64 { " : ".to_string().to_string() + &ty_s[..] } else { "".to_string() })) + &" = ".to_string()[..], display_expr(rv.clone()));
        let rr: (String, i64) = ("let ".to_string().to_string() + &full[..], rend);
        rr
    }
    Token::Newline => {
        let rr: (String, i64) = ("".to_string(), LzAdd::__add__(pos, 1i64));
        rr
    }
    _ => {
        let r = parse_logic(toks.clone(), pos);
        let rv = r.0;
        let rp = r.1;
        let eq_t = tok_at(toks.clone(), rp);
        if eq_t == Token::Eq {
            let r2 = parse_logic(toks.clone(), LzAdd::__add__(rp, 1i64));
            let rv2 = r2.0;
            let rend = r2.1;
            let rr: (String, i64) = (LzAdd::__add__(LzAdd::__add__(display_expr(rv.clone()), " = ".to_string()), display_expr(rv2.clone())), rend);
            rr
        } else {
            let rr: (String, i64) = (LzAdd::__add__("expr ".to_string(), display_expr(rv.clone())), rp);
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
    Token::Ident { name: nm } => {
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
    Token::IntLit { v: n } => {
        n.to_string()
    }
    Token::Ident { name: nm } => {
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
    Token::IntLit { v: n } => {
        n.to_string()
    }
    Token::Ident { name: nm } => {
        nm
    }
    _ => {
        "?".to_string()
    }
}
    };
    return res;
}

pub fn list_summary(mut xs: Vec<String>) -> String {
    let mut out: String = "".to_string();
    for idx in 0i64..(xs.len() as i64) {
        if idx > 0i64 {
            out = out.to_string() + &", ".to_string()[..];
        }
        out = out.to_string() + &xs[idx as usize];
    }
    return out;
}

pub fn parse_params(toks: Vec<Token>, pos: i64) -> (String, i64) {
    let mut out: Vec<String> = Vec::new();
    let mut p: i64 = pos + 1i64;
    let mut done: bool = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        if t == Token::RParen {
            done = true;
            p += 1i64;
        } else { if t == Token::Eof {
            done = true;
        } else {
            let nm: String = ident_name(t.clone());
            let colon_t = tok_at(toks.clone(), p + 1i64);
            if colon_t == Token::Colon {
                let ty_t = tok_at(toks.clone(), p + 2i64);
                let ty: String = ident_name(ty_t.clone());
                let item: String = LzAdd::__add__(LzAdd::__add__(nm, ": ".to_string()), ty);
                out.push(item.clone());
                let sep = tok_at(toks.clone(), p + 3i64);
                if sep == Token::Comma {
                    p += 4i64;
                } else {
                    p += 3i64;
                }
            } else {
                out.push(nm.clone());
                let sep = tok_at(toks.clone(), p + 1i64);
                if sep == Token::Comma {
                    p += 2i64;
                } else {
                    p += 1i64;
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
        let res: (String, i64) = (LzAdd::__add__(" -> ".to_string(), ty), pos + 2i64);
        res
    } else {
        let res: (String, i64) = ("".to_string(), pos);
        res
    };
}

pub fn parse_block(toks: Vec<Token>, pos: i64, indent: i64) -> (Vec<String>, i64) {
    let mut out: Vec<String> = Vec::new();
    let mut p: i64 = pos;
    let mut done: bool = false;
    while !done {
        let t = tok_at(toks.clone(), p);
        let tag: i64 = block_tag(t.clone());
        if tag == 0i64 {
            done = true;
        } else { if tag == 1i64 {
            p += 1i64;
        } else { if tag == 2i64 {
            let n: i64 = block_indent(t.clone());
            if n > indent {
                let r: (String, i64) = parse_stmt(toks.clone(), p + 1i64, n);
                let s = r.0;
                if (s.len() as i64) > 0i64 {
                    out.push(s.clone());
                }
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
    Token::Indent { v: n } => {
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
    Token::Indent { v: n } => {
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
    let mut stmts: Vec<String> = r.0;
    for idx in 0i64..(stmts.len() as i64) {
        println!("{}", stmts[idx as usize]);
    }
}

const __name__: &str = "main";

const __file__: &str = "lz_parser.lz";

const __package__: &str = "frontend";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\src\\frontend";

const __doc__: &str = "";

const __is_macro__: bool = false;


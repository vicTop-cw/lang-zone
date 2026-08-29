
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
pub enum IrType {
    Int,
    F64,
    Str,
    Bool,
    Unit,
    Never,
    Any,
    Self_,
    Named(String, Box<Vec<IrType>>),
    Opt(Box<IrType>),
    Res(Box<IrType>, Box<IrType>),
    Tuple(Box<Vec<IrType>>),
    FnType(Box<Vec<IrType>>, Box<IrType>),
    Ref(Box<IrType>),
    MutRef(Box<IrType>),
    Generic(String),
    Duck(Box<Vec<(String, IrType)>>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    LitInt(i64, IrType),
    LitF64(f64, IrType),
    LitStr(String, IrType),
    LitFStr(String, IrType),
    LitBool(bool, IrType),
    LitUnit(IrType),
    LitNone(IrType),
    Var(String, IrType),
    Call(Box<Expr>, Box<Vec<Expr>>, IrType),
    MethodCall(Box<Expr>, String, Box<Vec<Expr>>, IrType),
    FieldAccess(Box<Expr>, String, IrType),
    IndexGet(Box<Expr>, Box<Expr>, IrType),
    IndexSet(Box<Expr>, Box<Expr>, Box<Expr>, IrType),
    BinOp(String, Box<Expr>, Box<Expr>, IrType),
    UnOp(String, Box<Expr>, IrType),
    StructCtor(String, Box<Vec<(String, Expr)>>, IrType),
    EnumCtor(String, String, Box<Vec<Expr>>, IrType),
    Cast(Box<Expr>, IrType, IrType),
    MagicCall(String, Box<Vec<Expr>>, IrType),
    IfExpr(Box<Expr>, Box<Expr>, Box<Expr>, IrType),
    Lambda(Vec<String>, Box<Expr>, IrType),
    Pipe(Box<Expr>, Box<Expr>, Box<Vec<Expr>>, IrType),
    TupleLit(Box<Vec<Expr>>, IrType),
    ListLit(Box<Vec<Expr>>, IrType),
    BlockExpr(Vec<Stmt>, IrType),
    GenExpr(Box<Expr>, IrType),
    Paren(Box<Expr>, IrType),
    Range(Box<Expr>, bool, IrType),
    Dict(Box<Vec<(Expr, Expr)>>, IrType),
    AssignExpr(Box<Expr>, Box<Expr>, IrType),
    ImplicitConvert(Box<Expr>, IrType, IrType),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockIR {
    Block(Vec<Stmt>, IrType),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaybeExpr {
    NoExpr,
    YesExpr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaybeBlock {
    NoBlock,
    YesBlock(BlockIR),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaybeStr {
    NoStr,
    YesStr(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaybeIrType {
    NoTy,
    YesTy(IrType),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaybePattern {
    NoPat,
    YesPat(Pattern),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let(String, IrType, Expr, bool, bool),
    Assign(Expr, Expr),
    Return(MaybeExpr),
    ExprStmt(Expr),
    If(Expr, BlockIR, MaybeBlock),
    For(String, Expr, MaybeExpr, BlockIR, MaybeBlock),
    While(Expr, MaybeExpr, BlockIR, MaybeBlock),
    Block(Box<Vec<Stmt>>),
    Match(Expr, Vec<(Pattern, MaybeExpr, BlockIR)>),
    Yield(Expr),
    YieldFrom(Expr),
    Break,
    BreakLabel(String, MaybeExpr),
    Continue,
    BlockLabel(String, BlockIR),
    Defer(BlockIR),
    TryCatch(BlockIR, Vec<(MaybePattern, BlockIR)>, MaybeBlock, MaybeBlock),
    WhileLet(Pattern, Expr, MaybeExpr, BlockIR),
    Raise(Expr),
    Assert(Expr),
    TypeAlias(String, IrType),
    CheckerBlock(String, MaybeStr),
    Pass,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard,
    Ident(String),
    RefMutIdent(String),
    LitInt(i64),
    LitStr(String),
    LitBool(bool),
    LitF64(f64),
    Tuple(Box<Vec<Pattern>>),
    List(Box<Vec<Pattern>>),
    Struct(String, Box<Vec<(String, Pattern)>>),
    Enum(String, String, Box<Vec<Pattern>>),
    Rest(MaybeStr),
    Range(i64, i64, bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    FnDef(String, Vec<String>, Vec<(String, IrType, bool, bool, bool)>, IrType, Vec<Stmt>),
    Const(String, IrType, Expr),
    StructDef(String, Vec<String>, Vec<(String, IrType)>),
    EnumDef(String, Vec<String>, Vec<(String, Vec<IrType>)>),
    TraitDef(String, Vec<IrType>, Vec<(String, Vec<IrType>, IrType)>),
    DuckDef(String, i64),
    UseStmt(Vec<String>, MaybeStr, Vec<String>, bool),
    TypeAlias(String, IrType),
    Impl(MaybeIrType, IrType, Vec<String>),
    CheckerBlock(String, MaybeStr),
    Test(String, Vec<Stmt>),
}

#[derive(Debug, Clone)]
pub struct IrModule {
    pub name: String,
    pub items: Vec<Item>,
    pub prelude: Vec<String>,
    pub version: i64,
}

pub fn display_type(t: IrType) -> String {
    match t.clone() {
        IrType::Int => {
            "int".to_string()
        }
        IrType::F64 => {
            "f64".to_string()
        }
        IrType::Str => {
            "str".to_string()
        }
        IrType::Bool => {
            "bool".to_string()
        }
        IrType::Unit => {
            "()".to_string()
        }
        IrType::Never => {
            "!".to_string()
        }
        IrType::Any => {
            "?".to_string()
        }
        IrType::Self_ => {
            "Self".to_string()
        }
        IrType::Generic(g) => {
            g
        }
        IrType::Named(p, a) => {
            let a = *a;
            p + &(if (a.len() as i64) > 0i64 { "<".to_string().to_string() + &type_list(a.clone())[..] + &">".to_string()[..] } else { "".to_string() })
        }
        IrType::Opt(x) => {
            let x = *x;
            "Option<".to_string().to_string() + &display_type(x.clone())[..] + &">".to_string()[..]
        }
        IrType::Res(o, e) => {
            let o = *o;
            let e = *e;
            "Result<".to_string().to_string() + &display_type(o.clone())[..] + &", ".to_string()[..] + &display_type(e.clone())[..] + &">".to_string()[..]
        }
        IrType::Tuple(es) => {
            let es = *es;
            "(".to_string().to_string() + &type_list(es.clone())[..] + &")".to_string()[..]
        }
        IrType::FnType(ps, r) => {
            let ps = *ps;
            let r = *r;
            "fn(".to_string().to_string() + &type_list(ps.clone())[..] + &") -> ".to_string()[..] + &display_type(r.clone())[..]
        }
        IrType::Ref(x) => {
            let x = *x;
            "&".to_string().to_string() + &display_type(x.clone())[..]
        }
        IrType::MutRef(x) => {
            let x = *x;
            "&mut ".to_string().to_string() + &display_type(x.clone())[..]
        }
        IrType::Duck(fs) => {
            let fs = *fs;
            "duck {".to_string().to_string() + &duck_fields(fs.clone())[..] + &"}".to_string()[..]
        }
    }
}

pub fn type_list(ts: Vec<IrType>) -> String {
    return if (ts.len() as i64) == 0i64 { "".to_string()} else {
        let head: String = display_type(ts[((0i64) as usize)].clone());
        if (ts.len() as i64) > 1i64 { head + &", ".to_string()[..] + &type_list(tail_t(ts.clone()))[..] } else { head }
    };
}

pub fn tail_t(ts: Vec<IrType>) -> Vec<IrType> {
    let mut out: Vec<IrType> = Vec::new();
    for idx in (1i64..(ts.len() as i64)).into_iter() {
        out.push(ts[((idx) as usize)].clone());
    }
    return out;
}

pub fn duck_fields(fs: Vec<(String, IrType)>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        let f = fs[((idx) as usize)].clone();
        out = out + &(if idx > 0i64 { ", ".to_string() } else { "".to_string() }) + &f.0 + &": ".to_string()[..] + &display_type(f.1.clone())[..];
    }
    return out;
}

pub fn display_expr(e: Expr) -> String {
    match e.clone() {
        Expr::LitInt(n, t) => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string()[..] + &n.to_string()[..] + &"_i64".to_string()[..]
        }
        Expr::LitF64(n, t) => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string()[..] + &n.to_string()[..] + &"_f64".to_string()[..]
        }
        Expr::LitStr(s, t) => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] \"".to_string()[..] + &s[..] + &"\"".to_string()[..]
        }
        Expr::LitFStr(s, t) => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] f\"".to_string()[..] + &s[..] + &"\"".to_string()[..]
        }
        Expr::LitBool(b, t) => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string()[..] + &b.to_string()[..]
        }
        Expr::LitUnit(t) => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ()".to_string()[..]
        }
        Expr::LitNone(t) => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] None".to_string()[..]
        }
        Expr::Var(n, t) => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string()[..] + &n[..]
        }
        Expr::Call(c, a, t) => {
            let c = *c;
            let a = *a;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] call ".to_string()[..] + &display_expr(c.clone())[..] + &arg_list(a.clone())[..]
        }
        Expr::MethodCall(r, m, a, t) => {
            let r = *r;
            let a = *a;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] method ".to_string()[..] + &display_expr(r.clone())[..] + &".".to_string()[..] + &m[..] + &arg_list(a.clone())[..]
        }
        Expr::FieldAccess(b, f, t) => {
            let b = *b;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] field ".to_string()[..] + &display_expr(b.clone())[..] + &".".to_string()[..] + &f[..]
        }
        Expr::IndexGet(b, k, t) => {
            let b = *b;
            let k = *k;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] index ".to_string()[..] + &display_expr(b.clone())[..] + &"[".to_string()[..] + &display_expr(k.clone())[..] + &"]".to_string()[..]
        }
        Expr::IndexSet(b, k, v, t) => {
            let b = *b;
            let k = *k;
            let v = *v;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] index_set ".to_string()[..] + &display_expr(b.clone())[..] + &"[".to_string()[..] + &display_expr(k.clone())[..] + &"] = ".to_string()[..] + &display_expr(v.clone())[..]
        }
        Expr::BinOp(o, l, r, t) => {
            let l = *l;
            let r = *r;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] binop ".to_string()[..] + &display_expr(l.clone())[..] + &" ".to_string()[..] + &o[..] + &" ".to_string()[..] + &display_expr(r.clone())[..]
        }
        Expr::UnOp(o, p, t) => {
            let p = *p;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] unop ".to_string()[..] + &o[..] + &" ".to_string()[..] + &display_expr(p.clone())[..]
        }
        Expr::StructCtor(n, fs, t) => {
            let fs = *fs;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] new ".to_string()[..] + &n[..] + &field_pairs(fs.clone())[..]
        }
        Expr::EnumCtor(en, v, a, t) => {
            let a = *a;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string()[..] + &en[..] + &"::".to_string()[..] + &v[..] + &arg_list(a.clone())[..]
        }
        Expr::Cast(i, tg, t) => {
            let i = *i;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] cast ".to_string()[..] + &display_expr(i.clone())[..] + &" as ".to_string()[..] + &display_type(tg.clone())[..]
        }
        Expr::MagicCall(m, a, t) => {
            let a = *a;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] magic ".to_string()[..] + &m[..] + &arg_list(a.clone())[..]
        }
        Expr::IfExpr(c, th, el, t) => {
            let c = *c;
            let th = *th;
            let el = *el;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] if ".to_string()[..] + &display_expr(c.clone())[..] + &" then ".to_string()[..] + &display_expr(th.clone())[..] + &" else ".to_string()[..] + &display_expr(el.clone())[..]
        }
        Expr::Lambda(ps, b, t) => {
            let b = *b;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] |".to_string()[..] + &str_join(ps.clone())[..] + &"| ".to_string()[..] + &display_expr(b.clone())[..]
        }
        Expr::Pipe(r, c, a, t) => {
            let r = *r;
            let c = *c;
            let a = *a;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] pipe ".to_string()[..] + &display_expr(r.clone())[..] + &" |> ".to_string()[..] + &display_expr(c.clone())[..] + &arg_list(a.clone())[..]
        }
        Expr::TupleLit(es, t) => {
            let es = *es;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] (".to_string()[..] + &expr_list(es.clone())[..] + &")".to_string()[..]
        }
        Expr::ListLit(xs, t) => {
            let xs = *xs;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] [".to_string()[..] + &expr_list(xs.clone())[..] + &"]".to_string()[..]
        }
        Expr::BlockExpr(ss, t) => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] block ".to_string()[..] + &block_with_ty(ss.clone(), t.clone())[..]
        }
        Expr::GenExpr(y, t) => {
            let y = *y;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] gen ".to_string()[..] + &display_expr(y.clone())[..]
        }
        Expr::Paren(i, t) => {
            let i = *i;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] (".to_string()[..] + &display_expr(i.clone())[..] + &")".to_string()[..]
        }
        Expr::Range(en, inc, t) => {
            let en = *en;
            "[".to_string().to_string() + &display_type(t)[..] + &"] <expr>".to_string()[..]
        }
        Expr::Dict(ps, t) => {
            let ps = *ps;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] <expr>".to_string()[..]
        }
        Expr::AssignExpr(tg, v, t) => {
            let tg = *tg;
            let v = *v;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] <expr>".to_string()[..]
        }
        Expr::ImplicitConvert(s, tt, t) => {
            let s = *s;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] <expr>".to_string()[..]
        }
    }
}

pub fn arg_list(as_: Vec<Expr>) -> String {
    return if (as_.len() as i64) == 0i64 { "".to_string() } else { "(".to_string().to_string() + &expr_list(as_.clone())[..] + &")".to_string()[..] };
}

pub fn expr_list(es: Vec<Expr>) -> String {
    return if (es.len() as i64) == 0i64 { "".to_string()} else {
        let head: String = display_expr(es[((0i64) as usize)].clone());
        if (es.len() as i64) > 1i64 { head + &", ".to_string()[..] + &expr_list(tail_e(es.clone()))[..] } else { head }
    };
}

pub fn tail_e(es: Vec<Expr>) -> Vec<Expr> {
    let mut out: Vec<Expr> = Vec::new();
    for idx in (1i64..(es.len() as i64)).into_iter() {
        out.push(es[((idx) as usize)].clone());
    }
    return out;
}

pub fn field_pairs(fs: Vec<(String, Expr)>) -> String {
    return if (fs.len() as i64) == 0i64 { "".to_string()} else {
        let mut out: String = "{ ".to_string();
        for idx in (0i64..(fs.len() as i64)).into_iter() {
            out = out + &(if idx > 0i64 { ", ".to_string() } else { "".to_string() }) + &pair_str(fs[((idx) as usize)].clone())[..];
        }
        out + &" }".to_string()[..]
    };
}

pub fn pair_str(p: (String, Expr)) -> String {
    return p.0 + &": ".to_string()[..] + &display_expr(p.1.clone())[..];
}

pub fn display_stmt(s: Stmt) -> String {
    match s.clone() {
        Stmt::Let(n, t, v, m, r) => {
            let_kw(m, r) + &n[..] + &": ".to_string()[..] + &display_type(t.clone())[..] + &" = ".to_string()[..] + &display_expr(v.clone())[..]
        }
        Stmt::Assign(t, v) => {
            display_expr(t.clone()) + &" = ".to_string()[..] + &display_expr(v.clone())[..]
        }
        Stmt::Return(v) => {
            match v.clone() {
                MaybeExpr::YesExpr(inner) => {
                    "return ".to_string().to_string() + &display_expr(inner.clone())[..]
                }
                MaybeExpr::NoExpr => {
                    "return".to_string()
                }
            }
        }
        Stmt::ExprStmt(e) => {
            display_expr(e.clone())
        }
        Stmt::If(c, t, e) => {
            let base: String = "if ".to_string().to_string() + &display_expr(c.clone())[..] + &" ".to_string()[..] + &block_disp(t.clone())[..];
            match e.clone() {
                MaybeBlock::YesBlock(es) => {
                    base + &" else ".to_string()[..] + &block_disp(es.clone())[..]
                }
                MaybeBlock::NoBlock => {
                    base
                }
            }
        }
        Stmt::For(v, it, g, b, eb) => {
            "for ".to_string().to_string() + &v[..] + &" in ".to_string()[..] + &display_expr(it.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string()[..] + &block_disp(b.clone())[..] + &else_s(eb.clone())[..]
        }
        Stmt::While(c, g, b, eb) => {
            "while ".to_string().to_string() + &display_expr(c.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string()[..] + &block_disp(b.clone())[..] + &else_s(eb.clone())[..]
        }
        Stmt::Block(b) => {
            let b = *b;
            stmt_block(b.clone())
        }
        Stmt::Match(s, as_) => {
            match_block(s.clone(), as_.clone())
        }
        Stmt::Yield(v) => {
            "yield ".to_string().to_string() + &display_expr(v.clone())[..]
        }
        Stmt::YieldFrom(it) => {
            "yield from ".to_string().to_string() + &display_expr(it.clone())[..]
        }
        Stmt::Break => {
            "break".to_string()
        }
        Stmt::BreakLabel(l, v) => {
            let base: String = "break \'".to_string().to_string() + &l[..];
            match v.clone() {
                MaybeExpr::YesExpr(inner) => {
                    base + &" ".to_string()[..] + &display_expr(inner.clone())[..]
                }
                MaybeExpr::NoExpr => {
                    base
                }
            }
        }
        Stmt::Continue => {
            "continue".to_string()
        }
        Stmt::BlockLabel(l, b) => {
            "block \'".to_string().to_string() + &l[..] + &" ".to_string()[..] + &block_disp(b.clone())[..]
        }
        Stmt::Defer(b) => {
            "defer ".to_string().to_string() + &block_disp(b.clone())[..]
        }
        Stmt::TryCatch(b, cs, eb, fb) => {
            let mut out: String = "try ".to_string().to_string() + &block_disp(b.clone())[..];
            out = out + &try_catches_str(cs.clone())[..];
            out = out + &else_s(eb.clone())[..];
            out = out + &finally_s(fb.clone())[..];
            out
        }
        Stmt::WhileLet(p, e, g, b) => {
            "while let ".to_string().to_string() + &display_pattern(p.clone())[..] + &" = ".to_string()[..] + &display_expr(e.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string()[..] + &block_disp(b.clone())[..]
        }
        Stmt::CheckerBlock(l, p) => {
            "block \'".to_string().to_string() + &l[..] + &"[ps:".to_string()[..] + &opt_debug(p.clone())[..] + &"]".to_string()[..]
        }
        Stmt::Raise(v) => {
            "<stmt>".to_string()
        }
        Stmt::Assert(c) => {
            "<stmt>".to_string()
        }
        Stmt::TypeAlias(n, t) => {
            "<stmt>".to_string()
        }
        Stmt::Pass => {
            "<stmt>".to_string()
        }
    }
}

pub fn let_kw(m: bool, r: bool) -> String {
    return (if (m && r) { "let mut ref ".to_string() } else { (if r { "let ref ".to_string() } else { (if m { "let mut ".to_string() } else { "let ".to_string() }) }) });
}

pub fn guard_s(g: MaybeExpr) -> String {
    match g.clone() {
        MaybeExpr::YesExpr(e) => {
            " if ".to_string().to_string() + &display_expr(e.clone())[..]
        }
        MaybeExpr::NoExpr => {
            "".to_string()
        }
    }
}

pub fn else_s(eb: MaybeBlock) -> String {
    match eb.clone() {
        MaybeBlock::YesBlock(b) => {
            " else ".to_string().to_string() + &block_disp(b.clone())[..]
        }
        MaybeBlock::NoBlock => {
            "".to_string()
        }
    }
}

pub fn finally_s(fb: MaybeBlock) -> String {
    match fb.clone() {
        MaybeBlock::YesBlock(b) => {
            " finally ".to_string().to_string() + &block_disp(b.clone())[..]
        }
        MaybeBlock::NoBlock => {
            "".to_string()
        }
    }
}

pub fn try_catches_str(cs: Vec<(MaybePattern, BlockIR)>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(cs.len() as i64)).into_iter() {
        let c = cs[((idx) as usize)].clone();
        out = out + &catch_str(c.clone())[..];
    }
    return out;
}

pub fn catch_str(c: (MaybePattern, BlockIR)) -> String {
    match c.0 {
        MaybePattern::YesPat(p) => {
            " catch(".to_string().to_string() + &display_pattern(p.clone())[..] + &") ".to_string()[..] + &block_disp(c.1.clone())[..]
        }
        MaybePattern::NoPat => {
            " catch ".to_string().to_string() + &block_disp(c.1.clone())[..]
        }
    }
}

pub fn display_pattern(p: Pattern) -> String {
    match p.clone() {
        Pattern::Wildcard => {
            "_".to_string()
        }
        Pattern::Ident(n) => {
            n
        }
        Pattern::RefMutIdent(n) => {
            "ref mut ".to_string().to_string() + &n[..]
        }
        Pattern::LitInt(n) => {
            n.to_string() + &"_i64".to_string()[..]
        }
        Pattern::LitStr(s) => {
            "\"".to_string().to_string() + &s[..] + &"\"".to_string()[..]
        }
        Pattern::LitBool(b) => {
            b.to_string()
        }
        Pattern::LitF64(n) => {
            n.to_string() + &"_f64".to_string()[..]
        }
        Pattern::Tuple(es) => {
            let es = *es;
            "(".to_string().to_string() + &pat_list(es.clone())[..] + &")".to_string()[..]
        }
        Pattern::List(es) => {
            let es = *es;
            "[".to_string().to_string() + &pat_list(es.clone())[..] + &"]".to_string()[..]
        }
        Pattern::Struct(n, fs) => {
            let fs = *fs;
            n + &pat_struct_fields(fs.clone())[..]
        }
        Pattern::Enum(en, v, a) => {
            let a = *a;
            en + &"::".to_string()[..] + &v[..] + &arg_pat_list(a.clone())[..]
        }
        Pattern::Rest(rn) => {
            match rn.clone() {
                MaybeStr::YesStr(n2) => {
                    "..".to_string().to_string() + &n2[..]
                }
                MaybeStr::NoStr => {
                    "..".to_string()
                }
            }
        }
        Pattern::Range(st, en, inc) => {
            st.to_string() + &(if inc { "..=".to_string() } else { "..".to_string() }) + &en.to_string()[..]
        }
    }
}

pub fn pat_list(ps: Vec<Pattern>) -> String {
    return if (ps.len() as i64) == 0i64 { "".to_string()} else {
        let head: String = display_pattern(ps[((0i64) as usize)].clone());
        if (ps.len() as i64) > 1i64 { head + &", ".to_string()[..] + &pat_list(tail_pat(ps.clone()))[..] } else { head }
    };
}

pub fn tail_pat(ps: Vec<Pattern>) -> Vec<Pattern> {
    let mut out: Vec<Pattern> = Vec::new();
    for idx in (1i64..(ps.len() as i64)).into_iter() {
        out.push(ps[((idx) as usize)].clone());
    }
    return out;
}

pub fn arg_pat_list(ps: Vec<Pattern>) -> String {
    return if (ps.len() as i64) == 0i64 { "".to_string() } else { "(".to_string().to_string() + &pat_list(ps.clone())[..] + &")".to_string()[..] };
}

pub fn pat_struct_fields(fs: Vec<(String, Pattern)>) -> String {
    return if (fs.len() as i64) == 0i64 { "".to_string()} else {
        let mut out: String = "{ ".to_string();
        for idx in (0i64..(fs.len() as i64)).into_iter() {
            let f = fs[((idx) as usize)].clone();
            out = out + &(if idx > 0i64 { ", ".to_string() } else { "".to_string() }) + &f.0 + &": ".to_string()[..] + &display_pattern(f.1.clone())[..];
        }
        out + &" }".to_string()[..]
    };
}

pub fn match_block(s: Expr, as_: Vec<(Pattern, MaybeExpr, BlockIR)>) -> String {
    let mut out: String = "match ".to_string().to_string() + &display_expr(s.clone())[..] + &" {".to_string()[..];
    for idx in (0i64..(as_.len() as i64)).into_iter() {
        let arm = as_[((idx) as usize)].clone();
        out = out + &" ".to_string()[..] + &display_pattern(arm.0.clone())[..] + &guard_s(arm.1.clone())[..] + &" => ".to_string()[..] + &block_disp(arm.2.clone())[..];
    }
    return out + &" }".to_string()[..];
}

pub fn stmt_block(ss: Vec<Stmt>) -> String {
    return if (ss.len() as i64) == 0i64 { "{ }".to_string()} else {
        let mut out: String = "{\n".to_string();
        for idx in (0i64..(ss.len() as i64)).into_iter() {
            out = out + &"  ".to_string()[..] + &display_stmt(ss[((idx) as usize)].clone())[..] + &"\n".to_string()[..];
        }
        out + &"}".to_string()[..]
    };
}

pub fn block_disp(b: BlockIR) -> String {
    match b.clone() {
        BlockIR::Block(ss, t) => {
            if (ss.len() as i64) == 0i64 { "{ } [".to_string().to_string() + &display_type(t.clone())[..] + &"]".to_string()[..]} else {
                let mut out: String = "{ [".to_string().to_string() + &display_type(t.clone())[..] + &"]\n".to_string()[..];
                for idx in (0i64..(ss.len() as i64)).into_iter() {
                    out = out + &"  ".to_string()[..] + &display_stmt(ss[((idx) as usize)].clone())[..] + &"\n".to_string()[..];
                }
                out + &"}".to_string()[..]
            }
        }
    }
}

pub fn block_with_ty(ss: Vec<Stmt>, t: IrType) -> String {
    return if (ss.len() as i64) == 0i64 { "{ } [".to_string().to_string() + &display_type(t.clone())[..] + &"]".to_string()[..]} else {
        let mut out: String = "{ [".to_string().to_string() + &display_type(t.clone())[..] + &"]\n".to_string()[..];
        for idx in (0i64..(ss.len() as i64)).into_iter() {
            out = out + &"  ".to_string()[..] + &display_stmt(ss[((idx) as usize)].clone())[..] + &"\n".to_string()[..];
        }
        out + &"}".to_string()[..]
    };
}

pub fn opt_debug(o: MaybeStr) -> String {
    match o.clone() {
        MaybeStr::YesStr(s) => {
            "Some(\"".to_string().to_string() + &s[..] + &"\")".to_string()[..]
        }
        MaybeStr::NoStr => {
            "None".to_string()
        }
    }
}

pub fn stmt_lines(ss: Vec<Stmt>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(ss.len() as i64)).into_iter() {
        out = out + &"  ".to_string()[..] + &display_stmt(ss[((idx) as usize)].clone())[..] + &"\n".to_string()[..];
    }
    return out;
}

pub fn display_item(it: Item) -> String {
    match it.clone() {
        Item::FnDef(n, gs, ps, r, b) => {
            "fn ".to_string().to_string() + &n[..] + &generic_sig(gs.clone())[..] + &param_sig(ps.clone())[..] + &" -> ".to_string()[..] + &display_type(r.clone())[..] + &":\n".to_string()[..] + &stmt_lines(b.clone())[..]
        }
        Item::Const(n, t, v) => {
            "const ".to_string().to_string() + &n[..] + &": ".to_string()[..] + &display_type(t.clone())[..] + &" = ".to_string()[..] + &display_expr(v.clone())[..]
        }
        Item::StructDef(n, gs, fs) => {
            "struct ".to_string().to_string() + &n[..] + &generic_sig(gs.clone())[..] + &" {\n".to_string()[..] + &field_lines(fs.clone())[..] + &"}".to_string()[..]
        }
        Item::EnumDef(n, gs, vs) => {
            "enum ".to_string().to_string() + &n[..] + &generic_sig(gs.clone())[..] + &" {\n".to_string()[..] + &variant_lines(vs.clone())[..] + &"}".to_string()[..]
        }
        Item::TraitDef(n, ss, ms) => {
            "trait ".to_string().to_string() + &n[..] + &super_join(ss.clone())[..] + &" {\n".to_string()[..] + &method_lines(ms.clone())[..] + &"}".to_string()[..]
        }
        Item::DuckDef(n, c) => {
            "duck ".to_string().to_string() + &n[..] + &" { ".to_string()[..] + &c.to_string()[..] + &" methods }".to_string()[..]
        }
        Item::UseStmt(p, a, xs, f) => {
            (if f { ("from ".to_string().to_string() + &dot_join(p.clone())[..] + &" import ".to_string()[..] + &str_join(xs.clone())[..]) } else { ("import ".to_string().to_string() + &dot_join(p.clone())[..]) })
        }
        Item::TypeAlias(n, t) => {
            "type ".to_string().to_string() + &n[..] + &" = ".to_string()[..] + &display_type(t.clone())[..]
        }
        Item::Impl(tr, fty, ms) => {
            let mut out: String = "impl ".to_string().to_string() + &impl_head(tr.clone())[..] + &display_type(fty.clone())[..] + &" {\n".to_string()[..];
            for idx in (0i64..(ms.len() as i64)).into_iter() {
                out = out + &"  fn ".to_string()[..] + &ms[((idx) as usize)].clone() + &" ...\n".to_string()[..];
            }
            out + &"}".to_string()[..]
        }
        Item::CheckerBlock(n, p) => {
            "checker block \'".to_string().to_string() + &n[..] + &"[ps:".to_string()[..] + &opt_debug(p.clone())[..] + &"]".to_string()[..]
        }
        Item::Test(n, b) => {
            "test ".to_string().to_string() + &n[..] + &" ".to_string()[..] + &stmt_block(b.clone())[..]
        }
    }
}

pub fn generic_sig(gs: Vec<String>) -> String {
    return if (gs.len() as i64) == 0i64 { "".to_string() } else { "<".to_string().to_string() + &str_join(gs.clone())[..] + &">".to_string()[..] };
}

pub fn variant_lines(vs: Vec<(String, Vec<IrType>)>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(vs.len() as i64)).into_iter() {
        let v = vs[((idx) as usize)].clone();
        out = out + &"  ".to_string()[..] + &v.0 + &variant_args(v.1.clone())[..] + &"\n".to_string()[..];
    }
    return out;
}

pub fn field_lines(fs: Vec<(String, IrType)>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        let f = fs[((idx) as usize)].clone();
        out = out + &"  ".to_string()[..] + &f.0 + &": ".to_string()[..] + &display_type(f.1.clone())[..] + &"\n".to_string()[..];
    }
    return out;
}

pub fn variant_args(ts: Vec<IrType>) -> String {
    return if (ts.len() as i64) == 0i64 { "".to_string() } else { "(".to_string().to_string() + &type_list(ts.clone())[..] + &")".to_string()[..] };
}

pub fn super_join(ss: Vec<IrType>) -> String {
    return if (ss.len() as i64) == 0i64 { "".to_string()} else {
        let mut out: String = " : ".to_string().to_string() + &display_type(ss[((0i64) as usize)].clone())[..];
        for idx in (1i64..(ss.len() as i64)).into_iter() {
            out = out + &" + ".to_string()[..] + &display_type(ss[((idx) as usize)].clone())[..];
        }
        out
    };
}

pub fn method_lines(ms: Vec<(String, Vec<IrType>, IrType)>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(ms.len() as i64)).into_iter() {
        let m = ms[((idx) as usize)].clone();
        out = out + &"  fn ".to_string()[..] + &m.0 + &"(".to_string()[..] + &type_list(m.1.clone())[..] + &") -> ".to_string()[..] + &display_type(m.2.clone())[..] + &"\n".to_string()[..];
    }
    return out;
}

pub fn param_sig(ps: Vec<(String, IrType, bool, bool, bool)>) -> String {
    return if (ps.len() as i64) == 0i64 { "()".to_string() } else { "(".to_string().to_string() + &param_list(ps.clone())[..] + &")".to_string()[..] };
}

pub fn param_list(ps: Vec<(String, IrType, bool, bool, bool)>) -> String {
    return if (ps.len() as i64) == 0i64 { "".to_string()} else {
        let p0 = ps[((0i64) as usize)].clone();
        param_str(p0.clone()) + &(if (ps.len() as i64) > 1i64 { ", ".to_string().to_string() + &param_list(tail_p(ps.clone()))[..] } else { "".to_string() })
    };
}

pub fn param_str(p: (String, IrType, bool, bool, bool)) -> String {
    return (if p.3 { "mut ".to_string() } else { "".to_string() }) + &(if p.4 { "owned ".to_string() } else { "".to_string() }) + &(if p.2 { "ref ".to_string() } else { "".to_string() }) + &p.0 + &": ".to_string()[..] + &display_type(p.1.clone())[..];
}

pub fn tail_p(ps: Vec<(String, IrType, bool, bool, bool)>) -> Vec<(String, IrType, bool, bool, bool)> {
    let mut out: Vec<(String, IrType, bool, bool, bool)> = Vec::new();
    for idx in (1i64..(ps.len() as i64)).into_iter() {
        out.push(ps[((idx) as usize)].clone());
    }
    return out;
}

pub fn impl_head(tr: MaybeIrType) -> String {
    match tr.clone() {
        MaybeIrType::YesTy(t) => {
            display_type(t.clone()) + &" for ".to_string()[..]
        }
        MaybeIrType::NoTy => {
            "".to_string()
        }
    }
}

pub fn dot_join(xs: Vec<String>) -> String {
    return if (xs.len() as i64) == 0i64 { "".to_string()} else {
        let mut out = xs[((0i64) as usize)].clone();
        for idx in (1i64..(xs.len() as i64)).into_iter() {
            out = out + &".".to_string()[..] + &xs[((idx) as usize)].clone();
        }
        out
    };
}

pub fn display_module(m: IrModule) -> String {
    let mut out: String = ";; LZIR v".to_string().to_string() + &m.version.to_string()[..] + &" \u{2014} module \'".to_string()[..] + &m.name + &"\'\n".to_string()[..];
    out = out + &";; ".to_string()[..] + &(m.items.len() as i64).to_string()[..] + &" items\n".to_string()[..];
    if (m.prelude.len() as i64) > 0i64 {
        out = out + &";; prelude: ".to_string()[..] + &str_join(m.prelude.clone())[..] + &"\n".to_string()[..];
    } else { ()};
    out = out + &"\n".to_string()[..];
    for idx in (0i64..(m.items.len() as i64)).into_iter() {
        out = out + &display_item(m.items[((idx) as usize)].clone())[..] + &"\n\n".to_string()[..];
    }
    return out;
}

pub fn str_join(xs: Vec<String>) -> String {
    return if (xs.len() as i64) == 0i64 { "".to_string()} else {
        let mut out = xs[((0i64) as usize)].clone();
        for idx in (1i64..(xs.len() as i64)).into_iter() {
            out = out + &", ".to_string()[..] + &xs[((idx) as usize)].clone();
        }
        out
    };
}

pub fn rust_type(t: IrType) -> String {
    match t.clone() {
        IrType::Int => {
            "i64".to_string()
        }
        IrType::F64 => {
            "f64".to_string()
        }
        IrType::Str => {
            "String".to_string()
        }
        IrType::Bool => {
            "bool".to_string()
        }
        IrType::Unit => {
            "()".to_string()
        }
        IrType::Never => {
            "!".to_string()
        }
        IrType::Any => {
            "i64".to_string()
        }
        IrType::Self_ => {
            "Self".to_string()
        }
        IrType::Generic(n) => {
            n
        }
        IrType::Named(p, a) => {
            let a = *a;
            named_rust_type(p.clone(), a.clone())
        }
        IrType::Opt(x) => {
            let x = *x;
            "Option<".to_string().to_string() + &rust_type(x.clone())[..] + &">".to_string()[..]
        }
        IrType::Res(o, e) => {
            let o = *o;
            let e = *e;
            "Result<".to_string().to_string() + &rust_type(o.clone())[..] + &", ".to_string()[..] + &rust_type(e.clone())[..] + &">".to_string()[..]
        }
        IrType::Tuple(es) => {
            let es = *es;
            "(".to_string().to_string() + &type_rust_list(es.clone())[..] + &")".to_string()[..]
        }
        IrType::FnType(ps, r) => {
            let ps = *ps;
            let r = *r;
            "impl Fn(".to_string().to_string() + &type_rust_list(ps.clone())[..] + &") -> ".to_string()[..] + &rust_type(r.clone())[..]
        }
        IrType::Ref(x) => {
            let x = *x;
            "&".to_string().to_string() + &rust_type(x.clone())[..]
        }
        IrType::MutRef(x) => {
            let x = *x;
            "&mut ".to_string().to_string() + &rust_type(x.clone())[..]
        }
        IrType::Duck(fs) => {
            let fs = *fs;
            "()".to_string()
        }
    }
}

pub fn named_rust_type(p: String, a: Vec<IrType>) -> String {
    let m: String = named_map(p.clone());
    return if (a.len() as i64) > 0i64 { m + &"<".to_string()[..] + &type_rust_list(a.clone())[..] + &">".to_string()[..] } else { named_default(m.clone()) };
}

pub fn named_map(p: String) -> String {
    return if p == "int".to_string() || p == "float".to_string() || p == "f64".to_string() || p == "str".to_string() || p == "bool".to_string() || p == "List".to_string() || p == "Dict".to_string() || p == "Set".to_string() || p == "Iter".to_string() || p == "Future".to_string() || p == "Tokens".to_string() { rust_prim_name(p.clone()) } else { p };
}

pub fn rust_prim_name(p: String) -> String {
    return if p == "int".to_string() { "i64".to_string() } else { if p == "float".to_string() || p == "f64".to_string() { "f64".to_string() } else { if p == "str".to_string() { "String".to_string() } else { if p == "bool".to_string() { "bool".to_string() } else { if p == "List".to_string() { "Vec".to_string() } else { if p == "Dict".to_string() { "HashMap".to_string() } else { if p == "Set".to_string() { "HashSet".to_string() } else { if p == "Iter".to_string() { "Vec".to_string() } else { if p == "Future".to_string() { "std::future::Future<Output = i64>".to_string() } else { if p == "Tokens".to_string() { "String".to_string() } else { p } } } } } } } } } };
}

pub fn named_default(m: String) -> String {
    return if m == "Vec".to_string() || m == "List".to_string() { "Vec<i64>".to_string() } else { if m == "HashMap".to_string() || m == "Dict".to_string() { "HashMap<i64, i64>".to_string() } else { if m == "HashSet".to_string() || m == "Set".to_string() { "HashSet<i64>".to_string() } else { if m == "Option".to_string() { "Option<i64>".to_string() } else { if m == "Result".to_string() { "Result<i64, i64>".to_string() } else { m } } } } };
}

pub fn type_rust_list(ts: Vec<IrType>) -> String {
    return if (ts.len() as i64) == 0i64 { "".to_string()} else {
        let head: String = rust_type(ts[((0i64) as usize)].clone());
        if (ts.len() as i64) > 1i64 { head + &", ".to_string()[..] + &type_rust_list(tr_tail(ts.clone()))[..] } else { head }
    };
}

pub fn tr_tail(ts: Vec<IrType>) -> Vec<IrType> {
    let mut out: Vec<IrType> = Vec::new();
    for idx in (1i64..(ts.len() as i64)).into_iter() {
        out.push(ts[((idx) as usize)].clone());
    }
    return out;
}

pub fn base_is_dict(b: Expr) -> bool {
    match b.clone() {
        Expr::Var(n, t) => {
            is_dict_ty(t.clone())
        }
        Expr::FieldAccess(bb, f, t) => {
            let bb = *bb;
            is_dict_ty(t.clone())
        }
        _ => {
            false
        }
    }
}

pub fn base_is_set(b: Expr) -> bool {
    match b.clone() {
        Expr::Var(n, t) => {
            is_set_ty(t.clone())
        }
        Expr::FieldAccess(bb, f, t) => {
            let bb = *bb;
            is_set_ty(t.clone())
        }
        _ => {
            false
        }
    }
}

pub fn is_set_ty(t: IrType) -> bool {
    match t.clone() {
        IrType::Named(p, a) => {
            let a = *a;
            p == "Set".to_string() || p == "HashSet".to_string()
        }
        _ => {
            false
        }
    }
}

pub fn strip_ts_args(es: Vec<Expr>) -> String {
    return if (es.len() as i64) == 0i64 { "".to_string()} else {
        let head: String = strip_ts(es[((0i64) as usize)].clone());
        if (es.len() as i64) > 1i64 { head + &", ".to_string()[..] + &strip_ts_args(st_tail(es.clone()))[..] } else { head }
    };
}

pub fn strip_ts(e: Expr) -> String {
    match e.clone() {
        Expr::LitStr(sv, tv) => {
            "\"".to_string().to_string() + &esc_rust(sv.clone())[..] + &"\"".to_string()[..]
        }
        _ => {
            "&".to_string().to_string() + &gen_expr(e.clone())[..]
        }
    }
}

pub fn st_tail(es: Vec<Expr>) -> Vec<Expr> {
    let mut out: Vec<Expr> = Vec::new();
    for idx in (1i64..(es.len() as i64)).into_iter() {
        out.push(es[((idx) as usize)].clone());
    }
    return out;
}

pub fn is_dict_ty(t: IrType) -> bool {
    match t.clone() {
        IrType::Named(p, a) => {
            let a = *a;
            p == "Dict".to_string() || p == "HashMap".to_string()
        }
        _ => {
            false
        }
    }
}

pub fn gen_expr(e: Expr) -> String {
    match e.clone() {
        Expr::LitInt(n, t) => {
            n.to_string() + &"i64".to_string()[..]
        }
        Expr::LitF64(n, t) => {
            fmt_f64(n)
        }
        Expr::LitStr(s, t) => {
            "\"".to_string().to_string() + &esc_rust(s.clone())[..] + &"\".to_string()".to_string()[..]
        }
        Expr::LitFStr(s, t) => {
            gen_fstring(s.clone())
        }
        Expr::LitBool(b, t) => {
            b.to_string()
        }
        Expr::LitUnit(t) => {
            "()".to_string()
        }
        Expr::LitNone(t) => {
            "None".to_string()
        }
        Expr::Var(n, t) => {
            n
        }
        Expr::BinOp(o, l, r, t) => {
            let l = *l;
            let r = *r;
            gen_expr(l.clone()) + &" ".to_string()[..] + &o[..] + &" ".to_string()[..] + &gen_expr(r.clone())[..]
        }
        Expr::UnOp(o, p, t) => {
            let p = *p;
            o + &gen_expr(p.clone())[..]
        }
        Expr::Call(c, a, t) => {
            let c = *c;
            let a = *a;
            gen_call(c.clone(), a.clone())
        }
        Expr::MethodCall(r, m, a, t) => {
            let r = *r;
            let a = *a;
            if m == "length".to_string() { "(".to_string().to_string() + &gen_expr(r.clone())[..] + &".len() as i64)".to_string()[..] } else { if m == "contains".to_string() && base_is_set(r.clone()) { gen_expr(r.clone()) + &".contains(".to_string()[..] + &strip_ts_args(a.clone())[..] + &")".to_string()[..] } else { gen_expr(r.clone()) + &".".to_string()[..] + &m[..] + &"(".to_string()[..] + &expr_cs_list(a.clone())[..] + &")".to_string()[..] } }
        }
        Expr::FieldAccess(b, f, t) => {
            let b = *b;
            gen_expr(b.clone()) + &".".to_string()[..] + &f[..]
        }
        Expr::IndexGet(b, k, t) => {
            let b = *b;
            let k = *k;
            if base_is_dict(b.clone()) { "(".to_string().to_string() + &gen_expr(b.clone())[..] + &").get(&".to_string()[..] + &gen_expr(k.clone())[..] + &").cloned().unwrap()".to_string()[..] } else { gen_expr(b.clone()) + &"[((".to_string()[..] + &gen_expr(k.clone())[..] + &") as usize)]".to_string()[..] }
        }
        Expr::IndexSet(b, k, v, t) => {
            let b = *b;
            let k = *k;
            let v = *v;
            if base_is_dict(b.clone()) { gen_expr(b.clone()) + &".insert(".to_string()[..] + &gen_expr(k.clone())[..] + &", ".to_string()[..] + &gen_expr(v.clone())[..] + &");".to_string()[..] } else { gen_expr(b.clone()) + &"[((".to_string()[..] + &gen_expr(k.clone())[..] + &") as usize)] = ".to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string()[..] }
        }
        Expr::StructCtor(n, fs, t) => {
            let fs = *fs;
            if n == "Range".to_string() { gen_range_ctor(fs.clone()) } else { if n == "Dict".to_string() { gen_dict_ctor(fs.clone()) } else { n + &" { ".to_string()[..] + &ctor_fields(fs.clone())[..] + &" }".to_string()[..] } }
        }
        Expr::ListLit(xs, t) => {
            let xs = *xs;
            if (xs.len() as i64) == 0i64 { "Vec::new()".to_string() } else { "vec![".to_string().to_string() + &expr_cs_list(xs.clone())[..] + &"]".to_string()[..] }
        }
        Expr::IfExpr(c, th, el, t) => {
            let c = *c;
            let th = *th;
            let el = *el;
            "if ".to_string().to_string() + &gen_expr(c.clone())[..] + &" { ".to_string()[..] + &gen_expr(th.clone())[..] + &" } else { ".to_string()[..] + &gen_expr(el.clone())[..] + &" }".to_string()[..]
        }
        Expr::TupleLit(es, t) => {
            let es = *es;
            "(".to_string().to_string() + &expr_cs_list(es.clone())[..] + &")".to_string()[..]
        }
        _ => {
            "/* TODO ".to_string().to_string() + &display_expr(e.clone())[..] + &" */".to_string()[..]
        }
    }
}

pub fn gen_range_ctor(fs: Vec<(String, Expr)>) -> String {
    let mut start_s: String = "".to_string();
    let mut end_s: String = "".to_string();
    let mut incl = false;
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        let f = fs[((idx) as usize)].clone();
        if f.0 == "start".to_string() {
            start_s = gen_expr(f.1.clone());
        } else { if f.0 == "end".to_string() {
            end_s = gen_expr(f.1.clone());
        } else { if f.0 == "inclusive".to_string() {
            match f.1 {
                Expr::LitBool(bv, tv) => {
                    incl = bv;
                }
                _ => {
                    incl = incl;
                }
            }
        } else { ()}}};
    }
    return start_s + &(if incl { "..=".to_string() } else { "..".to_string() }) + &end_s[..];
}

pub fn gen_dict_ctor(fs: Vec<(String, Expr)>) -> String {
    return if (fs.len() as i64) == 0i64 { "std::collections::HashMap::new()".to_string()} else {
        let mut pairs: String = "".to_string();
        let mut i: i64 = 0i64;
        while i + 1i64 < (fs.len() as i64) {
            let k = fs[((i) as usize)].clone();
            let v = fs[((i + 1i64) as usize)].clone();
            pairs = pairs + &("(".to_string().to_string() + &gen_expr(k.1.clone())[..] + &", ".to_string()[..] + &gen_expr(v.1.clone())[..] + &")".to_string()[..]);
            if i + 2i64 < (fs.len() as i64) {
                pairs = pairs + &", ".to_string()[..];
            } else { ()};
            i = i + 2i64;
        }
        "std::collections::HashMap::from([".to_string().to_string() + &pairs[..] + &"])".to_string()[..]
    };
}

pub fn gen_call(c: Expr, a: Vec<Expr>) -> String {
    match c.clone() {
        Expr::Var(n, t) => {
            if n == "print".to_string() { "println!(".to_string().to_string() + &print_fmt(a.clone())[..] + &expr_cs_list(a.clone())[..] + &")".to_string()[..] } else { if n == "len".to_string() { "(".to_string().to_string() + &expr_cs_list(a.clone())[..] + &".len() as i64)".to_string()[..] } else { if n == "set!".to_string() { "std::collections::HashSet::from([".to_string().to_string() + &expr_cs_list(a.clone())[..] + &"])".to_string()[..] } else { n + &"(".to_string()[..] + &expr_cs_list(a.clone())[..] + &")".to_string()[..] } } }
        }
        _ => {
            gen_expr(c.clone()) + &"(".to_string()[..] + &expr_cs_list(a.clone())[..] + &")".to_string()[..]
        }
    }
}

pub fn print_fmt(as_: Vec<Expr>) -> String {
    return if (as_.len() as i64) == 0i64 { "\"\"".to_string()} else {
        let mut out: String = "\"{:?}".to_string();
        for idx in (1i64..(as_.len() as i64)).into_iter() {
            out = out + &" {:?}".to_string()[..];
        }
        out + &"\", ".to_string()[..]
    };
}

pub fn expr_cs_list(es: Vec<Expr>) -> String {
    return if (es.len() as i64) == 0i64 { "".to_string()} else {
        let head: String = gen_expr(es[((0i64) as usize)].clone());
        if (es.len() as i64) > 1i64 { head + &", ".to_string()[..] + &expr_cs_list(ec_tail(es.clone()))[..] } else { head }
    };
}

pub fn ec_tail(es: Vec<Expr>) -> Vec<Expr> {
    let mut out: Vec<Expr> = Vec::new();
    for idx in (1i64..(es.len() as i64)).into_iter() {
        out.push(es[((idx) as usize)].clone());
    }
    return out;
}

pub fn ctor_fields(fs: Vec<(String, Expr)>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        let f = fs[((idx) as usize)].clone();
        out = out + &(if idx > 0i64 { ", ".to_string() } else { "".to_string() }) + &f.0 + &": ".to_string()[..] + &gen_expr(f.1.clone())[..];
    }
    return out;
}

pub fn gen_stmt(s: Stmt, is_tail: bool, is_main: bool) -> String {
    match s.clone() {
        Stmt::Let(n, t, v, m, r) => {
            gen_let(n.clone(), t.clone(), v.clone(), m, r)
        }
        Stmt::Assign(tg, v) => {
            match tg.clone() {
                Expr::IndexGet(b, k, t) => {
                    let b = *b;
                    let k = *k;
                    if base_is_dict(b.clone()) { gen_expr(b.clone()) + &".insert(".to_string()[..] + &gen_expr(k.clone())[..] + &", ".to_string()[..] + &gen_expr(v.clone())[..] + &");".to_string()[..] } else { gen_expr(tg.clone()) + &" = ".to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string()[..] }
                }
                _ => {
                    gen_expr(tg.clone()) + &" = ".to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string()[..]
                }
            }
        }
        Stmt::Return(v) => {
            match v.clone() {
                MaybeExpr::YesExpr(inner) => {
                    "return ".to_string().to_string() + &gen_expr(inner.clone())[..] + &";".to_string()[..]
                }
                MaybeExpr::NoExpr => {
                    "return;".to_string()
                }
            }
        }
        Stmt::ExprStmt(e) => {
            let ret_prefix: String = if (is_tail && !is_main) { "return ".to_string() } else { "".to_string() };
            ret_prefix + &gen_expr(e.clone())[..] + &";".to_string()[..]
        }
        Stmt::While(c, g, b, eb) => {
            gen_while(c.clone(), b.clone())
        }
        _ => {
            "// TODO stmt".to_string()
        }
    }
}

pub fn gen_while(c: Expr, b: BlockIR) -> String {
    let inf: bool = is_true_cond(c.clone());
    return if inf && block_is_pass_only(b.clone()) { "loop {\n        unimplemented!()\n    }".to_string() } else { "// TODO while".to_string() };
}

pub fn is_true_cond(c: Expr) -> bool {
    match c.clone() {
        Expr::LitBool(bv, tv) => {
            bv
        }
        _ => {
            false
        }
    }
}

pub fn block_is_pass_only(b: BlockIR) -> bool {
    match b.clone() {
        BlockIR::Block(ss, t) => {
            if (ss.len() as i64) == 1i64 {
                match ss[((0i64) as usize)].clone() {
                    Stmt::Pass => {
                        true
                    }
                    _ => {
                        false
                    }
                }
            } else { false}
        }
    }
}

pub fn gen_let(n: String, t: IrType, v: Expr, m: bool, r: bool) -> String {
    let mut_kw: String = if m { "mut ".to_string() } else { "".to_string() };
    let ref_kw: String = if r { "ref ".to_string() } else { "".to_string() };
    let ty_s: String = rust_type(t.clone());
    let empty_c: bool = is_empty_container(v.clone());
    let force: bool = empty_c && is_dict_or_set_ty(t.clone());
    return if skip_ty_ann(t.clone()) && !force { "let ".to_string().to_string() + &mut_kw[..] + &ref_kw[..] + &n[..] + &" = ".to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string()[..] } else { "let ".to_string().to_string() + &mut_kw[..] + &ref_kw[..] + &n[..] + &": ".to_string()[..] + &ty_s[..] + &" = ".to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string()[..] };
}

pub fn is_empty_container(v: Expr) -> bool {
    match v.clone() {
        Expr::ListLit(xs, t) => {
            let xs = *xs;
            (xs.len() as i64) == 0i64
        }
        Expr::StructCtor(nn, fs, t) => {
            let fs = *fs;
            nn == "Dict".to_string() && (fs.len() as i64) == 0i64
        }
        _ => {
            false
        }
    }
}

pub fn is_dict_or_set_ty(t: IrType) -> bool {
    match t.clone() {
        IrType::Named(p, a) => {
            let a = *a;
            p == "Dict".to_string() || p == "Set".to_string()
        }
        _ => {
            false
        }
    }
}

pub fn skip_ty_ann(t: IrType) -> bool {
    match t.clone() {
        IrType::Any => {
            true
        }
        IrType::Unit => {
            true
        }
        IrType::Duck(fs) => {
            let fs = *fs;
            true
        }
        IrType::Generic(nn) => {
            true
        }
        IrType::FnType(ps, rr) => {
            let ps = *ps;
            let rr = *rr;
            true
        }
        IrType::Named(p, a) => {
            let a = *a;
            p == "Dict".to_string() || p == "Set".to_string() || p == "Range".to_string() || p == "Nil".to_string() || args_is_empty_or_generic(a.clone())
        }
        IrType::Ref(x) => {
            let x = *x;
            match x.clone() {
                IrType::Generic(nn) => {
                    true
                }
                _ => {
                    false
                }
            }
        }
        IrType::MutRef(x) => {
            let x = *x;
            match x.clone() {
                IrType::Generic(nn) => {
                    true
                }
                _ => {
                    false
                }
            }
        }
        _ => {
            false
        }
    }
}

pub fn args_is_empty_or_generic(a: Vec<IrType>) -> bool {
    return if (a.len() as i64) == 0i64 { true} else {
        let mut has_generic = false;
        for idx in (0i64..(a.len() as i64)).into_iter() {
            let ai = a[((idx) as usize)].clone();
            match ai.clone() {
                IrType::Generic(nn) => {
                    has_generic = true;
                }
                IrType::Any => {
                    has_generic = true;
                }
                _ => {
                    has_generic = has_generic;
                }
            }
        }
        has_generic
    };
}

pub fn fmt_f64(n: f64) -> String {
    let s = n.to_string();
    return if has_dot_or_e(s.clone()) { s + &"f64".to_string()[..] } else { s + &".0f64".to_string()[..] };
}

pub fn has_dot_or_e(s: String) -> bool {
    let len = (s.len() as i64);
    let mut i: i64 = 0i64;
    let mut found = false;
    while i < len {
        let c = s[(i as usize)..((i + 1i64) as usize)].to_string();
        if c == ".".to_string() || c == "e".to_string() {
            found = true;
        } else { ()};
        i = i + 1i64;
    }
    return found;
}

pub fn gen_fstring(s: String) -> String {
    let mut fmt: String = "".to_string();
    let mut args: Vec<String> = Vec::new();
    let n = (s.len() as i64);
    let mut i: i64 = 0i64;
    while i < n {
        let c = s[(i as usize)..((i + 1i64) as usize)].to_string();
        if c == "{".to_string() { if i + 1i64 < n {
            let c2 = s[((i + 1i64) as usize)..((i + 2i64) as usize)].to_string();
            if c2 == "{".to_string() {
                fmt = fmt + &"{{".to_string()[..];
                i = i + 2i64;
            } else {
                let mut j: i64 = i + 1i64;
                while j < n {
                    let cj = s[(j as usize)..((j + 1i64) as usize)].to_string();
                    if cj == "}".to_string() {
                        break;
                    } else { ()};
                    j = j + 1i64;
                }
                let expr = s[((i + 1i64) as usize)..(j as usize)].to_string();
                fmt = fmt + &"{:?}".to_string()[..];
                args.push(expr);
                i = j + 1i64;
            }
        } else {
            fmt = fmt + &"{".to_string()[..];
            i = i + 1i64;
        }} else { if c == "}".to_string() { if i + 1i64 < n {
            let c2 = s[((i + 1i64) as usize)..((i + 2i64) as usize)].to_string();
            if c2 == "}".to_string() {
                fmt = fmt + &"}}".to_string()[..];
                i = i + 2i64;
            } else {
                fmt = fmt + &"}".to_string()[..];
                i = i + 1i64;
            }
        } else {
            fmt = fmt + &"}".to_string()[..];
            i = i + 1i64;
        }} else {
            fmt = fmt + &c[..];
            i = i + 1i64;
        }}
    }
    let fmt_q: String = "\"".to_string().to_string() + &esc_quote_only(fmt.clone())[..] + &"\"".to_string()[..];
    return if (args.len() as i64) == 0i64 { fmt_q + &".to_string()".to_string()[..] } else { "format!(".to_string().to_string() + &fmt_q[..] + &", ".to_string()[..] + &args_join(args.clone())[..] + &")".to_string()[..] };
}

pub fn args_join(xs: Vec<String>) -> String {
    return if (xs.len() as i64) == 0i64 { "".to_string()} else {
        let head = xs[((0i64) as usize)].clone();
        if (xs.len() as i64) > 1i64 { head + &", ".to_string()[..] + &args_join(aj_tail(xs.clone()))[..] } else { head }
    };
}

pub fn aj_tail(xs: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for idx in (1i64..(xs.len() as i64)).into_iter() {
        out.push(xs[((idx) as usize)].clone());
    }
    return out;
}

pub fn esc_quote_only(s: String) -> String {
    let len = (s.len() as i64);
    let mut out: String = "".to_string();
    let mut i: i64 = 0i64;
    while i < len {
        let c = s[(i as usize)..((i + 1i64) as usize)].to_string();
        if c == "\"".to_string() {
            out = out + &"\\\"".to_string()[..];
        } else {
            out = out + &c[..];
        };
        i = i + 1i64;
    }
    return out;
}

pub fn esc_rust(s: String) -> String {
    let len = (s.len() as i64);
    let mut out: String = "".to_string();
    let mut i: i64 = 0i64;
    while i < len {
        let c = s[(i as usize)..((i + 1i64) as usize)].to_string();
        if c == "\\".to_string() {
            out = out + &"\\\\".to_string()[..];
        } else { if c == "\"".to_string() {
            out = out + &"\\\"".to_string()[..];
        } else { if c == "\n".to_string() {
            out = out + &"\\n".to_string()[..];
        } else { if c == "\t".to_string() {
            out = out + &"\\t".to_string()[..];
        } else { if c == "\r".to_string() {
            out = out + &"\\r".to_string()[..];
        } else {
            out = out + &c[..];
        }}}}};
        i = i + 1i64;
    }
    return out;
}

pub fn gen_item(i: Item) -> String {
    match i.clone() {
        Item::FnDef(n, gs, ps, r, b) => {
            let is_main: bool = (n == "main".to_string());
            let ret_s: String = if (is_main && r_eq_unit(r.clone())) { "".to_string() } else { (" -> ".to_string().to_string() + &rust_type(r.clone())[..]) };
            "pub fn ".to_string().to_string() + &n[..] + &generic_sig(gs.clone())[..] + &"(".to_string()[..] + &fn_param_list(ps.clone())[..] + &")".to_string()[..] + &ret_s[..] + &" {\n".to_string()[..] + &gen_body(b.clone(), is_main)[..] + &"}".to_string()[..]
        }
        Item::StructDef(n, gs, fs) => {
            "#[derive(Debug, Clone)]\npub struct ".to_string().to_string() + &n[..] + &generic_sig(gs.clone())[..] + &" {\n".to_string()[..] + &gen_struct_fields(fs.clone())[..] + &"}".to_string()[..]
        }
        Item::EnumDef(n, gs, vs) => {
            "#[derive(Debug, Clone, PartialEq)]\npub enum ".to_string().to_string() + &n[..] + &generic_sig(gs.clone())[..] + &" {\n".to_string()[..] + &gen_enum_variants(vs.clone())[..] + &"}".to_string()[..]
        }
        Item::Const(n, t, v) => {
            gen_const_item(n.clone(), t.clone(), v.clone())
        }
        _ => {
            "// TODO Item ".to_string().to_string() + &display_item(i.clone())[..]
        }
    }
}

pub fn r_eq_unit(t: IrType) -> bool {
    match t.clone() {
        IrType::Unit => {
            true
        }
        _ => {
            false
        }
    }
}

pub fn fn_param_list(ps: Vec<(String, IrType, bool, bool, bool)>) -> String {
    return if (ps.len() as i64) == 0i64 { "".to_string()} else {
        let p0 = ps[((0i64) as usize)].clone();
        fn_param_str(p0.clone()) + &(if (ps.len() as i64) > 1i64 { ", ".to_string().to_string() + &fn_param_list(pl_tail(ps.clone()))[..] } else { "".to_string() })
    };
}

pub fn fn_param_str(p: (String, IrType, bool, bool, bool)) -> String {
    return p.0 + &": ".to_string()[..] + &rust_type(p.1.clone())[..];
}

pub fn pl_tail(ps: Vec<(String, IrType, bool, bool, bool)>) -> Vec<(String, IrType, bool, bool, bool)> {
    let mut out: Vec<(String, IrType, bool, bool, bool)> = Vec::new();
    for idx in (1i64..(ps.len() as i64)).into_iter() {
        out.push(ps[((idx) as usize)].clone());
    }
    return out;
}

pub fn gen_body(ss: Vec<Stmt>, is_main: bool) -> String {
    let n: i64 = (ss.len() as i64);
    return if n == 0i64 { "    loop {\n        unimplemented!()\n    }\n".to_string()} else {
        let mut out: String = "".to_string();
        for idx in (0i64..n).into_iter() {
            let is_tail: bool = (idx == n - 1i64);
            out = out + &"    ".to_string()[..] + &gen_stmt(ss[((idx) as usize)].clone(), is_tail, is_main)[..] + &"\n".to_string()[..];
        }
        out
    };
}

pub fn gen_struct_fields(fs: Vec<(String, IrType)>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        let f = fs[((idx) as usize)].clone();
        out = out + &"    pub ".to_string()[..] + &f.0 + &": ".to_string()[..] + &rust_type(f.1.clone())[..] + &",\n".to_string()[..];
    }
    return out;
}

pub fn gen_enum_variants(vs: Vec<(String, Vec<IrType>)>) -> String {
    let mut out: String = "".to_string();
    for idx in (0i64..(vs.len() as i64)).into_iter() {
        let v = vs[((idx) as usize)].clone();
        out = out + &"    ".to_string()[..] + &v.0 + &gen_enum_variant_args(v.1.clone())[..] + &",\n".to_string()[..];
    }
    return out;
}

pub fn gen_enum_variant_args(ts: Vec<IrType>) -> String {
    return if (ts.len() as i64) == 0i64 { "".to_string() } else { "(".to_string().to_string() + &type_rust_list(ts.clone())[..] + &")".to_string()[..] };
}

pub fn gen_const_item(n: String, t: IrType, v: Expr) -> String {
    match t.clone() {
        IrType::Str => {
            match v.clone() {
                Expr::LitStr(sv, tv) => {
                    "const ".to_string().to_string() + &n[..] + &": &str = \"".to_string()[..] + &esc_rust(sv.clone())[..] + &"\";".to_string()[..]
                }
                _ => {
                    "const ".to_string().to_string() + &n[..] + &": String = ".to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string()[..]
                }
            }
        }
        _ => {
            "const ".to_string().to_string() + &n[..] + &": ".to_string()[..] + &rust_type(t.clone())[..] + &" = ".to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string()[..]
        }
    }
}

pub fn is_magic_name(n: String) -> bool {
    return n == "__name__".to_string() || n == "__file__".to_string() || n == "__package__".to_string() || n == "__path__".to_string() || n == "__doc__".to_string() || n == "__is_macro__".to_string();
}

pub fn gen_magic_const(n: String, t: IrType, v: Expr) -> String {
    return if n == "__is_macro__".to_string() { "const ".to_string().to_string() + &n[..] + &": bool = ".to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string()[..] } else { "const ".to_string().to_string() + &n[..] + &": &str = ".to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string()[..] };
}

pub fn codegen_module(m: IrModule) -> String {
    let mut out: String = module_header();
    for idx in (0i64..(m.items.len() as i64)).into_iter() {
        out = out + &gen_item(m.items[((idx) as usize)].clone())[..] + &"\n\n".to_string()[..];
    }
    return out[(0i64 as usize)..(((out.len() as i64) - 2i64) as usize)].to_string().clone() + &"\n".to_string()[..];
}

pub fn module_header() -> String {
    return "\n#[allow(unused_imports)]\n#[allow(unused_variables)]\n#[allow(dead_code)]\n#[allow(non_snake_case)]\n\nuse std::collections::{HashMap, HashSet};\nuse std::any::Any;\nuse std::rc::Rc;\nuse std::sync::Arc;\nuse std::fmt::Debug;\nuse std::fmt::Display;\n\nuse lz_builtins::*;\n\n".to_string();
}

const __name__: &str = "main";

const __file__: &str = "E:\\IDEProjects\\AI\\lang-zone\\src\\ir\\lz_codegen_lib.lz";

const __package__: &str = "ir";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\src\\ir";

const __doc__: &str = "";

const __is_macro__: bool = false;

pub fn main() {
    // auto-generated: LZ module has no main entry point
}

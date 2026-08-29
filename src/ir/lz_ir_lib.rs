
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

const __name__: &str = "main";

const __file__: &str = "E:\\IDEProjects\\AI\\lang-zone\\src\\ir\\lz_ir_lib.lz";

const __package__: &str = "ir";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\src\\ir";

const __doc__: &str = "";

const __is_macro__: bool = false;

pub fn main() {
    // auto-generated: LZ module has no main entry point
}

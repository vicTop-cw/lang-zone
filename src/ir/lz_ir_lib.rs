
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
pub enum IrType {
    Int,
    F64,
    Str,
    Bool,
    Unit,
    Never,
    Any,
    Self_,
    Named {
        path: String,
        args: Box<Vec<IrType>>,
    },
    Opt {
        inner: Box<IrType>,
    },
    Res {
        ok: Box<IrType>,
        err: Box<IrType>,
    },
    Tuple {
        elems: Box<Vec<IrType>>,
    },
    FnType {
        params: Box<Vec<IrType>>,
        ret: Box<IrType>,
    },
    Ref {
        inner: Box<IrType>,
    },
    MutRef {
        inner: Box<IrType>,
    },
    Generic {
        name: String,
    },
    Duck {
        fields: Box<Vec<(String, IrType)>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    LitInt {
        v: i64,
        ty: IrType,
    },
    LitF64 {
        v: f64,
        ty: IrType,
    },
    LitStr {
        s: String,
        ty: IrType,
    },
    LitFStr {
        s: String,
        ty: IrType,
    },
    LitBool {
        b: bool,
        ty: IrType,
    },
    LitUnit {
        ty: IrType,
    },
    LitNone {
        ty: IrType,
    },
    Var {
        name: String,
        ty: IrType,
    },
    Call {
        callee: Box<Expr>,
        args: Box<Vec<Expr>>,
        ty: IrType,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Box<Vec<Expr>>,
        ty: IrType,
    },
    FieldAccess {
        base: Box<Expr>,
        field: String,
        ty: IrType,
    },
    IndexGet {
        base: Box<Expr>,
        key: Box<Expr>,
        ty: IrType,
    },
    IndexSet {
        base: Box<Expr>,
        key: Box<Expr>,
        value: Box<Expr>,
        ty: IrType,
    },
    BinOp {
        op: String,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        ty: IrType,
    },
    UnOp {
        op: String,
        operand: Box<Expr>,
        ty: IrType,
    },
    StructCtor {
        name: String,
        fields: Box<Vec<(String, Expr)>>,
        ty: IrType,
    },
    EnumCtor {
        enum_name: String,
        variant: String,
        args: Box<Vec<Expr>>,
        ty: IrType,
    },
    Cast {
        inner: Box<Expr>,
        target: IrType,
        ty: IrType,
    },
    MagicCall {
        magic: String,
        args: Box<Vec<Expr>>,
        ty: IrType,
    },
    IfExpr {
        cond: Box<Expr>,
        then: Box<Expr>,
        els: Box<Expr>,
        ty: IrType,
    },
    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
        ty: IrType,
    },
    Pipe {
        receiver: Box<Expr>,
        callee: Box<Expr>,
        args: Box<Vec<Expr>>,
        ty: IrType,
    },
    TupleLit {
        elems: Box<Vec<Expr>>,
        ty: IrType,
    },
    ListLit {
        items: Box<Vec<Expr>>,
        ty: IrType,
    },
    BlockExpr {
        stmts: Vec<Stmt>,
        ty: IrType,
    },
    GenExpr {
        yield_of: Box<Expr>,
        ty: IrType,
    },
    Paren {
        inner: Box<Expr>,
        ty: IrType,
    },
    Range {
        end: Box<Expr>,
        inclusive: bool,
        ty: IrType,
    },
    Dict {
        pairs: Box<Vec<(Expr, Expr)>>,
        ty: IrType,
    },
    AssignExpr {
        target: Box<Expr>,
        value: Box<Expr>,
        ty: IrType,
    },
    ImplicitConvert {
        source: Box<Expr>,
        target_ty: IrType,
        ty: IrType,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockIR {
    Block {
        stmts: Vec<Stmt>,
        ty: IrType,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaybeExpr {
    NoExpr,
    YesExpr {
        value: Expr,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaybeBlock {
    NoBlock,
    YesBlock {
        value: BlockIR,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaybeStr {
    NoStr,
    YesStr {
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaybeIrType {
    NoTy,
    YesTy {
        value: IrType,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaybePattern {
    NoPat,
    YesPat {
        value: Pattern,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        name: String,
        ty: IrType,
        value: Expr,
        is_mut: bool,
        is_ref: bool,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    Return {
        value: MaybeExpr,
    },
    ExprStmt {
        expr: Expr,
    },
    If {
        cond: Expr,
        then: BlockIR,
        els: MaybeBlock,
    },
    For {
        var: String,
        iter: Expr,
        guard_e: MaybeExpr,
        body: BlockIR,
        else_body: MaybeBlock,
    },
    While {
        cond: Expr,
        guard_e: MaybeExpr,
        body: BlockIR,
        else_body: MaybeBlock,
    },
    Block {
        stmts: Box<Vec<Stmt>>,
    },
    Match {
        scrutinee: Expr,
        arms: Vec<(Pattern, MaybeExpr, BlockIR)>,
    },
    Yield {
        value: Expr,
    },
    YieldFrom {
        iter: Expr,
    },
    Break,
    BreakLabel {
        label: String,
        value: MaybeExpr,
    },
    Continue,
    BlockLabel {
        label: String,
        body: BlockIR,
    },
    Defer {
        body: BlockIR,
    },
    TryCatch {
        body: BlockIR,
        catches: Vec<(MaybePattern, BlockIR)>,
        else_body: MaybeBlock,
        finally_body: MaybeBlock,
    },
    WhileLet {
        pattern: Pattern,
        expr: Expr,
        guard_e: MaybeExpr,
        body: BlockIR,
    },
    Raise {
        value: Expr,
    },
    Assert {
        cond: Expr,
    },
    TypeAlias {
        name: String,
        ty: IrType,
    },
    CheckerBlock {
        label: String,
        ps_name: MaybeStr,
    },
    Pass,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard,
    Ident {
        name: String,
    },
    RefMutIdent {
        name: String,
    },
    LitInt {
        v: i64,
    },
    LitStr {
        s: String,
    },
    LitBool {
        b: bool,
    },
    LitF64 {
        v: f64,
    },
    Tuple {
        elems: Box<Vec<Pattern>>,
    },
    List {
        elems: Box<Vec<Pattern>>,
    },
    Struct {
        name: String,
        fields: Box<Vec<(String, Pattern)>>,
    },
    Enum {
        enum_name: String,
        variant: String,
        args: Box<Vec<Pattern>>,
    },
    Rest {
        name: MaybeStr,
    },
    Range {
        start: i64,
        end: i64,
        inclusive: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    FnDef {
        name: String,
        generics: Vec<String>,
        params: Vec<(String, IrType, bool, bool, bool)>,
        ret: IrType,
        exc: MaybeIrType,
        body: Vec<Stmt>,
    },
    Const {
        name: String,
        ty: IrType,
        value: Expr,
    },
    StructDef {
        name: String,
        generics: Vec<String>,
        fields: Vec<(String, IrType)>,
    },
    EnumDef {
        name: String,
        generics: Vec<String>,
        variants: Vec<(String, Vec<IrType>)>,
    },
    TraitDef {
        name: String,
        supertraits: Vec<IrType>,
        methods: Vec<(String, Vec<IrType>, IrType)>,
    },
    DuckDef {
        name: String,
        method_count: i64,
    },
    UseStmt {
        path: Vec<String>,
        alias: MaybeStr,
        items: Vec<String>,
        is_from: bool,
    },
    TypeAlias {
        name: String,
        ty: IrType,
    },
    Impl {
        trait_: MaybeIrType,
        for_type: IrType,
        method_names: Vec<String>,
    },
    CheckerBlock {
        name: String,
        ps_name: MaybeStr,
    },
    Test {
        name: String,
        body: Vec<Stmt>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrModule {
    pub name: String,
    pub items: Vec<Item>,
    pub prelude: Vec<String>,
    pub version: i64,
}

pub fn display_type(t: IrType) -> String {
    match t.clone() {
        IrType::Int => {
            "int".to_string().to_string()
        }
        IrType::F64 => {
            "f64".to_string().to_string()
        }
        IrType::Str => {
            "str".to_string().to_string()
        }
        IrType::Bool => {
            "bool".to_string().to_string()
        }
        IrType::Unit => {
            "()".to_string().to_string()
        }
        IrType::Never => {
            "!".to_string().to_string()
        }
        IrType::Any => {
            "?".to_string().to_string()
        }
        IrType::Self_ => {
            "Self".to_string().to_string()
        }
        IrType::Generic { name: g } => {
            g.to_string()
        }
        IrType::Named { path: p, args: a } => {
            let a = *a;
            p.to_string() + &(if (a.len() as i64) > 0i64 { "<".to_string().to_string() + &type_list(a.clone())[..] + &">".to_string()[..] } else { "".to_string() })
        }
        IrType::Opt { inner: x } => {
            let x = *x;
            "Option<".to_string().to_string() + &display_type(x.clone())[..] + &">".to_string()[..]
        }
        IrType::Res { ok: o, err: e } => {
            let o = *o;
            let e = *e;
            "Result<".to_string().to_string() + &display_type(o.clone())[..] + &", ".to_string()[..] + &display_type(e.clone())[..] + &">".to_string()[..]
        }
        IrType::Tuple { elems: es } => {
            let es = *es;
            "(".to_string().to_string() + &type_list(es.clone())[..] + &")".to_string()[..]
        }
        IrType::FnType { params: ps, ret: r } => {
            let ps = *ps;
            let r = *r;
            "fn(".to_string().to_string() + &type_list(ps.clone())[..] + &") -> ".to_string()[..] + &display_type(r.clone())[..]
        }
        IrType::Ref { inner: x } => {
            let x = *x;
            "&".to_string().to_string() + &display_type(x.clone())[..]
        }
        IrType::MutRef { inner: x } => {
            let x = *x;
            "&mut ".to_string().to_string() + &display_type(x.clone())[..]
        }
        IrType::Duck { fields: fs } => {
            let fs = *fs;
            "duck {".to_string().to_string() + &duck_fields(fs.clone())[..] + &"}".to_string()[..]
        }
    }
}

pub fn type_list(mut ts: Vec<IrType>) -> String {
    return if (ts.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let head = display_type(ts[0i64 as usize].clone());
        if (ts.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &type_list(tail_t(ts.clone()))[..] } else { head }
    };
}

pub fn tail_t(mut ts: Vec<IrType>) -> Vec<IrType> {
    let mut out: Vec<IrType> = Vec::new();
    for idx in 1i64..(ts.len() as i64) {
        out.push(ts[idx as usize].clone());
    }
    return out;
}

pub fn duck_fields(mut fs: Vec<(String, IrType)>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(fs.len() as i64) {
        let f = fs[idx as usize].clone();
        out = LzAdd::__add__(LzAdd::__add__(out + &(if idx > 0i64 { ", ".to_string().to_string() } else { "".to_string().to_string() }), f.0.to_string()) + &": ".to_string().to_string()[..], display_type(f.1.clone()));
    }
    return out;
}

pub fn display_expr(e: Expr) -> String {
    match e.clone() {
        Expr::LitInt { v: n, ty: t } => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..] + &n.to_string()[..] + &"_i64".to_string()[..]
        }
        Expr::LitF64 { v: n, ty: t } => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..] + &n.to_string()[..] + &"_f64".to_string()[..]
        }
        Expr::LitStr { s: s, ty: t } => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] \"".to_string().to_string()[..] + &s[..] + &"\"".to_string().to_string()[..]
        }
        Expr::LitFStr { s: s, ty: t } => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] f\"".to_string().to_string()[..] + &s[..] + &"\"".to_string().to_string()[..]
        }
        Expr::LitBool { b: b, ty: t } => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..] + &b.to_string()[..]
        }
        Expr::LitUnit { ty: t } => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ()".to_string().to_string()[..]
        }
        Expr::LitNone { ty: t } => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] None".to_string().to_string()[..]
        }
        Expr::Var { name: n, ty: t } => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..] + &n[..]
        }
        Expr::Call { callee: c, args: a, ty: t } => {
            let c = *c;
            let a = *a;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] call ".to_string().to_string()[..] + &display_expr(c.clone())[..] + &arg_list(a.clone())[..]
        }
        Expr::MethodCall { receiver: r, method: m, args: a, ty: t } => {
            let r = *r;
            let a = *a;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] method ".to_string().to_string()[..] + &display_expr(r.clone())[..] + &".".to_string().to_string()[..] + &m[..] + &arg_list(a.clone())[..]
        }
        Expr::FieldAccess { base: b, field: f, ty: t } => {
            let b = *b;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] field ".to_string().to_string()[..] + &display_expr(b.clone())[..] + &".".to_string().to_string()[..] + &f[..]
        }
        Expr::IndexGet { base: b, key: k, ty: t } => {
            let b = *b;
            let k = *k;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] index ".to_string().to_string()[..] + &display_expr(b.clone())[..] + &"[".to_string().to_string()[..] + &display_expr(k.clone())[..] + &"]".to_string().to_string()[..]
        }
        Expr::IndexSet { base: b, key: k, value: v, ty: t } => {
            let b = *b;
            let k = *k;
            let v = *v;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] index_set ".to_string().to_string()[..] + &display_expr(b.clone())[..] + &"[".to_string().to_string()[..] + &display_expr(k.clone())[..] + &"] = ".to_string().to_string()[..] + &display_expr(v.clone())[..]
        }
        Expr::BinOp { op: o, lhs: l, rhs: r, ty: t } => {
            let l = *l;
            let r = *r;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] binop ".to_string().to_string()[..] + &display_expr(l.clone())[..] + &" ".to_string().to_string()[..] + &o[..] + &" ".to_string().to_string()[..] + &display_expr(r.clone())[..]
        }
        Expr::UnOp { op: o, operand: p, ty: t } => {
            let p = *p;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] unop ".to_string().to_string()[..] + &o[..] + &" ".to_string().to_string()[..] + &display_expr(p.clone())[..]
        }
        Expr::StructCtor { name: n, fields: fs, ty: t } => {
            let fs = *fs;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] new ".to_string().to_string()[..] + &n[..] + &field_pairs(fs.clone())[..]
        }
        Expr::EnumCtor { enum_name: en, variant: v, args: a, ty: t } => {
            let a = *a;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..] + &en[..] + &"::".to_string().to_string()[..] + &v[..] + &arg_list(a.clone())[..]
        }
        Expr::Cast { inner: i, target: tg, ty: t } => {
            let i = *i;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] cast ".to_string().to_string()[..] + &display_expr(i.clone())[..] + &" as ".to_string().to_string()[..] + &display_type(tg.clone())[..]
        }
        Expr::MagicCall { magic: m, args: a, ty: t } => {
            let a = *a;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] magic ".to_string().to_string()[..] + &m[..] + &arg_list(a.clone())[..]
        }
        Expr::IfExpr { cond: c, then: th, els: el, ty: t } => {
            let c = *c;
            let th = *th;
            let el = *el;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] if ".to_string().to_string()[..] + &display_expr(c.clone())[..] + &" then ".to_string().to_string()[..] + &display_expr(th.clone())[..] + &" else ".to_string().to_string()[..] + &display_expr(el.clone())[..]
        }
        Expr::Lambda { params: ps, body: b, ty: t } => {
            let b = *b;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] |".to_string().to_string()[..] + &str_join(ps.clone())[..] + &"| ".to_string().to_string()[..] + &display_expr(b.clone())[..]
        }
        Expr::Pipe { receiver: r, callee: c, args: a, ty: t } => {
            let r = *r;
            let c = *c;
            let a = *a;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] pipe ".to_string().to_string()[..] + &display_expr(r.clone())[..] + &" |> ".to_string().to_string()[..] + &display_expr(c.clone())[..] + &arg_list(a.clone())[..]
        }
        Expr::TupleLit { elems: es, ty: t } => {
            let es = *es;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] (".to_string().to_string()[..] + &expr_list(es.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::ListLit { items: xs, ty: t } => {
            let xs = *xs;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] [".to_string().to_string()[..] + &expr_list(xs.clone())[..] + &"]".to_string().to_string()[..]
        }
        Expr::BlockExpr { stmts: ss, ty: t } => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] block ".to_string().to_string()[..] + &block_with_ty(ss.clone(), t.clone())[..]
        }
        Expr::GenExpr { yield_of: y, ty: t } => {
            let y = *y;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] gen ".to_string().to_string()[..] + &display_expr(y.clone())[..]
        }
        Expr::Paren { inner: i, ty: t } => {
            let i = *i;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] (".to_string().to_string()[..] + &display_expr(i.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::Range { end: en, inclusive: inc, ty: t } => {
            let en = *en;
            "[".to_string().to_string() + &display_type(t)[..] + &"] <expr>".to_string().to_string()[..]
        }
        Expr::Dict { pairs: ps, ty: t } => {
            let ps = *ps;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] <expr>".to_string().to_string()[..]
        }
        Expr::AssignExpr { target: tg, value: v, ty: t } => {
            let tg = *tg;
            let v = *v;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] <expr>".to_string().to_string()[..]
        }
        Expr::ImplicitConvert { source: s, target_ty: tt, ty: t } => {
            let s = *s;
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] <expr>".to_string().to_string()[..]
        }
    }
}

pub fn arg_list(mut as_: Vec<Expr>) -> String {
    return if (as_.len() as i64) == 0i64 { "".to_string().to_string() } else { "(".to_string().to_string() + &expr_list(as_.clone())[..] + &")".to_string().to_string()[..] };
}

pub fn expr_list(mut es: Vec<Expr>) -> String {
    return if (es.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let head = display_expr(es[0i64 as usize].clone());
        if (es.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &expr_list(tail_e(es.clone()))[..] } else { head }
    };
}

pub fn tail_e(mut es: Vec<Expr>) -> Vec<Expr> {
    let mut out: Vec<Expr> = Vec::new();
    for idx in 1i64..(es.len() as i64) {
        out.push(es[idx as usize].clone());
    }
    return out;
}

pub fn field_pairs(mut fs: Vec<(String, Expr)>) -> String {
    return if (fs.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let mut out = "{ ".to_string().to_string();
        for idx in 0i64..(fs.len() as i64) {
            out = LzAdd::__add__(out + &(if idx > 0i64 { ", ".to_string().to_string() } else { "".to_string().to_string() }), pair_str(fs[idx as usize].clone()));
        }
        LzAdd::__add__(out, " }".to_string().to_string())
    };
}

pub fn pair_str(p: (String, Expr)) -> String {
    return p.0.to_string() + &": ".to_string().to_string()[..] + &display_expr(p.1.clone())[..];
}

pub fn display_stmt(s: Stmt) -> String {
    match s.clone() {
        Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
            let_kw(m, r) + &n[..] + &": ".to_string().to_string()[..] + &display_type(t.clone())[..] + &" = ".to_string().to_string()[..] + &display_expr(v.clone())[..]
        }
        Stmt::Assign { target: t, value: v } => {
            display_expr(t.clone()) + &" = ".to_string().to_string()[..] + &display_expr(v.clone())[..]
        }
        Stmt::Return { value: v } => {
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    "return ".to_string().to_string() + &display_expr(inner.clone())[..]
                }
                MaybeExpr::NoExpr => {
                    "return".to_string().to_string()
                }
            }
        }
        Stmt::ExprStmt { expr: e } => {
            display_expr(e.clone())
        }
        Stmt::If { cond: c, then: t, els: e } => {
            let base = "if ".to_string().to_string() + &display_expr(c.clone())[..] + &" ".to_string().to_string()[..] + &block_disp(t.clone())[..];
            match e.clone() {
                MaybeBlock::YesBlock { value: es } => {
                    base + &" else ".to_string().to_string()[..] + &block_disp(es.clone())[..]
                }
                MaybeBlock::NoBlock => {
                    base
                }
            }
        }
        Stmt::For { var: v, iter: it, guard_e: g, body: b, else_body: eb } => {
            "for ".to_string().to_string() + &v[..] + &" in ".to_string().to_string()[..] + &display_expr(it.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string().to_string()[..] + &block_disp(b.clone())[..] + &else_s(eb.clone())[..]
        }
        Stmt::While { cond: c, guard_e: g, body: b, else_body: eb } => {
            "while ".to_string().to_string() + &display_expr(c.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string().to_string()[..] + &block_disp(b.clone())[..] + &else_s(eb.clone())[..]
        }
        Stmt::Block { stmts: b } => {
            let b = *b;
            stmt_block(b.clone())
        }
        Stmt::Match { scrutinee: s, arms: as_ } => {
            match_block(s.clone(), as_.clone())
        }
        Stmt::Yield { value: v } => {
            "yield ".to_string().to_string() + &display_expr(v.clone())[..]
        }
        Stmt::YieldFrom { iter: it } => {
            "yield from ".to_string().to_string() + &display_expr(it.clone())[..]
        }
        Stmt::Break => {
            "break".to_string().to_string()
        }
        Stmt::BreakLabel { label: l, value: v } => {
            let base = "break \'".to_string().to_string() + &l[..];
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    base + &" ".to_string().to_string()[..] + &display_expr(inner.clone())[..]
                }
                MaybeExpr::NoExpr => {
                    base
                }
            }
        }
        Stmt::Continue => {
            "continue".to_string().to_string()
        }
        Stmt::BlockLabel { label: l, body: b } => {
            "block \'".to_string().to_string() + &l[..] + &" ".to_string().to_string()[..] + &block_disp(b.clone())[..]
        }
        Stmt::Defer { body: b } => {
            "defer ".to_string().to_string() + &block_disp(b.clone())[..]
        }
        Stmt::TryCatch { body: b, catches: cs, else_body: eb, finally_body: fb } => {
            let mut out = "try ".to_string().to_string() + &block_disp(b.clone())[..];
            out = out + &try_catches_str(cs.clone())[..];
            out = out + &else_s(eb.clone())[..];
            out = out + &finally_s(fb.clone())[..];
            out
        }
        Stmt::WhileLet { pattern: p, expr: e, guard_e: g, body: b } => {
            "while let ".to_string().to_string() + &display_pattern(p.clone())[..] + &" = ".to_string().to_string()[..] + &display_expr(e.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string().to_string()[..] + &block_disp(b.clone())[..]
        }
        Stmt::CheckerBlock { label: l, ps_name: p } => {
            "block \'".to_string().to_string() + &l[..] + &"[ps:".to_string().to_string()[..] + &opt_debug(p.clone())[..] + &"]".to_string().to_string()[..]
        }
        Stmt::Raise { value: v } => {
            "<stmt>".to_string()
        }
        Stmt::Assert { cond: c } => {
            "<stmt>".to_string()
        }
        Stmt::TypeAlias { name: n, ty: t } => {
            "<stmt>".to_string()
        }
        Stmt::Pass => {
            "<stmt>".to_string()
        }
    }
}

pub fn let_kw(m: bool, r: bool) -> String {
    return (if (m && r) { "let mut ref ".to_string().to_string() } else { (if r { "let ref ".to_string().to_string() } else { (if m { "let mut ".to_string().to_string() } else { "let ".to_string().to_string() }) }) });
}

pub fn guard_s(g: MaybeExpr) -> String {
    match g.clone() {
        MaybeExpr::YesExpr { value: e } => {
            " if ".to_string().to_string() + &display_expr(e.clone())[..]
        }
        MaybeExpr::NoExpr => {
            "".to_string().to_string()
        }
    }
}

pub fn else_s(eb: MaybeBlock) -> String {
    match eb.clone() {
        MaybeBlock::YesBlock { value: b } => {
            " else ".to_string().to_string() + &block_disp(b.clone())[..]
        }
        MaybeBlock::NoBlock => {
            "".to_string().to_string()
        }
    }
}

pub fn finally_s(fb: MaybeBlock) -> String {
    match fb.clone() {
        MaybeBlock::YesBlock { value: b } => {
            " finally ".to_string().to_string() + &block_disp(b.clone())[..]
        }
        MaybeBlock::NoBlock => {
            "".to_string().to_string()
        }
    }
}

pub fn try_catches_str(mut cs: Vec<(MaybePattern, BlockIR)>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(cs.len() as i64) {
        let c = cs[idx as usize].clone();
        out = LzAdd::__add__(out, catch_str(c.clone()));
    }
    return out;
}

pub fn catch_str(c: (MaybePattern, BlockIR)) -> String {
    match c.0 {
        MaybePattern::YesPat { value: p } => {
            " catch(".to_string().to_string() + &display_pattern(p.clone())[..] + &") ".to_string().to_string()[..] + &block_disp(c.1.clone())[..]
        }
        MaybePattern::NoPat => {
            " catch ".to_string().to_string() + &block_disp(c.1.clone())[..]
        }
    }
}

pub fn display_pattern(p: Pattern) -> String {
    match p.clone() {
        Pattern::Wildcard => {
            "_".to_string().to_string()
        }
        Pattern::Ident { name: n } => {
            n
        }
        Pattern::RefMutIdent { name: n } => {
            "ref mut ".to_string().to_string() + &n[..]
        }
        Pattern::LitInt { v: n } => {
            n.to_string() + &"_i64".to_string()[..]
        }
        Pattern::LitStr { s: s } => {
            "\"".to_string().to_string() + &s[..] + &"\"".to_string().to_string()[..]
        }
        Pattern::LitBool { b: b } => {
            b.to_string()
        }
        Pattern::LitF64 { v: n } => {
            n.to_string() + &"_f64".to_string()[..]
        }
        Pattern::Tuple { elems: es } => {
            let es = *es;
            "(".to_string().to_string() + &pat_list(es.clone())[..] + &")".to_string().to_string()[..]
        }
        Pattern::List { elems: es } => {
            let es = *es;
            "[".to_string().to_string() + &pat_list(es.clone())[..] + &"]".to_string().to_string()[..]
        }
        Pattern::Struct { name: n, fields: fs } => {
            let fs = *fs;
            n.to_string() + &pat_struct_fields(fs.clone())[..]
        }
        Pattern::Enum { enum_name: en, variant: v, args: a } => {
            let a = *a;
            en.to_string() + &"::".to_string().to_string()[..] + &v[..] + &arg_pat_list(a.clone())[..]
        }
        Pattern::Rest { name: rn } => {
            match rn.clone() {
                MaybeStr::YesStr { value: n2 } => {
                    "..".to_string().to_string() + &n2[..]
                }
                MaybeStr::NoStr => {
                    "..".to_string().to_string()
                }
            }
        }
        Pattern::Range { start: st, end: en, inclusive: inc } => {
            st.to_string() + &(if inc { "..=".to_string().to_string() } else { "..".to_string().to_string() }) + &en.to_string()[..]
        }
    }
}

pub fn pat_list(mut ps: Vec<Pattern>) -> String {
    return if (ps.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let head = display_pattern(ps[0i64 as usize].clone());
        if (ps.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &pat_list(tail_pat(ps.clone()))[..] } else { head }
    };
}

pub fn tail_pat(mut ps: Vec<Pattern>) -> Vec<Pattern> {
    let mut out: Vec<Pattern> = Vec::new();
    for idx in 1i64..(ps.len() as i64) {
        out.push(ps[idx as usize].clone());
    }
    return out;
}

pub fn arg_pat_list(mut ps: Vec<Pattern>) -> String {
    return if (ps.len() as i64) == 0i64 { "".to_string().to_string() } else { "(".to_string().to_string() + &pat_list(ps.clone())[..] + &")".to_string().to_string()[..] };
}

pub fn pat_struct_fields(mut fs: Vec<(String, Pattern)>) -> String {
    return if (fs.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let mut out = "{ ".to_string().to_string();
        for idx in 0i64..(fs.len() as i64) {
            let f = fs[idx as usize].clone();
            out = LzAdd::__add__(LzAdd::__add__(out + &(if idx > 0i64 { ", ".to_string().to_string() } else { "".to_string().to_string() }), f.0) + &": ".to_string().to_string()[..], display_pattern(f.1.clone()));
        }
        LzAdd::__add__(out, " }".to_string().to_string())
    };
}

pub fn match_block(s: Expr, mut as_: Vec<(Pattern, MaybeExpr, BlockIR)>) -> String {
    let mut out = "match ".to_string().to_string() + &display_expr(s.clone())[..] + &" {".to_string().to_string()[..];
    for idx in 0i64..(as_.len() as i64) {
        let arm = as_[idx as usize].clone();
        out = LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(out + &" ".to_string().to_string()[..], display_pattern(arm.0.clone())), guard_s(arm.1.clone())) + &" => ".to_string().to_string()[..], block_disp(arm.2.clone()));
    }
    return out + &" }".to_string().to_string()[..];
}

pub fn stmt_block(mut ss: Vec<Stmt>) -> String {
    return if (ss.len() as i64) == 0i64 { "{ }".to_string().to_string()} else {
        let mut out = "{\n".to_string().to_string();
        for idx in 0i64..(ss.len() as i64) {
            out = LzAdd::__add__(out + &"  ".to_string().to_string()[..], display_stmt(ss[idx as usize].clone())) + &"\n".to_string().to_string()[..];
        }
        LzAdd::__add__(out, "}".to_string().to_string())
    };
}

pub fn block_disp(b: BlockIR) -> String {
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            if (ss.len() as i64) == 0i64 { "{ } [".to_string().to_string() + &display_type(t.clone())[..] + &"]".to_string().to_string()[..]} else {
                let mut out = "{ [".to_string().to_string() + &display_type(t.clone())[..] + &"]\n".to_string().to_string()[..];
                for idx in 0i64..(ss.len() as i64) {
                    out = LzAdd::__add__(out + &"  ".to_string().to_string()[..], display_stmt(ss[idx as usize].clone())) + &"\n".to_string().to_string()[..];
                }
                LzAdd::__add__(out, "}".to_string())
            }
        }
    }
}

pub fn block_with_ty(mut ss: Vec<Stmt>, t: IrType) -> String {
    return if (ss.len() as i64) == 0i64 { "{ } [".to_string().to_string() + &display_type(t.clone())[..] + &"]".to_string().to_string()[..]} else {
        let mut out = "{ [".to_string().to_string() + &display_type(t.clone())[..] + &"]\n".to_string().to_string()[..];
        for idx in 0i64..(ss.len() as i64) {
            out = LzAdd::__add__(out + &"  ".to_string().to_string()[..], display_stmt(ss[idx as usize].clone())) + &"\n".to_string().to_string()[..];
        }
        LzAdd::__add__(out, "}".to_string())
    };
}

pub fn opt_debug(o: MaybeStr) -> String {
    match o.clone() {
        MaybeStr::YesStr { value: s } => {
            "Some(\"".to_string().to_string() + &s[..] + &")".to_string().to_string()[..]
        }
        MaybeStr::NoStr => {
            "None".to_string().to_string()
        }
    }
}

pub fn stmt_lines(mut ss: Vec<Stmt>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(ss.len() as i64) {
        out = LzAdd::__add__(out + &"  ".to_string().to_string()[..], display_stmt(ss[idx as usize].clone())) + &"\n".to_string().to_string()[..];
    }
    return out;
}

pub fn display_item(it: Item) -> String {
    match it.clone() {
        Item::FnDef { name: n, generics: gs, params: ps, ret: r, exc: rs, body: b } => {
            "fn ".to_string().to_string() + &n[..] + &generic_sig(gs.clone())[..] + &param_sig(ps.clone())[..] + &" -> ".to_string().to_string()[..] + &display_type(r.clone())[..] + &raises_sig(rs.clone())[..] + &":\n".to_string().to_string()[..] + &stmt_lines(b.clone())[..]
        }
        Item::Const { name: n, ty: t, value: v } => {
            "const ".to_string().to_string() + &n[..] + &": ".to_string().to_string()[..] + &display_type(t.clone())[..] + &" = ".to_string().to_string()[..] + &display_expr(v.clone())[..]
        }
        Item::StructDef { name: n, generics: gs, fields: fs } => {
            "struct ".to_string().to_string() + &n[..] + &generic_sig(gs.clone())[..] + &" {\n".to_string().to_string()[..] + &field_lines(fs.clone())[..] + &"}".to_string().to_string()[..]
        }
        Item::EnumDef { name: n, generics: gs, variants: vs } => {
            "enum ".to_string().to_string() + &n[..] + &generic_sig(gs.clone())[..] + &" {\n".to_string().to_string()[..] + &variant_lines(vs.clone())[..] + &"}".to_string().to_string()[..]
        }
        Item::TraitDef { name: n, supertraits: ss, methods: ms } => {
            "trait ".to_string().to_string() + &n[..] + &super_join(ss.clone())[..] + &" {\n".to_string().to_string()[..] + &method_lines(ms.clone())[..] + &"}".to_string().to_string()[..]
        }
        Item::DuckDef { name: n, method_count: c } => {
            "duck ".to_string().to_string() + &n[..] + &" { ".to_string().to_string()[..] + &c.to_string()[..] + &" methods }".to_string().to_string()[..]
        }
        Item::UseStmt { path: p, alias: a, items: xs, is_from: f } => {
            (if f { ("from ".to_string().to_string() + &dot_join(p.clone())[..] + &" import ".to_string().to_string()[..] + &str_join(xs.clone())[..]) } else { ("import ".to_string().to_string() + &dot_join(p.clone())[..]) })
        }
        Item::TypeAlias { name: n, ty: t } => {
            "type ".to_string().to_string() + &n[..] + &" = ".to_string().to_string()[..] + &display_type(t.clone())[..]
        }
        Item::Impl { trait_: tr, for_type: fty, method_names: ms } => {
            let mut out = "impl ".to_string().to_string() + &impl_head(tr.clone())[..] + &display_type(fty.clone())[..] + &" {\n".to_string().to_string()[..];
            for idx in 0i64..(ms.len() as i64) {
                out = out + &"  fn ".to_string().to_string()[..] + &ms[idx as usize].to_string()[..] + &" ...\n".to_string().to_string()[..];
            }
            out + &"}".to_string().to_string()[..]
        }
        Item::CheckerBlock { name: n, ps_name: p } => {
            "checker block \'".to_string().to_string() + &n[..] + &"[ps:".to_string().to_string()[..] + &opt_debug(p.clone())[..] + &"]".to_string().to_string()[..]
        }
        Item::Test { name: n, body: b } => {
            "test ".to_string().to_string() + &n[..] + &" ".to_string().to_string()[..] + &stmt_block(b.clone())[..]
        }
    }
}

pub fn generic_sig(mut gs: Vec<String>) -> String {
    return if (gs.len() as i64) == 0i64 { "".to_string().to_string() } else { "<".to_string().to_string() + &str_join(gs.clone())[..] + &">".to_string().to_string()[..] };
}

pub fn variant_lines(mut vs: Vec<(String, Vec<IrType>)>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(vs.len() as i64) {
        let v = vs[idx as usize].clone();
        out = LzAdd::__add__(LzAdd::__add__(out + &"  ".to_string().to_string()[..], v.0), variant_args(v.1.clone())) + &"\n".to_string().to_string()[..];
    }
    return out;
}

pub fn field_lines(mut fs: Vec<(String, IrType)>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(fs.len() as i64) {
        let f = fs[idx as usize].clone();
        out = LzAdd::__add__(LzAdd::__add__(out + &"  ".to_string().to_string()[..], f.0) + &": ".to_string().to_string()[..], display_type(f.1.clone())) + &"\n".to_string().to_string()[..];
    }
    return out;
}

pub fn variant_args(mut ts: Vec<IrType>) -> String {
    return if (ts.len() as i64) == 0i64 { "".to_string().to_string() } else { "(".to_string().to_string() + &type_list(ts.clone())[..] + &")".to_string().to_string()[..] };
}

pub fn super_join(mut ss: Vec<IrType>) -> String {
    return if (ss.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let mut out = " : ".to_string().to_string() + &display_type(ss[0i64 as usize].clone())[..];
        for idx in 1i64..(ss.len() as i64) {
            out = LzAdd::__add__(out + &" + ".to_string().to_string()[..], display_type(ss[idx as usize].clone()));
        }
        out
    };
}

pub fn method_lines(mut ms: Vec<(String, Vec<IrType>, IrType)>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(ms.len() as i64) {
        let m = ms[idx as usize].clone();
        out = LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(out + &"  fn ".to_string().to_string()[..], m.0) + &"(".to_string().to_string()[..], type_list(m.1.clone())) + &") -> ".to_string().to_string()[..], display_type(m.2.clone())) + &"\n".to_string().to_string()[..];
    }
    return out;
}

pub fn param_sig(mut ps: Vec<(String, IrType, bool, bool, bool)>) -> String {
    return if (ps.len() as i64) == 0i64 { "()".to_string().to_string() } else { "(".to_string().to_string() + &param_list(ps.clone())[..] + &")".to_string().to_string()[..] };
}

pub fn param_list(mut ps: Vec<(String, IrType, bool, bool, bool)>) -> String {
    return if (ps.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let p0 = ps[0i64 as usize].clone();
        param_str(p0.clone()) + &(if (ps.len() as i64) > 1i64 { ", ".to_string().to_string() + &param_list(tail_p(ps.clone()))[..] } else { "".to_string().to_string() })
    };
}

pub fn param_str(p: (String, IrType, bool, bool, bool)) -> String {
    return (if p.3 { "mut ".to_string().to_string() } else { "".to_string().to_string() }) + &(if p.4 { "owned ".to_string().to_string() } else { "".to_string().to_string() }) + &(if p.2 { "ref ".to_string().to_string() } else { "".to_string().to_string() }) + &p.0 + &": ".to_string().to_string()[..] + &display_type(p.1.clone())[..];
}

pub fn tail_p(mut ps: Vec<(String, IrType, bool, bool, bool)>) -> Vec<(String, IrType, bool, bool, bool)> {
    let mut out: Vec<(String, IrType, bool, bool, bool)> = Vec::new();
    for idx in 1i64..(ps.len() as i64) {
        out.push(ps[idx as usize].clone());
    }
    return out;
}

pub fn impl_head(tr: MaybeIrType) -> String {
    match tr.clone() {
        MaybeIrType::YesTy { value: t } => {
            display_type(t.clone()) + &" for ".to_string().to_string()[..]
        }
        MaybeIrType::NoTy => {
            "".to_string().to_string()
        }
    }
}

pub fn raises_sig(rs: MaybeIrType) -> String {
    match rs.clone() {
        MaybeIrType::YesTy { value: t } => {
            " raises ".to_string().to_string() + &display_type(t.clone())[..]
        }
        MaybeIrType::NoTy => {
            "".to_string().to_string()
        }
    }
}

pub fn dot_join(mut xs: Vec<String>) -> String {
    return if (xs.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let mut out = xs[0i64 as usize].to_string();
        for idx in 1i64..(xs.len() as i64) {
            out = out + &".".to_string().to_string()[..] + &xs[idx as usize].to_string()[..];
        }
        out
    };
}

pub fn display_module(m: IrModule) -> String {
    let mut out = ";; LZIR v".to_string().to_string() + &m.version.to_string()[..] + &" \u{2014} module \'".to_string().to_string()[..] + &m.name + &"\'\n".to_string().to_string()[..];
    out = out + &";; ".to_string().to_string()[..] + &(m.items.len() as i64).to_string()[..] + &" items\n".to_string().to_string()[..];
    if (m.prelude.len() as i64) > 0i64 {
        out = out + &";; prelude: ".to_string().to_string()[..] + &str_join(m.prelude.clone())[..] + &"\n".to_string().to_string()[..];
    }
    out = out + &"\n".to_string().to_string()[..];
    for idx in 0i64..(m.items.len() as i64) {
        out = LzAdd::__add__(out, display_item(m.items[idx as usize].clone())) + &"\n\n".to_string().to_string()[..];
    }
    return out;
}

pub fn str_join(mut xs: Vec<String>) -> String {
    return if (xs.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let mut out = xs[0i64 as usize].to_string();
        for idx in 1i64..(xs.len() as i64) {
            out = out + &", ".to_string().to_string()[..] + &xs[idx as usize].to_string()[..];
        }
        out
    };
}

const __name__: &str = "main";

const __file__: &str = "lz_ir_lib.lz";

const __package__: &str = "ir";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\src\\ir";

const __doc__: &str = "";

const __is_macro__: bool = false;

pub fn main() {
}


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
            p.to_string() + &(if (a.len() as i64) > 0i64 { "<".to_string().to_string() + &type_list(a.clone())[..] + &">".to_string().to_string()[..] } else { "".to_string().to_string() })
        }
        IrType::Opt { inner: x } => {
            let x = *x;
            "Option<".to_string().to_string() + &display_type(x.clone())[..] + &">".to_string().to_string()[..]
        }
        IrType::Res { ok: o, err: e } => {
            let o = *o;
            let e = *e;
            "Result<".to_string().to_string() + &display_type(o.clone())[..] + &", ".to_string().to_string()[..] + &display_type(e.clone())[..] + &">".to_string().to_string()[..]
        }
        IrType::Tuple { elems: es } => {
            let es = *es;
            "(".to_string().to_string() + &type_list(es.clone())[..] + &")".to_string().to_string()[..]
        }
        IrType::FnType { params: ps, ret: r } => {
            let ps = *ps;
            let r = *r;
            "fn(".to_string().to_string() + &type_list(ps.clone())[..] + &") -> ".to_string().to_string()[..] + &display_type(r.clone())[..]
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
            "duck {".to_string().to_string() + &duck_fields(fs.clone())[..] + &"}".to_string().to_string()[..]
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
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..] + &n.to_string()[..] + &"_i64".to_string().to_string()[..]
        }
        Expr::LitF64 { v: n, ty: t } => {
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..] + &n.to_string()[..] + &"_f64".to_string().to_string()[..]
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
    return p.0 + &": ".to_string().to_string()[..] + &display_expr(p.1.clone())[..];
}

pub fn display_stmt(s: Stmt) -> String {
    match s.clone() {
        Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
            let_kw(m, r) + &n[..] + &": ".to_string()[..] + &display_type(t.clone())[..] + &" = ".to_string()[..] + &display_expr(v.clone())[..]
        }
        Stmt::Assign { target: t, value: v } => {
            display_expr(t.clone()) + &" = ".to_string()[..] + &display_expr(v.clone())[..]
        }
        Stmt::Return { value: v } => {
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    "return ".to_string().to_string() + &display_expr(inner.clone())[..]
                }
                MaybeExpr::NoExpr => {
                    "return".to_string()
                }
            }
        }
        Stmt::ExprStmt { expr: e } => {
            display_expr(e.clone())
        }
        Stmt::If { cond: c, then: t, els: e } => {
            let base: String = "if ".to_string().to_string() + &display_expr(c.clone())[..] + &" ".to_string()[..] + &block_disp(t.clone())[..];
            match e.clone() {
                MaybeBlock::YesBlock { value: es } => {
                    base.to_string() + &" else ".to_string()[..] + &block_disp(es.clone())[..]
                }
                MaybeBlock::NoBlock => {
                    base
                }
            }
        }
        Stmt::For { var: v, iter: it, guard_e: g, body: b, else_body: eb } => {
            "for ".to_string().to_string() + &v[..] + &" in ".to_string()[..] + &display_expr(it.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string()[..] + &block_disp(b.clone())[..] + &else_s(eb.clone())[..]
        }
        Stmt::While { cond: c, guard_e: g, body: b, else_body: eb } => {
            "while ".to_string().to_string() + &display_expr(c.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string()[..] + &block_disp(b.clone())[..] + &else_s(eb.clone())[..]
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
            "break".to_string()
        }
        Stmt::BreakLabel { label: l, value: v } => {
            let base: String = "break \'".to_string().to_string() + &l[..];
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    base.to_string() + &" ".to_string()[..] + &display_expr(inner.clone())[..]
                }
                MaybeExpr::NoExpr => {
                    base
                }
            }
        }
        Stmt::Continue => {
            "continue".to_string()
        }
        Stmt::BlockLabel { label: l, body: b } => {
            "block \'".to_string().to_string() + &l[..] + &" ".to_string()[..] + &block_disp(b.clone())[..]
        }
        Stmt::Defer { body: b } => {
            "defer ".to_string().to_string() + &block_disp(b.clone())[..]
        }
        Stmt::TryCatch { body: b, catches: cs, else_body: eb, finally_body: fb } => {
            let mut out: String = "try ".to_string().to_string() + &block_disp(b.clone())[..];
            out = out.to_string() + &try_catches_str(cs.clone())[..];
            out = out.to_string() + &else_s(eb.clone())[..];
            out = out.to_string() + &finally_s(fb.clone())[..];
            out
        }
        Stmt::WhileLet { pattern: p, expr: e, guard_e: g, body: b } => {
            "while let ".to_string().to_string() + &display_pattern(p.clone())[..] + &" = ".to_string()[..] + &display_expr(e.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string()[..] + &block_disp(b.clone())[..]
        }
        Stmt::CheckerBlock { label: l, ps_name: p } => {
            "block \'".to_string().to_string() + &l[..] + &"[ps:".to_string()[..] + &opt_debug(p.clone())[..] + &"]".to_string()[..]
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
            n.to_string() + &"_i64".to_string().to_string()[..]
        }
        Pattern::LitStr { s: s } => {
            "\"".to_string().to_string() + &s[..] + &"\"".to_string().to_string()[..]
        }
        Pattern::LitBool { b: b } => {
            b.to_string()
        }
        Pattern::LitF64 { v: n } => {
            n.to_string() + &"_f64".to_string().to_string()[..]
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
            let mut fields_s = "".to_string().to_string();
            for idx in 0i64..(fs.len() as i64) {
                if idx > 0i64 {
                    fields_s = fields_s + &", ".to_string().to_string()[..];
                }
                let f = fs[idx as usize].clone();
                fields_s = LzAdd::__add__(LzAdd::__add__(fields_s, f.0) + &": ".to_string().to_string()[..], display_pattern(f.1.clone()));
            }
            n.to_string() + &" { ".to_string().to_string()[..] + &fields_s[..] + &" }".to_string().to_string()[..]
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

pub fn gen_match_arms(mut arms: Vec<(Pattern, MaybeExpr, BlockIR)>, fn_raises: MaybeIrType) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(arms.len() as i64) {
        let arm = arms[idx as usize].clone();
        out = LzAdd::__add__(out, gen_match_arm(arm.clone(), fn_raises.clone()));
    }
    return out;
}

pub fn gen_match_arm(arm: (Pattern, MaybeExpr, BlockIR), fn_raises: MaybeIrType) -> String {
    let pat = arm.0;
    let gd = arm.1;
    let blk = arm.2;
    let pat_s = gen_pattern(pat.clone());
    let gd_s = {
match gd.clone() {
    MaybeExpr::YesExpr { value: vv } => {
        LzAdd::__add__(" if ".to_string().to_string(), gen_expr(vv.clone(), MaybeIrType::NoTy))
    }
    MaybeExpr::NoExpr => {
        "".to_string().to_string()
    }
}
    };
    let blk_s = gen_block(blk.clone(), fn_raises.clone());
    return "    ".to_string().to_string() + &pat_s[..] + &gd_s[..] + &" => ".to_string().to_string()[..] + &blk_s[..] + &",\n".to_string().to_string()[..];
}

pub fn gen_pattern(p: Pattern) -> String {
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
            n.to_string() + &"_i64".to_string().to_string()[..]
        }
        Pattern::LitStr { s: s } => {
            "\"".to_string().to_string() + &esc_rust(s.clone())[..] + &"\"".to_string().to_string()[..]
        }
        Pattern::LitBool { b: b } => {
            b.to_string()
        }
        Pattern::LitF64 { v: n } => {
            n.to_string() + &"_f64".to_string().to_string()[..]
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
            let mut fields_s = " { ".to_string().to_string();
            for idx in 0i64..(fs.len() as i64) {
                if idx > 0i64 {
                    fields_s = fields_s + &", ".to_string().to_string()[..];
                }
                let f = fs[idx as usize].clone();
                fields_s = LzAdd::__add__(LzAdd::__add__(fields_s, f.0) + &": ".to_string().to_string()[..], gen_pattern(f.1.clone()));
            }
            fields_s + &" }".to_string().to_string()[..]
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
            st.to_string() + &"_i64..".to_string().to_string()[..] + &en.to_string()[..] + &"_i64".to_string().to_string()[..]
        }
    }
}

pub fn gen_block(b: BlockIR, fn_raises: MaybeIrType) -> String {
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            if (ss.len() as i64) == 0i64 { String::new() } else { "{\n".to_string().to_string() + &gen_body(ss.clone(), false, "".to_string().to_string(), fn_raises.clone())[..] + &"}".to_string()[..] }
        }
    }
}

pub fn rust_type(t: IrType) -> String {
    match t.clone() {
        IrType::Int => {
            "i64".to_string().to_string()
        }
        IrType::F64 => {
            "f64".to_string().to_string()
        }
        IrType::Str => {
            "String".to_string().to_string()
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
            "Box<dyn Any>".to_string().to_string()
        }
        IrType::Self_ => {
            "Self".to_string().to_string()
        }
        IrType::Generic { name: g } => {
            g.to_string()
        }
        IrType::Named { path: p, args: a } => {
            let a = *a;
            p.to_string() + &(if (a.len() as i64) > 0i64 { "<".to_string().to_string() + &type_rust_list(a.clone())[..] + &">".to_string()[..] } else { "".to_string() })
        }
        IrType::Opt { inner: x } => {
            let x = *x;
            "Option<".to_string().to_string() + &rust_type(x.clone())[..] + &">".to_string().to_string()[..]
        }
        IrType::Res { ok: o, err: e } => {
            let o = *o;
            let e = *e;
            "Result<".to_string().to_string() + &rust_type(o.clone())[..] + &", ".to_string()[..] + &rust_type(e.clone())[..] + &">".to_string().to_string()[..]
        }
        IrType::Tuple { elems: es } => {
            let es = *es;
            "(".to_string().to_string() + &type_rust_list(es.clone())[..] + &")".to_string().to_string()[..]
        }
        IrType::FnType { params: ps, ret: r } => {
            let ps = *ps;
            let r = *r;
            "fn(".to_string().to_string() + &type_rust_list(ps.clone())[..] + &") -> ".to_string()[..] + &rust_type(r.clone())[..]
        }
        IrType::Ref { inner: x } => {
            let x = *x;
            "&".to_string().to_string() + &rust_type(x.clone())[..]
        }
        IrType::MutRef { inner: x } => {
            let x = *x;
            "&mut ".to_string().to_string() + &rust_type(x.clone())[..]
        }
        IrType::Duck { fields: fs } => {
            let fs = *fs;
            "Box<dyn Any>".to_string().to_string()
        }
    }
}

pub fn type_rust_list(mut ts: Vec<IrType>) -> String {
    return if (ts.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let head = rust_type(ts[0i64 as usize].clone());
        if (ts.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &type_rust_list(tail_t(ts.clone()))[..] } else { head }
    };
}

pub fn generic_sig(mut gs: Vec<String>) -> String {
    return if (gs.len() as i64) == 0i64 { "".to_string().to_string() } else { "<".to_string().to_string() + &str_join(gs.clone())[..] + &">".to_string().to_string()[..] };
}

pub fn str_join(mut xs: Vec<String>) -> String {
    return if (xs.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let head = xs[0i64 as usize].to_string();
        if (xs.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &str_join(aj_tail(xs.clone()))[..] } else { head }
    };
}

pub fn is_result_ctor(e: Expr) -> bool {
    match e.clone() {
        Expr::Call { callee: c, args: a, ty: t } => {
            let c = *c;
            let a = *a;
            match c.clone() {
                Expr::Var { name: n, ty: t2 } => {
                    (n).to_string() == ("Ok".to_string()).to_string() || (n).to_string() == ("Err".to_string()).to_string()
                }
                _ => {
                    false
                }
            }
        }
        Expr::EnumCtor { enum_name: en, variant: v, args: a, ty: t } => {
            let a = *a;
            (en).to_string() == ("Result".to_string()).to_string() && ((v).to_string() == ("Ok".to_string()).to_string() || (v).to_string() == ("Err".to_string()).to_string())
        }
        _ => {
            false
        }
    }
}

pub fn block_tail_expr(b: BlockIR) -> MaybeExpr {
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            let mut result = MaybeExpr::NoExpr;
            for si in 0i64..(ss.len() as i64) {
                match ss[si as usize].clone() {
                    Stmt::ExprStmt { expr: e } => {
                        result = MaybeExpr::YesExpr { value: e };
                    }
                    _ => {
                        ();
                    }
                }
            }
            result
        }
    }
}

pub fn is_res_type(t: IrType) -> bool {
    match t.clone() {
        IrType::Res { ok: o, err: e } => {
            let o = *o;
            let e = *e;
            true
        }
        _ => {
            false
        }
    }
}

pub fn scan_result_let(mut ss: Vec<Stmt>) -> bool {
    let mut found: bool = false;
    for si in 0i64..(ss.len() as i64) {
        match ss[si as usize].clone() {
            Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
                if is_res_type(t.clone()) && !is_result_ctor(v.clone()) {
                    found = true;
                }
            }
            Stmt::ExprStmt { expr: e } => {
                if is_res_type(expr_ty(e.clone())) && !is_result_ctor(e.clone()) {
                    found = true;
                }
            }
            _ => {
                ();
            }
        }
    }
    return found;
}

pub fn is_result_body(b: BlockIR) -> bool {
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            let tail = block_tail_expr(b.clone());
            let tail_is_ctor: bool = {
match tail.clone() {
    MaybeExpr::YesExpr { value: e } => {
        is_result_ctor(e.clone())
    }
    _ => {
        false
    }
}
    };
            let rule1: bool = is_res_type(t.clone()) && !tail_is_ctor;
            if rule1 { true } else { scan_result_let(ss.clone()) }
        }
    }
}

pub fn scan_result_ty(mut ss: Vec<Stmt>) -> MaybeIrType {
    let mut result = MaybeIrType::NoTy;
    for si in 0i64..(ss.len() as i64) {
        match ss[si as usize].clone() {
            Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
                match result.clone() {
                    MaybeIrType::NoTy => {
                        match t.clone() {
                            IrType::Res { ok: o, err: e } => {
                                let o = *o;
                                let e = *e;
                                result = MaybeIrType::YesTy { value: IrType::Res { ok: Box::new(o), err: Box::new(e) } };
                            }
                            _ => {
                                ();
                            }
                        }
                    }
                    _ => {
                        ();
                    }
                }
            }
            Stmt::ExprStmt { expr: e } => {
                match result.clone() {
                    MaybeIrType::NoTy => {
                        match expr_ty(e.clone()) {
                            IrType::Res { ok: o, err: e } => {
                                let o = *o;
                                let e = *e;
                                result = MaybeIrType::YesTy { value: IrType::Res { ok: Box::new(o), err: Box::new(e) } };
                            }
                            _ => {
                                ();
                            }
                        }
                    }
                    _ => {
                        ();
                    }
                }
            }
            _ => {
                ();
            }
        }
    }
    return result;
}

pub fn result_ty_from_body(b: BlockIR) -> MaybeIrType {
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            match t.clone() {
                IrType::Res { ok: o, err: e } => {
                    let o = *o;
                    let e = *e;
                    MaybeIrType::YesTy { value: IrType::Res { ok: Box::new(o), err: Box::new(e) } }
                }
                _ => {
                    scan_result_ty(ss.clone())
                }
            }
        }
    }
}

pub fn expr_ty(e: Expr) -> IrType {
    match e.clone() {
        Expr::LitInt { v: _, ty: t } => {
            t
        }
        Expr::LitF64 { v: _, ty: t } => {
            t
        }
        Expr::LitStr { s: _, ty: t } => {
            t
        }
        Expr::LitFStr { s: _, ty: t } => {
            t
        }
        Expr::LitBool { b: _, ty: t } => {
            t
        }
        Expr::LitUnit { ty: t } => {
            t
        }
        Expr::LitNone { ty: t } => {
            t
        }
        Expr::Var { name: _, ty: t } => {
            t
        }
        Expr::Call { callee: _, args: _, ty: t } => {
            t
        }
        Expr::MethodCall { receiver: _, method: _, args: _, ty: t } => {
            t
        }
        Expr::FieldAccess { base: _, field: _, ty: t } => {
            t
        }
        Expr::IndexGet { base: _, key: _, ty: t } => {
            t
        }
        Expr::IndexSet { base: _, key: _, value: _, ty: t } => {
            t
        }
        Expr::BinOp { op: _, lhs: _, rhs: _, ty: t } => {
            t
        }
        Expr::UnOp { op: _, operand: _, ty: t } => {
            t
        }
        Expr::StructCtor { name: _, fields: _, ty: t } => {
            t
        }
        Expr::EnumCtor { enum_name: _, variant: _, args: _, ty: t } => {
            t
        }
        Expr::Cast { inner: _, target: _, ty: t } => {
            t
        }
        Expr::MagicCall { magic: _, args: _, ty: t } => {
            t
        }
        Expr::IfExpr { cond: _, then: _, els: _, ty: t } => {
            t
        }
        Expr::Lambda { params: _, body: _, ty: t } => {
            t
        }
        Expr::Pipe { receiver: _, callee: _, args: _, ty: t } => {
            t
        }
        Expr::TupleLit { elems: _, ty: t } => {
            t
        }
        Expr::ListLit { items: _, ty: t } => {
            t
        }
        Expr::BlockExpr { stmts: _, ty: t } => {
            t
        }
        Expr::GenExpr { yield_of: _, ty: t } => {
            t
        }
        Expr::Paren { inner: _, ty: t } => {
            t
        }
        Expr::Range { end: _, inclusive: _, ty: t } => {
            t
        }
        Expr::Dict { pairs: _, ty: t } => {
            t
        }
        Expr::AssignExpr { target: _, value: _, ty: t } => {
            t
        }
        Expr::ImplicitConvert { source: _, target_ty: _, ty: t } => {
            t
        }
    }
}

pub fn gen_expr(e: Expr, fn_raises: MaybeIrType) -> String {
    match e.clone() {
        Expr::LitInt { v: n, ty: t } => {
            n.to_string() + &"i64".to_string().to_string()[..]
        }
        Expr::LitF64 { v: n, ty: t } => {
            fmt_f64(n).to_string()
        }
        Expr::LitStr { s: s, ty: t } => {
            "\"".to_string().to_string() + &esc_rust(s.clone())[..] + &"\".to_string()".to_string().to_string()[..]
        }
        Expr::LitFStr { s: s, ty: t } => {
            gen_fstring(s.clone()).to_string()
        }
        Expr::LitBool { b: b, ty: t } => {
            b.to_string()
        }
        Expr::LitUnit { ty: t } => {
            "()".to_string().to_string()
        }
        Expr::LitNone { ty: t } => {
            "None".to_string().to_string()
        }
        Expr::Var { name: n, ty: t } => {
            n.to_string()
        }
        Expr::Call { callee: c, args: a, ty: t } => {
            let c = *c;
            let a = *a;
            gen_call(c.clone(), a.clone(), fn_raises.clone()).to_string()
        }
        Expr::MethodCall { receiver: r, method: m, args: a, ty: t } => {
            let r = *r;
            let a = *a;
            gen_expr(r.clone(), fn_raises.clone()) + &".".to_string().to_string()[..] + &m.to_string()[..] + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone(), fn_raises.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::FieldAccess { base: b, field: f, ty: t } => {
            let b = *b;
            gen_expr(b.clone(), fn_raises.clone()) + &".".to_string().to_string()[..] + &f.to_string()[..]
        }
        Expr::IndexGet { base: b, key: k, ty: t } => {
            let b = *b;
            let k = *k;
            gen_expr(b.clone(), fn_raises.clone()) + &"[".to_string().to_string()[..] + &gen_expr(k.clone(), fn_raises.clone())[..] + &"]".to_string().to_string()[..]
        }
        Expr::IndexSet { base: b, key: k, value: v, ty: t } => {
            let b = *b;
            let k = *k;
            let v = *v;
            gen_expr(b.clone(), fn_raises.clone()) + &"[".to_string().to_string()[..] + &gen_expr(k.clone(), fn_raises.clone())[..] + &"] = ".to_string().to_string()[..] + &gen_expr(v.clone(), fn_raises.clone())[..]
        }
        Expr::BinOp { op: o, lhs: l, rhs: r, ty: t } => {
            let l = *l;
            let r = *r;
            gen_expr(l.clone(), fn_raises.clone()) + &" ".to_string().to_string()[..] + &o.to_string()[..] + &" ".to_string().to_string()[..] + &gen_expr(r.clone(), fn_raises.clone())[..]
        }
        Expr::UnOp { op: o, operand: p, ty: t } => {
            let p = *p;
            o.to_string() + &gen_expr(p.clone(), fn_raises.clone())[..]
        }
        Expr::StructCtor { name: n, fields: fs, ty: t } => {
            let fs = *fs;
            if (n).to_string() == ("Range".to_string()).to_string() { gen_range_ctor(fs.clone()) } else { if (n).to_string() == ("Dict".to_string()).to_string() { gen_dict_ctor(fs.clone()) } else { n.to_string() + &" { ".to_string().to_string()[..] + &ctor_fields(fs.clone())[..] + &" }".to_string().to_string()[..] } }
        }
        Expr::EnumCtor { enum_name: en, variant: v, args: a, ty: t } => {
            let a = *a;
            en.to_string() + &"::".to_string().to_string()[..] + &v.to_string()[..] + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone(), fn_raises.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::Cast { inner: i, target: tg, ty: t } => {
            let i = *i;
            "(".to_string().to_string() + &gen_expr(i.clone(), fn_raises.clone())[..] + &" as ".to_string().to_string()[..] + &rust_type(tg.clone()).to_string()[..] + &")".to_string().to_string()[..]
        }
        Expr::MagicCall { magic: m, args: a, ty: t } => {
            let a = *a;
            m.to_string() + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone(), fn_raises.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::IfExpr { cond: c, then: th, els: el, ty: t } => {
            let c = *c;
            let th = *th;
            let el = *el;
            "if ".to_string().to_string() + &gen_expr(c.clone(), fn_raises.clone())[..] + &" { ".to_string().to_string()[..] + &gen_expr(th.clone(), fn_raises.clone())[..] + &" } else { ".to_string().to_string()[..] + &gen_expr(el.clone(), fn_raises.clone())[..] + &" }".to_string().to_string()[..]
        }
        Expr::Lambda { params: ps, body: b, ty: t } => {
            let b = *b;
            "move |".to_string().to_string() + &str_join(ps.clone()).to_string()[..] + &"| ".to_string().to_string()[..] + &gen_expr(b.clone(), fn_raises.clone())[..]
        }
        Expr::Pipe { receiver: r, callee: c, args: a, ty: t } => {
            let r = *r;
            let c = *c;
            let a = *a;
            gen_expr(r.clone(), fn_raises.clone()) + &".".to_string().to_string()[..] + &gen_expr(c.clone(), fn_raises.clone())[..] + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone(), fn_raises.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::TupleLit { elems: es, ty: t } => {
            let es = *es;
            "(".to_string().to_string() + &expr_cs_list(es.clone(), fn_raises.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::ListLit { items: xs, ty: t } => {
            let xs = *xs;
            "[".to_string().to_string() + &expr_cs_list(xs.clone(), fn_raises.clone())[..] + &"]".to_string().to_string()[..]
        }
        Expr::BlockExpr { stmts: ss, ty: t } => {
            "{\n".to_string().to_string() + &gen_body(ss.clone(), false, "".to_string().to_string(), fn_raises.clone())[..] + &"}".to_string()[..]
        }
        Expr::GenExpr { yield_of: y, ty: t } => {
            let y = *y;
            "gen ".to_string().to_string() + &gen_expr(y.clone(), fn_raises.clone())[..]
        }
        Expr::Paren { inner: i, ty: t } => {
            let i = *i;
            "(".to_string().to_string() + &gen_expr(i.clone(), fn_raises.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::Range { end: en, inclusive: inc, ty: t } => {
            let en = *en;
            "0_i64..".to_string().to_string() + &gen_expr(en, fn_raises.clone())[..]
        }
        Expr::Dict { pairs: ps, ty: t } => {
            let ps = *ps;
            "HashMap::new()".to_string().to_string()
        }
        Expr::AssignExpr { target: tg, value: v, ty: t } => {
            let tg = *tg;
            let v = *v;
            gen_expr(tg.clone(), fn_raises.clone()) + &" = ".to_string().to_string()[..] + &gen_expr(v.clone(), fn_raises.clone())[..]
        }
        Expr::ImplicitConvert { source: s, target_ty: tt, ty: t } => {
            let s = *s;
            gen_expr(s.clone(), fn_raises.clone())
        }
        _ => {
            "unimplemented!".to_string().to_string()
        }
    }
}

pub fn display_item(i: Item) -> String {
    match i.clone() {
        Item::FnDef { name: n, generics: gs, params: ps, ret: r, exc: _rs, body: b } => {
            "fn ".to_string().to_string() + &n[..]
        }
        Item::Const { name: n, ty: t, value: v } => {
            "const ".to_string().to_string() + &n[..]
        }
        Item::StructDef { name: n, generics: gs, fields: fs } => {
            "struct ".to_string().to_string() + &n[..]
        }
        Item::EnumDef { name: n, generics: gs, variants: vs } => {
            "enum ".to_string().to_string() + &n[..]
        }
        Item::TraitDef { name: n, supertraits: sts, methods: ms } => {
            "trait ".to_string().to_string() + &n[..]
        }
        Item::DuckDef { name: n, method_count: mc } => {
            "duck ".to_string().to_string() + &n[..]
        }
        Item::UseStmt { path: pth, alias: al, items: its, is_from: fr } => {
            "use ".to_string().to_string() + &str_join(pth.clone())[..]
        }
        Item::TypeAlias { name: n, ty: t } => {
            "type ".to_string().to_string() + &n[..]
        }
        Item::Impl { trait_: tr, for_type: ft, method_names: mns } => {
            "impl ".to_string().to_string() + &rust_type(ft.clone())[..]
        }
        Item::CheckerBlock { name: n, ps_name: p } => {
            "checker ".to_string().to_string() + &n[..]
        }
        Item::Test { name: n, body: b } => {
            "test ".to_string().to_string() + &n[..]
        }
        _ => {
            "item".to_string().to_string()
        }
    }
}

pub fn opt_debug(p: MaybeStr) -> String {
    match p.clone() {
        MaybeStr::YesStr { value: v } => {
            v
        }
        MaybeStr::NoStr => {
            "".to_string().to_string()
        }
    }
}

pub fn block_disp(b: BlockIR) -> String {
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            "{\n".to_string().to_string() + &block_with_ty(ss.clone(), t.clone())[..] + &"}".to_string().to_string()[..]
        }
    }
}

pub fn stmt_block(mut ss: Vec<Stmt>) -> String {
    return if (ss.len() as i64) == 0i64 { "{}".to_string().to_string()} else {
        let mut out = "{\n".to_string().to_string();
        for idx in 0i64..(ss.len() as i64) {
            out = LzAdd::__add__(out, display_stmt(ss[idx as usize].clone())) + &"\n".to_string().to_string()[..];
        }
        LzAdd::__add__(out, "}".to_string().to_string())
    };
}

pub fn match_block(s: Expr, mut arms: Vec<(Pattern, MaybeExpr, BlockIR)>) -> String {
    let mut out = "match ".to_string().to_string() + &display_expr(s.clone())[..] + &" {\n".to_string().to_string()[..];
    for idx in 0i64..(arms.len() as i64) {
        let arm = arms[idx as usize].clone();
        let pat_s = display_pattern(arm.0.clone());
        let gd_s = {
match arm.1 {
    MaybeExpr::YesExpr { value: vv } => {
        LzAdd::__add__(" if ".to_string().to_string(), display_expr(vv.clone()))
    }
    MaybeExpr::NoExpr => {
        "".to_string().to_string()
    }
}
    };
        let blk_s = block_disp(arm.2.clone());
        out = LzAdd::__add__(LzAdd::__add__(out + &"    ".to_string().to_string()[..], pat_s) + &gd_s[..] + &" => ".to_string().to_string()[..], blk_s) + &",\n".to_string().to_string()[..];
    }
    return out + &"}".to_string().to_string()[..];
}

pub fn block_with_ty(mut ss: Vec<Stmt>, ty: IrType) -> String {
    return if (ss.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let mut out = "".to_string().to_string();
        for idx in 0i64..(ss.len() as i64) {
            out = LzAdd::__add__(out, display_stmt(ss[idx as usize].clone())) + &"\n".to_string().to_string()[..];
        }
        out
    };
}

pub fn base_is_dict(b: Expr) -> bool {
    match b.clone() {
        Expr::Var { name: n, ty: t } => {
            match t.clone() {
                IrType::Named { path: p, args: a } => {
                    let a = *a;
                    (p).to_string() == ("Dict".to_string()).to_string() || (p).to_string() == ("HashMap".to_string()).to_string()
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

pub fn gen_range_ctor(mut fs: Vec<(String, Expr)>) -> String {
    let mut start_s = "".to_string().to_string();
    let mut end_s = "".to_string().to_string();
    let mut incl: bool = false;
    for idx in 0i64..(fs.len() as i64) {
        let f = fs[idx as usize].clone();
        if f.0 == "start".to_string() {
            start_s = gen_expr(f.1.clone(), MaybeIrType::NoTy);
        } else { if f.0 == "end".to_string() {
            end_s = gen_expr(f.1.clone(), MaybeIrType::NoTy);
        } else { if f.0 == "inclusive".to_string() {
            match f.1 {
                Expr::LitBool { b: bv, ty: tv } => {
                    incl = bv;
                }
                _ => {
                    incl = incl;
                }
            }
        }}}
    }
    return start_s + &(if incl { "..=".to_string().to_string() } else { "..".to_string().to_string() }) + &end_s[..];
}

pub fn gen_dict_ctor(mut fs: Vec<(String, Expr)>) -> String {
    return if (fs.len() as i64) == 0i64 { "std::collections::HashMap::new()".to_string().to_string()} else {
        let mut pairs = "".to_string().to_string();
        let mut i: i64 = 0i64;
        while LzAdd::__add__(i, 1i64) < (fs.len() as i64) {
            let k = fs[i as usize].clone();
            let v = fs[((LzAdd::__add__(i, 1i64)) as usize)].clone();
            pairs = pairs + &"(".to_string().to_string()[..] + &gen_expr(k.1.clone(), MaybeIrType::NoTy)[..] + &", ".to_string().to_string()[..] + &gen_expr(v.1.clone(), MaybeIrType::NoTy)[..] + &")".to_string().to_string()[..];
            if LzAdd::__add__(i, 2i64) < (fs.len() as i64) {
                pairs = pairs + &", ".to_string().to_string()[..];
            }
            i += 2i64;
        }
        LzAdd::__add__("[".to_string().to_string(), pairs) + &"].into_iter().collect()".to_string().to_string()[..]
    };
}

pub fn gen_call(c: Expr, a: Vec<Expr>, fn_raises: MaybeIrType) -> String {
    match c.clone() {
        Expr::Var { name: n, ty: t } => {
            if (n).to_string() == ("print".to_string()).to_string() { "println!(".to_string().to_string() + &print_fmt(a.clone())[..] + &expr_cs_list(a.clone(), fn_raises.clone())[..] + &")".to_string().to_string()[..] } else { if (n).to_string() == ("len".to_string()).to_string() { "(".to_string().to_string() + &expr_cs_list(a.clone(), fn_raises.clone())[..] + &".len() as i64)".to_string().to_string()[..] } else { if (n).to_string() == ("set!".to_string()).to_string() { "[".to_string().to_string() + &expr_cs_list(a.clone(), fn_raises.clone())[..] + &"].into_iter().collect()".to_string().to_string()[..] } else { n.to_string() + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone(), fn_raises.clone())[..] + &")".to_string().to_string()[..] } } }
        }
        _ => {
            gen_expr(c.clone(), fn_raises.clone()) + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone(), fn_raises.clone())[..] + &")".to_string().to_string()[..]
        }
    }
}

pub fn print_fmt(mut as_: Vec<Expr>) -> String {
    return if (as_.len() as i64) == 0i64 { "\"\"".to_string().to_string()} else {
        let mut out = "\"{:?}".to_string().to_string();
        for idx in 1i64..(as_.len() as i64) {
            out.push_str(&" {:?}".to_string().to_string());
        }
        LzAdd::__add__(out, "\", ".to_string().to_string())
    };
}

pub fn expr_cs_list(mut es: Vec<Expr>, fn_raises: MaybeIrType) -> String {
    return if (es.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let head = gen_expr(es[0i64 as usize].clone(), fn_raises.clone());
        if (es.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &expr_cs_list(ec_tail(es.clone()), fn_raises.clone())[..] } else { head }
    };
}

pub fn ec_tail(mut es: Vec<Expr>) -> Vec<Expr> {
    let mut out: Vec<Expr> = Vec::new();
    for idx in 1i64..(es.len() as i64) {
        out.push(es[idx as usize].clone());
    }
    return out;
}

pub fn ctor_fields(mut fs: Vec<(String, Expr)>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(fs.len() as i64) {
        let f = fs[idx as usize].clone();
        out = LzAdd::__add__(LzAdd::__add__(out + &(if idx > 0i64 { ", ".to_string().to_string() } else { "".to_string().to_string() }), f.0.to_string()) + &": ".to_string().to_string()[..], gen_expr(f.1.clone(), MaybeIrType::NoTy));
    }
    return out;
}

pub fn gen_stmt(s: Stmt, is_tail: bool, is_main: bool, auto_mut: String, fn_raises: MaybeIrType) -> String {
    match s.clone() {
        Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
            gen_let(n.clone(), t.clone(), v.clone(), m, r, auto_mut.clone(), fn_raises.clone())
        }
        Stmt::Assign { target: tg, value: v } => {
            match tg.clone() {
                Expr::IndexGet { base: b, key: k, ty: t } => {
                    let b = *b;
                    let k = *k;
                    if base_is_dict(b.clone()) { gen_expr(b.clone(), MaybeIrType::NoTy) + &".insert(".to_string().to_string()[..] + &gen_expr(k.clone(), MaybeIrType::NoTy)[..] + &", ".to_string().to_string()[..] + &gen_expr(v.clone(), MaybeIrType::NoTy)[..] + &");".to_string().to_string()[..] } else { gen_expr(tg.clone(), MaybeIrType::NoTy) + &" = ".to_string().to_string()[..] + &gen_expr(v.clone(), MaybeIrType::NoTy)[..] + &";".to_string().to_string()[..] }
                }
                _ => {
                    gen_expr(tg.clone(), MaybeIrType::NoTy) + &" = ".to_string().to_string()[..] + &gen_expr(v.clone(), MaybeIrType::NoTy)[..] + &";".to_string().to_string()[..]
                }
            }
        }
        Stmt::Return { value: v } => {
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    "return ".to_string().to_string() + &gen_expr(inner.clone(), fn_raises.clone())[..] + &";".to_string().to_string()[..]
                }
                MaybeExpr::NoExpr => {
                    "return;".to_string().to_string()
                }
            }
        }
        Stmt::ExprStmt { expr: e } => {
            let ret_prefix = (if (is_tail && !is_main) { "return ".to_string() } else { "".to_string() }).to_string();
            ret_prefix + &gen_expr(e.clone(), fn_raises.clone())[..] + &";".to_string().to_string()[..]
        }
        Stmt::While { cond: c, guard_e: g, body: b, else_body: eb } => {
            gen_while(c.clone(), b.clone(), fn_raises.clone())
        }
        Stmt::If { cond: c, then: tb, els: eb } => {
            gen_if(c.clone(), tb.clone(), eb.clone(), fn_raises.clone())
        }
        Stmt::Break => {
            "break;".to_string().to_string()
        }
        Stmt::Continue => {
            "continue;".to_string().to_string()
        }
        Stmt::Pass => {
            "();  // pass".to_string().to_string()
        }
        Stmt::For { var: v, iter: it, guard_e: g, body: b, else_body: eb } => {
            "for ".to_string().to_string() + &v[..] + &" in ".to_string().to_string()[..] + &gen_expr(it.clone(), fn_raises.clone())[..] + &" ".to_string().to_string()[..] + &gen_block(b.clone(), fn_raises.clone())[..]
        }
        Stmt::Match { scrutinee: s, arms: as_ } => {
            "match ".to_string().to_string() + &gen_expr(s.clone(), fn_raises.clone())[..] + &" {\n".to_string().to_string()[..] + &gen_match_arms(as_.clone(), fn_raises.clone())[..] + &"}".to_string().to_string()[..]
        }
        Stmt::Block { stmts: ss } => {
            let ss = *ss;
            "{\n".to_string().to_string() + &gen_body(ss.clone(), false, "".to_string().to_string(), fn_raises.clone())[..] + &"}".to_string().to_string()[..]
        }
        Stmt::Assert { cond: c } => {
            "assert!(".to_string().to_string() + &gen_expr(c.clone(), fn_raises.clone())[..] + &");".to_string().to_string()[..]
        }
        Stmt::Raise { value: v } => {
            match fn_raises.clone() {
                MaybeIrType::YesTy { value: _t } => {
                    "return Result::Err(".to_string().to_string() + &gen_expr(v.clone(), fn_raises.clone())[..] + &");".to_string().to_string()[..]
                }
                MaybeIrType::NoTy => {
                    "panic!(".to_string().to_string() + &gen_expr(v.clone(), fn_raises.clone())[..] + &");".to_string().to_string()[..]
                }
            }
        }
        Stmt::TypeAlias { name: n, ty: t } => {
            "type ".to_string().to_string() + &n.to_string()[..] + &" = ".to_string().to_string()[..] + &rust_type(t.clone())[..] + &";".to_string().to_string()[..]
        }
        Stmt::BreakLabel { label: l, value: v } => {
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    "break \'".to_string().to_string() + &l[..] + &"\' ".to_string().to_string()[..] + &gen_expr(inner.clone(), fn_raises.clone())[..] + &";".to_string().to_string()[..]
                }
                MaybeExpr::NoExpr => {
                    "break \'".to_string().to_string() + &l[..] + &"\';".to_string().to_string()[..]
                }
            }
        }
        Stmt::BlockLabel { label: l, body: b } => {
            "\'".to_string().to_string() + &l[..] + &": ".to_string().to_string()[..] + &gen_block(b.clone(), fn_raises.clone())[..]
        }
        Stmt::WhileLet { pattern: p, expr: e, guard_e: g, body: b } => {
            "while let ".to_string().to_string() + &gen_pattern(p.clone())[..] + &" = ".to_string().to_string()[..] + &gen_expr(e.clone(), fn_raises.clone())[..] + &" ".to_string().to_string()[..] + &gen_block(b.clone(), fn_raises.clone())[..]
        }
        Stmt::Defer { body: b } => {
            "{ /* defer */ ".to_string().to_string() + &gen_block(b.clone(), fn_raises.clone())[..] + &" }".to_string().to_string()[..]
        }
        Stmt::CheckerBlock { label: l, ps_name: p } => {
            "();  // checker block (defined at module level)".to_string().to_string()
        }
        Stmt::TryCatch { body: b, catches: cs, else_body: eb, finally_body: fb } => {
            let use_result_try: bool = is_result_body(b.clone());
            let result_marker = (if use_result_try { "// [Result base detected]\n".to_string().to_string() } else { "".to_string().to_string() });
            let try_body = gen_block(b.clone(), MaybeIrType::NoTy);
            let pr_prefix = "{ let __pr = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {".to_string().to_string();
            let pr_mid = " })); ".to_string().to_string();
            let match_expr = if (cs.len() as i64) == 0i64 {
                match eb.clone() {
                    MaybeBlock::YesBlock { value: eb_blk } => {
                        let else_body_s = gen_block(eb_blk.clone(), MaybeIrType::NoTy);
                        LzAdd::__add__("match __pr { Ok(_v) => { _v; ".to_string().to_string(), else_body_s) + &" }, Err(_p) => () }".to_string().to_string()[..]
                    }
                    _ => {
                        "__pr.unwrap_or_else(|_p| ())".to_string().to_string()
                    }
                }
            } else {
                let last_idx: i64 = (cs.len() as i64) - 1i64;
                let mut skipped_comments = String::new();
                for ci in 0i64..last_idx {
                    let pre_catch = cs[ci as usize].clone();
                    let pre_pat = pre_catch.0;
                    let cmt = {
                match pre_pat.clone() {
                    MaybePattern::YesPat { value: pp } => {
                        match pp.clone() {
                            Pattern::Ident { name: pn } => {
                                LzAdd::__add__("// catch ".to_string().to_string(), pn) + &" skipped\n".to_string().to_string()[..]
                            }
                            Pattern::Enum { enum_name: en, variant: vv, args: _ } => {
                                LzAdd::__add__(LzAdd::__add__("// catch ".to_string().to_string(), en) + &"::".to_string().to_string()[..], vv) + &" skipped\n".to_string().to_string()[..]
                            }
                            _ => {
                                "// catch _ skipped\n".to_string().to_string()
                            }
                        }
                    }
                    _ => {
                        "// catch _ skipped\n".to_string().to_string()
                    }
                }
                    };
                    skipped_comments.push_str(&cmt);
                }
                let last_catch = cs[last_idx as usize].clone();
                let pat = last_catch.0;
                let catch_blk = last_catch.1;
                let catch_body = gen_block(catch_blk.clone(), MaybeIrType::NoTy);
                let mut binds = String::new();
                let _ = match pat.clone() {
                    MaybePattern::YesPat { value: p2 } => {
                        match p2.clone() {
                            Pattern::Ident { name: n } => {
                                let safe_n: String = if (n).to_string() == ("line".to_string()).to_string() || (n).to_string() == ("column".to_string()).to_string() || (n).to_string() == ("file".to_string()).to_string() { n.to_string() + &"_".to_string().to_string()[..] } else { n };
                                binds = "let ".to_string().to_string() + &safe_n[..] + &" = format!(\"{:?}\", _p); ".to_string().to_string()[..];
                            }
                            Pattern::Enum { enum_name: en, variant: v, args: a } => {
                                let a = *a;
                                for ai in 0i64..(a.len() as i64) {
                                    let arg_p = a[ai as usize].clone();
                                    match arg_p.clone() {
                                        Pattern::Ident { name: n2 } => {
                                            let safe_n2 = if n2 == "line".to_string() || n2 == "column".to_string() || n2 == "file".to_string() { LzAdd::__add__(n2, "_".to_string().to_string()) } else { n2 };
                                            binds = binds + &"let ".to_string().to_string()[..] + &safe_n2[..] + &" = format!(\"{:?}\", _p); ".to_string().to_string()[..];
                                        }
                                        _ => {
                                            ();
                                        }
                                    }
                                }
                            }
                            _ => {
                                ()
                            }
                        }
                    }
                    _ => {
                        ()
                    }
                };
                let ok_arm = {
                match eb.clone() {
                    MaybeBlock::YesBlock { value: eb_blk2 } => {
                        LzAdd::__add__("{ _v; ".to_string().to_string(), gen_block(eb_blk2.clone(), MaybeIrType::NoTy)) + &" }".to_string().to_string()[..]
                    }
                    _ => {
                        "_v".to_string().to_string()
                    }
                }
                    };
                LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(skipped_comments, "match __pr { Ok(_v) => ".to_string().to_string()), ok_arm) + &", Err(_p) => { ".to_string().to_string()[..], binds), catch_body) + &" } }".to_string().to_string()[..]
            };
            let try_val_stmt = "let __try_val = ".to_string().to_string() + &match_expr[..] + &"; ".to_string().to_string()[..];
            let finally_part = {
match fb.clone() {
    MaybeBlock::YesBlock { value: fb_blk } => {
        let fb_body = gen_block(fb_blk.clone(), MaybeIrType::NoTy);
        LzAdd::__add__("let __final_val = __try_val; ".to_string().to_string(), fb_body) + &"; __final_val".to_string().to_string()[..]
    }
    _ => {
        "__try_val".to_string().to_string()
    }
}
    };
            result_marker + &pr_prefix[..] + &try_body[..] + &pr_mid[..] + &try_val_stmt[..] + &finally_part[..] + &" }".to_string().to_string()[..]
        }
        Stmt::Yield { value: v } => {
            "__gen_vec.push(".to_string().to_string() + &gen_expr(v.clone(), fn_raises.clone())[..] + &");".to_string().to_string()[..]
        }
        Stmt::YieldFrom { iter: it } => {
            "__gen_vec.extend(".to_string().to_string() + &gen_expr(it.clone(), fn_raises.clone())[..] + &".into_iter());".to_string().to_string()[..]
        }
        _ => {
            "// TODO stmt".to_string().to_string()
        }
    }
}

pub fn gen_while(c: Expr, b: BlockIR, fn_raises: MaybeIrType) -> String {
    let inf: bool = is_true_cond(c.clone());
    return if inf && block_is_pass_only(b.clone()) { "loop {\n        unimplemented!()\n    }".to_string().to_string() } else { "while ".to_string().to_string() + &gen_expr(c.clone(), fn_raises.clone())[..] + &" ".to_string().to_string()[..] + &gen_block(b.clone(), fn_raises.clone())[..] };
}

pub fn gen_if(c: Expr, tb: BlockIR, eb: MaybeBlock, fn_raises: MaybeIrType) -> String {
    let base = "if ".to_string().to_string() + &gen_expr(c.clone(), fn_raises.clone())[..] + &" ".to_string().to_string()[..] + &gen_block(tb.clone(), fn_raises.clone())[..];
    match eb.clone() {
        MaybeBlock::YesBlock { value: b } => {
            base + &" else ".to_string().to_string()[..] + &gen_block(b.clone(), fn_raises.clone())[..]
        }
        MaybeBlock::NoBlock => {
            base
        }
    }
}

pub fn is_true_cond(c: Expr) -> bool {
    match c.clone() {
        Expr::LitBool { b: bv, ty: tv } => {
            bv
        }
        _ => {
            false
        }
    }
}

pub fn block_is_pass_only(b: BlockIR) -> bool {
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            if (ss.len() as i64) == 1i64 {
                match ss[0i64 as usize].clone() {
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

pub fn gen_let(mut n: String, t: IrType, v: Expr, m: bool, r: bool, auto_mut: String, fn_raises: MaybeIrType) -> String {
    let mut_kw = (if (m || str_contains(auto_mut.clone(), n.clone())) { "mut ".to_string() } else { "".to_string() }).to_string();
    let ref_kw = (if r { "ref ".to_string() } else { "".to_string() }).to_string();
    let ty_s = rust_type(t.clone());
    let empty_c: bool = is_empty_container(v.clone());
    let force: bool = empty_c && is_dict_or_set_ty(t.clone());
    return if skip_ty_ann(t.clone()) && !force { "let ".to_string().to_string() + &mut_kw[..] + &ref_kw[..] + &n.to_string()[..] + &" = ".to_string().to_string()[..] + &gen_expr(v.clone(), fn_raises.clone())[..] + &";".to_string().to_string()[..] } else { "let ".to_string().to_string() + &mut_kw[..] + &ref_kw[..] + &n.to_string()[..] + &": ".to_string().to_string()[..] + &ty_s[..] + &" = ".to_string().to_string()[..] + &gen_expr(v.clone(), fn_raises.clone())[..] + &";".to_string().to_string()[..] };
}

pub fn is_empty_container(v: Expr) -> bool {
    match v.clone() {
        Expr::ListLit { items: xs, ty: t } => {
            let xs = *xs;
            (xs.len() as i64) == 0i64
        }
        Expr::StructCtor { name: nn, fields: fs, ty: t } => {
            let fs = *fs;
            (nn).to_string() == ("Dict".to_string()).to_string() && (fs.len() as i64) == 0i64
        }
        _ => {
            false
        }
    }
}

pub fn is_dict_or_set_ty(t: IrType) -> bool {
    match t.clone() {
        IrType::Named { path: p, args: a } => {
            let a = *a;
            (p).to_string() == ("Dict".to_string()).to_string() || (p).to_string() == ("Set".to_string()).to_string()
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
        IrType::Duck { fields: fs } => {
            let fs = *fs;
            true
        }
        IrType::Generic { name: nn } => {
            true
        }
        IrType::FnType { params: ps, ret: rr } => {
            let ps = *ps;
            let rr = *rr;
            true
        }
        IrType::Named { path: p, args: a } => {
            let a = *a;
            (p).to_string() == ("Dict".to_string()).to_string() || (p).to_string() == ("Set".to_string()).to_string() || (p).to_string() == ("Range".to_string()).to_string() || (p).to_string() == ("Nil".to_string()).to_string() || args_is_empty_or_generic(a.clone())
        }
        IrType::Ref { inner: x } => {
            let x = *x;
            match x.clone() {
                IrType::Generic { name: nn } => {
                    true
                }
                _ => {
                    false
                }
            }
        }
        IrType::MutRef { inner: x } => {
            let x = *x;
            match x.clone() {
                IrType::Generic { name: nn } => {
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

pub fn args_is_empty_or_generic(mut a: Vec<IrType>) -> bool {
    return if (a.len() as i64) == 0i64 { true} else {
        let mut has_generic: bool = false;
        for idx in 0i64..(a.len() as i64) {
            let ai = a[idx as usize].clone();
            match ai.clone() {
                IrType::Generic { name: nn } => {
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

pub fn fmt_f64(mut n: f64) -> String {
    let s = n.to_string();
    return if has_dot_or_e(s.clone()) { s + &"f64".to_string().to_string()[..] } else { s + &".0f64".to_string().to_string()[..] };
}

pub fn has_dot_or_e(mut s: String) -> bool {
    let len: i64 = (s.len() as i64);
    let mut i: i64 = 0i64;
    let mut found: bool = false;
    while i < len {
        let c: String = s[((i) as usize)..(((i + 1i64)) as usize)].to_string();
        if (c).to_string() == (".".to_string()).to_string() || (c).to_string() == ("e".to_string()).to_string() {
            found = true;
        }
        i += 1i64;
    }
    return found;
}

pub fn gen_fstring(mut s: String) -> String {
    let mut fmt = "".to_string().to_string();
    let mut args: Vec<String> = Vec::new();
    let n: i64 = (s.len() as i64);
    let mut i: i64 = 0i64;
    while i < n {
        let c: String = s[((i) as usize)..(((i + 1i64)) as usize)].to_string();
        if (c).to_string() == ("{".to_string()).to_string() { if i + 1i64 < n {
            let c2: String = s[(((i + 1i64)) as usize)..(((i + 2i64)) as usize)].to_string();
            if c2 == "{".to_string() {
                fmt.push_str(&"{{".to_string().to_string());
                i += 2i64;
            } else {
                let mut j: i64 = i + 1i64;
                while j < n {
                    let cj: String = s[((j) as usize)..(((LzAdd::__add__(j, 1i64))) as usize)].to_string();
                    if (cj).to_string() == ("}".to_string()).to_string() {
                        break;
                    }
                    j += 1i64;
                }
                let expr: String = s[(((i + 1i64)) as usize)..((j) as usize)].to_string();
                fmt.push_str(&"{:?}".to_string().to_string());
                args.push(expr);
                i = LzAdd::__add__(j, 1i64);
            }
        } else {
            fmt.push_str(&"{".to_string().to_string());
            i += 1i64;
        }} else { if (c).to_string() == ("}".to_string()).to_string() { if i + 1i64 < n {
            let c2: String = s[(((i + 1i64)) as usize)..(((i + 2i64)) as usize)].to_string();
            if c2 == "}".to_string() {
                fmt.push_str(&"}}".to_string().to_string());
                i += 2i64;
            } else {
                fmt.push_str(&"}".to_string().to_string());
                i += 1i64;
            }
        } else {
            fmt.push_str(&"}".to_string().to_string());
            i += 1i64;
        }} else {
            fmt.push_str(&c);
            i += 1i64;
        }}
    }
    let fmt_q = "\"".to_string().to_string() + &esc_quote_only(fmt.clone())[..] + &"\"".to_string().to_string()[..];
    return if (args.len() as i64) == 0i64 { fmt_q + &".to_string()".to_string().to_string()[..] } else { "format!(".to_string().to_string() + &fmt_q[..] + &", ".to_string().to_string()[..] + &args_join(args.clone())[..] + &")".to_string().to_string()[..] };
}

pub fn args_join(mut xs: Vec<String>) -> String {
    return if (xs.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let head = xs[0i64 as usize].to_string();
        if (xs.len() as i64) > 1i64 { LzAdd::__add__(head.to_string(), ", ".to_string().to_string()) + &args_join(aj_tail(xs.clone()))[..] } else { head.to_string() }
    };
}

pub fn aj_tail(mut xs: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for idx in 1i64..(xs.len() as i64) {
        out.push(xs[idx as usize].to_string());
    }
    return out;
}

pub fn esc_quote_only(mut s: String) -> String {
    let len: i64 = (s.len() as i64);
    let mut out = "".to_string().to_string();
    let mut i: i64 = 0i64;
    while i < len {
        let c = s[((i) as usize)..(((i + 1i64)) as usize)].to_string();
        if c == "\"".to_string() {
            out.push_str(&"\\\"".to_string().to_string());
        } else {
            out.push_str(&c);
        }
        i += 1i64;
    }
    return out;
}

pub fn esc_rust(mut s: String) -> String {
    let len: i64 = (s.len() as i64);
    let mut out = "".to_string().to_string();
    let mut i: i64 = 0i64;
    while i < len {
        let c: String = s[((i) as usize)..(((i + 1i64)) as usize)].to_string();
        if (c).to_string() == ("\\".to_string()).to_string() {
            out.push_str("\\\\");
        } else { if (c).to_string() == ("\"".to_string()).to_string() {
            out.push_str("\\\"");
        } else { if (c).to_string() == ("\n".to_string()).to_string() {
            out.push_str("\\n");
        } else { if (c).to_string() == ("\t".to_string()).to_string() {
            out.push_str("\\t");
        } else { if (c).to_string() == ("\r".to_string()).to_string() {
            out.push_str("\\r");
        } else {
            out.push_str(&c);
        }}}}}
        i += 1i64;
    }
    return out;
}

pub fn gen_trait_def(mut name: String, mut supertraits: Vec<IrType>, methods: Vec<(String, Vec<IrType>, IrType)>) -> String {
    let st_str = if (supertraits.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let mut st_parts = "".to_string().to_string();
        for idx in 0i64..(supertraits.len() as i64) {
            let st = supertraits[idx as usize].clone();
            if idx > 0i64 {
                st_parts = LzAdd::__add__(st_parts, " + ".to_string().to_string());
            }
            st_parts = LzAdd::__add__(st_parts, rust_type(st.clone()));
        }
        LzAdd::__add__(": ".to_string().to_string(), st_parts)
    };
    return "pub trait ".to_string().to_string() + &name.to_string()[..] + &st_str[..] + &" {\n".to_string().to_string()[..] + &gen_trait_methods(methods.clone())[..] + &"\n}".to_string().to_string()[..];
}

pub fn gen_trait_methods(mut methods: Vec<(String, Vec<IrType>, IrType)>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(methods.len() as i64) {
        let m = methods[idx as usize].clone();
        let mut params = "".to_string().to_string();
        for pidx in 0i64..(m.1.len() as i64) {
            let pt = m.1[pidx as usize].clone();
            if pidx > 0i64 {
                params = params + &", ".to_string().to_string()[..];
            }
            params = LzAdd::__add__(params, rust_type(pt.clone()));
        }
        out = LzAdd::__add__(LzAdd::__add__(out + &"    fn ".to_string().to_string()[..], m.0.to_string()) + &"(".to_string().to_string()[..] + &params[..] + &") -> ".to_string().to_string()[..], rust_type(m.2.clone())) + &";\n".to_string().to_string()[..];
    }
    return out;
}

pub fn gen_impl_def(trait_: MaybeIrType, for_type: IrType, method_names: Vec<String>) -> String {
    let trait_part = {
match trait_.clone() {
    MaybeIrType::YesTy { value: ty } => {
        LzAdd::__add__(rust_type(ty.clone()), " for ".to_string().to_string())
    }
    MaybeIrType::NoTy => {
        "".to_string().to_string()
    }
}
    };
    return "impl ".to_string().to_string() + &trait_part[..] + &rust_type(for_type.clone())[..] + &" {\n".to_string().to_string()[..] + &gen_impl_methods(method_names.clone())[..] + &"\n}".to_string().to_string()[..];
}

pub fn gen_impl_methods(mut method_names: Vec<String>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(method_names.len() as i64) {
        let mut mn = method_names[idx as usize].to_string();
        out = out + &"    fn ".to_string().to_string()[..] + &mn.to_string()[..] + &"(&self) {\n        unimplemented!()\n    }\n".to_string().to_string()[..];
    }
    return out;
}

pub fn gen_use_stmt(mut path: Vec<String>, alias: MaybeStr, mut items: Vec<String>, is_from: bool) -> String {
    let mut path_parts = "".to_string().to_string();
    for idx in 0i64..(path.len() as i64) {
        let mut ps = path[idx as usize].to_string();
        if idx > 0i64 {
            path_parts = path_parts + &"::".to_string().to_string()[..];
        }
        path_parts = path_parts + &ps.to_string()[..];
    }
    let path_str = path_parts;
    return if is_from { if (items.len() as i64) == 1i64 && (items[0i64 as usize]).to_string() == ("*".to_string()).to_string() { "use ".to_string().to_string() + &path_str[..] + &"::*;".to_string().to_string()[..]} else {
        let mut item_parts = "".to_string().to_string();
        for idx in 0i64..(items.len() as i64) {
            let it = items[idx as usize].to_string();
            if idx > 0i64 {
                item_parts = LzAdd::__add__(item_parts, ", ".to_string().to_string());
            }
            item_parts.push_str(&it.to_string());
        }
        LzAdd::__add__("use ".to_string().to_string() + &path_str[..] + &"::{".to_string().to_string()[..], item_parts) + &"}".to_string().to_string()[..]
    }} else { "use ".to_string().to_string() + &path_str[..] + &";".to_string().to_string()[..]};
}

pub fn gen_type_alias(mut name: String, ty: IrType) -> String {
    return "pub type ".to_string().to_string() + &name.to_string()[..] + &" = ".to_string().to_string()[..] + &rust_type(ty.clone())[..] + &";".to_string().to_string()[..];
}

pub fn gen_test_def(mut name: String, body: Vec<Stmt>) -> String {
    return "#[test]\nfn ".to_string().to_string() + &name.to_string()[..] + &"() {\n".to_string().to_string()[..] + &gen_body(body.clone(), false, auto_mut_names(body.clone()), MaybeIrType::NoTy)[..] + &"}".to_string().to_string()[..];
}

pub fn gen_item(i: Item) -> String {
    match i.clone() {
        Item::FnDef { name: n, generics: gs, params: ps, ret: r, exc: rs, body: b } => {
            let is_main: bool = ((n).to_string() == ("main".to_string()).to_string());
            let ret_s = if (is_main && r_eq_unit(r.clone())) { "".to_string().to_string() } else { (" -> ".to_string().to_string() + &rust_type(r.clone())[..]) };
            "pub fn ".to_string().to_string() + &n.to_string()[..] + &generic_sig(gs.clone())[..] + &"(".to_string().to_string()[..] + &fn_param_list(ps.clone())[..] + &")".to_string().to_string()[..] + &ret_s[..] + &" {\n".to_string().to_string()[..] + &gen_body(b.clone(), is_main, auto_mut_names(b.clone()), rs.clone())[..] + &"}".to_string().to_string()[..]
        }
        Item::StructDef { name: n, generics: gs, fields: fs } => {
            "#[derive(Debug, Clone, PartialEq)]\npub struct ".to_string().to_string() + &n.to_string()[..] + &generic_sig(gs.clone())[..] + &" {\n".to_string().to_string()[..] + &gen_struct_fields(fs.clone())[..] + &"}".to_string().to_string()[..]
        }
        Item::EnumDef { name: n, generics: gs, variants: vs } => {
            "#[derive(Debug, Clone, PartialEq)]\npub enum ".to_string().to_string() + &n.to_string()[..] + &generic_sig(gs.clone())[..] + &" {\n".to_string().to_string()[..] + &gen_enum_variants(vs.clone())[..] + &"}".to_string().to_string()[..]
        }
        Item::Const { name: n, ty: t, value: v } => {
            gen_const_item(n.clone(), t.clone(), v.clone())
        }
        Item::TraitDef { name: n, supertraits: sts, methods: ms } => {
            gen_trait_def(n.clone(), sts.clone(), ms.clone())
        }
        Item::Impl { trait_: tr, for_type: ft, method_names: mns } => {
            gen_impl_def(tr.clone(), ft.clone(), mns.clone())
        }
        Item::UseStmt { path: pth, alias: al, items: its, is_from: fr } => {
            gen_use_stmt(pth.clone(), al.clone(), its.clone(), fr)
        }
        Item::TypeAlias { name: n, ty: t } => {
            gen_type_alias(n.clone(), t.clone())
        }
        Item::Test { name: n, body: b } => {
            gen_test_def(n.clone(), b.clone())
        }
        Item::CheckerBlock { name: n, ps_name: p } => {
            "fn ".to_string().to_string() + &n.to_string()[..] + &"(ps: &mut __Params) { unimplemented!() }".to_string().to_string()[..]
        }
        Item::DuckDef { name: n, method_count: mc } => {
            "pub trait ".to_string().to_string() + &n.to_string()[..] + &" { /* ".to_string().to_string()[..] + &mc.to_string()[..] + &" methods */ }".to_string().to_string()[..]
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

pub fn fn_param_list(mut ps: Vec<(String, IrType, bool, bool, bool)>) -> String {
    return if (ps.len() as i64) == 0i64 { "".to_string().to_string()} else {
        let p0 = ps[0i64 as usize].clone();
        fn_param_str(p0.clone()) + &(if (ps.len() as i64) > 1i64 { ", ".to_string().to_string() + &fn_param_list(pl_tail(ps.clone()))[..] } else { "".to_string() })
    };
}

pub fn fn_param_str(p: (String, IrType, bool, bool, bool)) -> String {
    return p.0.to_string() + &": ".to_string().to_string()[..] + &rust_type(p.1.clone())[..];
}

pub fn pl_tail(mut ps: Vec<(String, IrType, bool, bool, bool)>) -> Vec<(String, IrType, bool, bool, bool)> {
    let mut out: Vec<(String, IrType, bool, bool, bool)> = Vec::new();
    for idx in 1i64..(ps.len() as i64) {
        out.push(ps[idx as usize].clone());
    }
    return out;
}

pub fn gen_body(mut ss: Vec<Stmt>, is_main: bool, auto_mut: String, fn_raises: MaybeIrType) -> String {
    let n: i64 = (ss.len() as i64);
    return if n == 0i64 { "    loop {\n        unimplemented!()\n    }\n".to_string().to_string()} else {
        let mut out = String::new();
        for idx in 0i64..n {
            let is_tail: bool = (idx == n - 1i64);
            out = LzAdd::__add__(out + &"    ".to_string().to_string()[..], gen_stmt(ss[idx as usize].clone(), is_tail, is_main, auto_mut.clone(), fn_raises.clone())) + &"\n".to_string().to_string()[..];
        }
        out
    };
}

pub fn stmt_type_label(s: Stmt) -> String {
    match s.clone() {
        Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
            "Let".to_string().to_string()
        }
        Stmt::ExprStmt { expr: e } => {
            "Expr".to_string().to_string()
        }
        Stmt::TryCatch { body: b, catches: cs, else_body: eb, finally_body: fb } => {
            "Try".to_string().to_string()
        }
        Stmt::Block { stmts: b } => {
            let b = *b;
            "Block".to_string().to_string()
        }
        Stmt::Defer { body: b } => {
            "Defer".to_string().to_string()
        }
        Stmt::If { cond: c, then: t, els: e } => {
            "If".to_string().to_string()
        }
        Stmt::For { var: v, iter: it, guard_e: g, body: b, else_body: eb } => {
            "For".to_string().to_string()
        }
        _ => {
            "Other".to_string().to_string()
        }
    }
}

pub fn auto_mut_names(mut ss: Vec<Stmt>) -> String {
    let mut out = String::new();
    for idx in 0i64..(ss.len() as i64) {
        out = scan_stmt_auto_mut(ss[idx as usize].clone(), out.clone());
    }
    return out;
}

pub fn scan_stmt_auto_mut(s: Stmt, mut out: String) -> String {
    let mut acc = out.clone();
    match s.clone() {
        Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
            scan_expr_auto_mut(v.clone(), out.clone())
        }
        Stmt::Assign { target: tg, value: v } => {
            acc = add_var_auto_mut(tg.clone(), acc.clone());
            acc = scan_expr_auto_mut(tg.clone(), acc.clone());
            acc = scan_expr_auto_mut(v.clone(), acc.clone());
            acc
        }
        Stmt::ExprStmt { expr: e } => {
            scan_expr_auto_mut(e.clone(), out.clone())
        }
        Stmt::Return { value: v } => {
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    scan_expr_auto_mut(inner.clone(), out.clone())
                }
                MaybeExpr::NoExpr => {
                    out
                }
            }
        }
        Stmt::If { cond: c, then: t, els: e } => {
            acc = scan_expr_auto_mut(c.clone(), acc.clone());
            acc = scan_block_stmts_auto_mut(t.clone(), acc.clone());
            acc = scan_block_maybe_auto_mut(e.clone(), acc.clone());
            acc
        }
        _ => {
            out
        }
    }
}

pub fn scan_block_stmts_auto_mut(b: BlockIR, mut out: String) -> String {
    let mut acc = out.clone();
    match b.clone() {
        BlockIR::Block { stmts: bss, ty: bt } => {
            for idx in 0i64..(bss.len() as i64) {
                acc = scan_stmt_auto_mut(bss[idx as usize].clone(), acc.clone());
            }
            acc
        }
    }
}

pub fn scan_block_maybe_auto_mut(mb: MaybeBlock, mut out: String) -> String {
    let mut acc = out.clone();
    match mb.clone() {
        MaybeBlock::YesBlock { value: es } => {
            scan_block_stmts_auto_mut(es.clone(), acc.clone())
        }
        MaybeBlock::NoBlock => {
            out
        }
    }
}

pub fn add_var_auto_mut(e: Expr, mut out: String) -> String {
    match e.clone() {
        Expr::Var { name: n, ty: t } => {
            if str_contains(out.clone(), n.clone()) { out.clone()} else {
                let sep = if (out.len() as i64) > 0i64 { ",".to_string().to_string() } else { "".to_string().to_string() };
                LzAdd::__add__(out, sep) + &n.to_string()[..]
            }
        }
        _ => {
            out.clone()
        }
    }
}

pub fn str_contains(mut s: String, mut sub: String) -> bool {
    let len: i64 = (s.len() as i64);
    let sub_len: i64 = (sub.len() as i64);
    return if sub_len == 0i64 { false} else {
        let mut i: i64 = 0i64;
        let mut found: bool = false;
        while LzAdd::__add__(i, sub_len) <= len {
            if s[((i) as usize)..(((LzAdd::__add__(i, sub_len))) as usize)].to_string() == sub {
                found = true;
            }
            i += 1i64;
        }
        found
    };
}

pub fn scan_expr_auto_mut(e: Expr, mut out: String) -> String {
    let mut acc = out.clone();
    match e.clone() {
        Expr::Call { callee: c, args: a, ty: t } => {
            let c = *c;
            let a = *a;
            acc = scan_expr_auto_mut(c.clone(), acc.clone());
            for idx in 0i64..(a.len() as i64) {
                acc = scan_expr_auto_mut(a[idx as usize].clone(), acc.clone());
            }
            if callee_is_mut_fn(c.clone()) && (a.len() as i64) > 0i64 {
                acc = add_var_auto_mut(a[0i64 as usize].clone(), acc.clone());
            }
            acc
        }
        Expr::MethodCall { receiver: r, method: m, args: a, ty: t } => {
            let r = *r;
            let a = *a;
            acc = add_var_auto_mut(r.clone(), acc.clone());
            acc = scan_expr_auto_mut(r.clone(), acc.clone());
            for idx in 0i64..(a.len() as i64) {
                acc = scan_expr_auto_mut(a[idx as usize].clone(), acc.clone());
            }
            acc
        }
        Expr::IndexSet { base: b, key: k, value: v, ty: t } => {
            let b = *b;
            let k = *k;
            let v = *v;
            acc = add_var_auto_mut(b.clone(), acc.clone());
            acc = scan_expr_auto_mut(b.clone(), acc.clone());
            acc = scan_expr_auto_mut(k.clone(), acc.clone());
            acc = scan_expr_auto_mut(v.clone(), acc.clone());
            acc
        }
        Expr::AssignExpr { target: tg, value: v, ty: t } => {
            let tg = *tg;
            let v = *v;
            acc = add_var_auto_mut(tg.clone(), acc.clone());
            acc = scan_expr_auto_mut(tg.clone(), acc.clone());
            acc = scan_expr_auto_mut(v.clone(), acc.clone());
            acc
        }
        Expr::FieldAccess { base: b, field: f, ty: t } => {
            let b = *b;
            scan_expr_auto_mut(b.clone(), out.clone())
        }
        Expr::IndexGet { base: b, key: k, ty: t } => {
            let b = *b;
            let k = *k;
            acc = scan_expr_auto_mut(b.clone(), acc.clone());
            scan_expr_auto_mut(k.clone(), acc.clone())
        }
        _ => {
            out
        }
    }
}

pub fn callee_is_mut_fn(c: Expr) -> bool {
    match c.clone() {
        Expr::Var { name: nn, ty: tt } => {
            if (nn).to_string() == ("add".to_string()).to_string() || (nn).to_string() == ("push".to_string()).to_string() || (nn).to_string() == ("append".to_string()).to_string() || (nn).to_string() == ("pop".to_string()).to_string() || (nn).to_string() == ("extend".to_string()).to_string() || (nn).to_string() == ("insert".to_string()).to_string() || (nn).to_string() == ("remove".to_string()).to_string() || (nn).to_string() == ("delete".to_string()).to_string() { true } else { false }
        }
        _ => {
            false
        }
    }
}

pub fn gen_struct_fields(mut fs: Vec<(String, IrType)>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(fs.len() as i64) {
        let f = fs[idx as usize].clone();
        out = LzAdd::__add__(LzAdd::__add__(out + &"    pub ".to_string().to_string()[..], f.0.to_string()) + &": ".to_string().to_string()[..], rust_type(f.1.clone())) + &",\n".to_string().to_string()[..];
    }
    return out;
}

pub fn gen_enum_variants(mut vs: Vec<(String, Vec<IrType>)>) -> String {
    let mut out = "".to_string().to_string();
    for idx in 0i64..(vs.len() as i64) {
        let v = vs[idx as usize].clone();
        out = LzAdd::__add__(LzAdd::__add__(out + &"    ".to_string().to_string()[..], v.0.to_string()), gen_enum_variant_args(v.1.clone())) + &",\n".to_string().to_string()[..];
    }
    return out;
}

pub fn gen_enum_variant_args(mut ts: Vec<IrType>) -> String {
    return if (ts.len() as i64) == 0i64 { "".to_string().to_string() } else { "(".to_string().to_string() + &type_rust_list(ts.clone())[..] + &")".to_string().to_string()[..] };
}

pub fn gen_const_item(mut n: String, t: IrType, v: Expr) -> String {
    match t.clone() {
        IrType::Str => {
            match v.clone() {
                Expr::LitStr { s: sv, ty: tv } => {
                    "const ".to_string().to_string() + &n.to_string()[..] + &": &str = \"".to_string().to_string()[..] + &esc_rust(sv.clone())[..] + &"\";".to_string().to_string()[..]
                }
                _ => {
                    "const ".to_string().to_string() + &n.to_string()[..] + &": String = ".to_string().to_string()[..] + &gen_expr(v.clone(), MaybeIrType::NoTy)[..] + &";".to_string().to_string()[..]
                }
            }
        }
        _ => {
            "const ".to_string().to_string() + &n.to_string()[..] + &": ".to_string().to_string()[..] + &rust_type(t.clone())[..] + &" = ".to_string().to_string()[..] + &gen_expr(v.clone(), MaybeIrType::NoTy)[..] + &";".to_string().to_string()[..]
        }
    }
}

pub fn is_magic_name(n: String) -> bool {
    return (n).to_string() == ("__name__".to_string()).to_string() || (n).to_string() == ("__file__".to_string()).to_string() || (n).to_string() == ("__package__".to_string()).to_string() || (n).to_string() == ("__path__".to_string()).to_string() || (n).to_string() == ("__doc__".to_string()).to_string() || (n).to_string() == ("__is_macro__".to_string()).to_string();
}

pub fn gen_magic_const(mut n: String, t: IrType, v: Expr) -> String {
    return if (n).to_string() == ("__is_macro__".to_string()).to_string() { "const ".to_string().to_string() + &n.to_string()[..] + &": bool = ".to_string().to_string()[..] + &gen_expr(v.clone(), MaybeIrType::NoTy)[..] + &";".to_string().to_string()[..] } else { "const ".to_string().to_string() + &n.to_string()[..] + &": &str = ".to_string().to_string()[..] + &gen_expr(v.clone(), MaybeIrType::NoTy)[..] + &";".to_string().to_string()[..] };
}

pub fn codegen_module(m: IrModule) -> String {
    let mut out = module_header();
    for idx in 0i64..(m.items.len() as i64) {
        out = LzAdd::__add__(out, gen_item(m.items[idx as usize].clone())) + &"\n\n".to_string().to_string()[..];
    }
    return out[((0i64) as usize)..((((out.len() as i64) - 2i64)) as usize)].to_string().clone() + &"\n".to_string().to_string()[..];
}

pub fn module_header() -> String {
    return "\n// \u{2500}\u{2500} LZ rustc link recipe (BUG-10: lz_builtins re-exports num-bigint/num-complex) \u{2500}\u{2500}\n// To compile & run this generated file in isolation:\n//   rustc --edition 2021 --extern lz_builtins=<rlib> -L dependency=<deps_dir> -O <this_file>.rs\n//   <rlib>     = liblz_builtins.rlib (build it: `cargo build -p lz_builtins`)\n//   <deps_dir> = the directory containing that rlib (e.g. <workspace>/target/debug/deps)\n// Omitting -L gives error[E0463] even if this program never uses BigInt/Complex.\n// \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n\n#[allow(unused_imports)]\n#[allow(unused_variables)]\n#[allow(dead_code)]\n#[allow(non_snake_case)]\n\nuse std::collections::{HashMap, HashSet};\nuse std::any::Any;\nuse std::rc::Rc;\nuse std::sync::Arc;\nuse std::fmt::Debug;\nuse std::fmt::Display;\n\nuse lz_builtins::*;\n\n".to_string().to_string();
}

const __name__: &str = "main";

const __file__: &str = "lz_codegen_lib.lz";

const __package__: &str = "ir";

const __path__: &str = "E:\\IDEProjects\\AI\\lang-zone\\src\\ir";

const __doc__: &str = "";

const __is_macro__: bool = false;

pub fn main() {
}

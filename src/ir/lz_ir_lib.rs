
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
    // STMT:Other
    match t.clone() {
        IrType::Int => {
            // STMT:Expr
            "int".to_string()
        }
        IrType::F64 => {
            // STMT:Expr
            "f64".to_string()
        }
        IrType::Str => {
            // STMT:Expr
            "str".to_string()
        }
        IrType::Bool => {
            // STMT:Expr
            "bool".to_string()
        }
        IrType::Unit => {
            // STMT:Expr
            "()".to_string()
        }
        IrType::Never => {
            // STMT:Expr
            "!".to_string()
        }
        IrType::Any => {
            // STMT:Expr
            "?".to_string()
        }
        IrType::Self_ => {
            // STMT:Expr
            "Self".to_string()
        }
        IrType::Generic { name: g } => {
            // STMT:Expr
            g
        }
        IrType::Named { path: p, args: a } => {
            let a = *a;
            // STMT:Expr
            p + &(if (a.len() as i64) > 0i64 { LzAdd::__add__("<".to_string(), type_list(a.clone())) + &">".to_string()[..] } else { "".to_string() })
        }
        IrType::Opt { inner: x } => {
            let x = *x;
            // STMT:Expr
            LzAdd::__add__("Option<".to_string(), display_type(x.clone())) + &">".to_string()[..]
        }
        IrType::Res { ok: o, err: e } => {
            let o = *o;
            let e = *e;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("Result<".to_string(), display_type(o.clone())) + &", ".to_string()[..], display_type(e.clone())) + &">".to_string()[..]
        }
        IrType::Tuple { elems: es } => {
            let es = *es;
            // STMT:Expr
            LzAdd::__add__("(".to_string(), type_list(es.clone())) + &")".to_string()[..]
        }
        IrType::FnType { params: ps, ret: r } => {
            let ps = *ps;
            let r = *r;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("fn(".to_string(), type_list(ps.clone())) + &") -> ".to_string()[..], display_type(r.clone()))
        }
        IrType::Ref { inner: x } => {
            let x = *x;
            // STMT:Expr
            LzAdd::__add__("&".to_string(), display_type(x.clone()))
        }
        IrType::MutRef { inner: x } => {
            let x = *x;
            // STMT:Expr
            LzAdd::__add__("&mut ".to_string(), display_type(x.clone()))
        }
        IrType::Duck { fields: fs } => {
            let fs = *fs;
            // STMT:Expr
            LzAdd::__add__("duck {".to_string(), duck_fields(fs.clone())) + &"}".to_string()[..]
        }
    }
}

pub fn type_list(ts: Vec<IrType>) -> String {
    // STMT:Expr
    return if (ts.len() as i64) == 0i64 { "".to_string()} else {
        // STMT:Let
        let head: String = display_type(ts[((0i64) as usize)].clone());
        // STMT:Expr
        if (ts.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string()) + &type_list(tail_t(ts.clone()))[..] } else { head }
    };
}

pub fn tail_t(ts: Vec<IrType>) -> Vec<IrType> {
    // STMT:Let
    let mut out: Vec<IrType> = Vec::new();
    // STMT:For
    for idx in (1i64..(ts.len() as i64)).into_iter() {
        // STMT:Expr
        out.push(ts[((idx) as usize)].clone());
    }
    // STMT:Expr
    return out;
}

pub fn duck_fields(fs: Vec<(String, IrType)>) -> String {
    // STMT:Let
    let mut out: String = "".to_string();
    // STMT:For
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        // STMT:Let
        let f = fs[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(out + &(if idx > 0i64 { ", ".to_string() } else { "".to_string() }), f.0) + &": ".to_string()[..], display_type(f.1.clone()));
    }
    // STMT:Expr
    return out;
}

pub fn display_expr(e: Expr) -> String {
    // STMT:Other
    match e.clone() {
        Expr::LitInt { v: n, ty: t } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] ".to_string()[..], n.to_string()) + &"_i64".to_string()[..]
        }
        Expr::LitF64 { v: n, ty: t } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] ".to_string()[..], n.to_string()) + &"_f64".to_string()[..]
        }
        Expr::LitStr { s: s, ty: t } => {
            // STMT:Expr
            LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] \"".to_string()[..] + &s[..] + &"\"".to_string()[..]
        }
        Expr::LitFStr { s: s, ty: t } => {
            // STMT:Expr
            LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] f\"".to_string()[..] + &s[..] + &"\"".to_string()[..]
        }
        Expr::LitBool { b: b, ty: t } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] ".to_string()[..], b.to_string())
        }
        Expr::LitUnit { ty: t } => {
            // STMT:Expr
            LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] ()".to_string()[..]
        }
        Expr::LitNone { ty: t } => {
            // STMT:Expr
            LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] None".to_string()[..]
        }
        Expr::Var { name: n, ty: t } => {
            // STMT:Expr
            LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] ".to_string()[..] + &n[..]
        }
        Expr::Call { callee: c, args: a, ty: t } => {
            let c = *c;
            let a = *a;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] call ".to_string()[..], display_expr(c.clone())), arg_list(a.clone()))
        }
        Expr::MethodCall { receiver: r, method: m, args: a, ty: t } => {
            let r = *r;
            let a = *a;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] method ".to_string()[..], display_expr(r.clone())) + &".".to_string()[..] + &m[..], arg_list(a.clone()))
        }
        Expr::FieldAccess { base: b, field: f, ty: t } => {
            let b = *b;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] field ".to_string()[..], display_expr(b.clone())) + &".".to_string()[..] + &f[..]
        }
        Expr::IndexGet { base: b, key: k, ty: t } => {
            let b = *b;
            let k = *k;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] index ".to_string()[..], display_expr(b.clone())) + &"[".to_string()[..], display_expr(k.clone())) + &"]".to_string()[..]
        }
        Expr::IndexSet { base: b, key: k, value: v, ty: t } => {
            let b = *b;
            let k = *k;
            let v = *v;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] index_set ".to_string()[..], display_expr(b.clone())) + &"[".to_string()[..], display_expr(k.clone())) + &"] = ".to_string()[..], display_expr(v.clone()))
        }
        Expr::BinOp { op: o, lhs: l, rhs: r, ty: t } => {
            let l = *l;
            let r = *r;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] binop ".to_string()[..], display_expr(l.clone())) + &" ".to_string()[..] + &o[..] + &" ".to_string()[..], display_expr(r.clone()))
        }
        Expr::UnOp { op: o, operand: p, ty: t } => {
            let p = *p;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] unop ".to_string()[..] + &o[..] + &" ".to_string()[..], display_expr(p.clone()))
        }
        Expr::StructCtor { name: n, fields: fs, ty: t } => {
            let fs = *fs;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] new ".to_string()[..] + &n[..], field_pairs(fs.clone()))
        }
        Expr::EnumCtor { enum_name: en, variant: v, args: a, ty: t } => {
            let a = *a;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] ".to_string()[..] + &en[..] + &"::".to_string()[..] + &v[..], arg_list(a.clone()))
        }
        Expr::Cast { inner: i, target: tg, ty: t } => {
            let i = *i;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] cast ".to_string()[..], display_expr(i.clone())) + &" as ".to_string()[..], display_type(tg.clone()))
        }
        Expr::MagicCall { magic: m, args: a, ty: t } => {
            let a = *a;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] magic ".to_string()[..] + &m[..], arg_list(a.clone()))
        }
        Expr::IfExpr { cond: c, then: th, els: el, ty: t } => {
            let c = *c;
            let th = *th;
            let el = *el;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] if ".to_string()[..], display_expr(c.clone())) + &" then ".to_string()[..], display_expr(th.clone())) + &" else ".to_string()[..], display_expr(el.clone()))
        }
        Expr::Lambda { params: ps, body: b, ty: t } => {
            let b = *b;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] |".to_string()[..], str_join(ps.clone())) + &"| ".to_string()[..], display_expr(b.clone()))
        }
        Expr::Pipe { receiver: r, callee: c, args: a, ty: t } => {
            let r = *r;
            let c = *c;
            let a = *a;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] pipe ".to_string()[..], display_expr(r.clone())) + &" |> ".to_string()[..], display_expr(c.clone())), arg_list(a.clone()))
        }
        Expr::TupleLit { elems: es, ty: t } => {
            let es = *es;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] (".to_string()[..], expr_list(es.clone())) + &")".to_string()[..]
        }
        Expr::ListLit { items: xs, ty: t } => {
            let xs = *xs;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] [".to_string()[..], expr_list(xs.clone())) + &"]".to_string()[..]
        }
        Expr::BlockExpr { stmts: ss, ty: t } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] block ".to_string()[..], block_with_ty(ss.clone(), t.clone()))
        }
        Expr::GenExpr { yield_of: y, ty: t } => {
            let y = *y;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] gen ".to_string()[..], display_expr(y.clone()))
        }
        Expr::Paren { inner: i, ty: t } => {
            let i = *i;
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] (".to_string()[..], display_expr(i.clone())) + &")".to_string()[..]
        }
        Expr::Range { end: en, inclusive: inc, ty: t } => {
            let en = *en;
            // STMT:Expr
            LzAdd::__add__("[".to_string(), display_type(t)) + &"] <expr>".to_string()[..]
        }
        Expr::Dict { pairs: ps, ty: t } => {
            let ps = *ps;
            // STMT:Expr
            LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] <expr>".to_string()[..]
        }
        Expr::AssignExpr { target: tg, value: v, ty: t } => {
            let tg = *tg;
            let v = *v;
            // STMT:Expr
            LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] <expr>".to_string()[..]
        }
        Expr::ImplicitConvert { source: s, target_ty: tt, ty: t } => {
            let s = *s;
            // STMT:Expr
            LzAdd::__add__("[".to_string(), display_type(t.clone())) + &"] <expr>".to_string()[..]
        }
    }
}

pub fn arg_list(as_: Vec<Expr>) -> String {
    // STMT:Expr
    return if (as_.len() as i64) == 0i64 { "".to_string() } else { "(".to_string().to_string() + &expr_list(as_.clone())[..] + &")".to_string()[..] };
}

pub fn expr_list(es: Vec<Expr>) -> String {
    // STMT:Expr
    return if (es.len() as i64) == 0i64 { "".to_string()} else {
        // STMT:Let
        let head: String = display_expr(es[((0i64) as usize)].clone());
        // STMT:Expr
        if (es.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string()) + &expr_list(tail_e(es.clone()))[..] } else { head }
    };
}

pub fn tail_e(es: Vec<Expr>) -> Vec<Expr> {
    // STMT:Let
    let mut out: Vec<Expr> = Vec::new();
    // STMT:For
    for idx in (1i64..(es.len() as i64)).into_iter() {
        // STMT:Expr
        out.push(es[((idx) as usize)].clone());
    }
    // STMT:Expr
    return out;
}

pub fn field_pairs(fs: Vec<(String, Expr)>) -> String {
    // STMT:Expr
    return if (fs.len() as i64) == 0i64 { "".to_string()} else {
        // STMT:Let
        let mut out: String = "{ ".to_string();
        // STMT:For
        for idx in (0i64..(fs.len() as i64)).into_iter() {
            // STMT:Let
            out = LzAdd::__add__(out + &(if idx > 0i64 { ", ".to_string() } else { "".to_string() }), pair_str(fs[((idx) as usize)].clone()));
        }
        // STMT:Expr
        LzAdd::__add__(out, " }".to_string())
    };
}

pub fn pair_str(p: (String, Expr)) -> String {
    // STMT:Expr
    return LzAdd::__add__(p.0, ": ".to_string()) + &display_expr(p.1.clone())[..];
}

pub fn display_stmt(s: Stmt) -> String {
    // STMT:Other
    match s.clone() {
        Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(let_kw(m, r), n) + &": ".to_string()[..], display_type(t.clone())) + &" = ".to_string()[..], display_expr(v.clone()))
        }
        Stmt::Assign { target: t, value: v } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(display_expr(t.clone()), " = ".to_string()), display_expr(v.clone()))
        }
        Stmt::Return { value: v } => {
            // STMT:Other
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    // STMT:Expr
                    LzAdd::__add__("return ".to_string(), display_expr(inner.clone()))
                }
                MaybeExpr::NoExpr => {
                    // STMT:Expr
                    "return".to_string()
                }
            }
        }
        Stmt::ExprStmt { expr: e } => {
            // STMT:Expr
            display_expr(e.clone())
        }
        Stmt::If { cond: c, then: t, els: e } => {
            // STMT:Let
            let base: String = LzAdd::__add__(LzAdd::__add__("if ".to_string(), display_expr(c.clone())) + &" ".to_string()[..], block_disp(t.clone()));
            // STMT:Other
            match e.clone() {
                MaybeBlock::YesBlock { value: es } => {
                    // STMT:Expr
                    LzAdd::__add__(base + &" else ".to_string()[..], block_disp(es.clone()))
                }
                MaybeBlock::NoBlock => {
                    // STMT:Expr
                    base
                }
            }
        }
        Stmt::For { var: v, iter: it, guard_e: g, body: b, else_body: eb } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("for ".to_string().to_string() + &v[..] + &" in ".to_string()[..], display_expr(it.clone())), guard_s(g.clone())) + &" ".to_string()[..], block_disp(b.clone())), else_s(eb.clone()))
        }
        Stmt::While { cond: c, guard_e: g, body: b, else_body: eb } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("while ".to_string(), display_expr(c.clone())), guard_s(g.clone())) + &" ".to_string()[..], block_disp(b.clone())), else_s(eb.clone()))
        }
        Stmt::Block { stmts: b } => {
            let b = *b;
            // STMT:Expr
            stmt_block(b.clone())
        }
        Stmt::Match { scrutinee: s, arms: as_ } => {
            // STMT:Expr
            match_block(s.clone(), as_.clone())
        }
        Stmt::Yield { value: v } => {
            // STMT:Expr
            LzAdd::__add__("yield ".to_string(), display_expr(v.clone()))
        }
        Stmt::YieldFrom { iter: it } => {
            // STMT:Expr
            LzAdd::__add__("yield from ".to_string(), display_expr(it.clone()))
        }
        Stmt::Break => {
            // STMT:Expr
            "break".to_string()
        }
        Stmt::BreakLabel { label: l, value: v } => {
            // STMT:Let
            let base: String = "break \'".to_string().to_string() + &l[..];
            // STMT:Other
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    // STMT:Expr
                    LzAdd::__add__(base + &" ".to_string()[..], display_expr(inner.clone()))
                }
                MaybeExpr::NoExpr => {
                    // STMT:Expr
                    base
                }
            }
        }
        Stmt::Continue => {
            // STMT:Expr
            "continue".to_string()
        }
        Stmt::BlockLabel { label: l, body: b } => {
            // STMT:Expr
            LzAdd::__add__("block \'".to_string().to_string() + &l[..] + &" ".to_string()[..], block_disp(b.clone()))
        }
        Stmt::Defer { body: b } => {
            // STMT:Expr
            LzAdd::__add__("defer ".to_string(), block_disp(b.clone()))
        }
        Stmt::TryCatch { body: b, catches: cs, else_body: eb, finally_body: fb } => {
            // STMT:Let
            let mut out: String = LzAdd::__add__("try ".to_string(), block_disp(b.clone()));
            // STMT:Let
            out = LzAdd::__add__(out, try_catches_str(cs.clone()));
            // STMT:Let
            out = LzAdd::__add__(out, else_s(eb.clone()));
            // STMT:Let
            out = LzAdd::__add__(out, finally_s(fb.clone()));
            // STMT:Expr
            out
        }
        Stmt::WhileLet { pattern: p, expr: e, guard_e: g, body: b } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("while let ".to_string(), display_pattern(p.clone())) + &" = ".to_string()[..], display_expr(e.clone())), guard_s(g.clone())) + &" ".to_string()[..], block_disp(b.clone()))
        }
        Stmt::CheckerBlock { label: l, ps_name: p } => {
            // STMT:Expr
            LzAdd::__add__("block \'".to_string().to_string() + &l[..] + &"[ps:".to_string()[..], opt_debug(p.clone())) + &"]".to_string()[..]
        }
        Stmt::Raise { value: v } => {
            // STMT:Expr
            "<stmt>".to_string()
        }
        Stmt::Assert { cond: c } => {
            // STMT:Expr
            "<stmt>".to_string()
        }
        Stmt::TypeAlias { name: n, ty: t } => {
            // STMT:Expr
            "<stmt>".to_string()
        }
        Stmt::Pass => {
            // STMT:Expr
            "<stmt>".to_string()
        }
    }
}

pub fn let_kw(m: bool, r: bool) -> String {
    // STMT:Expr
    return (if (m && r) { "let mut ref ".to_string() } else { (if r { "let ref ".to_string() } else { (if m { "let mut ".to_string() } else { "let ".to_string() }) }) });
}

pub fn guard_s(g: MaybeExpr) -> String {
    // STMT:Other
    match g.clone() {
        MaybeExpr::YesExpr { value: e } => {
            // STMT:Expr
            LzAdd::__add__(" if ".to_string(), display_expr(e.clone()))
        }
        MaybeExpr::NoExpr => {
            // STMT:Expr
            "".to_string()
        }
    }
}

pub fn else_s(eb: MaybeBlock) -> String {
    // STMT:Other
    match eb.clone() {
        MaybeBlock::YesBlock { value: b } => {
            // STMT:Expr
            LzAdd::__add__(" else ".to_string(), block_disp(b.clone()))
        }
        MaybeBlock::NoBlock => {
            // STMT:Expr
            "".to_string()
        }
    }
}

pub fn finally_s(fb: MaybeBlock) -> String {
    // STMT:Other
    match fb.clone() {
        MaybeBlock::YesBlock { value: b } => {
            // STMT:Expr
            LzAdd::__add__(" finally ".to_string(), block_disp(b.clone()))
        }
        MaybeBlock::NoBlock => {
            // STMT:Expr
            "".to_string()
        }
    }
}

pub fn try_catches_str(cs: Vec<(MaybePattern, BlockIR)>) -> String {
    // STMT:Let
    let mut out: String = "".to_string();
    // STMT:For
    for idx in (0i64..(cs.len() as i64)).into_iter() {
        // STMT:Let
        let c = cs[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(out, catch_str(c.clone()));
    }
    // STMT:Expr
    return out;
}

pub fn catch_str(c: (MaybePattern, BlockIR)) -> String {
    // STMT:Other
    match c.0 {
        MaybePattern::YesPat { value: p } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(" catch(".to_string(), display_pattern(p.clone())) + &") ".to_string()[..], block_disp(c.1.clone()))
        }
        MaybePattern::NoPat => {
            // STMT:Expr
            LzAdd::__add__(" catch ".to_string(), block_disp(c.1.clone()))
        }
    }
}

pub fn display_pattern(p: Pattern) -> String {
    // STMT:Other
    match p.clone() {
        Pattern::Wildcard => {
            // STMT:Expr
            "_".to_string()
        }
        Pattern::Ident { name: n } => {
            // STMT:Expr
            n
        }
        Pattern::RefMutIdent { name: n } => {
            // STMT:Expr
            "ref mut ".to_string().to_string() + &n[..]
        }
        Pattern::LitInt { v: n } => {
            // STMT:Expr
            LzAdd::__add__(n.to_string(), "_i64".to_string())
        }
        Pattern::LitStr { s: s } => {
            // STMT:Expr
            "\"".to_string().to_string() + &s[..] + &"\"".to_string()[..]
        }
        Pattern::LitBool { b: b } => {
            // STMT:Expr
            b.to_string()
        }
        Pattern::LitF64 { v: n } => {
            // STMT:Expr
            LzAdd::__add__(n.to_string(), "_f64".to_string())
        }
        Pattern::Tuple { elems: es } => {
            let es = *es;
            // STMT:Expr
            LzAdd::__add__("(".to_string(), pat_list(es.clone())) + &")".to_string()[..]
        }
        Pattern::List { elems: es } => {
            let es = *es;
            // STMT:Expr
            LzAdd::__add__("[".to_string(), pat_list(es.clone())) + &"]".to_string()[..]
        }
        Pattern::Struct { name: n, fields: fs } => {
            let fs = *fs;
            // STMT:Expr
            LzAdd::__add__(n, pat_struct_fields(fs.clone()))
        }
        Pattern::Enum { enum_name: en, variant: v, args: a } => {
            let a = *a;
            // STMT:Expr
            LzAdd::__add__(en + &"::".to_string()[..] + &v[..], arg_pat_list(a.clone()))
        }
        Pattern::Rest { name: rn } => {
            // STMT:Other
            match rn.clone() {
                MaybeStr::YesStr { value: n2 } => {
                    // STMT:Expr
                    "..".to_string().to_string() + &n2[..]
                }
                MaybeStr::NoStr => {
                    // STMT:Expr
                    "..".to_string()
                }
            }
        }
        Pattern::Range { start: st, end: en, inclusive: inc } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(st.to_string(), (if inc { "..=".to_string() } else { "..".to_string() })), en.to_string())
        }
    }
}

pub fn pat_list(ps: Vec<Pattern>) -> String {
    // STMT:Expr
    return if (ps.len() as i64) == 0i64 { "".to_string()} else {
        // STMT:Let
        let head: String = display_pattern(ps[((0i64) as usize)].clone());
        // STMT:Expr
        if (ps.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string()) + &pat_list(tail_pat(ps.clone()))[..] } else { head }
    };
}

pub fn tail_pat(ps: Vec<Pattern>) -> Vec<Pattern> {
    // STMT:Let
    let mut out: Vec<Pattern> = Vec::new();
    // STMT:For
    for idx in (1i64..(ps.len() as i64)).into_iter() {
        // STMT:Expr
        out.push(ps[((idx) as usize)].clone());
    }
    // STMT:Expr
    return out;
}

pub fn arg_pat_list(ps: Vec<Pattern>) -> String {
    // STMT:Expr
    return if (ps.len() as i64) == 0i64 { "".to_string() } else { "(".to_string().to_string() + &pat_list(ps.clone())[..] + &")".to_string()[..] };
}

pub fn pat_struct_fields(fs: Vec<(String, Pattern)>) -> String {
    // STMT:Expr
    return if (fs.len() as i64) == 0i64 { "".to_string()} else {
        // STMT:Let
        let mut out: String = "{ ".to_string();
        // STMT:For
        for idx in (0i64..(fs.len() as i64)).into_iter() {
            // STMT:Let
            let f = fs[((idx) as usize)].clone();
            // STMT:Let
            out = LzAdd::__add__(LzAdd::__add__(out + &(if idx > 0i64 { ", ".to_string() } else { "".to_string() }), f.0) + &": ".to_string()[..], display_pattern(f.1.clone()));
        }
        // STMT:Expr
        LzAdd::__add__(out, " }".to_string())
    };
}

pub fn match_block(s: Expr, as_: Vec<(Pattern, MaybeExpr, BlockIR)>) -> String {
    // STMT:Let
    let mut out: String = "match ".to_string().to_string() + &display_expr(s.clone())[..] + &" {".to_string()[..];
    // STMT:For
    for idx in (0i64..(as_.len() as i64)).into_iter() {
        // STMT:Let
        let arm = as_[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(out + &" ".to_string()[..], display_pattern(arm.0.clone())), guard_s(arm.1.clone())) + &" => ".to_string()[..], block_disp(arm.2.clone()));
    }
    // STMT:Expr
    return out + &" }".to_string()[..];
}

pub fn stmt_block(ss: Vec<Stmt>) -> String {
    // STMT:Expr
    return if (ss.len() as i64) == 0i64 { "{ }".to_string()} else {
        // STMT:Let
        let mut out: String = "{\n".to_string();
        // STMT:For
        for idx in (0i64..(ss.len() as i64)).into_iter() {
            // STMT:Let
            out = LzAdd::__add__(out + &"  ".to_string()[..], display_stmt(ss[((idx) as usize)].clone())) + &"\n".to_string()[..];
        }
        // STMT:Expr
        LzAdd::__add__(out, "}".to_string())
    };
}

pub fn block_disp(b: BlockIR) -> String {
    // STMT:Other
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            // STMT:Expr
            if (ss.len() as i64) == 0i64 { LzAdd::__add__("{ } [".to_string(), display_type(t.clone())) + &"]".to_string()[..]} else {
                // STMT:Let
                let mut out: String = LzAdd::__add__("{ [".to_string(), display_type(t.clone())) + &"]\n".to_string()[..];
                // STMT:For
                for idx in (0i64..(ss.len() as i64)).into_iter() {
                    // STMT:Let
                    out = LzAdd::__add__(out + &"  ".to_string()[..], display_stmt(ss[((idx) as usize)].clone())) + &"\n".to_string()[..];
                }
                // STMT:Expr
                LzAdd::__add__(out, "}".to_string())
            }
        }
    }
}

pub fn block_with_ty(ss: Vec<Stmt>, t: IrType) -> String {
    // STMT:Expr
    return if (ss.len() as i64) == 0i64 { "{ } [".to_string().to_string() + &display_type(t.clone())[..] + &"]".to_string()[..]} else {
        // STMT:Let
        let mut out: String = "{ [".to_string().to_string() + &display_type(t.clone())[..] + &"]\n".to_string()[..];
        // STMT:For
        for idx in (0i64..(ss.len() as i64)).into_iter() {
            // STMT:Let
            out = LzAdd::__add__(out + &"  ".to_string()[..], display_stmt(ss[((idx) as usize)].clone())) + &"\n".to_string()[..];
        }
        // STMT:Expr
        LzAdd::__add__(out, "}".to_string())
    };
}

pub fn opt_debug(o: MaybeStr) -> String {
    // STMT:Other
    match o.clone() {
        MaybeStr::YesStr { value: s } => {
            // STMT:Expr
            "Some(\"".to_string().to_string() + &s[..] + &"\")".to_string()[..]
        }
        MaybeStr::NoStr => {
            // STMT:Expr
            "None".to_string()
        }
    }
}

pub fn stmt_lines(ss: Vec<Stmt>) -> String {
    // STMT:Let
    let mut out: String = "".to_string();
    // STMT:For
    for idx in (0i64..(ss.len() as i64)).into_iter() {
        // STMT:Other
        out = LzAdd::__add__(out + &"  ".to_string()[..], display_stmt(ss[((idx) as usize)].clone())) + &"\n".to_string()[..];
    }
    // STMT:Expr
    return out;
}

pub fn display_item(it: Item) -> String {
    // STMT:Other
    match it.clone() {
        Item::FnDef { name: n, generics: gs, params: ps, ret: r, body: b } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(LzAdd::__add__("fn ".to_string().to_string() + &n[..], generic_sig(gs.clone())), param_sig(ps.clone())) + &" -> ".to_string()[..], display_type(r.clone())) + &":\n".to_string()[..], stmt_lines(b.clone()))
        }
        Item::Const { name: n, ty: t, value: v } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("const ".to_string().to_string() + &n[..] + &": ".to_string()[..], display_type(t.clone())) + &" = ".to_string()[..], display_expr(v.clone()))
        }
        Item::StructDef { name: n, generics: gs, fields: fs } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("struct ".to_string().to_string() + &n[..], generic_sig(gs.clone())) + &" {\n".to_string()[..], field_lines(fs.clone())) + &"}".to_string()[..]
        }
        Item::EnumDef { name: n, generics: gs, variants: vs } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("enum ".to_string().to_string() + &n[..], generic_sig(gs.clone())) + &" {\n".to_string()[..], variant_lines(vs.clone())) + &"}".to_string()[..]
        }
        Item::TraitDef { name: n, supertraits: ss, methods: ms } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__("trait ".to_string().to_string() + &n[..], super_join(ss.clone())) + &" {\n".to_string()[..], method_lines(ms.clone())) + &"}".to_string()[..]
        }
        Item::DuckDef { name: n, method_count: c } => {
            // STMT:Expr
            LzAdd::__add__("duck ".to_string().to_string() + &n[..] + &" { ".to_string()[..], c.to_string()) + &" methods }".to_string()[..]
        }
        Item::UseStmt { path: p, alias: a, items: xs, is_from: f } => {
            // STMT:Expr
            (if f { (LzAdd::__add__(LzAdd::__add__("from ".to_string(), dot_join(p.clone())) + &" import ".to_string()[..], str_join(xs.clone()))) } else { (LzAdd::__add__("import ".to_string(), dot_join(p.clone()))) })
        }
        Item::TypeAlias { name: n, ty: t } => {
            // STMT:Expr
            LzAdd::__add__("type ".to_string().to_string() + &n[..] + &" = ".to_string()[..], display_type(t.clone()))
        }
        Item::Impl { trait_: tr, for_type: fty, method_names: ms } => {
            // STMT:Let
            let mut out: String = LzAdd::__add__(LzAdd::__add__("impl ".to_string(), impl_head(tr.clone())), display_type(fty.clone())) + &" {\n".to_string()[..];
            // STMT:For
            for idx in (0i64..(ms.len() as i64)).into_iter() {
                // STMT:Other
                out = LzAdd::__add__(out + &"  fn ".to_string()[..], ms[((idx) as usize)].clone()) + &" ...\n".to_string()[..];
            }
            // STMT:Expr
            out + &"}".to_string()[..]
        }
        Item::CheckerBlock { name: n, ps_name: p } => {
            // STMT:Expr
            LzAdd::__add__("checker block \'".to_string().to_string() + &n[..] + &"[ps:".to_string()[..], opt_debug(p.clone())) + &"]".to_string()[..]
        }
        Item::Test { name: n, body: b } => {
            // STMT:Expr
            LzAdd::__add__("test ".to_string().to_string() + &n[..] + &" ".to_string()[..], stmt_block(b.clone()))
        }
    }
}

pub fn generic_sig(gs: Vec<String>) -> String {
    // STMT:Expr
    return if (gs.len() as i64) == 0i64 { "".to_string() } else { "<".to_string().to_string() + &str_join(gs.clone())[..] + &">".to_string()[..] };
}

pub fn variant_lines(vs: Vec<(String, Vec<IrType>)>) -> String {
    // STMT:Let
    let mut out: String = "".to_string();
    // STMT:For
    for idx in (0i64..(vs.len() as i64)).into_iter() {
        // STMT:Let
        let v = vs[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(out + &"  ".to_string()[..], v.0), variant_args(v.1.clone())) + &"\n".to_string()[..];
    }
    // STMT:Expr
    return out;
}

pub fn field_lines(fs: Vec<(String, IrType)>) -> String {
    // STMT:Let
    let mut out: String = "".to_string();
    // STMT:For
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        // STMT:Let
        let f = fs[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(out + &"  ".to_string()[..], f.0) + &": ".to_string()[..], display_type(f.1.clone())) + &"\n".to_string()[..];
    }
    // STMT:Expr
    return out;
}

pub fn variant_args(ts: Vec<IrType>) -> String {
    // STMT:Expr
    return if (ts.len() as i64) == 0i64 { "".to_string() } else { "(".to_string().to_string() + &type_list(ts.clone())[..] + &")".to_string()[..] };
}

pub fn super_join(ss: Vec<IrType>) -> String {
    // STMT:Expr
    return if (ss.len() as i64) == 0i64 { "".to_string()} else {
        // STMT:Let
        let mut out: String = " : ".to_string().to_string() + &display_type(ss[((0i64) as usize)].clone())[..];
        // STMT:For
        for idx in (1i64..(ss.len() as i64)).into_iter() {
            // STMT:Let
            out = LzAdd::__add__(out + &" + ".to_string()[..], display_type(ss[((idx) as usize)].clone()));
        }
        // STMT:Expr
        out
    };
}

pub fn method_lines(ms: Vec<(String, Vec<IrType>, IrType)>) -> String {
    // STMT:Let
    let mut out: String = "".to_string();
    // STMT:For
    for idx in (0i64..(ms.len() as i64)).into_iter() {
        // STMT:Let
        let m = ms[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(LzAdd::__add__(out + &"  fn ".to_string()[..], m.0) + &"(".to_string()[..], type_list(m.1.clone())) + &") -> ".to_string()[..], display_type(m.2.clone())) + &"\n".to_string()[..];
    }
    // STMT:Expr
    return out;
}

pub fn param_sig(ps: Vec<(String, IrType, bool, bool, bool)>) -> String {
    // STMT:Expr
    return if (ps.len() as i64) == 0i64 { "()".to_string() } else { "(".to_string().to_string() + &param_list(ps.clone())[..] + &")".to_string()[..] };
}

pub fn param_list(ps: Vec<(String, IrType, bool, bool, bool)>) -> String {
    // STMT:Expr
    return if (ps.len() as i64) == 0i64 { "".to_string()} else {
        // STMT:Let
        let p0 = ps[((0i64) as usize)].clone();
        // STMT:Expr
        param_str(p0.clone()) + &(if (ps.len() as i64) > 1i64 { ", ".to_string().to_string() + &param_list(tail_p(ps.clone()))[..] } else { "".to_string() })
    };
}

pub fn param_str(p: (String, IrType, bool, bool, bool)) -> String {
    // STMT:Expr
    return LzAdd::__add__((if p.3 { "mut ".to_string() } else { "".to_string() }) + &(if p.4 { "owned ".to_string() } else { "".to_string() }) + &(if p.2 { "ref ".to_string() } else { "".to_string() }), p.0) + &": ".to_string()[..] + &display_type(p.1.clone())[..];
}

pub fn tail_p(ps: Vec<(String, IrType, bool, bool, bool)>) -> Vec<(String, IrType, bool, bool, bool)> {
    // STMT:Let
    let mut out: Vec<(String, IrType, bool, bool, bool)> = Vec::new();
    // STMT:For
    for idx in (1i64..(ps.len() as i64)).into_iter() {
        // STMT:Expr
        out.push(ps[((idx) as usize)].clone());
    }
    // STMT:Expr
    return out;
}

pub fn impl_head(tr: MaybeIrType) -> String {
    // STMT:Other
    match tr.clone() {
        MaybeIrType::YesTy { value: t } => {
            // STMT:Expr
            LzAdd::__add__(display_type(t.clone()), " for ".to_string())
        }
        MaybeIrType::NoTy => {
            // STMT:Expr
            "".to_string()
        }
    }
}

pub fn dot_join(xs: Vec<String>) -> String {
    // STMT:Expr
    return if (xs.len() as i64) == 0i64 { "".to_string()} else {
        // STMT:Let
        let mut out = xs[((0i64) as usize)].clone();
        // STMT:For
        for idx in (1i64..(xs.len() as i64)).into_iter() {
            // STMT:Let
            out = LzAdd::__add__(out + &".".to_string()[..], xs[((idx) as usize)].clone());
        }
        // STMT:Expr
        out
    };
}

pub fn display_module(m: IrModule) -> String {
    // STMT:Let
    let mut out: String = LzAdd::__add__(";; LZIR v".to_string(), m.version.to_string()) + &" \u{2014} module \'".to_string()[..] + &m.name + &"\'\n".to_string()[..];
    // STMT:Let
    out = LzAdd::__add__(out + &";; ".to_string()[..], (m.items.len() as i64).to_string()) + &" items\n".to_string()[..];
    // STMT:Expr
    if (m.prelude.len() as i64) > 0i64 {
        // STMT:Let
        out = out + &";; prelude: ".to_string()[..] + &str_join(m.prelude.clone())[..] + &"\n".to_string()[..];
    } else { ()};
    // STMT:Let
    out = out + &"\n".to_string()[..];
    // STMT:For
    for idx in (0i64..(m.items.len() as i64)).into_iter() {
        // STMT:Other
        out = LzAdd::__add__(out, display_item(m.items[((idx) as usize)].clone())) + &"\n\n".to_string()[..];
    }
    // STMT:Expr
    return out;
}

pub fn str_join(xs: Vec<String>) -> String {
    // STMT:Expr
    return if (xs.len() as i64) == 0i64 { "".to_string()} else {
        // STMT:Let
        let mut out = xs[((0i64) as usize)].clone();
        // STMT:For
        for idx in (1i64..(xs.len() as i64)).into_iter() {
            // STMT:Let
            out = LzAdd::__add__(out + &", ".to_string()[..], xs[((idx) as usize)].clone());
        }
        // STMT:Expr
        out
    };
}

const __name__: &str = "main";

const __file__: &str = "F:\\AI\\lang-zone\\src\\ir\\lz_ir_lib.lz";

const __package__: &str = "ir";

const __path__: &str = "F:\\AI\\lang-zone\\src\\ir";

const __doc__: &str = "";

const __is_macro__: bool = false;

pub fn main() {
}

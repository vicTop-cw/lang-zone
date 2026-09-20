
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
            "int".to_string().to_string()
        }
        IrType::F64 => {
            // STMT:Expr
            "f64".to_string().to_string()
        }
        IrType::Str => {
            // STMT:Expr
            "str".to_string().to_string()
        }
        IrType::Bool => {
            // STMT:Expr
            "bool".to_string().to_string()
        }
        IrType::Unit => {
            // STMT:Expr
            "()".to_string().to_string()
        }
        IrType::Never => {
            // STMT:Expr
            "!".to_string().to_string()
        }
        IrType::Any => {
            // STMT:Expr
            "?".to_string().to_string()
        }
        IrType::Self_ => {
            // STMT:Expr
            "Self".to_string().to_string()
        }
        IrType::Generic { name: g } => {
            // STMT:Expr
            g.to_string()
        }
        IrType::Named { path: p, args: a } => {
            let a = *a;
            // STMT:Expr
            p.to_string() + &(if (a.len() as i64) > 0i64 { "<".to_string().to_string() + &type_list(a.clone())[..] + &">".to_string().to_string()[..] } else { "".to_string().to_string() })
        }
        IrType::Opt { inner: x } => {
            let x = *x;
            // STMT:Expr
            "Option<".to_string().to_string() + &display_type(x.clone())[..] + &">".to_string().to_string()[..]
        }
        IrType::Res { ok: o, err: e } => {
            let o = *o;
            let e = *e;
            // STMT:Expr
            "Result<".to_string().to_string() + &display_type(o.clone())[..] + &", ".to_string().to_string()[..] + &display_type(e.clone())[..] + &">".to_string().to_string()[..]
        }
        IrType::Tuple { elems: es } => {
            let es = *es;
            // STMT:Expr
            "(".to_string().to_string() + &type_list(es.clone())[..] + &")".to_string().to_string()[..]
        }
        IrType::FnType { params: ps, ret: r } => {
            let ps = *ps;
            let r = *r;
            // STMT:Expr
            "fn(".to_string().to_string() + &type_list(ps.clone())[..] + &") -> ".to_string().to_string()[..] + &display_type(r.clone())[..]
        }
        IrType::Ref { inner: x } => {
            let x = *x;
            // STMT:Expr
            "&".to_string().to_string() + &display_type(x.clone())[..]
        }
        IrType::MutRef { inner: x } => {
            let x = *x;
            // STMT:Expr
            "&mut ".to_string().to_string() + &display_type(x.clone())[..]
        }
        IrType::Duck { fields: fs } => {
            let fs = *fs;
            // STMT:Expr
            "duck {".to_string().to_string() + &duck_fields(fs.clone())[..] + &"}".to_string().to_string()[..]
        }
    }
}

pub fn type_list(ts: Vec<IrType>) -> String {
    // STMT:Expr
    return if (ts.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let head = display_type(ts[((0i64) as usize)].clone());
        // STMT:Expr
        if (ts.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &type_list(tail_t(ts.clone()))[..] } else { head }
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
    let mut out = "".to_string().to_string();
    // STMT:For
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        // STMT:Let
        let f = fs[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(out + &(if idx > 0i64 { ", ".to_string().to_string() } else { "".to_string().to_string() }), f.0.to_string()) + &": ".to_string().to_string()[..], display_type(f.1.clone()));
    }
    // STMT:Expr
    return out;
}

pub fn display_expr(e: Expr) -> String {
    // STMT:Other
    match e.clone() {
        Expr::LitInt { v: n, ty: t } => {
            // STMT:Expr
            LzAdd::__add__("[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..], n.to_string()) + &"_i64".to_string().to_string()[..]
        }
        Expr::LitF64 { v: n, ty: t } => {
            // STMT:Expr
            LzAdd::__add__("[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..], n.to_string()) + &"_f64".to_string().to_string()[..]
        }
        Expr::LitStr { s: s, ty: t } => {
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] \"".to_string().to_string()[..] + &s[..] + &"\"".to_string().to_string()[..]
        }
        Expr::LitFStr { s: s, ty: t } => {
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] f\"".to_string().to_string()[..] + &s[..] + &"\"".to_string().to_string()[..]
        }
        Expr::LitBool { b: b, ty: t } => {
            // STMT:Expr
            LzAdd::__add__("[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..], b.to_string())
        }
        Expr::LitUnit { ty: t } => {
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ()".to_string().to_string()[..]
        }
        Expr::LitNone { ty: t } => {
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] None".to_string().to_string()[..]
        }
        Expr::Var { name: n, ty: t } => {
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..] + &n[..]
        }
        Expr::Call { callee: c, args: a, ty: t } => {
            let c = *c;
            let a = *a;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] call ".to_string().to_string()[..] + &display_expr(c.clone())[..] + &arg_list(a.clone())[..]
        }
        Expr::MethodCall { receiver: r, method: m, args: a, ty: t } => {
            let r = *r;
            let a = *a;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] method ".to_string().to_string()[..] + &display_expr(r.clone())[..] + &".".to_string().to_string()[..] + &m[..] + &arg_list(a.clone())[..]
        }
        Expr::FieldAccess { base: b, field: f, ty: t } => {
            let b = *b;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] field ".to_string().to_string()[..] + &display_expr(b.clone())[..] + &".".to_string().to_string()[..] + &f[..]
        }
        Expr::IndexGet { base: b, key: k, ty: t } => {
            let b = *b;
            let k = *k;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] index ".to_string().to_string()[..] + &display_expr(b.clone())[..] + &"[".to_string().to_string()[..] + &display_expr(k.clone())[..] + &"]".to_string().to_string()[..]
        }
        Expr::IndexSet { base: b, key: k, value: v, ty: t } => {
            let b = *b;
            let k = *k;
            let v = *v;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] index_set ".to_string().to_string()[..] + &display_expr(b.clone())[..] + &"[".to_string().to_string()[..] + &display_expr(k.clone())[..] + &"] = ".to_string().to_string()[..] + &display_expr(v.clone())[..]
        }
        Expr::BinOp { op: o, lhs: l, rhs: r, ty: t } => {
            let l = *l;
            let r = *r;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] binop ".to_string().to_string()[..] + &display_expr(l.clone())[..] + &" ".to_string().to_string()[..] + &o[..] + &" ".to_string().to_string()[..] + &display_expr(r.clone())[..]
        }
        Expr::UnOp { op: o, operand: p, ty: t } => {
            let p = *p;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] unop ".to_string().to_string()[..] + &o[..] + &" ".to_string().to_string()[..] + &display_expr(p.clone())[..]
        }
        Expr::StructCtor { name: n, fields: fs, ty: t } => {
            let fs = *fs;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] new ".to_string().to_string()[..] + &n[..] + &field_pairs(fs.clone())[..]
        }
        Expr::EnumCtor { enum_name: en, variant: v, args: a, ty: t } => {
            let a = *a;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] ".to_string().to_string()[..] + &en[..] + &"::".to_string().to_string()[..] + &v[..] + &arg_list(a.clone())[..]
        }
        Expr::Cast { inner: i, target: tg, ty: t } => {
            let i = *i;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] cast ".to_string().to_string()[..] + &display_expr(i.clone())[..] + &" as ".to_string().to_string()[..] + &display_type(tg.clone())[..]
        }
        Expr::MagicCall { magic: m, args: a, ty: t } => {
            let a = *a;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] magic ".to_string().to_string()[..] + &m[..] + &arg_list(a.clone())[..]
        }
        Expr::IfExpr { cond: c, then: th, els: el, ty: t } => {
            let c = *c;
            let th = *th;
            let el = *el;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] if ".to_string().to_string()[..] + &display_expr(c.clone())[..] + &" then ".to_string().to_string()[..] + &display_expr(th.clone())[..] + &" else ".to_string().to_string()[..] + &display_expr(el.clone())[..]
        }
        Expr::Lambda { params: ps, body: b, ty: t } => {
            let b = *b;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] |".to_string().to_string()[..] + &str_join(ps.clone())[..] + &"| ".to_string().to_string()[..] + &display_expr(b.clone())[..]
        }
        Expr::Pipe { receiver: r, callee: c, args: a, ty: t } => {
            let r = *r;
            let c = *c;
            let a = *a;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] pipe ".to_string().to_string()[..] + &display_expr(r.clone())[..] + &" |> ".to_string().to_string()[..] + &display_expr(c.clone())[..] + &arg_list(a.clone())[..]
        }
        Expr::TupleLit { elems: es, ty: t } => {
            let es = *es;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] (".to_string().to_string()[..] + &expr_list(es.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::ListLit { items: xs, ty: t } => {
            let xs = *xs;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] [".to_string().to_string()[..] + &expr_list(xs.clone())[..] + &"]".to_string().to_string()[..]
        }
        Expr::BlockExpr { stmts: ss, ty: t } => {
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] block ".to_string().to_string()[..] + &block_with_ty(ss.clone(), t.clone())[..]
        }
        Expr::GenExpr { yield_of: y, ty: t } => {
            let y = *y;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] gen ".to_string().to_string()[..] + &display_expr(y.clone())[..]
        }
        Expr::Paren { inner: i, ty: t } => {
            let i = *i;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] (".to_string().to_string()[..] + &display_expr(i.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::Range { end: en, inclusive: inc, ty: t } => {
            let en = *en;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t)[..] + &"] <expr>".to_string().to_string()[..]
        }
        Expr::Dict { pairs: ps, ty: t } => {
            let ps = *ps;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] <expr>".to_string().to_string()[..]
        }
        Expr::AssignExpr { target: tg, value: v, ty: t } => {
            let tg = *tg;
            let v = *v;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] <expr>".to_string().to_string()[..]
        }
        Expr::ImplicitConvert { source: s, target_ty: tt, ty: t } => {
            let s = *s;
            // STMT:Expr
            "[".to_string().to_string() + &display_type(t.clone())[..] + &"] <expr>".to_string().to_string()[..]
        }
    }
}

pub fn arg_list(as_: Vec<Expr>) -> String {
    // STMT:Expr
    return if (as_.len() as i64) == 0i64 { "".to_string().to_string() } else { "(".to_string().to_string() + &expr_list(as_.clone())[..] + &")".to_string().to_string()[..] };
}

pub fn expr_list(es: Vec<Expr>) -> String {
    // STMT:Expr
    return if (es.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let head = display_expr(es[((0i64) as usize)].clone());
        // STMT:Expr
        if (es.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &expr_list(tail_e(es.clone()))[..] } else { head }
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
    return if (fs.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let mut out = "{ ".to_string().to_string();
        // STMT:For
        for idx in (0i64..(fs.len() as i64)).into_iter() {
            // STMT:Let
            out = LzAdd::__add__(out + &(if idx > 0i64 { ", ".to_string().to_string() } else { "".to_string().to_string() }), pair_str(fs[((idx) as usize)].clone()));
        }
        // STMT:Expr
        LzAdd::__add__(out, " }".to_string().to_string())
    };
}

pub fn pair_str(p: (String, Expr)) -> String {
    // STMT:Expr
    return LzAdd::__add__(p.0, ": ".to_string().to_string()) + &display_expr(p.1.clone())[..];
}

pub fn display_stmt(s: Stmt) -> String {
    // STMT:Other
    match s.clone() {
        Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
            // STMT:Expr
            let_kw(m, r) + &n[..] + &": ".to_string()[..] + &display_type(t.clone())[..] + &" = ".to_string()[..] + &display_expr(v.clone())[..]
        }
        Stmt::Assign { target: t, value: v } => {
            // STMT:Expr
            display_expr(t.clone()) + &" = ".to_string()[..] + &display_expr(v.clone())[..]
        }
        Stmt::Return { value: v } => {
            // STMT:Other
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    // STMT:Expr
                    "return ".to_string().to_string() + &display_expr(inner.clone())[..]
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
            let base: String = "if ".to_string().to_string() + &display_expr(c.clone())[..] + &" ".to_string()[..] + &block_disp(t.clone())[..];
            // STMT:Other
            match e.clone() {
                MaybeBlock::YesBlock { value: es } => {
                    // STMT:Expr
                    base.to_string() + &" else ".to_string()[..] + &block_disp(es.clone())[..]
                }
                MaybeBlock::NoBlock => {
                    // STMT:Expr
                    base
                }
            }
        }
        Stmt::For { var: v, iter: it, guard_e: g, body: b, else_body: eb } => {
            // STMT:Expr
            "for ".to_string().to_string() + &v[..] + &" in ".to_string()[..] + &display_expr(it.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string()[..] + &block_disp(b.clone())[..] + &else_s(eb.clone())[..]
        }
        Stmt::While { cond: c, guard_e: g, body: b, else_body: eb } => {
            // STMT:Expr
            "while ".to_string().to_string() + &display_expr(c.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string()[..] + &block_disp(b.clone())[..] + &else_s(eb.clone())[..]
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
            "yield ".to_string().to_string() + &display_expr(v.clone())[..]
        }
        Stmt::YieldFrom { iter: it } => {
            // STMT:Expr
            "yield from ".to_string().to_string() + &display_expr(it.clone())[..]
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
                    base.to_string() + &" ".to_string()[..] + &display_expr(inner.clone())[..]
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
            "block \'".to_string().to_string() + &l[..] + &" ".to_string()[..] + &block_disp(b.clone())[..]
        }
        Stmt::Defer { body: b } => {
            // STMT:Expr
            "defer ".to_string().to_string() + &block_disp(b.clone())[..]
        }
        Stmt::TryCatch { body: b, catches: cs, else_body: eb, finally_body: fb } => {
            // STMT:Let
            let mut out: String = "try ".to_string().to_string() + &block_disp(b.clone())[..];
            // STMT:Let
            out = out.to_string() + &try_catches_str(cs.clone())[..];
            // STMT:Let
            out = out.to_string() + &else_s(eb.clone())[..];
            // STMT:Let
            out = out.to_string() + &finally_s(fb.clone())[..];
            // STMT:Expr
            out
        }
        Stmt::WhileLet { pattern: p, expr: e, guard_e: g, body: b } => {
            // STMT:Expr
            "while let ".to_string().to_string() + &display_pattern(p.clone())[..] + &" = ".to_string()[..] + &display_expr(e.clone())[..] + &guard_s(g.clone())[..] + &" ".to_string()[..] + &block_disp(b.clone())[..]
        }
        Stmt::CheckerBlock { label: l, ps_name: p } => {
            // STMT:Expr
            "block \'".to_string().to_string() + &l[..] + &"[ps:".to_string()[..] + &opt_debug(p.clone())[..] + &"]".to_string()[..]
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
    return (if (m && r) { "let mut ref ".to_string().to_string() } else { (if r { "let ref ".to_string().to_string() } else { (if m { "let mut ".to_string().to_string() } else { "let ".to_string().to_string() }) }) });
}

pub fn guard_s(g: MaybeExpr) -> String {
    // STMT:Other
    match g.clone() {
        MaybeExpr::YesExpr { value: e } => {
            // STMT:Expr
            " if ".to_string().to_string() + &display_expr(e.clone())[..]
        }
        MaybeExpr::NoExpr => {
            // STMT:Expr
            "".to_string().to_string()
        }
    }
}

pub fn else_s(eb: MaybeBlock) -> String {
    // STMT:Other
    match eb.clone() {
        MaybeBlock::YesBlock { value: b } => {
            // STMT:Expr
            " else ".to_string().to_string() + &block_disp(b.clone())[..]
        }
        MaybeBlock::NoBlock => {
            // STMT:Expr
            "".to_string().to_string()
        }
    }
}

pub fn finally_s(fb: MaybeBlock) -> String {
    // STMT:Other
    match fb.clone() {
        MaybeBlock::YesBlock { value: b } => {
            // STMT:Expr
            " finally ".to_string().to_string() + &block_disp(b.clone())[..]
        }
        MaybeBlock::NoBlock => {
            // STMT:Expr
            "".to_string().to_string()
        }
    }
}

pub fn try_catches_str(cs: Vec<(MaybePattern, BlockIR)>) -> String {
    // STMT:Let
    let mut out = "".to_string().to_string();
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
            " catch(".to_string().to_string() + &display_pattern(p.clone())[..] + &") ".to_string().to_string()[..] + &block_disp(c.1.clone())[..]
        }
        MaybePattern::NoPat => {
            // STMT:Expr
            " catch ".to_string().to_string() + &block_disp(c.1.clone())[..]
        }
    }
}

pub fn display_pattern(p: Pattern) -> String {
    // STMT:Other
    match p.clone() {
        Pattern::Wildcard => {
            // STMT:Expr
            "_".to_string().to_string()
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
            LzAdd::__add__(n.to_string(), "_i64".to_string().to_string())
        }
        Pattern::LitStr { s: s } => {
            // STMT:Expr
            "\"".to_string().to_string() + &s[..] + &"\"".to_string().to_string()[..]
        }
        Pattern::LitBool { b: b } => {
            // STMT:Expr
            b.to_string()
        }
        Pattern::LitF64 { v: n } => {
            // STMT:Expr
            LzAdd::__add__(n.to_string(), "_f64".to_string().to_string())
        }
        Pattern::Tuple { elems: es } => {
            let es = *es;
            // STMT:Expr
            "(".to_string().to_string() + &pat_list(es.clone())[..] + &")".to_string().to_string()[..]
        }
        Pattern::List { elems: es } => {
            let es = *es;
            // STMT:Expr
            "[".to_string().to_string() + &pat_list(es.clone())[..] + &"]".to_string().to_string()[..]
        }
        Pattern::Struct { name: n, fields: fs } => {
            let fs = *fs;
            // STMT:Let
            let mut fields_s = "".to_string().to_string();
            // STMT:For
            for idx in (0i64..(fs.len() as i64)).into_iter() {
                // STMT:Expr
                if idx > 0i64 {
                    // STMT:Other
                    fields_s = fields_s + &", ".to_string().to_string()[..];
                } else { ()};
                // STMT:Let
                let f = fs[((idx) as usize)].clone();
                // STMT:Other
                fields_s = LzAdd::__add__(LzAdd::__add__(fields_s, f.0) + &": ".to_string().to_string()[..], display_pattern(f.1.clone()));
            }
            // STMT:Expr
            n.to_string() + &" { ".to_string().to_string()[..] + &fields_s[..] + &" }".to_string().to_string()[..]
        }
        Pattern::Enum { enum_name: en, variant: v, args: a } => {
            let a = *a;
            // STMT:Expr
            en.to_string() + &"::".to_string().to_string()[..] + &v[..] + &arg_pat_list(a.clone())[..]
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
                    "..".to_string().to_string()
                }
            }
        }
        Pattern::Range { start: st, end: en, inclusive: inc } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(st.to_string(), (if inc { "..=".to_string().to_string() } else { "..".to_string().to_string() })), en.to_string())
        }
    }
}

pub fn pat_list(ps: Vec<Pattern>) -> String {
    // STMT:Expr
    return if (ps.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let head = display_pattern(ps[((0i64) as usize)].clone());
        // STMT:Expr
        if (ps.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &pat_list(tail_pat(ps.clone()))[..] } else { head }
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
    return if (ps.len() as i64) == 0i64 { "".to_string().to_string() } else { "(".to_string().to_string() + &pat_list(ps.clone())[..] + &")".to_string().to_string()[..] };
}

pub fn gen_match_arms(arms: Vec<(Pattern, MaybeExpr, BlockIR)>) -> String {
    // STMT:Let
    let mut out = "".to_string().to_string();
    // STMT:For
    for idx in (0i64..(arms.len() as i64)).into_iter() {
        // STMT:Let
        let arm = arms[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(out, gen_match_arm(arm.clone()));
    }
    // STMT:Expr
    return out;
}

pub fn gen_match_arm(arm: (Pattern, MaybeExpr, BlockIR)) -> String {
    // STMT:Let
    let pat = arm.0;
    // STMT:Let
    let gd = arm.1;
    // STMT:Let
    let blk = arm.2;
    // STMT:Let
    let pat_s = gen_pattern(pat.clone());
    // STMT:Let
    let gd_s = {
// STMT:Other
match gd.clone() {
    MaybeExpr::YesExpr { value: vv } => {
        // STMT:Expr
        LzAdd::__add__(" if ".to_string().to_string(), gen_expr(vv.clone()))
    }
    MaybeExpr::NoExpr => {
        // STMT:Expr
        "".to_string().to_string()
    }
}
    };
    // STMT:Let
    let blk_s = gen_block(blk.clone());
    // STMT:Expr
    return "    ".to_string().to_string() + &pat_s[..] + &gd_s[..] + &" => ".to_string().to_string()[..] + &blk_s[..] + &",\n".to_string().to_string()[..];
}

pub fn gen_pattern(p: Pattern) -> String {
    // STMT:Other
    match p.clone() {
        Pattern::Wildcard => {
            // STMT:Expr
            "_".to_string().to_string()
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
            LzAdd::__add__(n.to_string(), "_i64".to_string().to_string())
        }
        Pattern::LitStr { s: s } => {
            // STMT:Expr
            "\"".to_string().to_string() + &esc_rust(s.clone())[..] + &"\"".to_string().to_string()[..]
        }
        Pattern::LitBool { b: b } => {
            // STMT:Expr
            b.to_string()
        }
        Pattern::LitF64 { v: n } => {
            // STMT:Expr
            LzAdd::__add__(n.to_string(), "_f64".to_string().to_string())
        }
        Pattern::Tuple { elems: es } => {
            let es = *es;
            // STMT:Expr
            "(".to_string().to_string() + &pat_list(es.clone())[..] + &")".to_string().to_string()[..]
        }
        Pattern::List { elems: es } => {
            let es = *es;
            // STMT:Expr
            "[".to_string().to_string() + &pat_list(es.clone())[..] + &"]".to_string().to_string()[..]
        }
        Pattern::Struct { name: n, fields: fs } => {
            let fs = *fs;
            // STMT:Let
            let mut fields_s = " { ".to_string().to_string();
            // STMT:For
            for idx in (0i64..(fs.len() as i64)).into_iter() {
                // STMT:Expr
                if idx > 0i64 {
                    // STMT:Other
                    fields_s = fields_s + &", ".to_string().to_string()[..];
                } else { ()};
                // STMT:Let
                let f = fs[((idx) as usize)].clone();
                // STMT:Other
                fields_s = LzAdd::__add__(LzAdd::__add__(fields_s, f.0) + &": ".to_string().to_string()[..], gen_pattern(f.1.clone()));
            }
            // STMT:Expr
            fields_s + &" }".to_string().to_string()[..]
        }
        Pattern::Enum { enum_name: en, variant: v, args: a } => {
            let a = *a;
            // STMT:Expr
            en.to_string() + &"::".to_string().to_string()[..] + &v[..] + &arg_pat_list(a.clone())[..]
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
                    "..".to_string().to_string()
                }
            }
        }
        Pattern::Range { start: st, end: en, inclusive: inc } => {
            // STMT:Expr
            LzAdd::__add__(LzAdd::__add__(st.to_string(), "_i64..".to_string().to_string()), en.to_string()) + &"_i64".to_string().to_string()[..]
        }
    }
}

pub fn gen_block(b: BlockIR) -> String {
    // STMT:Other
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            // STMT:Expr
            if (ss.len() as i64) == 0i64 { String::new() } else { "{\n".to_string().to_string() + &gen_body(ss.clone(), false, "".to_string().to_string())[..] + &"}".to_string()[..] }
        }
    }
}

pub fn rust_type(t: IrType) -> String {
    // STMT:Other
    match t.clone() {
        IrType::Int => {
            // STMT:Expr
            "i64".to_string().to_string()
        }
        IrType::F64 => {
            // STMT:Expr
            "f64".to_string().to_string()
        }
        IrType::Str => {
            // STMT:Expr
            "String".to_string().to_string()
        }
        IrType::Bool => {
            // STMT:Expr
            "bool".to_string().to_string()
        }
        IrType::Unit => {
            // STMT:Expr
            "()".to_string().to_string()
        }
        IrType::Never => {
            // STMT:Expr
            "!".to_string().to_string()
        }
        IrType::Any => {
            // STMT:Expr
            "Box<dyn Any>".to_string().to_string()
        }
        IrType::Self_ => {
            // STMT:Expr
            "Self".to_string().to_string()
        }
        IrType::Generic { name: g } => {
            // STMT:Expr
            g.to_string()
        }
        IrType::Named { path: p, args: a } => {
            let a = *a;
            // STMT:Expr
            p.to_string() + &(if (a.len() as i64) > 0i64 { "<".to_string().to_string() + &type_rust_list(a.clone())[..] + &">".to_string()[..] } else { "".to_string() })
        }
        IrType::Opt { inner: x } => {
            let x = *x;
            // STMT:Expr
            "Option<".to_string().to_string() + &rust_type(x.clone())[..] + &">".to_string().to_string()[..]
        }
        IrType::Res { ok: o, err: e } => {
            let o = *o;
            let e = *e;
            // STMT:Expr
            "Result<".to_string().to_string() + &rust_type(o.clone())[..] + &", ".to_string()[..] + &rust_type(e.clone())[..] + &">".to_string().to_string()[..]
        }
        IrType::Tuple { elems: es } => {
            let es = *es;
            // STMT:Expr
            "(".to_string().to_string() + &type_rust_list(es.clone())[..] + &")".to_string().to_string()[..]
        }
        IrType::FnType { params: ps, ret: r } => {
            let ps = *ps;
            let r = *r;
            // STMT:Expr
            "fn(".to_string().to_string() + &type_rust_list(ps.clone())[..] + &") -> ".to_string()[..] + &rust_type(r.clone())[..]
        }
        IrType::Ref { inner: x } => {
            let x = *x;
            // STMT:Expr
            "&".to_string().to_string() + &rust_type(x.clone())[..]
        }
        IrType::MutRef { inner: x } => {
            let x = *x;
            // STMT:Expr
            "&mut ".to_string().to_string() + &rust_type(x.clone())[..]
        }
        IrType::Duck { fields: fs } => {
            let fs = *fs;
            // STMT:Expr
            "Box<dyn Any>".to_string().to_string()
        }
    }
}

pub fn type_rust_list(ts: Vec<IrType>) -> String {
    // STMT:Expr
    return if (ts.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let head = rust_type(ts[((0i64) as usize)].clone());
        // STMT:Expr
        if (ts.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &type_rust_list(tail_t(ts.clone()))[..] } else { head }
    };
}

pub fn generic_sig(gs: Vec<String>) -> String {
    // STMT:Expr
    return if (gs.len() as i64) == 0i64 { "".to_string().to_string() } else { "<".to_string().to_string() + &str_join(gs.clone())[..] + &">".to_string().to_string()[..] };
}

pub fn str_join(xs: Vec<String>) -> String {
    // STMT:Expr
    return if (xs.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let head = xs[((0i64) as usize)].to_string();
        // STMT:Expr
        if (xs.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &str_join(aj_tail(xs.clone()))[..] } else { head }
    };
}

pub fn gen_expr(e: Expr) -> String {
    // STMT:Other
    match e.clone() {
        Expr::LitInt { v: n, ty: t } => {
            // STMT:Expr
            LzAdd::__add__(n.to_string(), "i64".to_string().to_string())
        }
        Expr::LitF64 { v: n, ty: t } => {
            // STMT:Expr
            fmt_f64(n).to_string()
        }
        Expr::LitStr { s: s, ty: t } => {
            // STMT:Expr
            "\"".to_string().to_string() + &esc_rust(s.clone())[..] + &"\".to_string()".to_string().to_string()[..]
        }
        Expr::LitFStr { s: s, ty: t } => {
            // STMT:Expr
            gen_fstring(s.clone()).to_string()
        }
        Expr::LitBool { b: b, ty: t } => {
            // STMT:Expr
            b.to_string()
        }
        Expr::LitUnit { ty: t } => {
            // STMT:Expr
            "()".to_string().to_string()
        }
        Expr::LitNone { ty: t } => {
            // STMT:Expr
            "None".to_string().to_string()
        }
        Expr::Var { name: n, ty: t } => {
            // STMT:Expr
            n.to_string()
        }
        Expr::Call { callee: c, args: a, ty: t } => {
            let c = *c;
            let a = *a;
            // STMT:Expr
            gen_call(c.clone(), a.clone()).to_string()
        }
        Expr::MethodCall { receiver: r, method: m, args: a, ty: t } => {
            let r = *r;
            let a = *a;
            // STMT:Expr
            gen_expr(r.clone()) + &".".to_string().to_string()[..] + &m.to_string()[..] + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::FieldAccess { base: b, field: f, ty: t } => {
            let b = *b;
            // STMT:Expr
            gen_expr(b.clone()) + &".".to_string().to_string()[..] + &f.to_string()[..]
        }
        Expr::IndexGet { base: b, key: k, ty: t } => {
            let b = *b;
            let k = *k;
            // STMT:Expr
            gen_expr(b.clone()) + &"[".to_string().to_string()[..] + &gen_expr(k.clone())[..] + &"]".to_string().to_string()[..]
        }
        Expr::IndexSet { base: b, key: k, value: v, ty: t } => {
            let b = *b;
            let k = *k;
            let v = *v;
            // STMT:Expr
            gen_expr(b.clone()) + &"[".to_string().to_string()[..] + &gen_expr(k.clone())[..] + &"] = ".to_string().to_string()[..] + &gen_expr(v.clone())[..]
        }
        Expr::BinOp { op: o, lhs: l, rhs: r, ty: t } => {
            let l = *l;
            let r = *r;
            // STMT:Expr
            gen_expr(l.clone()) + &" ".to_string().to_string()[..] + &o.to_string()[..] + &" ".to_string().to_string()[..] + &gen_expr(r.clone())[..]
        }
        Expr::UnOp { op: o, operand: p, ty: t } => {
            let p = *p;
            // STMT:Expr
            o.to_string() + &gen_expr(p.clone())[..]
        }
        Expr::StructCtor { name: n, fields: fs, ty: t } => {
            let fs = *fs;
            // STMT:Expr
            if (n).to_string() == ("Range".to_string()).to_string() { gen_range_ctor(fs.clone()) } else { if (n).to_string() == ("Dict".to_string()).to_string() { gen_dict_ctor(fs.clone()) } else { n.to_string() + &" { ".to_string().to_string()[..] + &ctor_fields(fs.clone())[..] + &" }".to_string().to_string()[..] } }
        }
        Expr::EnumCtor { enum_name: en, variant: v, args: a, ty: t } => {
            let a = *a;
            // STMT:Expr
            en.to_string() + &"::".to_string().to_string()[..] + &v.to_string()[..] + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::Cast { inner: i, target: tg, ty: t } => {
            let i = *i;
            // STMT:Expr
            LzAdd::__add__("(".to_string().to_string() + &gen_expr(i.clone())[..] + &" as ".to_string().to_string()[..], rust_type(tg.clone()).to_string()) + &")".to_string().to_string()[..]
        }
        Expr::MagicCall { magic: m, args: a, ty: t } => {
            let a = *a;
            // STMT:Expr
            m.to_string() + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::IfExpr { cond: c, then: th, els: el, ty: t } => {
            let c = *c;
            let th = *th;
            let el = *el;
            // STMT:Expr
            "if ".to_string().to_string() + &gen_expr(c.clone())[..] + &" { ".to_string().to_string()[..] + &gen_expr(th.clone())[..] + &" } else { ".to_string().to_string()[..] + &gen_expr(el.clone())[..] + &" }".to_string().to_string()[..]
        }
        Expr::Lambda { params: ps, body: b, ty: t } => {
            let b = *b;
            // STMT:Expr
            LzAdd::__add__("move |".to_string().to_string(), str_join(ps.clone()).to_string()) + &"| ".to_string().to_string()[..] + &gen_expr(b.clone())[..]
        }
        Expr::Pipe { receiver: r, callee: c, args: a, ty: t } => {
            let r = *r;
            let c = *c;
            let a = *a;
            // STMT:Expr
            gen_expr(r.clone()) + &".".to_string().to_string()[..] + &gen_expr(c.clone())[..] + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::TupleLit { elems: es, ty: t } => {
            let es = *es;
            // STMT:Expr
            "(".to_string().to_string() + &expr_cs_list(es.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::ListLit { items: xs, ty: t } => {
            let xs = *xs;
            // STMT:Expr
            "[".to_string().to_string() + &expr_cs_list(xs.clone())[..] + &"]".to_string().to_string()[..]
        }
        Expr::BlockExpr { stmts: ss, ty: t } => {
            // STMT:Expr
            "{\n".to_string().to_string() + &gen_body(ss.clone(), false, "".to_string().to_string())[..] + &"}".to_string()[..]
        }
        Expr::GenExpr { yield_of: y, ty: t } => {
            let y = *y;
            // STMT:Expr
            "gen ".to_string().to_string() + &gen_expr(y.clone())[..]
        }
        Expr::Paren { inner: i, ty: t } => {
            let i = *i;
            // STMT:Expr
            "(".to_string().to_string() + &gen_expr(i.clone())[..] + &")".to_string().to_string()[..]
        }
        Expr::Range { end: en, inclusive: inc, ty: t } => {
            let en = *en;
            // STMT:Expr
            "0_i64..".to_string().to_string() + &gen_expr(en)[..]
        }
        Expr::Dict { pairs: ps, ty: t } => {
            let ps = *ps;
            // STMT:Expr
            "HashMap::new()".to_string().to_string()
        }
        Expr::AssignExpr { target: tg, value: v, ty: t } => {
            let tg = *tg;
            let v = *v;
            // STMT:Expr
            gen_expr(tg.clone()) + &" = ".to_string().to_string()[..] + &gen_expr(v.clone())[..]
        }
        Expr::ImplicitConvert { source: s, target_ty: tt, ty: t } => {
            let s = *s;
            // STMT:Expr
            gen_expr(s.clone())
        }
        _ => {
            // STMT:Expr
            "unimplemented!".to_string().to_string()
        }
    }
}

pub fn display_item(i: Item) -> String {
    // STMT:Other
    match i.clone() {
        Item::FnDef { name: n, generics: gs, params: ps, ret: r, body: b } => {
            // STMT:Expr
            "fn ".to_string().to_string() + &n[..]
        }
        Item::Const { name: n, ty: t, value: v } => {
            // STMT:Expr
            "const ".to_string().to_string() + &n[..]
        }
        Item::StructDef { name: n, generics: gs, fields: fs } => {
            // STMT:Expr
            "struct ".to_string().to_string() + &n[..]
        }
        Item::EnumDef { name: n, generics: gs, variants: vs } => {
            // STMT:Expr
            "enum ".to_string().to_string() + &n[..]
        }
        Item::TraitDef { name: n, supertraits: sts, methods: ms } => {
            // STMT:Expr
            "trait ".to_string().to_string() + &n[..]
        }
        Item::DuckDef { name: n, method_count: mc } => {
            // STMT:Expr
            "duck ".to_string().to_string() + &n[..]
        }
        Item::UseStmt { path: pth, alias: al, items: its, is_from: fr } => {
            // STMT:Expr
            "use ".to_string().to_string() + &str_join(pth.clone())[..]
        }
        Item::TypeAlias { name: n, ty: t } => {
            // STMT:Expr
            "type ".to_string().to_string() + &n[..]
        }
        Item::Impl { trait_: tr, for_type: ft, method_names: mns } => {
            // STMT:Expr
            "impl ".to_string().to_string() + &rust_type(ft.clone())[..]
        }
        Item::CheckerBlock { name: n, ps_name: p } => {
            // STMT:Expr
            "checker ".to_string().to_string() + &n[..]
        }
        Item::Test { name: n, body: b } => {
            // STMT:Expr
            "test ".to_string().to_string() + &n[..]
        }
        _ => {
            // STMT:Expr
            "item".to_string().to_string()
        }
    }
}

pub fn opt_debug(p: MaybeStr) -> String {
    // STMT:Other
    match p.clone() {
        MaybeStr::YesStr { value: v } => {
            // STMT:Expr
            v
        }
        MaybeStr::NoStr => {
            // STMT:Expr
            "".to_string().to_string()
        }
    }
}

pub fn block_disp(b: BlockIR) -> String {
    // STMT:Other
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            // STMT:Expr
            "{\n".to_string().to_string() + &block_with_ty(ss.clone(), t.clone())[..] + &"}".to_string().to_string()[..]
        }
    }
}

pub fn stmt_block(ss: Vec<Stmt>) -> String {
    // STMT:Expr
    return if (ss.len() as i64) == 0i64 { "{}".to_string().to_string()} else {
        // STMT:Let
        let mut out = "{\n".to_string().to_string();
        // STMT:For
        for idx in (0i64..(ss.len() as i64)).into_iter() {
            // STMT:Let
            out = LzAdd::__add__(out, display_stmt(ss[((idx) as usize)].clone())) + &"\n".to_string().to_string()[..];
        }
        // STMT:Expr
        LzAdd::__add__(out, "}".to_string().to_string())
    };
}

pub fn match_block(s: Expr, arms: Vec<(Pattern, MaybeExpr, BlockIR)>) -> String {
    // STMT:Let
    let mut out = "match ".to_string().to_string() + &display_expr(s.clone())[..] + &" {\n".to_string().to_string()[..];
    // STMT:For
    for idx in (0i64..(arms.len() as i64)).into_iter() {
        // STMT:Let
        let arm = arms[((idx) as usize)].clone();
        // STMT:Let
        let pat_s = display_pattern(arm.0.clone());
        // STMT:Let
        let gd_s = {
// STMT:Other
match arm.1 {
    MaybeExpr::YesExpr { value: vv } => {
        // STMT:Expr
        LzAdd::__add__(" if ".to_string().to_string(), display_expr(vv.clone()))
    }
    MaybeExpr::NoExpr => {
        // STMT:Expr
        "".to_string().to_string()
    }
}
    };
        // STMT:Let
        let blk_s = block_disp(arm.2.clone());
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(out + &"    ".to_string().to_string()[..], pat_s) + &gd_s[..] + &" => ".to_string().to_string()[..], blk_s) + &",\n".to_string().to_string()[..];
    }
    // STMT:Expr
    return out + &"}".to_string().to_string()[..];
}

pub fn block_with_ty(ss: Vec<Stmt>, ty: IrType) -> String {
    // STMT:Expr
    return if (ss.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let mut out = "".to_string().to_string();
        // STMT:For
        for idx in (0i64..(ss.len() as i64)).into_iter() {
            // STMT:Let
            out = LzAdd::__add__(out, display_stmt(ss[((idx) as usize)].clone())) + &"\n".to_string().to_string()[..];
        }
        // STMT:Expr
        out
    };
}

pub fn base_is_dict(b: Expr) -> bool {
    // STMT:Other
    match b.clone() {
        Expr::Var { name: n, ty: t } => {
            // STMT:Other
            match t.clone() {
                IrType::Named { path: p, args: a } => {
                    let a = *a;
                    // STMT:Expr
                    (p).to_string() == ("Dict".to_string()).to_string() || (p).to_string() == ("HashMap".to_string()).to_string()
                }
                _ => {
                    // STMT:Expr
                    false
                }
            }
        }
        _ => {
            // STMT:Expr
            false
        }
    }
}

pub fn gen_range_ctor(fs: Vec<(String, Expr)>) -> String {
    // STMT:Let
    let mut start_s = "".to_string().to_string();
    // STMT:Let
    let mut end_s = "".to_string().to_string();
    // STMT:Let
    let mut incl: bool = false;
    // STMT:For
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        // STMT:Let
        let f = fs[((idx) as usize)].clone();
        // STMT:Expr
        if f.0 == "start".to_string() {
            // STMT:Other
            start_s = gen_expr(f.1.clone());
        } else { if f.0 == "end".to_string() {
            // STMT:Other
            end_s = gen_expr(f.1.clone());
        } else { if f.0 == "inclusive".to_string() {
            // STMT:Other
            match f.1 {
                Expr::LitBool { b: bv, ty: tv } => {
                    // STMT:Let
                    incl = bv;
                }
                _ => {
                    // STMT:Let
                    incl = incl;
                }
            }
        } else { ()}}};
    }
    // STMT:Expr
    return start_s + &(if incl { "..=".to_string().to_string() } else { "..".to_string().to_string() }) + &end_s[..];
}

pub fn gen_dict_ctor(fs: Vec<(String, Expr)>) -> String {
    // STMT:Expr
    return if (fs.len() as i64) == 0i64 { "std::collections::HashMap::new()".to_string().to_string()} else {
        // STMT:Let
        let mut pairs = "".to_string().to_string();
        // STMT:Let
        let mut i: i64 = 0i64;
        // STMT:Other
        while LzAdd::__add__(i, 1i64) < (fs.len() as i64) {
            // STMT:Let
            let k = fs[((i) as usize)].clone();
            // STMT:Let
            let v = fs[((LzAdd::__add__(i, 1i64)) as usize)].clone();
            // STMT:Let
            pairs = pairs + &"(".to_string().to_string()[..] + &gen_expr(k.1.clone())[..] + &", ".to_string().to_string()[..] + &gen_expr(v.1.clone())[..] + &")".to_string().to_string()[..];
            // STMT:Expr
            if LzAdd::__add__(i, 2i64) < (fs.len() as i64) {
                // STMT:Let
                pairs = pairs + &", ".to_string().to_string()[..];
            } else { ()};
            // STMT:Let
            i = i + 2i64;
        }
        // STMT:Expr
        LzAdd::__add__("[".to_string().to_string(), pairs) + &"].into_iter().collect()".to_string().to_string()[..]
    };
}

pub fn gen_call(c: Expr, a: Vec<Expr>) -> String {
    // STMT:Other
    match c.clone() {
        Expr::Var { name: n, ty: t } => {
            // STMT:Expr
            if (n).to_string() == ("print".to_string()).to_string() { "println!(".to_string().to_string() + &print_fmt(a.clone())[..] + &expr_cs_list(a.clone())[..] + &")".to_string().to_string()[..] } else { if (n).to_string() == ("len".to_string()).to_string() { "(".to_string().to_string() + &expr_cs_list(a.clone())[..] + &".len() as i64)".to_string().to_string()[..] } else { if (n).to_string() == ("set!".to_string()).to_string() { "[".to_string().to_string() + &expr_cs_list(a.clone())[..] + &"].into_iter().collect()".to_string().to_string()[..] } else { n.to_string() + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone())[..] + &")".to_string().to_string()[..] } } }
        }
        _ => {
            // STMT:Expr
            gen_expr(c.clone()) + &"(".to_string().to_string()[..] + &expr_cs_list(a.clone())[..] + &")".to_string().to_string()[..]
        }
    }
}

pub fn print_fmt(as_: Vec<Expr>) -> String {
    // STMT:Expr
    return if (as_.len() as i64) == 0i64 { "\"\"".to_string().to_string()} else {
        // STMT:Let
        let mut out = "\"{:?}".to_string().to_string();
        // STMT:For
        for idx in (1i64..(as_.len() as i64)).into_iter() {
            // STMT:Let
            out = out + &" {:?}".to_string().to_string()[..];
        }
        // STMT:Expr
        LzAdd::__add__(out, "\", ".to_string().to_string())
    };
}

pub fn expr_cs_list(es: Vec<Expr>) -> String {
    // STMT:Expr
    return if (es.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let head = gen_expr(es[((0i64) as usize)].clone());
        // STMT:Expr
        if (es.len() as i64) > 1i64 { LzAdd::__add__(head, ", ".to_string().to_string()) + &expr_cs_list(ec_tail(es.clone()))[..] } else { head }
    };
}

pub fn ec_tail(es: Vec<Expr>) -> Vec<Expr> {
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

pub fn ctor_fields(fs: Vec<(String, Expr)>) -> String {
    // STMT:Let
    let mut out = "".to_string().to_string();
    // STMT:For
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        // STMT:Let
        let f = fs[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(out + &(if idx > 0i64 { ", ".to_string().to_string() } else { "".to_string().to_string() }), f.0.to_string()) + &": ".to_string().to_string()[..], gen_expr(f.1.clone()));
    }
    // STMT:Expr
    return out;
}

pub fn gen_stmt(s: Stmt, is_tail: bool, is_main: bool, auto_mut: String) -> String {
    // STMT:Other
    match s.clone() {
        Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
            // STMT:Expr
            gen_let(n.clone(), t.clone(), v.clone(), m, r, auto_mut.clone())
        }
        Stmt::Assign { target: tg, value: v } => {
            // STMT:Other
            match tg.clone() {
                Expr::IndexGet { base: b, key: k, ty: t } => {
                    let b = *b;
                    let k = *k;
                    // STMT:Expr
                    if base_is_dict(b.clone()) { gen_expr(b.clone()) + &".insert(".to_string().to_string()[..] + &gen_expr(k.clone())[..] + &", ".to_string().to_string()[..] + &gen_expr(v.clone())[..] + &");".to_string().to_string()[..] } else { gen_expr(tg.clone()) + &" = ".to_string().to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string().to_string()[..] }
                }
                _ => {
                    // STMT:Expr
                    gen_expr(tg.clone()) + &" = ".to_string().to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string().to_string()[..]
                }
            }
        }
        Stmt::Return { value: v } => {
            // STMT:Other
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    // STMT:Expr
                    "return ".to_string().to_string() + &gen_expr(inner.clone())[..] + &";".to_string().to_string()[..]
                }
                MaybeExpr::NoExpr => {
                    // STMT:Expr
                    "return;".to_string().to_string()
                }
            }
        }
        Stmt::ExprStmt { expr: e } => {
            // STMT:Let
            let ret_prefix = (if (is_tail && !is_main) { "return ".to_string() } else { "".to_string() }).to_string();
            // STMT:Expr
            ret_prefix + &gen_expr(e.clone())[..] + &";".to_string().to_string()[..]
        }
        Stmt::While { cond: c, guard_e: g, body: b, else_body: eb } => {
            // STMT:Expr
            gen_while(c.clone(), b.clone())
        }
        _ => {
            // STMT:Expr
            "// TODO stmt".to_string().to_string()
        }
    }
}

pub fn gen_while(c: Expr, b: BlockIR) -> String {
    // STMT:Let
    let inf: bool = is_true_cond(c.clone());
    // STMT:Expr
    return if inf && block_is_pass_only(b.clone()) { "loop {\n        unimplemented!()\n    }".to_string().to_string() } else { "// TODO while".to_string().to_string() };
}

pub fn is_true_cond(c: Expr) -> bool {
    // STMT:Other
    match c.clone() {
        Expr::LitBool { b: bv, ty: tv } => {
            // STMT:Expr
            bv
        }
        _ => {
            // STMT:Expr
            false
        }
    }
}

pub fn block_is_pass_only(b: BlockIR) -> bool {
    // STMT:Other
    match b.clone() {
        BlockIR::Block { stmts: ss, ty: t } => {
            // STMT:Expr
            if (ss.len() as i64) == 1i64 {
                // STMT:Other
                match ss[((0i64) as usize)].clone() {
                    Stmt::Pass => {
                        // STMT:Expr
                        true
                    }
                    _ => {
                        // STMT:Expr
                        false
                    }
                }
            } else { false}
        }
    }
}

pub fn gen_let(n: String, t: IrType, v: Expr, m: bool, r: bool, auto_mut: String) -> String {
    // STMT:Let
    let mut_kw = (if (m || str_contains(auto_mut.clone(), n.clone())) { "mut ".to_string() } else { "".to_string() }).to_string();
    // STMT:Let
    let ref_kw = (if r { "ref ".to_string() } else { "".to_string() }).to_string();
    // STMT:Let
    let ty_s = rust_type(t.clone());
    // STMT:Let
    let empty_c: bool = is_empty_container(v.clone());
    // STMT:Let
    let force: bool = empty_c && is_dict_or_set_ty(t.clone());
    // STMT:Expr
    return if skip_ty_ann(t.clone()) && !force { "let ".to_string().to_string() + &mut_kw[..] + &ref_kw[..] + &n.to_string()[..] + &" = ".to_string().to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string().to_string()[..] } else { "let ".to_string().to_string() + &mut_kw[..] + &ref_kw[..] + &n.to_string()[..] + &": ".to_string().to_string()[..] + &ty_s[..] + &" = ".to_string().to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string().to_string()[..] };
}

pub fn is_empty_container(v: Expr) -> bool {
    // STMT:Other
    match v.clone() {
        Expr::ListLit { items: xs, ty: t } => {
            let xs = *xs;
            // STMT:Expr
            (xs.len() as i64) == 0i64
        }
        Expr::StructCtor { name: nn, fields: fs, ty: t } => {
            let fs = *fs;
            // STMT:Expr
            (nn).to_string() == ("Dict".to_string()).to_string() && (fs.len() as i64) == 0i64
        }
        _ => {
            // STMT:Expr
            false
        }
    }
}

pub fn is_dict_or_set_ty(t: IrType) -> bool {
    // STMT:Other
    match t.clone() {
        IrType::Named { path: p, args: a } => {
            let a = *a;
            // STMT:Expr
            (p).to_string() == ("Dict".to_string()).to_string() || (p).to_string() == ("Set".to_string()).to_string()
        }
        _ => {
            // STMT:Expr
            false
        }
    }
}

pub fn skip_ty_ann(t: IrType) -> bool {
    // STMT:Other
    match t.clone() {
        IrType::Any => {
            // STMT:Expr
            true
        }
        IrType::Unit => {
            // STMT:Expr
            true
        }
        IrType::Duck { fields: fs } => {
            let fs = *fs;
            // STMT:Expr
            true
        }
        IrType::Generic { name: nn } => {
            // STMT:Expr
            true
        }
        IrType::FnType { params: ps, ret: rr } => {
            let ps = *ps;
            let rr = *rr;
            // STMT:Expr
            true
        }
        IrType::Named { path: p, args: a } => {
            let a = *a;
            // STMT:Expr
            (p).to_string() == ("Dict".to_string()).to_string() || (p).to_string() == ("Set".to_string()).to_string() || (p).to_string() == ("Range".to_string()).to_string() || (p).to_string() == ("Nil".to_string()).to_string() || args_is_empty_or_generic(a.clone())
        }
        IrType::Ref { inner: x } => {
            let x = *x;
            // STMT:Other
            match x.clone() {
                IrType::Generic { name: nn } => {
                    // STMT:Expr
                    true
                }
                _ => {
                    // STMT:Expr
                    false
                }
            }
        }
        IrType::MutRef { inner: x } => {
            let x = *x;
            // STMT:Other
            match x.clone() {
                IrType::Generic { name: nn } => {
                    // STMT:Expr
                    true
                }
                _ => {
                    // STMT:Expr
                    false
                }
            }
        }
        _ => {
            // STMT:Expr
            false
        }
    }
}

pub fn args_is_empty_or_generic(a: Vec<IrType>) -> bool {
    // STMT:Expr
    return if (a.len() as i64) == 0i64 { true} else {
        // STMT:Let
        let mut has_generic: bool = false;
        // STMT:For
        for idx in (0i64..(a.len() as i64)).into_iter() {
            // STMT:Let
            let ai = a[((idx) as usize)].clone();
            // STMT:Other
            match ai.clone() {
                IrType::Generic { name: nn } => {
                    // STMT:Let
                    has_generic = true;
                }
                IrType::Any => {
                    // STMT:Let
                    has_generic = true;
                }
                _ => {
                    // STMT:Let
                    has_generic = has_generic;
                }
            }
        }
        // STMT:Expr
        has_generic
    };
}

pub fn fmt_f64(n: f64) -> String {
    // STMT:Let
    let s = n.to_string();
    // STMT:Expr
    return if has_dot_or_e(s.clone()) { LzAdd::__add__(s, "f64".to_string().to_string()) } else { LzAdd::__add__(s, ".0f64".to_string().to_string()) };
}

pub fn has_dot_or_e(s: String) -> bool {
    // STMT:Let
    let len: i64 = (s.len() as i64);
    // STMT:Let
    let mut i: i64 = 0i64;
    // STMT:Let
    let mut found: bool = false;
    // STMT:Other
    while i < len {
        // STMT:Let
        let c: String = s[((i) as usize)..(((i + 1i64)) as usize)].to_string();
        // STMT:Expr
        if (c).to_string() == (".".to_string()).to_string() || (c).to_string() == ("e".to_string()).to_string() {
            // STMT:Let
            found = true;
        } else { ()};
        // STMT:Let
        i = i + 1i64;
    }
    // STMT:Expr
    return found;
}

pub fn gen_fstring(s: String) -> String {
    // STMT:Let
    let mut fmt = "".to_string().to_string();
    // STMT:Let
    let mut args: Vec<String> = Vec::new();
    // STMT:Let
    let n: i64 = (s.len() as i64);
    // STMT:Let
    let mut i: i64 = 0i64;
    // STMT:Other
    while i < n {
        // STMT:Let
        let c: String = s[((i) as usize)..(((i + 1i64)) as usize)].to_string();
        // STMT:Expr
        if (c).to_string() == ("{".to_string()).to_string() { if i + 1i64 < n {
            // STMT:Let
            let c2: String = s[(((i + 1i64)) as usize)..(((i + 2i64)) as usize)].to_string();
            // STMT:Expr
            if c2 == "{".to_string() {
                // STMT:Let
                fmt = fmt + &"{{".to_string().to_string()[..];
                // STMT:Let
                i = i + 2i64;
            } else {
                // STMT:Let
                let mut j: i64 = i + 1i64;
                // STMT:Other
                while j < n {
                    // STMT:Let
                    let cj: String = s[((j) as usize)..(((LzAdd::__add__(j, 1i64))) as usize)].to_string();
                    // STMT:Expr
                    if (cj).to_string() == ("}".to_string()).to_string() {
                        // STMT:Other
                        break;
                    } else { ()};
                    // STMT:Let
                    j = j + 1i64;
                }
                // STMT:Let
                let expr: String = s[(((i + 1i64)) as usize)..((j) as usize)].to_string();
                // STMT:Let
                fmt = fmt + &"{:?}".to_string().to_string()[..];
                // STMT:Expr
                args.push(expr);
                // STMT:Let
                i = LzAdd::__add__(j, 1i64);
            }
        } else {
            // STMT:Let
            fmt = fmt + &"{".to_string().to_string()[..];
            // STMT:Let
            i = i + 1i64;
        }} else { if (c).to_string() == ("}".to_string()).to_string() { if i + 1i64 < n {
            // STMT:Let
            let c2: String = s[(((i + 1i64)) as usize)..(((i + 2i64)) as usize)].to_string();
            // STMT:Expr
            if c2 == "}".to_string() {
                // STMT:Let
                fmt = fmt + &"}}".to_string().to_string()[..];
                // STMT:Let
                i = i + 2i64;
            } else {
                // STMT:Let
                fmt = fmt + &"}".to_string().to_string()[..];
                // STMT:Let
                i = i + 1i64;
            }
        } else {
            // STMT:Let
            fmt = fmt + &"}".to_string().to_string()[..];
            // STMT:Let
            i = i + 1i64;
        }} else {
            // STMT:Let
            fmt = fmt + &c[..];
            // STMT:Let
            i = i + 1i64;
        }}
    }
    // STMT:Let
    let fmt_q = "\"".to_string().to_string() + &esc_quote_only(fmt.clone())[..] + &"\"".to_string().to_string()[..];
    // STMT:Expr
    return if (args.len() as i64) == 0i64 { fmt_q + &".to_string()".to_string().to_string()[..] } else { "format!(".to_string().to_string() + &fmt_q[..] + &", ".to_string().to_string()[..] + &args_join(args.clone())[..] + &")".to_string().to_string()[..] };
}

pub fn args_join(xs: Vec<String>) -> String {
    // STMT:Expr
    return if (xs.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let head = xs[((0i64) as usize)].to_string();
        // STMT:Expr
        if (xs.len() as i64) > 1i64 { LzAdd::__add__(head.to_string(), ", ".to_string().to_string()) + &args_join(aj_tail(xs.clone()))[..] } else { head.to_string() }
    };
}

pub fn aj_tail(xs: Vec<String>) -> Vec<String> {
    // STMT:Let
    let mut out: Vec<String> = Vec::new();
    // STMT:For
    for idx in (1i64..(xs.len() as i64)).into_iter() {
        // STMT:Expr
        out.push(xs[((idx) as usize)].to_string());
    }
    // STMT:Expr
    return out;
}

pub fn esc_quote_only(s: String) -> String {
    // STMT:Let
    let len: i64 = (s.len() as i64);
    // STMT:Let
    let mut out = "".to_string().to_string();
    // STMT:Let
    let mut i: i64 = 0i64;
    // STMT:Other
    while i < len {
        // STMT:Let
        let c = s[((i) as usize)..(((i + 1i64)) as usize)].to_string();
        // STMT:Expr
        if c == "\"".to_string() {
            // STMT:Let
            out = out + &"\\\"".to_string().to_string()[..];
        } else {
            // STMT:Let
            out = out + &c[..];
        };
        // STMT:Let
        i = i + 1i64;
    }
    // STMT:Expr
    return out;
}

pub fn esc_rust(s: String) -> String {
    // STMT:Let
    let len: i64 = (s.len() as i64);
    // STMT:Let
    let mut out = "".to_string().to_string();
    // STMT:Let
    let mut i: i64 = 0i64;
    // STMT:Other
    while i < len {
        // STMT:Let
        let c: String = s[((i) as usize)..(((i + 1i64)) as usize)].to_string();
        // STMT:Expr
        if (c).to_string() == ("\\".to_string()).to_string() {
            // STMT:Let
            out = out + &"\\\\".to_string()[..];
        } else { if (c).to_string() == ("\"".to_string()).to_string() {
            // STMT:Let
            out = out + &"\\\"".to_string()[..];
        } else { if (c).to_string() == ("\n".to_string()).to_string() {
            // STMT:Let
            out = out + &"\\n".to_string()[..];
        } else { if (c).to_string() == ("\t".to_string()).to_string() {
            // STMT:Let
            out = out + &"\\t".to_string()[..];
        } else { if (c).to_string() == ("\r".to_string()).to_string() {
            // STMT:Let
            out = out + &"\\r".to_string()[..];
        } else {
            // STMT:Let
            out = out + &c[..];
        }}}}};
        // STMT:Let
        i = i + 1i64;
    }
    // STMT:Expr
    return out;
}

pub fn gen_trait_def(name: String, supertraits: Vec<IrType>, methods: Vec<(String, Vec<IrType>, IrType)>) -> String {
    // STMT:Let
    let st_str = if (supertraits.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let mut st_parts = "".to_string().to_string();
        // STMT:For
        for idx in (0i64..(supertraits.len() as i64)).into_iter() {
            // STMT:Let
            let st = supertraits[((idx) as usize)].clone();
            // STMT:Expr
            if idx > 0i64 {
                // STMT:Let
                st_parts = LzAdd::__add__(st_parts, " + ".to_string().to_string());
            } else { ()};
            // STMT:Let
            st_parts = LzAdd::__add__(st_parts, rust_type(st.clone()));
        }
        // STMT:Expr
        LzAdd::__add__(": ".to_string().to_string(), st_parts)
    };
    // STMT:Expr
    return "pub trait ".to_string().to_string() + &name.to_string()[..] + &st_str[..] + &" {\n".to_string().to_string()[..] + &gen_trait_methods(methods.clone())[..] + &"\n}".to_string().to_string()[..];
}

pub fn gen_trait_methods(methods: Vec<(String, Vec<IrType>, IrType)>) -> String {
    // STMT:Let
    let mut out = "".to_string().to_string();
    // STMT:For
    for idx in (0i64..(methods.len() as i64)).into_iter() {
        // STMT:Let
        let m = methods[((idx) as usize)].clone();
        // STMT:Let
        let mut params = "".to_string().to_string();
        // STMT:For
        for pidx in (0i64..(m.1.len() as i64)).into_iter() {
            // STMT:Let
            let pt = m.1[((pidx) as usize)].clone();
            // STMT:Expr
            if pidx > 0i64 {
                // STMT:Other
                params = params + &", ".to_string().to_string()[..];
            } else { ()};
            // STMT:Other
            params = LzAdd::__add__(params, rust_type(pt.clone()));
        }
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(out + &"    fn ".to_string().to_string()[..], m.0.to_string()) + &"(".to_string().to_string()[..] + &params[..] + &") -> ".to_string().to_string()[..], rust_type(m.2.clone())) + &";\n".to_string().to_string()[..];
    }
    // STMT:Expr
    return out;
}

pub fn gen_impl_def(trait_: MaybeIrType, for_type: IrType, method_names: Vec<String>) -> String {
    // STMT:Let
    let trait_part = {
// STMT:Other
match trait_.clone() {
    MaybeIrType::YesTy { value: ty } => {
        // STMT:Expr
        LzAdd::__add__(rust_type(ty.clone()), " for ".to_string().to_string())
    }
    MaybeIrType::NoTy => {
        // STMT:Expr
        "".to_string().to_string()
    }
}
    };
    // STMT:Expr
    return "impl ".to_string().to_string() + &trait_part[..] + &rust_type(for_type.clone())[..] + &" {\n".to_string().to_string()[..] + &gen_impl_methods(method_names.clone())[..] + &"\n}".to_string().to_string()[..];
}

pub fn gen_impl_methods(method_names: Vec<String>) -> String {
    // STMT:Let
    let mut out = "".to_string().to_string();
    // STMT:For
    for idx in (0i64..(method_names.len() as i64)).into_iter() {
        // STMT:Let
        let mut mn = method_names[((idx) as usize)].to_string();
        // STMT:Other
        out = LzAdd::__add__(out + &"    fn ".to_string().to_string()[..], mn.to_string()) + &"(&self) {\n        unimplemented!()\n    }\n".to_string().to_string()[..];
    }
    // STMT:Expr
    return out;
}

pub fn gen_use_stmt(path: Vec<String>, alias: MaybeStr, items: Vec<String>, is_from: bool) -> String {
    // STMT:Let
    let mut path_parts = "".to_string().to_string();
    // STMT:For
    for idx in (0i64..(path.len() as i64)).into_iter() {
        // STMT:Let
        let mut ps = path[((idx) as usize)].to_string();
        // STMT:Expr
        if idx > 0i64 {
            // STMT:Other
            path_parts = path_parts + &"::".to_string().to_string()[..];
        } else { ()};
        // STMT:Other
        path_parts = LzAdd::__add__(path_parts, ps.to_string());
    }
    // STMT:Let
    let path_str = path_parts;
    // STMT:Expr
    return if is_from { if (items.len() as i64) == 1i64 && (items[((0i64) as usize)]).to_string() == ("*".to_string()).to_string() { "use ".to_string().to_string() + &path_str[..] + &"::*;".to_string().to_string()[..]} else {
        // STMT:Let
        let mut item_parts = "".to_string().to_string();
        // STMT:For
        for idx in (0i64..(items.len() as i64)).into_iter() {
            // STMT:Let
            let it = items[((idx) as usize)].to_string();
            // STMT:Expr
            if idx > 0i64 {
                // STMT:Let
                item_parts = LzAdd::__add__(item_parts, ", ".to_string().to_string());
            } else { ()};
            // STMT:Let
            item_parts = LzAdd::__add__(item_parts, it.to_string());
        }
        // STMT:Expr
        LzAdd::__add__("use ".to_string().to_string() + &path_str[..] + &"::{".to_string().to_string()[..], item_parts) + &"}".to_string().to_string()[..]
    }} else { "use ".to_string().to_string() + &path_str[..] + &";".to_string().to_string()[..]};
}

pub fn gen_type_alias(name: String, ty: IrType) -> String {
    // STMT:Expr
    return "pub type ".to_string().to_string() + &name.to_string()[..] + &" = ".to_string().to_string()[..] + &rust_type(ty.clone())[..] + &";".to_string().to_string()[..];
}

pub fn gen_test_def(name: String, body: Vec<Stmt>) -> String {
    // STMT:Expr
    return "#[test]\nfn ".to_string().to_string() + &name.to_string()[..] + &"() {\n".to_string().to_string()[..] + &gen_body(body.clone(), false, auto_mut_names(body.clone()))[..] + &"}".to_string().to_string()[..];
}

pub fn gen_item(i: Item) -> String {
    // STMT:Other
    match i.clone() {
        Item::FnDef { name: n, generics: gs, params: ps, ret: r, body: b } => {
            // STMT:Let
            let is_main: bool = ((n).to_string() == ("main".to_string()).to_string());
            // STMT:Let
            let ret_s = if (is_main && r_eq_unit(r.clone())) { "".to_string().to_string() } else { (" -> ".to_string().to_string() + &rust_type(r.clone())[..]) };
            // STMT:Expr
            "pub fn ".to_string().to_string() + &n.to_string()[..] + &generic_sig(gs.clone())[..] + &"(".to_string().to_string()[..] + &fn_param_list(ps.clone())[..] + &")".to_string().to_string()[..] + &ret_s[..] + &" {\n".to_string().to_string()[..] + &gen_body(b.clone(), is_main, auto_mut_names(b.clone()))[..] + &"}".to_string().to_string()[..]
        }
        Item::StructDef { name: n, generics: gs, fields: fs } => {
            // STMT:Expr
            "#[derive(Debug, Clone, PartialEq)]\npub struct ".to_string().to_string() + &n.to_string()[..] + &generic_sig(gs.clone())[..] + &" {\n".to_string().to_string()[..] + &gen_struct_fields(fs.clone())[..] + &"}".to_string().to_string()[..]
        }
        Item::EnumDef { name: n, generics: gs, variants: vs } => {
            // STMT:Expr
            "#[derive(Debug, Clone, PartialEq)]\npub enum ".to_string().to_string() + &n.to_string()[..] + &generic_sig(gs.clone())[..] + &" {\n".to_string().to_string()[..] + &gen_enum_variants(vs.clone())[..] + &"}".to_string().to_string()[..]
        }
        Item::Const { name: n, ty: t, value: v } => {
            // STMT:Expr
            gen_const_item(n.clone(), t.clone(), v.clone())
        }
        Item::TraitDef { name: n, supertraits: sts, methods: ms } => {
            // STMT:Expr
            gen_trait_def(n.clone(), sts.clone(), ms.clone())
        }
        Item::Impl { trait_: tr, for_type: ft, method_names: mns } => {
            // STMT:Expr
            gen_impl_def(tr.clone(), ft.clone(), mns.clone())
        }
        Item::UseStmt { path: pth, alias: al, items: its, is_from: fr } => {
            // STMT:Expr
            gen_use_stmt(pth.clone(), al.clone(), its.clone(), fr)
        }
        Item::TypeAlias { name: n, ty: t } => {
            // STMT:Expr
            gen_type_alias(n.clone(), t.clone())
        }
        Item::Test { name: n, body: b } => {
            // STMT:Expr
            gen_test_def(n.clone(), b.clone())
        }
        _ => {
            // STMT:Expr
            "// TODO Item ".to_string().to_string() + &display_item(i.clone())[..]
        }
    }
}

pub fn r_eq_unit(t: IrType) -> bool {
    // STMT:Other
    match t.clone() {
        IrType::Unit => {
            // STMT:Expr
            true
        }
        _ => {
            // STMT:Expr
            false
        }
    }
}

pub fn fn_param_list(ps: Vec<(String, IrType, bool, bool, bool)>) -> String {
    // STMT:Expr
    return if (ps.len() as i64) == 0i64 { "".to_string().to_string()} else {
        // STMT:Let
        let p0 = ps[((0i64) as usize)].clone();
        // STMT:Expr
        fn_param_str(p0.clone()) + &(if (ps.len() as i64) > 1i64 { ", ".to_string().to_string() + &fn_param_list(pl_tail(ps.clone()))[..] } else { "".to_string() })
    };
}

pub fn fn_param_str(p: (String, IrType, bool, bool, bool)) -> String {
    // STMT:Expr
    return LzAdd::__add__(p.0.to_string(), ": ".to_string().to_string()) + &rust_type(p.1.clone())[..];
}

pub fn pl_tail(ps: Vec<(String, IrType, bool, bool, bool)>) -> Vec<(String, IrType, bool, bool, bool)> {
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

pub fn gen_body(ss: Vec<Stmt>, is_main: bool, auto_mut: String) -> String {
    // STMT:Let
    let n: i64 = (ss.len() as i64);
    // STMT:Expr
    return if n == 0i64 { "    loop {\n        unimplemented!()\n    }\n".to_string().to_string()} else {
        // STMT:Let
        let mut out = String::new();
        // STMT:For
        for idx in (0i64..n).into_iter() {
            // STMT:Let
            let is_tail: bool = (idx == n - 1i64);
            // STMT:Let
            let stmt_type = stmt_type_label(ss[((idx) as usize)].clone());
            // STMT:Let
            out = LzAdd::__add__(LzAdd::__add__(out + &"    // STMT:".to_string().to_string()[..], stmt_type) + &"\n    ".to_string().to_string()[..], gen_stmt(ss[((idx) as usize)].clone(), is_tail, is_main, auto_mut.clone())) + &"\n".to_string().to_string()[..];
        }
        // STMT:Expr
        out
    };
}

pub fn stmt_type_label(s: Stmt) -> String {
    // STMT:Other
    match s.clone() {
        Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
            // STMT:Expr
            "Let".to_string().to_string()
        }
        Stmt::ExprStmt { expr: e } => {
            // STMT:Expr
            "Expr".to_string().to_string()
        }
        Stmt::TryCatch { body: b, catches: cs, else_body: eb, finally_body: fb } => {
            // STMT:Expr
            "Try".to_string().to_string()
        }
        Stmt::Block { stmts: b } => {
            let b = *b;
            // STMT:Expr
            "Block".to_string().to_string()
        }
        Stmt::Defer { body: b } => {
            // STMT:Expr
            "Defer".to_string().to_string()
        }
        Stmt::If { cond: c, then: t, els: e } => {
            // STMT:Expr
            "If".to_string().to_string()
        }
        Stmt::For { var: v, iter: it, guard_e: g, body: b, else_body: eb } => {
            // STMT:Expr
            "For".to_string().to_string()
        }
        _ => {
            // STMT:Expr
            "Other".to_string().to_string()
        }
    }
}

pub fn auto_mut_names(ss: Vec<Stmt>) -> String {
    // STMT:Let
    let mut out = String::new();
    // STMT:For
    for idx in (0i64..(ss.len() as i64)).into_iter() {
        // STMT:Other
        out = scan_stmt_auto_mut(ss[((idx) as usize)].clone(), out.clone());
    }
    // STMT:Expr
    return out;
}

pub fn scan_stmt_auto_mut(s: Stmt, out: String) -> String {
    // STMT:Let
    let mut acc = out.clone();
    // STMT:Other
    match s.clone() {
        Stmt::Let { name: n, ty: t, value: v, is_mut: m, is_ref: r } => {
            // STMT:Expr
            scan_expr_auto_mut(v.clone(), out.clone())
        }
        Stmt::Assign { target: tg, value: v } => {
            // STMT:Other
            acc = add_var_auto_mut(tg.clone(), acc.clone());
            // STMT:Other
            acc = scan_expr_auto_mut(tg.clone(), acc.clone());
            // STMT:Other
            acc = scan_expr_auto_mut(v.clone(), acc.clone());
            // STMT:Expr
            acc
        }
        Stmt::ExprStmt { expr: e } => {
            // STMT:Expr
            scan_expr_auto_mut(e.clone(), out.clone())
        }
        Stmt::Return { value: v } => {
            // STMT:Other
            match v.clone() {
                MaybeExpr::YesExpr { value: inner } => {
                    // STMT:Expr
                    scan_expr_auto_mut(inner.clone(), out.clone())
                }
                MaybeExpr::NoExpr => {
                    // STMT:Expr
                    out
                }
            }
        }
        Stmt::If { cond: c, then: t, els: e } => {
            // STMT:Other
            acc = scan_expr_auto_mut(c.clone(), acc.clone());
            // STMT:Other
            acc = scan_block_stmts_auto_mut(t.clone(), acc.clone());
            // STMT:Other
            acc = scan_block_maybe_auto_mut(e.clone(), acc.clone());
            // STMT:Expr
            acc
        }
        _ => {
            // STMT:Expr
            out
        }
    }
}

pub fn scan_block_stmts_auto_mut(b: BlockIR, out: String) -> String {
    // STMT:Let
    let mut acc = out.clone();
    // STMT:Other
    match b.clone() {
        BlockIR::Block { stmts: bss, ty: bt } => {
            // STMT:For
            for idx in (0i64..(bss.len() as i64)).into_iter() {
                // STMT:Other
                acc = scan_stmt_auto_mut(bss[((idx) as usize)].clone(), acc.clone());
            }
            // STMT:Expr
            acc
        }
    }
}

pub fn scan_block_maybe_auto_mut(mb: MaybeBlock, out: String) -> String {
    // STMT:Let
    let mut acc = out.clone();
    // STMT:Other
    match mb.clone() {
        MaybeBlock::YesBlock { value: es } => {
            // STMT:Expr
            scan_block_stmts_auto_mut(es.clone(), acc.clone())
        }
        MaybeBlock::NoBlock => {
            // STMT:Expr
            out
        }
    }
}

pub fn add_var_auto_mut(e: Expr, out: String) -> String {
    // STMT:Other
    match e.clone() {
        Expr::Var { name: n, ty: t } => {
            // STMT:Expr
            if str_contains(out.clone(), n.clone()) { out.clone()} else {
                // STMT:Let
                let sep = if (out.len() as i64) > 0i64 { ",".to_string().to_string() } else { "".to_string().to_string() };
                // STMT:Expr
                LzAdd::__add__(out, sep) + &n.to_string()[..]
            }
        }
        _ => {
            // STMT:Expr
            out.clone()
        }
    }
}

pub fn str_contains(s: String, sub: String) -> bool {
    // STMT:Let
    let len: i64 = (s.len() as i64);
    // STMT:Let
    let sub_len: i64 = (sub.len() as i64);
    // STMT:Expr
    return if sub_len == 0i64 { false} else {
        // STMT:Let
        let mut i: i64 = 0i64;
        // STMT:Let
        let mut found: bool = false;
        // STMT:Other
        while LzAdd::__add__(i, sub_len) <= len {
            // STMT:Expr
            if s[((i) as usize)..(((LzAdd::__add__(i, sub_len))) as usize)].to_string() == sub {
                // STMT:Let
                found = true;
            } else { ()};
            // STMT:Let
            i = i + 1i64;
        }
        // STMT:Expr
        found
    };
}

pub fn scan_expr_auto_mut(e: Expr, out: String) -> String {
    // STMT:Let
    let mut acc = out.clone();
    // STMT:Other
    match e.clone() {
        Expr::Call { callee: c, args: a, ty: t } => {
            let c = *c;
            let a = *a;
            // STMT:Other
            acc = scan_expr_auto_mut(c.clone(), acc.clone());
            // STMT:For
            for idx in (0i64..(a.len() as i64)).into_iter() {
                // STMT:Other
                acc = scan_expr_auto_mut(a[((idx) as usize)].clone(), acc.clone());
            }
            // STMT:Expr
            if callee_is_mut_fn(c.clone()) && (a.len() as i64) > 0i64 {
                // STMT:Other
                acc = add_var_auto_mut(a[((0i64) as usize)].clone(), acc.clone());
            } else { ()};
            // STMT:Expr
            acc
        }
        Expr::MethodCall { receiver: r, method: m, args: a, ty: t } => {
            let r = *r;
            let a = *a;
            // STMT:Other
            acc = add_var_auto_mut(r.clone(), acc.clone());
            // STMT:Other
            acc = scan_expr_auto_mut(r.clone(), acc.clone());
            // STMT:For
            for idx in (0i64..(a.len() as i64)).into_iter() {
                // STMT:Other
                acc = scan_expr_auto_mut(a[((idx) as usize)].clone(), acc.clone());
            }
            // STMT:Expr
            acc
        }
        Expr::IndexSet { base: b, key: k, value: v, ty: t } => {
            let b = *b;
            let k = *k;
            let v = *v;
            // STMT:Other
            acc = add_var_auto_mut(b.clone(), acc.clone());
            // STMT:Other
            acc = scan_expr_auto_mut(b.clone(), acc.clone());
            // STMT:Other
            acc = scan_expr_auto_mut(k.clone(), acc.clone());
            // STMT:Other
            acc = scan_expr_auto_mut(v.clone(), acc.clone());
            // STMT:Expr
            acc
        }
        Expr::AssignExpr { target: tg, value: v, ty: t } => {
            let tg = *tg;
            let v = *v;
            // STMT:Other
            acc = add_var_auto_mut(tg.clone(), acc.clone());
            // STMT:Other
            acc = scan_expr_auto_mut(tg.clone(), acc.clone());
            // STMT:Other
            acc = scan_expr_auto_mut(v.clone(), acc.clone());
            // STMT:Expr
            acc
        }
        Expr::FieldAccess { base: b, field: f, ty: t } => {
            let b = *b;
            // STMT:Expr
            scan_expr_auto_mut(b.clone(), out.clone())
        }
        Expr::IndexGet { base: b, key: k, ty: t } => {
            let b = *b;
            let k = *k;
            // STMT:Other
            acc = scan_expr_auto_mut(b.clone(), acc.clone());
            // STMT:Expr
            scan_expr_auto_mut(k.clone(), acc.clone())
        }
        _ => {
            // STMT:Expr
            out
        }
    }
}

pub fn callee_is_mut_fn(c: Expr) -> bool {
    // STMT:Other
    match c.clone() {
        Expr::Var { name: nn, ty: tt } => {
            // STMT:Expr
            if (nn).to_string() == ("add".to_string()).to_string() || (nn).to_string() == ("push".to_string()).to_string() || (nn).to_string() == ("append".to_string()).to_string() || (nn).to_string() == ("pop".to_string()).to_string() || (nn).to_string() == ("extend".to_string()).to_string() || (nn).to_string() == ("insert".to_string()).to_string() || (nn).to_string() == ("remove".to_string()).to_string() || (nn).to_string() == ("delete".to_string()).to_string() { true } else { false }
        }
        _ => {
            // STMT:Expr
            false
        }
    }
}

pub fn gen_struct_fields(fs: Vec<(String, IrType)>) -> String {
    // STMT:Let
    let mut out = "".to_string().to_string();
    // STMT:For
    for idx in (0i64..(fs.len() as i64)).into_iter() {
        // STMT:Let
        let f = fs[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(out + &"    pub ".to_string().to_string()[..], f.0.to_string()) + &": ".to_string().to_string()[..], rust_type(f.1.clone())) + &",\n".to_string().to_string()[..];
    }
    // STMT:Expr
    return out;
}

pub fn gen_enum_variants(vs: Vec<(String, Vec<IrType>)>) -> String {
    // STMT:Let
    let mut out = "".to_string().to_string();
    // STMT:For
    for idx in (0i64..(vs.len() as i64)).into_iter() {
        // STMT:Let
        let v = vs[((idx) as usize)].clone();
        // STMT:Other
        out = LzAdd::__add__(LzAdd::__add__(out + &"    ".to_string().to_string()[..], v.0.to_string()), gen_enum_variant_args(v.1.clone())) + &",\n".to_string().to_string()[..];
    }
    // STMT:Expr
    return out;
}

pub fn gen_enum_variant_args(ts: Vec<IrType>) -> String {
    // STMT:Expr
    return if (ts.len() as i64) == 0i64 { "".to_string().to_string() } else { "(".to_string().to_string() + &type_rust_list(ts.clone())[..] + &")".to_string().to_string()[..] };
}

pub fn gen_const_item(n: String, t: IrType, v: Expr) -> String {
    // STMT:Other
    match t.clone() {
        IrType::Str => {
            // STMT:Other
            match v.clone() {
                Expr::LitStr { s: sv, ty: tv } => {
                    // STMT:Expr
                    "const ".to_string().to_string() + &n.to_string()[..] + &": &str = \"".to_string().to_string()[..] + &esc_rust(sv.clone())[..] + &"\";".to_string().to_string()[..]
                }
                _ => {
                    // STMT:Expr
                    "const ".to_string().to_string() + &n.to_string()[..] + &": String = ".to_string().to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string().to_string()[..]
                }
            }
        }
        _ => {
            // STMT:Expr
            "const ".to_string().to_string() + &n.to_string()[..] + &": ".to_string().to_string()[..] + &rust_type(t.clone())[..] + &" = ".to_string().to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string().to_string()[..]
        }
    }
}

pub fn is_magic_name(n: String) -> bool {
    // STMT:Expr
    return (n).to_string() == ("__name__".to_string()).to_string() || (n).to_string() == ("__file__".to_string()).to_string() || (n).to_string() == ("__package__".to_string()).to_string() || (n).to_string() == ("__path__".to_string()).to_string() || (n).to_string() == ("__doc__".to_string()).to_string() || (n).to_string() == ("__is_macro__".to_string()).to_string();
}

pub fn gen_magic_const(n: String, t: IrType, v: Expr) -> String {
    // STMT:Expr
    return if (n).to_string() == ("__is_macro__".to_string()).to_string() { "const ".to_string().to_string() + &n.to_string()[..] + &": bool = ".to_string().to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string().to_string()[..] } else { "const ".to_string().to_string() + &n.to_string()[..] + &": &str = ".to_string().to_string()[..] + &gen_expr(v.clone())[..] + &";".to_string().to_string()[..] };
}

pub fn codegen_module(m: IrModule) -> String {
    // STMT:Let
    let mut out = module_header();
    // STMT:For
    for idx in (0i64..(m.items.len() as i64)).into_iter() {
        // STMT:Other
        out = LzAdd::__add__(out, gen_item(m.items[((idx) as usize)].clone())) + &"\n\n".to_string().to_string()[..];
    }
    // STMT:Expr
    return out[((0i64) as usize)..((((out.len() as i64) - 2i64)) as usize)].to_string().clone() + &"\n".to_string().to_string()[..];
}

pub fn module_header() -> String {
    // STMT:Expr
    return "\n#[allow(unused_imports)]\n#[allow(unused_variables)]\n#[allow(dead_code)]\n#[allow(non_snake_case)]\n\nuse std::collections::{HashMap, HashSet};\nuse std::any::Any;\nuse std::rc::Rc;\nuse std::sync::Arc;\nuse std::fmt::Debug;\nuse std::fmt::Display;\n\nuse lz_builtins::*;\n\n".to_string().to_string();
}

const __name__: &str = "main";

const __file__: &str = "lz_codegen_lib.lz";

const __package__: &str = "ir";

const __path__: &str = "src/ir";

const __doc__: &str = "";

const __is_macro__: bool = false;

pub fn main() {
}

// Lang-Zong 编译器 — ast/expr.rs
// 表达式类 AST 节点：Expr, BuildKind, BinOp, UnaryOp, AssignOp

use super::stmt::{Stmt, MatchArm};

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    FStrLit(String),
    RawStrLit(String),
    BoolLit(bool),
    NoneLit,
    Ident(String),

    // 容器
    ListLit(Vec<Expr>),
    DictLit(Vec<(Expr, Expr)>),
    SetLit(Vec<Expr>),
    TupleLit(Vec<Expr>),

    // 运算
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    // 调用
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
        type_args: Vec<String>,
    },
    KwArg {
        name: String,
        value: Box<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    FieldAccess {
        receiver: Box<Expr>,
        field: String,
    },
    PathAccess {
        receiver: Box<Expr>,
        segment: String,
    },
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },

    // 控制流表达式
    If {
        cond: Box<Expr>,
        then_body: Vec<Stmt>,
        elif_clauses: Vec<(Expr, Vec<Stmt>)>,
        else_body: Option<Vec<Stmt>>,
    },
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    // 特殊
    Closure {
        params: Vec<String>,
        body: Box<Expr>,
    },
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },
    Walrus {
        target: Box<Expr>,
        value: Box<Expr>,
    },
    Pipe {
        receiver: Box<Expr>,
        func: String,
        args: Vec<Expr>,
    },
    SafeNav {
        receiver: Box<Expr>,
        field: String,
    },
    Try(Box<Expr>),
    NullCoalesce {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    ListComprehension {
        output: Box<Expr>,
        var: String,
        iter: Box<Expr>,
        cond: Option<Box<Expr>>,
    },
    DictComprehension {
        key: Box<Expr>,
        value: Box<Expr>,
        var: String,
        iter: Box<Expr>,
        cond: Option<Box<Expr>>,
    },
    SetComprehension {
        elem: Box<Expr>,
        var: String,
        iter: Box<Expr>,
        cond: Option<Box<Expr>>,
    },
    Assign {
        target: Box<Expr>,
        op: AssignOp,
        value: Box<Expr>,
    },
    Spawn(Box<Expr>),
    Move(Box<Expr>),
    Panic(Box<Expr>),
    Await(Box<Expr>),

    // 构建块
    BuildBlock {
        kind: BuildKind,
        lhs: Box<Expr>,
        body: Vec<Stmt>,
    },

    // try/catch/else 错误捕获
    TryCatch {
        body: Vec<Stmt>,
        catches: Vec<MatchArm>,
        else_body: Option<Vec<Stmt>>,
        finally_body: Option<Vec<Stmt>>,
    },

    /// 括号分组 (expr) — 保留优先级信息
    Paren(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildKind {
    Var,
    Call,
    Gen,
    Index,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod, Pow,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
    In, Is,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Neg, Not, BitNot,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssignOp {
    Eq, AddEq, SubEq, MulEq, DivEq, ModEq,
    AndEq, OrEq, XorEq, ShlEq, ShrEq, PowEq,
}

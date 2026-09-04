// Lang-Zong 编译器 — ast/expr.rs
// 表达式类 AST 节点：Expr, BuildKind, BinOp, UnaryOp, AssignOp

use super::stmt::{Stmt, MatchArm};
use crate::types::Type;

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

    /// 列表展开元素：`[0, ...a, 4]` 中的 `...a`（BUG-SG-005）
    Spread(Box<Expr>),

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
        /// 参数类型注解（与 params 一一对应；无注解为 None）。
        /// 修复 E0282：`|x: int|` 的类型原被 parse_type() 丢弃，导致
        /// Option.None.map(|x: int| ...) 生成无类型闭包无法推断
        param_tys: Vec<Option<Type>>,
        /// 返回类型注解（`|x| -> T = ...`；无注解为 None）。
        /// 修复 E0283：`or_else(b, |e: str| -> Result<int, int> = Ok(100))`
        /// 的返回类型原被丢弃，Rust 闭包无法从 Ok(100) 推断 Err 泛型
        ret_ty: Option<Type>,
        body: Box<Expr>,
    },
    // 块表达式（|x| => block body）
    BlockExpr(Vec<Stmt>),
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
        /// 右侧 callable 完整表达式：函数名 Ident / 闭包 Closure / 方法 PathAccess /
        /// 构造调用 Call / 变量 Ident（实现 __call__ 的实例）
        callee: Box<Expr>,
        /// 显式实参（首参预填充 receiver 后追加）
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
        extra_clauses: Vec<(String, Box<Expr>, Option<Box<Expr>>)>,
    },
    DictComprehension {
        key: Box<Expr>,
        value: Box<Expr>,
        var: String,
        iter: Box<Expr>,
        cond: Option<Box<Expr>>,
        extra_clauses: Vec<(String, Box<Expr>, Option<Box<Expr>>)>,
    },
    SetComprehension {
        elem: Box<Expr>,
        var: String,
        iter: Box<Expr>,
        cond: Option<Box<Expr>>,
        extra_clauses: Vec<(String, Box<Expr>, Option<Box<Expr>>)>,
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

    /// comptime 表达式：`comptime <expr>` — 编译期求值后内联结果
    Comptime(Box<Expr>),
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
    In, NotIn, Is,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Neg, Not, BitNot,
    /// 一元 `*` 解引用（`*(&(*boxed))` 前缀叠写，12-操作符.md §1.18）
    Deref,
    /// 一元 `&` 取引用（`*(&(*boxed))` 前缀叠写）
    Ref,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssignOp {
    Eq, AddEq, SubEq, MulEq, DivEq, ModEq,
    AndEq, OrEq, XorEq, ShlEq, ShrEq, PowEq,
}

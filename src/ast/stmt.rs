// Lang-Zong 编译器 — ast/stmt.rs
// 语句类 AST 节点：Stmt, MatchArm, Pattern

use crate::types::Type;
use super::expr::{Expr, AssignOp};
use super::decl::StructDef;
use super::decl::Function;

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    Let {
        name: String,
        mutable: bool,
        is_ref: bool,
        ty: Option<Type>,
        value: Expr,
    },
    Const {
        name: String,
        ty: Option<Type>,
        value: Expr,
    },
    Return(Option<Expr>),
    Yield(Option<Expr>),
    YieldFrom(Expr),  // yield from expr — 委托生成器
    While {
        cond: Expr,
        guard: Option<Expr>,          // while cond if guard:
        body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,  // while ... else:
    },
    WhileLet {
        pattern: Pattern,
        expr: Expr,
        guard: Option<Expr>,
        body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    For {
        var: String,
        iter: Expr,
        guard: Option<Expr>,          // for x in iter if guard:
        body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,  // for ... else:
    },
    Loop(Vec<Stmt>),
    Break(Option<Expr>),
    /// break label: value  — 命名块跳出（可选带值）
    BreakLabel { label: String, value: Option<Expr> },
    Continue,
    /// block label: body  — 命名块，break label 可跨层跳出
    Block { label: String, body: Vec<Stmt> },
    /// block NAME[ps: __Params]: body  — checker 块（惰性登记，不执行）
    CheckerBlock { label: String, ps_name: String, body: Vec<Stmt> },
    Defer(Vec<Stmt>),
    Raise(Expr),
    Guard {
        cond: Option<Expr>,
        let_binding: Option<(Pattern, Expr)>,
        success_expr: Option<Expr>,   // guard cond success_expr else fail_body
        else_body: Vec<Stmt>,
    },
    With {
        expr: Expr,
        alias: Option<String>,
        body: Vec<Stmt>,
    },
    /// 函数体内的 enum 定义
    EnumDef(StructDef),
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
    },

    FnDef { func: Function },

    // ── 占位符 ──
    Pass,

    // ���─ 测试 ──
    Test {
        name: String,
        body: Vec<Stmt>,
    },
    Assert {
        expr: Expr,
        expected: Option<Expr>,
    },
    Check {
        expr: Expr,
        message: Option<Expr>,
    },
    Suite {
        name: String,
        setup: Option<Vec<Stmt>>,
        teardown: Option<Vec<Stmt>>,
        tests: Vec<Stmt>,
    },

    // ── 编译期 ──
    Comptime {
        body: Vec<Stmt>,
    },

    // ── 局部类型别名 ──
    TypeAlias {
        name: String,
        ty: Type,
    },

    // ── 解构绑定 ──
    LetTuple {
        names: Vec<String>,
        ty: Option<Type>,
        value: Expr,
    },
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Int(i64),
    Str(String),
    Bool(bool),
    Ident(String),
    Variant(String, Vec<Pattern>),
    Tuple(Vec<Pattern>),
    List(Vec<Pattern>),
    Range { start: i64, end: i64, inclusive: bool },
    Wildcard,
}

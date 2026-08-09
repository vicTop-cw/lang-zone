// Lang-Zong 编译器 — ast/stmt.rs
// 语句类 AST 节点：Stmt, MatchArm, Pattern

use super::decl::Function;
use super::decl::StructDef;
use super::expr::{AssignOp, Expr};
use crate::types::Type;

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    Let {
        name: String,
        mutable: bool,
        is_ref: bool,
        /// owned 绑定：强制显式消费（x^ / return / 传 owned 形参），消费后毒化
        is_owned: bool,
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
    YieldFrom(Expr), // yield from expr — 委托生成器
    While {
        cond: Expr,
        guard: Option<Expr>, // while cond if guard:
        body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>, // while ... else:
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
        guard: Option<Expr>, // for x in iter if guard:
        body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>, // for ... else:
    },
    Loop(Vec<Stmt>),
    Break(Option<Expr>),
    /// break label: value  — 命名块跳出（可选带值）
    BreakLabel {
        label: String,
        value: Option<Expr>,
    },
    Continue,
    /// block label: body  — 命名块，break label 可跨层跳出
    Block {
        label: String,
        body: Vec<Stmt>,
    },
    /// block NAME[ps: __Params]: body  — checker 块（惰性，定义 ps）
    /// block NAME[chk]: body           — checker 块（惰性，引用已有检查站）
    /// block NAME[None]: body          — checker 块（显式无检查站）
    CheckerBlock {
        label: String,
        ps_name: Option<String>,
        default_checker: Option<String>,
        body: Vec<Stmt>,
    },
    Defer(Vec<Stmt>),
    Raise(Expr),
    Guard {
        cond: Option<Expr>,
        let_binding: Option<(Pattern, Expr)>,
        success_expr: Option<Expr>, // guard cond success_expr else fail_body
        else_body: Vec<Stmt>,
    },
    With {
        expr: Expr,
        alias: Option<String>,
        body: Vec<Stmt>,
    },
    /// checker 块触发调用（block NAME ^: / block NAME[(expr)]）
    BlockCall {
        label: String,
        args: Expr,
    },
    /// 函数体内的 enum 定义
    EnumDef(StructDef),
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
    },

    FnDef {
        func: Function,
    },

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
    /// `ref mut name` 模式绑定：c 绑定为 &mut 引用（case Some(ref mut c)）
    RefMutIdent(String),
    Variant(String, Vec<Pattern>),
    Tuple(Vec<Pattern>),
    List(Vec<Pattern>),
    /// `{"k": p, ...}` 字典模式：键匹配 + 值绑定（未列出的键忽略）
    Dict(Vec<(String, Pattern)>),
    /// `..` / `..rest` 剩余绑定（仅出现在 List 模式内）
    Rest(Option<String>),
    Range {
        start: i64,
        end: i64,
        inclusive: bool,
    },
    Wildcard,
}

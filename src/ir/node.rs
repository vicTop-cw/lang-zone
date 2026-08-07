// Lang-Zone 编译器 — ir/node.rs
// LZIR-H 节点定义：Item, Stmt, Expr, Pattern 及辅助类型
//
// 形态：强类型树 / ANF 风格。每个 Expr 携带 IrType 与 Span。

use super::types::IrType;

// ── 源码位置 ──

/// 源码区间
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn new(line: usize, col: usize) -> Self {
        Span {
            start: 0,
            end: 0,
            line,
            col,
        }
    }
    pub fn unknown() -> Self {
        Span {
            start: 0,
            end: 0,
            line: 0,
            col: 0,
        }
    }
}

// ── 泛型参数 ──

#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<IrType>,
    pub default: Option<IrType>,
}

// ── 魔法属性 ──

// ── 模块顶层指令 ──

/// 后端语言
#[derive(Debug, Clone, PartialEq)]
pub enum Backend {
    Rust,
    Cython,
    Wasm,
}

impl Default for Backend {
    fn default() -> Self {
        Backend::Rust
    }
}

/// 模块类型
#[derive(Debug, Clone, PartialEq)]
pub enum ModuleKind {
    Normal,
    Macro,
    Template,
    Prelude,
    Test,
}

impl Default for ModuleKind {
    fn default() -> Self {
        ModuleKind::Normal
    }
}

/// 顶层编译指令
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDirective {
    pub backend: Backend,
    pub kind: ModuleKind,
    pub bridge: Option<String>,
    pub bridge_tier: Option<String>,
    pub name: Option<String>,
    pub doc: Option<String>,
    pub public: Vec<String>,
    pub private: Vec<String>,
    pub deps: Vec<String>,
    pub no_std: bool,
}

impl Default for ModuleDirective {
    fn default() -> Self {
        ModuleDirective {
            backend: Backend::default(),
            kind: ModuleKind::default(),
            bridge: None,
            bridge_tier: None,
            name: None,
            doc: None,
            public: vec![],
            private: vec![],
            deps: vec![],
            no_std: false,
        }
    }
}

/// 旧字段，保留兼容
#[derive(Debug, Clone, PartialEq)]
pub struct MagicAttrs {
    pub name: Option<String>,
    pub doc: Option<String>,
    pub all: Option<Vec<String>>,
    pub bridge: Option<String>,
    pub bridge_tier: Option<String>,
}

impl Default for MagicAttrs {
    fn default() -> Self {
        MagicAttrs {
            name: None,
            doc: None,
            all: None,
            bridge: None,
            bridge_tier: None,
        }
    }
}

impl From<&ModuleDirective> for MagicAttrs {
    fn from(d: &ModuleDirective) -> Self {
        MagicAttrs {
            name: d.name.clone(),
            doc: d.doc.clone(),
            all: if d.public.is_empty() {
                None
            } else {
                Some(d.public.clone())
            },
            bridge: d.bridge.clone(),
            bridge_tier: d.bridge_tier.clone(),
        }
    }
}

// ── 内建装饰器 ──

#[derive(Debug, Clone, PartialEq)]
pub struct Intrinsic {
    pub kind: IntrinsicKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IntrinsicKind {
    Memoize,
    Parallel,
    Curry,
    Overload,
    Derive,
    TailCall,
    Export(Vec<String>), // @export(Rust), @export(Python)
    Init,
}

// ── 函数签名（用于 Trait 声明） ──

#[derive(Debug, Clone, PartialEq)]
pub struct FnSig {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub params: Vec<IrType>,
    pub ret: IrType,
}

// ── 参数 ──

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: IrType,
    pub is_mut: bool,
    /// 是否为引用参数（ref self / ref x）
    pub is_ref: bool,
    /// 是否为 owned 参数（owned self）
    pub is_owned: bool,
    /// 默认值（可选）
    pub default: Option<Expr>,
    /// 是否为 variadic 参数（..name: T → 在调用处收集剩余实参为切片）
    pub variadic: bool,
}

// ── 字段 ──

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: IrType,
}

// ── 枚举变体 ──

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: String,
    /// 变体字段：名称 + 类型（空名称 = 位置/元组字段，非空 = 命名字段）
    pub fields: Vec<Field>,
}

// ══════════════════════════════════════════════════════════════
// Item — 顶层定义项
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    FnDef(FnDef),
    StructDef(StructDef),
    EnumDef(EnumDef),
    TraitDef(TraitDef),
    Impl(ImplDef),
    Use(UseStmt),
    Const(ConstDef),
    TypeAlias(TypeAliasDef),
    Test(TestDef),
    /// checker 块 → 编译为 fn NAME(ps: &mut __Params)
    CheckerBlock {
        name: String,
        ps_name: Option<String>,
        default_checker: Option<String>,
        body: Block,
    },
    /// duck 类型约束 → 编译为 Rust trait
    DuckDef(DuckDef),
}

/// 函数定义
#[derive(Debug, Clone, PartialEq)]
pub struct FnDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub ret_ty: IrType,
    pub body: Block,
    pub intrinsics: Vec<Intrinsic>,
    pub is_async: bool,
    pub is_iterator: bool, // iterator 关键字定义的生成器
    pub is_test: bool,
    /// checker 参数名（def f[ps: __Params]）
    pub checker_param: Option<String>,
    /// 引用的默认检查站名（def f[cache]）
    pub default_checker: Option<String>,
    pub span: Span,
}

/// 结构体定义
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub fields: Vec<Field>,
    pub methods: Vec<FnDef>,
    /// 是否定义了 __new__ 魔术构造（用于构造时补齐默认字段）
    pub has_new: bool,
    /// __new__ 的参数列表（用于 codegen 生成 __lz_new 函数签名）
    pub new_params: Vec<(String, IrType)>,
    pub new_ret_ty: Option<IrType>,
    /// 是否定义了 __init__ 后初始化方法
    pub has_init: bool,
    pub init_params: Vec<(String, IrType)>,
    /// __implicit_from__ 隐式转换（源类型列表）
    pub implicit_froms: Vec<IrType>,
    pub span: Span,
}

/// 枚举定义
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub variants: Vec<Variant>,
    pub methods: Vec<FnDef>,
    pub span: Span,
}

/// Trait 定义
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub supertraits: Vec<IrType>,
    pub methods: Vec<FnSig>,
}

/// Impl 定义
#[derive(Debug, Clone, PartialEq)]
pub struct ImplDef {
    pub trait_: Option<IrType>, // None = inherent impl
    pub for_type: IrType,
    pub generics: Vec<GenericParam>,
    pub methods: Vec<FnDef>,
}

/// Use 语句（仅记录依赖路径）
#[derive(Debug, Clone, PartialEq)]
pub struct UseStmt {
    pub path: Vec<String>,
    pub items: Vec<String>,
    pub is_from: bool,
}

/// 常量定义
#[derive(Debug, Clone, PartialEq)]
pub struct ConstDef {
    pub name: String,
    pub ty: IrType,
    pub value: Expr,
}

/// 类型别名定义
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAliasDef {
    pub name: String,
    pub ty: IrType,
}

/// 测试定义
#[derive(Debug, Clone, PartialEq)]
pub struct TestDef {
    pub name: String,
    pub body: Block,
}

/// Duck 类型约束定义 — 编译期结构匹配，零开销
#[derive(Debug, Clone, PartialEq)]
pub struct DuckDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    /// 方法签名列表
    pub methods: Vec<DuckMethod>,
    /// 字段约束: field_name → type（多泛型 duck 可带类型前缀）
    pub fields: Vec<DuckField>,
}

/// Duck 字段约束 — 结构匹配的最小单元之一
#[derive(Debug, Clone, PartialEq)]
pub struct DuckField {
    /// 所属类型前缀（多泛型关系 duck），None 表示无前缀
    pub owner: Option<String>,
    pub name: String,
    pub ty: IrType,
}

/// Duck 方法签名
#[derive(Debug, Clone, PartialEq)]
pub struct DuckMethod {
    /// 所属类型前缀（多泛型关系 duck，如 `def T.map`），None 表示无前缀
    pub owner: Option<String>,
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: IrType,
    /// 参数数量约束: range(min, max)
    pub param_range: Option<(usize, usize)>,
}

// ══════════════════════════════════════════════════════════════
// Stmt — 语句节点
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        name: String,
        ty: IrType,
        value: Expr,
        is_mut: bool,
    },
    Assign {
        target: Expr, // 可赋值左值（Var / FieldAccess / IndexGet）
        value: Expr,
    },
    Return {
        value: Option<Expr>,
    },
    ExprStmt {
        expr: Expr,
    },
    If {
        cond: Expr,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    For {
        var: String,
        iter: Expr,
        guard: Option<Expr>,
        body: Block,
    },
    While {
        cond: Expr,
        guard: Option<Expr>,
        body: Block,
    },
    WhileLet {
        pattern: Pattern,
        expr: Expr,
        guard: Option<Expr>,
        body: Block,
    },
    Match {
        scrutinee: Expr,
        arms: Vec<MatchArm>,
    },
    Raise {
        value: Expr,
    },
    Assert {
        cond: Expr,
        message: Option<Expr>,
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
        value: Option<Expr>,
    },
    Continue,
    BlockLabel {
        label: String,
        body: Block,
    },
    /// checker 块 → IR 压缩为 fn NAME(ps: &mut __Params)
    CheckerBlock {
        label: String,
        ps_name: Option<String>,
        default_checker: Option<String>,
        body: Block,
    },
    Defer {
        body: Block,
    },
    /// try/catch/else/finally 错误捕获
    TryCatch {
        body: Block,
        catches: Vec<(Option<Pattern>, Block)>,
        else_body: Option<Block>,
        finally_body: Option<Block>,
    },
    /// 裸 Block（含构建块块体）
    Block {
        stmts: Vec<Stmt>,
    },
    /// pass 占位符，无操作
    Pass,
    /// 局部类型别名（仅文档/注释，代码生成时提升到模块级）
    TypeAlias {
        name: String,
        ty: IrType,
    },
}

/// 代码块
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub ty: IrType, // 块的结果类型（最后一条语句的表达式类型，或 Unit）
}

// ══════════════════════════════════════════════════════════════
// Expr — 表达式节点（强类型，携带 IrType + Span）
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: IrType,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, ty: IrType, span: Span) -> Self {
        Expr { kind, ty, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// 字面量
    Lit(LitKind),

    /// 变量引用
    Var(String),

    /// 普通调用 f(args)
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        /// 泛型类型参数（如 foo<T>(args) 中的 T）
        type_args: Vec<String>,
    },

    /// 方法调用 x.method(args)
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },

    /// 字段访问 x.field
    FieldAccess {
        base: Box<Expr>,
        field: String,
    },

    /// 下标读取 base[key]（来自 ^: 脱糖 / []）
    IndexGet {
        base: Box<Expr>,
        key: Box<Expr>,
    },

    /// 下标赋值（__setitem__）
    IndexSet {
        base: Box<Expr>,
        key: Box<Expr>,
        value: Box<Expr>,
    },

    /// 二元运算
    BinOp {
        op: BinOpKind,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    /// 一元运算
    UnOp {
        op: UnOpKind,
        operand: Box<Expr>,
    },

    /// 表达式型 if（三元 a if cond else b）
    IfExpr {
        cond: Box<Expr>,
        then: Box<Expr>,
        els: Box<Expr>,
    },

    /// 匿名函数 |a, b| a + b
    Lambda {
        params: Vec<Param>,
        body: Box<Expr>,
        is_move: bool,
    },

    /// 结构体构造 Adder(base: 10)
    StructCtor {
        name: String,
        fields: Vec<(String, Expr)>,
    },

    /// 枚举构造 Some(x) / Ok(v)
    EnumCtor {
        enum_name: String,
        variant: String,
        args: Vec<Expr>,
    },

    /// 生成器 *: yield e 脱糖
    GenExpr {
        yield_of: Box<Expr>,
    },

    /// 类型转换（隐式 .into() / 显式 as）
    Cast {
        expr: Box<Expr>,
        target: IrType,
    },

    /// 魔法方法调用（__iter__ / __next__ / __str__ / __eq__ 等）
    MagicCall {
        kind: MagicKind,
        args: Vec<Expr>,
    },

    /// 代码块作为表达式
    BlockExpr {
        block: Block,
    },

    /// 元组字面量
    TupleLit(Vec<Expr>),
    Tuple(Vec<Expr>),

    /// List 字面量
    ListLit(Vec<Expr>),

    /// List 字面量 (cython 别名)
    List(Vec<Expr>),

    /// Dict 字面量 (cython)
    Dict(Vec<(Expr, Expr)>),

    /// Range 表达式 (cython)
    Range {
        start: Option<Box<Expr>>,
        end: Box<Expr>,
        inclusive: bool,
    },

    /// 管道调用 x |> f(args)
    Pipe {
        receiver: Box<Expr>,
        func: String,
        args: Vec<Expr>,
    },

    /// 括号分组 (expr) — 保留优先级语义
    Paren(Box<Expr>),

    /// 隐式类型转换: source: S → target_ty: T
    /// 优先级: __implicit_from__ → __implicit_to__ → __default__
    ImplicitConvert {
        source: Box<Expr>,
        target_ty: IrType,
    },
}

// ── 辅助类型 ──

#[derive(Debug, Clone, PartialEq)]
pub enum LitKind {
    Int(i64),
    F64(f64),
    Str(String),
    /// f-string 字面量（保留原始内容，含 {expr} 插值标记）
    FStr(String),
    Bool(bool),
    Unit,
    None_,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    Xor,
    Shl,
    Shr,
    In,
    NotIn,
}

impl BinOpKind {
    /// 是否为比较运算符（Lt/Gt/Le/Ge/Eq/Neq）
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            BinOpKind::Lt
                | BinOpKind::Gt
                | BinOpKind::Le
                | BinOpKind::Ge
                | BinOpKind::Eq
                | BinOpKind::Neq
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOpKind {
    Neg,
    Not,
    Ref,
    MutRef,
    Deref,
}

/// 魔法方法种类
#[derive(Debug, Clone, PartialEq)]
pub enum MagicKind {
    GetItem, // __getitem__
    SetItem, // __setitem__
    Call,    // __call__
    Iter,    // __iter__ (→ into_iter)
    Next,    // __next__
    Display, // __str__
    Eq,      // __eq__
    Cmp,     // __cmp__
    Drop,    // __drop__
    Rev,     // __rev__
    Len,     // __len__
    Add,
    Sub,
    Mul, // 算术魔法
    Neg,
    Not_,            // 一元魔法
    IntoIter,        // __into_iter__
    SizeHint,        // __size_hint__
    IterStrategy,    // __iter_strategy__
    UnpackBuildCall, // ~: 构建块参数解包
}

// ══════════════════════════════════════════════════════════════
// MatchArm — match 分支节点
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Block,
}

// ══════════════════════════════════════════════════════════════
// Pattern — 模式匹配节点
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard,
    Ident(String),
    Lit(LitKind),
    Tuple(Vec<Pattern>),
    List(Vec<Pattern>),
    Range {
        start: i64,
        end: i64,
        inclusive: bool,
    },
    Struct {
        name: String,
        fields: Vec<(String, Pattern)>,
    },
    Enum {
        enum_name: String,
        variant: String,
        args: Vec<Pattern>,
    },
}

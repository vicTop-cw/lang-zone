// Lang-Zone 编译器 — ir/node.rs
// LZIR-H 节点定义：Item, Stmt, Expr, Pattern 及辅助类型
//
// 形态：强类型树 / ANF 风格。每个 Expr 携带 IrType 与 Span。
//
// 序列化：所有节点在 `infer` feature 下派生 serde Serialize/Deserialize，
// 支持 JSON / bincode 两种缓存格式（见 ir/mod.rs 的 to_json / to_bincode）。

use super::types::IrType;

// ── 源码位置 ──

/// 源码区间
///
/// 携带文件路径（`file`）与行列（`line`/`col`），支持从 IR 节点回溯源码。
/// `file == None` 表示位置未知（宏展开/合成节点），`line == 0` 表示 unknown。
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
    /// 源文件路径（None = 未知/合成节点）
    pub file: Option<String>,
}

impl Span {
    pub fn new(line: usize, col: usize) -> Self {
        Span {
            start: 0,
            end: 0,
            line,
            col,
            file: None,
        }
    }
    /// 带文件路径的 span（行/列未知时传 0）
    pub fn with_file(file: impl Into<String>, line: usize, col: usize) -> Self {
        Span {
            start: 0,
            end: 0,
            line,
            col,
            file: Some(file.into()),
        }
    }
    pub fn unknown() -> Self {
        Span {
            start: 0,
            end: 0,
            line: 0,
            col: 0,
            file: None,
        }
    }
    /// 带文件路径的 unknown span（宏展开/合成节点，但已知来源文件）
    pub fn unknown_with_file(file: impl Into<String>) -> Self {
        Span {
            start: 0,
            end: 0,
            line: 0,
            col: 0,
            file: Some(file.into()),
        }
    }
    /// 是否为 unknown（无行列信息）
    pub fn is_unknown(&self) -> bool {
        self.line == 0 && self.col == 0
    }
    /// 是否携带文件路径
    pub fn has_file(&self) -> bool {
        self.file.is_some()
    }
    /// 将文件路径注入本 span（若尚未设置）
    pub fn with_file_if_missing(&mut self, file: &str) {
        if self.file.is_none() {
            self.file = Some(file.to_string());
        }
    }
}

// ── 泛型参数 ──

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<IrType>,
    pub default: Option<IrType>,
}

// ── 魔法属性 ──

// ── 模块顶层指令 ──

/// 后端语言
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct Intrinsic {
    pub kind: IntrinsicKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub enum IntrinsicKind {
    Memoize,
    Parallel,
    Curry,
    Overload,
    Derive,
    TailCall,
    Export(Vec<String>), // @export(Rust), @export(Python)
    Extern(Vec<String>), // @extern(Rust), @extern(Python) 外部声明（L1 机制）
    Init,
}

// ── 函数签名（用于 Trait 声明） ──

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct FnSig {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub params: Vec<IrType>,
    /// trait 方法参数名（默认方法体用真实参数名，抽象声明用 _pN）
    pub params_names: Vec<String>,
    /// trait 方法 where 约束（`try_from ... where Self: Sized` 的 Self: Sized）
    pub where_clause: Vec<(String, Vec<IrType>)>,
    pub ret: IrType,
    /// trait 默认方法体（Some = 带默认实现，None = 抽象声明）
    pub body: Option<Block>,
}

// ── 参数 ──

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct Field {
    pub name: String,
    pub ty: IrType,
}

// ── 枚举变体 ──

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct Variant {
    pub name: String,
    /// 变体字段：名称 + 类型（空名称 = 位置/元组字段，非空 = 命名字段）
    pub fields: Vec<Field>,
}

// ══════════════════════════════════════════════════════════════
// Item — 顶层定义项
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
        /// 捕获的外层函数局部变量（block 闭包语义，规范 05b-block命名块.md §三）：
        /// checker 块体引用的 main 局部变量（out/depth/result 等）需作为
        /// fn 的 &mut 参数传入，否则提升为模块级 fn 后 E0425（block_demo 等）
        captured: Vec<(String, IrType)>,
    },
    /// duck 类型约束 → 编译为 Rust trait
    DuckDef(DuckDef),
}

/// 函数定义
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
    /// 额外 where 约束（type_param → bounds）：引用 impl 级泛型的 where 子句
    /// （如 `impl<K,V> Dict<K,V>` 方法 `where K: Eq + Hash`，K 不在方法泛型中），
    /// builder 合并不到方法泛型上，需原样输出到方法签名（codegen 用）
    pub where_clause: Vec<(String, Vec<IrType>)>,
    pub span: Span,
}

/// 结构体定义
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct EnumDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub variants: Vec<Variant>,
    pub methods: Vec<FnDef>,
    pub span: Span,
}

/// Trait 定义
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct TraitDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub supertraits: Vec<IrType>,
    pub methods: Vec<FnSig>,
    /// 关联类型声明（§五 `type Item`）→ Rust trait 关联类型
    pub assoc_types: Vec<String>,
}

/// Impl 定义
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct ImplDef {
    pub trait_: Option<IrType>, // None = inherent impl
    pub for_type: IrType,
    pub generics: Vec<GenericParam>,
    pub methods: Vec<FnDef>,
    /// 关联类型绑定（§五 `type Item = T`）→ `type Item = ...;`
    pub assoc_type_bindings: Vec<(String, IrType)>,
    /// impl 级 where 约束（`impl<I: Iterator> Iterator for Peekable<I> where
    /// I::Item: Clone` 的关联类型约束，type_param 含点号）
    pub where_clause: Vec<(String, Vec<IrType>)>,
}

/// Use 语句（仅记录依赖路径）
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct UseStmt {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub items: Vec<String>,
    pub is_from: bool,
}

/// 常量定义
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct ConstDef {
    pub name: String,
    pub ty: IrType,
    pub value: Expr,
}

/// 类型别名定义
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeAliasDef {
    pub name: String,
    pub generics: Vec<String>,
    pub ty: IrType,
}

/// 测试定义
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct TestDef {
    pub name: String,
    pub body: Block,
}

/// Duck 类型约束定义 — 编译期结构匹配，零开销
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct DuckDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    /// 关联类型约束: `type I.Item`（§2.3）
    pub assoc_types: Vec<DuckAssocType>,
    /// satisfies 约束行（§11.4①）：要求目标类型同时满足另一 duck
    pub satisfies: Vec<String>,
    /// sealed 闭合约束（§11.4②）：目标类型不得有额外成员
    pub sealed: bool,
    /// 正则方法匹配约束（§8.4）：`match /pattern/ at_least(N)`
    pub match_rules: Vec<DuckMatchRule>,
    /// 命名参数约束（§8.2.2）：`require(...)` / `optional(...)` 独立行
    pub param_reqs: Vec<DuckParamReq>,
    /// 方法签名列表
    pub methods: Vec<DuckMethod>,
    /// 字段约束: field_name → type（多泛型 duck 可带类型前缀）
    pub fields: Vec<DuckField>,
}

/// Duck 正则匹配约束行（§8.4）：`match /pattern/ at_least(N)`
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct DuckMatchRule {
    pub pattern: String,
    /// (lo, hi) 数量约束：at_least→(N, MAX)、at_most→(0, N)、exact→(N, N)
    pub range: (usize, usize),
}

/// Duck 命名参数约束行（§8.2.2）：`require(name: str, version: int)` /
/// `optional(timeout: int)`
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct DuckParamReq {
    /// true = require（必需命名参数）；false = optional（可选命名参数）
    pub is_required: bool,
    /// 参数名列表
    pub names: Vec<String>,
}

/// Duck 关联类型约束 — `type I.Item`（§2.3）
/// owner 为所属类型前缀（如 I），None 表示当前类型自身
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct DuckAssocType {
    pub owner: Option<String>,
    pub name: String,
}

/// Duck 字段约束 — 结构匹配的最小单元之一
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct DuckField {
    /// 所属类型前缀（多泛型关系 duck），None 表示无前缀
    pub owner: Option<String>,
    pub name: String,
    pub ty: IrType,
    /// 字段关系约束（§2.2）：`A.id == B.id` / `A.name: B.name`，
    /// 要求本字段类型等于 (rel_owner, rel_name) 字段的类型。None = 无关系。
    pub rel: Option<(String, String)>,
}

/// Duck 方法签名
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct DuckMethod {
    /// 所属类型前缀（多泛型关系 duck，如 `def T.map`），None 表示无前缀
    pub owner: Option<String>,
    pub name: String,
    /// 正则模式方法名（§8.4）：`def /get_\w+/ (ref self) -> int`
    pub name_pattern: Option<String>,
    pub params: Vec<Param>,
    pub ret_ty: IrType,
    /// 参数数量约束: range(min, max)
    pub param_range: Option<(usize, usize)>,
    /// default 修饰（§11.4③）：该成员可选，目标类型可不实现
    pub is_default: bool,
}

// ══════════════════════════════════════════════════════════════
// Stmt — 语句节点
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub enum Stmt {
    Let {
        name: String,
        ty: IrType,
        value: Expr,
        is_mut: bool,
        /// 引用绑定（ref r = x / let ref r = x）：codegen 生成 `let r = &mut x;` / `&x`
        is_ref: bool,
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
        else_body: Option<Block>,
    },
    While {
        cond: Expr,
        guard: Option<Expr>,
        body: Block,
        else_body: Option<Block>,
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub ty: IrType, // 块的结果类型（最后一条语句的表达式类型，或 Unit）
    /// 块级源码位置（含文件路径）
    pub span: Span,
}

impl Default for Block {
    fn default() -> Self {
        Block {
            stmts: vec![],
            ty: IrType::Unit,
            span: Span::unknown(),
        }
    }
}

// ══════════════════════════════════════════════════════════════
// Expr — 表达式节点（强类型，携带 IrType + Span）
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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

    /// 赋值表达式（`total = total + x`，闭包体/表达式上下文中的纯赋值 `=`）：
    /// Rust 渲染为 `target = value`（赋值表达式返回 ()）。
    /// 注意：不能用 BinOp(Eq) 表达——codegen 会把 Eq 渲染为 `==` 比较（E0308）
    AssignExpr {
        target: Box<Expr>,
        value: Box<Expr>,
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

    /// 生成器构建块 func *: { yield ... } — 收集 yield 参数包，逐包调用 callee（无 callee 时返回参数包迭代器）
    GenBuild {
        callee: Option<Box<Expr>>,
        block: Block,
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

    /// 管道调用 x |> f(args) — callee 为右侧完整表达式（函数 Var / 闭包 Lambda /
    /// 方法 MethodCall / 构造 Var / __call__ 实例 Var），args 为显式实参
    Pipe {
        receiver: Box<Expr>,
        callee: Box<Expr>,
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub enum UnOpKind {
    Neg,
    Not,
    Ref,
    MutRef,
    Deref,
}

/// 魔法方法种类
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Block,
}

// ══════════════════════════════════════════════════════════════
// Pattern — 模式匹配节点
// ══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub enum Pattern {
    Wildcard,
    Ident(String),
    /// `ref mut name` 模式绑定：c 绑定为 &mut 引用（case Some(ref mut c)）
    RefMutIdent(String),
    Lit(LitKind),
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

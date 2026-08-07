// Lang-Zong 编译器 — ast/decl.rs
// 声明类 AST 节点：Module, Function, StructDef, TraitDef, ImplDef 等

use super::expr::Expr;
use super::stmt::Stmt;
use crate::types::Type;

#[derive(Debug, Clone)]
pub struct Module {
    pub name: Option<String>, // 模块名（来自 __name__ 或文件路径）
    pub imports: Vec<ImportStmt>,
    pub functions: Vec<Function>,
    pub structs: Vec<StructDef>,
    pub traits: Vec<TraitDef>,
    pub impls: Vec<ImplDef>,
    pub consts: Vec<ConstDef>,
    pub type_aliases: Vec<TypeAliasDef>,
    pub tests: Vec<Stmt>,
    pub top_level_builds: Vec<(String, Vec<Stmt>)>, // 顶层构建块 (name, body)
    /// 顶层 block / checker 块语句
    pub top_stmts: Vec<Stmt>,
    /// duck 类型约束定义
    pub duck_defs: Vec<DuckDef>,
    /// 独立 magic 块: magic __str__: def __str__(self: MyStruct) -> str = ...
    pub magic_blocks: Vec<MagicDef>,
}

/// 独立 magic 方法块定义
#[derive(Debug, Clone)]
pub struct MagicDef {
    pub method_name: String, // __str__
    pub function: Function,  // def __str__(self: MyStruct) -> str
}

#[derive(Debug, Clone)]
pub struct TypeAliasDef {
    pub name: String,
    pub generics: Vec<String>,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct ImportStmt {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub items: Vec<String>,
    pub is_from: bool,
}

#[derive(Debug, Clone)]
pub struct ConstDef {
    pub name: String,
    pub ty: Option<Type>,
    pub value: Expr,
    pub mutable: bool,
}

/// 可变参数模式：记录 `..` 分隔符在参数列表中的位置
#[derive(Debug, Clone, PartialEq)]
pub enum VariadicMode {
    /// 无 `..` 分隔符
    None,
    /// 单个 `..`：此前 params[0..pos] 为仅位置，此后 params[pos..] 为仅关键字
    Single { dotdot_at: usize },
    /// 两个 `..`：`args` + `kwargs` 模式
    Double { first_at: usize, second_at: usize },
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub raises: Option<Type>,
    pub where_clause: Vec<WhereBound>,
    pub body: Vec<Stmt>,
    pub is_async: bool,
    pub is_abstract: bool,
    pub is_iterator: bool, // iterator 关键字定义的生成器函数
    pub is_magic: bool,    // magic 关键字标记的内建方法
    pub decorators: Vec<Decorator>,
    /// `..` 可变参数模式
    pub variadic: VariadicMode,
    /// checker 参数名（def f[ps: __Params](...) — ps 接收 &mut __Params）
    pub checker_param: Option<String>,
    /// 引用的默认检查站名（def f[cache](...) 或 def f[cache][ps: __Params](...)）
    pub default_checker: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Decorator {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
    pub is_mut: bool,
    pub is_owned: bool,
    pub is_ref: bool,
}

#[derive(Debug, Clone)]
pub struct WhereBound {
    pub type_param: String,
    pub bounds: Vec<Type>,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub generics: Vec<String>,
    pub fields: Vec<Field>,
    pub methods: Vec<Function>,
    pub magic_methods: Vec<Function>,
    pub is_enum: bool,
    pub decorators: Vec<Decorator>,
    pub repr_attr: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct TraitDef {
    pub name: String,
    pub generics: Vec<String>,
    pub methods: Vec<Function>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct ImplDef {
    pub trait_name: Option<String>,
    pub type_name: String,
    pub generics: Vec<String>,
    pub where_clause: Vec<WhereBound>,
    pub methods: Vec<Function>,
}

/// Duck 类型约束定义 — 编译期结构匹配，零运行时开销
/// `duck Name = def method(self) -> Ret ...`
#[derive(Debug, Clone)]
pub struct DuckDef {
    pub name: String,
    pub generics: Vec<String>,
    /// 方法签名列表
    pub methods: Vec<DuckMethod>,
    /// 字段约束: .field_name: Type（多泛型 duck 可带类型前缀 A.x: Type）
    pub fields: Vec<DuckField>,
}

/// Duck 字段约束 — 结构匹配的最小单元之一
/// `.name: str`（无前缀）或 `A.x: f64`（多泛型关系 duck 的类型前缀）
#[derive(Debug, Clone)]
pub struct DuckField {
    /// 所属类型前缀（多泛型关系 duck），None 表示无前缀（单类型 duck）
    pub owner: Option<String>,
    pub name: String,
    pub ty: Type,
}

/// Duck 方法签名 — 结构匹配的最小单元
#[derive(Debug, Clone)]
pub struct DuckMethod {
    /// 所属类型前缀（多泛型关系 duck，如 `def T.map`），None 表示无前缀
    pub owner: Option<String>,
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    /// 参数数量约束: range(min, max)，编译期检查
    pub param_range: Option<(usize, usize)>,
}

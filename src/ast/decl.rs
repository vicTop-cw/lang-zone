// Lang-Zong 编译器 — ast/decl.rs
// 声明类 AST 节点：Module, Function, StructDef, TraitDef, ImplDef 等

use super::expr::Expr;
use super::stmt::Stmt;
use crate::types::Type;

#[derive(Debug, Clone, Default)]
pub struct Module {
    pub name: Option<String>, // 模块名（来自 __name__ 或文件路径）
    /// .lz 源文件路径（06e 模块级魔法属性 __file__/__package__/__path__ 的数据源）
    pub file_path: Option<String>,
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
    /// 是否为宏模块（首行 `#!bin macro` 声明）：宏/template 仅能定义在宏模块，
    /// 模块级魔法属性 __is_macro__ 据此填充（06e-模块级魔法属性.md）
    pub is_macro: bool,
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

/// 可变参数模式：`..` 是变参注入标记（非边界分隔符），最多出现 2 次。
/// 任何 `..` 出现即触发注入：单 `..` 无注解 → 只注入 args（元素 Any）；
/// `..: Tuple<T>` → args-only；`..: Dict<K,V>` → kwargs-only；双 `..` → args + kwargs。
/// （位置/关键字边界由 `/` `*` 安全分隔符负责，与 `..` 互斥，见 03d-可变参数.md §三）
#[derive(Debug, Clone, PartialEq)]
pub enum VariadicMode {
    /// 无 `..` 注入
    None,
    /// 单 `..`（无注解或 `..: Tuple<T>`）：注入 args（位置变长参数）
    ArgsOnly {
        dotdot_at: usize,
        /// `..: Tuple<T>` 的元素类型；None = Any 擦除
        elem_ty: Option<Type>,
        /// 03d §2.3 多类型位置约束：`..: Tuple<T1, T2, ..>` 的完整类型列表
        /// （尾部 `..` 通配解析为 Type::Any 作为哨兵；非多类型时为 []）
        elem_tys: Vec<Type>,
    },
    /// 单 `..: Dict<K,V>`：注入 kwargs（关键字变长参数）
    KwargsOnly {
        dotdot_at: usize,
        /// `..: Dict<K,V>` 的值类型；None = Any 擦除
        value_ty: Option<Type>,
    },
    /// 双 `..`：args + kwargs 双收集
    Both {
        first_at: usize,
        args_elem_ty: Option<Type>,
        second_at: usize,
        kwargs_value_ty: Option<Type>,
    },
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub generics: Vec<String>,
    /// 泛型默认类型（§四 `T = int`）— (type_param → default type)
    pub generic_defaults: Vec<(String, Type)>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub raises: Option<Type>,
    pub where_clause: Vec<WhereBound>,
    pub body: Vec<Stmt>,
    pub is_async: bool,
    pub is_abstract: bool,
    pub is_iterator: bool, // iterator 关键字定义的生成器函数
    pub is_magic: bool,    // magic 关键字标记的内建方法
    /// comptime def 编译期函数：仅在编译期存在，不生成运行时代码
    pub is_comptime: bool,
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
    /// 泛型内联约束（`struct Map<I: Iterator, B>` 的 I: Iterator）
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// 泛型默认类型（§四 `T = int`）
    pub generic_defaults: Vec<(String, Type)>,
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
    /// 泛型默认类型（§四 `T = int`）
    pub generic_defaults: Vec<(String, Type)>,
    /// 父 trait（`trait DoubleEndedIterator: Iterator` 的 : Iterator）
    pub supertraits: Vec<Type>,
    pub methods: Vec<Function>,
    pub fields: Vec<Field>,
    /// trait 内声明的关联类型（§五 `type Item`），impl 时需提供具体类型
    pub assoc_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ImplDef {
    pub trait_name: Option<String>,
    pub type_name: String,
    pub generics: Vec<String>,
    /// 泛型默认类型（§四 `T = int`）
    pub generic_defaults: Vec<(String, Type)>,
    pub where_clause: Vec<WhereBound>,
    pub methods: Vec<Function>,
    /// impl 中的关联类型绑定（§五 `type Item = T`）
    pub assoc_type_bindings: Vec<(String, Type)>,
}

/// Duck 类型约束定义 — 编译期结构匹配，零运行时开销
/// `duck Name = def method(self) -> Ret ...`
#[derive(Debug, Clone)]
pub struct DuckDef {
    pub name: String,
    pub generics: Vec<String>,
    /// 嵌套约束: `duck D<T> where T: Iterable = ...`（§2.4）
    pub where_clause: Vec<WhereBound>,
    /// 关联类型约束: `type I.Item`（§2.3，I 有关联类型 Item）
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
    /// 字段约束: .field_name: Type（多泛型 duck 可带类型前缀 A.x: Type）
    pub fields: Vec<DuckField>,
}

/// Duck 正则匹配约束行（§8.4）：`match /pattern/ at_least(N)`
#[derive(Debug, Clone)]
pub struct DuckMatchRule {
    pub pattern: String,
    /// (lo, hi) 数量约束：at_least→(N, MAX)、at_most→(0, N)、exact→(N, N)
    pub range: (usize, usize),
}

/// Duck 命名参数约束行（§8.2.2）：`require(name: str, version: int)` /
/// `optional(timeout: int)`
#[derive(Debug, Clone)]
pub struct DuckParamReq {
    /// true = require（必需命名参数）；false = optional（可选命名参数）
    pub is_required: bool,
    /// 参数名列表（类型标注已解析，但检查器只需名字）
    pub names: Vec<String>,
}

/// Duck 关联类型约束 — `type I.Item`（§2.3）
/// owner 为所属类型前缀（如 I），None 表示当前类型自身
#[derive(Debug, Clone)]
pub struct DuckAssocType {
    pub owner: Option<String>,
    pub name: String,
}

/// Duck 字段约束 — 结构匹配的最小单元之一
/// `.name: str`（无前缀）或 `A.x: f64`（多泛型关系 duck 的类型前缀）
#[derive(Debug, Clone)]
pub struct DuckField {
    /// 所属类型前缀（多泛型关系 duck），None 表示无前缀（单类型 duck）
    pub owner: Option<String>,
    pub name: String,
    pub ty: Type,
    /// 字段关系约束（§2.2）：`A.id == B.id` / `A.name: B.name`，
    /// 要求本字段类型等于 (rel_owner, rel_name) 字段的类型。None = 无关系。
    pub rel: Option<(String, String)>,
}

/// Duck 方法签名 — 结构匹配的最小单元
#[derive(Debug, Clone)]
pub struct DuckMethod {
    /// 所属类型前缀（多泛型关系 duck，如 `def T.map`），None 表示无前缀
    pub owner: Option<String>,
    pub name: String,
    /// 正则模式方法名（§8.4）：`def /get_\w+/ (ref self) -> int`
    pub name_pattern: Option<String>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    /// 参数数量约束: range(min, max)，编译期检查
    pub param_range: Option<(usize, usize)>,
    /// default 修饰（§11.4③）：该成员可选，目标类型可不实现
    pub is_default: bool,
}

// Lang-Zong 编译器 — ast/decl.rs
// 声明类 AST 节点：Module, Function, StructDef, TraitDef, ImplDef 等

use crate::types::Type;
use super::expr::Expr;
use super::stmt::Stmt;

#[derive(Debug, Clone)]
pub struct Module {
    pub name: Option<String>,         // 模块名（来自 __name__ 或文件路径）
    pub imports: Vec<ImportStmt>,
    pub functions: Vec<Function>,
    pub structs: Vec<StructDef>,
    pub traits: Vec<TraitDef>,
    pub impls: Vec<ImplDef>,
    pub consts: Vec<ConstDef>,
    pub type_aliases: Vec<TypeAliasDef>,
    pub tests: Vec<Stmt>,
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
    pub is_iterator: bool,       // iterator 关键字定义的生成器函数
    pub decorators: Vec<Decorator>,
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

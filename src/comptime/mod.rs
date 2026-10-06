// Lang-Zong 编译器 — comptime 模块
// 编译期求值引擎 + `inspect` 源码内饰库（编译期专用）
//
// 集成位置：codegen 在遇到 Stmt::Comptime / Expr::Comptime / comptime let/const/fn 时
// 调用 ComptimeContext + ComptimeEvaluator 求值，成功则内联字面量（或不产出代码），
// 失败则降级为 compile_error!（编译失败，而非运行时错误）。
//
// inspect 命名空间仅在 comptime 上下文中可用；运行时不可调用、不生成代码。

use crate::ast::*;

use crate::types::Type;

use std::collections::HashMap;

// 大文件拆分（move-only，2026-10-06）：单文件按职责切分为子模块，逻辑零改动
mod eval;
mod frame;
mod literal;

// ═══════════════════════════════════════════════════════════════════
// 编译期值域
// ═══════════════════════════════════════════════════════════════════

/// 编译期求值产生的值。它 **不是** 运行时值，仅存在于编译期。
#[derive(Debug, Clone)]
pub enum ComptimeValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    None,
    /// 同构列表
    List(Vec<ComptimeValue>),
    /// 异构元组
    Tuple(Vec<ComptimeValue>),
    /// 键值映射（对应 dict / kwargs）
    Map(HashMap<String, ComptimeValue>),
    /// 类型值：编译期持有的 `Type`（如 `int`、`List[str]`）
    Type(Type),
    /// inspect 返回的结构化对象
    Inspect(InspectObject),
}



// ═══════════════════════════════════════════════════════════════════
// Inspect 数据结构（对齐 Python 3.12+ inspect / `inspect.Parameter` 等）
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    PositionalOnly,
    PositionalOrKeyword,
    VarPositional,
    KeywordOnly,
    VarKeyword,
}
impl ParameterKind {

    pub fn as_str(&self) -> &'static str {
        match self {
            ParameterKind::PositionalOnly => "POSITIONAL_ONLY",
            ParameterKind::PositionalOrKeyword => "POSITIONAL_OR_KEYWORD",
            ParameterKind::VarPositional => "VAR_POSITIONAL",
            ParameterKind::KeywordOnly => "KEYWORD_ONLY",
            ParameterKind::VarKeyword => "VAR_KEYWORD",
        }
    }}


#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub kind: ParameterKind,
    pub annotation: Option<Type>,
    pub default: Option<Box<ComptimeValue>>,
}


#[derive(Debug, Clone)]
pub struct Signature {
    pub name: Option<String>,
    pub parameters: Vec<Parameter>,
    pub return_annotation: Option<Type>,
}


#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub doc: Option<String>,
    pub functions: Vec<String>,
    pub structs: Vec<String>,
    pub traits: Vec<String>,
    pub consts: Vec<String>,
}


#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_annotation: Option<Type>,
    pub is_comptime: bool,
}


#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub bases: Vec<String>,
    pub methods: Vec<String>,
}


#[derive(Debug, Clone)]
pub struct MroInfo {
    pub name: String,
    pub mro: Vec<String>,
}


#[derive(Debug, Clone)]
pub struct ClassTree {
    pub name: String,
    pub children: Vec<ClassTree>,
}


#[derive(Debug, Clone)]
pub struct Abstracts {
    pub name: String,
    pub abstract_methods: Vec<String>,
}


#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub function: Option<String>,
    pub filename: String,
    pub lineno: Option<i64>,
}


#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub filename: String,
    pub source: String,
    pub first_lineno: i64,
    pub lines: Vec<String>,
}


/// Inspect 结构化对象
#[derive(Debug, Clone)]
pub enum InspectObject {
    Module(ModuleInfo),
    Function(FunctionInfo),
    Class(ClassInfo),
    Signature(Signature),
    Parameter(Parameter),
    Mro(MroInfo),
    ClassTree(ClassTree),
    Abstracts(Abstracts),
    Frame(FrameInfo),
    Source(SourceInfo),
}


// ═══════════════════════════════════════════════════════════════════
// 编译期上下文
// ═══════════════════════════════════════════════════════════════════

/// 编译期求值的最大嵌套深度（Zig 式 runaway 保护）
const MAX_COMPTIME_DEPTH: u32 = 256;


/// 编译期上下文
pub struct ComptimeContext<'a> {
    /// 当前编译模块的完整 AST
    pub module: &'a Module,
    /// 符号表：编译期变量名 → 值
    pub symtab: HashMap<String, ComptimeValue>,
    /// 嵌套深度（每次 eval_expr / eval_stmt +1，超限报错）
    depth: u32,
    /// 可选的源码文本（供 `inspect.getsource` 等使用）
    source: Option<String>,
}



// ═══════════════════════════════════════════════════════════════════
// 编译期求值器
// ═══════════════════════════════════════════════════════════════════

pub struct ComptimeEvaluator;



// 为参数提取添加辅助 trait
trait AsStr {
    fn as_str(&self) -> Option<&str>;
}
impl AsStr for ComptimeValue {

    fn as_str(&self) -> Option<&str> {
        match self {
            ComptimeValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }}

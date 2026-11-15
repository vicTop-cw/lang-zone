//! 类型判定错误 —— `typing` 库的错误类型
//!
//! 与 `hints::TypeError`（推断期统一错误）区分：`TypingError` 描述
//! **已推断完成之后**的符合性判定失败（子类型不成立 / trait 方法缺失 / 签名不符）。

use crate::types::def::Type;

/// 类型符合性判定错误
#[derive(Debug, Clone, PartialEq)]
pub enum TypingError {
    /// `sub` 不符合（不能用于）`sup`：子类型 / 包含关系不成立
    Conformance(Type, Type),

    /// 泛型 / 元组 / 函数实参个数不匹配（(提供的, 要求的)）
    Arity(usize, usize),

    /// 类型未实现某 trait 要求的某个方法：(trait 名, 方法名)
    MissingMethod(String, String),

    /// 方法签名不符合 trait 要求：(trait 名, 方法名, 实际签名, 要求签名)
    ///
    /// 签名以 `Type::Fn { params, ret }` 形式携带，便于调试展示。
    SignatureMismatch(String, String, Type, Type),

    /// 推断孔在 typing 阶段仍未消解（应在 zonk 之后才进入 typing）
    UnresolvedVar(Type),

    /// 引用了未知 trait（未在 `TraitEnv` 注册）
    UnknownTrait(String),
}

impl std::fmt::Display for TypingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypingError::Conformance(sub, sup) =>
                write!(f, "type `{}` does not conform to `{}`", sub, sup),
            TypingError::Arity(provided, required) =>
                write!(f, "arity mismatch: trait requires {} type argument(s), found {}", required, provided),
            TypingError::MissingMethod(trait_name, method) =>
                write!(f, "type does not implement trait `{}`: missing method `{}`", trait_name, method),
            TypingError::SignatureMismatch(trait_name, method, provided, required) =>
                write!(f, "method `{}` of trait `{}` has signature `{}` but required `{}`",
                    method, trait_name, provided, required),
            TypingError::UnresolvedVar(t) =>
                write!(f, "unresolved inference variable in typing phase: `{}` (zonk first)", t),
            TypingError::UnknownTrait(name) =>
                write!(f, "unknown trait: `{}`", name),
        }
    }
}

impl std::error::Error for TypingError {}

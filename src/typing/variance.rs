//! 变型（variance）计算
//!
//! 给定一个类型 `ty` 与其中出现的某类型参数 `param`，计算 `param` 在 `ty` 中的位置变型。
//! 这是泛型安全的核心：例如 `Mut<T>` 若把 `T` 出现在可变字段中则为不变，
//! `Vec<T>` 中 `T` 协变，`fn(T) -> T` 中 `T` 因参数逆变 + 返回协变而呈现**双变（bivariant→不变）**。
//!
//! 与 [`relate::conforms`] 配合：推断层可在「把 `F<Sub>` 赋给 `F<Sup>`」时，
//! 先用本模块确认 `F` 在该位置确为协变，否则拒绝（避免不变/逆变成员的非法赋值）。

use crate::types::def::Type;

/// 类型参数在某位置上的变型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    /// 协变：Sub <: Sup ⇒ F<Sub> <: F<Sup>
    Covariant,
    /// 逆变：Sub <: Sup ⇒ F<Sup> <: F<Sub>
    Contravariant,
    /// 不变：仅当 Sub == Sup 时 F<Sub> == F<Sup>
    Invariant,
    /// 该位置不出现此参数（无关）
    Irrelevant,
}

impl Variance {
    /// 组合变型：`self` 为外层位置贡献，`inner` 为该位置内部已算出的变型。
    ///
    /// 规则：协变位置透传内层；逆变位置翻转内层；不变位置压成不变；无关位置取内层。
    pub fn compose(self, inner: Variance) -> Variance {
        match self {
            Variance::Irrelevant => inner,
            Variance::Covariant => inner,
            Variance::Contravariant => match inner {
                Variance::Covariant => Variance::Contravariant,
                Variance::Contravariant => Variance::Covariant,
                Variance::Invariant => Variance::Invariant,
                Variance::Irrelevant => Variance::Irrelevant,
            },
            Variance::Invariant => match inner {
                Variance::Irrelevant => Variance::Irrelevant,
                _ => Variance::Invariant,
            },
        }
    }
}

/// 计算 `param` 在 `ty` 中的变型。
///
/// 默认协变；函数参数位置逆变；`&mut T` 整体不变；同参数出现于多处时取最严格（不变）。
pub fn variance_of(ty: &Type, param: &Type) -> Variance {
    walk(ty, param)
}

fn walk(ty: &Type, param: &Type) -> Variance {
    match ty {
        // 基础 / 命名 / 自类型：命中参数则协变，否则无关
        Type::Var(_) | Type::Named(_) | Type::PathDependent { .. } | Type::Wildcard
        | Type::Int | Type::F64 | Type::Float | Type::Str
        | Type::Bool | Type::None_ | Type::Never | Type::Any | Type::Unit | Type::Self_ => {
            if ty == param { Variance::Covariant } else { Variance::Irrelevant }
        }

        // 协变透传容器：`Option` / `T?` / `&T`
        Type::Option(inner) | Type::Optional(inner) | Type::Ref(inner) => walk(inner, param),

        // 可变引用整体不变：内部任意变型都被 &mut 压成不变
        Type::MutRef(_) => Variance::Invariant,

        // 多位置：合并（任一不变 → 不变；协变+逆变 → 不变）
        Type::Result { ok, err } => combine2(walk(ok, param), walk(err, param)),
        Type::Generic { args, .. } | Type::Tuple(args) =>
            args.iter().map(|a| walk(a, param)).fold(Variance::Irrelevant, combine2),

        // 函数：参数逆变，返回协变
        Type::Fn { params, ret } => {
            let param_var = params
                .iter()
                .map(|p| walk(p, param).compose(Variance::Contravariant))
                .fold(Variance::Irrelevant, combine2);
            let ret_var = walk(ret, param);
            combine2(param_var, ret_var)
        }

        Type::Simd { elem, .. } => walk(elem, param),

        // Record 字段与 Tuple 一致，按协变组合
        Type::Record(fields) =>
            fields.iter().map(|(_, t)| walk(t, param)).fold(Variance::Irrelevant, combine2),

        // Future / Futures 与容器一致，协变透传
        Type::Future(inner) => walk(inner, param),
        Type::Futures(types) =>
            types.iter().map(|t| walk(t, param)).fold(Variance::Irrelevant, combine2),

        // 联合类型：各成员均为协变位置
        Type::Union(types) =>
            types.iter().map(|t| walk(t, param)).fold(Variance::Irrelevant, combine2),

        // 交集类型：与联合类型一致，各成员均为协变位置
        Type::Intersection(types) =>
            types.iter().map(|t| walk(t, param)).fold(Variance::Irrelevant, combine2),
        Type::Constructor { .. } => Variance::Irrelevant,
        Type::Apply { constructor, args } => {
            let c = walk(constructor, param);
            args.iter().map(|t| walk(t, param)).fold(c, combine2)
        }
    }
}

/// 合并同一类型内多处出现的变型：
/// 任一不变 → 不变；协变 + 逆变 → 不变；同类合并；无关透传。
fn combine2(a: Variance, b: Variance) -> Variance {
    match (a, b) {
        (Variance::Invariant, _) | (_, Variance::Invariant) => Variance::Invariant,
        (Variance::Irrelevant, x) | (x, Variance::Irrelevant) => x,
        (Variance::Covariant, Variance::Covariant) => Variance::Covariant,
        (Variance::Contravariant, Variance::Contravariant) => Variance::Contravariant,
        (Variance::Covariant, Variance::Contravariant)
        | (Variance::Contravariant, Variance::Covariant) => Variance::Invariant,
    }
}

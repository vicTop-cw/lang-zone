//! Trait / 方法满足性 —— `satisfies`
//!
//! 判断某类型是否满足某 trait 要求：
//! 1. **方法存在性**：类型必须暴露 trait 要求的全部方法名。
//! 2. **签名符合性**（可选）：若方法提供方给出签名，则校验
//!    - 返回类型：实际返回 `<:` 要求返回（协变）
//!    - 参数类型：要求参数 `<:` 实际参数（逆变，等价于「实现方法接受面不窄于 trait 承诺」）
//!
//! 与 AST 解耦：本模块不认识 `ImplDef` / 魔法方法的具体来源，而是通过
//! [`MethodProvider`] trait 由上层（parser / codegen）注入真实方法查询。
//! [`MemProvider`] 提供内存实现，用于测试与轻量运行时查询。

use std::collections::HashMap;

use crate::hints::InferCtx;
use crate::types::def::Type;

use super::errors::TypingError;
use super::relate::conforms;

/// 一个被要求的方法签名（不含 self）
#[derive(Debug, Clone)]
pub struct MethodReq {
    /// 方法名（magic 名如 `__add__`，或 trait 方法名如 `add`）
    pub name: String,
    /// 参数类型列表（不含 self）
    pub params: Vec<Type>,
    /// 返回类型
    pub ret: Type,
}

impl MethodReq {
    /// 仅按方法名构造要求（不校验签名，只检查存在性）
    pub fn new(name: impl Into<String>) -> Self {
        MethodReq {
            name: name.into(),
            params: Vec::new(),
            ret: Type::Any,
        }
    }

    /// 附带签名要求（params / ret 均用 `Type::Any` 表示「不约束」）
    pub fn with_sig(mut self, params: Vec<Type>, ret: Type) -> Self {
        self.params = params;
        self.ret = ret;
        self
    }
}

/// 一个 trait 要求：名称 + 一组必需方法
#[derive(Debug, Clone)]
pub struct TraitReq {
    pub name: String,
    pub methods: Vec<MethodReq>,
}

impl TraitReq {
    pub fn new(name: impl Into<String>) -> Self {
        TraitReq {
            name: name.into(),
            methods: Vec::new(),
        }
    }

    /// 追加一个必需方法
    pub fn require(mut self, m: MethodReq) -> Self {
        self.methods.push(m);
        self
    }
}

/// trait 注册表：trait 名 → 要求
#[derive(Debug, Clone, Default)]
pub struct TraitEnv {
    traits: HashMap<String, TraitReq>,
}

impl TraitEnv {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册 / 覆盖一个 trait 要求
    pub fn register(&mut self, t: TraitReq) {
        self.traits.insert(t.name.clone(), t);
    }

    /// 查询 trait 要求
    pub fn get(&self, name: &str) -> Option<&TraitReq> {
        self.traits.get(name)
    }

    /// 是否已注册
    pub fn contains(&self, name: &str) -> bool {
        self.traits.contains_key(name)
    }
}

/// 类型方法提供方：types 库不耦合 AST，由上层实现以查询真实方法。
pub trait MethodProvider {
    /// 返回类型 `ty` 暴露的全部方法名（magic 或 inherent）
    fn methods_of(&self, ty: &Type) -> Vec<String>;

    /// 可选：返回某方法的签名 `(params, ret)`；提供后 `satisfies` 会做签名符合性检查
    fn method_sig(&self, _ty: &Type, _name: &str) -> Option<(Vec<Type>, Type)> {
        None
    }
}

/// 内存实现：用于测试与运行时轻量查询
#[derive(Debug, Clone, Default)]
pub struct MemProvider {
    /// 类型名 → (方法名 → (参数, 返回))
    map: HashMap<String, HashMap<String, (Vec<Type>, Type)>>,
}

impl MemProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// 为命名类型 `ty_name` 登记一个方法（含签名）
    pub fn add(&mut self, ty_name: &str, method: &str, params: Vec<Type>, ret: Type) {
        self.map
            .entry(ty_name.to_string())
            .or_default()
            .insert(method.to_string(), (params, ret));
    }
}

impl MethodProvider for MemProvider {
    fn methods_of(&self, ty: &Type) -> Vec<String> {
        match ty {
            Type::Named(n) => self
                .map
                .get(n)
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn method_sig(&self, ty: &Type, name: &str) -> Option<(Vec<Type>, Type)> {
        match ty {
            Type::Named(n) => self.map.get(n).and_then(|m| m.get(name).cloned()),
            _ => None,
        }
    }
}

/// 判断 `ty` 是否满足名为 `trait_name` 的 trait 要求。
///
/// - 缺少方法 → [`TypingError::MissingMethod`]
/// - 签名不符（若 provider 提供签名）→ [`TypingError::SignatureMismatch`]
/// - 未知 trait → [`TypingError::UnknownTrait`]
pub fn satisfies(
    env: &TraitEnv,
    provider: &dyn MethodProvider,
    ty: &Type,
    trait_name: &str,
) -> Result<(), TypingError> {
    let req = env
        .get(trait_name)
        .ok_or_else(|| TypingError::UnknownTrait(trait_name.to_string()))?;

    let provided = provider.methods_of(ty);
    let ctx = InferCtx::new(); // 满足性判定在 zonk 之后，无推断变量

    for m in &req.methods {
        if !provided.iter().any(|n| n == &m.name) {
            return Err(TypingError::MissingMethod(req.name.clone(), m.name.clone()));
        }

        // 若 provider 能提供签名，则检查方法签名是否符合
        if let Some((p_params, p_ret)) = provider.method_sig(ty, &m.name) {
            // 返回类型：实际返回 <: 要求返回（协变）
            conforms(&ctx, &p_ret, &m.ret).map_err(|_| {
                TypingError::SignatureMismatch(
                    req.name.clone(),
                    m.name.clone(),
                    p_ret,
                    m.ret.clone(),
                )
            })?;

            // 参数类型：仅当要求显式给出参数签名时才校验（空 params 表示不约束参数，
            // 此时只检查「方法存在性」，已由上方完成）。要求参数 <: 实际参数（逆变）。
            if !m.params.is_empty() {
                if p_params.len() != m.params.len() {
                    return Err(TypingError::Arity(p_params.len(), m.params.len()));
                }
                for (required_p, provided_p) in m.params.iter().zip(p_params.iter()) {
                conforms(&ctx, required_p, provided_p).map_err(|_| {
                    TypingError::SignatureMismatch(
                        req.name.clone(),
                        m.name.clone(),
                        provided_p.clone(),
                        required_p.clone(),
                    )
                })?;
            }
        }
    }
    }

    Ok(())
}

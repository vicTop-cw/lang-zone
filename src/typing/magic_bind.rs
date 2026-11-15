//! `magic` 引擎 → `typing` trait 要求的桥接
//!
//! lz 的「接口」目前由魔法方法体系承载：`__add__` → `std::ops::Add`、
//! `__iter__` → `std::iter::IntoIterator` 等（`magic/engine.rs` 的 `MagicEntry.trait_path`）。
//! 本模块把这些映射聚合为 [`typing::trait`] 的 `TraitReq`，使「某类型是否实现某 trait」
//! 可直接复用 `satisfies` 判定。

use std::collections::HashMap;

use crate::magic::engine::MagicEngine;
use crate::types::def::Type;

use super::errors::TypingError;
use super::traits::{satisfies, MethodProvider, TraitEnv, TraitReq, MethodReq};

/// 由单个魔法方法名生成一个 `TraitReq`：
/// 以 `MagicEntry.trait_path` 为 trait 名，魔法方法名为一个必需方法。
pub fn trait_req_from_magic(engine: &MagicEngine, magic: &str) -> Option<TraitReq> {
    let entries = engine.resolve(magic)?;
    let entry = entries.first()?;
    Some(TraitReq::new(entry.trait_path.clone()).require(MethodReq::new(magic)))
}

/// 将 `MagicEngine` 中所有映射注册为 `TraitEnv` 中的 trait 要求。
///
/// 同名 `trait_path` 的多个魔法方法（如 `__lt__`/`__le__`/`__gt__`/`__ge__` 都映射到
/// `std::cmp::PartialOrd`）会被聚合为该 trait 的一组必需方法。
pub fn register_magic_traits(env: &mut TraitEnv, engine: &MagicEngine) {
    let mut by_trait: HashMap<String, Vec<String>> = HashMap::new();
    for (magic, entries) in engine.iter_mappings() {
        if let Some(e) = entries.first() {
            by_trait
                .entry(e.trait_path.to_string())
                .or_default()
                .push(magic.clone());
        }
    }
    for (trait_path, magics) in by_trait {
        let mut req = TraitReq::new(trait_path);
        for m in magics {
            req = req.require(MethodReq::new(m));
        }
        env.register(req);
    }
}

/// 便捷：检查 `ty`（经 `provider`）是否满足某魔法方法对应的 trait。
pub fn satisfies_magic(
    env: &TraitEnv,
    provider: &dyn MethodProvider,
    ty: &Type,
    engine: &MagicEngine,
    magic: &str,
) -> Result<(), TypingError> {
    let entries = engine
        .resolve(magic)
        .ok_or_else(|| TypingError::UnknownTrait(magic.to_string()))?;
    let trait_path = entries
        .first()
        .map(|e| e.trait_path.to_string())
        .unwrap_or_else(|| magic.to_string());
    satisfies(env, provider, ty, &trait_path)
}

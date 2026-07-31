//! Trait Bound 满足性检查
//!
//! 判断具体类型是否满足某 trait 要求：
//!
//! 1. **内置类型表**：Int/Str/Bool 等原生类型直接查表
//! 2. **泛型容器自动派生**：Option<T>/List<T>/Dict<K,V> 等递归检查 T/K/V
//! 3. **命名类型**：委托给 [`super::traits::satisfies`]（需外部提供 MethodProvider）
//!
//! 与 [`super::traits::satisfies`] 的区别：
//! - `satisfies` 通过 MethodProvider 查方法存在性（需要 provider 注册）
//! - `check_trait` 内置常见 trait 知识库，无需注册即可判断原生/容器类型

use crate::types::def::Type;
use super::traits::{TraitEnv, MethodProvider, satisfies};

/// 检查 `ty` 是否满足 trait `trait_name`。
///
/// - 若 `ty` 是内置原生类型 → 查内置表
/// - 若 `ty` 是泛型容器 → 递归检查类型实参
/// - 若 `ty` 是 Named → 委托 `satisfies`（需 provider + env）
///
/// `provider` 和 `env` 可选：传 None 时 Named 类型返回 Ok（延迟到 rustc）
pub fn check_trait(
    ty: &Type,
    trait_name: &str,
    provider: Option<&dyn MethodProvider>,
    env: Option<&TraitEnv>,
) -> Result<(), String> {
    // 1. Never 满足一切
    if matches!(ty, Type::Never) {
        return Ok(());
    }

    // 2. 内置原生类型查表
    if is_builtin_type(ty) {
        if builtin_traits_of(ty).iter().any(|t| *t == trait_name) {
            return Ok(());
        }
        return Err(format!("type `{}` does not implement trait `{}`", ty, trait_name));
    }

    // 3. 泛型容器：auto-derive 递归
    if let Type::Generic { base, args } = ty {
        let base_name = base_name(base);
        if let Some(name) = base_name {
            if auto_derive_containers(name, trait_name) {
                return check_all_args(args, trait_name, provider, env);
            }
            // 已知容器但该 trait 不自推导
            return Err(format!(
                "container `{}` does not auto-derive trait `{}`", name, trait_name
            ));
        }
        // 未知 base 类型 → defer 到 rustc
        return Ok(());
    }

    // 4. Option/Optional/Result 简写 → 递归
    match ty {
        Type::Option(inner) | Type::Optional(inner) => {
            if auto_derive_containers("Option", trait_name) {
                return check_trait(inner, trait_name, provider, env);
            }
            return Err(format!(
                "`Option` does not auto-derive trait `{}`", trait_name
            ));
        }
        Type::Result { ok, err } => {
            if auto_derive_containers("Result", trait_name) {
                check_trait(ok, trait_name, provider, env)?;
                return check_trait(err, trait_name, provider, env);
            }
            return Err(format!(
                "`Result` does not auto-derive trait `{}`", trait_name
            ));
        }
        _ => {}
    }

    // 5. 引用 / 可变引用：委派给内部类型
    match ty {
        Type::Ref(inner) | Type::MutRef(inner) => {
            return check_trait(inner, trait_name, provider, env);
        }
        _ => {}
    }

    // 6. 元组：所有元素都满足
    if let Type::Tuple(elems) = ty {
        for e in elems {
            check_trait(e, trait_name, provider, env)?;
        }
        return Ok(());
    }

    // 7. 交集类型：所有成员都必须满足
    if let Type::Intersection(members) = ty {
        for m in members {
            check_trait(m, trait_name, provider, env)?;
        }
        return Ok(());
    }

    // 8. 命名类型：委托 satisfies（若有 provider + env）
    if let Type::Named(_) = ty {
        if let (Some(p), Some(e)) = (provider, env) {
            return satisfies(e, p, ty, trait_name)
                .map_err(|e| format!("{}", e));
        }
        // 无 provider → 信任用户（延迟到 rustc）
        return Ok(());
    }

    // 其他类型（Fn/Simd/Self_/Unit/Any/Var）：保守通过
    Ok(())
}

/// 递归检查一组类型实参是否都满足某 trait
fn check_all_args(
    args: &[Type],
    trait_name: &str,
    provider: Option<&dyn MethodProvider>,
    env: Option<&TraitEnv>,
) -> Result<(), String> {
    for a in args {
        check_trait(a, trait_name, provider, env)?;
    }
    Ok(())
}

/// 获取 Generic base 的名称（若 base 为 Named）
fn base_name(base: &Type) -> Option<&str> {
    match base {
        Type::Named(n) => Some(n.as_str()),
        _ => None,
    }
}

/// 判断某类型是否为内置原生类型
fn is_builtin_type(ty: &Type) -> bool {
    matches!(ty,
        Type::Int | Type::F64 | Type::Float | Type::Str | Type::Bool
        | Type::None_ | Type::Unit | Type::Never
    )
}

/// 返回内置类型满足的 trait 列表
fn builtin_traits_of(ty: &Type) -> Vec<&'static str> {
    match ty {
        Type::Int => vec![
            "Clone", "Copy", "Debug", "Default", "Eq", "Ord", "Hash", "Display",
            "Add", "Sub", "Mul", "Div", "Rem", "Neg", "Not",
            "BitAnd", "BitOr", "BitXor", "Shl", "Shr",
            "Send", "Sync", "Unpin",
        ],
        Type::Str => vec![
            "Clone", "Debug", "Default", "Eq", "Ord", "Hash", "Display", "Add",
            "Send", "Sync", "Unpin",
        ],
        Type::Bool => vec![
            "Clone", "Copy", "Debug", "Default", "Eq", "Ord", "Hash", "Display",
            "Not", "BitAnd", "BitOr", "BitXor",
            "Send", "Sync", "Unpin",
        ],
        Type::F64 | Type::Float => vec![
            "Clone", "Copy", "Debug", "Default",
            "PartialEq", "PartialOrd", "Display",
            "Add", "Sub", "Mul", "Div", "Rem", "Neg",
            "Send", "Sync", "Unpin",
        ],
        Type::None_ | Type::Unit => vec![
            "Clone", "Copy", "Debug", "Default", "Eq", "Ord", "Hash",
            "Send", "Sync", "Unpin",
        ],
        _ => vec![],
    }
}

/// 已知容器类型的 auto-derive trait 传播规则
///
/// 返回 true 当 trait_name 通过 auto-derive 从类型实参传播
/// （即 Container<T>: Trait 当且仅当 T: Trait）
pub(crate) fn auto_derive_containers(container_name: &str, trait_name: &str) -> bool {
    // 所有标准容器 + 智能指针共通的 auto-derive trait
    let auto_derive = [
        "Clone", "Debug", "Default", "PartialEq", "Eq",
        "PartialOrd", "Ord", "Hash",
        "Send", "Sync", "Unpin",
    ];

    // 特定容器支持的额外 trait
    match container_name {
        "Option" | "Box" | "Rc" | "Arc" | "Cell" | "RefCell" => {
            auto_derive.contains(&trait_name)
        }
        "Result" | "Either" => {
            // Result 需要 T 和 E 都满足
            auto_derive.contains(&trait_name)
        }
        "List" | "Vec" | "VecDeque" | "LinkedList" | "BinaryHeap" => {
            auto_derive.contains(&trait_name)
        }
        "Dict" | "HashMap" | "BTreeMap" => {
            // HashMap/BTreeMap: key 需要更多约束
            // 简化：同上 auto_derive
            auto_derive.contains(&trait_name)
        }
        "Set" | "HashSet" | "BTreeSet" => {
            auto_derive.contains(&trait_name)
        }
        "String" => {
            // String 本身不是容器但有类似行为
            false
        }
        // 未知容器名 → 保守返回 true（信任用户/rustc）
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::def::Type;

    fn list(inner: Type) -> Type {
        Type::Generic {
            base: Box::new(Type::Named("List".into())),
            args: vec![inner],
        }
    }

    fn option(inner: Type) -> Type {
        Type::Generic {
            base: Box::new(Type::Named("Option".into())),
            args: vec![inner],
        }
    }

    fn dict(k: Type, v: Type) -> Type {
        Type::Generic {
            base: Box::new(Type::Named("Dict".into())),
            args: vec![k, v],
        }
    }

    // ── 内置类型 ──

    #[test]
    fn int_satisfies_clone() {
        assert!(check_trait(&Type::Int, "Clone", None, None).is_ok());
    }

    #[test]
    fn int_satisfies_add() {
        assert!(check_trait(&Type::Int, "Add", None, None).is_ok());
    }

    #[test]
    fn int_does_not_satisfy_display_as_str() {
        // Int 满足 Display 但不以 "Display" 字符串存储
        assert!(check_trait(&Type::Int, "Display", None, None).is_ok());
    }

    #[test]
    fn str_satisfies_clone() {
        assert!(check_trait(&Type::Str, "Clone", None, None).is_ok());
    }

    #[test]
    fn bool_satisfies_eq() {
        assert!(check_trait(&Type::Bool, "Eq", None, None).is_ok());
    }

    #[test]
    fn float_does_not_satisfy_eq() {
        assert!(check_trait(&Type::F64, "Eq", None, None).is_err(),
            "f64 should not implement Eq (NaN)");
    }

    #[test]
    fn float_satisfies_partial_eq() {
        assert!(check_trait(&Type::F64, "PartialEq", None, None).is_ok());
    }

    #[test]
    fn int_does_not_satisfy_nonexistent() {
        assert!(check_trait(&Type::Int, "SomeNonsenseTrait", None, None).is_err());
    }

    #[test]
    fn unit_satisfies_default() {
        assert!(check_trait(&Type::Unit, "Default", None, None).is_ok());
    }

    #[test]
    fn never_satisfies_everything() {
        assert!(check_trait(&Type::Never, "AnyTrait", None, None).is_ok());
        assert!(check_trait(&Type::Never, "Clone", None, None).is_ok());
    }

    // ── 泛型容器 auto-derive ──

    #[test]
    fn list_int_satisfies_clone() {
        let t = list(Type::Int);
        assert!(check_trait(&t, "Clone", None, None).is_ok());
    }

    #[test]
    fn list_named_satisfies_clone_deferred() {
        // Named 类型无条件通过（无 provider 时 defer 到 rustc）
        let t = list(Type::Named("MyStruct".into()));
        assert!(check_trait(&t, "Clone", None, None).is_ok());
    }

    #[test]
    fn nested_option_list_int_satisfies_clone() {
        let t = option(list(Type::Int));
        assert!(check_trait(&t, "Clone", None, None).is_ok());
    }

    #[test]
    fn dict_int_str_satisfies_clone() {
        let t = dict(Type::Int, Type::Str);
        assert!(check_trait(&t, "Clone", None, None).is_ok());
    }

    #[test]
    fn list_int_does_not_satisfy_add() {
        let t = list(Type::Int);
        // List 不自推导 Add
        assert!(check_trait(&t, "Add", None, None).is_err());
    }

    #[test]
    fn option_shorthand_satisfies_clone() {
        let t = Type::Option(Box::new(Type::Int));
        assert!(check_trait(&t, "Clone", None, None).is_ok());
    }

    #[test]
    fn result_satisfies_clone() {
        let t = Type::Result { ok: Box::new(Type::Int), err: Box::new(Type::Str) };
        assert!(check_trait(&t, "Clone", None, None).is_ok());
    }

    #[test]
    fn tuple_of_clonable_satisfies_clone() {
        let t = Type::Tuple(vec![Type::Int, Type::Str, Type::Bool]);
        assert!(check_trait(&t, "Clone", None, None).is_ok());
    }

    #[test]
    fn ref_of_clonable_satisfies_clone() {
        let t = Type::Ref(Box::new(Type::Int));
        assert!(check_trait(&t, "Clone", None, None).is_ok());
    }

    #[test]
    fn mut_ref_of_clonable_satisfies_clone() {
        let t = Type::MutRef(Box::new(Type::Int));
        assert!(check_trait(&t, "Clone", None, None).is_ok());
    }

    // ── 深层嵌套 ──

    #[test]
    fn five_level_nested_satisfies_clone() {
        let t = option(list(option(dict(Type::Int, Type::Str))));
        assert!(check_trait(&t, "Clone", None, None).is_ok());
    }

    #[test]
    fn five_level_nested_does_not_satisfy_add() {
        let t = option(list(option(dict(Type::Int, Type::Str))));
        assert!(check_trait(&t, "Add", None, None).is_err());
    }
}

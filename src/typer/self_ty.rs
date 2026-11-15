// Lang-Zone 编译器 — typer/self_ty.rs
// （由 typer/mod.rs move-only 拆出，逻辑零改动）

use super::*;

/// 计算枚举变体构造的返回类型（使用新的 fresh 泛型变量）
pub(crate) fn enum_self_type(variant: &EnumVariant, ctx: &mut InferCtx) -> Type {
    let subst = fresh_subst_for_generics(ctx, &variant.generics);
    enum_self_type_with_subst(variant, ctx, &subst)
}

/// 计算枚举变体构造的返回类型，使用外部提供的替换映射（保证 payload 与 return 共享 subst）
pub(crate) fn enum_self_type_with_subst(
    variant: &EnumVariant,
    ctx: &mut InferCtx,
    subst: &HashMap<String, Type>,
) -> Type {
    if let Some(ret) = &variant.return_type {
        apply_subst(subst, ret)
    } else if variant.generics.is_empty() {
        Type::Named(variant.enum_name.clone())
    } else {
        let args: Vec<Type> = variant
            .generics
            .iter()
            .map(|g| subst.get(g).cloned().unwrap_or_else(|| ctx.fresh_ty(0)))
            .collect();
        Type::Generic {
            base: Box::new(Type::Named(variant.enum_name.clone())),
            args,
        }
    }
}

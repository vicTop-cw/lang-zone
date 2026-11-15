//! 模式匹配穷尽性检查
//!
//! 目前仅检查最外层构造器是否覆盖完整；不深入子模式。

use crate::ast::Pattern;
use crate::types::Type;
use std::collections::HashMap;
use std::collections::HashSet;

/// 返回 scrutinee 类型在当前注册表下所有可能的顶层构造器名称。
fn type_constructors(
    ty: &Type,
    enum_variants: &HashMap<String, Vec<String>>,
) -> Option<Vec<String>> {
    match ty {
        Type::Bool => Some(vec!["True".into(), "False".into()]),
        Type::Option(_) | Type::Optional(_) => Some(vec!["Some".into(), "None".into()]),
        Type::Result { .. } => Some(vec!["Ok".into(), "Err".into()]),
        Type::Named(name) => enum_variants.get(name).cloned(),
        Type::Generic { base, .. } => match base.as_ref() {
            Type::Named(name) => enum_variants.get(name).cloned(),
            _ => None,
        },
        Type::Apply { constructor, .. } => match constructor.as_ref() {
            Type::Named(name) => enum_variants.get(name).cloned(),
            _ => None,
        },
        Type::Ref(inner) | Type::MutRef(inner) => type_constructors(inner, enum_variants),
        _ => None,
    }
}

/// 递归收集单个 pattern 覆盖的构造器。
///
/// 返回 `true` 表示该模式覆盖所有构造器（如 wildcard / 标识符绑定）。
fn collect_covered(
    pattern: &Pattern,
    constructors: &[String],
    covered: &mut HashSet<String>,
) -> bool {
    match pattern {
        Pattern::Wildcard | Pattern::Ident(_) => true,
        Pattern::As(inner, _) => collect_covered(inner, constructors, covered),
        Pattern::Bool(b) => {
            covered.insert(if *b { "True".into() } else { "False".into() });
            false
        }
        Pattern::Variant(name, _) | Pattern::StructVariant { name, .. } => {
            let variant = name.rsplit('.').next().unwrap_or(name.as_str());
            if constructors.iter().any(|c| c == variant) {
                covered.insert(variant.to_string());
            }
            false
        }
        _ => false,
    }
}

/// 检查模式列表是否穷尽了 `scrutinee_ty` 的所有顶层构造器。
///
/// - 返回 `None` 表示已穷尽或当前无法判断（避免误报）。
/// - 返回 `Some("missing: A, B")` 表示缺少的构造器。
pub fn check_exhaustive(
    scrutinee_ty: &Type,
    patterns: &[Pattern],
    enum_variants: &HashMap<String, Vec<String>>,
) -> Option<String> {
    let constructors = type_constructors(scrutinee_ty, enum_variants)?;
    let mut covered = HashSet::new();

    for pattern in patterns {
        if collect_covered(pattern, &constructors, &mut covered) {
            return None;
        }
    }

    if covered.len() == constructors.len() {
        return None;
    }

    let missing: Vec<String> = constructors
        .iter()
        .filter(|c| !covered.contains(*c))
        .cloned()
        .collect();

    Some(format!("missing: {}", missing.join(", ")))
}

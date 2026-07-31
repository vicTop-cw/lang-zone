// Lang-Zong 编译器 — codegen/derive.rs
// @derive 装饰器：收集去重 + 生成 Rust #[derive(...)] 属性

use crate::ast::Decorator;
use std::collections::HashMap;

/// 去重合并 derive trait 列表（后覆盖前），自动追加必需 trait
pub(super) fn collect_derive_traits(decorators: &[Decorator]) -> Vec<String> {
    let mut map: HashMap<String, usize> = HashMap::new(); // name → last index
    let mut ordered: Vec<String> = Vec::new();

    for d in decorators {
        if d.name == "derive" {
            for trait_name in &d.args {
                let s = match trait_name {
                    crate::ast::Expr::Ident(n) => n.clone(),
                    crate::ast::Expr::StrLit(n) => n.clone(),
                    _ => continue,
                };
                // 后覆盖前：移除旧位置
                if let Some(&old_idx) = map.get(&s) {
                    ordered[old_idx] = String::new(); // 标记为空
                }
                map.insert(s.clone(), ordered.len());
                ordered.push(s);
            }
        }
    }
    // 过滤空位 + 去空串
    ordered.retain(|s| !s.is_empty());
    ordered
}

/// 生成 #[derive(Trait1, Trait2, ...)] 字符串
/// 自动追加 Clone（lz 默认拷贝模型）
pub(super) fn gen_derive_attr(decorators: &[Decorator], is_enum: bool, has_debug: bool) -> String {
    let mut traits = collect_derive_traits(decorators);
    let has_explicit_clone = traits.iter().any(|t| t == "Clone");
    let has_explicit_debug = traits.iter().any(|t| t == "Debug");

    // 强制追加 Clone（lz 的 copy-by-default 模型要求）
    if !has_explicit_clone {
        traits.push("Clone".into());
    }
    // enum 或 __repr__ 存在时强制追加 Debug
    if (is_enum || has_debug) && !has_explicit_debug {
        traits.push("Debug".into());
    }

    // 过滤非内置 derivable trait（不在 Rust 内置 derive 列表中的跳过）
    let rust_builtin: &[&str] = &[
        "Clone", "Copy", "Debug", "Default", "PartialEq", "Eq",
        "PartialOrd", "Ord", "Hash",
    ];
    traits.retain(|t| rust_builtin.contains(&t.as_str()));

    if traits.is_empty() {
        String::new()
    } else {
        format!("#[derive({})]\n", traits.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Decorator, Expr};

    #[test]
    fn test_derive_dedup_last_wins() {
        let decors = vec![
            Decorator {
                name: "derive".into(),
                args: vec![
                    Expr::Ident("Clone".into()),
                    Expr::Ident("Debug".into()),
                    Expr::Ident("Clone".into()),  // 覆盖前面的 Clone
                ],
            },
        ];
        let traits = collect_derive_traits(&decors);
        assert_eq!(traits, vec!["Debug", "Clone"]);  // Clone 位置移到 Debug 之后
    }

    #[test]
    fn test_derive_empty() {
        let decors: Vec<Decorator> = vec![];
        let attr = gen_derive_attr(&decors, false, false);
        assert_eq!(attr, "#[derive(Clone)]\n");  // 始终有 Clone
    }

    #[test]
    fn test_derive_multiple() {
        let decors = vec![
            Decorator {
                name: "derive".into(),
                args: vec![
                    Expr::Ident("Default".into()),
                    Expr::Ident("Debug".into()),
                    Expr::Ident("Hash".into()),
                ],
            },
        ];
        let attr = gen_derive_attr(&decors, false, false);
        assert!(attr.contains("Default"));
        assert!(attr.contains("Debug"));
        assert!(attr.contains("Hash"));
        assert!(attr.contains("Clone"));  // 自动追加
    }

    #[test]
    fn test_derive_non_rust_filtered() {
        let decors = vec![
            Decorator {
                name: "derive".into(),
                args: vec![
                    Expr::Ident("Serialize".into()),    // 不在内置列表，被过滤
                    Expr::Ident("Deserialize".into()),  // 同上
                    Expr::Ident("Clone".into()),
                ],
            },
        ];
        let attr = gen_derive_attr(&decors, false, false);
        // 只有 Clone 在内置列表
        assert_eq!(attr, "#[derive(Clone)]\n");
    }
}

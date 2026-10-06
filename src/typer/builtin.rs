// Lang-Zone 编译器 — typer/builtin.rs
// （由 typer/mod.rs move-only 拆出，逻辑零改动）

use super::*;

/// 将 .lzi 类型字符串转换为 Type（支持常见基本类型）
pub(crate) fn str_to_type(s: &str) -> Type {
    str_to_type_opt(s).unwrap_or(Type::Named(s.to_string()))
}

pub(crate) fn str_to_type_opt(s: &str) -> Option<Type> {
    match s {
        "int" => Some(Type::Int),
        "f64" => Some(Type::F64),
        "str" | "String" => Some(Type::Str),
        "bool" => Some(Type::Bool),
        "()" | "Unit" => Some(Type::Unit),
        "!" | "Never" => Some(Type::Never),
        _ => None,
    }
}

/// 为注册表注入内置类型（List/Str/Option 等）的常用方法签名
pub(crate) fn inject_builtin_methods(
    registry: &mut std::collections::HashMap<
        String,
        std::collections::HashMap<String, (Vec<Type>, Type)>,
    >,
) {
    // List<T> / Vec<T>
    let mut list_methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
        std::collections::HashMap::new();
    list_methods.insert("len".into(), (vec![], Type::Int));
    list_methods.insert("is_empty".into(), (vec![], Type::Bool));
    list_methods.insert("push".into(), (vec![Type::Named("T".into())], Type::Unit));
    list_methods.insert("pop".into(), (vec![], Type::Unit)); // 泛型不实例化，codegen 处理 .pop().unwrap()
    registry.insert("List".into(), list_methods);

    // Vec<T> 是 List 的 Rust 映射名
    let mut vec_methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
        std::collections::HashMap::new();
    vec_methods.insert("len".into(), (vec![], Type::Int));
    vec_methods.insert("is_empty".into(), (vec![], Type::Bool));
    registry.insert("Vec".into(), vec_methods);

    // String / Str
    let mut str_methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
        std::collections::HashMap::new();
    str_methods.insert("len".into(), (vec![], Type::Int));
    str_methods.insert("is_empty".into(), (vec![], Type::Bool));
    str_methods.insert("as_str".into(), (vec![], Type::Str));
    str_methods.insert("to_string".into(), (vec![], Type::Str));
    str_methods.insert("clone".into(), (vec![], Type::Str));
    registry.insert("String".into(), str_methods.clone());
    registry.insert("str".into(), str_methods);

    // Option<T>
    let mut option_methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
        std::collections::HashMap::new();
    option_methods.insert("is_some".into(), (vec![], Type::Bool));
    option_methods.insert("is_none".into(), (vec![], Type::Bool));
    option_methods.insert("unwrap".into(), (vec![], Type::Unit)); // 泛型不实例化
    registry.insert("Option".into(), option_methods);

    // Result<T, E>
    let mut result_methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
        std::collections::HashMap::new();
    result_methods.insert("is_ok".into(), (vec![], Type::Bool));
    result_methods.insert("is_err".into(), (vec![], Type::Bool));
    result_methods.insert(
        "ok".into(),
        (vec![], Type::Optional(Box::new(Type::Named("T".into())))),
    );
    result_methods.insert(
        "err".into(),
        (vec![], Type::Optional(Box::new(Type::Named("E".into())))),
    );
    registry.insert("Result".into(), result_methods);
}

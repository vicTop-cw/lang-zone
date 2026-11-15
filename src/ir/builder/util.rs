// Lang-Zone 编译器 — ir/builder/util.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

/// 从装饰器列表提取 `@derive(...)` 请求的派生 trait 名（如 `@derive(Clone, Debug)` → ["Clone", "Debug"]）。
pub(crate) fn collect_derives(decorators: &[crate::ast::Decorator]) -> Vec<String> {
    decorators
        .iter()
        .filter(|d| d.name == "derive")
        .flat_map(|d| {
            d.args.iter().filter_map(|a| {
                if let crate::ast::Expr::Ident(n) = a {
                    Some(n.clone())
                } else {
                    None
                }
            })
        })
        .collect()
}

/// 判断 IR 类型是否可 Clone + PartialEq（@memoize 约束检查用）。
///
/// 内置标量（Int/F64/Bool/Str/Unit）均可 Clone+PartialEq；
/// 容器（Named List/Tuple/Dict）默认可 Clone+PartialEq（元素可 Clone+PartialEq）；
/// 自定义 struct/enum 由用户 derive 决定——此处保守判定为可 Clone（trust user）。
pub(crate) fn is_cloneable_ir_type(ty: &IrType) -> bool {
    match ty {
        IrType::Int
        | IrType::Int128
        | IrType::BigInt
        | IrType::Complex
        | IrType::F64
        | IrType::Bool
        | IrType::Str
        | IrType::Unit
        | IrType::Any => true,
        IrType::Option(inner) => is_cloneable_ir_type(inner),
        IrType::Result { ok, err } => is_cloneable_ir_type(ok) && is_cloneable_ir_type(err),
        IrType::Tuple(elems) => elems.iter().all(is_cloneable_ir_type),
        IrType::Ref(inner) | IrType::MutRef(inner) => is_cloneable_ir_type(inner),
        IrType::Named { path, args } => {
            // Named 容器（List/Vec/Dict/Rc/Arc 等）：元素可 Clone 即可
            let is_container = matches!(
                path.as_str(),
                "List" | "Vec" | "Dict" | "Rc" | "Arc" | "HashSet" | "HashMap"
            );
            if is_container {
                args.iter().all(is_cloneable_ir_type)
            } else {
                // 自定义 struct/enum：保守放行（由 derive 保证）
                true
            }
        }
        IrType::Generic(_)
        | IrType::Self_
        | IrType::Never
        | IrType::Ext
        | IrType::Duck { .. }
        | IrType::Fn { .. } => true,
    }
}

/// 生成 IR 类型的人类可读名称（约束检查错误信息用）。
pub(crate) fn ir_type_name(ty: &IrType) -> String {
    match ty {
        IrType::Int => "int".into(),
        IrType::Int128 => "int128".into(),
        IrType::BigInt => "bigint".into(),
        IrType::Complex => "complex".into(),
        IrType::F64 => "float".into(),
        IrType::Bool => "bool".into(),
        IrType::Str => "str".into(),
        IrType::Unit => "()".into(),
        IrType::Any => "any".into(),
        IrType::Named { path, args } => {
            if args.is_empty() {
                path.clone()
            } else {
                let names: Vec<String> = args.iter().map(ir_type_name).collect();
                format!("{}[{}]", path, names.join(", "))
            }
        }
        IrType::Option(inner) => format!("{}?", ir_type_name(inner)),
        IrType::Result { ok, err } => {
            format!("Result[{}, {}]", ir_type_name(ok), ir_type_name(err))
        }
        IrType::Tuple(elems) => {
            let names: Vec<String> = elems.iter().map(ir_type_name).collect();
            format!("({})", names.join(", "))
        }
        IrType::Ref(inner) => format!("&{}", ir_type_name(inner)),
        IrType::MutRef(inner) => format!("&mut {}", ir_type_name(inner)),
        IrType::Generic(name) => name.clone(),
        IrType::Self_ => "Self".into(),
        IrType::Never => "!".into(),
        IrType::Ext => "Ext".into(),
        IrType::Duck { .. } => "duck".into(),
        IrType::Fn { .. } => "fn".into(),
    }
}

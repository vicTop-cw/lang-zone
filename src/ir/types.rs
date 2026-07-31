// Lang-Zone 编译器 — ir/types.rs
// LZIR 类型系统：与后端无关的类型表示，独立于 crate::types::Type
//
// 设计原则：
// 1. 只表达语义类型，不含语法糖（如 Optional(T)→Option(T)）
// 2. 不包含后端映射（如 List→Vec 由后端 mapping table 处理）
// 3. Named 统一表达用户定义类型和标准库内建类型

/// LZIR 类型表示 — 与任何后端无关
#[derive(Debug, Clone, PartialEq)]
pub enum IrType {
    // ── 内建原语 ──
    Int,
    F64,
    Str,
    Bool,
    Unit,       // ()
    Never,      // !
    Any,        // 未确定类型（fallback）
    Self_,      // self 参数类型

    // ── 命名类型（含泛型参数） ──
    // 例：Named("Option", [Int])、Named("Vec", [Str])、Named("MyStruct", [])
    Named {
        path: String,
        args: Vec<IrType>,
    },

    // ── 特殊容器（语义标记，方便后端处理） ──
    Option(Box<IrType>),            // Option<T>
    Result {
        ok: Box<IrType>,
        err: Box<IrType>,
    },

    // ── 复合类型 ──
    Tuple(Vec<IrType>),             // (T1, T2, ...)
    Fn {
        params: Vec<IrType>,
        ret: Box<IrType>,
    },
    Ref(Box<IrType>),               // &T
    MutRef(Box<IrType>),            // &mut T

    // ── 结构化类型 (duck typing) ──
    Duck { fields: Vec<(String, IrType)> },  // 结构匹配：{ name: T, ... }

    // ── 泛型变量 ──
    Generic(String),                // 未实例化的泛型参数，如 T
}

impl IrType {
    /// 快速构造命名类型（无泛型参数）
    pub fn named(path: &str) -> Self {
        IrType::Named { path: path.to_string(), args: vec![] }
    }

    /// 快速构造命名类型（带泛型参数）
    pub fn named_with(path: &str, args: Vec<IrType>) -> Self {
        IrType::Named { path: path.to_string(), args }
    }

    /// 判断是否为 Any（未确定类型）
    pub fn is_any(&self) -> bool {
        matches!(self, IrType::Any)
    }

    /// 判断类型中是否包含未解析的泛型变量
    pub fn contains_generics(&self) -> bool {
        match self {
            IrType::Generic(_) => true,
            IrType::Named { args, .. } => args.iter().any(|a| a.contains_generics()),
            IrType::Option(inner) => inner.contains_generics(),
            IrType::Result { ok, err } => ok.contains_generics() || err.contains_generics(),
            IrType::Tuple(elems) => elems.iter().any(|e| e.contains_generics()),
            IrType::Fn { params, ret } => params.iter().any(|p| p.contains_generics()) || ret.contains_generics(),
            IrType::Ref(inner) | IrType::MutRef(inner) => inner.contains_generics(),
            IrType::Duck { fields } => fields.iter().any(|(_, t)| t.contains_generics()),
            _ => false,
        }
    }

    /// 用具体类型替换泛型变量
    /// generics: 泛型参数名列表（如 ["T"]）
    /// concrete: 对应的具体类型（如 [Str]）
    pub fn substitute_generics(&self, generics: &[String], concrete: &[IrType]) -> IrType {
        match self {
            IrType::Generic(name) => {
                generics.iter()
                    .position(|g| g == name)
                    .and_then(|i| concrete.get(i))
                    .cloned()
                    .unwrap_or_else(|| self.clone())
            }
            IrType::Named { path, args } => {
                let new_args: Vec<IrType> = args.iter()
                    .map(|a| a.substitute_generics(generics, concrete))
                    .collect();
                IrType::Named { path: path.clone(), args: new_args }
            }
            IrType::Option(inner) => {
                IrType::Option(Box::new(inner.substitute_generics(generics, concrete)))
            }
            IrType::Result { ok, err } => IrType::Result {
                ok: Box::new(ok.substitute_generics(generics, concrete)),
                err: Box::new(err.substitute_generics(generics, concrete)),
            },
            IrType::Tuple(elems) => IrType::Tuple(
                elems.iter().map(|e| e.substitute_generics(generics, concrete)).collect()
            ),
            IrType::Fn { params, ret } => IrType::Fn {
                params: params.iter().map(|p| p.substitute_generics(generics, concrete)).collect(),
                ret: Box::new(ret.substitute_generics(generics, concrete)),
            },
            IrType::Ref(inner) => IrType::Ref(Box::new(inner.substitute_generics(generics, concrete))),
            IrType::MutRef(inner) => IrType::MutRef(Box::new(inner.substitute_generics(generics, concrete))),
            IrType::Duck { fields } => IrType::Duck {
                fields: fields.iter()
                    .map(|(n, t)| (n.clone(), t.substitute_generics(generics, concrete)))
                    .collect(),
            },
            other => other.clone(),
        }
    }
}

/// 将 crate::types::Type 映射为 IrType（带泛型参数上下文）
/// generics: 当前函数定义的泛型参数名列表
pub fn from_ast_type_with_generics(ast_ty: &crate::types::Type, generics: &[String]) -> IrType {
    use crate::types::Type as AstType;
    match ast_ty {
        AstType::Named(name) => {
            if generics.contains(name) {
                IrType::Generic(name.clone())
            } else {
                IrType::named(name)
            }
        }
        AstType::Generic { base, args } => {
            let base_name = match base.as_ref() {
                AstType::Named(n) => n.clone(),
                other => format!("{:?}", other),
            };
            let ir_args: Vec<IrType> = args.iter()
                .map(|a| from_ast_type_with_generics(a, generics))
                .collect();
            IrType::Named { path: base_name, args: ir_args }
        }
        AstType::Option(inner) => IrType::Option(Box::new(from_ast_type_with_generics(inner, generics))),
        AstType::Result { ok, err } => IrType::Result {
            ok: Box::new(from_ast_type_with_generics(ok, generics)),
            err: Box::new(from_ast_type_with_generics(err, generics)),
        },
        AstType::Optional(inner) => IrType::Option(Box::new(from_ast_type_with_generics(inner, generics))),
        AstType::Ref(inner) => IrType::Ref(Box::new(from_ast_type_with_generics(inner, generics))),
        AstType::MutRef(inner) => IrType::MutRef(Box::new(from_ast_type_with_generics(inner, generics))),
        AstType::Fn { params, ret } => IrType::Fn {
            params: params.iter().map(|p| from_ast_type_with_generics(p, generics)).collect(),
            ret: Box::new(from_ast_type_with_generics(ret, generics)),
        },
        AstType::Tuple(elems) => IrType::Tuple(
            elems.iter().map(|e| from_ast_type_with_generics(e, generics)).collect()
        ),
        other => from_ast_type(other),
    }
}

/// 将 crate::types::Type 映射为 IrType
/// 在 AST → IR 构造阶段使用
pub fn from_ast_type(ast_ty: &crate::types::Type) -> IrType {
    use crate::types::Type as AstType;
    match ast_ty {
        AstType::Int => IrType::Int,
        AstType::F64 | AstType::Float => IrType::F64,
        AstType::Str => IrType::Str,
        AstType::Bool => IrType::Bool,
        AstType::None_ | AstType::Unit => IrType::Unit,
        AstType::Never => IrType::Never,
        AstType::Any => IrType::Any,
        AstType::Self_ => IrType::Self_,
        AstType::Duck { fields } => IrType::Duck {
            fields: fields.iter().map(|(n, t)| (n.clone(), from_ast_type(t))).collect(),
        },
        AstType::Named(name) => IrType::named(name),
        AstType::Generic { base, args } => {
            let base_name = match base.as_ref() {
                AstType::Named(n) => n.clone(),
                other => format!("{:?}", other),
            };
            let ir_args: Vec<IrType> = args.iter().map(|a| from_ast_type(a)).collect();
            IrType::Named { path: base_name, args: ir_args }
        }
        AstType::Option(inner) => IrType::Option(Box::new(from_ast_type(inner))),
        AstType::Result { ok, err } => IrType::Result {
            ok: Box::new(from_ast_type(ok)),
            err: Box::new(from_ast_type(err)),
        },
        AstType::Optional(inner) => {
            IrType::Option(Box::new(from_ast_type(inner)))
        }
        AstType::Ref(inner) => IrType::Ref(Box::new(from_ast_type(inner))),
        AstType::MutRef(inner) => IrType::MutRef(Box::new(from_ast_type(inner))),
        AstType::Fn { params, ret } => IrType::Fn {
            params: params.iter().map(|p| from_ast_type(p)).collect(),
            ret: Box::new(from_ast_type(ret)),
        },
        AstType::Tuple(elems) => IrType::Tuple(
            elems.iter().map(|e| from_ast_type(e)).collect()
        ),
        AstType::Simd { elem, .. } => IrType::Named {
            path: "Simd".to_string(),
            args: vec![from_ast_type(elem)],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Type as AstType;

    #[test]
    fn test_from_ast_primitives() {
        assert_eq!(from_ast_type(&AstType::Int), IrType::Int);
        assert_eq!(from_ast_type(&AstType::F64), IrType::F64);
        assert_eq!(from_ast_type(&AstType::Str), IrType::Str);
        assert_eq!(from_ast_type(&AstType::Bool), IrType::Bool);
        assert_eq!(from_ast_type(&AstType::Unit), IrType::Unit);
    }

    #[test]
    fn test_from_ast_option() {
        let opt = AstType::Option(Box::new(AstType::Int));
        assert_eq!(from_ast_type(&opt), IrType::Option(Box::new(IrType::Int)));
    }

    #[test]
    fn test_from_ast_optional_sugar() {
        let opt = AstType::Optional(Box::new(AstType::Str));
        assert_eq!(from_ast_type(&opt), IrType::Option(Box::new(IrType::Str)));
    }

    #[test]
    fn test_from_ast_result() {
        let res = AstType::Result {
            ok: Box::new(AstType::Str),
            err: Box::new(AstType::Named("IOError".into())),
        };
        let expected = IrType::Result {
            ok: Box::new(IrType::Str),
            err: Box::new(IrType::named("IOError")),
        };
        assert_eq!(from_ast_type(&res), expected);
    }

    #[test]
    fn test_from_ast_generic() {
        let list_int = AstType::Generic {
            base: Box::new(AstType::Named("List".into())),
            args: vec![AstType::Int],
        };
        let expected = IrType::Named {
            path: "List".to_string(),
            args: vec![IrType::Int],
        };
        assert_eq!(from_ast_type(&list_int), expected);
    }
}

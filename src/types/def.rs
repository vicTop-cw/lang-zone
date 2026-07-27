// Lang-Zong 编译器 — types/def.rs
// 结构化类型表示
// Lang-Zong 编译器 — type_system.rs
// 结构化类型表示：从纯 String 迁移到 Type 枚举
// align with hermes/00-最终语法规范.md

/// 携带源码位置信息的泛型包装器
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub line: usize,
    pub col: usize,
}

impl<T> Spanned<T> {
    pub fn new(node: T, line: usize, col: usize) -> Self {
        Spanned { node, line, col }
    }
}

/// Lang-Zong 结构化类型表示
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // ── 基本类型 ──
    Int,
    F64,
    Float,   // float 别名，等价于 f64
    Str,
    Bool,
    None_,
    Never,
    Any,
    Unit,    // 枚举无字段变体（空类型）

    // ── 命名类型（自定义 struct/enum/trait 或泛型参数） ──
    Named(String),

    // ── 泛型实例化 List<int>, Dict<K,V>, Set<T> ──
    Generic {
        base: Box<Type>,
        args: Vec<Type>,
    },

    // ── 标准容器（语法层面区分，语义等价于 Generic 但便于模式匹配） ──
    Option(Box<Type>),
    Result {
        ok: Box<Type>,
        err: Box<Type>,
    },

    // ── 语法糖 ──
    Optional(Box<Type>),   // T? → Option<T>

    // ── 引用 ──
    Ref(Box<Type>),
    MutRef(Box<Type>),

    // ── 函数类型 ──
    Fn {
        params: Vec<Type>,
        ret: Box<Type>,
    },

    // ── 元组 ──
    Tuple(Vec<Type>),

    // ── SIMD 向量 ──
    Simd { elem: Box<Type>, width: usize },

    // ── Self 占位 ──
    Self_,
}

impl Type {
    /// 将 Lang-Zong Type 映射为 Rust 类型字符串
    /// 等价于原 codegen.rs 中的 map_type(&str)
    pub fn to_rust_type_string(&self) -> String {
        match self {
            Type::Int => "i64".to_string(),
            Type::F64 | Type::Float => "f64".to_string(),
            Type::Str => "String".to_string(),
            Type::Bool => "bool".to_string(),
            Type::None_ => "()".to_string(),
            Type::Never => "!".to_string(),
            Type::Any => "std::any::Any".to_string(),
            Type::Unit => String::new(),
            Type::Self_ => "Self".to_string(),

            Type::Named(name) => name.clone(),

            Type::Generic { base, args } => {
                // 容器类型映射：List→Vec, Dict→HashMap, Set→HashSet
                let rust_base = match base.as_ref() {
                    Type::Named(name) => match name.as_str() {
                        "List" => "Vec",
                        "Dict" => "HashMap",
                        "Set" => "HashSet",
                        _ => name.as_str(),
                    },
                    other => {
                        // 递归映射（非 Named base 的直接使用其输出）
                        let mapped = other.to_rust_type_string();
                        let args_s: Vec<String> = args.iter()
                            .map(|a| a.to_rust_type_string())
                            .collect();
                        return format!("{}<{}>", mapped, args_s.join(", "));
                    }
                };
                let args_s: Vec<String> = args.iter()
                    .map(|a| a.to_rust_type_string())
                    .collect();
                format!("{}<{}>", rust_base, args_s.join(", "))
            }

            Type::Option(inner) => {
                format!("Option<{}>", inner.to_rust_type_string())
            }

            Type::Result { ok, err } => {
                format!("Result<{}, {}>", ok.to_rust_type_string(), err.to_rust_type_string())
            }

            Type::Optional(inner) => {
                // T? → Option<T>
                format!("Option<{}>", inner.to_rust_type_string())
            }

            Type::Ref(inner) => {
                format!("&{}", inner.to_rust_type_string())
            }

            Type::MutRef(inner) => {
                format!("&mut {}", inner.to_rust_type_string())
            }

            Type::Fn { params, ret } => {
                let params_s: Vec<String> = params.iter()
                    .map(|p| p.to_rust_type_string())
                    .collect();
                format!("fn({}) -> {}", params_s.join(", "), ret.to_rust_type_string())
            }

            Type::Tuple(elems) => {
                let elems_s: Vec<String> = elems.iter()
                    .map(|e| e.to_rust_type_string())
                    .collect();
                format!("({})", elems_s.join(", "))
            }

            Type::Simd { elem, width } => {
                // Simd[f64, 4] → wide::f64x4, Simd[i32, 8] → wide::i32x8
                format!("wide::{}x{}", elem.to_rust_type_string(), width)
            }
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_rust_type_string())
    }
}

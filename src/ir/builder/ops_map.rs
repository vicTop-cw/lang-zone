// Lang-Zone 编译器 — ir/builder/ops_map.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

// ══════════════════════════════════════════════════════════════
// AST 运算符映射
// ══════════════════════════════════════════════════════════════

pub(crate) fn map_binop(op: &BinOp) -> BinOpKind {
    match op {
        BinOp::Add => BinOpKind::Add,
        BinOp::Sub => BinOpKind::Sub,
        BinOp::Mul => BinOpKind::Mul,
        BinOp::Div => BinOpKind::Div,
        BinOp::Mod => BinOpKind::Mod,
        BinOp::Eq => BinOpKind::Eq,
        BinOp::Ne => BinOpKind::Neq,
        BinOp::Lt => BinOpKind::Lt,
        BinOp::Gt => BinOpKind::Gt,
        BinOp::Le => BinOpKind::Le,
        BinOp::Ge => BinOpKind::Ge,
        BinOp::And => BinOpKind::And,
        BinOp::Or => BinOpKind::Or,
        BinOp::BitAnd => BinOpKind::BitAnd,
        BinOp::BitOr => BinOpKind::BitOr,
        BinOp::BitXor => BinOpKind::Xor,
        BinOp::Shl => BinOpKind::Shl,
        BinOp::Shr => BinOpKind::Shr,
        BinOp::Pow => BinOpKind::Pow,
        BinOp::In => BinOpKind::In,
        BinOp::NotIn => BinOpKind::NotIn,
        BinOp::Is => BinOpKind::Eq, // Is 降级 (Rust 无 is 运算符)
    }
}

/// 运算符 → 魔术方法名 映射（用于用户自定义类型重载）
pub(crate) fn magic_method_for_binop(op: &BinOp) -> Option<&'static str> {
    match op {
        BinOp::Add => Some("__add__"),
        BinOp::Sub => Some("__sub__"),
        BinOp::Mul => Some("__mul__"),
        BinOp::Div => Some("__div__"),
        BinOp::Mod => Some("__rem__"),
        BinOp::Eq => Some("__eq__"),
        BinOp::Ne => Some("__ne__"),
        BinOp::Lt => Some("__lt__"),
        BinOp::Gt => Some("__gt__"),
        BinOp::Le => Some("__le__"),
        BinOp::Ge => Some("__ge__"),
        BinOp::BitAnd => Some("__bitand__"),
        BinOp::BitOr => Some("__bitor__"),
        BinOp::BitXor => Some("__bitxor__"),
        BinOp::Pow => Some("__pow__"),
        BinOp::In => Some("__contains__"),
        _ => None,
    }
}

pub(crate) fn map_unop(op: &UnaryOp) -> UnOpKind {
    match op {
        // 不可达：Pos 在 convert_expr 中已恒等返回（数值）或魔术分派（struct）
        UnaryOp::Pos => UnOpKind::Neg,
        UnaryOp::Neg => UnOpKind::Neg,
        UnaryOp::Not => UnOpKind::Not,
        UnaryOp::BitNot => UnOpKind::Not, // 位非降级为逻辑非
        UnaryOp::Deref => UnOpKind::Deref,
        UnaryOp::Ref => UnOpKind::Ref,
    }
}

pub(crate) fn map_assign_op(op: &AssignOp) -> BinOpKind {
    match op {
        AssignOp::Eq => BinOpKind::Eq,
        AssignOp::AddEq => BinOpKind::Add,
        AssignOp::SubEq => BinOpKind::Sub,
        AssignOp::MulEq => BinOpKind::Mul,
        AssignOp::DivEq => BinOpKind::Div,
        AssignOp::ModEq => BinOpKind::Mod,
        AssignOp::AndEq => BinOpKind::BitAnd,
        AssignOp::OrEq => BinOpKind::BitOr,
        AssignOp::XorEq => BinOpKind::Xor,
        AssignOp::ShlEq => BinOpKind::Shl,
        AssignOp::ShrEq => BinOpKind::Shr,
        AssignOp::PowEq => BinOpKind::Pow,
    }
}

/// 复合赋值对应的就地魔术方法名（06d §四）：`+=`→`__iadd__` 等。
/// 目标类型定义了该魔术方法时优先调用，而非脱糖 `a = a + b`
///（脱糖需要 `Add` impl，只定义 `__iadd__` 的类型会 E0369）。
pub(crate) fn assign_op_magic(op: &AssignOp) -> Option<&'static str> {
    match op {
        AssignOp::AddEq => Some("__iadd__"),
        AssignOp::SubEq => Some("__isub__"),
        AssignOp::MulEq => Some("__imul__"),
        AssignOp::DivEq => Some("__idiv__"),
        _ => None,
    }
}

/// 泛型参数规范化：`Named("T", [])`（from_ast_type 表示）→ `Generic("T")`
/// （infer 表示），递归处理嵌套（`Named("Rc", [Named("T")])` → `Named("Rc", [Generic("T")])`）。
/// 用于 return 隐式转换判断中两侧容器元素类型比较（box.lz `Err(self.clone())`）。
pub(crate) fn normalize_gen(ty: &IrType, generics: &[String]) -> IrType {
    match ty {
        IrType::Named { path, args } => {
            if args.is_empty() {
                if generics.iter().any(|g| g == path) {
                    IrType::Generic(path.clone())
                } else {
                    ty.clone()
                }
            } else {
                let new_args: Vec<IrType> =
                    args.iter().map(|a| normalize_gen(a, generics)).collect();
                IrType::Named {
                    path: path.clone(),
                    args: new_args,
                }
            }
        }
        _ => ty.clone(),
    }
}

/// 将类型名字符串转换为 IrType（用于 `is` 运算符）
pub(crate) fn name_to_ir_type(name: &str) -> IrType {
    match name {
        "int" | "i64" => IrType::Int,
        "bigint" | "BigInt" => IrType::BigInt,
        "complex" | "Complex" | "Complex64" => IrType::Complex,
        "str" | "String" => IrType::Str,
        "f64" | "float" => IrType::F64,
        "bool" => IrType::Bool,
        "Ext" => IrType::Ext,
        "List" | "Vec" => IrType::Named {
            path: "List".into(),
            args: vec![],
        },
        "Dict" | "HashMap" => IrType::Named {
            path: "Dict".into(),
            args: vec![],
        },
        "Set" | "HashSet" => IrType::Named {
            path: "Set".into(),
            args: vec![],
        },
        _ => IrType::Named {
            path: name.to_string(),
            args: vec![],
        },
    }
}

/// 判断 `x as T` 是否为编译器内置转换（无需 `__cast__`/`__try_cast__`）：
/// 基本数值类型（int/f64/bool/str）之间的转换由编译器内置实现
/// （01-类型系统.md §6：数值基本类型间 `as` 不依赖魔法方法）；
/// 内置容器类型（List/Dict/Set/Option/Result/Box/Tuple 等）的字面量
/// 类型标注（`[] as List<int>`、`None as Option<int>`）同样是内置标注，
/// 不需要 `__cast__`/`__try_cast__`。
/// 非内置转换（用户自定义 struct/enum/duck 参与）必须由源类型实现
/// `__cast__<T>()` 或 `__try_cast__<T>() -> Result<T, E>`。
pub(crate) fn is_builtin_cast(src: &IrType, target: &IrType) -> bool {
    let src_builtin = matches!(
        src,
        IrType::Int | IrType::F64 | IrType::Bool | IrType::Str | IrType::Unit | IrType::Never
    );
    let tgt_builtin = matches!(
        target,
        IrType::Int | IrType::F64 | IrType::Bool | IrType::Str | IrType::Unit | IrType::Never
    );
    if src_builtin && tgt_builtin {
        return true;
    }
    // 内置容器类型名（字面量标注放行）
    // 注意：AST 中 `List<int>` 的 type_name 被解析成整串 "Vec<i64>"，
    // name_to_ir_type 走 `_ =>` 分支存为 Named { path: "Vec<i64>" }，
    // 因此需按 `<` 前的基名匹配（"Vec"）而非全名。
    let builtin_container = |t: &IrType| -> bool {
        match t {
            IrType::Named { path, .. } => {
                let base = path.split('<').next().unwrap_or(path.as_str());
                matches!(
                    base,
                    "List"
                        | "Vec"
                        | "Dict"
                        | "HashMap"
                        | "Set"
                        | "HashSet"
                        | "Option"
                        | "Result"
                        | "Box"
                        | "Tuple"
                        | "String"
                        | "Any"
                )
            }
            _ => false,
        }
    };
    builtin_container(src) && builtin_container(target)
}

/// 编译期类型兼容检查（用于 `is` 运算符和类型转换）
pub(crate) fn ir_types_compatible(a: &IrType, b: &IrType) -> bool {
    match (a, b) {
        // Any 与任何类型兼容（None、未知等）
        (IrType::Any, _) | (_, IrType::Any) => true,
        // 相同基础类型
        (IrType::Int, IrType::Int)
        | (IrType::F64, IrType::F64)
        | (IrType::Str, IrType::Str)
        | (IrType::Bool, IrType::Bool)
        | (IrType::Unit, IrType::Unit)
        | (IrType::Never, IrType::Never) => true,
        // Named 类型按名称匹配
        (IrType::Named { path: a_path, .. }, IrType::Named { path: b_path, .. }) => {
            a_path == b_path
        }
        // Option 解包
        (IrType::Option(inner), other) | (other, IrType::Option(inner)) => {
            ir_types_compatible(inner, other)
        }
        _ => false,
    }
}

/// 按类型名构造 IrType（turbofish 类型参数用）
pub(crate) fn from_ast_type_name(name: &str) -> IrType {
    match name {
        "int" | "i64" => IrType::Int,
        "bigint" | "BigInt" => IrType::BigInt,
        "complex" | "Complex" | "Complex64" => IrType::Complex,
        "str" | "String" => IrType::Str,
        "f64" | "float" => IrType::F64,
        "bool" => IrType::Bool,
        "Ext" => IrType::Ext,
        other => IrType::Named {
            path: other.to_string(),
            args: vec![],
        },
    }
}

/// 递归替换类型中的 `Self` 引用为具体类型（struct 定义内 Self → 自身类型名）。
pub(crate) fn replace_self(ty: &IrType, self_ty: &IrType) -> IrType {
    match ty {
        IrType::Self_ => self_ty.clone(),
        IrType::Named { path, args } => {
            let new_args: Vec<IrType> = args.iter().map(|a| replace_self(a, self_ty)).collect();
            IrType::Named {
                path: path.clone(),
                args: new_args,
            }
        }
        IrType::Option(inner) => IrType::Option(Box::new(replace_self(inner, self_ty))),
        IrType::Result { ok, err } => IrType::Result {
            ok: Box::new(replace_self(ok, self_ty)),
            err: Box::new(replace_self(err, self_ty)),
        },
        IrType::Tuple(elems) => {
            IrType::Tuple(elems.iter().map(|e| replace_self(e, self_ty)).collect())
        }
        IrType::Ref(inner) => IrType::Ref(Box::new(replace_self(inner, self_ty))),
        IrType::MutRef(inner) => IrType::MutRef(Box::new(replace_self(inner, self_ty))),
        _ => ty.clone(),
    }
}

/// 剥掉一层 Option（`T?` 的两种 IR 表示），非 Option 原样返回。
pub(crate) fn is_option_ty_ir(ty: &IrType) -> bool {
    matches!(ty, IrType::Option(_))
        || matches!(ty, IrType::Named { path, args } if path == "Option" && args.len() == 1)
}

pub(crate) fn strip_option_ty(ty: &IrType) -> IrType {
    match ty {
        IrType::Option(inner) => (**inner).clone(),
        IrType::Named { path, args } if path == "Option" && args.len() == 1 => args[0].clone(),
        other => other.clone(),
    }
}

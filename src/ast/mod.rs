// Lang-Zong 编译器 — ast/mod.rs
// AST 抽象语法树模块入口：重导出所有节点

mod decl;
mod expr;
mod stmt;

pub use decl::*;
pub use expr::*;
pub use stmt::*;

use std::collections::HashSet;

/// 内建类型白名单（codegen 直接映射，不经过自定义类型表）。
/// 同时作为「隐式泛型」判定依据：签名中出现的非常规 Named 类型名视为泛型形参。
pub fn builtin_type_names() -> HashSet<&'static str> {
    let mut s = HashSet::new();
    for n in [
        "int", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
        "u128", "usize", "float", "f32", "f64", "str", "string", "char", "byte",
        "bool", "None", "unit", "void", "any", "never", "Self", "self",
        "List", "Array", "Vec", "Dict", "Map", "HashMap", "BTreeMap", "Set",
        "HashSet", "BTreeSet", "Option", "Some", "Result", "Ok", "Err", "Tuple",
        "Ptr", "Pointer", "Ref", "MutRef", "Fn", "FnMut", "FnOnce", "Iterator",
        "Iter", "Generator", "Simd", "Box", "Any", "Object", "Json", "JSON",
        "Rc", "Arc", "Weak", "Maybe", "Never", "Auto", "Ext",
        "String", "__Params",
        "Iterable", "Cell", "Ordering", "Error", "Box", "Mutex", "RwLock", "AtomicBool",
        "AtomicI32", "AtomicU64", "Duration", "Instant", "Path", "PathBuf", "File",
        "Ordered", "Clone", "Copy", "Display", "Debug", "Eq", "Ord", "PartialEq",
        "PartialOrd", "Default", "Hash", "Add", "Sub", "Mul", "Div", "Rem", "Neg",
        "Index", "IndexMut", "IntoIterator", "FromIterator", "AsRef", "AsMut",
        "Deref", "Drop", "Send", "Sync", "Into", "From", "ToString",
    ] {
        s.insert(n);
    }
    s
}

/// 从类型签名收集「未声明的隐式泛型形参」：遍历 Type 树，收集所有简单标识符形态的
/// `Named` 叶子（即 `a`/`b` 这类），排除 `known` 命中的常规类型与已显式声明的 `explicit`。
/// 语义：支持 `def fold(xs: List<a>, init: b, f: fn(b, a) -> b) -> b`（不写 `<a,b>` 也视为泛型）。
pub fn collect_implicit_generics(
    types: &[&crate::types::def::Type],
    known: &dyn Fn(&str) -> bool,
    explicit: &[String],
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    fn walk(
        ty: &crate::types::def::Type,
        known: &dyn Fn(&str) -> bool,
        explicit: &[String],
        out: &mut Vec<String>,
        seen: &mut HashSet<String>,
    ) {
        match ty {
            crate::types::def::Type::Named(n) => {
                let simple = n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if simple && !known(n) && !explicit.contains(n) && seen.insert(n.clone()) {
                    out.push(n.clone());
                }
            }
            crate::types::def::Type::Generic { base, args } => {
                walk(base, known, explicit, &mut *out, &mut *seen);
                for a in args {
                    walk(a, known, explicit, &mut *out, &mut *seen);
                }
            }
            crate::types::def::Type::Option(i)
            | crate::types::def::Type::Optional(i)
            | crate::types::def::Type::Ref(i)
            | crate::types::def::Type::MutRef(i) => walk(i, known, explicit, &mut *out, &mut *seen),
            crate::types::def::Type::Result { ok, err } => {
                walk(ok, known, explicit, &mut *out, &mut *seen);
                walk(err, known, explicit, &mut *out, &mut *seen);
            }
            crate::types::def::Type::Fn { params, ret } => {
                for p in params {
                    walk(p, known, explicit, &mut *out, &mut *seen);
                }
                walk(ret, known, explicit, &mut *out, &mut *seen);
            }
            crate::types::def::Type::Tuple(ts) => {
                for t in ts {
                    walk(t, known, explicit, &mut *out, &mut *seen);
                }
            }
            crate::types::def::Type::Simd { elem, .. } => walk(elem, known, explicit, &mut *out, &mut *seen),
            crate::types::def::Type::Duck { fields } => {
                for (_, t) in fields {
                    walk(t, known, explicit, &mut *out, &mut *seen);
                }
            }
            _ => {}
        }
    }
    for t in types {
        walk(t, known, explicit, &mut out, &mut seen);
    }
    out
}

/// 计算函数的最终泛型形参列表：显式声明的 `func.generics` 加上签名中未声明的隐式泛型。
/// `known` 用于排除常规类型（内建/自定义 struct/enum），避免把已知类型误当泛型。
pub fn augmented_fn_generics(func: &Function, known: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut g = func.generics.clone();
    let mut sig: Vec<&crate::types::def::Type> = Vec::new();
    for p in &func.params {
        sig.push(&p.ty);
    }
    if let Some(rt) = &func.return_type {
        sig.push(rt);
    }
    for (_, t) in &func.generic_defaults {
        sig.push(t);
    }
    for w in &func.where_clause {
        for b in &w.bounds {
            sig.push(b);
        }
    }
    let implicit = collect_implicit_generics(&sig, known, &func.generics);
    for ig in implicit {
        if !g.contains(&ig) {
            g.push(ig);
        }
    }
    g
}

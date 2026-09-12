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
        "Ordered", "PartialOrder", "Clone", "Copy", "Display", "Debug", "Eq", "Ord",
        "PartialEq", "PartialOrd", "Default", "Hash", "Add", "Sub", "Mul", "Div",
        "Rem", "Neg",
        "Index", "IndexMut", "IntoIterator", "FromIterator", "AsRef", "AsMut",
        "Deref", "Drop", "Send", "Sync", "Into", "From", "ToString",
    ] {
        s.insert(n);
    }
    s
}

/// 计算函数的最终泛型形参列表：显式声明的 `func.generics` 加上外层作用域提供的泛型
/// （如 `impl<T> List<T>` 的方法体内可用的 `T`，由调用方通过 `enclosing` 传入）。
///
/// 设计约束：**泛型必须显式声明**。签名中未声明且不属于外层作用域的类型名
/// （如顶层 `def fold(xs: List<a>)` 中的 `a`）一律视为「未知类型」，不再隐式当作
/// 泛型形参。因此 `List<a>` 这类缺省泛型声明的写法会被明确拒绝（报「未知类型: a」）。
pub fn augmented_fn_generics(func: &Function, enclosing: &[String]) -> Vec<String> {
    let mut g = func.generics.clone();
    for e in enclosing {
        if !g.contains(e) {
            g.push(e.clone());
        }
    }
    g
}

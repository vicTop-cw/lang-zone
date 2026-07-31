import re
path = r'e:\IDEProjects\AI\lang-zone\src\types\def.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

old = '''/// Lang-Zong 结构化类型表示
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // ── 推断变量（inference hole）──
    // 仅存在于推断阶段，codegen 前必须被 zonk 解析为具体类型
    Var(TypeVar),

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
    },'''

new = '''/// 类型构造器的 kind（HKT 最小支持）。
/// - `Star`: 普通类型（*）。
/// - `Arrow { params, ret }`: 类型构造器，如 `* -> *`。
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Star,
    Arrow { params: Vec<Kind>, ret: Box<Kind> },
}

/// Lang-Zong 结构化类型表示
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // ── 推断变量（inference hole）──
    // 仅存在于推断阶段，codegen 前必须被 zonk 解析为具体类型
    Var(TypeVar),

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

    // ── 类型构造器（HKT）：如 List、Option，携带 arity ──
    Constructor { name: String, arity: usize },

    // ── 类型构造器应用（HKT）：如 F[A]、List<int> ──
    Apply { constructor: Box<Type>, args: Vec<Type> },

    // ── 泛型实例化 List<int>, Dict<K,V>, Set<T> ──
    // 语义上等价于 Apply { constructor: Named(base), args }，保留以兼容现有代码。
    Generic {
        base: Box<Type>,
        args: Vec<Type>,
    },

    // ── 标准容器（语法层面区分，语义等价于 Generic 但便于模式匹配） ──
    Option(Box<Type>),
    Result {
        ok: Box<Type>,
        err: Box<Type>,
    },'''

if old not in content:
    print('OLD NOT FOUND')
    raise SystemExit(1)

content = content.replace(old, new)
with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print('OK')

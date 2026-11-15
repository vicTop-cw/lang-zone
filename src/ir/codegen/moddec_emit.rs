// Lang-Zone 编译器 — src/ir/codegen/moddec_emit.rs
// 修饰符装饰器 → Rust 目标类型 的**纯发射函数**（无状态、可独立单测）。
//
// 设计真值：workbuddy/plan/2026-09-14-moddec-架构设计.md §1.4
// 「IR/codegen 新轴 → Rust 目标类型」。
//
// 职责边界：本模块**只做字符串发射**，不持有 `CodeGen` 状态、不生成语句；
// 由 `ir/codegen/mod.rs` 的 let / 形参 / 常量发射路径按需调用（接线归 T04）。
//
// 组合约定（架构 §1.4 目标类型表：`@mutex → Arc<Mutex<T>>`、`@atomic → Arc<AtomicI64>`）：
// **内部可变轴在内、共享轴在外**（与 `Arc<Mutex<T>>` / `Rc<RefCell<T>>` 的嵌套一致）：
//
//     wrap_shared( &wrap_interior(T, interior), shared )
//     //          ^^^^^^^^^^^^^^^^ 先内层（interior）， ^^^^^^^ 后外层（shared）
//
// 例：
//     wrap_shared(&wrap_interior("i64", InteriorMode::Atomic), SharedMode::Arc)
//         == "Arc<AtomicI64>"                       // `@atomic`
//     wrap_shared(&wrap_interior("i64", InteriorMode::Mutex), SharedMode::Arc)
//         == "Arc<Mutex<i64>>"                      // `@mutex`
//
// 接线方（T04）务必按此顺序调用：先 `wrap_interior` 生成内层，再 `wrap_shared` 包外层。
//
// 枚举来源：`src/ast/modifier.rs`（T01 冻结契约）。该模块在本 crate 中以
// `pub use` 重导出为 `crate::ast::{SharedMode, InteriorMode}`（`ast::modifier`
// 子模块本身为私有），故此处经重导出路径引入。

use crate::ast::{InteriorMode, SharedMode};

/// 共享轴 → 引用计数包装（架构 §1.4）。
///
/// - `None`   → 原样（无共享，零开销）；
/// - `Rc`     → `Rc<inner>`（单线程引用计数，`@shared` / `@rc`）；
/// - `Arc`    → `Arc<inner>`（原子引用计数，跨线程，`@arc`）；
/// - `Weak`   → `Weak<inner>`（弱引用，`@weak`）。
///
/// `inner` 为**已发射**的内层 Rust 类型串（可能已是 `RefCell<T>` 等），
/// 本函数只负责最外层包装，不做类型解析。
pub fn wrap_shared(inner: &str, mode: SharedMode) -> String {
    match mode {
        SharedMode::None => inner.to_string(),
        SharedMode::Rc => format!("Rc<{inner}>"),
        SharedMode::Arc => format!("Arc<{inner}>"),
        SharedMode::Weak => format!("Weak<{inner}>"),
    }
}

/// 内部可变轴 → 内部可变包装（架构 §1.4）。
///
/// - `None`    → 原样；
/// - `Cell`    → `Cell<inner>`（`inner: Copy`）；
/// - `RefCell` → `RefCell<inner>`（运行时借用检查）；
/// - `Mutex`   → `Mutex<inner>`（互斥锁）；
/// - `RwLock`  → `RwLock<inner>`（读写锁）；
/// - `Atomic`  → 标量原子类型（委托 [`atomic_type`]，如 `i64` → `AtomicI64`）。
///
/// 注意 `Atomic` 分支**不是** `Atomic<inner>`：std 无泛型原子类型，
/// 故按 [`atomic_type`] 的标量映射产出具体类型（`Arc<AtomicI64>` 等）。
pub fn wrap_interior(inner: &str, mode: InteriorMode) -> String {
    match mode {
        InteriorMode::None => inner.to_string(),
        InteriorMode::Cell => format!("Cell<{inner}>"),
        InteriorMode::RefCell => format!("RefCell<{inner}>"),
        InteriorMode::Mutex => format!("Mutex<{inner}>"),
        InteriorMode::RwLock => format!("RwLock<{inner}>"),
        InteriorMode::Atomic => atomic_type(inner),
    }
}

/// 判据：某 `origin` 是否处于「线程安全 / 静态存储」语境。
///
/// `wrap_lazy` 不接收额外参数，线程安全性由**装饰器来源**推断：
/// - `once`（`@once`）     → 跨线程一次性初始化 → 线程安全；
/// - `lazy_static`（`@lazy_static`）→ 模块级静态 → 线程安全；
/// - 其余（`lazy` / `lazy_mut` / `None`）→ 局部（单线程）语境。
fn lazy_is_thread_safe(origin: Option<&str>) -> bool {
    matches!(origin, Some("once") | Some("lazy_static"))
}

/// 惰性轴 → `OnceCell` / `OnceLock` 包装（架构 §1.4）。
///
/// - `origin == Some("once")` → 恒为 `OnceLock<inner>`（**契约要求**，QA L4 断言 `OnceLock<`）；
/// - `origin == Some("lazy_static")` → `OnceLock<inner>`（静态、线程安全）；
/// - `origin == Some("lazy")` / `Some("lazy_mut")` / `None` → `OnceCell<inner>`（局部）。
///
/// 语义：绑定处**不求值**，首次访问经 `get_or_init`（`OnceCell`）/ `get_or_init`、
/// `get_or_try_init`（`OnceLock`）仅求值一次（AC4）。
pub fn wrap_lazy(inner: &str, origin: Option<&str>) -> String {
    if lazy_is_thread_safe(origin) {
        format!("OnceLock<{inner}>")
    } else {
        format!("OnceCell<{inner}>")
    }
}

/// 标量类型 → std 原子类型（用于 `@atomic`）。
///
/// 映射表（架构 §1.4：`@atomic → Arc<AtomicXxx>`）：
/// `i8→AtomicI8`、`i16→AtomicI16`、`i32→AtomicI32`、`i64→AtomicI64`、
/// `isize→AtomicIsize`、`u8→AtomicU8`、`u16→AtomicU16`、`u32→AtomicU32`、
/// `u64→AtomicU64`、`usize→AtomicUsize`、`bool→AtomicBool`；
/// 源语言别名：`int→AtomicI64`、`byte→AtomicU8`。
///
/// 非标量类型在 std 中**无对应原子类型**（如 `i128`/`u128`/`f32`/`f64`/结构体）：
/// 返回 `Atomic<inner>` 占位——它会在 `rustc` 阶段暴露为类型错误；
/// 语义层（T02/上游）应先行拒绝非标量上的 `@atomic`。
pub fn atomic_type(inner: &str) -> String {
    let atom = match inner {
        "i8" => "AtomicI8",
        "i16" => "AtomicI16",
        "i32" => "AtomicI32",
        "i64" => "AtomicI64",
        "isize" => "AtomicIsize",
        "u8" => "AtomicU8",
        "u16" => "AtomicU16",
        "u32" => "AtomicU32",
        "u64" => "AtomicU64",
        "usize" => "AtomicUsize",
        "bool" => "AtomicBool",
        // 源语言（LZ）别名 —— 与 ast::builtin_type_names 对齐。
        "int" => "AtomicI64",
        "byte" => "AtomicU8",
        // 非标量：占位（详见函数文档）。
        _ => return format!("Atomic<{inner}>"),
    };
    atom.to_string()
}

/// `@static` 全局声明发射（架构 §1.4）。
///
/// - 有初始化式 → `static NAME: TY = INIT;`（模块级静态）；
/// - 无初始化式 → `static NAME: LazyLock<TY> = LazyLock::new(|| Default::default());`
///   （惰性静态，首次访问经 `LazyLock` 求值一次；要求 `TY: Default`）。
///
/// `TY` 为已发射的 Rust 类型串；`INIT` 为已发射的 Rust 表达式串。
pub fn static_decl(name: &str, ty: &str, init: Option<&str>) -> String {
    match init {
        Some(expr) => format!("static {name}: {ty} = {expr};"),
        None => format!("static {name}: LazyLock<{ty}> = LazyLock::new(|| Default::default());"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── wrap_shared ──────────────────────────────────────────────
    #[test]
    fn wrap_shared_none_is_identity() {
        assert_eq!(wrap_shared("i64", SharedMode::None), "i64");
    }

    #[test]
    fn wrap_shared_rc() {
        assert_eq!(wrap_shared("String", SharedMode::Rc), "Rc<String>");
    }

    #[test]
    fn wrap_shared_arc() {
        assert_eq!(wrap_shared("i64", SharedMode::Arc), "Arc<i64>");
    }

    #[test]
    fn wrap_shared_weak() {
        assert_eq!(wrap_shared("Node", SharedMode::Weak), "Weak<Node>");
    }

    // ── wrap_interior ────────────────────────────────────────────
    #[test]
    fn wrap_interior_none_is_identity() {
        assert_eq!(wrap_interior("i64", InteriorMode::None), "i64");
    }

    #[test]
    fn wrap_interior_cell() {
        assert_eq!(wrap_interior("i64", InteriorMode::Cell), "Cell<i64>");
    }

    #[test]
    fn wrap_interior_refcell() {
        assert_eq!(
            wrap_interior("String", InteriorMode::RefCell),
            "RefCell<String>"
        );
    }

    #[test]
    fn wrap_interior_mutex() {
        assert_eq!(wrap_interior("i64", InteriorMode::Mutex), "Mutex<i64>");
    }

    #[test]
    fn wrap_interior_rwlock() {
        assert_eq!(wrap_interior("i64", InteriorMode::RwLock), "RwLock<i64>");
    }

    #[test]
    fn wrap_interior_atomic_delegates_to_scalar_mapping() {
        assert_eq!(wrap_interior("i64", InteriorMode::Atomic), "AtomicI64");
        assert_eq!(wrap_interior("bool", InteriorMode::Atomic), "AtomicBool");
    }

    // ── 组合（架构 §1.4 时序图；QA L4 断言）────────────────────────
    #[test]
    fn composite_arc_mutex() {
        // @mutex：interior=Mutex（内） + shared=Arc（外） → Arc<Mutex<i64>>。
        let inner = wrap_interior("i64", InteriorMode::Mutex);
        let ty = wrap_shared(&inner, SharedMode::Arc);
        assert_eq!(ty, "Arc<Mutex<i64>>");
        assert!(ty.starts_with("Arc<Mutex<"));
    }

    #[test]
    fn composite_arc_rwlock() {
        // @rwlock：Arc<RwLock<Config>>。
        let inner = wrap_interior("Config", InteriorMode::RwLock);
        let ty = wrap_shared(&inner, SharedMode::Arc);
        assert_eq!(ty, "Arc<RwLock<Config>>");
        assert!(ty.starts_with("Arc<RwLock<"));
    }

    #[test]
    fn composite_arc_atomic_i64_matches_l4() {
        // @atomic：Arc<AtomicI64>（QA L4 断言）。
        let inner = wrap_interior("i64", InteriorMode::Atomic);
        let ty = wrap_shared(&inner, SharedMode::Arc);
        assert_eq!(ty, "Arc<AtomicI64>");
        assert!(ty.contains("AtomicI64"));
    }

    #[test]
    fn composite_rc_refcell() {
        // @shared(Rc) + @cell(RefCell) → Rc<RefCell<State>>。
        let inner = wrap_interior("State", InteriorMode::RefCell);
        let ty = wrap_shared(&inner, SharedMode::Rc);
        assert_eq!(ty, "Rc<RefCell<State>>");
        assert!(ty.starts_with("Rc<"));
    }

    // ── wrap_lazy ────────────────────────────────────────────────
    #[test]
    fn wrap_lazy_once_is_oncelock() {
        let ty = wrap_lazy("Config", Some("once"));
        assert_eq!(ty, "OnceLock<Config>");
        // 契约硬约束：QA L4 期望子串 `OnceLock<`。
        assert!(ty.starts_with("OnceLock<"));
    }

    #[test]
    fn wrap_lazy_static_is_oncelock() {
        assert_eq!(wrap_lazy("Config", Some("lazy_static")), "OnceLock<Config>");
    }

    #[test]
    fn wrap_lazy_local_is_oncecell() {
        assert_eq!(wrap_lazy("Config", Some("lazy")), "OnceCell<Config>");
        assert_eq!(wrap_lazy("Config", Some("lazy_mut")), "OnceCell<Config>");
    }

    #[test]
    fn wrap_lazy_basic_default_is_oncecell() {
        // `@lazy` 为基础装饰器（origin == None）：局部缓存 → OnceCell。
        assert_eq!(wrap_lazy("Config", None), "OnceCell<Config>");
    }

    #[test]
    fn wrap_lazy_unknown_origin_is_oncecell() {
        assert_eq!(wrap_lazy("Config", Some("whatever")), "OnceCell<Config>");
    }

    // ── atomic_type 映射 ─────────────────────────────────────────
    #[test]
    fn atomic_type_integer_mapping() {
        let cases = [
            ("i8", "AtomicI8"),
            ("i16", "AtomicI16"),
            ("i32", "AtomicI32"),
            ("i64", "AtomicI64"),
            ("isize", "AtomicIsize"),
            ("u8", "AtomicU8"),
            ("u16", "AtomicU16"),
            ("u32", "AtomicU32"),
            ("u64", "AtomicU64"),
            ("usize", "AtomicUsize"),
        ];
        for (inner, expected) in cases {
            assert_eq!(atomic_type(inner), expected, "inner = {inner}");
        }
    }

    #[test]
    fn atomic_type_bool_and_aliases() {
        assert_eq!(atomic_type("bool"), "AtomicBool");
        assert_eq!(atomic_type("int"), "AtomicI64");
        assert_eq!(atomic_type("byte"), "AtomicU8");
    }

    #[test]
    fn atomic_type_non_scalar_is_placeholder() {
        // 非标量无 std 原子类型：给出可辨识占位（下游 rustc 报错）。
        assert_eq!(atomic_type("String"), "Atomic<String>");
        assert_eq!(atomic_type("Vec<i64>"), "Atomic<Vec<i64>>");
    }

    // ── static_decl ──────────────────────────────────────────────
    #[test]
    fn static_decl_with_init() {
        assert_eq!(
            static_decl("MAX", "i64", Some("100")),
            "static MAX: i64 = 100;"
        );
    }

    #[test]
    fn static_decl_without_init_is_lazylock() {
        let s = static_decl("CFG", "Config", None);
        assert_eq!(
            s,
            "static CFG: LazyLock<Config> = LazyLock::new(|| Default::default());"
        );
        assert!(s.starts_with("static CFG: LazyLock<Config> = LazyLock::new("));
        assert!(s.ends_with(';'));
    }
}

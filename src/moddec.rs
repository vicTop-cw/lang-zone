// Lang-Zong 编译器 — src/moddec.rs
// 装饰器注册表（内置封闭集）：28 个修饰符装饰器（11 基础 + 17 融合）+ 13 个普通装饰器，
// 以及统一诊断文案 / 错误码段（E-MODDEC-*）。
// 设计真值：workbuddy/plan/2026-09-14-moddec-架构设计.md §1.5 / §3.2 / §8。
//
// 关键决策：
// - 融合型「解析即展开为成分轴」，并在 `Modifiers.origin` 保留融合原名（仅诊断）。
// - 基础装饰器 `origin = None`，保证与关键字形式产物一致（AC1）。
// - 注册表为运行期构建一次（`LazyLock`），因 `Modifiers.origin: Option<String>` 无法 const 化。

use std::sync::LazyLock;

use crate::ast::{InteriorMode, Modifiers, Mutability, SharedMode};

/// 装饰器分类：修饰符装饰器（与目标同行，每目标 ≤ 1）vs 普通装饰器（独占一行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoratorClass {
    /// 修饰符装饰器（基础 11 + 融合 17）。
    Modifier,
    /// 普通装饰器（函数 / 类型级，13 个既有装饰器）。
    Normal,
}

/// 单个装饰器的规格。
#[derive(Debug, Clone, PartialEq)]
pub struct DecoratorSpec {
    /// 装饰器名（小写 snake_case，不含前导 `@`）。
    pub name: &'static str,
    /// 分类。
    pub class: DecoratorClass,
    /// 展开后的轴集合（`Normal` 类为空）。
    pub axes: Modifiers,
    /// 是否为融合型（多轴组合语法糖）。
    pub fusion: bool,
}

/// 装饰器注册表：内置封闭集的查询入口。
pub struct DecoratorRegistry;

impl DecoratorRegistry {
    /// 全部内置装饰器（注册顺序 = 基础 → 融合 → 普通）。
    pub fn all() -> &'static [DecoratorSpec] {
        &REGISTRY
    }

    /// 按名字查找装饰器规格。
    pub fn lookup(name: &str) -> Option<&'static DecoratorSpec> {
        REGISTRY.iter().find(|s| s.name == name)
    }

    /// 是否为修饰符装饰器。
    pub fn is_modifier(name: &str) -> bool {
        matches!(Self::lookup(name), Some(s) if s.class == DecoratorClass::Modifier)
    }

    /// 是否为普通装饰器。
    pub fn is_normal(name: &str) -> bool {
        matches!(Self::lookup(name), Some(s) if s.class == DecoratorClass::Normal)
    }

    /// 是否为已知（内置）装饰器。
    pub fn is_known(name: &str) -> bool {
        Self::lookup(name).is_some()
    }
}

/// 运行期构建一次的注册表。
static REGISTRY: LazyLock<Vec<DecoratorSpec>> = LazyLock::new(build_registry);

/// 构造内置装饰器注册表（单一真值来源）。
fn build_registry() -> Vec<DecoratorSpec> {
    let mut specs = Vec::with_capacity(41);

    // ── ① 基础修饰符装饰器（11 个，origin = None，fusion = false）──
    specs.push(basic(
        "mut",
        Modifiers {
            mutability: Mutability::Mut,
            ..Modifiers::empty()
        },
    ));
    specs.push(basic(
        "immut",
        Modifiers {
            mutability: Mutability::Immut,
            ..Modifiers::empty()
        },
    ));
    specs.push(basic(
        "ref",
        Modifiers {
            is_ref: true,
            mutability: Mutability::Mut, // 裸 ref 默认可变引用（§1.3）
            ..Modifiers::empty()
        },
    ));
    specs.push(basic(
        "owned",
        Modifiers {
            owned: true,
            ..Modifiers::empty()
        },
    ));
    specs.push(basic(
        "const",
        Modifiers {
            is_const: true,
            ..Modifiers::empty()
        },
    ));
    specs.push(basic(
        "static",
        Modifiers {
            storage_static: true,
            ..Modifiers::empty()
        },
    ));
    specs.push(basic(
        "shared",
        Modifiers {
            shared: SharedMode::Rc, // O5：默认 Rc
            ..Modifiers::empty()
        },
    ));
    specs.push(basic(
        "cell",
        Modifiers {
            interior: InteriorMode::Cell,
            ..Modifiers::empty()
        },
    ));
    specs.push(basic(
        "lazy",
        Modifiers {
            lazy: true,
            ..Modifiers::empty()
        },
    ));
    specs.push(basic(
        "comptime",
        Modifiers {
            comptime: true,
            ..Modifiers::empty()
        },
    ));
    specs.push(basic(
        "source",
        Modifiers {
            source: true,
            ..Modifiers::empty()
        },
    ));

    // ── ② 融合型装饰器（17 个，origin = Some(name)，fusion = true）──
    // ① 引用族
    specs.push(fusion(
        "borrow",
        Modifiers {
            is_ref: true,
            mutability: Mutability::Immut,
            origin: Some("borrow".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "borrow_mut",
        Modifiers {
            is_ref: true,
            mutability: Mutability::Mut,
            origin: Some("borrow_mut".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "rcell",
        Modifiers {
            is_ref: true,
            interior: InteriorMode::Cell,
            origin: Some("rcell".to_string()),
            ..Modifiers::empty()
        },
    ));
    // ② 所有权族
    specs.push(fusion(
        "take",
        Modifiers {
            owned: true,
            mutability: Mutability::Mut,
            origin: Some("take".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "take_ro",
        Modifiers {
            owned: true,
            mutability: Mutability::Immut,
            origin: Some("take_ro".to_string()),
            ..Modifiers::empty()
        },
    ));
    // ③ 共享 / 并发族
    specs.push(fusion(
        "rc",
        Modifiers {
            shared: SharedMode::Rc,
            origin: Some("rc".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "arc",
        Modifiers {
            shared: SharedMode::Arc,
            origin: Some("arc".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "weak",
        Modifiers {
            shared: SharedMode::Weak,
            origin: Some("weak".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "mutex",
        Modifiers {
            shared: SharedMode::Arc,
            interior: InteriorMode::Mutex,
            origin: Some("mutex".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "rwlock",
        Modifiers {
            shared: SharedMode::Arc,
            interior: InteriorMode::RwLock,
            origin: Some("rwlock".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "atomic",
        Modifiers {
            shared: SharedMode::Arc,
            interior: InteriorMode::Atomic,
            origin: Some("atomic".to_string()),
            ..Modifiers::empty()
        },
    ));
    // ④ 静态 / 惰性族
    specs.push(fusion(
        "lazy_static",
        Modifiers {
            storage_static: true,
            lazy: true,
            origin: Some("lazy_static".to_string()),
            ..Modifiers::empty()
        },
    ));
    // 注：设计 §1.5 将 @once 记作 `shared=OnceLock`，但 SharedMode 冻结为 {None,Rc,Arc,Weak}，
    // 无 OnceLock 变体；OnceLock<T> 为线程安全，故取 Arc 表达其「跨线程一次性共享」语义。
    specs.push(fusion(
        "once",
        Modifiers {
            lazy: true,
            shared: SharedMode::Arc,
            origin: Some("once".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "lazy_mut",
        Modifiers {
            lazy: true,
            mutability: Mutability::Mut,
            origin: Some("lazy_mut".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "static_mut",
        Modifiers {
            storage_static: true,
            mutability: Mutability::Mut,
            origin: Some("static_mut".to_string()),
            ..Modifiers::empty()
        },
    ));
    // ⑤ 编译期族
    specs.push(fusion(
        "compile_time",
        Modifiers {
            comptime: true,
            is_const: true,
            origin: Some("compile_time".to_string()),
            ..Modifiers::empty()
        },
    ));
    specs.push(fusion(
        "meta",
        Modifiers {
            comptime: true,
            source: true,
            origin: Some("meta".to_string()),
            ..Modifiers::empty()
        },
    ));

    // ── ③ 普通装饰器（13 个既有；独占一行；axes 为空）──
    for name in [
        "export",
        "extern",
        "embed",
        "memoize",
        "parallel",
        "simd",
        "tail_call",
        "overload",
        "math",
        "derive",
        "curry",
        "init",
        "unsafe",
        "case",
    ] {
        specs.push(normal(name));
    }

    specs
}

/// 构造「基础修饰符装饰器」规格（`origin = None`）。
fn basic(name: &'static str, axes: Modifiers) -> DecoratorSpec {
    DecoratorSpec {
        name,
        class: DecoratorClass::Modifier,
        axes,
        fusion: false,
    }
}

/// 构造「融合型修饰符装饰器」规格。
fn fusion(name: &'static str, axes: Modifiers) -> DecoratorSpec {
    DecoratorSpec {
        name,
        class: DecoratorClass::Modifier,
        axes,
        fusion: true,
    }
}

/// 构造「普通装饰器」规格（空轴）。
fn normal(name: &'static str) -> DecoratorSpec {
    DecoratorSpec {
        name,
        class: DecoratorClass::Normal,
        axes: Modifiers::empty(),
        fusion: false,
    }
}

/// 统一诊断文案与错误码（§8 Shared Knowledge）。
///
/// 文案模板中的 `{name}` / `{a}` / `{b}` 为占位符，由下方 helper 填充；
/// 物理位置（行:列）由 [`LOC_SUFFIX`] 约定，经 [`at_loc`] 附加。
pub mod errmsg {
    // ── 错误码（供测试断言 / 诊断输出）──
    /// 未知装饰器。
    pub const E_MODDEC_UNKNOWN: &str = "E-MODDEC-UNKNOWN";
    /// 修饰符装饰器未与目标同行。
    pub const E_MODDEC_INLINE: &str = "E-MODDEC-INLINE";
    /// 普通装饰器未独占一行。
    pub const E_MODDEC_OWNLINE: &str = "E-MODDEC-OWNLINE";
    /// 每目标超过一个修饰符装饰器。
    pub const E_MODDEC_AT_MOST_ONE: &str = "E-MODDEC-AT-MOST-ONE";
    /// `let` 与修饰符装饰器冲突。
    pub const E_MODDEC_LET_CONFLICT: &str = "E-MODDEC-LET-CONFLICT";
    /// 修饰轴冲突。
    pub const E_MODDEC_AXIS_CONFLICT: &str = "E-MODDEC-AXIS-CONFLICT";

    // ── 文案模板（占位：`{name}` / `{a}` / `{b}`）──
    /// 未知装饰器文案。
    pub const UNKNOWN: &str = "未知装饰器 '@{name}'（内置封闭集，不可扩展）";
    /// 修饰符装饰器「须同行」文案。
    pub const MUST_INLINE: &str = "修饰符装饰器 '@{name}' 须与其目标同行";
    /// 普通装饰器「须独占一行」文案。
    pub const MUST_OWN_LINE: &str = "普通装饰器 '@{name}' 须独占一行";
    /// 「每目标至多一个」文案。
    pub const AT_MOST_ONE: &str = "每目标至多一个修饰符装饰器（如需组合多轴，请改用融合型）";
    /// `let` 冲突文案。
    pub const LET_CONFLICT: &str = "'let' 与 '@{name}' 语义冲突（let 已固定不可变）";
    /// 轴冲突文案。
    pub const AXIS_CONFLICT: &str = "修饰轴冲突：@{a} 与 @{b} 不可并存";

    // ── 别名（任务文本与设计文本的两种命名并存）──
    /// [`MUST_INLINE`] 的别名。
    pub const INLINE: &str = MUST_INLINE;
    /// [`MUST_OWN_LINE`] 的别名。
    pub const OWNLINE: &str = MUST_OWN_LINE;

    /// 物理位置占位（行:列），供 [`LOC_SUFFIX`] / [`at_loc`] 使用。
    pub const LOC_PLACEHOLDER: &str = "{line}:{col}";
    /// 位置后缀模板。
    pub const LOC_SUFFIX: &str = "（位置 {line}:{col}）";

    /// 生成「未知装饰器」文案。
    pub fn unknown(name: &str) -> String {
        UNKNOWN.replace("{name}", name)
    }

    /// 生成「修饰符装饰器须同行」文案。
    pub fn must_inline(name: &str) -> String {
        MUST_INLINE.replace("{name}", name)
    }

    /// 生成「普通装饰器须独占一行」文案。
    pub fn must_own_line(name: &str) -> String {
        MUST_OWN_LINE.replace("{name}", name)
    }

    /// 生成「每目标至多一个」文案。
    pub fn at_most_one() -> String {
        AT_MOST_ONE.to_string()
    }

    /// 生成「`let` 冲突」文案。
    pub fn let_conflict(name: &str) -> String {
        LET_CONFLICT.replace("{name}", name)
    }

    /// 生成「轴冲突」文案。
    pub fn axis_conflict(a: &str, b: &str) -> String {
        AXIS_CONFLICT.replace("{a}", a).replace("{b}", b)
    }

    /// 在文案尾部附加「（位置 行:列）」，满足「错误信息须含位置」的约定。
    pub fn at_loc(msg: &str, line: usize, col: usize) -> String {
        let suffix = LOC_SUFFIX
            .replace("{line}", &line.to_string())
            .replace("{col}", &col.to_string());
        format!("{msg}{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_counts() {
        let all = DecoratorRegistry::all();
        let modifiers = all
            .iter()
            .filter(|s| s.class == DecoratorClass::Modifier)
            .count();
        let normals = all
            .iter()
            .filter(|s| s.class == DecoratorClass::Normal)
            .count();
        let fusions = all.iter().filter(|s| s.fusion).count();
        // 11 基础 + 17 融合 = 28 修饰符装饰器；14 普通；共 42。
        assert_eq!(modifiers, 28, "修饰符装饰器应为 28 个");
        assert_eq!(normals, 14, "普通装饰器应为 14 个");
        assert_eq!(fusions, 17, "融合型应为 17 个");
        assert_eq!(all.len(), 42, "内置封闭集应为 42 个");
    }

    #[test]
    fn basic_lookup_axes() {
        let spec = DecoratorRegistry::lookup("mut").expect("mut 应存在");
        assert_eq!(spec.class, DecoratorClass::Modifier);
        assert!(!spec.fusion);
        assert_eq!(spec.axes.mutability, Mutability::Mut);
        assert!(spec.axes.origin.is_none());
    }

    #[test]
    fn fusion_lookup_axes() {
        let spec = DecoratorRegistry::lookup("mutex").expect("mutex 应存在");
        assert!(spec.fusion);
        assert_eq!(spec.axes.shared, SharedMode::Arc);
        assert_eq!(spec.axes.interior, InteriorMode::Mutex);
        assert_eq!(spec.axes.origin.as_deref(), Some("mutex"));
    }

    #[test]
    fn class_predicates() {
        assert!(DecoratorRegistry::is_modifier("lazy"));
        assert!(DecoratorRegistry::is_normal("export"));
        assert!(DecoratorRegistry::is_known("unsafe"));
        assert!(!DecoratorRegistry::is_modifier("export"));
        assert!(!DecoratorRegistry::is_normal("lazy"));
    }

    #[test]
    fn owend_is_not_known() {
        // AC13：拼写别名 `owend` 已删除，不再被识别。
        assert!(!DecoratorRegistry::is_known("owend"));
    }

    #[test]
    fn errmsg_helpers_embed_placeholders() {
        assert_eq!(
            errmsg::unknown("foo"),
            "未知装饰器 '@foo'（内置封闭集，不可扩展）"
        );
        assert_eq!(
            errmsg::axis_conflict("mut", "immut"),
            "修饰轴冲突：@mut 与 @immut 不可并存"
        );
        assert_eq!(
            errmsg::at_loc(&errmsg::at_most_one(), 3, 7),
            "每目标至多一个修饰符装饰器（如需组合多轴，请改用融合型）（位置 3:7）"
        );
        // 别名一致。
        assert_eq!(errmsg::INLINE, errmsg::MUST_INLINE);
        assert_eq!(errmsg::OWNLINE, errmsg::MUST_OWN_LINE);
    }
}

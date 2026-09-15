// Lang-Zong 编译器 — src/ast/modifier.rs
// 「修饰信息」轴集合：关键字形式（mut / ref / owned / const / comptime / let）与 `@` 装饰器
// 形式产出的**同一表示**（AC1/AC3）。设计真值：workbuddy/plan/2026-09-14-moddec-架构设计.md §3.2。
//
// 分层关系（信息流，无类型递归）：
//   ast::modifier::Modifiers  ←(类型)─  moddec::DecoratorRegistry（内置装饰器表）
//   ast::modifier::from_decorator_name  ─(查表)─►  moddec::DecoratorRegistry
// 说明：modifier 与 moddec 互为模块引用，但无循环**类型**依赖，Rust 允许。

/// 可变性轴：声明式可变性（`@mut` / `@immut` / 裸绑定 / `let`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub enum Mutability {
    /// 未显式指定。裸 `x = ...` 语义为可变，但该轴在未声明时保持 Implicit，
    /// 以保证「基础装饰器 ≡ 关键字」的产物一致性（AC1）。
    #[default]
    Implicit,
    /// 显式可变：`mut x = ...` / `@mut`。
    Mut,
    /// 不可变：`let x = ...` / `@immut`。
    Immut,
}

/// 共享模式轴：`@shared` 族（默认 `Rc`，见主理人裁决 O5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub enum SharedMode {
    /// 无共享。
    #[default]
    None,
    /// 单线程引用计数：`Rc<T>`。
    Rc,
    /// 原子引用计数：`Arc<T>`。
    Arc,
    /// 弱引用：`Weak<T>`。
    Weak,
}

/// 内部可变模式轴：`@cell` 族。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "infer", derive(serde::Serialize, serde::Deserialize))]
pub enum InteriorMode {
    /// 无内部可变。
    #[default]
    None,
    /// `Cell<T>`（`T: Copy`）。
    Cell,
    /// `RefCell<T>`（运行时借用检查）。
    RefCell,
    /// `Mutex<T>`（互斥锁）。
    Mutex,
    /// `RwLock<T>`（读写锁）。
    RwLock,
    /// `AtomicXxx`（原子标量）。
    Atomic,
}

/// 修饰信息：展开后的「轴集合」，每轴 ≤ 1（关键字路径与装饰器路径共用）。
///
/// 等价不变量（§8）：
/// - 基础装饰器 `origin == None` ⇒ 产物与关键字形式逐字节一致（AC1/AC3）。
/// - 融合型 `origin == Some(name)` ⇒ 展开为成分轴后与「显式轴组合」产物一致（AC11）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Modifiers {
    /// 可变性轴。
    pub mutability: Mutability,
    /// 是否引用绑定（`ref` / `@ref`）。
    pub is_ref: bool,
    /// 是否线性消费（`owned` / `@owned`）。
    pub owned: bool,
    /// 是否编译期常量（`const` / `@const`）。
    pub is_const: bool,
    /// 是否编译期求值（`comptime` / `@comptime`）。
    pub comptime: bool,
    /// 是否静态存储期（`@static`）。
    pub storage_static: bool,
    /// 共享模式轴（`@shared` 族）。
    pub shared: SharedMode,
    /// 内部可变模式轴（`@cell` 族）。
    pub interior: InteriorMode,
    /// 是否惰性初始化（`@lazy`）。
    pub lazy: bool,
    /// 实参是否为源码形态（`@source`）。
    pub source: bool,
    /// 融合型原名（仅诊断用）；基础装饰器 / 关键字 = `None`（保证 AC1 产物一致）。
    pub origin: Option<String>,
    /// 是否来自 `@` 装饰器形式（供 AC9 二选一校验与错误信息）。
    pub from_decorator: bool,
}

/// 修饰构造 / 合并错误。
#[derive(Debug, Clone, PartialEq)]
pub enum ModifierError {
    /// 未知装饰器名（内置封闭集之外，或非修饰符类装饰器）。
    UnknownDecorator(String),
    /// 同一轴冲突（如 `Mut` vs `Immut`、`Rc` vs `Arc`、`Cell` vs `Mutex`）。
    AxisConflict {
        /// 冲突所在的轴名（`mutability` / `shared` / `interior`）。
        axis: &'static str,
        /// 已存在的轴取值标签。
        previous: String,
        /// 新引入的轴取值标签。
        incoming: String,
    },
    /// `let` 与 `Mut` / `Ref` 语义冲突（AC9）。
    LetConflict {
        /// 与之冲突的装饰器名。
        name: String,
    },
    /// 每目标至多一个修饰符装饰器（AC8）。
    AtMostOne,
}

impl ModifierError {
    /// 生成统一诊断文案（§8）。
    pub fn to_message(&self) -> String {
        use crate::moddec::errmsg;
        match self {
            ModifierError::UnknownDecorator(name) => errmsg::unknown(name),
            ModifierError::AxisConflict {
                previous, incoming, ..
            } => errmsg::axis_conflict(previous, incoming),
            ModifierError::LetConflict { name } => errmsg::let_conflict(name),
            ModifierError::AtMostOne => errmsg::at_most_one(),
        }
    }
}

impl Modifiers {
    /// 空轴集合（全默认）。
    pub fn empty() -> Self {
        Self::default()
    }

    /// 迁移期兼容构造：由旧关键字布尔字段（`mut`/`let`/`ref`/`owned`/`const`/`comptime`）
    /// 派生轴集合，使「关键字路径」与「装饰器路径」产出**同一表示**（AC1/AC3）。
    /// `from_decorator` 保持 `false`（关键字来源）。
    pub fn from_keywords(
        mutable: bool,
        is_ref: bool,
        is_owned: bool,
        is_const: bool,
        comptime: bool,
    ) -> Self {
        Modifiers {
            mutability: if mutable {
                Mutability::Mut
            } else {
                Mutability::Immut
            },
            is_ref,
            owned: is_owned,
            is_const,
            comptime,
            ..Modifiers::empty()
        }
    }

    /// 由「装饰器名」构造（含融合展开）；未知名 / 非修饰符类 → `Err`。
    ///
    /// 展开表为单一真值来源：委托给 [`crate::moddec::DecoratorRegistry`]。
    pub fn from_decorator_name(
        name: &str,
        from_decorator: bool,
    ) -> Result<Modifiers, ModifierError> {
        match crate::moddec::DecoratorRegistry::lookup(name) {
            Some(spec) if spec.class == crate::moddec::DecoratorClass::Modifier => {
                let mut m = spec.axes.clone();
                m.from_decorator = from_decorator;
                Ok(m)
            }
            _ => Err(ModifierError::UnknownDecorator(name.to_string())),
        }
    }

    /// 合并两个轴集合（同轴 ≤ 1 校验；`let` 正交规则，AC8/AC9）。
    ///
    /// - 同轴冲突（如 `Mut` vs `Immut`、`Rc` vs `Arc`、`Cell` vs `Mutex`）→ [`ModifierError::AxisConflict`]。
    /// - `let`（不可变关键字）⊕ {`Mut`, `Ref`} → [`ModifierError::LetConflict`]（AC9）。
    /// - 布尔轴取并集（同真不冲突）；`origin` 取非空者；`from_decorator` 取或。
    pub fn merge(mut self, other: Modifiers) -> Result<Modifiers, ModifierError> {
        use Mutability::*;

        // AC9：`let`（不可变、来自关键字）⊕ {Mut, Ref} → LetConflict。
        let self_is_let = !self.from_decorator && self.mutability == Immut;
        let other_is_let = !other.from_decorator && other.mutability == Immut;
        if self_is_let && (other.mutability == Mut || other.is_ref) {
            return Err(ModifierError::LetConflict {
                name: other.repr_name(),
            });
        }
        if other_is_let && (self.mutability == Mut || self.is_ref) {
            return Err(ModifierError::LetConflict {
                name: self.repr_name(),
            });
        }

        // 逐轴合并（每轴 ≤ 1）。
        self.mutability = merge_mutability(self.mutability, other.mutability)?;
        self.shared = merge_shared(self.shared, other.shared)?;
        self.interior = merge_interior(self.interior, other.interior)?;

        // 布尔轴：并集语义（true 优先，同真不算冲突）。
        self.is_ref |= other.is_ref;
        self.owned |= other.owned;
        self.is_const |= other.is_const;
        self.comptime |= other.comptime;
        self.storage_static |= other.storage_static;
        self.lazy |= other.lazy;
        self.source |= other.source;

        // 诊断字段。
        if self.origin.is_none() {
            self.origin = other.origin;
        }
        self.from_decorator |= other.from_decorator;

        Ok(self)
    }

    /// 是否为空轴集合（全部轴为默认、无 `origin`）。
    pub fn is_empty(&self) -> bool {
        self.mutability == Mutability::Implicit
            && !self.is_ref
            && !self.owned
            && !self.is_const
            && !self.comptime
            && !self.storage_static
            && self.shared == SharedMode::None
            && self.interior == InteriorMode::None
            && !self.lazy
            && !self.source
            && self.origin.is_none()
    }

    /// 用于诊断的代表名：`origin` 优先，其次由已置位的轴推断。
    pub fn repr_name(&self) -> String {
        if let Some(o) = &self.origin {
            return o.clone();
        }
        if self.mutability == Mutability::Mut {
            return "mut".to_string();
        }
        if self.mutability == Mutability::Immut {
            return "immut".to_string();
        }
        if self.is_ref {
            return "ref".to_string();
        }
        if self.owned {
            return "owned".to_string();
        }
        if self.is_const {
            return "const".to_string();
        }
        if self.storage_static {
            return "static".to_string();
        }
        match self.shared {
            SharedMode::Rc => return "rc".to_string(),
            SharedMode::Arc => return "arc".to_string(),
            SharedMode::Weak => return "weak".to_string(),
            SharedMode::None => {}
        }
        match self.interior {
            InteriorMode::Cell => return "cell".to_string(),
            InteriorMode::RefCell => return "refcell".to_string(),
            InteriorMode::Mutex => return "mutex".to_string(),
            InteriorMode::RwLock => return "rwlock".to_string(),
            InteriorMode::Atomic => return "atomic".to_string(),
            InteriorMode::None => {}
        }
        if self.lazy {
            return "lazy".to_string();
        }
        if self.comptime {
            return "comptime".to_string();
        }
        if self.source {
            return "source".to_string();
        }
        "modifier".to_string()
    }
}

/// 可变性轴合并：`Implicit` 让位于显式值；`Mut` / `Immut` 互斥。
fn merge_mutability(a: Mutability, b: Mutability) -> Result<Mutability, ModifierError> {
    use Mutability::*;
    match (a, b) {
        (Implicit, y) => Ok(y),
        (x, Implicit) => Ok(x),
        (Mut, Mut) => Ok(Mut),
        (Immut, Immut) => Ok(Immut),
        (Mut, Immut) | (Immut, Mut) => Err(ModifierError::AxisConflict {
            axis: "mutability",
            previous: mutability_label(a).to_string(),
            incoming: mutability_label(b).to_string(),
        }),
    }
}

/// 共享模式轴合并：`None` 让位于显式值；`Rc` / `Arc` / `Weak` 互斥。
fn merge_shared(a: SharedMode, b: SharedMode) -> Result<SharedMode, ModifierError> {
    use SharedMode::*;
    match (a, b) {
        (None, y) => Ok(y),
        (x, None) => Ok(x),
        (x, y) if x == y => Ok(x),
        (x, y) => Err(ModifierError::AxisConflict {
            axis: "shared",
            previous: shared_label(x).to_string(),
            incoming: shared_label(y).to_string(),
        }),
    }
}

/// 内部可变模式轴合并：`None` 让位于显式值；各模式互斥。
fn merge_interior(a: InteriorMode, b: InteriorMode) -> Result<InteriorMode, ModifierError> {
    use InteriorMode::*;
    match (a, b) {
        (None, y) => Ok(y),
        (x, None) => Ok(x),
        (x, y) if x == y => Ok(x),
        (x, y) => Err(ModifierError::AxisConflict {
            axis: "interior",
            previous: interior_label(x).to_string(),
            incoming: interior_label(y).to_string(),
        }),
    }
}

/// 可变性轴取值标签（诊断用）。
fn mutability_label(m: Mutability) -> &'static str {
    match m {
        Mutability::Implicit => "implicit",
        Mutability::Mut => "mut",
        Mutability::Immut => "immut",
    }
}

/// 共享模式轴取值标签（诊断用）。
fn shared_label(s: SharedMode) -> &'static str {
    match s {
        SharedMode::None => "none",
        SharedMode::Rc => "rc",
        SharedMode::Arc => "arc",
        SharedMode::Weak => "weak",
    }
}

/// 内部可变模式轴取值标签（诊断用）。
fn interior_label(i: InteriorMode) -> &'static str {
    match i {
        InteriorMode::None => "none",
        InteriorMode::Cell => "cell",
        InteriorMode::RefCell => "refcell",
        InteriorMode::Mutex => "mutex",
        InteriorMode::RwLock => "rwlock",
        InteriorMode::Atomic => "atomic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_empty() {
        assert!(Modifiers::empty().is_empty());
        assert!(!Modifiers::from_decorator_name("mut", true)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn basic_decorator_equivalence() {
        // @mut ≡ mut：mutability=Mut，无其它轴，origin=None（AC1）。
        let m = Modifiers::from_decorator_name("mut", true).unwrap();
        assert_eq!(m.mutability, Mutability::Mut);
        assert!(!m.is_ref);
        assert!(!m.owned);
        assert!(m.origin.is_none());
        assert!(m.from_decorator);
    }

    #[test]
    fn fusion_expands_to_axes() {
        // @atomic → shared:Arc + interior:Atomic + origin:"atomic"（AC11）。
        let m = Modifiers::from_decorator_name("atomic", true).unwrap();
        assert_eq!(m.shared, SharedMode::Arc);
        assert_eq!(m.interior, InteriorMode::Atomic);
        assert_eq!(m.origin.as_deref(), Some("atomic"));
    }

    #[test]
    fn unknown_decorator_is_err() {
        assert!(Modifiers::from_decorator_name("magic_mod", true).is_err());
        // 普通装饰器不可作修饰符构造。
        assert!(Modifiers::from_decorator_name("export", true).is_err());
    }

    #[test]
    fn merge_axis_conflict() {
        let a = Modifiers::from_decorator_name("mut", true).unwrap();
        let b = Modifiers::from_decorator_name("immut", true).unwrap();
        match a.merge(b) {
            Err(ModifierError::AxisConflict { axis, .. }) => assert_eq!(axis, "mutability"),
            other => panic!("expected AxisConflict, got {other:?}"),
        }
    }

    #[test]
    fn merge_let_conflict() {
        let let_side = Modifiers {
            mutability: Mutability::Immut,
            from_decorator: false,
            ..Modifiers::empty()
        };
        let dec = Modifiers::from_decorator_name("mut", true).unwrap();
        match let_side.merge(dec) {
            Err(ModifierError::LetConflict { name }) => assert_eq!(name, "mut"),
            other => panic!("expected LetConflict, got {other:?}"),
        }
    }

    #[test]
    fn merge_orthogonal_is_ok() {
        // let(Immut) ⊕ @shared(Rc) 放行（AC9 正交）。
        let let_side = Modifiers {
            mutability: Mutability::Immut,
            from_decorator: false,
            ..Modifiers::empty()
        };
        let shared = Modifiers::from_decorator_name("shared", true).unwrap();
        let merged = let_side.merge(shared).unwrap();
        assert_eq!(merged.mutability, Mutability::Immut);
        assert_eq!(merged.shared, SharedMode::Rc);
    }

    #[test]
    fn merge_idempotent_same_axis() {
        let a = Modifiers::from_decorator_name("arc", true).unwrap();
        let b = Modifiers::from_decorator_name("arc", true).unwrap();
        let merged = a.merge(b).unwrap();
        assert_eq!(merged.shared, SharedMode::Arc);
    }
}

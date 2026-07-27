// Lang-Zong 编译器 — magic/engine.rs
// 魔法方法映射引擎：__xxx__ 魔法方法 → 自动生成 Rust trait impl
// 对齐 ready/20-魔法方法与自动trait.md

use std::collections::HashMap;

/// 魔法方法实现方式分类
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MagicKind {
    /// 二元运算符：self + rhs 均消费，Output 来自返回类型
    /// __add__(self, other: Rhs) → Ret → impl Add<Rhs> for Self { type Output = Ret; fn add(self, rhs: Rhs) → Ret }
    BinaryOp,

    /// 一元运算符：self 消费，Output 来自返回类型
    /// __neg__(self) → Ret → impl Neg for Self { type Output = Ret; fn neg(self) → Ret }
    UnaryOp,

    /// Drop trait：fn drop(&mut self)
    Drop,

    /// Default trait：fn default() → Self
    Default,

    /// Clone trait：fn clone(&self) → Self
    Clone,

    /// From trait：fn from(value: Source) → Self
    From,

    /// Into trait：fn into(self) → Target
    Into,

    /// Display trait：fn fmt(&self, f: &mut Formatter) → fmt::Result
    Display,

    /// Debug trait：fn fmt(&self, f: &mut Formatter) → fmt::Result
    Debug,

    /// Iterator trait：fn next(&mut self) → Option<Item>
    Iterator_,

    /// IntoIterator trait：fn into_iter(self) → Self::IntoIter
    IntoIterator_,

    /// 比较 PartialEq/Rhs：fn eq(&self, other: &Rhs) → bool
    PartialEq,

    /// 比较 PartialOrd/Rhs：fn partial_cmp(&self, other: &Rhs) → Option<Ordering>
    PartialOrd,

    /// 全序 Ord：fn cmp(&self, other: &Self) → Ordering
    Ord,

    /// Index trait：fn index(&self, index: Idx) → &Output
    Index,

    /// Hash trait：fn hash<H: Hasher>(&self, state: &mut H)
    Hash,
}

/// 魔法方法映射条目
pub struct MagicEntry {
    /// Rust trait 全限定路径
    pub trait_path: &'static str,
    /// trait 中的方法名
    pub trait_method: &'static str,
    /// 实现方式分类
    pub kind: MagicKind,
    /// 是否根据 other 参数类型多分派（同名魔法方法、不同 Rhs 类型 → 多个 impl）
    pub multi_dispatch: bool,
}

/// 魔法方法映射引擎
pub struct MagicEngine {
    mappings: HashMap<String, Vec<MagicEntry>>,
}

impl MagicEngine {
    /// 创建引擎，初始化所有映射规则
    pub fn new() -> Self {
        let mut engine = MagicEngine {
            mappings: HashMap::new(),
        };
        engine.init_default_mappings();
        engine
    }

    fn register(&mut self, magic: &str, entry: MagicEntry) {
        self.mappings.entry(magic.to_string()).or_default().push(entry);
    }

    fn init_default_mappings(&mut self) {
        // ═══════════════════════════════════════════
        // 一、签名固定型 — 算术/位运算符
        // self 和 rhs 均消费，Output 来自返回类型
        // ═══════════════════════════════════════════
        for (magic, trait_path, method) in &[
            ("__add__",  "std::ops::Add",    "add"),
            ("__sub__",  "std::ops::Sub",    "sub"),
            ("__mul__",  "std::ops::Mul",    "mul"),
            ("__div__",  "std::ops::Div",    "div"),
            ("__rem__",  "std::ops::Rem",    "rem"),
            ("__bitand__", "std::ops::BitAnd", "bitand"),
            ("__bitor__",  "std::ops::BitOr",  "bitor"),
            ("__bitxor__", "std::ops::BitXor", "bitxor"),
            ("__shl__",  "std::ops::Shl",    "shl"),
            ("__shr__",  "std::ops::Shr",    "shr"),
        ] {
            self.register(magic, MagicEntry {
                trait_path, trait_method: method,
                kind: MagicKind::BinaryOp,
                multi_dispatch: true,
            });
        }
        // __pow__ → 自定义 Pow trait
        self.register("__pow__", MagicEntry {
            trait_path: "Pow", trait_method: "pow",
            kind: MagicKind::BinaryOp,
            multi_dispatch: true,
        });
        // __pipe__ → 自定义 Pipe trait
        self.register("__pipe__", MagicEntry {
            trait_path: "Pipe", trait_method: "pipe",
            kind: MagicKind::BinaryOp,
            multi_dispatch: true,
        });

        // 一元运算符
        for (magic, trait_path, method) in &[
            ("__neg__",  "std::ops::Neg", "neg"),
            ("__not__",  "std::ops::Not", "not"),
        ] {
            self.register(magic, MagicEntry {
                trait_path, trait_method: method,
                kind: MagicKind::UnaryOp,
                multi_dispatch: false,
            });
        }

        // ═══════════════════════════════════════════
        // 二、签名受限型 — 比较
        // ═══════════════════════════════════════════
        self.register("__eq__", MagicEntry {
            trait_path: "std::cmp::PartialEq", trait_method: "eq",
            kind: MagicKind::PartialEq,
            multi_dispatch: true,
        });
        self.register("__ne__", MagicEntry {
            trait_path: "std::cmp::PartialEq", trait_method: "ne",
            kind: MagicKind::PartialEq,
            multi_dispatch: true,
        });
        self.register("__lt__", MagicEntry {
            trait_path: "std::cmp::PartialOrd", trait_method: "partial_cmp",
            kind: MagicKind::PartialOrd,
            multi_dispatch: true,
        });
        self.register("__le__", MagicEntry {
            trait_path: "std::cmp::PartialOrd", trait_method: "partial_cmp",
            kind: MagicKind::PartialOrd,
            multi_dispatch: true,
        });
        self.register("__gt__", MagicEntry {
            trait_path: "std::cmp::PartialOrd", trait_method: "partial_cmp",
            kind: MagicKind::PartialOrd,
            multi_dispatch: true,
        });
        self.register("__ge__", MagicEntry {
            trait_path: "std::cmp::PartialOrd", trait_method: "partial_cmp",
            kind: MagicKind::PartialOrd,
            multi_dispatch: true,
        });
        self.register("__cmp__", MagicEntry {
            trait_path: "std::cmp::Ord", trait_method: "cmp",
            kind: MagicKind::Ord,
            multi_dispatch: false,
        });
        self.register("__hash__", MagicEntry {
            trait_path: "std::hash::Hash", trait_method: "hash",
            kind: MagicKind::Hash,
            multi_dispatch: false,
        });
        self.register("__getitem__", MagicEntry {
            trait_path: "std::ops::Index", trait_method: "index",
            kind: MagicKind::Index,
            multi_dispatch: true,
        });

        // ═══════════════════════════════════════════
        // 三、签名受限型 — 显示/调试
        // ═══════════════════════════════════════════
        self.register("__str__", MagicEntry {
            trait_path: "std::fmt::Display", trait_method: "fmt",
            kind: MagicKind::Display,
            multi_dispatch: false,
        });
        self.register("__repr__", MagicEntry {
            trait_path: "std::fmt::Debug", trait_method: "fmt",
            kind: MagicKind::Debug,
            multi_dispatch: false,
        });

        // ═══════════════════════════════════════════
        // 四、签名自由型 — 类型转换
        // ═══════════════════════════════════════════
        self.register("__from__", MagicEntry {
            trait_path: "std::convert::From", trait_method: "from",
            kind: MagicKind::From,
            multi_dispatch: true,  // 按参数类型多分派
        });
        self.register("__into__", MagicEntry {
            trait_path: "std::convert::Into", trait_method: "into",
            kind: MagicKind::Into,
            multi_dispatch: true,  // 按返回类型多分派（罕见）
        });
        self.register("__try_from__", MagicEntry {
            trait_path: "std::convert::TryFrom", trait_method: "try_from",
            kind: MagicKind::From,
            multi_dispatch: true,
        });
        self.register("__try_into__", MagicEntry {
            trait_path: "std::convert::TryInto", trait_method: "try_into",
            kind: MagicKind::Into,
            multi_dispatch: true,
        });

        // ═══════════════════════════════════════════
        // 五、生命周期/资源
        // ═══════════════════════════════════════════
        self.register("__drop__", MagicEntry {
            trait_path: "std::ops::Drop", trait_method: "drop",
            kind: MagicKind::Drop,
            multi_dispatch: false,
        });
        self.register("__clone__", MagicEntry {
            trait_path: "std::clone::Clone", trait_method: "clone",
            kind: MagicKind::Clone,
            multi_dispatch: false,
        });
        self.register("__default__", MagicEntry {
            trait_path: "std::default::Default", trait_method: "default",
            kind: MagicKind::Default,
            multi_dispatch: false,
        });

        // ═══════════════════════════════════════════
        // 六、容器/迭代
        // ═══════════════════════════════════════════
        self.register("__next__", MagicEntry {
            trait_path: "std::iter::Iterator", trait_method: "next",
            kind: MagicKind::Iterator_,
            multi_dispatch: false,
        });
        self.register("__iter__", MagicEntry {
            trait_path: "std::iter::IntoIterator", trait_method: "into_iter",
            kind: MagicKind::IntoIterator_,
            multi_dispatch: false,
        });
    }

    /// 查询魔法方法对应的映射条目
    pub fn resolve(&self, magic_method: &str) -> Option<&Vec<MagicEntry>> {
        self.mappings.get(magic_method)
    }

    /// 是否是多分派方法（同名魔法方法按 other 类型可定义多个）
    pub fn is_multi_dispatch(&self, magic_method: &str) -> bool {
        self.mappings.get(magic_method)
            .map(|v| v.first().map(|e| e.multi_dispatch).unwrap_or(false))
            .unwrap_or(false)
    }
}

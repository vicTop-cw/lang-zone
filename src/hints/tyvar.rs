//! 推断变量与推断上下文（union-find 管理）
//!
//! 借鉴 rustc / ena 的并查集思路：每个类型变量是一个节点，
//! 通过 `parent` 链接形成等价类；绑定（bound）存储该等价类的代表所指向的具体类型。

use crate::types::def::Type;
/// 推断变量标识：`TypeVar` 来自 `types::def`，此处以 `TyVar` 别名暴露给 hints 库使用
pub use crate::types::def::TypeVar as TyVar;

/// 推断变量的链接状态
#[derive(Debug, Clone)]
enum Link {
    /// 指向另一个变量（union-find 路径压缩）
    Var(TyVar),
    /// 已绑定到具体类型（内部仍可含其它变量，待后续递归展开）
    Bound(Type),
    /// 尚未绑定，携带 let 泛化层级（level 供 P1 多态泛化使用）
    #[allow(dead_code)]
    Unbound(u32),
}

/// 类型推断上下文：持有全部类型变量，以 union-find 管理链接与绑定
#[derive(Debug, Clone, Default)]
pub struct InferCtx {
    links: Vec<Link>,
}

/// 类型推断错误
#[derive(Debug, Clone)]
pub enum TypeError {
    /// occurs-check 失败：变量出现在自身绑定中（无限类型）
    Occurs(TyVar, Type),
    /// 两类型无法统一
    Mismatch(Type, Type),
    /// 元数（构造器参数个数）不一致
    Arity(usize, usize),
    /// 引用了未绑定的标识符
    Unbound(String),
    /// 上层类型检查产生的通用消息（如 trait 实例解析失败）
    Message(String),
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeError::Occurs(v, t) =>
                write!(f, "occurs-check failed: type variable {:?} occurs in {:?}", v, t),
            TypeError::Mismatch(a, b) =>
                write!(f, "cannot unify {} with {}", a, b),
            TypeError::Arity(x, y) =>
                write!(f, "arity mismatch: expected {} arguments, found {}", x, y),
            TypeError::Unbound(n) =>
                write!(f, "unbound identifier: {}", n),
            TypeError::Message(msg) =>
                write!(f, "{}", msg),
        }
    }
}

impl InferCtx {
    /// 创建空推断上下文
    pub fn new() -> Self {
        Self { links: Vec::new() }
    }

    /// 分配一个新推断变量（level 用于后续 let 泛化）
    pub fn fresh(&mut self, level: u32) -> TyVar {
        let id = self.links.len() as u32;
        self.links.push(Link::Unbound(level));
        TyVar(id)
    }

    /// 便捷：分配变量并直接返回 `Type::Var`
    pub fn fresh_ty(&mut self, level: u32) -> Type {
        Type::Var(self.fresh(level))
    }

    /// union-find find（不可变版本，不含路径压缩）
    /// 用于仅需要读取规范变量、不需要写入的场景（prune/resolve/occurs）
    pub fn find(&self, v: TyVar) -> TyVar {
        let mut cur = v;
        loop {
            match &self.links[cur.0 as usize] {
                Link::Var(w) if *w != cur => cur = *w,
                _ => return cur,
            }
        }
    }

    /// union-find find（可变版本，带路径压缩）
    /// 查找时将路径上的节点直接指向根节点，后续查找 O(α(n))
    fn find_mut(&mut self, v: TyVar) -> TyVar {
        let cur = v;
        loop {
            match self.links[cur.0 as usize].clone() {
                Link::Var(w) if w != cur => {
                    let root = self.find_mut(w);
                    // 路径压缩：将当前节点直接指向根
                    self.links[cur.0 as usize] = Link::Var(root);
                    return root;
                }
                _ => return cur,
            }
        }
    }

    /// 将变量 v 绑定到类型 t（t 可为变量或具体类型）
    pub fn bind(&mut self, v: TyVar, t: Type) {
        let rv = self.find_mut(v);
        match t {
            Type::Var(w) => {
                let rw = self.find_mut(w);
                if rv == rw {
                    return; // 已在同一等价类
                }
                // 链接两个根（带路径压缩的 find_mut 保证后续查找 O(α(n))）
                self.links[rv.0 as usize] = Link::Var(rw);
            }
            other => {
                self.links[rv.0 as usize] = Link::Bound(other);
            }
        }
    }

    /// 跟随链接，返回类型变量的规范形式。
    ///
    /// 若变量已绑定到具体类型（`Link::Bound`），则递归解析该绑定（深度展开），
    /// 否则返回其规范变量标识（union-find 路径压缩后的代表）。
    /// 深度展开是统一算法正确性的前提：已解析为具体类型的变量在后续统一时
    /// 必须暴露其真实类型，才能触发 `Int` 与 `Bool` 这类不匹配错误，
    /// 而非被静默重新绑定。
    pub fn prune(&self, t: &Type) -> Type {
        match t {
            Type::Var(v) => {
                let rv = self.find(*v);
                match &self.links[rv.0 as usize] {
                    Link::Bound(bt) => self.prune(bt),
                    _ => Type::Var(rv),
                }
            }
            other => other.clone(),
        }
    }

    /// 完全解析单个变量：若为 Bound 返回其类型，否则 None（仍自由）
    pub fn resolve(&self, v: TyVar) -> Option<Type> {
        match &self.links[self.find(v).0 as usize] {
            Link::Bound(t) => Some(t.clone()),
            _ => None,
        }
    }

    /// occurs-check：类型 t 中是否出现变量 v（需跟随链接到规范形式）
    ///
    /// 这是防止 `α = [α]` 这类无限类型展开的根本保障。
    pub fn occurs(&self, v: TyVar, t: &Type) -> bool {
        let target = self.find(v);
        match t {
            Type::Var(w) => self.find(*w) == target,
            Type::Option(inner) | Type::Optional(inner)
            | Type::Ref(inner) | Type::MutRef(inner) =>
                self.occurs(v, inner),
            Type::Result { ok, err } =>
                self.occurs(v, ok) || self.occurs(v, err),
            Type::Generic { args, .. } | Type::Tuple(args) |
            Type::Union(args) | Type::Futures(args) |
            Type::Intersection(args) =>
                args.iter().any(|a| self.occurs(v, a)),
            Type::Record(fields) =>
                fields.iter().any(|(_, t)| self.occurs(v, t)),
            Type::Apply { constructor, args } =>
                self.occurs(v, constructor) || args.iter().any(|a| self.occurs(v, a)),
            Type::Fn { params, ret } =>
                params.iter().any(|p| self.occurs(v, p)) || self.occurs(v, ret),
            Type::Simd { elem, .. } | Type::Future(elem) =>
                self.occurs(v, elem),
            _ => false,
        }
    }
}

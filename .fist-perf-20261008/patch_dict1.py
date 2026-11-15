import io
p = "src/ir/codegen/owned_local.rs"
s = io.open(p, encoding="utf-8").read()

# 1) LocalKind 增加 Dict / Set
s = s.replace(
"""    /// `Vec<T>`：可 `push` / `extend`。
    List,""",
"""    /// `Vec<T>`：可 `push` / `extend`。
    List,
    /// `BTreeMap<K, V>`：可 `insert` / `extend`。
    Dict,
    /// `HashSet<T>`：可 `insert` / `extend`。
    Set,""", 1)

# 2) 追加形态枚举：把 ListAppend 扩成 ContainerAppend
s = s.replace(
"""/// 列表追加的落地形态。
pub(crate) enum ListAppend<'e> {
    /// 单元素列表字面量 → `push(e)`（零分配，优于 extend(vec![e])）。
    Push(&'e Expr),
    /// 多元素列表字面量 → `extend(vec![..])`。
    ExtendLit(Vec<String>),
    /// 其他表达式 → `extend(<expr>)`（与通用路径的实参文本一致）。
    ExtendExpr(&'e Expr),
}""",
"""/// 容器追加的落地形态（List / Dict / Set 共用）。
pub(crate) enum ContainerAppend<'e> {
    /// 单元素 → `push(e)`（列表，零分配，优于 `extend(vec![e])`）。
    Push(&'e Expr),
    /// 单条目 → `insert(k, v)`（Dict，零分配，优于「建一张 1 entry 的表再合并」）。
    Insert(&'e Expr, &'e Expr),
    /// 单元素 → `insert(e)`（Set，零分配）。
    InsertOne(&'e Expr),
    /// 多元素列表/集合字面量 → `extend([e, ..])`。
    ExtendItems(Vec<String>),
    /// 多条目字典字面量 → `extend([(k, v), ..])`。
    ExtendPairs(Vec<String>),
    /// 其他表达式 → `extend(<expr>)`（与通用路径的实参文本一致，move 语义不变）。
    ExtendExpr(&'e Expr),
}""", 1)

# 3) mark_list 之后加 mark_dict / mark_set
s = s.replace(
"""    /// 撤销登记（`ref` 绑定遮蔽同名 by-value 绑定时）。""",
"""    /// 登记 by-value 的字典局部绑定。
    pub(crate) fn owned_local_mark_dict(&self, name: &str) {
        self.owned_locals
            .borrow_mut()
            .insert(name.to_string(), LocalKind::Dict);
    }

    /// 登记 by-value 的集合局部绑定。
    pub(crate) fn owned_local_mark_set(&self, name: &str) {
        self.owned_locals
            .borrow_mut()
            .insert(name.to_string(), LocalKind::Set);
    }

    /// 撤销登记（`ref` 绑定遮蔽同名 by-value 绑定时）。""", 1)
io.open(p, "w", encoding="utf-8", newline="").write(s)
print("step1 ok")

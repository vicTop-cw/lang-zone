import io
p = "src/ir/codegen/owned_local.rs"
s = io.open(p, encoding="utf-8").read()

old_list = s[s.index("    /// `value` 是否形如 `name + <list>"):s.index("    /// 匹配「`<name> + <rhs>`")]
new_fn = '''    /// `value` 是否形如 `name + <容器>`（列表/字典/集合合并），是则给出落地形态。
    ///
    /// 通用路径对四类容器都发Rust 原生 `+`：
    /// - `Vec`：`{ let mut __cat = a.clone(); __cat.extend(..); __cat }`（每轮全量 clone）
    /// - `BTreeMap`/`HashSet`：`d = d + BTreeMap::from([(k, v)])` —— 每轮先**新建一张表**
    ///   再整表合并，字典/集合越大越慢（同样 O(n^2)）。
    /// 本函数把它们统一降级为 `insert` / `push` / `extend`。
    pub(crate) fn container_append_rhs<'e>(
        &self,
        name: &str,
        value: &'e Expr,
    ) -> Option<ContainerAppend<'e>> {
        let (lhs, rhs) = self.add_operands(name, value)?;
        match self.owned_local_kind(name)? {
            LocalKind::List => {
                if !is_list_ty(&lhs.ty) || !is_list_ty(&rhs.ty) {
                    return None;
                }
                match &rhs.kind {
                    // 列表字面量：单元素走 push（零分配），多元素走 extend([..])
                    ExprKind::ListLit(elems) if elems.len() == 1 => {
                        Some(ContainerAppend::Push(&elems[0]))
                    }
                    ExprKind::ListLit(elems) => {
                        let mut parts = Vec::with_capacity(elems.len());
                        for e in elems {
                            parts.push(self.gen_expr(e));
                        }
                        Some(ContainerAppend::ExtendItems(parts))
                    }
                    // 其他：实参文本与通用路径一致，move 语义不变
                    _ => Some(ContainerAppend::ExtendExpr(rhs)),
                }
            }
            LocalKind::Dict => {
                if !is_dict_ty(&lhs.ty) {
                    return None;
                }
                // Dict 字面量：StructCtor { name: "Dict", fields: [("_k0", k), ("_v0", v), ..] }
                if let ExprKind::StructCtor { name: n, fields } = &rhs.kind {
                    if n == "Dict" || n == "BTreeMap" || n == "HashMap" {
                        let mut pairs: Vec<(String, String)> = Vec::new();
                        let mut i = 0;
                        loop {
                            let key = fields.iter().find(|(n, _)| *n == format!("_k{}", i));
                            let val = fields.iter().find(|(n, _)| *n == format!("_v{}", i));
                            match (key, val) {
                                (Some((_, k)), Some((_, v))) => {
                                    pairs.push((self.gen_expr(k), self.gen_expr(v)));
                                }
                                _ => break,
                            }
                            i += 1;
                        }
                        if pairs.len() == 1 {
                            // 借用：pairs 里的文本已是 String，需回指不可行—— 改为直接用
                            // 渲染好的文本构造 insert语句（见 stmt_gen 的 ContainerAppend 分支）
                            return Some(ContainerAppend::ExtendPairs(
                                pairs.into_iter().map(|(k, v)| format!("({}, {})", k, v)).collect(),
                            ));
                        }
                        if pairs.len() > 1 {
                            return Some(ContainerAppend::ExtendPairs(
                                pairs.into_iter().map(|(k, v)| format!("({}, {})", k, v)).collect(),
                            ));
                        }
                        // 空 Dict 字面量：无条目可追加（等价于 no-op），交回通用路径
                        return None;
                    }
                }
                if !is_dict_ty(&rhs.ty) {
                    return None;
                }
                Some(ContainerAppend::ExtendExpr(rhs))
            }
            LocalKind::Set => {
                if !is_set_ty(&lhs.ty) {
                    return None;
                }
                // Set 字面量 `{a, b}` 被脱糖成 `set!(a, b)` 调用
                if let ExprKind::Call { callee, args, .. } = &rhs.kind {
                    if matches!(&callee.kind, ExprKind::Var(v) if v == "set!" || v == "set") {
                        if args.len() == 1 {
                            return Some(ContainerAppend::InsertOne(&args[0]));
                        }
                        if args.len() > 1 {
                            let mut parts = Vec::with_capacity(args.len());
                            for a in args {
                                parts.push(self.gen_expr(a));
                            }
                            return Some(ContainerAppend::ExtendItems(parts));
                        }
                        return None;
                    }
                }
                if !is_set_ty(&rhs.ty) {
                    return None;
                }
                Some(ContainerAppend::ExtendExpr(rhs))
            }
            LocalKind::ReadonlyBorrow | LocalKind::Str => None,
        }
    }

'''
s = s.replace(old_list, new_fn, 1)

# 类型判定补Dict / Set
s = s.replace(
"""/// 表达式本身是否产出字符串（字面量/ f-string / 字符串方法链）。""",
"""/// IR 类型是否为字典（BTreeMap / HashMap）。
pub(crate) fn is_dict_ty(ty: &IrType) -> bool {
    match ty {
        IrType::Named { path, .. } => {
            let p = path.as_str();
            p == "Dict" || p == "HashMap" || p == "BTreeMap"
        }
        _ => false,
    }
}

/// IR 类型是否为集合（HashSet）。
pub(crate) fn is_set_ty(ty: &IrType) -> bool {
    match ty {
        IrType::Named { path, .. } => {
            let p = path.as_str();
            p == "Set" || p == "HashSet"
        }
        _ => false,
    }
}

/// 表达式本身是否产出字符串（字面量/ f-string / 字符串方法链）。""", 1)
io.open(p, "w", encoding="utf-8", newline="").write(s)
print("step2 ok")

import io
p = "src/ir/codegen/owned_local.rs"
s = io.open(p, encoding="utf-8").read()
s = s.replace(
"""    /// 多条目字典字面量 → `extend([(k, v), ..])`。
    ExtendPairs(Vec<String>),""",
"""    /// 字典字面量的条目：单条目 → `insert(k, v)`（零分配），
    /// 多条目 → `extend([(k, v), ..])`。
    ExtendPairs(Vec<(String, String)>),""", 1)
old = """                        if pairs.len() == 1 {
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
                        return None;"""
new = """                        if pairs.is_empty() {
                            // 空 Dict 字面量：无条目可追加（等价于 no-op），交回通用路径
                            return None;
                        }
                        return Some(ContainerAppend::ExtendPairs(pairs));"""
assert old in s
s = s.replace(old, new, 1)
io.open(p, "w", encoding="utf-8", newline="").write(s)

p2 = "src/ir/codegen/stmt_gen.rs"
t = io.open(p2, encoding="utf-8").read()
old2 = """                                    owned_local::ContainerAppend::ExtendPairs(pairs) => {
                                        // Dict 字面量：单条目退化为 insert（零分配），
                                        // 多条目走 extend([(k, v), ..])
                                        if pairs.len() == 1 {
                                            self.emit_line(&format!(
                                                "{}.insert({});",
                                                safe_name, pairs[0]
                                            ));
                                        } else {
                                            self.emit_line(&format!(
                                                "{}.extend([{}]);",
                                                safe_name,
                                                pairs.join(", ")
                                            ));
                                        }
                                    }"""
new2 = """                                    owned_local::ContainerAppend::ExtendPairs(pairs) => {
                                        // Dict 字面量：单条目 → insert(k, v)（零分配，
                                        // 优于「建一张1-entry 的表再整表合并」）；
                                        // 多条目 → extend([(k, v), ..])
                                        if pairs.len() == 1 {
                                            let (k, v) = &pairs[0];
                                            self.emit_line(&format!(
                                                "{}.insert({}, {});",
                                                safe_name, k, v
                                            ));
                                        } else {
                                            let joined = pairs
                                                .iter()
                                                .map(|(k, v)| format!("({}, {})", k, v))
                                                .collect::<Vec<_>>()
                                                .join(", ");
                                            self.emit_line(&format!(
                                                "{}.extend([{}]);",
                                                safe_name, joined
                                            ));
                                        }
                                    }"""
assert old2 in t
t = t.replace(old2, new2, 1)
io.open(p2, "w", encoding="utf-8", newline="").write(t)
print("fixed")

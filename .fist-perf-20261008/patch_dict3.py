import io
p = "src/ir/codegen/stmt_gen.rs"
s = io.open(p, encoding="utf-8").read()

start = s.index("                        Some(owned_local::LocalKind::List) => {")
end = s.index("                        // 只读借用形参：上面已给出精确诊断并return，这里只是让")
new = '''                        Some(owned_local::LocalKind::List)
                        | Some(owned_local::LocalKind::Dict)
                        | Some(owned_local::LocalKind::Set) => {
                            if let Some(plan) = self.container_append_rhs(name, value) {
                                match plan {
                                    owned_local::ContainerAppend::Push(elem) => {
                                        let elem_s = self.gen_expr(elem);
                                        self.emit_line(&format!(
                                            "{}.push({});",
                                            safe_name, elem_s
                                        ));
                                    }
                                    owned_local::ContainerAppend::InsertOne(elem) => {
                                        // Set 单元素：零分配，优于「建 1-element HashSet 再整表合并」
                                        let elem_s = self.gen_expr(elem);
                                        self.emit_line(&format!(
                                            "{}.insert({});",
                                            safe_name, elem_s
                                        ));
                                    }
                                    owned_local::ContainerAppend::ExtendItems(parts) => {
                                        // 列表 / 集合字面量：`extend([..])` 直接吃数组，
                                        // 不必先构造 Vec/HashSet 临时值
                                        self.emit_line(&format!(
                                            "{}.extend([{}]);",
                                            safe_name,
                                            parts.join(", ")
                                        ));
                                    }
                                    owned_local::ContainerAppend::ExtendPairs(pairs) => {
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
                                    }
                                    owned_local::ContainerAppend::ExtendExpr(rhs) => {
                                        // 实参文本与通用路径完全一致 ⇒ move 语义不变
                                        // （后续再用该变量仍会编译失败，与改写前同构）
                                        let rhs_s = self.gen_expr(rhs);
                                        self.emit_line(&format!(
                                            "{}.extend({});",
                                            safe_name, rhs_s
                                        ));
                                    }
                                }
                                return;
                            }
                        }
'''
s = s[:start] + new + s[end:]

# 登记块补 Dict / Set
s = s.replace(
"""                } else if by_value && owned_local::is_list_ty(ty) {
                    self.owned_local_mark_list(&safe_name);
                } else if *is_ref {""",
"""                } else if by_value && owned_local::is_list_ty(ty) {
                    self.owned_local_mark_list(&safe_name);
                } else if by_value && owned_local::is_dict_ty(ty) {
                    self.owned_local_mark_dict(&safe_name);
                } else if by_value && owned_local::is_set_ty(ty) {
                    self.owned_local_mark_set(&safe_name);
                } else if *is_ref {""", 1)
io.open(p, "w", encoding="utf-8", newline="").write(s)
print("step3 ok")

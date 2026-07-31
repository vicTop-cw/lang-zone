// Lang-Zong 编译器 — codegen/stmt.rs
// CodeGenStmtExt trait 扩展

use super::CodeGen;
use crate::parser::*;
use std::collections::HashSet;
use super::expr::CodeGenExprExt;
use super::builders::CodeGenBuildersExt;


pub trait CodeGenStmtExt {
    fn gen_block(&self, stmts: &[Stmt], indent: usize, locals: &mut HashSet<String>) -> String;
    fn gen_stmt(&self, stmt: &Stmt, indent: usize, locals: &mut HashSet<String>) -> String;
}

impl CodeGenStmtExt for CodeGen {
    fn gen_block(&self, stmts: &[Stmt], indent: usize, locals: &mut HashSet<String>) -> String {
        let mut out = String::new();
        for stmt in stmts {
            out.push_str(&self.gen_stmt(stmt, indent, locals));
        }
        out
    }

    fn gen_stmt(&self, stmt: &Stmt, indent: usize, locals: &mut HashSet<String>) -> String {
        let pad = "    ".repeat(indent);
        match stmt {
            Stmt::Pass => {
                format!("{}();  // pass\n", pad)
            }

            Stmt::Expr(e) => {
                match e {
                    // 构建块语句：生成闭包表达式（变量构建块直接绑定到 lhs 变量名）
                    Expr::BuildBlock { kind, lhs, body } => {
                        let s = self.gen_build_block(*kind, lhs, body, indent, locals);
                        format!("{}{}\n", pad, s)
                    }
                    _ => format!("{}{};\n", pad, self.gen_expr(e)),
                }
            }

            Stmt::Let { name, mutable, is_ref, ty, value } => {
                // mutable=false → `let x = ...`；mutable=true → `let mut x = ...`
                // 作用域感知：若同名变量已在当前作用域声明过，则作为赋值（=）而非重新 let，
                // 避免循环/闭包体内变量被反复遮蔽、外层变量永不更新（无限循环 bug）。
                // 构建块作为 RHS 时不再 .clone()（它是闭包表达式，不可 clone）
                let val = self.gen_expr(value);
                if locals.contains(name) {
                    // 重赋值：不重新声明 let，直接 x = ...
                    let r = if *is_ref { "&" } else { "" };
                    format!("{}{}{} = {};\n", pad, r, name, val)
                } else {
                    let m = if *mutable { "mut " } else { "" };
                    let r = if *is_ref { "&" } else { "" };
                    let t = ty.as_ref()
                        .map(|t| format!(": {}", self.map_type(t)))
                        .unwrap_or_default();
                    locals.insert(name.clone());
                    format!("{}let {}{}{}{} = {};\n", pad, m, r, name, t, val)
                }
            }

            Stmt::Const { name, ty, value } => {
                // 函数体内 const 退化为 let mut（const item 在函数内需要显式类型，且生命周期受限）
                let t = ty.as_ref()
                    .map(|t| format!(": {}", self.map_type(t)))
                    .unwrap_or_default();
                locals.insert(name.clone());
                format!("{}let mut {}{} = {};\n", pad, name, t, self.gen_expr(value))
            }

            Stmt::Return(Some(e)) => {
                if self.in_gen.get() {
                    // 生成器构建块内：return（带值）先求值，再发出 IterStopException 停止信号
                    format!("{}let _ = {}; return IterStopException;\n", pad, self.gen_expr(e))
                } else if self.in_build_call.get() {
                    // 调用构建块内：return 的值即参数包，类型擦除为 __Pack
                    format!("{}return {};\n", pad, self.gen_pack_value(e, indent, locals))
                } else {
                    format!("{}return {};\n", pad, self.gen_expr(e))
                }
            }
            Stmt::Return(None) => {
                if self.in_gen.get() {
                    // 生成器构建块内：裸 return 自动返回 IterStopException（停止产出）
                    format!("{}return IterStopException;\n", pad)
                } else if self.in_build_call.get() {
                    // 调用构建块内：裸 return 视为空参数包
                    format!("{}return __Pack::Tuple(vec![]);\n", pad)
                } else {
                    format!("{}return;\n", pad)
                }
            }

            Stmt::Yield(Some(e)) => {
                if self.in_gen.get() {
                    // 生成器构建块内：yield 逐步产出参数包 → 类型擦除后推入收集器
                    format!("{}__bb.push({});\n", pad, self.gen_pack_value(e, indent, locals))
                } else {
                    format!("{}yield {};\n", pad, self.gen_expr(e))
                }
            }
            Stmt::Yield(None) => {
                if self.in_gen.get() {
                    // 空参数包
                    format!("{}__bb.push(__Pack::Tuple(vec![]));\n", pad)
                } else {
                    format!("{}yield;\n", pad)
                }
            }
            Stmt::YieldFrom(e) => {
                // yield from expr: 委托生成器迭代
                if self.in_gen.get() {
                    let inner = self.gen_expr(e);
                    format!("{}for __yf_val in {}.into_iter() {{ __bb.push(__yf_val); }}\n", pad, inner)
                } else {
                    let inner = self.gen_expr(e);
                    format!("{}// yield from {}\n", pad, inner)
                }
            }

            Stmt::While { cond, guard, body, else_body: _ } => {
                let body_s = self.gen_block(body, indent + 1, locals);
                match guard {
                    Some(g) => {
                        let pad_inc = "    ".repeat(indent + 1);
                        let guard_s = self.gen_expr(g);
                        format!("{}while {} {{\n{}if !({}) {{ break; }}\n{}{}}}\n",
                            pad, self.gen_expr(cond), pad_inc, guard_s, body_s, pad)
                    }
                    None => format!("{}while {} {{\n{}{}}}\n", pad, self.gen_expr(cond), body_s, pad),
                }
            }

            Stmt::For { var, iter, guard, body, else_body: _ } => {
                let iter_s = self.gen_expr(iter);
                let body_s = self.gen_block(body, indent + 1, locals);
                let var_decl = format!("mut {}", var);
                let inner = match guard {
                    Some(g) => {
                        let pad_inc = "    ".repeat(indent + 1);
                        format!("{}if !({}) {{ continue; }}\n{}", pad_inc, self.gen_expr(g), body_s)
                    }
                    None => body_s,
                };
                format!("{}for {} in {} {{\n{}{}}}\n", pad, var_decl, iter_s, inner, pad)
            }

            Stmt::Loop(body) => {
                let body_s = self.gen_block(body, indent + 1, locals);
                format!("{}loop {{\n{}{}}}\n", pad, body_s, pad)
            }

            Stmt::Break(None) => format!("{}break;\n", pad),
            Stmt::Break(Some(e)) => format!("{}break {};\n", pad, self.gen_expr(e)),
            Stmt::Continue => format!("{}continue;\n", pad),

            Stmt::Defer(body) => {
                let n = self.defer_count.get();
                self.defer_count.set(n + 1);
                let body_s = self.gen_block(body, indent + 1, locals);
                // 生成 __defer_N 守卫变量，Drop 时执行 defer 体
                // 使用 move 闭包 + FnOnce 确保捕获环境（包括可变引用）
                format!("{}let __defer_{} = DeferGuard(Some(|| {{\n{}{}}}));\n", pad, n, body_s, pad)
            }

            Stmt::Raise(expr) => {
                format!("{}return Err({});\n", pad, self.gen_expr(expr))
            }

            Stmt::Guard { cond, let_binding, else_body } => {
                // else 分支：单表达式自动包 return；多语句/块直接展开
                let else_s = if else_body.len() == 1 {
                    match &else_body[0] {
                        Stmt::Expr(e) => {
                            let val = match e {
                                Expr::StrLit(_) => format!("{}.to_string()", self.gen_expr(e)),
                                _ => self.gen_expr(e),
                            };
                            format!("{}    return {};\n", pad, val)
                        }
                        _ => self.gen_block(else_body, indent + 1, locals),
                    }
                } else {
                    self.gen_block(else_body, indent + 1, locals)
                };

                match (cond, let_binding) {
                    (Some(c), None) => {
                        // guard cond else: VALUE  →  if !(cond) { return VALUE; }
                        format!("{}if !({}) {{\n{}{}}}\n", pad, self.gen_expr(c), else_s, pad)
                    }
                    (None, Some((pat, expr))) => {
                        // guard let PATTERN = EXPR else: VALUE  →  let PATTERN = EXPR else { return VALUE; };
                        format!(
                            "{}let {} = {} else {{\n{}{}}};\n",
                            pad,
                            self.gen_pattern(pat),
                            self.gen_expr(expr),
                            else_s,
                            pad
                        )
                    }
                    _ => unreachable!("guard must have either cond or let_binding"),
                }
            }

            Stmt::With { expr, alias, body } => {
                // with Expr() as r: body → let r = Expr(); body; drop(r);
                let expr_s = self.gen_expr(expr);
                match alias {
                    Some(name) => {
                        let body_s = self.gen_block(body, indent + 1, locals);
                        format!("{}let mut {} = {};\n{}{}__exit__(&mut {});\n",
                            pad, name, expr_s,
                            body_s,
                            pad, name)
                    }
                    None => {
                        let body_s = self.gen_block(body, indent + 1, locals);
                        format!("{}{{ let __res = {};\n{}{}}}\n", pad, expr_s, body_s, pad)
                    }
                }
            }

            Stmt::Assign { target, op, value } => {
                let op_s = match op {
                    AssignOp::Eq => "=",
                    AssignOp::AddEq => "+=",
                    AssignOp::SubEq => "-=",
                    AssignOp::MulEq => "*=",
                    AssignOp::DivEq => "/=",
                    AssignOp::ModEq => "%=",
                    _ => "=",
                };
                // 普通赋值 = 默认拷贝；复合赋值 (+= 等) 保持 Rust 值语义（移动 RHS）
                // 构建块 RHS 不再 .clone()
                let val_s = if matches!(value, Expr::BuildBlock { .. }) {
                    self.gen_expr(value)
                } else {
                    self.gen_expr(value)
                };
                format!("{}{} {} {};\n", pad, self.gen_expr(target), op_s, val_s)
            }

            // 测试语句不应出现在普通函数体中，作为兜底生成空字符串
            Stmt::Test { .. } | Stmt::Suite { .. } => {
                String::new()
            }
            Stmt::Assert { expr, expected } => {
                if let Some(exp) = expected {
                    format!("{}assert_eq!({}, {});\n", pad, self.gen_expr(expr), self.gen_expr(exp))
                } else {
                    format!("{}assert!({});\n", pad, self.gen_expr(expr))
                }
            }
            Stmt::Check { expr, message } => {
                let msg = message.as_ref()
                    .map(|m| self.gen_expr(m))
                    .unwrap_or_else(|| format!("\"check failed: {}\"", self.gen_expr(expr)));
                format!("{}if !({}) {{ eprintln!(\"CHECK: {{}}\", {}); }}\n", pad, self.gen_expr(expr), msg)
            }
            Stmt::FnDef { func } => {
                // 嵌套函数已提升为模块级函数，此处生成 let 绑定指向 mangled 名称
                let parent = self.current_fn_name.borrow().clone().unwrap_or_default();
                let mangled = format!("{}_{}", parent, func.name);
                locals.insert(func.name.clone());
                format!("{}let {} = {};\n", pad, func.name, mangled)
            }
            Stmt::Comptime { body } => {
                // comptime: 块 — 在 Rust 中直接内联
                let mut out = String::new();
                out.push_str(&format!("{}// comptime block\n", pad));
                for stmt in body {
                    out.push_str(&self.gen_stmt(stmt, indent, locals));
                }
                out
            }

            Stmt::TypeAlias { name, ty } => {
                self.local_type_aliases.borrow_mut().push((name.clone(), ty.to_rust_type_string()));
                String::new()  // hoisted to module level, no inline output
            }

            Stmt::LetTuple { names, ty: _, value } => {
                // 解构绑定 → 临时变量 + 逐个字段提取
                let val_s = self.gen_expr(value);
                let tmp_name = format!("__lz_t{}", names.join("_"));
                let mut out = format!("{}let {} = {};\n", pad, tmp_name, val_s);
                for (i, name) in names.iter().enumerate() {
                    if *name != "_" {
                        out.push_str(&format!("{}let {} = {}.{};\n", pad, name, tmp_name, i));
                    }
                }
                out
            }
        }
    }


}

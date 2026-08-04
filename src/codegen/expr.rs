// Lang-Zong 编译器 — codegen/expr.rs
// CodeGenExprExt trait 扩展

use super::CodeGen;
use crate::parser::*;
use std::collections::HashSet;
use super::stmt::CodeGenStmtExt;
use super::builders::CodeGenBuildersExt;
use super::func::CodeGenFuncExt;
use super::helpers::{escape_str, gen_fstring};


pub trait CodeGenExprExt {
    fn gen_expr(&self, expr: &Expr) -> String;
    fn gen_pattern(&self, pat: &Pattern) -> String;
}

impl CodeGenExprExt for CodeGen {
    fn gen_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::IntLit(n) => n.to_string(),
            Expr::FloatLit(f) => {
                // Rust f64::to_string() 对 0.0 返回 "0"，需要确保有小数点
                let s = f.to_string();
                if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
                    s
                } else {
                    format!("{}.0", s)
                }
            }
            Expr::StrLit(s) => format!("\"{}\"", escape_str(s)),
            Expr::FStrLit(s) => gen_fstring(self, s),
            Expr::RawStrLit(s) => format!("r\"{}\"", s),
            Expr::BoolLit(b) => b.to_string(),
            Expr::NoneLit => {
                // 如果有自定义 Option 枚举，限定为 Option::None
                if let Some((_, enum_name)) = self.enum_variants.iter().find(|(v, _)| v == "None") {
                    format!("{}::None", enum_name)
                } else {
                    "None".to_string()
                }
            }
            Expr::Ident(name) => name.clone(),

            Expr::ListLit(elems) => {
                let items: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                format!("vec![{}]", items.join(", "))
            }

            Expr::DictLit(pairs) => {
                let entries: Vec<String> = pairs.iter()
                    .map(|(k, v)| format!("({}, {})", self.gen_expr(k), self.gen_expr(v)))
                    .collect();
                // 使用 vec! 而非数组字面量：[...].into_iter() 在本工具链会回退到切片迭代器（产出 & 引用），
                // 无法 collect 成 HashMap；vec![...].into_iter() 产出所有权元素，可正确 collect。
                format!("vec![{}].into_iter().collect()", entries.join(", "))
            }

            Expr::SetLit(elems) => {
                let items: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                format!("[{}].into_iter().collect()", items.join(", "))
            }

            Expr::TupleLit(elems) => {
                let items: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                format!("({})", items.join(", "))
            }

            Expr::Binary { left, op, right } => {
                let op_s = match op {
                    BinOp::Add => "+", BinOp::Sub => "-",
                    BinOp::Mul => "*", BinOp::Div => "/",
                    BinOp::Mod => "%", BinOp::Pow => "_pow_",
                    BinOp::Eq => "==", BinOp::Ne => "!=",
                    BinOp::Lt => "<", BinOp::Gt => ">",
                    BinOp::Le => "<=", BinOp::Ge => ">=",
                    BinOp::And => "&&", BinOp::Or => "||",
                    BinOp::BitAnd => "&", BinOp::BitOr => "|",
                    BinOp::BitXor => "^", BinOp::Shl => "<<", BinOp::Shr => ">>",
                    BinOp::In => "in", BinOp::Is => "is",
                };
                // 字符串拼接: "a" + "b" → format!("{}{}", a, b)（Rust 不允许 &str + &str）
                if *op == BinOp::Add && (is_str_like(left) || is_str_like(right)) {
                    return format!("format!(\"{{}}{{}}\", {}, {})",
                        self.gen_expr(left), self.gen_expr(right));
                }
                if *op == BinOp::Pow {
                    format!("({} as i64).pow({} as u32)", self.gen_expr(left), self.gen_expr(right))
                } else if *op == BinOp::In {
                    format!("{}.contains(&{})", self.gen_expr(right), self.gen_expr(left))
                } else {
                    format!("{} {} {}", self.gen_expr(left), op_s, self.gen_expr(right))
                }
            }

            Expr::Unary { op, operand } => {
                match op {
                    UnaryOp::Neg => {
                        // i64::MIN 的绝对值 9223372036854775808 超出 i64 正数范围，
                        // lexer 将其 wrapping 存储为 i64::MIN。一元负号在此为冗余，
                        // 直接输出字面量即可。
                        if let Expr::IntLit(n) = operand.as_ref() {
                            if *n == i64::MIN {
                                return format!("{}", n);
                            }
                        }
                        format!("(-{})", self.gen_expr(operand))
                    }
                    UnaryOp::Not => format!("(!{})", self.gen_expr(operand)),
                    UnaryOp::BitNot => format!("(!{})", self.gen_expr(operand)),
                }
            }

            Expr::Call { func, args, .. } => {
                // 实参：Rust 默认语义 — Copy自动复制，!Copy move
                let args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                let func_s = self.gen_expr(func);

                // print → println!
                if func_s == "print" {
                    if args.len() == 1 {
                        match &args[0] {
                            Expr::FStrLit(s) => {
                                // f"hello {name}" → println!("hello {}", name)
                                let fmt = gen_fstring(self, s);
                                // 从 format!("fmt", args) 提取为 println!("fmt", args)
                                if let Some(stripped) = fmt.strip_prefix("format!(") {
                                    return format!("println!({}", stripped);
                                }
                                return format!("println!({})", fmt);
                            }
                            Expr::StrLit(s) => {
                                return format!("println!(\"{}\")", escape_str(s));
                            }
                            _ => {
                                return format!("println!(\"{{}}\", {})", args_s[0]);
                            }
                        }
                    }
                    return format!("println!({})", args_s.join(", "));
                }

                // str(x) → format!("{}", x)（Rust 中 str 是原始类型，不是转换函数）
                if func_s == "str" && args.len() == 1 {
                    return format!("format!(\"{{}}\", {})", args_s[0]);
                }

                // int(x) → x as i64, float(x) → x as f64（Rust 中没有 int()/float() 函数）
                if func_s == "int" && args.len() == 1 {
                    return format!("({} as i64)", args_s[0]);
                }
                if func_s == "float" && args.len() == 1 {
                    return format!("({} as f64)", args_s[0]);
                }

                // ─── SIMD builtins ───
                if func_s == "simd_alloc" && args.len() >= 1 {
                    let count = &args_s[0];
                    let align = if args.len() >= 2 { args_s[1].as_str() } else { "32" };
                    return format!("unsafe {{ std::alloc::alloc(std::alloc::Layout::from_size_align({} * 16, {}).unwrap()) }}", count, align);
                }
                if func_s == "simd_free" && args.len() >= 2 {
                    let ptr = &args_s[0];
                    let count = &args_s[1];
                    let align = if args.len() >= 3 { args_s[2].as_str() } else { "32" };
                    return format!("unsafe {{ std::alloc::dealloc({}, std::alloc::Layout::from_size_align({} * 16, {}).unwrap()) }}", ptr, count, align);
                }

                // ─── 桥接层函数映射（resolve_call）───
                // 1) 裸函数名: exists("file")
                if let Expr::Ident(func_name) = func.as_ref() {
                    // 通过 Registry 统一路由（含 is_template / is_macro 检测）
                    if let Some(call_result) = self.registry.resolve_call_full(func_name, &args_s) {
                        if call_result.is_template {
                            return self.apply_call_template(&call_result.rust_path, &args_s);
                        }
                        let rust_path = if call_result.is_macro {
                            format!("{}!", call_result.rust_path.trim_end_matches('!'))
                        } else {
                            call_result.rust_path.clone()
                        };
                        return format!("{}({})", rust_path, args_s.join(", "));
                    }
                }

                // 2) 模块限定调用: fs::exists("file") → PathAccess.receiver=Ident("fs"), segment="exists"
                if let Expr::PathAccess { receiver, segment } = func.as_ref() {
                    // 仅处理 receiver 为 Ident 的简单情况（Module::func）
                    if let Expr::Ident(_mod_name) = receiver.as_ref() {
                        if let Some(call_result) = self.registry.resolve_call_full(segment, &args_s) {
                            if call_result.is_template {
                                return self.apply_call_template(&call_result.rust_path, &args_s);
                            }
                            let rust_path = if call_result.is_macro {
                                format!("{}!", call_result.rust_path.trim_end_matches('!'))
                            } else {
                                call_result.rust_path.clone()
                            };
                            return format!("{}({})", rust_path, args_s.join(", "));
                        }
                    }
                }

                // Struct 构造
                if let Expr::Ident(name) = func.as_ref() {
                    if let Some((_, fields)) = self.structs.iter().find(|(n, _)| n == name) {
                        if !fields.is_empty() {
                            let all_kw = args.iter().all(|a| matches!(a, Expr::KwArg { .. }));
                            if all_kw && args.len() == fields.len() {
                                // 关键字构造: User(name: "a", profile: Some(1))
                                let pairs: Vec<String> = args.iter().map(|a| {
                                    if let Expr::KwArg { name, value } = a {
                                        format!("{}: {}", name, self.gen_expr(value))
                                    } else { String::new() }
                                }).collect();
                                return format!("{} {{ {} }}", name, pairs.join(", "));
                            } else if !all_kw && fields.len() == args.len() {
                                // 位置构造: Point(1.0, 2.0)
                                let pairs: Vec<String> = fields.iter().zip(args_s.iter())
                                    .map(|(f, v)| format!("{}: {}", f, v))
                                    .collect();
                                return format!("{} {{ {} }}", name, pairs.join(", "));
                            }
                        }
                    }
                    // 枚举变体构造: Disk(...) → Shape::Disk(...)
                    if let Some((_, enum_name)) = self.enum_variants.iter().find(|(v, _)| v == name) {
                        if args.is_empty() {
                            return format!("{}::{}", enum_name, name);
                        } else {
                            return format!("{}::{}({})", enum_name, name, args_s.join(", "));
                        }
                    }
                    // Some(x) / Ok(x) / Err(x) 保持原样
                    if name == "Some" || name == "Ok" || name == "Err" {
                        return format!("{}({})", name, args_s.join(", "));
                    }
                }

                // 泛型类型构造器 T() → T::default()（无参数且非已知结构体/枚举/函数）
                if let Expr::Ident(name) = func.as_ref() {
                    if args.is_empty()
                        && !self.structs.iter().any(|(n, _)| n == name)
                        && !self.enum_variants.iter().any(|(v, _)| v == name)
                        && !self.fn_params.contains_key(name)
                        && !self.method_params.contains_key(name)
                        && name != "print" && name != "panic"
                        && name != "Some" && name != "Ok" && name != "Err"
                    {
                        return format!("{}::default()", name);
                    }
                }

                // 可变参数（末尾 List<T> 自动压栈）：调用方实参多于声明形参时，
                // 将多余实参打包为 vec![...]，作为最后一个参数传入
                if let Expr::Ident(name) = func.as_ref() {
                    if let Some(params) = self.fn_params.get(name) {
                        if !params.is_empty() && args.len() > params.len() {
                            let last_type = &params.last().unwrap().1;
                            if last_type.starts_with("Vec<") {
                                let keep = params.len() - 1;
                                let normal: Vec<String> = args_s[..keep].to_vec();
                                let extra: Vec<String> = args_s[keep..].to_vec();
                                let packed = format!("vec![{}]", extra.join(", "));
                                let mut all = normal;
                                all.push(packed);
                                return format!("{}({})", func_s, all.join(", "));
                            }
                        }
                    }
                }

                format!("{}({})", func_s, args_s.join(", "))
            }

            Expr::KwArg { name, value } => {
                format!("{}: {}", name, self.gen_expr(value))
            }

            Expr::MethodCall { receiver, method, args } => {
                // 枚举变体构造: MyError.NotFound("x") → MyError::NotFound("x")
                if let Expr::Ident(type_name) = receiver.as_ref() {
                    let is_known_type = self.structs.iter().any(|(n, _)| n == type_name);
                    let is_variant = self.enum_variants.iter().any(|(v, _)| v == method);
                    if is_known_type && is_variant {
                        let args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                        if args_s.is_empty() {
                            return format!("{}::{}", type_name, method);
                        }
                        return format!("{}::{}({})", type_name, method, args_s.join(", "));
                    }
                }
                let recv_s = self.gen_expr(receiver);
                let args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                // sqrt() → .sqrt()
                if method == "sqrt" && args.is_empty() {
                    return format!("({}).sqrt()", recv_s);
                }
                // 桥接层方法别名（如 append→push, length→len）
                let rust_method = self.registry.resolve_method(method, "");
                format!("{}.{}({})", recv_s, rust_method, args_s.join(", "))
            }

            Expr::FieldAccess { receiver, field } => {
                // 若 receiver 是已知类型名且 field 是枚举变体 → 生成 Enum::Variant
                // 否则保持标准的 field access: receiver.field
                if let Expr::Ident(type_name) = receiver.as_ref() {
                    let is_known_type = self.structs.iter().any(|(n, _)| n == type_name);
                    let is_variant = self.enum_variants.iter().any(|(v, _)| v == field);
                    if is_known_type && is_variant {
                        return format!("{}::{}", type_name, field);
                    }
                }
                format!("{}.{}", self.gen_expr(receiver), field)
            }

            Expr::PathAccess { receiver, segment } => {
                format!("{}::{}", self.gen_expr(receiver), segment)
            }

            Expr::Try(inner) => {
                // expr?  →  Rust try 运算符（Result/Option 传播）
                format!("({})?", self.gen_expr(inner))
            }

            Expr::Move(inner) => {
                // y^ 显式转移所有权：直出内部表达式（Rust 默认 move），不加 .clone()
                self.gen_expr(inner)
            }

            Expr::Index { receiver, index } => {
                format!("{}[{}]", self.gen_expr(receiver), self.gen_expr(index))
            }

            Expr::If { cond, then_body, elif_clauses, else_body } => {
                // 使用 gen_block_return 确保分支最后一条表达式不加分号（自动返回）
                // 分支体为表达式作用域，使用独立 locals 集合
                let mut locals = HashSet::new();
                let then_s = self.gen_block_return(then_body, 2, &mut locals);
                let mut out = format!("if {} {{\n{}    }}", self.gen_expr(cond), then_s);

                for (ec, eb) in elif_clauses {
                    let mut l = HashSet::new();
                    let eb_s = self.gen_block_return(eb, 2, &mut l);
                    out.push_str(&format!(" else if {} {{\n{}    }}", self.gen_expr(ec), eb_s));
                }

                if let Some(eb) = else_body {
                    let mut l = HashSet::new();
                    let eb_s = self.gen_block_return(eb, 2, &mut l);
                    out.push_str(&format!(" else {{\n{}    }}", eb_s));
                }

                out
            }

            Expr::Match { expr, arms } => {
                // __unapply__ 提取器: 检测 struct 解构模式
                let struct_arms: Vec<Option<(&str, &[Pattern])>> = arms.iter()
                    .map(|arm| self.is_struct_destructure(&arm.pattern))
                    .collect();
                let has_struct_arm = struct_arms.iter().any(|s| s.is_some());

                if has_struct_arm {
                    // 结构体解构: 绑定 scrutinee 到临时变量，用 if-else 链分发
                    let scrutinee = self.gen_expr(expr);
                    let mut out = format!("{{\n    let __match_val = {};\n", scrutinee);
                    for (i, arm) in arms.iter().enumerate() {
                        let chain = if i == 0 { "if" } else { " else if" };
                        let mut l = HashSet::new();
                        let body = self.gen_block_return(&arm.body, 2, &mut l);
                        if let Some((_, subs)) = struct_arms[i] {
                            let sub_pats: Vec<String> = subs.iter().map(|p| self.gen_pattern(p)).collect();
                            let destructure = format!("let ({}) = __match_val.__unapply__();", sub_pats.join(", "));
                            let guard_str = arm.guard.as_ref()
                                .map(|g| format!("\n        if {} {{", self.gen_expr(g)))
                                .unwrap_or_default();
                            let guard_close = if arm.guard.is_some() { "\n        }" } else { "" };
                            out.push_str(&format!(
                                "    {} true {{\n        {}{}\n        {}{}\n    }}",
                                chain, destructure, guard_str, body.trim_end(), guard_close
                            ));
                        } else {
                            // 非 struct 臂：wildcard pattern（此处所有臂都到了 if-else 链，因为 Rust 不允许重复 `_`）
                            let pat = self.gen_pattern(&arm.pattern);
                            let guard = arm.guard.as_ref()
                                .map(|g| format!(" if {}", self.gen_expr(g)))
                                .unwrap_or_default();
                            // 用 match 分发非 struct 臂
                            out.push_str(&format!(
                                "    {} match __match_val {{\n        {}{} => {},\n        _ => unreachable!()\n    }}",
                                chain, pat, guard, body.trim()
                            ));
                        }
                    }
                    out.push_str("\n}");
                    return out;
                }

                // 标准 match（无 struct 解构）
                let mut out = format!("match {} {{\n", self.gen_expr(expr));
                for arm in arms {
                    let pat = self.gen_pattern(&arm.pattern);
                    let guard = arm.guard.as_ref()
                        .map(|g| format!(" if {}", self.gen_expr(g)))
                        .unwrap_or_default();
                    let mut l = HashSet::new();
                    let body = self.gen_block_return(&arm.body, 3, &mut l);
                    let body_trimmed = body.trim();
                    if !body_trimmed.contains('\n') {
                        out.push_str(&format!("        {}{} => {},\n", pat, guard, body_trimmed));
                    } else {
                        out.push_str(&format!("        {}{} => {{\n{}}},\n", pat, guard, body));
                    }
                }
                out.push_str("    }");
                out
            }

            Expr::Closure { params, body } => {
                format!("|{}| {}", params.join(", "), self.gen_expr(body))
            }

            Expr::Range { start, end, inclusive } => {
                let s = start.as_ref().map(|e| self.gen_expr(e)).unwrap_or_default();
                let e = end.as_ref().map(|e| self.gen_expr(e)).unwrap_or_default();
                if *inclusive {
                    format!("{}..={}", s, e)
                } else {
                    format!("{}..{}", s, e)
                }
            }

            Expr::Pipe { receiver, func, args } => {
                // 函数线程管道：a |> f(args) ≡ f(a, args)
                let recv_s = self.gen_expr(receiver);
                let mut all_args = vec![recv_s];
                for a in args {
                    all_args.push(self.gen_expr(a));
                }
                format!("{}({})", func, all_args.join(", "))
            }

            Expr::Walrus { target, value } => {
                // x := expr → { x = expr; x }（栈/Copy 类型）/ { x = expr; x.clone() }（堆类型）
                let t = self.gen_expr(target);
                let v = self.gen_expr(value);
                // 检查目标变量类型：Copy 类型（栈）不 clone，非 Copy（堆）需 clone
                let needs_clone = match target.as_ref() {
                    Expr::Ident(name) => {
                        self.lookup_var_type(name)
                            .map(|ty| !Self::is_copy_type(&ty))
                            .unwrap_or(true) // 未知类型保守 clone
                    }
                    _ => true, // 非简单变量保守 clone
                };
                if needs_clone {
                    format!("{{ {} = {}; {}.clone() }}", t, v, t)
                } else {
                    format!("{{ {} = {}; {} }}", t, v, t)
                }
            }

            Expr::SafeNav { receiver, field } => {
                // Option 安全导航：a?.b ≡ a.map(|x| x.b)
                // 链式 a?.b?.c 自动嵌套为 a.map(|x| x.b).map(|x| x.c)
                format!("({}).map(|x| x.{})", self.gen_expr(receiver), field)
            }

            Expr::NullCoalesce { left, right } => {
                format!("({}).unwrap_or({})", self.gen_expr(left), self.gen_expr(right))
            }

            Expr::ListComprehension { output, var, iter, cond, .. } => {
                let iter_s = self.gen_expr(iter);
                let out_s = self.gen_expr(output);
                match cond {
                    Some(c) => format!(
                        "({}).into_iter().filter(|{}| {}).map(|{}| {}).collect::<Vec<_>>()",
                        iter_s, var, self.gen_expr(c), var, out_s
                    ),
                    None => format!(
                        "({}).into_iter().map(|{}| {}).collect::<Vec<_>>()",
                        iter_s, var, out_s
                    ),
                }
            }

            Expr::DictComprehension { key, value, var, iter, cond } => {
                let iter_s = self.gen_expr(iter);
                let k_s = self.gen_expr(key);
                let v_s = self.gen_expr(value);
                match cond {
                    Some(c) => format!(
                        "({}).into_iter().filter(|{}| {}).map(|{}| ({}, {})).collect::<std::collections::HashMap<_, _>>()",
                        iter_s, var, self.gen_expr(c), var, k_s, v_s
                    ),
                    None => format!(
                        "({}).into_iter().map(|{}| ({}, {})).collect::<std::collections::HashMap<_, _>>()",
                        iter_s, var, k_s, v_s
                    ),
                }
            }

            Expr::SetComprehension { elem, var, iter, cond } => {
                let iter_s = self.gen_expr(iter);
                let e_s = self.gen_expr(elem);
                match cond {
                    Some(c) => format!(
                        "({}).into_iter().filter(|{}| {}).map(|{}| {}).collect::<std::collections::HashSet<_>>()",
                        iter_s, var, self.gen_expr(c), var, e_s
                    ),
                    None => format!(
                        "({}).into_iter().map(|{}| {}).collect::<std::collections::HashSet<_>>()",
                        iter_s, var, e_s
                    ),
                }
            }

            Expr::Assign { target, op, value } => {
                let op_s = match op {
                    AssignOp::Eq => "=",
                    AssignOp::AddEq => "+=",
                    AssignOp::SubEq => "-=",
                    AssignOp::MulEq => "*=",
                    AssignOp::DivEq => "/=",
                    AssignOp::ModEq => "%=",
                    AssignOp::AndEq => "&=",
                    AssignOp::OrEq => "|=",
                    AssignOp::XorEq => "^=",
                    AssignOp::ShlEq => "<<=",
                    AssignOp::ShrEq => ">>=",
                    AssignOp::PowEq => "**=",
                };
                format!("({} {} {})", self.gen_expr(target), op_s, self.gen_expr(value))
            }

            Expr::Spawn(inner) => {
                format!("std::thread::spawn(move || {{ {} }})", self.gen_expr(inner))
            }

            Expr::Panic(inner) => {
                format!("panic!(\"{{}}\", {})", self.gen_expr(inner))
            }

            Expr::Await(inner) => {
                format!("{{ {} }}.await", self.gen_expr(inner))
            }

            Expr::TryCatch { body, catches, else_body, finally_body } => {
                // try: body_stmts; catch Pattern: handler; else: handler; finally: cleanup
                // → { let __try = match { body } { Err => ..., Ok => ... }; finally; __try }
                let mut tl = HashSet::new();
                let try_val = self.gen_block_return(body, 3, &mut tl);
                let mut match_out = format!("let __try_result = match {{\n{}\n        }} {{\n", try_val);
                for arm in catches {
                    let pat = self.gen_pattern(&arm.pattern);
                    let guard = arm.guard.as_ref()
                        .map(|g| format!(" if {}", self.gen_expr(g)))
                        .unwrap_or_default();
                    let mut l = HashSet::new();
                    let cb = self.gen_block_return(&arm.body, 3, &mut l);
                    let cb_trimmed = cb.trim();
                    let err_pat = format!("Err({})", pat);
                    if !cb_trimmed.contains('\n') {
                        match_out.push_str(&format!("            {}{} => {},\n", err_pat, guard, cb_trimmed));
                    } else {
                        match_out.push_str(&format!("            {}{} => {{\n{}}},\n", err_pat, guard, cb));
                    }
                }
                if let Some(else_stmts) = else_body {
                    let mut l = HashSet::new();
                    let eb = self.gen_block_return(else_stmts, 3, &mut l);
                    let eb_trimmed = eb.trim();
                    if !eb_trimmed.contains('\n') {
                        match_out.push_str(&format!("            Ok(__v) => {},\n", eb_trimmed));
                    } else {
                        match_out.push_str(&format!("            Ok(__v) => {{\n{}}},\n", eb));
                    }
                } else {
                    match_out.push_str("            Ok(v) => v,\n");
                }
                match_out.push_str("        };");

                if let Some(finally_stmts) = finally_body {
                    let mut fl = HashSet::new();
                    let fb = self.gen_block(finally_stmts, 2, &mut fl);
                    format!("{{\n{}\n{}\n        __try_result\n    }}", match_out, fb)
                } else {
                    // 无 finally 时直接返回 match（保持原有行为）
                    // 还原为简单的 match 表达式（去掉 let __try_result = ）
                    let simple = match_out
                        .replace("let __try_result = match {\n", "match {\n")
                        .replace("        };", "    }");
                    simple
                }
            }

            Expr::BuildBlock { kind, lhs, body } => {
                // 罕见：构建块作为嵌套表达式（如函数实参）。使用空作用域与 0 缩进兜底；
                // 常规路径（语句级）由 gen_stmt 调用 gen_build_block 并传入正确 indent/locals。
                let mut locals = HashSet::new();
                self.gen_build_block(*kind, lhs, body, 0, &mut locals)
            }
            Expr::Paren(inner) => self.gen_expr(inner),
        }
    }

    /// 生成构建块：本质为无参闭包，内部默认 unsafe（指针语法作用域限定块内）。
    /// - Var(=:):  let NAME = (|| unsafe { 块体（尾部表达式自动返回） })();
    /// - Call(~:): let NAME = { let __p = (|| unsafe { 块体（返回参数包） })(); f(<args>) };
    ///             args 由参数包形状决定：元组→位置实参；其余→整体 f(__p)
    /// - Gen(*:):  let NAME = { let mut __bb = Vec::new();
    ///             (|| unsafe { 块体（yield→push 参数包, return/正常结束→IterStopException） })();
    ///             结果：__bb.into_iter().map(move |__p| f(__p)) —— 惰性迭代器，对每个参数包调用 f };
    fn gen_pattern(&self, pat: &Pattern) -> String {
        match pat {
            Pattern::Int(n) => n.to_string(),
            Pattern::Str(s) => format!("\"{}\"", escape_str(s)),
            Pattern::Bool(b) => b.to_string(),
            Pattern::Ident(s) => s.clone(),
            Pattern::Wildcard => "_".to_string(),
            Pattern::Variant(name, sub) => {
                // 限定枚举变体名: Disk → Shape::Disk
                let qualified = if let Some((_, enum_name)) = self.enum_variants.iter().find(|(v, _)| v == name) {
                    format!("{}::{}", enum_name, name)
                } else {
                    name.clone()
                };
                if sub.is_empty() {
                    qualified
                } else {
                    let subs: Vec<String> = sub.iter().map(|p| self.gen_pattern(p)).collect();
                    format!("{}({})", qualified, subs.join(", "))
                }
            }
            Pattern::Tuple(elems) => {
                let subs: Vec<String> = elems.iter().map(|p| self.gen_pattern(p)).collect();
                format!("({})", subs.join(", "))
            }
        }
    }
}

/// 判断表达式是否为字符串类型（用于字符串拼接检测）
fn is_str_like(expr: &Expr) -> bool {
    matches!(expr, Expr::StrLit(_) | Expr::FStrLit(_))
}

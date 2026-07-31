// Lang-Zong 编译器 — codegen/func.rs
// CodeGenFuncExt trait 扩展

use super::CodeGen;
use crate::parser::*;
use std::collections::HashSet;
use crate::types::Type;
use super::stmt::CodeGenStmtExt;
use super::builders::CodeGenBuildersExt;
use super::helpers::gen_decorator_attr;
use super::helpers::{has_decorator, apply_parallel_transforms};
use super::expr::CodeGenExprExt;


pub trait CodeGenFuncExt {
    fn gen_function(&self, f: &Function) -> String;
    fn gen_test_stmt(&self, stmt: &Stmt, indent: usize) -> String;
    fn gen_suite_test(&self, stmt: &Stmt, indent: usize, setup: Option<&[Stmt]>, teardown: Option<&[Stmt]>) -> String;
    fn gen_method(&self, f: &Function, indent: usize) -> String;
    fn gen_block_return(&self, stmts: &[Stmt], indent: usize, locals: &mut HashSet<String>) -> String;
    fn gen_param(&self, p: &Param) -> String;
    #[allow(dead_code)]
    fn gen_stmt_body(&self, stmts: &[Stmt], indent: usize) -> String;
}

impl CodeGenFuncExt for CodeGen {
    fn gen_function(&self, f: &Function) -> String {
        let mut out = String::new();

        // 设置当前函数名（�� gen_stmt 处理 Stmt::FnDef 时使用）
        self.current_fn_name.replace(Some(f.name.clone()));

        // @export(Rust) -> #[no_mangle] pub fn
        let has_export = has_decorator(f, "export");
        let visibility = if has_export { "#[no_mangle]\npub " } else { "" };
        let has_memoize = has_decorator(f, "memoize");

        // decorators
        for d in &f.decorators {
            if d.name == "export" { continue; }
            out.push_str(&gen_decorator_attr(d));
        }

        let async_kw = if f.is_async { "async " } else { "" };
        let generics = if f.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", f.generics.join(", "))
        };

        let params: Vec<String> = f.params.iter()
            .map(|p| self.gen_param(p))
            .collect();

        let ret = if let Some(ref raises_ty) = f.raises {
            let inner = f.return_type.as_ref()
                .map(|t| self.map_type(t))
                .unwrap_or_else(|| "()".to_string());
            format!(" -> Result<{}, {}>", inner, self.map_type(raises_ty))
        } else {
            f.return_type.as_ref()
                .map(|t| format!(" -> {}", self.map_type(t)))
                .unwrap_or_default()
        };

        let where_str = if f.where_clause.is_empty() {
            String::new()
        } else {
            let bounds: Vec<String> = f.where_clause.iter()
                .map(|b| {
                    let bounds_s: Vec<String> = b.bounds.iter()
                        .map(|bound| self.map_type(bound))
                        .collect();
                    // Any 作为 trait bound 需要 'static 约束
                    let has_any = b.bounds.iter().any(|bound| matches!(bound, Type::Any));
                    let extra = if has_any { " + 'static" } else { "" };
                    format!("{}: {}{}", b.type_param, bounds_s.join(" + "), extra)
                })
                .collect();
            format!("\nwhere {}", bounds.join(", "))
        };

        let mut locals: HashSet<String> = f.params.iter().map(|p| p.name.clone()).collect();
        self.defer_count.set(0);  // 每函数重置 defer 计数器
        let mut body = self.gen_block_return(&f.body, 1, &mut locals);
        
        // raises: 函数体末尾表达式自动包 Ok(...)；无尾表达式补 Ok(())
        if f.raises.is_some() {
            let trimmed = body.trim_end();
            // 检查最后一行是否以 ; 或 } 结尾（说明是语句，非尾表达式）
            let last_line = trimmed.lines().last().unwrap_or("");
            let needs_ok = last_line.ends_with(';') || last_line.ends_with('}') || last_line.is_empty();
            if needs_ok {
                // 无尾表达式 -> 补 Ok(())
                body = format!("{}    Ok(())\n", trimmed);
            } else {
                // 尾表达式 -> 包 Ok(...)
                // 提取最后一行的缩进
                let indent = last_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                let expr = last_line.trim();
                // 重建: 去掉最后一行，加上 Ok(wrapped)
                let mut lines: Vec<&str> = trimmed.lines().collect();
                lines.pop();
                let prefix = if lines.is_empty() { String::new() } else { format!("{}\n", lines.join("\n")) };
                body = format!("{}{}Ok({})\n", prefix, indent, expr);
            }
        }
        
        // 顶层构建块：在 main() 函数开头注入初始化代码
        if f.name == "main" && !self.top_level_builds.is_empty() {
            let mut init_lines = String::new();
            for (name, build_body) in &self.top_level_builds {
                let mut build_locals = HashSet::new();
                let build_s = self.gen_block_return(build_body, 2, &mut build_locals);
                let build_s = build_s.trim_end();
                init_lines.push_str(&format!(
                    "    let {} = (|| unsafe {{\n{}\n    }})();\n",
                    name, build_s
                ));
            }
            body = format!("    // Top-level build blocks\n{}{}", init_lines, body);
        }
        
        // @parallel：替换迭代器为并行版本
        let body = if has_decorator(f, "parallel") {
            apply_parallel_transforms(&body)
        } else {
            body
        };
        let body = body.trim_end();

        // @memoize wrapper: add OnceLock<HashMap<>> cache for single-arg functions
        let body = if has_memoize && f.params.len() == 1 {
            let param_name = &f.params[0].name;
            let param_ty = self.map_type(&f.params[0].ty);
            let ret_ty = f.return_type.as_ref()
                .map(|t| self.map_type(t))
                .unwrap_or_else(|| "()".to_string());
            let cache_name = format!("__MEMOIZE_{}", f.name.to_uppercase());
            // Wrap original body in a closure to capture result
            let inner_body = if body.is_empty() { "()".to_string() } else { format!("(|| {{ {} }})()", body) };
            format!(
                "    static {}: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<{}, {}>>> = std::sync::OnceLock::new();\n    let cache = {}.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));\n    {{\n        let lock = cache.lock().unwrap();\n        if let Some(v) = lock.get(&{}) {{ return v.clone(); }}\n    }}\n    let result = {};\n    cache.lock().unwrap().insert({}, result.clone());\n    result",
                cache_name, param_ty, ret_ty, cache_name, param_name, inner_body, param_name
            )
        } else {
            body.to_string()
        };

        out.push_str(&format!(
            "{}{}fn {}{}({}){}{} {{\n{}\n}}\n",
            async_kw, visibility, f.name, generics,
            params.join(", "), ret, where_str, body
        ));

        // 重置当前函数名
        self.current_fn_name.replace(None);

        out
    }

    fn gen_test_stmt(&self, stmt: &Stmt, indent: usize) -> String {
        match stmt {
            Stmt::Test { name, body } => {
                let pad = "    ".repeat(indent);
                let fn_name = name.replace(' ', "_").to_lowercase();
                let mut out = format!("{}#[test]\n{}fn {}() {{\n", pad, pad, fn_name);
                let mut locals = HashSet::new();
                for s in body {
                    out.push_str(&self.gen_stmt(s, indent + 1, &mut locals));
                }
                out.push_str(&format!("{}}}\n", pad));
                out
            }
            Stmt::Assert { expr, expected } => {
                let pad = "    ".repeat(indent);
                if let Some(expected_expr) = expected {
                    format!("{}assert_eq!({}, {});\n", pad,
                        self.gen_expr(expr), self.gen_expr(expected_expr))
                } else {
                    format!("{}assert!({});\n", pad, self.gen_expr(expr))
                }
            }
            Stmt::Suite { name, setup, teardown, tests: suite_tests } => {
                let pad = "    ".repeat(indent);
                let mod_name = name.replace(' ', "_").to_lowercase();
                let mut out = format!("{}mod {} {{\n{}    use super::*;\n\n", pad, mod_name, pad);
                for t in suite_tests {
                    out.push_str(&self.gen_suite_test(t, indent + 1, setup.as_deref(), teardown.as_deref()));
                }
                out.push_str(&format!("{}}}\n", pad));
                out
            }
            _ => String::new(),
        }
    }

    /// 生成套件内的单个测试（含 setup/teardown 内联）
    fn gen_suite_test(&self, stmt: &Stmt, indent: usize, setup: Option<&[Stmt]>, teardown: Option<&[Stmt]>) -> String {
        match stmt {
            Stmt::Test { name, body } => {
                let pad = "    ".repeat(indent);
                let fn_name = name.replace(' ', "_").to_lowercase();
                let mut out = format!("{}#[test]\n{}fn {}() {{\n", pad, pad, fn_name);
                let mut locals = HashSet::new();
                // 内联 setup
                if let Some(setup_stmts) = setup {
                    out.push_str(&format!("{}    // === setup ===\n", pad));
                    for s in setup_stmts {
                        out.push_str(&self.gen_stmt(s, indent + 1, &mut locals));
                    }
                }
                // 测试体
                for s in body {
                    out.push_str(&self.gen_stmt(s, indent + 1, &mut locals));
                }
                // 内联 teardown
                if let Some(teardown_stmts) = teardown {
                    out.push_str(&format!("{}    // === teardown ===\n", pad));
                    for s in teardown_stmts {
                        out.push_str(&self.gen_stmt(s, indent + 1, &mut locals));
                    }
                }
                out.push_str(&format!("{}}}\n", pad));
                out
            }
            _ => self.gen_test_stmt(stmt, indent),
        }
    }

    /// 生成语句块（无特殊尾表达式处理，纯语句）
    fn gen_stmt_body(&self, stmts: &[Stmt], indent: usize) -> String {
        let mut out = String::new();
        let mut locals = HashSet::new();
        for s in stmts {
            out.push_str(&self.gen_stmt(s, indent, &mut locals));
        }
        out
    }

    fn gen_method(&self, f: &Function, indent: usize) -> String {
        let pad = "    ".repeat(indent);

        // 设置当前函数名
        self.current_fn_name.replace(Some(f.name.clone()));

        let async_kw = if f.is_async { "async " } else { "" };
        let generics = if f.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", f.generics.join(", "))
        };

        let params: Vec<String> = f.params.iter()
            .map(|p| self.gen_param(p))
            .collect();

        let ret = f.return_type.as_ref()
            .map(|t| format!(" -> {}", self.map_type(t)))
            .unwrap_or_default();

        let where_str = if f.where_clause.is_empty() {
            String::new()
        } else {
            let bounds: Vec<String> = f.where_clause.iter()
                .map(|b| {
                    let bounds_s: Vec<String> = b.bounds.iter()
                        .map(|bound| self.map_type(bound))
                        .collect();
                    let has_any = b.bounds.iter().any(|bound| matches!(bound, Type::Any));
                    let extra = if has_any { " + 'static" } else { "" };
                    format!("{}: {}{}", b.type_param, bounds_s.join(" + "), extra)
                })
                .collect();
            format!(" where {}", bounds.join(", "))
        };

        let mut locals: HashSet<String> = f.params.iter().map(|p| p.name.clone()).collect();
        self.defer_count.set(0);  // 每方法重置 defer 计数器
        let body = self.gen_block_return(&f.body, indent + 1, &mut locals);
        let body = body.trim_end();

        // 重置当前函数名
        self.current_fn_name.replace(None);

        format!(
            "{}{}fn {}{}({}){}{} {{\n{}\n{}}}\n",
            pad, async_kw, f.name, generics,
            params.join(", "), ret, where_str, body, pad
        )
    }

    /// 生成函数体，最后一条表达式语句不加分号（实现自动返回）
    fn gen_block_return(&self, stmts: &[Stmt], indent: usize, locals: &mut HashSet<String>) -> String {
        let mut out = String::new();
        let n = stmts.len();
        for (i, stmt) in stmts.iter().enumerate() {
            let is_last = i == n - 1;
            if is_last {
                // 最后一条语句：如果是普通 Expr，不加分号；构建块则走 gen_stmt 以保留缩进与作用域
                match stmt {
                    Stmt::Expr(e) if !matches!(e, Expr::BuildBlock { .. }) => {
                        let pad = "    ".repeat(indent);
                        // 调用构建块(~:) 内：尾部表达式即参数包，需经类型擦除为 __Pack
                        let s = if self.in_build_call.get() {
                            self.gen_pack_value(e, indent, locals)
                        } else {
                            self.gen_expr(e)
                        };
                        out.push_str(&format!("{}{}\n", pad, s));
                    }
                    _ => {
                        out.push_str(&self.gen_stmt(stmt, indent, locals));
                    }
                }
            } else {
                out.push_str(&self.gen_stmt(stmt, indent, locals));
            }
        }
        out
    }

    fn gen_param(&self, p: &Param) -> String {
        // self 特殊处理：Lang-Zong 默认借用
        if p.name == "self" {
            if p.is_mut {
                return "&mut self".to_string();
            } else if p.is_owned {
                return "self".to_string();
            } else {
                return "&self".to_string();
            }
        }

        let prefix = if p.is_ref {
            "&".to_string()
        } else if p.is_mut {
            "mut ".to_string()
        } else {
            String::new()
        };

        let ty = self.map_type(&p.ty);
        let default = p.default.as_ref()
            .map(|d| format!(" = {}", self.gen_expr(d)))
            .unwrap_or_default();
        format!("{}{}: {}{}", prefix, p.name, ty, default)
    }

}

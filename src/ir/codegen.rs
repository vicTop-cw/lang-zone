// Lang-Zone 编译器 — ir/codegen.rs
// LZIR → Rust 源代码生成器
//
// 职责：
// 1. 将 IrModule 转换为合法的 Rust 源代码字符串
// 2. 类型映射：IrType → Rust 类型（如 Option→Option, List→Vec）
// 3. 生成完整的、可编译的 .rs 文件

use super::node::*;
use super::types::IrType;
use super::IrModule;
use std::collections::HashMap;

/// IR → Rust 代码生成器
pub struct CodeGen {
    /// 缩进级别（空格数）
    indent: usize,
    /// 类型映射表：LZ 类型名 → Rust 类型名
    type_map: HashMap<&'static str, &'static str>,
    /// 当前函数返回类型
    current_ret_ty: Option<IrType>,
    is_main: bool,
    declared: std::collections::HashSet<String>,
    emitted_types: std::collections::HashSet<String>,
    /// enum variant → enum name 映射（用于构造器调用路由）
    enum_variants: HashMap<String, String>,
    /// 抑制尾表达式隐式 return（用于 match arm / 块表达式内部）
    suppress_tail_return: bool,
    /// 函数名 → (总参数数, 默认参数数)（用于调用时自动填充 None）
    fn_param_info: HashMap<String, (usize, usize)>,
    buf: String,
}

impl CodeGen {
    pub fn new() -> Self {
        let mut type_map = HashMap::new();
        type_map.insert("List", "Vec");
        type_map.insert("Dict", "HashMap");
        type_map.insert("Set", "HashSet");
        type_map.insert("String", "String");
        type_map.insert("Nil", "()");
        type_map.insert("Unit", "()");
        type_map.insert("Range", "std::ops::Range<i64>");
        type_map.insert("RangeInclusive", "std::ops::RangeInclusive<i64>");
        // 基础类型保持原样
        CodeGen {
            indent: 0,
            type_map,
            current_ret_ty: None,
            is_main: false,
            declared: std::collections::HashSet::new(),
            emitted_types: std::collections::HashSet::new(),
            enum_variants: HashMap::new(),
            suppress_tail_return: false,
            fn_param_info: HashMap::new(),
            buf: String::new(),
        }
    }

    // ── 入口 ──

    /// 将整个 IrModule 生成为 Rust 源代码
    pub fn generate(&mut self, module: &IrModule) -> String {
        self.buf.clear();
        self.indent = 0;

        // 预扫描：收集所有 enum variant → enum name 映射
        self.enum_variants.clear();
        self.fn_param_info.clear();
        for item in &module.items {
            if let Item::EnumDef(e) = item {
                for variant in &e.variants {
                    self.enum_variants.insert(variant.name.clone(), e.name.clone());
                }
            }
            if let Item::FnDef(f) = item {
                let default_count = f.params.iter().filter(|p| p.default.is_some()).count();
                if default_count > 0 {
                    self.fn_param_info.insert(f.name.clone(), (f.params.len(), default_count));
                }
            }
        }

        // 标准 prelude
        self.emit_prelude();

        // 每个顶层 item
        for item in &module.items {
            self.gen_item(item);
            self.buf.push('\n');
        }

        std::mem::take(&mut self.buf)
    }

    // ── 辅助方法 ──

    fn pad(&self) -> String {
        "    ".repeat(self.indent)
    }

    #[allow(dead_code)]
    fn emit(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    fn emit_line(&mut self, s: &str) {
        self.buf.push_str(&self.pad());
        self.buf.push_str(s);
        self.buf.push('\n');
    }

    fn emit_prelude(&mut self) {
        self.emit_line("#![allow(unused_imports)]");
        self.emit_line("#![allow(unused_variables)]");
        self.emit_line("#![allow(dead_code)]");
        self.buf.push('\n');
        self.emit_line("use std::collections::{HashMap, HashSet};");
        self.buf.push('\n');
    }

    // ── 类型映射 ──

    fn rust_type_name(&self, name: &str) -> String {
        match name {
            "int" => "i64".into(),
            "float" | "f64" => "f64".into(),
            "str" => "String".into(),
            "bool" => "bool".into(),
            "List" => "Vec".into(),
            "Dict" => "HashMap".into(),
            "Set" => "HashSet".into(),
            other => other.to_string(),
        }
    }

    fn rust_type(&self, ty: &IrType) -> String {
        match ty {
            IrType::Int => "i64".into(),
            IrType::F64 => "f64".into(),
            IrType::Str => "String".into(),
            IrType::Bool => "bool".into(),
            IrType::Unit => "()".into(),
            IrType::Never => "!".into(),
            IrType::Any => "i64".into(),
            IrType::Self_ => "Self".into(),
            IrType::Duck { .. } => "()".into(),  // Duck types: cannot determine Rust type, use unit
            IrType::Named { path, args } => {
                let mapped = self.type_map.get(path.as_str()).map(|s| s.to_string())
                    .unwrap_or_else(|| path.clone());
                if args.is_empty() {
                    // Vec/HashMap/HashSet 需要默认泛型参数
                    if path == "List" || path == "Vec" {
                        format!("{}<_>", mapped)
                    } else if path == "Dict" || path == "HashMap" {
                        format!("{}<_, _>", mapped)
                    } else if path == "Set" || path == "HashSet" {
                        format!("{}<_>", mapped)
                    } else {
                        mapped
                    }
                } else {
                    let args: Vec<String> = args.iter().map(|a| self.rust_type(a)).collect();
                    format!("{}<{}>", mapped, args.join(", "))
                }
            }
            IrType::Option(inner) => {
                format!("Option<{}>", self.rust_type(inner))
            }
            IrType::Result { ok, err } => {
                format!("Result<{}, {}>", self.rust_type(ok), self.rust_type(err))
            }
            IrType::Tuple(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.rust_type(e)).collect();
                format!("({})", elems.join(", "))
            }
            IrType::Fn { params, ret } => {
                let params: Vec<String> = params.iter().map(|p| self.rust_type(p)).collect();
                format!("fn({}) -> {}", params.join(", "), self.rust_type(ret))
            }
            IrType::Ref(inner) => format!("&{}", self.rust_type(inner)),
            IrType::MutRef(inner) => format!("&mut {}", self.rust_type(inner)),
            IrType::Generic(name) => name.clone(),
        }
    }

    // ── Item 生成 ──

    fn gen_item(&mut self, item: &Item) {
        match item {
            Item::FnDef(f) => {
                // 检测方法定义语法 `fn X.method()` → 生成 impl X { fn method() }
                if let Some((ty_name, _method_name)) = f.name.split_once('.') {
                    // 收集所有同类型的方法定义（因 gen_item 逐个调用，此处按需即时生成 impl）
                    self.emit_line(&format!("impl {} {{", ty_name));
                    self.indent += 1;
                    // 临时替换函数名为纯方法名
                    let mut mf = f.clone();
                    mf.name = f.name.split('.').last().unwrap_or(&f.name).to_string();
                    // 方法在 impl 块内不需要 pub
                    self.gen_fn_def(&mf);
                    self.indent -= 1;
                    self.emit_line("}");
                    self.buf.push('\n');
                } else {
                    self.gen_fn_def(f);
                }
            }
            Item::StructDef(s) => self.gen_struct_def(s),
            Item::EnumDef(e) => self.gen_enum_def(e),
            Item::TraitDef(t) => self.gen_trait_def(t),
            Item::Impl(i) => self.gen_impl_def(i),
            Item::Use(u) => self.gen_use_stmt(u),
            Item::Const(c) => self.gen_const_def(c),
            Item::Test(t) => self.gen_test_def(t),
        }
    }

    fn gen_fn_def(&mut self, f: &FnDef) {
        self.declared.clear();
        // 检测 duck 参数 → 自动注入泛型类型
        let duck_params: Vec<String> = f.params.iter().enumerate()
            .filter(|(_, p)| matches!(&p.ty, IrType::Duck { .. }))
            .map(|(i, _)| format!("DuckParam{}", i))
            .collect();
        let duck_indices: Vec<usize> = f.params.iter().enumerate()
            .filter(|(_, p)| matches!(&p.ty, IrType::Duck { .. }))
            .map(|(i, _)| i)
            .collect();

        let has_ducks = !duck_params.is_empty();
        let is_math = f.intrinsics.iter().any(|intr| matches!(&intr.kind, IntrinsicKind::Export(targets) if targets.iter().any(|t| t == "Math")));

        let generics = if has_ducks {
            let base = self.gen_generics(&f.generics);
            if base.is_empty() {
                format!("<{}>", duck_params.join(", "))
            } else {
                format!("<{}, {}>", base.trim_matches(|c| c == '<' || c == '>'), duck_params.join(", "))
            }
        } else if is_math && !f.generics.is_empty() {
            // @math: 泛型名直接用
            self.gen_generics(&f.generics)
        } else {
            self.gen_generics(&f.generics)
        };

        // @math where 子句：每个泛型参数都需要算术 trait bounds
        let math_where = if is_math && !f.generics.is_empty() {
            let clauses: Vec<String> = f.generics.iter().map(|g| {
                format!("    {}: std::ops::Add<Output={}> + std::ops::Mul<Output={}> + Copy", g.name, g.name, g.name)
            }).collect();
            if clauses.is_empty() {
                String::new()
            } else {
                format!("\nwhere\n{}", clauses.join(",\n"))
            }
        } else {
            String::new()
        };

        let params: Vec<String> = f.params.iter().enumerate().map(|(i, p)| {
            if duck_indices.contains(&i) {
                let idx = duck_indices.iter().position(|&d| d == i).unwrap();
                format!("{}: {}", p.name, duck_params[idx])
            } else {
                if p.default.is_some() {
                    // 默认参数 → Option<T>（函数签名）
                    format!("{}: Option<{}>", p.name, self.rust_type(&p.ty))
                } else if p.is_mut {
                    format!("mut {}: {}", p.name, self.rust_type(&p.ty))
                } else {
                    format!("{}: {}", p.name, self.rust_type(&p.ty))
                }
            }
        }).collect();
        let has_yield = block_has_yield(&f.body);
        let ret = if f.name == "main" {
            String::new()  // Rust main always returns ()
        } else if has_yield {
            format!(" -> Vec<{}>", self.rust_type(&f.ret_ty))
        } else if f.ret_ty != IrType::Unit {
            let ret_ty_str = match &f.ret_ty {
                IrType::Fn { params, ret } => {
                    let p: Vec<String> = params.iter().map(|p| self.rust_type(p)).collect();
                    format!("impl Fn({}) -> {}", p.join(", "), self.rust_type(ret))
                }
                _ => self.rust_type(&f.ret_ty),
            };
            format!(" -> {}", ret_ty_str)
        } else {
            String::new()
        };
        let _async_kw = if f.is_async { "async " } else { "" };
        let is_method = f.params.first().map_or(false, |p| p.name == "self");
        let vis = if is_method { "" } else { "pub " };

        let sig = format!(
            "{}{}fn {}{}({}){}{}",
            if f.is_test { "#[test]\n" } else { "" },
            vis,
            f.name,
            generics,
            params.join(", "),
            ret,
            math_where,
        );

        self.emit_line(&format!("{} {{", sig));
        self.indent += 1;

        // 生成器：body 包含 Yield → prepend __gen_vec
        if has_yield {
            self.emit_line("let mut __gen_vec = Vec::new();");
        }

        // 默认参数 unwrap: greet(name: str = "World") → let name = name.unwrap_or_else(|| "World".to_string());
        for p in &f.params {
            if let Some(ref default_val) = p.default {
                let def_s = self.gen_expr(default_val);
                self.emit_line(&format!("let {} = {}.unwrap_or_else(|| {});", p.name, p.name, def_s));
            }
        }

        // 函数体
        self.current_ret_ty = Some(f.ret_ty.clone());
        self.is_main = f.name == "main";
        self.gen_block_inner(&f.body);
        self.current_ret_ty = None;
        self.is_main = false;

        // 生成器：追加 return __gen_vec
        if has_yield {
            self.emit_line("return __gen_vec;");
        }

        self.indent -= 1;
        self.emit_line("}");
    }

    fn gen_struct_def(&mut self, s: &StructDef) {
        if self.emitted_types.contains(&s.name) { return; }
        self.emitted_types.insert(s.name.clone());

        let generics = self.gen_generics(&s.generics);
        self.emit_line(&format!("pub struct {}{} {{", s.name, generics));
        self.indent += 1;
        for field in &s.fields {
            self.emit_line(&format!("pub {}: {},", field.name, self.rust_type(&field.ty)));
        }
        self.indent -= 1;
        self.emit_line("}");

        // 方法（impl 块）
        if !s.methods.is_empty() {
            self.buf.push('\n');
            self.emit_line(&format!("impl{} {} {{", generics, s.name));
            self.indent += 1;
            for m in &s.methods {
                self.gen_fn_def(m);
                self.buf.push('\n');
            }
            self.indent -= 1;
            self.emit_line("}");
        }
    }

    fn gen_enum_def(&mut self, e: &EnumDef) {
        // 去重：同名 enum 已生成则跳过
        if self.emitted_types.contains(&e.name) { return; }
        self.emitted_types.insert(e.name.clone());

        let generics = self.gen_generics(&e.generics);
        self.emit_line(&format!("#[derive(Debug, Clone, PartialEq)]"));
        self.emit_line(&format!("pub enum {}{} {{", e.name, generics));
        self.indent += 1;
        for variant in &e.variants {
            if variant.fields.is_empty() {
                self.emit_line(&format!("{},", variant.name));
            } else {
                let types: Vec<String> = variant.fields.iter().map(|f| {
                    let mut rust_ty = self.rust_type(&f.ty);
                    // 递归枚举字段自动 Box
                    if type_refers_to(&f.ty, &e.name) {
                        rust_ty = format!("Box<{}>", rust_ty);
                    }
                    rust_ty
                }).collect();
                self.emit_line(&format!("{}({}),", variant.name, types.join(", ")));
            }
        }
        self.indent -= 1;
        self.emit_line("}");
    }

    fn gen_trait_def(&mut self, t: &TraitDef) {
        let generics = self.gen_generics(&t.generics);
        let supertraits = if t.supertraits.is_empty() {
            String::new()
        } else {
            let st: Vec<String> = t.supertraits.iter().map(|s| self.rust_type(s)).collect();
            format!(": {}", st.join(" + "))
        };
        self.emit_line(&format!("pub trait {}{}{} {{", t.name, generics, supertraits));
        self.indent += 1;
        for sig in &t.methods {
            let params: Vec<String> = sig.params.iter().map(|p| self.rust_type(p)).collect();
            let ret = if sig.ret != IrType::Unit {
                format!(" -> {}", self.rust_type(&sig.ret))
            } else {
                String::new()
            };
            self.emit_line(&format!("fn {}({}){};", sig.name, params.join(", "), ret));
        }
        self.indent -= 1;
        self.emit_line("}");
    }

    fn gen_impl_def(&mut self, i: &ImplDef) {
        let generics = self.gen_generics(&i.generics);
        let trait_part = i.trait_.as_ref()
            .map(|t| format!("{} for ", self.rust_type(t)))
            .unwrap_or_default();
        self.emit_line(&format!("impl{} {}{} {{", generics, trait_part, self.rust_type(&i.for_type)));
        self.indent += 1;
        for m in &i.methods {
            self.gen_fn_def(m);
            self.buf.push('\n');
        }
        self.indent -= 1;
        self.emit_line("}");
    }

    fn gen_use_stmt(&mut self, u: &UseStmt) {
        let path = u.path.join("::");
        if u.is_from {
            self.emit_line(&format!("use {}::{{{}}};", path, u.items.join(", ")));
        } else {
            self.emit_line(&format!("use {};", path));
        }
    }

    fn gen_const_def(&mut self, c: &ConstDef) {
        // const 不支持 .to_string()，直接用 &str
        let (ty_str, val_str) = match &c.ty {
            IrType::Str => {
                if let ExprKind::Lit(LitKind::Str(s)) = &c.value.kind {
                    let escaped = s.escape_default().to_string();
                    ("&str".into(), format!("\"{}\"", escaped))
                } else {
                    (self.rust_type(&c.ty), self.gen_expr(&c.value))
                }
            }
            _ => (self.rust_type(&c.ty), self.gen_expr(&c.value)),
        };
        self.emit_line(&format!("const {}: {} = {};", c.name, ty_str, val_str));
    }

    fn gen_test_def(&mut self, t: &TestDef) {
        self.emit_line("#[test]");
        self.emit_line(&format!("fn {}() {{", t.name));
        self.indent += 1;
        self.gen_block_inner(&t.body);
        self.indent -= 1;
        self.emit_line("}");
    }

    // ── 泛型 ──

    fn gen_generics(&self, g: &[GenericParam]) -> String {
        if g.is_empty() {
            return String::new();
        }
        let params: Vec<String> = g.iter().map(|p| {
            let mut s = p.name.clone();
            if !p.bounds.is_empty() {
                let bounds: Vec<String> = p.bounds.iter().map(|b| self.rust_type(b)).collect();
                s.push_str(&format!(": {}", bounds.join(" + ")));
            }
            if let Some(ref def) = p.default {
                s.push_str(&format!(" = {}", self.rust_type(def)));
            }
            s
        }).collect();
        format!("<{}>", params.join(", "))
    }

    fn gen_param(&self, p: &Param) -> String {
        if p.name == "self" {
            // self → &self / &mut self / self 取决于 is_mut + ty ref修饰
            match (&p.ty, p.is_mut) {
                (IrType::Self_, true) => "&mut self".into(),
                (IrType::Self_, false) => "&self".into(),
                (IrType::MutRef(_), _) => "&mut self".into(),
                (IrType::Ref(_), _) => "&self".into(),
                _ => format!("self: {}", self.rust_type(&p.ty)),
            }
        } else {
            // duck 类型参数 — 代码生成层用 `_` 占位，语义校验在编译期完成
            // 实际 Rust 输出不包含 duck 字段约束
            if matches!(&p.ty, IrType::Duck { .. }) {
                format!("{}: T_DUCK_{}", p.name, p.name.to_uppercase())
            } else {
                format!("{}: {}", p.name, self.rust_type(&p.ty))
            }
        }
    }

    // ── Block / Stmt 生成 ──

    fn gen_block_inner(&mut self, block: &Block) {
        let n = block.stmts.len();
        for (i, stmt) in block.stmts.iter().enumerate() {
            let is_last = i == n - 1;
            self.gen_stmt(stmt, is_last);
        }
    }

    fn gen_stmt(&mut self, stmt: &Stmt, is_last: bool) {
        match stmt {
            Stmt::Let { name, ty, value, is_mut } => {
                // LZ: Let{is_mut:true} = 无 let 关键字的赋值
                //   - 首次出现: "let mut x = val"
                //   - 已声明过: "x = val"（纯赋值）
                if *is_mut && self.declared.contains(name) {
                    self.emit_line(&format!("{} = {};", name, self.gen_expr(value)));
                    return;
                }
                self.declared.insert(name.clone());
                let mut_kw = if *is_mut { "mut " } else { "" };
                let skip_ty = *ty == IrType::Any || *ty == IrType::Unit
                    || matches!(ty, IrType::Duck { .. })
                    || if let IrType::Named { path, args } = ty { path == "Range" || args.is_empty() } else { false };
                let ty_str = if skip_ty {
                    String::new()
                } else {
                    format!(": {}", self.rust_type(ty))
                };
                self.emit_line(&format!("let {}{}{} = {};", mut_kw, name, ty_str, self.gen_expr(value)));
            }
            Stmt::Assign { target, value } => {
                self.emit_line(&format!("{} = {};", self.gen_expr(target), self.gen_expr(value)));
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    self.emit_line(&format!("return {};", self.gen_expr(v)));
                } else {
                    self.emit_line("return;");
                }
            }
            Stmt::ExprStmt { expr } => {
                if is_last && !self.is_main && !self.suppress_tail_return {
                    // 非 main 函数尾表达式 → return expr;
                    self.emit_line(&format!("return {};", self.gen_expr(expr)));
                } else if is_last && self.suppress_tail_return {
                    // match arm / 块表达式尾值 → 裸表达式（无分号，作为块值）
                    self.emit_line(&format!("{}", self.gen_expr(expr)));
                } else if is_last {
                    // main 函数尾表达式 → expr;
                    self.emit_line(&format!("{};", self.gen_expr(expr)));
                } else {
                    self.emit_line(&format!("{};", self.gen_expr(expr)));
                }
            }
            Stmt::If { cond, then_branch, else_branch } => {
                if let Some(else_blk) = else_branch {
                    self.emit_line(&format!("if {} {{", self.gen_expr(cond)));
                    self.indent += 1;
                    self.gen_block_inner(then_branch);
                    self.indent -= 1;
                    self.emit_line("} else {");
                    self.indent += 1;
                    self.gen_block_inner(else_blk);
                    self.indent -= 1;
                    self.emit_line("}");
                } else {
                    self.emit_line(&format!("if {} {{", self.gen_expr(cond)));
                    self.indent += 1;
                    self.gen_block_inner(then_branch);
                    self.indent -= 1;
                    self.emit_line("}");
                }
            }
            Stmt::For { var, iter, guard, body } => {
                let iter_s = if let Some(g) = guard {
                    format!("({}).into_iter().filter(|&{}| {})", self.gen_expr(iter), var, self.gen_expr(g))
                } else {
                    format!("({}).into_iter()", self.gen_expr(iter))
                };
                self.emit_line(&format!("for {} in {} {{", var, iter_s));
                self.indent += 1;
                self.gen_block_inner(body);
                self.indent -= 1;
                self.emit_line("}");
            }
            Stmt::While { cond, guard, body } => {
                let cond_s = if let Some(g) = guard {
                    format!("({}) && ({})", self.gen_expr(cond), self.gen_expr(g))
                } else {
                    self.gen_expr(cond)
                };
                self.emit_line(&format!("while {} {{", cond_s));
                self.indent += 1;
                self.gen_block_inner(body);
                self.indent -= 1;
                self.emit_line("}");
            }
            Stmt::Match { scrutinee, arms } => {
                let scrut_s = self.gen_expr(scrutinee);
                // String 类型模式匹配：match name { "hello" => } 需要 &str
                let scrut_str = if matches!(&scrutinee.ty, IrType::Str) {
                    format!("{}.as_str()", scrut_s)
                } else {
                    scrut_s
                };
                self.emit_line(&format!("match {} {{", scrut_str));
                self.indent += 1;
                for (pat, body) in arms {
                    let pat_s = self.gen_pattern(pat);
                    self.emit_line(&format!("{} => {{", pat_s));
                    self.indent += 1;
                    // Match arm body 不应生成 return（值应流向 match 表达式外层）
                    let saved = self.suppress_tail_return;
                    self.suppress_tail_return = true;
                    self.gen_block_inner(body);
                    self.suppress_tail_return = saved;
                    self.indent -= 1;
                    self.emit_line("}");
                }
                self.indent -= 1;
                self.emit_line("}");
            }
            Stmt::Break => self.emit_line("break;"),
            Stmt::Continue => self.emit_line("continue;"),
            Stmt::Pass => self.emit_line("();  // pass"),
            Stmt::TypeAlias { name, ty } => {
                self.emit_line(&format!("// type {} = {};", name, self.rust_type(ty)));
            }
            Stmt::Raise { value } => {
                self.emit_line(&format!("panic!(\"{{}}\", {});", self.gen_expr(value)));
            }
            Stmt::Assert { cond, message: _ } => {
                self.emit_line(&format!("assert!({});", self.gen_expr(cond)));
            }
            Stmt::Yield { value } => {
                self.emit_line(&format!("__gen_vec.push({});", self.gen_expr(value)));
            }
            Stmt::YieldFrom { iter } => {
                self.emit_line(&format!("// yield from {}", self.gen_expr(iter)));
                self.emit_line(&format!("__gen_vec.extend({}.into_iter());", self.gen_expr(iter)));
            }
            Stmt::Defer { body: _ } => {
                // Defer 在 Rust 中使用 Drop trait 或 defer-lite crate
                self.emit_line("// defer");
            }
            Stmt::TryCatch { body, catches, else_body, finally_body } => {
                // try/catch → Rust 的 match Result / ? 运算符
                self.emit_line("{");
                self.indent += 1;
                if let Some(fin) = finally_body {
                    self.emit_line("let __finally = || {");
                    self.indent += 1;
                    self.gen_block_inner(fin);
                    self.indent -= 1;
                    self.emit_line("};");
                }
                self.emit_line("let __result = (|| {");
                self.indent += 1;
                self.gen_block_inner(body);
                self.emit_line("Ok(())");
                self.indent -= 1;
                self.emit_line("})();");
                for (pat, block) in catches {
                    let pat_s = match pat {
                        Some(p) => self.gen_pattern(p),
                        None => "_".into(),
                    };
                    self.emit_line(&format!("if let Err({}) = __result {{", pat_s));
                    self.indent += 1;
                    self.gen_block_inner(block);
                    self.indent -= 1;
                    self.emit_line("}");
                }
                if let Some(els) = else_body {
                    self.emit_line("if __result.is_ok() {");
                    self.indent += 1;
                    self.gen_block_inner(els);
                    self.indent -= 1;
                    self.emit_line("}");
                }
                if finally_body.is_some() {
                    self.emit_line("__finally();");
                }
                self.emit_line("__result.ok();");
                self.indent -= 1;
                self.emit_line("};");
            }
            Stmt::Block { stmts } => {
                self.emit_line("{");
                self.indent += 1;
                let n = stmts.len();
                for (i, s) in stmts.iter().enumerate() {
                    self.gen_stmt(s, i == n - 1);
                }
                self.indent -= 1;
                self.emit_line("}");
            }
            #[allow(unreachable_patterns)]
            _ => self.emit_line("// TODO: Stmt variant not yet supported"),
        }
    }

    // ── Expr 生成 ──

    fn gen_expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Lit(lit) => self.gen_lit(lit, &expr.ty),
            ExprKind::Var(name) => {
                if name == "pass" { "()".into() } else { name.clone() }
            }
            ExprKind::Call { callee, args, type_args } => {
                let callee_s = self.gen_expr(callee);
                let mut args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                
                // 泛型类型参数 → turbofish 语法: foo::<T>(args)
                let turbofish = if !type_args.is_empty() {
                    let types: Vec<String> = type_args.iter().map(|t| self.rust_type_name(t)).collect();
                    format!("::<{}>", types.join(", "))
                } else {
                    String::new()
                };
                
                // 默认参数：函数有 def_count 个默认参数，调用方少传了 → 补 None
                if let Some(&(total_params, def_count)) = self.fn_param_info.get(&callee_s) {
                    let required = total_params - def_count;
                    if args_s.len() < required {
                        // 少传了必需参数——这是编译器 bug，插入占位符
                        while args_s.len() < required {
                            args_s.push("/* missing arg */".to_string());
                        }
                    }
                    // 补默认参数：将显式传入的后几个参数包裹在 Some() 中
                    let explicit_default_args = if args_s.len() > required { args_s.len() - required } else { 0 };
                    for i in required..args_s.len() {
                        let arg_idx = i - required;
                        if arg_idx < explicit_default_args {
                            args_s[i] = format!("Some({})", args_s[i]);
                        }
                    }
                    // 补 None 填充未提供的默认参数
                    while args_s.len() < total_params {
                        args_s.push("None".to_string());
                    }
                }
                
                // 推导式展开: comp!(|x| body, iter) → (iter).into_iter().map(|x| body).collect()
                if callee_s == "comp!" {
                    if let (Some(lambda), Some(iter)) = (args_s.first(), args_s.get(1)) {
                        return format!("({}).into_iter().map({}).collect::<Vec<_>>()", iter, lambda);
                    }
                    return format!("vec![]");
                }
                // dict_comp!(|x| (k, v), iter) → (iter).into_iter().map(|x| (k,v)).collect()
                if callee_s == "dict_comp!" {
                    if let (Some(lambda), Some(iter)) = (args_s.first(), args_s.get(1)) {
                        return format!("({}).into_iter().map({}).collect::<HashMap<_,_>>()", iter, lambda);
                    }
                    return format!("HashMap::new()");
                }
                // set_comp!(|x| elem, iter) → (iter).into_iter().map(|x| elem).collect()
                if callee_s == "set_comp!" {
                    if let (Some(lambda), Some(iter)) = (args_s.first(), args_s.get(1)) {
                        return format!("({}).into_iter().map({}).collect::<HashSet<_>>()", iter, lambda);
                    }
                    return format!("HashSet::new()");
                }

                // 检测 enum variant 构造器调用: Circle(0,0,5) → Shape::Circle(0, 0, 5)
                if let Some(enum_name) = self.enum_variants.get(&callee_s) {
                    return if args_s.is_empty() {
                        format!("{}::{}", enum_name, callee_s)
                    } else {
                        format!("{}::{}({})", enum_name, callee_s, args_s.join(", "))
                    };
                }
                
                if callee_s == "print" {
                    let fmt_placeholders: String = args_s.iter().map(|_| "{:?}").collect::<Vec<_>>().join(" ");
                    let fmt = format!("\"{}\"", fmt_placeholders);
                    format!("println!({}, {})", fmt, args_s.join(", "))
                } else if callee_s == "set!" {
                    format!("std::collections::HashSet::from([{}])", args_s.join(", "))
                } else if callee_s == "panic!" {
                    format!("panic!(\"{{:?}}\", {})", args_s.join(", "))
                } else if callee_s == "Exception" {
                    format!("panic!(\"Exception: {{:?}}\", {})", args_s.join(", "))
                } else if !args.is_empty() && is_kwarg_call(args) {
                    // Struct constructor with keyword args: Point(x=3, y=4) → Point { x: 3.0, y: 4.0 }
                    let fields: Vec<String> = args.iter().map(|a| gen_kwarg_field(a, self)).collect();
                    format!("{}{} {{ {} }}", callee_s, turbofish, fields.join(", "))
                } else {
                    format!("{}{}({})", callee_s, turbofish, args_s.join(", "))
                }
            }
            ExprKind::MethodCall { receiver, method, args } => {
                let recv = self.gen_expr(receiver);
                let args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();

                // null coalesce: a ?? b → .or() 或 .unwrap_or()
                if method == "__null_coalesce" && !args.is_empty() {
                    let arg_is_option = matches!(&args[0].ty, IrType::Option(_))
                        || matches!(&args[0].ty, IrType::Named { path, .. } if path == "Option");
                    return if arg_is_option {
                        format!("{}.or({})", recv, args_s[0])
                    } else {
                        format!("{}.unwrap_or({})", recv, args_s[0])
                    };
                }

                // Enum variant 构造: Type.Variant(kwargs...) → Type::Variant(val1, val2, ...)
                // 生成位置参数构造（与 tuple variant 定义一致）
                let is_enum_variant = self.emitted_types.contains(&recv) && is_kwarg_call(args);
                if is_enum_variant {
                    let values: Vec<String> = args.iter()
                        .map(|a| gen_kwarg_value(a, self))
                        .collect();
                    return format!("{}::{}({})", recv, method, values.join(", "));
                }
                // Enum 类型调用变体（位置参数）: Status.Pending("x") → Status::Pending("x")
                if self.emitted_types.contains(&recv) {
                    return format!("{}::{}({})", recv, method, args_s.join(", "));
                }

                // LZ magic methods → Rust equivalents
                // plus common method name mappings
                let rust_method = match method.as_str() {
                    "__str__" => "to_string",
                    "__add__" => "add",
                    "__eq__" => "eq",
                    "__iter__" => "iter",
                    "length" => "len",    // LZ .length() → Rust .len()
                    "new" if self.emitted_types.contains(&recv) || recv == "Box" || recv == "Rc" || recv == "Arc" => {
                        // Static method on type → use :: syntax
                        return format!("{}::new({})", recv, args_s.join(", "));
                    }
                    _ => method,
                };
                let call = format!("{}.{}({})", recv, rust_method, args_s.join(", "));
                // .len() on collections → cast usize to i64
                if method == "len" { format!("({} as i64)", call) } else { call }
            }
            ExprKind::FieldAccess { base, field } => {
                // Enum variant: Color.Red → Color::Red
                let base_s = self.gen_expr(base);
                let sep = if self.emitted_types.contains(&base_s) { "::" } else { "." };
                format!("{}{}{}", base_s, sep, field)
            }
            ExprKind::IndexGet { base, key } => {
                format!("{}[{}]", self.gen_expr(base), self.gen_expr(key))
            }
            ExprKind::IndexSet { base, key, value } => {
                format!("{}[{}] = {}", self.gen_expr(base), self.gen_expr(key), self.gen_expr(value))
            }
            ExprKind::BinOp { op, lhs, rhs } => {
                let op_s = self.binop_str(op);
                format!("{} {} {}", self.gen_expr(lhs), op_s, self.gen_expr(rhs))
            }
            ExprKind::UnOp { op, operand } => {
                // P1: i64::MIN 特判 — -(-9223372036854775808) → i64::MIN
                if *op == UnOpKind::Neg {
                    if let ExprKind::Lit(LitKind::Int(v)) = &operand.kind {
                        if *v == i64::MIN {
                            return "i64::MIN".to_string();
                        }
                    }
                }
                let op_s = self.unop_str(op);
                let inner = self.gen_expr(operand);
                // P1: ! 运算符高优先级 — 操作数是 BinOp 时需要括号
                if *op == UnOpKind::Not && matches!(operand.kind, ExprKind::BinOp { .. }) {
                    format!("{}({})", op_s, inner)
                } else {
                    format!("{}{}", op_s, inner)
                }
            }
            ExprKind::IfExpr { cond, then, els } => {
                format!(
                    "if {} {{ {} }} else {{ {} }}",
                    self.gen_expr(cond),
                    self.gen_expr(then),
                    self.gen_expr(els)
                )
            }
            ExprKind::Lambda { params, body } => {
                let params: Vec<String> = params.iter().map(|p| self.gen_param(p)).collect();
                format!("|{}| {{ {} }}", params.join(", "), self.gen_expr(body))
            }
            ExprKind::StructCtor { name, fields } => {
                // Special handling for built-in types
                match name.as_str() {
                    "_KwArg" => {
                        // 关键字参数 → 提取 value（builder 层暂未完全降级）
                        fields.iter().find(|(n, _)| n == "value")
                            .map(|(_, v)| self.gen_expr(v))
                            .unwrap_or_else(|| "()".into())
                    }
                    "_Walrus" => {
                        // := walrus 运算符: { let x = value; value }
                        let bind = fields.iter().find(|(n, _)| n == "_bind");
                        let val = fields.iter().find(|(n, _)| n == "_val");
                        let bind_s = bind.map(|(_, v)| self.gen_expr(v)).unwrap_or_default();
                        let val_s = val.map(|(_, v)| self.gen_expr(v)).unwrap_or_default();
                        format!("{{ let {} = {}; {} }}", bind_s, val_s, bind_s)
                    }
                    "Dict" => "std::collections::HashMap::new()".to_string(),
                    "Range" => {
                        let start = fields.iter().find(|(n, _)| n == "start");
                        let end = fields.iter().find(|(n, _)| n == "end");
                        let inclusive = fields.iter().any(|(n, v)| {
                            n == "inclusive" && matches!(&v.kind, ExprKind::Lit(LitKind::Bool(true)))
                        });
                        match (start, end) {
                            (Some((_, s)), Some((_, e))) if inclusive =>
                                format!("{}..={}", self.gen_expr(s), self.gen_expr(e)),
                            (Some((_, s)), Some((_, e))) =>
                                format!("{}..{}", self.gen_expr(s), self.gen_expr(e)),
                            (Some((_, s)), None) => format!("{}..", self.gen_expr(s)),
                            (None, Some((_, e))) => format!("..{}", self.gen_expr(e)),
                            _ => "0..0".to_string(),
                        }
                    }
                    _ => {
                        let fields: Vec<String> = fields.iter()
                            .map(|(n, v)| format!("{}: {}", n, self.gen_expr(v)))
                            .collect();
                        format!("{} {{ {} }}", name, fields.join(", "))
                    }
                }
            }
            ExprKind::EnumCtor { enum_name, variant, args } => {
                let args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                if args_s.is_empty() {
                    format!("{}::{}", enum_name, variant)
                } else {
                    format!("{}::{}({})", enum_name, variant, args_s.join(", "))
                }
            }
            ExprKind::Cast { expr, target } => {
                format!("{} as {}", self.gen_expr(expr), self.rust_type(target))
            }
            ExprKind::GenExpr { yield_of } => {
                format!("gen {{ yield {}; }}", self.gen_expr(yield_of))
            }
            ExprKind::MagicCall { kind, args } => {
                // 魔法方法 → Rust 方法/运算符降级
                // args[0] 是 receiver，后续是额外参数
                self.gen_magic_call(kind, args)
            }
            ExprKind::Pipe { receiver, func, args } => {
                let recv = self.gen_expr(receiver);
                let args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                format!("{}({}, {})", func, recv, args_s.join(", "))
            }
            ExprKind::BlockExpr { block } => {
                let mut child = CodeGen::new();
                child.gen_block_inner(block);
                format!("{{\n{}    }}", child.buf)
            }
            ExprKind::TupleLit(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                format!("({})", elems.join(", "))
            }
            ExprKind::ListLit(elems) => {
                // 空列表且在 Nil/Unit 上下文中 → ()
                let is_nil = elems.is_empty() && (
                    matches!(expr.ty, IrType::Unit)
                    || if let IrType::Named { ref path, .. } = expr.ty {
                        path == "Nil" || path == "List"
                    } else { false }
                );
                if is_nil {
                    "()".to_string()
                } else {
                    let elems: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                    format!("vec![{}]", elems.join(", "))
                }
            }
            _ => format!("/* TODO: unsupported expr */"),
        }
    }

    fn gen_lit(&self, lit: &LitKind, _ty: &IrType) -> String {
        match lit {
            LitKind::Int(n) => n.to_string(),
            LitKind::F64(f) => {
                let s = f.to_string();
                if s.contains('.') || s.contains('e') { s } else { format!("{}.0", s) }
            }
            LitKind::Str(s) => {
                let escaped = s.escape_default().to_string();
                format!("\"{}\".to_string()", escaped)
            }
            LitKind::Bool(b) => b.to_string(),
            LitKind::Unit => "()".to_string(),
            LitKind::None_ => "None".to_string(),
        }
    }

    fn binop_str(&self, op: &BinOpKind) -> &'static str {
        match op {
            BinOpKind::Add => "+",
            BinOpKind::Sub => "-",
            BinOpKind::Mul => "*",
            BinOpKind::Div => "/",
            BinOpKind::Mod => "%",
            BinOpKind::Eq => "==",
            BinOpKind::Neq => "!=",
            BinOpKind::Lt => "<",
            BinOpKind::Gt => ">",
            BinOpKind::Le => "<=",
            BinOpKind::Ge => ">=",
            BinOpKind::And => "&&",
            BinOpKind::Or => "||",
            BinOpKind::BitAnd => "&",
            BinOpKind::BitOr => "|",
            BinOpKind::Xor => "^",
            BinOpKind::Shl => "<<",
            BinOpKind::Shr => ">>",
        }
    }

    fn unop_str(&self, op: &UnOpKind) -> &'static str {
        match op {
            UnOpKind::Neg => "-",
            UnOpKind::Not => "!",
            UnOpKind::Ref => "&",
            UnOpKind::MutRef => "&mut ",
            UnOpKind::Deref => "*",
        }
    }

    // ── Pattern 生成 ──

    fn gen_pattern(&self, pat: &Pattern) -> String {
        match pat {
            Pattern::Wildcard => "_".into(),
            Pattern::Ident(name) => {
                // Handle dotted patterns like "Color.Red" → convert to Rust enum pattern "Color::Red"
                if let Some(dot_pos) = name.rfind('.') {
                    let type_name = &name[..dot_pos];
                    let variant = &name[dot_pos+1..];
                    // Check if the prefix is a known type name
                    if self.emitted_types.contains(type_name)
                        || type_name == "Option" || type_name == "Result"
                        || type_name == "Some" || type_name == "None"
                        || type_name == "Ok" || type_name == "Err"
                    {
                        format!("{}::{}", type_name, variant)
                    } else {
                        name.clone()
                    }
                } else {
                    name.clone()
                }
            }
            Pattern::Lit(lit) => {
                // Pattern literals: no .to_string() wrapper
                match lit {
                    LitKind::Int(n) => n.to_string(),
                    LitKind::Str(s) => format!("\"{}\"", s.escape_default()),
                    LitKind::Bool(b) => b.to_string(),
                    _ => self.gen_lit(lit, &IrType::Any),
                }
            }
            Pattern::Tuple(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.gen_pattern(e)).collect();
                format!("({})", elems.join(", "))
            }
            Pattern::Struct { name, fields } => {
                let fields: Vec<String> = fields.iter()
                    .map(|(n, p)| format!("{}: {}", n, self.gen_pattern(p)))
                    .collect();
                format!("{} {{ {} }}", name, fields.join(", "))
            }
            Pattern::Enum { enum_name, variant, args } => {
                if args.is_empty() {
                    format!("{}::{}", enum_name, variant)
                } else {
                    let args: Vec<String> = args.iter().map(|a| self.gen_pattern(a)).collect();
                    format!("{}::{}({})", enum_name, variant, args.join(", "))
                }
            }
        }
    }
}

impl Default for CodeGen {
    fn default() -> Self {
        Self::new()
    }
}

/// 判断表达式是否为 _KwArg（关键字参数）
fn is_kwarg_call(args: &[Expr]) -> bool {
    args.iter().any(|a| matches!(&a.kind, ExprKind::StructCtor { name, .. } if name == "_KwArg"))
}

impl CodeGen {
    /// 魔法方法 → Rust 降级映射
    fn gen_magic_call(&self, kind: &MagicKind, args: &[Expr]) -> String {
        let gen_args = |a: &[Expr]| -> Vec<String> {
            a.iter().map(|e| self.gen_expr(e)).collect()
        };
        let args_s = gen_args(args);
        match kind {
            MagicKind::Call => {
                // __call__ → receiver(args...)
                if args_s.is_empty() {
                    "()".into()
                } else {
                    format!("{}({})", args_s[0], args_s[1..].join(", "))
                }
            }
            MagicKind::GetItem => {
                if args_s.len() >= 2 {
                    format!("{}[{}]", args_s[0], args_s[1])
                } else {
                    "()".into()
                }
            }
            MagicKind::SetItem => {
                if args_s.len() >= 3 {
                    format!("{}[{}] = {}", args_s[0], args_s[1], args_s[2])
                } else {
                    "()".into()
                }
            }
            MagicKind::Iter | MagicKind::IntoIter => {
                if args_s.is_empty() { "().into_iter()".into() }
                else { format!("{}.into_iter()", args_s[0]) }
            }
            MagicKind::Next => {
                if args_s.is_empty() { "None".into() }
                else { format!("{}.next()", args_s[0]) }
            }
            MagicKind::Display => {
                if args_s.is_empty() { "\"\"".into() }
                else { format!("{}.to_string()", args_s[0]) }
            }
            MagicKind::Eq => {
                if args_s.len() >= 2 { format!("{} == {}", args_s[0], args_s[1]) }
                else { "true".into() }
            }
            MagicKind::Cmp => {
                if args_s.len() >= 2 { format!("{}.cmp(&{})", args_s[0], args_s[1]) }
                else { "std::cmp::Ordering::Equal".into() }
            }
            MagicKind::Drop => {
                if args_s.is_empty() { "()".into() }
                else { format!("drop({})", args_s[0]) }
            }
            MagicKind::Add => {
                if args_s.len() >= 2 { format!("{} + {}", args_s[0], args_s[1]) }
                else { args_s.first().cloned().unwrap_or_default() }
            }
            MagicKind::Sub => {
                if args_s.len() >= 2 { format!("{} - {}", args_s[0], args_s[1]) }
                else { format!("-{}", args_s.first().cloned().unwrap_or_default()) }
            }
            MagicKind::Mul => {
                if args_s.len() >= 2 { format!("{} * {}", args_s[0], args_s[1]) }
                else { args_s.first().cloned().unwrap_or_default() }
            }
            MagicKind::Neg => {
                if args_s.is_empty() { "0".into() }
                else { format!("-{}", args_s[0]) }
            }
            MagicKind::Not_ => {
                if args_s.is_empty() { "false".into() }
                else { format!("!{}", args_s[0]) }
            }
            MagicKind::Len => {
                if args_s.is_empty() { "0".into() }
                else { format!("{}.len()", args_s[0]) }
            }
            MagicKind::Rev => {
                if args_s.is_empty() { "().into_iter().rev()".into() }
                else { format!("{}.into_iter().rev()", args_s[0]) }
            }
            MagicKind::SizeHint => {
                if args_s.is_empty() { "(0, None)".into() }
                else { format!("{}.size_hint()", args_s[0]) }
            }
            MagicKind::IterStrategy => {
                args_s.first().cloned().unwrap_or_else(|| "()".into())
            }
        }
    }
}

/// 检测 Block 中是否包含 yield 语句
fn block_has_yield(block: &Block) -> bool {
    for stmt in &block.stmts {
        if matches!(stmt, Stmt::Yield { .. }) {
            return true;
        }
        match stmt {
            Stmt::If { then_branch, else_branch, .. } => {
                if block_has_yield(then_branch) { return true; }
                if let Some(ref e) = else_branch { if block_has_yield(e) { return true; } }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } => {
                if block_has_yield(body) { return true; }
            }
            Stmt::Block { stmts } => {
                if block_has_yield(&Block { stmts: stmts.clone(), ty: IrType::Unit }) { return true; }
            }
            _ => {}
        }
    }
    false
}

/// 从 _KwArg 中提取字段值（丢弃字段名，用于位置参数构造）
fn gen_kwarg_value(arg: &Expr, cg: &CodeGen) -> String {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
        if name == "_KwArg" {
            return fields.iter().find(|(n, _)| n == "value")
                .map(|(_, v)| cg.gen_expr(v))
                .unwrap_or_default();
        }
    }
    cg.gen_expr(arg)
}

/// 将 _KwArg { name, value } 展开为 "field: value"
fn gen_kwarg_field(arg: &Expr, cg: &CodeGen) -> String {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
        if name == "_KwArg" {
            let name_raw = fields.iter().find(|(n, _)| n == "name")
                .and_then(|(_, v)| match &v.kind {
                    ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let value = fields.iter().find(|(n, _)| n == "value")
                .map(|(_, v)| cg.gen_expr(v))
                .unwrap_or_default();
            return format!("{}: {}", name_raw, value);
        }
    }
    cg.gen_expr(arg)
}

/// 检测 IrType 是否引用了指定的类型名（用于递归枚举检测）
fn type_refers_to(ty: &IrType, name: &str) -> bool {
    match ty {
        IrType::Named { path, args } => {
            if path == name { return true; }
            args.iter().any(|a| type_refers_to(a, name))
        }
        IrType::Option(inner) | IrType::Result { ok: inner, err: _ } | IrType::Ref(inner) | IrType::MutRef(inner) => {
            type_refers_to(inner, name)
        }
        IrType::Tuple(elems) => elems.iter().any(|e| type_refers_to(e, name)),
        IrType::Fn { params, ret } => {
            params.iter().any(|p| type_refers_to(p, name)) || type_refers_to(ret, name)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_module() {
        let module = IrModule::new("test".into());
        let mut cg = CodeGen::new();
        let rust = cg.generate(&module);
        assert!(rust.contains("use std::collections"));
    }

    #[test]
    fn test_simple_fn() {
        let mut module = IrModule::new("test".into());
        module.items.push(Item::FnDef(FnDef {
            name: "hello".into(),
            generics: vec![],
            params: vec![],
            ret_ty: IrType::Unit,
            body: Block { stmts: vec![], ty: IrType::Unit },
            intrinsics: vec![],
            is_async: false,
            is_iterator: false,
            is_test: false,
            span: Span::unknown(),
        }));
        let mut cg = CodeGen::new();
        let rust = cg.generate(&module);
        assert!(rust.contains("pub fn hello()"));
    }

    #[test]
    fn test_fn_with_params() {
        let mut module = IrModule::new("test".into());
        module.items.push(Item::FnDef(FnDef {
            name: "add".into(),
            generics: vec![],
            params: vec![
                Param { name: "a".into(), ty: IrType::Int, is_mut: false, default: None },
                Param { name: "b".into(), ty: IrType::Int, is_mut: false, default: None },
            ],
            ret_ty: IrType::Int,
            body: Block {
                stmts: vec![
                    Stmt::ExprStmt {
                        expr: Expr::new(
                            ExprKind::BinOp {
                                op: BinOpKind::Add,
                                lhs: Box::new(Expr::new(ExprKind::Var("a".into()), IrType::Int, Span::unknown())),
                                rhs: Box::new(Expr::new(ExprKind::Var("b".into()), IrType::Int, Span::unknown())),
                            },
                            IrType::Int,
                            Span::unknown(),
                        )
                    }
                ],
                ty: IrType::Int,
            },
            intrinsics: vec![],
            is_async: false,
            is_iterator: false,
            is_test: false,
            span: Span::unknown(),
        }));
        let mut cg = CodeGen::new();
        let rust = cg.generate(&module);
        assert!(rust.contains("pub fn add(a: i64, b: i64) -> i64"));
        assert!(rust.contains("a + b"));
    }
}

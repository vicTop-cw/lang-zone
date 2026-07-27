// Lang-Zong 编译器 — codegen/mod.rs
// 代码生成主模块：CodeGen 结构体 + generate() + gen_import + apply_call_template + map_type

mod decl;
mod func;
mod stmt;
mod expr;
mod magic;
mod builders;
mod helpers;

use crate::parser::*;
use crate::lexer::Lexer;
use crate::types::Type;
use crate::magic::{MagicEngine, MagicKind, MagicEntry};
use crate::bridge::StdBridge;
use crate::bridge::core::BridgeRegistry;
use crate::bridge::source::SourceBridge;

use crate::parser::ParserExprExt;
use self::decl::CodeGenDeclExt;
use self::func::CodeGenFuncExt;
use self::stmt::CodeGenStmtExt;
use self::expr::CodeGenExprExt;
use self::magic::CodeGenMagicExt;
use self::builders::CodeGenBuildersExt;

use std::cell::{Cell, RefCell};
use std::collections::{HashSet, HashMap};
use std::path::PathBuf;

/// 魔法方法 self 模式
pub(super) enum MagicSelfMode { Owned, Ref, RefMut, None }

pub struct CodeGen {
    pub(super) structs: Vec<(String, Vec<String>)>,
    pub(super) enum_variants: Vec<(String, String)>,
    pub(super) fn_owned: HashMap<String, Vec<bool>>,
    pub(super) in_gen: Cell<bool>,
    pub(super) in_build_call: Cell<bool>,
    pub(super) defer_count: Cell<usize>,
    pub(super) fn_params: HashMap<String, Vec<(String, String)>>,
    pub(super) method_params: HashMap<String, Vec<(String, String)>>,
    pub(super) pack_types: RefCell<Vec<String>>,
    pub(super) pack_names: RefCell<Vec<String>>,
    pub(super) bridge: StdBridge,                    // 底层源码桥接（shims/tier2 内部管理）
    pub(super) registry: BridgeRegistry,             // 统一桥接注册中心（对外路由）
    pub(super) magic_engine: MagicEngine,            // 魔法方法映射引擎
}

impl CodeGen {
    pub fn generate(module: &Module, std_dir: Option<PathBuf>, allow_rustc_private: bool, rustc_version: String) -> String {
        // 加载标准库桥接层
        let bridge = match std_dir.clone() {
            Some(ref dir) => {
                let mut b = StdBridge::load(dir)
                    .unwrap_or_else(|e| {
                        eprintln!("Warning: std bridge load failed: {}. Falling back to identity mapping.", e);
                        StdBridge::load(PathBuf::from(".").as_path())
                            .unwrap_or_else(|_| StdBridge::load_fallback())
                    });
                b.set_tier2_allowed(allow_rustc_private);
                b.set_rustc_version(rustc_version);
                b
            },
            None => StdBridge::load_fallback(),
        };

        // 构建统一桥接注册中心
        let mut registry = BridgeRegistry::new();
        if let Some(ref dir) = std_dir {
            match SourceBridge::new(dir.clone()) {
                Ok(source_bridge) => {
                    registry.register(Box::new(source_bridge));
                    registry.set_default("source");
                }
                Err(e) => eprintln!("Warning: SourceBridge init failed: {}", e),
            }
        }

        let mut cg = CodeGen {
            structs: module.structs.iter()
                .map(|s| (s.name.clone(), s.fields.iter().map(|f| f.name.clone()).collect()))
                .collect(),
            enum_variants: module.structs.iter()
                .filter(|s| s.is_enum)
                .flat_map(|s| s.fields.iter().map(move |f| (f.name.clone(), s.name.clone())))
                .collect(),
            fn_owned: module.functions.iter()
                .map(|f| (f.name.clone(), f.params.iter().map(|p| p.is_owned).collect()))
                .collect(),
            in_gen: Cell::new(false),
            in_build_call: Cell::new(false),
            defer_count: Cell::new(0),
            fn_params: HashMap::new(),
            method_params: HashMap::new(),
            pack_types: RefCell::new(Vec::new()),
            pack_names: RefCell::new(Vec::new()),
            bridge,
            registry,
            magic_engine: MagicEngine::new(),
        };
        // 收集函数/方法参数（名 + Rust 类型），供构建块解包（位置 *args / 命名 **kwargs）使用
        for f in &module.functions {
            cg.fn_params.insert(f.name.clone(), f.params.iter().map(|p| (p.name.clone(), cg.map_type(&p.ty))).collect());
        }
        for imp in &module.impls {
            for m in &imp.methods {
                cg.method_params.insert(m.name.clone(), m.params.iter().map(|p| (p.name.clone(), cg.map_type(&p.ty))).collect());
            }
        }
        let mut out = String::new();

        // 1. Imports → Rust use 语句
        let mut emitted_aliases = HashMap::new();
        let mut shims_needed = false;
        for imp in &module.imports {
            out.push_str(&cg.gen_import(imp, &mut emitted_aliases, &mut shims_needed));
        }
        if !module.imports.is_empty() {
            out.push('\n');
        }

        // ── 桥接层 shims 注入 ──
        if shims_needed {
            out.push_str("// ── Lang-Zong 标准库桥接 shims ──\n");
            let shims_src = include_str!("../../std/shims.rs");
            for line in shims_src.lines() {
                if !line.starts_with("//") && !line.starts_with("#!") && !line.is_empty() {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out.push('\n');
        }

        // ── 按需 prelude：只生成实际用到的运行时类型 ──
        let (has_build, has_defer) = scan_usage(&module);
        if has_build {
            out.push_str("// ── Lang-Zong 构建块 prelude ──\n");
            out.push_str("pub trait BuildParams { type Args; fn into_args(self) -> Self::Args; }\n");
            out.push_str("pub struct IterStopException;\n");
            out.push_str("#[derive(Clone)]\n");
            out.push_str("pub enum __Pack {\n");
            out.push_str("    Tuple(Vec<*const ()>),\n");
            out.push_str("    Dict(std::collections::HashMap<String, *const ()>),\n");
            out.push_str("    Single(*const ()),\n");
            out.push_str("}\n\n");
        }
        // ── 操作符自定义 trait prelude（仅当使用 pow/pipe 操作符时）──
        let has_pow = scan_has_pow(&module);
        if has_pow {
            out.push_str("pub trait Pow<Rhs = Self> { type Output; fn pow(self, exp: Rhs) -> Self::Output; }\n");
        }
        // Pipe trait: 需 Sized 约束
        out.push_str("pub trait Pipe<T> where Self: Sized { fn pipe(self, f: impl FnOnce(Self) -> T) -> T { f(self) } }\n\n");
        if has_defer {
            out.push_str("// ── defer guard (LIFO drop-order) ──\n");
            out.push_str("struct DeferGuard<F: FnMut()>(Option<F>);\n");
            out.push_str("impl<F: FnMut()> Drop for DeferGuard<F> {\n");
            out.push_str("    fn drop(&mut self) { if let Some(mut f) = self.0.take() { f(); } }\n");
            out.push_str("}\n\n");
        }

        // 2. Consts
        for c in &module.consts {
            out.push_str(&cg.gen_const(c));
            out.push('\n');
        }

        // 3. Traits
        for t in &module.traits {
            out.push_str(&cg.gen_trait(t));
            out.push('\n');
        }

        // 4. Structs / Enums
        let raises_types: HashSet<String> = module.functions.iter()
            .filter_map(|f| f.raises.as_ref().and_then(|t| {
                if let Type::Named(name) = t { Some(name.clone()) } else { None }
            }))
            .collect();
        for s in &module.structs {
            out.push_str(&cg.gen_struct(s, &raises_types));
            out.push('\n');
        }

        // 5. Impls
        for i in &module.impls {
            out.push_str(&cg.gen_impl(i));
            out.push('\n');
        }

        // 6. Functions
        for f in &module.functions {
            out.push_str(&cg.gen_function(f));
            out.push_str("\n\n");
        }

        // 7. Tests
        if !module.tests.is_empty() {
            out.push_str("#[cfg(test)]\n");
            out.push_str("mod tests {\n");
            out.push_str("    use super::*;\n\n");
            for t in &module.tests {
                out.push_str(&cg.gen_test_stmt(t, 1));
                out.push('\n');
            }
            out.push_str("}\n");
        }

        // ── 三方 crate 依赖提示 ──
        let used = cg.bridge.used_crates.borrow();
        if !used.is_empty() {
            out.push_str("// ── Cargo.toml dependencies (required) ──\n");
            for (name, version, features) in used.iter() {
                if features.is_empty() {
                    out.push_str(&format!("// {} = \"{}\"\n", name, version));
                } else {
                    out.push_str(&format!("// {} = {{ version = \"{}\", features = [{}] }}\n",
                        name, version, features.iter().map(|f| format!("\"{}\"", f)).collect::<Vec<_>>().join(", ")));
                }
            }
            out.push('\n');
        }

        out
    }

    fn gen_import(&self, imp: &ImportStmt, emitted_aliases: &mut HashMap<String, String>, shims_needed: &mut bool) -> String {
        let result = self.registry.resolve_import_full(&imp.path, &imp.items);
        if result.requires_shim {
            *shims_needed = true;
        }
        if let Some(err) = &result.error {
            eprintln!("Warning: {}", err);
        }
        let mut out = String::new();
        for (alias_name, rust_type) in &result.type_aliases {
            if let Some(prev_type) = emitted_aliases.get(alias_name) {
                if prev_type != rust_type {
                    eprintln!("Error: type alias conflict: '{}' mapped to both '{}' and '{}'",
                        alias_name, prev_type, rust_type);
                }
            } else {
                out.push_str(&format!("pub type {} = {};\n", alias_name, rust_type));
                emitted_aliases.insert(alias_name.clone(), rust_type.clone());
            }
        }
        if !imp.items.is_empty() {
            out.push_str(&format!("use {}::{{{}}};\n", result.rust_path, imp.items.join(", ")));
        } else if let Some(alias) = &imp.alias {
            out.push_str(&format!("use {} as {};\n", result.rust_path, alias));
        } else {
            out.push_str(&format!("use {};\n", result.rust_path));
        }
        out
    }

    fn apply_call_template(&self, template: &str, args: &[String]) -> String {
        let mut result = template.to_string();
        for (i, arg) in args.iter().enumerate() {
            let placeholder = format!("{{{}}}", i);
            result = result.replace(&placeholder, arg);
        }
        result
    }

    /// 判断 Rust 类型是否为 Copy（栈类型），判定是否需要在 walrus 中 clone
    fn is_copy_type(rust_type: &str) -> bool {
        match rust_type {
            "i64" | "f64" | "bool" | "f32" | "i8" | "i16" | "i32"
            | "u8" | "u16" | "u32" | "u64" | "isize" | "usize"
            | "char" | "()" | "!" => true,
            s if s.starts_with('&') => true,
            _ => false,
        }
    }

    /// 查找变量名的 Rust 类型（参数优先，否则从上下文推断）
    fn lookup_var_type(&self, name: &str) -> Option<String> {
        // 1. 函数参数
        for params in self.fn_params.values() {
            for (n, ty) in params {
                if n == name { return Some(ty.clone()); }
            }
        }
        // 2. 方法参数
        for params in self.method_params.values() {
            for (n, ty) in params {
                if n == name { return Some(ty.clone()); }
            }
        }
        None
    }

    fn map_type(&self, ty: &Type) -> String {
        if let Type::Named(name) = ty {
            if let Some(rewritten) = self.registry.resolve_type(name) {
                return rewritten;
            }
        }
        ty.to_rust_type_string()
    }
}

/// 扫描模块：是否使用了构建块 / defer
fn scan_usage(module: &Module) -> (bool, bool) {
    use crate::ast::{Stmt, Expr};
    let mut has_build = false;
    let mut has_defer = false;

    fn scan_stmts(stmts: &[Stmt], has_build: &mut bool, has_defer: &mut bool) {
        for s in stmts {
            match s {
                Stmt::Defer(_) => *has_defer = true,
                Stmt::Expr(Expr::BuildBlock { .. }) => *has_build = true,
                Stmt::Let { value: e, .. } | Stmt::Assign { value: e, .. } | Stmt::Return(Some(e)) | Stmt::Yield(Some(e)) => {
                    scan_expr(e, has_build, has_defer);
                }
                Stmt::Expr(e) => scan_expr(e, has_build, has_defer),
                Stmt::While { body, .. } | Stmt::Loop(body) | Stmt::With { body, .. } => {
                    scan_stmts(body, has_build, has_defer);
                }
                Stmt::For { body, .. } => scan_stmts(body, has_build, has_defer),
                Stmt::Guard { else_body, .. } => scan_stmts(else_body, has_build, has_defer),
                _ => {}
            }
        }
    }

    fn scan_expr(e: &Expr, has_build: &mut bool, has_defer: &mut bool) {
        if *has_build && *has_defer { return; }
        match e {
            Expr::BuildBlock { .. } => *has_build = true,
            Expr::Call { args, .. } | Expr::MethodCall { args, .. } => {
                for a in args { scan_expr(a, has_build, has_defer); }
            }
            Expr::Binary { left, right, .. } => {
                scan_expr(left, has_build, has_defer);
                scan_expr(right, has_build, has_defer);
            }
            Expr::Unary { operand, .. } | Expr::Move(operand) | Expr::Try(operand) => {
                scan_expr(operand, has_build, has_defer);
            }
            Expr::ListLit(elems) | Expr::TupleLit(elems) => {
                for e in elems { scan_expr(e, has_build, has_defer); }
            }
            Expr::DictLit(entries) => {
                for (_, v) in entries.iter() { scan_expr(v, has_build, has_defer); }
            }
            Expr::If { then_body, elif_clauses, else_body, .. } => {
                scan_stmts(then_body, has_build, has_defer);
                for (_, b) in elif_clauses { scan_stmts(b, has_build, has_defer); }
                if let Some(b) = else_body { scan_stmts(b, has_build, has_defer); }
            }
            Expr::Match { arms, .. } => {
                for arm in arms { scan_stmts(&arm.body, has_build, has_defer); }
            }
            Expr::Spawn(inner) => scan_expr(inner, has_build, has_defer),
            _ => {}
        }
    }

    for f in &module.functions {
        scan_stmts(&f.body, &mut has_build, &mut has_defer);
    }
    for imp in &module.impls {
        for m in &imp.methods {
            scan_stmts(&m.body, &mut has_build, &mut has_defer);
        }
    }
    // tests 模块内部单独生成，此处无需扫描
    let _ = &module.tests;

    (has_build, has_defer)
}

/// 扫描模块是否使用 `**` 运算符（需要 Pow trait）
fn scan_has_pow(module: &Module) -> bool {
    use crate::ast::{Stmt, Expr, BinOp};
    fn scan_expr_pow(e: &Expr) -> bool {
        match e {
            Expr::Binary { op, left, right, .. } => {
                if matches!(op, BinOp::Pow) { return true; }
                scan_expr_pow(left) || scan_expr_pow(right)
            }
            Expr::Call { args, .. } | Expr::MethodCall { args, .. } => {
                args.iter().any(scan_expr_pow)
            }
            _ => false,
        }
    }
    fn scan_stmts_pow(stmts: &[Stmt]) -> bool {
        for s in stmts {
            match s {
                Stmt::Expr(e) | Stmt::Let { value: e, .. } | Stmt::Return(Some(e)) | Stmt::Yield(Some(e)) => {
                    if scan_expr_pow(e) { return true; }
                }
                _ => {}
            }
        }
        false
    }
    for f in &module.functions {
        if scan_stmts_pow(&f.body) { return true; }
    }
    false
}


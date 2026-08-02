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
    /// 仅 impl 块（非 struct/enum）的类型名，用于 FieldAccess 生成 :: 语法
    impl_types: std::collections::HashSet<String>,
    /// enum variant → enum name 映射（用于构造器调用路由）
    enum_variants: HashMap<String, String>,
    /// 抑制尾表达式隐式 return（用于 match arm / 块表达式内部）
    suppress_tail_return: bool,
    /// 函数名 → (总参数数, 默认参数数)（用于调用时自动填充 None）
    fn_param_info: HashMap<String, (usize, usize)>,
    /// 被修改的模块级 const 名称（需生成 static mut 而非 const）
    mutated_consts: std::collections::HashSet<String>,
    /// enum variant → field types 映射: (enum_name, variant_name) → Vec<IrType>
    enum_variant_fields: HashMap<(String, String), Vec<IrType>>,
    /// 函数名 → variadic 参数起始索引（该索引及之后的参数收集为 &[T]）
    fn_variadic: HashMap<String, usize>,
    /// 函数名 → 参数类型列表（用于隐式 variadic + 调用方类型检查）
    fn_param_types: HashMap<String, Vec<IrType>>,
    /// 重载函数签名集合：函数名 → 多个参数类型签名（同名函数 >1 个时启用 mangling）
    overload_sigs: HashMap<String, Vec<Vec<IrType>>>,
    /// 当前正在生成的函数的 variadic 参数名集合
    current_variadic_params: std::collections::HashSet<String>,
    /// 模块级 const/static 名称（用于参数重命名避免 E0530 冲突）
    top_level_static_names: std::collections::HashSet<String>,
    /// 当前函数的参数重命名映射（原名 → 新名），用于 E0530 冲突解决
    param_renames: HashMap<String, String>,
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
            impl_types: std::collections::HashSet::new(),
            enum_variants: HashMap::new(),
            suppress_tail_return: false,
            fn_param_info: HashMap::new(),
            mutated_consts: std::collections::HashSet::new(),
            enum_variant_fields: HashMap::new(),
            current_variadic_params: std::collections::HashSet::new(),
            fn_variadic: HashMap::new(),
            fn_param_types: HashMap::new(),
            overload_sigs: HashMap::new(),
            top_level_static_names: std::collections::HashSet::new(),
            param_renames: HashMap::new(),
            buf: String::new(),
        }
    }

    // ── 入口 ──

    /// 将整个 IrModule 生成为 Rust 源代码
    pub fn generate(&mut self, module: &IrModule) -> String {
        self.buf.clear();
        self.indent = 0;

        // 预扫描：收集 enum variant → enum name 映射 + 函数参数信息 + impl-only 类型名
        // 注意：不能插入 emitted_types（会阻断 gen_enum_def / gen_struct_def 的去重逻辑）
        self.enum_variants.clear();
        self.fn_param_info.clear();
        self.emitted_types.clear();
        self.impl_types.clear();
        self.mutated_consts.clear();
        self.enum_variant_fields.clear();
        self.overload_sigs.clear();

        // 收集所有模块级 const 名称
        let const_names: std::collections::HashSet<String> = module.items.iter()
            .filter_map(|item| if let Item::Const(c) = item { Some(c.name.clone()) } else { None })
            .collect();
        
        // 收集所有模块级顶层名称（const + 函数名）以避免 E0530 参数冲突
        self.top_level_static_names.clear();
        for item in &module.items {
            match item {
                Item::Const(c) => { self.top_level_static_names.insert(c.name.clone()); }
                Item::FnDef(f) => { self.top_level_static_names.insert(f.name.clone()); }
                Item::StructDef(s) => { self.top_level_static_names.insert(s.name.clone()); }
                Item::EnumDef(e) => { self.top_level_static_names.insert(e.name.clone()); }
                Item::TraitDef(t) => { self.top_level_static_names.insert(t.name.clone()); }
                _ => {}
            }
        }

        // 提前生成所有类型别名（必须在使用前声明）
        for item in &module.items {
            if let Item::TypeAlias(ta) = item {
                self.gen_type_alias_def(ta);
            }
        }
        self.buf.push('\n');

        for item in &module.items {
            if let Item::EnumDef(e) = item {
                for variant in &e.variants {
                    self.enum_variants.insert(variant.name.clone(), e.name.clone());
                    // 收集变体字段类型（用于构造时 Box::new() 包装判断）
                    let field_types: Vec<IrType> = variant.fields.iter()
                        .map(|f| f.ty.clone())
                        .collect();
                    self.enum_variant_fields.insert(
                        (e.name.clone(), variant.name.clone()),
                        field_types,
                    );
                }
            }
            if let Item::FnDef(f) = item {
                let default_count = f.params.iter().filter(|p| p.default.is_some()).count();
                if default_count > 0 {
                    self.fn_param_info.insert(f.name.clone(), (f.params.len(), default_count));
                }
                // 收集所有参数类型（用于隐式 variadic 检测）
                self.fn_param_types.insert(f.name.clone(), f.params.iter().map(|p| p.ty.clone()).collect());
                // 收集重载签名：同名非方法函数出现多次 → 记录各签名（用于函数重载 mangling）
                if !f.params.iter().any(|p| p.name == "self") {
                    let sig: Vec<IrType> = f.params.iter().map(|p| p.ty.clone()).collect();
                    self.overload_sigs.entry(f.name.clone())
                        .or_insert_with(Vec::new)
                        .push(sig);
                }
                // 收集 variadic 参数信息（函数名 → variadic 参数起始索引）
                if let Some((idx, _)) = f.params.iter().enumerate().find(|(_, p)| p.variadic) {
                    self.fn_variadic.insert(f.name.clone(), idx);
                }
                // 方法定义语法 fn Type.method() → Type 是 impl-only 类型名
                if let Some((ty_name, _)) = f.name.split_once('.') {
                    self.impl_types.insert(ty_name.to_string());
                }
                // 扫描函数体中的 const 修改
                if !const_names.is_empty() {
                    scan_const_mutations(&f.body, &const_names, &mut self.mutated_consts);
                }
                // 检测函数参数名与模块级 static 的冲突（E0530）
                // 冲突解决在 gen_fn_def 中通过 param_renames 处理
            }
        }

        // 标准 prelude
        self.emit_prelude();

        // 每个顶层 item
        let mut has_main = false;
        // 追踪已生成的 use 语句（去重 prelude imports）
        let mut emitted_uses: std::collections::HashSet<String> = std::collections::HashSet::new();
        // prelude 已自动导入的模块/类型
        let prelude_imports: std::collections::HashSet<&str> = [
            "std::collections::HashMap", "std::collections::HashSet"
        ].iter().cloned().collect();
        
        for item in &module.items {
            if let Item::FnDef(f) = item {
                if f.name == "main" { has_main = true; }
            }
            // 跳过已在 prelude 中导入的重复 use 语句
            if let Item::Use(u) = item {
                let key = u.path.join("::");
                if prelude_imports.contains(key.as_str()) {
                    continue;
                }
                if u.is_from && u.items.len() == 1 {
                    let full = format!("{}::{}", key, u.items[0]);
                    if prelude_imports.contains(full.as_str()) {
                        continue;
                    }
                }
                if emitted_uses.contains(&key) && u.items.is_empty() {
                    continue;  // 完全重复的 use path;
                }
                emitted_uses.insert(key);
            }
            self.gen_item(item);
            self.buf.push('\n');
        }

        // 如果没有 main 函数，自动生成空 main（避免 E0601）
        if !has_main {
            self.buf.push_str("pub fn main() {\n    // auto-generated: LZ module has no main entry point\n}\n");
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

    /// 返回最后发射的一行（不含缩进和前导空白）
    fn last_emitted_line(&self) -> &str {
        let trimmed = self.buf.trim_end();
        trimmed.rsplit('\n').next().unwrap_or("").trim_start()
    }

    /// 在最后发射的一行末尾追加文本
    fn append_to_last_line(&mut self, s: &str) {
        let len = self.buf.trim_end().len();
        self.buf.insert_str(len, s);
    }

    /// 从表达式中收集 walrus 变量名（用于预声明）
    fn collect_walrus_vars(expr: &Expr, vars: &mut Vec<String>) {
        match &expr.kind {
            ExprKind::StructCtor { name, fields } if name == "_Walrus" => {
                if let Some((_, bind_expr)) = fields.iter().find(|(n, _)| n == "_bind") {
                    if let ExprKind::Var(v) = &bind_expr.kind {
                        if !vars.contains(v) { vars.push(v.clone()); }
                    }
                }
            }
            ExprKind::BinOp { lhs, rhs, .. } => {
                Self::collect_walrus_vars(lhs, vars);
                Self::collect_walrus_vars(rhs, vars);
            }
            ExprKind::UnOp { operand, .. } => {
                Self::collect_walrus_vars(operand, vars);
            }
            ExprKind::Call { callee, args, .. } => {
                Self::collect_walrus_vars(callee, vars);
                for a in args { Self::collect_walrus_vars(a, vars); }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                Self::collect_walrus_vars(receiver, vars);
                for a in args { Self::collect_walrus_vars(a, vars); }
            }
            ExprKind::IfExpr { cond, then, els } => {
                Self::collect_walrus_vars(cond, vars);
                Self::collect_walrus_vars(then, vars);
                Self::collect_walrus_vars(els, vars);
            }
            ExprKind::Paren(inner) => {
                Self::collect_walrus_vars(inner, vars);
            }
            _ => {}
        }
    }

    /// 为 walrus 变量生成预声明: let mut n: i64;
    fn emit_walrus_predecls(&mut self, cond: &Expr) {
        let mut vars = Vec::new();
        Self::collect_walrus_vars(cond, &mut vars);
        for v in &vars {
            self.emit_line(&format!("let mut {}: i64;", v));
        }
    }

    fn emit_prelude(&mut self) {
        // Rust 2021 edition support (async/await, etc.)
        // 使用 outer attributes (#[..]) 而非 inner attributes (#![..])
        // 因为 type alias 可能已在 prelude 之前输出，inner attributes 不允许出现在 item 之后
        self.emit_line("#[allow(unused_imports)]");
        self.emit_line("#[allow(unused_variables)]");
        self.emit_line("#[allow(dead_code)]");
        self.emit_line("#[allow(non_snake_case)]");
        self.buf.push('\n');
        self.emit_line("use std::collections::{HashMap, HashSet};");
        self.emit_line("use std::rc::Rc;");
        self.emit_line("use std::sync::Arc;");
        self.buf.push('\n');

        // ── Lang-Zone 运行时桥接 shims ──
        self.buf.push_str("// ── 运行时桥接 shims ──\n");
        self.buf.push_str("use std::any::Any;\n");
        self.buf.push_str("#[derive(Debug)]\n");
        self.buf.push_str("pub struct __Params {\n");
        self.buf.push_str("    pub args: Vec<Box<dyn Any>>,\n");
        self.buf.push_str("    pub kwargs: HashMap<String, Box<dyn Any>>,\n");
        self.buf.push_str("}\n\n");
        // spawn_task: 异步任务包装器（保持 Future 语义，允许 .await）
        self.buf.push_str("async fn __spawn_task<T>(f: impl std::future::Future<Output = T>) -> T {\n");
        self.buf.push_str("    f.await\n");
        self.buf.push_str("}\n\n");
        // block_on: 同步阻塞执行 async 代码（用于 async main，无外部依赖）
        self.buf.push_str("fn __block_on<F: std::future::Future>(mut f: F) -> F::Output {\n");
        self.buf.push_str("    use std::task::{{Context, Poll, RawWaker, RawWakerVTable, Waker}};\n");
        self.buf.push_str("    use std::pin::Pin;\n");
        self.buf.push_str("    unsafe fn clone_raw(_: *const ()) -> RawWaker { RawWaker::new(std::ptr::null(), &VTABLE) }\n");
        self.buf.push_str("    unsafe fn noop(_: *const ()) {{}}\n");
        self.buf.push_str("    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone_raw, noop, noop, noop);\n");
        self.buf.push_str("    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };\n");
        self.buf.push_str("    let mut cx = Context::from_waker(&waker);\n");
        self.buf.push_str("    let mut f = unsafe { Pin::new_unchecked(&mut f) };\n");
        self.buf.push_str("    loop {\n");
        self.buf.push_str("        match f.as_mut().poll(&mut cx) {\n");
        self.buf.push_str("            Poll::Ready(val) => return val,\n");
        self.buf.push_str("            Poll::Pending => std::thread::yield_now(),\n");
        self.buf.push_str("        }\n");
        self.buf.push_str("    }\n");
        self.buf.push_str("}\n\n");
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

    fn is_collection_type(&self, ty: &IrType) -> bool {
        matches!(ty, IrType::Named { path, .. }
            if ["Vec","List","HashMap","HashSet","Dict","Set"].contains(&path.as_str()))
    }

    /// 检查名称是否为已知的类型名（内置枚举 + 用户定义的 enum/impl 类型）
    fn is_known_type_or_enum(&self, name: &str) -> bool {
        self.emitted_types.contains(name)
        || self.impl_types.contains(name)
        || matches!(name, "Option" | "Result" | "Some" | "None" | "Ok" | "Err")
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
                // Future<T> → 保持 Future 类型用于函数签名
                // 对于变量声明，由 gen_let 等处理方决定是否省略类型标注
                if path == "Future" {
                    if let Some(inner) = args.first() {
                        let inner_ty = self.rust_type(inner);
                        return format!("std::future::Future<Output = {}>", inner_ty);
                    }
                    return "std::future::Future<Output = ()>".into();
                }
                let mapped = self.type_map.get(path.as_str()).map(|s| s.to_string())
                    .unwrap_or_else(|| path.clone());
                if args.is_empty() {
                    // 常见容器类型需要默认泛型参数，否则 Rust 无法推断
                    if path == "List" || path == "Vec" {
                        format!("{}<_>", mapped)
                    } else if path == "Dict" || path == "HashMap" {
                        format!("{}<_, _>", mapped)
                    } else if path == "Set" || path == "HashSet" {
                        format!("{}<_>", mapped)
                    } else if path == "Option" || path == "Result" || path == "Rc" || path == "Arc" || path == "Box" {
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
            Item::TypeAlias(_) => { /* 已提前生成，跳过 */ }
            Item::Test(t) => self.gen_test_def(t),
        }
    }

    /// 计算重载函数的 mangled 名称。仅当函数名有多个重载签名时返回 mangled 名，
    /// 否则返回原名。用于函数定义处。
    fn mangled_fn_name(&self, name: String, sig: &[IrType]) -> String {
        if let Some(sigs) = self.overload_sigs.get(&name) {
            if sigs.len() > 1 {
                let suffix: Vec<String> = sig.iter().map(|t| self.type_mangle_suffix(t)).collect();
                return format!("{}__{}", name, suffix.join("_"));
            }
        }
        name
    }

    /// 根据实参 IR 类型匹配重载签名，返回选择的 mangled 函数名。
    /// 找不到匹配时返回 None（调用方保留原名）。
    fn match_overload(&self, name: &str, sigs: &[Vec<IrType>], args: &[Expr]) -> Option<String> {
        // 参数类型兼容：实参类型与签名参数类型匹配（含 Any 通配）
        let compatible = |arg_ty: &IrType, param_ty: &IrType| -> bool {
            if matches!(param_ty, IrType::Any) { return true; }
            if matches!(arg_ty, IrType::Any) { return true; }
            arg_ty == param_ty
        };
        // 优先找精确参数数量匹配且类型全兼容的签名
        for sig in sigs.iter().filter(|s| s.len() == args.len()) {
            if args.iter().zip(sig.iter()).all(|(a, p)| compatible(&a.ty, p)) {
                let suffix: Vec<String> = sig.iter().map(|t| self.type_mangle_suffix(t)).collect();
                return Some(format!("{}__{}", name, suffix.join("_")));
            }
        }
        None
    }

    /// 将 IrType 编码为 mangled 后缀（简短稳定编码）
    fn type_mangle_suffix(&self, ty: &IrType) -> String {
        match ty {
            IrType::Int => "i64".to_string(),
            IrType::F64 => "f64".to_string(),
            IrType::Bool => "bool".to_string(),
            IrType::Str => "String".to_string(),
            IrType::Named { path, args } => {
                if args.is_empty() {
                    path.replace("::", "_")
                } else {
                    let inner: Vec<String> = args.iter().map(|a| self.type_mangle_suffix(a)).collect();
                    format!("{}_{}", path.replace("::", "_"), inner.join("_"))
                }
            }
            other => format!("{:?}", other).replace(['<', '>', ' ', '(', ')', ',', '{', '}'], "_"),
        }
    }

    fn gen_fn_def(&mut self, f: &FnDef) {
        self.declared.clear();
        // 收集当前函数的 variadic 参数名
        self.current_variadic_params.clear();
        for p in &f.params {
            if p.variadic {
                self.current_variadic_params.insert(p.name.clone());
            }
        }
        // 检测参数名与模块级名称冲突 → 重命名参数（E0530）
        self.param_renames.clear();
        for p in &f.params {
            if p.name != "self" && self.top_level_static_names.contains(&p.name) {
                self.param_renames.insert(p.name.clone(), format!("{}_", p.name));
            }
        }
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
            let pname = self.param_renames.get(&p.name).cloned().unwrap_or_else(|| p.name.clone());
            if duck_indices.contains(&i) {
                let idx = duck_indices.iter().position(|&d| d == i).unwrap();
                format!("{}: {}", pname, duck_params[idx])
            } else if p.name == "self" {
                // self 参数：借用而非移动（LZ 语义为引用）
                if p.is_mut { "&mut self" } else { "&self" }.into()
            } else {
                let ty_str = if p.variadic {
                    format!("&[{}]", self.rust_type(&p.ty))
                } else if p.default.is_some() {
                    format!("Option<{}>", self.rust_type(&p.ty))
                } else {
                    self.rust_type(&p.ty).to_string()
                };
                if p.is_mut {
                    format!("mut {}: {}", pname, ty_str)
                } else {
                    format!("{}: {}", pname, ty_str)
                }
            }
        }).collect();
        let has_yield = block_has_yield(&f.body);
        // Rust 不允许 async main，对于 async main 使用 block_on 包装
        let is_async_main = f.is_async && f.name == "main";
        let ret = if f.name == "main" && !is_async_main {
            String::new()  // Rust main always returns ()
        } else if is_async_main {
            String::new()  // async main 也返回 ()（block_on 内部处理）
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
        let async_kw = if f.is_async && !is_async_main { "async " } else { "" };
        let is_method = f.params.first().map_or(false, |p| p.name == "self");
        let vis = if is_method { "" } else { "pub " };

        let fn_name = self.mangled_fn_name(f.name.clone(), &f.params.iter().map(|p| p.ty.clone()).collect::<Vec<_>>());
        let sig = format!(
            "{}{}{}fn {}{}({}){}{}",
            if f.is_test { "#[test]\n" } else { "" },
            vis,
            async_kw,
            fn_name,
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
                let pname = self.param_renames.get(&p.name).cloned().unwrap_or_else(|| p.name.clone());
                let def_s = self.gen_expr(default_val);
                self.emit_line(&format!("let {} = {}.unwrap_or_else(|| {});", p.name, pname, def_s));
            }
        }

        // 函数体
        self.current_ret_ty = Some(f.ret_ty.clone());
        self.is_main = f.name == "main";
        if is_async_main {
            // async main → 使用 block_on 包装：fn main() { __block_on(async { body }) }
            self.emit_line("let __async_main = async {");
            self.indent += 1;
            self.gen_block_inner(&f.body);
            self.indent -= 1;
            self.emit_line("};");
            self.emit_line("__block_on(__async_main);");
        } else {
            self.gen_block_inner(&f.body);
        }
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
        self.emit_line("#[derive(Debug, Clone)]");
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
            self.emit_line(&format!("impl{} {}{} {{", generics, s.name, generics));
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

        // 方法（impl 块）
        if !e.methods.is_empty() {
            self.buf.push('\n');
            // 枚举方法 impl：为泛型参数添加 Clone 约束，以支持 self.clone() 提取内部值
            let impl_generics = if e.generics.is_empty() {
                String::new()
            } else {
                let params: Vec<String> = e.generics.iter().map(|g| {
                    if g.bounds.is_empty() {
                        format!("{}: Clone", g.name)
                    } else {
                        let bounds: Vec<String> = g.bounds.iter().map(|b| self.rust_type(b)).collect();
                        format!("{}: Clone + {}", g.name, bounds.join(" + "))
                    }
                }).collect();
                format!("<{}>", params.join(", "))
            };
            self.emit_line(&format!("impl{} {}{} {{", impl_generics, e.name, generics));
            self.indent += 1;
            for m in &e.methods {
                self.gen_fn_def(m);
                self.buf.push('\n');
            }
            self.indent -= 1;
            self.emit_line("}");
        }
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
            // 如果第一个参数是 Self，转为 &self（trait 方法与 impl 块签名需一致）
            let params: Vec<String> = sig.params.iter().enumerate().map(|(i, p)| {
                if i == 0 && matches!(p, IrType::Self_) {
                    "&self".to_string()
                } else {
                    self.rust_type(p)
                }
            }).collect();
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
        // 映射 LZ 类型名 → Rust 类型名（仅在 import 路径中使用）
        // 以及相对路径前缀映射：. → self, .. → super
        let lz_to_rust: HashMap<&str, &str> = [
            ("List", "Vec"), ("Dict", "HashMap"), ("Set", "HashSet"),
            ("String", "String"), ("Nil", "()"), ("int", "i64"),
            ("str", "String"), ("f64", "f64"), ("bool", "bool"),
            (".", "self"), ("..", "super"),
        ].iter().cloned().collect();
        
        // LZ 内建函数/类型：由 codegen 直接生成，不需要 Rust use 语句
        let builtin_items: std::collections::HashSet<&str> = [
            "print", "read", "len", "panic", "type", "range",
            "spawn", "await", "yield", "comptime",
        ].iter().cloned().collect();
        
        // 已知的 LZ 模块路径 → Rust 模块路径映射
        // 空字符串 = 无 Rust 对应模块，跳过 use 语句生成
        let known_module_paths: std::collections::HashSet<&str> = [
            "std::io",           // → std::io
            "std::collections",  // → std::collections
            "std::sync",         // → std::sync
            "std::rc",           // → std::rc
            "std::time",         // → std::time
            "std::thread",       // → std::thread
            "std::net",          // → std::net
            "std::fs",           // → std::fs
            "std::env",          // → std::env
            "std::process",      // → std::process
            "std::path",         // → std::path
            "std::hash",         // → std::hash
            "std::iter",         // std::iter (稳定)
            "std::mem",          // std::mem (稳定)
            "std::fmt",          // std::fmt (稳定)
            "std::cmp",          // std::cmp (稳定)
            "std::str",          // std::str (稳定)
            "std::marker",       // std::marker (稳定)
            "std::any",          // std::any (稳定)
            "std::convert",      // std::convert (稳定)
            "std::cell",         // std::cell (稳定)
            "std::os",           // std::os (稳定)
        ].iter().cloned().collect();
        
        // prelude 已导入的项（不需要重复导入）
        let prelude_items: std::collections::HashSet<&str> = [
            "HashMap", "HashSet", "Rc", "Arc", "Vec",
        ].iter().cloned().collect();
        
        let path: Vec<String> = u.path.iter().map(|seg| {
            lz_to_rust.get(seg.as_str()).map(|s| s.to_string()).unwrap_or_else(|| seg.clone())
        }).collect();
        let path_str = path.join("::");
        
        // 相对导入（self::、super::）无法在生成的文件中解析，跳过
        if path_str.starts_with("self::") || path_str.starts_with("super::") {
            return;
        }
        
        // 非相对路径：检查是否为已知模块或已知模块的子路径
        let is_known = known_module_paths.contains(path_str.as_str());
        let parent_path = path_str.rsplitn(2, "::").nth(1).unwrap_or("");
        let parent_is_known = known_module_paths.contains(parent_path);
        let is_std_root = path_str == "std";
        if !is_known && !parent_is_known && !is_std_root {
            // 未知模块路径，跳过（如 std::math, std::bridge.rust.serde_json）
            return;
        }
        
        if u.is_from {
            if u.items.is_empty() {
                if !known_module_paths.contains(path_str.as_str()) && path_str != "std" {
                    return;
                }
                self.emit_line(&format!("use {};", path_str));
            } else if u.items.len() == 1 && u.items[0] == "*" {
                if !known_module_paths.contains(path_str.as_str()) {
                    return;
                }
                self.emit_line(&format!("use {}::*;", path_str));
            } else {
                // 过滤掉内建函数和已在 prelude 中的项
                let items: Vec<String> = u.items.iter()
                    .filter(|item| !builtin_items.contains(item.as_str()))
                    .map(|item| {
                        lz_to_rust.get(item.as_str()).map(|s| s.to_string()).unwrap_or_else(|| item.clone())
                    })
                    .filter(|rust_item| !prelude_items.contains(rust_item.as_str()))
                    .collect();
                if items.is_empty() {
                    return;
                }
                self.emit_line(&format!("use {}::{{{}}};", path_str, items.join(", ")));
            }
        } else {
            // import std.io → use std::io;
            // import std.math → 跳过（无 Rust 对应模块）
            if !known_module_paths.contains(path_str.as_str()) && path_str != "std" {
                return;
            }
            self.emit_line(&format!("use {};", path_str));
        }
    }

    fn gen_const_def(&mut self, c: &ConstDef) {
        let is_mutated = self.mutated_consts.contains(&c.name);
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
        let kw = if is_mutated { "static mut" } else { "const" };
        // 需要使用 lhs!() 惰性初始化的情况：
        // 1. 集合类型（Vec, HashMap, HashSet）— 不能 const 初始化（需要 .to_string() 等）
        // 2. 包含 catch_unwind 等非 const 调用的值
        let needs_lazy = !is_mutated && (
            matches!(&c.ty,
                IrType::Named { path, .. }
                if ["Vec","List","HashMap","HashSet","Dict","Set"].contains(&path.as_str())
            ) || matches!(&c.ty, IrType::Tuple(_))
                || val_str.contains("catch_unwind")
                || val_str.contains("LazyLock")
                || val_str.contains(".to_string()")
        );
        if needs_lazy {
            let lazy_ty = self.rust_type(&c.ty);
            self.emit_line(&format!(
                "static {}: std::sync::LazyLock<{}> = std::sync::LazyLock::new(|| {});",
                c.name, lazy_ty, val_str
            ));
        } else {
            self.emit_line(&format!("{} {}: {} = {};", kw, c.name, ty_str, val_str));
        }
    }

    fn gen_type_alias_def(&mut self, ta: &TypeAliasDef) {
        self.emit_line(&format!("pub type {} = {};", ta.name, self.rust_type(&ta.ty)));
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
                _ => {
                    // Fallback: treat any self param as &self (LZ semantics: self is borrowed by default)
                    if p.is_mut { "&mut self" } else { "&self" }.into()
                }
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
                    if self.mutated_consts.contains(name) {
                        self.emit_line(&format!("unsafe {{ {} = {}; }}", name, self.gen_expr(value)));
                    } else {
                        self.emit_line(&format!("{} = {};", name, self.gen_expr(value)));
                    }
                    return;
                }
                self.declared.insert(name.clone());
                // LZ 语义：所有 let 绑定生成 mut（LZ 中容器/结构体方法可修改内容）
                // 例外：`_` 通配符不能有 mut（Rust E0573）
                let mut_kw = if name == "_" { "" } else { "mut " };
                let skip_ty = *ty == IrType::Any || *ty == IrType::Unit
                    || matches!(ty, IrType::Duck { .. })
                    || matches!(ty, IrType::Generic(_))
                    || matches!(ty, IrType::Fn { .. })
                    || if let IrType::Named { path, args } = ty {
                        path == "Range" || path == "Nil" || path == "Dict" || path == "Set"
                            || path == "Future"  // Future<T> 是 trait 不是具体类型，无法用于变量标注
                            || args.is_empty()
                            || args.iter().any(|a| matches!(a, IrType::Generic(_)))
                    } else { false };
                // 空容器需要类型提示 Vec<_> / HashMap<_, _>（Nil 类型除外）
                // Dir/Set 空容器：即使 skip_ty 为 true，也强制输出类型标注（Rust 无法推断 K, V）
                let is_empty_container = match &value.kind {
                    ExprKind::ListLit(elems) => elems.is_empty() && !matches!(ty, IrType::Named { path, .. } if path == "Nil"),
                    ExprKind::StructCtor { name: n, fields } => n == "Dict" && fields.is_empty(),
                    _ => false,
                };
                // 空 Dict/Set 强制输出类型标注
                let force_ty = is_empty_container && matches!(ty, IrType::Named { path, .. } if path == "Dict" || path == "Set");
                let ty_str = if is_empty_container {
                    // 优先使用声明的类型；若无则使用占位符
                    if !skip_ty || force_ty {
                        format!(": {}", self.rust_type(ty))
                    } else if let ExprKind::StructCtor { name: n, .. } = &value.kind {
                        if n == "Dict" { ": std::collections::HashMap<_, _>".to_string() }
                        else { String::new() }
                    } else { ": Vec<_>".to_string() }
                } else if skip_ty {
                    String::new()
                } else {
                    format!(": {}", self.rust_type(ty))
                };
                // walrus 变量预声明（let 绑定中的 := 需要先声明变量再赋值）
                self.emit_walrus_predecls(value);
                self.emit_line(&format!("let {}{}{} = {};", mut_kw, name, ty_str, 
                    if is_empty_container {
                        match ty {
                            IrType::Named { path, .. } if path == "Dict" || path == "Set" || path == "HashMap" || path == "HashSet" => {
                                "std::collections::HashMap::new()".to_string()
                            }
                            _ => "Vec::new()".to_string()
                        }
                    } else {
                        self.gen_expr(value)
                    }
                ));
            }
            Stmt::Assign { target, value } => {
                // Dict/HashMap 索引赋值 → .insert() 替代（HashMap 不实现 IndexMut）
                if let ExprKind::IndexGet { base, key } = &target.kind {
                    let is_dict = matches!(&base.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                    if is_dict {
                        let base_s = self.gen_expr(base);
                        let key_s = self.gen_expr(key);
                        let val_s = self.gen_expr(value);
                        self.emit_line(&format!("{}.insert({}, {});", base_s, key_s, val_s));
                        return;
                    }
                    // 用户 struct 索引赋值 → .__setitem__(key, value)
                    let is_struct = matches!(&base.ty, IrType::Named { path, .. } if self.emitted_types.contains(path));
                    if is_struct {
                        let base_s = self.gen_expr(base);
                        let key_s = self.gen_index_key(key, base);
                        let val_s = self.gen_expr(value);
                        self.emit_line(&format!("({}).__setitem__({}, {});", base_s, key_s, val_s));
                        return;
                    }
                }
                let target_s = self.gen_target_expr(target);
                let val_s = self.gen_expr(value);
                // 模块级可变变量 → 需 unsafe 块
                if self.mutated_consts.contains(&target_s) {
                    self.emit_line(&format!("unsafe {{ {} = {}; }}", target_s, val_s));
                } else {
                    self.emit_line(&format!("{} = {};", target_s, val_s));
                }
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    self.emit_line(&format!("return {};", self.gen_expr(v)));
                } else {
                    self.emit_line("return;");
                }
            }
            Stmt::ExprStmt { expr } => {
                self.emit_walrus_predecls(expr);
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
                self.emit_walrus_predecls(cond);
                if let Some(else_blk) = else_branch {
                    self.emit_line(&format!("if {} {{", self.gen_bool_cond(cond)));
                    self.indent += 1;
                    self.gen_block_inner(then_branch);
                    self.indent -= 1;
                    self.emit_line("} else {");
                    self.indent += 1;
                    self.gen_block_inner(else_blk);
                    self.indent -= 1;
                    self.emit_line("}");
                } else {
                    self.emit_line(&format!("if {} {{", self.gen_bool_cond(cond)));
                    self.indent += 1;
                    self.gen_block_inner(then_branch);
                    self.indent -= 1;
                    self.emit_line("}");
                }
            }
            Stmt::For { var, iter, guard, body } => {
                self.emit_walrus_predecls(iter);
                // 顶层静态集合（LazyLock<Vec<..>>）不能用 into_iter()（共享引用不可 move），
                // 改用 .iter().cloned()（LZ 元素均 Clone）
                let use_lazy_iter = if let ExprKind::Var(name) = &iter.kind {
                    self.is_collection_type(&iter.ty) && (self.top_level_static_names.contains(name))
                } else { false };
                let iter_s = if let Some(g) = guard {
                    let base = if use_lazy_iter {
                        format!("({}).iter().cloned()", self.gen_expr(iter))
                    } else {
                        format!("({}).into_iter()", self.gen_expr(iter))
                    };
                    format!("{}.filter(|&{}| {})", base, var, self.gen_expr(g))
                } else if use_lazy_iter {
                    format!("({}).iter().cloned()", self.gen_expr(iter))
                } else {
                    format!("({}).into_iter()", self.gen_expr(iter))
                };
                self.emit_line(&format!("for {} in {} {{", var, iter_s));
                self.indent += 1;
                // For loop body should not emit return for tail expressions
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                self.gen_block_inner(body);
                self.suppress_tail_return = saved;
                self.indent -= 1;
                self.emit_line("}");
            }
            Stmt::While { cond, guard, body } => {
                self.emit_walrus_predecls(cond);
                // while true → loop (Rust warns about while true)
                let is_infinite = guard.is_none() && matches!(&cond.kind, ExprKind::Lit(LitKind::Bool(true)));
                let cond_s = if let Some(g) = guard {
                    format!("({}) && ({})", self.gen_expr(cond), self.gen_expr(g))
                } else if is_infinite {
                    String::new()
                } else {
                    self.gen_expr(cond)
                };
                if is_infinite {
                    self.emit_line("loop {");
                } else {
                    self.emit_line(&format!("while {} {{", cond_s));
                }
                self.indent += 1;
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                self.gen_block_inner(body);
                self.suppress_tail_return = saved;
                self.indent -= 1;
                self.emit_line("}");
            }
            Stmt::Match { scrutinee, arms } => {
                let scrut_s = self.gen_expr(scrutinee);
                // String 类型模式匹配：match name { "hello" => } 需要 &str
                // self (引用) → clone 以获得 owned 值用于模式匹配提取
                // 其他变量 → clone 以防止局部移动（如 Result::Err(e) 移动 e）
                let scrut_str = if matches!(&scrutinee.ty, IrType::Str) {
                    format!("{}.as_str()", scrut_s)
                } else if scrut_s == "self" {
                    "self.clone()".to_string()
                } else if matches!(&scrutinee.kind, ExprKind::Var(_)) {
                    format!("{}.clone()", scrut_s)
                } else {
                    scrut_s
                };
                self.emit_line(&format!("match {} {{", scrut_str));
                self.indent += 1;
                for arm in arms {
                    let pat_s = self.gen_pattern(&arm.pattern);
                    let guard_s = arm.guard.as_ref()
                        .map(|g| format!(" if {}", self.gen_expr(g)))
                        .unwrap_or_default();
                    self.emit_line(&format!("{} => {{", format!("{}{}", pat_s, guard_s)));
                    self.indent += 1;
                    // 为递归枚举 Box 字段自动插入 let binding = *binding; 解引用
                    let box_bindings = self.collect_box_pattern_bindings(&arm.pattern);
                    for b in &box_bindings {
                        self.emit_line(&format!("let {} = *{};", b, b));
                    }
                    // Match arm body 不应生成 return（值应流向 match 表达式外层）
                    let saved = self.suppress_tail_return;
                    self.suppress_tail_return = true;
                    self.gen_block_inner(&arm.body);
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
                // try/catch → std::panic::catch_unwind pattern
                let has_catch = !catches.is_empty();
                let has_else = else_body.is_some();
                let has_finally = finally_body.is_some();
                
                // ── catch_unwind wrapping ──
                // suppress_tail_return = true: closure body's last expr is the return value (no explicit return)
                self.emit_line("let __panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {");
                self.indent += 1;
                let saved = self.suppress_tail_return;
                self.suppress_tail_return = true;
                self.gen_block_inner(body);
                self.suppress_tail_return = saved;
                self.indent -= 1;
                self.emit_line("}));");
                
                if has_catch || has_else {
                    self.emit_line("let __try_val = match __panic_result {");
                    self.indent += 1;
                    self.emit_line("Ok(val) => val,");
                    self.emit_line("Err(_panic) => {");
                    self.indent += 1;
                    // Catch handlers: suppress tail return — values flow through match expr
                    // (explicit return statements still work via Stmt::Return handler)
                    let _saved = self.suppress_tail_return;
                    self.suppress_tail_return = true;
                    if catches.len() > 1 {
                        // Multi-catch: emit only the last catch arm (catch-all).
                        // catch_unwind can't do type-specific downcasting at codegen level.
                        // Specific-type catches are emitted as comments for documentation.
                        for (i, (pat, block)) in catches.iter().enumerate() {
                            if i < catches.len() - 1 {
                                // Specific-type catch → comment only
                                let pat_str = match pat {
                                    Some(Pattern::Ident(name)) => name.clone(),
                                    Some(pat) => format!("{:?}", pat),
                                    None => "(catch-all)".into(),
                                };
                                self.emit_line(&format!("// catch {}: (specific-type catch not supported with catch_unwind)", pat_str));
                            } else {
                                // Last arm is the catch-all
                                if let Some(Pattern::Ident(var_name)) = pat {
                                    self.emit_line(&format!(
                                        "let {} = format!(\"{{:?}}\", _panic);",
                                        var_name
                                    ));
                                    self.declared.insert(var_name.clone());
                                }
                                self.gen_block_inner(block);
                            }
                        }
                    } else {
                        for (pat, block) in catches {
                            // Bind catch variable from panic info if pattern is a named ident
                            if let Some(Pattern::Ident(var_name)) = pat {
                                self.emit_line(&format!(
                                    "let {} = format!(\"{{:?}}\", _panic);",
                                    var_name
                                ));
                                self.declared.insert(var_name.clone());
                            }
                            self.gen_block_inner(block);
                        }
                    }
                    self.suppress_tail_return = _saved;
                    self.indent -= 1;
                    self.emit_line("}");
                    self.indent -= 1;
                    self.emit_line("};");
                    // else_body runs after successful try
                    if let Some(els) = else_body {
                        self.emit_line("{");
                        self.indent += 1;
                        let _saved = self.suppress_tail_return;
                        self.suppress_tail_return = true;
                        self.gen_block_inner(els);
                        self.suppress_tail_return = _saved;
                        self.indent -= 1;
                        self.emit_line("}");
                    }
                } else {
                    // No catch/else: unwrap the result (re-panics on error)
                    self.emit_line("let __try_val = __panic_result.unwrap();");
                }
                
                // ── finally cleanup + return value ──
                if has_finally {
                    // Save value, run cleanup statements, then return value
                    self.emit_line("let __final_val = __try_val;");
                    // Emit all finally statements with semicolons (suppress tail = true → bare expr, then append ;)
                    let _saved = self.suppress_tail_return;
                    self.suppress_tail_return = true;
                    self.gen_block_inner(finally_body.as_ref().unwrap());
                    self.suppress_tail_return = _saved;
                    // Fix: ensure last finally statement ends with ; before __final_val
                    if !self.last_emitted_line().ends_with(';') && !self.last_emitted_line().ends_with('}') 
                        && !self.last_emitted_line().is_empty() {
                        self.append_to_last_line(";");
                    }
                    self.emit_line("__final_val");
                } else {
                    self.emit_line("__try_val");
                }
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

    /// 生成索引 key：Rust 的 Vec/切片/字符串索引需要 usize，
    /// 而 LZ 的 int 是 i64，因此对整数索引自动转换为 usize。
    /// 对 HashMap/Dict 保持引用语义（contains_key/get 需要 &K）。
    fn gen_index_key(&self, key: &Expr, base: &Expr) -> String {
        let is_dict = matches!(&base.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
        // 对整数 key（i64）转换为 usize，除非目标是 dict（其 key 不是数值索引）
        if !is_dict && matches!(&key.ty, IrType::Int) {
            let key_s = self.gen_expr(key);
            // 避免对已是 usize 的表达式重复转换；若 key 是纯字面量整数，直接转换
            format!("({} as usize)", key_s)
        } else {
            self.gen_expr(key)
        }
    }

    /// 生成赋值目标表达式（不放 unsafe 包装，用于 Stmt::Assign 等）
    fn gen_target_expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Var(name) => name.clone(),
            ExprKind::FieldAccess { base, field } => {
                format!("{}.{}", self.gen_target_expr(base), field)
            }
            ExprKind::IndexGet { base, key } => {
                let key_s = self.gen_index_key(key, base);
                let is_dict = matches!(&base.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                let key_expr = if is_dict { format!("&{}", key_s) } else { key_s };
                format!("{}[{}]", self.gen_target_expr(base), key_expr)
            }
            _ => self.gen_expr(expr),
        }
    }

    fn gen_expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Lit(lit) => self.gen_lit(lit, &expr.ty),
            ExprKind::Var(name) => {
                if name == "pass" { "()".into() }
                else if self.mutated_consts.contains(name) { format!("unsafe {{ {} }}", name) }
                else if let Some(renamed) = self.param_renames.get(name) { renamed.clone() }
                else { name.clone() }
            }
            ExprKind::Call { callee, args, type_args } => {
                let callee_s = self.gen_expr(callee);
                // 如果 callee 是 Lambda（立即调用闭包），需要用括号包裹
                // move || { body }() → (move || { body })()
                let callee_s = if matches!(&callee.kind, ExprKind::Lambda { .. }) {
                    format!("({})", callee_s)
                } else {
                    callee_s
                };
                // 函数重载分派：根据实参类型选择对应的 mangled 版本
                let callee_s = if let ExprKind::Var(name) = &callee.kind {
                    if let Some(sigs) = self.overload_sigs.get(name) {
                        if sigs.len() > 1 {
                            // 从实参 IR 类型匹配签名
                            if let Some(sel) = self.match_overload(name, sigs, args) {
                                sel
                            } else { callee_s }
                        } else { callee_s }
                    } else { callee_s }
                } else { callee_s };
                
                // 检测 ~: 元组解包模式：连续的 UnpackBuildCall 参数
                let has_unpack = args.iter().any(|a| matches!(&a.kind, ExprKind::MagicCall { kind: MagicKind::UnpackBuildCall, .. }));
                
                // 收集 unpack 的 packed 表达式和索引
                let (unpack_packed, unpack_indices): (Option<String>, Vec<String>) = if has_unpack {
                    let mut packed_s = String::new();
                    let mut idx_list = Vec::new();
                    for a in args.iter() {
                        if let ExprKind::MagicCall { kind: MagicKind::UnpackBuildCall, args: ua } = &a.kind {
                            if ua.len() >= 2 {
                                if packed_s.is_empty() {
                                    packed_s = self.gen_expr(&ua[0]);
                                }
                                idx_list.push(self.gen_expr(&ua[1]));
                            }
                        }
                    }
                    (Some(packed_s), idx_list)
                } else {
                    (None, Vec::new())
                };
                
                let mut args_s: Vec<String> = if has_unpack {
                    // 为所有 unpack 参数生成 __t.0, __t.1 等引用
                    let mut result_args: Vec<String> = Vec::new();
                    let mut idx_iter = unpack_indices.iter();
                    for a in args.iter() {
                        if matches!(&a.kind, ExprKind::MagicCall { kind: MagicKind::UnpackBuildCall, .. }) {
                            if let Some(idx) = idx_iter.next() {
                                result_args.push(format!("__t.{}", idx));
                            } else {
                                result_args.push(self.gen_expr(a));
                            }
                        } else {
                            result_args.push(self.gen_expr(a));
                        }
                    }
                    result_args
                } else {
                    args.iter().map(|a| self.gen_expr(a)).collect()
                };
                
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
                
                // 推导式展开: comp!(|x| body, iter[, cond]) → (iter).into_iter().filter(|x| cond).map(|x| body).collect()
                if callee_s == "comp!" {
                    if let (Some(lambda), Some(iter)) = (args_s.first(), args_s.get(1)) {
                        let lambda = strip_lambda_type(lambda);
                        // 第三个参数存在 → 过滤条件（filter 闭包接收 &Item，用 & 解引用参数）
                        if let Some(cond) = args_s.get(2) {
                            let cond = strip_lambda_type_with_ref(cond);
                            return format!("({}).into_iter().filter({}).map({}).collect::<Vec<_>>()", iter, cond, lambda);
                        }
                        return format!("({}).into_iter().map({}).collect::<Vec<_>>()", iter, lambda);
                    }
                    return format!("vec![]");
                }
                // dict_comp!(|x| (k, v), iter[, cond]) → (iter).into_iter().filter(|&x| cond).map(|x| (k,v)).collect()
                if callee_s == "dict_comp!" {
                    if let (Some(lambda), Some(iter)) = (args_s.first(), args_s.get(1)) {
                        let iter_method = if let Some(iter_expr) = args.get(1) {
                            if matches!(&iter_expr.ty, IrType::Str) { ".chars()" } else { ".into_iter()" }
                        } else { ".into_iter()" };
                        let lambda = strip_lambda_type(lambda);
                        if let Some(cond) = args_s.get(2) {
                            let cond = strip_lambda_type_with_ref(cond);
                            return format!("({}){}.filter({}).map({}).collect::<HashMap<_,_>>()", iter, iter_method, cond, lambda);
                        }
                        return format!("({}){}.map({}).collect::<HashMap<_,_>>()", iter, iter_method, lambda);
                    }
                    return format!("HashMap::new()");
                }
                // set_comp!(|x| elem, iter[, cond]) → (iter).into_iter().filter(|&x| cond).map(|x| elem).collect()
                if callee_s == "set_comp!" {
                    if let (Some(lambda), Some(iter)) = (args_s.first(), args_s.get(1)) {
                        let iter_method = if let Some(iter_expr) = args.get(1) {
                            if matches!(&iter_expr.ty, IrType::Str) { ".chars()" } else { ".into_iter()" }
                        } else { ".into_iter()" };
                        let lambda = strip_lambda_type(lambda);
                        if let Some(cond) = args_s.get(2) {
                            let cond = strip_lambda_type_with_ref(cond);
                            return format!("({}){}.filter({}).map({}).collect::<HashSet<_>>()", iter, iter_method, cond, lambda);
                        }
                        return format!("({}){}.map({}).collect::<HashSet<_>>()", iter, iter_method, lambda);
                    }
                    return format!("HashSet::new()");
                }

                // 检测 callee 是否为 FieldAccess 形式 Type.Variant → Type::Variant
                // 仅当 field 是大写开头（枚举变体）时才用 ::；小写开头为方法调用，用 .
                if let ExprKind::FieldAccess { base, field } = &callee.kind {
                    let base_s = self.gen_expr(base);
                    let known_modules = ["std", "core", "alloc", "crate", "self", "super"];
                    let is_std_module = known_modules.contains(&base_s.as_str());
                    let is_var_base = matches!(&base.kind, ExprKind::Var(_));
                    let is_known_type = is_var_base && self.is_known_type_or_enum(&base_s);
                    let field_is_uppercase = field.chars().next().map_or(false, |c| c.is_uppercase());
                    let sep = if is_var_base && (is_std_module || is_known_type) && field_is_uppercase { "::" } else { "." };
                    if sep == "::" {
                        // 检查变体字段类型，为递归字段自动包裹 Box::new()
                        let field_types = self.enum_variant_fields.get(&(base_s.clone(), field.clone()));
                        let wrapped_args: Vec<String> = args_s.iter().enumerate().map(|(i, a)| {
                            let needs_box = field_types.as_ref().map_or(false, |types| {
                                types.get(i).map_or(false, |ty| type_refers_to(ty, &base_s))
                            });
                            if needs_box { format!("Box::new({})", a) } else { a.clone() }
                        }).collect();
                        return format!("{}::{}({})", base_s, field, wrapped_args.join(", "));
                    }
                    // else: normal field access call, fall through
                }
                
                // 检测 enum variant 构造器调用: Circle(0,0,5) → Shape::Circle(0, 0, 5)
                if let Some(enum_name) = self.enum_variants.get(&callee_s) {
                    return if args_s.is_empty() {
                        format!("{}::{}", enum_name, callee_s)
                    } else {
                        format!("{}::{}({})", enum_name, callee_s, args_s.join(", "))
                    };
                }
                
                // 类型转换: int(x) → x as i64, str(x) → format!("{}", x), f64(x) → x as f64
                if matches!(callee_s.as_str(), "int" | "str" | "f64" | "float") && !args_s.is_empty() {
                    return match callee_s.as_str() {
                        "int" => {
                            // 检查参数表达式类型来决定转换方式
                            if args.len() == 1 {
                                let arg_ty = &args[0].ty;
                                if matches!(arg_ty, IrType::Str) {
                                    format!("({}).parse::<i64>().unwrap()", args_s[0])
                                } else {
                                    format!("({} as i64)", args_s[0])
                                }
                            } else {
                                format!("({} as i64)", args_s[0])
                            }
                        }
                        "str" => {
                            // 用户 struct 且有 __str__ → 调用 __str__()；否则用 Display
                            if args.len() == 1 {
                                if let IrType::Named { path, .. } = &args[0].ty {
                                    if self.emitted_types.contains(path) {
                                        format!("({}).__str__()", args_s[0])
                                    } else {
                                        format!("format!(\"{{}}\", {})", args_s[0])
                                    }
                                } else {
                                    format!("format!(\"{{}}\", {})", args_s[0])
                                }
                            } else {
                                format!("format!(\"{{}}\", {})", args_s[0])
                            }
                        }
                        "f64" | "float" => {
                            if args.len() == 1 {
                                let arg_ty = &args[0].ty;
                                if matches!(arg_ty, IrType::Str) {
                                    format!("({}).parse::<f64>().unwrap()", args_s[0])
                                } else {
                                    format!("({} as f64)", args_s[0])
                                }
                            } else {
                                format!("({} as f64)", args_s[0])
                            }
                        }
                        _ => unreachable!(),
                    };
                }

                if callee_s == "print" {
                    let fmt_placeholders: String = args_s.iter().map(|_| "{:?}").collect::<Vec<_>>().join(" ");
                    let fmt = format!("\"{}\"", fmt_placeholders);
                    // 顶层静态（LazyLock<..>）需解引用才能打印值：print(config) → print(*config)
                    let print_args: Vec<String> = args.iter().zip(args_s.iter()).map(|(a, s)| {
                        if let ExprKind::Var(name) = &a.kind {
                            if self.top_level_static_names.contains(name) {
                                format!("(*{})", s)
                            } else { s.clone() }
                        } else { s.clone() }
                    }).collect();
                    format!("println!({}, {})", fmt, print_args.join(", "))
                } else if callee_s == "set!" {
                    format!("std::collections::HashSet::from([{}])", args_s.join(", "))
                } else if callee_s == "panic!" || callee_s == "panic" {
                    format!("panic!(\"{{:?}}\", {})", args_s.join(", "))
                } else if callee_s == "Exception" {
                    format!("panic!(\"Exception: {{:?}}\", {})", args_s.join(", "))
                // --- Prelude free function → method/expression mappings ---
                } else if callee_s == "len" && args_s.len() == 1 {
                    format!("({}.len() as i64)", args_s[0])
                } else if callee_s == "contains" && args_s.len() == 2 {
                    // HashMap/Dict → contains_key; String/Vec → contains
                    let is_dict = matches!(&args[0].ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                    if is_dict {
                        format!("({}).contains_key(&{})", args_s[0], args_s[1])
                    } else {
                        format!("({}).contains(&{})", args_s[0], args_s[1])
                    }
                } else if callee_s == "iter" && args_s.len() == 1 {
                    format!("({}).iter()", args_s[0])
                } else if callee_s == "enumerate" && args_s.len() == 1 {
                    format!("({}).iter().enumerate()", args_s[0])
                } else if callee_s == "zip" && args_s.len() == 2 {
                    format!("({}).into_iter().zip({}.into_iter())", args_s[0], args_s[1])
                } else if callee_s == "clone" && args_s.len() == 1 {
                    format!("({}).clone()", args_s[0])
                } else if callee_s == "spawn" && args_s.len() >= 1 {
                    // spawn(expr) → 保持异步 Future 语义
                    // 在 async 上下文中：spawn fetch(1) 生成 __spawn_task(fetch(1))
                    // 注意：fetch 是 async fn，直接调用返回 Future
                    format!("__spawn_task({})", args_s.join(", "))
                } else if callee_s == "sort" && args_s.len() == 1 {
                    format!("{{ let mut _tmp = {0}.clone(); _tmp.sort(); _tmp }}", args_s[0])
                } else if callee_s == "reverse" && args_s.len() == 1 {
                    format!("{{ let mut _tmp = {0}.clone(); _tmp.reverse(); _tmp }}", args_s[0])
                } else if callee_s == "format" {
                    // format("fmt", args...) → format!("fmt", args...)
                    // 第一个参数若是字面量 → 直接取字符串值；否则使用生成的表达式
                    let fmt_str = if args.len() >= 1 {
                        if let ExprKind::Lit(LitKind::Str(s)) = &args[0].kind {
                            format!("\"{}\"", s)
                        } else {
                            args_s[0].clone()
                        }
                    } else {
                        "\"\"".to_string()
                    };
                    let rest = if args_s.len() > 1 { format!(", {}", args_s[1..].join(", ")) } else { String::new() };
                    format!("format!({}{})", fmt_str, rest)
                } else if callee_s == "hash" && args_s.len() == 1 {
                    format!("{{ let mut _hasher = std::collections::hash_map::DefaultHasher::new(); std::hash::Hash::hash(&{}, &mut _hasher); std::hash::Hasher::finish(&_hasher) as i64 }}", args_s[0])
                } else if callee_s == "bool" && args_s.len() == 1 {
                    format!("({} != 0)", args_s[0])
                } else if callee_s == "range" && args_s.len() >= 1 {
                    // range(start, end) or range(end) → start..end or 0..end
                    if args_s.len() == 1 {
                        format!("0..{}", args_s[0])
                    } else {
                        format!("{}..{}", args_s[0], args_s[1])
                    }
                // ── Iterator/collection free-function → method mappings ──
                // Pipe inserts receiver as first arg: [1,2,3] |> f(args) → f([1,2,3], args)
                // Strip type annotations from closure args for Rust iterator adapters
                } else if callee_s == "sum" && args_s.len() == 1 {
                    // sum(collection) → collection.iter().sum()
                    format!("({}).iter().sum()", args_s[0])
                } else if callee_s == "map" && args_s.len() == 2 {
                    // map(collection, fn) → collection.into_iter().map(fn)
                    let lambda = strip_lambda_type(&args_s[1]);
                    format!("({}).into_iter().map({})", args_s[0], lambda)
                } else if callee_s == "filter" && args_s.len() == 2 {
                    // filter(iterator, fn) → iterator.filter(fn)
                    // filter takes &Item, so add & before closure params
                    let lambda = strip_lambda_type_with_ref(&args_s[1]);
                    format!("({}).filter({})", args_s[0], lambda)
                } else if callee_s == "collect" && args_s.len() == 1 {
                    format!("({}).collect::<Vec<_>>()", args_s[0])
                } else if callee_s == "max" && args_s.len() == 1 {
                    format!("(*(&{}).iter().max().unwrap())", args_s[0])
                } else if callee_s == "min" && args_s.len() == 1 {
                    format!("(*(&{}).iter().min().unwrap())", args_s[0])
                } else if callee_s == "any" && args_s.len() == 2 {
                    let lambda = strip_lambda_type(&args_s[1]);
                    format!("({}).iter().any({})", args_s[0], lambda)
                } else if callee_s == "all" && args_s.len() == 2 {
                    let lambda = strip_lambda_type(&args_s[1]);
                    format!("({}).iter().all({})", args_s[0], lambda)
                } else if callee_s == "sorted" && args_s.len() == 1 {
                    format!("{{ let mut _tmp = {0}.clone(); _tmp.sort(); _tmp }}", args_s[0])
                } else if callee_s == "reversed" && args_s.len() == 1 {
                    format!("{{ let mut _tmp = {0}.clone(); _tmp.reverse(); _tmp }}", args_s[0])
                // --- End prelude mappings ---
                } else if !args.is_empty() && is_kwarg_call(args) && self.emitted_types.contains(&callee_s) {
                    // Struct constructor with keyword args: Point(x=3, y=4) → Point { x: 3.0, y: 4.0 }
                    let fields: Vec<String> = args.iter().map(|a| gen_kwarg_field(a, self)).collect();
                    format!("{}{} {{ {} }}", callee_s, turbofish, fields.join(", "))
                } else if !args.is_empty() && is_kwarg_call(args) {
                    // Function call with named args: func(a, b~) → func(a, b)
                    let flat_args: Vec<String> = args.iter().map(|a| gen_kwarg_value(a, self)).collect();
                    format!("{}{}({})", callee_s, turbofish, flat_args.join(", "))
                } else if let Some(&variadic_idx) = self.fn_variadic.get(&callee_s) {
                    // Variadic 函数调用: 将 variadic_idx 及之后的实参打包为 &[...]
                    let normal_args = &args_s[..variadic_idx.min(args_s.len())];
                    let variadic_args = if args_s.len() > variadic_idx {
                        args_s[variadic_idx..].join(", ")
                    } else {
                        String::new()
                    };
                    let mut all_args: Vec<String> = normal_args.to_vec();
                    if args_s.len() >= variadic_idx {
                        all_args.push(format!("&[{}]", variadic_args));
                    } else {
                        all_args.push("&[]".to_string());
                    }
                    format!("{}{}({})", callee_s, turbofish, all_args.join(", "))
                } else if let Some(ptypes) = self.fn_param_types.get(&callee_s) {
                    // 隐式 variadic: 单集合参数 + 实参数量不匹配 → auto-pack
                    if ptypes.len() == 1 && args_s.len() != 1
                        && self.is_collection_type(&ptypes[0])
                    {
                        let packed = if args_s.is_empty() {
                            "vec![]".to_string()
                        } else {
                            format!("vec![{}]", args_s.join(", "))
                        };
                        format!("{}{}({})", callee_s, turbofish, packed)
                    } else {
                        let call_str = format!("{}{}({})", callee_s, turbofish, args_s.join(", "));
                        // ~: 元组解包：将调用包装在 { let __t = <packed>; callee(__t.0, __t.1) } 中
                        if let Some(ref packed) = unpack_packed {
                            format!("{{ let __t = {}; {} }}", packed, call_str)
                        } else {
                            call_str
                        }
                    }
                } else {
                    let call_str = format!("{}{}({})", callee_s, turbofish, args_s.join(", "));
                    if let Some(ref packed) = unpack_packed {
                        format!("{{ let __t = {}; {} }}", packed, call_str)
                    } else {
                        call_str
                    }
                }
            }
            ExprKind::MethodCall { receiver, method, args } => {
                let recv = self.gen_expr(receiver);
                let mut args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();

                // await: x.await() → x.await (Rust postfix keyword)
                if method == "await" {
                    return format!("({}).await", recv);
                }

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

                // try_into (the ? operator): convert to Result::unwrap() for now
                // In the future, this should emit ? operator when in a Result-returning context
                if method == "try_into" {
                    return format!("{}.unwrap()", recv);
                }

                // Enum variant 构造: Type.Variant(kwargs...) → Type::Variant(val1, val2, ...)
                // 生成位置参数构造（与 tuple variant 定义一致）
                let is_enum_variant = (self.emitted_types.contains(&recv) || matches!(recv.as_str(), "Option" | "Result")) && is_kwarg_call(args);
                if is_enum_variant {
                    let field_types = self.enum_variant_fields.get(&(recv.clone(), method.clone()));
                    let values: Vec<String> = args.iter().enumerate()
                        .map(|(i, a)| {
                            let val = gen_kwarg_value(a, self);
                            let needs_box = field_types.as_ref().map_or(false, |types| {
                                types.get(i).map_or(false, |ty| type_refers_to(ty, &recv))
                            });
                            if needs_box { format!("Box::new({})", val) } else { val }
                        })
                        .collect();
                    return format!("{}::{}({})", recv, method, values.join(", "));
                }
                // Enum 类型调用变体: Status.Pending("x") → Status::Pending("x")
                // Also: Option.Some(42) → Option::Some(42)
                if self.emitted_types.contains(&recv) || matches!(recv.as_str(), "Option" | "Result") {
                    let field_types = self.enum_variant_fields.get(&(recv.clone(), method.clone()));
                    let wrapped_args: Vec<String> = args_s.iter().enumerate()
                        .map(|(i, a)| {
                            let needs_box = field_types.as_ref().map_or(false, |types| {
                                types.get(i).map_or(false, |ty| type_refers_to(ty, &recv))
                            });
                            if needs_box { format!("Box::new({})", a) } else { a.clone() }
                        })
                        .collect();
                    return format!("{}::{}({})", recv, method, wrapped_args.join(", "));
                }

                // 判断 receiver 是否为用户自定义 struct（有对应魔术方法时用魔术方法名）
                let recv_is_struct = matches!(&receiver.ty, IrType::Named { path, .. } if self.emitted_types.contains(path));

                // LZ magic methods → Rust equivalents
                // plus common method name mappings
                // 注意：算术/比较魔术方法（__add__/__eq__ 等）保留原名，
                // 因为用户 struct 的 impl 方法就叫 __add__；__str__/__iter__ 用于
                // str() 转换和迭代的容器场景，继续映射
                let rust_method = match method.as_str() {
                    // 用户 struct：len/iter/next/contains 等映射到魔术方法
                    "len" if recv_is_struct => "__len__",
                    "iter" if recv_is_struct => "__iter__",
                    "next" if recv_is_struct => "__next__",
                    "getitem" if recv_is_struct => "__getitem__",
                    "setitem" if recv_is_struct => "__setitem__",
                    "contains" if recv_is_struct => "__contains__",
                    // 非用户 struct 的 __str__/__iter__ 用于内置容器/字符串场景
                    "__str__" if !recv_is_struct => "to_string",
                    "__iter__" if !recv_is_struct => "iter",
                    "length" => "len",    // LZ .length() → Rust .len()
                    "to_upper" => "to_uppercase",
                    "to_lower" => "to_lowercase",
                    "push" | "append" => "push",
                    "insert" => "insert",
                    "remove" => "remove",
                    "pop" => "pop",
                    "sort" => "sort",
                    "reverse" => "reverse",
                    "contains" => {
                        // HashMap/Dict → contains_key; String/Vec → contains
                        let is_dict = matches!(&receiver.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                        // 也检查是否为 kwargs 字段（__Params 的 kwargs 是 HashMap）
                        let is_kwargs = matches!(&receiver.kind, ExprKind::FieldAccess { field, .. } if field == "kwargs");
                        if is_dict || is_kwargs { "contains_key" } else { "contains" }
                    }
                    "split" => "split",
                    "join" => "join",
                    "replace" => "replace",
                    "trim" => "trim",
                    "starts_with" => "starts_with",
                    "ends_with" => "ends_with",
                    "new" if self.emitted_types.contains(&recv) || recv == "Box" || recv == "Rc" || recv == "Arc" => {
                        // Static method on type → use :: syntax
                        return format!("{}::new({})", recv, args_s.join(", "));
                    }
                    _ => method,
                };
                // String Pattern trait方法 + 集合contains等需要引用的方法
                // String Pattern trait方法 + 集合contains等需要引用的方法
                let pattern_methods = ["starts_with", "ends_with", "find", "rfind", "replace", "trim_start_matches", "trim_end_matches", "contains", "split", "rsplit", "splitn", "rsplitn"];
                if pattern_methods.contains(&method.as_str()) && !args_s.is_empty() {
                    // 若第一个参数是字符串字面量，直接使用 &str 避免临时 String 生命周期问题
                    if let ExprKind::Lit(LitKind::Str(s)) = &args[0].kind {
                        args_s[0] = format!("\"{}\"", s);
                    } else {
                        args_s[0] = format!("&{}", args_s[0]);
                    }
                }
                let call = format!("{}.{}({})", recv, rust_method, args_s.join(", "));
                // .len()/.length() on collections → cast usize to i64
                if method == "len" || method == "length" { format!("({} as i64)", call) } else { call }
            }
            ExprKind::FieldAccess { base, field } => {
                // Enum variant: Color.Red → Color::Red (field 大写开头)
                // Module path: std.io.print → std::io::print
                // Method/field access: config.get() -> config.get (field 小写开头)
                let base_s = self.gen_expr(base);
                // self 在 impl 方法中始终是 receiver，用 `.` 访问字段
                if base_s == "self" {
                    return format!("{}.{}", base_s, field);
                }
                let known_modules = ["std", "core", "alloc", "crate", "self", "super"];
                let is_var_base = matches!(&base.kind, ExprKind::Var(_));
                let root = base_s.split("::").next().unwrap_or("");
                let is_root_known = known_modules.contains(&root) && root != base_s;
                let is_known_type = is_var_base && self.is_known_type_or_enum(&base_s);
                // 仅当 field 是大写开头（枚举变体/模块）时才用 ::；小写开头为方法/字段，用 .
                let field_is_uppercase = field.chars().next().map_or(false, |c| c.is_uppercase());
                let sep = if (is_root_known || is_known_type) && field_is_uppercase { "::" } else { "." };
                format!("{}{}{}", base_s, sep, field)
            }
            ExprKind::IndexGet { base, key } => {
                let base_s = self.gen_expr(base);
                // Box/Rc/Arc dereference: x[0] on Box<i64> → *x
                if matches!(&base.ty, IrType::Named { path, .. } if path == "Box" || path == "Rc" || path == "Arc") {
                    let key_s = self.gen_expr(key);
                    if key_s == "0" {
                        format!("(*{})", base_s)
                    } else {
                        format!("{}[{}]", base_s, key_s)
                    }
                } else {
                    let key_s = self.gen_index_key(key, base);
                    // HashMap/Dict 索引: map["key"] → map.get(&"key").cloned() 
                    // Rust HashMap 不实现 Index trait
                    let is_dict = matches!(&base.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                    // 也检查是否为 kwargs 字段（__Params 的 kwargs 是 HashMap）
                    let is_kwargs = matches!(&base.kind, ExprKind::FieldAccess { field, .. } if field == "kwargs");
                    // 用户 struct：ml[0] → ml.__getitem__(0)
                    let is_struct = matches!(&base.ty, IrType::Named { path, .. } if self.emitted_types.contains(path));
                    if is_struct {
                        format!("({}).__getitem__({})", base_s, key_s)
                    } else if is_dict || is_kwargs {
                        format!("({}).get(&{}).cloned().unwrap()", base_s, key_s)
                    } else {
                        format!("{}[{}]", base_s, key_s)
                    }
                }
            }
            ExprKind::IndexSet { base, key, value } => {
                let base_s = self.gen_expr(base);
                let key_s = self.gen_index_key(key, base);
                let is_dict = matches!(&base.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap");
                // 用户 struct：ml[1] = v → ml.__setitem__(1, v)
                let is_struct = matches!(&base.ty, IrType::Named { path, .. } if self.emitted_types.contains(path));
                if is_struct {
                    format!("({}).__setitem__({}, {})", base_s, key_s, self.gen_expr(value))
                } else if is_dict {
                    // HashMap 不支持 IndexMut，使用 .insert() 代替
                    format!("{}.insert(&{}, {})", base_s, key_s, self.gen_expr(value))
                } else {
                    format!("{}[{}] = {}", base_s, key_s, self.gen_expr(value))
                }
            }
            ExprKind::BinOp { op, lhs, rhs } => {
                // Pow: ** → .pow() 方法调用 (a ** b → a.pow(b))
                if matches!(op, BinOpKind::Pow) {
                    let lhs_s = self.gen_expr(lhs);
                    let rhs_s = self.gen_expr(rhs);
                    // 整数字面量需要类型后缀，否则 Rust 无法推断 .pow() 的接收者类型
                    if matches!(&lhs.kind, ExprKind::Lit(LitKind::Int(_))) {
                        let suffix = "_i64";
                        return format!("{}{}.pow({})", lhs_s, suffix, rhs_s);
                    }
                    return format!("{}.pow({})", lhs_s, rhs_s);
                }
                // In: 成员测试 → .contains() 方法 (elem in container → container.contains(&elem))
                if matches!(op, BinOpKind::In) {
                    let elem_s = self.gen_expr(lhs);
                    let cont_s = self.gen_expr(rhs);
                    // 字符串包含: "llo" in "hello" → "hello".contains("llo")
                    // 用不带 & 的 contains：对 char / &str / String 都有效（均实现 Pattern）
                    if matches!(&rhs.ty, IrType::Str) {
                        // 若 elem 是 filter 闭包参数（&char 引用），需解引用为 char
                        let elem_arg = if elem_s.starts_with('&') {
                            format!("*({})", elem_s)
                        } else {
                            elem_s.clone()
                        };
                        return format!("{}.contains({})", cont_s, elem_arg);
                    }
                    // Dict/HashMap: key in map → map.contains_key(&key)
                    if matches!(&rhs.ty, IrType::Named { path, .. } if path == "Dict" || path == "HashMap") {
                        return format!("{}.contains_key(&{})", cont_s, elem_s);
                    }
                    // List/Set/其他集合: elem in container → container.contains(&elem)
                    return format!("{}.contains(&{})", cont_s, elem_s);
                }
                // String + 拼接: 右侧需借用 & 以匹配 Rust Add<&str>
                // 但如果 rhs 是 variadic 参数（类型已是 &[T]），不应再加 &
                if *op == BinOpKind::Add && matches!(&rhs.ty, IrType::Str) {
                    let lhs_s = self.gen_expr(lhs);
                    let rhs_s = self.gen_expr(rhs);
                    let rhs_is_variadic = matches!(&rhs.kind, ExprKind::Var(name) if self.current_variadic_params.contains(name));
                    if rhs_is_variadic {
                        return format!("{} + {}", lhs_s, rhs_s);
                    }
                    return format!("{} + &{}", lhs_s, rhs_s);
                }
                let op_s = self.binop_str(op);
                // 链式比较分解: a < b < c → (a < b) && (b < c)
                // 检测：LHS 是比较表达式 且 当前操作符也是比较
                if op.is_comparison() && matches!(&lhs.kind, ExprKind::BinOp { op: lhs_op, .. } if lhs_op.is_comparison()) {
                    if let ExprKind::BinOp { op: inner_op, lhs: inner_lhs, rhs: inner_rhs } = &lhs.kind {
                        let inner_lhs_s = self.gen_expr(inner_lhs);
                        let inner_rhs_s = self.gen_expr(inner_rhs);
                        let rhs_s = self.gen_expr(rhs);
                        return format!("({} {} {}) && ({} {} {})", 
                            inner_lhs_s, self.binop_str(inner_op), inner_rhs_s,
                            inner_rhs_s, op_s, rhs_s);
                    }
                }
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
                    self.gen_bool_cond(cond),
                    self.gen_expr(then),
                    self.gen_expr(els)
                )
            }
            ExprKind::Lambda { params, body, .. } => {
                let params: Vec<String> = params.iter().map(|p| self.gen_param(p)).collect();
                // Use move for all closures - LZ doesn't have Rust borrow semantics
                // 当 body 是 BlockExpr 时，抑制 return 关键字让尾表达式正常工作
                if let ExprKind::BlockExpr { block } = &body.kind {
                    let mut child = CodeGen::new();
                    child.emitted_types = self.emitted_types.clone();
                    child.enum_variants = self.enum_variants.clone();
                    child.fn_param_info = self.fn_param_info.clone();
                    child.current_variadic_params = self.current_variadic_params.clone();
                    // Lambda 体内不生成 return，让尾表达式成为闭包返回值
                    child.suppress_tail_return = true;
                    child.gen_block_inner(block);
                    format!("move |{}| {{\n{}        }}", params.join(", "), child.buf.trim())
                } else {
                    format!("move |{}| {{ {} }}", params.join(", "), self.gen_expr(body))
                }
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
                        // := walrus 运算符：变量已在 emit_walrus_predecls 中预声明
                        // 这里做赋值（非 let 绑定）并返回变量值
                        let bind = fields.iter().find(|(n, _)| n == "_bind");
                        let val = fields.iter().find(|(n, _)| n == "_val");
                        let bind_s = bind.map(|(_, v)| self.gen_expr(v)).unwrap_or_default();
                        let val_s = val.map(|(_, v)| self.gen_expr(v)).unwrap_or_default();
                        format!("{{ {} = {}; {} }}", bind_s, val_s, bind_s)
                    }
                    "Dict" => {
                        if fields.is_empty() {
                            "std::collections::HashMap::new()".to_string()
                        } else {
                            // 带条目的 Dict: HashMap::from([(k, v), ...])
                            let mut pairs = Vec::new();
                            let mut i = 0;
                            while i < fields.len() {
                                let key = fields.iter().find(|(n, _)| n == &format!("_k{}", i));
                                let val = fields.iter().find(|(n, _)| n == &format!("_v{}", i));
                                if let (Some((_, k)), Some((_, v))) = (key, val) {
                                    pairs.push(format!("({}, {})", self.gen_expr(k), self.gen_expr(v)));
                                }
                                i += 1;
                            }
                            format!("std::collections::HashMap::from([{}])", pairs.join(", "))
                        }
                    }
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
                // 查找该变体的字段类型，为递归字段自动包裹 Box::new()
                let field_types = self.enum_variant_fields.get(&(enum_name.clone(), variant.clone()));
                let args_s: Vec<String> = args.iter().enumerate().map(|(i, a)| {
                    let expr_s = self.gen_expr(a);
                    // 检查该位置是否需要 Box::new() 包装
                    let needs_box = field_types.map_or(false, |types| {
                        types.get(i).map_or(false, |ty| type_refers_to(ty, enum_name))
                    });
                    if needs_box {
                        format!("Box::new({})", expr_s)
                    } else {
                        expr_s
                    }
                }).collect();
                if args_s.is_empty() {
                    format!("{}::{}", enum_name, variant)
                } else {
                    format!("{}::{}({})", enum_name, variant, args_s.join(", "))
                }
            }
            ExprKind::Cast { expr, target } => {
                // Special cases: as bool → != 0, as str → format/to_string
                if *target == IrType::Bool {
                    return format!("{} != 0", self.gen_expr(expr));
                }
                if *target == IrType::Str {
                    return format!("format!(\"{{}}\", {})", self.gen_expr(expr));
                }
                // int → f64: implicit widening
                // Non-primitive casts: as String → .to_string()
                if let IrType::Named { path, .. } = target {
                    if path == "String" {
                        return format!("({}).to_string()", self.gen_expr(expr));
                    }
                }
                format!("{} as {}", self.gen_expr(expr), self.rust_type(target))
            }
            ExprKind::GenExpr { yield_of } => {
                format!("gen {{ yield {}; }}", self.gen_expr(yield_of))
            }
            ExprKind::MagicCall { kind, args } => {
                // 特殊 magic: UnpackBuildCall → ~: 构建块元组解包
                // args[0] = 闭包立即调用表达式, args[1] = 元素索引
                if *kind == MagicKind::UnpackBuildCall && args.len() >= 2 {
                    let packed = self.gen_expr(&args[0]);
                    let idx = self.gen_expr(&args[1]);
                    // 使用临时变量访问元组字段: { let __t = packed; __t.<idx> }
                    return format!("{{ let __t = {}; __t.{} }}", packed, idx);
                }
                // 魔法方法 → Rust 方法/运算符降级
                self.gen_magic_call(kind, args)
            }
            ExprKind::Pipe { receiver, func, args } => {
                let recv = self.gen_expr(receiver);
                let args_s: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                format!("{}({}, {})", func, recv, args_s.join(", "))
            }
            ExprKind::BlockExpr { block } => {
                let mut child = CodeGen::new();
                // 复制父 CodeGen 的枚举/类型映射到子实例
                child.emitted_types = self.emitted_types.clone();
                child.enum_variants = self.enum_variants.clone();
                child.fn_param_info = self.fn_param_info.clone();
                // 块表达式尾值应为块尾表达式（非 return）
                child.suppress_tail_return = true;
                // 生成器构建块（含 yield）：预声明 __gen_vec 并返回
                let is_gen = block_has_yield(block);
                if is_gen {
                    child.emit_line("let mut __gen_vec = Vec::new();");
                }
                child.gen_block_inner(block);
                if is_gen {
                    // 块尾返回 __gen_vec
                    let trimmed = child.buf.trim_end();
                    return format!("{{\n{}\n    __gen_vec\n    }}", trimmed);
                }
                format!("{{\n{}    }}", child.buf)
            }
            ExprKind::Paren(inner) => {
                // 剥离不必要括号: (*expr) → *expr, (x != 0) → x != 0
                match &inner.kind {
                    ExprKind::UnOp { .. } | ExprKind::BinOp { .. } => {
                        self.gen_expr(inner)  // 这些运算符自身已有足够优先级
                    }
                    _ => format!("({})", self.gen_expr(inner))
                }
            }
            ExprKind::TupleLit(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                format!("({})", elems.join(", "))
            }
            ExprKind::ListLit(elems) => {
                // 空列表：Nil/Unit/Any → ()，否则 → Vec::new() 或 vec![...]
                let is_nil = elems.is_empty() && (
                    matches!(expr.ty, IrType::Unit | IrType::Any)
                    || matches!(self.rust_type(&expr.ty).as_str(), "()")
                );
                if is_nil {
                    "()".to_string()
                } else {
                    let elems_s: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                    if elems_s.is_empty() {
                        // 空列表：尝试从类型获取元素类型用于 Vec 标注
                        if let IrType::Named { path, args } = &expr.ty {
                            if (path == "List" || path == "Vec") && !args.is_empty() {
                                // 如果元素类型是泛型参数，使用 Vec::new() 让 Rust 推断
                                let elem_is_generic = matches!(&args[0], IrType::Generic(_));
                                if elem_is_generic {
                                    "Vec::new()".to_string()
                                } else {
                                    format!("Vec::<{}>::new()", self.rust_type(&args[0]))
                                }
                            } else if path == "List" || path == "Vec" {
                                "vec![]".to_string()
                            } else {
                                "vec![]".to_string()
                            }
                        } else {
                            "vec![]".to_string()
                        }
                    } else {
                        format!("vec![{}]", elems_s.join(", "))
                    }
                }
            }
            _ => format!("/* TODO: unsupported expr */"),
        }
    }

    /// 生成布尔条件：用户 struct 类型用 __bool__() 方法
    /// if acc → if acc.__bool__()；if not acc → if !(acc.__bool__())
    fn gen_bool_cond(&self, cond: &Expr) -> String {
        // 处理 Not 包裹：not expr → !(expr 转 bool)
        if let ExprKind::UnOp { op: UnOpKind::Not, operand } = &cond.kind {
            let inner = self.gen_bool_cond(operand);
            return format!("!({})", inner);
        }
        // 用户 struct 类型 → 调用 __bool__()
        if let IrType::Named { path, .. } = &cond.ty {
            if self.emitted_types.contains(path) {
                let s = self.gen_expr(cond);
                // 若表达式是赋值等复合，直接调用
                return format!("({}).__bool__()", s);
            }
        }
        self.gen_expr(cond)
    }

    /// 生成 f-string: 提取 {expr} 插值，转成 format!("literal", expr, ...)
    /// {{ / }} 转义为字面量大括号；单个 {expr} 为插值占位符
    fn gen_fstring(&self, s: &str) -> String {
        let mut format_str = String::new();
        let mut args: Vec<String> = Vec::new();
        let mut arg_idx = 0usize;
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    if chars.peek() == Some(&'{') {
                        // {{ → 显示字面 {（format! 中需要 {{）
                        chars.next();
                        format_str.push_str("{{");
                    } else {
                        // 提取插值表达式 {expr}
                        let mut expr = String::new();
                        let mut depth = 0usize;
                        while let Some(&ec) = chars.peek() {
                            match ec {
                                '}' if depth == 0 => { chars.next(); break; }
                                '{' => { depth += 1; expr.push(ec); chars.next(); }
                                '}' => { depth -= 1; expr.push(ec); chars.next(); }
                                _ => { expr.push(ec); chars.next(); }
                            }
                        }
                        // 用唯一标记占位，最后替换为 {} 占位符
                        format_str.push_str(&format!("__LZ_FMT_{}__", arg_idx));
                        arg_idx += 1;
                        args.push(self.gen_expr_str(&expr));
                    }
                }
                '}' => {
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        format_str.push_str("}}");
                    } else {
                        format_str.push('}');
                    }
                }
                _ => format_str.push(c),
            }
        }
        // 先转义文本中的 { / }，再恢复插值占位符为 {}，避免占位符被误转义
        let escaped = escape_format_braces(&format_str);
        let mut fmt_quoted = escaped;
        for i in 0..arg_idx {
            fmt_quoted = fmt_quoted.replace(&format!("__LZ_FMT_{}__", i), "{}");
        }
        let fmt_quoted = fmt_quoted.replace('"', "\\\"");
        if args.is_empty() {
            format!("format!(\"{}\")", fmt_quoted)
        } else {
            format!("format!(\"{}\", {})", fmt_quoted, args.join(", "))
        }
    }

    /// 将 IR 表达式字符串化（用于 f-string 插值）。简单提取：若为 Var/字段则直接用名字
    fn gen_expr_str(&self, expr: &str) -> String {
        expr.trim().to_string()
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
            LitKind::FStr(s) => self.gen_fstring(s),
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
            BinOpKind::Pow => "**",         // 不应直接输出，由 gen_expr 特殊处理
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
            BinOpKind::In => "in",          // 不应直接输出，由 gen_expr 特殊处理
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
                // Handle dotted patterns like "Color.Red" → Rust enum pattern "Color::Red"
                if let Some(dot_pos) = name.rfind('.') {
                    let type_name = &name[..dot_pos];
                    let variant = &name[dot_pos+1..];
                    if self.emitted_types.contains(type_name)
                        || type_name == "Option" || type_name == "Result"
                        || type_name == "Some" || type_name == "None"
                        || type_name == "Ok" || type_name == "Err"
                        || self.enum_variants.contains_key(variant)
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
                // 递归字段在模式中不添加 box 关键字（box_patterns 尚未稳定）
                // 由 gen_stmt(Match) 在臂体开头自动插入 let var = *var; 解引用
                if args.is_empty() {
                    format!("{}::{}", enum_name, variant)
                } else {
                    let args: Vec<String> = args.iter().map(|a| self.gen_pattern(a)).collect();
                    format!("{}::{}({})", enum_name, variant, args.join(", "))
                }
            }
        }
    }

    /// 收集 Enum 模式中需要 Box 解引用的绑定名（用于插入 let name = *name;）
    fn collect_box_pattern_bindings(&self, pat: &Pattern) -> Vec<String> {
        let mut bindings = Vec::new();
        if let Pattern::Enum { enum_name, variant, args } = pat {
            if let Some(field_types) = self.enum_variant_fields.get(&(enum_name.clone(), variant.clone())) {
                for (i, arg_pat) in args.iter().enumerate() {
                    if field_types.get(i).map_or(false, |ty| type_refers_to(ty, enum_name)) {
                        Self::collect_pattern_idents(arg_pat, &mut bindings);
                    }
                }
            }
        }
        bindings
    }

    /// 递归收集 Pattern 中的所有标识符名
    fn collect_pattern_idents(pat: &Pattern, out: &mut Vec<String>) {
        match pat {
            Pattern::Ident(name) => {
                if name != "_" { out.push(name.clone()); }
            }
            Pattern::Tuple(elems) => {
                for e in elems { Self::collect_pattern_idents(e, out); }
            }
            Pattern::Struct { fields, .. } => {
                for (_, p) in fields { Self::collect_pattern_idents(p, out); }
            }
            Pattern::Enum { args, .. } => {
                for a in args { Self::collect_pattern_idents(a, out); }
            }
            _ => {}
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
            MagicKind::UnpackBuildCall => {
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

/// 转义 format! 字符串中的独立 { / }（避免被误判为占位符）
/// 已转义的 {{ 或 }} 保持不变
fn escape_format_braces(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    // 已是 {{，保留（显示字面 {）
                    chars.next();
                    out.push_str("{{");
                } else {
                    out.push_str("{{");
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    out.push_str("}}");
                } else {
                    out.push_str("}}");
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Strip `: Type` annotations from closure params (for comprehension closures)
/// "move |x: i64| { ... }" → "move |x| { ... }"
fn strip_lambda_type(lambda: &str) -> String {
    // Find `|param: Type|` and replace with `|param|`
    let mut result = lambda.to_string();
    // Pattern: `|IDENT: TYPE|` (before `{`)
    if let Some(pipe_idx) = result.find('|') {
        let after_pipe = &result[pipe_idx + 1..];
        if let Some(colon_idx) = after_pipe.find(':') {
            if let Some(next_pipe) = after_pipe[colon_idx..].find('|') {
                let type_end = colon_idx + next_pipe;
                let param_name = after_pipe[..colon_idx].trim();
                let before = &result[..pipe_idx + 1];
                let after = &result[pipe_idx + 1 + type_end..];
                result = format!("{}{}{}", before, param_name, after);
            }
        }
    }
    result
}

/// Strip type annotations AND add `&` before each param for filter-style closures
/// "move |x: i64| { ... }" → "move |&x| { ... }"
/// "move |x| { ... }" → "move |&x| { ... }"
fn strip_lambda_type_with_ref(lambda: &str) -> String {
    let no_types = strip_lambda_type(lambda);
    // Now add `&` before each parameter name
    // Format: "move |x, y| { ... }" or "|x| { ... }"
    if let Some(pipe_open) = no_types.find('|') {
        if let Some(pipe_close) = no_types[pipe_open + 1..].find('|') {
            let params_part = &no_types[pipe_open + 1..pipe_open + 1 + pipe_close];
            let ref_params: String = params_part
                .split(',')
                .map(|p| {
                    let trimmed = p.trim();
                    if trimmed.is_empty() { String::new() }
                    else { format!("&{}", trimmed) }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let before = &no_types[..pipe_open + 1];
            let after = &no_types[pipe_open + 1 + pipe_close..];
            return format!("{}{}{}", before, ref_params, after);
        }
    }
    no_types
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

/// 扫描块中是否存在对 const 名称的修改
fn scan_const_mutations(block: &Block, const_names: &std::collections::HashSet<String>, mutated: &mut std::collections::HashSet<String>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name, is_mut, .. } => {
                if *is_mut && const_names.contains(name) {
                    mutated.insert(name.clone());
                }
            }
            Stmt::Assign { target, .. } => {
                if let ExprKind::Var(v) = &target.kind {
                    if const_names.contains(v) {
                        mutated.insert(v.clone());
                    }
                }
            }
            Stmt::If { then_branch, else_branch, .. } => {
                scan_const_mutations(then_branch, const_names, mutated);
                if let Some(ref e) = else_branch {
                    scan_const_mutations(e, const_names, mutated);
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } => {
                scan_const_mutations(body, const_names, mutated);
            }
            Stmt::Block { stmts } => {
                let inner_block = Block { stmts: stmts.clone(), ty: IrType::Unit };
                scan_const_mutations(&inner_block, const_names, mutated);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    scan_const_mutations(&arm.body, const_names, mutated);
                }
            }
            Stmt::ExprStmt { expr } => {
                scan_expr_mutations(expr, const_names, mutated);
            }
            _ => {}
        }
    }
}

/// 递归扫描表达式中对 const 名称的修改（如 +=, -= 等复合赋值）
fn scan_expr_mutations(expr: &Expr, const_names: &std::collections::HashSet<String>, mutated: &mut std::collections::HashSet<String>) {
    match &expr.kind {
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_mutations(lhs, const_names, mutated);
            scan_expr_mutations(rhs, const_names, mutated);
        }
        ExprKind::StructCtor { name, fields } if name == "_Walrus" => {
            if let Some((_, bind_expr)) = fields.iter().find(|(n, _)| n == "_bind") {
                if let ExprKind::Var(v) = &bind_expr.kind {
                    if const_names.contains(v) {
                        mutated.insert(v.clone());
                    }
                }
            }
        }
        _ => {}
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
                Param { name: "a".into(), ty: IrType::Int, is_mut: false, default: None, variadic: false },
                Param { name: "b".into(), ty: IrType::Int, is_mut: false, default: None, variadic: false },
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

// Lang-Zone 编译器 — semantic_check.rs
// G2 错误检测缺口补齐（方案.md 缺口 #G2）：
// 在 build_ir 之前对 AST 做轻量语义校验，拒绝语法矩阵 17 个反例中
// 当前静默通过的部分（重复枚举变体 / 未知类型 / 参数个数不匹配 /
// 未绑定变量 / 函数外 break / 函数外 yield / match 重复 case /
// 返回类型字面量不匹配 / 未声明 raises 却 raise / 重复参数 / 非法 import 路径等）。
//
// 设计原则：只收紧错误路径，不改变任何正例行为；白名单从宽，
// 无法可靠判断的场景一律放行交给后续阶段。

use crate::ast::*;
use crate::types::def::Type;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// 单函数签名（用于 arity 检查）
#[derive(Clone)]
struct FnSig {
    param_count: usize,
    /// 必需参数数（无默认值的参数个数）；调用参数个数 ∈ [param_count_min, param_count]
    param_count_min: usize,
    generic_count: usize,
    /// 函数声明含 `..` 变参注入：调用时参数个数上限不限
    variadic: bool,
    /// 最后参数为 List<T>（安全收集）：允许省略该参数、允许超量实参自动收集为 List
    collect_list: bool,
}

/// 判断类型是否为列表类（List<T>/Array<T>/Vec<T> 或裸 List/Array/Vec）
fn is_list_type(t: &Type) -> bool {
    match t {
        Type::Generic { base, .. } => {
            matches!(base.as_ref(), Type::Named(b) if b == "List" || b == "Array" || b == "Vec")
        }
        Type::Named(b) => b == "List" || b == "Array" || b == "Vec",
        _ => false,
    }
}

#[derive(Default)]
pub struct Checker {
    errors: Vec<String>,
    /// 模块级函数签名表
    fn_sigs: HashMap<String, FnSig>,
    /// 自定义类型名（struct / enum / trait / type alias）
    type_names: HashSet<String>,
    /// 枚举名（enum 的变体是 StuctDef::fields）
    enum_names: HashSet<String>,
    /// 枚举变体名（`Ordering.Less` 的 base / `case Less` 的裸变体）
    enum_variants: HashSet<String>,
    /// 导入符号（from x import a, b → 视为已知类型/变量来源，避免误伤跨文件类型）
    imported_names: HashSet<String>,
    /// 模块级函数名（顶层 def 视为模块作用域内可绑定）
    fn_names: HashSet<String>,
    /// 模块文件所在目录（import 路径检查）
    mod_dir: Option<std::path::PathBuf>,
    /// 当前函数上下文（yield / raise 规则）
    fn_ctx: Option<FnCtx>,
    /// 循环深度（break/continue 检查）
    loop_depth: usize,
    /// try/catch 捕获深度（raise 在函数内被 catch 捕获时无需声明 raises）
    catch_depth: usize,
    /// 作用域栈（unbound 变量 / 不可变赋值检查；值 = 是否可变）
    scopes: Vec<HashMap<String, bool>>,
}

#[derive(Default, Clone)]
struct FnCtx {
    name: String,
    return_type: Option<Type>,
    #[allow(dead_code)] // 保留：错误检测扩展预留（G2 后续可对 raises 做校验）
    raises: Option<Type>,
    has_yield: bool,
    has_raise: bool,
}

/// 剥离泛型参数：`Iterator<T>` → `Iterator`；`Vec<int>` → `Vec`
fn base_name(ty: &str) -> &str {
    ty.split('<').next().unwrap_or(ty)
}

/// 生成 impl 的可读标签（`impl Trait for Type` / `impl Type`）
fn imp_label(imp: &ImplDef) -> String {
    match &imp.trait_name {
        Some(t) => format!("impl {t} for {}", imp.type_name),
        None => format!("impl {}", imp.type_name),
    }
}

/// 内建类型白名单（codegen 直接映射，不经过自定义类型表）
fn builtin_type_names() -> HashSet<&'static str> {
    let mut s = HashSet::new();
    for n in [
        "int", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64",
        "u128", "usize", "float", "f32", "f64", "str", "string", "char", "byte",
        "bool", "None", "unit", "void", "any", "never", "Self", "self",
        "List", "Array", "Vec", "Dict", "Map", "HashMap", "BTreeMap", "Set",
        "HashSet", "BTreeSet", "Option", "Some", "Result", "Ok", "Err", "Tuple",
        "Ptr", "Pointer", "Ref", "MutRef", "Fn", "FnMut", "FnOnce", "Iterator",
        "Iter", "Generator", "Simd", "Box", "Any", "Object", "Json", "JSON",
        "Rc", "Arc", "Weak", "Maybe", "Never", "Auto", "Ext",
        "String", "__Params",
        "Iterable", "Cell", "Ordering", "Error", "Box", "Mutex", "RwLock", "AtomicBool",
        "AtomicI32", "AtomicU64", "Duration", "Instant", "Path", "PathBuf", "File",
        "Ordered", "Clone", "Copy", "Display", "Debug", "Eq", "Ord", "PartialEq",
        "PartialOrd", "Default", "Hash", "Add", "Sub", "Mul", "Div", "Rem", "Neg",
        "Index", "IndexMut", "IntoIterator", "FromIterator", "AsRef", "AsMut",
        "Deref", "Drop", "Send", "Sync", "Into", "From", "ToString",
    ] {
        s.insert(n);
    }
    s
}

/// 内建函数/值白名单（codegen builtin_items + 常用入口）
fn builtin_value_names() -> HashSet<&'static str> {
    let mut s = HashSet::new();
    for n in [
        "print", "read", "len", "panic", "type", "range", "spawn", "await", "yield",
        "comptime", "input", "assert", "sizeof", "alignof", "iter", "next", "clone",
        "to_string", "int", "float", "str", "bool", "list", "dict", "set", "tuple",
        "True", "False", "None", "self", "this", "_", "super",
        "true", "false", "map", "filter", "reduce", "fold", "zip", "enumerate", "sum", "min", "max",
        "__name__", "__doc__", "__is_macro__", "__slots__", "__file__", "__package__",
        "__path__", "__module__", "__qualname__", "__", "collect",
        "inspect",
        // std 模块名（作为 PathAccess 根节点时视为已绑定）
        "time", "io", "fs", "path", "env", "process", "sync", "thread",
        "collections", "iter", "ops", "fmt", "mem", "ptr", "ffi",
    ] {
        s.insert(n);
    }
    s
}

pub fn check_module(m: &Module) -> Vec<String> {
    let mut c = Checker::default();
    c.collect(m);
    c.analyze(m);
    c.errors
}

impl Checker {
    fn error(&mut self, msg: String) {
        self.errors.push(format!("Semantic error: {msg}"));
    }

    // ─────────────────────────── 收集阶段 ───────────────────────────

    fn collect(&mut self, m: &Module) {
        // 模块目录（import 检查）
        if let Some(fp) = &m.file_path {
            self.mod_dir = Path::new(fp).parent().map(|p| p.to_path_buf());
        }

        // 导入符号（from x import a, b → a/b 视为已知；import x as y → y 已知）
        for imp in &m.imports {
            if imp.is_from {
                for item in &imp.items {
                    self.imported_names.insert(item.clone());
                }
            }
            if let Some(alias) = &imp.alias {
                self.imported_names.insert(alias.clone());
            }
            if !imp.is_from && imp.path.first().map_or(false, |p| p != "std" && p != "macro") {
                // 裸 import path：模块名作为命名空间前缀（lib_math.square(...)）→ 视为已绑定
                if let Some(last) = imp.path.last() {
                    self.imported_names.insert(last.clone());
                }
                // 检查同目录文件存在性（模块文件）
                let rel_dir = self.mod_dir.clone();
                let ok = match (rel_dir, imp.path.last()) {
                    (Some(dir), Some(last)) => dir.join(format!("{last}.lz")).exists(),
                    _ => false,
                };
                if !ok {
                    self.error(format!(
                        "import 路径不存在: {}（在当前模块目录下找不到 {}.lz）",
                        imp.path.join("."),
                        imp.path.last().map(|s| s.as_str()).unwrap_or("?")
                    ));
                }
            }
        }

        // 自定义类型
        for s in &m.structs {
            self.type_names.insert(s.name.clone());
            if s.is_enum {
                self.enum_names.insert(s.name.clone());
                for f in &s.fields {
                    self.enum_variants.insert(f.name.clone());
                }
                self.check_enum_dup_variant(s);
            }
        }
        for t in &m.traits {
            self.type_names.insert(t.name.clone());
        }
        for t in &m.type_aliases {
            self.type_names.insert(t.name.clone());
        }
        for d in &m.duck_defs {
            self.type_names.insert(d.name.clone());
        }

        // G6: impl 块校验（未知类型 / 未知 trait / 抽象方法缺失 / 多余方法）
        self.check_impls(m);

        // 类型名全部收集完成后再做依赖类型名的检查（避免前向引用误报）：
        // 1) struct 字段未知类型；2) 函数签名（参数/返回/where/泛型默认）
        for s in &m.structs {
            if !s.is_enum {
                self.check_struct_fields_unknown(s);
            }
        }
        for f in &m.functions {
            let sig = FnSig {
                param_count: f.params.len(),
                param_count_min: f.params.iter().filter(|p| p.default.is_none()).count(),
                generic_count: f.generics.len(),
                variadic: !matches!(f.variadic, VariadicMode::None),
                collect_list: f
                    .params
                    .last()
                    .map(|p| p.default.is_none() && is_list_type(&p.ty))
                    .unwrap_or(false),
            };
            self.fn_names.insert(f.name.clone());
            self.fn_sigs.insert(f.name.clone(), sig);
            self.check_function_header(f);
        }
    }

    // G6: impl 块语义校验
    //  - impl 目标类型必须存在（内建类型或本模块自定义类型）
    //  - trait impl 的 trait 必须存在
    //  - trait 声明的抽象方法（is_abstract）必须在 impl 中全部实现（Bug-10）
    //  - impl 方法不得超出 trait 声明（Bug-9，仅 trait impl 生效）
    fn check_impls(&mut self, m: &Module) {
        let builtins = builtin_type_names();
        for imp in &m.impls {
            let type_base = base_name(&imp.type_name);
            if !self.type_names.contains(type_base) && !builtins.contains(type_base) {
                self.error(format!(
                    "impl 目标类型不存在: {type_base}（在 {} 中）",
                    imp_label(imp)
                ));
            }
            let Some(tn) = &imp.trait_name else { continue };
            let trait_base = base_name(tn);
            let Some(trait_def) = m.traits.iter().find(|t| t.name == trait_base) else {
                // 内建 trait（Iterator / Iterable / Display 等）隐式存在，允许 impl 且不做抽象方法校验
                if builtins.contains(trait_base) {
                    continue;
                }
                self.error(format!(
                    "impl for unknown trait: {trait_base}（在 {} 中）",
                    imp_label(imp)
                ));
                continue;
            };
            // Bug-10：抽象方法必须全部实现（带默认实现的方法可跳过）
            for tm in &trait_def.methods {
                if tm.is_abstract && !imp.methods.iter().any(|im| im.name == tm.name) {
                    self.error(format!(
                        "trait {trait_base} 要求实现抽象方法 {} 但 {} 未提供",
                        tm.name,
                        imp_label(imp)
                    ));
                }
            }
            // Bug-9：impl 方法不得超出 trait 声明
            for im in &imp.methods {
                if !trait_def.methods.iter().any(|tm| tm.name == im.name) {
                    self.error(format!(
                        "impl 方法 {} 未在 trait {trait_base} 中声明（{} 中）",
                        im.name,
                        imp_label(imp)
                    ));
                }
            }
        }
    }

    fn is_known_type(&self, name: &str, fn_generics: &[String], struct_generics: &[String]) -> bool {
        let r = builtin_type_names().contains(name)
            || self.type_names.contains(name)
            || self.imported_names.contains(name)
            || fn_generics.iter().any(|g| g == name)
            || struct_generics.iter().any(|g| g == name);
        r
    }

    fn check_type(
        &mut self,
        ty: &Type,
        ctx: &str,
        fn_generics: &[String],
        struct_generics: &[String],
    ) {
        match ty {
            Type::Named(name) => {
                // 关联类型绑定（Rust 风格 `Output = I.Item` / `Item = T`）：
                // 形如 `名 = 类型`，只校验右侧实际类型
                if let Some(eq_idx) = name.find(" = ") {
                    let rhs = &name[eq_idx + 3..];
                    let root = rhs.split('.').next().unwrap_or(rhs);
                    if !self.is_known_type(root, fn_generics, struct_generics) {
                        self.error(format!("未知类型: {name}（位于 {ctx}）"));
                    }
                    return;
                }
                // 关联类型路径（I.Item / Self.Item / Self.Output / A.B）：
                // 只校验根路径类型名，`.` 之后是关联类型访问（Rust 侧生成 I::Item）
                let root = name.split('.').next().unwrap_or(name);
                if !self.is_known_type(root, fn_generics, struct_generics) {
                    self.error(format!("未知类型: {name}（位于 {ctx}）"));
                }
            }
            Type::Generic { base, args } => {
                if let Type::Named(base_name) = base.as_ref() {
                    if !self.is_known_type(base_name, fn_generics, struct_generics) {
                        self.error(format!("未知类型: {base_name}（位于 {ctx}）"));
                    }
                }
                for a in args {
                    self.check_type(a, ctx, fn_generics, struct_generics);
                }
            }
            Type::Option(inner)
            | Type::Optional(inner)
            | Type::Ref(inner)
            | Type::MutRef(inner) => self.check_type(inner, ctx, fn_generics, struct_generics),
            Type::Result { ok, err } => {
                self.check_type(ok, ctx, fn_generics, struct_generics);
                self.check_type(err, ctx, fn_generics, struct_generics);
            }
            Type::Fn { params, ret } => {
                for p in params {
                    self.check_type(p, ctx, fn_generics, struct_generics);
                }
                self.check_type(ret, ctx, fn_generics, struct_generics);
            }
            Type::Tuple(ts) => {
                for t in ts {
                    self.check_type(t, ctx, fn_generics, struct_generics);
                }
            }
            Type::Simd { elem, .. } => self.check_type(elem, ctx, fn_generics, struct_generics),
            Type::Duck { fields } => {
                for (_, t) in fields {
                    self.check_type(t, ctx, fn_generics, struct_generics);
                }
            }
            _ => {}
        }
    }

    fn check_function_header(&mut self, f: &Function) {
        // 重复参数
        let mut seen = HashSet::new();
        for p in &f.params {
            if !seen.insert(p.name.clone()) {
                self.error(format!("重复参数名: {}（函数 {})", p.name, f.name));
            }
        }
        // 未知类型（参数 / 返回 / where / 泛型默认）
        for p in &f.params {
            self.check_type(&p.ty, &format!("参数 {} 的类型", p.name), &f.generics, &[]);
        }
        if let Some(rt) = &f.return_type {
            self.check_type(rt, &format!("函数 {} 的返回类型", f.name), &f.generics, &[]);
        }
        for (g, ty) in &f.generic_defaults {
            self.check_type(ty, &format!("泛型 {g} 的默认类型"), &f.generics, &[]);
        }
        for b in &f.where_clause {
            for bt in &b.bounds {
                self.check_type(bt, &format!("where 约束 {b:?}"), &f.generics, &[]);
            }
        }
    }

    fn check_enum_dup_variant(&mut self, s: &StructDef) {
        let mut seen = HashSet::new();
        for f in &s.fields {
            if !seen.insert(f.name.clone()) {
                self.error(format!(
                    "重复枚举变体: enum {} 中变体 {} 出现多次",
                    s.name, f.name
                ));
            }
        }
    }

    fn check_struct_fields_unknown(&mut self, s: &StructDef) {
        for f in &s.fields {
            self.check_type(
                &f.ty,
                &format!("struct {} 字段 {} 的类型", s.name, f.name),
                &[],
                &s.generics,
            );
        }
    }

    // ─────────────────────────── 分析阶段 ───────────────────────────

    fn analyze(&mut self, m: &Module) {
        // 模块根作用域：顶层构建块名是模块级不可变绑定（`BASE =: ...`，函数内可引用）
        self.push_scope();
        for (name, _) in &m.top_level_builds {
            self.bind_mut(name.clone(), false);
        }
        // 顶层 let/const 是模块级绑定（parser 将顶层 let 存入 consts），函数体内可见：先预绑定，再检查函数体
        for c in &m.consts {
            self.bind_mut(c.name.clone(), c.mutable);
        }
        for st in &m.top_stmts {
            match st {
                Stmt::Let { name, mutable, .. } => self.bind_mut(name.clone(), *mutable),
                Stmt::Const { name, .. } => self.bind_mut(name.clone(), false),
                _ => {}
            }
        }
        for f in &m.functions {
            self.check_function_body(f);
        }
        // 结构体内方法
        for s in &m.structs {
            for f in &s.methods {
                self.check_function_body(f);
            }
            for f in &s.magic_methods {
                self.check_function_body(f);
            }
        }
        // impl 方法
        for im in &m.impls {
            for f in &im.methods {
                self.check_function_body(f);
            }
        }
        // trait 方法（默认实现）
        for t in &m.traits {
            for f in &t.methods {
                self.check_function_body(f);
            }
        }
        // 顶层语句 / 构建块 / 测试
        let saved_ctx = self.fn_ctx.take();
        self.fn_ctx = Some(FnCtx {
            name: "<top>".into(),
            return_type: None,
            raises: None,
            has_yield: false,
            has_raise: false,
        });
        self.push_scope();
        for st in &m.top_stmts {
            self.check_stmt(st);
        }
        for (_, body) in &m.top_level_builds {
            self.push_scope();
            for st in body {
                self.check_stmt(st);
            }
            self.pop_scope();
        }
        self.pop_scope();
        for st in &m.tests {
            self.check_stmt(st);
        }
        self.fn_ctx = saved_ctx;
        self.pop_scope(); // 模块根作用域
    }

    fn check_function_body(&mut self, f: &Function) {
        // G7: #[embed(lang)] 函数体为原生代码段（字符串字面量），
        // 原样插入生成产物，不做 LZ 语义/类型检查（保留签名头检查）
        if f.decorators.iter().any(|d| d.name == "embed") {
            return;
        }
        // 嵌套函数：继承外层作用域；yield/raise 检查属于本函数
        let ctx = FnCtx {
            name: f.name.clone(),
            return_type: f.return_type.clone(),
            raises: f.raises.clone(),
            has_yield: false,
            has_raise: false,
        };
        let saved = self.fn_ctx.replace(ctx);
        let saved_loop = self.loop_depth;
        self.loop_depth = 0;
        self.push_scope();
        // 函数泛型参数（`def collect<C: ...>`）在函数体内可作为类型名引用
        // （如 `C.from_iter(self)`），需绑定到当前作用域
        for g in &f.generics {
            self.bind(g.clone());
        }
        for p in &f.params {
            self.bind(p.name.clone());
        }
        // 变参注入名：`..` → args / kwargs（函数体内可直接引用）
        match &f.variadic {
            VariadicMode::ArgsOnly { .. } => self.bind("args".to_string()),
            VariadicMode::KwargsOnly { .. } => self.bind("kwargs".to_string()),
            VariadicMode::Both { .. } => {
                self.bind("args".to_string());
                self.bind("kwargs".to_string());
            }
            VariadicMode::None => {}
        }
        // def f[ps: __Params](...) — ps 接收 &mut __Params，函数体内可引用
        if let Some(cp) = &f.checker_param {
            self.bind(cp.clone());
        }
        for st in &f.body {
            self.check_stmt(st);
        }
        // G2: 函数体尾部表达式（隐式返回）也做返回类型字面量匹配检查
        if let Some(Stmt::Expr(e)) = f.body.last() {
            self.check_return_literal(e);
        }
        let cur = self.fn_ctx.as_ref().unwrap().clone();
        if cur.has_yield && f.return_type.is_some() && !f.is_iterator {
            self.error(format!(
                "函数 {} 声明了返回类型但在函数体内使用 yield（yield 只允许用于无返回类型/iterator 的生成器）",
                f.name
            ));
        }
        // Never 返回类型（发散函数）隐含允许 raise，无需 raises 声明
        let is_never = matches!(&f.return_type, Some(Type::Never))
            || matches!(&f.return_type, Some(Type::Named(n)) if n == "Never" || n == "never");
        if cur.has_raise && f.raises.is_none() && !is_never {
            self.error(format!(
                "函数 {} 体内使用了 raise 但未在签名中声明 raises（raise 必须声明 raises 类型）",
                f.name
            ));
        }
        self.pop_scope();
        self.loop_depth = saved_loop;
        self.fn_ctx = saved;
    }

    fn bind(&mut self, name: String) {
        self.bind_mut(name, true);
    }

    fn bind_mut(&mut self, name: String, mutable: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, mutable);
        }
    }

    fn is_bound(&self, name: &str) -> bool {
        if builtin_value_names().contains(name)
            || builtin_type_names().contains(name)
            || self.imported_names.contains(name)
            || self.fn_names.contains(name)
            || self.type_names.contains(name)
            || self.enum_names.contains(name)
            || self.enum_variants.contains(name)
        {
            return true;
        }
        self.scopes.iter().rev().any(|s| s.contains_key(name))
    }

    /// 查询绑定是否可变（仅查最近作用域链上明确绑定为不可变的 let/const）
    fn binding_immutable(&self, name: &str) -> bool {
        for s in self.scopes.iter().rev() {
            if let Some(m) = s.get(name) {
                return !*m;
            }
        }
        false
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// 收集 pattern 中的绑定名
    fn pattern_bindings(&self, p: &Pattern, out: &mut Vec<String>) {
        match p {
            Pattern::Ident(n) => out.push(n.clone()),
            Pattern::RefMutIdent(n) => out.push(n.clone()),
            Pattern::Variant(_, sub) => {
                for s in sub {
                    self.pattern_bindings(s, out);
                }
            }
            Pattern::Tuple(ps) | Pattern::List(ps) => {
                for s in ps {
                    self.pattern_bindings(s, out);
                }
            }
            Pattern::Dict(ps) => {
                for (_, s) in ps {
                    self.pattern_bindings(s, out);
                }
            }
            Pattern::Rest(Some(n)) => out.push(n.clone()),
            _ => {}
        }
    }

    fn check_stmt(&mut self, st: &Stmt) {
        match st {
            Stmt::Expr(e) => self.check_expr(e),
            Stmt::Let { name, value, mutable, .. } => {
                self.check_expr(value);
                self.bind_mut(name.clone(), *mutable);
            }
            Stmt::Const { name, value, .. } => {
                self.check_expr(value);
                self.bind_mut(name.clone(), false);
            }
            Stmt::Return(Some(e)) => {
                self.check_expr(e);
                self.check_return_literal(e);
            }
            Stmt::Return(None) => {}
            Stmt::Yield(e) => {
                if let Some(c) = self.fn_ctx.as_mut() {
                    c.has_yield = true;
                }
                if let Some(e) = e {
                    self.check_expr(e);
                }
            }
            Stmt::YieldFrom(e) => {
                if let Some(c) = self.fn_ctx.as_mut() {
                    c.has_yield = true;
                }
                self.check_expr(e);
            }
            Stmt::While {
                cond, body, else_body, ..
            } => {
                self.check_expr(cond);
                self.loop_depth += 1;
                self.push_scope();
                for s in body {
                    self.check_stmt(s);
                }
                self.pop_scope();
                self.loop_depth -= 1;
                if let Some(eb) = else_body {
                    self.check_block(eb);
                }
            }
            Stmt::WhileLet {
                pattern, expr, body, else_body, ..
            } => {
                self.check_expr(expr);
                let mut binds = Vec::new();
                self.pattern_bindings(pattern, &mut binds);
                self.loop_depth += 1;
                self.push_scope();
                for b in binds {
                    self.bind(b);
                }
                for s in body {
                    self.check_stmt(s);
                }
                self.pop_scope();
                self.loop_depth -= 1;
                if let Some(eb) = else_body {
                    self.check_block(eb);
                }
            }
            Stmt::For {
                var,
                iter,
                body,
                else_body,
                ..
            } => {
                self.check_expr(iter);
                self.loop_depth += 1;
                self.push_scope();
                // `for (a, b) in ...` / `for a, b in ...`：元组解构多变量
                let vt = var.trim();
                let parts: Vec<&str> = if (vt.starts_with('(') && vt.ends_with(')'))
                    || vt.contains(',')
                {
                    let inner = vt
                        .strip_prefix('(')
                        .and_then(|s| s.strip_suffix(')'))
                        .unwrap_or(vt);
                    inner.split(',').map(|s| s.trim()).collect()
                } else {
                    vec![vt]
                };
                for part in parts {
                    if !part.is_empty() {
                        self.bind(part.to_string());
                    }
                }
                for s in body {
                    self.check_stmt(s);
                }
                self.pop_scope();
                self.loop_depth -= 1;
                if let Some(eb) = else_body {
                    self.check_block(eb);
                }
            }
            Stmt::Loop(body) => {
                self.loop_depth += 1;
                self.push_scope();
                for s in body {
                    self.check_stmt(s);
                }
                self.pop_scope();
                self.loop_depth -= 1;
            }
            Stmt::Break(Some(Expr::Ident(_))) => {
                // 命名跳出（break NAME）：跳出命名块/循环标签，非 break 表达式，
                // 不要求循环上下文（05b-block命名块.md 允许 break NAME 跳出词法祖先块）
            }
            Stmt::Break(Some(e)) => {
                // 带值的 break（break 表达式 / 循环带值）：需在循环内
                if self.loop_depth == 0 {
                    self.error("break 语句出现在循环（for/while/loop）之外".into());
                }
                self.check_expr(e);
            }
            Stmt::Break(None) => {
                if self.loop_depth == 0 {
                    self.error("break 语句出现在循环（for/while/loop）之外".into());
                }
            }
            Stmt::BreakLabel { value, .. } => {
                if let Some(v) = value {
                    self.check_expr(v);
                }
            }
            Stmt::Continue => {
                if self.loop_depth == 0 {
                    self.error("continue 语句出现在循环（for/while/loop）之外".into());
                }
            }
            Stmt::Block { body, .. } => {
                self.push_scope();
                for s in body {
                    self.check_stmt(s);
                }
                self.pop_scope();
            }
            Stmt::CheckerBlock { ps_name, body, .. } => {
                self.push_scope();
                // block NAME[ps: __Params] — ps 是块内可用变量（未显式命名时默认 ps）
                self.bind(ps_name.clone().unwrap_or_else(|| "ps".to_string()));
                for s in body {
                    self.check_stmt(s);
                }
                self.pop_scope();
            }
            Stmt::Defer(body) => self.check_block(body),
            Stmt::Raise(e) => {
                // raise 位于函数内 try/catch 的捕获范围内时，不会向函数外传播，
                // 无需在签名中声明 raises
                if self.catch_depth == 0 {
                    // 字符串字面量 raise（raise "message"）为消息式错误，可免 raises 声明；
                    // 类型化 raise（raise ErrorType(...)）仍必须声明 raises
                    let is_str_raise = matches!(e, Expr::StrLit(_) | Expr::FStrLit(_) | Expr::RawStrLit(_));
                    if let Some(c) = self.fn_ctx.as_mut() {
                        if !is_str_raise {
                            c.has_raise = true;
                        }
                    }
                }
                self.check_expr(e);
            }
            Stmt::Guard {
                cond,
                let_binding,
                success_expr,
                else_body,
            } => {
                if let Some(c) = cond {
                    self.check_expr(c);
                }
                if let Some((pat, e)) = let_binding {
                    self.check_expr(e);
                    let mut binds = Vec::new();
                    self.pattern_bindings(pat, &mut binds);
                    for b in binds {
                        self.bind(b);
                    }
                }
                if let Some(s) = success_expr {
                    self.check_expr(s);
                }
                self.check_block(else_body);
            }
            Stmt::With { expr, alias, body } => {
                self.check_expr(expr);
                self.push_scope();
                if let Some(a) = alias {
                    self.bind(a.clone());
                }
                for s in body {
                    self.check_stmt(s);
                }
                self.pop_scope();
            }
            Stmt::BlockCall { args, .. } => self.check_expr(args),
            Stmt::EnumDef(s) => {
                self.check_enum_dup_variant(s);
                // 函数内 enum 定义同样注册枚举名（pattern_more.lz 在函数内定义
                // enum Command 后于 match 模式 Command.Move(..) 中引用，否则误报未绑定）
                self.enum_names.insert(s.name.clone());
                self.type_names.insert(s.name.clone());
                for f in &s.fields {
                    self.enum_variants.insert(f.name.clone());
                    self.check_type(&f.ty, &format!("enum {} 变体类型", s.name), &[], &s.generics);
                }
            }
            Stmt::Assign { target, value, .. } => {
                // G2: 对不可变绑定赋值（`let x = 1; x = 2`）→ 报错
                if let Expr::Ident(name) = target {
                    if self.binding_immutable(name.as_str()) {
                        self.error(format!("对不可变绑定 `{name}` 赋值（声明时未使用 mut）"));
                    }
                }
                self.check_expr(target);
                self.check_expr(value);
            }
            Stmt::FnDef { func } => {
                self.check_function_header(func);
                // 嵌套函数：将函数名绑定到当前作用域（供后续表达式引用）
                self.bind(func.name.clone());
                self.check_function_body(func);
            }
            Stmt::Pass => {}
            Stmt::Test { body, .. } => self.check_block(body),
            Stmt::Assert { expr, expected } => {
                self.check_expr(expr);
                if let Some(e) = expected {
                    self.check_expr(e);
                }
            }
            Stmt::Check { expr, message } => {
                self.check_expr(expr);
                if let Some(m) = message {
                    self.check_expr(m);
                }
            }
            Stmt::Suite {
                setup, teardown, tests, ..
            } => {
                // setup/teardown/tests 共享同一作用域（setup 绑定可在 tests 中引用）
                self.push_scope();
                if let Some(s) = setup {
                    for st in s {
                        self.check_stmt(st);
                    }
                }
                if let Some(t) = teardown {
                    for st in t {
                        self.check_stmt(st);
                    }
                }
                for st in tests {
                    self.check_stmt(st);
                }
                self.pop_scope();
            }
            Stmt::Comptime { body } => self.check_block(body),
            Stmt::TypeAlias { ty, .. } => {
                self.check_type(ty, "局部类型别名", &[], &[]);
            }
            Stmt::LetTuple { names, value, .. } => {
                self.check_expr(value);
                for n in names {
                    self.bind(n.clone());
                }
            }
        }
    }

    fn check_block(&mut self, body: &[Stmt]) {
        self.push_scope();
        for s in body {
            self.check_stmt(s);
        }
        self.pop_scope();
    }

    /// 返回字面量与声明返回类型不匹配（仅处理可直接判定的字面量，其他放行）
    fn check_return_literal(&mut self, e: &Expr) {
        let Some(ctx) = self.fn_ctx.as_ref() else { return };
        let Some(ret) = &ctx.return_type else { return };
        let lit_kind = match e {
            Expr::IntLit(_) => Some("int"),
            Expr::FloatLit(_) => Some("float"),
            Expr::StrLit(_) | Expr::FStrLit(_) | Expr::RawStrLit(_) => Some("str"),
            Expr::BoolLit(_) => Some("bool"),
            Expr::NoneLit => Some("None"),
            _ => None,
        };
        let Some(kind) = lit_kind else { return };
        let ok = match ret {
            Type::Int => kind == "int",
            Type::Float | Type::F64 => kind == "float",
            Type::Str => kind == "str",
            Type::Bool => kind == "bool",
            Type::None_ | Type::Unit => kind == "None",
            _ => true,
        };
        if !ok {
            self.error(format!(
                "返回类型不匹配: 函数 {} 声明返回 {ret:?}，但返回了 {kind} 字面量",
                ctx.name
            ));
        }
    }

    fn check_expr(&mut self, e: &Expr) {
        match e {
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StrLit(_)
            | Expr::FStrLit(_)
            | Expr::RawStrLit(_)
            | Expr::BoolLit(_)
            | Expr::NoneLit => {}
            Expr::Ident(name) => {
                if !self.is_bound(name) {
                    self.error(format!("未绑定变量: {name}"));
                }
            }
            Expr::ListLit(items) => {
                for it in items {
                    self.check_expr(it);
                }
            }
            Expr::DictLit(pairs) => {
                for (k, v) in pairs {
                    self.check_expr(k);
                    self.check_expr(v);
                }
            }
            Expr::SetLit(items) => {
                for it in items {
                    self.check_expr(it);
                }
            }
            Expr::TupleLit(items) => {
                for it in items {
                    self.check_expr(it);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.check_expr(left);
                // 对字面量/容器做解引用（如 `1 +* 2` → `1 + (*2)`）→ 拒绝
                if matches!(right.as_ref(), Expr::Unary { op: UnaryOp::Deref, .. }) {
                    if let Expr::Unary { operand, .. } = right.as_ref() {
                        if is_literal_expr(operand) {
                            self.error("非法表达式: 对字面量解引用（如 `+*` 这类非法运算符）".into());
                        }
                    }
                }
                self.check_expr(right);
            }
            Expr::Unary { op, operand } => {
                if *op == UnaryOp::Deref && is_literal_expr(operand) {
                    self.error("非法表达式: 对字面量解引用".into());
                }
                self.check_expr(operand);
            }
            Expr::Call { func, args, type_args, .. } => {
                // func 位置不检查 Ident（允许调用尚未显式绑定的函数/宏名）
                if !matches!(func.as_ref(), Expr::Ident(_)) {
                    self.check_expr(func);
                }
                // G2: 同模块函数调用参数个数不匹配（`def f(a); f(1,2)`）→ 报错
                if let Expr::Ident(name) = func.as_ref() {
                    let sig = self
                        .fn_sigs
                        .get(name)
                        .map(|s| (s.param_count, s.param_count_min, s.generic_count, s.variadic, s.collect_list));
                    if let Some((param_count, param_count_min, generic_count, variadic, collect_list)) = sig {
                        let has_kwarg = args.iter().any(|a| matches!(a, Expr::KwArg { .. }));
                        // 默认参数可省略：参数个数 ∈ [必需数, 总数]；
                        // 变参（`..`）上限不限；安全收集（最后参数 List<T>）上限不限且允许省略该 List 参数
                        if !has_kwarg {
                            let lower = if collect_list {
                                param_count_min.saturating_sub(1)
                            } else {
                                param_count_min
                            };
                            let upper_ok = variadic || collect_list || args.len() <= param_count;
                            if args.len() < lower || !upper_ok {
                                let need = if variadic || collect_list {
                                    format!("至少 {param_count_min} 个参数")
                                } else if param_count_min == param_count {
                                    format!("需要 {param_count} 个参数")
                                } else {
                                    format!("需要 {param_count_min}~{param_count} 个参数")
                                };
                                self.error(format!(
                                    "调用参数个数不匹配: 函数 {name} {need}，但传入了 {} 个",
                                    args.len()
                                ));
                            }
                        }
                        // 泛型函数调用：显式类型参数缺失时，仅当无实参可推断（空调用）才报错；
                        // 有实参时允许类型推断（typer 层负责推断）
                        if generic_count > 0 && type_args.is_empty() && args.is_empty() {
                            self.error(format!(
                                "泛型函数 {name} 调用缺少类型参数且无实参可推断（请使用 `{name}<...>` 显式指定 {generic_count} 个类型参数）"
                            ));
                        }
                    }
                }
                // __as__(value, type_name)：第二参数是类型名（如 str→String），不做变量绑定检查
                let callee_name = match func.as_ref() {
                    Expr::Ident(n) => Some(n.as_str()),
                    _ => None,
                };
                for (idx, a) in args.iter().enumerate() {
                    if callee_name == Some("__as__") && idx == 1 {
                        continue;
                    }
                    self.check_expr(a);
                }
            }
            Expr::KwArg { value, .. } => self.check_expr(value),
            Expr::MethodCall { receiver, args, .. } => {
                self.check_expr(receiver);
                for a in args {
                    self.check_expr(a);
                }
            }
            Expr::FieldAccess { receiver, .. } => self.check_expr(receiver),
            Expr::PathAccess { receiver, .. } => self.check_expr(receiver),
            Expr::Index { receiver, index } => {
                self.check_expr(receiver);
                self.check_expr(index);
            }
            Expr::If {
                cond,
                then_body,
                elif_clauses,
                else_body,
            } => {
                self.check_expr(cond);
                self.check_block(then_body);
                for (c, b) in elif_clauses {
                    self.check_expr(c);
                    self.check_block(b);
                }
                if let Some(eb) = else_body {
                    self.check_block(eb);
                }
            }
            Expr::Match { expr, arms } => {
                self.check_expr(expr);
                let mut seen = HashSet::new();
                for arm in arms {
                    let key = pattern_key(&arm.pattern);
                    if let Some(k) = key {
                        if !seen.insert(k.clone()) {
                            self.error(format!("match 分支重复: 模式 {k} 出现多次"));
                        }
                    }
                    self.push_scope();
                    let mut binds = Vec::new();
                    self.pattern_bindings(&arm.pattern, &mut binds);
                    for b in binds {
                        self.bind(b);
                    }
                    // match 守卫可引用模式绑定的变量（如 case x if x > 7），须先 bind 再检查守卫
                    if let Some(g) = &arm.guard {
                        self.check_expr(g);
                    }
                    for s in &arm.body {
                        self.check_stmt(s);
                    }
                    self.pop_scope();
                }
            }
            Expr::Closure {
                params,
                param_tys,
                ret_ty,
                body,
            } => {
                self.push_scope();
                for (i, p) in params.iter().enumerate() {
                    self.bind(p.clone());
                    if let Some(Some(ty)) = param_tys.get(i) {
                        self.check_type(ty, &format!("闭包参数 {p} 的类型"), &[], &[]);
                    }
                }
                if let Some(ty) = ret_ty {
                    self.check_type(ty, "闭包返回类型", &[], &[]);
                }
                self.check_expr(body);
                self.pop_scope();
            }
            Expr::BlockExpr(body) => self.check_block(body),
            Expr::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.check_expr(s);
                }
                if let Some(e) = end {
                    self.check_expr(e);
                }
            }
            Expr::Walrus { target, value } => {
                self.check_expr(value);
                if let Expr::Ident(n) = target.as_ref() {
                    self.bind(n.clone());
                } else {
                    self.check_expr(target);
                }
            }
            Expr::Pipe {
                receiver, callee, args, ..
            } => {
                self.check_expr(receiver);
                self.check_expr(callee);
                for a in args {
                    self.check_expr(a);
                }
            }
            Expr::SafeNav { receiver, .. } => self.check_expr(receiver),
            Expr::Try(inner) => self.check_expr(inner),
            Expr::NullCoalesce { left, right } => {
                self.check_expr(left);
                self.check_expr(right);
            }
            Expr::ListComprehension {
                output,
                var,
                iter,
                cond,
                extra_clauses,
            }
            | Expr::SetComprehension {
                elem: output,
                var,
                iter,
                cond,
                extra_clauses,
            } => {
                self.check_expr(iter);
                self.push_scope();
                self.bind(var.clone());
                // 多 for 子句：先注册所有 extra 循环变量，再检查 output/cond，
                // 否则 output 引用 extra 变量（如 [(x, y) for x in .. for y in ..] 中的 y）会被误报未绑定
                for (v, it, c) in extra_clauses {
                    self.bind(v.clone());
                    self.check_expr(it);
                    if let Some(cc) = c {
                        self.check_expr(cc);
                    }
                }
                if let Some(c) = cond {
                    self.check_expr(c);
                }
                self.check_expr(output);
                self.pop_scope();
            }
            Expr::DictComprehension {
                key,
                value,
                var,
                iter,
                cond,
                extra_clauses,
            } => {
                self.check_expr(iter);
                self.push_scope();
                self.bind(var.clone());
                // 多 for 子句：先注册所有 extra 循环变量，再检查 key/value/cond
                for (v, it, c) in extra_clauses {
                    self.bind(v.clone());
                    self.check_expr(it);
                    if let Some(cc) = c {
                        self.check_expr(cc);
                    }
                }
                if let Some(c) = cond {
                    self.check_expr(c);
                }
                self.check_expr(key);
                self.check_expr(value);
                self.pop_scope();
            }
            Expr::Assign { target, value, .. } => {
                if !matches!(target.as_ref(), Expr::Ident(_)) {
                    self.check_expr(target);
                }
                self.check_expr(value);
            }
            Expr::Spawn(inner) | Expr::Move(inner) | Expr::Panic(inner) | Expr::Await(inner) => {
                self.check_expr(inner);
            }
            Expr::BuildBlock { lhs, body, .. } => {
                // 构建块 lhs 是绑定（`x =:` / `x ~:`）：先绑定再检查 body
                if let Expr::Ident(name) = lhs.as_ref() {
                    self.bind_mut(name.clone(), false);
                } else {
                    self.check_expr(lhs);
                }
                self.check_block(body);
            }
            Expr::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                // 进入有 catch 的 try 块：内部 raise 视为被函数内捕获
                if !catches.is_empty() {
                    self.catch_depth += 1;
                }
                self.check_block(body);
                if !catches.is_empty() {
                    self.catch_depth -= 1;
                }
                for arm in catches {
                    if let Some(g) = &arm.guard {
                        self.check_expr(g);
                    }
                    self.push_scope();
                    let mut binds = Vec::new();
                    self.pattern_bindings(&arm.pattern, &mut binds);
                    for b in binds {
                        self.bind(b);
                    }
                    for s in &arm.body {
                        self.check_stmt(s);
                    }
                    self.pop_scope();
                }
                if let Some(eb) = else_body {
                    self.check_block(eb);
                }
                if let Some(fb) = finally_body {
                    self.check_block(fb);
                }
            }
            Expr::Paren(inner) => self.check_expr(inner),
            Expr::Comptime(inner) => self.check_expr(inner),
        }
    }
}

/// 判断是否为纯字面量（无副作用、可静态判定类型）
fn is_literal_expr(e: &Expr) -> bool {
    matches!(
        e,
        Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StrLit(_)
            | Expr::FStrLit(_)
            | Expr::RawStrLit(_)
            | Expr::BoolLit(_)
            | Expr::NoneLit
            | Expr::ListLit(_)
            | Expr::DictLit(_)
            | Expr::SetLit(_)
            | Expr::TupleLit(_)
    )
}

/// match 模式去重键（仅对可直接判定的字面量/标识符）
fn pattern_key(p: &Pattern) -> Option<String> {
    match p {
        Pattern::Int(v) => Some(format!("int:{v}")),
        Pattern::Str(s) => Some(format!("str:{s}")),
        Pattern::Bool(b) => Some(format!("bool:{b}")),
        // 裸标识符模式是绑定模式（case x if ...），每个分支独立作用域，重复合法；
        // 与守卫组合时尤其常见（case s if s>=90 / case s if s>=75），不做去重
        Pattern::Ident(_) | Pattern::RefMutIdent(_) | Pattern::Wildcard => None,
        _ => None,
    }
}

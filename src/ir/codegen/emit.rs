// Lang-Zone 编译器 — ir/codegen/emit.rs
// （由 mod.rs move-only 拆出，逻辑零改动）

use super::types_emit::is_bigint_ty;
use super::types_emit::is_complex_ty;
use super::types_emit::BIGINT_RS;
use super::types_emit::BIGINT_ZERO;
use super::types_emit::COMPLEX_RS;
use super::types_emit::COMPLEX_ZERO;
use super::*;

impl CodeGen {
    /// 分析跨函数全局可变变量：在函数 A 中引用但未在 A 声明的变量，
    /// 若在另一个函数中作为局部变量声明，则视为模块级全局变量
    pub(crate) fn analyze_global_vars(
        &mut self,
        module: &IrModule,
        const_names: &std::collections::HashSet<String>,
    ) {
        // 收集每个函数声明的局部变量名（参数 + 局部 let + 闭包参数）
        let mut fn_locals: std::collections::HashMap<String, std::collections::HashSet<String>> =
            std::collections::HashMap::new();
        let mut fn_refs: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for item in &module.items {
            if let Item::FnDef(f) = item {
                let mut locals: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                for p in &f.params {
                    locals.insert(p.name.clone());
                }
                // 递归收集局部 let 绑定名（含闭包参数遮蔽）
                collect_local_lets(&f.body, &mut locals);
                fn_locals.insert(f.name.clone(), locals);

                let mut refs: Vec<String> = Vec::new();
                // 收集引用的自由变量（排除闭包参数遮蔽）
                collect_var_refs(&f.body, &mut std::collections::HashSet::new(), &mut refs);
                fn_refs.insert(f.name.clone(), refs);
            }
        }

        let known: std::collections::HashSet<String> = self.top_level_static_names.clone();
        // 枚举变体名（Less/Equal/Greater 等）不是变量——排除，否则被误判为
        // 跨函数全局变量生成 `static mut Less`，导致模式匹配重命名冲突（E0416）
        // 及枚举变体引用错误（E0423 expected value, found enum）
        let mut enum_variant_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for item in &module.items {
            if let Item::EnumDef(e) = item {
                for v in &e.variants {
                    enum_variant_names.insert(v.name.clone());
                }
            }
        }
        let mut candidates: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (fname, refs) in &fn_refs {
            let locals = fn_locals.get(fname).cloned().unwrap_or_default();
            for rname in refs {
                if locals.contains(rname.as_str()) {
                    continue;
                }
                if const_names.contains(rname.as_str()) {
                    continue;
                }
                if known.contains(rname.as_str()) {
                    continue;
                }
                if enum_variant_names.contains(rname.as_str()) {
                    continue;
                }
                if rname == "self" || rname == "self_" || rname == "pass" || rname == "_" {
                    continue;
                }
                if rname.starts_with('_') && rname != "_" {
                    continue;
                }
                // 排除枚举变体/字面量名（None/Some/Ok/Err/true/false）—— 非变量
                if matches!(
                    rname.as_str(),
                    "None" | "Some" | "Ok" | "Err" | "true" | "false" | "pass"
                ) {
                    continue;
                }
                candidates.insert(rname.clone());
            }
        }

        // 仅当变量在另一个函数中作为局部变量声明时，才视为全局
        for c in candidates {
            let declared_elsewhere = fn_locals.values().any(|l| l.contains(c.as_str()));
            if declared_elsewhere {
                let ty = self.infer_global_type(module, &c);
                self.global_vars.insert(c, ty);
            }
        }
    }

    /// 生成全局变量的 const 兼容默认值（不能调用 Default::default，非 const-stable）
    pub(crate) fn const_default_value(&self, ty: &IrType) -> String {
        match ty {
            IrType::Int => "0".into(),
            IrType::Int128 => "0i128".into(),
            // BUG-3：`BigInt::from(0)` 不是 const fn，用在 const/static 默认值位
            // 直接 E0015 ⇒ 用 `BigInt::ZERO`（num-bigint 0.4 的关联常量）
            IrType::BigInt => BIGINT_ZERO.into(),
            IrType::Complex => COMPLEX_ZERO.into(),
            IrType::F64 => "0.0".into(),
            IrType::Bool => "false".into(),
            IrType::Str => "String::new()".into(),
            IrType::Named { path, .. } => match path.as_str() {
                "String" => "String::new()".into(),
                "Vec" | "List" => "Vec::new()".into(),
                "HashMap" | "Dict" => "std::collections::BTreeMap::new()".into(),
                "HashSet" | "Set" => "std::collections::HashSet::new()".into(),
                // bigint/complex 注解在 IR 里也常以 Named 形态出现
                _ if is_bigint_ty(ty) => BIGINT_ZERO.into(),
                _ if is_complex_ty(ty) => COMPLEX_ZERO.into(),
                _ => "0".into(),
            },
            _ => "0".into(),
        }
    }

    /// 从模块中推断全局变量类型
    pub(crate) fn infer_global_type(&self, module: &IrModule, name: &str) -> IrType {
        for item in &module.items {
            if let Item::FnDef(f) = item {
                let t = infer_global_type(&f.body, name, &f.params);
                if t != IrType::Any {
                    return t;
                }
            }
        }
        IrType::Int
    }

    // ── 辅助方法 ──

    pub(crate) fn pad(&self) -> String {
        "    ".repeat(self.indent)
    }

    #[allow(dead_code)]
    pub(crate) fn emit(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    pub(crate) fn emit_line(&mut self, s: &str) {
        self.buf.push_str(&self.pad());
        self.buf.push_str(s);
        self.buf.push('\n');
    }

    /// 返回最后发射的一行（不含缩进和前导空白）
    pub(crate) fn last_emitted_line(&self) -> &str {
        let trimmed = self.buf.trim_end();
        trimmed.rsplit('\n').next().unwrap_or("").trim_start()
    }

    /// 在最后发射的一行末尾追加文本
    pub(crate) fn append_to_last_line(&mut self, s: &str) {
        let len = self.buf.trim_end().len();
        self.buf.insert_str(len, s);
    }

    /// 从表达式中收集 walrus 变量名（用于预声明）
    pub(crate) fn collect_walrus_vars(expr: &Expr, vars: &mut Vec<(String, IrType)>) {
        match &expr.kind {
            ExprKind::StructCtor { name, fields } if name == "_Walrus" => {
                if let Some((_, bind_expr)) = fields.iter().find(|(n, _)| n == "_bind") {
                    if let ExprKind::Var(v) = &bind_expr.kind {
                        // walrus 绑定变量类型来自值表达式（first := values.first()
                        // → Option<i64>，非 i64）；否则硬编码 i64 会 E0308
                        let val_ty = fields
                            .iter()
                            .find(|(n, _)| n == "_val")
                            .map(|(_, v)| v.ty.clone())
                            .unwrap_or(IrType::Int);
                        if !vars.iter().any(|(n, _)| n == v) {
                            vars.push((v.clone(), val_ty));
                        }
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
                for a in args {
                    Self::collect_walrus_vars(a, vars);
                }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                Self::collect_walrus_vars(receiver, vars);
                for a in args {
                    Self::collect_walrus_vars(a, vars);
                }
            }
            ExprKind::IfExpr { cond, then, els } => {
                Self::collect_walrus_vars(cond, vars);
                Self::collect_walrus_vars(then, vars);
                Self::collect_walrus_vars(els, vars);
            }
            ExprKind::Paren(inner) | ExprKind::ImplicitConvert { source: inner, .. } => {
                Self::collect_walrus_vars(inner, vars);
            }
            _ => {}
        }
    }

    /// 为 walrus 变量生成预声明: let mut n: i64;
    pub(crate) fn emit_walrus_predecls(&mut self, cond: &Expr) {
        let mut vars = Vec::new();
        Self::collect_walrus_vars(cond, &mut vars);
        for (v, ty) in &vars {
            // 用 walrus 绑定值的实际类型声明（first := values.first() → Option<i64>），
            // 否则硬编码 i64 与后续 Option 值比较时 E0308
            let ty_str = match ty {
                IrType::Int => "i64".to_string(),
                IrType::Int128 => "i128".to_string(),
                IrType::BigInt => BIGINT_RS.to_string(),
                IrType::Complex => COMPLEX_RS.to_string(),
                IrType::F64 => "f64".to_string(),
                IrType::Bool => "bool".to_string(),
                IrType::Str => "String".to_string(),
                IrType::Option(inner) => {
                    format!("Option<{}>", self.rust_type(inner))
                }
                IrType::Result { ok, err } => {
                    format!("Result<{}, {}>", self.rust_type(ok), self.rust_type(err))
                }
                IrType::Any => "i64".to_string(),
                other => self.rust_type(other),
            };
            // walrus 变量带默认值初始化：`(n := compute()) > 5 if n * 10 else 0` 中
            // 三元条件 `n * 10` 在 walrus 赋值（then 分支内）之前求值，未初始化变量
            // 报 E0381（combo_ternary_walrus.lz）。默认值仅为让 Rust 编译通过，
            // 实际值在 walrus 表达式求值时被覆盖。
            let default = match ty {
                IrType::Int => "0i64".to_string(),
                IrType::F64 => "0.0".to_string(),
                IrType::Bool => "false".to_string(),
                IrType::Str => "String::new()".to_string(),
                IrType::Option(_) => "None".to_string(),
                IrType::Result { .. } => "Err(\"lz_walrus_default\".to_string())".to_string(),
                _ => String::new(),
            };
            if default.is_empty() {
                self.emit_line(&format!("let mut {}: {};", v, ty_str));
            } else {
                self.emit_line(&format!("let mut {}: {} = {};", v, ty_str, default));
            }
        }
    }

    pub(crate) fn emit_prelude(&mut self) {
        let uses_ext = self.module_uses_ext;
        self.emit_prelude_base();
        if uses_ext {
            self.emit_ext_preamble();
        }
    }

    pub(crate) fn emit_ext_preamble(&mut self) {
        // 外部专用句柄 ExtHandle（#[extern(lang)] 返回类型，无 extern 关键字）
        self.emit_line("/// 外部专用句柄：#[extern(lang)] 标记入口的统一返回值。");
        self.emit_line("#[derive(Debug, Clone)]");
        self.emit_line("pub struct ExtHandle {");
        self.emit_line("    pub lang: String,       // 目标语言标记（Rust / Python / C）");
        self.emit_line("    pub ptr: usize,         // 外部对象不透明句柄（0 = 错误）");
        self.emit_line("    pub err: Option<String>, // 错误约定：非 None 即为错误");
        self.emit_line("}");
        self.emit_line("impl ExtHandle {");
        self.emit_line("    pub fn ok(lang: &str, ptr: usize) -> Self { Self { lang: lang.to_string(), ptr, err: None } }");
        self.emit_line("    pub fn fail(lang: &str, msg: &str) -> Self { Self { lang: lang.to_string(), ptr: 0, err: Some(msg.to_string()) } }");
        self.emit_line("    pub fn is_err(&self) -> bool { self.err.is_some() || self.ptr == 0 }");
        self.emit_line("    pub fn err_msg(&self) -> Option<&str> { self.err.as_deref() }");
        self.emit_line("}");
        // 分发器：extern 入口经此调用（缺失实现 → 可定位诊断，不静默）
        self.emit_line(
            "fn __lz_ext_call(lang: &str, name: &str, args: Vec<String>) -> ExtHandle {",
        );
        self.emit_line("    let _ = (lang, name, args);");
        self.emit_line("    ExtHandle::fail(lang, &format!(\"no external implementation for '{}' (lang {})\", name, lang))");
        self.emit_line("}");
        self.buf.push('\n');
    }

    pub(crate) fn emit_prelude_base(&mut self) {
        // BUG-10 产品化（方案 A，2026-10-01）：文件头静态链接配方。
        // 确定性文本（不含本机路径）⇒ 跨机稳定、可进 golden 快照。
        // lz_builtins re-export num-bigint/num-complex ⇒ 孤立 rustc 编译
        // 必须带 -L dependency，否则连不用 bigint 的程序也报 E0463。
        // 本机解析后的完整可复制命令由 CLI 在生成后打到 stderr（不落盘）。
        self.emit_line(
            "// ── LZ rustc link recipe (BUG-10: lz_builtins re-exports num-bigint/num-complex) ──",
        );
        self.emit_line("// To compile & run this generated file in isolation:");
        self.emit_line("//   rustc --edition 2021 --extern lz_builtins=<rlib> -L dependency=<deps_dir> -O <this_file>.rs");
        self.emit_line(
            "//   <rlib>     = liblz_builtins.rlib (build it: `cargo build -p lz_builtins`)",
        );
        self.emit_line("//   <deps_dir> = the directory containing that rlib (e.g. <workspace>/target/debug/deps)");
        self.emit_line(
            "// Omitting -L gives error[E0463] even if this program never uses BigInt/Complex.",
        );
        self.emit_line("// ─────────────────────────────────────────────────────────────────────────────────────");
        self.buf.push('\n');
        // Rust 2021 edition support (async/await, etc.)
        // 使用 outer attributes (#[..]) 而非 inner attributes (#![..])
        // 因为 type alias 可能已在 prelude 之前输出，inner attributes 不允许出现在 item 之后
        self.emit_line("#[allow(unused_imports)]");
        self.emit_line("#[allow(unused_variables)]");
        self.emit_line("#[allow(dead_code)]");
        self.emit_line("#[allow(non_snake_case)]");
        self.buf.push('\n');
        // 用户自定义 HashMap/HashSet（如 lib_hashmap 的 struct HashMap）时跳过
        // std 同名导入，否则 E0255 重复定义 + E0107 缺泛型（与 Rc/Arc 守卫同款）。
        // 注意 collections 导入在前、any::Any 在后，保持输出顺序稳定。
        let wants_hashmap = !self.known_types.contains("HashMap");
        let wants_hashset = !self.known_types.contains("HashSet");
        if wants_hashmap && wants_hashset {
            self.emit_line("use std::collections::{HashMap, HashSet};");
        } else if wants_hashmap {
            self.emit_line("use std::collections::HashMap;");
        } else if wants_hashset {
            self.emit_line("use std::collections::HashSet;");
        }
        // Dict → BTreeMap（有序，保证 JSON 序列化字段顺序稳定）——仅当模块用到 Dict/HashMap 时注入
        if self.module_uses_dict {
            self.emit_line("use std::collections::BTreeMap;");
        }
        // BUG-1：不再注入 `use num_bigint::BigInt;` / `use num_complex::Complex64;`。
        // num-* 只是 lz_builtins 的传递依赖，孤立 crate 里这条 `use` 路径解析不到
        // （rustc E0432）；类型串已统一为经 lz_builtins re-export 的
        // `lz_builtins::BigInt` / `lz_builtins::Complex64`（见 BIGINT_RS），
        // 由下方恒发 `use lz_builtins::*;` 联结，无需按模块再判定。
        // 多类型变参位置约束（03d §2.3 `..: Tuple<T1,T2,..>`）的尾部收集
        // args: (T1, T2, Vec<Box<dyn Any>>) 需要 std::any::Any
        self.emit_line("use std::any::Any;");
        // 若模块自定义了 Rc/Arc 类型（如 lz_std/box.lz 的 `struct Rc<T>`/`struct Arc<T>`，
        // LZ 自举标准库自行实现智能指针），跳过 std 同名导入，否则 E0255 重复定义
        if !self.known_types.contains("Rc") {
            self.emit_line("use std::rc::Rc;");
        }
        if !self.known_types.contains("Arc") {
            self.emit_line("use std::sync::Arc;");
        }
        // 修饰符装饰器目标类型（`@mutex`/`@cell`/`@once` 等）所需的 std 导入（T04）。
        // 仅在模块实际使用相应轴时发射，避免影响无装饰器模块的产物（L2/L3 golden 比对）。
        let moddec_imports = std::mem::take(&mut self.moddec_std_imports);
        for (module, items) in &moddec_imports {
            let names: Vec<&str> = items.iter().copied().collect();
            if names.len() == 1 {
                self.emit_line(&format!("use {}::{};", module, names[0]));
            } else {
                self.emit_line(&format!("use {}::{{{}}};", module, names.join(", ")));
            }
        }
        let moddec_atomics = std::mem::take(&mut self.moddec_atomic_types);
        if !moddec_atomics.is_empty() {
            let names: Vec<String> = moddec_atomics.into_iter().collect();
            self.emit_line(&format!("use std::sync::atomic::{{{}}};", names.join(", ")));
        }
        // traits.lz 定义了自定义 trait Debug/Display（LZ 语义）时，不 import
        // std::fmt 的同名 trait，否则 E0255 the name is defined multiple times
        if !self.trait_names.contains("Debug") {
            self.emit_line("use std::fmt::Debug;");
        }
        if !self.trait_names.contains("Display") {
            self.emit_line("use std::fmt::Display;");
        }
        self.buf.push('\n');

        // ── Lang-Zone 运行时 builtins（内部子库导入，避免重复内联）──
        // __Params / __spawn_task / __block_on 及 print/len/range 等全部
        // 由 lz_builtins crate 提供；生成代码仅导入 API。
        self.emit_line("use lz_builtins::*;");
        self.buf.push('\n');

        // 未定义外部函数 stub（p16_lzi 等跨模块探测用例：.lzi 签名未加载时兜底）。
        // 名字取自 lz_builtins 导出函数表之外、且模块内无定义的顶层调用。
        // 含 `::` 的路径调用（如 Struct::__from__ 隐式转换）是模块内已定义方法，
        // 不是外部函数，跳过——否则生成 `fn Wrapper::__from__(...)` 非法 Rust
        //（E0530 invalid path separator in function definition）
        if !self.unknown_extern_fns.is_empty() {
            let mut names: Vec<(String, usize)> = self
                .unknown_extern_fns
                .iter()
                .filter(|(k, _)| !k.contains("::"))
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            names.sort();
            for (name, arity) in names {
                let params: Vec<String> = (0..arity).map(|i| format!("__a{}: i64", i)).collect();
                self.emit_line(&format!(
                    "#[allow(non_snake_case)] #[allow(dead_code)]\nfn {}({}) -> i64 {{ i64::MAX }}",
                    name,
                    params.join(", ")
                ));
            }
            self.buf.push('\n');
        }
    }
}

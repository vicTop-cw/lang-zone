// Lang-Zone 编译器 — ir/codegen/pattern_gen.rs
// （由 mod.rs move-only 拆出，逻辑零改动）

use super::types_emit::bigint_lit_code;
use super::*;

impl CodeGen {
    // ── Pattern 生成 ──

    /// 切片上下文模式生成（type-pack 异质元组 03d §2.8 方案 B）：
    /// `..: Tuple<Ts...>` 的 args 编译为切片 &[Ts]，元组模式 `(a,)` / `(a, ..)`
    /// 需转为 Rust 切片模式 `[a]` / `[a, ..]`（否则 E0308 expected slice, found tuple）。
    /// 臂体绑定 a 为 &Ts（切片元素引用），臂体内自动 .clone() 取值。
    pub(crate) fn gen_slice_pattern(&self, pat: &Pattern) -> String {
        match pat {
            Pattern::Tuple(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.gen_slice_pattern(e)).collect();
                format!("[{}]", elems.join(", "))
            }
            Pattern::Rest(name) => match name {
                Some(n) => format!("{} @ ..", n),
                None => "..".into(),
            },
            Pattern::Ident(name) => name.clone(),
            Pattern::Wildcard => "_".into(),
            other => self.gen_pattern(other),
        }
    }

    /// 收集切片模式绑定名（type-pack 异质元组 03d §2.8 方案 B）：
    /// `[a]` / `[a, ..]` 模式中 a 绑定 &Ts（切片元素引用）
    pub(crate) fn collect_slice_bindings(&self, pat: &Pattern, out: &mut Vec<String>) {
        match pat {
            Pattern::Ident(name) => out.push(name.clone()),
            Pattern::Tuple(elems) | Pattern::List(elems) => {
                for e in elems {
                    self.collect_slice_bindings(e, out);
                }
            }
            Pattern::Rest(name) => {
                if let Some(n) = name {
                    out.push(n.clone());
                }
            }
            _ => {}
        }
    }

    pub(crate) fn gen_pattern(&self, pat: &Pattern) -> String {
        match pat {
            Pattern::Wildcard => "_".into(),
            Pattern::RefMutIdent(name) => {
                // `ref mut c` 模式：c 绑定为 &mut 引用（case Some(ref mut c)）
                // Rust 模式语法为 `Some(ref mut c)`
                if let Some(dot_pos) = name.rfind('.') {
                    let type_name = &name[..dot_pos];
                    let variant = &name[dot_pos + 1..];
                    if self.emitted_types.contains(type_name)
                        || type_name == "Option"
                        || type_name == "Result"
                        || type_name == "Some"
                        || type_name == "None"
                        || type_name == "Ok"
                        || type_name == "Err"
                        || self.enum_variants.contains_key(variant)
                    {
                        format!("{}::{}", type_name, variant)
                    } else {
                        format!("ref mut {}", name)
                    }
                } else {
                    format!("ref mut {}", name)
                }
            }
            Pattern::Ident(name) => {
                // Handle dotted patterns like "Color.Red" → Rust enum pattern "Color::Red"
                if let Some(dot_pos) = name.rfind('.') {
                    let type_name = &name[..dot_pos];
                    let variant = &name[dot_pos + 1..];
                    if self.emitted_types.contains(type_name)
                        || type_name == "Option"
                        || type_name == "Result"
                        || type_name == "Some"
                        || type_name == "None"
                        || type_name == "Ok"
                        || type_name == "Err"
                        || self.enum_variants.contains_key(variant)
                    {
                        format!("{}::{}", type_name, variant)
                    } else {
                        // 检测 pattern 绑定名与模块级 static/global 冲突（E0530）
                        if self.global_vars.contains_key(name.as_str())
                            || self.top_level_static_names.contains(name.as_str())
                        {
                            format!("{}_", name)
                        } else {
                            name.clone()
                        }
                    }
                } else if let Some(enum_name) = self.enum_variants.get(name.as_str()) {
                    // 裸枚举变体名（无点号）：`case Less:` → `Ordering::Less`
                    // （否则 Rust 将 Less 当作标识符绑定，元组模式 `(Less, Less)`
                    // 报 E0416 bound more than once）
                    format!("{}::{}", enum_name, name)
                } else {
                    // 检测 pattern 绑定名与模块级 static/global 冲突（E0530）
                    if self.global_vars.contains_key(name.as_str())
                        || self.top_level_static_names.contains(name.as_str())
                    {
                        format!("{}_", name)
                    } else {
                        name.clone()
                    }
                }
            }
            Pattern::Lit(lit) => {
                // Pattern literals: no .to_string() wrapper
                match lit {
                    LitKind::Int(n) => format!("{}i64", n),
                    LitKind::Int128(n) => {
                        // 检查周围上下文确定目标类型（此处无 ty 参数，保守生成 i128）
                        format!("{}i128", n)
                    }
                    // BUG-4 同类点穷尽：match 模式位**不能**发 BigInt 构造式——
                    // `BigInt::from(..)` 不是合法模式（rustc E0532 pattern 非恒定），
                    // BigInt 也不满足结构相等匹配；发 `from(...)` 只会把 E0308
                    // 换成更难的错。此处保持类型串统一（BUG-1），语义限制记入台账残留。
                    LitKind::BigInt(s) => bigint_lit_code(s),
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
                let fields: Vec<String> = fields
                    .iter()
                    .map(|(n, p)| format!("{}: {}", n, self.gen_pattern(p)))
                    .collect();
                format!("{} {{ {} }}", name, fields.join(", "))
            }
            Pattern::Enum {
                enum_name,
                variant,
                args,
            } => {
                if std::env::var("LZ_DBG_PAT").is_ok() {
                    eprintln!(
                        "PATDBG enum={:?} variant={:?} args={:?} named={:?}",
                        enum_name,
                        variant,
                        args.len(),
                        self.enum_variant_named_fields
                            .get(&(enum_name.clone(), variant.clone()))
                    );
                }
                // 递归字段在模式中不添加 box 关键字（box_patterns 尚未稳定）
                // 由 gen_stmt(Match) 在臂体开头自动插入 let var = *var; 解引用
                if args.is_empty() {
                    format!("{}::{}", enum_name, variant)
                } else {
                    let args: Vec<String> = args.iter().map(|a| self.gen_pattern(a)).collect();
                    let named = self
                        .enum_variant_named_fields
                        .get(&(enum_name.clone(), variant.clone()))
                        .cloned()
                        .unwrap_or_default();
                    if named.len() == args.len() && !named.is_empty() {
                        let pairs: Vec<String> = named
                            .iter()
                            .zip(args.iter())
                            .map(|(f, v)| format!("{}: {}", f, v))
                            .collect();
                        format!("{}::{} {{ {} }}", enum_name, variant, pairs.join(", "))
                    } else {
                        format!("{}::{}({})", enum_name, variant, args.join(", "))
                    }
                }
            }
            Pattern::List(elems) => {
                let elems: Vec<String> = elems.iter().map(|e| self.gen_pattern(e)).collect();
                format!("[{}]", elems.join(", "))
            }
            Pattern::Rest(name) => match name {
                Some(n) => format!("{} @ ..", n),
                None => "..".into(),
            },
            Pattern::Dict(entries) => {
                // 字典模式：Rust 无原生 HashMap 模式，由 Match 语句层
                // 生成 contains_key 守卫 + 值绑定；此处仅作占位
                let _ = entries;
                "_".into()
            }
            Pattern::Range {
                start,
                end,
                inclusive,
            } => {
                if *inclusive {
                    format!("{}i64..={}i64", start, end)
                } else {
                    format!("{}i64..{}i64", start, end)
                }
            }
        }
    }

    /// 收集模式中的所有 Ident 绑定名（用于 catch 模式参数提取等场景）
    pub(crate) fn collect_pattern_idents(&self, pat: &Pattern) -> Vec<String> {
        let mut out = Vec::new();
        match pat {
            Pattern::Ident(name) => out.push(name.clone()),
            Pattern::Tuple(elems) => {
                for e in elems {
                    out.extend(self.collect_pattern_idents(e));
                }
            }
            Pattern::Struct { fields, .. } => {
                for (_, p) in fields {
                    out.extend(self.collect_pattern_idents(p));
                }
            }
            Pattern::Enum { args, .. } => {
                for a in args {
                    out.extend(self.collect_pattern_idents(a));
                }
            }
            _ => {}
        }
        out
    }

    /// 收集 match 臂 `ref mut` 模式绑定名（case Some(ref mut c) → ["c"]），
    /// 供臂体内 c = c + 1 生成 *c = *c + 1 解引用赋值（E0384 修复）
    pub(crate) fn collect_ref_mut_bindings(
        &self,
        pat: &Pattern,
    ) -> std::collections::HashSet<String> {
        let mut out = std::collections::HashSet::new();
        self.collect_ref_mut_inner(pat, &mut out);
        out
    }

    pub(crate) fn collect_ref_mut_inner(
        &self,
        pat: &Pattern,
        out: &mut std::collections::HashSet<String>,
    ) {
        match pat {
            Pattern::RefMutIdent(name) => {
                out.insert(name.clone());
            }
            Pattern::Tuple(elems) | Pattern::List(elems) => {
                for e in elems {
                    self.collect_ref_mut_inner(e, out);
                }
            }
            Pattern::Dict(entries) => {
                for (_, p) in entries {
                    self.collect_ref_mut_inner(p, out);
                }
            }
            Pattern::Enum { args, .. } => {
                for a in args {
                    self.collect_ref_mut_inner(a, out);
                }
            }
            Pattern::Struct { fields, .. } => {
                for (_, p) in fields {
                    self.collect_ref_mut_inner(p, out);
                }
            }
            _ => {}
        }
    }

    /// 收集 Enum 模式中需要 Box 解引用的绑定名（用于插入 let name = *name;）
    pub(crate) fn collect_box_pattern_bindings(&self, pat: &Pattern) -> Vec<String> {
        let mut bindings = Vec::new();
        if let Pattern::Enum {
            enum_name,
            variant,
            args,
        } = pat
        {
            if let Some(field_types) = self
                .enum_variant_fields
                .get(&(enum_name.clone(), variant.clone()))
            {
                for (i, arg_pat) in args.iter().enumerate() {
                    if field_types
                        .get(i)
                        .map_or(false, |ty| type_refers_to(ty, enum_name))
                    {
                        bindings.extend(self.collect_pattern_idents(arg_pat));
                    }
                }
            }
        }
        bindings
    }

    /// 注册枚举变体字段绑定到 str_typed_vars（f-string 用 {} 而非 {:?}）
    pub(crate) fn register_enum_variant_str_bindings(&mut self, pat: &Pattern) {
        if let Pattern::Enum {
            enum_name,
            variant,
            args,
        } = pat
        {
            if let Some(field_types) = self
                .enum_variant_fields
                .get(&(enum_name.clone(), variant.clone()))
            {
                for (i, arg_pat) in args.iter().enumerate() {
                    if let Some(ty) = field_types.get(i) {
                        let is_str = matches!(ty, IrType::Str)
                            || matches!(ty, IrType::Named { path, args: _ } if path == "String");
                        if is_str {
                            for ident in self.collect_pattern_idents(arg_pat) {
                                self.str_typed_vars.insert(ident);
                            }
                        }
                    }
                }
            }
        }
    }
}

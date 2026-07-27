// Lang-Zong 编译器 — codegen/magic.rs
// CodeGenMagicExt trait 扩展

use super::CodeGen;
use super::MagicSelfMode;
use crate::parser::*;
use std::collections::{HashSet, HashMap};
use crate::types::Type;
use crate::magic::*;
use super::func::CodeGenFuncExt;


pub trait CodeGenMagicExt {
    fn magic_duplicate_count(&self, s: &StructDef) -> HashMap<String, usize>;
    fn magic_unique_name(&self, m: &Function, dupes: &HashMap<String, usize>) -> String;
    fn magic_needs_owned_self(&self, magic_name: &str) -> MagicSelfMode;
    fn gen_magic_method_body(&self, m: &Function, unique_name: &str, indent: usize) -> String;
    fn gen_magic_impls(&self, s: &StructDef) -> String;
    fn magic_dispatch_key(&self, entry: &MagicEntry, method: &Function) -> String;
    fn gen_magic_trait_impl(&self, entry: &MagicEntry, method: &Function,
                            type_name: &str, generics: &[String], unique_name: &str) -> String;
}

impl CodeGenMagicExt for CodeGen {
    fn magic_duplicate_count(&self, s: &StructDef) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for m in &s.methods {
            if m.name.starts_with("__") && m.name.ends_with("__") {
                *counts.entry(m.name.clone()).or_insert(0) += 1;
            }
        }
        counts
    }

    /// 为魔法方法生成唯一名称（重复时追加类型后缀）
    fn magic_unique_name(&self, m: &Function, dupes: &HashMap<String, usize>) -> String {
        let count = *dupes.get(&m.name).unwrap_or(&1);
        if count <= 1 {
            return m.name.clone();
        }
        let suffix = if m.params.len() >= 2 {
            self.map_type(&m.params[1].ty)
                .replace("::", "_")
                .replace("<", "_")
                .replace(">", "")
                .replace(", ", "_")
        } else if m.params.len() == 1 {
            self.map_type(&m.params[0].ty)
                .replace("::", "_")
        } else {
            String::from("0")
        };
        let base = m.name.trim_end_matches('_');
        format!("{}__{}", base, suffix)
    }

    fn magic_needs_owned_self(&self, magic_name: &str) -> MagicSelfMode {
        match magic_name {
            "__add__" | "__sub__" | "__mul__" | "__div__" | "__rem__" | "__pow__"
            | "__pipe__"
            | "__neg__" | "__not__"
            | "__bitand__" | "__bitor__" | "__bitxor__" | "__shl__" | "__shr__"
            | "__into__" | "__try_into__" => MagicSelfMode::Owned,
            "__default__" | "__from__" | "__try_from__" => MagicSelfMode::None,
            "__clone__" | "__eq__" | "__ne__" | "__str__" | "__hash__" | "__repr__" | "__getitem__"
            | "__lt__" | "__le__" | "__gt__" | "__ge__" | "__cmp__" => MagicSelfMode::Ref,
            "__drop__" => MagicSelfMode::RefMut,
            _ => MagicSelfMode::RefMut,
        }
    }

    /// 生成魔法方法体（根据 trait 需要调整 self 模式）
    fn gen_magic_method_body(&self, m: &Function, unique_name: &str, indent: usize) -> String {
        let pad = "    ".repeat(indent);
        let params: Vec<String> = m.params.iter().map(|p| {
            if p.name == "self" {
                match self.magic_needs_owned_self(&m.name) {
                    MagicSelfMode::Owned => "mut self".to_string(),
                    MagicSelfMode::Ref => "&self".to_string(),
                    MagicSelfMode::RefMut => "&mut self".to_string(),
                    MagicSelfMode::None => String::new(),
                }
            } else {
                self.gen_param(p)
            }
        }).filter(|s| !s.is_empty()).collect();

        let ret = m.return_type.as_ref()
            .map(|t| format!(" -> {}", self.map_type(t)))
            .unwrap_or_default();

        let mut locals: HashSet<String> = m.params.iter().map(|p| p.name.clone()).collect();
        let body = self.gen_block_return(&m.body, indent + 1, &mut locals);

        format!("{pad}fn {unique_name}({}){ret} {{\n{body}{pad}}}\n",
            params.join(", "))
    }

    /// 检测 struct 内的魔法方法，自动生成对应 Rust trait impl
    fn gen_magic_impls(&self, s: &StructDef) -> String {
        if s.is_enum || s.methods.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        let mut seen: HashSet<String> = HashSet::new();
        let dupes = self.magic_duplicate_count(s);

        for method in &s.methods {
            if !method.name.starts_with("__") || !method.name.ends_with("__") {
                continue;
            }

            if method.name == "__repr__" {
                continue;
            }

            if let Some(entries) = self.magic_engine.resolve(&method.name) {
                for entry in entries {
                    let type_key = self.magic_dispatch_key(entry, method);
                    // PartialEq/PartialOrd: 用 trait_path 去重，避免 __eq__/__ne__ 或 __lt__/__gt__/__le__/__ge__ 生成多个 impl
                    let dedup_key = match entry.kind {
                        MagicKind::PartialEq | MagicKind::PartialOrd => entry.trait_path.to_string(),
                        _ => format!("{}|{}", method.name, type_key),
                    };
                    if seen.contains(&dedup_key) { continue; }
                    seen.insert(dedup_key);

                    let unique_name = self.magic_unique_name(method, &dupes);
                    out.push_str(&self.gen_magic_trait_impl(entry, method, &s.name, &s.generics, &unique_name));
                }
            }
        }
        out
    }

    /// 计算多分派的类型键（用于去重和 trait 类型参数）
    fn magic_dispatch_key(&self, entry: &MagicEntry, method: &Function) -> String {
        match entry.kind {
            // 二元运算符：按 other 参数类型分派
            MagicKind::BinaryOp => {
                if method.params.len() >= 2 {
                    self.map_type(&method.params[1].ty)
                } else {
                    String::new()
                }
            }
            // From/TryFrom：按第一个非 self 参数类型分派
            MagicKind::From => {
                // __from__(value: Source) -> Self → 按 source 类型
                if let Some(first_param) = method.params.first() {
                    if first_param.name != "self" {
                        return self.map_type(&first_param.ty);
                    } else if method.params.len() >= 2 {
                        return self.map_type(&method.params[1].ty);
                    }
                }
                String::new()
            }
            // Into/TryInto：按返回类型分派
            MagicKind::Into => {
                method.return_type.as_ref()
                    .map(|t| self.map_type(t))
                    .unwrap_or_default()
            }
            // Index：按 index 类型分派
            MagicKind::Index => {
                if method.params.len() >= 2 {
                    self.map_type(&method.params[1].ty)
                } else {
                    String::new()
                }
            }
            // 其他不分派
            _ => String::new(),
        }
    }

    /// 为单个魔法方法生成完整的 trait impl
    fn gen_magic_trait_impl(&self, entry: &MagicEntry, method: &Function,
                            type_name: &str, generics: &[String], unique_name: &str) -> String {
        let generics_s = if generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", generics.join(", "))
        };
        let type_params_s = generics_s.clone();

        let mut out = String::new();
        out.push('\n');

        match entry.kind {
            MagicKind::BinaryOp => {
                let rhs_ty = if method.params.len() >= 2 {
                    self.map_type(&method.params[1].ty)
                } else {
                    "()".to_string()
                };
                let ret_ty = method.return_type.as_ref()
                    .map(|t| self.map_type(t))
                    .unwrap_or_else(|| "()".to_string());
                out.push_str(&format!(
                    "impl{} {}<{}> for {}{} {{\n    type Output = {};\n    fn {}(self, other: {}) -> {} {{ self.{}(other) }}\n}}\n",
                    generics_s, entry.trait_path, rhs_ty, type_name, type_params_s,
                    ret_ty,
                    entry.trait_method, rhs_ty, ret_ty,
                    unique_name
                ));
            }

            MagicKind::UnaryOp => {
                let ret_ty = method.return_type.as_ref()
                    .map(|t| self.map_type(t))
                    .unwrap_or_else(|| "()".to_string());
                out.push_str(&format!(
                    "impl{} {} for {}{} {{\n    type Output = {};\n    fn {}(self) -> {} {{ self.{}() }}\n}}\n",
                    generics_s, entry.trait_path, type_name, type_params_s,
                    ret_ty,
                    entry.trait_method, ret_ty,
                    unique_name
                ));
            }

            MagicKind::Default => {
                out.push_str(&format!(
                    "impl{} std::default::Default for {}{} {{\n    fn default() -> Self {{ {}::{}() }}\n}}\n",
                    generics_s, type_name, type_params_s, type_name, unique_name
                ));
            }

            MagicKind::From => {
                let src_ty = if let Some(first_param) = method.params.first() {
                    if first_param.name != "self" {
                        self.map_type(&first_param.ty)
                    } else if method.params.len() >= 2 {
                        self.map_type(&method.params[1].ty)
                    } else {
                        return String::new();
                    }
                } else {
                    return String::new();
                };
                out.push_str(&format!(
                    "impl{} {}<{}> for {}{} {{\n    fn from(value: {}) -> Self {{ {}::{}(value) }}\n}}\n",
                    generics_s, entry.trait_path, src_ty, type_name, type_params_s,
                    src_ty, type_name, unique_name
                ));
            }

            MagicKind::Into => {
                let target_ty = method.return_type.as_ref()
                    .map(|t| self.map_type(t))
                    .unwrap_or_else(|| {
                        eprintln!("Warning: __into__ without return type annotation in struct {}", type_name);
                        "()".to_string()
                    });
                out.push_str(&format!(
                    "impl{} {}<{}> for {}{} {{\n    fn into(self) -> {} {{ self.{}() }}\n}}\n",
                    generics_s, entry.trait_path, target_ty, type_name, type_params_s,
                    target_ty, unique_name
                ));
            }

            // ── Phase 2: 比较 / 显示 / Hash ──

            MagicKind::PartialEq => {
                let rhs_ty = if method.params.len() >= 2 {
                    self.map_type(&method.params[1].ty)
                } else {
                    type_name.to_string()
                };
                out.push_str(&format!(
                    "impl{} {}<{}> for {}{} {{\n    fn eq(&self, other: &{}) -> bool {{ self.{}(other.clone()) }}\n}}\n",
                    generics_s, entry.trait_path, rhs_ty, type_name, type_params_s,
                    rhs_ty, unique_name
                ));
            }

            MagicKind::Display => {
                out.push_str(&format!(
                    "impl{} std::fmt::Display for {}{} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {{\n        write!(f, \"{{}}\", self.{}())\n    }}\n}}\n",
                    generics_s, type_name, type_params_s, unique_name
                ));
            }

            MagicKind::Hash => {
                out.push_str(&format!(
                    "impl{} std::hash::Hash for {}{} {{\n    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {{\n        self.{}(state)\n    }}\n}}\n",
                    generics_s, type_name, type_params_s, unique_name
                ));
            }

            MagicKind::Drop => {
                out.push_str(&format!(
                    "impl{} std::ops::Drop for {}{} {{\n    fn drop(&mut self) {{ self.{}() }}\n}}\n",
                    generics_s, type_name, type_params_s, unique_name
                ));
            }

            // ── Phase 3: 剩余 trait impl ──

            MagicKind::Clone => {
                out.push_str(&format!(
                    "impl{} std::clone::Clone for {}{} {{\n    fn clone(&self) -> Self {{ self.{}() }}\n}}\n",
                    generics_s, type_name, type_params_s, unique_name
                ));
            }

            MagicKind::PartialOrd => {
                // 使用 __lt__ 作为 primary ordering operator
                // partial_cmp 通过 lt + 反序 lt + eq 推导全排序
                out.push_str(&format!(
                    "impl{} std::cmp::PartialOrd for {}{} {{\n    fn partial_cmp(&self, other: &Self) -> std::option::Option<std::cmp::Ordering> {{\n        if self.{}(other.clone()) {{ std::option::Option::Some(std::cmp::Ordering::Less) }}\n        else if other.{}(self.clone()) {{ std::option::Option::Some(std::cmp::Ordering::Greater) }}\n        else if self.__eq__(other.clone()) {{ std::option::Option::Some(std::cmp::Ordering::Equal) }}\n        else {{ std::option::Option::None }}\n    }}\n}}\n",
                    generics_s, type_name, type_params_s,
                    unique_name, unique_name
                ));
            }

            MagicKind::Ord => {
                out.push_str(&format!(
                    "impl{} std::cmp::Ord for {}{} {{\n    fn cmp(&self, other: &Self) -> std::cmp::Ordering {{ self.{}(other.clone()) }}\n}}\n",
                    generics_s, type_name, type_params_s, unique_name
                ));
            }

            MagicKind::Index => {
                let idx_ty = if method.params.len() >= 2 {
                    self.map_type(&method.params[1].ty)
                } else {
                    "usize".to_string()
                };
                let ret_ty = method.return_type.as_ref()
                    .map(|t| self.map_type(t))
                    .unwrap_or_else(|| "()".to_string());
                out.push_str(&format!(
                    "impl{} std::ops::Index<{}> for {}{} {{\n    type Output = {};\n    fn index(&self, idx: {}) -> &{} {{ &self.{}(idx.clone()) }}\n}}\n",
                    generics_s, idx_ty, type_name, type_params_s,
                    ret_ty, idx_ty, ret_ty,
                    unique_name
                ));
            }

            MagicKind::Iterator_ => {
                let item_ty = method.return_type.as_ref()
                    .map(|t| self.map_type(t))
                    .unwrap_or_else(|| "()".to_string());
                // Iterator::next returns Option<Item>
                out.push_str(&format!(
                    "impl{} std::iter::Iterator for {}{} {{\n    type Item = {};\n    fn next(&mut self) -> std::option::Option<Self::Item> {{ self.{}() }}\n}}\n",
                    generics_s, type_name, type_params_s,
                    item_ty,
                    unique_name
                ));
            }

            MagicKind::IntoIterator_ => {
                // 简单情况：IntoIter = Self
                out.push_str(&format!(
                    "impl{} std::iter::IntoIterator for {}{} {{\n    type Item = Self;\n    type IntoIter = Self;\n    fn into_iter(self) -> Self {{ self.{}() }}\n}}\n",
                    generics_s, type_name, type_params_s,
                    unique_name
                ));
            }

            // Debug 仅 auto-derive，不生成显式 impl
            MagicKind::Debug => {}
        }

        out
    }

}

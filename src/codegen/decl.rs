// Lang-Zong 编译器 — codegen/decl.rs
// CodeGenDeclExt trait 扩展

use super::CodeGen;
use crate::parser::*;
use std::collections::HashSet;
use crate::types::Type;
use super::magic::CodeGenMagicExt;
use super::func::CodeGenFuncExt;
use super::helpers::{escape_str, out_push_attr, gen_decorator_attr};
use super::expr::CodeGenExprExt;
use super::stmt::CodeGenStmtExt;


pub trait CodeGenDeclExt {
    fn gen_const(&self, c: &ConstDef) -> String;
    fn gen_trait(&self, t: &TraitDef) -> String;
    fn gen_struct(&self, s: &StructDef, raises_types: &HashSet<String>) -> String;
    fn gen_impl(&self, i: &ImplDef) -> String;
}

impl CodeGenDeclExt for CodeGen {
    fn gen_const(&self, c: &ConstDef) -> String {
        let ty = c.ty.as_ref()
            .map(|t| format!(": {}", self.map_type(t)))
            .unwrap_or_else(|| {
                // Rust const 必须有类型标注，从字面量推断
                match &c.value {
                    Expr::IntLit(_) => ": i64".to_string(),
                    Expr::FloatLit(_) => ": f64".to_string(),
                    Expr::BoolLit(_) => ": bool".to_string(),
                    Expr::StrLit(_) => ": &str".to_string(),
                    _ => ": i64".to_string(), // fallback
                }
            });
        if c.mutable {
            // mut 全局变量 → static mut（Rust 中需要 unsafe 访问）
            format!("static mut {}{} = {};\n", c.name, ty, self.gen_expr(&c.value))
        } else {
            // const 字符串不需要 .to_string()
            let val = if c.ty.is_none() {
                match &c.value {
                    Expr::StrLit(s) => format!("\"{}\"", escape_str(s)),
                    _ => self.gen_expr(&c.value),
                }
            } else {
                self.gen_expr(&c.value)
            };
            format!("const {}{} = {};\n", c.name, ty, val)
        }
    }

    fn gen_trait(&self, t: &TraitDef) -> String {
        let generics = if t.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", t.generics.join(", "))
        };
        let mut out = format!("trait {}{} {{\n", t.name, generics);

        // trait 字段 → 关联类型或常量（简化处理：注释掉）
        for f in &t.fields {
            out.push_str(&format!("    // field: {}: {}\n", f.name, f.ty));
        }

        for m in &t.methods {
            let params: Vec<String> = m.params.iter()
                .map(|p| self.gen_param(p))
                .collect();
            let ret = m.return_type.as_ref()
                .map(|t| format!(" -> {}", self.map_type(t)))
                .unwrap_or_default();

            if m.is_abstract {
                // 抽象方法 → trait 方法签名（无方法体）
                out.push_str(&format!("    fn {}({}){};\n", m.name, params.join(", "), ret));
            } else {
                // 有方法体 → 默认实现
                let mut locals: HashSet<String> = m.params.iter().map(|p| p.name.clone()).collect();
                let body = self.gen_block(&m.body, 2, &mut locals);
                out.push_str(&format!("    fn {}({}){} {{\n{}\n    }}\n", m.name, params.join(", "), ret, body));
            }
        }
        out.push_str("}\n");
        out
    }

    fn gen_struct(&self, s: &StructDef, raises_types: &HashSet<String>) -> String {
        let keyword = if s.is_enum { "enum" } else { "struct" };
        let generics = if s.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", s.generics.join(", "))
        };

        // 装饰器 → Rust 属性
        for d in &s.decorators {
            out_push_attr(&mut String::new(), d);
        }

        let mut out = String::new();
        for d in &s.decorators {
            out.push_str(&gen_decorator_attr(d));
        }

        // __slots__ → #[repr(...)]
        if let Some(ref repr) = s.repr_attr {
            out.push_str(&format!("#[repr({})]\n", repr));
        }

        // copy-by-default 模型：所有用户复合类型自动 derive Clone，使默认拷贝可用
        // 错误枚举（raises 引用）额外 derive Debug + 实现 Display/Error
        let is_error_enum = s.is_enum && raises_types.contains(&s.name);
        let has_repr = s.methods.iter().any(|m| m.name == "__repr__");
        if s.is_enum {
            if is_error_enum || has_repr {
                out.push_str("#[derive(Debug, Clone)]\n");
            } else {
                out.push_str("#[derive(Clone)]\n");
            }
        } else {
            if has_repr {
                out.push_str("#[derive(Debug, Clone)]\n");
            } else {
                out.push_str("#[derive(Clone)]\n");
            }
        }

        out.push_str(&format!("{} {}{} {{\n", keyword, s.name, generics));

        for field in &s.fields {
            if matches!(&field.ty, Type::Unit) {
                // enum unit variant
                out.push_str(&format!("    {},\n", field.name));
            } else if s.is_enum {
                // enum tuple variant: Some(T), Color(f64, f64, f64)
                let rust_ty = if let Type::Tuple(types) = &field.ty {
                    types.iter()
                        .map(|t| self.map_type(t))
                        .collect::<Vec<_>>()
                        .join(", ")
                } else {
                    self.map_type(&field.ty)
                };
                out.push_str(&format!("    {}({}),\n", field.name, rust_ty));
            } else {
                // struct field: name: type
                let rust_ty = self.map_type(&field.ty);
                let default = field.default.as_ref()
                    .map(|d| format!(" = {}", self.gen_expr(d)))
                    .unwrap_or_default();
                out.push_str(&format!("    {}: {}{},\n", field.name, rust_ty, default));
            }
        }
        out.push_str("}\n");

        // 错误枚举：自动实现 Display + std::error::Error（raises 引用的枚举）
        if is_error_enum {
            let gen_params = if s.generics.is_empty() {
                String::new()
            } else {
                let params: Vec<String> = s.generics.iter()
                    .map(|g| format!("{}: std::fmt::Debug + std::fmt::Display", g))
                    .collect();
                format!("<{}>", params.join(", "))
            };
            let type_params = if s.generics.is_empty() {
                String::new()
            } else {
                format!("<{}>", s.generics.join(", "))
            };
            out.push_str(&format!("\nimpl{} std::fmt::Display for {}{} {{\n", gen_params, s.name, type_params));
            out.push_str("    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {\n");
            out.push_str("        match self {\n");
            for field in &s.fields {
                if matches!(&field.ty, Type::Unit) {
                    out.push_str(&format!("            {}::{} => write!(f, \"{}\"),\n",
                        s.name, field.name, field.name));
                } else {
                    // 单字段：直接展示值
                    if matches!(&field.ty, Type::Tuple(_)) {
                        out.push_str(&format!("            {}::{}(..) => write!(f, \"{}(..)\"),\n",
                            s.name, field.name, field.name));
                    } else {
                        out.push_str(&format!("            {}::{}(v) => write!(f, \"{}: {{}}\", v),\n",
                            s.name, field.name, field.name));
                    }
                }
            }
            out.push_str("        }\n");
            out.push_str("    }\n");
            out.push_str("}\n");
            out.push_str(&format!("\nimpl{} std::error::Error for {}{} {{}}\n", gen_params, s.name, type_params));
        }

        // 方法（包含 def + magic 定义）
        if !s.methods.is_empty() || !s.magic_methods.is_empty() {
            let impl_generics = if s.generics.is_empty() {
                String::new()
            } else {
                format!("<{}>", s.generics.join(", "))
            };
            out.push_str(&format!("\nimpl{} {}{} {{\n", impl_generics, s.name, impl_generics));
            // 处理重复魔法方法（同名不同签名）→ 追加类型后缀消歧
            let magic_dupes = self.magic_duplicate_count(s);
            for m in &s.methods {
                if m.name.starts_with("__") && m.name.ends_with("__") {
                    let unique = self.magic_unique_name(m, &magic_dupes);
                    out.push_str(&self.gen_magic_method_body(m, &unique, 1));
                    out.push('\n');
                } else {
                    out.push_str(&self.gen_method(m, 1));
                    out.push('\n');
                }
            }
            // 生成 magic 方法（__unapply__ 等不需要 trait 映射的魔法方法）
            for m in &s.magic_methods {
                if self.magic_engine.resolve(&m.name).is_none() {
                    // 未注册到 MagicEngine 的 magic 方法 → 作为普通方法生成
                    out.push_str(&self.gen_method(m, 1));
                    out.push('\n');
                } else {
                    // 已注册的 magic 方法 → 在 gen_magic_impls 中生成 trait impl
                    let unique = self.magic_unique_name(m, &magic_dupes);
                    out.push_str(&self.gen_magic_method_body(m, &unique, 1));
                    out.push('\n');
                }
            }
            out.push_str("}\n");
        }

        // 魔法方法 → 自动 trait impl
        out.push_str(&self.gen_magic_impls(s));

        out
    }

    // ─── 魔法方法辅助 ───

    /// 统计每个魔法方法名的重复次数（同名不同签名）
    fn gen_impl(&self, i: &ImplDef) -> String {
        let generics = if i.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", i.generics.join(", "))
        };

        let mut out = match &i.trait_name {
            Some(tn) => format!("impl{} {} for {} {{\n", generics, tn, i.type_name),
            None => format!("impl{} {} {{\n", generics, i.type_name),
        };

        for m in &i.methods {
            out.push_str(&self.gen_method(m, 1));
            out.push('\n');
        }
        out.push_str("}\n");
        out
    }

}

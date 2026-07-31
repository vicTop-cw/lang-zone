// Lang-Zong 编译器 — codegen/builders.rs
// CodeGenBuildersExt trait 扩展

use super::CodeGen;
use crate::parser::*;
use std::collections::HashSet;
use crate::types::Type;
use super::func::CodeGenFuncExt;
use super::stmt::CodeGenStmtExt;
use std::collections::HashMap;
use super::expr::CodeGenExprExt;


pub trait CodeGenBuildersExt {
    fn gen_build_block(&self, kind: BuildKind, lhs: &Expr, body: &[Stmt], indent: usize, locals: &mut HashSet<String>) -> String;
    fn callee_name(&self, lhs: &Expr) -> String;
    fn callee_entries(&self, callee: &str) -> Vec<(String, String)>;
    fn callee_prefix(&self, lhs: &Expr) -> String;
    fn gen_pack_value(&self, e: &Expr, _indent: usize, _locals: &mut HashSet<String>) -> String;
    fn pack_cast(&self, i: usize, val: &str) -> String;
    fn dict_value_cast(key_s: &str, val: &str, n2t: &HashMap<String, String>) -> String;
    fn cast_to(val: &str, ty: &str) -> String;
    fn is_as_castable(ty: &str) -> bool;
    fn literal_str(s: &str) -> Option<&str>;
    fn gen_unpack_call(&self, pack_var: &str, lhs_s: &str, params: &[String]) -> String;
}

impl CodeGenBuildersExt for CodeGen {
    fn gen_build_block(&self, kind: BuildKind, lhs: &Expr, body: &[Stmt], indent: usize, locals: &mut HashSet<String>) -> String {
        let pad = "    ".repeat(indent);
        match kind {
            BuildKind::Var => {
                // lhs 为变量名（Ident），结果自动绑定到该变量
                let name = if let Expr::Ident(n) = lhs { n.clone() } else { "_".to_string() };
                locals.insert(name.clone());
                let body_s = self.gen_block_return(body, indent + 1, locals);
                let body_s = body_s.trim_end();
                format!("let {} = (|| unsafe {{\n{}\n{}}})();", name, body_s, pad)
            }
            BuildKind::Call => {
                // 调用构建块(~:)：块体在 in_build_call 上下文中生成，return/尾部表达式经类型擦除为 __Pack。
                // 闭包返回 __Pack，再据 callee 的参数名动态解包为 *args / **kwargs。
                let callee = self.callee_name(lhs);
                let entries = self.callee_entries(&callee);
                let params: Vec<String> = entries.iter().map(|(n, _)| n.clone()).collect();
                let types: Vec<String> = entries.iter().map(|(_, t)| t.clone()).collect();
                let saved_types = self.pack_types.replace(types);
                let saved_names = self.pack_names.replace(params.clone());
                let prev = self.in_build_call.replace(true);
                let body_s = self.gen_block_return(body, indent + 1, locals);
                let body_s = body_s.trim_end();
                self.in_build_call.set(prev);
                self.pack_types.replace(saved_types);
                self.pack_names.replace(saved_names);
                let lhs_s = self.callee_prefix(lhs);
                let unpack = self.gen_unpack_call("__p", &lhs_s, &params);
                format!("{{ let __p = (|| unsafe {{\n{}\n{}}})();\n{}    {} }}", body_s, pad, pad, unpack)
            }
            BuildKind::Gen => {
                // 生成器构建块(*:)：进入 in_gen 上下文，yield 逐步产出类型擦除的参数包(__Pack) 推入 __bb；
                // 闭包正常结束以 IterStopException 收尾（停止信号）。
                // 若无 callee（独立 *:），返回原始 __Pack 迭代器；否则对每包调用 callee 解包。
                let callee = self.callee_name(lhs);
                let has_callee = !callee.is_empty();
                let entries = self.callee_entries(&callee);
                let params: Vec<String> = entries.iter().map(|(n, _)| n.clone()).collect();
                let types: Vec<String> = entries.iter().map(|(_, t)| t.clone()).collect();
                let saved_types = self.pack_types.replace(types);
                let saved_names = self.pack_names.replace(params.clone());
                let prev = self.in_gen.replace(true);
                let body_s = self.gen_block(body, indent + 1, locals);
                let body_s = body_s.trim_end();
                self.in_gen.set(prev);
                self.pack_types.replace(saved_types);
                self.pack_names.replace(saved_names);
                let tail = if has_callee {
                    let lhs_s = self.callee_prefix(lhs);
                    let unpack = self.gen_unpack_call("__p", &lhs_s, &params);
                    format!(".map(move |__p| {{ {} }})", unpack)
                } else {
                    String::new()
                };
                format!(
                    "{{ let mut __bb: Vec<__Pack> = Vec::new();\n{}    (|| unsafe {{\n{}\n{}        IterStopException\n{}    }})();\n{}    __bb.into_iter(){}\n{}    }}",
                    pad, body_s, pad, pad, pad, tail, pad
                )
            }
            BuildKind::Index => {
                let lhs_s = self.callee_prefix(lhs);
                let body_s = self.gen_block(body, indent + 1, locals);
                format!("{{ {}.__getitem__({}) }}", lhs_s, body_s.trim_end())
            }
        }
    }

    /// 取构建块所调用函数/方法的名称（Ident 直接取；MethodCall/FieldAccess 取方法/字段名）
    fn callee_name(&self, lhs: &Expr) -> String {
        match lhs {
            Expr::Ident(n) => n.clone(),
            Expr::MethodCall { method, .. } => method.clone(),
            Expr::FieldAccess { field, .. } => field.clone(),
            _ => String::new(),
        }
    }

    /// 取 callee 的(参数名, Rust 类型)列表（优先函数表，其次方法表）
    fn callee_entries(&self, callee: &str) -> Vec<(String, String)> {
        self.fn_params.get(callee)
            .or_else(|| self.method_params.get(callee))
            .cloned()
            .unwrap_or_default()
    }

    /// 取 callee 的调用前缀（不含参数括号）：函数 → 名称；方法 → receiver.method
    fn callee_prefix(&self, lhs: &Expr) -> String {
        match lhs {
            Expr::Ident(n) => n.clone(),
            Expr::MethodCall { receiver, method, .. } => format!("{}.{}", self.gen_expr(receiver), method),
            Expr::FieldAccess { receiver, field } => {
                if let Expr::Ident(type_name) = receiver.as_ref() {
                    let is_known_type = self.structs.iter().any(|(n, _)| n == type_name);
                    let is_variant = self.enum_variants.iter().any(|(v, _)| v == field);
                    if is_known_type && is_variant {
                        return format!("{}::{}", type_name, field);
                    }
                }
                format!("{}.{}", self.gen_expr(receiver), field)
            }
            _ => self.gen_expr(lhs),
        }
    }

    /// 将参数包表达式类型擦除为 __Pack（实际以裸指针承载，配合 unsafe 解包）：
    /// - 元组/具名元组 → __Pack::Tuple(vec![Box::into_raw(Box::new(<e>)) as *const () ...])，按位置对齐 callee 参数类型
    /// - 字典        → __Pack::Dict(vec![(k, Box::into_raw(Box::new(<v>)) as *const ()) ...])，按 key 名对齐 callee 参数类型
    /// - 结构体 / 标识符 / move / try / 其他 → __Pack::Single(单个值指针)（BuildParams 语义，整体作为单参数）
    /// 元素在擦除时即对齐到 callee 的目标类型（如 int 字面量 i32 → callee 的 i64），保证拆包处指针类型一致。
    fn gen_pack_value(&self, e: &Expr, _indent: usize, _locals: &mut HashSet<String>) -> String {
        match e {
            Expr::TupleLit(elems) => {
                let parts: Vec<String> = elems.iter().enumerate()
                    .map(|(i, el)| {
                        let cast = self.pack_cast(i, &self.gen_expr(el));
                        format!("Box::into_raw(Box::new({})) as *const ()", cast)
                    })
                    .collect();
                format!("__Pack::Tuple(vec![{}])", parts.join(", "))
            }
            Expr::DictLit(pairs) => {
                // key 名 -> 目标类型 映射，用于按名对齐 callee 参数类型
                let n2t: HashMap<String, String> = {
                    let names = self.pack_names.borrow();
                    let types = self.pack_types.borrow();
                    names.iter().zip(types.iter())
                        .map(|(n, t)| (n.clone(), t.clone()))
                        .collect()
                };
                let parts: Vec<String> = pairs.iter().map(|(k, v)| {
                    // 构建块字典 key 在 .lz 中通常写成裸标识符（如 name:），须转成 Rust 字符串字面量作 HashMap<String,_> 的 key
                    let key_lit = match k {
                        Expr::Ident(name) => format!("\"{}\"", name),
                        _ => self.gen_expr(k),
                    };
                    let val = self.gen_expr(v);
                    let cast = Self::dict_value_cast(&key_lit, &val, &n2t);
                    // HashMap<String, _> 需要 String key：字面量统一 .to_string()（key_lit 仍保留裸字面量供 dict_value_cast 按名定位类型）
                    let key_owned = format!("{}.to_string()", key_lit);
                    format!("({}, Box::into_raw(Box::new({})) as *const ())", key_owned, cast)
                }).collect();
                format!("__Pack::Dict(vec![{}].into_iter().collect())", parts.join(", "))
            }
            Expr::Try(inner) => self.gen_pack_value(inner, _indent, _locals),
            Expr::Move(inner) => self.gen_pack_value(inner, _indent, _locals),
            _ => format!("__Pack::Single(Box::into_raw(Box::new({})) as *const ())", self.gen_expr(e)),
        }
    }

    /// 元组第 i 个元素按 callee 第 i 个参数类型对齐（必要时用 as 转换）
    fn pack_cast(&self, i: usize, val: &str) -> String {
        let types = self.pack_types.borrow();
        if i < types.len() {
            Self::cast_to(val, &types[i])
        } else {
            val.to_string()
        }
    }

    /// 字典值按 key 名对齐到 callee 对应参数类型（key 不匹配任何参数名则保留原类型）
    fn dict_value_cast(key_s: &str, val: &str, n2t: &HashMap<String, String>) -> String {
        if let Some(k) = Self::literal_str(key_s) {
            if let Some(t) = n2t.get(k) {
                return Self::cast_to(val, t);
            }
        }
        val.to_string()
    }

    /// 仅对可用 as 互转的原始数值/布尔/字符类型插入 `val as TY`；字符串/结构体等直接透传（类型已一致）
    fn cast_to(val: &str, ty: &str) -> String {
        if Self::is_as_castable(ty) {
            format!("({} as {})", val, ty)
        } else {
            val.to_string()
        }
    }

    fn is_as_castable(ty: &str) -> bool {
        matches!(ty, "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
            | "usize" | "isize" | "f32" | "f64" | "bool" | "char")
    }

    /// 从字符串字面量源码（含引号）提取内部内容；非字面量返回 None
    fn literal_str(s: &str) -> Option<&str> {
        let t = s.trim();
        if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
            Some(t[1..t.len() - 1].trim())
        } else {
            None
        }
    }

    /// 动态解包 __Pack 并调用 callee（类似 Python *args / **kwargs）：
    /// - Tuple(v) → 按 callee 参数个数位置解包：f(*(Box::from_raw(v[i] as *mut _)) ...)
    /// - Dict(m)  → 按 callee 参数名命名解包：f(name = *(Box::from_raw(*m.get("name").expect(...) as *mut _)) ...)
    /// - Single(p)→ 整体作为单个参数：f(*(Box::from_raw(p as *mut _)))
    /// 不校验字典 key 与参数名是否匹配（缺少 key 运行期 panic、多余 key 忽略），保持 unsafe 语义，由使用者负责。
    fn gen_unpack_call(&self, pack_var: &str, lhs_s: &str, params: &[String]) -> String {
        // 注意：match 各分支已分别将参数包绑定为 v(Tuple) / m(Dict) / p(Single)，
        // 拆包表达式必须引用分支内绑定的变量，而非外层 pack_var。
        // Box::from_raw 为 unsafe，整体包在 unsafe 块中（构建块默认 unsafe 语义）。
        let tuple_args: String = (0..params.len())
            .map(|i| format!("*(Box::from_raw(v[{}] as *mut _))", i))
            .collect::<Vec<_>>()
            .join(", ");
        // 字典按 callee 参数名顺序逐一定位（Rust 无命名实参，故以位置实参形式按参数顺序展开，
        // 相当于 Python **kwargs：dict 的 key 与参数名对应，多余 key 忽略、缺失 key 运行期 panic）。
        let dict_args: String = params.iter()
            .map(|pname| format!(
                "*(Box::from_raw(*m.get(\"{}\").expect(\"build param not found: {}\") as *mut _))",
                pname, pname
            ))
            .collect::<Vec<_>>()
            .join(", ");
        // 单值(Single)仅在 callee 恰有 1 个参数时成立（结构体/BuildParams 整体传参）；
        // 否则该分支恒不可达（避免 Rust 对所有 match 分支做类型检查时报参数个数不匹配）。
        let single_arm = if params.len() == 1 {
            format!("__Pack::Single(p) => {}(*(Box::from_raw(p as *mut _))),", lhs_s)
        } else {
            String::new()
        };
        format!(
            "unsafe {{ match {} {{ __Pack::Tuple(v) => {}({}), __Pack::Dict(m) => {}({}), {}_ => unreachable!() }} }}",
            pack_var, lhs_s, tuple_args, lhs_s, dict_args, single_arm
        )
    }

}

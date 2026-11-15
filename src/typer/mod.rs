// Lang-Zong 编译器 — typer/mod.rs
// 类型推断管道：AST 类型注解填充 + 约束收集 + 求解 + zonk
//
// 流水线：Parser → Module → [Typer::infer_module] → [escape check] → CodeGen
//
// 设计原则：
// 1. 每个函数独立推断（暂无跨函数类型传播）
// 2. 副作用在 InferCtx 上累积（unify 实时绑定），函数结束后 zonk 并写回 AST
// 3. 推断失败不 panic，累积错误列表由调用方处理

use crate::types::Type;

use crate::ast::{Module, Function, Stmt, Expr, BinOp, UnaryOp, Pattern};

use crate::hints::{InferCtx, unify, zonk, TypeError};

use crate::typing::{InstanceRegistry, resolve_instance};

use std::collections::HashMap;

// 大文件拆分（move-only，2026-10-06）：单文件按职责切分为子模块，逻辑零改动
mod builtin;
mod infer_fn;
mod pattern;
mod self_ty;
mod subst;
#[cfg(test)]
mod tests;

pub(crate) use builtin::*;
pub(crate) use infer_fn::*;
pub(crate) use pattern::*;
pub(crate) use self_ty::*;
pub(crate) use subst::*;

/// 函数签名注册表：用于跨函数类型传播
#[derive(Debug, Clone)]
struct FnSig {
    /// 泛型参数名列表，如 ["T", "U"]
    pub generics: Vec<String>,
    /// 泛型参数 bound：T → [Clone, Debug]
    pub generic_bounds: Vec<(String, Vec<Type>)>,
    /// 参数类型列表（类型别名已展开）
    pub param_types: Vec<Type>,
    /// 返回类型（类型别名已展开）
    pub return_type: Type,
}


/// 从 AST 的显式类型注解构建 FnSig，若任一参数或返回类型缺失则返回 None
fn build_fn_sig(f: &Function, aliases: &HashMap<String, (Vec<String>, Type)>) -> Option<FnSig> {
    let return_type = f.return_type.as_ref()?;
    let param_types: Vec<Type> = f.params.iter()
        .map(|p| p.ty.clone())
        .collect::<Option<Vec<_>>>()?;
    let param_types = param_types.into_iter()
        .map(|t| expand_type(aliases, &t))
        .collect();
    let return_type = expand_type(aliases, return_type);
    // 合并 generic_bounds 和 where_clause（两者语法等价）
    let mut generic_bounds = f.generic_bounds.clone();
    for wb in &f.where_clause {
        // 避免重复：若同名泛型参数已有 bound，追加而非覆盖
        if let Some((_, existing_bounds)) = generic_bounds.iter_mut()
            .find(|(name, _)| name == &wb.type_param)
        {
            for b in &wb.bounds {
                if !existing_bounds.contains(b) {
                    existing_bounds.push(b.clone());
                }
            }
        } else {
            generic_bounds.push((wb.type_param.clone(), wb.bounds.clone()));
        }
    }
    Some(FnSig {
        generics: f.generics.clone(),
        generic_bounds,
        param_types,
        return_type,
    })
}


/// 构建跨函数注册表：扫描模块中所有含显式类型注解的函数
fn build_fn_registry(module: &Module, aliases: &HashMap<String, (Vec<String>, Type)>, lzi: Option<&crate::infer::LziRegistry>) -> HashMap<String, FnSig> {
    let mut registry = HashMap::new();
    for f in &module.functions {
        if let Some(sig) = build_fn_sig(f, aliases) {
            registry.insert(f.name.clone(), sig);
        } else if let Some(file) = lzi {
            if let Some(module_name) = &module.name {
                if let Some(lzi_fn) = file.lookup_function(module_name, &f.name) {
                    let param_types: Vec<Type> = lzi_fn.params.iter().map(|p| str_to_type(&p.ty)).collect();
                    if param_types.len() == lzi_fn.params.len() {
                        let return_type = lzi_fn.return_type.as_ref().and_then(|t| str_to_type_opt(t)).unwrap_or(Type::Unit);
                        registry.insert(f.name.clone(), FnSig {
                            generics: lzi_fn.generics.clone(),
                            generic_bounds: Vec::new(),
                            param_types,
                            return_type,
                        });
                    }
                }
            }
        }
    }
    for imp in &module.impls {
        for m in &imp.methods {
            if let Some(sig) = build_fn_sig(m, aliases) {
                registry.insert(m.name.clone(), sig);
            }
        }
    }
    for s in &module.structs {
        for m in &s.methods {
            if let Some(sig) = build_fn_sig(m, aliases) {
                registry.insert(m.name.clone(), sig);
            }
        }
    }
    registry
}


/// 类型推断器
pub struct Typer;
impl Typer {


    /// 推断整个模块：遍历所有函数/方法体，填充 Param.ty / Function.return_type / Stmt::Let.ty
    pub fn infer_module(module: &mut Module) -> Vec<String> {
        Self::infer_module_with(module, None)
    }

    /// 带跨模块签名注册表的类型推断
    pub fn infer_module_with(module: &mut Module, registry: Option<&crate::infer::LziRegistry>) -> Vec<String> {
        let mut errors = Vec::new();

        // 收集模块级类型别名，供类型注解展开（消除 cannot unify 警告）。
        // key = 别名名, value = (泛型参数名列表, 展开后的底层类型)。
        let mut alias_map: HashMap<String, (Vec<String>, Type)> = HashMap::new();
        for ta in &module.type_aliases {
            alias_map.insert(ta.name.clone(), (ta.generics.clone(), ta.ty.clone()));
        }
        // 二次遍历：展开别名之间的互相引用（处理嵌套别名）
        let alias_names: Vec<String> = alias_map.keys().cloned().collect();
        for n in &alias_names {
            if let Some((params, body)) = alias_map.get(n).cloned() {
                let expanded = expand_type(&alias_map, &body);
                alias_map.insert(n.clone(), (params, expanded));
            }
        }

        // 收集模块级结构体名称（用于识别 struct 构造器调用）
        let struct_names: std::collections::HashSet<String> =
            module.structs.iter().map(|s| s.name.clone()).collect();

        // 收集可调用 struct（有 __call__ 方法且类型已显式注解）的方法签名
        // struct_name → (params[1..] 跳过 self, 返回类型)
        let mut callable_types: std::collections::HashMap<String, (Vec<Type>, Type)> =
            std::collections::HashMap::new();
        for s in &module.structs {
            if let Some(call_method) = s.methods.iter().find(|m| m.name == "__call__") {
                let params_no_self: Vec<Type> = call_method.params.iter()
                    .skip(1) // 跳过 self
                    .filter_map(|p| p.ty.clone())
                    .collect();
                if params_no_self.len() == call_method.params.len().saturating_sub(1)
                    && call_method.return_type.is_some() {
                    callable_types.insert(s.name.clone(),
                        (params_no_self, call_method.return_type.as_ref().unwrap().clone()));
                }
            }
        }

        // 构建结构体字段注册表：struct_name → [(field_name, field_type)]
        let mut struct_fields: std::collections::HashMap<String, Vec<(String, Type)>> =
            std::collections::HashMap::new();
        for s in &module.structs {
            let fields: Vec<(String, Type)> = s.fields.iter()
                .map(|f| (f.name.clone(), expand_type(&alias_map, &f.ty)))
                .collect();
            struct_fields.insert(s.name.clone(), fields);
        }

        // 构建枚举变体注册表（支持 GADT 构造与模式匹配）
        let mut enum_variants: std::collections::HashMap<String, EnumVariant> =
            std::collections::HashMap::new();
        let mut enum_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for s in &module.structs {
            if s.is_enum {
                enum_names.insert(s.name.clone());
                for f in &s.fields {
                    let payload = expand_type(&alias_map, &f.ty);
                    let return_type = f.variant_return.as_ref().map(|t| expand_type(&alias_map, t));
                    enum_variants.insert(
                        format!("{}.{}", s.name, f.name),
                        EnumVariant {
                            enum_name: s.name.clone(),
                            generics: s.generics.clone(),
                            payload,
                            return_type,
                        },
                    );
                }
            }
        }

        // 构建枚举名 → 变体名列表的简明注册表，用于穷尽性检查
        let mut enum_variant_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for s in &module.structs {
            if s.is_enum {
                for f in &s.fields {
                    enum_variant_map
                        .entry(s.name.clone())
                        .or_insert_with(Vec::new)
                        .push(f.name.clone());
                }
            }
        }

        // 构建方法注册表：(type_name, method_name) → (params[1..] skip self, return类型)
        let mut method_registry: std::collections::HashMap<String,
            std::collections::HashMap<String, (Vec<Type>, Type)>> =
            std::collections::HashMap::new();
        // 收集 struct 方法
        for s in &module.structs {
            let mut methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
                std::collections::HashMap::new();
            for m in &s.methods {
                let params_no_self: Vec<Type> = m.params.iter()
                    .skip(1) // 跳过 self
                    .filter_map(|p| p.ty.as_ref().map(|t| expand_type(&alias_map, t)))
                    .collect();
                if let Some(ref ret) = m.return_type {
                    let ret_expanded = expand_type(&alias_map, ret);
                    methods.insert(m.name.clone(), (params_no_self, ret_expanded));
                }
            }
            if !methods.is_empty() {
                method_registry.insert(s.name.clone(), methods);
            }
        }
        // 收集 impl 方法
        for imp in &module.impls {
            let mut methods = method_registry.remove(&imp.type_name).unwrap_or_default();
            for m in &imp.methods {
                let params_no_self: Vec<Type> = m.params.iter()
                    .skip(1)
                    .filter_map(|p| p.ty.as_ref().map(|t| expand_type(&alias_map, t)))
                    .collect();
                if let Some(ref ret) = m.return_type {
                    let ret_expanded = expand_type(&alias_map, ret);
                    methods.insert(m.name.clone(), (params_no_self, ret_expanded));
                }
            }
            if !methods.is_empty() {
                method_registry.insert(imp.type_name.clone(), methods);
            }
        }

        // 注入内置类型方法表（List/Str/Option/Result 的常用方法）
        inject_builtin_methods(&mut method_registry);

        // 构建跨函数类型传播注册表（基于显式类型注解）
        let fn_registry = build_fn_registry(module, &alias_map, registry);

        // 构建 trait 实例注册表（用于隐式推导）
        let instance_registry = InstanceRegistry::from_module(module);

        // 推断顶层函数
        for f in &mut module.functions {
            if let Err(e) = Self::infer_function(f, &alias_map, &struct_names, &enum_names, &enum_variants, &enum_variant_map, &callable_types, &fn_registry, &struct_fields, &method_registry, &instance_registry) {
                errors.push(format!("In function '{}': {}", f.name, e));
            }
        }

        // 推断顶层 const（在单独的函数体中推断值表达式）
        for c in &mut module.consts {
            // 展开 const 类型注解中的别名引用
            if let Some(t) = &c.ty {
                let e = expand_type(&alias_map, t);
                c.ty = Some(e);
            }
            if c.ty.is_none() {
                // 为 const 创建临时函数用于推断
                let mut temp_fn = Function {
                    name: c.name.clone(),
                    generics: vec![], generic_kinds: vec![], generic_bounds: vec![], generic_defaults: vec![],
                    params: vec![],
                    return_type: None,
                    raises: None,
                    where_clause: vec![],
                    body: vec![Stmt::Return(Some(c.value.clone()))],
                    is_async: false,
                    is_abstract: false,
                    comptime: false,
                    decorators: vec![],
                    attributes: vec![],
                    variadic: None, params_checker: None,
                };
                if let Err(e) = Self::infer_function(&mut temp_fn, &alias_map, &struct_names, &enum_names, &enum_variants, &enum_variant_map, &callable_types, &fn_registry, &struct_fields, &method_registry, &instance_registry) {
                    errors.push(format!("In const '{}': {}", c.name, e));
                } else {
                    // infer_function 填充了 temp_fn 的 return_type
                    if temp_fn.return_type.is_some() {
                        c.ty = temp_fn.return_type.clone();
                    }
                }
            }
        }

        // 推断 impl 方法
        for imp in &mut module.impls {
            for m in &mut imp.methods {
                if let Err(e) = Self::infer_function(m, &alias_map, &struct_names, &enum_names, &enum_variants, &enum_variant_map, &callable_types, &fn_registry, &struct_fields, &method_registry, &instance_registry) {
                    let ctx = imp.trait_name.as_deref().unwrap_or(&imp.type_name);
                    errors.push(format!("In impl '{}': method '{}': {}", ctx, m.name, e));
                }
            }
        }

        // 推断 struct 方法
        for s in &mut module.structs {
            for m in &mut s.methods {
                if let Err(e) = Self::infer_function(m, &alias_map, &struct_names, &enum_names, &enum_variants, &enum_variant_map, &callable_types, &fn_registry, &struct_fields, &method_registry, &instance_registry) {
                    errors.push(format!("In struct '{}': method '{}': {}", s.name, m.name, e));
                }
            }
        }

        errors
    }

    /// 推断函数体内所有语句，累积约束
    pub(crate) fn infer_body(sess: &mut InferSession, stmts: &mut [Stmt], ret_type: &Option<Type>, raises_type: &Option<Type>) -> Result<(), TypeError> {
        for stmt in stmts.iter_mut() {
            Self::infer_stmt(sess, stmt, ret_type, raises_type)?;
        }
        // 等式风格函数：最后一个表达式 = 隐式返回 → 捕获为 inferred_ret
        if ret_type.is_none() && sess.inferred_ret.is_none() {
            if let Some(Stmt::Expr(e)) = stmts.last() {
                // 尝试推断最后表达式的类型（如果已有 var，zonk 会解析）
                if let Ok(t) = Self::infer_expr(sess, e) {
                    sess.inferred_ret = Some(t);
                }
            }
        }
        Ok(())
    }}


#[derive(Debug, Clone)]
struct EnumVariant {
    enum_name: String,
    generics: Vec<String>,
    /// 变体数据类型（Unit / Tuple / Record / Named）
    payload: Type,
    /// GADT 返回类型（如 Expr<int>）
    return_type: Option<Type>,
}


/// 推断会话：每个函数独立使用
struct InferSession {
    ctx: InferCtx,
    env: std::collections::HashMap<String, Type>,
    inferred_ret: Option<Type>,
    /// 类型别名表：模块级别名（初始化时注入）+ 函数内局部别名（推断时追加）
    aliases: std::collections::HashMap<String, (Vec<String>, Type)>,
    /// 模块结构体名称集：用于识别 struct 构造器调用
    struct_names: std::collections::HashSet<String>,
    /// 可调用 struct 的方法签名：struct_name → (params[1..] 跳过 self, 返回类型)
    callable_types: std::collections::HashMap<String, (Vec<Type>, Type)>,
    /// 跨函数类型传播注册表：函数名 → 签名
    fn_registry: std::collections::HashMap<String, FnSig>,
    /// 结构体字段注册表：struct_name → [(field_name, field_type)]
    struct_fields: std::collections::HashMap<String, Vec<(String, Type)>>,
    /// 枚举变体注册表："EnumName.VariantName" → EnumVariant
    enum_variants: std::collections::HashMap<String, EnumVariant>,
    /// 枚举名 → 变体名列表（用于穷尽性检查）
    enum_variant_map: std::collections::HashMap<String, Vec<String>>,
    /// 枚举类型名称集
    enum_names: std::collections::HashSet<String>,
    /// 方法注册表：(type_name, method_name) → (params[1..] skip self, return_type)
    method_registry: std::collections::HashMap<String,
        std::collections::HashMap<String, (Vec<Type>, Type)>>,
    /// trait 实例注册表：用于泛型 bound 隐式推导
    instance_registry: InstanceRegistry,
    /// 类型 bound 检查警告（不阻断编译，但收集到 infer_module 的错误列表）
    bound_warnings: Vec<String>,
    /// trait 实例解析失败的类型错误（阻断编译）
    instance_errors: Vec<String>,
    /// 模式匹配非穷尽错误（阻断编译）
    exhaustiveness_errors: Vec<String>,
    /// @math 模式：跳过算术运算的 Int 强制统一，改为泛型 Number bound
    math_mode: bool,
    /// @math 模式下原本无类型注解的参数名集合
    math_params: std::collections::HashSet<String>,
    /// 当前作用域内由类型判断引入的收窄类型：变量名 → 收窄后的类型
    narrowings: std::collections::HashMap<String, Type>,
}
impl InferSession {


    pub(crate) fn enum_variant<'a>(&'a self, enum_name: &str, variant_name: &str) -> Option<&'a EnumVariant> {
        self.enum_variants.get(&format!("{}.{}", enum_name, variant_name))
    }

    pub(crate) fn new(aliases: std::collections::HashMap<String, (Vec<String>, Type)>,
           struct_names: std::collections::HashSet<String>,
           enum_names: std::collections::HashSet<String>,
           enum_variants: std::collections::HashMap<String, EnumVariant>,
           enum_variant_map: std::collections::HashMap<String, Vec<String>>,
           callable_types: std::collections::HashMap<String, (Vec<Type>, Type)>,
           fn_registry: std::collections::HashMap<String, FnSig>,
           struct_fields: std::collections::HashMap<String, Vec<(String, Type)>>,
           method_registry: std::collections::HashMap<String,
               std::collections::HashMap<String, (Vec<Type>, Type)>>,
           instance_registry: InstanceRegistry) -> Self {
        InferSession {
            ctx: InferCtx::new(),
            env: std::collections::HashMap::new(),
            inferred_ret: None,
            aliases,
            struct_names,
            enum_names,
            enum_variants,
            enum_variant_map,
            callable_types,
            fn_registry,
            struct_fields,
            method_registry,
            instance_registry,
            bound_warnings: Vec::new(),
            instance_errors: Vec::new(),
            exhaustiveness_errors: Vec::new(),
            math_mode: false,
            math_params: std::collections::HashSet::new(),
            narrowings: std::collections::HashMap::new(),
        }
    }}




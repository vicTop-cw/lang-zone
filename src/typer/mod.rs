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

/// 将 .lzi 类型字符串转换为 Type（支持常见基本类型）
fn str_to_type(s: &str) -> Type {
    str_to_type_opt(s).unwrap_or(Type::Named(s.to_string()))
}
fn str_to_type_opt(s: &str) -> Option<Type> {
    match s {
        "int" => Some(Type::Int),
        "f64" => Some(Type::F64),
        "str" | "String" => Some(Type::Str),
        "bool" => Some(Type::Bool),
        "()" | "Unit" => Some(Type::Unit),
        "!" | "Never" => Some(Type::Never),
        _ => None,
    }
}

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

    /// 推断单个函数的类型
    fn infer_function(f: &mut Function,
                      aliases: &HashMap<String, (Vec<String>, Type)>,
                      struct_names: &std::collections::HashSet<String>,
                      enum_names: &std::collections::HashSet<String>,
                      enum_variants: &std::collections::HashMap<String, EnumVariant>,
                      enum_variant_map: &std::collections::HashMap<String, Vec<String>>,
                      callable_types: &std::collections::HashMap<String, (Vec<Type>, Type)>,
                      fn_registry: &HashMap<String, FnSig>,
                      struct_fields: &std::collections::HashMap<String, Vec<(String, Type)>>,
                      method_registry: &std::collections::HashMap<String,
                          std::collections::HashMap<String, (Vec<Type>, Type)>>,
                      instance_registry: &InstanceRegistry) -> Result<(), TypeError> {
        let mut sess = InferSession::new(
            aliases.clone(), struct_names.clone(), enum_names.clone(), enum_variants.clone(),
            enum_variant_map.clone(), callable_types.clone(), fn_registry.clone(), struct_fields.clone(),
            method_registry.clone(), instance_registry.clone());

        // 展开参数 / 返回类型注解中的类型别名引用（如 Reduce<int,int> → fn(i64,i64)->i64）
        for p in &mut f.params {
            if let Some(t) = &p.ty {
                let e = expand_type(&sess.aliases, t);
                p.ty = Some(e);
            }
        }
        if let Some(rt) = &f.return_type {
            let e = expand_type(&sess.aliases, rt);
            f.return_type = Some(e);
        }
        if let Some(raises_ty) = &f.raises {
            let e = expand_type(&sess.aliases, raises_ty);
            f.raises = Some(e);
        }

        // 检测 @math 装饰器：标记数学模式并记录无注解参数
        let is_math = f.decorators.iter().any(|d| d.name == "math");
        if is_math {
            sess.math_mode = true;
            for p in &f.params {
                if p.ty.is_none() {
                    sess.math_params.insert(p.name.clone());
                }
            }
        }

        // 注册参数类型：已有注解的直接用，没有的创建推断变量
        for p in &f.params {
            match &p.ty {
                Some(ty) => { sess.env.insert(p.name.clone(), ty.clone()); }
                None => {
                    let tv = sess.ctx.fresh_ty(0);
                    sess.env.insert(p.name.clone(), tv);
                }
            }
        }
        // 注册 variadic 伪变量类型（避免 args/kwargs 被推断为默认 i64）
        if let Some(ref v) = f.variadic {
            if matches!(v.mode, crate::ast::VariadicMode::ArgsOnly | crate::ast::VariadicMode::Both) {
                // Vec<Box<dyn Any>> 用 Generic Vec<_> 表示，让 codegen 从 fn 签名中取完整类型
                sess.env.insert("args".into(),
                    Type::Generic {
                        base: Box::new(Type::Named("Vec".into())),
                        args: vec![Type::Named("Box".into())],
                    });
            }
            if matches!(v.mode, crate::ast::VariadicMode::KwargsOnly | crate::ast::VariadicMode::Both) {
                // Dict<str, Box<dyn Any>> → 代码映射为 HashMap<String, Box<dyn Any>>
                sess.env.insert("kwargs".into(),
                    Type::Generic {
                        base: Box::new(Type::Named("Dict".into())),
                        args: vec![Type::Str, Type::Named("Box".into())],
                    });
            }
        }

        // 注册返回类型和 raises 类型（如果有）
        let ret_type = f.return_type.clone();
        let raises_type = f.raises.clone();

        // 推断函数体
        Self::infer_body(&mut sess, &mut f.body, &ret_type, &raises_type)?;

        // — 求解 + zonk —
        // 收集函数中所有需要 zonk 的 Type slot
        let mut to_zonk: Vec<&mut Type> = Vec::new();

        // 参数类型
        for p in &mut f.params {
            if p.ty.is_none() && sess.env.contains_key(&p.name) {
                p.ty = sess.env.remove(&p.name);
            }
            if let Some(ref mut ty) = p.ty {
                to_zonk.push(ty);
            }
        }

        // 返回类型：若未注解且体中有 return，统一返回值
        if f.return_type.is_none() {
            if let Some(rt) = sess.inferred_ret.take() {
                f.return_type = Some(rt);
            }
        }
        if let Some(ref mut ty) = f.return_type {
            to_zonk.push(ty);
        }

        // let 绑定的类型（以及函数体内的所有 type slot）
        // 我们在 infer 过程中已把 Stmt::Let.ty 设为 Some(Type::Var(...))
        // 现在只需 zonk 整个函数体的所有 type 字段
        Self::zonk_function_types(&sess.ctx, &mut f.body);

        // zonk 参数和返回类型
        for slot in to_zonk {
            // 只有包含 Type::Var 的才需要 zonk；已解析的 zonk 是恒等
            let resolved = zonk(&sess.ctx, slot);
            *slot = resolved;
        }

        // @math 泛型转换：将推理出的数学参数替换为泛型 T: Number
        if is_math && !sess.math_params.is_empty() {
            // 生成一个不与已有泛型冲突的名称
            let mut gen_name = "T".to_string();
            let mut counter = 1;
            while f.generics.contains(&gen_name) {
                gen_name = format!("T{}", counter);
                counter += 1;
            }
            f.generics.push(gen_name.clone());
            f.generic_bounds.push((gen_name.clone(), vec![Type::Named("Number".into())]));

            for p in &mut f.params {
                if sess.math_params.contains(&p.name) {
                    p.ty = Some(Type::Named(gen_name.clone()));
                }
            }

            // 若返回类型是推理出的默认 Int（未绑定变量），也替换为泛型
            if let Some(ref ret_ty) = f.return_type {
                if matches!(ret_ty, Type::Int) && f.generics.contains(&gen_name) {
                    f.return_type = Some(Type::Named(gen_name.clone()));
                }
            }
        }

        // 输出类型 bound 检查警告（不阻断编译，但收集到 infer_module 的错误列表）
        let warnings: Vec<String> = sess.bound_warnings.drain(..).collect();
        for warn in &warnings {
            eprintln!("Type bound warning: {}", warn);
        }

        // trait 实例解析失败的类型错误（阻断编译）
        if !sess.instance_errors.is_empty() {
            let msg = sess.instance_errors.join("\n");
            return Err(TypeError::Message(msg));
        }

        // 模式匹配非穷尽错误（阻断编译）
        if !sess.exhaustiveness_errors.is_empty() {
            let msg = sess.exhaustiveness_errors.join("\n");
            return Err(TypeError::Message(msg));
        }

        Ok(())
    }

    /// 推断函数体内所有语句，累积约束
    fn infer_body(sess: &mut InferSession, stmts: &mut [Stmt], ret_type: &Option<Type>, raises_type: &Option<Type>) -> Result<(), TypeError> {
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
    }

    /// 推断一条语句，可能修改其内部的 type 字段（设为 Type::Var 供后续 zonk）
    fn infer_stmt(sess: &mut InferSession, stmt: &mut Stmt, ret_type: &Option<Type>, raises_type: &Option<Type>) -> Result<(), TypeError> {
        match stmt {
            Stmt::Let { name, value, ty, .. } => {
                let val_type = Self::infer_expr(sess, value)?;
                match ty {
                    Some(annotated) => {
                        // 注解类型与表达式类型统一（先展开类型别名引用）
                        let expanded = expand_type(&sess.aliases, annotated);
                        *ty = Some(expanded.clone());
                        unify(&mut sess.ctx, &val_type, &expanded)?;
                    }
                    None => {
                        // 无注解：用推断结果
                        *ty = Some(val_type);
                    }
                }
                // 将绑定名加入环境（类型可能含 Var，后续 zonk）
                if let Some(t) = ty.as_ref() {
                    sess.env.insert(name.clone(), t.clone());
                }
                Ok(())
            }
            Stmt::Const { value, ty, .. } => {
                let val_type = Self::infer_expr(sess, value)?;
                if let Some(annotated) = ty {
                    let expanded = expand_type(&sess.aliases, annotated);
                    *annotated = expanded.clone();
                    unify(&mut sess.ctx, &val_type, &expanded)?;
                } else {
                    *ty = Some(val_type);
                }
                Ok(())
            }
            Stmt::Expr(expr) => {
                Self::infer_expr(sess, expr)?;
                Ok(())
            }
            Stmt::Return(Some(expr)) => {
                let ret = Self::infer_expr(sess, expr)?;
                // 统一返回值与函数声明的返回类型
                if let Some(decl_ret) = ret_type {
                    unify(&mut sess.ctx, &ret, decl_ret)?;
                } else {
                    // 记录推断的返回类型
                    sess.inferred_ret = Some(ret);
                }
                Ok(())
            }
            Stmt::Return(None) => {
                if let Some(decl_ret) = ret_type {
                    // fn f() -> Int { return } 时返回类型必须是 Unit 或 Optional
                    unify(&mut sess.ctx, &Type::Unit, decl_ret)?;
                }
                Ok(())
            }
            Stmt::While { cond, body, .. } => {
                let cond_type = Self::infer_expr(sess, cond)?;
                unify(&mut sess.ctx, &cond_type, &Type::Bool)?;
                Self::infer_body(sess, body, ret_type, raises_type)?;
                Ok(())
            }
            Stmt::For { var, iter, body, .. } => {
                let iter_type = Self::infer_expr(sess, iter)?;
                // 从容器类型提取元素类型并与循环变量统一
                let elem_ty = sess.ctx.fresh_ty(0);
                if let Type::Generic { args, .. } = &iter_type {
                    if let Some(first_arg) = args.first() {
                        let _ = unify(&mut sess.ctx, &elem_ty, first_arg);
                    }
                }
                sess.env.insert(var.clone(), elem_ty);
                Self::infer_body(sess, body, ret_type, raises_type)?;
                Ok(())
            }
            Stmt::Loop(body) => {
                Self::infer_body(sess, body, ret_type, raises_type)?;
                Ok(())
            }
            Stmt::Break(_) | Stmt::Continue(_) => Ok(()),
            Stmt::Pass => Ok(()),
            Stmt::Defer(body) => {
                Self::infer_body(sess, body, ret_type, raises_type)?;
                Ok(())
            }
            Stmt::Raise(expr) => {
                let raise_expr_type = Self::infer_expr(sess, expr)?;
                // 若函数标注了 raises 类型，将 raise 表达式的类型与之统一
                if let Some(raises_ty) = raises_type {
                    unify(&mut sess.ctx, &raise_expr_type, raises_ty)?;
                }
                Ok(())
            }
            Stmt::Guard { cond, else_body, .. } => {
                if let Some(c) = cond {
                    let cond_type = Self::infer_expr(sess, c)?;
                    unify(&mut sess.ctx, &cond_type, &Type::Bool)?;
                }
                Self::infer_body(sess, else_body, ret_type, raises_type)?;
                Ok(())
            }
            Stmt::With { expr, body, .. } => {
                Self::infer_expr(sess, expr)?;
                Self::infer_body(sess, body, ret_type, raises_type)?;
                Ok(())
            }
            Stmt::Assign { target, value, .. } => {
                let target_type = Self::infer_expr(sess, target)?;
                let val_type = Self::infer_expr(sess, value)?;
                unify(&mut sess.ctx, &target_type, &val_type)?;
                Ok(())
            }
            Stmt::Test { body, .. } => {
                Self::infer_body(sess, body, ret_type, raises_type)?;
                Ok(())
            }
            Stmt::Assert { expr, .. } => {
                let t = Self::infer_expr(sess, expr)?;
                unify(&mut sess.ctx, &t, &Type::Bool)?;
                Ok(())
            }
            Stmt::Suite { tests, .. } => {
                for t in tests.iter_mut() {
                    Self::infer_stmt(sess, t, ret_type, raises_type)?;
                }
                Ok(())
            }
            Stmt::Check { expr, .. } => {
                let t = Self::infer_expr(sess, expr)?;
                unify(&mut sess.ctx, &t, &Type::Bool)?;
                Ok(())
            }
            Stmt::Yield(Some(expr)) => {
                Self::infer_expr(sess, expr)?;
                Ok(())
            }
            Stmt::Yield(None) => Ok(()),
            Stmt::YieldFrom { expr, transform } => {
                Self::infer_expr(sess, expr)?;
                if let Some(f) = transform { Self::infer_expr(sess, f)?; }
                Ok(())
            }
            Stmt::Comptime(_) => {
                // comptime 块由 comptime 引擎处理，类型推断跳过
                Ok(())
            }
            Stmt::FnDef(_) => {
                // 内嵌函数暂不支持类型推断
                Ok(())
            }
            Stmt::TypeAlias(ta) => {
                // 局部类型别名：注册进当前推断会话，供后续注解展开
                let expanded = expand_type(&sess.aliases, &ta.ty);
                sess.aliases.insert(ta.name.clone(), (ta.generics.clone(), expanded));
                Ok(())
            }
            Stmt::If { .. } => {
                // 条件分支的类型推断（TODO: 实现详细推断逻辑）
                Ok(())
            }
            Stmt::Match { expr, arms } => {
                let scrut_ty = Self::infer_expr(sess, expr)?;
                let patterns: Vec<Pattern> = arms.iter().map(|a| a.pattern.clone()).collect();
                if let Some(msg) = crate::typing::check_exhaustive(&scrut_ty, &patterns, &sess.enum_variant_map) {
                    sess.exhaustiveness_errors.push(format!("non-exhaustive match: {}", msg));
                }
                for arm in arms {
                    Self::infer_pattern(sess, &arm.pattern, None)?;
                    Self::infer_body(sess, &mut arm.body, ret_type, raises_type)?;
                }
                Ok(())
            }
            Stmt::Destructure { names, value, .. } => {
                let val_type = Self::infer_expr(sess, value)?;
                // 简化：将每个解构绑定名绑定为新鲜变量，后续使用点由 unify 约束
                for name in names {
                    let tv = sess.ctx.fresh_ty(0);
                    sess.env.insert(name.clone(), tv);
                }
                let _ = val_type;
                Ok(())
            }
        }
    }

    /// 推断表达式类型，返回其类型（可能含 Type::Var）
    fn infer_expr(sess: &mut InferSession, expr: &Expr) -> Result<Type, TypeError> {
        match expr {
            Expr::IntLit(_) => Ok(Type::Int),
            Expr::FloatLit(_) => Ok(Type::Float),
            Expr::StrLit(_) | Expr::FStrLit(_) | Expr::RawStrLit(_) => Ok(Type::Str),
            Expr::BoolLit(_) => Ok(Type::Bool),
            // Bug-5: `None` 应推断为 Option<_>（而非单位类型 ()），使 `let x = None` 得到
            // `Option<_>`，后续 `?.`/`??` 与 Rust 的 Option 体系一致。
            Expr::NoneLit => Ok(Type::Optional(Box::new(sess.ctx.fresh_ty(0)))),
            Expr::Underscore => Ok(sess.ctx.fresh_ty(0)),

            Expr::Ident(name) => {
                // 优先应用类型判断引入的收窄类型
                if let Some(ty) = sess.narrowings.get(name.as_str()) {
                    return Ok(ty.clone());
                }
                // 已知变量 → 返回其类型；未知标识符（函数名/全局）→ 创建自由变量，不报错
                Ok(sess.env.get(name.as_str())
                    .cloned()
                    .unwrap_or_else(|| sess.ctx.fresh_ty(0)))
            }

            // 容器字面量
            Expr::ListLit(elems) => {
                if elems.is_empty() {
                    // 空列表：无法推断元素类型，返回 List<??> 保留自由变元
                    let elem = sess.ctx.fresh_ty(0);
                    Ok(Type::Generic {
                        base: Box::new(Type::Named("List".into())),
                        args: vec![elem],
                    })
                } else {
                    let first = Self::infer_expr(sess, &elems[0])?;
                    for e in &elems[1..] {
                        let t = Self::infer_expr(sess, e)?;
                        unify(&mut sess.ctx, &first, &t)?;
                    }
                    Ok(Type::Generic {
                        base: Box::new(Type::Named("List".into())),
                        args: vec![first],
                    })
                }
            }
            Expr::TupleLit(elems) => {
                let types: Result<Vec<Type>, _> = elems.iter().map(|e| Self::infer_expr(sess, e)).collect();
                Ok(Type::Tuple(types?))
            }
            Expr::SetLit(elems) => {
                if elems.is_empty() {
                    let elem = sess.ctx.fresh_ty(0);
                    Ok(Type::Generic {
                        base: Box::new(Type::Named("Set".into())),
                        args: vec![elem],
                    })
                } else {
                    let first = Self::infer_expr(sess, &elems[0])?;
                    for e in &elems[1..] {
                        let t = Self::infer_expr(sess, e)?;
                        unify(&mut sess.ctx, &first, &t)?;
                    }
                    Ok(Type::Generic {
                        base: Box::new(Type::Named("Set".into())),
                        args: vec![first],
                    })
                }
            }
            Expr::DictLit(entries) => {
                if entries.is_empty() {
                    let k = sess.ctx.fresh_ty(0);
                    let v = sess.ctx.fresh_ty(0);
                    Ok(Type::Generic {
                        base: Box::new(Type::Named("Dict".into())),
                        args: vec![k, v],
                    })
                } else {
                    let (k0, v0) = &entries[0];
                    let kt = Self::infer_expr(sess, k0)?;
                    let vt = Self::infer_expr(sess, v0)?;
                    for (k, v) in &entries[1..] {
                        let kn = Self::infer_expr(sess, k)?;
                        let vn = Self::infer_expr(sess, v)?;
                        unify(&mut sess.ctx, &kt, &kn)?;
                        unify(&mut sess.ctx, &vt, &vn)?;
                    }
                    Ok(Type::Generic {
                        base: Box::new(Type::Named("Dict".into())),
                        args: vec![kt, vt],
                    })
                }
            }

            // 类型判断表达式
            Expr::TypeTest { expr, .. } => {
                let _ = Self::infer_expr(sess, expr)?;
                Ok(Type::Bool)
            }

            // 二元运算
            Expr::Binary { left, op, right } => {
                let l = Self::infer_expr(sess, left)?;
                let r = Self::infer_expr(sess, right)?;
                use BinOp::*;
                match op {
                    Add | Sub | Mul | Div | Mod | Pow => {
                        // Bug-29: 字符串拼接（String + &str / str 字面量）在 Rust 中合法，
                        // 类型检查器不应报 "cannot unify" 误报，也不应强制约束为整数。
                        let lz = zonk(&sess.ctx, &l);
                        let rz = zonk(&sess.ctx, &r);
                        let is_str = |t: &Type| match t {
                            Type::Str => true,
                            Type::Named(n) => n.as_str() == "String",
                            _ => false,
                        };
                        if is_str(&lz) || is_str(&rz) {
                            // 字符串拼接：结果类型为 str（LZ str → Rust String）。
                            // 不强制 unify 为整数，避免误报；两侧不要求同类型。
                            Ok(Type::Str)
                        } else {
                            unify(&mut sess.ctx, &l, &r)?;
                            // @math: 跳过 Int 强制统一，允许泛型 Number 多态
                            if !sess.math_mode {
                                let _ = unify(&mut sess.ctx, &l, &Type::Int);
                            }
                            Ok(l)
                        }
                    }
                    Eq | Ne | Lt | Gt | Le | Ge => {
                        unify(&mut sess.ctx, &l, &r)?;
                        Ok(Type::Bool)
                    }
                    And | Or => {
                        unify(&mut sess.ctx, &l, &Type::Bool)?;
                        unify(&mut sess.ctx, &r, &Type::Bool)?;
                        Ok(Type::Bool)
                    }
                    BitAnd | BitOr | BitXor | Shl | Shr => {
                        unify(&mut sess.ctx, &l, &Type::Int)?;
                        unify(&mut sess.ctx, &r, &Type::Int)?;
                        Ok(Type::Int)
                    }
                    In => Ok(Type::Bool),
                    Is => Ok(Type::Bool),
                }
            }

            // 一元运算
            Expr::Unary { op, operand } => {
                let t = Self::infer_expr(sess, operand)?;
                match op {
                    UnaryOp::Neg => {
                        // 数字取负：必须是 Int 或 Float
                        Ok(t) // 不强制约束，让后续使用决定
                    }
                    UnaryOp::Not => {
                        unify(&mut sess.ctx, &t, &Type::Bool)?;
                        Ok(Type::Bool)
                    }
                    UnaryOp::BitNot => {
                        unify(&mut sess.ctx, &t, &Type::Int)?;
                        Ok(Type::Int)
                    }
                }
            }

            // 函数调用
            Expr::Call { func, args, .. } => {
                // ── 内置构造器：Some(x) → Option<T>, Ok(x) → Result<T, E>, Err(x) → Result<!, E> ──
                if let Expr::Ident(name) = func.as_ref() {
                    if name == "Some" && args.len() == 1 {
                        let inner = Self::infer_expr(sess, &args[0])?;
                        return Ok(Type::Optional(Box::new(inner)));
                    }
                    if name == "Ok" && args.len() == 1 {
                        let inner = Self::infer_expr(sess, &args[0])?;
                        let err_var = sess.ctx.fresh_ty(0);
                        return Ok(Type::Result { ok: Box::new(inner), err: Box::new(err_var) });
                    }
                    if name == "Err" && args.len() == 1 {
                        let inner = Self::infer_expr(sess, &args[0])?;
                        let ok_var = sess.ctx.fresh_ty(0);
                        return Ok(Type::Result { ok: Box::new(ok_var), err: Box::new(inner) });
                    }
                }

                // ── struct 构造器：返回 Named 类型而非 fn 类型 ──
                // 若 func 为 Ident(name) 且 name 是模块内 struct，则这是一次 struct 构造，
                // 其结果类型为 Named(name)。避免后续 a(args) 统一将 `a.ty` 绑定为 Fn 类型。
                if let Expr::Ident(name) = func.as_ref() {
                    if sess.struct_names.contains(name.as_str()) {
                        // 仍需要推断参数中的表达式（用于参数约束），但不做 fn 类型统一
                        for a in args {
                            Self::infer_expr(sess, a)?;
                        }
                        return Ok(Type::Named(name.clone()));
                    }
                }

                // ── 跨函数类型传播 ──
                // 若 func 为 Ident(name) 且 name 在注册表中，查签名并传播类型
                if let Expr::Ident(name) = func.as_ref() {
                    // Clone 注册项以绕过 borrow checker（infer_expr 需 &mut sess）
                    let registered = sess.fn_registry.get(name.as_str()).cloned();
                    if let Some(sig) = registered {
                        // 推断参数类型
                        let arg_types: Vec<Type> = args.iter()
                            .map(|a| Self::infer_expr(sess, a))
                            .collect::<Result<Vec<_>, _>>()?;

                        // 创建泛型参数替换表：T → fresh_ty, U → fresh_ty ...
                        let mut subst: HashMap<String, Type> = HashMap::new();
                        for gp in &sig.generics {
                            subst.insert(gp.clone(), sess.ctx.fresh_ty(0));
                        }

                        // 用 fresh 泛型变量替代签名中的泛型参数
                        let instantiate = |t: &Type| -> Type {
                            if subst.is_empty() { t.clone() } else { substitute(&subst, t) }
                        };
                        let sig_params: Vec<Type> = sig.param_types.iter()
                            .map(|p| instantiate(p))
                            .collect();
                        let sig_ret = instantiate(&sig.return_type);

                        // 参数统一：实参类型 <: 形参类型
                        if arg_types.len() == sig_params.len() {
                            for (arg_t, param_t) in arg_types.iter().zip(sig_params.iter()) {
                                unify(&mut sess.ctx, arg_t, param_t)?;
                            }
                        }

                        // ── Trait Bound 检查（Phase 3） ──
                        // 对每个泛型 bound（如 T: Show），获取 T 对应实参类型并检查
                        if !sig.generic_bounds.is_empty() {
                            // 收集泛型参数名 → 具体类型（经过 unify 后 prune 消解）
                            let mut concrete: HashMap<String, Type> = HashMap::new();
                            for gp in &sig.generics {
                                if let Some(fresh) = subst.get(gp) {
                                    let resolved = sess.ctx.prune(fresh);
                                    concrete.insert(gp.clone(), resolved);
                                }
                            }
                            for (param_name, bounds) in &sig.generic_bounds {
                                if let Some(concrete_ty) = concrete.get(param_name) {
                                    // 未实例化的泛型参数继续延迟到 codegen/rustc
                                    if type_contains_var(concrete_ty) {
                                        continue;
                                    }
                                    if let Type::Named(name) = concrete_ty {
                                        if sig.generics.contains(name) {
                                            continue;
                                        }
                                    }
                                    for bound_ty in bounds {
                                        let bound_name = match bound_ty {
                                            Type::Named(n) => n.as_str(),
                                            Type::Str => "str",
                                            Type::Int => "int",
                                            _ => continue,
                                        };
                                        if resolve_instance(&sess.instance_registry, bound_name, concrete_ty).is_none() {
                                            sess.instance_errors.push(format!(
                                                "type `{}` does not implement trait `{}` (required by `{}`)",
                                                concrete_ty, bound_name, param_name
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        // 即使参数数量不匹配，也允许后续 rustc 报错（不中断）
                        // 返回签名中的返回类型（泛型变量会在 zonk 时被具体类型替换）
                        return Ok(sig_ret);
                    }
                }

                let func_type = Self::infer_expr(sess, func)?;
                let arg_types: Result<Vec<Type>, _> = args.iter().map(|a| Self::infer_expr(sess, a)).collect();
                let arg_types = arg_types?;

                // print/println → 返回 Unit
                if let Expr::Ident(name) = func.as_ref() {
                    if name == "print" || name == "println" {
                        let _ = arg_types; // 消耗
                        return Ok(Type::Unit);
                    }
                }

                // ── 可调用 struct 处理 ──
                // 当 func_type 为 Named(struct_name) 且该 struct 有 __call__ 时，
                // 用 __call__ 的方法签名约束 arg_types 和返回类型，而非统一 fn 类型。
                if let Type::Named(name) = &func_type {
                    if let Some((call_params, call_ret)) = sess.callable_types.get(name.as_str()) {
                        if arg_types.len() == call_params.len() {
                            for (arg_t, param_t) in arg_types.iter().zip(call_params.iter()) {
                                unify(&mut sess.ctx, arg_t, param_t)?;
                            }
                            return Ok(call_ret.clone());
                        }
                    }
                }

                // 创建函数类型：fn(arg1, arg2, ...) -> ret
                let ret_var = sess.ctx.fresh(0); // TyVar (Copy)
                let fn_ty = Type::Fn {
                    params: arg_types,
                    ret: Box::new(Type::Var(ret_var)),
                };
                unify(&mut sess.ctx, &func_type, &fn_ty)?;

                // 返回推断的返回类型（可能仍为自由变量，zonk 时会降级为 Unit）
                Ok(sess.ctx.prune(&Type::Var(ret_var)))
            }

            // 方法调用
            Expr::MethodCall { receiver, method, args } => {
                // GADT 数据变体构造：EnumName.VariantName(args...)
                if let Expr::Ident(enum_name) = receiver.as_ref() {
                    if sess.enum_names.contains(enum_name.as_str()) {
                        if let Some(variant) = sess.enum_variant(enum_name, method).cloned() {
                            let subst = fresh_subst_for_generics(&mut sess.ctx, &variant.generics);
                            let payload_subst = substitute(&subst, &variant.payload);
                            match &payload_subst {
                                Type::Unit => {
                                    // 单元变体不应携带参数
                                    let _ = args;
                                }
                                Type::Tuple(types) => {
                                    for (arg_expr, param_ty) in args.iter().zip(types.iter()) {
                                        let arg_expr = match arg_expr {
                                            Expr::KwArg { value, .. } => value.as_ref(),
                                            other => other,
                                        };
                                        let arg_ty = Self::infer_expr(sess, arg_expr)?;
                                        let _ = unify(&mut sess.ctx, &arg_ty, param_ty);
                                    }
                                }
                                Type::Record(fields) => {
                                    for arg_expr in args {
                                        if let Expr::KwArg { name, value } = arg_expr {
                                            if let Some((_, param_ty)) = fields.iter().find(|(n, _)| n == name) {
                                                let arg_ty = Self::infer_expr(sess, value)?;
                                                let _ = unify(&mut sess.ctx, &arg_ty, param_ty);
                                            }
                                        }
                                    }
                                }
                                single => {
                                    // 单字段变体：裸类型
                                    if let Some(arg_expr) = args.first() {
                                        let arg_expr = match arg_expr {
                                            Expr::KwArg { value, .. } => value.as_ref(),
                                            other => other,
                                        };
                                        let arg_ty = Self::infer_expr(sess, arg_expr)?;
                                        let _ = unify(&mut sess.ctx, &arg_ty, single);
                                    }
                                }
                            }
                            return Ok(enum_self_type_with_subst(&variant, &mut sess.ctx, &subst));
                        }
                    }
                }
                let recv_type = Self::infer_expr(sess, receiver)?;
                // 从 receiver 类型提取类型名
                let type_name = resolve_type_name(&recv_type);
                // Clone 方法签名以绕过 borrow checker
                let method_sig = match &recv_type {
                    Type::Intersection(members) => {
                        members.iter()
                            .filter_map(|m| resolve_type_name(m))
                            .filter_map(|tn| sess.method_registry.get(&tn))
                            .filter_map(|methods| methods.get(method.as_str()))
                            .next()
                            .cloned()
                    }
                    _ => type_name.as_ref()
                        .and_then(|tn| sess.method_registry.get(tn))
                        .and_then(|methods| methods.get(method.as_str()))
                        .cloned(),
                };
                if let Some((params, ret)) = method_sig {
                    // 推断参数表达式
                    for a in args {
                        Self::infer_expr(sess, a)?;
                    }
                    // 若方法有参数且数量匹配，尝试统一
                    if args.len() == params.len() {
                        for (arg_expr, param_ty) in args.iter().zip(params.iter()) {
                            let arg_ty = Self::infer_expr(sess, arg_expr)?;
                            let _ = unify(&mut sess.ctx, &arg_ty, param_ty);
                        }
                    }
                    return Ok(ret);
                }
                // 未找到 → 回退：返回自由变量（原行为）
                for a in args {
                    Self::infer_expr(sess, a)?;
                }
                Ok(sess.ctx.fresh_ty(0))
            }

            // 字段/路径访问
            Expr::FieldAccess { receiver, field } => {
                // GADT 单元变体构造：EnumName.VariantName
                if let Expr::Ident(enum_name) = receiver.as_ref() {
                    if sess.enum_names.contains(enum_name.as_str()) {
                        if let Some(variant) = sess.enum_variant(enum_name, field).cloned() {
                            return Ok(enum_self_type(&variant, &mut sess.ctx));
                        }
                    }
                }
                let recv_type = Self::infer_expr(sess, receiver)?;
                // 从 receiver 类型提取类型名，查字段注册表
                let type_name = resolve_type_name(&recv_type);
                if let Some(tn) = type_name {
                    if let Some(fields) = sess.struct_fields.get(&tn) {
                        for (fn_name, fn_ty) in fields {
                            if fn_name == field {
                                return Ok(fn_ty.clone());
                            }
                        }
                    }
                }
                // 未找到 → 自由变量
                Ok(sess.ctx.fresh_ty(0))
            }
            Expr::PathAccess { receiver, .. } => {
                Self::infer_expr(sess, receiver)
            }

            // 索引/下标
            Expr::Index { receiver, index } => {
                let _recv = Self::infer_expr(sess, receiver)?;
                let _idx = Self::infer_expr(sess, index)?;
                Ok(sess.ctx.fresh_ty(0))
            }

            // 控制流表达式
            Expr::If { cond, then_body, elif_clauses, else_body, .. } => {
                let ct = Self::infer_expr(sess, cond)?;
                unify(&mut sess.ctx, &ct, &Type::Bool)?;

                // 从类型判断条件中提取收窄信息
                let apply_narrowing = |sess: &mut InferSession, cond: &Expr| {
                    if let Expr::TypeTest { expr, ty } = cond {
                        if let Expr::Ident(name) = expr.as_ref() {
                            sess.narrowings.insert(name.clone(), ty.clone());
                        }
                    }
                };

                // 各个分支的类型必须统一
                // 用 if 最后一条语句的类型作为分支类型（无语句则为 Unit）
                let branch_type = |stmts: &[Stmt], sess: &mut InferSession| -> Result<Type, TypeError> {
                    // 推断所有语句
                    for s in stmts.iter() {
                        let mut cloned = s.clone();
                        Self::infer_stmt(sess, &mut cloned, &None, &None)?;
                    }
                    // 尾部表达式类型作为分支结果
                    if let Some(Stmt::Expr(e)) = stmts.last() {
                        Self::infer_expr(sess, e)
                    } else {
                        Ok(Type::Unit)
                    }
                };

                let saved = sess.narrowings.clone();
                apply_narrowing(sess, cond);
                let then_t = branch_type(then_body, sess)?;
                sess.narrowings = saved;
                let mut result_t = then_t;

                for (elif_cond, b) in elif_clauses {
                    let ct = Self::infer_expr(sess, elif_cond)?;
                    unify(&mut sess.ctx, &ct, &Type::Bool)?;
                    let saved = sess.narrowings.clone();
                    apply_narrowing(sess, elif_cond);
                    let et = branch_type(b, sess)?;
                    sess.narrowings = saved;
                    result_t = merge_branch_types(&mut sess.ctx, result_t, et);
                }

                if let Some(eb) = else_body {
                    let et = branch_type(eb, sess)?;
                    result_t = merge_branch_types(&mut sess.ctx, result_t, et);
                }

                Ok(result_t)
            }
            Expr::Match { expr: match_expr, arms } => {
                let scrut_ty = Self::infer_expr(sess, match_expr)?;
                let patterns: Vec<Pattern> = arms.iter().map(|a| a.pattern.clone()).collect();
                if let Some(msg) = crate::typing::check_exhaustive(&scrut_ty, &patterns, &sess.enum_variant_map) {
                    sess.exhaustiveness_errors.push(format!("non-exhaustive match: {}", msg));
                }
                // 收集各分支结果类型并合并为最小联合类型
                let mut arm_types: Vec<Type> = Vec::new();
                for arm in arms {
                    // 解析模式对应的枚举名与变体名
                    let (enum_name, variant_name) = match &arm.pattern {
                        crate::ast::Pattern::Variant(name, _) |
                        crate::ast::Pattern::StructVariant { name, .. } => {
                            if name.contains('.') {
                                let mut parts = name.split('.');
                                let en = parts.next().unwrap_or("").to_string();
                                let vn = parts.next().unwrap_or("").to_string();
                                (en, vn)
                            } else {
                                (resolve_type_name(&scrut_ty).unwrap_or_default(), name.clone())
                            }
                        }
                        _ => (String::new(), String::new()),
                    };

                    if !enum_name.is_empty() {
                        if let Some(variant) = sess.enum_variant(&enum_name, &variant_name).cloned() {
                            let subst = fresh_subst_for_generics(&mut sess.ctx, &variant.generics);
                            let variant_return = enum_self_type_with_subst(&variant, &mut sess.ctx, &subst);
                            // GADT 核心：将变体返回类型与 scrutinee 统一，收窄索引类型
                            let _ = unify(&mut sess.ctx, &variant_return, &scrut_ty);
                            let payload_subst = substitute(&subst, &variant.payload);
                            Self::infer_pattern(sess, &arm.pattern, Some(&payload_subst))?;
                        } else {
                            Self::infer_pattern(sess, &arm.pattern, None)?;
                        }
                    } else {
                        Self::infer_pattern(sess, &arm.pattern, None)?;
                    }

                    for s in &arm.body {
                        let mut cloned = s.clone();
                        Self::infer_stmt(sess, &mut cloned, &None, &None)?;
                    }
                    let t = if let Some(Stmt::Expr(e)) = arm.body.last() {
                        Self::infer_expr(sess, e)?
                    } else {
                        Type::Unit
                    };
                    arm_types.push(t);
                }
                if arm_types.is_empty() {
                    Ok(Type::Unit)
                } else {
                    let mut result = arm_types.remove(0);
                    for t in arm_types {
                        result = merge_branch_types(&mut sess.ctx, result, t);
                    }
                    Ok(result)
                }
            }

            // 特殊表达式
            Expr::Closure { params, body } => {
                let param_types: Vec<Type> = params.iter().map(|_| sess.ctx.fresh_ty(0)).collect();
                // 将 lambda 参数加入环境并遮蔽外部同名变量，避免从外部 env 误取类型
                let mut saved: Vec<(String, Option<Type>)> = Vec::new();
                for (name, ty) in params.iter().zip(param_types.iter()) {
                    let old = sess.env.insert(name.clone(), ty.clone());
                    saved.push((name.clone(), old));
                }
                let ret_t = Self::infer_expr(sess, body)?;
                // 恢复环境
                for (name, old) in saved {
                    match old {
                        Some(ty) => { sess.env.insert(name, ty); }
                        None => { sess.env.remove(&name); }
                    }
                }
                Ok(Type::Fn {
                    params: param_types,
                    ret: Box::new(ret_t),
                })
            }
            Expr::ClosureBlock { params, body } => {
                let param_types: Vec<Type> = params.iter().map(|_| sess.ctx.fresh_ty(0)).collect();
                let mut saved: Vec<(String, Option<Type>)> = Vec::new();
                for (name, ty) in params.iter().zip(param_types.iter()) {
                    let old = sess.env.insert(name.clone(), ty.clone());
                    saved.push((name.clone(), old));
                }
                // 推断多行闭包体内所有语句
                for stmt in body {
                    let mut cloned = stmt.clone();
                    Self::infer_stmt(sess, &mut cloned, &None, &None)?;
                }
                // 尝试从最后表达式推断返回类型
                let ret_t = if let Some(Stmt::Expr(e)) = body.last() {
                    Self::infer_expr(sess, e)?
                } else {
                    sess.ctx.fresh_ty(0)
                };
                // 恢复环境
                for (name, old) in saved {
                    match old {
                        Some(ty) => { sess.env.insert(name, ty); }
                        None => { sess.env.remove(&name); }
                    }
                }
                Ok(Type::Fn {
                    params: param_types,
                    ret: Box::new(ret_t),
                })
            }
            Expr::Range { start, end, .. } => {
                // 推断 start/end 类型并统一，作为 Range 的泛型参数
                let mut elem_ty = None;
                if let Some(s) = start {
                    let st = Self::infer_expr(sess, s)?;
                    elem_ty = Some(st);
                }
                if let Some(e) = end {
                    let et = Self::infer_expr(sess, e)?;
                    match &elem_ty {
                        Some(st) => { let _ = unify(&mut sess.ctx, st, &et); }
                        None => { elem_ty = Some(et); }
                    }
                }
                Ok(Type::Generic {
                    base: Box::new(Type::Named("std::ops::Range".into())),
                    args: vec![elem_ty.unwrap_or_else(|| sess.ctx.fresh_ty(0))],
                })
            }
            Expr::Walrus { target, value } => {
                let t = Self::infer_expr(sess, target)?;
                let v = Self::infer_expr(sess, value)?;
                unify(&mut sess.ctx, &t, &v)?;
                Ok(v)
            }
            Expr::Pipe { receiver, callee, args } => {
                // a |> f(args) ≡ f(a, args...) 在类型层面
                let _recv_t = Self::infer_expr(sess, receiver)?;
                // 用 Expr::Call(callee(a, args...)) 推断返回类型
                let mut pipe_args = vec![receiver.as_ref().clone()];
                for a in args.iter() {
                    pipe_args.push(a.clone());
                }
                let pipe_func = callee.as_ref().clone();
                let mut pipe_expr = Expr::Call {
                    func: Box::new(pipe_func),
                    args: pipe_args,
                    checker: None,
                };
                Self::infer_expr(sess, &mut pipe_expr)
            }
            Expr::SafeNav { receiver, .. } => {
                // Bug-6: 安全导航 `a?.b` 要求 a 为 Option<T>，结果为 Option<T>（T 是 receiver 的内部类型）。
                // 将 receiver 约束为 Optional(inner)，结果返回 Optional(inner)——这样字段访问
                // `.map(|x| x.b)` 中 x 的类型与整体 Option 一致，且代码生成使用 Option::map 而非 Iterator::map。
                let r = Self::infer_expr(sess, receiver)?;
                let inner = sess.ctx.fresh_ty(0);
                let _ = unify(&mut sess.ctx, &r, &Type::Optional(Box::new(inner.clone())));
                Ok(Type::Optional(Box::new(inner)))
            }
            Expr::Try(inner) | Expr::Move(inner) | Expr::Spawn(inner) | Expr::Await(inner) | Expr::Panic(inner) => {
                Self::infer_expr(sess, inner)
            }
            Expr::SpawnBlock(body) | Expr::GoBlock(body) => {
                let mut cloned = body.clone();
                Self::infer_body(sess, &mut cloned, &None, &None)?;
                Ok(Type::Unit)
            }
            Expr::Go(inner) => {
                Self::infer_expr(sess, inner)?;
                Ok(Type::Unit)
            }
            Expr::NullCoalesce { left, right } => {
                let l = Self::infer_expr(sess, left)?;
                let r = Self::infer_expr(sess, right)?;
                // l 应为 Option<T>，r 应为 T，结果为 T
                // 用 r 的类型作为基准，与 l 的内部类型统一
                let inner = sess.ctx.fresh_ty(0);
                unify(&mut sess.ctx, &l, &Type::Optional(Box::new(inner.clone())))?;
                unify(&mut sess.ctx, &r, &inner)?;
                Ok(r)
            }
            Expr::ListComprehension { output, clauses, .. } => {
                // 为每个 for var in iter 子句插入循环变量到环境中
                for (var, iter) in clauses {
                    let iter_type = Self::infer_expr(sess, iter)?;
                    let elem_ty = if let Type::Generic { args, .. } = &iter_type {
                        args.first().cloned().unwrap_or_else(|| sess.ctx.fresh_ty(0))
                    } else {
                        sess.ctx.fresh_ty(0)
                    };
                    sess.env.insert(var.clone(), elem_ty);
                }
                let out_t = Self::infer_expr(sess, output)?;
                Ok(Type::Generic {
                    base: Box::new(Type::Named("List".into())),
                    args: vec![out_t],
                })
            }
            Expr::Assign { target, value, .. } => {
                let t = Self::infer_expr(sess, target)?;
                let v = Self::infer_expr(sess, value)?;
                unify(&mut sess.ctx, &t, &v)?;
                Ok(v)
            }
            Expr::Comptime(inner) => {
                // comptime 表达式：编译期求值，类型取决于求值结果
                // 简化：返回自由变量
                Self::infer_expr(sess, inner)
            }
            Expr::KwArg { value, .. } => {
                Self::infer_expr(sess, value)
            }
            Expr::TryCatch { body, catches, else_body, finally_body, .. } => {
                // try 块返回值类型
                let try_t = sess.ctx.fresh_ty(0);
                for s in body {
                    let mut cloned = s.clone();
                    Self::infer_stmt(sess, &mut cloned, &None, &None)?;
                }
                for arm in catches {
                    Self::infer_pattern(sess, &arm.pattern, None)?;
                    for s in &arm.body {
                        let mut cloned = s.clone();
                        Self::infer_stmt(sess, &mut cloned, &None, &None)?;
                    }
                }
                if let Some(eb) = else_body {
                    for s in eb {
                        let mut cloned = s.clone();
                        Self::infer_stmt(sess, &mut cloned, &None, &None)?;
                    }
                }
                if let Some(fb) = finally_body {
                    for s in fb {
                        let mut cloned = s.clone();
                        Self::infer_stmt(sess, &mut cloned, &None, &None)?;
                    }
                }
                Ok(try_t)
            }
            Expr::BuildBlock { body, .. } => {
                let mut cloned = body.clone();
                Self::infer_body(sess, &mut cloned, &None, &None)?;
                Ok(sess.ctx.fresh_ty(0))
            }
            Expr::As { expr, ty } => {
                let _expr_ty = Self::infer_expr(sess, expr)?;
                Ok(expand_type(&sess.aliases, ty))
            }
        }
    }

    /// 推断模式中的标识符类型
    fn infer_pattern(sess: &mut InferSession, pattern: &crate::ast::Pattern, expected_ty: Option<&Type>) -> Result<(), TypeError> {
        match pattern {
            crate::ast::Pattern::Ident(name) => {
                let ty = expected_ty.cloned().unwrap_or_else(|| sess.ctx.fresh_ty(0));
                sess.env.insert(name.clone(), ty);
            }
            crate::ast::Pattern::Variant(_, sub) => {
                if let Some(Type::Tuple(elems)) = expected_ty {
                    if elems.len() == sub.len() {
                        for (p, t) in sub.iter().zip(elems.iter()) {
                            Self::infer_pattern(sess, p, Some(t))?;
                        }
                        return Ok(());
                    }
                }
                if let Some(Type::Record(fields)) = expected_ty {
                    if fields.len() == sub.len() {
                        for ((_, t), p) in fields.iter().zip(sub.iter()) {
                            Self::infer_pattern(sess, p, Some(t))?;
                        }
                        return Ok(());
                    }
                }
                if sub.len() == 1 {
                    Self::infer_pattern(sess, &sub[0], expected_ty)?;
                } else {
                    for p in sub { Self::infer_pattern(sess, p, None)?; }
                }
            }
            crate::ast::Pattern::Tuple(ps) => {
                if let Some(Type::Tuple(elems)) = expected_ty {
                    if elems.len() == ps.len() {
                        for (p, t) in ps.iter().zip(elems.iter()) {
                            Self::infer_pattern(sess, p, Some(t))?;
                        }
                    } else {
                        for p in ps { Self::infer_pattern(sess, p, None)?; }
                    }
                } else {
                    for p in ps { Self::infer_pattern(sess, p, None)?; }
                }
            }
            crate::ast::Pattern::Array(elems) => {
                if let Some(Type::Tuple(elems_ty)) = expected_ty {
                    if elems_ty.len() == elems.len() {
                        for (p, t) in elems.iter().zip(elems_ty.iter()) {
                            Self::infer_pattern(sess, p, Some(t))?;
                        }
                    } else {
                        for p in elems { Self::infer_pattern(sess, p, None)?; }
                    }
                } else {
                    for p in elems { Self::infer_pattern(sess, p, None)?; }
                }
            }
            // P5: AS 模式 — 递归推断内层 pattern，同时绑定 as_name
            crate::ast::Pattern::As(inner, as_name) => {
                Self::infer_pattern(sess, inner, expected_ty)?;
                let ty = expected_ty.cloned().unwrap_or_else(|| sess.ctx.fresh_ty(0));
                sess.env.insert(as_name.clone(), ty);
            }
            // P6: 类型模式 — 绑定变量到环境
            crate::ast::Pattern::Type { type_name: _, binding } => {
                let ty = expected_ty.cloned().unwrap_or_else(|| sess.ctx.fresh_ty(0));
                sess.env.insert(binding.clone(), ty);
            }
            // P7: 范围模式 — 无变量绑定
            crate::ast::Pattern::Range { .. } => {}
            // P1~P4 已有模式无新绑定
            crate::ast::Pattern::Slice(Some(name)) => {
                let ty = expected_ty.cloned().unwrap_or_else(|| sess.ctx.fresh_ty(0));
                sess.env.insert(name.clone(), ty);
            }
            crate::ast::Pattern::Slice(None) => {}
            crate::ast::Pattern::Dict { pairs, rest } => {
                for (k, v) in pairs {
                    Self::infer_pattern(sess, k, None)?;
                    Self::infer_pattern(sess, v, None)?;
                }
                if let Some(name) = rest {
                    let tv = sess.ctx.fresh_ty(0);
                    sess.env.insert(name.clone(), tv);
                }
            }
            crate::ast::Pattern::StructVariant { fields, .. } => {
                if let Some(Type::Record(map)) = expected_ty {
                    for (fname, p) in fields {
                        let fty = map.iter().find(|(n, _)| n == fname).map(|(_, t)| t);
                        Self::infer_pattern(sess, p, fty)?;
                    }
                } else {
                    for (_, p) in fields {
                        Self::infer_pattern(sess, p, None)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// zonk 函数体内所有类型的递归
    fn zonk_function_types(ctx: &InferCtx, stmts: &mut [Stmt]) {
        for stmt in stmts.iter_mut() {
            match stmt {
                Stmt::Let { ty, value, .. } => {
                    if let Some(t) = ty {
                        *t = zonk(ctx, t);
                    }
                    Self::zonk_expr(ctx, value);
                }
                Stmt::Const { ty, value, .. } => {
                    if let Some(t) = ty {
                        *t = zonk(ctx, t);
                    }
                    Self::zonk_expr(ctx, value);
                }
                Stmt::Expr(expr) => Self::zonk_expr(ctx, expr),
                Stmt::Return(Some(expr)) => Self::zonk_expr(ctx, expr),
                Stmt::While { cond, body, .. } => {
                    Self::zonk_expr(ctx, cond);
                    Self::zonk_function_types(ctx, body);
                }
                Stmt::For { iter, body, .. } => {
                    Self::zonk_expr(ctx, iter);
                    Self::zonk_function_types(ctx, body);
                }
                Stmt::Loop(body) | Stmt::Comptime(body) | Stmt::Defer(body) => {
                    Self::zonk_function_types(ctx, body);
                }
                Stmt::Guard { cond, else_body, .. } => {
                    if let Some(c) = cond { Self::zonk_expr(ctx, c); }
                    Self::zonk_function_types(ctx, else_body);
                }
                Stmt::With { expr, body, .. } => {
                    Self::zonk_expr(ctx, expr);
                    Self::zonk_function_types(ctx, body);
                }
                Stmt::Assign { target, value, .. } => {
                    Self::zonk_expr(ctx, target);
                    Self::zonk_expr(ctx, value);
                }
                Stmt::Test { body, .. } => {
                    Self::zonk_function_types(ctx, body);
                }
                Stmt::Suite { tests, .. } => {
                    Self::zonk_function_types(ctx, tests);
                }
                Stmt::Assert { expr, expected, .. } | Stmt::Check { expr, expected, .. } => {
                    Self::zonk_expr(ctx, expr);
                    if let Some(e) = expected { Self::zonk_expr(ctx, e); }
                }
                Stmt::Raise(expr) | Stmt::Yield(Some(expr)) => {
                    Self::zonk_expr(ctx, expr);
                }
                Stmt::YieldFrom { expr, transform } => {
                    Self::zonk_expr(ctx, expr);
                    if let Some(f) = transform { Self::zonk_expr(ctx, f); }
                }
                Stmt::Break(Some(expr)) | Stmt::Continue(Some(expr)) => Self::zonk_expr(ctx, expr),
                Stmt::Yield(None) | Stmt::Break(None) | Stmt::Continue(None) | Stmt::Return(None) => {}
                Stmt::FnDef(_) => {} // 内嵌函数暂不支持
                Stmt::TypeAlias(_) => {} // 类型别名无需 zonk（类型已在推断时展开注册）
                Stmt::Pass => {}
                Stmt::If { cond, body, elifs, else_body } => {
                    Self::zonk_expr(ctx, cond);
                    Self::zonk_function_types(ctx, body);
                    for (elif_cond, elif_body) in elifs.iter_mut() {
                        Self::zonk_expr(ctx, elif_cond);
                        Self::zonk_function_types(ctx, elif_body);
                    }
                    if let Some(eb) = else_body {
                        Self::zonk_function_types(ctx, eb);
                    }
                }
                Stmt::Match { expr, arms } => {
                    Self::zonk_expr(ctx, expr);
                    for arm in arms.iter_mut() {
                        Self::zonk_function_types(ctx, &mut arm.body);
                    }
                }
                Stmt::Destructure { value, .. } => {
                    Self::zonk_expr(ctx, value);
                }
            }
        }
    }

    fn zonk_expr(_ctx: &InferCtx, _expr: &mut Expr) {
        // Expr 中不含 Type 字段需要 zonk（类型注解在 Stmt/Param/Function 层）
        // 如果将来 Expr 含有类型标注，在这里递归处理
        // 目前 Expr 只在 CodeGen 阶段才需要 Rust 类型，Expr 本身不持有 Type
    }
}

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
    fn enum_variant<'a>(&'a self, enum_name: &str, variant_name: &str) -> Option<&'a EnumVariant> {
        self.enum_variants.get(&format!("{}.{}", enum_name, variant_name))
    }

    fn new(aliases: std::collections::HashMap<String, (Vec<String>, Type)>,
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
    }
}

/// 为枚举的泛型参数生成 fresh 替换映射
fn fresh_subst_for_generics(ctx: &mut InferCtx, generics: &[String]) -> HashMap<String, Type> {
    generics.iter()
        .map(|g| (g.clone(), ctx.fresh_ty(0)))
        .collect()
}

/// 计算枚举变体构造的返回类型（使用新的 fresh 泛型变量）
fn enum_self_type(variant: &EnumVariant, ctx: &mut InferCtx) -> Type {
    let subst = fresh_subst_for_generics(ctx, &variant.generics);
    enum_self_type_with_subst(variant, ctx, &subst)
}

/// 计算枚举变体构造的返回类型，使用外部提供的替换映射（保证 payload 与 return 共享 subst）
fn enum_self_type_with_subst(variant: &EnumVariant, ctx: &mut InferCtx, subst: &HashMap<String, Type>) -> Type {
    if let Some(ret) = &variant.return_type {
        apply_subst(subst, ret)
    } else if variant.generics.is_empty() {
        Type::Named(variant.enum_name.clone())
    } else {
        let args: Vec<Type> = variant.generics.iter()
            .map(|g| subst.get(g).cloned().unwrap_or_else(|| ctx.fresh_ty(0)))
            .collect();
        Type::Generic {
            base: Box::new(Type::Named(variant.enum_name.clone())),
            args,
        }
    }
}

/// 别名：对类型应用替换映射
fn apply_subst(subst: &HashMap<String, Type>, t: &Type) -> Type {
    substitute(subst, t)
}


/// 在类型 `t` 中展开所有已知类型别名引用（递归）。
/// - `Named("Alias")` → 别名底层类型（无参）
/// - `Generic { base: Named("Alias"), args }` → 将别名泛型参数替换为 args 后展开 body
/// 别名自身的定义在 `aliases` 的 value 中已是展开后的类型。
fn expand_type(aliases: &std::collections::HashMap<String, (Vec<String>, Type)>, t: &Type) -> Type {
    match t {
        Type::Named(name) => {
            if let Some((params, body)) = aliases.get(name) {
                if params.is_empty() {
                    return body.clone();
                }
                // 有参别名但此处无实参（裸 Named），无法替换 → 返回已存 body（含自由命名参数）
                return body.clone();
            }
            t.clone()
        }
        Type::Generic { base, args } => {
            if let Type::Named(name) = base.as_ref() {
                if let Some((params, body)) = aliases.get(name) {
                    // 构造泛型参数替换映射：params[i] -> 展开后的 args[i]
                    let subst: std::collections::HashMap<String, Type> = params.iter().cloned()
                        .zip(args.iter().map(|a| expand_type(aliases, a)))
                        .collect();
                    return substitute(&subst, body);
                }
            }
            let new_base = match base.as_ref() {
                Type::Named(n) => Type::Named(n.clone()),
                other => expand_type(aliases, other),
            };
            let new_args = args.iter().map(|a| expand_type(aliases, a)).collect();
            Type::Generic { base: Box::new(new_base), args: new_args }
        }
        Type::Option(i) => Type::Option(Box::new(expand_type(aliases, i))),
        Type::Result { ok, err } => Type::Result {
            ok: Box::new(expand_type(aliases, ok)),
            err: Box::new(expand_type(aliases, err)),
        },
        Type::Tuple(ts) => Type::Tuple(ts.iter().map(|x| expand_type(aliases, x)).collect()),
        Type::Ref(i) => Type::Ref(Box::new(expand_type(aliases, i))),
        Type::MutRef(i) => Type::MutRef(Box::new(expand_type(aliases, i))),
        Type::Optional(i) => Type::Optional(Box::new(expand_type(aliases, i))),
        Type::Fn { params, ret } => Type::Fn {
            params: params.iter().map(|x| expand_type(aliases, x)).collect(),
            ret: Box::new(expand_type(aliases, ret)),
        },
        Type::Simd { elem, width } => Type::Simd { elem: Box::new(expand_type(aliases, elem)), width: *width },
        Type::Union(ts) => Type::Union(ts.iter().map(|x| expand_type(aliases, x)).collect()),
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|x| expand_type(aliases, x)).collect()),
        Type::Constructor { name, arity } => Type::Constructor { name: name.clone(), arity: *arity },
        Type::Apply { constructor, args } => Type::Apply {
            constructor: Box::new(expand_type(aliases, constructor)),
            args: args.iter().map(|a| expand_type(aliases, a)).collect(),
        },
        _ => t.clone(),
    }
}

/// 将类型 `t` 中所有命名参数（Named）按 `subst` 映射替换（用于泛型别名实参代入）
fn substitute(subst: &std::collections::HashMap<String, Type>, t: &Type) -> Type {
    match t {
        Type::Named(name) => {
            if let Some(repl) = subst.get(name) {
                return repl.clone();
            }
            t.clone()
        }
        Type::Generic { base, args } => {
            let new_base = substitute(subst, base);
            let new_args = args.iter().map(|a| substitute(subst, a)).collect();
            Type::Generic { base: Box::new(new_base), args: new_args }
        }
        Type::Option(i) => Type::Option(Box::new(substitute(subst, i))),
        Type::Result { ok, err } => Type::Result {
            ok: Box::new(substitute(subst, ok)),
            err: Box::new(substitute(subst, err)),
        },
        Type::Tuple(ts) => Type::Tuple(ts.iter().map(|x| substitute(subst, x)).collect()),
        Type::Ref(i) => Type::Ref(Box::new(substitute(subst, i))),
        Type::MutRef(i) => Type::MutRef(Box::new(substitute(subst, i))),
        Type::Optional(i) => Type::Optional(Box::new(substitute(subst, i))),
        Type::Fn { params, ret } => Type::Fn {
            params: params.iter().map(|x| substitute(subst, x)).collect(),
            ret: Box::new(substitute(subst, ret)),
        },
        Type::Simd { elem, width } => Type::Simd { elem: Box::new(substitute(subst, elem)), width: *width },
        Type::Union(ts) => Type::Union(ts.iter().map(|x| substitute(subst, x)).collect()),
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|x| substitute(subst, x)).collect()),
        Type::Constructor { name, arity } => {
            if let Some(repl) = subst.get(name) {
                return repl.clone();
            }
            Type::Constructor { name: name.clone(), arity: *arity }
        }
        Type::Apply { constructor, args } => Type::Apply {
            constructor: Box::new(substitute(subst, constructor)),
            args: args.iter().map(|a| substitute(subst, a)).collect(),
        },
        _ => t.clone(),
    }
}

/// 合并两个分支类型：能统一则取统一结果，否则构造最小联合类型。
fn merge_branch_types(ctx: &mut InferCtx, a: Type, b: Type) -> Type {
    let za = zonk(ctx, &a);
    let zb = zonk(ctx, &b);
    if unify(ctx, &za, &zb).is_ok() {
        za
    } else {
        flatten_union(vec![za, zb])
    }
}

/// 扁平化并去重联合类型成员。
fn flatten_union(types: Vec<Type>) -> Type {
    let mut flat = Vec::new();
    for t in types {
        if let Type::Union(inner) = t {
            for it in inner {
                if !flat.contains(&it) {
                    flat.push(it);
                }
            }
        } else if !flat.contains(&t) {
            flat.push(t);
        }
    }
    if flat.len() == 1 {
        flat.into_iter().next().unwrap()
    } else {
        Type::Union(flat)
    }
}

/// 从类型中提取最外层类型名（用于方法/字段查找）
///
/// - `Named("Point")` → Some("Point")
/// - `Generic { base: Named("List"), .. }` → Some("List")
/// - `Ref(Named("Point"))` → Some("Point")
/// - `MutRef(Generic { base: Named("Option"), .. })` → Some("Option")
fn resolve_type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Named(name) => Some(name.clone()),
        Type::Generic { base, .. } => match base.as_ref() {
            Type::Named(name) => Some(name.clone()),
            _ => None,
        },
        Type::Apply { constructor, .. } => match constructor.as_ref() {
            Type::Named(name) | Type::Constructor { name, .. } => Some(name.clone()),
            _ => resolve_type_name(constructor),
        },
        Type::Ref(inner) | Type::MutRef(inner) => resolve_type_name(inner),
        Type::Option(inner) | Type::Optional(inner) => resolve_type_name(inner),
        _ => None,
    }
}

/// 判断类型中是否仍含未解析的推断变量
fn type_contains_var(ty: &Type) -> bool {
    match ty {
        Type::Var(_) => true,
        Type::Option(inner) | Type::Optional(inner) | Type::Ref(inner) | Type::MutRef(inner) =>
            type_contains_var(inner),
        Type::Result { ok, err } => type_contains_var(ok) || type_contains_var(err),
        Type::Generic { base, args } | Type::Apply { constructor: base, args } =>
            type_contains_var(base) || args.iter().any(type_contains_var),
        Type::Tuple(ts) | Type::Union(ts) | Type::Intersection(ts) | Type::Futures(ts) =>
            ts.iter().any(type_contains_var),
        Type::Fn { params, ret } =>
            params.iter().any(type_contains_var) || type_contains_var(ret),
        Type::Record(fields) => fields.iter().any(|(_, t)| type_contains_var(t)),
        Type::Simd { elem, .. } | Type::Future(elem) => type_contains_var(elem),
        _ => false,
    }
}

/// 为注册表注入内置类型（List/Str/Option 等）的常用方法签名
fn inject_builtin_methods(registry: &mut std::collections::HashMap<String,
        std::collections::HashMap<String, (Vec<Type>, Type)>>) {
    // List<T> / Vec<T>
    let mut list_methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
        std::collections::HashMap::new();
    list_methods.insert("len".into(), (vec![], Type::Int));
    list_methods.insert("is_empty".into(), (vec![], Type::Bool));
    list_methods.insert("push".into(), (vec![Type::Named("T".into())], Type::Unit));
    list_methods.insert("pop".into(), (vec![], Type::Unit));  // 泛型不实例化，codegen 处理 .pop().unwrap()
    registry.insert("List".into(), list_methods);

    // Vec<T> 是 List 的 Rust 映射名
    let mut vec_methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
        std::collections::HashMap::new();
    vec_methods.insert("len".into(), (vec![], Type::Int));
    vec_methods.insert("is_empty".into(), (vec![], Type::Bool));
    registry.insert("Vec".into(), vec_methods);

    // String / Str
    let mut str_methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
        std::collections::HashMap::new();
    str_methods.insert("len".into(), (vec![], Type::Int));
    str_methods.insert("is_empty".into(), (vec![], Type::Bool));
    str_methods.insert("as_str".into(), (vec![], Type::Str));
    str_methods.insert("to_string".into(), (vec![], Type::Str));
    str_methods.insert("clone".into(), (vec![], Type::Str));
    registry.insert("String".into(), str_methods.clone());
    registry.insert("str".into(), str_methods);

    // Option<T>
    let mut option_methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
        std::collections::HashMap::new();
    option_methods.insert("is_some".into(), (vec![], Type::Bool));
    option_methods.insert("is_none".into(), (vec![], Type::Bool));
    option_methods.insert("unwrap".into(), (vec![], Type::Unit));  // 泛型不实例化
    registry.insert("Option".into(), option_methods);

    // Result<T, E>
    let mut result_methods: std::collections::HashMap<String, (Vec<Type>, Type)> =
        std::collections::HashMap::new();
    result_methods.insert("is_ok".into(), (vec![], Type::Bool));
    result_methods.insert("is_err".into(), (vec![], Type::Bool));
    result_methods.insert("ok".into(), (vec![], Type::Optional(Box::new(Type::Named("T".into())))));
    result_methods.insert("err".into(), (vec![], Type::Optional(Box::new(Type::Named("E".into())))));
    registry.insert("Result".into(), result_methods);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    /// 快速构造一个含 let 绑定和函数调用的模块，验证推断填充类型
    #[test]
    fn test_typer_fills_let_binding() {
        let mut module = Module {
            imports: vec![],
            functions: vec![Function {
                name: "add".into(),
                generics: vec![],
                generic_kinds: Vec::new(),
                generic_bounds: vec![],
                generic_defaults: vec![],
                params: vec![
                    Param { name: "x".into(), ty: Some(Type::Int), default: None, is_mut: false, is_owned: false, is_ref: false, is_positional_only: false },
                    Param { name: "y".into(), ty: Some(Type::Int), default: None, is_mut: false, is_owned: false, is_ref: false, is_positional_only: false },
                ],
                return_type: None,
                raises: None,
                where_clause: vec![],
                body: vec![
                    Stmt::Let {
                        name: "z".into(), mutable: false, is_ref: false, comptime: false,
                        ty: None,
                        value: Expr::Binary {
                            left: Box::new(Expr::Ident("x".into())),
                            op: BinOp::Add,
                            right: Box::new(Expr::Ident("y".into())),
                        },
                    },
                    Stmt::Return(Some(Expr::Ident("z".into()))),
                ],
                is_async: false, is_abstract: false, comptime: false,
                decorators: vec![],
                attributes: vec![],
                variadic: None, params_checker: None,
            }],
            structs: vec![],
            traits: vec![],
            impls: vec![],
            consts: vec![],
            type_aliases: vec![],
            magic_decls: vec![],
            tests: vec![],
            name: Some("test".into()),
            file_path: Some("test.lz".into()),
            package: None,
            is_macro: false,
            doc: None,
        };

        let errors = Typer::infer_module(&mut module);
        assert!(errors.is_empty(), "infer errors: {:?}", errors);

        // let z = x + y 应被推断为 Int
        let f = &module.functions[0];
        let let_stmt = &f.body[0];
        if let Stmt::Let { ty, .. } = let_stmt {
            assert!(ty.is_some(), "let z type should be inferred");
            let inferred = ty.as_ref().unwrap();
            // 应为 Int（Int + Int → Int）
            assert_eq!(inferred.to_rust_type_string(), "i64",
                "expected i64 for Int-Inferred, got: {}", inferred.to_rust_type_string());
        } else {
            panic!("expected Stmt::Let");
        }

        // 返回类型应被推断为 Int
        assert!(f.return_type.is_some(), "return type should be inferred");
        assert_eq!(f.return_type.as_ref().unwrap().to_rust_type_string(), "i64");
    }

    #[test]
    fn test_typer_int_binary() {
        let mut module = Module {
            imports: vec![],
            functions: vec![Function {
                name: "calc".into(),
                generics: vec![],
                generic_kinds: Vec::new(),
                generic_bounds: vec![],
                generic_defaults: vec![],
                params: vec![],
                return_type: None,
                raises: None,
                where_clause: vec![],
                body: vec![
                    Stmt::Let {
                        name: "a".into(), mutable: false, is_ref: false, comptime: false,
                        ty: None,
                        value: Expr::IntLit(10),
                    },
                    Stmt::Let {
                        name: "b".into(), mutable: false, is_ref: false, comptime: false,
                        ty: None,
                        value: Expr::Binary {
                            left: Box::new(Expr::Ident("a".into())),
                            op: BinOp::Mul,
                            right: Box::new(Expr::IntLit(3)),
                        },
                    },
                    Stmt::Return(Some(Expr::Ident("b".into()))),
                ],
                is_async: false, is_abstract: false, comptime: false,
                decorators: vec![],
                attributes: vec![],
                variadic: None, params_checker: None,
            }],
            structs: vec![],
            traits: vec![],
            impls: vec![],
            consts: vec![ConstDef {
                name: "MAX".into(),
                ty: None,
                value: Expr::IntLit(100),
                mutable: false,
                comptime: false,
            }],
            type_aliases: vec![],
            magic_decls: vec![],
            tests: vec![],
            name: Some("test".into()),
            file_path: Some("test.lz".into()),
            package: None,
            is_macro: false,
            doc: None,
        };

        let errors = Typer::infer_module(&mut module);
        assert!(errors.is_empty(), "infer errors: {:?}", errors);

        // let a = 10 → a: Int
        let a_ty = match &module.functions[0].body[0] {
            Stmt::Let { ty, .. } => ty.clone(),
            _ => panic!("expected Stmt::Let"),
        };
        assert_eq!(a_ty.unwrap().to_rust_type_string(), "i64");

        // let b = a * 3 → b: Int
        let b_ty = match &module.functions[0].body[1] {
            Stmt::Let { ty, .. } => ty.clone(),
            _ => panic!("expected Stmt::Let"),
        };
        assert_eq!(b_ty.unwrap().to_rust_type_string(), "i64");

        // const MAX: inferred as Int
        assert_eq!(module.consts[0].ty.as_ref().unwrap().to_rust_type_string(), "i64");
    }

    /// 端到端验证 trait Show + impl Show for int + 泛型函数 print_show[T: Show]
    #[test]
    fn test_typer_resolves_trait_instance() {
        let show_method = Function {
            name: "show".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            params: vec![Param {
                name: "self".into(),
                ty: Some(Type::Self_),
                default: None,
                is_mut: false,
                is_owned: false,
                is_ref: false,
                is_positional_only: false,
            }],
            return_type: Some(Type::Str),
            raises: None,
            where_clause: vec![],
            body: vec![],
            is_async: false,
            is_abstract: false,
            comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None,
            params_checker: None,
        };

        let trait_show = TraitDef {
            name: "Show".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            methods: vec![show_method.clone()],
            fields: vec![],
            type_aliases: vec![],
        };

        let impl_show_int = ImplDef {
            trait_name: Some("Show".into()),
            type_name: "int".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            where_clause: vec![],
            methods: vec![Function {
                name: "show".into(),
                generics: vec![],
                generic_kinds: vec![],
                generic_bounds: vec![],
                generic_defaults: vec![],
                params: vec![Param {
                    name: "self".into(),
                    ty: Some(Type::Int),
                    default: None,
                    is_mut: false,
                    is_owned: false,
                    is_ref: false,
                    is_positional_only: false,
                }],
                return_type: Some(Type::Str),
                raises: None,
                where_clause: vec![],
                body: vec![Stmt::Return(Some(Expr::StrLit("".into())))],
                is_async: false,
                is_abstract: false,
                comptime: false,
                decorators: vec![],
                attributes: vec![],
                variadic: None,
                params_checker: None,
            }],
            type_aliases: vec![],
        };

        let print_show = Function {
            name: "print_show".into(),
            generics: vec!["T".into()],
            generic_kinds: vec![],
            generic_bounds: vec![("T".into(), vec![Type::Named("Show".into())])],
            generic_defaults: vec![],
            params: vec![Param {
                name: "x".into(),
                ty: Some(Type::Named("T".into())),
                default: None,
                is_mut: false,
                is_owned: false,
                is_ref: false,
                is_positional_only: false,
            }],
            return_type: Some(Type::Str),
            raises: None,
            where_clause: vec![],
            body: vec![Stmt::Return(Some(Expr::MethodCall {
                receiver: Box::new(Expr::Ident("x".into())),
                method: "show".into(),
                args: vec![],
            }))],
            is_async: false,
            is_abstract: false,
            comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None,
            params_checker: None,
        };

        let main_fn = Function {
            name: "main".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            params: vec![],
            return_type: None,
            raises: None,
            where_clause: vec![],
            body: vec![Stmt::Expr(Expr::Call {
                func: Box::new(Expr::Ident("print_show".into())),
                args: vec![Expr::IntLit(42)],
                checker: None,
            })],
            is_async: false,
            is_abstract: false,
            comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None,
            params_checker: None,
        };

        let mut module = Module {
            imports: vec![],
            functions: vec![print_show, main_fn],
            structs: vec![],
            traits: vec![trait_show],
            impls: vec![impl_show_int],
            consts: vec![],
            type_aliases: vec![],
            magic_decls: vec![],
            tests: vec![],
            name: Some("test".into()),
            file_path: Some("test.lz".into()),
            package: None,
            is_macro: false,
            doc: None,
        };

        let errors = Typer::infer_module(&mut module);
        assert!(errors.is_empty(), "expected no infer errors, got: {:?}", errors);
    }

    /// 端到端验证缺失 trait 实例时类型推断报错
    #[test]
    fn test_typer_rejects_missing_trait_instance() {
        let show_method = Function {
            name: "show".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            params: vec![Param {
                name: "self".into(),
                ty: Some(Type::Self_),
                default: None,
                is_mut: false,
                is_owned: false,
                is_ref: false,
                is_positional_only: false,
            }],
            return_type: Some(Type::Str),
            raises: None,
            where_clause: vec![],
            body: vec![],
            is_async: false,
            is_abstract: false,
            comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None,
            params_checker: None,
        };

        let trait_show = TraitDef {
            name: "Show".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            methods: vec![show_method],
            fields: vec![],
            type_aliases: vec![],
        };

        let foo = Function {
            name: "foo".into(),
            generics: vec!["T".into()],
            generic_kinds: vec![],
            generic_bounds: vec![("T".into(), vec![Type::Named("Show".into())])],
            generic_defaults: vec![],
            params: vec![Param {
                name: "x".into(),
                ty: Some(Type::Named("T".into())),
                default: None,
                is_mut: false,
                is_owned: false,
                is_ref: false,
                is_positional_only: false,
            }],
            return_type: Some(Type::Named("T".into())),
            raises: None,
            where_clause: vec![],
            body: vec![Stmt::Return(Some(Expr::Ident("x".into())))],
            is_async: false,
            is_abstract: false,
            comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None,
            params_checker: None,
        };

        let main_fn = Function {
            name: "main".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            params: vec![],
            return_type: None,
            raises: None,
            where_clause: vec![],
            body: vec![Stmt::Expr(Expr::Call {
                func: Box::new(Expr::Ident("foo".into())),
                args: vec![Expr::IntLit(42)],
                checker: None,
            })],
            is_async: false,
            is_abstract: false,
            comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None,
            params_checker: None,
        };

        let mut module = Module {
            imports: vec![],
            functions: vec![foo, main_fn],
            structs: vec![],
            traits: vec![trait_show],
            impls: vec![],
            consts: vec![],
            type_aliases: vec![],
            magic_decls: vec![],
            tests: vec![],
            name: Some("test".into()),
            file_path: Some("test.lz".into()),
            package: None,
            is_macro: false,
            doc: None,
        };

        let errors = Typer::infer_module(&mut module);
        assert!(!errors.is_empty(), "expected infer error for missing Show instance");
        let joined = errors.join("\n");
        assert!(joined.contains("does not implement trait `Show`"),
            "expected Show trait error, got: {}", joined);
    }

    /// GADT 构造与模式匹配推断
    #[test]
    fn test_gadt_construct_and_match() {
        let expr_enum = StructDef {
            name: "Expr".into(),
            generics: vec!["T".into()],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            fields: vec![
                Field {
                    name: "Lit".into(),
                    ty: Type::Record(vec![("value".into(), Type::Int)]),
                    default: None,
                    variant_return: Some(Type::Generic {
                        base: Box::new(Type::Named("Expr".into())),
                        args: vec![Type::Int],
                    }),
                },
                Field {
                    name: "Add".into(),
                    ty: Type::Record(vec![
                        ("left".into(), Type::Generic {
                            base: Box::new(Type::Named("Expr".into())),
                            args: vec![Type::Int],
                        }),
                        ("right".into(), Type::Generic {
                            base: Box::new(Type::Named("Expr".into())),
                            args: vec![Type::Int],
                        }),
                    ]),
                    default: None,
                    variant_return: Some(Type::Generic {
                        base: Box::new(Type::Named("Expr".into())),
                        args: vec![Type::Int],
                    }),
                },
            ],
            methods: vec![],
            is_enum: true,
            decorators: vec![],
            attributes: vec![],
            repr_attr: None,
        };

        let main_fn = Function {
            name: "main".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            params: vec![],
            return_type: Some(Type::Int),
            raises: None,
            where_clause: vec![],
            body: vec![
                Stmt::Let {
                    name: "e".into(), mutable: false, is_ref: false, comptime: false,
                    ty: Some(Type::Generic {
                        base: Box::new(Type::Named("Expr".into())),
                        args: vec![Type::Int],
                    }),
                    value: Expr::MethodCall {
                        receiver: Box::new(Expr::Ident("Expr".into())),
                        method: "Lit".into(),
                        args: vec![Expr::IntLit(42)],
                    },
                },
                Stmt::Return(Some(Expr::Match {
                    expr: Box::new(Expr::Ident("e".into())),
                    arms: vec![MatchArm {
                        pattern: Pattern::Variant("Expr.Lit".into(), vec![Pattern::Ident("x".into())]),
                        guard: None,
                        body: vec![
                            Stmt::Let {
                                name: "z".into(), mutable: false, is_ref: false, comptime: false,
                                ty: Some(Type::Int),
                                value: Expr::Ident("x".into()),
                            },
                            Stmt::Expr(Expr::Ident("z".into())),
                        ],
                    },
                    MatchArm {
                        pattern: Pattern::Wildcard,
                        guard: None,
                        body: vec![Stmt::Expr(Expr::IntLit(0))],
                    }],
                })),
            ],
            is_async: false, is_abstract: false, comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None, params_checker: None,
        };

        let mut module = Module {
            imports: vec![],
            functions: vec![main_fn],
            structs: vec![expr_enum],
            traits: vec![],
            impls: vec![],
            consts: vec![],
            type_aliases: vec![],
            magic_decls: vec![],
            tests: vec![],
            name: Some("test".into()),
            file_path: Some("test.lz".into()),
            package: None,
            is_macro: false,
            doc: None,
        };

        let errors = Typer::infer_module(&mut module);
        assert!(errors.is_empty(), "infer errors: {:?}", errors);
    }

    /// 泛型枚举变体返回 fresh 泛型类型并与具体索引统一
    #[test]
    fn test_generic_enum_variant_return_fresh_generics() {
        let option_enum = StructDef {
            name: "Option".into(),
            generics: vec!["T".into()],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            fields: vec![
                Field {
                    name: "Some".into(),
                    ty: Type::Named("T".into()),
                    default: None,
                    variant_return: None,
                },
                Field {
                    name: "None".into(),
                    ty: Type::Unit,
                    default: None,
                    variant_return: None,
                },
            ],
            methods: vec![],
            is_enum: true,
            decorators: vec![],
            attributes: vec![],
            repr_attr: None,
        };

        let main_fn = Function {
            name: "main".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            params: vec![],
            return_type: Some(Type::Int),
            raises: None,
            where_clause: vec![],
            body: vec![
                Stmt::Let {
                    name: "o".into(), mutable: false, is_ref: false, comptime: false,
                    ty: Some(Type::Generic {
                        base: Box::new(Type::Named("Option".into())),
                        args: vec![Type::Int],
                    }),
                    value: Expr::MethodCall {
                        receiver: Box::new(Expr::Ident("Option".into())),
                        method: "Some".into(),
                        args: vec![Expr::IntLit(1)],
                    },
                },
                Stmt::Return(Some(Expr::Match {
                    expr: Box::new(Expr::Ident("o".into())),
                    arms: vec![MatchArm {
                        pattern: Pattern::Variant("Option.Some".into(), vec![Pattern::Ident("v".into())]),
                        guard: None,
                        body: vec![
                            Stmt::Let {
                                name: "w".into(), mutable: false, is_ref: false, comptime: false,
                                ty: Some(Type::Int),
                                value: Expr::Ident("v".into()),
                            },
                            Stmt::Expr(Expr::Ident("w".into())),
                        ],
                    },
                    MatchArm {
                        pattern: Pattern::Wildcard,
                        guard: None,
                        body: vec![Stmt::Expr(Expr::IntLit(0))],
                    }],
                })),
            ],
            is_async: false, is_abstract: false, comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None, params_checker: None,
        };

        let mut module = Module {
            imports: vec![],
            functions: vec![main_fn],
            structs: vec![option_enum],
            traits: vec![],
            impls: vec![],
            consts: vec![],
            type_aliases: vec![],
            magic_decls: vec![],
            tests: vec![],
            name: Some("test".into()),
            file_path: Some("test.lz".into()),
            package: None,
            is_macro: false,
            doc: None,
        };

        let errors = Typer::infer_module(&mut module);
        assert!(errors.is_empty(), "infer errors: {:?}", errors);
    }
}

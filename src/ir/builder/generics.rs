// Lang-Zone 编译器 — ir/builder/generics.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

/// 从 AST 表达式提取类型名称列表（支持单类型和多类型参数）
/// - Ident("int") → Some(vec!["int"])
/// - TupleLit([Ident("int"), Ident("str")]) → Some(vec!["int", "str"])
/// - 其他 → None
pub(crate) fn extract_type_names(expr: &AstExpr) -> Option<Vec<String>> {
    match expr {
        AstExpr::Ident(name) => Some(vec![name.clone()]),
        AstExpr::TupleLit(elems) => {
            let names: Option<Vec<String>> = elems
                .iter()
                .map(|e| {
                    if let AstExpr::Ident(n) = e {
                        Some(n.clone())
                    } else {
                        None
                    }
                })
                .collect();
            names
        }
        _ => None,
    }
}

/// 将 LZ 类型名映射为 Rust 类型名（用于泛型类型参数）
pub(crate) fn map_type_args(names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|t| match t.as_str() {
            "int" => "i64".to_string(),
            "str" => "String".to_string(),
            "f64" | "float" => "f64".to_string(),
            "bool" => "bool".to_string(),
            other => other.to_string(),
        })
        .collect()
}

/// 从实参类型解析泛型函数调用：推断泛型参数的具体类型
///
/// 策略：
/// 1. 收集函数定义中泛型参数名列表（从 param_tys 和 ret_ty 中提取 Generic 变量）
/// 2. 对每个参数位置，尝试将定义的 param_ty 与实参 arg_ty 匹配
/// 3. 如果 param_ty 是 Generic("T") 且 arg_ty 是具体类型，则将 T 绑定到 arg_ty
/// 4. 用绑定结果替换 ret_ty 中的泛型变量
/// 用显式 turbofish 类型参数（`parse_num.<int>("42")`）替换返回类型中的泛型。
/// 泛型名按返回类型中出现的顺序与 type_args 一一对应（T → 第一个实参类型）。
pub(crate) fn apply_explicit_type_args(ret_ty: &IrType, type_args: &[String]) -> IrType {
    // 收集返回类型中的泛型名（按出现顺序去重）
    let mut generics: Vec<String> = Vec::new();
    fn collect(ty: &IrType, out: &mut Vec<String>) {
        match ty {
            IrType::Generic(name) => {
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
            IrType::Named { args, .. } => {
                for a in args {
                    collect(a, out);
                }
            }
            IrType::Option(inner) => collect(inner, out),
            IrType::Result { ok, err } => {
                collect(ok, out);
                collect(err, out);
            }
            IrType::Tuple(elems) => {
                for e in elems {
                    collect(e, out);
                }
            }
            IrType::Fn { params, ret } => {
                for p in params {
                    collect(p, out);
                }
                collect(ret, out);
            }
            IrType::Ref(inner) | IrType::MutRef(inner) => collect(inner, out),
            IrType::Duck { fields } => {
                for (_, t) in fields {
                    collect(t, out);
                }
            }
            _ => {}
        }
    }
    collect(ret_ty, &mut generics);

    // 构造替换映射：generics[i] → type_args[i] 转换的 IrType
    let mut subst: std::collections::HashMap<String, IrType> = std::collections::HashMap::new();
    for (i, g) in generics.iter().enumerate() {
        let concrete = type_args
            .get(i)
            .map(|s| from_ast_type_name(s))
            .unwrap_or(IrType::Any);
        subst.insert(g.clone(), concrete);
    }

    // 递归替换
    fn replace(ty: &IrType, subst: &std::collections::HashMap<String, IrType>) -> IrType {
        match ty {
            IrType::Generic(name) => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
            IrType::Named { path, args } => IrType::Named {
                path: path.clone(),
                args: args.iter().map(|a| replace(a, subst)).collect(),
            },
            IrType::Option(inner) => IrType::Option(Box::new(replace(inner, subst))),
            IrType::Result { ok, err } => IrType::Result {
                ok: Box::new(replace(ok, subst)),
                err: Box::new(replace(err, subst)),
            },
            IrType::Tuple(elems) => {
                IrType::Tuple(elems.iter().map(|e| replace(e, subst)).collect())
            }
            IrType::Fn { params, ret } => IrType::Fn {
                params: params.iter().map(|p| replace(p, subst)).collect(),
                ret: Box::new(replace(ret, subst)),
            },
            IrType::Ref(inner) => IrType::Ref(Box::new(replace(inner, subst))),
            IrType::MutRef(inner) => IrType::MutRef(Box::new(replace(inner, subst))),
            IrType::Duck { fields } => IrType::Duck {
                fields: fields
                    .iter()
                    .map(|(n, t)| (n.clone(), replace(t, subst)))
                    .collect(),
            },
            other => other.clone(),
        }
    }
    replace(ret_ty, &subst)
}

pub(crate) fn resolve_call_generics(
    ret_ty: &IrType,
    fn_name: &str,
    param_tys: &[IrType],
    arg_tys: &[IrType],
    ctx: &TypeCtx,
) -> IrType {
    // 收集所有泛型参数名
    let mut generic_names = std::collections::HashSet::new();
    fn collect_generics(ty: &IrType, set: &mut std::collections::HashSet<String>) {
        match ty {
            IrType::Generic(name) => {
                set.insert(name.clone());
            }
            IrType::Named { args, .. } => {
                for a in args {
                    collect_generics(a, set);
                }
            }
            IrType::Option(inner) => collect_generics(inner, set),
            IrType::Result { ok, err } => {
                collect_generics(ok, set);
                collect_generics(err, set);
            }
            IrType::Tuple(elems) => {
                for e in elems {
                    collect_generics(e, set);
                }
            }
            IrType::Fn { params, ret } => {
                for p in params {
                    collect_generics(p, set);
                }
                collect_generics(ret, set);
            }
            IrType::Ref(inner) | IrType::MutRef(inner) => collect_generics(inner, set),
            IrType::Duck { fields } => {
                for (_, t) in fields {
                    collect_generics(t, set);
                }
            }
            _ => {}
        }
    }
    for pt in param_tys {
        collect_generics(pt, &mut generic_names);
    }
    collect_generics(ret_ty, &mut generic_names);

    if generic_names.is_empty() {
        return ret_ty.clone();
    }

    // 尝试从实参类型推断泛型绑定
    let mut bindings: std::collections::HashMap<String, IrType> = std::collections::HashMap::new();
    let n = param_tys.len().min(arg_tys.len());
    for i in 0..n {
        infer_generic_binding(&param_tys[i], &arg_tys[i], &mut bindings);
    }

    // 如果没有任何绑定（例如无参泛型函数），尝试从 ctx 的泛型列表推断
    if bindings.is_empty() && !ctx.current_generics.is_empty() {
        // 使用当前位置的泛型参数（当前函数定义的泛型）
        // 这处理了同一泛型函数内调用自身或其他泛型函数的情况
        let mut alt_bindings = std::collections::HashMap::new();
        for g in &ctx.current_generics {
            if generic_names.contains(g) {
                alt_bindings.insert(g.clone(), IrType::Generic(g.clone()));
            }
        }
        if !alt_bindings.is_empty() {
            // 有上层泛型上下文 → 传播泛型变量
            let generics: Vec<String> = alt_bindings.keys().cloned().collect();
            let concrete: Vec<IrType> = alt_bindings.values().cloned().collect();
            return ret_ty.substitute_generics(&generics, &concrete);
        }
    }

    if bindings.is_empty() {
        // G2 反例：无实参且无法推断 → 必须拒绝（如 `def f<T>() -> T` 后 `f()`：
        // 返回类型 T 无法绑定，属于程序错误）。
        // 注意：带实参的调用（即使本浅层类型检查上下文因变量类型为 Any 而暂时
        // 无法推断），由真正的 IR 构建/codegen 在拥有完整类型信息的上下文中处理
        // （如泛型函数内 `fold1(xs, f)` 会经 alt_bindings 传播上层泛型，或 Rust
        // 在调用点据实参推断）。此处不报错，避免对合法泛型调用误报。
        if arg_tys.is_empty() {
            ctx.report_error(format!(
                "无法推断泛型参数: 调用 {fn_name} 未提供显式类型实参（如 {fn_name}.<T>(...)），且无法从实参推断"
            ));
        }
        return ret_ty.clone();
    }

    let generics: Vec<String> = bindings.keys().cloned().collect();
    let concrete: Vec<IrType> = bindings.values().cloned().collect();
    ret_ty.substitute_generics(&generics, &concrete)
}

/// 尝试从 (param_ty, arg_ty) 对中推断泛型绑定
pub(crate) fn infer_generic_binding(
    param_ty: &IrType,
    arg_ty: &IrType,
    bindings: &mut std::collections::HashMap<String, IrType>,
) {
    match param_ty {
        IrType::Generic(name) => {
            // 直接绑定：T = arg_ty
            // 只有 arg_ty 不是 Any 也不是 Generic 时才绑定
            if !matches!(arg_ty, IrType::Any | IrType::Generic(_)) {
                bindings
                    .entry(name.clone())
                    .or_insert_with(|| arg_ty.clone());
            }
        }
        IrType::Named {
            path: p_path,
            args: p_args,
        } => {
            // 跨表示桥接：let 注解经 from_ast_type 产出 Named("Result"/"Option")，
            // 函数签名经 from_ast_type_with_generics 产出 Result/Option 变体；
            // 两侧表示不互通会导致泛型零绑定（lib_result and_then 实测）
            match (p_path.as_str(), arg_ty) {
                (
                    "Result",
                    IrType::Result {
                        ok: a_ok,
                        err: a_err,
                    },
                ) if p_args.len() == 2 => {
                    infer_generic_binding(&p_args[0], a_ok, bindings);
                    infer_generic_binding(&p_args[1], a_err, bindings);
                }
                ("Option", IrType::Option(a_inner)) if p_args.len() == 1 => {
                    infer_generic_binding(&p_args[0], a_inner, bindings);
                }
                _ => {
                    if let IrType::Named {
                        path: a_path,
                        args: a_args,
                    } = arg_ty
                    {
                        if p_path == a_path {
                            for (p, a) in p_args.iter().zip(a_args.iter()) {
                                infer_generic_binding(p, a, bindings);
                            }
                        }
                    }
                }
            }
        }
        IrType::Option(p_inner) => {
            if let IrType::Option(a_inner) = arg_ty {
                infer_generic_binding(p_inner, a_inner, bindings);
            }
        }
        IrType::Result {
            ok: p_ok,
            err: p_err,
        } => {
            if let IrType::Result {
                ok: a_ok,
                err: a_err,
            } = arg_ty
            {
                infer_generic_binding(p_ok, a_ok, bindings);
                infer_generic_binding(p_err, a_err, bindings);
            }
        }
        IrType::Tuple(p_elems) => {
            if let IrType::Tuple(a_elems) = arg_ty {
                for (p, a) in p_elems.iter().zip(a_elems.iter()) {
                    infer_generic_binding(p, a, bindings);
                }
            }
        }
        IrType::Ref(p_inner) => {
            if let IrType::Ref(a_inner) = arg_ty {
                infer_generic_binding(p_inner, a_inner, bindings);
            }
        }
        IrType::Fn {
            params: p_ps,
            ret: p_ret,
        } => {
            // 闭包实参（infer_expr_type 对带注解闭包产出 Fn）→ 逐参数/返回匹配：
            // `map_res(a, |x: int| -> int = x*2)` 形参 Fn{params:[T],ret:U} 与
            // 实参 Fn{params:[Int],ret:Int} 匹配得 T=Int,U=Int（lib_result 链式）
            if let IrType::Fn {
                params: a_ps,
                ret: a_ret,
            } = arg_ty
            {
                for (p, a) in p_ps.iter().zip(a_ps.iter()) {
                    infer_generic_binding(p, a, bindings);
                }
                infer_generic_binding(p_ret, a_ret, bindings);
            }
        }
        _ => {}
    }
}

// Lang-Zone 编译器 — typer/subst.rs
// （由 typer/mod.rs move-only 拆出，逻辑零改动）

use super::*;

impl Typer {
    /// zonk 函数体内所有类型的递归
    pub(crate) fn zonk_function_types(ctx: &InferCtx, stmts: &mut [Stmt]) {
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
                Stmt::Guard {
                    cond, else_body, ..
                } => {
                    if let Some(c) = cond {
                        Self::zonk_expr(ctx, c);
                    }
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
                    if let Some(e) = expected {
                        Self::zonk_expr(ctx, e);
                    }
                }
                Stmt::Raise(expr) | Stmt::Yield(Some(expr)) => {
                    Self::zonk_expr(ctx, expr);
                }
                Stmt::YieldFrom { expr, transform } => {
                    Self::zonk_expr(ctx, expr);
                    if let Some(f) = transform {
                        Self::zonk_expr(ctx, f);
                    }
                }
                Stmt::Break(Some(expr)) | Stmt::Continue(Some(expr)) => Self::zonk_expr(ctx, expr),
                Stmt::Yield(None)
                | Stmt::Break(None)
                | Stmt::Continue(None)
                | Stmt::Return(None) => {}
                Stmt::FnDef(_) => {}     // 内嵌函数暂不支持
                Stmt::TypeAlias(_) => {} // 类型别名无需 zonk（类型已在推断时展开注册）
                Stmt::Pass => {}
                Stmt::If {
                    cond,
                    body,
                    elifs,
                    else_body,
                } => {
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

    pub(crate) fn zonk_expr(_ctx: &InferCtx, _expr: &mut Expr) {
        // Expr 中不含 Type 字段需要 zonk（类型注解在 Stmt/Param/Function 层）
        // 如果将来 Expr 含有类型标注，在这里递归处理
        // 目前 Expr 只在 CodeGen 阶段才需要 Rust 类型，Expr 本身不持有 Type
    }
}

/// 为枚举的泛型参数生成 fresh 替换映射
pub(crate) fn fresh_subst_for_generics(
    ctx: &mut InferCtx,
    generics: &[String],
) -> HashMap<String, Type> {
    generics
        .iter()
        .map(|g| (g.clone(), ctx.fresh_ty(0)))
        .collect()
}

/// 别名：对类型应用替换映射
pub(crate) fn apply_subst(subst: &HashMap<String, Type>, t: &Type) -> Type {
    substitute(subst, t)
}

/// 在类型 `t` 中展开所有已知类型别名引用（递归）。
/// - `Named("Alias")` → 别名底层类型（无参）
/// - `Generic { base: Named("Alias"), args }` → 将别名泛型参数替换为 args 后展开 body
/// 别名自身的定义在 `aliases` 的 value 中已是展开后的类型。
pub(crate) fn expand_type(
    aliases: &std::collections::HashMap<String, (Vec<String>, Type)>,
    t: &Type,
) -> Type {
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
                    let subst: std::collections::HashMap<String, Type> = params
                        .iter()
                        .cloned()
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
            Type::Generic {
                base: Box::new(new_base),
                args: new_args,
            }
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
        Type::Simd { elem, width } => Type::Simd {
            elem: Box::new(expand_type(aliases, elem)),
            width: *width,
        },
        Type::Union(ts) => Type::Union(ts.iter().map(|x| expand_type(aliases, x)).collect()),
        Type::Intersection(ts) => {
            Type::Intersection(ts.iter().map(|x| expand_type(aliases, x)).collect())
        }
        Type::Constructor { name, arity } => Type::Constructor {
            name: name.clone(),
            arity: *arity,
        },
        Type::Apply { constructor, args } => Type::Apply {
            constructor: Box::new(expand_type(aliases, constructor)),
            args: args.iter().map(|a| expand_type(aliases, a)).collect(),
        },
        _ => t.clone(),
    }
}

/// 将类型 `t` 中所有命名参数（Named）按 `subst` 映射替换（用于泛型别名实参代入）
pub(crate) fn substitute(subst: &std::collections::HashMap<String, Type>, t: &Type) -> Type {
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
            Type::Generic {
                base: Box::new(new_base),
                args: new_args,
            }
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
        Type::Simd { elem, width } => Type::Simd {
            elem: Box::new(substitute(subst, elem)),
            width: *width,
        },
        Type::Union(ts) => Type::Union(ts.iter().map(|x| substitute(subst, x)).collect()),
        Type::Intersection(ts) => {
            Type::Intersection(ts.iter().map(|x| substitute(subst, x)).collect())
        }
        Type::Constructor { name, arity } => {
            if let Some(repl) = subst.get(name) {
                return repl.clone();
            }
            Type::Constructor {
                name: name.clone(),
                arity: *arity,
            }
        }
        Type::Apply { constructor, args } => Type::Apply {
            constructor: Box::new(substitute(subst, constructor)),
            args: args.iter().map(|a| substitute(subst, a)).collect(),
        },
        _ => t.clone(),
    }
}

/// 合并两个分支类型：能统一则取统一结果，否则构造最小联合类型。
pub(crate) fn merge_branch_types(ctx: &mut InferCtx, a: Type, b: Type) -> Type {
    let za = zonk(ctx, &a);
    let zb = zonk(ctx, &b);
    if unify(ctx, &za, &zb).is_ok() {
        za
    } else {
        flatten_union(vec![za, zb])
    }
}

/// 扁平化并去重联合类型成员。
pub(crate) fn flatten_union(types: Vec<Type>) -> Type {
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
pub(crate) fn resolve_type_name(ty: &Type) -> Option<String> {
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
pub(crate) fn type_contains_var(ty: &Type) -> bool {
    match ty {
        Type::Var(_) => true,
        Type::Option(inner) | Type::Optional(inner) | Type::Ref(inner) | Type::MutRef(inner) => {
            type_contains_var(inner)
        }
        Type::Result { ok, err } => type_contains_var(ok) || type_contains_var(err),
        Type::Generic { base, args }
        | Type::Apply {
            constructor: base,
            args,
        } => type_contains_var(base) || args.iter().any(type_contains_var),
        Type::Tuple(ts) | Type::Union(ts) | Type::Intersection(ts) | Type::Futures(ts) => {
            ts.iter().any(type_contains_var)
        }
        Type::Fn { params, ret } => params.iter().any(type_contains_var) || type_contains_var(ret),
        Type::Record(fields) => fields.iter().any(|(_, t)| type_contains_var(t)),
        Type::Simd { elem, .. } | Type::Future(elem) => type_contains_var(elem),
        _ => false,
    }
}

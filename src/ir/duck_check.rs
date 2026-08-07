//! duck 结构匹配编译期检查（TypeScript 级别静态检查的 IR 层实现）
//!
//! 在 IR 构建完成后运行：对每个「具体类型被用作 duck 约束泛型实参」的调用点，
//! 验证该类型的方法 / 字段结构是否满足 duck 约束，不满足则报告 E 错误。
//! 零运行时开销 — 全部检查在编译期完成。

use std::collections::{HashMap, HashSet};

use super::types::IrType;
use super::IrModule;
use crate::ir::node::*;

/// 具体类型的方法签名（用于结构匹配）
#[derive(Default)]
struct TypeInfo {
    /// 方法名 → (非 self 参数类型列表, 返回类型, self 是否 mut)
    methods: HashMap<String, (Vec<IrType>, IrType, bool)>,
    /// 字段名 → 类型
    fields: HashMap<String, IrType>,
}

/// 对 IR 模块执行 duck 结构匹配检查，返回错误列表（可能为空）。
pub fn check_duck_satisfaction(ir: &IrModule) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();
    let mut checked: HashSet<(String, String)> = HashSet::new();

    // ── 1. 索引 duck 定义 ──
    let mut ducks: HashMap<&str, &DuckDef> = HashMap::new();
    for item in &ir.items {
        if let Item::DuckDef(d) = item {
            ducks.insert(d.name.as_str(), d);
        }
    }
    if ducks.is_empty() {
        return errors;
    }

    // ── 2. 索引具体类型（struct / enum）与方法签名 ──
    let mut types: HashMap<&str, TypeInfo> = HashMap::new();
    let mut fn_defs: HashMap<&str, &FnDef> = HashMap::new();
    for item in &ir.items {
        match item {
            Item::StructDef(s) => {
                let mut ti = TypeInfo::default();
                for f in &s.fields {
                    ti.fields.insert(f.name.clone(), f.ty.clone());
                }
                for m in &s.methods {
                    let (params, is_mut_self) = split_self(&m.params);
                    ti.methods
                        .insert(m.name.clone(), (params, m.ret_ty.clone(), is_mut_self));
                    fn_defs.insert(m.name.as_str(), m);
                }
                types.insert(s.name.as_str(), ti);
            }
            Item::EnumDef(e) => {
                let mut ti = TypeInfo::default();
                for m in &e.methods {
                    let (params, is_mut_self) = split_self(&m.params);
                    ti.methods
                        .insert(m.name.clone(), (params, m.ret_ty.clone(), is_mut_self));
                    fn_defs.insert(m.name.as_str(), m);
                }
                types.insert(e.name.as_str(), ti);
            }
            Item::FnDef(f) => {
                fn_defs.insert(f.name.as_str(), f);
            }
            _ => {}
        }
    }

    // ── 3. 遍历所有函数体，检查调用点 ──
    //     收集所有函数体（顶层函数 + struct/enum 方法）
    let mut bodies: Vec<&Block> = Vec::new();
    for item in &ir.items {
        match item {
            Item::FnDef(f) => bodies.push(&f.body),
            Item::StructDef(s) => bodies.extend(s.methods.iter().map(|m| &m.body)),
            Item::EnumDef(e) => bodies.extend(e.methods.iter().map(|m| &m.body)),
            Item::Test(t) => bodies.push(&t.body),
            _ => {}
        }
    }

    for body in &bodies {
        walk_block(body, &mut |expr| {
            if let ExprKind::Call { callee, args, .. } = &expr.kind {
                if let ExprKind::Var(fname) = &callee.kind {
                    if let Some(fdef) = fn_defs.get(fname.as_str()) {
                        check_call_site(fdef, args, &ducks, &types, &mut checked, &mut errors);
                    }
                }
            }
        });
    }

    errors
}

/// 拆分 self 参数：返回 (非 self 参数类型列表, self 是否 mut)
fn split_self(params: &[Param]) -> (Vec<IrType>, bool) {
    let mut tys = Vec::new();
    let mut is_mut_self = false;
    for p in params {
        if p.name == "self" {
            is_mut_self = p.is_mut;
        } else {
            tys.push(p.ty.clone());
        }
    }
    (tys, is_mut_self)
}

/// 收集需要自动生成 Rust impl 的 (具体类型名, duck 名) 对。
/// 仅当具体类型在调用点被用作 duck 约束泛型实参时才需要 impl。
pub fn collect_duck_impls(ir: &IrModule) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();

    // ── 1. 索引 duck 定义 ──
    let mut ducks: HashMap<&str, &DuckDef> = HashMap::new();
    for item in &ir.items {
        if let Item::DuckDef(d) = item {
            ducks.insert(d.name.as_str(), d);
        }
    }
    if ducks.is_empty() {
        return result;
    }

    // ── 2. 索引函数定义（含 struct/enum 方法） ──
    let mut fn_defs: HashMap<&str, &FnDef> = HashMap::new();
    let mut bodies: Vec<&Block> = Vec::new();
    for item in &ir.items {
        match item {
            Item::FnDef(f) => {
                fn_defs.insert(f.name.as_str(), f);
                bodies.push(&f.body);
            }
            Item::StructDef(s) => {
                for m in &s.methods {
                    fn_defs.insert(m.name.as_str(), m);
                    bodies.push(&m.body);
                }
            }
            Item::EnumDef(e) => {
                for m in &e.methods {
                    fn_defs.insert(m.name.as_str(), m);
                    bodies.push(&m.body);
                }
            }
            Item::Test(t) => bodies.push(&t.body),
            _ => {}
        }
    }

    // ── 3. 遍历所有函数体，收集调用点中「具体类型 + duck 约束」组合 ──
    for body in &bodies {
        walk_block(body, &mut |expr| {
            if let ExprKind::Call { callee, args, .. } = &expr.kind {
                if let ExprKind::Var(fname) = &callee.kind {
                    if let Some(fdef) = fn_defs.get(fname.as_str()) {
                        for g in &fdef.generics {
                            let duck_bounds: Vec<&str> = g
                                .bounds
                                .iter()
                                .filter_map(|b| {
                                    if let IrType::Named { path, .. } = b {
                                        if ducks.contains_key(path.as_str()) {
                                            Some(path.as_str())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            if duck_bounds.is_empty() {
                                continue;
                            }
                            for (pi, param) in fdef.params.iter().enumerate() {
                                if !matches!(&param.ty, IrType::Generic(name) if name == &g.name) {
                                    continue;
                                }
                                let Some(arg) = args.get(pi) else { continue };
                                let IrType::Named { path, .. } = &arg.ty else {
                                    continue;
                                };
                                for dname in &duck_bounds {
                                    if seen.insert((path.clone(), dname.to_string())) {
                                        result.push((path.clone(), dname.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    result
}

#[allow(clippy::too_many_arguments)]
fn check_call_site(
    fdef: &FnDef,
    args: &[Expr],
    ducks: &HashMap<&str, &DuckDef>,
    types: &HashMap<&str, TypeInfo>,
    checked: &mut HashSet<(String, String)>,
    errors: &mut Vec<String>,
) {
    // 找带 duck bound 的泛型参数（bounds 引用已定义的 duck 名）
    for (gi, g) in fdef.generics.iter().enumerate() {
        let duck_bounds: Vec<&IrType> = g
            .bounds
            .iter()
            .filter(|b| matches!(b, IrType::Named { path, .. } if ducks.contains_key(path.as_str())))
            .collect();
        if duck_bounds.is_empty() {
            continue;
        }
        // 找到使用该泛型参数的位置（参数类型为 Generic(g.name)）
        for (pi, param) in fdef.params.iter().enumerate() {
            if !matches!(&param.ty, IrType::Generic(name) if name == &g.name) {
                continue;
            }
            let Some(arg) = args.get(pi) else { continue };
            // 实参类型必须是具体类型（Named path 且在 types 索引中）
            let IrType::Named { path, args: type_args } = &arg.ty else {
                continue;
            };
            let Some(type_info) = types.get(path.as_str()) else { continue };
            // 该具体类型已经检查过该 duck → 跳过（避免重复报错）
            if !checked.insert((path.clone(), g.name.clone())) {
                continue;
            }
            for bound in &duck_bounds {
                if let IrType::Named { path: dname, .. } = bound {
                    let duck = ducks[dname.as_str()];
                    verify_type_satisfies_duck(
                        path,
                        type_args,
                        type_info,
                        duck,
                        &g.name,
                        gi,
                        errors,
                    );
                }
            }
        }
    }
}

/// 验证具体类型结构满足 duck 约束
fn verify_type_satisfies_duck(
    type_name: &str,
    type_args: &[IrType],
    type_info: &TypeInfo,
    duck: &DuckDef,
    generic_name: &str,
    generic_idx: usize,
    errors: &mut Vec<String>,
) {
    // duck 泛型参数 → 具体类型实参的映射（用于替换 duck 方法签名中的泛型引用）
    let mut subst: HashMap<String, IrType> = HashMap::new();
    for (i, g) in duck.generics.iter().enumerate() {
        if let Some(a) = type_args.get(i) {
            subst.insert(g.name.clone(), a.clone());
        } else if i == generic_idx {
            subst.insert(g.name.clone(), IrType::named(type_name));
        }
    }
    // 单泛型 duck（无尖括号声明）→ 泛型参数即被检查的类型本身
    if duck.generics.is_empty() {
        subst.insert(generic_name.to_string(), IrType::named(type_name));
    }

    let prefix = format!(
        "error[E0600]: type `{type_name}` does not satisfy duck constraint `{}`",
        duck.name
    );

    // ── 方法约束 ──
    for m in &duck.methods {
        // 多泛型关系 duck：方法带类型前缀（owner），只检查属于被检查类型的约束
        if let Some(owner) = &m.owner {
            // owner 必须是 duck 泛型参数；若 owner 对应具体类型实参，则要求该实参等于当前类型
            let owner_arg = duck
                .generics
                .iter()
                .position(|g| &g.name == owner)
                .and_then(|i| type_args.get(i));
            if let Some(oa) = owner_arg {
                if !matches!(oa, IrType::Named { path, .. } if path == type_name) {
                    continue; // 该约束属于其它类型
                }
            }
        }
        let Some((c_params, c_ret, _c_mut)) = type_info.methods.get(&m.name) else {
            errors.push(format!("{prefix}: missing method `{}`", m.name));
            continue;
        };
        // 参数数量匹配（duck 非 self 参数数 == 具体方法非 self 参数数，
        // 或满足 param_range 数量约束: range(L,R)/exact(N)/min(N)/max(N)）
        let duck_params: Vec<&IrType> = m
            .params
            .iter()
            .filter(|p| p.name != "self")
            .map(|p| &p.ty)
            .collect();
        let param_ok = match m.param_range {
            Some((lo, hi)) => {
                // 约束描述的是「位置参数总数」：显式参数数 + [lo, hi]
                let min_expected = duck_params.len() + lo;
                let max_expected = if hi == usize::MAX {
                    usize::MAX
                } else {
                    duck_params.len() + hi
                };
                c_params.len() >= min_expected && c_params.len() <= max_expected
            }
            None => c_params.len() == duck_params.len(),
        };
        if !param_ok {
            let expected = match m.param_range {
                Some((lo, hi)) if hi == usize::MAX => {
                    format!("at least {}", duck_params.len() + lo)
                }
                Some((lo, hi)) if lo == hi => {
                    format!("exactly {}", duck_params.len() + lo)
                }
                Some((lo, hi)) => format!(
                    "{} to {}",
                    duck_params.len() + lo,
                    duck_params.len() + hi
                ),
                None => duck_params.len().to_string(),
            };
            errors.push(format!(
                "{prefix}: method `{}` expects {} positional parameter(s), found {}",
                m.name,
                expected,
                c_params.len()
            ));
            continue;
        }
        // 返回类型匹配（duck 泛型引用替换后比较）
        let d_ret = substitute(&m.ret_ty, &subst);
        if d_ret != *c_ret {
            errors.push(format!(
                "{prefix}: method `{}` must return `{}`, found `{}`",
                m.name, d_ret, c_ret
            ));
        }
    }

    // ── 字段约束 ──
    for f in &duck.fields {
        if let Some(owner) = &f.owner {
            let owner_arg = duck
                .generics
                .iter()
                .position(|g| &g.name == owner)
                .and_then(|i| type_args.get(i));
            if let Some(oa) = owner_arg {
                if !matches!(oa, IrType::Named { path, .. } if path == type_name) {
                    continue;
                }
            }
        }
        let Some(c_ty) = type_info.fields.get(&f.name) else {
            errors.push(format!("{prefix}: missing field `{}`", f.name));
            continue;
        };
        let d_ty = substitute(&f.ty, &subst);
        if d_ty != *c_ty {
            errors.push(format!(
                "{prefix}: field `{}` must have type `{}`, found `{}`",
                f.name, d_ty, c_ty
            ));
        }
    }
}

/// 将类型中的 duck 泛型引用替换为具体类型实参
fn substitute(ty: &IrType, subst: &HashMap<String, IrType>) -> IrType {
    match ty {
        IrType::Named { path, args } => {
            if let Some(repl) = subst.get(path.as_str()) {
                repl.clone()
            } else if args.is_empty() {
                IrType::Named {
                    path: path.clone(),
                    args: vec![],
                }
            } else {
                IrType::Named {
                    path: path.clone(),
                    args: args.iter().map(|a| substitute(a, subst)).collect(),
                }
            }
        }
        IrType::Generic(name) => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
        IrType::Option(inner) => IrType::Option(Box::new(substitute(inner, subst))),
        IrType::Tuple(items) => IrType::Tuple(items.iter().map(|i| substitute(i, subst)).collect()),
        IrType::Ref(inner) => IrType::Ref(Box::new(substitute(inner, subst))),
        IrType::MutRef(inner) => IrType::MutRef(Box::new(substitute(inner, subst))),
        IrType::Result { ok, err } => IrType::Result {
            ok: Box::new(substitute(ok, subst)),
            err: Box::new(substitute(err, subst)),
        },
        IrType::Fn { params, ret } => IrType::Fn {
            params: params.iter().map(|p| substitute(p, subst)).collect(),
            ret: Box::new(substitute(ret, subst)),
        },
        other => other.clone(),
    }
}

// ── 表达式 / 语句递归遍历（用于定位调用点） ──

fn walk_block(block: &Block, f: &mut dyn FnMut(&Expr)) {
    for stmt in &block.stmts {
        walk_stmt(stmt, f);
    }
}

fn walk_stmt(stmt: &Stmt, f: &mut dyn FnMut(&Expr)) {
    match stmt {
        Stmt::Let { value, .. } => walk_expr(value, f),
        Stmt::Assign { target, value } => {
            walk_expr(target, f);
            walk_expr(value, f);
        }
        Stmt::Return { value } => {
            if let Some(v) = value {
                walk_expr(v, f);
            }
        }
        Stmt::ExprStmt { expr } => walk_expr(expr, f),
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            walk_expr(cond, f);
            walk_block(then_branch, f);
            if let Some(b) = else_branch {
                walk_block(b, f);
            }
        }
        Stmt::For { iter, guard, body, .. } => {
            walk_expr(iter, f);
            if let Some(g) = guard {
                walk_expr(g, f);
            }
            walk_block(body, f);
        }
        Stmt::While { cond, guard, body } => {
            walk_expr(cond, f);
            if let Some(g) = guard {
                walk_expr(g, f);
            }
            walk_block(body, f);
        }
        Stmt::WhileLet { expr, guard, body, .. } => {
            walk_expr(expr, f);
            if let Some(g) = guard {
                walk_expr(g, f);
            }
            walk_block(body, f);
        }
        Stmt::Match { scrutinee, arms } => {
            walk_expr(scrutinee, f);
            for arm in arms {
                walk_block(&arm.body, f);
            }
        }
        Stmt::Raise { value } => walk_expr(value, f),
        Stmt::Assert { cond, message } => {
            walk_expr(cond, f);
            if let Some(m) = message {
                walk_expr(m, f);
            }
        }
        Stmt::Yield { value } => walk_expr(value, f),
        Stmt::YieldFrom { iter } => walk_expr(iter, f),
        Stmt::BreakLabel { value, .. } => {
            if let Some(v) = value {
                walk_expr(v, f);
            }
        }
        Stmt::BlockLabel { body, .. } => walk_block(body, f),
        Stmt::Defer { body } => walk_block(body, f),
        Stmt::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            walk_block(body, f);
            for (_, b) in catches {
                walk_block(b, f);
            }
            if let Some(b) = else_body {
                walk_block(b, f);
            }
            if let Some(b) = finally_body {
                walk_block(b, f);
            }
        }
        Stmt::Block { stmts } => {
            for s in stmts {
                walk_stmt(s, f);
            }
        }
        Stmt::CheckerBlock { body, .. } => walk_block(body, f),
        Stmt::Pass | Stmt::Break | Stmt::Continue | Stmt::TypeAlias { .. } => {}
    }
}

fn walk_expr(expr: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(expr);
    match &expr.kind {
        ExprKind::Call { callee, args, .. } => {
            walk_expr(callee, f);
            for a in args {
                walk_expr(a, f);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            walk_expr(receiver, f);
            for a in args {
                walk_expr(a, f);
            }
        }
        ExprKind::FieldAccess { base, .. } => walk_expr(base, f),
        ExprKind::IndexGet { base, key } => {
            walk_expr(base, f);
            walk_expr(key, f);
        }
        ExprKind::IndexSet { base, key, value } => {
            walk_expr(base, f);
            walk_expr(key, f);
            walk_expr(value, f);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            walk_expr(lhs, f);
            walk_expr(rhs, f);
        }
        ExprKind::UnOp { operand, .. } => walk_expr(operand, f),
        ExprKind::IfExpr { cond, then, els } => {
            walk_expr(cond, f);
            walk_expr(then, f);
            walk_expr(els, f);
        }
        ExprKind::Lambda { body, .. } => walk_expr(body, f),
        ExprKind::StructCtor { fields, .. } => {
            for (_, e) in fields {
                walk_expr(e, f);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                walk_expr(a, f);
            }
        }
        ExprKind::GenExpr { yield_of } => walk_expr(yield_of, f),
        ExprKind::Cast { expr: inner, .. } => walk_expr(inner, f),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                walk_expr(a, f);
            }
        }
        ExprKind::BlockExpr { block } => walk_block(block, f),
        ExprKind::TupleLit(items) | ExprKind::Tuple(items) | ExprKind::ListLit(items)
        | ExprKind::List(items) => {
            for e in items {
                walk_expr(e, f);
            }
        }
        ExprKind::Dict(items) => {
            for (k, v) in items {
                walk_expr(k, f);
                walk_expr(v, f);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                walk_expr(s, f);
            }
            walk_expr(end, f);
        }
        ExprKind::Pipe { receiver, args, .. } => {
            walk_expr(receiver, f);
            for a in args {
                walk_expr(a, f);
            }
        }
        ExprKind::Paren(inner) => walk_expr(inner, f),
        ExprKind::ImplicitConvert { source, .. } => walk_expr(source, f),
        ExprKind::Lit(_) | ExprKind::Var(_) => {}
    }
}

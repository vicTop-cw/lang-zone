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
    /// 具体类型的泛型参数名（如 Box2 的 ["T"]，用于关联类型绑定推断）
    generics: Vec<String>,
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
                ti.generics = s.generics.iter().map(|g| g.name.clone()).collect();
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
                ti.generics = e.generics.iter().map(|g| g.name.clone()).collect();
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

/// 收集需要自动生成 Rust impl 的 (具体类型名, duck 名, 调用点 duck 泛型绑定) 三元组。
/// 仅当具体类型在调用点被用作 duck 约束泛型实参时才需要 impl。
/// 第三个元素是调用点推断出的 duck 泛型参数 → 具体类型的绑定（如 Mapper<T,R> 中 T→Celsius）。
pub fn collect_duck_impls(ir: &IrModule) -> Vec<(String, String, HashMap<String, IrType>)> {
    let mut result: Vec<(String, String, HashMap<String, IrType>)> = Vec::new();
    let mut seen: HashSet<(String, String, String)> = HashSet::new();

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
    // 具体类型泛型参数索引（type_name → 泛型参数名列表），用于构建带泛型实参的绑定
    let mut struct_generics: HashMap<&str, Vec<String>> = HashMap::new();
    for item in &ir.items {
        match item {
            Item::FnDef(f) => {
                fn_defs.insert(f.name.as_str(), f);
                bodies.push(&f.body);
            }
            Item::StructDef(s) => {
                struct_generics.insert(
                    s.name.as_str(),
                    s.generics.iter().map(|g| g.name.clone()).collect(),
                );
                for m in &s.methods {
                    fn_defs.insert(m.name.as_str(), m);
                    bodies.push(&m.body);
                }
            }
            Item::EnumDef(e) => {
                struct_generics.insert(
                    e.name.as_str(),
                    e.generics.iter().map(|g| g.name.clone()).collect(),
                );
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
                        // 参数类型直接是 duck 名（pet: Pet）：生成 impl Duck for 具体类型
                        for (pi, param) in fdef.params.iter().enumerate() {
                            let IrType::Named { path: dname, .. } = &param.ty else {
                                continue;
                            };
                            if !ducks.contains_key(dname.as_str()) {
                                continue;
                            }
                            let Some(arg) = args.get(pi) else { continue };
                            let IrType::Named { path: type_name, .. } = &arg.ty else {
                                continue;
                            };
                            let type_name = type_name.clone();
                            let key = (type_name.clone(), dname.clone(), format!("{:?}", arg.ty));
                            if !seen.insert(key) {
                                continue;
                            }
                            result.push((type_name, dname.clone(), HashMap::new()));
                        }
                        for g in &fdef.generics {
                            let duck_bounds: Vec<(&str, &[IrType])> = g
                                .bounds
                                .iter()
                                .filter_map(|b| {
                                    if let IrType::Named { path, args } = b {
                                        if ducks.contains_key(path.as_str()) {
                                            Some((path.as_str(), args.as_slice()))
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
                                let type_name = path.clone();
                                for (dname, dargs) in &duck_bounds {
                                    // 构建调用点 duck 泛型绑定：bound 实参（如 Mapper<T,R> 的 T/R）
                                    // 中引用函数泛型的位置 → 调用点实参类型；直接引用具体类型 → 本身
                                    let mut bindings: HashMap<String, IrType> = HashMap::new();
                                    for (i, ba) in dargs.iter().enumerate() {
                                        let Some(dg) = ducks[dname].generics.get(i) else {
                                            continue;
                                        };
                                        let dg_name = dg.name.clone();
                                        let is_self = matches!(ba, IrType::Generic(n) if n == &g.name)
                                            || matches!(ba, IrType::Named { path, .. } if path == &g.name);
                                        if is_self {
                                            // 被检查的函数泛型（如 T）→ 调用点实参具体类型
                                            // 泛型具体类型（Box2<T>）需带泛型实参，供 trait 泛型实参使用
                                            let self_ty = if let Some(gs) =
                                                struct_generics.get(type_name.as_str())
                                            {
                                                if gs.is_empty() {
                                                    IrType::named(&type_name)
                                                } else {
                                                    IrType::Named {
                                                        path: type_name.clone(),
                                                        args: gs
                                                            .iter()
                                                            .map(|n| IrType::Generic(n.clone()))
                                                            .collect(),
                                                    }
                                                }
                                            } else {
                                                IrType::named(&type_name)
                                            };
                                            bindings.insert(dg_name, self_ty);
                                        } else if let IrType::Named { path: bp, args: bargs } = ba {
                                            // bound 实参引用的是另一个函数泛型参数 → 从调用点实参推断
                                            if fdef.generics.iter().any(|gp| &gp.name == bp) {
                                                if let Some(pi2) = fdef.params.iter().position(|p| {
                                                    matches!(&p.ty, IrType::Generic(n) if n == bp)
                                                }) {
                                                    if let Some(at) = args.get(pi2) {
                                                        if let IrType::Named { path: atp, .. } = &at.ty {
                                                            bindings.insert(dg_name, IrType::named(atp));
                                                        }
                                                    }
                                                }
                                            } else {
                                                // 直接引用具体类型（如 str / i64）
                                                bindings.insert(
                                                    dg_name,
                                                    IrType::Named {
                                                        path: bp.clone(),
                                                        args: bargs.clone(),
                                                    },
                                                );
                                            }
                                        }
                                    }
                                    let key = format!(
                                        "{}::{}::{:?}",
                                        type_name, dname, bindings
                                    );
                                    if seen.insert((type_name.clone(), dname.to_string(), key)) {
                                        result.push((
                                            type_name.clone(),
                                            dname.to_string(),
                                            bindings,
                                        ));
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
    // 调用点实参类型（用于通过 duck bound 推断其他泛型参数的实际类型）
    let arg_tys: Vec<IrType> = args.iter().map(|a| a.ty.clone()).collect();
    // 找带 duck bound 的泛型参数（bounds 引用已定义的 duck 名）
    for (_gi, g) in fdef.generics.iter().enumerate() {
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
                        bound,
                        &fdef.generics,
                        &fdef.params,
                        &arg_tys,
                        &g.name,
                        ducks,
                        types,
                        0,
                        errors,
                    );
                }
            }
        }
    }
    // 参数类型直接是 duck 名（pet: Pet）的调用点：验证实参满足 duck
    // （与泛型 bound 形式不同，此处 duck 名直接出现在参数类型注解中）
    for (pi, param) in fdef.params.iter().enumerate() {
        let IrType::Named { path: dname, .. } = &param.ty else {
            continue;
        };
        if !ducks.contains_key(dname.as_str()) {
            continue;
        }
        let Some(arg) = args.get(pi) else { continue };
        let IrType::Named { path, args: type_args } = &arg.ty else {
            continue;
        };
        let Some(type_info) = types.get(path.as_str()) else { continue };
        if !checked.insert((path.clone(), dname.clone())) {
            continue;
        }
        let duck = ducks[dname.as_str()];
        let bound = param.ty.clone();
        verify_type_satisfies_duck(
            path,
            type_args,
            type_info,
            duck,
            &bound,
            &fdef.generics,
            &fdef.params,
            &arg_tys,
            dname,
            ducks,
            types,
            0,
            errors,
        );
    }
}

/// 验证具体类型结构满足 duck 约束
/// `ducks`/`types` 用于递归验证嵌套约束（§2.4 `where T: Iterable`）；
/// `depth` 防止递归过深（循环嵌套 duck 约束）。
#[allow(clippy::too_many_arguments)]
fn verify_type_satisfies_duck(
    type_name: &str,
    type_args: &[IrType],
    type_info: &TypeInfo,
    duck: &DuckDef,
    bound: &IrType,
    fdef_generics: &[GenericParam],
    fdef_params: &[Param],
    arg_tys: &[IrType],
    generic_name: &str,
    ducks: &HashMap<&str, &DuckDef>,
    types: &HashMap<&str, TypeInfo>,
    depth: usize,
    errors: &mut Vec<String>,
) {
    // ── 构建 duck 泛型参数 → 具体类型实参 的映射 subst ──
    // 依据：调用点 where 约束 `Mapper<X, Y>` 中 X/Y 与实际实参类型的绑定关系。
    // 单泛型 duck（无尖括号声明）→ 泛型参数即被检查的类型本身。
    let mut subst: HashMap<String, IrType> = HashMap::new();
    if duck.generics.is_empty() {
        subst.insert(generic_name.to_string(), IrType::named(type_name));
    } else if let IrType::Named { args: bound_args, .. } = bound {
        for (i, ba) in bound_args.iter().enumerate() {
            let Some(dg) = duck.generics.get(i) else { continue };
            let dname = dg.name.clone();
            let is_self = matches!(ba, IrType::Generic(n) if n == generic_name)
                || matches!(ba, IrType::Named { path, .. } if path == generic_name);
            if is_self {
                // 该 duck 泛型参数即被检查的类型本身
                subst.insert(
                    dname,
                    IrType::Named {
                        path: type_name.to_string(),
                        args: type_args.to_vec(),
                    },
                );
            } else if let IrType::Named { path, args } = ba {
                // bound 实参引用的是函数泛型参数 → 从调用点实参推断其实际类型
                if fdef_generics.iter().any(|gp| &gp.name == path) {
                    if let Some(pi) = fdef_params.iter().position(|p| {
                        matches!(&p.ty, IrType::Generic(n) if n == path)
                    }) {
                        if let Some(at) = arg_tys.get(pi) {
                            subst.insert(dname, at.clone());
                        }
                    }
                } else {
                    // bound 实参是具体类型（如 str / i64 / List<int>）
                    subst.insert(
                        dname,
                        IrType::Named {
                            path: path.clone(),
                            args: args.clone(),
                        },
                    );
                }
            }
        }
    }

    let prefix = format!(
        "error[E0600]: type `{type_name}` does not satisfy duck constraint `{}`",
        duck.name
    );

    // 判断方法/字段签名中引用的 duck 泛型是否都已确定（未确定则保守跳过，避免误报）
    let is_bound = |ty: &IrType| ty_fully_bound(ty, &duck.generics, &subst);

    // ── 方法约束 ──
    for m in &duck.methods {
        // 多泛型关系 duck：方法带类型前缀（owner），只检查属于被检查类型的约束
        if let Some(owner) = &m.owner {
            // owner 是 duck 泛型参数；若其 subst 绑定到当前类型，则属于本类型
            let belongs = match subst.get(owner) {
                Some(IrType::Named { path, .. }) => path == type_name,
                _ => false,
            };
            if !belongs {
                continue;
            }
        }
        // 正则模式方法名（§8.4）：name_pattern 非空时，要求至少有一个方法名匹配该模式
        if let Some(pat) = &m.name_pattern {
            let matched = type_info
                .methods
                .keys()
                .any(|name| regex_like_match(pat, name));
            if !matched {
                errors.push(format!(
                    "{prefix}: no method matches pattern `{}`",
                    pat
                ));
            }
            continue; // 正则约束只检查存在性，签名逐项检查留给普通方法
        }
        let Some((c_params, c_ret, _c_mut)) = type_info.methods.get(&m.name) else {
            // default 修饰（§11.4③）：该成员可选，目标类型可不实现
            if m.is_default {
                continue;
            }
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
        // 返回类型匹配（duck 泛型引用替换后比较；未确定的 duck 泛型 → 保守跳过）
        if !is_bound(&m.ret_ty) {
            continue;
        }
        // 关联类型引用（I.Item）解析为具体类型的关联绑定（§2.3）：
        // 结构类型系统中，关联类型值由「具体类型同名方法的返回类型」推断
        let resolved = resolve_assoc_refs(&m.ret_ty, duck, &subst, type_info, c_ret);
        let d_ret = substitute(&resolved, &subst);
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
            let belongs = match subst.get(owner) {
                Some(IrType::Named { path, .. }) => path == type_name,
                _ => false,
            };
            if !belongs {
                continue;
            }
        }
        // 字段关系约束（§2.2）：A.id == B.id → 本字段类型必须等于 rel 字段类型
        if let Some((rel_owner, rel_name)) = &f.rel {
            let Some(c_ty) = type_info.fields.get(&f.name) else {
                errors.push(format!("{prefix}: missing field `{}`", f.name));
                continue;
            };
            // rel_owner 在 subst 中绑定的具体类型（如 B → Celsius/Fahrenheit）
            let Some(IrType::Named { path: rel_path, .. }) = subst.get(rel_owner) else {
                continue; // 关系方未绑定具体类型 → 保守跳过
            };
            let Some(rel_info) = types.get(rel_path.as_str()) else { continue };
            let Some(rel_ty) = rel_info.fields.get(rel_name) else {
                errors.push(format!(
                    "{prefix}: related type `{rel_path}` missing field `{rel_name}`"
                ));
                continue;
            };
            if c_ty != rel_ty {
                errors.push(format!(
                    "{prefix}: field `{}.{}` must have the same type as `{}.{}` (`{:?}`), found `{:?}`",
                    type_name, f.name, rel_path, rel_name, rel_ty, c_ty
                ));
            }
            continue;
        }
        let Some(c_ty) = type_info.fields.get(&f.name) else {
            errors.push(format!("{prefix}: missing field `{}`", f.name));
            continue;
        };
        if !is_bound(&f.ty) {
            continue;
        }
        let d_ty = substitute(&f.ty, &subst);
        if d_ty != *c_ty {
            errors.push(format!(
                "{prefix}: field `{}` must have type `{}`, found `{}`",
                f.name, d_ty, c_ty
            ));
        }
    }

    // ── 嵌套约束递归验证（§2.4 `duck D<T> where T: Iterable`）──
    // duck 泛型参数 bounds 中引用的已定义 duck → 递归验证绑定类型也满足该 duck
    if depth < 8 {
        for g in &duck.generics {
            for b in &g.bounds {
                let IrType::Named {
                    path: bname,
                    args: bargs,
                } = b
                else {
                    continue;
                };
                let Some(nested_duck) = ducks.get(bname.as_str()) else {
                    continue; // 内建约束（Clone/Display/Iterable）由 Rust 侧保证
                };
                // 该泛型参数在 subst 中的绑定（应为具体类型 Named）
                let Some(IrType::Named {
                    path: bpath,
                    args: btype_args,
                }) = subst.get(&g.name)
                else {
                    continue;
                };
                // 自环保护：绑定类型就是当前类型且嵌套 duck 相同 → 跳过
                if bpath == type_name && *bname == duck.name {
                    continue;
                }
                let Some(nested_info) = types.get(bpath.as_str()) else {
                    continue;
                };
                // 把 bound 实参中的 duck 泛型引用替换为具体类型
                let mut nested_bound_args: Vec<IrType> =
                    bargs.iter().map(|a| substitute(a, &subst)).collect();
                // `where T: Iterable`（无实参）→ 默认实参 = 被检查类型绑定
                if nested_bound_args.is_empty() && !nested_duck.generics.is_empty() {
                    nested_bound_args.push(IrType::named(bpath));
                }
                let nested_bound = IrType::Named {
                    path: bname.clone(),
                    args: nested_bound_args,
                };
                verify_type_satisfies_duck(
                    bpath,
                    btype_args,
                    nested_info,
                    nested_duck,
                    &nested_bound,
                    &duck.generics,
                    &[],
                    &[],
                    &g.name,
                    ducks,
                    types,
                    depth + 1,
                    errors,
                );
            }
        }
    }

    // ── satisfies 约束行（§11.4①）：目标类型必须同时满足另一 duck ──
    for sname in &duck.satisfies {
        let Some(nested_duck) = ducks.get(sname.as_str()) else {
            continue;
        };
        // 自环保护
        if sname == &duck.name {
            continue;
        }
        let Some(nested_info) = types.get(type_name) else { continue };
        if depth >= 8 {
            continue;
        }
        let nested_bound = IrType::Named {
            path: sname.clone(),
            args: Vec::new(),
        };
        verify_type_satisfies_duck(
            type_name,
            type_args,
            nested_info,
            nested_duck,
            &nested_bound,
            fdef_generics,
            fdef_params,
            arg_tys,
            generic_name,
            ducks,
            types,
            depth + 1,
            errors,
        );
    }

    // ── sealed 闭合约束（§11.4②）：目标类型不得有额外成员 ──
    if duck.sealed {
        let declared_fields: usize = duck.fields.len();
        let declared_methods: usize = duck
            .methods
            .iter()
            .filter(|m| m.owner.is_none() || m.name_pattern.is_some())
            .count();
        let actual_fields = type_info.fields.len();
        let actual_methods = type_info.methods.len();
        if actual_fields > declared_fields || actual_methods > declared_methods {
            errors.push(format!(
                "{prefix}: duck is sealed, type has extra members \
                 ({} fields/{} methods declared, found {} fields/{} methods)",
                declared_fields, declared_methods, actual_fields, actual_methods
            ));
        }
    }

    // ── 正则方法匹配约束（§8.4）：match /pattern/ at_least(N) 等 ──
    for rule in &duck.match_rules {
        let matched = type_info
            .methods
            .keys()
            .filter(|name| regex_like_match(&rule.pattern, name))
            .count();
        let (lo, hi) = rule.range;
        let ok = matched >= lo && matched <= hi;
        if !ok {
            let expect = if hi == usize::MAX {
                format!("at least {}", lo)
            } else if lo == hi {
                format!("exactly {}", lo)
            } else {
                format!("{} to {}", lo, hi)
            };
            errors.push(format!(
                "{prefix}: pattern `{}` matches {} method(s), expected {}",
                rule.pattern, matched, expect
            ));
        }
    }

    // ── 命名参数约束（§8.2.2）：require(...) / optional(...) ──
    for req in &duck.param_reqs {
        // 检查目标类型是否有名为该参数的方法签名（有则参数名可用）
        // 简单实现：require 的每个命名参数，目标类型同名方法应能接受（不报错即通过）
        // 这里仅对缺失同名方法的情况提示（与 sealed 配合更强，此处保守）
        for n in &req.names {
            if req.is_required && !type_info.methods.contains_key(n) {
                // 命名参数约束的语义是「调用时必须提供命名参数」，不要求独立方法；
                // 保守跳过（方法级约束已在上面逐项检查）
                let _ = n;
            }
        }
    }
}

/// 将 duck 签名中的关联类型引用（`I.Item`，§2.3）解析为具体类型的关联绑定。
/// 结构类型系统中，关联类型值由「具体类型同名方法的返回类型」推断；
/// 顶层 `I.Item` 直接替换为 c_ret，嵌套引用递归处理。
fn resolve_assoc_refs(
    ty: &IrType,
    duck: &DuckDef,
    subst: &HashMap<String, IrType>,
    type_info: &TypeInfo,
    c_ret: &IrType,
) -> IrType {
    match ty {
        IrType::Named { path, args } => {
            // `I.Item` 形式：path 含点号且前缀是 duck 泛型参数
            if let Some((owner, _member)) = path.split_once('.') {
                if duck.generics.iter().any(|g| g.name == owner) {
                    // 该 duck 泛型绑定到当前具体类型 → 关联绑定 = 具体方法返回类型
                    if let Some(IrType::Named { path: tpath, .. }) = subst.get(owner) {
                        if type_info.generics.is_empty() && !tpath.is_empty() {
                            return c_ret.clone();
                        }
                        if !type_info.generics.is_empty() {
                            return c_ret.clone();
                        }
                        return IrType::Any;
                    }
                }
            }
            if args.is_empty() {
                ty.clone()
            } else {
                IrType::Named {
                    path: path.clone(),
                    args: args
                        .iter()
                        .map(|a| resolve_assoc_refs(a, duck, subst, type_info, c_ret))
                        .collect(),
                }
            }
        }
        IrType::Option(inner) => IrType::Option(Box::new(resolve_assoc_refs(
            inner, duck, subst, type_info, c_ret,
        ))),
        IrType::Tuple(items) => IrType::Tuple(
            items
                .iter()
                .map(|i| resolve_assoc_refs(i, duck, subst, type_info, c_ret))
                .collect(),
        ),
        IrType::Ref(inner) => IrType::Ref(Box::new(resolve_assoc_refs(
            inner, duck, subst, type_info, c_ret,
        ))),
        IrType::MutRef(inner) => IrType::MutRef(Box::new(resolve_assoc_refs(
            inner, duck, subst, type_info, c_ret,
        ))),
        IrType::Result { ok, err } => IrType::Result {
            ok: Box::new(resolve_assoc_refs(ok, duck, subst, type_info, c_ret)),
            err: Box::new(resolve_assoc_refs(err, duck, subst, type_info, c_ret)),
        },
        other => other.clone(),
    }
}

/// 简单正则风格模式匹配（§8.4）：支持 `\w`/`\d`/`(a|b)`/`+`/`*`/`^`/`$` 子集。
/// 够用的子集：`\w`=字母数字下划线、`\d`=数字、`(x|y)` 分组、`+`/`*` 量词、
/// `_` 通配。不依赖 regex crate（编译期错误提示友好）。
fn regex_like_match(pattern: &str, name: &str) -> bool {
    // 转成字节流做回溯匹配
    let pat: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = name.chars().collect();
    match_regex_at(&pat, &s, 0, 0, None)
}

#[allow(clippy::too_many_arguments)]
fn match_regex_at(
    pat: &[char],
    s: &[char],
    mut pi: usize,
    mut si: usize,
    // 记录已展开的备选分支起点（防无限递归）
    mut _alt_stack: Option<usize>,
) -> bool {
    while pi < pat.len() {
        let c = pat[pi];
        match c {
            '^' => {
                if si != 0 {
                    return false;
                }
                pi += 1;
            }
            '$' => {
                if si != s.len() {
                    return false;
                }
                pi += 1;
            }
            '(' => {
                // 寻找匹配的右括号，取 | 分隔的备选
                let mut depth = 1;
                let mut j = pi + 1;
                let mut alts: Vec<usize> = Vec::new();
                alts.push(j);
                while j < pat.len() && depth > 0 {
                    match pat[j] {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        '|' if depth == 1 => alts.push(j + 1),
                        _ => {}
                    }
                    if depth == 0 {
                        break;
                    }
                    j += 1;
                }
                if j >= pat.len() {
                    return false;
                }
                let end = j;
                // 每个备选：递归匹配 [start, end) 内容 + 后续
                for k in 0..alts.len() {
                    let start = alts[k];
                    let stop = if k + 1 < alts.len() { alts[k + 1] - 1 } else { end };
                    let mut rest_ok = false;
                    // 尝试备选内容匹配任意长度后，继续后续模式
                    for split in si..=s.len() {
                        if match_regex_at(&pat[start..stop], &s[si..split], 0, 0, None)
                            && match_regex_at(&pat[end + 1..], s, 0, split, None)
                        {
                            rest_ok = true;
                            break;
                        }
                    }
                    if rest_ok {
                        return true;
                    }
                }
                return false;
            }
            '\\' => {
                // \w \d \s 或转义字面
                if pi + 1 >= pat.len() {
                    return false;
                }
                let ec = pat[pi + 1];
                if si >= s.len() {
                    return false;
                }
                let ok = match ec {
                    'w' => s[si].is_alphanumeric() || s[si] == '_',
                    'd' => s[si].is_ascii_digit(),
                    's' => s[si].is_whitespace(),
                    other => s[si] == other,
                };
                if !ok {
                    return false;
                }
                pi += 2;
                si += 1;
            }
            '?' => {
                // 前一个字符可选（简单处理：0 或 1 个）
                if pi == 0 {
                    return false;
                }
                let prev = pat[pi - 1];
                // 尝试 0 次
                if match_regex_at(&pat[pi + 1..], s, 0, si, None) {
                    return true;
                }
                // 尝试 1 次
                if si < s.len() && char_matches(prev, s[si]) {
                    return match_regex_at(&pat[pi + 1..], s, 0, si + 1, None);
                }
                return false;
            }
            '+' | '*' => {
                if pi == 0 {
                    return false;
                }
                let prev = pat[pi - 1];
                let min = if c == '+' { 1 } else { 0 };
                // 贪心：匹配尽量多，然后回溯
                let mut count = 0;
                let mut sp = si;
                while sp < s.len() && char_matches(prev, s[sp]) {
                    count += 1;
                    sp += 1;
                }
                for take in (min..=count).rev() {
                    if match_regex_at(&pat[pi + 1..], s, 0, si + take, None) {
                        return true;
                    }
                }
                return false;
            }
            '.' => {
                if si >= s.len() {
                    return false;
                }
                pi += 1;
                si += 1;
            }
            _ => {
                if si >= s.len() || s[si] != c {
                    return false;
                }
                pi += 1;
                si += 1;
            }
        }
    }
    si == s.len()
}

fn char_matches(pat: char, c: char) -> bool {
    match pat {
        '\\' => false, // 转义由上层处理，这里不应出现
        _ => pat == c,
    }
}

/// 判断类型中引用的 duck 泛型是否全部已在 subst 中确定绑定
/// （未确定的 duck 泛型引用 → 返回 false，调用方保守跳过，避免误报）
fn ty_fully_bound(ty: &IrType, duck_generics: &[GenericParam], subst: &HashMap<String, IrType>) -> bool {
    match ty {
        IrType::Named { path, args } => {
            if duck_generics.iter().any(|g| &g.name == path) {
                subst.contains_key(path)
            } else {
                args.iter().all(|a| ty_fully_bound(a, duck_generics, subst))
            }
        }
        IrType::Generic(n) => {
            !duck_generics.iter().any(|g| &g.name == n) || subst.contains_key(n)
        }
        IrType::Option(inner) | IrType::Ref(inner) | IrType::MutRef(inner) => {
            ty_fully_bound(inner, duck_generics, subst)
        }
        IrType::Tuple(items) => items
            .iter()
            .all(|i| ty_fully_bound(i, duck_generics, subst)),
        IrType::Result { ok, err } => {
            ty_fully_bound(ok, duck_generics, subst) && ty_fully_bound(err, duck_generics, subst)
        }
        IrType::Fn { params, ret } => {
            params
                .iter()
                .all(|p| ty_fully_bound(p, duck_generics, subst))
                && ty_fully_bound(ret, duck_generics, subst)
        }
        _ => true,
    }
}

/// 通过 duck 方法签名与具体类型方法签名的 unify，反推 duck 泛型参数 → 具体类型表达式的绑定。
/// 返回 duck 泛型参数名 → IrType（具体类型名 / 具体类型自身的泛型参数引用）的映射。
/// 适用于多泛型关系 duck（Mapper<T,R>）与泛型具体类型（Wrapper<T>）。
/// `initial` 是调用点已推断出的 duck 泛型参数绑定（collect_duck_impls 提供）；
/// 缺失的绑定通过方法签名 unify 反推补全。
pub fn infer_duck_bindings(
    duck: &DuckDef,
    sdef: &StructDef,
    initial: &HashMap<String, IrType>,
) -> Option<HashMap<String, IrType>> {
    let mut subst: HashMap<String, IrType> = initial.clone();
    let concrete_generics: Vec<String> = sdef.generics.iter().map(|g| g.name.clone()).collect();
    // 具体类型自身表达式：TypeName<T1, T2>
    let self_ir = if concrete_generics.is_empty() {
        IrType::named(&sdef.name)
    } else {
        IrType::Named {
            path: sdef.name.clone(),
            args: concrete_generics
                .iter()
                .map(|n| IrType::Generic(n.clone()))
                .collect(),
        }
    };
    let is_duck_generic = |name: &str| duck.generics.iter().any(|g| g.name == name);

    // ── 1. 被检查的 duck 泛型参数 → 绑定为具体类型自身 ──
    //    （调用点绑定中已包含该映射；未包含时由 unify 反推）
    for g in &duck.generics {
        if !subst.contains_key(&g.name) {
            continue;
        }
        if let Some(IrType::Named { path, .. }) = subst.get(&g.name) {
            if path == &sdef.name {
                // 调用点绑定已是具体类型自身（无泛型参数），无需处理
            }
        }
    }
    let _ = self_ir;

    // ── 2. 方法签名 unify：duck 签名 vs 具体类型同名方法签名 ──
    for m in &duck.methods {
        let Some(c) = sdef.methods.iter().find(|c| c.name == m.name) else {
            continue;
        };
        // 返回类型对齐
        unify_sig_type(&m.ret_ty, &c.ret_ty, &is_duck_generic, &mut subst);
        // 非 self 参数对齐
        let d_params: Vec<&Param> = m.params.iter().filter(|p| p.name != "self").collect();
        let c_params: Vec<&Param> = c.params.iter().filter(|p| p.name != "self").collect();
        for (dp, cp) in d_params.iter().zip(c_params.iter()) {
            unify_sig_type(&dp.ty, &cp.ty, &is_duck_generic, &mut subst);
        }
    }

    // ── 3. 全部 duck 泛型参数都已确定才算成功 ──
    for g in &duck.generics {
        if !subst.contains_key(&g.name) {
            return None;
        }
    }
    Some(subst)
}

/// 递归 unify：duck 签名中的泛型引用 → 具体类型签名中的类型
fn unify_sig_type(
    d: &IrType,
    c: &IrType,
    is_duck_generic: &dyn Fn(&str) -> bool,
    subst: &mut HashMap<String, IrType>,
) {
    match d {
        IrType::Named { path, args } if is_duck_generic(path) => {
            // duck 泛型引用：整个替换为具体类型对应位置的类型
            subst.entry(path.clone()).or_insert_with(|| c.clone());
        }
        IrType::Named { path, args } => {
            // 结构化递归（如 List<R> vs List<T>）
            if let IrType::Named {
                path: cpath,
                args: cargs,
            } = c
            {
                if cpath == path && args.len() == cargs.len() {
                    for (a, b) in args.iter().zip(cargs.iter()) {
                        unify_sig_type(a, b, is_duck_generic, subst);
                    }
                }
            }
        }
        IrType::Generic(name) if is_duck_generic(name) => {
            subst.entry(name.clone()).or_insert_with(|| c.clone());
        }
        IrType::Option(di) => {
            if let IrType::Option(ci) = c {
                unify_sig_type(di, ci, is_duck_generic, subst);
            }
        }
        IrType::Result { ok, err } => {
            if let IrType::Result {
                ok: cok,
                err: cerr,
            } = c
            {
                unify_sig_type(ok, cok, is_duck_generic, subst);
                unify_sig_type(err, cerr, is_duck_generic, subst);
            }
        }
        IrType::Tuple(ds) => {
            if let IrType::Tuple(cs) = c {
                if ds.len() == cs.len() {
                    for (a, b) in ds.iter().zip(cs.iter()) {
                        unify_sig_type(a, b, is_duck_generic, subst);
                    }
                }
            }
        }
        _ => {}
    }
}

/// 将类型中的 duck 泛型引用替换为具体类型实参
/// （Named path 与 Generic name 均匹配 subst 键；codegen 自动 impl 生成亦复用）
pub fn substitute(ty: &IrType, subst: &HashMap<String, IrType>) -> IrType {
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
        Stmt::While {
            cond,
            guard,
            body,
            else_body: _,
        } => {
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
        ExprKind::Spread(inner) => walk_expr(inner, f),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                walk_expr(c, f);
            }
            for stmt in &block.stmts {
                walk_stmt(stmt, f);
            }
        }
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
        ExprKind::AssignExpr { target, value } => {
            walk_expr(target, f);
            walk_expr(value, f);
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

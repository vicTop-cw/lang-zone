// Lang-Zone 编译器 — ir/builder/extended_check.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

/// 扩展语义检查（ERROR_BUG 补漏 20 例）。
/// 在 build_ir_inner 步骤 11（统一错误出口前）对 AST 做深度类型/模式检查。
/// 目标：closure_param_type_mismatch / hash_key_type_mismatch / method_not_found_put /
/// return_type_mismatch2 / dict_key_type_mismatch / enum_field_type_mismatch /
/// field_not_found / method_call_arity / match_branch_type_mismatch /
/// non_exhaustive_match / duplicate_match_pattern / match_subject_type_mismatch /
/// use_result_without_unwrap / variant_type_mismatch / unconstrained_generic_cmp /
/// method_arg_type / str_plus_int / tree_insert_type_mismatch / index_type / mixed_list_type
pub(crate) fn check_extended_semantics(module: &ast::Module, base: &TypeCtx) {
    let mut enum_variants_all: HashMap<String, Vec<String>> = HashMap::new();
    for s in &module.structs {
        if s.is_enum {
            enum_variants_all.insert(
                s.name.clone(),
                s.fields.iter().map(|f| f.name.clone()).collect(),
            );
        }
    }
    // 重载签名收集（同名 def 多个签名，含泛型参数）
    let mut over_fns: HashMap<String, Vec<(Vec<IrType>, IrType)>> = HashMap::new();
    for f in &module.functions {
        let tys: Vec<IrType> = f
            .params
            .iter()
            .map(|pp| from_ast_type_with_generics(&pp.ty, &f.generics))
            .collect();
        let ret = f
            .return_type
            .as_ref()
            .map(|t| from_ast_type_with_generics(t, &f.generics))
            .unwrap_or(IrType::Any);
        over_fns.entry(f.name.clone()).or_default().push((tys, ret));
    }
    let mut duck_names: HashSet<String> = HashSet::new();
    for d in &module.duck_defs {
        duck_names.insert(d.name.clone());
    }
    let w = ExWalker {
        enum_variants_all: &enum_variants_all,
        over_fns: &over_fns,
        duck_names: &duck_names,
    };
    // 宏模块（#!bin macro）：macro/template 定义体是代码生成模板，
    // quote 内的字符串拼接（如 "x is " + x）属模板操作，常规类型检查不适用
    if module.is_macro {
        return;
    }
    for f in &module.functions {
        if f.is_comptime || f.is_abstract {
            continue;
        }
        ex_check_fn(f, base, &w);
    }
}

/// 扩展语义检查所需的只读元数据
pub(crate) struct ExWalker<'m> {
    /// 模块内枚举 → 变体名列表（match 穷尽检查用）
    enum_variants_all: &'m HashMap<String, Vec<String>>,
    /// 重载函数签名（name → [(param_tys, ret_ty)]，同名 def 可能有多个）
    over_fns: &'m HashMap<String, Vec<(Vec<IrType>, IrType)>>,
    /// 模块级 duck 约束名（形参标注为命名 duck → 运行时结构匹配，编译期放行）
    duck_names: &'m HashSet<String>,
}

/// 推断表达式类型：先同步局部变量表再复用现有 infer_expr_type（只读 ctx）
pub(crate) fn ex_infer(e: &AstExpr, work: &TypeCtx, env: &HashMap<String, IrType>) -> IrType {
    let mut tmp = work.clone();
    tmp.vars = env.clone();
    infer_expr_type(e, &tmp)
}

/// 字符串单字符索引表达式（非切片）？
/// codegen 对 `s[i]` 按上下文多态生成：字符串上下文 → String，数值上下文 →
/// char 码点 i64（src/ir/codegen/mod.rs `s[i]` 分支，注释点名 comptime
/// simple_hash 场景；FIND_BUG.md G3 已固化 `"你好世界"[1]`→22909）。
/// 因此绑定为该类表达式的变量类型「无法定论」，在 ex 检查环境登记为 Any，
/// 避免对 `let c = s[i]; h = h * 31 + c` 误报 str_plus_int
/// （DEMO/09_macros/comptime_external_lib.lz，ir_demo_snapshots 回归）。
pub(crate) fn ex_is_str_char_index(
    e: &AstExpr,
    work: &TypeCtx,
    env: &HashMap<String, IrType>,
) -> bool {
    if let AstExpr::Index { receiver, index } = e {
        if matches!(index.as_ref(), AstExpr::Range { .. }) {
            return false; // 切片是真字符串（s[i..j] → Str），不多态
        }
        let rt = ex_infer(receiver, work, env);
        return matches!(rt, IrType::Str)
            || matches!(rt, IrType::Named { path, .. } if path == "str" || path == "String");
    }
    false
}

/// 无法定论（禁止据其报错）的类型
pub(crate) fn ex_type_queer(t: &IrType) -> bool {
    matches!(
        t,
        IrType::Any
            | IrType::Generic(_)
            | IrType::Never
            | IrType::Self_
            | IrType::Ext
            | IrType::Duck { .. }
            | IrType::Unit
    )
}

/// 类型严格兼容（回归友好）：Any/Generic/Unknown 一律放行；其余按结构递归比较。
pub(crate) fn ex_type_agrees(a: &IrType, b: &IrType) -> bool {
    if ex_type_queer(a) || ex_type_queer(b) {
        return true;
    }
    match (a, b) {
        (IrType::Ref(x), y) | (IrType::MutRef(x), y) => ex_type_agrees(x, y),
        (x, IrType::Ref(y)) | (x, IrType::MutRef(y)) => ex_type_agrees(x, y),
        (IrType::Option(x), y) => ex_type_agrees(&**x, y),
        (x, IrType::Option(y)) => ex_type_agrees(x, &**y),
        (IrType::Int, IrType::F64) | (IrType::F64, IrType::Int) => true,
        (IrType::Int, IrType::Int)
        | (IrType::F64, IrType::F64)
        | (IrType::Str, IrType::Str)
        | (IrType::Bool, IrType::Bool)
        | (IrType::Unit, IrType::Unit) => true,
        (IrType::Str, IrType::Named { path, .. }) | (IrType::Named { path, .. }, IrType::Str)
            if path == "String" || path == "str" =>
        {
            true
        }
        (IrType::Named { path: ap, args: aa }, IrType::Named { path: bp, args: ba }) => {
            if ap != bp {
                return false;
            }
            if aa.len() != ba.len() {
                return false;
            }
            aa.iter().zip(ba.iter()).all(|(x, y)| ex_type_agrees(x, y))
        }
        (IrType::Result { ok: o1, err: e1 }, IrType::Result { ok: o2, err: e2 }) => {
            ex_type_agrees(&**o1, &**o2) && ex_type_agrees(&**e1, &**e2)
        }
        (
            IrType::Fn {
                params: p1,
                ret: r1,
            },
            IrType::Fn {
                params: p2,
                ret: r2,
            },
        ) => {
            p1.len() == p2.len()
                && p1.iter().zip(p2.iter()).all(|(x, y)| ex_type_agrees(x, y))
                && ex_type_agrees(&**r1, &**r2)
        }
        (IrType::Tuple(t1), IrType::Tuple(t2)) => {
            t1.len() == t2.len() && t1.iter().zip(t2.iter()).all(|(x, y)| ex_type_agrees(x, y))
        }
        _ => false,
    }
}

/// 类型中是否含自由泛型形参（T / List<T> 等，未实例化前禁止据此报类型错误）
/// 是否为已注册的具体命名类型：结构体/枚举名或内置基元/容器名
pub(crate) fn ex_concrete_named(path: &str, struct_names: &HashSet<String>) -> bool {
    if struct_names.contains(path) {
        return true;
    }
    matches!(
        path,
        "int"
            | "i64"
            | "i32"
            | "u64"
            | "usize"
            | "bool"
            | "str"
            | "String"
            | "f64"
            | "f32"
            | "char"
            | "Never"
            | "Unit"
            | "Any"
            | "Void"
            | "Option"
            | "Result"
            | "List"
            | "Set"
            | "Dict"
            | "Vec"
            | "HashMap"
            | "Iter"
            | "Iterator"
            | "Range"
            | "Fn"
            | "fn"
            | "Self"
            | "Ext"
    )
}

pub(crate) fn ex_type_has_generic(t: &IrType, gs: &[String]) -> bool {
    match t {
        IrType::Generic(g) => gs.iter().any(|x| x == g),
        IrType::Named { path, args } => {
            gs.iter().any(|g| path == g) || args.iter().any(|a| ex_type_has_generic(a, gs))
        }
        IrType::Option(x) => ex_type_has_generic(x, gs),
        IrType::Result { ok, err } => ex_type_has_generic(ok, gs) || ex_type_has_generic(err, gs),
        IrType::Tuple(ts) => ts.iter().any(|a| ex_type_has_generic(a, gs)),
        IrType::Fn { params, ret } => {
            params.iter().any(|a| ex_type_has_generic(a, gs)) || ex_type_has_generic(ret, gs)
        }
        IrType::Ref(x) | IrType::MutRef(x) => ex_type_has_generic(x, gs),
        _ => false,
    }
}

/// 类型中是否出现任何自由泛型占位（Generic）——泛型形参无法断言类型错误
pub(crate) fn ex_has_any_gen(t: &IrType) -> bool {
    match t {
        IrType::Generic(_) => true,
        IrType::Named { path: _, args } => args.iter().any(ex_has_any_gen),
        IrType::Option(x) => ex_has_any_gen(x),
        IrType::Result { ok, err } => ex_has_any_gen(ok) || ex_has_any_gen(err),
        IrType::Tuple(ts) => ts.iter().any(ex_has_any_gen),
        IrType::Fn { params, ret } => params.iter().any(ex_has_any_gen) || ex_has_any_gen(ret),
        IrType::Ref(x) | IrType::MutRef(x) => ex_has_any_gen(x),
        _ => false,
    }
}

/// 兼容比较（回归友好增强版）：自由泛型 / 鸭子类型 / 命名 duck 约束 /
/// 关联类型 / 泛型实参未实例化 / Range↔Iterator 一律放行。
/// duck: 模块级 duck 约束名集合（此类标注由运行时结构匹配 check_duck_satisfaction 保证）。
pub(crate) fn ex_agrees_g(a: &IrType, b: &IrType, gs: &[String], duck: &HashSet<String>) -> bool {
    if matches!(a, IrType::Duck { .. }) || matches!(b, IrType::Duck { .. }) {
        return true;
    }
    // 单字母大写基名的命名类型视作泛型形参（I.Item → Named{I,args:[Item]}；T/E 自由形参）
    if let IrType::Named { path, .. } = a {
        if path.len() == 1 && path.as_bytes()[0].is_ascii_uppercase() {
            return true;
        }
    }
    if let IrType::Named { path, .. } = b {
        if path.len() == 1 && path.as_bytes()[0].is_ascii_uppercase() {
            return true;
        }
    }
    if ex_has_any_gen(a)
        || ex_has_any_gen(b)
        || ex_type_has_generic(a, gs)
        || ex_type_has_generic(b, gs)
    {
        return true;
    }
    if let (IrType::Named { path: ap, args: aa }, IrType::Named { path: bp, args: ba }) = (a, b) {
        if duck.contains(ap.as_str()) || duck.contains(bp.as_str()) {
            return true;
        }
        if ap.contains('.') || bp.contains('.') {
            return true;
        }
        if (ap == "Iterator" && matches!(bp.as_str(), "List" | "Vec" | "Range" | "Set" | "HashSet"))
            || (bp == "Iterator"
                && matches!(ap.as_str(), "List" | "Vec" | "Range" | "Set" | "HashSet"))
        {
            return true;
        }
        // trait 对象参数（如 `def collect(iter: Iterator)`）接受任意具名类型：
        // RangeIter/MapIter 等自定义 struct 鸭子类型满足 trait 方法（next）。
        // 基础类型（int/str 等）不含 next，仍由后续 rustc 阶段拒绝。
        if (ap == "Iterator" || bp == "Iterator") && (ap != "Iterator" || bp != "Iterator") {
            return true;
        }
        if ap == bp && (aa.is_empty() || ba.is_empty()) {
            return true;
        }
    }
    // 未实例化容器名（裸 Option/Result，泛型实参未列出）与实例化容器互相放行
    if (matches!(a, IrType::Named { path, args }
            if (path == "Option" || path == "Result") && args.is_empty())
        && (matches!(b, IrType::Option(_)) || matches!(b, IrType::Result { .. })))
        || (matches!(b, IrType::Named { path, args }
                if (path == "Option" || path == "Result") && args.is_empty())
            && (matches!(a, IrType::Option(_)) || matches!(a, IrType::Result { .. })))
    {
        return true;
    }
    ex_type_agrees(a, b)
}

/// 模式的结构化键（match 重复模式判定）：区分嵌套子模式与位置
pub(crate) fn ex_pat_key(pat: &AstPattern) -> String {
    match pat {
        AstPattern::Int(v) => format!("I{v}"),
        AstPattern::Str(x) => format!("S{x}"),
        AstPattern::Bool(b) => format!("B{b}"),
        AstPattern::Ident(n) => {
            if n == "_" {
                "_".into()
            } else {
                "$".into()
            }
        }
        AstPattern::RefMutIdent(n) => format!("mut{n}"),
        AstPattern::Wildcard => "_".into(),
        AstPattern::Variant(vn, subpats) => format!(
            "{}({})",
            ex_base_of(vn),
            subpats.iter().map(ex_pat_key).collect::<Vec<_>>().join(",")
        ),
        AstPattern::Tuple(ps) => format!("tuple({})", ps.len()),
        AstPattern::List(ps) => format!("list({})", ps.len()),
        AstPattern::Dict(ps) => format!("dict({})", ps.len()),
        _ => "?".into(),
    }
}

/// 变体名去路径前缀（Color.Red → Red）
pub(crate) fn ex_base_of(name: &str) -> String {
    match name.rfind('.') {
        Some(i) => name[i + 1..].to_string(),
        None => name.to_string(),
    }
}

/// 用具体实参替换自由泛型形参（自定义枚举字段类型里的 Named 单字母大写形如 T / Box<T>）
pub(crate) fn ex_subst(ty: &IrType, params: &[String], args: &[IrType]) -> IrType {
    match ty {
        IrType::Named { path, args: a } => {
            if a.is_empty() && params.iter().any(|g| g == path) && path.len() == 1 {
                let idx = params.iter().position(|g| g == path).unwrap();
                args.get(idx).cloned().unwrap_or_else(|| ty.clone())
            } else {
                IrType::Named {
                    path: path.clone(),
                    args: a.iter().map(|t| ex_subst(t, params, args)).collect(),
                }
            }
        }
        IrType::Option(x) => IrType::Option(Box::new(ex_subst(x, params, args))),
        IrType::Result { ok, err } => IrType::Result {
            ok: Box::new(ex_subst(ok, params, args)),
            err: Box::new(ex_subst(err, params, args)),
        },
        IrType::Tuple(ts) => IrType::Tuple(ts.iter().map(|t| ex_subst(t, params, args)).collect()),
        IrType::Fn { params: ps, ret } => IrType::Fn {
            params: ps.iter().map(|t| ex_subst(t, params, args)).collect(),
            ret: Box::new(ex_subst(ret, params, args)),
        },
        IrType::Ref(x) => IrType::Ref(Box::new(ex_subst(x, params, args))),
        IrType::MutRef(x) => IrType::MutRef(Box::new(ex_subst(x, params, args))),
        IrType::Duck { fields } => IrType::Duck {
            fields: fields
                .iter()
                .map(|(n, t)| (n.clone(), ex_subst(t, params, args)))
                .collect(),
        },
        other => other.clone(),
    }
}

/// 模式绑定：将 pattern 绑定的变量按 scrutinee 类型登记进 env
pub(crate) fn ex_bind_pattern(
    pat: &AstPattern,
    scrut_ty: &IrType,
    env: &mut HashMap<String, IrType>,
    work: &TypeCtx,
) {
    match pat {
        AstPattern::Ident(n) => {
            if n != "_" {
                env.insert(n.clone(), scrut_ty.clone());
            }
        }
        AstPattern::Complex(_, _) => {}
        AstPattern::RefMutIdent(n) => {
            env.insert(n.clone(), scrut_ty.clone());
        }
        AstPattern::Wildcard => {}
        AstPattern::Int128(_) => {}
        AstPattern::BigInt(_) => {}
        AstPattern::Variant(vname, subpats) => {
            let base = ex_base_of(vname);
            // 变体载荷解析优先限定名（Enum.Variant），其次 scrutinee 枚举名消歧，
            // 最后裸名兑底：跨枚举同名变体（IrType.Tuple vs Pattern.Tuple）若按
            // 裸名查表会拿错载荷类型，导致扩展语义检查误报（.lzlz 自举库实测）。
            let vtys: Vec<IrType> = if vname.contains('.') {
                work.enum_variant_field_types
                    .get(vname.as_str())
                    .cloned()
                    .unwrap_or_else(|| {
                        work.enum_variant_field_types
                            .get(&base)
                            .cloned()
                            .unwrap_or_default()
                    })
            } else {
                let scrut_enum = match scrut_ty {
                    IrType::Named { path, .. } => Some(path.clone()),
                    _ => None,
                };
                scrut_enum
                    .and_then(|en| {
                        work.enum_variant_field_types
                            .get(&format!("{}.{}", en, base))
                            .cloned()
                    })
                    .unwrap_or_else(|| {
                        work.enum_variant_field_types
                            .get(&base)
                            .cloned()
                            .unwrap_or_default()
                    })
            };
            if vtys.is_empty() {
                match (base.as_str(), scrut_ty) {
                    ("Some", IrType::Option(inner)) => {
                        for sp in subpats {
                            ex_bind_pattern(sp, &**inner, env, work);
                        }
                    }
                    ("Ok", IrType::Result { ok, .. }) => {
                        for sp in subpats {
                            ex_bind_pattern(sp, &**ok, env, work);
                        }
                    }
                    ("Err", IrType::Result { err, .. }) => {
                        for sp in subpats {
                            ex_bind_pattern(sp, &**err, env, work);
                        }
                    }
                    _ => {
                        for sp in subpats {
                            ex_bind_pattern(sp, &IrType::Any, env, work);
                        }
                    }
                }
            } else {
                // 自定义枚举：用 scrutinee 实例化替换变体字段中的自由泛型形参
                let inst: Vec<IrType> = match scrut_ty {
                    IrType::Named { path, args: a } => work
                        .enum_generics
                        .get(path.as_str())
                        .map(|gg| vtys.iter().map(|t| ex_subst(t, gg, a)).collect())
                        .unwrap_or_else(|| vtys.clone()),
                    // 自定义 enum Option<T>/Result<T,E> 以容器形态出现：按 [T] / [T,E] 实例化
                    IrType::Option(inner) => vtys
                        .iter()
                        .map(|t| ex_subst(t, &["T".to_string()], &[(**inner).clone()]))
                        .collect(),
                    IrType::Result { ok, err } => vtys
                        .iter()
                        .map(|t| {
                            ex_subst(
                                t,
                                &["T".to_string(), "E".to_string()],
                                &[(**ok).clone(), (**err).clone()],
                            )
                        })
                        .collect(),
                    _ => vtys.clone(),
                };
                for (i, sp) in subpats.iter().enumerate() {
                    let sty = inst.get(i).cloned().unwrap_or(IrType::Any);
                    ex_bind_pattern(sp, &sty, env, work);
                }
            }
        }
        AstPattern::Tuple(ps) => {
            if let IrType::Tuple(items) = scrut_ty {
                for (i, sp) in ps.iter().enumerate() {
                    let sty = items.get(i).cloned().unwrap_or(IrType::Any);
                    ex_bind_pattern(sp, &sty, env, work);
                }
            } else {
                for sp in ps {
                    ex_bind_pattern(sp, &IrType::Any, env, work);
                }
            }
        }
        AstPattern::List(ps) => {
            let sty = match scrut_ty {
                IrType::Named { path, args } if path == "List" || path == "Vec" => {
                    args.first().cloned().unwrap_or(IrType::Any)
                }
                _ => IrType::Any,
            };
            for sp in ps {
                match sp {
                    AstPattern::Rest(_) => {}
                    _ => ex_bind_pattern(sp, &sty, env, work),
                }
            }
        }
        AstPattern::Dict(kvs) => {
            let vty = match scrut_ty {
                IrType::Named { path, args } if path == "Dict" || path == "HashMap" => {
                    args.get(1).cloned().unwrap_or(IrType::Any)
                }
                _ => IrType::Any,
            };
            for (_, sp) in kvs {
                ex_bind_pattern(sp, &vty, env, work);
            }
        }
        AstPattern::Int(_)
        | AstPattern::Str(_)
        | AstPattern::Bool(_)
        | AstPattern::Range { .. }
        | AstPattern::Rest(_) => {}
    }
}

/// 检查单个函数体
pub(crate) fn ex_check_fn(f: &ast::Function, base: &TypeCtx, w: &ExWalker) {
    // #[embed(...)] 函数体为原生代码段字符串字面量，跳过扩展语义检查
    //（embed 由 semantic_check 与 embed 诊断块专门处理）
    if f.decorators.iter().any(|d| d.name == "embed") {
        return;
    }
    let mut work = base.clone();
    let gens = f.generics.clone();
    let ret_ty = f
        .return_type
        .as_ref()
        .map(|t| from_ast_type_with_generics(t, &gens));
    let mut hinted: HashSet<String> = HashSet::new();
    for b in &f.where_clause {
        hinted.insert(b.type_param.clone());
    }
    for (p, _) in &f.generic_defaults {
        hinted.insert(p.clone());
    }
    let mut env: HashMap<String, IrType> = HashMap::new();
    for p in &f.params {
        let ty = from_ast_type_with_generics(&p.ty, &gens);
        env.insert(p.name.clone(), ty);
    }
    work.current_generics = gens.clone();
    work.current_ret_ty = ret_ty.clone();
    work.current_fn_name = Some(f.name.clone());
    work.current_is_iterator = f.is_iterator;
    work.vars = env.clone();
    ex_check_stmts(
        &f.body,
        &mut work,
        &mut env,
        &gens,
        ret_ty.as_ref(),
        &hinted,
        w,
    );
    // 函数体最后一个表达式语句的返回类型检查（return_type_mismatch2）
    if let (Some(rty), Some(AstStmt::Expr(last))) = (ret_ty.as_ref(), f.body.last()) {
        let ety = ex_infer(last, &work, &env);
        // try/catch 有 catch 分支时：try body 返回 Result<T,E>（raises），
        // 但 catch 已处理错误，整体等价于 T。解包 Result 与期望类型比较。
        // 仅当函数声明返回类型本身不是 Result 时才解包：此时 try 体尾是 raises 调用
        // 返回 Result<T,E>，catch 已处理错误，整体等价于 T。
        // 若函数本身就返回 Result（如 `parse_int` 显式 Ok/Err 构造），则不解包，
        // 直接按 Result<int,Any> vs Result<int,str> 比较（Any 属 queer 放行），避免误报。
        let effective_ty = match last {
            AstExpr::TryCatch { catches, .. }
                if !catches.is_empty() && !matches!(rty, IrType::Result { .. }) =>
            {
                if let IrType::Result { ok, .. } = &ety {
                    (**ok).clone()
                } else {
                    ety.clone()
                }
            }
            _ => ety.clone(),
        };
        if !matches!(&effective_ty, IrType::Unit)
            && !ex_type_queer(rty)
            && !ex_agrees_g(rty, &effective_ty, &gens, w.duck_names)
        {
            work.report_error(format!(
                "函数 {} 返回类型不匹配：期望 {}，实际 {}",
                f.name,
                ex_ty_desc(rty),
                ex_ty_desc(&ety)
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn ex_check_stmts(
    stmts: &[AstStmt],
    work: &mut TypeCtx,
    env: &mut HashMap<String, IrType>,
    gens: &[String],
    ret: Option<&IrType>,
    hinted: &HashSet<String>,
    w: &ExWalker,
) {
    for st in stmts {
        match st {
            AstStmt::Expr(e) => {
                ex_check_expr(e, work, env, gens, ret, hinted, w);
            }
            AstStmt::Let { name, value, .. } => {
                ex_check_expr(value, work, env, gens, ret, hinted, w);
                // 绑定自字符串单字符索引的变量：codegen 按上下文多态生成
                // （String 或 i64 码点），类型无法定论 → 登记 Any 防误报
                let vt = if ex_is_str_char_index(value, work, env) {
                    IrType::Any
                } else {
                    ex_infer(value, work, env)
                };
                env.insert(name.clone(), vt);
            }
            AstStmt::Const { name, value, .. } => {
                ex_check_expr(value, work, env, gens, ret, hinted, w);
                let vt = if ex_is_str_char_index(value, work, env) {
                    IrType::Any
                } else {
                    ex_infer(value, work, env)
                };
                env.insert(name.clone(), vt);
            }
            AstStmt::LetTuple { names, value, .. } => {
                ex_check_expr(value, work, env, gens, ret, hinted, w);
                let vt = ex_infer(value, work, env);
                if let IrType::Tuple(items) = &vt {
                    for (i, n) in names.iter().enumerate() {
                        let ty = items.get(i).cloned().unwrap_or(IrType::Any);
                        env.insert(n.clone(), ty);
                    }
                } else {
                    for n in names {
                        env.insert(n.clone(), IrType::Any);
                    }
                }
            }
            AstStmt::Return(Some(e)) => {
                ex_check_expr(e, work, env, gens, ret, hinted, w);
                if let Some(rty) = ret {
                    let ety = ex_infer(e, work, env);
                    if !ex_type_queer(rty) && !ex_agrees_g(rty, &ety, gens, w.duck_names) {
                        // __from__ 返回值转换放行（06d §十四，P1-2 第三环）：
                        // 返回类型是用户 struct 且定义了 __from__、实际返回类型
                        // 可作其源 → 不报错，转换由 builder 在 return 注入
                        let from_ok = match rty {
                            IrType::Named { path, .. } => work
                                .struct_methods
                                .get(path)
                                .map(|ms| ms.contains("__from__"))
                                .unwrap_or(false),
                            _ => false,
                        };
                        if !from_ok {
                            work.report_error(format!(
                                "函数返回值类型不匹配：期望 {}，实际返回 {}",
                                ex_ty_desc(rty),
                                ex_ty_desc(&ety)
                            ));
                            break;
                        }
                    }
                }
            }
            AstStmt::Return(None)
            | AstStmt::Yield(_)
            | AstStmt::YieldFrom(_)
            | AstStmt::Break(_)
            | AstStmt::BreakLabel { .. }
            | AstStmt::Continue => {}
            AstStmt::Raise(e) => {
                ex_check_expr(e, work, env, gens, ret, hinted, w);
            }
            AstStmt::While {
                cond,
                guard,
                body,
                else_body,
                ..
            } => {
                ex_check_expr(cond, work, env, gens, ret, hinted, w);
                if let Some(g) = guard {
                    ex_check_expr(g, work, env, gens, ret, hinted, w);
                }
                let mut sub = env.clone();
                ex_check_stmts(body, work, &mut sub, gens, ret, hinted, w);
                if let Some(el) = else_body {
                    let mut s2 = env.clone();
                    ex_check_stmts(el, work, &mut s2, gens, ret, hinted, w);
                }
            }
            AstStmt::WhileLet {
                pattern,
                expr,
                body,
                else_body,
                ..
            } => {
                ex_check_expr(expr, work, env, gens, ret, hinted, w);
                let st = ex_infer(expr, work, env);
                let mut sub = env.clone();
                ex_bind_pattern(pattern, &st, &mut sub, work);
                ex_check_stmts(body, work, &mut sub, gens, ret, hinted, w);
                if let Some(el) = else_body {
                    let mut s2 = env.clone();
                    ex_check_stmts(el, work, &mut s2, gens, ret, hinted, w);
                }
            }
            AstStmt::For {
                var,
                iter,
                guard,
                body,
                else_body,
                ..
            } => {
                let it = ex_infer(iter, work, env);
                let elem_t = match &it {
                    IrType::Named { path, args } if path == "List" || path == "Vec" => {
                        args.first().cloned().unwrap_or(IrType::Any)
                    }
                    IrType::Named { path, .. } if path == "Range" => IrType::Int,
                    IrType::Option(x) => (**x).clone(),
                    IrType::Named { path, args } if path == "Set" || path == "HashSet" => {
                        args.first().cloned().unwrap_or(IrType::Any)
                    }
                    _ => IrType::Any,
                };
                let mut sub = env.clone();
                sub.insert(var.clone(), elem_t);
                if let Some(g) = guard {
                    ex_check_expr(g, work, &mut sub, gens, ret, hinted, w);
                }
                ex_check_stmts(body, work, &mut sub, gens, ret, hinted, w);
                if let Some(el) = else_body {
                    let mut s2 = env.clone();
                    ex_check_stmts(el, work, &mut s2, gens, ret, hinted, w);
                }
            }
            AstStmt::Loop(body)
            | AstStmt::Block { body, .. }
            | AstStmt::CheckerBlock { body, .. }
            | AstStmt::Defer(body)
            | AstStmt::Comptime { body } => {
                let mut sub = env.clone();
                ex_check_stmts(body, work, &mut sub, gens, ret, hinted, w);
            }
            AstStmt::Guard {
                cond,
                let_binding,
                success_expr,
                else_body,
                ..
            } => {
                if let Some(c) = cond {
                    ex_check_expr(c, work, env, gens, ret, hinted, w);
                }
                match let_binding {
                    Some((pat, e)) => {
                        let st = ex_infer(e, work, env);
                        let mut sub = env.clone();
                        ex_bind_pattern(pat, &st, &mut sub, work);
                        ex_check_stmts(else_body, work, &mut sub, gens, ret, hinted, w);
                    }
                    None => {
                        let mut s2 = env.clone();
                        ex_check_stmts(else_body, work, &mut s2, gens, ret, hinted, w);
                    }
                }
                if let Some(se) = success_expr {
                    ex_check_expr(se, work, env, gens, ret, hinted, w);
                }
            }
            AstStmt::With {
                expr, alias, body, ..
            } => {
                ex_check_expr(expr, work, env, gens, ret, hinted, w);
                let st = ex_infer(expr, work, env);
                let mut sub = env.clone();
                if let Some(a) = alias {
                    sub.insert(a.clone(), st);
                }
                ex_check_stmts(body, work, &mut sub, gens, ret, hinted, w);
            }
            AstStmt::BlockCall { args, .. } => {
                ex_check_expr(args, work, env, gens, ret, hinted, w);
            }
            AstStmt::EnumDef(_) | AstStmt::Pass | AstStmt::TypeAlias { .. } => {}
            AstStmt::FnDef { func } => {
                ex_check_fn(func, work, w);
            }
            AstStmt::Assign { target, value, .. } => {
                ex_check_expr(target, work, env, gens, ret, hinted, w);
                ex_check_expr(value, work, env, gens, ret, hinted, w);
                if let AstExpr::Ident(n) = target {
                    let vt = if ex_is_str_char_index(value, work, env) {
                        IrType::Any
                    } else {
                        ex_infer(value, work, env)
                    };
                    env.insert(n.clone(), vt);
                }
            }
            AstStmt::Test { body, .. } => {
                let mut sub = env.clone();
                ex_check_stmts(body, work, &mut sub, gens, ret, hinted, w);
            }
            AstStmt::Assert { expr, .. } | AstStmt::Check { expr, .. } => {
                ex_check_expr(expr, work, env, gens, ret, hinted, w);
            }
            AstStmt::Suite {
                setup,
                teardown,
                tests,
                ..
            } => {
                for blk in [setup.as_ref(), teardown.as_ref(), Some(tests)] {
                    if let Some(b) = blk {
                        let mut sub = env.clone();
                        ex_check_stmts(b, work, &mut sub, gens, ret, hinted, w);
                    }
                }
            }
            AstStmt::EmbedBlock { .. } => {}
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::collapsible_if)]
pub(crate) fn ex_check_expr(
    e: &AstExpr,
    work: &mut TypeCtx,
    env: &mut HashMap<String, IrType>,
    gens: &[String],
    ret: Option<&IrType>,
    hinted: &HashSet<String>,
    w: &ExWalker,
) {
    match e {
        AstExpr::Spread(inner) => ex_check_expr(inner, work, env, gens, ret, hinted, w),
        AstExpr::DefaultExpr => {}      /* default 无需参数检查 */
        AstExpr::Int128Lit(_) => {}     /* i128 字面量无需检查 */
        AstExpr::BigIntLit(_) => {}     /* bigint 字面量无需检查 */
        AstExpr::ComplexLit(_, _) => {} /* complex 字面量无需检查 */
        AstExpr::Call { func, args, .. } => {
            if let AstExpr::Ident(fname) = func.as_ref() {
                let plain = !fname.contains('.');
                // (a) 裸枚举变体构造参数检查（enum_field_type_mismatch）
                if plain && work.enum_variants.contains_key(fname.as_str()) {
                    let vtys: Vec<IrType> = work
                        .enum_variant_field_types
                        .get(fname.as_str())
                        .cloned()
                        .unwrap_or_default();
                    for (i, arg) in args.iter().enumerate() {
                        if let Some(want) = vtys.get(i) {
                            if ex_type_queer(want) {
                                continue;
                            }
                            let at = ex_infer(arg, work, env);
                            if !ex_agrees_g(want, &at, gens, w.duck_names) {
                                work.report_error(format!(
                                    "构造 {fname} 第 {} 个参数类型不匹配：期望 {}，实际 {}",
                                    i + 1,
                                    ex_ty_desc(want),
                                    ex_ty_desc(&at)
                                ));
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                // (b) Ok/Err/Some 变体在当前返回类型约束下检查（variant_type_mismatch）
                if plain {
                    match fname.as_str() {
                        "Ok" | "Some" | "Err" => {
                            if let Some(rty) = ret {
                                let want_inner = match (fname.as_str(), rty) {
                                    ("Ok", IrType::Result { ok, .. }) => Some((**ok).clone()),
                                    ("Some", IrType::Option(x)) => Some((**x).clone()),
                                    ("Err", IrType::Result { err, .. }) => Some((**err).clone()),
                                    // 自定义枚举容器：Oh/Ok→第 0 个实参，Err→第 1 个实参
                                    ("Some", IrType::Named { path, args }) if path == "Option" => {
                                        args.first().cloned()
                                    }
                                    ("Ok", IrType::Named { path, args }) if path == "Result" => {
                                        args.first().cloned()
                                    }
                                    ("Err", IrType::Named { path, args }) if path == "Result" => {
                                        args.get(1).cloned()
                                    }
                                    _ => None,
                                };
                                if let (Some(want), Some(a0)) = (want_inner, args.first()) {
                                    if !ex_type_queer(&want) {
                                        let at = ex_infer(a0, work, env);
                                        if !ex_agrees_g(&want, &at, gens, w.duck_names) {
                                            work.report_error(format!(
                                                "变体 {fname} 期望参数类型 {}，实际 {}",
                                                ex_ty_desc(&want),
                                                ex_ty_desc(&at)
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                // (c) fn 形参逐参数检查（match_subject_type_mismatch / tree_insert_type_mismatch
                // / closure_param_type_mismatch）
                let pts: Option<Vec<IrType>> = work.fn_params.get(fname.as_str()).cloned();
                let is_overload = w
                    .over_fns
                    .get(fname.as_str())
                    .map_or(false, |v| v.len() > 1);
                if let Some(param_tys) = pts {
                    // variadic auto-collect: extra args into a collection param are fine
                    let is_collect = args.len() > param_tys.len()
                        && matches!(
                            param_tys.last(),
                            Some(IrType::Named { path, .. })
                                if path == "List" || path == "Set" || path == "Vec"
                                    || path == "Dict" || path == "HashMap"
                                    || path == "Iter" || path == "Iterator"
                        );
                    if is_overload {
                        // overloaded fn: multiple signatures, skip single-signature arg check
                    } else {
                        for (i, arg) in args.iter().enumerate() {
                            if is_collect && i + 1 == param_tys.len() {
                                break;
                            }
                            let Some(want) = param_tys.get(i) else { break };
                            if ex_type_queer(want) {
                                continue;
                            }
                            if let IrType::Fn {
                                params: fp,
                                ret: fr,
                            } = want
                            {
                                // 闭包实参 vs fn 形参检查
                                if let AstExpr::Closure {
                                    params: cps,
                                    param_tys: cpts,
                                    body: cbody,
                                    ..
                                } = arg
                                {
                                    let mut actual: Vec<IrType> = Vec::new();
                                    let mut sub_env = env.clone();
                                    for (ci, cn) in cps.iter().enumerate() {
                                        let ct = match cpts.get(ci) {
                                            Some(Some(t)) => {
                                                let ty = from_ast_type_with_generics(t, gens);
                                                sub_env.insert(cn.clone(), ty.clone());
                                                ty
                                            }
                                            _ => IrType::Any,
                                        };
                                        actual.push(ct);
                                    }
                                    let actual_ret = ex_infer(cbody, work, &sub_env);
                                    let mut mismatch = fp.len() != actual.len();
                                    if !mismatch {
                                        for (x, y) in fp.iter().zip(actual.iter()) {
                                            // 关联类型/泛型接口占位（如 I.Item、fn(ref I.Item) 中的 I.Item）
                                            // 在具体实例化未知时无法断言参数错误：解包 ref 后看基底
                                            let xb = match x {
                                                IrType::Ref(i) | IrType::MutRef(i) => &**i,
                                                o => o,
                                            };
                                            if let IrType::Named { path, args } = xb {
                                                if args.is_empty()
                                                    && !ex_concrete_named(path, &work.struct_names)
                                                {
                                                    continue;
                                                }
                                            }
                                            if !ex_agrees_g(x, y, gens, w.duck_names) {
                                                mismatch = true;
                                                break;
                                            }
                                        }
                                    }
                                    // 返回类型检查：期望 ()/Any/Never → 语句式闭包放行
                                    let ret_ok = match &**fr {
                                        IrType::Unit | IrType::Any | IrType::Never => true,
                                        IrType::Tuple(t) => t.is_empty(),
                                        _ => {
                                            ex_type_queer(&**fr)
                                                || ex_agrees_g(
                                                    &**fr,
                                                    &actual_ret,
                                                    gens,
                                                    w.duck_names,
                                                )
                                        }
                                    };
                                    if !mismatch && !ret_ok {
                                        mismatch = true;
                                    }
                                    if mismatch {
                                        work.report_error(format!(
                                            "调用 {fname} 的参数 {}：函数类型参数与闭包不匹配",
                                            i + 1
                                        ));
                                    }
                                    // 闭包体内递归检查
                                    let mut sub2 = env.clone();
                                    for (ci, cn) in cps.iter().enumerate() {
                                        let ct = match cpts.get(ci) {
                                            Some(Some(t)) => from_ast_type_with_generics(t, gens),
                                            _ => IrType::Any,
                                        };
                                        sub2.insert(cn.clone(), ct);
                                    }
                                    let saved = work.current_ret_ty.clone();
                                    work.current_ret_ty = Some(actual_ret.clone());
                                    ex_check_expr(cbody, work, &mut sub2, gens, ret, hinted, w);
                                    work.current_ret_ty = saved;
                                    continue; // 已递归，避免重复
                                }
                            } else {
                                let at = ex_infer(arg, work, env);
                                if !ex_agrees_g(want, &at, gens, w.duck_names) {
                                    // __from__ 实参转换放行（06d §十四，P1-2）：形参是
                                    // 用户 struct 且定义了 __from__、实参类型可作其源
                                    // （与 convert_expr 的包装触发点一致）→ 不报错，
                                    // 转换由 builder 在调用点注入
                                    let from_ok = match want {
                                        IrType::Named { path, .. } => work
                                            .struct_methods
                                            .get(path)
                                            .map(|ms| ms.contains("__from__"))
                                            .unwrap_or(false),
                                        _ => false,
                                    };
                                    if !from_ok {
                                        eprintln!("DBG fnarg: fn={} idx={} want={:?} at={:?} gens={:?} duck={:?}", fname, i + 1, want, at, gens, w.duck_names);
                                        let caller = work
                                            .current_fn_name
                                            .clone()
                                            .unwrap_or_else(|| "<toplevel>".to_string());
                                        work.report_error(format!(
                                        "调用 {fname} 的参数 {} 类型不匹配：期望 {}，实际 {}（位于函数 {caller}）",
                                        i + 1,
                                        ex_ty_desc(want),
                                        ex_ty_desc(&at)
                                    ));
                                        break;
                                    }
                                }
                            }
                        }
                        // 其余参数递归
                        for (i, arg) in args.iter().enumerate() {
                            let skip_fn = matches!(param_tys.get(i), Some(IrType::Fn { .. }));
                            if skip_fn {
                                continue;
                            }
                            match arg {
                                AstExpr::KwArg { value, .. } => {
                                    ex_check_expr(value, work, env, gens, ret, hinted, w);
                                }
                                _ => {
                                    ex_check_expr(arg, work, env, gens, ret, hinted, w);
                                }
                            }
                        }
                    }
                } else {
                    for arg in args {
                        match arg {
                            AstExpr::KwArg { value, .. } => {
                                ex_check_expr(value, work, env, gens, ret, hinted, w);
                            }
                            _ => {
                                ex_check_expr(arg, work, env, gens, ret, hinted, w);
                            }
                        }
                    }
                }
            } else if let AstExpr::PathAccess { receiver, segment } = func.as_ref() {
                // 静态方法调用 arity（method_call_arity：Counter.inc(c, 5)）
                if let AstExpr::Ident(tn) = receiver.as_ref() {
                    if work.struct_names.contains(tn.as_str()) {
                        if let Some(am) = work.struct_method_arity.get(tn).cloned() {
                            if let Some(&n) = am.get(segment) {
                                if args.len() > n {
                                    work.report_error(format!(
                                        "静态方法 {}.{} 最多接受 {n} 个参数，实际 {} 个",
                                        tn,
                                        segment,
                                        args.len()
                                    ));
                                }
                            }
                        }
                    }
                }
                ex_check_expr(func, work, env, gens, ret, hinted, w);
                for arg in args {
                    match arg {
                        AstExpr::KwArg { value, .. } => {
                            ex_check_expr(value, work, env, gens, ret, hinted, w);
                        }
                        _ => {
                            ex_check_expr(arg, work, env, gens, ret, hinted, w);
                        }
                    }
                }
            } else {
                ex_check_expr(func, work, env, gens, ret, hinted, w);
                for arg in args {
                    match arg {
                        AstExpr::KwArg { value, .. } => {
                            ex_check_expr(value, work, env, gens, ret, hinted, w);
                        }
                        _ => {
                            ex_check_expr(arg, work, env, gens, ret, hinted, w);
                        }
                    }
                }
            }
        }
        AstExpr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let rty0 = ex_infer(receiver, work, env);
            // 静态方法调用（receiver 为类型名）：method_call_arity（Counter.inc(c,5) 形式）
            if let AstExpr::Ident(tn) = receiver.as_ref() {
                if work.struct_names.contains(tn.as_str()) {
                    if let Some(am) = work.struct_method_arity.get(tn).cloned() {
                        if let Some(&n) = am.get(method) {
                            if args.len() > n {
                                work.report_error(format!(
                                    "静态方法 {}.{} 最多接受 {n} 个参数，实际 {} 个",
                                    tn,
                                    method,
                                    args.len()
                                ));
                            }
                        }
                    }
                }
            }
            // String.replace 参数必须是字符串（method_arg_type）
            if matches!(&rty0, IrType::Str) && method == "replace" {
                for a in args {
                    let at = ex_infer(a, work, env);
                    if !ex_type_queer(&at) && !ex_type_agrees(&IrType::Str, &at) {
                        work.report_error(format!(
                            "String.replace 参数应为 str，实际 {}",
                            ex_ty_desc(&at)
                        ));
                        break;
                    }
                }
            }
            if let IrType::Named { path, args: targs } = &rty0 {
                // 用户自定义 struct 与内置 Dict/HashMap 同名时，用户方法遮蔽内置特检
                //（lib_hashmap 实测：用户 HashMap.put 被误报为内置 Dict 缺 put）
                let is_user_struct = work.struct_names.contains(path.as_str());
                // dict 键参数白名单（hash_key_type_mismatch）
                if !is_user_struct
                    && (path == "Dict" || path == "HashMap")
                    && matches!(
                        method.as_str(),
                        "get" | "contains_key" | "contains" | "insert" | "remove" | "set"
                    )
                    && !args.is_empty()
                {
                    let kty = targs.first().cloned().unwrap_or(IrType::Any);
                    if !ex_type_queer(&kty) {
                        let at = ex_infer(&args[0], work, env);
                        if !ex_agrees_g(&kty, &at, gens, w.duck_names) {
                            work.report_error(
                                "字典键类型不匹配：访问的键类型与字典 Key 类型不一致".to_string(),
                            );
                        }
                    }
                }
                // method_not_found_put：内置 Dict/HashMap 没有 put
                if !is_user_struct && (path == "Dict" || path == "HashMap") && method == "put" {
                    work.report_error(
                        "Dict/HashMap 没有 put 方法（请使用 insert/set）".to_string(),
                    );
                }
                // 实例方法 arity（method_call_arity）
                if let Some(am) = work.struct_method_arity.get(path).cloned() {
                    if let Some(&n) = am.get(method) {
                        if args.len() > n {
                            work.report_error(format!(
                                "方法 {}.{} 最多接受 {n} 个参数，实际 {} 个",
                                path,
                                method,
                                args.len()
                            ));
                        }
                    }
                }
            }
            ex_check_expr(receiver, work, env, gens, ret, hinted, w);
            for a in args {
                match a {
                    AstExpr::KwArg { value, .. } => {
                        ex_check_expr(value, work, env, gens, ret, hinted, w);
                    }
                    _ => {
                        ex_check_expr(a, work, env, gens, ret, hinted, w);
                    }
                }
            }
        }
        AstExpr::FieldAccess { receiver, field } => {
            let rty = ex_infer(receiver, work, env);
            if let IrType::Named { path, .. } = &rty {
                // field_not_found：自定义 struct 上访问不存在的字段
                if work.struct_names.contains(path) {
                    if let Some(fields) = work.struct_fields.get(path).cloned() {
                        if !fields.contains_key(field.as_str()) {
                            work.report_error(format!("类型 {path} 没有字段 `{field}`"));
                        }
                    }
                }
            }
            ex_check_expr(receiver, work, env, gens, ret, hinted, w);
        }
        AstExpr::PathAccess { receiver, segment } => {
            // PathAccess 非法不可直接访问（codegen 有独立校验，这里只递归防止漏报嵌套）
            if let AstExpr::Ident(tn) = receiver.as_ref() {
                let _ = tn;
            }
            let _ = segment;
            ex_check_expr(receiver, work, env, gens, ret, hinted, w);
        }
        AstExpr::Index { receiver, index } => {
            let is_magic_index = matches!(receiver.as_ref(), AstExpr::FieldAccess { .. })
                && matches!(index.as_ref(), AstExpr::TupleLit(_));
            if !is_magic_index {
                let rty = ex_infer(receiver, work, env);
                match &rty {
                    IrType::Named { path, .. } if path == "List" || path == "Vec" => {
                        let ity = ex_infer(index, work, env);
                        let is_ok = matches!(&ity, IrType::Int)
                            || matches!(&ity, IrType::Named { path: p, .. } if p == "Range")
                            || ex_type_queer(&ity);
                        if !is_ok {
                            work.report_error("列表索引必须是整数或切片".to_string());
                        }
                    }
                    IrType::Named { path, args } if path == "Dict" || path == "HashMap" => {
                        let kty = args.first().cloned().unwrap_or(IrType::Any);
                        if !ex_type_queer(&kty) {
                            let ity = ex_infer(index, work, env);
                            if !ex_agrees_g(&kty, &ity, gens, w.duck_names) {
                                work.report_error("字典索引键类型不匹配".to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
            ex_check_expr(receiver, work, env, gens, ret, hinted, w);
            ex_check_expr(index, work, env, gens, ret, hinted, w);
        }
        AstExpr::Binary { left, op, right } => {
            let lt = ex_infer(left, work, env);
            let rt = ex_infer(right, work, env);
            // use_result_without_unwrap：Result 直接参与运算
            if matches!(&lt, IrType::Result { .. }) || matches!(&rt, IrType::Result { .. }) {
                work.report_error("Result 值未经解包（unwrap/?）直接参与运算".to_string());
            }
            // str_plus_int：字符串与数字直接相加（含 fstring 混拼场景）
            if matches!(op, &BinOp::Add) {
                let is_str = |t: &IrType| {
                    matches!(t, IrType::Str)
                        || matches!(t, IrType::Named { path, .. } if path == "String" || path == "str")
                };
                if is_str(&lt) && !ex_type_queer(&rt) && !is_str(&rt) {
                    work.report_error("字符串与数字不能直接相加".to_string());
                } else if is_str(&rt) && !ex_type_queer(&lt) && !is_str(&lt) {
                    work.report_error("字符串与数字不能直接相加".to_string());
                }
            }
            // unconstrained_generic_cmp：无约束泛型参数参与 < > <= >=
            if matches!(&op, &BinOp::Lt | &BinOp::Gt | &BinOp::Le | &BinOp::Ge) {
                let unbound: Vec<String> = gens
                    .iter()
                    .filter(|g| !hinted.contains(*g))
                    .cloned()
                    .collect();
                if !unbound.is_empty()
                    && (ex_type_has_any_of(&lt, &unbound) || ex_type_has_any_of(&rt, &unbound))
                    && !matches!(op, &BinOp::Le)
                {
                    work.report_error(
                        "对无约束泛型参数进行比较运算（缺少 T: Ordered 约束）".to_string(),
                    );
                }
            }
            ex_check_expr(left, work, env, gens, ret, hinted, w);
            ex_check_expr(right, work, env, gens, ret, hinted, w);
        }
        AstExpr::Unary { operand, .. } => {
            ex_check_expr(operand, work, env, gens, ret, hinted, w);
        }
        AstExpr::ListLit(items) => {
            // mixed_list_type：异构列表（标量异质时报）
            if items.len() >= 2 {
                let t0 = ex_infer(&items[0], work, env);
                for it in items.iter().skip(1) {
                    let tn = ex_infer(it, work, env);
                    let scalar = |t: &IrType| -> bool {
                        matches!(t, IrType::Int | IrType::F64 | IrType::Str | IrType::Bool)
                    };
                    if scalar(&t0) && scalar(&tn) && !ex_type_agrees(&t0, &tn) {
                        work.report_error("列表包含不同类型元素".to_string());
                        break;
                    }
                }
            }
            for it in items {
                ex_check_expr(it, work, env, gens, ret, hinted, w);
            }
        }
        AstExpr::DictLit(pairs) => {
            for (k, v) in pairs {
                ex_check_expr(k, work, env, gens, ret, hinted, w);
                ex_check_expr(v, work, env, gens, ret, hinted, w);
            }
        }
        AstExpr::SetLit(items) | AstExpr::TupleLit(items) => {
            for it in items {
                ex_check_expr(it, work, env, gens, ret, hinted, w);
            }
        }
        AstExpr::Match { expr, arms } => {
            ex_check_expr(expr, work, env, gens, ret, hinted, w);
            let scrut_ty = ex_infer(expr, work, env);
            // 穷尽性（仅模块定义枚举且无 guard/通配 时检查）
            if let IrType::Named { path, .. } = &scrut_ty {
                if let Some(variants) = w.enum_variants_all.get(path) {
                    let mut covered: HashSet<String> = HashSet::new();
                    let mut has_wild = false;
                    let mut has_guard = false;
                    for arm in arms {
                        if arm.guard.is_some() {
                            has_guard = true;
                        }
                        match &arm.pattern {
                            AstPattern::Variant(vn, _) => {
                                covered.insert(ex_base_of(vn));
                            }
                            AstPattern::Ident(n) => {
                                if n == "_" {
                                    has_wild = true;
                                } else if variants.iter().any(|v| v == n) {
                                    // 裸变体名写法（case None:）：parser 产出
                                    // Ident 模式，codegen gen_pattern 会解析为
                                    // Enum::Variant（与 `case Less:` 同路），
                                    // 穷尽性判定须与之一致（lib_option 实测）。
                                    covered.insert(n.clone());
                                }
                            }
                            AstPattern::Wildcard => {
                                has_wild = true;
                            }
                            _ => {}
                        }
                    }
                    if !has_wild && !has_guard {
                        for v in variants {
                            if !covered.contains(v) {
                                work.report_error(format!("match 未穷尽枚举 {path} 的变体 `{v}`"));
                                break;
                            }
                        }
                    }
                }
            }
            // 重复模式（duplicate_match_pattern）：结构化 key，区分嵌套子模式；
            // 带 guard 的臂不参与（同一变体可按条件多次匹配）
            {
                let mut seen: HashSet<String> = HashSet::new();
                for arm in arms {
                    if arm.guard.is_some() {
                        continue;
                    }
                    if let AstPattern::Variant(vn, subpats) = &arm.pattern {
                        let key = format!(
                            "{}({})",
                            ex_base_of(vn),
                            subpats.iter().map(ex_pat_key).collect::<Vec<_>>().join(",")
                        );
                        if !seen.insert(key) {
                            work.report_error(format!("match 重复匹配模式 `{}`", ex_base_of(vn)));
                            break;
                        }
                    }
                }
            }
            // 分支返回类型一致性（match_branch_type_mismatch）
            {
                let mut base_ty: Option<IrType> = None;
                for arm in arms {
                    // 先绑定本臂模式，否则臂内变量（case Some(v): v）未绑定会推成 Any 而永远放行
                    let mut sub = env.clone();
                    ex_bind_pattern(&arm.pattern, &scrut_ty, &mut sub, work);
                    let this = match arm.body.last() {
                        Some(AstStmt::Expr(e)) => ex_infer(e, work, &sub),
                        _ => continue,
                    };
                    if ex_type_queer(&this) {
                        continue;
                    }
                    match &base_ty {
                        None => {
                            base_ty = Some(this);
                        }
                        Some(prev) => {
                            if !ex_agrees_g(prev, &this, gens, w.duck_names) {
                                work.report_error(format!(
                                    "match 分支返回类型不一致：{} 与 {}",
                                    ex_ty_desc(prev),
                                    ex_ty_desc(&this)
                                ));
                                break;
                            }
                        }
                    }
                }
            }
            // 逐臂递归
            for arm in arms {
                if let Some(g) = &arm.guard {
                    ex_check_expr(g, work, env, gens, ret, hinted, w);
                }
                let mut sub = env.clone();
                ex_bind_pattern(&arm.pattern, &scrut_ty, &mut sub, work);
                ex_check_stmts(&arm.body, work, &mut sub, gens, ret, hinted, w);
            }
        }
        AstExpr::Closure {
            params,
            param_tys,
            body,
            ..
        } => {
            let mut sub = env.clone();
            for (i, n) in params.iter().enumerate() {
                let ty = match param_tys.get(i) {
                    Some(Some(t)) => from_ast_type_with_generics(t, gens),
                    _ => IrType::Any,
                };
                sub.insert(n.clone(), ty);
            }
            ex_check_expr(body, work, &mut sub, gens, ret, hinted, w);
        }
        AstExpr::If {
            cond,
            then_body,
            elif_clauses,
            else_body,
        } => {
            ex_check_expr(cond, work, env, gens, ret, hinted, w);
            let mut s1 = env.clone();
            ex_check_stmts(then_body, work, &mut s1, gens, ret, hinted, w);
            for (c, b) in elif_clauses {
                ex_check_expr(c, work, env, gens, ret, hinted, w);
                let mut s2 = env.clone();
                ex_check_stmts(b, work, &mut s2, gens, ret, hinted, w);
            }
            if let Some(eb) = else_body {
                let mut s3 = env.clone();
                ex_check_stmts(eb, work, &mut s3, gens, ret, hinted, w);
            }
        }
        AstExpr::BlockExpr(body) => {
            let mut sub = env.clone();
            ex_check_stmts(body, work, &mut sub, gens, ret, hinted, w);
        }
        AstExpr::Pipe {
            receiver,
            callee,
            args,
        } => {
            ex_check_expr(receiver, work, env, gens, ret, hinted, w);
            ex_check_expr(callee, work, env, gens, ret, hinted, w);
            for a in args {
                let mut sub = env.clone();
                ex_check_expr(a, work, &mut sub, gens, ret, hinted, w);
            }
        }
        AstExpr::Range { start, end, .. } => {
            if let Some(s) = start {
                ex_check_expr(s, work, env, gens, ret, hinted, w);
            }
            if let Some(en) = end {
                ex_check_expr(en, work, env, gens, ret, hinted, w);
            }
        }
        AstExpr::Walrus { target, value } => {
            ex_check_expr(target, work, env, gens, ret, hinted, w);
            ex_check_expr(value, work, env, gens, ret, hinted, w);
        }
        AstExpr::SafeNav { receiver, .. } => {
            ex_check_expr(receiver, work, env, gens, ret, hinted, w);
        }
        AstExpr::Try(inner) => {
            ex_check_expr(inner, work, env, gens, ret, hinted, w);
        }
        AstExpr::NullCoalesce { left, right } => {
            ex_check_expr(left, work, env, gens, ret, hinted, w);
            ex_check_expr(right, work, env, gens, ret, hinted, w);
        }
        AstExpr::ListComprehension {
            output,
            iter,
            cond,
            extra_clauses,
            ..
        } => {
            ex_check_expr(output, work, env, gens, ret, hinted, w);
            ex_check_expr(iter, work, env, gens, ret, hinted, w);
            if let Some(c) = cond {
                ex_check_expr(c, work, env, gens, ret, hinted, w);
            }
            for (_v, it, c) in extra_clauses {
                ex_check_expr(it, work, env, gens, ret, hinted, w);
                if let Some(cc) = c {
                    ex_check_expr(cc, work, env, gens, ret, hinted, w);
                }
            }
        }
        AstExpr::DictComprehension {
            key,
            value,
            iter,
            cond,
            extra_clauses,
            ..
        } => {
            ex_check_expr(key, work, env, gens, ret, hinted, w);
            ex_check_expr(value, work, env, gens, ret, hinted, w);
            ex_check_expr(iter, work, env, gens, ret, hinted, w);
            if let Some(c) = cond {
                ex_check_expr(c, work, env, gens, ret, hinted, w);
            }
            for (_v, it, c) in extra_clauses {
                ex_check_expr(it, work, env, gens, ret, hinted, w);
                if let Some(cc) = c {
                    ex_check_expr(cc, work, env, gens, ret, hinted, w);
                }
            }
        }
        AstExpr::SetComprehension {
            elem,
            iter,
            cond,
            extra_clauses,
            ..
        } => {
            ex_check_expr(elem, work, env, gens, ret, hinted, w);
            ex_check_expr(iter, work, env, gens, ret, hinted, w);
            if let Some(c) = cond {
                ex_check_expr(c, work, env, gens, ret, hinted, w);
            }
            for (_v, it, c) in extra_clauses {
                ex_check_expr(it, work, env, gens, ret, hinted, w);
                if let Some(cc) = c {
                    ex_check_expr(cc, work, env, gens, ret, hinted, w);
                }
            }
        }
        AstExpr::Assign { target, value, .. } => {
            ex_check_expr(target, work, env, gens, ret, hinted, w);
            ex_check_expr(value, work, env, gens, ret, hinted, w);
        }
        AstExpr::Spawn(x) | AstExpr::Move(x) | AstExpr::Panic(x) | AstExpr::Await(x) => {
            ex_check_expr(x, work, env, gens, ret, hinted, w);
        }
        AstExpr::BuildBlock { lhs, body, .. } => {
            ex_check_expr(lhs, work, env, gens, ret, hinted, w);
            let mut sub = env.clone();
            ex_check_stmts(body, work, &mut sub, gens, ret, hinted, w);
        }
        AstExpr::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            let mut s1 = env.clone();
            ex_check_stmts(body, work, &mut s1, gens, ret, hinted, w);
            let scrut_any = IrType::Any;
            for c in catches {
                let mut subc = env.clone();
                ex_bind_pattern(&c.pattern, &scrut_any, &mut subc, work);
                ex_check_stmts(&c.body, work, &mut subc, gens, ret, hinted, w);
            }
            if let Some(eb) = else_body {
                let mut s2 = env.clone();
                ex_check_stmts(eb, work, &mut s2, gens, ret, hinted, w);
            }
            if let Some(fb) = finally_body {
                let mut s3 = env.clone();
                ex_check_stmts(fb, work, &mut s3, gens, ret, hinted, w);
            }
        }
        AstExpr::Paren(inner) | AstExpr::Comptime(inner) => {
            ex_check_expr(inner, work, env, gens, ret, hinted, w);
        }
        AstExpr::KwArg { value, .. } => {
            ex_check_expr(value, work, env, gens, ret, hinted, w);
        }
        AstExpr::IntLit(_)
        | AstExpr::FloatLit(_)
        | AstExpr::StrLit(_)
        | AstExpr::FStrLit(_)
        | AstExpr::RawStrLit(_)
        | AstExpr::BoolLit(_)
        | AstExpr::NoneLit
        | AstExpr::Ident(_) => {}
    }
}

/// 人类可读类型描述（错误信息用）
pub(crate) fn ex_ty_desc(t: &IrType) -> String {
    match t {
        IrType::Int => "int".into(),
        IrType::Int128 => "int128".into(),
        IrType::BigInt => "bigint".into(),
        IrType::Complex => "complex".into(),
        IrType::F64 => "float".into(),
        IrType::Str => "str".into(),
        IrType::Bool => "bool".into(),
        IrType::Unit => "()".into(),
        IrType::Any => "unknown".into(),
        IrType::Never => "never".into(),
        IrType::Self_ => "Self".into(),
        IrType::Ext => "Ext".into(),
        IrType::Named { path, args } => {
            if args.is_empty() {
                path.clone()
            } else {
                format!(
                    "{}<{}>",
                    path,
                    args.iter().map(ex_ty_desc).collect::<Vec<_>>().join(",")
                )
            }
        }
        IrType::Option(x) => format!("Option<{}>", ex_ty_desc(x)),
        IrType::Result { ok, err } => {
            format!("Result<{},{}>", ex_ty_desc(ok), ex_ty_desc(err))
        }
        IrType::Tuple(ts) => format!(
            "({})",
            ts.iter().map(ex_ty_desc).collect::<Vec<_>>().join(",")
        ),
        IrType::Fn { params, ret } => format!(
            "fn({})->{}",
            params.iter().map(ex_ty_desc).collect::<Vec<_>>().join(","),
            ex_ty_desc(ret)
        ),
        IrType::Ref(x) => format!("&{}", ex_ty_desc(x)),
        IrType::MutRef(x) => format!("&mut {}", ex_ty_desc(x)),
        IrType::Duck { .. } => "struct".into(),
        IrType::Generic(g) => g.clone(),
    }
}

/// 类型是否含有指定泛型参数
pub(crate) fn ex_type_has_any_of(t: &IrType, gs: &[String]) -> bool {
    match t {
        IrType::Generic(g) => gs.iter().any(|x| x == g),
        IrType::Named { args, .. } => args.iter().any(|a| ex_type_has_any_of(a, gs)),
        IrType::Option(x) => ex_type_has_any_of(x, gs),
        IrType::Result { ok, err } => ex_type_has_any_of(ok, gs) || ex_type_has_any_of(err, gs),
        IrType::Tuple(ts) => ts.iter().any(|a| ex_type_has_any_of(a, gs)),
        IrType::Fn { params, ret } => {
            params.iter().any(|a| ex_type_has_any_of(a, gs)) || ex_type_has_any_of(ret, gs)
        }
        IrType::Ref(x) | IrType::MutRef(x) => ex_type_has_any_of(x, gs),
        _ => false,
    }
}

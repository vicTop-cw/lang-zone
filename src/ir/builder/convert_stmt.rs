// Lang-Zone 编译器 — ir/builder/convert_stmt.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

// ══════════════════════════════════════════════════════════════
// Stmt 转换
// ══════════════════════════════════════════════════════════════

/// 将 AST Stmt 列表转换为 IR Stmt 列表，展开 LetTuple
pub(crate) fn convert_stmts(ast_stmts: &[AstStmt], ctx: &TypeCtx) -> Vec<Stmt> {
    let mut result = Vec::new();
    for s in ast_stmts {
        if let AstStmt::LetTuple { names, ty, value } = s {
            let ir_value = convert_expr(value, ctx);
            let val_ty = ir_value.ty.clone();
            let tmp_name = format!("__destruct_{}", names.join("_"));
            result.push(Stmt::Let {
                name: tmp_name.clone(),
                ty: val_ty.clone(),
                value: ir_value,
                is_mut: false,
                is_ref: false,
                mods: IrMods::default(),
            });
            for (i, name) in names.iter().enumerate() {
                if name == "_" {
                    continue;
                }
                let field_expr = Expr::new(
                    ExprKind::FieldAccess {
                        base: Box::new(Expr::new(
                            ExprKind::Var(tmp_name.clone()),
                            val_ty.clone(),
                            Span::unknown(),
                        )),
                        field: format!("{}", i),
                    },
                    IrType::Any,
                    Span::unknown(),
                );
                // 解构字段类型：从元组类型提取元素（`let (lower, upper) = size_hint()`
                // 的 (int, Option<int>) → lower: int, upper: Option<int>），否则 Any
                // 导致 `return (lower, upper)` 推断错误（E0277 ImplicitFrom）
                let field_ty = match &val_ty {
                    IrType::Tuple(items) => items.get(i).cloned().unwrap_or(IrType::Any),
                    _ => ty.as_ref().map(|t| from_ast_type(t)).unwrap_or(IrType::Any),
                };
                result.push(Stmt::Let {
                    name: name.clone(),
                    ty: field_ty,
                    value: field_expr,
                    is_mut: false,
                    is_ref: false,
                    mods: IrMods::default(),
                });
            }
        } else {
            result.push(convert_stmt(s, ctx));
        }
    }
    result
}

pub(crate) fn convert_stmt(ast_stmt: &AstStmt, ctx: &TypeCtx) -> Stmt {
    match ast_stmt {
        AstStmt::Expr(AstExpr::Match { expr, arms }) => {
            // match 语句 → 直接用 IR Match 节点（codegen 已有完整支持）
            let ir_scrutinee = convert_expr(expr, ctx);
            let ir_arms: Vec<MatchArm> = arms
                .iter()
                .map(|arm| {
                    let pat = convert_ast_pattern(&arm.pattern, ctx).unwrap_or(Pattern::Wildcard);
                    let guard = arm.guard.as_ref().map(|g| convert_expr(g, ctx));
                    let mut arm_ctx = TypeCtx::new();
                    arm_ctx.vars = ctx.vars.clone();
                    arm_ctx.current_generics = ctx.current_generics.clone();
                    arm_ctx.current_ret_ty = ctx.current_ret_ty.clone();
                    arm_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                    // 拷贝 fn_returns：match 臂内函数调用的返回类型推断依赖此表
                    // （postorder(left) 在 case 分支内 → lhs.ty=Any → List+List 误走 LzAdd）
                    arm_ctx.fn_returns = ctx.fn_returns.clone();
                    arm_ctx.fn_raises = ctx.fn_raises.clone();
                    arm_ctx.fn_params = ctx.fn_params.clone();
                    arm_ctx.struct_fields = ctx.struct_fields.clone();
                    arm_ctx.struct_methods = ctx.struct_methods.clone();
                    arm_ctx.self_ty = ctx.self_ty.clone();
                    // 复制 enum_variants 以便模式匹配能正确解析枚举类型
                    for (vn, en) in &ctx.enum_variants {
                        arm_ctx.enum_variants.insert(vn.clone(), en.clone());
                    }
                    for (cn, ct) in &ctx.top_level_consts {
                        arm_ctx.top_level_consts.insert(cn.clone(), ct.clone());
                        arm_ctx.mutated_top_level_consts = ctx.mutated_top_level_consts.clone();
                    }
                    // 也复制 struct 信息用于模式匹配
                    for sn in &ctx.struct_names {
                        arm_ctx.struct_names.insert(sn.clone());
                    }
                    if let AstPattern::Ident(name) = &arm.pattern {
                        // 裸枚举变体名模式（`case Equal:`）是变体匹配而非变量绑定：
                        // 登记为绑定变量会把臂体内 `return Equal` 解析成类型为 Self 的
                        // 变量引用（E0277 ImplicitFrom<Self>）。仅当名字不是枚举变体
                        // 时才 add_var（如 `case x:` 绑定整个 scrutinee）
                        let is_enum_variant = ctx.enum_variants.contains_key(name.as_str());
                        if !is_enum_variant {
                            let scrut_ty = infer_expr_type(expr, ctx);
                            arm_ctx.add_var(name, scrut_ty);
                        }
                    }
                    // ref mut 绑定（case Some(ref mut c)）：c 登记为 MutRef 内层类型，
                    // 臂体内 c = c + 1 需生成 *c = *c + 1（解引用赋值）
                    if let AstPattern::RefMutIdent(name) = &arm.pattern {
                        let scrut_ty = infer_expr_type(expr, ctx);
                        let inner = match &scrut_ty {
                            IrType::MutRef(i) => *i.clone(),
                            IrType::Ref(i) => *i.clone(),
                            other => other.clone(),
                        };
                        arm_ctx.add_var(name, IrType::MutRef(Box::new(inner)));
                    }
                    // 变体模式字段绑定：Shape::Circle(x: _, y: _, radius: r) →
                    // r 绑定为 radius 字段类型（int），而非整个 scrutinee 类型
                    if let AstPattern::Variant(..) = &arm.pattern {
                        if let Some(ftypes) = field_types_for_variant(&arm.pattern, &arm_ctx) {
                            for (fname, ty) in ftypes {
                                arm_ctx.add_var(&fname, ty);
                            }
                        }
                    }
                    // 内置 Option/Result 变体：Some(v) → v 绑定为内层类型
                    let scrut_ty2 = infer_expr_type(expr, ctx);
                    if let Some(ftypes) = field_types_for_builtin_variant(&arm.pattern, &scrut_ty2)
                    {
                        for (fname, ty) in ftypes {
                            arm_ctx.add_var(&fname, ty);
                        }
                    }
                    let body = convert_block_with_ctx(&arm.body, &arm_ctx);
                    MatchArm {
                        pattern: pat,
                        guard,
                        body,
                    }
                })
                .collect();
            Stmt::Match {
                scrutinee: ir_scrutinee,
                arms: ir_arms,
            }
        }
        AstStmt::Expr(e) => {
            // 构建块（=:/~:）在表达式位置出现时，需要特殊处理
            // =: 构建块作为表达式语句：生成立即调用闭包作为表达式
            if let AstExpr::BuildBlock {
                kind: BuildKind::Var,
                lhs,
                body,
            } = e
            {
                let lhs_name = match &**lhs {
                    AstExpr::Ident(name) => name.clone(),
                    _ => return Stmt::Pass,
                };
                let body_block = convert_block_with_ctx(body, ctx);
                let body_expr = Expr::new(
                    ExprKind::BlockExpr { block: body_block },
                    IrType::Any,
                    Span::unknown(),
                );
                let init_expr = Expr::new(
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(Expr::new(
                            ExprKind::Lambda {
                                params: vec![],
                                body: Box::new(body_expr),
                                is_move: false,
                                ret_ty: None,
                            },
                            IrType::Any,
                            Span::unknown(),
                        )),
                        args: vec![],
                    },
                    IrType::Any,
                    Span::unknown(),
                );
                // =: 构建块作为语句：let lhs = <立即调用闭包>
                // 类型从 body 末尾表达式推断（元组如 (a,b,c) → Tuple），
                // 否则 factors 登记为 Any，后续 `multiply ~: factors`
                // 的元组拆包无法识别（E0061）
                // 若 body 含无值 return（return; 退出构建块自身）→ 块值 Unit
                let has_bare_return = body.iter().any(|s| ast_stmt_has_bare_return(s));
                let build_ty = if has_bare_return {
                    IrType::Unit
                } else {
                    body.last()
                        .map(|s| infer_stmt_type(s, ctx))
                        .filter(|t| !matches!(t, IrType::Any))
                        .unwrap_or(IrType::Any)
                };
                return Stmt::Let {
                    name: lhs_name,
                    ty: build_ty,
                    value: init_expr,
                    is_mut: false,
                    is_ref: false,
                    mods: IrMods::default(),
                };
            }
            Stmt::ExprStmt {
                expr: convert_expr(e, ctx),
            }
        }

        AstStmt::Pass => Stmt::Pass,

        AstStmt::TypeAlias { name, ty } => Stmt::TypeAlias {
            name: name.clone(),
            ty: from_ast_type(ty),
        },

        AstStmt::Let {
            name,
            mutable,
            is_ref,
            ty,
            value,
            mods,
            ..
        } => {
            let ir_ty = ty
                .as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, ctx));
            // 类型注解为 Option/Result 时，裸 `None`（内建构造器名作标识符场景，
            // 如 edge-keyword-identifier.lz 中先 `let None = 300` 后又
            // `let x: Option<int> = None`）应解析为 None 字面量，而非变量引用
            // （否则 codegen 的 downgraded_vars 会把它重命名为 None_，E0425）
            let is_option_result_annot =
                matches!(&ir_ty, IrType::Option(_) | IrType::Result { .. })
                    || matches!(&ir_ty, IrType::Named { path, .. }
                    if path == "Option" || path == "Result");
            let mut ir_value =
                if is_option_result_annot && matches!(value, AstExpr::Ident(n) if n == "None") {
                    Expr::new(
                        ExprKind::Lit(LitKind::None_),
                        ir_ty.clone(),
                        Span::unknown(),
                    )
                } else {
                    convert_expr(value, ctx)
                };
            // 若声明类型为 BigInt 且值为字面量，覆盖值为 BigInt 类型
            // 使 codegen 生成 BigInt::from(...) 而非 i128 后缀
            if matches!(&ir_ty, IrType::BigInt) && matches!(&ir_value.kind, ExprKind::Lit(_)) {
                ir_value.ty = IrType::BigInt;
            }
            // 当 value 是 Lambda（部分应用展开等），使用 Lambda 的类型而非 infer 的类型
            // 当 value 的 IR 类型为 Any 且无显式类型注解时，也使用 IR 类型避免错误标注
            // 注意：若存在显式类型注解（如 let n: Option<int> = None），必须保留注解类型
            let ir_ty = match &ir_value.ty {
                IrType::Fn { .. } => ir_value.ty.clone(),
                IrType::Any if ir_ty == IrType::Any => ir_value.ty.clone(),
                _ => ir_ty,
            };
            // __from__ 隐式转换触发（06d §十四，P1-2 第一环）：
            // `let w: Wrapper = 5`——注解类型是用户 struct、值类型与之不同、
            // 该 struct 定义了 __from__（静态方法）→ 包装为 Wrapper::__from__(value)。
            // 单步转换：值类型已是注解类型或 Any/泛型时不触发，避免过度转换
            if ty.is_some() {
                if let IrType::Named { path, .. } = &ir_ty {
                    let has_from = ctx
                        .struct_methods
                        .get(path)
                        .map(|ms| ms.contains("__from__"))
                        .unwrap_or(false);
                    let val_ty_matches = !matches!(ir_value.ty, IrType::Any | IrType::Generic(_))
                        && ir_value.ty != ir_ty;
                    if has_from && val_ty_matches {
                        ir_value = Expr::new(
                            ExprKind::Call {
                                type_args: vec![],
                                callee: Box::new(Expr::new(
                                    ExprKind::Var(format!("{}::__from__", path)),
                                    IrType::Any,
                                    Span::unknown(),
                                )),
                                args: vec![ir_value],
                            },
                            ir_ty.clone(),
                            Span::unknown(),
                        );
                    }
                }
            }
            // 当 Let 类型注解为 fn(..) -> .. 且 value 是 Lambda 时，
            // 将 fn 的参数类型传播到 Lambda 参数中
            if let IrType::Fn {
                params: fn_params, ..
            } = &ir_ty
            {
                if let ExprKind::Lambda {
                    params: lambda_params,
                    ..
                } = &mut ir_value.kind
                {
                    if lambda_params.len() == fn_params.len() {
                        for (lp, fp) in lambda_params.iter_mut().zip(fn_params.iter()) {
                            lp.ty = fp.clone();
                        }
                    }
                }
            }
            // 无 let 前缀的默认可变绑定（x = v）且变量在**本块之外**已存在 → 重新赋值
            // （闭包内写外部变量：total = total + x → total = total + x 而非新绑定）
            // 本块首次声明（block_declared 含 name）保持 Let；外部已有但本块未声明 → Assign
            // 顶层变量（top_level_consts 含 name）→ 修改全局（guard_for_3.lz size = size - 1）
            let is_top_level_mut = ctx.top_level_consts.contains_key(name.as_str());
            if *mutable
                && ty.is_none()
                && (ctx.vars.contains_key(name.as_str()) || is_top_level_mut)
                && !ctx.block_declared.contains(name.as_str())
            {
                Stmt::Assign {
                    target: Expr::new(ExprKind::Var(name.clone()), ir_ty.clone(), Span::unknown()),
                    value: ir_value,
                }
            } else {
                Stmt::Let {
                    name: name.clone(),
                    ty: ir_ty,
                    value: ir_value,
                    is_mut: *mutable,
                    is_ref: *is_ref,
                    mods: IrMods::from_ast(mods),
                }
            }
        }

        AstStmt::Const {
            name,
            ty,
            value,
            mods,
        } => {
            let ir_ty = ty
                .as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, ctx));
            let mut ir_value = convert_expr(value, ctx);
            // 当 value 是 Lambda 时，使用 Lambda 的类型
            // 当 value 的 IR 类型为 Any（~: 构建块等），也使用 IR 类型避免错误标注
            let ir_ty = match &ir_value.ty {
                IrType::Fn { .. } | IrType::Any => ir_value.ty.clone(),
                _ => ir_ty,
            };
            // 同上：传播 fn 参数类型到 Lambda
            if let IrType::Fn {
                params: fn_params, ..
            } = &ir_ty
            {
                if let ExprKind::Lambda {
                    params: lambda_params,
                    ..
                } = &mut ir_value.kind
                {
                    if lambda_params.len() == fn_params.len() {
                        for (lp, fp) in lambda_params.iter_mut().zip(fn_params.iter()) {
                            lp.ty = fp.clone();
                        }
                    }
                }
            }
            Stmt::Let {
                name: name.clone(),
                ty: ir_ty,
                value: ir_value,
                is_mut: false,
                is_ref: false,
                mods: IrMods::from_ast(mods),
            }
        }

        AstStmt::Return(val) => {
            let value = val.as_ref().map(|v| {
                let expr = convert_expr(v, ctx);
                // 返回值隐式转换: return S 但声明返回 T → 插入 ImplicitConvert
                // （iterator 函数内 return 等价 raise，不做隐式转换，避免 String→T 残留）
                if !ctx.current_is_iterator {
                    // `return None`（目标 Option<T>）：None 字面量直接返回即可，
                    // 无需 ImplicitConvert——否则生成 <Option<i64> as ImplicitFrom<i64>>
                    // 错误包装（E0277，iter.lz `__next__` 的 None 分支）
                    let ret_is_option = ctx
                        .current_ret_ty
                        .as_ref()
                        .map(|rt| {
                            matches!(rt, IrType::Option(_))
                                || matches!(rt, IrType::Named { path, .. }
                                    if path == "Option" || path == "Result")
                        })
                        .unwrap_or(false);
                    let expr_is_none = matches!(expr.kind, ExprKind::Lit(LitKind::None_))
                        || matches!(&expr.kind, ExprKind::Var(n) if n == "None" || n == "None_");
                    // 泛型参数规范化：`Named("T", [])`（from_ast_type 表示）→ `Generic("T")`
                    // （infer 表示），使两侧容器元素类型可比较（box.lz `Err(self.clone())`：
                    // ret_ty err 侧是 Named("Rc", [Named("T")])，expr.ty err 侧是
                    // Named("Rc", [Generic("T")])，不规范化则 re != ee → 误插 ImplicitConvert）
                    let normalize_generic = |ty: &IrType| -> IrType { normalize_gen(ty, &ctx.current_generics) };
                    // 同容器名、仅元素类型差异（List<Any> vs List<U>）：跳过转换，
                    // 让 Rust 从返回类型推断（`return result` 中 result 是 List() 空构造，
                    // 元素类型 Any→i64 与返回 List<I::Item> 不匹配，E0277）。
                    // 递归比较 Named 参数：`Option<Rc<T>>` vs `Option<Rc<i64>>`
                    // （box.lz Weak::upgrade `Some(Rc(_inner: self._rc))`，_inner:int
                    // 占位导致 Rc 推断为 Rc<i64>，E0277 ImplicitFrom）
                    let same_named_rec = |a: &IrType, b: &IrType| -> bool {
                        // 任何一侧为 Any/Generic 即兼容（collect_list 的 result 是
                        // List() 空构造 → List<Any>，返回类型 List<I::Item>；当前只有
                        // 双方 Named 才检查，Any 会落到 _ => a==b 返回 false，E0277）
                        if matches!(a, IrType::Generic(_) | IrType::Any)
                            || matches!(b, IrType::Generic(_) | IrType::Any)
                            || a == b
                        {
                            return true;
                        }
                        match (a, b) {
                            (IrType::Named { path: p1, args: r1 }, IrType::Named { path: p2, args: r2 })
                                if p1 == p2 && r1.len() == r2.len() && !r1.is_empty() =>
                            {
                                r1.iter().zip(r2.iter()).all(|(x, y)| {
                                    if x == y {
                                        true
                                    } else if matches!(x, IrType::Generic(_) | IrType::Any)
                                        || matches!(y, IrType::Generic(_) | IrType::Any)
                                    {
                                        true
                                    } else if let (IrType::Named { path: q1, .. }, IrType::Named { path: q2, .. }) =
                                        (x, y)
                                    {
                                        // 关联类型路径（I::Item / I.Item）与具体类型兼容：
                                        // collect_list 的 `return result` 中 result 推断为
                                        // Vec<i64>，返回类型 Vec<I::Item>（E0277 ImplicitFrom）
                                        q1 == q2
                                            || q1.contains("::")
                                            || q1.contains('.')
                                            || q2.contains("::")
                                            || q2.contains('.')
                                    } else {
                                        false
                                    }
                                })
                            }
                            // Option(Int) vs Option(Any)——内部 Any 兼容（size_hint 的
                            // return (new_lower, new_upper) 中 new_upper 是 Option<Any>，
                            // 否则 ImplicitFrom 插入 E0308/E0277）
                            (IrType::Option(i1), IrType::Option(i2)) => {
                                if i1 == i2 {
                                    true
                                } else if matches!(i1.as_ref(), IrType::Generic(_) | IrType::Any)
                                    || matches!(i2.as_ref(), IrType::Generic(_) | IrType::Any)
                                {
                                    true
                                } else {
                                    false
                                }
                            }
                            // 一方空 args（类型未推断，如 `Rc` vs `Rc<T>`）：视为兼容，
                            // 让 Rust 从返回类型推断（box.lz `Some(Rc(_inner: self._rc))`
                            // 的 Rc 推断为 Named{args:[]}，E0277 ImplicitFrom）
                            (
                                IrType::Named { path: p1, args },
                                IrType::Named { path: p2, args: args2 },
                            ) if p1 == p2
                                && (args.is_empty() || args2.is_empty())
                                && args.len() != args2.len() =>
                            {
                                true
                            }
                            _ => a == b,
                        }
                    };
                    let same_container_any = match (ctx.current_ret_ty.as_ref(), &expr.ty) {
                        // 元组返回（size_hint 的 `return (lower, upper)`）：元素递归
                        // 比较（Any 元素兼容），否则 (i64, Any) vs (i64, Option<i64>)
                        // 插入错误 ImplicitFrom（E0277）
                        (
                            Some(IrType::Tuple(ra)),
                            IrType::Tuple(ea),
                        ) if ra.len() == ea.len() => {
                            let ok = ra.iter().zip(ea.iter()).all(|(r, e)| same_named_rec(r, e));
                            ok
                        }
                        (
                            Some(IrType::Named { path: rp, args: ra }),
                            IrType::Named { path: ep, args: ea },
                        ) if rp == ep && !ra.is_empty() && !ea.is_empty() => {
                            ra.iter().zip(ea.iter()).all(|(r, e)| same_named_rec(r, e))
                        }
                        // Result<T, Any> vs Result<T, Rc<T>>：err/ok 侧 Any 时跳过转换
                        // （box.lz `Ok(self.get())`，get 返回 &T，Ok 推断 Result<T, Any>）
                        (
                            Some(IrType::Result { ok: ro, err: re }),
                            IrType::Result { ok: eo, err: ee },
                        ) => {
                            let ro = normalize_generic(ro);
                            let re = normalize_generic(re);
                            let eo = normalize_generic(eo);
                            let ee = normalize_generic(ee);
                            (matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || ro == eo)
                                && (matches!(re, IrType::Generic(_) | IrType::Any)
                                    || matches!(ee, IrType::Generic(_) | IrType::Any)
                                    || re == ee)
                        }
                        // Result<T, E> 在 AST 中可能解析为 Named("Result", [ok, err])，
                        // 而 Err(...) 构造推断为 IrType::Result 变体（box.lz try_unwrap
                        // `return Err(self)`）——跨表示形式比较，跳过等价的 ImplicitConvert
                        (
                            Some(IrType::Named { path: rp, args: ra }),
                            IrType::Result { ok: eo, err: ee },
                        ) if rp == "Result" && ra.len() == 2 => {
                            let ro = normalize_generic(&ra[0]);
                            let re = normalize_generic(&ra[1]);
                            let eo = normalize_generic(eo);
                            let ee = normalize_generic(ee);
                            (matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || ro == eo)
                                && (matches!(re, IrType::Generic(_) | IrType::Any)
                                    || matches!(ee, IrType::Generic(_) | IrType::Any)
                                    || re == ee)
                        }
                        (
                            Some(IrType::Result { ok: ro, err: re }),
                            IrType::Named { path: ep, args: ea },
                        ) if ep == "Result" && ea.len() == 2 => {
                            let ro = normalize_generic(ro);
                            let re = normalize_generic(re);
                            let eo = normalize_generic(&ea[0]);
                            let ee = normalize_generic(&ea[1]);
                            (matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || ro == eo)
                                && (matches!(re, IrType::Generic(_) | IrType::Any)
                                    || matches!(ee, IrType::Generic(_) | IrType::Any)
                                    || re == ee)
                        }
                        // Option 同理：Named("Option", [T]) vs IrType::Option(T)
                        (
                            Some(IrType::Named { path: rp, args: ra }),
                            IrType::Option(eo),
                        ) if rp == "Option" && ra.len() == 1 => {
                            let ro = normalize_generic(&ra[0]);
                            let eo = normalize_generic(eo);
                            matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || same_named_rec(&ro, &eo)
                        }
                        (
                            Some(IrType::Option(ro)),
                            IrType::Named { path: ep, args: ea },
                        ) if ep == "Option" && ea.len() == 1 => {
                            let ro = normalize_generic(ro);
                            let eo = normalize_generic(&ea[0]);
                            matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || same_named_rec(&ro, &eo)
                        }
                        // Option/Option：`Some(Rc(_inner: self._rc))` 的 ret/expr 都是
                        // IrType::Option 变体（box.lz Weak::upgrade，Rc 推断为空 args），
                        // 递归比较内部元素（E0277 ImplicitFrom）
                        (
                            Some(IrType::Option(ro)),
                            IrType::Option(eo),
                        ) => {
                            let ro = normalize_generic(ro);
                            let eo = normalize_generic(eo);
                            matches!(ro, IrType::Generic(_) | IrType::Any)
                                || matches!(eo, IrType::Generic(_) | IrType::Any)
                                || same_named_rec(&ro, &eo)
                        }
                        _ => false,
                    };
                    if !(expr_is_none && ret_is_option) && !same_container_any {
                        if let Some(ref ret_ty) = ctx.current_ret_ty {
                            // ref 返回（`return self[key]`，&V）且表达式是值（V）：
                            // 跳过 ImplicitConvert（E0277 &V: ImplicitFrom<V>），
                            // codegen 对 HashMap 索引生成 .get(&key).unwrap()（&V）
                            let ref_ret_ok =
                                matches!(ret_ty, IrType::Ref(inner) if expr.ty == (**inner).clone())
                                    || matches!(ret_ty, IrType::MutRef(inner)
                                        if expr.ty == (**inner).clone());
                            if !ref_ret_ok
                                && expr.ty != *ret_ty
                                && !matches!(ret_ty, IrType::Unit)
                            {
                                // __from__ 返回值触发点（06d §十四，P1-2 第三环）：
                                // `def make() -> Wrap { return 7 }`（Wrap 定义了
                                // __from__(int)）→ 包装为 Wrap::__from__(7)。优先于
                                // ImplicitConvert 兜底（用户定义的显式转换语义）
                                if let IrType::Named { path, .. } = ret_ty {
                                    let has_from = ctx
                                        .struct_methods
                                        .get(path)
                                        .map(|ms| ms.contains("__from__"))
                                        .unwrap_or(false);
                                    let val_ok = !matches!(
                                        expr.ty,
                                        IrType::Any | IrType::Generic(_)
                                    );
                                    if has_from && val_ok {
                                        return Expr::new(
                                            ExprKind::Call {
                                                type_args: vec![],
                                                callee: Box::new(Expr::new(
                                                    ExprKind::Var(format!("{}::__from__", path)),
                                                    IrType::Any,
                                                    Span::unknown(),
                                                )),
                                                args: vec![expr],
                                            },
                                            ret_ty.clone(),
                                            Span::unknown(),
                                        );
                                    }
                                }
                                // 返回类型含关联类型路径（`I::Item` / `Option<(A::Item, B::Item)>`，
                                // iter.lz sum/product/Zip::next）：跳过 ImplicitConvert，
                                // 让 Rust 从函数签名推断（E0277 ImplicitFrom）
                                fn contains_assoc_path(ty: &IrType) -> bool {
                                    match ty {
                                        IrType::Named { path, args } => {
                                            path.contains("::")
                                                || path.contains('.')
                                                || args.iter().any(contains_assoc_path)
                                        }
                                        IrType::Option(inner) => contains_assoc_path(inner),
                                        IrType::Result { ok, err } => {
                                            contains_assoc_path(ok) || contains_assoc_path(err)
                                        }
                                        IrType::Tuple(items) => {
                                            items.iter().any(contains_assoc_path)
                                        }
                                        _ => false,
                                    }
                                }
                                // 源/目标类型含泛型类型参数（K, V, T 等）：跳过——避免
                                // 在泛型函数体内对 Box<T>::replace `return old`（T 类型）
                                // 误生成 <T as ImplicitFrom<i64>>::__implicit_from__ (E0277)
                                fn contains_generic(ty: &IrType) -> bool {
                                    match ty {
                                        IrType::Generic(_) => true,
                                        IrType::Named { args, .. } => args.iter().any(contains_generic),
                                        IrType::Option(inner) => contains_generic(inner),
                                        IrType::Result { ok, err } => {
                                            contains_generic(ok) || contains_generic(err)
                                        }
                                        IrType::Tuple(items) => items.iter().any(contains_generic),
                                        IrType::Ref(inner) | IrType::MutRef(inner) => contains_generic(inner),
                                        _ => false,
                                    }
                                }
                                let ret_is_assoc_path = contains_assoc_path(ret_ty);
                                let src_or_ret_has_generic = contains_generic(ret_ty) || contains_generic(&expr.ty);
                                if !ret_is_assoc_path && !src_or_ret_has_generic {
                                    return Expr::new(
                                        ExprKind::ImplicitConvert {
                                            source: Box::new(expr.clone()),
                                            target_ty: ret_ty.clone(),
                                        },
                                        ret_ty.clone(),
                                        Span::unknown(),
                                    );
                                }
                            }
                        }
                    }
                }
                expr
            });
            Stmt::Return { value }
        }

        AstStmt::Yield(val) => {
            let value = match val {
                Some(expr) => convert_expr(expr, ctx),
                None => Expr::new(ExprKind::Lit(LitKind::None_), IrType::Unit, Span::unknown()),
            };
            Stmt::Yield { value }
        }

        AstStmt::YieldFrom(e) => Stmt::YieldFrom {
            iter: convert_expr(e, ctx),
        },

        AstStmt::While {
            cond,
            guard,
            body,
            else_body,
        } => Stmt::While {
            cond: convert_expr(cond, ctx),
            guard: guard.as_ref().map(|g| convert_expr(g, ctx)),
            body: convert_block(body, ctx),
            else_body: else_body.as_ref().map(|b| convert_block(b, ctx)),
        },

        AstStmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
            ..
        } => {
            let ir_expr = convert_expr(expr, ctx);

            // 从 expr 类型推断模式绑定变量的类型
            // e.g. while let Some(item) = opt (Option<int>) → item: int
            // 注意：方法调用返回类型推断不完整（it.next() → Any），
            // 仅对直接 Option<T> 类型变量生效
            let inner_ty = match &ir_expr.ty {
                IrType::Option(inner) => Some((**inner).clone()),
                IrType::Named { path, args } if path == "Option" && args.len() == 1 => {
                    Some(args[0].clone())
                }
                _ => None,
            };

            // 构建增强的 ctx（包含模式绑定变量类型）
            let mut body_ctx = ctx.clone();
            if let Some(ref ty) = inner_ty {
                let mut pattern_vars: Vec<String> = Vec::new();
                collect_ast_pattern_vars(pattern, &mut pattern_vars);
                for var in &pattern_vars {
                    body_ctx.vars.insert(var.clone(), ty.clone());
                }
            }

            let ir_pattern = convert_ast_pattern(pattern, ctx).unwrap_or(Pattern::Wildcard);
            let ir_body = convert_block(body, &body_ctx);

            Stmt::WhileLet {
                pattern: ir_pattern,
                expr: ir_expr,
                guard: guard.as_ref().map(|g| convert_expr(g, ctx)),
                body: ir_body,
            }
        }

        AstStmt::For {
            var,
            iter,
            guard,
            body,
            else_body,
        } => {
            let mut loop_ctx = TypeCtx::new();
            // 从 ctx 复制函数泛型上下文
            loop_ctx.current_generics = ctx.current_generics.clone();
            loop_ctx.current_ret_ty = ctx.current_ret_ty.clone();
            loop_ctx.current_is_iterator = ctx.current_is_iterator;
            // 复制变量类型（predicate: fn(ref I.Item) 等函数参数）：
            // 否则 for 循环体内 `predicate(item)` 中 predicate 类型丢失→Any，
            // 无法识别 ref 参数自动取引用（iter.lz filter/find，E0308）
            loop_ctx.vars = ctx.vars.clone();
            loop_ctx.current_fn_name = ctx.current_fn_name.clone();
            // 推导迭代变量的类型
            let iter_ty = infer_expr_type(iter, ctx);
            // __into_iter__ 分派（06d §九）：for-in 的迭代对象是定义了
            // __into_iter__ 的用户 struct → 包装为 `iter.__into_iter__()`
            //（返回 List<T>，复用 Vec 迭代机制；否则生成裸 .into_iter()
            // 对非 IntoIterator 类型报 E0599）
            let iter_expr = if let IrType::Named { path, .. } = &iter_ty {
                if ctx
                    .struct_methods
                    .get(path)
                    .map(|ms| ms.contains("__into_iter__"))
                    .unwrap_or(false)
                {
                    let recv = convert_expr(iter, ctx);
                    // 返回类型取方法注解（List<int> 等），注解缺失回退 Any
                    let ret_ty = ctx
                        .lookup_fn_return(&format!("{}.__into_iter__", path))
                        .clone();
                    Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(recv),
                            method: "__into_iter__".into(),
                            args: vec![],
                        },
                        ret_ty,
                        Span::unknown(),
                    )
                } else {
                    convert_expr(iter, ctx)
                }
            } else {
                convert_expr(iter, ctx)
            };
            let elem_ty = match &iter_expr.ty {
                IrType::Named { args, .. } if !args.is_empty() => args[0].clone(),
                _ => IrType::Any,
            };
            loop_ctx.add_var(var, elem_ty);
            Stmt::For {
                var: var.clone(),
                iter: iter_expr,
                guard: guard.as_ref().map(|g| convert_expr(g, ctx)),
                body: convert_block_with_ctx(body, &loop_ctx),
                else_body: else_body.as_ref().map(|b| convert_block(b, ctx)),
            }
        }

        AstStmt::Loop(body) => Stmt::While {
            cond: Expr::new(
                ExprKind::Lit(LitKind::Bool(true)),
                IrType::Bool,
                Span::unknown(),
            ),
            guard: None,
            body: convert_block(body, ctx),
            else_body: None,
        },

        AstStmt::Break(_) => Stmt::Break,
        AstStmt::BreakLabel { label, value } => Stmt::BreakLabel {
            label: label.clone(),
            value: value.as_ref().map(|v| convert_expr(v, ctx)),
        },
        AstStmt::Continue => Stmt::Continue,

        AstStmt::Block { label, body } => Stmt::BlockLabel {
            label: label.clone(),
            body: convert_block(body, ctx),
        },

        AstStmt::CheckerBlock {
            label,
            ps_name,
            default_checker,
            body,
        } => {
            // checker 块 → IR 压缩为模块级 fn NAME(ps: &mut __Params)
            // 惰性登记：定义时不执行，仅注册为 Item::CheckerBlock
            // checker 块体是独立词法块（闭包语义）：块内裸赋值 `depth = depth + 1`
            // 引用外部捕获变量，不应继承外层 block_declared 误转 let 绑定（E0425）
            let mut chk_ctx = ctx.clone();
            chk_ctx.block_declared.clear();
            let ir_body = convert_block(body, &chk_ctx);
            // 捕获的外层函数局部变量（block 闭包语义，规范 05b-block命名块.md §三）：
            // body 引用的、在函数作用域（ctx.vars）内声明的变量（out/depth/result 等），
            // 需作为 fn 的 &mut 参数传入，否则提升为模块级 fn 后 E0425（block_demo 等）
            let captured = collect_checker_captured(&ir_body, ctx, ps_name.as_deref());
            ctx.pending_items.borrow_mut().push(Item::CheckerBlock {
                name: label.clone(),
                ps_name: ps_name.clone(),
                default_checker: default_checker.clone(),
                body: ir_body,
                captured,
            });
            // 占位语句（checker 块不内联执行）
            Stmt::Pass
        }

        AstStmt::BlockCall { label, args } => {
            // 触发调用 → 转换为 fn_call(label)(args)
            // 元组实参 (a, b, c) → 展开为多个独立参数（checker 块 ps.args[i] 逐位解包，
            // block_tailrec.lz factorial[(5, 1)]；单元素 (10,) 也经 TupleLit 展开）
            let call_args: Vec<Expr> = match args {
                AstExpr::TupleLit(elems) => elems.iter().map(|e| convert_expr(e, ctx)).collect(),
                other => vec![convert_expr(other, ctx)],
            };
            Stmt::ExprStmt {
                expr: Expr::new(
                    ExprKind::Call {
                        callee: Box::new(Expr::new(
                            ExprKind::Var(label.clone()),
                            IrType::Any,
                            Span::unknown(),
                        )),
                        type_args: vec![],
                        args: call_args,
                    },
                    IrType::Unit,
                    Span::unknown(),
                ),
            }
        }

        AstStmt::Defer(body) => {
            // defer → 保留 Stmt::Defer，由 codegen 生成 DeferGuard（块退出时 Drop 执行）
            Stmt::Defer {
                body: convert_block(body, ctx),
            }
        }

        AstStmt::Comptime { body } => {
            // comptime: 块 — 编译期求值，结果内联（B3）。
            // 求值成功且有值 → 内联为字面量表达式；无值（块内仅 let/const）→ Pass；
            // 求值失败 → 收集错误并降级为普通 Block（保留原编译行为）。
            // 使用真实模块（comptime 块内可调用模块内函数/引用 const）
            let empty_module = ast::Module::default();
            let module_ref = ctx
                .comptime_module
                .as_ref()
                .map(|m| m.as_ref())
                .unwrap_or(&empty_module);
            let mut cctx = crate::comptime::ComptimeContext::new(module_ref);
            // 注入源码文本（inspect.getsource/getsourcelines 数据源，main.rs 已填）
            if let Some(src) = &module_ref.source_text {
                cctx = cctx.with_source(src.clone());
            }
            // 注入顶层 const 求值结果（comptime 块内解析 const 引用）
            for (n, v) in &ctx.comptime_consts {
                cctx.symtab.insert(n.clone(), v.clone());
            }
            match crate::comptime::ComptimeEvaluator::eval_block(body, &mut cctx) {
                Ok(Some(v)) => match comptime_value_to_lit(&v, None) {
                    // comptime 块仅打印/副作用（值为 None）时不产出代码，
                    // 避免生成裸 `None;` 语句导致 rustc E0282
                    Some(ExprKind::Lit(LitKind::None_)) => Stmt::Pass,
                    Some(kind) => Stmt::ExprStmt {
                        expr: Expr::new(kind, IrType::Any, Span::unknown()),
                    },
                    None => Stmt::Block {
                        stmts: convert_block(body, ctx).stmts,
                    },
                },
                Ok(None) => Stmt::Pass,
                Err(e) => {
                    ctx.errors
                        .borrow_mut()
                        .push(format!("comptime 块求值失败: {}", e));
                    Stmt::Block {
                        stmts: convert_block(body, ctx).stmts,
                    }
                }
            }
        }

        // BUG-CG-004（轮次12）：raise 统一表示为 Stmt::Raise 节点（而非 ExprStmt(panic! 调用)），
        // 否则 builder 的 raises 改写（Stmt::Raise → return Err）无法命中，raises 链仍会丢失。
        // 非 raises 函数由 codegen 将 Stmt::Raise 降级为 panic!（catch_unwind 仍可捕获），行为不变。
        AstStmt::Raise(e) => Stmt::Raise {
            value: convert_expr(e, ctx),
        },

        AstStmt::Guard {
            cond,
            let_binding,
            else_body,
            ..
        } => {
            // guard → if let ... else
            if let Some((pattern, value)) = let_binding {
                let val = convert_expr(value, ctx);
                if let AstPattern::Ident(name) = pattern {
                    let mut guard_ctx = TypeCtx::new();
                    guard_ctx.current_generics = ctx.current_generics.clone();
                    guard_ctx.add_var(name, val.ty.clone());
                    Stmt::If {
                        cond: Expr::new(
                            ExprKind::BinOp {
                                op: BinOpKind::Neq,
                                lhs: Box::new(val),
                                rhs: Box::new(Expr::new(
                                    ExprKind::Lit(LitKind::None_),
                                    IrType::Any,
                                    Span::unknown(),
                                )),
                            },
                            IrType::Bool,
                            Span::unknown(),
                        ),
                        then_branch: Block {
                            span: Span::unknown(),
                            stmts: vec![],
                            ty: IrType::Unit,
                        },
                        else_branch: Some(convert_block(else_body, &guard_ctx)),
                    }
                } else {
                    Stmt::Block {
                        stmts: else_body.iter().map(|s| convert_stmt(s, ctx)).collect(),
                    }
                }
            } else {
                // guard cond else: <body> —— 失败路径（05-控制流.md §7.1）：
                // 块尾表达式为隐式 return（中止并返回该值）
                let mut else_block = convert_block(else_body, ctx);
                let tail_is_value_expr = matches!(
                    else_block.stmts.last(),
                    Some(Stmt::ExprStmt { expr }) if expr.ty != IrType::Unit
                );
                if tail_is_value_expr {
                    if let Some(last) = else_block.stmts.last_mut() {
                        if let Stmt::ExprStmt { expr } = last {
                            *last = Stmt::Return {
                                value: Some(expr.clone()),
                            };
                        }
                    }
                }
                Stmt::If {
                    cond: cond
                        .as_ref()
                        .map(|c| convert_expr(c, ctx))
                        .unwrap_or(Expr::new(
                            ExprKind::Lit(LitKind::Bool(true)),
                            IrType::Bool,
                            Span::unknown(),
                        )),
                    then_branch: Block {
                        span: Span::unknown(),
                        stmts: vec![],
                        ty: IrType::Unit,
                    },
                    else_branch: Some(else_block),
                }
            }
        }

        AstStmt::With { expr, alias, body } => {
            // with → 构造链展开（06d §十七）：
            //   let mut raw = <expr>
            //   let alias = raw.__enter__()   （定义了 __enter__ 时，别名绑定其返回值）
            //   <body>
            //   alias.__exit__(...)           （定义了 __exit__ 时，体后清理）
            // 旧实现 __exit__ 在体**前**调用且从不调 __enter__（构造链断裂）；
            // with <普通表达式>:（无 as 绑定）无 enter/exit 语义，直接执行块
            let val = convert_expr(expr, ctx);
            let val_ty = val.ty.clone();
            let mut with_ctx = TypeCtx::new();
            with_ctx.current_generics = ctx.current_generics.clone();
            let name = alias.clone().unwrap_or_else(|| "_with".into());
            // 是否定义了 __enter__/__exit__（未定义则跳过对应调用，避免 E0599）
            let (has_enter, has_exit) = match &val_ty {
                IrType::Named { path, .. } => {
                    let ms = ctx.struct_methods.get(path);
                    (
                        ms.map(|m| m.contains("__enter__")).unwrap_or(false),
                        ms.map(|m| m.contains("__exit__")).unwrap_or(false),
                    )
                }
                _ => (false, false),
            };
            let mut stmts: Vec<Stmt> = Vec::new();
            if alias.is_some() && has_enter {
                // let mut __with_ctx = val; let alias = __with_ctx.__enter__()
                let ctx_name = format!("__with_{}", name);
                with_ctx.add_var(&ctx_name, val_ty.clone());
                stmts.push(Stmt::Let {
                    name: ctx_name.clone(),
                    ty: val_ty.clone(),
                    value: val,
                    is_mut: true,
                    is_ref: false,
                    mods: IrMods::default(),
                });
                let enter_ret_ty = match &val_ty {
                    IrType::Named { path, .. } => ctx
                        .lookup_fn_return(&format!("{}.{}", path, "__enter__"))
                        .clone(),
                    _ => val_ty.clone(),
                };
                let enter_ret_ty = if matches!(enter_ret_ty, IrType::Unit | IrType::Any) {
                    val_ty.clone()
                } else {
                    enter_ret_ty
                };
                with_ctx.add_var(&name, enter_ret_ty.clone());
                stmts.push(Stmt::Let {
                    name: name.clone(),
                    ty: enter_ret_ty.clone(),
                    value: Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(Expr::new(
                                ExprKind::Var(ctx_name),
                                val_ty.clone(),
                                Span::unknown(),
                            )),
                            method: "__enter__".into(),
                            args: vec![],
                        },
                        enter_ret_ty.clone(),
                        Span::unknown(),
                    ),
                    is_mut: true,
                    is_ref: false,
                    mods: IrMods::default(),
                });
            } else {
                with_ctx.add_var(&name, val_ty.clone());
                stmts.push(Stmt::Let {
                    name: name.clone(),
                    ty: val_ty.clone(),
                    value: val,
                    // with 资源绑定须可变：块内可被 __exit__/__enter__ 等可变借用
                    // （生成 `let mut res`，否则 E0596 cannot borrow as mutable）
                    is_mut: true,
                    is_ref: false,
                    mods: IrMods::default(),
                });
            }
            // 捕获 with 体的值：把体包成块表达式绑定到临时变量，__exit__() 作为
            // 非尾副作用语句（自动带 `;`），最后返回临时变量。否则 __exit__() 会占据
            // 块尾、丢弃体值，导致 with 作为值表达式（如函数尾值 / 赋值右值）时返回 ()。
            // 用 IrType::Any 让 codegen 跳过类型标注（Rust 自动推断临时变量类型）。
            let body_block = Block {
                stmts: body.iter().map(|s| convert_stmt(s, &with_ctx)).collect(),
                ty: IrType::Any,
                span: Span::unknown(),
            };
            let body_val_name = format!("__with_val_{}", name);
            stmts.push(Stmt::Let {
                name: body_val_name.clone(),
                ty: IrType::Any,
                value: Expr::new(
                    ExprKind::BlockExpr { block: body_block },
                    IrType::Any,
                    Span::unknown(),
                ),
                is_mut: false,
                is_ref: false,
                mods: IrMods::default(),
            });
            // __exit__ 在体**后**调用（构造链收尾）；参数数 = 方法实际非 self 参数数
            if alias.is_some() && has_exit {
                let exit_arity = match &val_ty {
                    IrType::Named { path, .. } => ctx
                        .struct_method_arity
                        .get(path)
                        .and_then(|m| m.get("__exit__"))
                        .copied()
                        .unwrap_or(0),
                    _ => 0,
                };
                let exit_args: Vec<Expr> = if exit_arity > 0 {
                    vec![Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(Expr::new(
                                ExprKind::Var(name.clone()),
                                val_ty.clone(),
                                Span::unknown(),
                            )),
                            method: "clone".into(),
                            args: vec![],
                        },
                        val_ty.clone(),
                        Span::unknown(),
                    )]
                } else {
                    vec![]
                };
                stmts.push(Stmt::ExprStmt {
                    expr: Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(Expr::new(
                                ExprKind::Var(name.clone()),
                                val_ty.clone(),
                                Span::unknown(),
                            )),
                            method: "__exit__".into(),
                            args: exit_args,
                        },
                        IrType::Unit,
                        Span::unknown(),
                    ),
                });
            }
            // with 块尾返回体值（__exit__ 已在上方作为非尾语句，返回值被 `;` 丢弃）
            stmts.push(Stmt::ExprStmt {
                expr: Expr::new(ExprKind::Var(body_val_name), IrType::Any, Span::unknown()),
            });
            // with 整体表现为块表达式语句（Stmt::ExprStmt{BlockExpr}）而非 Stmt::Block：
            // 当 with 是含 defer 函数的最后一条语句时，gen_block_inner 的 capture_tail
            // 仅对「末语句为 Stmt::ExprStmt」触发；若 with 为 Stmt::Block 则 capture_tail
            // 不触发、force_stmt_semicolon 补 `;`、函数落尾返回 ()（E0308 家族）。
            // BlockExpr codegen 用全新子 CodeGen（deferred/force_stmt_semicolon 重置），
            // 不污染函数级 deferred；capture_tail 据此正确生效：先求值块（体值 + __exit__），
            // 再 flush 函数 defer，最后 return 捕获值。候选脚手架代码沿用同一 IR 形态。
            Stmt::ExprStmt {
                expr: Expr::new(
                    ExprKind::BlockExpr {
                        block: Block {
                            stmts,
                            ty: IrType::Any,
                            span: Span::unknown(),
                        },
                    },
                    IrType::Any,
                    Span::unknown(),
                ),
            }
        }

        AstStmt::Assign { target, op, value } => {
            let val = convert_expr(value, ctx);
            let target_expr = convert_expr(target, ctx);
            match op {
                crate::ast::AssignOp::Eq => Stmt::Assign {
                    target: target_expr,
                    value: val,
                },
                _ => {
                    // 目标是用户 struct 且定义了对应就地魔术方法（如 __iadd__）→
                    // 生成魔术方法调用而非脱糖 a = a + b（只定义 __iadd__ 无
                    // __add__ 的类型脱糖后 E0369，06d §四）
                    if let Some(magic) = assign_op_magic(op) {
                        if let IrType::Named { path, .. } = &target_expr.ty {
                            if ctx
                                .struct_methods
                                .get(path)
                                .map(|ms| ms.contains(magic))
                                .unwrap_or(false)
                            {
                                return Stmt::ExprStmt {
                                    expr: Expr::new(
                                        ExprKind::MethodCall {
                                            receiver: Box::new(target_expr),
                                            method: magic.to_string(),
                                            args: vec![val],
                                        },
                                        IrType::Unit,
                                        Span::unknown(),
                                    ),
                                };
                            }
                        }
                    }
                    Stmt::Assign {
                        target: target_expr.clone(),
                        value: Expr::new(
                            ExprKind::BinOp {
                                op: map_assign_op(op),
                                lhs: Box::new(target_expr),
                                rhs: Box::new(val),
                            },
                            IrType::Any,
                            Span::unknown(),
                        ),
                    }
                }
            }
        }

        AstStmt::FnDef { func } => {
            let mut declared: HashSet<String> =
                func.params.iter().map(|p| p.name.clone()).collect();
            // 排除 def 自身名：自引用（递归）不视为「捕获外层局部」，
            // 否则递归嵌套函数会被误转闭包（闭包无法自递归 → E0425/E0391）。
            declared.insert(func.name.clone());
            // 写捕获（仅 Assign / 无前缀可变绑定写外层变量）→ 用于错误提示（E0425）
            let captured = check_stmts_capture(&func.body, &ctx.vars, &mut declared, false);
            // 任意捕获（含读外层变量）→ 触发闭包路径（IR-003：读外层变量也需捕获）
            let mut declared_any = declared.clone();
            let captured_any = check_stmts_capture(&func.body, &ctx.vars, &mut declared_any, true);
            // IR-003 修复：位于某函数体内（current_fn_name.is_some()）且捕获外层局部
            // 变量的嵌套 def → 转为本地闭包 `let name = |params| body`，由闭包按
            // move/借用捕获外层变量（原提升为模块级会丢失外层局部 → E0425）。
            // 仅非变参、非模块级时走闭包路径；其余保持原提升逻辑（零回归）。
            if ctx.current_fn_name.is_some()
                && captured_any.is_some()
                && matches!(func.variadic, ast::VariadicMode::None)
            {
                let is_math = func.decorators.iter().any(|d| d.name == "math");
                let generics: Vec<String> = if is_math {
                    vec!["T".to_string()]
                } else {
                    let mut g = ctx.fn_generics(func);
                    for gg in &ctx.current_generics {
                        if !g.contains(gg) {
                            g.push(gg.clone());
                        }
                    }
                    g
                };
                let params: Vec<Param> = func
                    .params
                    .iter()
                    .map(|p| Param {
                        name: p.name.clone(),
                        ty: from_ast_type_with_generics(&p.ty, &generics),
                        is_mut: p.is_mut,
                        is_ref: p.is_ref,
                        is_owned: p.is_owned,
                        default: p.default.as_ref().map(|d| convert_expr(d, ctx)),
                        variadic: false,
                        comptime: false,
                        mods: IrMods::from_ast(&p.mods),
                    })
                    .collect();
                let ret_ir = TypeCtx::fn_return_ir(&func.return_type, &func.raises, &generics);
                let body_block = convert_block(&func.body, ctx);
                let body_expr = Expr::new(
                    ExprKind::BlockExpr { block: body_block },
                    ret_ir.clone(),
                    Span::unknown(),
                );
                let fn_ty = IrType::Fn {
                    params: params.iter().map(|p| p.ty.clone()).collect(),
                    ret: Box::new(ret_ir.clone()),
                };
                let lambda = Expr::new(
                    ExprKind::Lambda {
                        params,
                        body: Box::new(body_expr),
                        is_move: true,
                        ret_ty: Some(ret_ir),
                    },
                    fn_ty.clone(),
                    Span::unknown(),
                );
                Stmt::Let {
                    name: func.name.clone(),
                    ty: fn_ty,
                    value: lambda,
                    is_mut: false,
                    is_ref: false,
                    mods: IrMods::default(),
                }
            } else {
                // 非捕获 / 模块级 def：提升为模块级 Item::FnDef（原逻辑，零回归）
                if let Some(captured) = captured {
                    ctx.report_error(format!(
                        "嵌套函数 `{}` 引用了外层局部变量 `{}`：嵌套函数提升为模块级后无法访问外层局部变量（E0425），请改用闭包捕获（如 `let {} = |...| ...`）",
                        func.name, captured, func.name
                    ));
                }
                // 嵌套函数提升为模块级 Item::FnDef
                let nested_name = func.name.clone();
                let mut nested_def = convert_fn_def(func, ctx);
                nested_def.name = nested_name;
                ctx.pending_items.borrow_mut().push(Item::FnDef(nested_def));

                // 占位语句（嵌套函数不作为语句，已在模块级注册）
                Stmt::ExprStmt {
                    expr: Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown()),
                }
            }
        }

        AstStmt::EnumDef(struct_def) => {
            // 函数体内的 enum 定义提升为模块级 Item
            let item = convert_struct(&struct_def, ctx);
            ctx.pending_items.borrow_mut().push(item);

            // 占位语句
            Stmt::ExprStmt {
                expr: Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown()),
            }
        }

        AstStmt::Test { name: _, body } => {
            // test 块体用 convert_block（可变 block_ctx 前向传播 let 变量类型）：
            // convert_stmt 的 ctx 不可变，let 变量不登记，后续 `assert a == b` 中
            // a 推断为 Any（box.lz E0369 assert_eq 需 PartialEq）
            let blk = convert_block(body, ctx);
            Stmt::Block { stmts: blk.stmts }
        }

        AstStmt::Assert {
            expr,
            expected,
            message,
        } => {
            // assert cond, "msg" → 消息形式（规范 SYNTAX/15 §六）：整体 expr 作条件，
            // 生成 IR Stmt::Assert { cond, message }，由 Rust codegen 输出
            // assert!(cond, "{:?}", msg)。与相等形式（assert_eq!）互斥。
            if let Some(msg) = message {
                return Stmt::Assert {
                    cond: convert_expr(expr, ctx),
                    message: Some(convert_expr(msg, ctx)),
                };
            }
            // assert expr == expected → assert_eq!(expr, expected)
            // assert expr（单表达式布尔断言）→ assert!(expr)
            // （否则 assert_eq! 只有单参数 → Rust 宏 "unexpected end of macro invocation"）
            let mut args = vec![convert_expr(expr, ctx)];
            let callee_name = if let Some(exp) = expected {
                // 用户自定义 struct 比较：assert_eq! 要求 PartialEq（box.lz 的
                // Box/Rc/Arc 只定义 __eq__ 魔术方法 → E0369），改用
                // assert!(lhs.__eq__(rhs)) 调用自定义魔术方法
                let ir_expr = args[0].clone();
                // `assert b.get() == 42`：b.get() 返回 `ref T`（&T），与 owned 值
                // 比较需解引用（&i64 == i64 无实现，E0277 can't compare）
                let lhs_is_ref = matches!(ir_expr.ty, IrType::Ref(_) | IrType::MutRef(_));
                let is_user_struct = matches!(&ir_expr.ty, IrType::Named { path, .. }
                    if (ctx.struct_names.contains(path.as_str())
                        || ctx.enum_variants.values().any(|e| e == path.as_str()))
                        && !["List", "Dict", "Set", "Option", "Result", "String"]
                            .contains(&path.as_str()));
                // 用户自定义类型须实际定义了 __eq__ 才走魔法方法路径；
                // 否则回退到 assert_eq!（依赖 Rust PartialEq，枚举默认 derive）
                let has_eq_magic = is_user_struct && {
                    let ty_name = match &ir_expr.ty {
                        IrType::Named { path, .. } => path.clone(),
                        _ => String::new(),
                    };
                    ctx.struct_methods
                        .get(&ty_name)
                        .map(|ms| ms.contains("__eq__"))
                        .unwrap_or(false)
                };
                if has_eq_magic {
                    // parser 把 `assert a != c` 拆成 expected=Not(c)：
                    // 若 struct 未定义 __ne__（box.lz 只有 __eq__），生成 !a.__eq__(&c)
                    let is_ne = matches!(
                        exp,
                        AstExpr::Unary {
                            op: UnaryOp::Not,
                            ..
                        }
                    );
                    let ne_operand = if let AstExpr::Unary { operand, .. } = exp {
                        (**operand).clone()
                    } else {
                        exp.clone()
                    };
                    let has_ne = ctx
                        .struct_methods
                        .get(&match &ir_expr.ty {
                            IrType::Named { path, .. } => path.clone(),
                            _ => String::new(),
                        })
                        .map(|ms| ms.contains("__ne__"))
                        .unwrap_or(false);
                    if is_ne && !has_ne {
                        // !a.__eq__(&c)：调用 __eq__ 后取反
                        args = vec![Expr::new(
                            ExprKind::UnOp {
                                op: UnOpKind::Not,
                                operand: Box::new(Expr::new(
                                    ExprKind::MethodCall {
                                        receiver: Box::new(ir_expr),
                                        method: "__eq__".into(),
                                        args: vec![Expr::new(
                                            ExprKind::UnOp {
                                                op: UnOpKind::Ref,
                                                operand: Box::new(convert_expr(&ne_operand, ctx)),
                                            },
                                            IrType::Any,
                                            Span::unknown(),
                                        )],
                                    },
                                    IrType::Bool,
                                    Span::unknown(),
                                )),
                            },
                            IrType::Bool,
                            Span::unknown(),
                        )];
                    } else {
                        let magic = if is_ne { "__ne__" } else { "__eq__" };
                        args = vec![Expr::new(
                            ExprKind::MethodCall {
                                receiver: Box::new(ir_expr),
                                method: magic.into(),
                                // __eq__/__ne__ 签名是 `fn __eq__(&self, other: &Self)`：
                                // 参数应为引用 `&rhs`（box.lz assert a == b → a.__eq__(&b)，
                                // 传 owned 值会 E0308 expected &Box, found Box）
                                args: vec![Expr::new(
                                    ExprKind::UnOp {
                                        op: UnOpKind::Ref,
                                        operand: Box::new(convert_expr(&ne_operand, ctx)),
                                    },
                                    IrType::Any,
                                    Span::unknown(),
                                )],
                            },
                            IrType::Bool,
                            Span::unknown(),
                        )];
                    }
                    "assert!"
                } else if lhs_is_ref {
                    // lhs 是引用（&T）：与 owned 值比较需解引用（*b.get() == 42），
                    // 但 rhs 也是引用时（rc1.get() == rc2.get()）保持引用比较
                    // （&Vec<i64> == &Vec<i64> 合法；*lhs vs &rhs 反而 E0277）
                    let rhs_is_ref = match exp {
                        AstExpr::MethodCall { .. } | AstExpr::Ident(_) => {
                            let rhs_ty = infer_expr_type(exp, ctx);
                            matches!(rhs_ty, IrType::Ref(_) | IrType::MutRef(_))
                        }
                        _ => false,
                    };
                    if rhs_is_ref {
                        args.push(convert_expr(exp, ctx));
                    } else {
                        args = vec![
                            Expr::new(
                                ExprKind::UnOp {
                                    op: UnOpKind::Deref,
                                    operand: Box::new(ir_expr),
                                },
                                IrType::Any,
                                Span::unknown(),
                            ),
                            convert_expr(exp, ctx),
                        ];
                    }
                    "assert_eq!"
                } else {
                    args.push(convert_expr(exp, ctx));
                    "assert_eq!"
                }
            } else {
                "assert!"
            };
            Stmt::ExprStmt {
                expr: Expr::new(
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(Expr::new(
                            ExprKind::Var(callee_name.into()),
                            IrType::Any,
                            Span::unknown(),
                        )),
                        args,
                    },
                    IrType::Unit,
                    Span::unknown(),
                ),
            }
        }

        AstStmt::Check { expr, message: _ } => {
            // check → 展开为 if !expr { eprintln!(...) }
            let cond = Expr::new(
                ExprKind::UnOp {
                    op: crate::ir::node::UnOpKind::Not,
                    operand: Box::new(convert_expr(expr, ctx)),
                },
                IrType::Bool,
                Span::unknown(),
            );
            let print_call = Expr::new(
                ExprKind::Call {
                    type_args: vec![],
                    callee: Box::new(Expr::new(
                        ExprKind::Var("eprintln!".into()),
                        IrType::Any,
                        Span::unknown(),
                    )),
                    args: vec![Expr::new(
                        ExprKind::Lit(LitKind::Str("CHECK failed".into())),
                        IrType::Str,
                        Span::unknown(),
                    )],
                },
                IrType::Unit,
                Span::unknown(),
            );
            Stmt::If {
                cond,
                then_branch: Block {
                    span: Span::unknown(),
                    stmts: vec![Stmt::ExprStmt { expr: print_call }],
                    ty: IrType::Unit,
                },
                else_branch: None,
            }
        }

        AstStmt::LetTuple { .. } => {
            // LetTuple 在 convert_stmts 中展开，不应到达此处
            Stmt::Pass
        }

        AstStmt::Suite {
            name: _,
            setup,
            teardown,
            tests,
        } => {
            // 将 setup 和 teardown 内联到每个 test 中
            let mut ir_tests = Vec::new();
            for t in tests {
                match t {
                    AstStmt::Test { name, body } => {
                        let mut combined = Vec::new();
                        if let Some(ref s) = setup {
                            combined.extend(s.iter().cloned());
                        }
                        combined.extend(body.iter().cloned());
                        if let Some(ref td) = teardown {
                            combined.extend(td.iter().cloned());
                        }
                        ir_tests.push(AstStmt::Test {
                            name: name.clone(),
                            body: combined,
                        });
                    }
                    _ => ir_tests.push(t.clone()),
                }
            }
            Stmt::Block {
                stmts: convert_stmts(&ir_tests, ctx),
            }
        }
        AstStmt::EmbedBlock { .. } => Stmt::Pass,
    }
}

/// 递归收集语句内所有 walrus `x := expr` 绑定（name → 推断类型）。
/// convert_expr 是 &TypeCtx 不可变借用，无法在 walrus 转换处 add_var
/// （builder.rs:3108 FIXME 的 scope issue）；改为在 convert_block 的
/// 前向传播中统一登记，语义与 Let/BuildBlock Var 前向传播一致。
pub(crate) fn collect_stmt_walrus(stmt: &AstStmt, ctx: &TypeCtx, out: &mut Vec<(String, IrType)>) {
    match stmt {
        AstStmt::Expr(e) => collect_expr_walrus(e, ctx, out),
        AstStmt::Let { value, .. } | AstStmt::Const { value, .. } => {
            collect_expr_walrus(value, ctx, out)
        }
        AstStmt::LetTuple { value, .. } => collect_expr_walrus(value, ctx, out),
        AstStmt::Return(Some(e)) | AstStmt::Yield(Some(e)) | AstStmt::Raise(e) => {
            collect_expr_walrus(e, ctx, out)
        }
        AstStmt::YieldFrom(e) | AstStmt::BlockCall { args: e, .. } => {
            collect_expr_walrus(e, ctx, out)
        }
        AstStmt::Break(Some(e)) | AstStmt::BreakLabel { value: Some(e), .. } => {
            collect_expr_walrus(e, ctx, out)
        }
        AstStmt::While { cond, guard, .. }
        | AstStmt::WhileLet {
            expr: cond, guard, ..
        } => {
            collect_expr_walrus(cond, ctx, out);
            if let Some(g) = guard {
                collect_expr_walrus(g, ctx, out);
            }
        }
        AstStmt::For { iter, guard, .. } => {
            collect_expr_walrus(iter, ctx, out);
            if let Some(g) = guard {
                collect_expr_walrus(g, ctx, out);
            }
        }
        AstStmt::Assign { target, value, .. } => {
            collect_expr_walrus(target, ctx, out);
            collect_expr_walrus(value, ctx, out);
        }
        AstStmt::Guard {
            cond,
            success_expr,
            let_binding,
            ..
        } => {
            if let Some(c) = cond {
                collect_expr_walrus(c, ctx, out);
            }
            if let Some(s) = success_expr {
                collect_expr_walrus(s, ctx, out);
            }
            if let Some((_, e)) = let_binding {
                collect_expr_walrus(e, ctx, out);
            }
        }
        AstStmt::With { expr, .. } => collect_expr_walrus(expr, ctx, out),
        AstStmt::Assert { expr, expected, .. } => {
            collect_expr_walrus(expr, ctx, out);
            if let Some(e) = expected {
                collect_expr_walrus(e, ctx, out);
            }
        }
        AstStmt::Check { expr, message } => {
            collect_expr_walrus(expr, ctx, out);
            if let Some(m) = message {
                collect_expr_walrus(m, ctx, out);
            }
        }
        // 嵌套语句体（Loop/Block/CheckerBlock/Defer/Test/Suite/Comptime/FnDef）：
        // 各自经独立的 convert_block 转换并自行登记，此处不递归避免作用域泄漏。
        _ => {}
    }
}

/// 递归扫描表达式树中的 walrus 绑定（跳过闭包体/块表达式/推导式作用域内绑定，
/// 那些属于独立词法块；推导式的 iter/output 表达式仍扫描，其中 walrus 作用于外层）。
pub(crate) fn collect_expr_walrus(e: &AstExpr, ctx: &TypeCtx, out: &mut Vec<(String, IrType)>) {
    match e {
        AstExpr::Walrus { target, value } => {
            if let AstExpr::Ident(name) = target.as_ref() {
                if !out.iter().any(|(n, _)| n == name) {
                    out.push((name.clone(), infer_expr_type(value, ctx)));
                }
            }
            collect_expr_walrus(target, ctx, out);
            collect_expr_walrus(value, ctx, out);
        }
        AstExpr::ListLit(elems) | AstExpr::SetLit(elems) | AstExpr::TupleLit(elems) => {
            for x in elems {
                collect_expr_walrus(x, ctx, out);
            }
        }
        AstExpr::DictLit(entries) => {
            for (k, v) in entries {
                collect_expr_walrus(k, ctx, out);
                collect_expr_walrus(v, ctx, out);
            }
        }
        AstExpr::Binary { left, right, .. } => {
            collect_expr_walrus(left, ctx, out);
            collect_expr_walrus(right, ctx, out);
        }
        AstExpr::Unary { operand, .. } => collect_expr_walrus(operand, ctx, out),
        AstExpr::Call { func, args, .. } => {
            collect_expr_walrus(func, ctx, out);
            for a in args {
                collect_expr_walrus(a, ctx, out);
            }
        }
        AstExpr::KwArg { value, .. } => collect_expr_walrus(value, ctx, out),
        AstExpr::MethodCall { receiver, args, .. } => {
            collect_expr_walrus(receiver, ctx, out);
            for a in args {
                collect_expr_walrus(a, ctx, out);
            }
        }
        AstExpr::FieldAccess { receiver, .. } | AstExpr::PathAccess { receiver, .. } => {
            collect_expr_walrus(receiver, ctx, out)
        }
        AstExpr::Index { receiver, index } => {
            collect_expr_walrus(receiver, ctx, out);
            collect_expr_walrus(index, ctx, out);
        }
        AstExpr::If {
            cond, elif_clauses, ..
        } => {
            collect_expr_walrus(cond, ctx, out);
            for (c, _) in elif_clauses {
                collect_expr_walrus(c, ctx, out);
            }
        }
        AstExpr::Match { expr, arms } => {
            collect_expr_walrus(expr, ctx, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_expr_walrus(g, ctx, out);
                }
            }
        }
        AstExpr::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_expr_walrus(s, ctx, out);
            }
            if let Some(e2) = end {
                collect_expr_walrus(e2, ctx, out);
            }
        }
        AstExpr::Pipe {
            receiver,
            callee,
            args,
        } => {
            collect_expr_walrus(receiver, ctx, out);
            collect_expr_walrus(callee, ctx, out);
            for a in args {
                collect_expr_walrus(a, ctx, out);
            }
        }
        AstExpr::SafeNav { receiver, .. } => collect_expr_walrus(receiver, ctx, out),
        AstExpr::Try(inner)
        | AstExpr::Spawn(inner)
        | AstExpr::Move(inner)
        | AstExpr::Panic(inner)
        | AstExpr::Await(inner)
        | AstExpr::Paren(inner)
        | AstExpr::Comptime(inner) => collect_expr_walrus(inner, ctx, out),
        AstExpr::NullCoalesce { left, right } => {
            collect_expr_walrus(left, ctx, out);
            collect_expr_walrus(right, ctx, out);
        }
        AstExpr::ListComprehension {
            output,
            iter,
            cond,
            extra_clauses,
            ..
        }
        | AstExpr::SetComprehension {
            elem: output,
            iter,
            cond,
            extra_clauses,
            ..
        } => {
            collect_expr_walrus(output, ctx, out);
            collect_expr_walrus(iter, ctx, out);
            if let Some(c) = cond {
                collect_expr_walrus(c, ctx, out);
            }
            for (_, i, c) in extra_clauses {
                collect_expr_walrus(i, ctx, out);
                if let Some(c) = c {
                    collect_expr_walrus(c, ctx, out);
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
            collect_expr_walrus(key, ctx, out);
            collect_expr_walrus(value, ctx, out);
            collect_expr_walrus(iter, ctx, out);
            if let Some(c) = cond {
                collect_expr_walrus(c, ctx, out);
            }
            for (_, i, c) in extra_clauses {
                collect_expr_walrus(i, ctx, out);
                if let Some(c) = c {
                    collect_expr_walrus(c, ctx, out);
                }
            }
        }
        AstExpr::Assign { target, value, .. } => {
            collect_expr_walrus(target, ctx, out);
            collect_expr_walrus(value, ctx, out);
        }
        // Closure/BlockExpr/BuildBlock/TryCatch：独立词法块，绑定不泄漏到外层
        _ => {}
    }
}

pub(crate) fn convert_block(stmts: &[AstStmt], ctx: &TypeCtx) -> Block {
    // 创建可变的本地上下文，支持 Let 变量传播
    let mut block_ctx = TypeCtx::new();
    // 继承父级上下文中的变量类型（支持 WhileLet 等模式绑定类型传递）
    block_ctx.vars = ctx.vars.clone();
    // 继承父级块首次声明集合（生成器预扫描 scan_iterator_yield_ty 已登记，
    // 否则 `let mut i = 0` 会被误转 Assign，生成裸赋值导致 E0425）
    block_ctx.block_declared = ctx.block_declared.clone();
    block_ctx.current_generics = ctx.current_generics.clone();
    block_ctx.current_ret_ty = ctx.current_ret_ty.clone();
    block_ctx.current_is_iterator = ctx.current_is_iterator;
    block_ctx.current_fn_name = ctx.current_fn_name.clone();
    block_ctx.pending_items = ctx.pending_items.clone();
    block_ctx.errors = ctx.errors.clone();
    block_ctx.comptime_consts = ctx.comptime_consts.clone();
    block_ctx.mutated_top_level_consts = ctx.mutated_top_level_consts.clone();
    block_ctx.comptime_module = ctx.comptime_module.clone();
    for sn in &ctx.struct_names {
        block_ctx.struct_names.insert(sn.clone());
    }
    for (sn, fields) in &ctx.struct_fields {
        let mut cloned = HashMap::new();
        for (fn_, ty) in fields {
            cloned.insert(fn_.clone(), ty.clone());
        }
        block_ctx.struct_fields.insert(sn.clone(), cloned);
    }
    for (sn, order) in &ctx.struct_field_order {
        block_ctx
            .struct_field_order
            .insert(sn.clone(), order.clone());
    }
    for (sn, ms) in &ctx.struct_methods {
        block_ctx.struct_methods.insert(sn.clone(), ms.clone());
    }
    for (sn, arity) in &ctx.struct_method_arity {
        block_ctx
            .struct_method_arity
            .insert(sn.clone(), arity.clone());
    }
    for (vn, vt) in &ctx.vars {
        block_ctx.vars.insert(vn.clone(), vt.clone());
    }
    for (name, ty) in &ctx.fn_returns {
        block_ctx.fn_returns.insert(name.clone(), ty.clone());
    }
    for (name, ty) in &ctx.fn_raises {
        block_ctx.fn_raises.insert(name.clone(), ty.clone());
    }
    for (name, p) in &ctx.fn_params {
        block_ctx.fn_params.insert(name.clone(), p.clone());
    }
    for (vn, en) in &ctx.enum_variants {
        block_ctx.enum_variants.insert(vn.clone(), en.clone());
    }
    for (vn, ft) in &ctx.enum_variant_field_types {
        block_ctx
            .enum_variant_field_types
            .insert(vn.clone(), ft.clone());
    }
    for (cn, ct) in &ctx.top_level_consts {
        block_ctx.top_level_consts.insert(cn.clone(), ct.clone());
    }

    let mut ir_stmts: Vec<Stmt> = Vec::new();
    for s in stmts {
        // 前向传播：Let 语句的变量添加到后续语句的上下文
        if let AstStmt::Let {
            name, ty, value, ..
        } = s
        {
            // 本块首次声明的变量：登记到 block_declared（区分首次绑定与重新赋值）。
            // 顶层变量（size = 3 等顶层 Assign 转 Const）不属于本块首次声明：
            // 否则 `size = size - 1`（无 let 前缀可变绑定）被误判为本块绑定走
            // Let 分支，生成局部新绑定而非修改全局（guard_for_3.lz while 死循环）
            if !block_ctx.vars.contains_key(name.as_str())
                && !block_ctx.top_level_consts.contains_key(name.as_str())
            {
                block_ctx.block_declared.insert(name.clone());
            }
            let ir_ty = ty
                .as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or_else(|| infer_expr_type(value, &block_ctx));
            block_ctx.add_var(name, ir_ty);
        }
        // 前向传播：函数体内的嵌套 def（IR-003 会转为本地闭包 let）也需登记其函数
        // 类型，否则后续 `return inner` / `inner(...)` 查 lookup_var 回退 Any→i64，
        // 触发错误的 ImplicitConvert（E0277）或调用推断失败。
        if let AstStmt::FnDef { func } = s {
            let mut g = block_ctx.fn_generics(func);
            for gg in &block_ctx.current_generics {
                if !g.contains(gg) {
                    g.push(gg.clone());
                }
            }
            let fparams: Vec<IrType> = func
                .params
                .iter()
                .map(|p| from_ast_type_with_generics(&p.ty, &g))
                .collect();
            let fret = func
                .return_type
                .as_ref()
                .map(|t| from_ast_type_with_generics(t, &g))
                .unwrap_or(IrType::Any);
            let fty = IrType::Fn {
                params: fparams,
                ret: Box::new(fret),
            };
            if !block_ctx.vars.contains_key(func.name.as_str())
                && !block_ctx.top_level_consts.contains_key(func.name.as_str())
            {
                block_ctx.block_declared.insert(func.name.clone());
            }
            block_ctx.add_var(&func.name, fty);
        }
        // 前向传播：`x =: <构建块>`（AstStmt::Expr 中的 BuildBlock Var）也登记变量，
        // 否则后续 `multiply ~: factors` 的元组拆包查 lookup_var 返回 Any（E0061）
        if let AstStmt::Expr(AstExpr::BuildBlock {
            kind: BuildKind::Var,
            lhs,
            body,
        }) = s
        {
            if let AstExpr::Ident(name) = &**lhs {
                if !block_ctx.vars.contains_key(name.as_str()) {
                    block_ctx.block_declared.insert(name.clone());
                }
                let has_bare_return = body.iter().any(|st| ast_stmt_has_bare_return(st));
                let ir_ty = if has_bare_return {
                    IrType::Unit
                } else {
                    body.last()
                        .map(|st| infer_stmt_type(st, &block_ctx))
                        .filter(|t| !matches!(t, IrType::Any))
                        .unwrap_or(IrType::Any)
                };
                block_ctx.add_var(name, ir_ty);
            }
        }
        if let AstStmt::LetTuple { names, ty, .. } = s {
            let ir_ty = ty.as_ref().map(|t| from_ast_type(t)).unwrap_or(IrType::Any);
            for name in names {
                if name != "_" {
                    block_ctx.add_var(name, ir_ty.clone());
                }
            }
        }
        // 前向传播：`x := expr`（walrus）在表达式内绑定变量，登记类型供
        // 后续语句引用（builder.rs:3108 FIXME：convert_expr 是 &TypeCtx
        // 不可变借用，无法在 walrus 转换处 add_var；此处仿照 Let 前向传播
        // 统一登记，使 `if (n := f()) > 0:` 的 then 分支 / 后续语句
        // 能 lookup_var(n) 拿到真实类型而非 Any）
        let mut walrus_binds = Vec::new();
        collect_stmt_walrus(s, &block_ctx, &mut walrus_binds);
        for (wname, wty) in walrus_binds {
            if !block_ctx.vars.contains_key(&wname)
                && !block_ctx.top_level_consts.contains_key(&wname)
            {
                block_ctx.block_declared.insert(wname.clone());
            }
            block_ctx.add_var(&wname, wty);
        }
        // guard let <Variant>(...) = expr else: ... → match value { Variant(r) => { 剩余语句 }, _ => { else_body } }
        if let AstStmt::Guard {
            let_binding,
            else_body,
            ..
        } = s
        {
            if let Some((pat, value)) = let_binding {
                if let AstPattern::Variant(_, args) = pat {
                    let val = convert_expr(value, &block_ctx);
                    // 剩余语句作为匹配分支的 body（guard 之后代码仅在匹配时执行）
                    let rest: Vec<AstStmt> = stmts
                        .iter()
                        .skip_while(|x| !std::ptr::eq(*x, s))
                        .skip(1)
                        .cloned()
                        .collect();
                    // 匹配分支上下文：绑定模式变量（类型 Any 占位，body 转换时按需解析）
                    let mut then_ctx = block_ctx.clone();
                    fn collect_pat_vars(pat: &AstPattern, ctx: &mut TypeCtx) {
                        match pat {
                            AstPattern::Ident(name) => {
                                ctx.add_var(name, IrType::Any);
                            }
                            AstPattern::Variant(_, as_) => {
                                for a in as_ {
                                    collect_pat_vars(a, ctx);
                                }
                            }
                            _ => {}
                        }
                    }
                    collect_pat_vars(
                        &AstPattern::Variant(String::new(), args.clone()),
                        &mut then_ctx,
                    );
                    let then_block = convert_block(&rest, &then_ctx);
                    let else_block = convert_block(else_body, &block_ctx);
                    // 构建 match 结构：Variant(...) 分支 + 默认分支
                    let ir_pat = convert_ast_pattern(pat, &block_ctx).unwrap_or(Pattern::Wildcard);
                    let match_stmt = Stmt::Match {
                        scrutinee: val,
                        arms: vec![
                            MatchArm {
                                pattern: ir_pat,
                                guard: None,
                                body: then_block,
                            },
                            MatchArm {
                                pattern: Pattern::Wildcard,
                                guard: None,
                                body: else_block,
                            },
                        ],
                    };
                    ir_stmts.push(match_stmt);
                    break;
                }
            }
        }
        ir_stmts.extend(convert_stmts(std::slice::from_ref(s), &block_ctx));
    }

    // 后处理：递归收集所有被赋值的变量名，标记对应首次 let 为 mut
    fn collect_reassigned(stmts: &[Stmt]) -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        for s in stmts {
            match s {
                Stmt::Assign { target, .. } => {
                    if let ExprKind::Var(name) = &target.kind {
                        set.insert(name.clone());
                    }
                }
                Stmt::Let { name, is_mut, .. } if *is_mut => {
                    set.insert(name.clone());
                }
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    set.extend(collect_reassigned(&then_branch.stmts));
                    if let Some(eb) = else_branch {
                        set.extend(collect_reassigned(&eb.stmts));
                    }
                }
                Stmt::While { body, .. } | Stmt::For { body, .. } => {
                    set.extend(collect_reassigned(&body.stmts));
                }
                Stmt::Match { arms, .. } => {
                    for arm in arms {
                        set.extend(collect_reassigned(&arm.body.stmts));
                    }
                }
                Stmt::TryCatch {
                    body,
                    catches,
                    else_body,
                    finally_body,
                } => {
                    set.extend(collect_reassigned(&body.stmts));
                    for (_, catch_body) in catches {
                        set.extend(collect_reassigned(&catch_body.stmts));
                    }
                    if let Some(eb) = else_body {
                        set.extend(collect_reassigned(&eb.stmts));
                    }
                    if let Some(fb) = finally_body {
                        set.extend(collect_reassigned(&fb.stmts));
                    }
                }
                _ => {}
            }
        }
        set
    }

    // 空列表字面量元素类型推断（支持 append/push 上下文推断，贴近 Rust）
    let empty_elems = resolve_empty_list_elems(&ir_stmts);

    // 不可变 `let` 重赋值 → E0384 错误（移除原本自动提升为 mut 的行为，贴近 Rust 语义）
    let reassigned = collect_reassigned(&ir_stmts);
    for s in &ir_stmts {
        if let Stmt::Let { name, is_mut, .. } = s {
            if reassigned.contains(name.as_str()) && !*is_mut {
                ctx.report_error(format!(
                    "error[E0384]: cannot assign twice to immutable variable `{}`\n  = help: change `let {}` to `let mut {}` if you intend to reassign it",
                    name, name, name
                ));
            }
        }
    }

    // 将推断出的空列表元素类型应用到对应 let；无法推断则报 E0282 错误
    for s in &mut ir_stmts {
        if let Stmt::Let {
            name, ty, value, ..
        } = s
        {
            if let Some(elem) = empty_elems.get(name.as_str()) {
                *ty = IrType::Named {
                    path: "List".to_string(),
                    args: vec![elem.clone()],
                };
            } else if let IrType::Named { path, args } = ty {
                if path == "List" && args.len() == 1 && matches!(args[0], IrType::Any) {
                    if let ExprKind::ListLit(items) = &value.kind {
                        if items.is_empty() {
                            ctx.report_error(format!(
                                "error[E0282]: type annotations needed\n  = cannot infer element type for empty list bound to `{}`\n  = help: give it an explicit type, e.g. `let {}: List<T> = []`",
                                name, name
                            ));
                        }
                    }
                }
            }
        }
    }

    // 递归收集空列表字面量，并通过 append/push 调用推断其元素类型（贴近 Rust 的上下文推断）
    fn resolve_empty_list_elems(stmts: &[Stmt]) -> std::collections::HashMap<String, IrType> {
        let mut empty_lets: std::collections::HashSet<String> = std::collections::HashSet::new();
        for s in stmts {
            if let Stmt::Let {
                name, ty, value, ..
            } = s
            {
                if let IrType::Named { path, args } = ty {
                    if path == "List" && args.len() == 1 && matches!(args[0], IrType::Any) {
                        if let ExprKind::ListLit(items) = &value.kind {
                            if items.is_empty() {
                                empty_lets.insert(name.clone());
                            }
                        }
                    }
                }
            }
        }
        let mut resolved: std::collections::HashMap<String, IrType> =
            std::collections::HashMap::new();
        fn scan(
            stmts: &[Stmt],
            empty_lets: &std::collections::HashSet<String>,
            resolved: &mut std::collections::HashMap<String, IrType>,
        ) {
            for s in stmts {
                match s {
                    Stmt::ExprStmt { expr: e } => {
                        if let ExprKind::MethodCall {
                            receiver,
                            method,
                            args,
                        } = &e.kind
                        {
                            if method == "append" || method == "push" {
                                if let ExprKind::Var(name) = &receiver.kind {
                                    if empty_lets.contains(name) && !resolved.contains_key(name) {
                                        if let Some(first) = args.first() {
                                            if first.ty != IrType::Any {
                                                resolved.insert(name.clone(), first.ty.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Stmt::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        scan(&then_branch.stmts, empty_lets, resolved);
                        if let Some(eb) = else_branch {
                            scan(&eb.stmts, empty_lets, resolved);
                        }
                    }
                    Stmt::While { body, .. }
                    | Stmt::For { body, .. }
                    | Stmt::WhileLet { body, .. } => scan(&body.stmts, empty_lets, resolved),
                    Stmt::Match { arms, .. } => {
                        for a in arms {
                            scan(&a.body.stmts, empty_lets, resolved);
                        }
                    }
                    Stmt::TryCatch {
                        body,
                        catches,
                        else_body,
                        finally_body,
                    } => {
                        scan(&body.stmts, empty_lets, resolved);
                        for (_, cb) in catches {
                            scan(&cb.stmts, empty_lets, resolved);
                        }
                        if let Some(eb) = else_body {
                            scan(&eb.stmts, empty_lets, resolved);
                        }
                        if let Some(fb) = finally_body {
                            scan(&fb.stmts, empty_lets, resolved);
                        }
                    }
                    Stmt::Block { stmts: b } => scan(&b, empty_lets, resolved),
                    _ => {}
                }
            }
        }
        scan(stmts, &empty_lets, &mut resolved);
        resolved
    }

    let ty = stmts
        .last()
        .map(|s| infer_stmt_type(s, ctx))
        .unwrap_or(IrType::Unit);
    Block {
        span: Span::unknown(),
        stmts: ir_stmts,
        ty,
    }
}

pub(crate) fn convert_block_with_ctx(stmts: &[AstStmt], ctx: &TypeCtx) -> Block {
    convert_block(stmts, ctx)
}

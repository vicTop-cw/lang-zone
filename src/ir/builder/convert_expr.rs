// Lang-Zone 编译器 — ir/builder/convert_expr.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

pub(crate) fn convert_expr(ast_expr: &AstExpr, ctx: &TypeCtx) -> Expr {
    let ty = infer_expr_type(ast_expr, ctx);
    let span = Span::unknown();

    let kind = match ast_expr {
        AstExpr::IntLit(n) => ExprKind::Lit(LitKind::Int(*n)),
        AstExpr::Int128Lit(n) => ExprKind::Lit(LitKind::Int128(*n)),
        AstExpr::BigIntLit(s) => ExprKind::Lit(LitKind::BigInt(s.clone())),
        AstExpr::ComplexLit(re, im) => ExprKind::Lit(LitKind::Complex(*re, *im)),
        AstExpr::FloatLit(n) => ExprKind::Lit(LitKind::F64(*n)),
        AstExpr::StrLit(s) => ExprKind::Lit(LitKind::Str(s.clone())),
        AstExpr::FStrLit(s) => ExprKind::Lit(LitKind::FStr(s.clone())),
        AstExpr::RawStrLit(s) => ExprKind::Lit(LitKind::Str(s.clone())),
        AstExpr::BoolLit(b) => ExprKind::Lit(LitKind::Bool(*b)),
        AstExpr::NoneLit => ExprKind::Lit(LitKind::None_),
        AstExpr::DefaultExpr => ExprKind::Default,
        AstExpr::Ident(name) => {
            // comptime const 内联：顶层 `comptime const X = ...` 求值后，
            // 普通表达式中的 X 引用直接内联为字面量（而非运行时变量引用）
            if let Some(cv) = ctx.comptime_consts.get(name.as_str()) {
                // BUG-7：被重新赋值的顶层 const（可变全局，如 walrus.lz 的
                // `count`）不能内联为字面量，否则 `count += 1` 会变成
                // `0i64 = 0i64 + 1i64`。此类保留 Var 引用，交给 codegen 走
                // `mutated_consts` → `static mut` + `unsafe {}` 路径。
                // 作用域保护：若 name 在当前作用域是局部变量/形参（如泛型函数
                // collect_items 的形参 items 与模块级 `let items` 同名），必须
                // 保留 Var 引用，否则会把模块级字面量错误内联进函数体，破坏泛型
                // 并触发 E0308（operators.lz 的 collect_items、comptime_external_lib.lz）。
                if ctx.mutated_top_level_consts.contains(name.as_str())
                    || ctx.vars.contains_key(name.as_str())
                {
                    ExprKind::Var(name.clone())
                } else {
                    match comptime_value_to_lit(cv, ctx.top_level_consts.get(name.as_str())) {
                        Some(kind) => kind,
                        None => ExprKind::Var(name.clone()),
                    }
                }
            } else {
                ExprKind::Var(name.clone())
            }
        }
        AstExpr::Paren(inner) => ExprKind::Paren(Box::new(convert_expr(inner, ctx))),
        // 列表展开元素：透传为 IR Spread（codegen 在 ListLit 内降级为 extend 块）
        AstExpr::Spread(inner) => ExprKind::Spread(Box::new(convert_expr(inner, ctx))),
        // comptime 表达式：编译期求值，结果内联为字面量（B3）
        AstExpr::Comptime(inner) => {
            // 使用真实模块（comptime 可调用模块内函数/引用 const）
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
            // 注入顶层 const 求值结果（`comptime LIMIT / 2` 解析 const 引用）
            for (n, v) in &ctx.comptime_consts {
                cctx.symtab.insert(n.clone(), v.clone());
            }
            match crate::comptime::ComptimeEvaluator::eval_expr(inner, &mut cctx) {
                Ok(v) => match comptime_value_to_lit(&v, None) {
                    Some(kind) => kind,
                    None => ExprKind::Paren(Box::new(convert_expr(inner, ctx))),
                },
                Err(e) => {
                    ctx.errors
                        .borrow_mut()
                        .push(format!("comptime 求值失败: {}", e));
                    ExprKind::Paren(Box::new(convert_expr(inner, ctx)))
                }
            }
        }

        AstExpr::Call {
            func,
            args,
            type_args,
        } => {
            // M3：调用点特化状态（默认不特化；命中 comptime 全已知后改写调用）
            let mut specialized_callee: Option<String> = None;
            let mut comptime_arg_positions: Vec<usize> = Vec::new();
            // ── M2：comptime 形参的调用点编译期已知性校验 ──
            // 被调函数含 `comptime` 形参时，对应实参须为编译期已知值
            // （字面量 / const / comptime 结果）；否则记编译错误（指向调用点）。
            // 复用 comptime 表达式同款求值器：能 eval 成功即编译期已知。
            if let AstExpr::Ident(callee) = func.as_ref() {
                // 从模块 AST 查同名函数，取各形参的 comptime 标记
                let cmpt_flags: Option<Vec<bool>> = ctx
                    .comptime_module
                    .as_ref()
                    .and_then(|m| {
                        m.functions
                            .iter()
                            .find(|f| f.name.as_str() == callee.as_str())
                    })
                    .map(|f| f.params.iter().map(|p| p.comptime).collect());
                if let Some(flags) = cmpt_flags {
                    if flags.iter().any(|&c| c) {
                        let empty_module = ast::Module::default();
                        let module_ref = ctx
                            .comptime_module
                            .as_ref()
                            .map(|m| m.as_ref())
                            .unwrap_or(&empty_module);
                        for (i, arg) in args.iter().enumerate() {
                            if !flags.get(i).copied().unwrap_or(false) {
                                continue;
                            }
                            let arg_expr: &AstExpr = match arg {
                                AstExpr::KwArg { value, .. } => value.as_ref(),
                                other => other,
                            };
                            // 剥离 `comptime` 前缀：求值其内部表达式
                            let eval_target: &AstExpr = match arg_expr {
                                AstExpr::Comptime(inner) => &**inner,
                                other => other,
                            };
                            let mut cctx = crate::comptime::ComptimeContext::new(module_ref);
                            if let Some(src) = &module_ref.source_text {
                                cctx = cctx.with_source(src.clone());
                            }
                            for (n, v) in &ctx.comptime_consts {
                                cctx.symtab.insert(n.clone(), v.clone());
                            }
                            if let Err(e) = crate::comptime::ComptimeEvaluator::eval_expr(
                                eval_target,
                                &mut cctx,
                            ) {
                                ctx.report_error(format!(
                                    "comptime 形参 `{}` 的第 {} 个实参必须是编译期已知值（字面量 / const / comptime 结果）；求值失败：{}",
                                    callee,
                                    i + 1,
                                    e
                                ));
                            }
                        }
                    }
                }
            }
            // ── M3：comptime 形参实参全部编译期可知 → 生成特化副本并改写调用 ──
            // 仅自由函数调用（Ident callee）触发；部分应用（含 `_` 通配）跳过。
            if !args
                .iter()
                .any(|a| matches!(a, AstExpr::Ident(s) if s == "_"))
            {
                if let AstExpr::Ident(callee) = func.as_ref() {
                    let orig_opt = ctx.comptime_module.as_ref().and_then(|m| {
                        m.functions
                            .iter()
                            .find(|f| f.name.as_str() == callee.as_str() && !f.is_comptime)
                    });
                    if let Some(orig) = orig_opt {
                        let orig_clone = orig.clone();
                        let flags: Vec<bool> =
                            orig_clone.params.iter().map(|p| p.comptime).collect();
                        if flags.iter().any(|&c| c) {
                            // 求值阶段：持有对 comptime_module 的不可变借用，块结束即释放
                            let (subst, positions, spec_repr, all_known) = {
                                let empty_module = ast::Module::default();
                                let module_ref = ctx
                                    .comptime_module
                                    .as_ref()
                                    .map(|m| m.as_ref())
                                    .unwrap_or(&empty_module);
                                let mut cctx = crate::comptime::ComptimeContext::new(module_ref);
                                if let Some(src) = &module_ref.source_text {
                                    cctx = cctx.with_source(src.clone());
                                }
                                for (n, v) in &ctx.comptime_consts {
                                    cctx.symtab.insert(n.clone(), v.clone());
                                }
                                let mut subst: std::collections::HashMap<String, AstExpr> =
                                    std::collections::HashMap::new();
                                let mut positions: Vec<usize> = Vec::new();
                                let mut spec_repr = String::new();
                                let mut all_known = true;
                                for (i, p) in orig_clone.params.iter().enumerate() {
                                    if !p.comptime {
                                        continue;
                                    }
                                    positions.push(i);
                                    let arg_expr: &AstExpr = match args.get(i) {
                                        Some(a) => match a {
                                            AstExpr::KwArg { value, .. } => value.as_ref(),
                                            other => other,
                                        },
                                        None => {
                                            all_known = false;
                                            break;
                                        }
                                    };
                                    let eval_target: &AstExpr = match arg_expr {
                                        AstExpr::Comptime(inner) => &**inner,
                                        other => other,
                                    };
                                    match crate::comptime::ComptimeEvaluator::eval_expr(
                                        eval_target,
                                        &mut cctx,
                                    ) {
                                        Ok(v) => match comptime_value_to_ast(&v) {
                                            Some(ast_val) => {
                                                subst.insert(p.name.clone(), ast_val);
                                                spec_repr.push_str(&format!("_{:?}", v));
                                            }
                                            None => {
                                                all_known = false;
                                                break;
                                            }
                                        },
                                        Err(_) => {
                                            all_known = false;
                                            break;
                                        }
                                    }
                                }
                                (subst, positions, spec_repr, all_known)
                            };
                            if all_known && !positions.is_empty() {
                                let spec_key = format!("{}__spec{}", callee, spec_repr);
                                let mangled = format!(
                                    "{}__lzspec_{:016x}",
                                    callee,
                                    fnv1a_64(spec_key.as_bytes())
                                );
                                let already = ctx.specialization_set.borrow().contains(&mangled);
                                if !already {
                                    let spec_fn = build_specialized_fn(
                                        &orig_clone,
                                        &positions,
                                        &subst,
                                        &mangled,
                                    );
                                    // 用克隆 ctx 注册特化函数返回类型（不污染外层 ctx），
                                    // 使特化函数体内的自递归调用获得正确类型推断
                                    let mut spec_ctx = ctx.clone();
                                    if let Some(rt) = ctx.fn_returns.get(callee) {
                                        spec_ctx.fn_returns.insert(mangled.clone(), rt.clone());
                                    }
                                    let spec_item = convert_fn_def(&spec_fn, &spec_ctx);
                                    ctx.pending_items.borrow_mut().push(Item::FnDef(spec_item));
                                    ctx.specialization_set.borrow_mut().insert(mangled.clone());
                                }
                                specialized_callee = Some(mangled);
                                comptime_arg_positions = positions;
                            }
                        }
                    }
                }
            }
            // SafeNav 后接方法调用：config?.get("key") →
            // config.map(|__sn| __sn.get("key").copied())（field 与 args 合并进闭包体，
            // 否则生成 config.map(|__sn| __sn.get)("key")，E0615；
            // get 返回 Option<&V> 需 .copied() 转 Option<V>，否则 ?? 30 unwrap_or 类型不匹配）
            if let AstExpr::SafeNav { receiver, field } = func.as_ref() {
                let recv = convert_expr(receiver, ctx);
                let param = "__sn".to_string();
                let call_args: Vec<Expr> = args.iter().map(|a| convert_expr(a, ctx)).collect();
                let get_call = Expr::new(
                    ExprKind::MethodCall {
                        receiver: Box::new(Expr::new(
                            ExprKind::Var(param.clone()),
                            IrType::Any,
                            Span::unknown(),
                        )),
                        method: field.clone(),
                        args: call_args,
                    },
                    IrType::Any,
                    Span::unknown(),
                );
                // get/方法调用结果若是 Option<&V>（如 HashMap::get），需 .copied()；
                // 对一般方法（如 len() 返回 i64）不附加
                let body = if field == "get" {
                    Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(get_call),
                            method: "copied".into(),
                            args: vec![],
                        },
                        IrType::Any,
                        Span::unknown(),
                    )
                } else {
                    get_call
                };
                let lambda = Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: param,
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                            comptime: false,
                            mods: IrMods::default(),
                        }],
                        body: Box::new(body),
                        is_move: true,
                        ret_ty: None,
                    },
                    IrType::Any,
                    Span::unknown(),
                );
                return Expr::new(
                    ExprKind::MethodCall {
                        receiver: Box::new(recv),
                        // get 返回 Option → and_then 扁平化（map 会得 Option<Option<..>>，
                        // ?? 30 unwrap_or 类型不匹配 E0308）
                        method: "and_then".into(),
                        args: vec![lambda],
                    },
                    IrType::Any,
                    Span::unknown(),
                );
            }
            // 部分应用检测：如果 args 中包含 _ 占位符，展开为 Lambda
            // add(_, 1) → |x| add(x, 1)
            let has_wildcard = args
                .iter()
                .any(|a| matches!(a, AstExpr::Ident(s) if s == "_"));
            if has_wildcard {
                let mut param_idx = 0u32;
                let mut lambda_params: Vec<Param> = Vec::new();
                let mut filled_args: Vec<Expr> = Vec::new();
                for a in args.iter() {
                    if matches!(a, AstExpr::Ident(s) if s == "_") {
                        let param_name = format!("__p{}", param_idx);
                        param_idx += 1;
                        lambda_params.push(Param {
                            name: param_name.clone(),
                            ty: IrType::Any,
                            default: None,
                            variadic: false,
                            comptime: false,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            mods: IrMods::default(),
                        });
                        filled_args.push(Expr::new(
                            ExprKind::Var(param_name),
                            IrType::Any,
                            Span::unknown(),
                        ));
                    } else {
                        filled_args.push(convert_expr(a, ctx));
                    }
                }
                let callee = convert_expr(func, ctx);
                let call = Expr::new(
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(callee),
                        args: filled_args,
                    },
                    IrType::Any,
                    Span::unknown(),
                );
                return Expr::new(
                    ExprKind::Lambda {
                        params: lambda_params,
                        body: Box::new(call),
                        is_move: true,
                        ret_ty: None,
                    },
                    IrType::Fn {
                        params: vec![IrType::Any; param_idx as usize],
                        ret: Box::new(IrType::Any),
                    },
                    Span::unknown(),
                );
            }
            // 特殊处理 __as__ 运算符：__as__(value, type_name) → Cast
            if let AstExpr::Ident(ref fname) = func.as_ref() {
                if fname == "__as__" && args.len() == 2 {
                    let value = convert_expr(&args[0], ctx);
                    if let AstExpr::Ident(ref type_name) = &args[1] {
                        let target = name_to_ir_type(type_name);
                        let target_ty = target.clone();
                        // __cast__/__try_cast__ 双缺检查（01-类型系统.md §6）：
                        // 自定义类型（struct/enum/duck）执行 `x as T` 必须实现
                        // `__cast__<T>()` 或 `__try_cast__<T>() -> Result<T, E>`，
                        // 两者均未实现 → LZ 编译期报错（而非生成非法 Rust `as` 被
                        // rustc E0605 兜底）。基本数值类型间转换由编译器内置放行；
                        // 内置容器类型（List/Dict/Option/Result 等，不在
                        // struct_methods 中）的 as 是字面量类型标注，同样放行。
                        let needs_magic = !is_builtin_cast(&value.ty, &target);
                        if needs_magic {
                            if let IrType::Named { path, .. } = &value.ty {
                                // 仅用户自定义类型（有方法集合可查）要求魔法方法；
                                // 内置类型/未登记类型保守放行，避免误伤
                                // `[] as List<int>` / `None as Option<int>` 标注
                                if ctx.struct_methods.contains_key(path.as_str()) {
                                    let has_magic =
                                        ctx.struct_methods.get(path).map_or(false, |ms| {
                                            ms.contains("__cast__") || ms.contains("__try_cast__")
                                        });
                                    if !has_magic {
                                        ctx.report_error(format!(
                                            "类型 `{}` 未实现 `__cast__` 或 `__try_cast__`，无法执行 `as {}` 转换（01-类型系统.md §6）",
                                            path, type_name
                                        ));
                                    }
                                }
                            }
                        }
                        return Expr::new(
                            ExprKind::Cast {
                                expr: Box::new(value),
                                target,
                            },
                            target_ty,
                            Span::unknown(),
                        );
                    }
                }
            }
            // 处理 func[type_arg](args) → 泛型调用: func::<type_arg>(args)
            // 例外：`[checker]` 挂载（已知函数名）不是泛型参数（03c-检查站.md），
            // 调用点重复挂载由定义处 default_checker 处理，此处忽略
            let actual_func: &AstExpr;
            let extra_type_args: Vec<String>;
            if let AstExpr::Index { receiver, index } = func.as_ref() {
                if let AstExpr::Ident(ref type_name) = index.as_ref() {
                    if ctx.fn_returns.contains_key(type_name) {
                        actual_func = receiver;
                        extra_type_args = vec![];
                    } else {
                        actual_func = receiver;
                        extra_type_args = vec![type_name.clone()];
                    }
                } else {
                    actual_func = func;
                    extra_type_args = vec![];
                }
            } else {
                actual_func = func;
                extra_type_args = vec![];
            }
            let mut ir_type_args: Vec<String> = type_args
                .iter()
                .map(|t| match t.as_str() {
                    "int" => "i64".to_string(),
                    "str" => "String".to_string(),
                    "f64" | "float" => "f64".to_string(),
                    "bool" => "bool".to_string(),
                    other => other.to_string(),
                })
                .collect();
            ir_type_args.extend(extra_type_args);

            // __call__ 检测：如果是 struct 实例变量调用，转换为 MethodCall
            if let AstExpr::Ident(ref fname) = actual_func {
                if !ctx.fn_returns.contains_key(fname)
                    && !ctx.is_builtin_function(fname)
                    && !ctx.is_struct(fname)
                {
                    // 变量可能是一个 struct 实例 → 使用 __call__
                    let var_ty = ctx.lookup_var(fname);
                    if ctx.is_struct_type(&var_ty) {
                        let recv = convert_expr(actual_func, ctx);
                        let ret_ty = recv.ty.clone();
                        return Expr::new(
                            ExprKind::MethodCall {
                                receiver: Box::new(recv),
                                method: "__call__".to_string(),
                                args: args.iter().map(|a| convert_expr(a, ctx)).collect(),
                            },
                            ret_ty,
                            Span::unknown(),
                        );
                    }
                }
            }

            // Self(...) 在 struct 方法体内 → StructCtor { name: <当前 struct>, fields }
            // parser 现在把 Token::Self_ 输出为 Expr::Ident("Self")（大写），
            // 此处根据 self_ty 上下文区分为结构体构造而非 self 变量调用
            if let AstExpr::Ident(ref fname) = actual_func {
                if fname == "Self" {
                    if let Some(IrType::Named {
                        path: ref struct_name,
                        ..
                    }) = ctx.self_ty
                    {
                        if !args.is_empty() {
                            let order = ctx
                                .struct_field_order
                                .get(struct_name)
                                .cloned()
                                .unwrap_or_default();
                            let fields: Vec<(String, Expr)> = args
                                .iter()
                                .enumerate()
                                .map(|(i, a)| match a {
                                    AstExpr::KwArg { name, value } => {
                                        (name.clone(), convert_expr(value, ctx))
                                    }
                                    _ => {
                                        let fname_i = order
                                            .get(i)
                                            .cloned()
                                            .unwrap_or_else(|| format!("_f{}", i));
                                        (fname_i, convert_expr(a, ctx))
                                    }
                                })
                                .collect();
                            return Expr::new(
                                ExprKind::StructCtor {
                                    name: struct_name.clone(),
                                    fields,
                                },
                                ctx.self_ty.clone().unwrap_or(IrType::named(struct_name)),
                                Span::unknown(),
                            );
                        }
                    }
                }
            }

            // struct 位置参数构造：Point(1.0, 2.0) → StructCtor { name: "Point", fields: [(x,..),(y,..)] }
            // （管道 `1.0 |> Point(2.0)` 预填充后即此形式；关键字构造在 codegen 端已有处理）
            if let AstExpr::Ident(ref fname) = actual_func {
                if ctx.is_struct(fname) && !args.is_empty() {
                    let is_all_positional =
                        args.iter().all(|a| !matches!(a, AstExpr::KwArg { .. }));
                    if is_all_positional {
                        let order = ctx
                            .struct_field_order
                            .get(fname)
                            .cloned()
                            .unwrap_or_default();
                        let fields: Vec<(String, Expr)> = args
                            .iter()
                            .enumerate()
                            .map(|(i, a)| {
                                let fname_i =
                                    order.get(i).cloned().unwrap_or_else(|| format!("_f{}", i));
                                (fname_i, convert_expr(a, ctx))
                            })
                            .collect();
                        return Expr::new(
                            ExprKind::StructCtor {
                                name: fname.clone(),
                                fields,
                            },
                            IrType::named(fname),
                            Span::unknown(),
                        );
                    }
                }
            }

            // 函数参数调用（iter.lz filter/find `predicate(item)`，predicate:
            // fn(ref I.Item) -> bool）：callee 是 Fn 类型变量且参数为 ref 时，
            // 实参自动取引用（&item），否则 E0308 expected &I::Item found owned
            let plain_fn_name: Option<&String> = if let AstExpr::Ident(fname) = func.as_ref() {
                Some(fname)
            } else {
                None
            };
            let fn_arg_refs: Vec<bool> = if let AstExpr::Ident(fname) = func.as_ref() {
                match ctx.lookup_var(fname) {
                    IrType::Fn { params, .. } => params
                        .iter()
                        .map(|p| matches!(p, IrType::Ref(_) | IrType::MutRef(_)))
                        .collect(),
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            };
            let args: Vec<Expr> = args
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let conv = convert_expr(a, ctx);
                    if i < fn_arg_refs.len() && fn_arg_refs[i] {
                        Expr::new(
                            ExprKind::UnOp {
                                op: UnOpKind::Ref,
                                operand: Box::new(conv),
                            },
                            IrType::Any,
                            Span::unknown(),
                        )
                    } else {
                        // __from__ 实参触发点（06d §十四，P1-2 第二环）：
                        // `draw(p)`（p: int，形参类型 Point 定义了 __from__(int)）→
                        // 包装为 Point::__from__(p)。循环防护：__from__ 自身的实参
                        // 不再触发（否则 Point::__from__(int raw) 的 raw: int 会试图
                        // 再经 int.__from__ 转换——int 无 __from__ 自然终止，此处
                        // 显式跳过内建目标更稳）
                        if plain_fn_name.is_some() {
                            if let Some(param_tys) = ctx.fn_params.get(plain_fn_name.unwrap()) {
                                if let Some(pt) = param_tys.get(i) {
                                    if let IrType::Named { path, .. } = pt {
                                        let has_from = ctx
                                            .struct_methods
                                            .get(path)
                                            .map(|ms| ms.contains("__from__"))
                                            .unwrap_or(false);
                                        let val_same = conv.ty == *pt
                                            || matches!(conv.ty, IrType::Any | IrType::Generic(_));
                                        if has_from && !val_same {
                                            return Expr::new(
                                                ExprKind::Call {
                                                    type_args: vec![],
                                                    callee: Box::new(Expr::new(
                                                        ExprKind::Var(format!(
                                                            "{}::__from__",
                                                            path
                                                        )),
                                                        IrType::Any,
                                                        Span::unknown(),
                                                    )),
                                                    args: vec![conv],
                                                },
                                                pt.clone(),
                                                Span::unknown(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        conv
                    }
                })
                .collect();
            // M3：若命中特化，改调 mangled 特化版本并丢弃 comptime 位置实参
            let (final_callee, final_args): (Box<Expr>, Vec<Expr>) =
                if let Some(spec) = &specialized_callee {
                    let callee_expr =
                        Expr::new(ExprKind::Var(spec.clone()), IrType::Any, Span::unknown());
                    let filtered: Vec<Expr> = args
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !comptime_arg_positions.contains(i))
                        .map(|(_, a)| a.clone())
                        .collect();
                    (Box::new(callee_expr), filtered)
                } else {
                    (Box::new(convert_expr(actual_func, ctx)), args)
                };
            ExprKind::Call {
                type_args: ir_type_args,
                callee: final_callee,
                args: final_args,
            }
        }

        AstExpr::MethodCall {
            receiver,
            method,
            args,
        } => ExprKind::MethodCall {
            receiver: Box::new(convert_expr(receiver, ctx)),
            method: method.clone(),
            args: args.iter().map(|a| convert_expr(a, ctx)).collect(),
        },

        AstExpr::FieldAccess { receiver, field } => ExprKind::FieldAccess {
            base: Box::new(convert_expr(receiver, ctx)),
            field: field.clone(),
        },

        AstExpr::Index { receiver, index } => {
            // `services.validate_port[(r,)]`：模块命名空间 + 元组实参 → checker 块调用
            // （services.X 在 codegen 层降级为 X，此处转为函数调用而非索引）
            if let AstExpr::FieldAccess { receiver: _, field } = receiver.as_ref() {
                if let AstExpr::TupleLit(_) = index.as_ref() {
                    return Expr::new(
                        ExprKind::Call {
                            type_args: vec![],
                            callee: Box::new(Expr::new(
                                ExprKind::Var(field.clone()),
                                IrType::Any,
                                Span::unknown(),
                            )),
                            args: vec![convert_expr(index, ctx)],
                        },
                        IrType::Any,
                        Span::unknown(),
                    );
                }
            }
            // [] 下标访问 → IndexGet
            ExprKind::IndexGet {
                base: Box::new(convert_expr(receiver, ctx)),
                key: Box::new(convert_expr(index, ctx)),
            }
        }

        AstExpr::PathAccess { receiver, segment } => {
            // :: 路径访问 → FieldAccess（简化处理）
            ExprKind::FieldAccess {
                base: Box::new(convert_expr(receiver, ctx)),
                field: segment.clone(),
            }
        }

        AstExpr::Binary { left, op, right } => {
            // 特殊处理 `is` 运算符：编译期类型检查
            if matches!(op, BinOp::Is) {
                let left_ty = infer_expr_type(left, ctx);
                if let AstExpr::Ident(type_name) = right.as_ref() {
                    // Option 构造器同一性检查：`x is Some` / `x is None` 是运行时
                    // 变体检查（iterator.lz `while n is Some:` / tree.lz
                    // `if found_left is Some:`），必须生成 n.is_some()/is_none()。
                    // 若走编译期类型兼容性静态求值，while 条件会被固化为
                    // Bool(true) 导致死循环（loop {}）。
                    // left 类型为 Any（方法调用返回类型推断不完整）时同样按
                    // 运行时变体检查处理，保证 iterator.lz `let n = iter.next()` 生效。
                    let left_is_option = matches!(&left_ty, IrType::Option(_))
                        || matches!(&left_ty, IrType::Any)
                        || matches!(&left_ty, IrType::Named { path, .. } if path == "Option");
                    if left_is_option && (type_name == "Some" || type_name == "None") {
                        let method = if type_name == "Some" {
                            "is_some"
                        } else {
                            "is_none"
                        };
                        return Expr::new(
                            ExprKind::MethodCall {
                                receiver: Box::new(convert_expr(left, ctx)),
                                method: method.to_string(),
                                args: vec![],
                            },
                            IrType::Bool,
                            Span::unknown(),
                        );
                    }
                    let expected = name_to_ir_type(type_name);
                    let result = ir_types_compatible(&left_ty, &expected);
                    return Expr::new(
                        ExprKind::Lit(LitKind::Bool(result)),
                        IrType::Bool,
                        Span::unknown(),
                    );
                }
                // RHS is not a simple type name → fallback to false
                return Expr::new(
                    ExprKind::Lit(LitKind::Bool(false)),
                    IrType::Bool,
                    Span::unknown(),
                );
            }

            // 用户自定义类型的运算符 → 魔术方法调用（如 Vector + Vector → Vector.__add__）
            let left_ty_bin = infer_expr_type(left, ctx);
            if let Some(magic) = magic_method_for_binop(op) {
                let is_user_struct = match &left_ty_bin {
                    IrType::Named { path, .. } => ctx
                        .struct_methods
                        .get(path)
                        .map(|ms| ms.contains(magic))
                        .unwrap_or(false),
                    _ => false,
                };
                if is_user_struct {
                    let recv = convert_expr(left, ctx);
                    // 比较魔术方法（__eq__/__ne__/__lt__/__le__/__gt__/__ge__）与
                    // __contains__ 返回 Bool，而非左操作数类型——否则 if 条件里的
                    // `a < b` 被标成用户 struct 类型，codegen 真值判定链兜底成
                    // `true`（比较结果被吞，静默错误）
                    let ret_ty = if matches!(
                        magic,
                        "__eq__"
                            | "__ne__"
                            | "__lt__"
                            | "__le__"
                            | "__gt__"
                            | "__ge__"
                            | "__contains__"
                    ) {
                        IrType::Bool
                    } else {
                        infer_expr_type(left, ctx)
                    };
                    let expr = Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(recv),
                            method: magic.to_string(),
                            args: vec![convert_expr(right, ctx)],
                        },
                        ret_ty,
                        Span::unknown(),
                    );
                    return expr;
                }
            }

            let ir_op = map_binop(op);

            // 泛型调用检测: ident < Type > (args) — 不是比较，而是泛型实例化
            // 支持单类型参数 ident < T > 和多类型参数 ident < T, U >
            if matches!(ir_op, BinOpKind::Gt) {
                if let AstExpr::Binary {
                    left: inner_left,
                    op: BinOp::Lt,
                    right: inner_right,
                } = left.as_ref()
                {
                    if let AstExpr::Ident(fname) = inner_left.as_ref() {
                        if let Some(type_names) = extract_type_names(inner_right) {
                            if let AstExpr::Call {
                                func: call_func,
                                args: call_args,
                                ..
                            } = right.as_ref()
                            {
                                if let AstExpr::Ident(call_fname) = call_func.as_ref() {
                                    if call_fname == fname {
                                        // 这是泛型调用: f < T, U > (args)
                                        let ir_callee = convert_expr(inner_left, ctx);
                                        let ir_args: Vec<Expr> = call_args
                                            .iter()
                                            .map(|a| convert_expr(a, ctx))
                                            .collect();
                                        let ir_type_args = map_type_args(&type_names);
                                        return Expr::new(
                                            ExprKind::Call {
                                                callee: Box::new(ir_callee),
                                                args: ir_args,
                                                type_args: ir_type_args,
                                            },
                                            IrType::Any,
                                            Span::unknown(),
                                        );
                                    }
                                }
                            }
                            // 泛型调用不带括号参数: f < T > — 收集实参
                            let call_args;
                            if let AstExpr::TupleLit(elems) = right.as_ref() {
                                call_args = elems.clone();
                            } else {
                                call_args = vec![right.as_ref().clone()];
                            }
                            let ir_callee = convert_expr(inner_left, ctx);
                            let ir_args: Vec<Expr> =
                                call_args.iter().map(|a| convert_expr(a, ctx)).collect();
                            let ir_type_args = map_type_args(&type_names);
                            let ret_ty = ctx.lookup_fn_return(&fname);
                            return Expr::new(
                                ExprKind::Call {
                                    callee: Box::new(ir_callee),
                                    args: ir_args,
                                    type_args: ir_type_args,
                                },
                                ret_ty,
                                Span::unknown(),
                            );
                        }
                    }
                }
            }

            // 链式比较展开: 1 < x < 10 → (1 < x) && (x < 10)
            if matches!(
                ir_op,
                BinOpKind::Lt
                    | BinOpKind::Gt
                    | BinOpKind::Le
                    | BinOpKind::Ge
                    | BinOpKind::Eq
                    | BinOpKind::Neq
            ) {
                if let AstExpr::Binary {
                    left: inner_left,
                    op: inner_op,
                    right: inner_right,
                } = left.as_ref()
                {
                    let inner_ir_op = map_binop(inner_op);
                    if matches!(
                        inner_ir_op,
                        BinOpKind::Lt
                            | BinOpKind::Gt
                            | BinOpKind::Le
                            | BinOpKind::Ge
                            | BinOpKind::Eq
                            | BinOpKind::Neq
                    ) {
                        // (a cmp1 b) cmp2 c → (a cmp1 b) && (b cmp2 c)
                        let a = convert_expr(inner_left, ctx);
                        let b = convert_expr(inner_right, ctx);
                        let c = convert_expr(right, ctx);
                        return Expr::new(
                            ExprKind::BinOp {
                                op: BinOpKind::And,
                                lhs: Box::new(Expr::new(
                                    ExprKind::BinOp {
                                        op: inner_ir_op,
                                        lhs: Box::new(a),
                                        rhs: Box::new(b.clone()),
                                    },
                                    IrType::Bool,
                                    Span::unknown(),
                                )),
                                rhs: Box::new(Expr::new(
                                    ExprKind::BinOp {
                                        op: ir_op,
                                        lhs: Box::new(b),
                                        rhs: Box::new(c),
                                    },
                                    IrType::Bool,
                                    Span::unknown(),
                                )),
                            },
                            IrType::Bool,
                            Span::unknown(),
                        );
                    }
                }
            }

            ExprKind::BinOp {
                op: ir_op,
                lhs: Box::new(convert_expr(left, ctx)),
                rhs: Box::new(convert_expr(right, ctx)),
            }
        }

        AstExpr::Unary { op, operand } => {
            // 用户 struct 定义了 __neg__/__not__ 魔术方法 → 方法调用
            //（06d §三：`-a` → `a.__neg__()`、`not a` → `a.__not__()`），
            // 否则裸 `-a` 需 Neg impl 且 `-a.x` 渲染有优先级歧义
            // 位非 `~a` 与 `!a`（parser 均归一为 BitNot，lexer 前缀 ~ → Exclamation）、
            // 逻辑非 `not a`（Not）：优先 __not__，未定义时回退 __invert__。
            // 正号 `+a`（Pos）→ `a.__pos__()`（06d §三）；Neg 已有独立分派。
            // 解引用 `*a`（Deref）→ `a.__deref__()`（06d §六；非指针类型裸
            // `*a` 报 E0614，用户 struct 须经魔法方法）
            if matches!(
                op,
                UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot | UnaryOp::Pos | UnaryOp::Deref
            ) {
                let op_ty = infer_expr_type(operand, ctx);
                if let IrType::Named { path, .. } = &op_ty {
                    let methods = ctx.struct_methods.get(path);
                    let magic = match op {
                        UnaryOp::Neg => Some("__neg__"),
                        UnaryOp::Pos => Some("__pos__"),
                        UnaryOp::Deref => Some("__deref__"),
                        UnaryOp::Not => {
                            if methods.map(|ms| ms.contains("__not__")).unwrap_or(false) {
                                Some("__not__")
                            } else if methods.map(|ms| ms.contains("__invert__")).unwrap_or(false) {
                                Some("__invert__")
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let Some(magic) = magic {
                        if methods.map(|ms| ms.contains(magic)).unwrap_or(false) {
                            let recv = convert_expr(operand, ctx);
                            let ret_ty = op_ty;
                            return Expr::new(
                                ExprKind::MethodCall {
                                    receiver: Box::new(recv),
                                    method: magic.to_string(),
                                    args: vec![],
                                },
                                ret_ty,
                                Span::unknown(),
                            );
                        }
                    }
                }
                // 无魔术方法的 `+a`：恒等返回操作数（数值正号不变值；
                // 不可落到 UnOp——map_unop 无恒等变体，兜底 Neg 会把 +a 变 -a）
                if matches!(op, UnaryOp::Pos) {
                    return convert_expr(operand, ctx);
                }
            }
            ExprKind::UnOp {
                op: map_unop(op),
                operand: Box::new(convert_expr(operand, ctx)),
            }
        }

        AstExpr::If {
            cond,
            then_body,
            elif_clauses,
            else_body,
        } => {
            // 多分支 if → 嵌套 IfExpr。elif 链必须嵌套在**原始 if 的 else 分支内**：
            // `if cond { then } else { if elif1 { b1 } else { if elif2 { b2 } else { else_body } } }`
            // 旧实现把 elif 反向包在原始 if **外层**（`if elif { b } else { if cond ... }`），
            // 导致分支顺序颠倒（age_group: age<20 的 elif 反而先判断）。
            // else 分支保留原始 block_to_expr 节点（含自身 Unit 类型）——用
            // Expr::new(els, ty) 以 if 整体类型标注 else 会导致 set.lz 等
            // 语句级 if 的 else () 类型错乱（E0308 if/else incompatible）
            let mut els_expr = if let Some(els) = else_body {
                block_to_expr(els, ctx)
            } else {
                Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown())
            };
            // 反向迭代 elif，使第一个 elif 成为最内层 else（紧贴 else_body）
            for (elif_cond, elif_body) in elif_clauses.iter().rev() {
                els_expr = Expr::new(
                    ExprKind::IfExpr {
                        cond: Box::new(convert_expr(elif_cond, ctx)),
                        then: Box::new(block_to_expr(elif_body, ctx)),
                        els: Box::new(els_expr),
                    },
                    ty.clone(),
                    Span::unknown(),
                );
            }
            ExprKind::IfExpr {
                cond: Box::new(convert_expr(cond, ctx)),
                then: Box::new(block_to_expr(then_body, ctx)),
                els: Box::new(els_expr),
            }
        }

        AstExpr::Match { expr, arms } => {
            // Match 表达式 → 包装为 BlockExpr 内含 Match 语句
            // （保留模式匹配和变量绑定，if-else 降级会丢失这些信息）
            let ir_scrutinee = convert_expr(expr, ctx);
            let mut arm_ctx = TypeCtx::new();
            arm_ctx.current_generics = ctx.current_generics.clone();
            arm_ctx.current_ret_ty = ctx.current_ret_ty.clone();

            let ir_arms: Vec<MatchArm> = arms
                .iter()
                .map(|arm| {
                    let pat = convert_ast_pattern(&arm.pattern, ctx).unwrap_or(Pattern::Wildcard);
                    let guard = arm.guard.as_ref().map(|g| convert_expr(g, ctx));
                    let mut body_ctx = TypeCtx::new();
                    body_ctx.current_generics = ctx.current_generics.clone();
                    body_ctx.current_ret_ty = ctx.current_ret_ty.clone();
                    body_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                    body_ctx.enum_variants = ctx.enum_variants.clone();
                    // 从模式中提取绑定变量名并添加到上下文
                    fn collect_pattern_vars(pat: &AstPattern, vars: &mut Vec<String>) {
                        match pat {
                            AstPattern::Ident(name) => vars.push(name.clone()),
                            AstPattern::Variant(_, args) => {
                                for a in args {
                                    collect_pattern_vars(a, vars);
                                }
                            }
                            AstPattern::Tuple(elems) => {
                                for e in elems {
                                    collect_pattern_vars(e, vars);
                                }
                            }
                            _ => {}
                        }
                    }
                    let mut bound_vars = Vec::new();
                    collect_pattern_vars(&arm.pattern, &mut bound_vars);
                    let scrut_ty = infer_expr_type(expr, ctx);
                    // 元组模式绑定（type-pack 异质元组 03d §2.8 方案 B）：
                    // `case (a,)` / `case (a, ..)` 中 a 绑定切片元素（&Ts），
                    // 类型应为集合元素类型而非整个集合（否则返回类型推断错）
                    let bind_ty = if matches!(&arm.pattern, AstPattern::Tuple(_)) {
                        match &scrut_ty {
                            IrType::Named { args, .. } if !args.is_empty() => args[0].clone(),
                            _ => scrut_ty.clone(),
                        }
                    } else {
                        scrut_ty.clone()
                    };
                    for v in &bound_vars {
                        body_ctx.add_var(v, bind_ty.clone());
                    }
                    // 变体模式字段绑定：Shape::Circle(_, _, r) → r 绑定为字段类型（int）
                    if let Some(ftypes) = field_types_for_variant(&arm.pattern, &body_ctx) {
                        for (fname, fty) in ftypes {
                            body_ctx.add_var(&fname, fty);
                        }
                    }
                    // 内置 Option/Result 变体：Some(v) → v 绑定为内层类型
                    if let Some(ftypes) = field_types_for_builtin_variant(&arm.pattern, &scrut_ty) {
                        for (fname, fty) in ftypes {
                            body_ctx.add_var(&fname, fty);
                        }
                    }
                    let body = convert_block_with_ctx(&arm.body, &body_ctx);
                    MatchArm {
                        pattern: pat,
                        guard,
                        body,
                    }
                })
                .collect();

            let match_stmt = Stmt::Match {
                scrutinee: ir_scrutinee,
                arms: ir_arms,
            };
            let blk_ty = arms
                .first()
                .and_then(|arm| arm.body.last())
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit);
            ExprKind::BlockExpr {
                block: Block {
                    span: Span::unknown(),
                    stmts: vec![match_stmt],
                    ty: blk_ty,
                },
            }
        }

        AstExpr::Closure {
            params,
            param_tys,
            ret_ty,
            body,
        } => {
            // 闭包体是独立的词法块：创建新 ctx（继承 vars 但重置 block_declared），
            // 使闭包内 `total = total + x` 能识别为写外部变量（Assign）而非新绑定
            let mut closure_ctx = TypeCtx::new();
            closure_ctx.vars = ctx.vars.clone();
            closure_ctx.current_generics = ctx.current_generics.clone();
            closure_ctx.current_ret_ty = ctx.current_ret_ty.clone();
            closure_ctx.current_is_iterator = ctx.current_is_iterator;
            closure_ctx.current_fn_name = ctx.current_fn_name.clone();
            closure_ctx.pending_items = ctx.pending_items.clone();
            closure_ctx.errors = ctx.errors.clone();
            for sn in &ctx.struct_names {
                closure_ctx.struct_names.insert(sn.clone());
            }
            for (sn, fields) in &ctx.struct_fields {
                closure_ctx.struct_fields.insert(sn.clone(), fields.clone());
            }
            for (sn, order) in &ctx.struct_field_order {
                closure_ctx
                    .struct_field_order
                    .insert(sn.clone(), order.clone());
            }
            for (sn, ms) in &ctx.struct_methods {
                closure_ctx.struct_methods.insert(sn.clone(), ms.clone());
            }
            for (sn, arity) in &ctx.struct_method_arity {
                closure_ctx
                    .struct_method_arity
                    .insert(sn.clone(), arity.clone());
            }
            for (vn, en) in &ctx.enum_variants {
                closure_ctx.enum_variants.insert(vn.clone(), en.clone());
            }
            for (vn, ft) in &ctx.enum_variant_field_types {
                closure_ctx
                    .enum_variant_field_types
                    .insert(vn.clone(), ft.clone());
            }
            for (cn, ct) in &ctx.top_level_consts {
                closure_ctx.top_level_consts.insert(cn.clone(), ct.clone());
            }
            for (name, ty) in &ctx.fn_returns {
                closure_ctx.fn_returns.insert(name.clone(), ty.clone());
            }
            for (name, p) in &ctx.fn_params {
                closure_ctx.fn_params.insert(name.clone(), p.clone());
            }
            // 闭包参数登记到 closure_ctx.vars（遮蔽外部同名变量）：
            // 否则体内 `x + y` 中 y 会错误回退到外部 f64 变量，触发混合提升
            // （E0282 参数类型注解丢失的过渡方案：参数以 Any 登记，Any→i64 fallback）。
            // 带类型注解的 ref 参数（`|x: ref int|`）按 Ref 登记，使体内 `x > 2`
            // 能识别 x 是引用（codegen 解引用，iter.lz find 闭包 E0308）
            for (i, name) in params.iter().enumerate() {
                let param_ty = param_tys.get(i).and_then(|t| t.as_ref());
                let declared_ty = param_ty.map(|t| from_ast_type(t)).unwrap_or(IrType::Any);
                closure_ctx.vars.insert(name.clone(), declared_ty);
            }
            ExprKind::Lambda {
                params: params
                    .iter()
                    .enumerate()
                    .map(|(i, name)| Param {
                        name: name.clone(),
                        // 闭包参数类型注解（|x: int|）→ 填入 Lambda 参数类型，
                        // 否则生成无类型闭包导致 Option.None.map(|x| ...) E0282
                        ty: param_tys
                            .get(i)
                            .and_then(|t| t.as_ref())
                            .map(|t| from_ast_type_with_generics(t, &ctx.current_generics))
                            .unwrap_or(IrType::Any),
                        is_mut: false,
                        is_ref: false,
                        is_owned: false,
                        default: None,
                        variadic: false,
                        comptime: false,
                        mods: IrMods::default(),
                    })
                    .collect(),
                body: Box::new(convert_expr(body, &closure_ctx)),
                is_move: true,
                ret_ty: ret_ty
                    .as_ref()
                    .map(|t| from_ast_type_with_generics(t, &ctx.current_generics)),
            }
        }

        AstExpr::BlockExpr(stmts) => {
            let ir_stmts: Vec<Stmt> = stmts.iter().map(|s| convert_stmt(s, ctx)).collect();
            let blk_ty = stmts
                .last()
                .map(|s| infer_stmt_type(s, ctx))
                .unwrap_or(IrType::Unit);
            ExprKind::BlockExpr {
                block: Block {
                    span: Span::unknown(),
                    stmts: ir_stmts,
                    ty: blk_ty,
                },
            }
        }

        AstExpr::Range {
            start,
            end,
            inclusive,
        } => {
            // Range → StructCtor { name: "Range", fields: [start, end, inclusive] }
            let mut fields = Vec::new();
            if let Some(s) = start {
                fields.push(("start".into(), convert_expr(s, ctx)));
            }
            if let Some(e) = end {
                fields.push(("end".into(), convert_expr(e, ctx)));
            }
            if *inclusive {
                fields.push((
                    "inclusive".into(),
                    Expr::new(
                        ExprKind::Lit(LitKind::Bool(true)),
                        IrType::Bool,
                        Span::unknown(),
                    ),
                ));
            }
            ExprKind::StructCtor {
                name: "Range".into(),
                fields,
            }
        }

        AstExpr::Walrus { target, value } => {
            // := → 展开为 let + 返回；在表达式层面转为复合
            if let AstExpr::Ident(name) = target.as_ref() {
                let inner_ctx = ctx;
                let val = convert_expr(value, &inner_ctx);
                // walrus 变量登记不在表达式层做（convert_expr 是 &TypeCtx 不可变
                // 借用，无法 add_var）；由 convert_block 的前向传播统一登记
                // （collect_stmt_walrus），使后续语句 lookup_var(name) 拿到真实类型
                ExprKind::StructCtor {
                    name: "_Walrus".into(),
                    fields: vec![
                        (
                            "_bind".into(),
                            Expr::new(ExprKind::Var(name.clone()), val.ty.clone(), Span::unknown()),
                        ),
                        ("_val".into(), val),
                    ],
                }
            } else {
                convert_expr(value, ctx).kind
            }
        }

        AstExpr::Pipe {
            receiver,
            callee,
            args,
        } => {
            // |> 管道：
            // - 左侧 receiver 作为数据；若其类型实现了 __lpipe__，先调用 recv.__lpipe__() 产出数据
            // - 右侧 callee 分类：
            //     * 变量（实例，类型实现 __rpipe__）→ (right.__rpipe__(recv))(recv)
            //     * 变量（实例，类型实现 __call__）→ right.__call__(recv, ...args)（首参预填充）
            //     * 函数/构造/闭包/其他 → 首参预填充调用 callee(recv, ...args)
            let recv_ir = convert_expr(receiver, ctx);
            let args_ir: Vec<Expr> = args.iter().map(|a| convert_expr(a, ctx)).collect();
            // 左侧 __lpipe__：类型实现 __lpipe__ 时，数据 = recv.__lpipe__()（默认实现返回自身）
            let data_ir = {
                let recv_ty = infer_expr_type(receiver, ctx);
                if let IrType::Named { path, .. } = &recv_ty {
                    let has_lpipe = ctx
                        .struct_methods
                        .get(path)
                        .map_or(false, |m| m.contains("__lpipe__"));
                    if has_lpipe {
                        Expr::new(
                            ExprKind::MethodCall {
                                receiver: Box::new(recv_ir),
                                method: "__lpipe__".into(),
                                args: vec![],
                            },
                            recv_ty.clone(),
                            Span::unknown(),
                        )
                    } else {
                        recv_ir
                    }
                } else {
                    recv_ir
                }
            };
            // 右侧分类
            match callee.as_ref() {
                AstExpr::Ident(name) => {
                    // 变量（实例）→ __rpipe__ 优先，其次 __call__（须单参函数，否则报错）
                    let var_ty = ctx.lookup_var(name);
                    if let IrType::Named { path, .. } = &var_ty {
                        if let Some(methods) = ctx.struct_methods.get(path) {
                            if methods.contains("__rpipe__") {
                                // (right.__rpipe__(recv))(recv)
                                return Expr::new(
                                    ExprKind::Call {
                                        type_args: vec![],
                                        callee: Box::new(Expr::new(
                                            ExprKind::MethodCall {
                                                receiver: Box::new(Expr::new(
                                                    ExprKind::Var(name.clone()),
                                                    var_ty.clone(),
                                                    Span::unknown(),
                                                )),
                                                method: "__rpipe__".into(),
                                                args: vec![data_ir.clone()],
                                            },
                                            IrType::Any,
                                            Span::unknown(),
                                        )),
                                        args: vec![data_ir],
                                    },
                                    ty,
                                    span,
                                );
                            }
                            if methods.contains("__call__") {
                                // __call__ 必须是单参函数（非 self 参数 = 1）
                                let arity = ctx
                                    .struct_method_arity
                                    .get(path)
                                    .and_then(|m| m.get("__call__"))
                                    .copied()
                                    .unwrap_or(0);
                                if arity != 1 {
                                    ctx.report_error(format!(
                                        "管道右值 `{}` 的 __call__ 不是单参函数（参数数 {}，期望 1）；\
                                         实现 __rpipe__ 或改为单参 __call__",
                                        name, arity
                                    ));
                                }
                                // right.__call__(recv, ...args)（首参预填充）
                                let mut call_args = vec![data_ir];
                                call_args.extend(args_ir);
                                return Expr::new(
                                    ExprKind::MethodCall {
                                        receiver: Box::new(Expr::new(
                                            ExprKind::Var(name.clone()),
                                            var_ty.clone(),
                                            Span::unknown(),
                                        )),
                                        method: "__call__".into(),
                                        args: call_args,
                                    },
                                    ty,
                                    span,
                                );
                            }
                            // 已知 struct 实例但既无 __rpipe__ 也无 __call__ → 非 callable
                            ctx.report_error(format!(
                                "管道右值 `{}`（类型 {}）不可调用：未实现 __rpipe__ 或 __call__",
                                name, path
                            ));
                            return Expr::new(ExprKind::Lit(LitKind::Unit), ty, span);
                        }
                    }
                    // 函数/构造/未知 → 首参预填充调用；
                    // 已知 struct（构造调用）→ StructCtor 按字段顺序映射
                    if ctx.is_struct(name) {
                        let order = ctx
                            .struct_field_order
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        let mut fields: Vec<(String, Expr)> = Vec::new();
                        fields.push(("".into(), data_ir));
                        fields.extend(args_ir.into_iter().map(|a| ("".into(), a)));
                        let mapped: Vec<(String, Expr)> = fields
                            .into_iter()
                            .enumerate()
                            .map(|(i, (_, e))| {
                                let fname_i =
                                    order.get(i).cloned().unwrap_or_else(|| format!("_f{}", i));
                                (fname_i, e)
                            })
                            .collect();
                        return Expr::new(
                            ExprKind::StructCtor {
                                name: name.clone(),
                                fields: mapped,
                            },
                            IrType::named(name),
                            Span::unknown(),
                        );
                    }
                    // args 含 `_` 洞（v |> add3(_, 10, 20)）→ 用左侧数据填充洞，而非盲目前置
                    let has_hole = args
                        .iter()
                        .any(|a| matches!(a, AstExpr::Ident(s) if s == "_"));
                    // callee 返回类型为 Fn（x |> if_func(flag)，if_func 返回 fn(int)->int）
                    // → 语义是先调用 if_func(flag) 得到函数，再把 x 作为其参数：
                    //   if_func(flag)(x)，而非 if_func(x, flag)（E0061）
                    let callee_ret_is_fn = matches!(ctx.lookup_fn_return(name), IrType::Fn { .. });
                    let all_args = if has_hole {
                        let mut filled: Vec<Expr> = Vec::new();
                        let mut data_used = false;
                        for a in args.iter() {
                            if matches!(a, AstExpr::Ident(s) if s == "_") {
                                filled.push(data_ir.clone());
                                data_used = true;
                            } else {
                                filled.push(convert_expr(a, ctx));
                            }
                        }
                        if !data_used {
                            filled.insert(0, data_ir);
                        }
                        filled
                    } else if callee_ret_is_fn {
                        // if_func(flag) 返回函数 → 先调 callee(args...)，再以 data 为参数调用
                        let inner_call = ExprKind::Call {
                            type_args: vec![],
                            callee: Box::new(Expr::new(
                                ExprKind::Var(name.clone()),
                                IrType::Any,
                                Span::unknown(),
                            )),
                            args: args_ir,
                        };
                        return Expr::new(
                            ExprKind::Call {
                                type_args: vec![],
                                callee: Box::new(Expr::new(
                                    inner_call,
                                    IrType::Any,
                                    Span::unknown(),
                                )),
                                args: vec![data_ir],
                            },
                            ty,
                            span,
                        );
                    } else {
                        let mut pre = vec![data_ir];
                        pre.extend(args_ir);
                        pre
                    };
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(Expr::new(
                            ExprKind::Var(name.clone()),
                            IrType::Any,
                            Span::unknown(),
                        )),
                        args: all_args,
                    }
                }
                // 右侧是调用且含 `_` 洞（v |> f(_, 5)）：用左侧数据填充洞，而非盲目前置
                AstExpr::Call {
                    func,
                    args: call_args,
                    ..
                } if call_args
                    .iter()
                    .any(|a| matches!(a, AstExpr::Ident(s) if s == "_")) =>
                {
                    let mut filled: Vec<Expr> = Vec::new();
                    let mut data_used = false;
                    for a in call_args.iter() {
                        if matches!(a, AstExpr::Ident(s) if s == "_") {
                            filled.push(data_ir.clone());
                            data_used = true;
                        } else {
                            filled.push(convert_expr(a, ctx));
                        }
                    }
                    // 无洞（防御）→ 首参预填充
                    if !data_used {
                        filled.insert(0, data_ir);
                    }
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(convert_expr(func, ctx)),
                        args: filled,
                    }
                }
                // 闭包/方法/复杂表达式 → 保留 Pipe 节点，codegen 兜底展开
                // 闭包作为管道右侧（val |> (|x| => ...)）：参数类型从 receiver 推断，
                // 否则生成无类型闭包导致 E0282（combo-pipe-lambda.lz pipe_match）
                AstExpr::Closure { params, body, .. } => {
                    let recv_ty = infer_expr_type(receiver, ctx);
                    let recv_ty_inner = if let IrType::Fn { ret, .. } = &recv_ty {
                        // receiver 本身可能是函数（double 的结果类型为 int）
                        *ret.clone()
                    } else {
                        recv_ty.clone()
                    };
                    let mut closure_ctx = TypeCtx::new();
                    closure_ctx.vars = ctx.vars.clone();
                    closure_ctx.current_generics = ctx.current_generics.clone();
                    closure_ctx.current_ret_ty = ctx.current_ret_ty.clone();
                    closure_ctx.current_is_iterator = ctx.current_is_iterator;
                    closure_ctx.current_fn_name = ctx.current_fn_name.clone();
                    closure_ctx.pending_items = ctx.pending_items.clone();
                    closure_ctx.errors = ctx.errors.clone();
                    closure_ctx.struct_names = ctx.struct_names.clone();
                    closure_ctx.struct_fields = ctx.struct_fields.clone();
                    closure_ctx.struct_field_order = ctx.struct_field_order.clone();
                    closure_ctx.struct_methods = ctx.struct_methods.clone();
                    closure_ctx.struct_method_arity = ctx.struct_method_arity.clone();
                    closure_ctx.enum_variants = ctx.enum_variants.clone();
                    closure_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                    closure_ctx.top_level_consts = ctx.top_level_consts.clone();
                    closure_ctx.mutated_top_level_consts = ctx.mutated_top_level_consts.clone();
                    closure_ctx.fn_returns = ctx.fn_returns.clone();
                    closure_ctx.fn_params = ctx.fn_params.clone();
                    for name in params {
                        closure_ctx.vars.insert(name.clone(), IrType::Any);
                    }
                    let lambda_params: Vec<Param> = params
                        .iter()
                        .enumerate()
                        .map(|(i, name)| Param {
                            name: name.clone(),
                            ty: if i == 0 {
                                recv_ty_inner.clone()
                            } else {
                                IrType::Any
                            },
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                            comptime: false,
                            mods: IrMods::default(),
                        })
                        .collect();
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(Expr::new(
                            ExprKind::Lambda {
                                params: lambda_params,
                                body: Box::new(convert_expr(body, &closure_ctx)),
                                is_move: true,
                                ret_ty: None,
                            },
                            IrType::Any,
                            Span::unknown(),
                        )),
                        args: vec![data_ir],
                    }
                }
                _ => ExprKind::Pipe {
                    receiver: Box::new(data_ir),
                    callee: Box::new(convert_expr(callee, ctx)),
                    args: args_ir,
                },
            }
        }

        AstExpr::SafeNav { receiver, field } => {
            // x?.field → if x == None then None else x.field
            // 但如果 receiver 是类型名（非变量），直接字段访问，跳过 null check
            let recv = convert_expr(receiver, ctx);

            // 检查 receiver 是否是已知类型名（非变量引用）→ 跳过 null check
            let is_type_name = match receiver.as_ref() {
                AstExpr::Ident(name) => {
                    !ctx.vars.contains_key(name.as_str())
                        && (ctx.struct_names.contains(name.as_str())
                            || ctx.enum_variants.values().any(|en| en == name.as_str()))
                }
                _ => false,
            };

            if is_type_name {
                ExprKind::FieldAccess {
                    base: Box::new(recv),
                    field: field.clone(),
                }
            } else {
                // x?.field → x.map(|__sn| __sn.field)（Option.map；x 为 None 时得 None）
                // 避免 == None 比较（需 PartialEq）和直接 .field（Option 无该字段）
                let param = "__sn".to_string();
                // receiver 是 Dict/HashMap 时，`?.field` 是键访问：
                // __sn.field → __sn.get("field")（否则 E0609 no field）
                // 注意 receiver 可能是 Option<Dict>（?. 解包后才是 Dict）
                let recv_is_dict = matches!(
                    &recv.ty,
                    IrType::Named { path, .. } if path == "Dict" || path == "HashMap"
                ) || matches!(
                    &recv.ty,
                    IrType::Option(inner)
                        if matches!(inner.as_ref(), IrType::Named { path, .. } if path == "Dict" || path == "HashMap")
                );
                let access_expr = if recv_is_dict {
                    // Dict 键访问：__sn.get("field") 返回 Option<&V>，
                    // 链式 SafeNav 需 and_then 扁平化（map 会得 Option<Option<..>>）
                    // 且 get 结果需 .copied() 转 Option<V>（否则 unwrap_or(30) 类型不匹配）
                    let get_call = Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(Expr::new(
                                ExprKind::Var(param.clone()),
                                IrType::Any,
                                Span::unknown(),
                            )),
                            method: "get".into(),
                            args: vec![Expr::new(
                                ExprKind::Lit(LitKind::Str(field.clone())),
                                IrType::Str,
                                Span::unknown(),
                            )],
                        },
                        IrType::Any,
                        Span::unknown(),
                    );
                    Expr::new(
                        ExprKind::MethodCall {
                            receiver: Box::new(get_call),
                            method: "copied".into(),
                            args: vec![],
                        },
                        IrType::Any,
                        Span::unknown(),
                    )
                } else {
                    Expr::new(
                        ExprKind::FieldAccess {
                            base: Box::new(Expr::new(
                                ExprKind::Var(param.clone()),
                                IrType::Any,
                                Span::unknown(),
                            )),
                            field: field.clone(),
                        },
                        IrType::Any,
                        Span::unknown(),
                    )
                };
                // BUG-SG-003：字段本身可空（`db: DbConfig?`）时 map 会得到
                // Option<Option<T>>，必须 and_then 扁平化；字段类型无法判定
                // 时保守用 map（保持既有行为）。Dict 键访问同理（get 返 Option）。
                let field_is_option =
                    safe_nav_field_ty(&recv, field, ctx).map_or(false, |t| is_option_ty_ir(&t));
                ExprKind::MethodCall {
                    receiver: Box::new(recv),
                    method: if recv_is_dict || field_is_option {
                        "and_then".into()
                    } else {
                        "map".into()
                    },
                    args: vec![Expr::new(
                        ExprKind::Lambda {
                            params: vec![Param {
                                name: param.clone(),
                                ty: IrType::Any,
                                is_mut: false,
                                is_ref: false,
                                is_owned: false,
                                default: None,
                                variadic: false,
                                comptime: false,
                                mods: IrMods::default(),
                            }],
                            body: Box::new(access_expr),
                            is_move: true,
                            ret_ty: None,
                        },
                        IrType::Any,
                        Span::unknown(),
                    )],
                }
            }
        }

        AstExpr::Try(inner) => {
            // try expr (? 操作符): 对 Result/Option 类型做错误传播，否则透传
            let inner_ty = infer_expr_type(inner, ctx);
            let result_like = matches!(&inner_ty, IrType::Result { .. } | IrType::Option(_))
                || matches!(&inner_ty,
                    IrType::Named { path, .. } if path == "Result" || path == "Option"
                );
            // 自定义传播类型（实现 __is_ok__/__unwrap__/__err__ 的 struct）：
            // 与 Result/Option 一样走解包路径（spread_protocol.lz 的 HttpResult）
            let custom_propagating = matches!(&inner_ty, IrType::Named { path, .. }
                if ctx.struct_methods.get(path).map_or(false, |ms| ms.contains("__is_ok__")));
            if result_like || custom_propagating {
                ExprKind::MethodCall {
                    receiver: Box::new(convert_expr(inner, ctx)),
                    method: "try_into".into(),
                    args: vec![],
                }
            } else {
                // Non-Result type: just pass through (raises-type propagation)
                convert_expr(inner, ctx).kind
            }
        }

        AstExpr::NullCoalesce { left, right } => {
            // a ?? b → null_coalesce 特殊方法调用（codegen 层按类型展开）
            let l = convert_expr(left, ctx);
            let r = convert_expr(right, ctx);
            ExprKind::MethodCall {
                receiver: Box::new(l),
                method: "__null_coalesce".into(),
                args: vec![r],
            }
        }

        AstExpr::ListLit(items) => {
            ExprKind::ListLit(items.iter().map(|i| convert_expr(i, ctx)).collect())
        }

        AstExpr::DictLit(entries) => {
            // Dict → StructCtor，将条目存储为 (k, v) 对
            let mut fields = Vec::new();
            for (i, (k, v)) in entries.iter().enumerate() {
                fields.push((format!("_k{}", i), convert_expr(k, ctx)));
                fields.push((format!("_v{}", i), convert_expr(v, ctx)));
            }
            ExprKind::StructCtor {
                name: "Dict".into(),
                fields,
            }
        }

        AstExpr::SetLit(items) => ExprKind::Call {
            type_args: vec![],
            callee: Box::new(Expr::new(
                ExprKind::Var("set!".into()),
                IrType::Any,
                Span::unknown(),
            )),
            args: items.iter().map(|i| convert_expr(i, ctx)).collect(),
        },

        AstExpr::TupleLit(elems) => {
            ExprKind::TupleLit(elems.iter().map(|e| convert_expr(e, ctx)).collect())
        }

        AstExpr::ListComprehension {
            output,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            // [out for x in iter if cond] → 展开为生成模式
            if !extra_clauses.is_empty() {
                // 多 for：构建嵌套 flat_map 链
                let mut clauses = vec![(var.clone(), iter.clone(), cond.clone())];
                clauses.extend(
                    extra_clauses
                        .iter()
                        .map(|(v, i, c)| (v.clone(), i.clone(), c.clone())),
                );
                let out_expr = convert_expr(output, ctx);
                return Expr::new(build_multi_comp(ctx, &clauses, out_expr, "comp"), ty, span);
            }
            let iter_expr = convert_expr(iter, ctx);
            let out_expr = convert_expr(output, ctx);
            let mut args = vec![
                Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                            comptime: false,
                            mods: IrMods::default(),
                        }],
                        body: Box::new(out_expr),
                        is_move: true,
                        ret_ty: None,
                    },
                    IrType::Any,
                    Span::unknown(),
                ),
                iter_expr,
            ];
            // 过滤条件 cond 作为第三个参数传入 (可选)
            if let Some(c) = cond {
                args.push(Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                            comptime: false,
                            mods: IrMods::default(),
                        }],
                        body: Box::new(convert_expr(c, ctx)),
                        is_move: true,
                        ret_ty: None,
                    },
                    IrType::Any,
                    Span::unknown(),
                ));
            }
            ExprKind::Call {
                type_args: vec![],
                callee: Box::new(Expr::new(
                    ExprKind::Var("comp!".into()),
                    IrType::Any,
                    Span::unknown(),
                )),
                args,
            }
        }

        AstExpr::DictComprehension {
            key,
            value,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            // {k: v for x in iter} → 展开为生成模式
            let key_expr = convert_expr(key, ctx);
            let val_expr = convert_expr(value, ctx);
            let body = Expr::new(
                ExprKind::TupleLit(vec![key_expr, val_expr]),
                IrType::Any,
                Span::unknown(),
            );
            if !extra_clauses.is_empty() {
                // 多 for：构建嵌套 flat_map 链
                let mut clauses = vec![(var.clone(), iter.clone(), cond.clone())];
                clauses.extend(
                    extra_clauses
                        .iter()
                        .map(|(v, i, c)| (v.clone(), i.clone(), c.clone())),
                );
                return Expr::new(build_multi_comp(ctx, &clauses, body, "dict_comp"), ty, span);
            }
            let iter_expr = convert_expr(iter, ctx);
            let mut args = vec![
                Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                            comptime: false,
                            mods: IrMods::default(),
                        }],
                        body: Box::new(body),
                        is_move: true,
                        ret_ty: None,
                    },
                    IrType::Any,
                    Span::unknown(),
                ),
                iter_expr,
            ];
            if let Some(c) = cond {
                args.push(Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                            comptime: false,
                            mods: IrMods::default(),
                        }],
                        body: Box::new(convert_expr(c, ctx)),
                        is_move: true,
                        ret_ty: None,
                    },
                    IrType::Any,
                    Span::unknown(),
                ));
            }
            ExprKind::Call {
                type_args: vec![],
                callee: Box::new(Expr::new(
                    ExprKind::Var("dict_comp!".into()),
                    IrType::Any,
                    Span::unknown(),
                )),
                args,
            }
        }

        AstExpr::SetComprehension {
            elem,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            // {x for x in iter} → 展开为生成模式
            let elem_expr = convert_expr(elem, ctx);
            if !extra_clauses.is_empty() {
                // 多 for：构建嵌套 flat_map 链
                let mut clauses = vec![(var.clone(), iter.clone(), cond.clone())];
                clauses.extend(
                    extra_clauses
                        .iter()
                        .map(|(v, i, c)| (v.clone(), i.clone(), c.clone())),
                );
                return Expr::new(
                    build_multi_comp(ctx, &clauses, elem_expr, "set_comp"),
                    ty,
                    span,
                );
            }
            let iter_expr = convert_expr(iter, ctx);
            let mut args = vec![
                Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                            comptime: false,
                            mods: IrMods::default(),
                        }],
                        body: Box::new(elem_expr),
                        is_move: true,
                        ret_ty: None,
                    },
                    IrType::Any,
                    Span::unknown(),
                ),
                iter_expr,
            ];
            if let Some(c) = cond {
                args.push(Expr::new(
                    ExprKind::Lambda {
                        params: vec![Param {
                            name: var.clone(),
                            ty: IrType::Any,
                            is_mut: false,
                            is_ref: false,
                            is_owned: false,
                            default: None,
                            variadic: false,
                            comptime: false,
                            mods: IrMods::default(),
                        }],
                        body: Box::new(convert_expr(c, ctx)),
                        is_move: true,
                        ret_ty: None,
                    },
                    IrType::Any,
                    Span::unknown(),
                ));
            }
            ExprKind::Call {
                type_args: vec![],
                callee: Box::new(Expr::new(
                    ExprKind::Var("set_comp!".into()),
                    IrType::Any,
                    Span::unknown(),
                )),
                args,
            }
        }

        AstExpr::Assign { target, op, value } => {
            // 纯赋值（`total = total + x`，闭包体/表达式上下文）→ AssignExpr，
            // codegen 渲染 `target = value`；复合赋值（+= 等）→ BinOp
            if *op == AssignOp::Eq {
                ExprKind::AssignExpr {
                    target: Box::new(convert_expr(target, ctx)),
                    value: Box::new(convert_expr(value, ctx)),
                }
            } else {
                // 目标是用户 struct 且定义了对应就地魔术方法（如 __iadd__）→
                // 生成魔术方法调用而非脱糖 a = a + b（只定义 __iadd__ 无
                // __add__ 的类型脱糖后 E0369，06d §四）
                if let Some(magic) = assign_op_magic(op) {
                    let lhs = convert_expr(target, ctx);
                    if let IrType::Named { path, .. } = &lhs.ty {
                        if ctx
                            .struct_methods
                            .get(path)
                            .map(|ms| ms.contains(magic))
                            .unwrap_or(false)
                        {
                            let rhs_v = convert_expr(value, ctx);
                            let ret_ty = IrType::Unit;
                            return Expr::new(
                                ExprKind::MethodCall {
                                    receiver: Box::new(lhs),
                                    method: magic.to_string(),
                                    args: vec![rhs_v],
                                },
                                ret_ty,
                                Span::unknown(),
                            );
                        }
                    }
                }
                ExprKind::BinOp {
                    op: map_assign_op(op),
                    lhs: Box::new(convert_expr(target, ctx)),
                    rhs: Box::new(convert_expr(value, ctx)),
                }
            }
        }

        AstExpr::Spawn(inner) => {
            // go expr → 并行线程：thread::spawn(move || { expr })
            // 与显式 spawn(expr)（异步任务）区分，使用内部标记 __go
            ExprKind::Call {
                type_args: vec![],
                callee: Box::new(Expr::new(
                    ExprKind::Var("__go".into()),
                    IrType::Any,
                    Span::unknown(),
                )),
                args: vec![convert_expr(inner, ctx)],
            }
        }

        AstExpr::Move(inner) => {
            convert_expr(inner, ctx).kind // move 语义在 IR 中由所有权表达，暂透传
        }

        AstExpr::Panic(inner) => ExprKind::Call {
            type_args: vec![],
            callee: Box::new(Expr::new(
                ExprKind::Var("panic!".into()),
                IrType::Any,
                Span::unknown(),
            )),
            args: vec![convert_expr(inner, ctx)],
        },

        AstExpr::Await(inner) => ExprKind::MethodCall {
            receiver: Box::new(convert_expr(inner, ctx)),
            method: "await".into(),
            args: vec![],
        },

        AstExpr::BuildBlock { kind, lhs, body } => {
            // 构建块脱糖：将 body 转换为 BlockExpr，然后包装为闭包立即调用
            // 参考 AST codegen 的 gen_build_block 实现
            match kind {
                BuildKind::Var => {
                    // =: → { let __tmp = (|| { body; __result })(); __tmp }
                    // body 中的变量声明在此作用域中，闭包立即执行
                    let body_block = convert_block_with_ctx(body, ctx);
                    // 用 BlockExpr 包装 body（作为 Lambda 立即调用）
                    let body_expr = Expr::new(
                        ExprKind::BlockExpr { block: body_block },
                        IrType::Any,
                        Span::unknown(),
                    );
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
                    }
                }
                BuildKind::Call => {
                    // ~: → callee(args...) 其中 args = body 返回元组的元素
                    // 如果 body 最后返回的是元组，解包为独立参数
                    let body_block = convert_block_with_ctx(body, ctx);
                    let block_ty = body_block.ty.clone(); // 在 move 之前提取类型
                    let body_expr = Expr::new(
                        ExprKind::BlockExpr { block: body_block },
                        IrType::Any,
                        Span::unknown(),
                    );
                    let packed = Expr::new(
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
                    // 如果 body 返回元组，解包为独立参数
                    // 元组类型可能来自：块体末尾 TupleLit 推断，或单个 Ident 引用元组变量
                    // （multiply ~: factors — factors 是先前 =: 构建块返回的元组）
                    let block_ty_for_unpack = if let IrType::Tuple(_) = block_ty {
                        block_ty.clone()
                    } else if let Some(AstStmt::Expr(AstExpr::Ident(n))) = body.last() {
                        // 块体末尾是变量引用：若其类型是元组，则按元组拆包
                        match ctx.lookup_var(n) {
                            IrType::Tuple(_) => ctx.lookup_var(n),
                            _ => block_ty.clone(),
                        }
                    } else {
                        block_ty.clone()
                    };
                    let args: Vec<Expr> = if let IrType::Tuple(elements) = block_ty_for_unpack {
                        // 元组解包：为每个元素生成一个 packed 字段访问
                        elements
                            .iter()
                            .enumerate()
                            .map(|(i, _elem)| {
                                Expr::new(
                                    ExprKind::MagicCall {
                                        kind: MagicKind::UnpackBuildCall,
                                        args: vec![
                                            packed.clone(),
                                            Expr::new(
                                                ExprKind::Lit(LitKind::Int(i as i64)),
                                                IrType::Int,
                                                Span::unknown(),
                                            ),
                                        ],
                                    },
                                    IrType::Any,
                                    Span::unknown(),
                                )
                            })
                            .collect()
                    } else if matches!(
                        &block_ty,
                        IrType::Named { path, .. } if path == "Dict" || path == "HashMap"
                    ) {
                        // 字典拆包：块体末尾是 DictLit → 按名称转关键字实参
                        // （greet ~: {"greeting": "Hello", "name": "Lang-Zone"} →
                        //   greet(greeting: "Hello", name: "Lang-Zone")）
                        let dict_entries = body.last().and_then(|s| match s {
                            AstStmt::Expr(AstExpr::DictLit(entries)) => Some(entries.clone()),
                            _ => None,
                        });
                        match dict_entries {
                            Some(entries) => entries
                                .iter()
                                .map(|(k, v)| {
                                    Expr::new(
                                        ExprKind::StructCtor {
                                            name: "_KwArg".into(),
                                            fields: vec![
                                                ("name".to_string(), convert_expr(k, ctx)),
                                                ("value".to_string(), convert_expr(v, ctx)),
                                            ],
                                        },
                                        IrType::Any,
                                        Span::unknown(),
                                    )
                                })
                                .collect(),
                            None => vec![packed],
                        }
                    } else if let Some(AstStmt::Expr(AstExpr::Ident(n))) = body.last() {
                        // 块体末尾是 struct 变量且实现了 __buildparams__：调用 into_args() 拆包参数
                        match ctx.lookup_var(n) {
                            IrType::Named { path, .. } => {
                                if ctx
                                    .struct_methods
                                    .get(path.as_str())
                                    .map(|ms| ms.contains("__buildparams__"))
                                    .unwrap_or(false)
                                {
                                    let receiver_expr = Expr::new(
                                        ExprKind::Var(n.clone()),
                                        IrType::Named {
                                            path: path.clone(),
                                            args: vec![],
                                        },
                                        Span::unknown(),
                                    );
                                    let into_args_expr = Expr::new(
                                        ExprKind::MethodCall {
                                            receiver: Box::new(receiver_expr),
                                            method: "into_args".to_string(),
                                            args: vec![],
                                        },
                                        IrType::Any,
                                        Span::unknown(),
                                    );
                                    let buildparams_ret = ctx.lookup_fn_return(&format!(
                                        "{}.{}",
                                        path, "__buildparams__"
                                    ));
                                    if let IrType::Tuple(elements) = buildparams_ret {
                                        elements
                                            .iter()
                                            .enumerate()
                                            .map(|(i, _)| {
                                                Expr::new(
                                                    ExprKind::MagicCall {
                                                        kind: MagicKind::UnpackBuildCall,
                                                        args: vec![
                                                            into_args_expr.clone(),
                                                            Expr::new(
                                                                ExprKind::Lit(LitKind::Int(
                                                                    i as i64,
                                                                )),
                                                                IrType::Int,
                                                                Span::unknown(),
                                                            ),
                                                        ],
                                                    },
                                                    IrType::Any,
                                                    Span::unknown(),
                                                )
                                            })
                                            .collect()
                                    } else {
                                        vec![into_args_expr]
                                    }
                                } else {
                                    vec![packed]
                                }
                            }
                            _ => vec![packed],
                        }
                    } else {
                        vec![packed]
                    };
                    ExprKind::Call {
                        type_args: vec![],
                        callee: Box::new(convert_expr(lhs, ctx)),
                        args,
                    }
                }
                BuildKind::Gen => {
                    // *: → 生成器构建块：callee 为左侧函数/方法引用（有 callee 时逐包调用，
                    // 无 callee 时仅收集参数包返回迭代器）。body 中的 yield 由 codegen 收集。
                    let body_block = convert_block_with_ctx(body, ctx);
                    let callee = match &**lhs {
                        AstExpr::Ident(_)
                        | AstExpr::MethodCall { .. }
                        | AstExpr::FieldAccess { .. } => Some(Box::new(convert_expr(lhs, ctx))),
                        _ => None,
                    };
                    ExprKind::GenBuild {
                        callee,
                        block: body_block,
                    }
                }
                BuildKind::Index => {
                    // ^: → IndexGet。key = body 块中的最后一个表达式值。
                    // 语法：container ^: <key>（冒号后换行缩进，块体为单值 key）
                    let key_expr = body
                        .last()
                        .and_then(|s| match s {
                            AstStmt::Expr(e) => Some(convert_expr(e, ctx)),
                            _ => None,
                        })
                        .unwrap_or_else(|| {
                            // 回退：若无单个尾部表达式，用整个块（BlockExpr）
                            let blk = convert_block_with_ctx(body, ctx);
                            Expr::new(
                                ExprKind::BlockExpr { block: blk },
                                IrType::Any,
                                Span::unknown(),
                            )
                        });
                    ExprKind::IndexGet {
                        base: Box::new(convert_expr(lhs, ctx)),
                        key: Box::new(key_expr),
                    }
                }
            }
        }

        AstExpr::KwArg { name, value } => {
            // 关键字参数：后端按目标语言映射
            ExprKind::StructCtor {
                name: "_KwArg".into(),
                fields: vec![
                    (
                        "name".into(),
                        Expr::new(
                            ExprKind::Lit(LitKind::Str(name.clone())),
                            IrType::Str,
                            Span::unknown(),
                        ),
                    ),
                    ("value".into(), convert_expr(value, ctx)),
                ],
            }
        }

        AstExpr::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            // 构建 Stmt::TryCatch 结构以供 codegen 层正确处理
            let mut body_block = convert_block(body, ctx);
            // BUG-CG-004 收口：若 try 体末尾为「调用 raises 函数」的表达式，其 Rust 实际
            // 返回 `Result<ok, err>`（raise 已被 builder 改写为 `return Err`），须将 body
            // 块类型标注为 Result，否则 codegen 的 use_result_try 误判为非 Result 而走
            // catch_unwind（panic 基），导致 `Ok(val) => val` 臂类型与 Err 臂不兼容 E0308。
            if let Some(result_ty) = try_body_result_ty(&body_block, ctx) {
                body_block.ty = result_ty;
            }
            let ir_catches: Vec<(Option<Pattern>, Block)> = catches
                .iter()
                .map(|c| {
                    let pat = convert_ast_pattern(&c.pattern, ctx);
                    let block = convert_block(&c.body, ctx);
                    (pat, block)
                })
                .collect();
            let ir_else = else_body.as_ref().map(|b| convert_block(b, ctx));
            let ir_finally = finally_body.as_ref().map(|b| convert_block(b, ctx));

            // 返回一个 TryCatch 包装块（codegen 会生成 catch_unwind 等逻辑）
            ExprKind::BlockExpr {
                block: Block {
                    span: Span::unknown(),
                    stmts: vec![Stmt::TryCatch {
                        body: body_block,
                        catches: ir_catches,
                        else_body: ir_else,
                        finally_body: ir_finally,
                    }],
                    ty: IrType::Any,
                },
            }
        }
    };

    Expr::new(kind, ty, span)
}

/// BUG-CG-004 收口：判断 try 体块末尾表达式是否为「调用 raises 函数」，
/// 是则返回其 IR 返回类型 `Result<ok, err>`，供 TryCatch lowering 将 body 块类型
/// 标注为 Result，使 codegen 走 use_result_try（match）而非 catch_unwind（panic 基），
/// 避免 `Ok(val) => val` 臂类型与 Err 臂不兼容（E0308）。
///
/// 直接取 `fn_returns[callee]`：collect_functions 已对 raises 函数登记
/// `Result<ok, err>`（无返回注解时 ok=Unit，有返回注解时 ok=声明类型），可靠且精确。
/// 仅处理 `Name(...)` 形式调用；方法调用/闭包调用暂无法判定，走原 catch_unwind。
pub(crate) fn try_body_result_ty(block: &Block, ctx: &TypeCtx) -> Option<IrType> {
    // body 末尾裸表达式语句
    let last = block
        .stmts
        .iter()
        .rev()
        .find(|s| matches!(s, Stmt::ExprStmt { .. }))?;
    // 取出被调用函数名
    let callee = match last {
        Stmt::ExprStmt { expr } => match &expr.kind {
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Var(name) => name.clone(),
                _ => return None,
            },
            _ => return None,
        },
        _ => return None,
    };
    // 被调函数若为 raises 函数，其 IR 返回类型已是 Result<ok, err>
    match ctx.fn_returns.get(&callee) {
        Some(IrType::Result { .. }) => ctx.fn_returns.get(&callee).cloned(),
        _ => None,
    }
}

/// 将 AST 语句块转为 IR 表达式（用于 if/match 分支）
pub(crate) fn block_to_expr(stmts: &[AstStmt], ctx: &TypeCtx) -> Expr {
    if stmts.is_empty() {
        return Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown());
    }
    if stmts.len() == 1 {
        if let AstStmt::Expr(e) = &stmts[0] {
            return convert_expr(e, ctx);
        }
    }
    let ir_stmts: Vec<Stmt> = convert_stmts(stmts, ctx);
    let blk_ty = stmts
        .last()
        .map(|s| infer_stmt_type(s, ctx))
        .unwrap_or(IrType::Unit);
    Expr::new(
        ExprKind::BlockExpr {
            block: Block {
                span: Span::unknown(),
                stmts: ir_stmts,
                ty: blk_ty.clone(),
            },
        },
        blk_ty,
        Span::unknown(),
    )
}

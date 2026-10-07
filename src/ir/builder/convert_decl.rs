// Lang-Zone 编译器 — ir/builder/convert_decl.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

// ══════════════════════════════════════════════════════════════
// 顶层 Item 转换
// ══════════════════════════════════════════════════════════════

/// 转换 duck 类型约束 → IR DuckDef
pub(crate) fn convert_duck_def(d: &ast::DuckDef) -> DuckDef {
    // 嵌套约束 where T: X → 存到对应泛型参数的 bounds（§2.4）
    let mut where_bounds: HashMap<String, Vec<IrType>> = HashMap::new();
    for wb in &d.where_clause {
        let ir_bounds: Vec<IrType> = wb.bounds.iter().map(|b| from_ast_type(b)).collect();
        where_bounds
            .entry(wb.type_param.clone())
            .or_default()
            .extend(ir_bounds);
    }
    let methods: Vec<DuckMethod> = d
        .methods
        .iter()
        .map(|m| DuckMethod {
            owner: m.owner.clone(),
            name: m.name.clone(),
            name_pattern: m.name_pattern.clone(),
            params: m
                .params
                .iter()
                .map(|p| Param {
                    name: p.name.clone(),
                    ty: from_ast_type(&p.ty),
                    is_mut: p.is_mut,
                    is_ref: p.is_ref,
                    is_owned: p.is_owned,
                    default: None,
                    variadic: false,
                    comptime: false,
                    mods: IrMods::from_ast(&p.mods),
                })
                .collect(),
            ret_ty: m
                .return_type
                .as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or(IrType::Unit),
            param_range: m.param_range,
            is_default: m.is_default,
        })
        .collect();
    let fields: Vec<DuckField> = d
        .fields
        .iter()
        .map(|f| DuckField {
            owner: f.owner.clone(),
            name: f.name.clone(),
            ty: from_ast_type(&f.ty),
            rel: f.rel.clone(),
        })
        .collect();
    DuckDef {
        name: d.name.clone(),
        generics: d
            .generics
            .iter()
            .map(|g| GenericParam {
                name: g.clone(),
                bounds: where_bounds.remove(g).unwrap_or_default(),
                default: None,
            })
            .collect(),
        assoc_types: d
            .assoc_types
            .iter()
            .map(|a| DuckAssocType {
                owner: a.owner.clone(),
                name: a.name.clone(),
            })
            .collect(),
        satisfies: d.satisfies.clone(),
        sealed: d.sealed,
        match_rules: d
            .match_rules
            .iter()
            .map(|r| DuckMatchRule {
                pattern: r.pattern.clone(),
                range: r.range,
            })
            .collect(),
        param_reqs: d
            .param_reqs
            .iter()
            .map(|r| DuckParamReq {
                is_required: r.is_required,
                names: r.names.clone(),
            })
            .collect(),
        methods,
        fields,
    }
}

/// 检测 IR Block 是否包含 yield 语句（递归嵌套块）
pub(crate) fn ir_block_has_yield(block: &Block) -> bool {
    for stmt in &block.stmts {
        if matches!(stmt, Stmt::Yield { .. } | Stmt::YieldFrom { .. }) {
            return true;
        }
        match stmt {
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if ir_block_has_yield(then_branch) {
                    return true;
                }
                if let Some(eb) = else_branch {
                    if ir_block_has_yield(eb) {
                        return true;
                    }
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if ir_block_has_yield(body) {
                    return true;
                }
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    if ir_block_has_yield(&arm.body) {
                        return true;
                    }
                }
            }
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                if ir_block_has_yield(body) {
                    return true;
                }
                for (_, cb) in catches {
                    if ir_block_has_yield(cb) {
                        return true;
                    }
                }
                if let Some(eb) = else_body {
                    if ir_block_has_yield(eb) {
                        return true;
                    }
                }
                if let Some(fb) = finally_body {
                    if ir_block_has_yield(fb) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// 将 iterator（生成器）函数体内的带值 return 重写为 raise（等价终止并抛出）。
/// 递归遍历嵌套块（if/for/while/match/try 等），codegen 会把 Stmt::Raise 生成 panic!。
pub(crate) fn rewrite_iterator_returns(block: &mut Block) {
    for stmt in &mut block.stmts {
        match stmt {
            Stmt::Return { value: Some(v) } => {
                // 带值 return → raise（生成器内 return expr 等价于 raise）
                let v = std::mem::replace(
                    v,
                    Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown()),
                );
                *stmt = Stmt::Raise { value: v };
            }
            Stmt::Return { value: None } => {
                // 无值 return → raise 空
                *stmt = Stmt::Raise {
                    value: Expr::new(ExprKind::Lit(LitKind::Unit), IrType::Unit, Span::unknown()),
                };
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                rewrite_iterator_returns(then_branch);
                if let Some(eb) = else_branch {
                    rewrite_iterator_returns(eb);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                rewrite_iterator_returns(body);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    rewrite_iterator_returns(&mut arm.body);
                }
            }
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                rewrite_iterator_returns(body);
                for (_, cb) in catches {
                    rewrite_iterator_returns(cb);
                }
                if let Some(eb) = else_body {
                    rewrite_iterator_returns(eb);
                }
                if let Some(fb) = finally_body {
                    rewrite_iterator_returns(fb);
                }
            }
            Stmt::Block { stmts } => {
                let mut inner = Block {
                    span: Span::unknown(),
                    stmts: std::mem::take(stmts),
                    ty: IrType::Unit,
                };
                rewrite_iterator_returns(&mut inner);
                *stmts = inner.stmts;
            }
            _ => {}
        }
    }
}

/// 递归扫描生成器函数体，返回第一个 `yield expr` 中 expr 的推断类型。
/// 同时预登记函数体（含嵌套块）内的 let 绑定，使 `yield i` 能推断出 i 的类型。
pub(crate) fn scan_iterator_yield_ty(stmts: &[AstStmt], ctx: &mut TypeCtx) -> Option<IrType> {
    for stmt in stmts {
        match stmt {
            AstStmt::Let {
                name, ty, value, ..
            } => {
                // 预登记 let 绑定：优先类型注解，否则从初始值推断
                let bind_ty = ty
                    .as_ref()
                    .map(|t| from_ast_type_with_generics(t, &ctx.current_generics))
                    .unwrap_or_else(|| infer_expr_type(value, ctx));
                ctx.add_var(name, bind_ty);
                // 同时登记为本块首次声明：否则后续 `let mut i = 0` 会因 vars 已含 i
                // 且 block_declared 不含 i 而被误转 Stmt::Assign（生成裸赋值，E0425）
                ctx.block_declared.insert(name.clone());
            }
            AstStmt::Yield(Some(e)) => {
                return Some(infer_expr_type(e, ctx));
            }
            AstStmt::While {
                body, else_body, ..
            } => {
                if let Some(t) = scan_iterator_yield_ty(body, ctx) {
                    return Some(t);
                }
                if let Some(eb) = else_body {
                    if let Some(t) = scan_iterator_yield_ty(eb, ctx) {
                        return Some(t);
                    }
                }
            }
            AstStmt::Loop(body)
            | AstStmt::Block { body, .. }
            | AstStmt::CheckerBlock { body, .. }
            | AstStmt::Defer(body)
            | AstStmt::Test { body, .. }
            | AstStmt::Comptime { body } => {
                if let Some(t) = scan_iterator_yield_ty(body, ctx) {
                    return Some(t);
                }
            }
            AstStmt::WhileLet {
                body, else_body, ..
            }
            | AstStmt::For {
                body, else_body, ..
            } => {
                if let Some(t) = scan_iterator_yield_ty(body, ctx) {
                    return Some(t);
                }
                if let Some(eb) = else_body {
                    if let Some(t) = scan_iterator_yield_ty(eb, ctx) {
                        return Some(t);
                    }
                }
            }
            AstStmt::With { body, .. } => {
                if let Some(t) = scan_iterator_yield_ty(body, ctx) {
                    return Some(t);
                }
            }
            AstStmt::Suite {
                setup,
                teardown,
                tests,
                ..
            } => {
                for sub in setup
                    .iter()
                    .flatten()
                    .chain(teardown.iter().flatten())
                    .chain(tests.iter())
                {
                    if let Some(t) = scan_iterator_yield_ty(std::slice::from_ref(sub), ctx) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// auto-mut 推断辅助：扫描方法体是否存在 `self.field = ...` 赋值。
/// 存在时 `ref self` 需升级为 `&mut self`，否则 Rust 报 E0594
///（cannot assign to field behind &ref）
pub(crate) fn self_field_is_mutated(stmts: &[AstStmt]) -> bool {
    for stmt in stmts {
        match stmt {
            AstStmt::Assign { target, .. } => {
                if let AstExpr::FieldAccess { receiver, .. } = target {
                    if let AstExpr::Ident(name) = &**receiver {
                        if name == "self" || name == "self_" {
                            return true;
                        }
                    }
                }
            }
            AstStmt::While {
                body, else_body, ..
            }
            | AstStmt::For {
                body, else_body, ..
            }
            | AstStmt::WhileLet {
                body, else_body, ..
            } => {
                if self_field_is_mutated(body) {
                    return true;
                }
                if let Some(eb) = else_body {
                    if self_field_is_mutated(eb) {
                        return true;
                    }
                }
            }
            AstStmt::Loop(body)
            | AstStmt::Block { body, .. }
            | AstStmt::CheckerBlock { body, .. }
            | AstStmt::Defer(body)
            | AstStmt::Test { body, .. }
            | AstStmt::Comptime { body }
            | AstStmt::With { body, .. } => {
                if self_field_is_mutated(body) {
                    return true;
                }
            }
            AstStmt::Suite {
                setup,
                teardown,
                tests,
                ..
            } => {
                for sub in setup
                    .iter()
                    .flatten()
                    .chain(teardown.iter().flatten())
                    .chain(tests.iter())
                {
                    if self_field_is_mutated(std::slice::from_ref(sub)) {
                        return true;
                    }
                }
            }
            AstStmt::Guard {
                success_expr,
                else_body,
                ..
            } => {
                if self_field_is_mutated(else_body) {
                    return true;
                }
                if let Some(se) = success_expr {
                    if let AstExpr::BlockExpr(stmts) = se {
                        if self_field_is_mutated(stmts) {
                            return true;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn convert_fn_def(func: &ast::Function, ctx: &TypeCtx) -> FnDef {
    let is_math = func.decorators.iter().any(|d| d.name == "math");
    // 方法泛型 + impl 级泛型合并（`impl<T> Box<T>` 的方法 try_unwrap 中
    // `Result<T, Rc<T>>` 的 T 是 impl 泛型；只含 func.generics 则 T 被解析为
    // Named("T") 而非 Generic("T")，导致 return 隐式转换误判（E0277））
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
        .enumerate()
        .map(|(_i, p)| {
            // `..` 注入的 args/kwargs 在下方单独追加；此处只转换具名参数
            // auto-mut 推断：ref self 方法体内对 self 字段赋值时，翻 is_mut
            // 使 codegen 签名渲染为 `&mut self`（否则 E0594 cannot assign to
            // field behind &ref）
            let auto_mut = (p.name == "self" || p.name == "self_")
                && !p.is_mut
                && self_field_is_mutated(&func.body);
            let param_ty = if is_math {
                IrType::Generic("T".into())
            } else {
                from_ast_type_with_generics(&p.ty, &generics)
            };
            Param {
                name: p.name.clone(),
                ty: param_ty,
                is_mut: p.is_mut || auto_mut,
                is_ref: p.is_ref,
                is_owned: p.is_owned,
                default: p.default.as_ref().map(|d| convert_expr(d, ctx)),
                variadic: false,
                comptime: p.comptime,
                mods: IrMods::from_ast(&p.mods),
            }
        })
        .collect();

    // `..` 变参注入：追加 args/kwargs 隐式参数（variadic 收集）
    // 文档 03d-可变参数.md §2：任何 `..` 出现即触发注入；
    // 单 `..` 无注解 → 注入 args（元素 Any）；`..: Tuple<T>` → args-only；
    // `..: Dict<K,V>` → kwargs-only；双 `..` → args + kwargs
    let mut variadic_params: Vec<Param> = Vec::new();
    match &func.variadic {
        ast::VariadicMode::ArgsOnly {
            elem_ty, elem_tys, ..
        } => {
            // 03d §2.3 多类型位置约束：`..: Tuple<T1, T2, ..>` 约束位置参数各自类型。
            // 生成固定前缀异构元组 args: (T1, T2, Vec<Box<dyn Any>>)——
            // 前 N 个位置有精确类型，尾部 `..` 通配（哨兵 Type::Any）收集为 Box<dyn Any> 切片。
            let is_multi = elem_tys.len() >= 2 && elem_tys.last() == Some(&AstType::Any);
            if is_multi {
                let prefix_tys: Vec<AstType> = elem_tys[..elem_tys.len() - 1].to_vec();
                let prefix_irs: Vec<IrType> = prefix_tys
                    .iter()
                    .map(|t| from_ast_type_with_generics(t, &generics))
                    .collect();
                variadic_params.push(Param {
                    name: "args".into(),
                    ty: IrType::Tuple(prefix_irs),
                    is_mut: false,
                    is_ref: false,
                    is_owned: false,
                    default: None,
                    variadic: true,
                    comptime: false,
                    mods: IrMods::default(),
                });
            } else {
                // 03d §2.8 方案 B type-pack：`..: Tuple<Ts...>`（元素为 `Ts...`，
                // parser 已标记为 Named("Ts...")）→ 异质元组，由调用点推断具体
                // 类型。注册为 IrType::Tuple([Generic("Ts")])，使函数体内
                // args.N 走元组字段访问（codegen FieldAccess Tuple 分支 → .N），
                // 且调用点打包为 Rust 元组字面量（Type::Tuple 全 Generic = type pack）。
                let is_type_pack = elem_ty.as_ref().map_or(
                    false,
                    |t| matches!(t, AstType::Named(n) if n.ends_with("...")),
                );
                if is_type_pack {
                    let pack_name = match elem_ty.as_ref().unwrap() {
                        AstType::Named(n) => n.trim_end_matches("...").to_string(),
                        _ => String::new(),
                    };
                    variadic_params.push(Param {
                        name: "args".into(),
                        ty: IrType::Tuple(vec![IrType::Generic(pack_name)]),
                        is_mut: false,
                        is_ref: false,
                        is_owned: false,
                        default: None,
                        variadic: true,
                        comptime: false,
                        mods: IrMods::default(),
                    });
                } else {
                    let elem = elem_ty
                        .as_ref()
                        .map(|t| from_ast_type_with_generics(t, &generics))
                        .unwrap_or(IrType::Any);
                    variadic_params.push(Param {
                        name: "args".into(),
                        ty: elem,
                        is_mut: false,
                        is_ref: false,
                        is_owned: false,
                        default: None,
                        variadic: true,
                        comptime: false,
                        mods: IrMods::default(),
                    });
                }
            }
        }
        ast::VariadicMode::KwargsOnly { value_ty, .. } => {
            let v = value_ty
                .as_ref()
                .map(|t| from_ast_type_with_generics(t, &generics))
                .unwrap_or(IrType::Any);
            variadic_params.push(Param {
                name: "kwargs".into(),
                ty: v,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: true,
                comptime: false,
                mods: IrMods::default(),
            });
        }
        ast::VariadicMode::Both {
            args_elem_ty,
            kwargs_value_ty,
            ..
        } => {
            let elem = args_elem_ty
                .as_ref()
                .map(|t| from_ast_type_with_generics(t, &generics))
                .unwrap_or(IrType::Any);
            variadic_params.push(Param {
                name: "args".into(),
                ty: elem,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: true,
                comptime: false,
                mods: IrMods::default(),
            });
            let v = kwargs_value_ty
                .as_ref()
                .map(|t| from_ast_type_with_generics(t, &generics))
                .unwrap_or(IrType::Any);
            variadic_params.push(Param {
                name: "kwargs".into(),
                ty: v,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: true,
                comptime: false,
                mods: IrMods::default(),
            });
        }
        ast::VariadicMode::None => {}
    }
    // `..` 注入槽不得与显式形参重名。冻结语法 SYNTAX/03d §2 里 `..` 是**无名标记**，
    // 收集槽固定叫 args/kwargs；「带名字的 ..」（`def f(..args)`）不在语法内，
    // 而改前照单注入会让形参表出现两个 `args`，两个后端各自产出非法代码：
    //   Rust   `pub fn sum_all(args: i64, args: &[i64]) -> i64`（重复形参）
    //   Cython `def sum_all(object args, *args)`（cython 报 Previous declaration is here）
    // ⇒ 把语法错误静默降级成"产物不可编译"。这里按 IR 错误硬拒并给出改法。
    {
        let mut taken: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
        for vp in &variadic_params {
            if taken.contains(&vp.name.as_str()) {
                ctx.errors.borrow_mut().push(format!(
                    "函数 {}：`..` 注入的 `{}` 与显式形参重名。`..` 是无名标记，\
                     收集参数固定叫 {}；请把形参 `..{}` 写成 `..`，函数体内直接用 {} 即可",
                    func.name, vp.name, vp.name, vp.name, vp.name
                ));
            } else {
                taken.push(vp.name.as_str());
            }
        }
    }
    // 构建函数体上下文
    let mut fn_ctx = TypeCtx::new();
    fn_ctx.pending_items = ctx.pending_items.clone();
    fn_ctx.errors = ctx.errors.clone();
    fn_ctx.comptime_consts = ctx.comptime_consts.clone();
    // BUG-7：继承被重赋值的顶层 const 集合，避免 `count = 0; count += 1`
    // 被内联为字面量（0i64 = 0i64 + 1i64）
    fn_ctx.mutated_top_level_consts = ctx.mutated_top_level_consts.clone();
    fn_ctx.comptime_module = ctx.comptime_module.clone();
    fn_ctx.current_fn_name = Some(func.name.clone());
    // 继承顶层函数返回类型表：嵌套函数内调用其他函数（`return classify(0)`）需
    // 查 lookup_fn_return——否则 fn_returns 为空 → Any→i64 fallback，
    // return 误插 <String as ImplicitFrom<i64>> 转换（E0277，match_patterns.lz）
    fn_ctx.fn_returns = ctx.fn_returns.clone();
    // 继承 raises 异常类型表：try 体末尾调用 raises 函数需据此将 body 块类型
    // 标注为 Result（BUG-CG-004 收口），否则 fn_raises 为空 → 漏标 catch_unwind E0308
    fn_ctx.fn_raises = ctx.fn_raises.clone();
    // 继承顶层变量（size = 3 等顶层 Assign 转 Const）：函数内 `x = v` 需识别为
    // 修改全局（guard_for_3.lz size = size - 1），否则生成局部新绑定 E0425
    fn_ctx.top_level_consts = ctx.top_level_consts.clone();
    fn_ctx.current_generics = generics.clone();
    // 复制全局 struct 信息
    for sn in &ctx.struct_names {
        fn_ctx.struct_names.insert(sn.clone());
    }
    for (sn, fields) in &ctx.struct_fields {
        let mut cloned = HashMap::new();
        for (fn_, ty) in fields {
            cloned.insert(fn_.clone(), ty.clone());
        }
        fn_ctx.struct_fields.insert(sn.clone(), cloned);
    }
    for (sn, order) in &ctx.struct_field_order {
        fn_ctx.struct_field_order.insert(sn.clone(), order.clone());
    }
    for (sn, ms) in &ctx.struct_methods {
        fn_ctx.struct_methods.insert(sn.clone(), ms.clone());
    }
    for (sn, arity) in &ctx.struct_method_arity {
        fn_ctx.struct_method_arity.insert(sn.clone(), arity.clone());
    }
    for (cn, ct) in &ctx.top_level_consts {
        fn_ctx.top_level_consts.insert(cn.clone(), ct.clone());
    }
    for (vn, en) in &ctx.enum_variants {
        fn_ctx.enum_variants.insert(vn.clone(), en.clone());
    }
    for (vn, ft) in &ctx.enum_variant_field_types {
        fn_ctx
            .enum_variant_field_types
            .insert(vn.clone(), ft.clone());
    }
    for (name, ty) in &ctx.fn_returns {
        fn_ctx.fn_returns.insert(name.clone(), ty.clone());
    }
    for (name, p) in &ctx.fn_params {
        fn_ctx.fn_params.insert(name.clone(), p.clone());
    }

    // 添加参数到作用域
    if is_math {
        for p in &func.params {
            fn_ctx.add_param(&p.name, IrType::Generic("T".into()));
        }
    } else {
        for p in &func.params {
            // impl 方法中 self 参数绑定为 impl 目标类型（self_ty，如 Dict<K,V>），
            // 使 self[key]、key in self、self.field 等按具体类型解析（E0277/E0599）
            if p.name == "self" || p.name == "self_" {
                if let Some(st) = &ctx.self_ty {
                    // auto-mut 推断：方法体内对 self 字段赋值时，`ref self`
                    // 需升级为 `&mut self`（Rust 否则 E0594 cannot assign to field
                    // behind &ref）。扫描方法体的 self.field = ... 语句判定
                    let base_ty = st.clone();
                    let mutated = self_field_is_mutated(&func.body);
                    if mutated {
                        fn_ctx.add_param(&p.name, IrType::MutRef(Box::new(base_ty)));
                    } else {
                        fn_ctx.add_param(&p.name, base_ty);
                    }
                    continue;
                }
            }
            // ref 参数（iterable: ref I）登记为 Ref(inner)：for 循环等按引用处理，
            // 否则 iterable 推断为 Generic("I")，`for item in iterable` 生成
            // (iterable).into_iter() 会 move 出 &I（E0507 cannot move out of *iterable）
            let base_ty = from_ast_type_with_generics(&p.ty, &generics);
            if p.is_ref && !p.is_mut {
                fn_ctx.add_param(&p.name, IrType::Ref(Box::new(base_ty)));
            } else {
                fn_ctx.add_param(&p.name, base_ty);
            }
        }
    }
    // 注入 args/kwargs 内置变量到函数作用域（函数体内可直接引用 args / kwargs）
    for vp in &variadic_params {
        if vp.name == "args" {
            // args: List<elem>（元素类型注解或无注解 → Any）
            let elem = vp.ty.clone();
            // 03d §2.3 多类型位置约束：`..: Tuple<T1, T2, ..>` 的 args 是固定前缀
            // 异构元组 (T1, T2, Vec<Box<dyn Any>>)——注册为 Tuple 类型，使函数体内
            // args[0] / args.N 走元组字段访问（codegen IndexGet 的 Tuple 分支 → .0/.1）
            if matches!(&elem, IrType::Tuple(_)) {
                fn_ctx.add_param("args", elem);
            } else {
                fn_ctx.add_param(
                    "args",
                    IrType::Named {
                        path: "List".into(),
                        args: vec![elem],
                    },
                );
            }
        } else if vp.name == "kwargs" {
            // kwargs: Dict<str, V>
            let v = vp.ty.clone();
            fn_ctx.add_param(
                "kwargs",
                IrType::Named {
                    path: "Dict".into(),
                    args: vec![IrType::Str, v],
                },
            );
        }
    }

    // 返回类型：优先 AST 注解，否则从函数体最后语句推断；
    // iterator 无标注时从第一个 yield 表达式推断元素类型（codegen 会包装 Vec<T>）
    // 预登记函数体内闭包 let 绑定（consume = |..| => ..）：否则 body 末尾 `consume()`
    // 推断返回类型时 lookup_var 查不到 → 回退 Any→i64（E0308）
    prescan_closure_bindings(&func.body, &mut fn_ctx);
    let ret_ty = func
        .return_type
        .as_ref()
        .map(|t| from_ast_type_with_generics(t, &generics))
        .unwrap_or_else(|| {
            if func.is_iterator {
                // 递归扫描（yield 可嵌套在 while/for/if 块内），并预登记 let 绑定类型
                scan_iterator_yield_ty(&func.body, &mut fn_ctx).unwrap_or(IrType::Unit)
            } else {
                func.body
                    .last()
                    .map(|stmt| infer_stmt_type(stmt, &fn_ctx))
                    .unwrap_or(IrType::Unit)
            }
        });
    // 注意：Iterator 函数的 Vec<T> 包装由 codegen 负责（基于 has_yield 检测），
    // 此处不包装，避免 Vec<Vec<T>> 双重包装。
    fn_ctx.current_ret_ty = Some(ret_ty.clone());
    fn_ctx.current_is_iterator = func.is_iterator;

    // #[extern(lang)]：外部声明无 lz 函数体，codegen 生成分发调用
    let has_extern = func.decorators.iter().any(|d| d.name == "extern");
    let body = if has_extern {
        Block::default()
    } else {
        let body = convert_block(&func.body, &fn_ctx);
        // 函数尾 `let x =: <构建块>`（BuildBlock Var）：构建块值赋给 x 后应作为
        // 函数返回值（如 combo-build-block.lz 的 build_if_else/build_match/build_try）。
        // 否则生成 `let x = (move || {...})();` 后无 return，E0308 类型不匹配。
        if !matches!(ret_ty, IrType::Unit) && !func.is_iterator {
            // 匹配两种形式：
            //   - `result =: ...`（AstStmt::Expr 包裹 BuildBlock）
            //   - `let result =: ...`（AstStmt::Let 的 value 是 BuildBlock）
            let tail_build_lhs: Option<String> = match func.body.last() {
                Some(AstStmt::Expr(AstExpr::BuildBlock {
                    kind: BuildKind::Var,
                    lhs,
                    ..
                })) => match &**lhs {
                    AstExpr::Ident(name) => Some(name.clone()),
                    _ => None,
                },
                Some(AstStmt::Let { name, value, .. }) => {
                    if matches!(
                        value,
                        AstExpr::BuildBlock {
                            kind: BuildKind::Var,
                            ..
                        }
                    ) {
                        Some(name.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(name) = tail_build_lhs {
                let mut b = body;
                b.stmts.push(Stmt::Return {
                    value: Some(Expr::new(
                        ExprKind::Var(name),
                        ret_ty.clone(),
                        Span::unknown(),
                    )),
                });
                b
            } else {
                body
            }
        } else {
            body
        }
    };
    // iterator 体内 return 等价 raise（08-生成器.md：终止迭代并抛出）
    // codegen 将 Stmt::Raise 生成 panic!，因此把带值 return 转成 raise
    // 判断条件：is_iterator 标志 或 body 实际含 yield（更可靠，覆盖标志缺失）
    let body = if func.is_iterator || ir_block_has_yield(&body) {
        let mut b = body;
        rewrite_iterator_returns(&mut b);
        b
    } else {
        body
    };
    // BUG-CG-004（轮次12）：raises → Result<T, E> 语义收口。
    // 在已推断出 ok_ty 的函数体内，将 return x → return Ok(x)、raise x → return Err(x)，
    // 并对隐式尾表达式（非 Result 类型时）包 Ok(...)。ret_ty 同步升为 Result<ok, err>。
    let body = if func.raises.is_some() {
        let err_ty = func.raises.as_ref().map(from_ast_type).unwrap();
        let ok_ty = func
            .return_type
            .as_ref()
            .map(from_ast_type)
            .unwrap_or(IrType::Unit);
        rewrite_raises_block(body, &ok_ty, &err_ty)
    } else {
        body
    };
    let ret_ty = if let Some(raises_ast) = &func.raises {
        let err_ty = from_ast_type(raises_ast);
        let ok_ty = func
            .return_type
            .as_ref()
            .map(from_ast_type)
            .unwrap_or(IrType::Unit);
        IrType::Result {
            ok: Box::new(ok_ty),
            err: Box::new(err_ty),
        }
    } else {
        ret_ty
    };

    let is_math = func.decorators.iter().any(|d| d.name == "math");
    let intrinsics: Vec<Intrinsic> = func
        .decorators
        .iter()
        .map(|d| {
            let kind = match d.name.as_str() {
                "memoize" | "cache" => IntrinsicKind::Memoize,
                "parallel" => IntrinsicKind::Parallel,
                "curry" => IntrinsicKind::Curry,
                "overload" => IntrinsicKind::Overload,
                "derive" => IntrinsicKind::Derive,
                "tail_call" => IntrinsicKind::TailCall,
                // @tailrec：尾递归保证性标注（Auto TCO，Scala 语义）；
                // 与 tail_call 同义，校验逻辑已统一到 tco verdict
                "tailrec" => IntrinsicKind::TailCall,
                "math" => IntrinsicKind::Export(vec!["Math".into()]),
                "extern" => {
                    // #[extern(Rust, Python)] 外部声明（L1 机制）
                    let targets: Vec<String> = d
                        .args
                        .iter()
                        .filter_map(|a| {
                            if let AstExpr::Ident(n) = a {
                                Some(n.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    IntrinsicKind::Extern(if targets.is_empty() {
                        vec![]
                    } else {
                        // 归一化语言标记（大小写不敏感：rust → Rust）
                        targets
                            .iter()
                            .map(|t| {
                                let mut c = t.chars();
                                match c.next() {
                                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                                    None => String::new(),
                                }
                            })
                            .collect()
                    })
                }
                "embed" => {
                    // #[embed(rust)] / #[embed(py)]：内嵌代码段（G7）
                    // 语言参数取 args[0]（rust / py，大小写不敏感归一化）；
                    // 代码段取函数体首个字符串字面量（原生代码段原样插入生成产物）。
                    let lang = d
                        .args
                        .first()
                        .and_then(|a| {
                            if let AstExpr::Ident(n) = a {
                                Some(n.clone())
                            } else {
                                None
                            }
                        })
                        .map(|t| {
                            let mut c = t.chars();
                            match c.next() {
                                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                                None => String::new(),
                            }
                        })
                        .unwrap_or_default();
                    IntrinsicKind::Embed {
                        lang,
                        code: extract_embed_code(&body),
                    }
                }
                name if name.starts_with("export") => {
                    // @export(Rust, Python)
                    let targets: Vec<String> = d
                        .args
                        .iter()
                        .filter_map(|a| {
                            if let AstExpr::Ident(n) = a {
                                Some(n.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    IntrinsicKind::Export(if targets.is_empty() {
                        vec!["Rust".into()]
                    } else {
                        targets
                    })
                }
                "init" => IntrinsicKind::Init,
                _ => IntrinsicKind::None, // 未知/未识别装饰器（@unsafe/@simd 等）：无操作，codegen 忽略
            };
            Intrinsic {
                kind,
                span: Span::unknown(),
            }
        })
        .collect();

    // ── 尾递归优化（Auto TCO + @tailrec 严格标注）────────────────────
    // 无标注：仅判定并把 verdict 挂到 FnDef.tco，由 build_ir_inner 收口处
    // rewrite_tco() 自动改写为循环（尾位置自调用 → 形参重赋 + while）。
    // 有标注：verdict != TailOptimizable ⇒ 编译错误（Scala 式保证性契约）；
    //         `tail_call` 为 `tailrec` 的弃用别名，语义一致 + info 提示。
    let tco_verdict = analyze_tail_recursion(&TcoInput {
        fname: &func.name,
        params: &params,
        ret_ty: &ret_ty,
        raises: func.raises.is_some(),
        is_iterator: func.is_iterator,
        is_async: func.is_async || ast_body_has_async(&func.body),
        intrinsics: &intrinsics,
        body: &body,
    });
    let has_tailrec = func.decorators.iter().any(|d| d.name == "tailrec");
    let has_tail_call = func.decorators.iter().any(|d| d.name == "tail_call");
    if has_tailrec || has_tail_call {
        if tco_verdict != TcoVerdict::TailOptimizable {
            let label = if has_tailrec { "@tailrec" } else { "#[tail_call]" };
            ctx.report_error(format!(
                "{} 标注函数 '{}' 不是可优化的尾递归：{}",
                label,
                func.name,
                verdict_reason(&tco_verdict)
            ));
        } else if has_tail_call {
            push_tco_warning(format!(
                "提示：#[tail_call] 是弃用别名，请改用 @tailrec（函数 '{}'）",
                func.name
            ));
        }
    }

    // @memoize：约束检查——参数类型须 PartialEq+Clone（缓存键查找 + 值读取），
    // 返回类型须 Clone（缓存值读取时 clone 返回）。不满足时报编译错误。
    if func.decorators.iter().any(|d| d.name == "memoize") {
        for p in &func.params {
            let ast_ty = from_ast_type(&p.ty);
            if !is_cloneable_ir_type(&ast_ty) {
                ctx.report_error(format!(
                    "@memoize: 参数 '{}' 的类型 {} 不满足约束（参数须 PartialEq + Clone）",
                    p.name,
                    ir_type_name(&ast_ty)
                ));
            }
        }
        let ret_ast_ty = func
            .return_type
            .as_ref()
            .map(from_ast_type)
            .unwrap_or(IrType::Unit);
        if !is_cloneable_ir_type(&ret_ast_ty) {
            ctx.report_error(format!(
                "@memoize: 返回类型 {} 不满足约束（返回须 Clone）",
                ir_type_name(&ret_ast_ty)
            ));
        }
    }

    // #[extern(lang)] 诊断：语言参数缺失 / 重复标记 / 返回类型必须为 Ext
    {
        let extern_count = func
            .decorators
            .iter()
            .filter(|d| d.name == "extern")
            .count();
        if extern_count > 1 {
            ctx.report_error(format!(
                "#[extern] 重复标记：函数 '{}' 有 {} 个 extern 装饰器",
                func.name, extern_count
            ));
        }
        if let Some(ei) = intrinsics
            .iter()
            .find(|i| matches!(i.kind, IntrinsicKind::Extern(_)))
        {
            if let IntrinsicKind::Extern(targets) = &ei.kind {
                if targets.is_empty() {
                    ctx.report_error(format!(
                        "#[extern] 缺少语言参数：函数 '{}' 需要 #[extern(Rust)] / #[extern(Python)]",
                        func.name
                    ));
                }
                for t in targets {
                    let known = matches!(t.as_str(), "Rust" | "Python" | "C");
                    if !known {
                        ctx.report_error(format!(
                            "#[extern] 未知语言 '{}'：函数 '{}' 支持 Rust / Python / C",
                            t, func.name
                        ));
                    }
                }
            }
            if !matches!(ret_ty, IrType::Ext) {
                ctx.report_error(format!(
                    "#[extern] 返回类型错误：函数 '{}' 必须返回 Ext（外部专用句柄），实际返回 {}",
                    func.name, ret_ty
                ));
            }
        }
    }

    // #[embed(lang)] 诊断：语言缺失 / 未知语言 / 代码段缺失（G7）
    {
        let embed_count = func.decorators.iter().filter(|d| d.name == "embed").count();
        if embed_count > 1 {
            ctx.report_error(format!(
                "#[embed] 重复标记：函数 '{}' 有 {} 个 embed 装饰器",
                func.name, embed_count
            ));
        }
        if let Some(ei) = intrinsics
            .iter()
            .find(|i| matches!(i.kind, IntrinsicKind::Embed { .. }))
        {
            if let IntrinsicKind::Embed { lang, code } = &ei.kind {
                if lang.is_empty() {
                    ctx.report_error(format!(
                        "#[embed] 缺少语言参数：函数 '{}' 需要 #[embed(rust)] / #[embed(py)]",
                        func.name
                    ));
                } else if !matches!(lang.as_str(), "Rust" | "Python") {
                    ctx.report_error(format!(
                        "#[embed] 未知语言 '{}'：函数 '{}' 支持 rust / py",
                        lang, func.name
                    ));
                }
                if code.is_empty() {
                    ctx.report_error(format!(
                        "#[embed] 缺少内嵌代码段：函数 '{}' 的函数体必须为单个字符串字面量（原生代码段）",
                        func.name
                    ));
                }
            }
        }
    }

    /// 提取 embed 内嵌代码段（G7）：函数体首个字符串字面量（Return 值 / ExprStmt）
    ///
    /// 约定：`#[embed(rust)] def foo(): return "let x = 1; x + 1"` 的代码段为
    /// 字符串字面量本身（原生代码原样插入生成产物，不做 LZ 语义处理）。
    /// 未找到返回空串，由 embed 诊断块报错。
    fn extract_embed_code(body: &Block) -> String {
        for stmt in &body.stmts {
            let expr = match stmt {
                Stmt::Return { value: Some(e) } => e,
                Stmt::ExprStmt { expr } => expr,
                _ => continue,
            };
            // builder 会按返回类型包装隐式转换（ImplicitConvert→Lit(Str)），
            // 需穿透一层取其 source 字符串字面量
            let inner = match &expr.kind {
                ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                ExprKind::ImplicitConvert { source, .. } => {
                    if let ExprKind::Lit(LitKind::Str(s)) = &source.kind {
                        Some(s.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(s) = inner {
                return s;
            }
        }
        String::new()
    }

    // 引用 impl 级泛型的 where 约束（如 `impl<K,V> Dict<K,V>` 方法
    // `where K: Eq + Hash`，K 不在方法泛型中）无法合并到方法泛型，
    // 保留到 FnDef.where_clause，由 codegen 输出到方法签名（E0277 修复）
    let extra_where: Vec<(String, Vec<IrType>)> = func
        .where_clause
        .iter()
        .filter(|wb| !func.generics.contains(&wb.type_param))
        .map(|wb| {
            (
                wb.type_param.clone(),
                wb.bounds.iter().map(|b| from_ast_type(b)).collect(),
            )
        })
        .collect();

    FnDef {
        name: func.name.clone(),
        generics: if is_math && func.generics.is_empty() {
            // @math 自動泛型: 单泛型 T（所有参数统一类型）
            vec![GenericParam {
                name: "T".into(),
                bounds: vec![],
                default: None,
            }]
        } else {
            // 从 where_clause 收集每个泛型参数的 bounds
            let mut bounds_map: HashMap<String, Vec<IrType>> = HashMap::new();
            for wb in &func.where_clause {
                let ir_bounds: Vec<IrType> = wb.bounds.iter().map(|b| from_ast_type(b)).collect();
                bounds_map
                    .entry(wb.type_param.clone())
                    .or_default()
                    .extend(ir_bounds);
            }
            // 泛型默认类型（§四 `T = int`）
            let defaults_map: HashMap<String, IrType> = func
                .generic_defaults
                .iter()
                .map(|(n, t)| (n.clone(), from_ast_type(t)))
                .collect();
            let generics: Vec<GenericParam> = generics
                .iter()
                .map(|g| {
                    let bounds = bounds_map.remove(g).unwrap_or_default();
                    GenericParam {
                        name: g.clone(),
                        bounds,
                        default: defaults_map.get(g).cloned(),
                    }
                })
                .collect();
            generics
        },
        params: {
            // 合并注入的 args/kwargs 变参（variadic 收集）
            let mut all = params;
            all.extend(variadic_params);
            all
        },
        ret_ty,
        raises: func.raises.as_ref().map(from_ast_type),
        body,
        intrinsics,
        // 自动检测：如果函数体包含 await/spawn 且未显式标记 async，自动标记
        is_async: func.is_async || ast_body_has_async(&func.body),
        is_iterator: func.is_iterator,
        is_test: false,
        checker_param: func.checker_param.clone(),
        default_checker: func.default_checker.clone(),
        where_clause: extra_where,
        span: Span::unknown(),
        // 尾递归判定（rewrite_tco 消费；标注校验已在上面完成）
        tco: Some(tco_verdict),
    }
}

/// BUG-CG-004（轮次12）：raises → Result<T, E> 函数体改写。
/// 将 `return x` 包为 `Ok(x)`、`raise x` 包为 `Err(x)`，并对块尾裸表达式（类型非
/// Result 时）包 `Ok(...)`。递归进入 if/for/while/match/block 等嵌套块。
pub(crate) fn wrap_ok(e: Expr, ok: &IrType, err: &IrType) -> Expr {
    Expr::new(
        ExprKind::EnumCtor {
            enum_name: "Result".into(),
            variant: "Ok".into(),
            args: vec![e],
        },
        IrType::Result {
            ok: Box::new(ok.clone()),
            err: Box::new(err.clone()),
        },
        Span::unknown(),
    )
}

pub(crate) fn rewrite_raises_stmt(stmt: Stmt, ok: &IrType, err: &IrType) -> Stmt {
    match stmt {
        Stmt::Return { value: Some(e) } => Stmt::Return {
            value: Some(wrap_ok(e, ok, err)),
        },
        // 注：raise → Err 不在 builder 改写，改由 codegen 的 Stmt::Raise 处理。
        // 原因：raise 可出现在表达式位（如 if/elif/else 表达式的 else 分支），
        // builder 的语句级改写无法下降到 Expr 内部；而 `return Err(..)` 在表达式位同样合法
        // （从外层函数/闭包返回），故统一在 codegen 按 current_fn_raises 决定 panic!/return Err。
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => Stmt::If {
            cond,
            then_branch: rewrite_raises_block(then_branch, ok, err),
            else_branch: else_branch.map(|b| rewrite_raises_block(b, ok, err)),
        },
        Stmt::For {
            var,
            iter,
            guard,
            body,
            else_body,
        } => Stmt::For {
            var,
            iter,
            guard,
            body: rewrite_raises_block(body, ok, err),
            else_body: else_body.map(|b| rewrite_raises_block(b, ok, err)),
        },
        Stmt::While {
            cond,
            guard,
            body,
            else_body,
        } => Stmt::While {
            cond,
            guard,
            body: rewrite_raises_block(body, ok, err),
            else_body: else_body.map(|b| rewrite_raises_block(b, ok, err)),
        },
        Stmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
        } => Stmt::WhileLet {
            pattern,
            expr,
            guard,
            body: rewrite_raises_block(body, ok, err),
        },
        Stmt::Match { scrutinee, arms } => Stmt::Match {
            scrutinee,
            arms: arms
                .into_iter()
                .map(|mut a| {
                    a.body = rewrite_raises_block(a.body, ok, err);
                    a
                })
                .collect(),
        },
        Stmt::Block { stmts } => Stmt::Block {
            stmts: rewrite_raises_block(
                Block {
                    stmts,
                    ty: IrType::Unit,
                    span: Span::unknown(),
                },
                ok,
                err,
            )
            .stmts,
        },
        // try/catch 内的 raise 已由本改写覆盖（catch 块本身不提升返回类型）
        other => other,
    }
}

pub(crate) fn rewrite_raises_block(mut block: Block, ok: &IrType, err: &IrType) -> Block {
    // 仅当尾语句是裸表达式且其类型尚非 Result 时，包 Ok(...)。
    // 若尾表达式已因 raise→Err / callee 返回 Result 而本身是 Result，则不再重复包裹。
    let wrap_tail = match block.stmts.last() {
        Some(Stmt::ExprStmt { expr }) => !matches!(expr.ty, IrType::Result { .. } | IrType::Unit),
        _ => false,
    };
    if wrap_tail {
        if let Some(Stmt::ExprStmt { expr }) = block.stmts.pop() {
            block.stmts.push(Stmt::ExprStmt {
                expr: wrap_ok(expr, ok, err),
            });
        }
    }
    block.stmts = block
        .stmts
        .into_iter()
        .map(|s| rewrite_raises_stmt(s, ok, err))
        .collect();
    block
}

/// 检测 AST 函数体（Vec<Stmt>）是否包含 async 相关表达式（await/spawn）
pub(crate) fn ast_body_has_async(stmts: &[ast::Stmt]) -> bool {
    stmts.iter().any(|stmt| ast_stmt_has_async(stmt))
}

pub(crate) fn ast_stmt_has_async(stmt: &ast::Stmt) -> bool {
    match stmt {
        ast::Stmt::Expr(e) | ast::Stmt::Return(Some(e)) | ast::Stmt::Yield(Some(e)) => {
            ast_expr_has_async(e)
        }
        ast::Stmt::Let { value, .. } | ast::Stmt::Const { value, .. } => ast_expr_has_async(value),
        ast::Stmt::While {
            cond, body, guard, ..
        } => {
            ast_expr_has_async(cond)
                || guard.as_ref().map_or(false, |e| ast_expr_has_async(e))
                || ast_body_has_async(body)
        }
        ast::Stmt::For {
            iter, body, guard, ..
        } => {
            ast_expr_has_async(iter)
                || guard.as_ref().map_or(false, |e| ast_expr_has_async(e))
                || ast_body_has_async(body)
        }
        ast::Stmt::Loop(body) | ast::Stmt::Defer(body) => ast_body_has_async(body),
        ast::Stmt::Assign { target, value, .. } => {
            ast_expr_has_async(target) || ast_expr_has_async(value)
        }
        ast::Stmt::With { expr, body, .. } => ast_expr_has_async(expr) || ast_body_has_async(body),
        ast::Stmt::Break(Some(e)) | ast::Stmt::Raise(e) | ast::Stmt::YieldFrom(e) => {
            ast_expr_has_async(e)
        }
        ast::Stmt::FnDef { func } => ast_body_has_async(&func.body),
        _ => false,
    }
}

pub(crate) fn ast_expr_has_async(expr: &ast::Expr) -> bool {
    match expr {
        // Spawn(go) → thread::spawn 同步启动，不触发 async；Await 触发 async
        ast::Expr::Await(_) => true,
        ast::Expr::Call { func, args, .. } => {
            ast_expr_has_async(func) || args.iter().any(ast_expr_has_async)
        }
        ast::Expr::MethodCall { receiver, args, .. } => {
            ast_expr_has_async(receiver) || args.iter().any(ast_expr_has_async)
        }
        ast::Expr::Binary { left, right, .. } => {
            ast_expr_has_async(left) || ast_expr_has_async(right)
        }
        ast::Expr::Unary { operand, .. } => ast_expr_has_async(operand),
        ast::Expr::If {
            cond,
            then_body,
            elif_clauses,
            else_body,
        } => {
            ast_expr_has_async(cond)
                || ast_body_has_async(then_body)
                || elif_clauses
                    .iter()
                    .any(|(c, b)| ast_expr_has_async(c) || ast_body_has_async(b))
                || else_body.as_ref().map_or(false, |b| ast_body_has_async(b))
        }
        ast::Expr::Match { expr, arms } => {
            ast_expr_has_async(expr)
                || arms.iter().any(|a| {
                    a.guard.as_ref().map_or(false, |g| ast_expr_has_async(g))
                        || ast_body_has_async(&a.body)
                })
        }
        ast::Expr::Closure { body, .. } => ast_expr_has_async(body),
        ast::Expr::Pipe { receiver, args, .. } => {
            ast_expr_has_async(receiver) || args.iter().any(ast_expr_has_async)
        }
        ast::Expr::ListLit(es) | ast::Expr::TupleLit(es) | ast::Expr::SetLit(es) => {
            es.iter().any(ast_expr_has_async)
        }
        ast::Expr::DictLit(kvs) => kvs
            .iter()
            .any(|(k, v)| ast_expr_has_async(k) || ast_expr_has_async(v)),
        ast::Expr::SafeNav { receiver, .. } | ast::Expr::Try(receiver) => {
            ast_expr_has_async(receiver)
        }
        ast::Expr::NullCoalesce { left, right } => {
            ast_expr_has_async(left) || ast_expr_has_async(right)
        }
        ast::Expr::Walrus { target, value } => {
            ast_expr_has_async(target) || ast_expr_has_async(value)
        }
        ast::Expr::Paren(e) | ast::Expr::Panic(e) | ast::Expr::Move(e) => ast_expr_has_async(e),
        ast::Expr::Range { start, end, .. } => {
            start.as_ref().map_or(false, |e| ast_expr_has_async(e))
                || end.as_ref().map_or(false, |e| ast_expr_has_async(e))
        }
        ast::Expr::Assign { target, value, .. } => {
            ast_expr_has_async(target) || ast_expr_has_async(value)
        }
        ast::Expr::ListComprehension {
            output, iter, cond, ..
        } => {
            ast_expr_has_async(output)
                || ast_expr_has_async(iter)
                || cond.as_ref().map_or(false, |e| ast_expr_has_async(e))
        }
        ast::Expr::DictComprehension {
            key,
            value,
            iter,
            cond,
            ..
        } => {
            ast_expr_has_async(key)
                || ast_expr_has_async(value)
                || ast_expr_has_async(iter)
                || cond.as_ref().map_or(false, |e| ast_expr_has_async(e))
        }
        ast::Expr::SetComprehension {
            elem, iter, cond, ..
        } => {
            ast_expr_has_async(elem)
                || ast_expr_has_async(iter)
                || cond.as_ref().map_or(false, |e| ast_expr_has_async(e))
        }
        ast::Expr::BuildBlock { lhs, body, .. } => {
            ast_expr_has_async(lhs) || ast_body_has_async(body)
        }
        ast::Expr::TryCatch { body, .. } => ast_body_has_async(body),
        ast::Expr::FieldAccess { receiver, .. } | ast::Expr::PathAccess { receiver, .. } => {
            ast_expr_has_async(receiver)
        }
        ast::Expr::Index { receiver, index } => {
            ast_expr_has_async(receiver) || ast_expr_has_async(index)
        }
        ast::Expr::KwArg { value, .. } => ast_expr_has_async(value),
        _ => false,
    }
}

/// 为 case struct 合成 `__unapply__`（定长提取，对应 Scala unapply）：
/// 返回 (self.f1, self.f2, ...)，使 `let PointEx(x, y) = p` / `case PointEx(x, y)`
/// 能经 `__unapply__` 完成提取。
pub(crate) fn synth_unapply(s: &ast::StructDef) -> ast::Function {
    use crate::types::Type as AstType;
    let field_tys: Vec<AstType> = s.fields.iter().map(|f| f.ty.clone()).collect();
    let elems: Vec<ast::Expr> = s
        .fields
        .iter()
        .map(|f| ast::Expr::FieldAccess {
            receiver: Box::new(ast::Expr::Ident("self".into())),
            field: f.name.clone(),
        })
        .collect();
    ast::Function {
        name: "__unapply__".into(),
        generics: vec![],
        generic_defaults: vec![],
        params: vec![ast::Param {
            name: "self".into(),
            ty: AstType::Any,
            default: None,
            is_mut: false,
            is_owned: false,
            is_ref: false,
            comptime: false,
            mods: crate::ast::Modifiers::empty(),
        }],
        return_type: Some(AstType::Tuple(field_tys)),
        raises: None,
        where_clause: vec![],
        body: vec![ast::Stmt::Return(Some(ast::Expr::TupleLit(elems)))],
        is_async: false,
        is_abstract: false,
        is_iterator: false,
        is_magic: true,
        is_comptime: false,
        decorators: vec![],
        variadic: crate::parser::VariadicMode::None,
        checker_param: None,
        default_checker: None,
    }
}

/// 为 case struct 合成 `__unapply_seq__`（变长提取，对应 Scala unapplySeq）：
/// 仅当所有字段类型相同时生成，返回 Vec<T>（T 为字段类型），使
/// `case PointEx(a, b, ..rest)` 能经 `__unapply_seq__` 完成变长提取。
pub(crate) fn synth_unapply_seq(s: &ast::StructDef) -> Option<ast::Function> {
    use crate::types::Type as AstType;
    if s.fields.is_empty() {
        return None;
    }
    let first = s.fields[0].ty.clone();
    if !s.fields.iter().all(|f| f.ty == first) {
        return None;
    }
    let elems: Vec<ast::Expr> = s
        .fields
        .iter()
        .map(|f| ast::Expr::FieldAccess {
            receiver: Box::new(ast::Expr::Ident("self".into())),
            field: f.name.clone(),
        })
        .collect();
    let list_ty = AstType::Generic {
        base: Box::new(AstType::Named("List".into())),
        args: vec![first],
    };
    Some(ast::Function {
        name: "__unapply_seq__".into(),
        generics: vec![],
        generic_defaults: vec![],
        params: vec![ast::Param {
            name: "self".into(),
            ty: AstType::Any,
            default: None,
            is_mut: false,
            is_owned: false,
            is_ref: false,
            comptime: false,
            mods: crate::ast::Modifiers::empty(),
        }],
        return_type: Some(list_ty),
        raises: None,
        where_clause: vec![],
        body: vec![ast::Stmt::Return(Some(ast::Expr::ListLit(elems)))],
        is_async: false,
        is_abstract: false,
        is_iterator: false,
        is_magic: true,
        is_comptime: false,
        decorators: vec![],
        variadic: crate::parser::VariadicMode::None,
        checker_param: None,
        default_checker: None,
    })
}

pub(crate) fn convert_struct(s: &ast::StructDef, ctx: &TypeCtx) -> Item {
    // case struct 自动配 __unapply__（定长提取，对应 Scala unapply）与
    // __unapply_seq__（变长提取，对应 Scala unapplySeq，仅当字段同构类型时生成）。
    // 注入为普通 magic 方法，复用既有 magic 方法 codegen 与类型推断。
    // @case 装饰器与 `case struct` 关键字双用法：装饰器形式
    // `@case struct Point(x: int, y: int)` 等价于 `case struct Point(x: int, y: int)`。
    let has_case_decorator = s.decorators.iter().any(|d| d.name == "case");
    let s = if s.is_case || has_case_decorator {
        let mut owned = s.clone();
        let mut methods = s.magic_methods.clone();
        methods.push(synth_unapply(s));
        if let Some(seq) = synth_unapply_seq(s) {
            methods.push(seq);
        }
        owned.magic_methods = methods;
        owned
    } else {
        s.clone()
    };
    if s.is_enum {
        let variants: Vec<Variant> = s
            .fields
            .iter()
            .map(|f| {
                // 简化：字段作为变体处理
                // 实际的 enum field 没有子类型（简单变体）
                Variant {
                    name: f.name.clone(),
                    fields: match &f.ty {
                        AstType::Unit | AstType::None_ => vec![],
                        AstType::Duck { fields } => {
                            // 命名字段变体: Circle(x: f64, y: f64) → 带名 Field
                            // （codegen 生成 Rust 结构体变体 { x: f64, y: f64 }）
                            fields
                                .iter()
                                .map(|(n, t)| Field {
                                    name: n.clone(),
                                    ty: from_ast_type(t),
                                })
                                .collect()
                        }
                        AstType::Tuple(elems) => {
                            // 元组变体: Circle(f64, f64, f64) → 三个无名 Field
                            elems
                                .iter()
                                .map(|t| Field {
                                    name: String::new(),
                                    ty: from_ast_type(t),
                                })
                                .collect()
                        }
                        other => vec![Field {
                            name: String::new(),
                            ty: from_ast_type(other),
                        }],
                    },
                }
            })
            .collect();

        let enum_methods: Vec<FnDef> = s
            .methods
            .iter()
            .map(|m| {
                let mut method_ctx = TypeCtx::new();
                method_ctx.pending_items = ctx.pending_items.clone();
                method_ctx.struct_names = ctx.struct_names.clone();
                method_ctx.struct_fields = ctx.struct_fields.clone();
                method_ctx.struct_field_order = ctx.struct_field_order.clone();
                method_ctx.struct_methods = ctx.struct_methods.clone();
                method_ctx.struct_method_arity = ctx.struct_method_arity.clone();
                // 枚举/类型注册表必须拷入方法体 ctx：否则方法体内 Type.Variant(...)
                // 构造推断退化（lib_hashmap get 返回 OptionInt.Some 被推断为 i64，
                // 误插 ImplicitFrom 转换桥 E0277/E0308），方法返回类型查询也失效
                method_ctx.enum_variants = ctx.enum_variants.clone();
                method_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                method_ctx.fn_returns = ctx.fn_returns.clone();
                // enum 内联方法（`enum E = ... def m(self)`）：self 绑定为枚举自身类型，
                // 否则 self.field / self 索引推断为 Any/Self_（json.lz E0277/E0308）
                method_ctx.self_ty = Some(IrType::Named {
                    path: s.name.clone(),
                    args: s
                        .generics
                        .iter()
                        .map(|g| IrType::Generic(g.clone()))
                        .collect(),
                });
                convert_fn_def(m, &method_ctx)
            })
            .collect();

        Item::EnumDef(EnumDef {
            name: s.name.clone(),
            generics: s
                .generics
                .iter()
                .map(|g| GenericParam {
                    name: g.clone(),
                    bounds: vec![],
                    default: s
                        .generic_defaults
                        .iter()
                        .find(|(n, _)| n == g)
                        .map(|(_, t)| from_ast_type(t)),
                })
                .collect(),
            variants,
            methods: enum_methods,
            derives: collect_derives(&s.decorators),
            span: Span::unknown(),
        })
    } else {
        let fields: Vec<Field> = s
            .fields
            .iter()
            .map(|f| {
                // Self 字段（next: Self?）在 struct 定义内解析为自身类型名，
                // 使字段访问（n.next）继承具体类型而非 Self_（后者在函数体内非法）
                let self_ty = IrType::Named {
                    path: s.name.clone(),
                    args: s
                        .generics
                        .iter()
                        .map(|g| IrType::Generic(g.clone()))
                        .collect(),
                };
                let field_ty = replace_self(&from_ast_type(&f.ty), &self_ty);

                Field {
                    name: f.name.clone(),
                    ty: field_ty,
                }
            })
            .collect();

        // 构造器/转换魔法（__new__/__init__/__implicit_from__）是特殊方法：
        // 它们由 gen_struct_def 单独生成构造器函数，不进 methods（否则会带出被
        // 降级的 `self(...)` 错误体，见 E0424；且与构造器函数重名）。
        let special_magic = ["__new__", "__init__", "__implicit_from__"];

        let methods: Vec<FnDef> = s
            .methods
            .iter()
            .filter(|m| !special_magic.contains(&m.name.as_str()))
            .map(|m| {
                let mut method_ctx = TypeCtx::new();
                method_ctx.pending_items = ctx.pending_items.clone();
                method_ctx.struct_names = ctx.struct_names.clone();
                method_ctx.struct_fields = ctx.struct_fields.clone();
                method_ctx.struct_field_order = ctx.struct_field_order.clone();
                method_ctx.struct_methods = ctx.struct_methods.clone();
                method_ctx.struct_method_arity = ctx.struct_method_arity.clone();
                // 同上：方法体 ctx 需枚举/类型注册表（lib_hashmap 实测）
                method_ctx.enum_variants = ctx.enum_variants.clone();
                method_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                method_ctx.fn_returns = ctx.fn_returns.clone();
                method_ctx.vars = ctx.vars.clone(); // 继承顶层函数类型，使方法体内能引用 `add` 等函数
                                                    // struct 内联方法（`struct Parser = s: str ... def m(self)`）：self 绑定为
                                                    // struct 自身类型（含泛型参数），否则 self.s 字段访问推断为 Any/Self_，
                                                    // 导致 str 字段索引/切片 codegen 生成非法 Rust（json.lz E0277/E0308）
                method_ctx.self_ty = Some(IrType::Named {
                    path: s.name.clone(),
                    args: s
                        .generics
                        .iter()
                        .map(|g| IrType::Generic(g.clone()))
                        .collect(),
                });
                convert_fn_def(m, &method_ctx)
            })
            .collect();

        // 普通 magic 方法体（__str__/__add__ 等，除 __new__/__init__/__implicit_from__ 特殊处理外）
        // 也转成 FnDef 并入 methods，否则 `magic __str__` 等方法体在 IR 中丢失
        let magic_methods: Vec<FnDef> = s
            .magic_methods
            .iter()
            .filter(|m| !special_magic.contains(&m.name.as_str()))
            .map(|m| {
                let mut method_ctx = TypeCtx::new();
                method_ctx.pending_items = ctx.pending_items.clone();
                method_ctx.struct_names = ctx.struct_names.clone();
                method_ctx.struct_fields = ctx.struct_fields.clone();
                method_ctx.struct_field_order = ctx.struct_field_order.clone();
                method_ctx.struct_methods = ctx.struct_methods.clone();
                method_ctx.struct_method_arity = ctx.struct_method_arity.clone();
                // 同上：方法体 ctx 需枚举/类型注册表（lib_hashmap 实测）
                method_ctx.enum_variants = ctx.enum_variants.clone();
                method_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
                method_ctx.fn_returns = ctx.fn_returns.clone();
                // magic 方法同样绑定 self_ty（同 struct 内联方法）
                method_ctx.self_ty = Some(IrType::Named {
                    path: s.name.clone(),
                    args: s
                        .generics
                        .iter()
                        .map(|g| IrType::Generic(g.clone()))
                        .collect(),
                });
                convert_fn_def(m, &method_ctx)
            })
            .collect();

        let mut methods = methods;
        methods.extend(magic_methods);

        // 提取 __new__ 的签名信息（构造器魔法可能落在 s.methods 或 s.magic_methods）
        let new_method = s
            .magic_methods
            .iter()
            .chain(s.methods.iter())
            .find(|m| m.name == "__new__");
        let has_new = new_method.is_some();
        let new_params: Vec<(String, IrType)> = new_method
            .iter()
            .flat_map(|m| {
                m.params
                    .iter()
                    .map(|p| (p.name.clone(), from_ast_type(&p.ty)))
            })
            .collect();
        let new_ret_ty = new_method.and_then(|m| m.return_type.as_ref().map(|t| from_ast_type(t)));
        // __new__ 用户体：struct 体内定义时保留，codegen 用其生成真实构造体
        // 必须用 method_ctx（含 self_ty）转换，否则 Self(...) 构造无法识别
        let new_body: Option<Block> = new_method.map(|m| {
            let mut method_ctx = TypeCtx::new();
            method_ctx.pending_items = ctx.pending_items.clone();
            method_ctx.struct_names = ctx.struct_names.clone();
            method_ctx.struct_fields = ctx.struct_fields.clone();
            method_ctx.struct_field_order = ctx.struct_field_order.clone();
            method_ctx.struct_methods = ctx.struct_methods.clone();
            method_ctx.struct_method_arity = ctx.struct_method_arity.clone();
            method_ctx.enum_variants = ctx.enum_variants.clone();
            method_ctx.enum_variant_field_types = ctx.enum_variant_field_types.clone();
            method_ctx.fn_returns = ctx.fn_returns.clone();
            method_ctx.self_ty = Some(IrType::Named {
                path: s.name.clone(),
                args: s
                    .generics
                    .iter()
                    .map(|g| IrType::Generic(g.clone()))
                    .collect(),
            });
            Block {
                stmts: convert_stmts(&m.body, &method_ctx),
                ty: m
                    .return_type
                    .as_ref()
                    .map_or(IrType::Unit, |t| from_ast_type(t)),
                span: Span::unknown(),
            }
        });

        // 提取 __init__ 的签名信息（同样可能落在 s.methods）
        let init_method = s
            .magic_methods
            .iter()
            .chain(s.methods.iter())
            .find(|m| m.name == "__init__");
        let has_init = init_method.is_some();
        let init_params: Vec<(String, IrType)> = init_method
            .iter()
            .flat_map(|m| {
                m.params
                    .iter()
                    .map(|p| (p.name.clone(), from_ast_type(&p.ty)))
            })
            .collect();
        // __init__ 用户体：struct 体内定义时保留
        let init_body: Option<Block> = init_method.map(|m| Block {
            stmts: convert_stmts(&m.body, ctx),
            ty: IrType::Unit,
            span: Span::unknown(),
        });

        // 提取 __implicit_from__ 的源类型列表（同样可能落在 s.methods）
        let implicit_froms: Vec<IrType> = s
            .magic_methods
            .iter()
            .chain(s.methods.iter())
            .filter(|m| m.name == "__implicit_from__")
            .flat_map(|m| m.params.first().map(|p| from_ast_type(&p.ty)))
            .collect();

        Item::StructDef(StructDef {
            name: s.name.clone(),
            generics: s
                .generics
                .iter()
                .map(|g| GenericParam {
                    name: g.clone(),
                    // struct 泛型内联约束（`struct Map<I: Iterator, B>` 的 I: Iterator）：
                    // 生成 `I: std::iter::Iterator`（E0220 associated type Item not found）
                    bounds: s
                        .generic_bounds
                        .iter()
                        .find(|(n, _)| n == g)
                        .map(|(_, bds)| bds.iter().map(|b| from_ast_type(b)).collect())
                        .unwrap_or_default(),
                    default: s
                        .generic_defaults
                        .iter()
                        .find(|(n, _)| n == g)
                        .map(|(_, t)| from_ast_type(t)),
                })
                .collect(),
            fields,
            methods,
            is_case: s.is_case || has_case_decorator,
            has_new,
            new_params,
            new_ret_ty,
            new_body,
            has_init,
            init_params,
            init_body,
            implicit_froms,
            derives: collect_derives(&s.decorators),
            span: Span::unknown(),
        })
    }
}

pub(crate) fn convert_trait(t: &ast::TraitDef, ctx: &TypeCtx) -> Item {
    let methods: Vec<FnSig> = t
        .methods
        .iter()
        .map(|m| FnSig {
            name: m.name.clone(),
            generics: m
                .generics
                .iter()
                .map(|g| GenericParam {
                    name: g.clone(),
                    // trait 方法泛型约束（collect<C: FromIterator<Self.Item>>）：
                    // 从方法 where_clause 提取（E0423 expected value, found type
                    // parameter C——C: FromIterator 约束丢失）
                    bounds: m
                        .where_clause
                        .iter()
                        .find(|wb| wb.type_param == *g)
                        .map(|wb| wb.bounds.iter().map(|b| from_ast_type(b)).collect())
                        .unwrap_or_default(),
                    default: None,
                })
                .collect(),
            params: m
                .params
                .iter()
                .map(|p| {
                    // 保留 self 可变性：mut self → MutRef(Self_)，使 trait 声明与 impl 签名一致
                    if p.name == "self" && p.is_mut {
                        IrType::MutRef(Box::new(IrType::Self_))
                    } else {
                        let t = from_ast_type(&p.ty);
                        // ref 参数（`ref other: Self`）→ Ref(Self_)，生成 &Self 参数
                        // （否则 other: Self 报 E0277 size for Self cannot be known）
                        if p.is_ref {
                            IrType::Ref(Box::new(t))
                        } else {
                            t
                        }
                    }
                })
                .collect(),
            params_names: m.params.iter().map(|p| p.name.clone()).collect(),
            // trait 方法 where 约束（try_from ... where Self: Sized）：生成到方法签名
            where_clause: m
                .where_clause
                .iter()
                .map(|wb| {
                    let bounds: Vec<IrType> = wb.bounds.iter().map(|b| from_ast_type(b)).collect();
                    (wb.type_param.clone(), bounds)
                })
                .collect(),
            ret: m
                .return_type
                .as_ref()
                .map(|t| from_ast_type(t))
                .unwrap_or(IrType::Unit),
            // trait 默认方法体（`def describe(self) -> str = f"..."` 带 body）：
            // 否则 impl 需实现全部方法（E0046，combo-trait-impl.lz）
            body: if m.body.is_empty() {
                None
            } else {
                // 注册方法参数变量到 ctx（predicate 等 fn 参数的类型），否则
                // 调用点 callee.ty 是 Any，callee_fn_refs 不生效（E0308
                // expected &Item, found Item——find/Filter 的 predicate(item)）
                let mut body_ctx = ctx.clone();
                for p in &m.params {
                    if p.name != "self" {
                        let t = from_ast_type(&p.ty);
                        body_ctx.add_var(&p.name, t);
                    }
                }
                Some(convert_block_with_ctx(&m.body, &body_ctx))
            },
        })
        .collect();

    Item::TraitDef(TraitDef {
        name: t.name.clone(),
        generics: t
            .generics
            .iter()
            .map(|g| GenericParam {
                name: g.clone(),
                bounds: vec![],
                default: t
                    .generic_defaults
                    .iter()
                    .find(|(n, _)| n == g)
                    .map(|(_, ty)| from_ast_type(ty)),
            })
            .collect(),
        supertraits: t.supertraits.iter().map(|st| from_ast_type(st)).collect(),
        methods,
        assoc_types: t.assoc_types.clone(),
    })
}

pub(crate) fn convert_impl(imp: &ast::ImplDef, ctx: &TypeCtx) -> Item {
    let methods: Vec<FnDef> = imp
        .methods
        .iter()
        .map(|m| {
            let mut impl_ctx = TypeCtx::new();
            impl_ctx.pending_items = ctx.pending_items.clone();
            for sn in &ctx.struct_names {
                impl_ctx.struct_names.insert(sn.clone());
            }
            for (sn, fields) in &ctx.struct_fields {
                let mut cloned = HashMap::new();
                for (fn_, ty) in fields {
                    cloned.insert(fn_.clone(), ty.clone());
                }
                impl_ctx.struct_fields.insert(sn.clone(), cloned);
            }
            for (sn, order) in &ctx.struct_field_order {
                impl_ctx
                    .struct_field_order
                    .insert(sn.clone(), order.clone());
            }
            for (sn, ms) in &ctx.struct_methods {
                impl_ctx.struct_methods.insert(sn.clone(), ms.clone());
            }
            for (sn, arity) in &ctx.struct_method_arity {
                impl_ctx
                    .struct_method_arity
                    .insert(sn.clone(), arity.clone());
            }
            for (cn, ct) in &ctx.top_level_consts {
                impl_ctx.top_level_consts.insert(cn.clone(), ct.clone());
            }
            // impl 方法体内裸枚举变体名（`return Less`）需枚举映射做类型推断，
            // 否则回退 Any→i64 生成 <Ordering as ImplicitFrom<i64>> 错误转换（E0277）
            for (vn, en) in &ctx.enum_variants {
                impl_ctx.enum_variants.insert(vn.clone(), en.clone());
            }
            for (vn, ft) in &ctx.enum_variant_field_types {
                impl_ctx
                    .enum_variant_field_types
                    .insert(vn.clone(), ft.clone());
            }
            for (name, ty) in &ctx.fn_returns {
                impl_ctx.fn_returns.insert(name.clone(), ty.clone());
            }
            impl_ctx.current_generics = imp.generics.clone();
            // self 参数绑定为 impl 目标类型（如 Dict<K,V>→HashMap<K,V>）：
            // 否则 self[key] 推断为 i64、key in self 无法走 contains_key 分支（E0277/E0599）
            impl_ctx.self_ty = Some(if imp.generics.is_empty() {
                IrType::named(&imp.type_name)
            } else {
                IrType::Named {
                    path: imp.type_name.clone(),
                    args: imp
                        .generics
                        .iter()
                        .map(|g| IrType::Generic(g.clone()))
                        .collect(),
                }
            });
            convert_fn_def(m, &impl_ctx)
        })
        .collect();

    Item::Impl(ImplDef {
        trait_: imp.trait_name.as_ref().map(|n| IrType::named(n)),
        // for_type 需携带 impl 泛型参数：impl Box<T> → Box<T>
        for_type: if imp.generics.is_empty() {
            IrType::named(&imp.type_name)
        } else {
            IrType::Named {
                path: imp.type_name.clone(),
                args: imp
                    .generics
                    .iter()
                    .map(|g| IrType::Generic(g.clone()))
                    .collect(),
            }
        },
        generics: {
            // 合并 where_clause / 内联约束（`impl<A: Iterator> ...` 的 A: Iterator）
            // 到泛型参数 bounds：否则生成 `impl<A: Clone + Debug>` 丢失 A: Iterator，
            // 后续 `A::Item` 关联类型无法解析（E0220 associated type not found）
            let mut bounds_map: HashMap<String, Vec<IrType>> = HashMap::new();
            for wb in &imp.where_clause {
                let ir_bounds: Vec<IrType> = wb.bounds.iter().map(|b| from_ast_type(b)).collect();
                bounds_map
                    .entry(wb.type_param.clone())
                    .or_default()
                    .extend(ir_bounds);
            }
            imp.generics
                .iter()
                .map(|g| GenericParam {
                    name: g.clone(),
                    bounds: bounds_map.remove(g).unwrap_or_default(),
                    default: imp
                        .generic_defaults
                        .iter()
                        .find(|(n, _)| n == g)
                        .map(|(_, ty)| from_ast_type(ty)),
                })
                .collect()
        },
        methods,
        assoc_type_bindings: imp
            .assoc_type_bindings
            .iter()
            .map(|(n, t)| (n.clone(), from_ast_type(t)))
            .collect(),
        // impl 级 where 约束（`where I::Item: Clone` 的关联类型约束，type_param
        // 含点号）：不能合并到泛型参数 bounds（I.Item 不是 I），需生成 impl where
        where_clause: imp
            .where_clause
            .iter()
            .filter(|wb| wb.type_param.contains('.'))
            .map(|wb| {
                let bounds: Vec<IrType> = wb.bounds.iter().map(|b| from_ast_type(b)).collect();
                (wb.type_param.clone(), bounds)
            })
            .collect(),
    })
}

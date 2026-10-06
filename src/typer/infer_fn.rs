// Lang-Zone 编译器 — typer/infer_fn.rs
// （由 typer/mod.rs move-only 拆出，逻辑零改动）

use super::*;

impl Typer {
    /// 推断单个函数的类型
    pub(crate) fn infer_function(
        f: &mut Function,
        aliases: &HashMap<String, (Vec<String>, Type)>,
        struct_names: &std::collections::HashSet<String>,
        enum_names: &std::collections::HashSet<String>,
        enum_variants: &std::collections::HashMap<String, EnumVariant>,
        enum_variant_map: &std::collections::HashMap<String, Vec<String>>,
        callable_types: &std::collections::HashMap<String, (Vec<Type>, Type)>,
        fn_registry: &HashMap<String, FnSig>,
        struct_fields: &std::collections::HashMap<String, Vec<(String, Type)>>,
        method_registry: &std::collections::HashMap<
            String,
            std::collections::HashMap<String, (Vec<Type>, Type)>,
        >,
        instance_registry: &InstanceRegistry,
    ) -> Result<(), TypeError> {
        let mut sess = InferSession::new(
            aliases.clone(),
            struct_names.clone(),
            enum_names.clone(),
            enum_variants.clone(),
            enum_variant_map.clone(),
            callable_types.clone(),
            fn_registry.clone(),
            struct_fields.clone(),
            method_registry.clone(),
            instance_registry.clone(),
        );

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
                Some(ty) => {
                    sess.env.insert(p.name.clone(), ty.clone());
                }
                None => {
                    let tv = sess.ctx.fresh_ty(0);
                    sess.env.insert(p.name.clone(), tv);
                }
            }
        }
        // 注册 variadic 伪变量类型（避免 args/kwargs 被推断为默认 i64）
        if let Some(ref v) = f.variadic {
            if matches!(
                v.mode,
                crate::ast::VariadicMode::ArgsOnly | crate::ast::VariadicMode::Both
            ) {
                // Vec<Box<dyn Any>> 用 Generic Vec<_> 表示，让 codegen 从 fn 签名中取完整类型
                sess.env.insert(
                    "args".into(),
                    Type::Generic {
                        base: Box::new(Type::Named("Vec".into())),
                        args: vec![Type::Named("Box".into())],
                    },
                );
            }
            if matches!(
                v.mode,
                crate::ast::VariadicMode::KwargsOnly | crate::ast::VariadicMode::Both
            ) {
                // Dict<str, Box<dyn Any>> → 代码映射为 HashMap<String, Box<dyn Any>>
                sess.env.insert(
                    "kwargs".into(),
                    Type::Generic {
                        base: Box::new(Type::Named("Dict".into())),
                        args: vec![Type::Str, Type::Named("Box".into())],
                    },
                );
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
            f.generic_bounds
                .push((gen_name.clone(), vec![Type::Named("Number".into())]));

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

    /// 推断一条语句，可能修改其内部的 type 字段（设为 Type::Var 供后续 zonk）
    pub(crate) fn infer_stmt(
        sess: &mut InferSession,
        stmt: &mut Stmt,
        ret_type: &Option<Type>,
        raises_type: &Option<Type>,
    ) -> Result<(), TypeError> {
        match stmt {
            Stmt::Let {
                name, value, ty, ..
            } => {
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
            Stmt::For {
                var, iter, body, ..
            } => {
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
            Stmt::Guard {
                cond, else_body, ..
            } => {
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
                if let Some(f) = transform {
                    Self::infer_expr(sess, f)?;
                }
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
                sess.aliases
                    .insert(ta.name.clone(), (ta.generics.clone(), expanded));
                Ok(())
            }
            Stmt::If { .. } => {
                // 条件分支的类型推断（TODO: 实现详细推断逻辑）
                Ok(())
            }
            Stmt::Match { expr, arms } => {
                let scrut_ty = Self::infer_expr(sess, expr)?;
                let patterns: Vec<Pattern> = arms.iter().map(|a| a.pattern.clone()).collect();
                if let Some(msg) =
                    crate::typing::check_exhaustive(&scrut_ty, &patterns, &sess.enum_variant_map)
                {
                    sess.exhaustiveness_errors
                        .push(format!("non-exhaustive match: {}", msg));
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
    pub(crate) fn infer_expr(sess: &mut InferSession, expr: &Expr) -> Result<Type, TypeError> {
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
                Ok(sess
                    .env
                    .get(name.as_str())
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
                let types: Result<Vec<Type>, _> =
                    elems.iter().map(|e| Self::infer_expr(sess, e)).collect();
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
                        return Ok(Type::Result {
                            ok: Box::new(inner),
                            err: Box::new(err_var),
                        });
                    }
                    if name == "Err" && args.len() == 1 {
                        let inner = Self::infer_expr(sess, &args[0])?;
                        let ok_var = sess.ctx.fresh_ty(0);
                        return Ok(Type::Result {
                            ok: Box::new(ok_var),
                            err: Box::new(inner),
                        });
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
                        let arg_types: Vec<Type> = args
                            .iter()
                            .map(|a| Self::infer_expr(sess, a))
                            .collect::<Result<Vec<_>, _>>()?;

                        // 创建泛型参数替换表：T → fresh_ty, U → fresh_ty ...
                        let mut subst: HashMap<String, Type> = HashMap::new();
                        for gp in &sig.generics {
                            subst.insert(gp.clone(), sess.ctx.fresh_ty(0));
                        }

                        // 用 fresh 泛型变量替代签名中的泛型参数
                        let instantiate = |t: &Type| -> Type {
                            if subst.is_empty() {
                                t.clone()
                            } else {
                                substitute(&subst, t)
                            }
                        };
                        let sig_params: Vec<Type> =
                            sig.param_types.iter().map(|p| instantiate(p)).collect();
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
                                        if resolve_instance(
                                            &sess.instance_registry,
                                            bound_name,
                                            concrete_ty,
                                        )
                                        .is_none()
                                        {
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
                let arg_types: Result<Vec<Type>, _> =
                    args.iter().map(|a| Self::infer_expr(sess, a)).collect();
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
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
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
                                            if let Some((_, param_ty)) =
                                                fields.iter().find(|(n, _)| n == name)
                                            {
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
                    Type::Intersection(members) => members
                        .iter()
                        .filter_map(|m| resolve_type_name(m))
                        .filter_map(|tn| sess.method_registry.get(&tn))
                        .filter_map(|methods| methods.get(method.as_str()))
                        .next()
                        .cloned(),
                    _ => type_name
                        .as_ref()
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
            Expr::PathAccess { receiver, .. } => Self::infer_expr(sess, receiver),

            // 索引/下标
            Expr::Index { receiver, index } => {
                let _recv = Self::infer_expr(sess, receiver)?;
                let _idx = Self::infer_expr(sess, index)?;
                Ok(sess.ctx.fresh_ty(0))
            }

            // 控制流表达式
            Expr::If {
                cond,
                then_body,
                elif_clauses,
                else_body,
                ..
            } => {
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
                let branch_type =
                    |stmts: &[Stmt], sess: &mut InferSession| -> Result<Type, TypeError> {
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
            Expr::Match {
                expr: match_expr,
                arms,
            } => {
                let scrut_ty = Self::infer_expr(sess, match_expr)?;
                let patterns: Vec<Pattern> = arms.iter().map(|a| a.pattern.clone()).collect();
                if let Some(msg) =
                    crate::typing::check_exhaustive(&scrut_ty, &patterns, &sess.enum_variant_map)
                {
                    sess.exhaustiveness_errors
                        .push(format!("non-exhaustive match: {}", msg));
                }
                // 收集各分支结果类型并合并为最小联合类型
                let mut arm_types: Vec<Type> = Vec::new();
                for arm in arms {
                    // 解析模式对应的枚举名与变体名
                    let (enum_name, variant_name) = match &arm.pattern {
                        crate::ast::Pattern::Variant(name, _)
                        | crate::ast::Pattern::StructVariant { name, .. } => {
                            if name.contains('.') {
                                let mut parts = name.split('.');
                                let en = parts.next().unwrap_or("").to_string();
                                let vn = parts.next().unwrap_or("").to_string();
                                (en, vn)
                            } else {
                                (
                                    resolve_type_name(&scrut_ty).unwrap_or_default(),
                                    name.clone(),
                                )
                            }
                        }
                        _ => (String::new(), String::new()),
                    };

                    if !enum_name.is_empty() {
                        if let Some(variant) = sess.enum_variant(&enum_name, &variant_name).cloned()
                        {
                            let subst = fresh_subst_for_generics(&mut sess.ctx, &variant.generics);
                            let variant_return =
                                enum_self_type_with_subst(&variant, &mut sess.ctx, &subst);
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
                        Some(ty) => {
                            sess.env.insert(name, ty);
                        }
                        None => {
                            sess.env.remove(&name);
                        }
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
                        Some(ty) => {
                            sess.env.insert(name, ty);
                        }
                        None => {
                            sess.env.remove(&name);
                        }
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
                        Some(st) => {
                            let _ = unify(&mut sess.ctx, st, &et);
                        }
                        None => {
                            elem_ty = Some(et);
                        }
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
            Expr::Pipe {
                receiver,
                callee,
                args,
            } => {
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
            Expr::Try(inner)
            | Expr::Move(inner)
            | Expr::Spawn(inner)
            | Expr::Await(inner)
            | Expr::Panic(inner) => Self::infer_expr(sess, inner),
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
            Expr::ListComprehension {
                output, clauses, ..
            } => {
                // 为每个 for var in iter 子句插入循环变量到环境中
                for (var, iter) in clauses {
                    let iter_type = Self::infer_expr(sess, iter)?;
                    let elem_ty = if let Type::Generic { args, .. } = &iter_type {
                        args.first()
                            .cloned()
                            .unwrap_or_else(|| sess.ctx.fresh_ty(0))
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
            Expr::KwArg { value, .. } => Self::infer_expr(sess, value),
            Expr::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
                ..
            } => {
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
}

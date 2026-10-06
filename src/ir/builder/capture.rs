// Lang-Zone 编译器 — ir/builder/capture.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

/// 检测 AST 语句是否包含无值 return（return;）——构建块（=:/~:）内
/// return; 退出构建块自身，块值应为 Unit；此时变量类型/尾表达式均按 Unit 处理
pub(crate) fn ast_stmt_has_bare_return(stmt: &AstStmt) -> bool {
    match stmt {
        AstStmt::Return(None) => true,
        AstStmt::Return(Some(_)) => false,
        AstStmt::While {
            body, else_body, ..
        } => {
            body.iter().any(ast_stmt_has_bare_return)
                || else_body
                    .as_ref()
                    .map_or(false, |b| b.iter().any(ast_stmt_has_bare_return))
        }
        AstStmt::WhileLet {
            body, else_body, ..
        } => {
            body.iter().any(ast_stmt_has_bare_return)
                || else_body
                    .as_ref()
                    .map_or(false, |b| b.iter().any(ast_stmt_has_bare_return))
        }
        AstStmt::For {
            body, else_body, ..
        } => {
            body.iter().any(ast_stmt_has_bare_return)
                || else_body
                    .as_ref()
                    .map_or(false, |b| b.iter().any(ast_stmt_has_bare_return))
        }
        AstStmt::Loop(body)
        | AstStmt::Block { body, .. }
        | AstStmt::CheckerBlock { body, .. }
        | AstStmt::Defer(body)
        | AstStmt::Comptime { body } => body.iter().any(ast_stmt_has_bare_return),
        AstStmt::Expr(AstExpr::If {
            then_body,
            elif_clauses,
            else_body,
            ..
        }) => {
            then_body.iter().any(ast_stmt_has_bare_return)
                || elif_clauses
                    .iter()
                    .any(|(_, b)| b.iter().any(ast_stmt_has_bare_return))
                || else_body
                    .as_ref()
                    .map_or(false, |b| b.iter().any(ast_stmt_has_bare_return))
        }
        AstStmt::With { body, .. } => body.iter().any(ast_stmt_has_bare_return),
        AstStmt::Test { body, .. } => body.iter().any(ast_stmt_has_bare_return),
        AstStmt::Guard { else_body, .. } => else_body.iter().any(ast_stmt_has_bare_return),
        AstStmt::Suite {
            setup,
            teardown,
            tests,
            ..
        } => setup
            .iter()
            .flatten()
            .chain(teardown.iter().flatten())
            .chain(tests.iter())
            .any(ast_stmt_has_bare_return),
        _ => false,
    }
}

/// 检测嵌套函数体是否引用了外层函数局部变量（E0425 修复）。
/// 嵌套函数被提升为模块级 fn 后，无法访问定义它的外层函数的局部变量；
/// 返回第一个被引用的外层局部变量名（None = 无捕获）。
/// declared：嵌套函数自身参数 + 体内已声明的局部变量（按语句顺序累计遮蔽）。
pub(crate) fn check_expr_capture(
    e: &AstExpr,
    outer: &HashMap<String, IrType>,
    declared: &mut HashSet<String>,
    any: bool,
) -> Option<String> {
    match e {
        AstExpr::Ident(n) => {
            // 闭包捕获路径（any=true）需把「读外层变量」也视为捕获；
            // 默认（any=false）只拦截写（E0425），纯读取交给 analyze_global_vars
            // 提升为模块级全局（static mut + unsafe 访问），跨函数可见合法。
            if any && outer.contains_key(n.as_str()) && !declared.contains(n.as_str()) {
                return Some(n.clone());
            }
            None
        }
        AstExpr::ListLit(items) | AstExpr::SetLit(items) | AstExpr::TupleLit(items) => items
            .iter()
            .find_map(|i| check_expr_capture(i, outer, declared, any)),
        AstExpr::DictLit(items) => items.iter().find_map(|(k, v)| {
            check_expr_capture(k, outer, declared, any)
                .or_else(|| check_expr_capture(v, outer, declared, any))
        }),
        AstExpr::Binary { left, right, .. } => check_expr_capture(left, outer, declared, any)
            .or_else(|| check_expr_capture(right, outer, declared, any)),
        AstExpr::Unary { operand, .. } => check_expr_capture(operand, outer, declared, any),
        AstExpr::Call { func, args, .. } => {
            check_expr_capture(func, outer, declared, any).or_else(|| {
                args.iter()
                    .find_map(|a| check_expr_capture(a, outer, declared, any))
            })
        }
        AstExpr::KwArg { value, .. } => check_expr_capture(value, outer, declared, any),
        AstExpr::MethodCall { receiver, args, .. } => {
            check_expr_capture(receiver, outer, declared, any).or_else(|| {
                args.iter()
                    .find_map(|a| check_expr_capture(a, outer, declared, any))
            })
        }
        AstExpr::FieldAccess { receiver, .. }
        | AstExpr::PathAccess { receiver, .. }
        | AstExpr::SafeNav { receiver, .. } => check_expr_capture(receiver, outer, declared, any),
        AstExpr::Index { receiver, index } => check_expr_capture(receiver, outer, declared, any)
            .or_else(|| check_expr_capture(index, outer, declared, any)),
        AstExpr::If {
            cond,
            then_body,
            elif_clauses,
            else_body,
        } => check_expr_capture(cond, outer, declared, any)
            .or_else(|| check_stmts_capture(then_body, outer, declared, any))
            .or_else(|| {
                elif_clauses.iter().find_map(|(c, b)| {
                    check_expr_capture(c, outer, declared, any)
                        .or_else(|| check_stmts_capture(b, outer, declared, any))
                })
            })
            .or_else(|| {
                else_body
                    .as_ref()
                    .and_then(|b| check_stmts_capture(b, outer, declared, any))
            }),
        AstExpr::Match { expr, arms } => {
            check_expr_capture(expr, outer, declared, any).or_else(|| {
                arms.iter().find_map(|arm| {
                    let mut sub = declared.clone();
                    let mut pv = vec![];
                    collect_ast_pattern_vars(&arm.pattern, &mut pv);
                    for n in pv {
                        sub.insert(n);
                    }
                    if let Some(g) = &arm.guard {
                        if let Some(hit) = check_expr_capture(g, outer, &mut sub, any) {
                            return Some(hit);
                        }
                    }
                    check_stmts_capture(&arm.body, outer, &mut sub, any)
                })
            })
        }
        AstExpr::Closure { params, body, .. } => {
            let mut sub = declared.clone();
            for p in params {
                sub.insert(p.clone());
            }
            check_expr_capture(body, outer, &mut sub, any)
        }
        AstExpr::BlockExpr(body) => check_stmts_capture(body, outer, declared, any),
        AstExpr::Range { start, end, .. } => start
            .as_ref()
            .and_then(|s| check_expr_capture(s, outer, declared, any))
            .or_else(|| {
                end.as_ref()
                    .and_then(|e| check_expr_capture(e, outer, declared, any))
            }),
        AstExpr::Walrus { target, value } => check_expr_capture(target, outer, declared, any)
            .or_else(|| check_expr_capture(value, outer, declared, any)),
        AstExpr::Pipe {
            receiver,
            callee,
            args,
        } => check_expr_capture(receiver, outer, declared, any)
            .or_else(|| check_expr_capture(callee, outer, declared, any))
            .or_else(|| {
                args.iter()
                    .find_map(|a| check_expr_capture(a, outer, declared, any))
            }),
        AstExpr::NullCoalesce { left, right } => check_expr_capture(left, outer, declared, any)
            .or_else(|| check_expr_capture(right, outer, declared, any)),
        AstExpr::ListComprehension {
            output,
            var,
            iter,
            cond,
            extra_clauses,
        }
        | AstExpr::SetComprehension {
            elem: output,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            let mut sub = declared.clone();
            sub.insert(var.clone());
            for (v, i, c) in extra_clauses {
                sub.insert(v.clone());
                if let Some(hit) = check_expr_capture(i, outer, &mut sub, any) {
                    return Some(hit);
                }
                if let Some(hit) = c
                    .as_ref()
                    .and_then(|c| check_expr_capture(c, outer, &mut sub, any))
                {
                    return Some(hit);
                }
            }
            check_expr_capture(iter, outer, &mut sub, any)
                .or_else(|| {
                    cond.as_ref()
                        .and_then(|c| check_expr_capture(c, outer, &mut sub, any))
                })
                .or_else(|| check_expr_capture(output, outer, &mut sub, any))
        }
        AstExpr::DictComprehension {
            key,
            value,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            let mut sub = declared.clone();
            sub.insert(var.clone());
            for (v, i, c) in extra_clauses {
                sub.insert(v.clone());
                if let Some(hit) = check_expr_capture(i, outer, &mut sub, any) {
                    return Some(hit);
                }
                if let Some(hit) = c
                    .as_ref()
                    .and_then(|c| check_expr_capture(c, outer, &mut sub, any))
                {
                    return Some(hit);
                }
            }
            check_expr_capture(iter, outer, &mut sub, any)
                .or_else(|| {
                    cond.as_ref()
                        .and_then(|c| check_expr_capture(c, outer, &mut sub, any))
                })
                .or_else(|| check_expr_capture(key, outer, &mut sub, any))
                .or_else(|| check_expr_capture(value, outer, &mut sub, any))
        }
        AstExpr::Assign { target, value, .. } => check_expr_capture(target, outer, declared, any)
            .or_else(|| check_expr_capture(value, outer, declared, any)),
        AstExpr::Spawn(inner)
        | AstExpr::Move(inner)
        | AstExpr::Panic(inner)
        | AstExpr::Await(inner)
        | AstExpr::Try(inner)
        | AstExpr::Paren(inner)
        | AstExpr::Comptime(inner) => check_expr_capture(inner, outer, declared, any),
        AstExpr::BuildBlock { lhs, body, .. } => check_expr_capture(lhs, outer, declared, any)
            .or_else(|| check_stmts_capture(body, outer, declared, any)),
        AstExpr::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => check_stmts_capture(body, outer, declared, any)
            .or_else(|| {
                catches.iter().find_map(|arm| {
                    let mut sub = declared.clone();
                    let mut pv = vec![];
                    collect_ast_pattern_vars(&arm.pattern, &mut pv);
                    for n in pv {
                        sub.insert(n);
                    }
                    if let Some(g) = &arm.guard {
                        if let Some(hit) = check_expr_capture(g, outer, &mut sub, any) {
                            return Some(hit);
                        }
                    }
                    check_stmts_capture(&arm.body, outer, &mut sub, any)
                })
            })
            .or_else(|| {
                else_body
                    .as_ref()
                    .and_then(|b| check_stmts_capture(b, outer, declared, any))
            })
            .or_else(|| {
                finally_body
                    .as_ref()
                    .and_then(|b| check_stmts_capture(b, outer, declared, any))
            }),
        _ => None,
    }
}

pub(crate) fn check_stmts_capture(
    stmts: &[AstStmt],
    outer: &HashMap<String, IrType>,
    declared: &mut HashSet<String>,
    any: bool,
) -> Option<String> {
    for s in stmts {
        if let Some(hit) = check_stmt_capture(s, outer, declared, any) {
            return Some(hit);
        }
    }
    None
}

pub(crate) fn check_stmt_capture(
    s: &AstStmt,
    outer: &HashMap<String, IrType>,
    declared: &mut HashSet<String>,
    any: bool,
) -> Option<String> {
    match s {
        AstStmt::Let {
            name,
            mutable,
            value,
            ..
        } => {
            // 只对**写外层局部变量**报错（无 let 前缀的默认可变绑定 `total = ...`
            // 且 total 在外层作用域存在）：builder 在嵌套函数体内会生成
            // `let mut total = total + x`（新绑定自引用）→ E0425。
            // 有 let 前缀的声明（let x = v）是本函数新绑定，不报。
            // 纯读取（value 中引用外层变量）不报——会被 analyze_global_vars
            // 提升为模块级全局（static mut + unsafe 访问），跨函数可见合法。
            let hit = if *mutable
                && outer.contains_key(name.as_str())
                && !declared.contains(name.as_str())
            {
                Some(name.clone())
            } else {
                None
            };
            declared.insert(name.clone());
            hit.or_else(|| check_expr_capture(value, outer, declared, any))
        }
        AstStmt::Const { name, value, .. } => {
            let hit = check_expr_capture(value, outer, declared, any);
            declared.insert(name.clone());
            hit
        }
        AstStmt::LetTuple { names, value, .. } => {
            let hit = check_expr_capture(value, outer, declared, any);
            for n in names {
                declared.insert(n.clone());
            }
            hit
        }
        AstStmt::Expr(e) => check_expr_capture(e, outer, declared, any),
        AstStmt::Return(Some(e)) | AstStmt::Yield(Some(e)) => {
            check_expr_capture(e, outer, declared, any)
        }
        AstStmt::YieldFrom(e) | AstStmt::Raise(e) => check_expr_capture(e, outer, declared, any),
        AstStmt::While {
            cond,
            guard,
            body,
            else_body,
        } => check_expr_capture(cond, outer, declared, any)
            .or_else(|| {
                guard
                    .as_ref()
                    .and_then(|g| check_expr_capture(g, outer, declared, any))
            })
            .or_else(|| check_stmts_capture(body, outer, declared, any))
            .or_else(|| {
                else_body
                    .as_ref()
                    .and_then(|b| check_stmts_capture(b, outer, declared, any))
            }),
        AstStmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
            else_body,
        } => {
            let hit = check_expr_capture(expr, outer, declared, any).or_else(|| {
                guard
                    .as_ref()
                    .and_then(|g| check_expr_capture(g, outer, declared, any))
            });
            let mut pv = vec![];
            collect_ast_pattern_vars(pattern, &mut pv);
            for n in pv {
                declared.insert(n);
            }
            hit.or_else(|| check_stmts_capture(body, outer, declared, any))
                .or_else(|| {
                    else_body
                        .as_ref()
                        .and_then(|b| check_stmts_capture(b, outer, declared, any))
                })
        }
        AstStmt::For {
            var,
            iter,
            guard,
            body,
            else_body,
        } => {
            let hit = check_expr_capture(iter, outer, declared, any).or_else(|| {
                guard
                    .as_ref()
                    .and_then(|g| check_expr_capture(g, outer, declared, any))
            });
            declared.insert(var.clone());
            hit.or_else(|| check_stmts_capture(body, outer, declared, any))
                .or_else(|| {
                    else_body
                        .as_ref()
                        .and_then(|b| check_stmts_capture(b, outer, declared, any))
                })
        }
        AstStmt::Loop(body) => check_stmts_capture(body, outer, declared, any),
        AstStmt::Break(Some(e)) => check_expr_capture(e, outer, declared, any),
        AstStmt::BreakLabel { value, .. } => value
            .as_ref()
            .and_then(|v| check_expr_capture(v, outer, declared, any)),
        AstStmt::Block { body, .. }
        | AstStmt::CheckerBlock { body, .. }
        | AstStmt::Defer(body)
        | AstStmt::Comptime { body } => check_stmts_capture(body, outer, declared, any),
        AstStmt::Guard {
            cond,
            let_binding,
            success_expr,
            else_body,
        } => {
            let mut hit = cond
                .as_ref()
                .and_then(|c| check_expr_capture(c, outer, declared, any));
            if let Some((pat, e)) = let_binding {
                if hit.is_none() {
                    hit = check_expr_capture(e, outer, declared, any);
                }
                let mut pv = vec![];
                collect_ast_pattern_vars(pat, &mut pv);
                for n in pv {
                    declared.insert(n);
                }
            }
            if hit.is_none() {
                hit = success_expr
                    .as_ref()
                    .and_then(|s| check_expr_capture(s, outer, declared, any));
            }
            hit.or_else(|| check_stmts_capture(else_body, outer, declared, any))
        }
        AstStmt::With { expr, alias, body } => {
            let hit = check_expr_capture(expr, outer, declared, any);
            if let Some(a) = alias {
                declared.insert(a.clone());
            }
            hit.or_else(|| check_stmts_capture(body, outer, declared, any))
        }
        AstStmt::BlockCall { args, .. } => check_expr_capture(args, outer, declared, any),
        AstStmt::Assign { target, value, .. } => {
            // 写外层局部变量（total = ... / total += ...）→ 生成新绑定自引用 E0425
            if let AstExpr::Ident(n) = target {
                if !declared.contains(n.as_str()) && outer.contains_key(n.as_str()) {
                    return Some(n.clone());
                }
            }
            check_expr_capture(target, outer, declared, any)
                .or_else(|| check_expr_capture(value, outer, declared, any))
        }
        AstStmt::Test { body, .. } => check_stmts_capture(body, outer, declared, any),
        AstStmt::Assert { expr, expected, .. } => check_expr_capture(expr, outer, declared, any)
            .or_else(|| {
                expected
                    .as_ref()
                    .and_then(|e| check_expr_capture(e, outer, declared, any))
            }),
        AstStmt::Check { expr, message } => {
            check_expr_capture(expr, outer, declared, any).or_else(|| {
                message
                    .as_ref()
                    .and_then(|m| check_expr_capture(m, outer, declared, any))
            })
        }
        AstStmt::Suite {
            setup,
            teardown,
            tests,
            ..
        } => {
            let mut hit = setup
                .as_ref()
                .and_then(|s| check_stmts_capture(s, outer, declared, any));
            if hit.is_none() {
                hit = teardown
                    .as_ref()
                    .and_then(|s| check_stmts_capture(s, outer, declared, any));
            }
            if hit.is_none() {
                hit = check_stmts_capture(tests, outer, declared, any);
            }
            hit
        }
        // 嵌套函数体内的嵌套函数：由该函数自身转换时独立检查（递归）
        _ => None,
    }
}

/// 预扫描函数体，登记闭包 let 绑定为 Fn 类型（用于返回类型推断）。
/// 否则 `def f() = ... consume()` 末尾的闭包调用在推断 ret_ty 时
/// lookup_var 查不到 consume → 回退 Any→i64（E0308）。
pub(crate) fn prescan_closure_bindings(stmts: &[AstStmt], ctx: &mut TypeCtx) {
    for stmt in stmts {
        if let AstStmt::Let { name, value, .. } = stmt {
            if let AstExpr::Closure { params, body, .. } = value {
                // fat-arrow 闭包体（|..| => 块）是 BlockExpr：取最后语句类型，
                // 否则 BlockExpr 推断为 Any → 闭包 ret=Any → 调用推断 i64（E0308）
                let ret = match body.as_ref() {
                    AstExpr::BlockExpr(block) => block
                        .last()
                        .map(|s| infer_stmt_type(s, ctx))
                        .unwrap_or(IrType::Unit),
                    other => infer_expr_type(other, ctx),
                };
                let fty = IrType::Fn {
                    params: vec![IrType::Any; params.len()],
                    ret: Box::new(ret),
                };
                ctx.add_var(name, fty);
                // 预登记的闭包绑定是本函数体**声明**（`mut f = 闭包` 的 f）：
                // 标记 block_declared，否则 convert_block 预扫描（`!vars.contains`）
                // 把预注册的 f 误当外部变量跳过 block_declared 注册，后续
                // 3888 行误转 Assign（生成裸赋值 `f = move |...|` 缺 let，E0425）
                ctx.block_declared.insert(name.clone());
            }
        }
    }
}

/// 收集 checker 块体引用的外层函数局部变量（block 闭包语义，规范 05b-block命名块.md §三）。
/// checker 块被提升为模块级 fn NAME(ps: &mut __Params)，body 中引用的 main 局部变量
/// （out/depth/result 等）需作为 &mut 参数传入，否则 E0425
/// （block_demo/block_stack_test/block_tailrec）。
/// 排除：ps 参数名、模块级常量（top_level_consts 已生成全局、无需捕获）、
/// 非变量引用（函数名/内置名不在 ctx.vars 中，自动排除）。
pub(crate) fn collect_checker_captured(
    body: &Block,
    ctx: &TypeCtx,
    ps_name: Option<&str>,
) -> Vec<(String, IrType)> {
    let mut refs = Vec::new();
    let mut shadow = std::collections::HashSet::new();
    collect_var_refs(body, &mut shadow, &mut refs);
    let mut captured: Vec<(String, IrType)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for name in refs {
        if seen.contains(&name) {
            continue;
        }
        seen.insert(name.clone());
        if ps_name == Some(name.as_str()) {
            continue; // ps 参数自身
        }
        if ctx.top_level_consts.contains_key(&name) {
            continue; // 模块级 const：全局生成，无需捕获
        }
        if let Some(ty) = ctx.vars.get(&name) {
            captured.push((name, ty.clone()));
        }
    }
    captured
}

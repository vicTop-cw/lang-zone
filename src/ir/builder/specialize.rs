// Lang-Zone 编译器 — ir/builder/specialize.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

/// 递归特化一个表达式：
/// - `Ident(name)` 且 `name` 在 `subst` 中且未被 `shadow` 遮蔽 → 替换为常量字面量；
/// - 对原函数 `orig_name` 的自递归调用 → 改写为特化名 `spec_name`，丢弃 comptime 位置实参；
/// - 其余节点递归下降。
pub(crate) fn specialize_expr(
    e: &AstExpr,
    subst: &std::collections::HashMap<String, AstExpr>,
    orig_name: &str,
    spec_name: &str,
    comptime_positions: &[usize],
    shadow: &std::collections::HashSet<String>,
) -> AstExpr {
    // 标识符直接替换（未被局部绑定遮蔽时）
    if let AstExpr::Ident(name) = e {
        if !shadow.contains(name) {
            if let Some(repl) = subst.get(name) {
                return repl.clone();
            }
        }
    }
    // 对原函数的自递归调用：改写 callee 为特化名，丢弃 comptime 位置实参
    if let AstExpr::Call {
        func,
        args,
        type_args,
    } = e
    {
        if let AstExpr::Ident(fname) = func.as_ref() {
            if fname == orig_name {
                let new_args: Vec<AstExpr> = args
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !comptime_positions.contains(i))
                    .map(|(_, a)| {
                        specialize_expr(a, subst, orig_name, spec_name, comptime_positions, shadow)
                    })
                    .collect();
                return AstExpr::Call {
                    func: Box::new(AstExpr::Ident(spec_name.to_string())),
                    args: new_args,
                    type_args: type_args.clone(),
                };
            }
        }
    }

    match e {
        AstExpr::Ident(_)
        | AstExpr::IntLit(_)
        | AstExpr::Int128Lit(_)
        | AstExpr::BigIntLit(_)
        | AstExpr::ComplexLit(_, _)
        | AstExpr::FloatLit(_)
        | AstExpr::StrLit(_)
        | AstExpr::FStrLit(_)
        | AstExpr::RawStrLit(_)
        | AstExpr::BoolLit(_)
        | AstExpr::NoneLit
        | AstExpr::DefaultExpr => e.clone(),
        AstExpr::ListLit(xs) => AstExpr::ListLit(
            xs.iter()
                .map(|x| {
                    specialize_expr(x, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        ),
        AstExpr::DictLit(kvs) => AstExpr::DictLit(
            kvs.iter()
                .map(|(k, v)| {
                    (
                        specialize_expr(k, subst, orig_name, spec_name, comptime_positions, shadow),
                        specialize_expr(v, subst, orig_name, spec_name, comptime_positions, shadow),
                    )
                })
                .collect(),
        ),
        AstExpr::SetLit(xs) => AstExpr::SetLit(
            xs.iter()
                .map(|x| {
                    specialize_expr(x, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        ),
        AstExpr::TupleLit(xs) => AstExpr::TupleLit(
            xs.iter()
                .map(|x| {
                    specialize_expr(x, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        ),
        AstExpr::Spread(x) => AstExpr::Spread(Box::new(specialize_expr(
            x,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        ))),
        AstExpr::Binary { left, op, right } => AstExpr::Binary {
            left: Box::new(specialize_expr(
                left,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            op: *op,
            right: Box::new(specialize_expr(
                right,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
        },
        AstExpr::Unary { op, operand } => AstExpr::Unary {
            op: *op,
            operand: Box::new(specialize_expr(
                operand,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
        },
        AstExpr::KwArg { name, value } => AstExpr::KwArg {
            name: name.clone(),
            value: Box::new(specialize_expr(
                value,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
        },
        AstExpr::MethodCall {
            receiver,
            method,
            args,
        } => AstExpr::MethodCall {
            receiver: Box::new(specialize_expr(
                receiver,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            method: method.clone(),
            args: args
                .iter()
                .map(|a| {
                    specialize_expr(a, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        },
        AstExpr::FieldAccess { receiver, field } => AstExpr::FieldAccess {
            receiver: Box::new(specialize_expr(
                receiver,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            field: field.clone(),
        },
        AstExpr::PathAccess { receiver, segment } => AstExpr::PathAccess {
            receiver: Box::new(specialize_expr(
                receiver,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            segment: segment.clone(),
        },
        AstExpr::Index { receiver, index } => AstExpr::Index {
            receiver: Box::new(specialize_expr(
                receiver,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            index: Box::new(specialize_expr(
                index,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
        },
        AstExpr::If {
            cond,
            then_body,
            elif_clauses,
            else_body,
        } => {
            let sh = shadow.clone();
            AstExpr::If {
                cond: Box::new(specialize_expr(
                    cond,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    shadow,
                )),
                then_body: then_body
                    .iter()
                    .map(|s| {
                        specialize_stmt(s, subst, orig_name, spec_name, comptime_positions, &sh)
                    })
                    .collect(),
                elif_clauses: elif_clauses
                    .iter()
                    .map(|(c, b)| {
                        (
                            specialize_expr(
                                c,
                                subst,
                                orig_name,
                                spec_name,
                                comptime_positions,
                                shadow,
                            ),
                            b.iter()
                                .map(|s| {
                                    specialize_stmt(
                                        s,
                                        subst,
                                        orig_name,
                                        spec_name,
                                        comptime_positions,
                                        &sh,
                                    )
                                })
                                .collect(),
                        )
                    })
                    .collect(),
                else_body: else_body.as_ref().map(|b| {
                    b.iter()
                        .map(|s| {
                            specialize_stmt(s, subst, orig_name, spec_name, comptime_positions, &sh)
                        })
                        .collect()
                }),
            }
        }
        AstExpr::Match { expr, arms } => {
            let sh = shadow.clone();
            AstExpr::Match {
                expr: Box::new(specialize_expr(
                    expr,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    shadow,
                )),
                arms: arms
                    .iter()
                    .map(|a| {
                        specialize_match_arm(
                            a,
                            subst,
                            orig_name,
                            spec_name,
                            comptime_positions,
                            &sh,
                        )
                    })
                    .collect(),
            }
        }
        AstExpr::Closure {
            params,
            param_tys,
            ret_ty,
            body,
        } => {
            let mut sh = shadow.clone();
            for p in params {
                sh.insert(p.clone());
            }
            AstExpr::Closure {
                params: params.clone(),
                param_tys: param_tys.clone(),
                ret_ty: ret_ty.clone(),
                body: Box::new(specialize_expr(
                    body,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    &sh,
                )),
            }
        }
        AstExpr::BlockExpr(stmts) => {
            let sh = shadow.clone();
            AstExpr::BlockExpr(
                stmts
                    .iter()
                    .map(|s| {
                        specialize_stmt(s, subst, orig_name, spec_name, comptime_positions, &sh)
                    })
                    .collect(),
            )
        }
        AstExpr::Range {
            start,
            end,
            inclusive,
        } => AstExpr::Range {
            start: start.as_ref().map(|s| {
                Box::new(specialize_expr(
                    s,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    shadow,
                ))
            }),
            end: end.as_ref().map(|s| {
                Box::new(specialize_expr(
                    s,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    shadow,
                ))
            }),
            inclusive: *inclusive,
        },
        AstExpr::Walrus { target, value } => AstExpr::Walrus {
            target: Box::new(specialize_expr(
                target,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            value: Box::new(specialize_expr(
                value,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
        },
        AstExpr::Pipe {
            receiver,
            callee,
            args,
        } => AstExpr::Pipe {
            receiver: Box::new(specialize_expr(
                receiver,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            callee: Box::new(specialize_expr(
                callee,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            args: args
                .iter()
                .map(|a| {
                    specialize_expr(a, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        },
        AstExpr::SafeNav { receiver, field } => AstExpr::SafeNav {
            receiver: Box::new(specialize_expr(
                receiver,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            field: field.clone(),
        },
        AstExpr::Try(x) => AstExpr::Try(Box::new(specialize_expr(
            x,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        ))),
        AstExpr::NullCoalesce { left, right } => AstExpr::NullCoalesce {
            left: Box::new(specialize_expr(
                left,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            right: Box::new(specialize_expr(
                right,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
        },
        AstExpr::ListComprehension {
            output,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            let mut sh = shadow.clone();
            sh.insert(var.clone());
            for (cv, _, _) in extra_clauses {
                sh.insert(cv.clone());
            }
            AstExpr::ListComprehension {
                output: Box::new(specialize_expr(
                    output,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    &sh,
                )),
                var: var.clone(),
                iter: Box::new(specialize_expr(
                    iter,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    &sh,
                )),
                cond: cond.as_ref().map(|c| {
                    Box::new(specialize_expr(
                        c,
                        subst,
                        orig_name,
                        spec_name,
                        comptime_positions,
                        &sh,
                    ))
                }),
                extra_clauses: extra_clauses
                    .iter()
                    .map(|(cv, it, c)| {
                        (
                            cv.clone(),
                            Box::new(specialize_expr(
                                it,
                                subst,
                                orig_name,
                                spec_name,
                                comptime_positions,
                                &sh,
                            )),
                            c.as_ref().map(|cc| {
                                Box::new(specialize_expr(
                                    cc,
                                    subst,
                                    orig_name,
                                    spec_name,
                                    comptime_positions,
                                    &sh,
                                ))
                            }),
                        )
                    })
                    .collect(),
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
            let mut sh = shadow.clone();
            sh.insert(var.clone());
            for (cv, _, _) in extra_clauses {
                sh.insert(cv.clone());
            }
            AstExpr::DictComprehension {
                key: Box::new(specialize_expr(
                    key,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    &sh,
                )),
                value: Box::new(specialize_expr(
                    value,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    &sh,
                )),
                var: var.clone(),
                iter: Box::new(specialize_expr(
                    iter,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    &sh,
                )),
                cond: cond.as_ref().map(|c| {
                    Box::new(specialize_expr(
                        c,
                        subst,
                        orig_name,
                        spec_name,
                        comptime_positions,
                        &sh,
                    ))
                }),
                extra_clauses: extra_clauses
                    .iter()
                    .map(|(cv, it, c)| {
                        (
                            cv.clone(),
                            Box::new(specialize_expr(
                                it,
                                subst,
                                orig_name,
                                spec_name,
                                comptime_positions,
                                &sh,
                            )),
                            c.as_ref().map(|cc| {
                                Box::new(specialize_expr(
                                    cc,
                                    subst,
                                    orig_name,
                                    spec_name,
                                    comptime_positions,
                                    &sh,
                                ))
                            }),
                        )
                    })
                    .collect(),
            }
        }
        AstExpr::SetComprehension {
            elem,
            var,
            iter,
            cond,
            extra_clauses,
        } => {
            let mut sh = shadow.clone();
            sh.insert(var.clone());
            for (cv, _, _) in extra_clauses {
                sh.insert(cv.clone());
            }
            AstExpr::SetComprehension {
                elem: Box::new(specialize_expr(
                    elem,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    &sh,
                )),
                var: var.clone(),
                iter: Box::new(specialize_expr(
                    iter,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    &sh,
                )),
                cond: cond.as_ref().map(|c| {
                    Box::new(specialize_expr(
                        c,
                        subst,
                        orig_name,
                        spec_name,
                        comptime_positions,
                        &sh,
                    ))
                }),
                extra_clauses: extra_clauses
                    .iter()
                    .map(|(cv, it, c)| {
                        (
                            cv.clone(),
                            Box::new(specialize_expr(
                                it,
                                subst,
                                orig_name,
                                spec_name,
                                comptime_positions,
                                &sh,
                            )),
                            c.as_ref().map(|cc| {
                                Box::new(specialize_expr(
                                    cc,
                                    subst,
                                    orig_name,
                                    spec_name,
                                    comptime_positions,
                                    &sh,
                                ))
                            }),
                        )
                    })
                    .collect(),
            }
        }
        AstExpr::Assign { target, op, value } => AstExpr::Assign {
            target: Box::new(specialize_expr(
                target,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            op: *op,
            value: Box::new(specialize_expr(
                value,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
        },
        AstExpr::Spawn(x) => AstExpr::Spawn(Box::new(specialize_expr(
            x,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        ))),
        AstExpr::Move(x) => AstExpr::Move(Box::new(specialize_expr(
            x,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        ))),
        AstExpr::Panic(x) => AstExpr::Panic(Box::new(specialize_expr(
            x,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        ))),
        AstExpr::Await(x) => AstExpr::Await(Box::new(specialize_expr(
            x,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        ))),
        AstExpr::BuildBlock { kind, lhs, body } => {
            let sh = shadow.clone();
            AstExpr::BuildBlock {
                kind: *kind,
                lhs: Box::new(specialize_expr(
                    lhs,
                    subst,
                    orig_name,
                    spec_name,
                    comptime_positions,
                    shadow,
                )),
                body: body
                    .iter()
                    .map(|s| {
                        specialize_stmt(s, subst, orig_name, spec_name, comptime_positions, &sh)
                    })
                    .collect(),
            }
        }
        AstExpr::TryCatch {
            body,
            catches,
            else_body,
            finally_body,
        } => {
            let sh = shadow.clone();
            AstExpr::TryCatch {
                body: body
                    .iter()
                    .map(|s| {
                        specialize_stmt(s, subst, orig_name, spec_name, comptime_positions, &sh)
                    })
                    .collect(),
                catches: catches
                    .iter()
                    .map(|a| {
                        specialize_match_arm(
                            a,
                            subst,
                            orig_name,
                            spec_name,
                            comptime_positions,
                            &sh,
                        )
                    })
                    .collect(),
                else_body: else_body.as_ref().map(|b| {
                    b.iter()
                        .map(|s| {
                            specialize_stmt(s, subst, orig_name, spec_name, comptime_positions, &sh)
                        })
                        .collect()
                }),
                finally_body: finally_body.as_ref().map(|b| {
                    b.iter()
                        .map(|s| {
                            specialize_stmt(s, subst, orig_name, spec_name, comptime_positions, &sh)
                        })
                        .collect()
                }),
            }
        }
        AstExpr::Paren(x) => AstExpr::Paren(Box::new(specialize_expr(
            x,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        ))),
        AstExpr::Comptime(x) => AstExpr::Comptime(Box::new(specialize_expr(
            x,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        ))),
        AstExpr::Call {
            func,
            args,
            type_args,
        } => AstExpr::Call {
            func: Box::new(specialize_expr(
                func,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            )),
            args: args
                .iter()
                .map(|a| {
                    specialize_expr(a, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
            type_args: type_args.clone(),
        },
    }
}

pub(crate) fn specialize_stmt(
    s: &AstStmt,
    subst: &std::collections::HashMap<String, AstExpr>,
    orig_name: &str,
    spec_name: &str,
    comptime_positions: &[usize],
    shadow: &std::collections::HashSet<String>,
) -> AstStmt {
    match s {
        AstStmt::Expr(e) => AstStmt::Expr(specialize_expr(
            e,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        )),
        AstStmt::Let {
            name,
            mutable,
            is_ref,
            is_owned,
            ty,
            value,
            mods,
        } => {
            let mut sh = shadow.clone();
            sh.insert(name.clone());
            AstStmt::Let {
                name: name.clone(),
                mutable: *mutable,
                is_ref: *is_ref,
                is_owned: *is_owned,
                ty: ty.clone(),
                value: specialize_expr(value, subst, orig_name, spec_name, comptime_positions, &sh),
                mods: mods.clone(),
            }
        }
        AstStmt::Const {
            name,
            ty,
            value,
            mods,
        } => {
            let mut sh = shadow.clone();
            sh.insert(name.clone());
            AstStmt::Const {
                name: name.clone(),
                ty: ty.clone(),
                value: specialize_expr(value, subst, orig_name, spec_name, comptime_positions, &sh),
                mods: mods.clone(),
            }
        }
        AstStmt::Return(opt) => {
            AstStmt::Return(opt.as_ref().map(|e| {
                specialize_expr(e, subst, orig_name, spec_name, comptime_positions, shadow)
            }))
        }
        AstStmt::Yield(opt) => {
            AstStmt::Yield(opt.as_ref().map(|e| {
                specialize_expr(e, subst, orig_name, spec_name, comptime_positions, shadow)
            }))
        }
        AstStmt::YieldFrom(e) => AstStmt::YieldFrom(specialize_expr(
            e,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        )),
        AstStmt::While {
            cond,
            guard,
            body,
            else_body,
        } => AstStmt::While {
            cond: specialize_expr(
                cond,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            ),
            guard: guard.as_ref().map(|g| {
                specialize_expr(g, subst, orig_name, spec_name, comptime_positions, shadow)
            }),
            body: body
                .iter()
                .map(|st| {
                    specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
            else_body: else_body.as_ref().map(|b| {
                b.iter()
                    .map(|st| {
                        specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                    })
                    .collect()
            }),
        },
        AstStmt::WhileLet {
            pattern,
            expr,
            guard,
            body,
            else_body,
        } => {
            let mut sh = shadow.clone();
            pattern_bound_names(pattern, &mut sh);
            AstStmt::WhileLet {
                pattern: pattern.clone(),
                expr: specialize_expr(expr, subst, orig_name, spec_name, comptime_positions, &sh),
                guard: guard.as_ref().map(|g| {
                    specialize_expr(g, subst, orig_name, spec_name, comptime_positions, &sh)
                }),
                body: body
                    .iter()
                    .map(|st| {
                        specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, &sh)
                    })
                    .collect(),
                else_body: else_body.as_ref().map(|b| {
                    b.iter()
                        .map(|st| {
                            specialize_stmt(
                                st,
                                subst,
                                orig_name,
                                spec_name,
                                comptime_positions,
                                &sh,
                            )
                        })
                        .collect()
                }),
            }
        }
        AstStmt::For {
            var,
            iter,
            guard,
            body,
            else_body,
        } => {
            let mut sh = shadow.clone();
            sh.insert(var.clone());
            AstStmt::For {
                var: var.clone(),
                iter: specialize_expr(iter, subst, orig_name, spec_name, comptime_positions, &sh),
                guard: guard.as_ref().map(|g| {
                    specialize_expr(g, subst, orig_name, spec_name, comptime_positions, &sh)
                }),
                body: body
                    .iter()
                    .map(|st| {
                        specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, &sh)
                    })
                    .collect(),
                else_body: else_body.as_ref().map(|b| {
                    b.iter()
                        .map(|st| {
                            specialize_stmt(
                                st,
                                subst,
                                orig_name,
                                spec_name,
                                comptime_positions,
                                &sh,
                            )
                        })
                        .collect()
                }),
            }
        }
        AstStmt::Loop(stmts) => AstStmt::Loop(
            stmts
                .iter()
                .map(|st| {
                    specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        ),
        AstStmt::Break(opt) => {
            AstStmt::Break(opt.as_ref().map(|e| {
                specialize_expr(e, subst, orig_name, spec_name, comptime_positions, shadow)
            }))
        }
        AstStmt::BreakLabel { label, value } => AstStmt::BreakLabel {
            label: label.clone(),
            value: value.as_ref().map(|e| {
                specialize_expr(e, subst, orig_name, spec_name, comptime_positions, shadow)
            }),
        },
        AstStmt::Continue => AstStmt::Continue,
        AstStmt::Block { label, body } => AstStmt::Block {
            label: label.clone(),
            body: body
                .iter()
                .map(|st| {
                    specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        },
        AstStmt::CheckerBlock {
            label,
            ps_name,
            default_checker,
            body,
        } => AstStmt::CheckerBlock {
            label: label.clone(),
            ps_name: ps_name.clone(),
            default_checker: default_checker.clone(),
            body: body
                .iter()
                .map(|st| {
                    specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        },
        AstStmt::Defer(stmts) => AstStmt::Defer(
            stmts
                .iter()
                .map(|st| {
                    specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        ),
        AstStmt::Raise(e) => AstStmt::Raise(specialize_expr(
            e,
            subst,
            orig_name,
            spec_name,
            comptime_positions,
            shadow,
        )),
        AstStmt::Guard {
            cond,
            let_binding,
            success_expr,
            else_body,
        } => {
            let mut sh = shadow.clone();
            if let Some((pat, _)) = let_binding {
                pattern_bound_names(pat, &mut sh);
            }
            AstStmt::Guard {
                cond: cond.as_ref().map(|c| {
                    specialize_expr(c, subst, orig_name, spec_name, comptime_positions, &sh)
                }),
                let_binding: let_binding.as_ref().map(|(pat, val)| {
                    (
                        pat.clone(),
                        specialize_expr(val, subst, orig_name, spec_name, comptime_positions, &sh),
                    )
                }),
                success_expr: success_expr.as_ref().map(|c| {
                    specialize_expr(c, subst, orig_name, spec_name, comptime_positions, &sh)
                }),
                else_body: else_body
                    .iter()
                    .map(|st| {
                        specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, &sh)
                    })
                    .collect(),
            }
        }
        AstStmt::With { expr, alias, body } => {
            let mut sh = shadow.clone();
            if let Some(a) = alias {
                sh.insert(a.clone());
            }
            AstStmt::With {
                expr: specialize_expr(expr, subst, orig_name, spec_name, comptime_positions, &sh),
                alias: alias.clone(),
                body: body
                    .iter()
                    .map(|st| {
                        specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, &sh)
                    })
                    .collect(),
            }
        }
        AstStmt::BlockCall { label, args } => AstStmt::BlockCall {
            label: label.clone(),
            args: specialize_expr(
                args,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            ),
        },
        // 函数体内嵌套 enum 定义：不进入特化（M3 v1 边界）
        AstStmt::EnumDef(sd) => AstStmt::EnumDef(sd.clone()),
        AstStmt::Assign { target, op, value } => AstStmt::Assign {
            target: specialize_expr(
                target,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            ),
            op: *op,
            value: specialize_expr(
                value,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            ),
        },
        AstStmt::FnDef { func } => {
            // 嵌套函数：其参数名遮蔽 comptime 形参，递归特化其函数体
            let mut sh = shadow.clone();
            for p in &func.params {
                sh.insert(p.name.clone());
            }
            let new_body: Vec<AstStmt> = func
                .body
                .iter()
                .map(|st| specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, &sh))
                .collect();
            let mut nf = func.clone();
            nf.body = new_body;
            AstStmt::FnDef { func: nf }
        }
        AstStmt::Pass => AstStmt::Pass,
        AstStmt::Test { name, body } => AstStmt::Test {
            name: name.clone(),
            body: body
                .iter()
                .map(|st| {
                    specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        },
        AstStmt::Assert {
            expr,
            expected,
            message,
        } => AstStmt::Assert {
            expr: specialize_expr(
                expr,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            ),
            expected: expected.as_ref().map(|e| {
                specialize_expr(e, subst, orig_name, spec_name, comptime_positions, shadow)
            }),
            message: message.as_ref().map(|e| {
                specialize_expr(e, subst, orig_name, spec_name, comptime_positions, shadow)
            }),
        },
        AstStmt::Check { expr, message } => AstStmt::Check {
            expr: specialize_expr(
                expr,
                subst,
                orig_name,
                spec_name,
                comptime_positions,
                shadow,
            ),
            message: message.as_ref().map(|e| {
                specialize_expr(e, subst, orig_name, spec_name, comptime_positions, shadow)
            }),
        },
        AstStmt::Suite {
            name,
            setup,
            teardown,
            tests,
        } => AstStmt::Suite {
            name: name.clone(),
            setup: setup.as_ref().map(|s| {
                s.iter()
                    .map(|st| {
                        specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                    })
                    .collect()
            }),
            teardown: teardown.as_ref().map(|s| {
                s.iter()
                    .map(|st| {
                        specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                    })
                    .collect()
            }),
            tests: tests
                .iter()
                .map(|st| {
                    specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        },
        AstStmt::Comptime { body } => AstStmt::Comptime {
            body: body
                .iter()
                .map(|st| {
                    specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, shadow)
                })
                .collect(),
        },
        AstStmt::TypeAlias { name, ty } => AstStmt::TypeAlias {
            name: name.clone(),
            ty: ty.clone(),
        },
        AstStmt::LetTuple { names, ty, value } => {
            let mut sh = shadow.clone();
            for n in names {
                sh.insert(n.clone());
            }
            AstStmt::LetTuple {
                names: names.clone(),
                ty: ty.clone(),
                value: specialize_expr(value, subst, orig_name, spec_name, comptime_positions, &sh),
            }
        }
        AstStmt::EmbedBlock { .. } => s.clone(),
    }
}

pub(crate) fn specialize_match_arm(
    arm: &crate::ast::MatchArm,
    subst: &std::collections::HashMap<String, AstExpr>,
    orig_name: &str,
    spec_name: &str,
    comptime_positions: &[usize],
    shadow: &std::collections::HashSet<String>,
) -> crate::ast::MatchArm {
    let mut sh = shadow.clone();
    pattern_bound_names(&arm.pattern, &mut sh);
    crate::ast::MatchArm {
        pattern: arm.pattern.clone(),
        guard: arm
            .guard
            .as_ref()
            .map(|g| specialize_expr(g, subst, orig_name, spec_name, comptime_positions, &sh)),
        body: arm
            .body
            .iter()
            .map(|st| specialize_stmt(st, subst, orig_name, spec_name, comptime_positions, &sh))
            .collect(),
    }
}

/// 构造特化函数：克隆原函数，移除 comptime 形参，替换其引用为编译期常量，
/// 自递归调用改写为 mangled 名（丢弃 comptime 位置实参）。
pub(crate) fn build_specialized_fn(
    orig: &ast::Function,
    comptime_positions: &[usize],
    subst: &std::collections::HashMap<String, AstExpr>,
    spec_name: &str,
) -> ast::Function {
    let cp_set: std::collections::HashSet<usize> = comptime_positions.iter().cloned().collect();
    let new_params: Vec<ast::Param> = orig
        .params
        .iter()
        .enumerate()
        .filter(|(i, _)| !cp_set.contains(i))
        .map(|(_, p)| p.clone())
        .collect();
    let new_body: Vec<AstStmt> = orig
        .body
        .iter()
        .map(|s| {
            specialize_stmt(
                s,
                subst,
                &orig.name,
                spec_name,
                comptime_positions,
                &std::collections::HashSet::new(),
            )
        })
        .collect();
    ast::Function {
        name: spec_name.to_string(),
        generics: orig.generics.clone(),
        generic_defaults: orig.generic_defaults.clone(),
        params: new_params,
        return_type: orig.return_type.clone(),
        raises: orig.raises.clone(),
        where_clause: orig.where_clause.clone(),
        body: new_body,
        is_async: orig.is_async,
        is_abstract: orig.is_abstract,
        is_iterator: orig.is_iterator,
        is_magic: orig.is_magic,
        is_comptime: false,
        decorators: orig.decorators.clone(),
        variadic: orig.variadic.clone(),
        checker_param: orig.checker_param.clone(),
        default_checker: orig.default_checker.clone(),
    }
}

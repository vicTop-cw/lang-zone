// Lang-Zone 编译器 — ir/builder/pattern.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

// ══════════════════════════════════════════════════════════════
// Pattern 转换
// ══════════════════════════════════════════════════════════════

/// 将 AST Pattern 转为 IR Pattern，返回 None 表示通配（catch-all）
#[allow(dead_code)]
pub(crate) fn convert_ast_pattern(pat: &AstPattern, ctx: &TypeCtx) -> Option<Pattern> {
    match pat {
        AstPattern::Wildcard => None,
        AstPattern::Ident(name) => Some(Pattern::Ident(name.clone())),
        AstPattern::RefMutIdent(name) => Some(Pattern::RefMutIdent(name.clone())),
        AstPattern::Int(n) => Some(Pattern::Lit(LitKind::Int(*n))),
        AstPattern::Int128(n) => Some(Pattern::Lit(LitKind::Int128(*n))),
        AstPattern::BigInt(s) => Some(Pattern::Lit(LitKind::BigInt(s.clone()))),
        AstPattern::Complex(re, im) => Some(Pattern::Lit(LitKind::Complex(*re, *im))),
        AstPattern::Str(s) => Some(Pattern::Lit(LitKind::Str(s.clone()))),
        AstPattern::Bool(b) => Some(Pattern::Lit(LitKind::Bool(*b))),
        AstPattern::Variant(name, args) => {
            let ir_args: Vec<Pattern> = args
                .iter()
                .map(|a| convert_ast_pattern(a, ctx).unwrap_or(Pattern::Wildcard))
                .collect();

            // 区分 struct 解构 vs enum 变体模式
            if ctx.is_struct(name) {
                // struct 模式: Point(px, py) → Point { x: px, y: py }
                let field_names: Vec<String> = ctx
                    .struct_fields
                    .get(name)
                    .map(|fields| fields.keys().cloned().collect())
                    .unwrap_or_default();
                let fields: Vec<(String, Pattern)> = ir_args
                    .into_iter()
                    .enumerate()
                    .map(|(i, pat)| {
                        let fname = field_names
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| format!("field_{}", i));
                        (fname, pat)
                    })
                    .collect();
                return Some(Pattern::Struct {
                    name: name.clone(),
                    fields,
                });
            }
            // enum 变体模式
            let (enum_name, variant) = if let Some(dot_pos) = name.rfind('.') {
                (name[..dot_pos].to_string(), name[dot_pos + 1..].to_string())
            } else {
                let enum_name = ctx
                    .enum_variants
                    .get(name.as_str())
                    .cloned()
                    .unwrap_or_else(|| match name.as_str() {
                        "Some" | "None" => "Option".into(),
                        "Ok" | "Err" => "Result".into(),
                        _ => "Error".into(),
                    });
                (enum_name, name.clone())
            };
            Some(Pattern::Enum {
                enum_name,
                variant,
                args: ir_args,
            })
        }
        AstPattern::Tuple(elems) => {
            let ir_elems: Vec<Pattern> = elems
                .iter()
                .map(|e| convert_ast_pattern(e, ctx).unwrap_or(Pattern::Wildcard))
                .collect();
            Some(Pattern::Tuple(ir_elems))
        }
        AstPattern::List(elems) => {
            let ir_elems: Vec<Pattern> = elems
                .iter()
                .map(|e| convert_ast_pattern(e, ctx).unwrap_or(Pattern::Wildcard))
                .collect();
            Some(Pattern::List(ir_elems))
        }
        AstPattern::Dict(entries) => {
            let ir_entries: Vec<(String, Pattern)> = entries
                .iter()
                .map(|(k, p)| {
                    (
                        k.clone(),
                        convert_ast_pattern(p, ctx).unwrap_or(Pattern::Wildcard),
                    )
                })
                .collect();
            Some(Pattern::Dict(ir_entries))
        }
        AstPattern::Rest(name) => Some(Pattern::Rest(name.clone())),
        AstPattern::Range {
            start,
            end,
            inclusive,
        } => Some(Pattern::Range {
            start: *start,
            end: *end,
            inclusive: *inclusive,
        }),
    }
}

/// 收集 AST Pattern 中绑定的所有变量名
pub(crate) fn collect_ast_pattern_vars(pat: &AstPattern, out: &mut Vec<String>) {
    match pat {
        AstPattern::Wildcard => {}
        AstPattern::Ident(name) => {
            out.push(name.clone());
        }
        AstPattern::RefMutIdent(name) => {
            out.push(name.clone());
        }
        AstPattern::Int(_)
        | AstPattern::Int128(_)
        | AstPattern::BigInt(_)
        | AstPattern::Complex(_, _)
        | AstPattern::Str(_)
        | AstPattern::Bool(_) => {}
        AstPattern::Variant(_, args) | AstPattern::Tuple(args) | AstPattern::List(args) => {
            for a in args {
                collect_ast_pattern_vars(a, out);
            }
        }
        AstPattern::Dict(entries) => {
            for (_, p) in entries {
                collect_ast_pattern_vars(p, out);
            }
        }
        AstPattern::Rest(name) => {
            if let Some(n) = name {
                out.push(n.clone());
            }
        }
        AstPattern::Range { .. } => {}
    }
}

/// 内置 Option/Result 变体模式的字段绑定：
/// `Some(v)` / `Ok(v)` / `Err(e)` → v/e 绑定为内层类型（而非整个 scrutinee 类型）。
/// 返回 None 表示非内置变体模式。
pub(crate) fn field_types_for_builtin_variant(
    pat: &AstPattern,
    scrut_ty: &IrType,
) -> Option<Vec<(String, IrType)>> {
    let (vname, field_pats) = match pat {
        AstPattern::Variant(name, args) => (name, args),
        _ => return None,
    };
    let vbase = vname.rsplit('.').next().unwrap_or(vname);
    match (vbase, scrut_ty) {
        ("Some", IrType::Option(inner)) => {
            if let AstPattern::RefMutIdent(bind) = field_pats.first()? {
                // `Some(ref mut c)`：c 登记为 MutRef 内层类型，臂体内 c = c + 1
                // 生成 *c = *c + 1（解引用赋值，E0384 修复）
                Some(vec![(
                    bind.clone(),
                    IrType::MutRef(Box::new(inner.as_ref().clone())),
                )])
            } else if let AstPattern::Ident(bind) = field_pats.first()? {
                Some(vec![(bind.clone(), inner.as_ref().clone())])
            } else {
                None
            }
        }
        ("Ok", IrType::Result { ok, .. }) => {
            if let AstPattern::RefMutIdent(bind) = field_pats.first()? {
                Some(vec![(
                    bind.clone(),
                    IrType::MutRef(Box::new(ok.as_ref().clone())),
                )])
            } else if let AstPattern::Ident(bind) = field_pats.first()? {
                Some(vec![(bind.clone(), ok.as_ref().clone())])
            } else {
                None
            }
        }
        ("Err", IrType::Result { err, .. }) => {
            if let AstPattern::RefMutIdent(bind) = field_pats.first()? {
                Some(vec![(
                    bind.clone(),
                    IrType::MutRef(Box::new(err.as_ref().clone())),
                )])
            } else if let AstPattern::Ident(bind) = field_pats.first()? {
                Some(vec![(bind.clone(), err.as_ref().clone())])
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 变体模式字段绑定：`Shape::Circle(x: _, y: _, radius: r)` → r 绑定为
/// radius 字段类型（int）而非整个 scrutinee 类型。按字段位置匹配。
pub(crate) fn field_types_for_variant(
    pat: &AstPattern,
    ctx: &TypeCtx,
) -> Option<Vec<(String, IrType)>> {
    let (vname, field_pats) = match pat {
        AstPattern::Variant(name, args) => (name, args),
        _ => return None,
    };
    // 变体名可能是 "Shape.Circle" 或 "Circle"
    let vbase = vname.rsplit('.').next().unwrap_or(vname);
    let ftypes = ctx.enum_variant_field_types.get(vbase)?;
    let mut out = Vec::new();
    for (i, fp) in field_pats.iter().enumerate() {
        if let AstPattern::Ident(bind) = fp {
            if let Some(fty) = ftypes.get(i) {
                out.push((bind.clone(), fty.clone()));
            }
        }
    }
    Some(out)
}

/// 收集 IR Pattern 中绑定的所有变量名
#[allow(dead_code)]
pub(crate) fn collect_pattern_vars(pat: &Pattern, out: &mut Vec<String>) {
    match pat {
        Pattern::Wildcard => {}
        Pattern::Ident(name) => {
            out.push(name.clone());
        }
        Pattern::RefMutIdent(name) => {
            out.push(name.clone());
        }
        Pattern::Lit(_) => {}
        Pattern::Tuple(elems) | Pattern::List(elems) => {
            for e in elems {
                collect_pattern_vars(e, out);
            }
        }
        Pattern::Dict(entries) => {
            for (_, p) in entries {
                collect_pattern_vars(p, out);
            }
        }
        Pattern::Rest(name) => {
            if let Some(n) = name {
                out.push(n.clone());
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields {
                collect_pattern_vars(p, out);
            }
        }
        Pattern::Enum { args, .. } => {
            for a in args {
                collect_pattern_vars(a, out);
            }
        }
        Pattern::Range { .. } => {}
    }
}

// （arm_body_to_expr 已移除 — Match 表达式现在通过 BlockExpr + Stmt::Match 处理）

// ══════════════════════════════════════════════════════════════
// 核心转换函数
// ══════════════════════════════════════════════════════════════

/// 构建多 for 推导链（2+ 子句）：
/// `[out for x in a for y in b]` → `comp_outer!(|x| comp_leaf!(|y| out, b), a)`
/// codegen 端展开为 `(a).into_iter().flat_map(|x| (b).into_iter().map(|y| out)).collect()`。
/// `kind` 为 `comp` / `dict_comp` / `set_comp`，决定最外层 callee 前缀。
pub(crate) fn build_multi_comp(
    ctx: &TypeCtx,
    clauses: &[(String, Box<AstExpr>, Option<Box<AstExpr>>)],
    body: Expr,
    kind: &str,
) -> ExprKind {
    let n = clauses.len();
    debug_assert!(n >= 2, "multi-comp needs >= 2 clauses");
    let mut inner = body.kind;
    // 从最内层子句开始向外包裹：最内层用 leaf（map），中间层用 mid（flat_map），最外层用 outer（flat_map + collect）
    for i in (0..n).rev() {
        let (var, iter, cond) = &clauses[i];
        let iter_expr = convert_expr(iter, ctx);
        let level = if i == 0 {
            "outer"
        } else if i == n - 1 {
            "leaf"
        } else {
            "mid"
        };
        let callee = format!("{}_{}!", kind, level);
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
                    body: Box::new(Expr::new(inner, IrType::Any, Span::unknown())),
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
        inner = ExprKind::Call {
            type_args: vec![],
            callee: Box::new(Expr::new(
                ExprKind::Var(callee),
                IrType::Any,
                Span::unknown(),
            )),
            args,
        };
    }
    inner
}

/// 收集模式（Pattern）中绑定的变量名，用于遮蔽判断（避免误替换闭包/for/let 内的同名标识符）。
pub(crate) fn pattern_bound_names(p: &AstPattern, out: &mut std::collections::HashSet<String>) {
    match p {
        AstPattern::Ident(n) => {
            out.insert(n.clone());
        }
        AstPattern::RefMutIdent(n) => {
            out.insert(n.clone());
        }
        AstPattern::Variant(_, ps) => {
            for sub in ps {
                pattern_bound_names(sub, out);
            }
        }
        AstPattern::Tuple(ps) => {
            for sub in ps {
                pattern_bound_names(sub, out);
            }
        }
        AstPattern::List(ps) => {
            for sub in ps {
                pattern_bound_names(sub, out);
            }
        }
        AstPattern::Dict(kvs) => {
            for (_, sub) in kvs {
                pattern_bound_names(sub, out);
            }
        }
        AstPattern::Rest(opt) => {
            if let Some(n) = opt {
                out.insert(n.clone());
            }
        }
        _ => {}
    }
}

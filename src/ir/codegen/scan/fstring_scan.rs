// Lang-Zone 编译器 — ir/codegen/scan/fstring_scan.rs
// （由 scan.rs move-only 拆出，逻辑零改动；2026-10-29 大文件拆分第二批）
//
// 职责：模块级预扫描——f-string 插值引用的变量、lambda 形参绑定、
// 未绑定插值变量（供 CodeGen::unbound_fstring_vars 决定降级策略）。

use super::*;

// ── f-string 未绑定插值预扫描（lit_probe 等探测用例）──

/// 提取 f-string 中纯变量插值名（`f"hello {name}"` → "name"）；
/// 复杂插值（len(x)、a.b 等）不提取
pub(crate) fn fstring_var_interps(s: &str, out: &mut std::collections::HashSet<String>) {
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                continue;
            }
            let mut expr = String::new();
            let mut depth = 0usize;
            while let Some(&ec) = chars.peek() {
                match ec {
                    '}' if depth == 0 => {
                        chars.next();
                        break;
                    }
                    '{' => {
                        depth += 1;
                        expr.push(ec);
                        chars.next();
                    }
                    '}' => {
                        depth -= 1;
                        expr.push(ec);
                        chars.next();
                    }
                    _ => {
                        expr.push(ec);
                        chars.next();
                    }
                }
            }
            let t = expr.trim();
            // 纯标识符（含格式说明符前的冒号取前段）
            let ident = t.split(':').next().unwrap_or(t).trim();
            if !ident.is_empty()
                && ident
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                && !ident.chars().next().unwrap().is_ascii_digit()
            {
                out.insert(ident.to_string());
            }
        }
    }
}

/// 扫描表达式树，收集 f-string 插值变量引用
pub(crate) fn scan_expr_fstrings(e: &Expr, used: &mut std::collections::HashSet<String>) {
    match &e.kind {
        ExprKind::Lit(LitKind::FStr(s)) => fstring_var_interps(s, used),
        ExprKind::Lit(_) => {}
        ExprKind::Var(_) => {}
        ExprKind::Call { callee, args, .. } => {
            scan_expr_fstrings(callee, used);
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            scan_expr_fstrings(receiver, used);
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::FieldAccess { base, .. } => scan_expr_fstrings(base, used),
        ExprKind::IndexGet { base, key } => {
            scan_expr_fstrings(base, used);
            scan_expr_fstrings(key, used);
        }
        ExprKind::IndexSet { base, key, value } => {
            scan_expr_fstrings(base, used);
            scan_expr_fstrings(key, used);
            scan_expr_fstrings(value, used);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_fstrings(lhs, used);
            scan_expr_fstrings(rhs, used);
        }
        ExprKind::AssignExpr { target, value } => {
            scan_expr_fstrings(target, used);
            scan_expr_fstrings(value, used);
        }
        ExprKind::UnOp { operand, .. } => scan_expr_fstrings(operand, used),
        ExprKind::IfExpr { cond, then, els } => {
            scan_expr_fstrings(cond, used);
            scan_expr_fstrings(then, used);
            scan_expr_fstrings(els, used);
        }
        ExprKind::Lambda { body, .. } => scan_expr_fstrings(body, used),
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                scan_expr_fstrings(v, used);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::GenExpr { yield_of } => scan_expr_fstrings(yield_of, used),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                scan_expr_fstrings(c, used);
            }
            scan_block_fstrings(block, &mut std::collections::HashSet::new(), used);
        }
        ExprKind::Cast { expr, .. } => scan_expr_fstrings(expr, used),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::BlockExpr { block } => {
            scan_block_fstrings(block, &mut std::collections::HashSet::new(), used);
        }
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => {
            for a in es {
                scan_expr_fstrings(a, used);
            }
        }
        ExprKind::Dict(kvs) => {
            for (k, v) in kvs {
                scan_expr_fstrings(k, used);
                scan_expr_fstrings(v, used);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                scan_expr_fstrings(s, used);
            }
            scan_expr_fstrings(end, used);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            scan_expr_fstrings(receiver, used);
            scan_expr_fstrings(callee, used);
            for a in args {
                scan_expr_fstrings(a, used);
            }
        }
        _ => {}
    }
}

/// 收集模式中的绑定名（match 臂 / while let / catch 模式）到 bound：
/// f-string 插值若引用这些名字，不得降级为空串（lib_pattern 实测：
/// case Color.RGB(r, g, b) 臂体内 f"#{r}{g}{b}" 曾被降级为 ""）
pub(crate) fn scan_pattern_bindings(pat: &Pattern, bound: &mut std::collections::HashSet<String>) {
    match pat {
        Pattern::Ident(n) | Pattern::RefMutIdent(n) => {
            bound.insert(n.clone());
        }
        Pattern::Tuple(ps) | Pattern::List(ps) => {
            for p in ps {
                scan_pattern_bindings(p, bound);
            }
        }
        Pattern::Dict(kvps) => {
            for (_, p) in kvps {
                scan_pattern_bindings(p, bound);
            }
        }
        Pattern::Rest(Some(n)) => {
            bound.insert(n.clone());
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields {
                scan_pattern_bindings(p, bound);
            }
        }
        Pattern::Enum { args, .. } => {
            for p in args {
                scan_pattern_bindings(p, bound);
            }
        }
        _ => {}
    }
}

/// 扫描块：let 名收集到 bound，f-string 插值收集到 used
pub(crate) fn scan_block_fstrings(
    b: &Block,
    bound: &mut std::collections::HashSet<String>,
    used: &mut std::collections::HashSet<String>,
) {
    for st in &b.stmts {
        match st {
            Stmt::Let { name, value, .. } => {
                bound.insert(name.clone());
                scan_expr_fstrings(value, used);
            }
            Stmt::Assign { target, value } => {
                scan_expr_fstrings(target, used);
                scan_expr_fstrings(value, used);
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    scan_expr_fstrings(v, used);
                }
            }
            Stmt::ExprStmt { expr } => scan_expr_fstrings(expr, used),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                scan_expr_fstrings(cond, used);
                scan_block_fstrings(then_branch, bound, used);
                if let Some(eb) = else_branch {
                    scan_block_fstrings(eb, bound, used);
                }
            }
            Stmt::For {
                var,
                iter,
                guard,
                body,
                else_body,
            } => {
                bound.insert(var.clone());
                scan_expr_fstrings(iter, used);
                if let Some(g) = guard {
                    scan_expr_fstrings(g, used);
                }
                scan_block_fstrings(body, bound, used);
                if let Some(eb) = else_body {
                    scan_block_fstrings(eb, bound, used);
                }
            }
            Stmt::While {
                cond,
                guard,
                body,
                else_body,
            } => {
                scan_expr_fstrings(cond, used);
                if let Some(g) = guard {
                    scan_expr_fstrings(g, used);
                }
                scan_block_fstrings(body, bound, used);
                if let Some(eb) = else_body {
                    scan_block_fstrings(eb, bound, used);
                }
            }
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                scan_pattern_bindings(pattern, bound);
                scan_expr_fstrings(expr, used);
                if let Some(g) = guard {
                    scan_expr_fstrings(g, used);
                }
                scan_block_fstrings(body, bound, used);
            }
            Stmt::Match { scrutinee, arms } => {
                scan_expr_fstrings(scrutinee, used);
                for arm in arms {
                    // 臂模式绑定（含枚举/元组/结构子模式）也是有效绑定
                    scan_pattern_bindings(&arm.pattern, bound);
                    if let Some(g) = &arm.guard {
                        scan_expr_fstrings(g, used);
                    }
                    scan_block_fstrings(&arm.body, bound, used);
                }
            }
            Stmt::Raise { value } => scan_expr_fstrings(value, used),
            Stmt::Assert { cond, message } => {
                scan_expr_fstrings(cond, used);
                if let Some(m) = message {
                    scan_expr_fstrings(m, used);
                }
            }
            Stmt::Yield { value } => scan_expr_fstrings(value, used),
            Stmt::YieldFrom { iter } => scan_expr_fstrings(iter, used),
            Stmt::BreakLabel { value, .. } => {
                if let Some(v) = value {
                    scan_expr_fstrings(v, used);
                }
            }
            Stmt::BlockLabel { body, .. } => scan_block_fstrings(body, bound, used),
            Stmt::CheckerBlock { body, .. } => scan_block_fstrings(body, bound, used),
            Stmt::Defer { body } => scan_block_fstrings(body, bound, used),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                scan_block_fstrings(body, bound, used);
                for (cpat, cb) in catches {
                    if let Some(p) = cpat {
                        scan_pattern_bindings(p, bound);
                    }
                    scan_block_fstrings(cb, bound, used);
                }
                if let Some(eb) = else_body {
                    scan_block_fstrings(eb, bound, used);
                }
                if let Some(fb) = finally_body {
                    scan_block_fstrings(fb, bound, used);
                }
            }
            Stmt::Block { stmts } => {
                let tmp = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                    span: Span::unknown(),
                };
                scan_block_fstrings(&tmp, bound, used);
            }
            _ => {}
        }
    }
}

/// 收集表达式树中所有 lambda 参数名（f-string 插值若引用 lambda 参数，不得降级为空串）
pub(crate) fn collect_lambda_params_expr(e: &Expr, bound: &mut std::collections::HashSet<String>) {
    match &e.kind {
        ExprKind::Lambda { params, body, .. } => {
            for p in params {
                bound.insert(p.name.clone());
            }
            collect_lambda_params_expr(body, bound);
        }
        ExprKind::Call { callee, args, .. } => {
            collect_lambda_params_expr(callee, bound);
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_lambda_params_expr(receiver, bound);
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::FieldAccess { base, .. } => collect_lambda_params_expr(base, bound),
        ExprKind::IndexGet { base, key } => {
            collect_lambda_params_expr(base, bound);
            collect_lambda_params_expr(key, bound);
        }
        ExprKind::IndexSet { base, key, value } => {
            collect_lambda_params_expr(base, bound);
            collect_lambda_params_expr(key, bound);
            collect_lambda_params_expr(value, bound);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            collect_lambda_params_expr(lhs, bound);
            collect_lambda_params_expr(rhs, bound);
        }
        ExprKind::AssignExpr { target, value } => {
            collect_lambda_params_expr(target, bound);
            collect_lambda_params_expr(value, bound);
        }
        ExprKind::UnOp { operand, .. } => collect_lambda_params_expr(operand, bound),
        ExprKind::IfExpr { cond, then, els } => {
            collect_lambda_params_expr(cond, bound);
            collect_lambda_params_expr(then, bound);
            collect_lambda_params_expr(els, bound);
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                collect_lambda_params_expr(v, bound);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::GenExpr { yield_of } => collect_lambda_params_expr(yield_of, bound),
        ExprKind::GenBuild { callee, block } => {
            if let Some(c) = callee {
                collect_lambda_params_expr(c, bound);
            }
            collect_lambda_params_block(block, bound);
        }
        ExprKind::Cast { expr, .. } => collect_lambda_params_expr(expr, bound),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::BlockExpr { block } => collect_lambda_params_block(block, bound),
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => {
            for a in es {
                collect_lambda_params_expr(a, bound);
            }
        }
        ExprKind::Dict(kvs) => {
            for (k, v) in kvs {
                collect_lambda_params_expr(k, bound);
                collect_lambda_params_expr(v, bound);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_lambda_params_expr(s, bound);
            }
            collect_lambda_params_expr(end, bound);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            collect_lambda_params_expr(receiver, bound);
            collect_lambda_params_expr(callee, bound);
            for a in args {
                collect_lambda_params_expr(a, bound);
            }
        }
        _ => {}
    }
}

/// 收集 block 中所有 lambda 参数名
pub(crate) fn collect_lambda_params_block(
    b: &Block,
    bound: &mut std::collections::HashSet<String>,
) {
    for st in &b.stmts {
        match st {
            Stmt::Let { value, .. } => collect_lambda_params_expr(value, bound),
            Stmt::Assign { target, value } => {
                collect_lambda_params_expr(target, bound);
                collect_lambda_params_expr(value, bound);
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    collect_lambda_params_expr(v, bound);
                }
            }
            Stmt::ExprStmt { expr } => collect_lambda_params_expr(expr, bound),
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_lambda_params_expr(cond, bound);
                collect_lambda_params_block(then_branch, bound);
                if let Some(eb) = else_branch {
                    collect_lambda_params_block(eb, bound);
                }
            }
            Stmt::For {
                iter,
                body,
                else_body,
                ..
            } => {
                collect_lambda_params_expr(iter, bound);
                collect_lambda_params_block(body, bound);
                if let Some(eb) = else_body {
                    collect_lambda_params_block(eb, bound);
                }
            }
            Stmt::While {
                cond,
                body,
                else_body,
                ..
            } => {
                collect_lambda_params_expr(cond, bound);
                collect_lambda_params_block(body, bound);
                if let Some(eb) = else_body {
                    collect_lambda_params_block(eb, bound);
                }
            }
            Stmt::WhileLet { expr, body, .. } => {
                collect_lambda_params_expr(expr, bound);
                collect_lambda_params_block(body, bound);
            }
            Stmt::Match { scrutinee, arms } => {
                collect_lambda_params_expr(scrutinee, bound);
                for arm in arms {
                    collect_lambda_params_block(&arm.body, bound);
                }
            }
            Stmt::Raise { value } => collect_lambda_params_expr(value, bound),
            Stmt::Assert { cond, message } => {
                collect_lambda_params_expr(cond, bound);
                if let Some(m) = message {
                    collect_lambda_params_expr(m, bound);
                }
            }
            Stmt::Yield { value } => collect_lambda_params_expr(value, bound),
            Stmt::YieldFrom { iter } => collect_lambda_params_expr(iter, bound),
            Stmt::BlockLabel { body, .. } => collect_lambda_params_block(body, bound),
            Stmt::CheckerBlock { body, .. } => collect_lambda_params_block(body, bound),
            Stmt::Defer { body } => collect_lambda_params_block(body, bound),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                collect_lambda_params_block(body, bound);
                for (_, cb) in catches {
                    collect_lambda_params_block(cb, bound);
                }
                if let Some(eb) = else_body {
                    collect_lambda_params_block(eb, bound);
                }
                if let Some(fb) = finally_body {
                    collect_lambda_params_block(fb, bound);
                }
            }
            Stmt::Block { stmts } => {
                let tmp = Block {
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                    span: Span::unknown(),
                };
                collect_lambda_params_block(&tmp, bound);
            }
            _ => {}
        }
    }
}

/// 预扫描 IR 模块，返回 f-string 插值中未绑定的变量名集合
pub(crate) fn collect_unbound_fstring_vars(module: &IrModule) -> std::collections::HashSet<String> {
    let mut bound: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 全局绑定：fn / const / struct / enum / trait / checker / alias / duck 名 + 函数参数
    for item in &module.items {
        match item {
            Item::FnDef(f) => {
                bound.insert(f.name.clone());
                for p in &f.params {
                    bound.insert(p.name.clone());
                }
                collect_lambda_params_block(&f.body, &mut bound);
            }
            Item::Const(c) => {
                bound.insert(c.name.clone());
                collect_lambda_params_expr(&c.value, &mut bound);
            }
            Item::StructDef(s) => {
                bound.insert(s.name.clone());
                for m in &s.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params {
                        bound.insert(p.name.clone());
                    }
                    collect_lambda_params_block(&m.body, &mut bound);
                }
            }
            Item::EnumDef(e) => {
                bound.insert(e.name.clone());
                for m in &e.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params {
                        bound.insert(p.name.clone());
                    }
                    collect_lambda_params_block(&m.body, &mut bound);
                }
            }
            Item::TraitDef(t) => {
                bound.insert(t.name.clone());
                for m in &t.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params_names {
                        bound.insert(p.clone());
                    }
                    if let Some(body) = &m.body {
                        collect_lambda_params_block(body, &mut bound);
                    }
                }
            }
            Item::Impl(im) => {
                for m in &im.methods {
                    bound.insert(m.name.clone());
                    for p in &m.params {
                        bound.insert(p.name.clone());
                    }
                    collect_lambda_params_block(&m.body, &mut bound);
                }
            }
            Item::CheckerBlock { name, body, .. } => {
                bound.insert(name.clone());
                collect_lambda_params_block(body, &mut bound);
            }
            Item::TypeAlias(t) => {
                bound.insert(t.name.clone());
            }
            Item::DuckDef(d) => {
                bound.insert(d.name.clone());
            }
            _ => {}
        }
    }

    // 函数体/checker/方法/const 中的 let 绑定与 f-string 使用
    for item in &module.items {
        match item {
            Item::FnDef(f) => scan_block_fstrings(&f.body, &mut bound, &mut used),
            Item::CheckerBlock { body, .. } => scan_block_fstrings(body, &mut bound, &mut used),
            Item::StructDef(s) => {
                for m in &s.methods {
                    scan_block_fstrings(&m.body, &mut bound, &mut used);
                }
            }
            Item::EnumDef(e) => {
                for m in &e.methods {
                    scan_block_fstrings(&m.body, &mut bound, &mut used);
                }
            }
            Item::Impl(im) => {
                for m in &im.methods {
                    scan_block_fstrings(&m.body, &mut bound, &mut used);
                }
            }
            Item::Test(t) => scan_block_fstrings(&t.body, &mut bound, &mut used),
            Item::Const(c) => scan_expr_fstrings(&c.value, &mut used),
            _ => {}
        }
    }

    used.into_iter().filter(|u| !bound.contains(u)).collect()
}

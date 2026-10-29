// Lang-Zone 编译器 — ir/codegen/helpers/scan.rs
// 收集与可变性扫描：递归遍历把结论写进 out 集合（let mut / 全局提升 / 写穿别名），供 decl 发射消费
// 自 helpers.rs 逐字节拆出（move-only）：函数体/注释/签名原样保留，仅新增本文件头与 use。
use crate::ir::node::*;
use crate::ir::types::IrType;
use super::pred::expr_mentions_var;

/// 扫描块中是否存在对 const 名称的修改
pub(crate) fn scan_const_mutations(
    block: &Block,
    const_names: &std::collections::HashSet<String>,
    mutated: &mut std::collections::HashSet<String>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name, is_mut, .. } => {
                if *is_mut && const_names.contains(name) {
                    mutated.insert(name.clone());
                }
            }
            Stmt::Assign { target, .. } => {
                if let ExprKind::Var(v) = &target.kind {
                    if const_names.contains(v) {
                        mutated.insert(v.clone());
                    }
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                scan_const_mutations(then_branch, const_names, mutated);
                if let Some(ref e) = else_branch {
                    scan_const_mutations(e, const_names, mutated);
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
                scan_const_mutations(body, const_names, mutated);
            }
            Stmt::Block { stmts } => {
                let inner_block = Block {
                    span: Span::unknown(),
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                };
                scan_const_mutations(&inner_block, const_names, mutated);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    scan_const_mutations(&arm.body, const_names, mutated);
                }
            }
            Stmt::ExprStmt { expr } => {
                scan_expr_mutations(expr, const_names, mutated);
            }
            _ => {}
        }
    }
}

/// 递归扫描表达式中对 const 名称的修改（如 +=, -= 等复合赋值）
pub(crate) fn scan_expr_mutations(
    expr: &Expr,
    const_names: &std::collections::HashSet<String>,
    mutated: &mut std::collections::HashSet<String>,
) {
    match &expr.kind {
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_mutations(lhs, const_names, mutated);
            scan_expr_mutations(rhs, const_names, mutated);
        }
        ExprKind::StructCtor { name, fields } if name == "_Walrus" => {
            if let Some((_, bind_expr)) = fields.iter().find(|(n, _)| n == "_bind") {
                if let ExprKind::Var(v) = &bind_expr.kind {
                    if const_names.contains(v) {
                        mutated.insert(v.clone());
                    }
                }
            }
        }
        _ => {}
    }
}

/// 预扫描函数体，收集需要自动加 `mut` 的局部 let 变量名。
/// LZ 源码允许 `let v = ...; v.push(1)`（不写 mut），但生成 Rust 时 push
/// 需要可变绑定（E0596）。这里宽松收集：凡作为方法调用接收者、赋值目标或
/// IndexSet 基的变量名都标为需 mut（over-approximation 安全：多余的 let mut
/// 只产生 unused_mut warning，批测以 -A warnings 忽略；漏标则报 E0596）。
/// 预扫描函数体，收集需自动加 mut 的局部 let 变量名（LZ `let v = vec; v.push(1)`
/// 未写 mut，但 Rust 需可变绑定，E0596）。宽松收集：
/// - 作为方法调用接收者 / 赋值目标 / IndexSet 基；
/// - 自引用重绑定（`let parts = parts + [p]`，value 引用自身）；
/// - 循环内重绑定外层已声明变量（`let counter = i*10`，外层 `let counter = 0`
///   会被 gen_stmt 的 loop_rebind 发射成赋值，故首声明须为 mut，否则 E0384）。
/// over-approximation 安全：多余的 let mut 只产生 unused_mut warning（批测 -A warnings
/// 忽略）；漏标则报 E0596 / E0384。
pub(crate) fn scan_auto_mut_locals(block: &Block, out: &mut std::collections::HashSet<String>) {
    let mut bound: std::collections::HashSet<String> = std::collections::HashSet::new();
    scan_auto_mut_block(block, &mut bound, false, out);
}

/// 可赋值 place 表达式的根变量名：Var → 自身；FieldAccess/IndexGet → 递归基底。
/// 非 place（字面量/调用/构造/元组）返回 None。用于 ref 别名（ref f = h.n）的
/// 基底可变性判定——基底 h 须 let mut，否则别名写入 `h.n = v` 报 E0596。
pub(crate) fn root_var_of_place(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Var(n) => Some(n.clone()),
        ExprKind::FieldAccess { base, .. } => root_var_of_place(base),
        ExprKind::IndexGet { base, .. } => root_var_of_place(base),
        _ => None,
    }
}

/// 遍历 stmts 内所有语句（含 if/for/while/while-let/match/try/checker/defer/block
/// 等嵌套块），对每条语句调用 f。
fn for_each_stmts<'a>(stmts: &'a [Stmt], f: &mut dyn FnMut(&'a Stmt)) {
    for s in stmts {
        f(s);
        match s {
            Stmt::If { then_branch, else_branch, .. } => {
                for_each_stmts(&then_branch.stmts, f);
                if let Some(e) = else_branch {
                    for_each_stmts(&e.stmts, f);
                }
            }
            Stmt::For { body, .. }
            | Stmt::While { body, .. }
            | Stmt::WhileLet { body, .. }
            | Stmt::CheckerBlock { body, .. }
            | Stmt::Defer { body } => {
                for_each_stmts(&body.stmts, f);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    for_each_stmts(&arm.body.stmts, f);
                }
            }
            Stmt::TryCatch { body, catches, .. } => {
                for_each_stmts(&body.stmts, f);
                for (_, cb) in catches {
                    for_each_stmts(&cb.stmts, f);
                }
            }
            Stmt::Block { stmts } => for_each_stmts(stmts, f),
            _ => {}
        }
    }
}

/// BUG-20：预扫 ref 别名（is_ref Let + value 是可赋值 place）的「写穿」——别名名
/// 在声明之外被赋值（re-Let is_ref=false 或 Assign target）。写穿 ref 走别名替换
/// （避免 `&mut` 引用的 E0502 借用冲突）；只读 ref（moddec @ref）保持 `&mut` 形态。
/// 仅收写穿名到 out；基底可变性由 scan_auto_mut_block 的 Let 臂处理。
pub(crate) fn scan_ref_write_through(
    block: &Block,
    out: &mut std::collections::HashSet<String>,
) {
    let mut decls: std::collections::HashSet<String> = std::collections::HashSet::new();
    for_each_stmts(&block.stmts, &mut |s| {
        if let Stmt::Let { name, value, is_ref: true, .. } = s {
            if root_var_of_place(value).is_some() {
                decls.insert(name.clone());
            }
        }
    });
    for_each_stmts(&block.stmts, &mut |s| {
        match s {
            Stmt::Let { name, is_ref: false, .. } if decls.contains(name) => {
                out.insert(name.clone());
            }
            Stmt::Assign { target, .. } => {
                if let ExprKind::Var(n) = &target.kind {
                    if decls.contains(n) {
                        out.insert(n.clone());
                    }
                }
            }
            _ => {}
        }
    });
}

/// 作用域感知的预扫描：递归穿行语句序列，`bound` 记录当前作用域内已声明的变量名，
/// `in_loop` 标注是否处于循环体内（用于触发 loop_rebind 的 mut 需求）。
fn scan_auto_mut_block(
    block: &Block,
    bound: &mut std::collections::HashSet<String>,
    in_loop: bool,
    out: &mut std::collections::HashSet<String>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name, value, is_ref, .. } => {
                // BUG-20：ref 别名字段/位置（ref f = h.n）：基底变量 h 需 let mut，
                // 否则别名写入 `h.n = v` 报 E0596 cannot borrow as mutable。
                if *is_ref {
                    if let Some(v) = root_var_of_place(value) {
                        out.insert(v);
                    }
                }
                // 覆盖基线行为（自引用重绑定 `let parts = parts + [p]`：value 引用自身即标
                // mut），并叠加 T3 的循环内重绑定外层已声明变量（`let counter = i*10`：
                // bound 已含 name 且处于循环体）。注意：必须是基线判定的超集，否则会漏标
                // 导致 E0384（lib_hashmap/lib_closure 回归）。
                if expr_mentions_var(value, name)
                    || (bound.contains(name.as_str()) && in_loop)
                {
                    out.insert(name.clone());
                }
                scan_expr_auto_mut(value, out);
                bound.insert(name.clone());
            }
            Stmt::Assign { target, value } => {
                if let ExprKind::Var(v) = &target.kind {
                    out.insert(v.clone());
                }
                // 索引赋值目标（d["k"] = v → d.insert(..)，p[i] = v → d[i] = ..）：
                // 基变量被可变借用，需标 mut，否则 E0596 cannot borrow as mutable
                // （Dict 空构造 + 索引赋值是既有漏标场景）
                match &target.kind {
                    ExprKind::IndexGet { base, .. } | ExprKind::IndexSet { base, .. } => {
                        if let ExprKind::Var(v) = &base.kind {
                            out.insert(v.clone());
                        }
                    }
                    // BUG-261：`root.left = ...` 的 FieldAccess 赋值目标（如
                    // `root.as_mut().unwrap().left = Some(...)`）需要基底 `root` 为 mut，
                    // 否则 E0596 cannot borrow `root` as mutable（Option<Box<T>> 递归字段）
                    ExprKind::FieldAccess { base, .. } => {
                        if let Some(v) = root_var_of_place(target) {
                            out.insert(v);
                        }
                        let _ = base;
                    }
                    _ => {}
                }
                scan_expr_auto_mut(target, out);
                scan_expr_auto_mut(value, out);
            }
            Stmt::ExprStmt { expr } => scan_expr_auto_mut(expr, out),
            Stmt::Return { value } => {
                if let Some(v) = value {
                    scan_expr_auto_mut(v, out);
                }
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                scan_expr_auto_mut(cond, out);
                scan_auto_mut_block(then_branch, &mut bound.clone(), in_loop, out);
                if let Some(e) = else_branch {
                    scan_auto_mut_block(e, &mut bound.clone(), in_loop, out);
                }
            }
            Stmt::For { iter, body, .. } => {
                scan_expr_auto_mut(iter, out);
                scan_auto_mut_block(body, &mut bound.clone(), true, out);
            }
            Stmt::While { cond, body, .. } => {
                scan_expr_auto_mut(cond, out);
                scan_auto_mut_block(body, &mut bound.clone(), true, out);
            }
            Stmt::WhileLet { expr, body, .. } => {
                scan_expr_auto_mut(expr, out);
                scan_auto_mut_block(body, &mut bound.clone(), true, out);
            }
            Stmt::Match { scrutinee, arms } => {
                scan_expr_auto_mut(scrutinee, out);
                for arm in arms {
                    scan_auto_mut_block(&arm.body, &mut bound.clone(), in_loop, out);
                }
            }
            Stmt::Raise { value, .. } => {
                scan_expr_auto_mut(value, out);
            }
            Stmt::Assert { cond, .. } => scan_expr_auto_mut(cond, out),
            Stmt::Yield { value } => scan_expr_auto_mut(value, out),
            Stmt::YieldFrom { iter } => scan_expr_auto_mut(iter, out),
            Stmt::Block { stmts } => {
                let inner = Block {
                    span: Span::unknown(),
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                };
                scan_auto_mut_block(&inner, &mut bound.clone(), in_loop, out);
            }
            Stmt::TryCatch { body, catches, .. } => {
                scan_auto_mut_block(body, &mut bound.clone(), in_loop, out);
                for (_, cb) in catches {
                    scan_auto_mut_block(cb, &mut bound.clone(), in_loop, out);
                }
            }
            Stmt::CheckerBlock { body, .. } => scan_auto_mut_block(body, &mut bound.clone(), in_loop, out),
            Stmt::Defer { body } => scan_auto_mut_block(body, &mut bound.clone(), in_loop, out),
            _ => {}
        }
    }
}

/// 递归扫描表达式中的可变使用（方法调用接收者 / 赋值目标 / IndexSet 基）
fn scan_expr_auto_mut(expr: &Expr, out: &mut std::collections::HashSet<String>) {
    match &expr.kind {
        ExprKind::Var(_) | ExprKind::Lit(_) | ExprKind::Default => {}
        ExprKind::Spread(inner) => scan_expr_auto_mut(inner, out),
        ExprKind::Call { callee, args, .. } => {
            // 闭包变量调用（double_then_inc(5)）：FnMut 闭包需 mut 绑定（E0596）
            if let ExprKind::Var(v) = &callee.kind {
                if matches!(callee.ty, IrType::Fn { .. }) {
                    out.insert(v.clone());
                }
            }
            // 集合可变自由函数（push/append/pop/extend/insert/remove）：首参为被修改的
            // 接收者，需标为 mut，否则降级为 (recv).push(item) 时报 E0596 cannot borrow
            // immutable（BUG-IR-002 复现 `push(log, ...)` 场景）
            if let ExprKind::Var(name) = &callee.kind {
                if matches!(
                    name.as_str(),
                    "push" | "append" | "pop" | "extend" | "insert" | "remove" | "add" | "delete"
                ) {
                    if let Some(ExprKind::Var(recv)) = args.first().map(|a| &a.kind) {
                        out.insert(recv.clone());
                    }
                }
            }
            scan_expr_auto_mut(callee, out);
            for a in args {
                scan_expr_auto_mut(a, out);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            if let ExprKind::Var(v) = &receiver.kind {
                out.insert(v.clone());
            }
            scan_expr_auto_mut(receiver, out);
            for a in args {
                scan_expr_auto_mut(a, out);
            }
        }
        ExprKind::FieldAccess { base, .. } => scan_expr_auto_mut(base, out),
        ExprKind::IndexGet { base, key } => {
            scan_expr_auto_mut(base, out);
            scan_expr_auto_mut(key, out);
        }
        ExprKind::IndexSet { base, key, value } => {
            if let ExprKind::Var(v) = &base.kind {
                out.insert(v.clone());
            }
            scan_expr_auto_mut(base, out);
            scan_expr_auto_mut(key, out);
            scan_expr_auto_mut(value, out);
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            scan_expr_auto_mut(lhs, out);
            scan_expr_auto_mut(rhs, out);
        }
        ExprKind::AssignExpr { target, value } => {
            if let ExprKind::Var(v) = &target.kind {
                out.insert(v.clone());
            }
            scan_expr_auto_mut(target, out);
            scan_expr_auto_mut(value, out);
        }
        ExprKind::UnOp { operand, .. } => scan_expr_auto_mut(operand, out),
        ExprKind::IfExpr { cond, then, els } => {
            scan_expr_auto_mut(cond, out);
            scan_expr_auto_mut(then, out);
            scan_expr_auto_mut(els, out);
        }
        ExprKind::Lambda { body, .. } => scan_expr_auto_mut(body, out),
        ExprKind::StructCtor { fields, .. } => {
            for (_, e) in fields {
                scan_expr_auto_mut(e, out);
            }
        }
        ExprKind::EnumCtor { args, .. } => {
            for a in args {
                scan_expr_auto_mut(a, out);
            }
        }
        ExprKind::GenExpr { yield_of } => scan_expr_auto_mut(yield_of, out),
        ExprKind::GenBuild { block, .. } => scan_auto_mut_locals(block, out),
        ExprKind::Cast { expr, .. } => scan_expr_auto_mut(expr, out),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                scan_expr_auto_mut(a, out);
            }
        }
        ExprKind::BlockExpr { block } => scan_auto_mut_locals(block, out),
        ExprKind::TupleLit(items)
        | ExprKind::Tuple(items)
        | ExprKind::ListLit(items)
        | ExprKind::List(items) => {
            for i in items {
                scan_expr_auto_mut(i, out);
            }
        }
        ExprKind::Dict(entries) => {
            for (k, v) in entries {
                scan_expr_auto_mut(k, out);
                scan_expr_auto_mut(v, out);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                scan_expr_auto_mut(s, out);
            }
            scan_expr_auto_mut(end, out);
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            scan_expr_auto_mut(receiver, out);
            scan_expr_auto_mut(callee, out);
            for a in args {
                scan_expr_auto_mut(a, out);
            }
        }
        ExprKind::Paren(inner) => scan_expr_auto_mut(inner, out),
        ExprKind::ImplicitConvert { source, .. } => scan_expr_auto_mut(source, out),
    }
}

/// 收集块中的局部 let 绑定名 + 闭包参数（遮蔽名）
pub(crate) fn collect_local_lets(block: &Block, locals: &mut std::collections::HashSet<String>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name, .. } => {
                locals.insert(name.clone());
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_local_lets(then_branch, locals);
                if let Some(e) = else_branch {
                    collect_local_lets(e, locals);
                }
            }
            Stmt::For { var, body, .. } => {
                // 元组解构循环变量 `for (k, v) in ...`：var 形如 "(k, v)"，
                // 需把 k、v 分别收集为局部变量，否则 analyze_global_vars 把
                // 未收集的名字误判为跨函数全局变量（E0530 static mut 冲突）
                collect_for_var_bindings(var, locals);
                collect_local_lets(body, locals);
            }
            Stmt::While { body, .. } => collect_local_lets(body, locals),
            Stmt::WhileLet { pattern, body, .. } => {
                // while-let 模式绑定（如 Some(item) 中的 item）也是局部变量：
                // 不收集会导致 analyze_global_vars 误判为跨函数全局变量，
                // 生成 static mut item 与 for 绑定冲突（E0530，while_let.lz）
                collect_pattern_bindings(pattern, locals);
                collect_local_lets(body, locals);
            }
            Stmt::Block { stmts } => {
                let inner = Block {
                    span: Span::unknown(),
                    stmts: stmts.clone(),
                    ty: IrType::Unit,
                };
                collect_local_lets(&inner, locals);
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    // 收集 match 模式绑定名（遮蔽外部变量）
                    collect_pattern_bindings(&a.pattern, locals);
                    collect_local_lets(&a.body, locals);
                }
            }
            _ => {}
        }
    }
}

/// 收集模式中的绑定名（match 臂的 Ident/Tuple/Struct/Enum 绑定）
pub(crate) fn collect_pattern_bindings(
    pattern: &Pattern,
    locals: &mut std::collections::HashSet<String>,
) {
    match pattern {
        Pattern::Ident(name) => {
            locals.insert(name.clone());
        }
        Pattern::Tuple(ps) => {
            for p in ps {
                collect_pattern_bindings(p, locals);
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields {
                collect_pattern_bindings(p, locals);
            }
        }
        Pattern::Enum { args, .. } => {
            for p in args {
                collect_pattern_bindings(p, locals);
            }
        }
        _ => {}
    }
}

/// 收集 for 循环变量的绑定名：`for x in ...` → x；
/// 元组解构 `for (k, v) in ...` → k、v 分别收集（否则 analyze_global_vars
/// 把未收集的名字误判为跨函数全局变量，生成 static mut 与 for 绑定冲突 E0530）
pub(crate) fn collect_for_var_bindings(var: &str, locals: &mut std::collections::HashSet<String>) {
    let trimmed = var.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        // 元组解构：(k, v) → 分别收集
        let inner = &trimmed[1..trimmed.len() - 1];
        for part in inner.split(',') {
            let name = part.trim();
            if !name.is_empty() && name != "_" {
                locals.insert(name.to_string());
            }
        }
    } else if !trimmed.is_empty() && trimmed != "_" {
        locals.insert(trimmed.to_string());
    }
}

/// 递归收集块中引用的自由变量名（shadow 为遮蔽名集合：闭包参数）
/// 递归收集自由变量引用。in_closure=true 表示当前处于闭包作用域，
/// 此时裸赋值 `x = v` 视为局部声明（加入遮蔽集），而不是外部变量引用。
/// 用于构建块（=: → 闭包）内的赋值，避免被误提升为全局变量。
pub(crate) fn collect_var_refs(
    block: &Block,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
) {
    collect_var_refs_inner(block, shadow, refs, false);
}

pub(crate) fn collect_var_refs_inner(
    block: &Block,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
    in_closure: bool,
) {
    for stmt in &block.stmts {
        collect_stmt_var_refs(stmt, shadow, refs, in_closure);
    }
}

pub(crate) fn collect_stmt_var_refs(
    stmt: &Stmt,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
    in_closure: bool,
) {
    match stmt {
        Stmt::Let { name, value, .. } => {
            // let 声明引入新局部变量，遮蔽同名外部引用
            shadow.insert(name.clone());
            collect_expr_var_refs(value, shadow, refs, in_closure);
        }
        Stmt::Assign { target, value } => {
            // 闭包作用域内：裸赋值视为局部声明（如构建块内 a = 10）
            if in_closure {
                if let ExprKind::Var(n) = &target.kind {
                    shadow.insert(n.clone());
                }
            }
            collect_expr_var_refs(target, shadow, refs, in_closure);
            collect_expr_var_refs(value, shadow, refs, in_closure);
        }
        Stmt::ExprStmt { expr } => collect_expr_var_refs(expr, shadow, refs, in_closure),
        Stmt::Return { value } => {
            if let Some(v) = value {
                collect_expr_var_refs(v, shadow, refs, in_closure);
            }
        }
        Stmt::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr_var_refs(cond, shadow, refs, in_closure);
            collect_var_refs_inner(then_branch, shadow, refs, in_closure);
            if let Some(e) = else_branch {
                collect_var_refs_inner(e, shadow, refs, in_closure);
            }
        }
        Stmt::For {
            var,
            iter,
            guard,
            body,
            ..
        } => {
            // for 循环变量是局部绑定：遮蔽体内引用（`ts[idx]` 的 idx），
            // 否则 idx 被当作自由引用 → analyze_global_vars 误判为跨函数
            // 全局变量，生成 `static mut idx` 与 for 绑定冲突（E0530，
            // 自举试点 bootstrap/work/lz_ir 暴露：多函数 for idx 触发）
            shadow.insert(var.clone());
            collect_expr_var_refs(iter, shadow, refs, in_closure);
            if let Some(g) = guard {
                collect_expr_var_refs(g, shadow, refs, in_closure);
            }
            collect_var_refs_inner(body, shadow, refs, in_closure);
        }
        Stmt::While {
            cond,
            guard,
            body,
            else_body,
        } => {
            collect_expr_var_refs(cond, shadow, refs, in_closure);
            if let Some(g) = guard {
                collect_expr_var_refs(g, shadow, refs, in_closure);
            }
            collect_var_refs_inner(body, shadow, refs, in_closure);
            if let Some(eb) = else_body {
                collect_var_refs_inner(eb, shadow, refs, in_closure);
            }
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            collect_expr_var_refs(expr, shadow, refs, in_closure);
            if let Some(g) = guard {
                collect_expr_var_refs(g, shadow, refs, in_closure);
            }
            collect_var_refs_inner(body, shadow, refs, in_closure);
        }
        Stmt::Block { stmts } => {
            let inner = Block {
                span: Span::unknown(),
                stmts: stmts.clone(),
                ty: IrType::Unit,
            };
            collect_var_refs_inner(&inner, shadow, refs, in_closure);
        }
        Stmt::Match {
            scrutinee, arms, ..
        } => {
            collect_expr_var_refs(scrutinee, shadow, refs, in_closure);
            for arm in arms {
                // match 臂模式绑定是局部变量（`case X(path: p)` 的 p）：遮蔽
                // 臂体内引用，否则被 collect_var_refs 误收为自由引用 →
                // analyze_global_vars 误判为跨函数全局变量，生成 static mut
                // 与 let/参数绑定冲突（E0530，自举试点 bootstrap/work/lz_ir
                // 暴露：多函数同名模式绑定 a/o/es/r 触发）
                collect_pattern_bindings(&arm.pattern, shadow);
                collect_var_refs_inner(&arm.body, shadow, refs, in_closure);
            }
        }
        Stmt::Raise { value } => collect_expr_var_refs(value, shadow, refs, in_closure),
        Stmt::Assert { cond, .. } => collect_expr_var_refs(cond, shadow, refs, in_closure),
        _ => {}
    }
}

pub(crate) fn collect_expr_var_refs(
    expr: &Expr,
    shadow: &mut std::collections::HashSet<String>,
    refs: &mut Vec<String>,
    in_closure: bool,
) {
    match &expr.kind {
        ExprKind::Var(name) => {
            // 被闭包参数/局部变量遮蔽的跳过
            if !shadow.contains(name.as_str()) {
                refs.push(name.clone());
            }
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            collect_expr_var_refs(lhs, shadow, refs, in_closure);
            collect_expr_var_refs(rhs, shadow, refs, in_closure);
        }
        ExprKind::UnOp { operand, .. } => collect_expr_var_refs(operand, shadow, refs, in_closure),
        ExprKind::Call { callee, args, .. } => {
            collect_expr_var_refs(callee, shadow, refs, in_closure);
            for a in args {
                collect_expr_var_refs(a, shadow, refs, in_closure);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            collect_expr_var_refs(receiver, shadow, refs, in_closure);
            for a in args {
                collect_expr_var_refs(a, shadow, refs, in_closure);
            }
        }
        ExprKind::FieldAccess { base, .. } => collect_expr_var_refs(base, shadow, refs, in_closure),
        ExprKind::IndexGet { base, key } => {
            collect_expr_var_refs(base, shadow, refs, in_closure);
            collect_expr_var_refs(key, shadow, refs, in_closure);
        }
        ExprKind::IfExpr { cond, then, els } => {
            collect_expr_var_refs(cond, shadow, refs, in_closure);
            collect_expr_var_refs(then, shadow, refs, in_closure);
            collect_expr_var_refs(els, shadow, refs, in_closure);
        }
        ExprKind::BlockExpr { block } => collect_var_refs_inner(block, shadow, refs, in_closure),
        ExprKind::Lambda { params, body, .. } => {
            // 闭包参数遮蔽：进入闭包体时加入遮蔽集；闭包内裸赋值视为局部声明
            let mut inner_shadow = shadow.clone();
            for p in params {
                inner_shadow.insert(p.name.clone());
            }
            collect_expr_var_refs(body, &mut inner_shadow, refs, true);
        }
        ExprKind::ListLit(elems) => {
            for e in elems {
                collect_expr_var_refs(e, shadow, refs, in_closure);
            }
        }
        ExprKind::TupleLit(elems) => {
            for e in elems {
                collect_expr_var_refs(e, shadow, refs, in_closure);
            }
        }
        ExprKind::StructCtor { fields, .. } => {
            for (_, v) in fields {
                collect_expr_var_refs(v, shadow, refs, in_closure);
            }
        }
        ExprKind::Cast { expr, .. } => collect_expr_var_refs(expr, shadow, refs, in_closure),
        ExprKind::MagicCall { args, .. } => {
            for a in args {
                collect_expr_var_refs(a, shadow, refs, in_closure);
            }
        }
        _ => {}
    }
}


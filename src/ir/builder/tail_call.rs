// Lang-Zone 编译器 — ir/builder/tail_call.rs
// 尾递归自动优化（TCO）：IR 层检测 + desugar 改写（后端无关，Rust / Cython 同时受益）
//
// 依据：`IR/tailrec-auto-plan.md`（2026-10-07 提案）
//
// 流水线：
//   AST → convert_fn_def → analyze_tail_recursion()（挂 FnDef.tco + @tailrec 静态校验）
//        → build_ir_inner 收口 → rewrite_tco()（verdict == TailOptimizable 时改写为循环）
//        → codegen 消费改写后的 IR（Rust 后端零改动）
//
// 改写形态（形态 B：while + 结果变量 + break）：
//   fn f(mut p1, mut p2) {
//     let mut __tco_res = <default>;
//     while true {
//       <body'>            // 尾自调用 f(a1,a2) → 求参进临时 → p1=a1; p2=a2; continue
//                          // 原 return v / 块尾值 → __tco_res = v; break;
//     }
//     __tco_res
//   }
//
// 关键不变量：
// - **求参先行**：实参先物化到临时槽再重赋形参，`f(a+1, a)` 第二参取旧 a（专项测试）。
// - **λ 边界截断**：嵌套闭包 / 生成器体内的自调用不算外层函数的尾调用（Scala 语义）。
// - **非尾容器黑名单**：while / for / try / defer 体内的自调用一律判非尾位置——
//   循环体里递归返回后还有循环回边，不是尾位置。
// - **不动后端**：改写只发生在这层 IR，Cython / Rust codegen 都不需要改。
//
// 开关：环境变量 `LZ_TCO=0` 或 CLI `--no-tco` 关闭**自动**改写；
// `@tailrec` / `#[tail_call]` 标注的静态报错**不受开关影响**（标注是静态契约）。

use super::*;

use crate::ir::node::TcoVerdict;

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

// ═══════════════════════════════════════════════════════════════════
// 开关与警告通道
// ═══════════════════════════════════════════════════════════════════

static TCO_ENABLED: AtomicBool = AtomicBool::new(true);

thread_local! {
    /// 非致命提示（如 `#[tail_call]` 弃用别名），由前端在编译结束后打印。
    static TCO_WARNINGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// 供 CLI `--no-tco` 调用：关闭自动尾递归改写（不影响标注校验）。
pub fn set_tco_enabled(on: bool) {
    TCO_ENABLED.store(on, Ordering::Relaxed);
}

/// 自动改写是否开启：`--no-tco` > `LZ_TCO=0/off/false` > 缺省开启。
pub fn tco_enabled() -> bool {
    if !TCO_ENABLED.load(Ordering::Relaxed) {
        return false;
    }
    match std::env::var("LZ_TCO") {
        Ok(v) => !matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "off" | "false"),
        Err(_) => true,
    }
}

pub(crate) fn push_tco_warning(msg: String) {
    TCO_WARNINGS.with(|w| w.borrow_mut().push(msg));
}

/// 取走并清空累积的 TCO 提示（前端打印一次）。
pub fn take_tco_warnings() -> Vec<String> {
    TCO_WARNINGS.with(|w| std::mem::take(&mut *w.borrow_mut()))
}

// ═══════════════════════════════════════════════════════════════════
// 检测（显式栈，避免深体爆栈）
// ═══════════════════════════════════════════════════════════════════

/// 检测入参（builder 阶段 AST→IR 时手上只有这些，故用引用借用）
pub(crate) struct TcoInput<'a> {
    pub fname: &'a str,
    pub params: &'a [Param],
    pub ret_ty: &'a IrType,
    pub raises: bool,
    pub is_iterator: bool,
    pub is_async: bool,
    pub intrinsics: &'a [Intrinsic],
    pub body: &'a Block,
}

enum Step<'a> {
    Stmt(&'a Stmt, bool),
    Stmts(&'a [Stmt], bool),
    Expr(&'a Expr, bool),
    Block(&'a Block, bool),
}

#[derive(Default)]
struct Scan {
    /// 直接自调用总数（λ / 生成器体不计）
    total: usize,
    /// 非尾位置自调用的描述（报错文案用）
    non_tail: Vec<String>,
    /// 各自调用的实参个数（判定 arity 是否可用默认值补齐）
    arities: Vec<usize>,
}

/// 非尾位置黑名单容器：体内自调用一律记为非尾位置
fn blacklist_container(scan: &mut Scan, kind: &str, blocks: &[&Block], fname: &str) {
    let mut n = 0;
    for b in blocks {
        n += count_self_calls(b, fname);
    }
    if n > 0 {
        scan.total += n;
        scan.non_tail.push(format!(
            "{} 体内有 {} 处自调用（循环/try 体之后仍有控制流，不是尾位置）",
            kind, n
        ));
    }
}

/// 迭代式全量计数（不下钻 λ / 生成器体）
fn count_self_calls(block: &Block, fname: &str) -> usize {
    let mut n = 0;
    let mut stmts: Vec<&[Stmt]> = vec![&block.stmts];
    let mut stack: Vec<&Expr> = Vec::new();
    while let Some(ss) = stmts.pop() {
        collect_stmt_subtrees(ss, &mut stmts, &mut stack);
    }
    while let Some(e) = stack.pop() {
        match &e.kind {
            ExprKind::Call { callee, args, .. } => {
                if matches!(&callee.kind, ExprKind::Var(v) if v == fname) {
                    n += 1;
                }
                stack.push(callee);
                for a in args {
                    stack.push(a);
                }
            }
            ExprKind::Lambda { .. } | ExprKind::GenExpr { .. } | ExprKind::GenBuild { .. } => {}
            other => {
                for c in expr_children(other) {
                    match c {
                        Child::Expr(x) => stack.push(x),
                        Child::Block(b) => stmts.push(&b.stmts),
                    }
                }
            }
        }
    }
    n
}

/// 把一个语句列表里的所有子表达式 / 子块收集到工作栈（λ、生成器体不收集）
fn collect_stmt_subtrees<'a>(ss: &'a [Stmt], stmts: &mut Vec<&'a [Stmt]>, es: &mut Vec<&'a Expr>) {
    for st in ss {
        match st {
            Stmt::Let { value, .. } => es.push(value),
            Stmt::Assign { target, value } => {
                es.push(target);
                es.push(value);
            }
            Stmt::Return { value: Some(v) } => es.push(v),
            Stmt::ExprStmt { expr } => es.push(expr),
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                stmts.push(&then_branch.stmts);
                if let Some(b) = else_branch {
                    stmts.push(&b.stmts);
                }
            }
            Stmt::Block { stmts: inner } => stmts.push(inner),
            Stmt::For {
                body, else_body, ..
            } => {
                stmts.push(&body.stmts);
                if let Some(b) = else_body {
                    stmts.push(&b.stmts);
                }
            }
            Stmt::While {
                body, else_body, ..
            } => {
                stmts.push(&body.stmts);
                if let Some(b) = else_body {
                    stmts.push(&b.stmts);
                }
            }
            Stmt::WhileLet { body, .. } => stmts.push(&body.stmts),
            Stmt::BlockLabel { body, .. } => stmts.push(&body.stmts),
            Stmt::Defer { body } => stmts.push(&body.stmts),
            Stmt::CheckerBlock { body, .. } => stmts.push(&body.stmts),
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                stmts.push(&body.stmts);
                for (_, b) in catches {
                    stmts.push(&b.stmts);
                }
                if let Some(b) = else_body {
                    stmts.push(&b.stmts);
                }
                if let Some(b) = finally_body {
                    stmts.push(&b.stmts);
                }
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    stmts.push(&a.body.stmts);
                }
            }
            Stmt::Raise { value } => es.push(value),
            Stmt::Assert { cond, message } => {
                es.push(cond);
                if let Some(m) = message {
                    es.push(m);
                }
            }
            Stmt::Yield { value } => es.push(value),
            Stmt::YieldFrom { iter } => es.push(iter),
            Stmt::BreakLabel { value, .. } => {
                if let Some(v) = value {
                    es.push(v);
                }
            }
            _ => {}
        }
    }
}

/// 尾位置感知的扫描
fn scan(body: &Block, fname: &str) -> Scan {
    let mut s = Scan::default();
    let mut stack = vec![Step::Block(body, true)];
    while let Some(step) = stack.pop() {
        match step {
            Step::Block(b, tail) => stack.push(Step::Stmts(&b.stmts, tail)),
            Step::Stmts(ss, tail) => {
                let n = ss.len();
                for (i, st) in ss.iter().enumerate() {
                    stack.push(Step::Stmt(st, tail && i + 1 == n));
                }
            }
            Step::Stmt(st, tail) => match st {
                Stmt::Return { value: Some(e) } => stack.push(Step::Expr(e, true)),
                Stmt::Return { value: None } => {}
                Stmt::ExprStmt { expr } => stack.push(Step::Expr(expr, tail)),
                Stmt::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    stack.push(Step::Block(then_branch, tail));
                    if let Some(b) = else_branch {
                        stack.push(Step::Block(b, tail));
                    }
                }
                Stmt::Block { stmts } => {
                    let n = stmts.len();
                    for (i, s2) in stmts.iter().enumerate() {
                        stack.push(Step::Stmt(s2, tail && i + 1 == n));
                    }
                }
                Stmt::BlockLabel { body, .. } => stack.push(Step::Block(body, tail)),
                Stmt::Match { arms, .. } => {
                    for a in arms {
                        stack.push(Step::Block(&a.body, tail));
                    }
                }
                // ↓ 非尾位置容器：体内自调用不是尾调用
                Stmt::While { body, else_body, .. } => {
                    let mut v: Vec<&Block> = vec![body];
                    if let Some(b) = else_body {
                        v.push(b);
                    }
                    blacklist_container(&mut s, "while", &v, fname);
                }
                Stmt::WhileLet { body, .. } => {
                    blacklist_container(&mut s, "while let", &[body], fname);
                }
                Stmt::For {
                    body, else_body, ..
                } => {
                    let mut v: Vec<&Block> = vec![body];
                    if let Some(b) = else_body {
                        v.push(b);
                    }
                    blacklist_container(&mut s, "for", &v, fname);
                }
                Stmt::TryCatch {
                    body,
                    catches,
                    else_body,
                    finally_body,
                } => {
                    let mut v: Vec<&Block> = vec![body];
                    for (_, b) in catches {
                        v.push(b);
                    }
                    if let Some(b) = else_body {
                        v.push(b);
                    }
                    if let Some(b) = finally_body {
                        v.push(b);
                    }
                    blacklist_container(&mut s, "try/catch", &v, fname);
                }
                Stmt::Defer { body } => blacklist_container(&mut s, "defer", &[body], fname),
                Stmt::CheckerBlock { body, .. } => {
                    blacklist_container(&mut s, "checker", &[body], fname)
                }
                // ↓ 普通语句：只下钻子表达式（非尾）
                _ => {
                    let mut es: Vec<&Expr> = Vec::new();
                    let mut ss: Vec<&[Stmt]> = Vec::new();
                    collect_stmt_subtrees(std::slice::from_ref(st), &mut ss, &mut es);
                    for e in es {
                        stack.push(Step::Expr(e, false));
                    }
                    for s2 in ss {
                        stack.push(Step::Stmts(s2, false));
                    }
                }
            },
            Step::Expr(e, tail) => match &e.kind {
                ExprKind::Call { callee, args, .. } => {
                    if matches!(&callee.kind, ExprKind::Var(v) if v == fname) {
                        s.total += 1;
                        s.arities.push(args.len());
                        if !tail {
                            s.non_tail.push(format!("自调用 `{}` 不在尾位置", fname));
                        }
                    } else {
                        stack.push(Step::Expr(callee, false));
                    }
                    for a in args {
                        stack.push(Step::Expr(a, false));
                    }
                }
                ExprKind::IfExpr { cond, then, els } => {
                    stack.push(Step::Expr(cond, false));
                    stack.push(Step::Expr(then, tail));
                    stack.push(Step::Expr(els, tail));
                }
                ExprKind::BlockExpr { block } => stack.push(Step::Block(block, tail)),
                // λ / 生成器体：Scala 语义边界，外层不算自己的尾调用
                ExprKind::Lambda { .. } | ExprKind::GenExpr { .. } | ExprKind::GenBuild { .. } => {}
                other => {
                    let mut es: Vec<&Expr> = Vec::new();
                    let mut bs: Vec<&Block> = Vec::new();
                    for x in expr_children(other) {
                        match x {
                            Child::Expr(e) => es.push(e),
                            Child::Block(b) => bs.push(b),
                        }
                    }
                    for e in es {
                        stack.push(Step::Expr(e, false));
                    }
                    for b in bs {
                        stack.push(Step::Block(b, false));
                    }
                }
            },
        }
    }
    s
}

enum Child<'a> {
    Expr(&'a Expr),
    Block(&'a Block),
}

/// 表达式的子节点（不含 λ / 生成器体）
fn expr_children(e: &ExprKind) -> Vec<Child<'_>> {
    let v: Vec<&Expr> = match e {
        ExprKind::Lit(_) | ExprKind::Default | ExprKind::Var(_) => Vec::new(),
        ExprKind::Call { callee, args, .. } => {
            let mut v = vec![&**callee];
            v.extend(args.iter());
            v
        }
        ExprKind::MethodCall {
            receiver, args, ..
        } => {
            let mut v = vec![&**receiver];
            v.extend(args.iter());
            v
        }
        ExprKind::FieldAccess { base, .. } => vec![base],
        ExprKind::IndexGet { base, key } => vec![base, key],
        ExprKind::IndexSet { base, key, value } => vec![base, key, value],
        ExprKind::BinOp { lhs, rhs, .. } => vec![lhs, rhs],
        ExprKind::AssignExpr { target, value } => vec![target, value],
        ExprKind::UnOp { operand, .. } => vec![operand],
        ExprKind::IfExpr { cond, then, els } => vec![cond, then, els],
        ExprKind::Lambda { .. } | ExprKind::GenExpr { .. } | ExprKind::GenBuild { .. } => {
            Vec::new()
        }
        ExprKind::StructCtor { fields, .. } => fields.iter().map(|(_, e)| e).collect(),
        ExprKind::EnumCtor { args, .. } => args.iter().collect(),
        ExprKind::Cast { expr, .. } => vec![expr],
        ExprKind::MagicCall { args, .. } => args.iter().collect(),
        ExprKind::BlockExpr { block } => return vec![Child::Block(block)],
        ExprKind::TupleLit(es)
        | ExprKind::Tuple(es)
        | ExprKind::ListLit(es)
        | ExprKind::List(es) => es.iter().collect(),
        ExprKind::Spread(e) => vec![&**e],
        ExprKind::Dict(pairs) => pairs.iter().flat_map(|(k, v)| vec![k, v]).collect(),
        ExprKind::Range { start, end, .. } => {
            let mut v: Vec<&Expr> = vec![&**end];
            if let Some(s) = start {
                v.push(&**s);
            }
            v
        }
        ExprKind::Pipe {
            receiver,
            callee,
            args,
        } => {
            let mut v = vec![&**receiver, &**callee];
            v.extend(args.iter());
            v
        }
        ExprKind::Paren(e) => vec![&**e],
        ExprKind::ImplicitConvert { source, .. } => vec![&**source],
    };
    v.into_iter().map(Child::Expr).collect()
}

/// （`collect_stmt_subtrees` 已覆盖所有语句形态，本文件不再需要旧谓词）

// ═══════════════════════════════════════════════════════════════════
// verdict 判定
// ═══════════════════════════════════════════════════════════════════

/// v1 可安全默认化的返回类型（用于 `__tco_res` 初值）
fn default_value(ty: &IrType) -> Option<Expr> {
    let (kind, ty2) = match ty {
        IrType::Int => (LitKind::Int(0), ty.clone()),
        IrType::Int128 => (LitKind::Int128(0), ty.clone()),
        IrType::BigInt => (LitKind::BigInt("0".into()), ty.clone()),
        IrType::F64 => (LitKind::F64(0.0), ty.clone()),
        IrType::Complex => (LitKind::Complex(0.0, 0.0), ty.clone()),
        IrType::Bool => (LitKind::Bool(false), ty.clone()),
        IrType::Str => (LitKind::Str(String::new()), ty.clone()),
        IrType::Option(_) | IrType::Result { .. } => (LitKind::None_, ty.clone()),
        IrType::Named { path, .. }
            if matches!(
                path.as_str(),
                "List" | "Vec" | "Dict" | "HashMap" | "Set" | "HashSet" | "String" | "Iter"
            ) =>
        {
            if path == "String" {
                (LitKind::Str(String::new()), ty.clone())
            } else if matches!(path.as_str(), "Dict" | "HashMap") {
                let e = Expr::new(
                    ExprKind::Dict(Vec::new()),
                    ty.clone(),
                    Span::unknown(),
                );
                return Some(e);
            } else {
                let e = Expr::new(ExprKind::ListLit(Vec::new()), ty.clone(), Span::unknown());
                return Some(e);
            }
        }
        _ => return None,
    };
    Some(Expr::new(ExprKind::Lit(kind), ty2, Span::unknown()))
}

/// 结构层限制（与自调用位置无关的改写前提）
fn structural_blocker(inp: &TcoInput, s: &Scan) -> Option<String> {
    if inp.raises {
        return Some("函数带 raises（异常类型）—— return 需包 Ok(..)，v1 不改写".into());
    }
    if inp.is_iterator {
        return Some("生成器（iterator）函数体内 return 语义为 raise，v1 不改写".into());
    }
    if inp.is_async {
        return Some("async 函数含 await/spawn，v1 不改写".into());
    }
    for p in inp.params {
        if p.is_ref || p.is_owned {
            return Some(format!(
                "形参 `{}` 是引用/owned 形参，重赋值语义不等价，v1 不改写",
                p.name
            ));
        }
        if p.variadic {
            return Some(format!("变参形参 `{}` 的收集语义与重赋值冲突，v1 不改写", p.name));
        }
        if p.comptime {
            return Some(format!("comptime 形参 `{}` 不能重赋值，v1 不改写", p.name));
        }
    }
    for i in inp.intrinsics.iter() {
        match &i.kind {
            IntrinsicKind::None | IntrinsicKind::TailCall => {}
            other => {
                return Some(format!(
                    "与其他 intrinsic 组合（{:?}），自动路径不转换",
                    other
                ))
            }
        }
    }
    // 自调用实参个数必须能用默认值补齐
    let n = inp.params.len();
    for (k, arity) in s.arities.iter().enumerate() {
        if *arity > n {
            return Some(format!(
                "第 {} 处自调用实参个数 {} 超过形参个数 {}",
                k + 1,
                arity,
                n
            ));
        }
        if *arity < n {
            for p in inp.params.iter().skip(*arity) {
                if p.default.is_none() {
                    return Some(format!(
                        "第 {} 处自调用省略了无默认值的形参 `{}`",
                        k + 1,
                        p.name
                    ));
                }
            }
        }
    }
    if *inp.ret_ty != IrType::Unit && default_value(inp.ret_ty).is_none() {
        return Some(format!(
            "返回类型 {:?} 无安全默认初值，无法构造结果变量",
            inp.ret_ty
        ));
    }
    None
}

/// 主入口：判定一个函数的尾递归 verdict
pub(crate) fn analyze_tail_recursion(inp: &TcoInput) -> TcoVerdict {
    let s = scan(inp.body, inp.fname);
    if s.total == 0 {
        return TcoVerdict::NoSelfCall;
    }
    if !s.non_tail.is_empty() {
        return TcoVerdict::NotTailPosition { sites: s.non_tail };
    }
    match structural_blocker(inp, &s) {
        Some(reason) => TcoVerdict::NotTransformable { reason },
        None => TcoVerdict::TailOptimizable,
    }
}

/// verdict 的可读原因（`@tailrec` 报错文案）
pub(crate) fn verdict_reason(v: &TcoVerdict) -> String {
    match v {
        TcoVerdict::NoSelfCall => "函数体内无直接自调用".to_string(),
        TcoVerdict::TailOptimizable => "可优化为循环".to_string(),
        TcoVerdict::NotTailPosition { sites } => format!(
            "自调用不在尾位置（{}；共 {} 处违规）",
            sites.first().cloned().unwrap_or_default(),
            sites.len()
        ),
        TcoVerdict::NotTransformable { reason } => format!("体含不可转换结构：{}", reason),
    }
}

// ═══════════════════════════════════════════════════════════════════
// 改写：尾自调用 → 形参重赋 + continue；原返回值 → __tco_res + break
// ═══════════════════════════════════════════════════════════════════

struct Rewriter {
    fname: String,
    res: String,
    params: Vec<Param>,
    /// 已在函数体出现过的名字（用于 `__tco_*` 去重）
    used: std::collections::HashSet<String>,
}

impl Rewriter {
    fn fresh(&mut self, base: &str) -> String {
        let mut name = format!("__tco_{}", base);
        let mut i = 0;
        while self.used.contains(&name) {
            i += 1;
            name = format!("__tco_{}{}", base, i);
        }
        self.used.insert(name.clone());
        name
    }

    fn var(&self, name: &str) -> Expr {
        Expr::new(
            ExprKind::Var(name.to_string()),
            IrType::Any,
            Span::unknown(),
        )
    }

    /// 尾自调用 → [求参临时槽…, 形参重赋…, continue]
    fn assign_from_call(&mut self, args: &[Expr]) -> Vec<Stmt> {
        let mut out = Vec::new();
        // 1) 求参先行：实参表达式先物化（防 p1=a+1 后 p2 用到新 p1）
        let mut slots: Vec<String> = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let t = self.fresh(&format!("a{}", i));
            out.push(Stmt::Let {
                name: t.clone(),
                ty: a.ty.clone(),
                value: a.clone(),
                is_mut: false,
                is_ref: false,
                mods: IrMods::default(),
            });
            slots.push(t);
        }
        // 2) 形参重赋
        for (i, p) in self.params.iter().enumerate() {
            let value = if let Some(slot) = slots.get(i) {
                self.var(slot)
            } else {
                // 省略的实参用形参默认值（arity/default 已在 verdict 阶段校验）
                p.default.clone().unwrap_or_else(|| self.var(&p.name))
            };
            out.push(Stmt::Assign {
                target: self.var(&p.name),
                value,
            });
        }
        // 3) 落循环尾
        out.push(Stmt::Continue);
        out
    }

    /// 尾位置 `return v`
    fn ret_value(&mut self, e: &Expr) -> Vec<Stmt> {
        if self.is_self_call(e) {
            let a = self.call_args(e);
            return self.assign_from_call(&a);
        }
        let mut out = vec![Stmt::Assign {
            target: self.var(&self.res),
            value: e.clone(),
        }];
        out.push(Stmt::Break);
        out
    }

    fn is_self_call(&self, e: &Expr) -> bool {
        matches!(&e.kind, ExprKind::Call { callee, .. }
            if matches!(&callee.kind, ExprKind::Var(v) if *v == self.fname))
    }

    fn call_args(&self, e: &Expr) -> Vec<Expr> {
        match &e.kind {
            ExprKind::Call { args, .. } => args.clone(),
            _ => Vec::new(),
        }
    }

    /// 块（含尾语句）改写
    fn block(&mut self, stmts: &[Stmt]) -> Vec<Stmt> {
        let n = stmts.len();
        let mut out = Vec::new();
        for (i, st) in stmts.iter().enumerate() {
            let is_last = i + 1 == n;
            match st {
                Stmt::Return { value: Some(e) } => {
                    out.extend(self.ret_value(e));
                }
                Stmt::Return { value: None } => {
                    out.push(Stmt::Break);
                }
                Stmt::ExprStmt { expr } if is_last => {
                    out.extend(self.tail_expr(expr));
                }
                Stmt::If {
                    cond,
                    then_branch,
                    else_branch,
                } if is_last => {
                    out.push(Stmt::If {
                        cond: cond.clone(),
                        then_branch: Block {
                            stmts: self.block(&then_branch.stmts),
                            ty: IrType::Unit,
                            span: then_branch.span.clone(),
                        },
                        else_branch: else_branch.as_ref().map(|b| Block {
                            stmts: self.block(&b.stmts),
                            ty: IrType::Unit,
                            span: b.span.clone(),
                        }),
                    });
                }
                other => out.push(other.clone()),
            }
        }
        out
    }

    /// 尾位置表达式改写
    fn tail_expr(&mut self, e: &Expr) -> Vec<Stmt> {
        match &e.kind {
            ExprKind::Call { .. } if self.is_self_call(e) => {
                let a = self.call_args(e);
                self.assign_from_call(&a)
            }
            ExprKind::IfExpr { cond, then, els } => {
                let then_b = self.tail_expr(then);
                let els_b = self.tail_expr(els);
                let ty = e.ty.clone();
                vec![Stmt::ExprStmt {
                    expr: Expr::new(
                        ExprKind::IfExpr {
                            cond: cond.clone(),
                            then: Box::new(Expr::new(
                                ExprKind::BlockExpr {
                                    block: Block {
                                        stmts: then_b,
                                        ty: IrType::Unit,
                                        span: Span::unknown(),
                                    },
                                },
                                ty.clone(),
                                Span::unknown(),
                            )),
                            els: Box::new(Expr::new(
                                ExprKind::BlockExpr {
                                    block: Block {
                                        stmts: els_b,
                                        ty: IrType::Unit,
                                        span: Span::unknown(),
                                    },
                                },
                                ty,
                                Span::unknown(),
                            )),
                        },
                        e.ty.clone(),
                        e.span.clone(),
                    ),
                }]
            }
            ExprKind::BlockExpr { block } => self.block(&block.stmts),
            _ => self.ret_value(e),
        }
    }
}

/// 收集函数体内已用到的名字（形参 / let 绑定 / var 引用），避免 `__tco_*` 撞名
fn collect_used_names(body: &Block, params: &[Param], out: &mut std::collections::HashSet<String>) {
    for p in params {
        out.insert(p.name.clone());
    }
    let mut stmts: Vec<&[Stmt]> = vec![&body.stmts];
    let mut stack: Vec<&Expr> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    while let Some(ss) = stmts.pop() {
        for st in ss {
            if let Stmt::Let { name, value, .. } = st {
                names.push(name.clone());
                stack.push(value);
            }
        }
        collect_stmt_subtrees(ss, &mut stmts, &mut stack);
    }
    while let Some(e) = stack.pop() {
        if let ExprKind::Var(n) = &e.kind {
            out.insert(n.clone());
        }
        for c in expr_children(&e.kind) {
            match c {
                Child::Expr(x) => stack.push(x),
                Child::Block(b) => stmts.push(&b.stmts),
            }
        }
    }
    out.extend(names);
}

/// 对单个函数就地改写为循环；返回是否真的改写了
fn rewrite_fn(f: &mut FnDef) -> bool {
    if !matches!(f.tco, Some(TcoVerdict::TailOptimizable)) {
        return false;
    }
    let inp = TcoInput {
        fname: &f.name,
        params: &f.params,
        ret_ty: &f.ret_ty,
        raises: f.raises.is_some(),
        is_iterator: f.is_iterator,
        is_async: f.is_async,
        intrinsics: &f.intrinsics,
        body: &f.body,
    };
    // 二次判定（verdict 可能来自旧 IR / 标注关闭自动路径）
    if analyze_tail_recursion(&inp) != TcoVerdict::TailOptimizable {
        return false;
    }
    let mut used = std::collections::HashSet::new();
    collect_used_names(&f.body, &f.params, &mut used);
    let has_res = f.ret_ty != IrType::Unit;
    let mut rw = Rewriter {
        fname: f.name.clone(),
        res: String::new(),
        params: f.params.clone(),
        used,
    };
    rw.res = rw.fresh("res");
    let loop_body = rw.block(&f.body.stmts);
    let res_name = rw.res.clone();

    let mut out: Vec<Stmt> = Vec::new();
    if has_res {
        let dv = default_value(&f.ret_ty).expect("verdict 已保证存在默认初值");
        out.push(Stmt::Let {
            name: res_name.clone(),
            ty: f.ret_ty.clone(),
            value: dv,
            is_mut: true,
            is_ref: false,
            mods: IrMods::default(),
        });
    }
    out.push(Stmt::While {
        cond: Expr::new(ExprKind::Lit(LitKind::Bool(true)), IrType::Bool, Span::unknown()),
        guard: None,
        body: Block {
            stmts: loop_body,
            ty: IrType::Unit,
            span: Span::unknown(),
        },
        else_body: None,
    });
    out.push(Stmt::Return {
        value: Some(Expr::new(
            ExprKind::Var(res_name),
            f.ret_ty.clone(),
            Span::unknown(),
        )),
    });
    // 形参需要可重赋（循环体内每次尾调用都重写全部形参）。
    // 带默认值的形参例外：Rust 侧形参是 `Option<T>`，真正的绑定是 codegen 发的
    // `let mut x = x.unwrap_or(..)` 影子（见 decl_gen::block_assigns_param），
    // 形参本身从不赋值，标 mut 只会产生 unused_mut 告警。
    for p in f.params.iter_mut() {
        if p.default.is_none() {
            p.is_mut = true;
        }
    }
    f.body = Block {
        stmts: out,
        ty: f.ret_ty.clone(),
        span: f.body.span.clone(),
    };
    // verdict 消费完毕，置 None 防止二次改写
    f.tco = Some(TcoVerdict::NoSelfCall);
    true
}

/// 模块级收口：把所有 TailOptimizable 的 FnDef 就地改写为循环。
///
/// 幂等：改写后 `tco` 置为 `NoSelfCall`，重复调用不会二次改写。
pub(crate) fn rewrite_tco(module: &mut IrModule) {
    if !tco_enabled() {
        return;
    }
    let mut n = 0;
    for item in module.items.iter_mut() {
        match item {
            Item::FnDef(f) => n += rewrite_fn(f) as usize,
            Item::StructDef(sd) => {
                for m in sd.methods.iter_mut() {
                    n += rewrite_fn(m) as usize;
                }
            }
            Item::EnumDef(ed) => {
                for m in ed.methods.iter_mut() {
                    n += rewrite_fn(m) as usize;
                }
            }
            Item::Impl(im) => {
                for m in im.methods.iter_mut() {
                    n += rewrite_fn(m) as usize;
                }
            }
            _ => {}
        }
    }
    if n > 0 {
        eprintln!("[tco] 自动改写 {} 个尾递归函数为循环", n);
    }
}

// ═══════════════════════════════════════════════════════════════════
// 单元测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn lit_i(v: i64) -> Expr {
        Expr::new(ExprKind::Lit(LitKind::Int(v)), IrType::Int, Span::unknown())
    }

    fn var(n: &str) -> Expr {
        Expr::new(ExprKind::Var(n.into()), IrType::Any, Span::unknown())
    }

    fn self_call(n: &str) -> Expr {
        Expr::new(
            ExprKind::Call {
                callee: Box::new(var(n)),
                type_args: vec![],
                args: vec![],
            },
            IrType::Any,
            Span::unknown(),
        )
    }

    fn ret(e: Expr) -> Stmt {
        Stmt::Return { value: Some(e) }
    }

    fn blk(stmts: Vec<Stmt>) -> Block {
        Block {
            stmts,
            ty: IrType::Unit,
            span: Span::unknown(),
        }
    }

    fn input<'a>(name: &'a str, params: &'a [Param], body: &'a Block) -> TcoInput<'a> {
        TcoInput {
            fname: name,
            params,
            ret_ty: &IrType::Int,
            raises: false,
            is_iterator: false,
            is_async: false,
            intrinsics: &[],
            body,
        }
    }

    fn params(names: &[&str]) -> Vec<Param> {
        names
            .iter()
            .map(|n| Param {
                name: (*n).into(),
                ty: IrType::Int,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: false,
                comptime: false,
                mods: IrMods::default(),
            })
            .collect()
    }

    #[test]
    fn verdict_no_self_call() {
        let ps = params(&["n"]);
        let body = blk(vec![ret(lit_i(1))]);
        assert_eq!(
            analyze_tail_recursion(&input("f", &ps, &body)),
            TcoVerdict::NoSelfCall
        );
    }

    #[test]
    fn verdict_tail_optimizable() {
        let ps = params(&["n", "acc"]);
        // if n<=1 { acc } else { f(n-1, acc*n) } → ExprStmt{IfExpr}
        let mut call = self_call("f");
        if let ExprKind::Call { args, .. } = &mut call.kind {
            args.push(var("n"));
            args.push(var("acc"));
        }
        let body = blk(vec![Stmt::ExprStmt {
            expr: Expr::new(
                ExprKind::IfExpr {
                    cond: Box::new(var("n")),
                    then: Box::new(var("acc")),
                    els: Box::new(call),
                },
                IrType::Int,
                Span::unknown(),
            ),
        }]);
        assert_eq!(
            analyze_tail_recursion(&input("f", &ps, &body)),
            TcoVerdict::TailOptimizable
        );
    }

    #[test]
    fn verdict_non_tail_position() {
        let ps = params(&["n"]);
        // let x = f(n) —— 自调用在 let 绑定，非尾
        let body = blk(vec![Stmt::Let {
            name: "x".into(),
            ty: IrType::Int,
            value: self_call("f"),
            is_mut: false,
            is_ref: false,
            mods: IrMods::default(),
        }, ret(var("x"))]);
        match analyze_tail_recursion(&input("f", &ps, &body)) {
            TcoVerdict::NotTailPosition { sites } => assert!(!sites.is_empty()),
            other => panic!("期望 NotTailPosition，得到 {:?}", other),
        }
    }

    #[test]
    fn verdict_closure_body_not_counted() {
        let ps = params(&["n"]);
        // let g = || f(n) —— 闭包内自调用不算外层尾调用（Scala 语义）
        let body = blk(vec![Stmt::ExprStmt {
            expr: Expr::new(
                ExprKind::Lambda {
                    params: vec![],
                    body: Box::new(self_call("f")),
                    is_move: false,
                    ret_ty: None,
                },
                IrType::Any,
                Span::unknown(),
            ),
        }]);
        assert_eq!(
            analyze_tail_recursion(&input("f", &ps, &body)),
            TcoVerdict::NoSelfCall
        );
    }

    #[test]
    fn verdict_loop_body_is_not_tail() {
        let ps = params(&["n"]);
        let body = blk(vec![Stmt::While {
            cond: var("n"),
            guard: None,
            body: blk(vec![ret(self_call("f"))]),
            else_body: None,
        }]);
        match analyze_tail_recursion(&input("f", &ps, &body)) {
            TcoVerdict::NotTailPosition { .. } => {}
            other => panic!("期望 NotTailPosition，得到 {:?}", other),
        }
    }

    #[test]
    fn verdict_raises_not_transformable() {
        let ps = params(&["n"]);
        let body = blk(vec![ret(self_call("f"))]);
        let mut inp = input("f", &ps, &body);
        inp.raises = true;
        match analyze_tail_recursion(&inp) {
            TcoVerdict::NotTransformable { .. } => {}
            other => panic!("期望 NotTransformable，得到 {:?}", other),
        }
    }

    #[test]
    fn rewrite_produces_loop_and_drops_self_call() {
        let ps = params(&["n", "acc"]);
        let mut call = self_call("f");
        if let ExprKind::Call { args, .. } = &mut call.kind {
            args.push(var("n"));
            args.push(var("acc"));
        }
        let body = blk(vec![Stmt::ExprStmt {
            expr: Expr::new(
                ExprKind::IfExpr {
                    cond: Box::new(var("n")),
                    then: Box::new(var("acc")),
                    els: Box::new(call),
                },
                IrType::Int,
                Span::unknown(),
            ),
        }]);
        let mut f = FnDef {
            name: "f".into(),
            generics: vec![],
            params: ps,
            ret_ty: IrType::Int,
            raises: None,
            body,
            intrinsics: vec![],
            is_async: false,
            is_iterator: false,
            is_test: false,
            checker_param: None,
            default_checker: None,
            where_clause: vec![],
            span: Span::unknown(),
            tco: Some(TcoVerdict::TailOptimizable),
        };
        assert!(rewrite_fn(&mut f));
        // 体内不再有对 f 的调用，且存在 While
        assert_eq!(count_self_calls(&f.body, "f"), 0, "改写后不应残留自调用");
        assert!(
            f.body.stmts.iter().any(|s| matches!(s, Stmt::While { .. })),
            "改写后应出现 While 节点"
        );
        // 形参变为 mut
        assert!(f.params.iter().all(|p| p.is_mut));
    }

    #[test]
    fn rewrite_evaluates_args_before_reassigning() {
        let ps = params(&["n", "m"]);
        // f(n + 1, n) —— 第二个实参必须取旧 n
        let mut call = self_call("f");
        if let ExprKind::Call { args, .. } = &mut call.kind {
            args.push(Expr::new(
                ExprKind::BinOp {
                    op: BinOpKind::Add,
                    lhs: Box::new(var("n")),
                    rhs: Box::new(lit_i(1)),
                },
                IrType::Int,
                Span::unknown(),
            ));
            args.push(var("n"));
        }
        let body = blk(vec![ret(call)]);
        let mut f = FnDef {
            name: "f".into(),
            generics: vec![],
            params: ps,
            ret_ty: IrType::Int,
            raises: None,
            body,
            intrinsics: vec![],
            is_async: false,
            is_iterator: false,
            is_test: false,
            checker_param: None,
            default_checker: None,
            where_clause: vec![],
            span: Span::unknown(),
            tco: Some(TcoVerdict::TailOptimizable),
        };
        assert!(rewrite_fn(&mut f));
        // 求参先行的临时绑定必须排在形参重赋之前（`let __tco_a0 = n + 1`）
        let loop_stmts: Vec<Stmt> = f
            .body
            .stmts
            .iter()
            .find_map(|s| match s {
                Stmt::While { body, .. } => Some(body.stmts.clone()),
                _ => None,
            })
            .expect("改写后应有 while 循环");
        let slot_pos = loop_stmts
            .iter()
            .position(|s| matches!(s, Stmt::Let { name, .. } if name.starts_with("__tco_a")))
            .expect("循环体内应有求参临时绑定");
        match &loop_stmts[slot_pos] {
            Stmt::Let { name, value, .. } => {
                assert!(name.starts_with("__tco_a"));
                assert!(
                    matches!(value.kind, ExprKind::BinOp { .. }),
                    "临时槽应保存实参表达式，实际: {:?}",
                    name
                );
            }
            other => panic!("求参临时槽应为 Let，实际: {:?}", other),
        }
        let first_assign = loop_stmts
            .iter()
            .position(|s| matches!(s, Stmt::Assign { .. }))
            .expect("循环体内应有形参重赋");
        assert!(
            slot_pos < first_assign,
            "求参临时绑定({}) 必须早于首个形参重赋({})",
            slot_pos,
            first_assign
        );
        // 形参重赋用的是临时槽而非原表达式
        assert!(loop_stmts
            .iter()
            .any(|s| matches!(s, Stmt::Continue)));
    }

    #[test]
    fn verdict_arity_mismatch_rejected() {
        let mut ps = params(&["n", "acc"]);
        ps[1].default = None;
        let body = blk(vec![ret(self_call("f"))]); // 实参 0 个 < 形参 2 个
        match analyze_tail_recursion(&input("f", &ps, &body)) {
            TcoVerdict::NotTransformable { reason } => {
                assert!(
                    reason.contains("省略了无默认值的形参"),
                    "实际: {}",
                    reason
                )
            }
            other => panic!("期望 NotTransformable，得到 {:?}", other),
        }
    }

    #[test]
    fn switch_env_and_explicit_toggle() {
        set_tco_enabled(true);
        // 不直接断言 env（受测试进程环境影响），只验证显式开关生效且可恢复
        set_tco_enabled(false);
        assert!(!tco_enabled());
        set_tco_enabled(true);
    }
}
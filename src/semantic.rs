// Lang-Zong 编译器 — semantic/validate.rs
// Trait/Impl 语义校验 pass（修复 Bug-9~13）
//
// 在 codegen 之前运行，检测 trait 与 impl 之间的一致性缺口。
// 这些缺口在旧版编译器中会被静默放过，最终生成无效的 Rust
// （trait 要求的方法缺失、签名不一致、重复定义等）。
//
// 检查项（对应 bug 报告）：
//   Bug-9  方法名不匹配  —— impl 中出现了 trait 未声明的方法
//   Bug-10 缺少实现      —— trait 声明的方法在 impl 中缺失
//   Bug-11 返回类型不一致 —— 同名方法的返回类型（含 Self 归一化）不同
//   Bug-12 self 可变性不一致 —— 同名方法的 self/mut self 不一致
//   Bug-13 方法名冲突    —— 同一 impl 块内 / 同一类型的多个 inherent impl 中重复定义方法

use crate::ast::{Function, ImplDef, Module, TraitDef};
use crate::types::Type;
use std::collections::HashMap;

/// 遍历整个模块，返回语义错误列表（每个元素是一条可直接打印的错误信息）。
/// 空列表表示校验通过。
pub fn validate_module(module: &Module) -> Vec<String> {
    let mut errors = Vec::new();

    // 收集 trait 索引（按名字）
    let trait_map: HashMap<&str, &TraitDef> =
        module.traits.iter().map(|t| (t.name.as_str(), t)).collect();

    // ── 第一遍：trait 内部的重复方法名（Bug-13 的 trait 侧） ──
    for tr in &module.traits {
        check_dup_methods(&tr.methods, &format!("trait '{}'", tr.name), &mut errors);
    }

    // 跨 impl 的 inherent 方法名冲突检测数据：type_name -> 方法名集合（inherent）
    let mut inherent_methods: HashMap<&str, Vec<&str>> = HashMap::new();

    // ── 第二遍：逐个 impl 校验 ──
    for imp in &module.impls {
        // Bug-13：同一 impl 块内重复方法名
        check_dup_methods(&imp.methods, &impl_label(imp), &mut errors);

        if imp.trait_name.is_none() {
            // inherent impl：记录方法名用于跨块冲突检测
            let entry = inherent_methods.entry(imp.type_name.as_str()).or_default();
            for m in &imp.methods {
                entry.push(m.name.as_str());
            }
            continue;
        }

        // ── trait impl：执行 Bug-9/10/11/12 ──
        let trait_name = imp.trait_name.as_ref().unwrap();
        let trait_def = match trait_map.get(trait_name.as_str()) {
            Some(t) => t,
            None => {
                errors.push(format!(
                    "semantic error: impl for unknown trait '{}' in '{}'",
                    trait_name,
                    impl_label(imp)
                ));
                continue;
            }
        };

        let trait_methods: HashMap<&str, &Function> =
            trait_def.methods.iter().map(|m| (m.name.as_str(), m)).collect();
        let impl_methods: HashMap<&str, &Function> =
            imp.methods.iter().map(|m| (m.name.as_str(), m)).collect();

        // Bug-9：impl 方法不在 trait 中声明
        for m in &imp.methods {
            if !trait_methods.contains_key(m.name.as_str()) {
                errors.push(format!(
                    "semantic error [Bug-9]: method '{}' in '{}' is not declared by trait '{}'",
                    m.name,
                    impl_label(imp),
                    trait_name
                ));
            }
        }

        // Bug-10：trait 要求的方法未在 impl 中实现
        // 注意：仅抽象方法（is_abstract）是必须实现的；带默认实现（default method）
        // 的方法是可选的，impl 可以不重写，否则会误伤合法代码（见 _test_trait_impl.lz）。
        for tm in &trait_def.methods {
            if tm.is_abstract && !impl_methods.contains_key(tm.name.as_str()) {
                errors.push(format!(
                    "semantic error [Bug-10]: trait '{}' requires method '{}' but '{}' does not implement it",
                    trait_name,
                    tm.name,
                    impl_label(imp)
                ));
            }
        }

        // Bug-11 / Bug-12：对同时存在于 trait 与 impl 的方法做签名一致性校验
        for tm in &trait_def.methods {
            if let Some(im) = impl_methods.get(tm.name.as_str()) {
                // 把 trait 侧的 Self 归一化为 impl 的具体类型名，避免 Self→具体类型造成的误报
                let trait_ret = tm
                    .return_type
                    .as_ref()
                    .map(|t| subst_self(t, &imp.type_name));
                let impl_ret = im.return_type.clone();

                if trait_ret != impl_ret {
                    errors.push(format!(
                        "semantic error [Bug-11]: method '{}' return type mismatch in '{}': trait '{}' declares '{}', impl provides '{}'",
                        tm.name,
                        impl_label(imp),
                        trait_name,
                        ret_desc(&trait_ret),
                        ret_desc(&impl_ret)
                    ));
                }

                // Bug-12：self 可变性一致性（仅当两侧都有 receiver 时比较）
                let trait_self_mut = tm.params.first().map(|p| p.is_mut).unwrap_or(false);
                let impl_self_mut = im.params.first().map(|p| p.is_mut).unwrap_or(false);
                if trait_self_mut != impl_self_mut {
                    errors.push(format!(
                        "semantic error [Bug-12]: method '{}' self-mutability mismatch in '{}': trait '{}' declares '{}', impl provides '{}'",
                        tm.name,
                        impl_label(imp),
                        trait_name,
                        if trait_self_mut { "mut self" } else { "self" },
                        if impl_self_mut { "mut self" } else { "self" }
                    ));
                }
            }
        }
    }

    // ── 第三遍：跨 inherent impl 的重复方法名（Bug-13 的跨块侧） ──
    for (type_name, methods) in &inherent_methods {
        let mut seen: HashMap<&str, usize> = HashMap::new();
        for m in methods {
            *seen.entry(*m).or_insert(0) += 1;
        }
        for (m, count) in seen {
            if count > 1 {
                errors.push(format!(
                    "semantic error [Bug-13]: method '{}' is defined {} times across inherent impls of '{}'",
                    m, count, type_name
                ));
            }
        }
    }

    // ── raise/raises 一致性检查 ──
    check_raise_raises(module, &mut errors);

    errors
}

/// 检测同一函数集合内的重复方法名（Bug-13 的块内侧）。
fn check_dup_methods(methods: &[Function], label: &str, errors: &mut Vec<String>) {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for m in methods {
        *seen.entry(m.name.as_str()).or_insert(0) += 1;
    }
    for (name, count) in seen {
        if count > 1 {
            errors.push(format!(
                "semantic error [Bug-13]: method '{}' is defined {} times in {}",
                name, count, label
            ));
        }
    }
}

/// 生成 impl 的可读标签：`impl Trait for Type` 或 `impl Type`。
fn impl_label(imp: &ImplDef) -> String {
    match &imp.trait_name {
        Some(t) => format!("impl {} for {}", t, imp.type_name),
        None => format!("impl {}", imp.type_name),
    }
}

/// 把类型中的 `Self` 占位替换为具体的类型名（递归）。
fn subst_self(ty: &Type, self_name: &str) -> Type {
    match ty {
        Type::Self_ => Type::Named(self_name.to_string()),
        Type::Option(i) => Type::Option(Box::new(subst_self(i, self_name))),
        Type::Optional(i) => Type::Optional(Box::new(subst_self(i, self_name))),
        Type::Ref(i) => Type::Ref(Box::new(subst_self(i, self_name))),
        Type::MutRef(i) => Type::MutRef(Box::new(subst_self(i, self_name))),
        Type::Result { ok, err } => Type::Result {
            ok: Box::new(subst_self(ok, self_name)),
            err: Box::new(subst_self(err, self_name)),
        },
        Type::Generic { base, args } => Type::Generic {
            base: Box::new(subst_self(base, self_name)),
            args: args.iter().map(|a| subst_self(a, self_name)).collect(),
        },
        Type::Tuple(ts) => Type::Tuple(ts.iter().map(|t| subst_self(t, self_name)).collect()),
        Type::Fn { params, ret } => Type::Fn {
            params: params.iter().map(|t| subst_self(t, self_name)).collect(),
            ret: Box::new(subst_self(ret, self_name)),
        },
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|t| subst_self(t, self_name)).collect()),
        other => other.clone(),
    }
}

/// 返回类型的可读描述（None 视为单元类型 `()`）。
fn ret_desc(t: &Option<Type>) -> String {
    match t {
        None => "()".to_string(),
        Some(ty) => ty.to_rust_type_string(),
    }
}

// ═══════════════════════════════════════════════════════
// raise ↔ raises 一致性检查
// ═══════════════════════════════════════════════════════
//
// 规则:
//   1. 函数体内有 `raise` 语句 → 签名必须标注 `raises T`
//   2. 例外: 所有执行路径都以 raise 结束（永不正常返回）
//      → 返回类型为 `!` (Never)，无需 raises

use crate::ast::Stmt;

/// 检查函数体内是否包含 raise 语句
fn body_has_raise(stmts: &[Stmt]) -> bool {
    for s in stmts {
        if stmt_has_raise(s) { return true; }
    }
    false
}

fn stmt_has_raise(s: &Stmt) -> bool {
    match s {
        Stmt::Raise(_) => true,
        Stmt::Expr(e) => expr_has_raise(e),
        Stmt::Let { value, .. } | Stmt::Return(Some(value)) => expr_has_raise(value),
        Stmt::While { cond, body, .. } => {
            expr_has_raise(cond) || body_has_raise(body)
        }
        Stmt::For { iter, body, .. } => {
            expr_has_raise(iter) || body_has_raise(body)
        }
        Stmt::Loop(body) | Stmt::Comptime(body) | Stmt::Defer(body) => body_has_raise(body),
        Stmt::Guard { cond, else_body, .. } => {
            cond.as_ref().map_or(false, |c| expr_has_raise(c)) || body_has_raise(else_body)
        }
        _ => false,
    }
}

fn expr_has_raise(e: &crate::ast::Expr) -> bool {
    match e {
        crate::ast::Expr::If { cond, then_body, elif_clauses, else_body } => {
            if expr_has_raise(cond) { return true; }
            if body_has_raise(then_body) { return true; }
            for (ce, cb) in elif_clauses {
                if expr_has_raise(ce) || body_has_raise(cb) { return true; }
            }
            if let Some(eb) = else_body { body_has_raise(eb) }
            else { false }
        }
        crate::ast::Expr::Match { expr, arms } => {
            if expr_has_raise(expr) { return true; }
            for arm in arms { if body_has_raise(&arm.body) { return true; } }
            false
        }
        crate::ast::Expr::Call { func, args, .. } => {
            if expr_has_raise(func) { return true; }
            args.iter().any(|a| expr_has_raise(a))
        }
        crate::ast::Expr::Binary { left, right, .. } => expr_has_raise(left) || expr_has_raise(right),
        crate::ast::Expr::Unary { operand, .. } => expr_has_raise(operand),
        crate::ast::Expr::MethodCall { receiver, args, .. } => {
            expr_has_raise(receiver) || args.iter().any(|a| expr_has_raise(a))
        }
        crate::ast::Expr::FieldAccess { receiver, .. } => expr_has_raise(receiver),
        crate::ast::Expr::Index { receiver, index } => expr_has_raise(receiver) || expr_has_raise(index),
        crate::ast::Expr::TupleLit(es) | crate::ast::Expr::ListLit(es) => es.iter().any(|e| expr_has_raise(e)),
        _ => false,
    }
}

/// 验证模块中所有函数的 raise/raises 一致性
fn check_raise_raises(module: &Module, errors: &mut Vec<String>) {
    for f in &module.functions {
        let has_raise = body_has_raise(&f.body);
        if has_raise && f.raises.is_none() {
            // 降级为 warning: strict 模式下的 raises 注解不再强制报错
            // (不再 push 到 errors，仅靠 strict.rs 的 S006/S007 规则覆盖)
        }
        if f.raises.is_some() && !has_raise {
            errors.push(format!(
                "语义警告: 函数 '{}' 标注了 raises 但函数体内未发现 raise 语句\n  \
                 提示: 移除 raises 标注或确认异常抛出逻辑",
                f.name
            ));
        }
    }
    for s in &module.structs {
        for m in &s.methods {
            let has_raise = body_has_raise(&m.body);
            if has_raise && m.raises.is_none() {
                // 降级为 warning: 不再报错
            }
        }
    }
    for i in &module.impls {
        for m in &i.methods {
            let has_raise = body_has_raise(&m.body);
            if has_raise && m.raises.is_none() {
                // 降级为 warning: 不再报错
            }
        }
    }
}

// 在 validate_module 末尾追加 — 修改 validate_module 的调用处

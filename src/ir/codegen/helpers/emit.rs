// Lang-Zone 编译器 — ir/codegen/helpers/emit.rs
// 文本发射/改写 + 类型变换：返回 String 的片段生成与重写，及 IrType 构造/替换/推断
// 自 helpers.rs 逐字节拆出（move-only）：函数体/注释/签名原样保留，仅新增本文件头与 use。
use crate::ir::codegen::CodeGen;
use crate::ir::node::*;
use crate::ir::types::IrType;
use std::collections::HashMap;
use super::pred::is_ctor_name;

/// 查找 Block 中第一个 yield 表达式的 IR 类型（生成器构建块 func *: 收集器类型用）
pub(crate) fn first_yield_type(block: &Block) -> Option<IrType> {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Yield { value } => return Some(value.ty.clone()),
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(t) = first_yield_type(then_branch) {
                    return Some(t);
                }
                if let Some(e) = else_branch {
                    if let Some(t) = first_yield_type(e) {
                        return Some(t);
                    }
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::WhileLet { body, .. } => {
                if let Some(t) = first_yield_type(body) {
                    return Some(t);
                }
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    if let Some(t) = first_yield_type(&arm.body) {
                        return Some(t);
                    }
                }
            }
            // BUG-289：try/catch/finally 各分支体是 Block，yield 需下钻
            Stmt::TryCatch {
                body,
                catches,
                else_body,
                finally_body,
            } => {
                if let Some(t) = first_yield_type(body) {
                    return Some(t);
                }
                for (_, b) in catches {
                    if let Some(t) = first_yield_type(b) {
                        return Some(t);
                    }
                }
                if let Some(eb) = else_body {
                    if let Some(t) = first_yield_type(eb) {
                        return Some(t);
                    }
                }
                if let Some(fb) = finally_body {
                    if let Some(t) = first_yield_type(fb) {
                        return Some(t);
                    }
                }
            }
            Stmt::ExprStmt { expr } => {
                if let Some(t) = expr_first_yield_type(expr) {
                    return Some(t);
                }
            }
            Stmt::Let { value, .. } => {
                if let Some(t) = expr_first_yield_type(value) {
                    return Some(t);
                }
            }
            _ => {}
        }
    }
    None
}

/// 查找 Expr 中第一个 yield 表达式的 IR 类型
pub(crate) fn expr_first_yield_type(expr: &Expr) -> Option<IrType> {
    match &expr.kind {
        ExprKind::BlockExpr { block } => first_yield_type(block),
        ExprKind::GenBuild { block, .. } => first_yield_type(block),
        ExprKind::IfExpr { then, els, .. } => {
            expr_first_yield_type(then).or_else(|| expr_first_yield_type(els))
        }
        ExprKind::Call { callee, args, .. } => {
            expr_first_yield_type(callee).or_else(|| args.iter().find_map(expr_first_yield_type))
        }
        ExprKind::Lambda { body, .. } => expr_first_yield_type(body),
        // 同 expr_has_yield 的 EnumCtor 下钻：raises 生成器体包在 Ok(BlockExpr) 内
        ExprKind::EnumCtor { args, .. } => args.iter().find_map(expr_first_yield_type),
        _ => None,
    }
}

/// 从 _KwArg 中提取字段值（丢弃字段名，用于位置参数构造）
pub(crate) fn gen_kwarg_value(arg: &Expr, cg: &CodeGen) -> String {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
        if name == "_KwArg" {
            return fields
                .iter()
                .find(|(n, _)| n == "value")
                .map(|(_, v)| cg.gen_expr(v))
                .unwrap_or_default();
        }
    }
    cg.gen_expr(arg)
}

/// 转义 format! 字符串中的独立 { / }（避免被误判为占位符）
/// 已转义的 {{ 或 }} 保持不变
pub(crate) fn escape_format_braces(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    // 已是 {{，保留（显示字面 {）
                    chars.next();
                    out.push_str("{{");
                } else {
                    out.push_str("{{");
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    out.push_str("}}");
                } else {
                    out.push_str("}}");
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Strip `: Type` annotations from closure params (for comprehension closures)
/// "move |x: i64| { ... }" → "move |x| { ... }"
/// "move |acc: i64, x: i64| { ... }" → "move |acc, x| { ... }"（多参数逐个剥离）
pub(crate) fn strip_lambda_type(lambda: &str) -> String {
    // Find `|params|` region and strip each `name: Type` down to `name`
    if let Some(pipe_open) = lambda.find('|') {
        if let Some(rel_close) = lambda[pipe_open + 1..].find('|') {
            let pipe_close = pipe_open + 1 + rel_close;
            let params_part = &lambda[pipe_open + 1..pipe_close];
            // 多参数：按逗号分割，逐个剥离类型注解
            let stripped: String = params_part
                .split(',')
                .map(|p| {
                    let trimmed = p.trim();
                    if trimmed.is_empty() {
                        String::new()
                    } else if let Some(colon) = trimmed.find(':') {
                        trimmed[..colon].trim().to_string()
                    } else {
                        trimmed.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let before = &lambda[..pipe_open + 1];
            let after = &lambda[pipe_close..];
            return format!("{}{}{}", before, stripped, after);
        }
    }
    lambda.to_string()
}

/// Strip type annotations for filter-style closures that receive `&Item`.
/// 旧实现在参数名前硬加 `&`（`|&x|`）。这会与**两种**闭包体冲突：
///   1. body 已含 `*x`（free function `filter(items, |x| x % 2 == 0)` 经
///      ref 解引用生成 `*x % 2`）→ `|&x|` + `*x` = E0614 双重解引用；
///   2. body 直接用 `x`（comprehension `[f(x) for x in xs if x > 0]` 的
///      cond 闭包生成 `x > 0`）→ 参数必须为 `&x` 模式才能把 `&i64` 解引用
///      成 `i64`，否则 E0308。
/// 正确做法（按 body 决策，v10）：
///   - body 含 `*<param>` → 保留裸参数（`|x|`），`*x` 解引用 &i64；
///   - body 无解引用    → 加 `&` 模式（`|&x|`），体内 `x` 直接是 i64。
/// "move |x: i64| { *x % 2 }" → "move |x| { *x % 2 }"
/// "move |item: int| { item > 0 }" → "move |&item| { item > 0 }"
pub(crate) fn strip_lambda_type_with_ref(lambda: &str) -> String {
    let stripped = strip_lambda_type(lambda);
    // 用 lambda_split 解析 (参数名, body)；失败则回退 stripped。
    if let Some((params, body)) = lambda_split(&stripped) {
        // 任一参数被 body 显式 `*` 解引用 → 参数保持裸名
        let deref_used = params
            .iter()
            .any(|p| body.contains(&format!("*{}", p)));
        if !deref_used && !params.is_empty() {
            // body 直接用参数名 → 加 & 模式
            let ref_params: String = params.iter().map(|p| format!("&{}", p)).collect::<Vec<_>>().join(", ");
            // 找到管道位置重建 "move |&item| { body }"（stripped 前缀含 "move "，
            // 故不再另加 head）
            if let Some(pipe_open) = stripped.find('|') {
                if let Some(rel_close) = stripped[pipe_open + 1..].find('|') {
                    let pipe_close = pipe_open + 1 + rel_close;
                    // after 需跳过闭管 `|` 本身，否则拼出 `|&item||`（E0214）
                    let after = &stripped[pipe_close + 1..];
                    return format!("{}|{}|{}", &stripped[..pipe_open], ref_params, after);
                }
            }
        }
    }
    stripped
}

/// 解析 lambda 字符串为 (参数名列表, body 表达式)
/// "move |&x| { x <= pivot }" → (["x"], "x <= pivot")
/// "|x, y| x + y" → (["x", "y"], "x + y")
/// body 自动剥离最外层花括号。
pub(crate) fn lambda_split(lambda: &str) -> Option<(Vec<String>, String)> {
    let s = lambda.trim();
    // 去掉 "move " 前缀（借用捕获，避免 move 外部变量）
    let s = s.strip_prefix("move ").unwrap_or(s).trim();
    let pipe_open = s.find('|')?;
    let rel_close = s[pipe_open + 1..].find('|')?;
    let pipe_close = pipe_open + 1 + rel_close;
    let params: Vec<String> = s[pipe_open + 1..pipe_close]
        .split(',')
        .map(|p| p.trim().trim_start_matches('&').to_string())
        .collect();
    let after = &s[pipe_close + 1..];
    let body = after.trim();
    let body = body
        .strip_prefix('{')
        .and_then(|b| b.strip_suffix('}'))
        .map(|b| b.trim().to_string())
        .unwrap_or_else(|| body.to_string());
    Some((params, body))
}

/// 将 _KwArg { name, value } 展开为 "field: value"
pub(crate) fn gen_kwarg_field(arg: &Expr, cg: &CodeGen) -> String {
    if let ExprKind::StructCtor { name, fields } = &arg.kind {
        if name == "_KwArg" {
            let name_raw = fields
                .iter()
                .find(|(n, _)| n == "name")
                .and_then(|(_, v)| match &v.kind {
                    ExprKind::Lit(LitKind::Str(s)) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let value = fields.iter().find(|(n, _)| n == "value")
                .map(|(_, v)| {
                    let s = cg.gen_expr(v);
                    // &self 方法内构造时 move self.字段 → 需 .clone()
                    if cg.borrow_self
                        && matches!(&v.kind, ExprKind::FieldAccess { base, .. } if matches!(&base.kind, ExprKind::Var(n) if n == "self")) {
                        format!("{}.clone()", s)
                    } else if matches!(&v.kind, ExprKind::Var(_))
                        && !matches!(&v.kind, ExprKind::Var(ref n)
                            if n == "None" || n == "None_")
                        && !matches!(&v.ty, IrType::Int | IrType::F64 | IrType::Bool)
                    {
                        // 非 Copy 变量实参（String/Vec/Option 等）：struct 构造会 move，
                        // 后续再用该变量报 E0382（combo-defer-guard.lz FileHandle{path: path}）。
                        // None 无需 clone（(None).clone() 报 E0277 Option<_>: Clone）
                        format!("({}).clone()", s)
                    } else {
                        s
                    }
                })
                .unwrap_or_default();
            return format!("{}: {}", name_raw, value);
        }
    }
    cg.gen_expr(arg)
}

/// 递归替换类型中的 `Self` 引用为具体类型（struct 定义内 Self → 自身类型名）。
pub(crate) fn replace_self(ty: &IrType, self_ty: &IrType) -> IrType {
    match ty {
        IrType::Self_ => self_ty.clone(),
        IrType::Named { path, args } => {
            let new_args: Vec<IrType> = args.iter().map(|a| replace_self(a, self_ty)).collect();
            IrType::Named {
                path: path.clone(),
                args: new_args,
            }
        }
        IrType::Option(inner) => IrType::Option(Box::new(replace_self(inner, self_ty))),
        IrType::Result { ok, err } => IrType::Result {
            ok: Box::new(replace_self(ok, self_ty)),
            err: Box::new(replace_self(err, self_ty)),
        },
        IrType::Tuple(elems) => {
            IrType::Tuple(elems.iter().map(|e| replace_self(e, self_ty)).collect())
        }
        IrType::Ref(inner) => IrType::Ref(Box::new(replace_self(inner, self_ty))),
        IrType::MutRef(inner) => IrType::MutRef(Box::new(replace_self(inner, self_ty))),
        _ => ty.clone(),
    }
}

/// 从函数体推断全局变量的类型（查找 name = value 赋值，从 value 推断）
pub(crate) fn infer_global_type(block: &Block, name: &str, params: &[Param]) -> IrType {
    for p in params {
        if p.name == name {
            return p.ty.clone();
        }
    }
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { name: n, value, .. } if n == name => return value.ty.clone(),
            Stmt::Assign { target, value } => {
                if let ExprKind::Var(v) = &target.kind {
                    if v == name {
                        return value.ty.clone();
                    }
                }
            }
            _ => {}
        }
    }
    IrType::Int
}

/// 将可能包含空格/特殊字符的名称转换为合法 Rust 标识符。
/// 用于测试函数名等场景（如 "string concat" → "string_concat"）。
pub(crate) fn sanitize_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    // Rust 标识符不能以数字开头
    if out.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// 边界感知的标识符替换：将 s 中作为**独立标识符**出现的 `from` 替换为 `to`。
/// 用于 guard 闭包变量重命名（`i % 2 == 0` → `i_owned % 2i64 == 0i64`）：
/// 无脑 replace("i", "i_owned") 会把字面量后缀 `2i64` 里的 i 也替换成
/// i_owned64（invalid suffix `i_owned64`）。
pub(crate) fn replace_ident_boundary(s: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find(from) {
        let before_ok = pos == 0
            || !rest[..pos]
                .chars()
                .next_back()
                .map_or(false, |c| c.is_ascii_alphanumeric() || c == '_');
        let after = &rest[pos + from.len()..];
        let after_ok = after
            .chars()
            .next()
            .map_or(true, |c| !(c.is_ascii_alphanumeric() || c == '_'));
        if before_ok && after_ok {
            out.push_str(&rest[..pos]);
            out.push_str(to);
        } else {
            out.push_str(&rest[..pos + from.len()]);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// 非 move 闭包体内引用外层非 Copy 变量需 `.clone()`，否则借用移出 E0507。
/// 递归把表达式内的非 Copy 外部变量替换为 `var.clone()`。
/// （BUG-356：推导链 `|v| (k, v)` 中 k 由外层闭包借用，直接移入 tuple 即 E0507）
pub(crate) fn clone_noncopy_external(
    e: &Expr,
    lparam_names: &[String],
    variants: &HashMap<String, String>,
) -> Expr {
    match &e.kind {
        ExprKind::Var(name) => {
            // BUG-358：类型/枚举构造器名（Some/None/Ok/Err/集合别名）不是变量，不 clone
            if !is_ctor_name(name, variants)
                && !lparam_names.contains(name)
                && !matches!(e.ty, IrType::Int | IrType::F64 | IrType::Bool)
                && !name.ends_with('!')
            {
                let mut new_e = e.clone();
                new_e.kind = ExprKind::MethodCall {
                    receiver: Box::new(e.clone()),
                    method: "clone".to_string(),
                    args: vec![],
                };
                new_e
            } else {
                e.clone()
            }
        }
        ExprKind::BlockExpr { block } => {
            let mut nb = block.clone();
            nb.stmts = nb
                .stmts
                .iter()
                .map(|s| clone_noncopy_stmt(s, lparam_names, variants))
                .collect();
            let mut new_e = e.clone();
            new_e.kind = ExprKind::BlockExpr { block: nb };
            new_e
        }
        ExprKind::Lambda { params, body, .. } => {
            let inner_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
            let mut new_e = e.clone();
            if let ExprKind::Lambda { body: nb, .. } = &mut new_e.kind {
                *nb = Box::new(clone_noncopy_external(body, &inner_names, variants));
            }
            new_e
        }
        ExprKind::Call { callee, args, .. } => {
            let mut new_e = e.clone();
            if let ExprKind::Call { callee: nc, args: na, .. } = &mut new_e.kind {
                *nc = Box::new(clone_noncopy_external(callee, lparam_names, variants));
                *na = args
                    .iter()
                    .map(|a| clone_noncopy_external(a, lparam_names, variants))
                    .collect();
            }
            new_e
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            // BUG-357：若该 MethodCall 本身是 `外部非 Copy 变量.clone()`（本函数上一轮
            // 已包装），不要再给 receiver 追加一层 clone（避免 `k.clone().clone()`）。
            let already_cloned = matches!(&receiver.kind, ExprKind::Var(n)
                if !is_ctor_name(n, variants)
                    && !lparam_names.contains(n)
                    && !matches!(receiver.ty, IrType::Int | IrType::F64 | IrType::Bool));
            if already_cloned {
                return e.clone();
            }
            let mut new_e = e.clone();
            if let ExprKind::MethodCall { receiver: nr, args: na, .. } = &mut new_e.kind {
                *nr = Box::new(clone_noncopy_external(receiver, lparam_names, variants));
                *na = args
                    .iter()
                    .map(|a| clone_noncopy_external(a, lparam_names, variants))
                    .collect();
            }
            new_e
        }
        ExprKind::BinOp { lhs, rhs, .. } => {
            let mut new_e = e.clone();
            if let ExprKind::BinOp { lhs: nl, rhs: nr, .. } = &mut new_e.kind {
                *nl = Box::new(clone_noncopy_external(lhs, lparam_names, variants));
                *nr = Box::new(clone_noncopy_external(rhs, lparam_names, variants));
            }
            new_e
        }
        ExprKind::UnOp { operand, .. } => {
            let mut new_e = e.clone();
            if let ExprKind::UnOp { operand: no, .. } = &mut new_e.kind {
                *no = Box::new(clone_noncopy_external(operand, lparam_names, variants));
            }
            new_e
        }
        ExprKind::IfExpr { cond, then, els } => {
            let mut new_e = e.clone();
            if let ExprKind::IfExpr { cond: nc, then: nt, els: ne } = &mut new_e.kind {
                *nc = Box::new(clone_noncopy_external(cond, lparam_names, variants));
                *nt = Box::new(clone_noncopy_external(then, lparam_names, variants));
                *ne = Box::new(clone_noncopy_external(els, lparam_names, variants));
            }
            new_e
        }
        ExprKind::ListLit(items) | ExprKind::TupleLit(items) => {
            let is_list = matches!(&e.kind, ExprKind::ListLit(_));
            let mut new_e = e.clone();
            let nitems: &mut Vec<Expr> = if is_list {
                match &mut new_e.kind {
                    ExprKind::ListLit(n) => n,
                    _ => unreachable!(),
                }
            } else {
                match &mut new_e.kind {
                    ExprKind::TupleLit(n) => n,
                    _ => unreachable!(),
                }
            };
            *nitems = items
                .iter()
                .map(|i| clone_noncopy_external(i, lparam_names, variants))
                .collect();
            new_e
        }
        ExprKind::IndexGet { base, key, .. } => {
            let mut new_e = e.clone();
            if let ExprKind::IndexGet { base: nb, key: nk, .. } = &mut new_e.kind {
                *nb = Box::new(clone_noncopy_external(base, lparam_names, variants));
                *nk = Box::new(clone_noncopy_external(key, lparam_names, variants));
            }
            new_e
        }
        ExprKind::FieldAccess { base, .. } => {
            let mut new_e = e.clone();
            if let ExprKind::FieldAccess { base: nb, .. } = &mut new_e.kind {
                *nb = Box::new(clone_noncopy_external(base, lparam_names, variants));
            }
            new_e
        }
        _ => e.clone(),
    }
}

pub(crate) fn clone_noncopy_stmt(
    s: &Stmt,
    lparam_names: &[String],
    variants: &HashMap<String, String>,
) -> Stmt {
    match s {
        Stmt::ExprStmt { expr, .. } => Stmt::ExprStmt {
            expr: clone_noncopy_external(expr, lparam_names, variants),
        },
        Stmt::Let {
            name,
            value,
            ty,
            is_mut,
            is_ref,
            mods,
        } => Stmt::Let {
            name: name.clone(),
            value: clone_noncopy_external(value, lparam_names, variants),
            ty: ty.clone(),
            is_mut: *is_mut,
            is_ref: *is_ref,
            mods: mods.clone(),
        },
        _ => s.clone(),
    }
}

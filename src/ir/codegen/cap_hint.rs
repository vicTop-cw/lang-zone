// Lang-Zone 编译器 — ir/codegen/cap_hint.rs
// 方向 E：`[]` + 纯 push 循环 → `Vec::with_capacity(..)` 预分配
//
// 问题：`mut v: List<T> = []` 紧跟 `for i in range(a..b): v.push(x)` 是 LZ 里最常见的
// 「按范围构造容器」写法。此前生成 `Vec::new()` + 逐个 push，运行时经历
// ceil(log2(count)) 次「realloc + 全量拷贝」（sieve(300000) 的 Vec<bool> ≈ 19 次重分配、
// 累计拷贝约 4.8MB）。手写 Rust 的惯用解是 `Vec::with_capacity(n)`。
//
// 保守边界（放宽任何一条都可能改变语义，故逐条收紧）：
//   1. 绑定必须 `mut` 且右值是**空列表字面量**（ListLit 且无元素）；
//   2. 下一条语句必须是 `For`（无 guard），且循环体**恰好一条** `v.push(..)`，
//      receiver 就是上面那个变量名（否则 push 目标不是它，提示无意义）；
//   3. 迭代源必须能静态取出起止表达式（Range 或 range(a,b) / range(a)）；
//   4. 起止表达式必须是**纯算术**（字面量/变量/字段/一元负号/四则运算），
//      绝不能含函数调用/方法调用 —— 容量表达式会被求值两次（一次 with_capacity、
//      一次 for 的 range），有副作用就出错；
//   5. 容量取 `end.saturating_sub(start)`：避免 `usize` 下溢 panic（debug 构建会 panic），
//      也避免负数 `as usize` 变成天文数字导致 OOM。最后 `.max(0)` 兜底。

use super::*;

impl CodeGen {
    pub(crate) fn set_vec_cap_hint(&self, cap: Option<String>) {
        *self.vec_cap_hint.borrow_mut() = cap;
    }

    pub(crate) fn take_vec_cap_hint(&self) -> Option<String> {
        self.vec_cap_hint.borrow_mut().take()
    }

    /// 相邻语句前瞻：命中「`mut v = []` + 紧随的纯 push 范围循环」时返回容量表达式文本。
    pub(crate) fn push_loop_capacity_hint(&self, stmts: &[Stmt], i: usize) -> Option<String> {
        match (&stmts[i], stmts.get(i + 1)?) {
            (Stmt::Let { name, value, is_mut, .. }, Stmt::For { .. }) if *is_mut => {
                if !Self::is_empty_list_literal(value) {
                    return None;
                }
                if !Self::for_body_is_single_push_of(&stmts[i + 1], name) {
                    return None;
                }
            }
            _ => return None,
        }
        let (start, end) = Self::for_range_bounds(&stmts[i + 1])?;
        let end_s = Self::render_pure_arith(&end)?;
        let cap = match start {
            Some(s) => {
                let start_s = Self::render_pure_arith(&s)?;
                format!("({}).saturating_sub({})", end_s, start_s)
            }
            None => end_s,
        };
        // saturating_sub 对 i64/u32/usize 均存在；`.max(0)` 后 `as usize` 安全
        Some(format!("({}).max(0) as usize", cap))
    }

    /// 空列表字面量（`[]`）。只认「确知是空集合字面量」的形态，避免误判普通变量。
    fn is_empty_list_literal(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::ListLit(v) | ExprKind::List(v) => v.is_empty(),
            _ => false,
        }
    }

    /// For 循环体恰好一条 `v.push(..)`（允许单层 `block:` 包裹）。
    fn for_body_is_single_push_of(for_stmt: &Stmt, var: &str) -> bool {
        let body: &Block = match for_stmt {
            Stmt::For { body, guard, .. } if guard.is_none() => body,
            _ => return false,
        };
        let target = match body.stmts.as_slice() {
            [Stmt::ExprStmt { expr }] => expr,
            [Stmt::Block { stmts }] if stmts.len() == 1 => match &stmts[0] {
                Stmt::ExprStmt { expr } => expr,
                _ => return false,
            },
            _ => return false,
        };
        match &target.kind {
            ExprKind::MethodCall { receiver, method, args } => {
                method == "push"
                    && args.len() == 1
                    && matches!(&receiver.kind, ExprKind::Var(n) if n == var)
            }
            _ => false,
        }
    }

    /// 从 For 语句的迭代源提取 (start?, end)：`Range{..}` 或 `range(a, b)` / `range(a)`。
    fn for_range_bounds(for_stmt: &Stmt) -> Option<(Option<Expr>, Expr)> {
        let iter = match for_stmt {
            Stmt::For { iter, guard, .. } if guard.is_none() => iter,
            _ => return None,
        };
        match &iter.kind {
            ExprKind::Range { start, end, .. } => {
                Some((start.as_ref().map(|b| (**b).clone()), (**end).clone()))
            }
            ExprKind::Call { callee, args, .. } => {
                let is_range = matches!(&callee.kind, ExprKind::Var(n) if n == "range");
                if !is_range || args.is_empty() || args.len() > 2 {
                    return None;
                }
                if args.len() == 2 {
                    Some((Some(args[0].clone()), args[1].clone()))
                } else {
                    Some((None, args[0].clone()))
                }
            }
            _ => return None,
        }
    }

    /// 纯算术表达式 → Rust 文本（只允许字面量/变量/字段/一元负号/四则运算）。
    /// 返回 None 表示表达式含副作用或形态未知，调用方必须放弃容量提示。
    pub(crate) fn render_pure_arith(e: &Expr) -> Option<String> {
        match &e.kind {
            ExprKind::Lit(l) => match l {
                LitKind::Int(v) => Some(format!("{}i64", v)),
                LitKind::Bool(b) => Some(if *b { "1i64" } else { "0i64" }.to_string()),
                _ => None,
            },
            ExprKind::Paren(inner) => Self::render_pure_arith(inner),
            ExprKind::Var(name) => Some(name.clone()),
            ExprKind::FieldAccess { base, field } => {
                Some(format!("{}.{}", Self::render_pure_arith(&**base)?, field))
            }
            ExprKind::UnOp { op, operand } => match op {
                UnOpKind::Neg => Some(format!("-({})", Self::render_pure_arith(&**operand)?)),
                _ => None,
            },
            ExprKind::BinOp { op, lhs, rhs } => {
                let o = match op {
                    BinOpKind::Add => "+",
                    BinOpKind::Sub => "-",
                    BinOpKind::Mul => "*",
                    BinOpKind::Div => "/",
                    BinOpKind::Mod => "%",
                    _ => return None,
                };
                // 子操作数仅当本身是 BinOp 时加括号（保持优先级），
                // 消除 Paren 包装 + BinOp 生成的冗余双层括号：
                // `((n + 1i64))` → `n + 1i64`（最终 `({}).max(0)` 外层已兜底优先级）
                let l = Self::render_pure_arith(&**lhs)?;
                let r = Self::render_pure_arith(&**rhs)?;
                let l = if matches!(lhs.kind, ExprKind::BinOp { .. }) {
                    format!("({})", l)
                } else {
                    l
                };
                let r = if matches!(rhs.kind, ExprKind::BinOp { .. }) {
                    format!("({})", r)
                } else {
                    r
                };
                Some(format!("{} {} {}", l, o, r))
            }
            _ => None,
        }
    }
}
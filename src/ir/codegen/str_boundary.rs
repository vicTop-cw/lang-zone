// Lang-Zone 编译器 — ir/codegen/str_boundary.rs
//! 字符串边界层（契约冻结 2026-09-27）
//!
//! # 为什么需要这一层
//!
//! LZ 只有**一个**字符串类型：`str`。它在 Rust 侧按位置落在四种形态上
//! （拥有值 / `&str` / `&String` / `'static str`），而运行时 crate `lz_builtins`
//! 的 ABI 是固定的一对：**入参 `&str`，出参 `String`**。
//!
//! 历史上每个发射点各自即兴处理形态转换（自己拼 `&` / `.to_string()` /
//! `.as_str()`），于是同一语义在 `src/ir/codegen/mod.rs` 里散出 400+ 处补丁，
//! 彼此不一致（例：`const str` 生成 `&str` 而变量生成 `String`；`StrExt` 调用点
//! 需要 `.to_string()` 又要剥掉误加的 `&`）。本模块把这些判定收敛为**唯一入口**：
//! 先判定表达式当前形态（[`StrForm`]），再按目标位置要求（[`StrWant`]）经
//! [`coerce`] 产出表达式文本。
//!
//! # 契约表（冻结）
//!
//! | 来源形态 \ 目标要求 | `Owned` | `Borrow` | `ExactStrRef` | `Display` |
//! |---------------------|---------|----------|---------------|-----------|
//! | `Owned`（`String`：绑定/字段/调用结果/插值结果） | 原样 | `&x` | `x.as_str()` | 原样 |
//! | `StrRef`（`&str`：`ref str` 形参 / `Ref(Str)` 局部 / 切片） | `x.to_string()` | 原样 | 原样 | 原样 |
//! | `StringRef`（`&String`：`ref self` 字段 / 自动加 `&` 的 String 实参） | `x.clone()` | 原样 | `x.as_str()` | 原样 |
//! | `Static`（`&'static str`：字符串字面量 / `const str`） | `x.to_string()` | 原样 | 原样 | 原样 |
//!
//! 三条铁律：
//!
//! 1. **值域唯一**：LZ `str` ≡ Rust `String`；`&str` 只出现在「借用视图」与
//!    「Rust 侧 ABI」两处；`const str` 显式建模为 [`StrForm::Static`]，
//!    不再作为特例打补丁。
//! 2. **方向唯一**：`&String → &str` 依赖 Rust 的 deref coercion（实参位零成本，
//!    故 `Borrow` 目标对 `StringRef` 原样传递）；`&str → String` 只在 `Owned`
//!    目标处发生（`to_string()` / `clone()`）。禁止反向即兴处理。
//! 3. **职责单一**：E0034 的方法解析强制限定（`<StrExt>::lz_*(self, ..)`）
//!    不属于边界层职责，保持现状；边界层只决定**实参 / 返回值 / 绑定值的形态**，
//!    不改变「调用哪个方法」。
//!
//! # 目标要求怎么选（调用方约定）
//!
//! - `Owned`：LZ 侧拥有值位 —— `String` 形参、`String` 字段、`-> str` 返回值、
//!   `List<str>` 元素、`let` 绑定的值。
//! - `Borrow`：Rust 侧 `&str` 形参（**具体类型**，不是泛型/`Pattern`），
//!   实参位靠 deref coercion 即可。
//! - `ExactStrRef`：泛型/`Pattern` 形参位（`starts_with<P: Pattern>`、
//!   `split<P: Pattern>`、`Into<String>` 之外的泛型透传）与任何**不发生
//!   coercion 的位置**（元组/数组元素、`turbofish` 实参、字面量之外的
//!   `match` 模式绑定等）。注意：对临时值取 `.as_str()` 只在**借用安全的
//!   位置**（实参、同语句内立即使用）合法，调用方需自行保证生命周期。
//! - `Display`：`{}` 插值 / `print` 等 Display 位 —— 四种形态都可直接使用；
//!   本层不做改写，只作为「此处形态无要求」的显式标注（`{}` vs `{:?}` 的
//!   选择不属于本层）。
//!
//! # 与既有启发式的关系（阶段②迁移记录）
//!
//! 已迁移到本层（原处只保留「要不要复制」等 move 语义判定，形态判定一律经
//! [`coerce`]）：返回位 `return self`（3 处）、`clone_if_multiuse` 的 str 分支、
//! `clone` 方法对 str 接收者、列表字面量 str 元素、StrExt 实参的拥有化
//! （取代「以 `"` / `&` 文本前缀判断」的启发式）、字符串比较两侧、Pattern/借用位
//! 的字符串实参。
//!
//! **有意保留**的启发式（附原因，改动前请先读这段）：
//!
//! - `str_typed_vars`：需要在**跨语句状态**上判断（输入还是渲染后的文本，不是 `Expr`）。
//!   并列项 `is_str_expr` 同属此档——它服务 f-string 插值的 `{}`/`{:?}` 分档
//!   （规范 SYNTAX/00-词法基础.md:188：str 插值走 Display 不带引号）。2026-10-02 曾把
//!   插值整块改成一律 Debug 以「对齐」DEMO 文案，代价是打红实跑的
//!   tests/str_boundary.rs c7_fstring，已回退；见台账 memory/bugs.md BUG-22。
//! - `expr_is_clone_of_string`：判断「对 str/String 调用 `.clone()`」的实参，
//!   这类实参 IR 类型常退化为 `Any`（`form_of_expr` 返回 `None`），只能靠
//!   接收者类型 + 方法名兜底。
//! - `is_str_producing`：需要 `fn_returns` 表与方法名白名单的上下文推断。
//! - `need_ref` 的接收者归属判定（std `Vec::contains` vs 自定义 `contains`、
//!   Dict `get`、kwargs Any 兜底）：需要方法解析知识，属「调用哪个方法」，
//!   不在形态层职责内。
//! - 文本层剥离（`trim_end_matches(".to_string()")` 等）：输入是已发射文本，
//!   且与发射管线顺序耦合；迁移前必须先拿到 `Expr`。
//! - 泛型/关联类型位的 clone 抑制（`I::Item` 无 `Clone` 约束）与 E0034 强制
//!   限定 UFCS：分别依赖泛型约束与方法解析，形态层不可见。
//! - `IrType::Str` 的**保守借用假设**：IR 里 `Str` 表示 LZ 的 `str`，但其渲染
//!   文本可能是 `&str`/`&String`（`ref` 形参、`&self` 字段、const 等）。因此凡
//!   「不确定渲染形态」的位置，调用方按 [`StrForm::StrRef`] 传入
//!   （`to_string()` 对三种文本都正确），只有**确认为拥有值绑定**时才用
//!   [`StrForm::Owned`]（原样 / `.clone()`）。

#![allow(dead_code)]

use crate::ir::node::{Expr, ExprKind, LitKind};
use crate::ir::types::IrType;

/// 表达式在 Rust 侧的字符串形态（由 IR 类型 + 表达式种类判定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StrForm {
    /// 拥有值：Rust `String`（let 绑定 / 字段 / 调用结果 / 插值结果）。
    Owned,
    /// 借用视图：Rust `&str`（`ref str` 形参、`Ref(Str)` 局部、切片结果）。
    StrRef,
    /// 借用拥有值：Rust `&String`（`ref self` 字段访问、值语义自动加 `&` 的实参）。
    StringRef,
    /// 静态字面量：Rust `&'static str`（字符串字面量与 `const str`）。
    Static,
}

/// 目标位置对字符串形态的要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StrWant {
    /// 拥有值位：`String` 形参 / `String` 字段 / `-> str` 返回值 / 容器元素。
    Owned,
    /// 可 deref 的借用位：`&str` 形参（具体类型），实参位靠 deref coercion。
    Borrow,
    /// 严格 `&str` 位：泛型 / `Pattern` 形参与任何不发生 coercion 的位置。
    ///
    /// **使用前提（2026-09-27 实测）**：`as_str()` 只存在于 `String`/`&String`，
    /// 在 `&str` 形态的值上不可用（`let s: &str; s.as_str();` → E0658
    /// `use of unstable library feature str_as_str`）。因此本目标只可用于
    /// **确认为拥有值**（[`StrForm::Owned`]）的位点；形态可能是借用视图时请用
    /// [`StrWant::Borrow`]（`&x`，`&String: Pattern` 与 deref coercion 都成立）。
    ExactStrRef,
    /// 格式化插值位（`{}` / `print`）：四种形态都可直接使用。
    Display,
}

/// 唯一的形态归一化入口：把表达式文本 `s` 由 `from` 形态转为 `want` 形态。
///
/// 纯函数、无状态：便于单元测试与在所有发射点复用。
pub(super) fn coerce(s: &str, from: StrForm, want: StrWant) -> String {
    use StrForm as F;
    use StrWant as W;
    match (from, want) {
        // Display 位：String / &str / &String / &'static str 都实现了 Display
        (_, W::Display) => s.to_string(),
        // 拥有值位：只有已是拥有值才原样；借用（含字面量）需拥有化
        (F::Owned, W::Owned) => s.to_string(),
        (F::StrRef, W::Owned) | (F::Static, W::Owned) => format!("{}.to_string()", s),
        // &String → String：auto-deref 的 Clone（`s.clone()` 得到 String）
        (F::StringRef, W::Owned) => format!("{}.clone()", s),
        // 可 deref 的借用位：Owned 加 `&`（实参位零成本 coercion），其余原样
        (F::Owned, W::Borrow) => format!("&{}", s),
        (_, W::Borrow) => s.to_string(),
        // 严格 &str 位：Owned / &String 取 as_str()，&str 与字面量原样
        (F::Owned, W::ExactStrRef) | (F::StringRef, W::ExactStrRef) => {
            format!("{}.as_str()", s)
        }
        (_, W::ExactStrRef) => s.to_string(),
    }
}

/// 值语义复制：给出字符串值的**拥有化副本**表达式。
///
/// 与「要不要复制」解耦：是否复制属 move 语义（多使用者 `fn_use_count`、
/// 索引取出、容器元素绑定），由调用方判定；本函数只回答「复制成什么形态」。
///
/// 字符串一律按**保守借用假设**（[`StrForm::StrRef`] → [`StrWant::Owned`]）
/// 产出 `x.to_string()`：IR `Str` 的渲染文本可能是 `String` / `&str` / `&String`
/// 三者之一（`ref` 形参、`&self` 字段、const），而 `.clone()` 只对确认为
/// `String` 的文本才得到拥有值（`&str.clone()` 仍是 `&str`，E0308）。
pub(super) fn owned_copy(s: &str) -> String {
    coerce(s, StrForm::StrRef, StrWant::Owned)
}

/// 判断 IR 类型是否为 LZ 字符串（含 `String` 与 `&str` 两种写成）。
pub(super) fn is_str_ir(ty: &IrType) -> bool {
    matches!(ty, IrType::Str)
        || matches!(ty, IrType::Named { path, .. }
            if path == "str" || path == "String" || path == "&str" || path == "&String")
}

/// 从 IR 类型判定字符串形态；非字符串类型返回 `None`。
///
/// 注意：字面量（`"x"`）虽然是 `IrType::Str`，其 Rust 形态是 [`StrForm::Static`]，
/// 必须用 [`form_of_expr`] 判定（本函数只看类型、看不到字面量）。
pub(super) fn form_of_ir(ty: &IrType) -> Option<StrForm> {
    match ty {
        IrType::Str => Some(StrForm::Owned),
        IrType::Named { path, .. } => match path.as_str() {
            "String" | "&String" => Some(StrForm::Owned),
            "str" | "&str" => Some(StrForm::StrRef),
            _ => None,
        },
        IrType::Ref(inner) | IrType::MutRef(inner) => match &**inner {
            IrType::Str => Some(StrForm::StrRef),
            IrType::Named { path, .. } => match path.as_str() {
                "String" => Some(StrForm::StringRef),
                "str" | "&str" => Some(StrForm::StrRef),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// 从字面量种类判定字符串形态（`"x"` → Static，f-string → Owned）。
pub(super) fn form_of_lit(lit: &LitKind) -> Option<StrForm> {
    match lit {
        LitKind::Str(_) => Some(StrForm::Static),
        LitKind::FStr(_) => Some(StrForm::Owned),
        _ => None,
    }
}

/// 从表达式判定字符串形态：字面量优先（`Static`），其余按 IR 类型。
///
/// 只做**无状态**判定；依赖 `str_typed_vars` 之类的上下文启发式不属于本层
/// （调用方若已知额外信息，应先自行判定形态再调用 [`coerce`]）。
pub(super) fn form_of_expr(expr: &Expr) -> Option<StrForm> {
    if let ExprKind::Lit(lit) = &expr.kind {
        if let Some(f) = form_of_lit(lit) {
            return Some(f);
        }
    }
    form_of_ir(&expr.ty)
}

#[cfg(test)]
mod tests {
    use super::StrForm as F;
    use super::StrWant as W;
    use super::*;

    /// 契约表 4×4 全格断言（形态归一化的唯一事实来源）
    #[test]
    fn contract_matrix_is_frozen() {
        // 行：来源形态；列：目标要求
        let cases: &[(StrForm, StrWant, &str)] = &[
            // Owned（String）
            (F::Owned, W::Owned, "s"),
            (F::Owned, W::Borrow, "&s"),
            (F::Owned, W::ExactStrRef, "s.as_str()"),
            (F::Owned, W::Display, "s"),
            // StrRef（&str）
            (F::StrRef, W::Owned, "s.to_string()"),
            (F::StrRef, W::Borrow, "s"),
            (F::StrRef, W::ExactStrRef, "s"),
            (F::StrRef, W::Display, "s"),
            // StringRef（&String）
            (F::StringRef, W::Owned, "s.clone()"),
            (F::StringRef, W::Borrow, "s"),
            (F::StringRef, W::ExactStrRef, "s.as_str()"),
            (F::StringRef, W::Display, "s"),
            // Static（&'static str）
            (F::Static, W::Owned, "s.to_string()"),
            (F::Static, W::Borrow, "s"),
            (F::Static, W::ExactStrRef, "s"),
            (F::Static, W::Display, "s"),
        ];
        for (from, want, expect) in cases {
            assert_eq!(
                coerce("s", *from, *want),
                *expect,
                "契约表不符：from={:?} want={:?}",
                from,
                want
            );
        }
    }

    #[test]
    fn coerce_preserves_complex_expressions() {
        // 复杂表达式不再加括号包装：调用方负责传入已带括号的文本
        assert_eq!(coerce("(a + b)", F::Owned, W::Borrow), "&(a + b)");
        assert_eq!(
            coerce("x.clone()", F::StringRef, W::Owned),
            "x.clone().clone()"
        );
    }

    #[test]
    fn form_of_ir_classifies_all_string_shapes() {
        assert_eq!(form_of_ir(&IrType::Str), Some(F::Owned));
        assert_eq!(
            form_of_ir(&IrType::Named {
                path: "String".into(),
                args: vec![]
            }),
            Some(F::Owned)
        );
        assert_eq!(
            form_of_ir(&IrType::Named {
                path: "str".into(),
                args: vec![]
            }),
            Some(F::StrRef)
        );
        // `ref str` 形参 / Ref(Str) 局部 → &str
        assert_eq!(form_of_ir(&IrType::Ref(Box::new(IrType::Str))), Some(F::StrRef));
        assert_eq!(
            form_of_ir(&IrType::Ref(Box::new(IrType::Named {
                path: "String".into(),
                args: vec![]
            }))),
            Some(F::StringRef)
        );
        // `mut ref str` 同族
        assert_eq!(
            form_of_ir(&IrType::MutRef(Box::new(IrType::Str))),
            Some(F::StrRef)
        );
        // 非字符串
        assert_eq!(form_of_ir(&IrType::Int), None);
        assert_eq!(form_of_ir(&IrType::Any), None);
        assert_eq!(
            form_of_ir(&IrType::Named {
                path: "List".into(),
                args: vec![IrType::Str]
            }),
            None
        );
    }

    /// 构造测试用表达式（span 用占位值：形态判定与位置无关）
    fn expr_of(kind: ExprKind, ty: IrType) -> Expr {
        Expr {
            kind,
            ty,
            span: crate::ir::node::Span {
                start: 0,
                end: 0,
                line: 0,
                col: 0,
                file: None,
            },
        }
    }

    #[test]
    fn form_of_expr_prefers_literal_form() {
        let lit = expr_of(ExprKind::Lit(LitKind::Str("hi".into())), IrType::Str);
        // 字面量是 &'static str（不是拥有值），必须判成 Static
        assert_eq!(form_of_expr(&lit), Some(F::Static));
        assert_eq!(coerce("\"hi\"", F::Static, W::Owned), "\"hi\".to_string()");
        assert_eq!(coerce("\"hi\"", F::Static, W::Borrow), "\"hi\"");
        // 非字符串字面量不参与判定
        let n = expr_of(ExprKind::Lit(LitKind::Int(1)), IrType::Int);
        assert_eq!(form_of_expr(&n), None);
    }

    /// 建议 3：值语义复制入口只表达形态（拥有化），不表达「要不要复制」
    #[test]
    fn owned_copy_is_form_only() {
        assert_eq!(owned_copy("x"), "x.to_string()");
        assert_eq!(owned_copy("(a + b)"), "(a + b).to_string()");
        // 与 coerce 的保守借用假设一致：对 String/&str/&String 三种文本都正确
        assert_eq!(owned_copy("x"), coerce("x", StrForm::StrRef, StrWant::Owned));
    }

    #[test]
    fn is_str_ir_covers_both_spellings() {
        assert!(is_str_ir(&IrType::Str));
        assert!(is_str_ir(&IrType::Named {
            path: "String".into(),
            args: vec![]
        }));
        assert!(is_str_ir(&IrType::Named {
            path: "str".into(),
            args: vec![]
        }));
        assert!(!is_str_ir(&IrType::Bool));
    }
}

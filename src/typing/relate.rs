//! 子类型 / 包含关系判定 —— `conforms`
//!
//! 基于类型格（lattice）：
//!
//! ```text
//!            Any  (顶：任何类型都 <: Any)
//!             │
//!   Int / Str / Bool / Named(...) / 容器 / Fn / Ref ...
//!             │
//!          Never (底：Never <: 任何类型)
//! ```
//!
//! 规则要点：
//! - **Any 顶**：`sub <: Any` 对任何 `sub` 成立。
//! - **Never 底**：`Never <: sup` 对任何 `sup` 成立。
//! - **泛型协变**：`F<A> <: F<B>` 当 `A <: B`（默认协变；不变位置由 variance 模块单独判定）。
//! - **函数子类型**：`(P1)->R1 <: (P2)->R2` 当 `P2 <: P1`（参数逆变）且 `R1 <: R2`（返回协变）。
//! - **引用变型**：`&T` 协变；`&mut T` **不变**（mutable 别名要求严格相等）。
//! - 推断孔（`Type::Var`）先经 `InferCtx::prune` 消解；若仍自由则报 `UnresolvedVar`。

use crate::hints::InferCtx;
use crate::types::def::Type;

use super::errors::TypingError;

/// 将类型简写归一化为规范形式（Generic 表示），消除 Option/Optional/Result 与 Generic 的不匹配
///
/// - `Option(T)` → `Generic { base: Named("Option"), args: [T] }`
/// - `Optional(T)` → `Generic { base: Named("Option"), args: [T] }`（T? 语法糖）
/// - `Result { ok, err }` → `Generic { base: Named("Result"), args: [ok, err] }`
/// - 其他类型保持原样
fn to_canonical(t: &Type) -> Type {
    match t {
        Type::Option(inner) => Type::Generic {
            base: Box::new(Type::Named("Option".into())),
            args: vec![*inner.clone()],
        },
        Type::Optional(inner) => Type::Generic {
            base: Box::new(Type::Named("Option".into())),
            args: vec![*inner.clone()],
        },
        Type::Result { ok, err } => Type::Generic {
            base: Box::new(Type::Named("Result".into())),
            args: vec![*ok.clone(), *err.clone()],
        },
        other => other.clone(),
    }
}

/// 判断 `sub` 能否用于期望 `sup` 的位置（即 `sub <: sup`）。
///
/// `ctx` 用于消解 `sub`/`sup` 中可能残留的推断变量（应在 zonk 之后调用，
/// 此时通常无变量；若仍有自由变量则视为判定失败）。
pub fn conforms(ctx: &InferCtx, sub: &Type, sup: &Type) -> Result<(), TypingError> {
    let sub = ctx.prune(sub);
    let sup = ctx.prune(sup);

    // 归一化：将 Option/Optional/Result 简写转为 Generic 规范形式
    let sub = to_canonical(&sub);
    let sup = to_canonical(&sup);

    // 推断孔未消解 → 无法决定（应在进入 typing 前先 zonk）
    if matches!(sub, Type::Var(_)) || matches!(sup, Type::Var(_)) {
        let hole = match &sub {
            Type::Var(_) => sub.clone(),
            _ => sup.clone(),
        };
        return Err(TypingError::UnresolvedVar(hole));
    }

    // 顶 / 底
    if matches!(sup, Type::Any) {
        return Ok(());
    }
    if matches!(sub, Type::Never) {
        return Ok(());
    }

    // 自反（结构相等）
    if sub == sup {
        return Ok(());
    }

    match (&sub, &sup) {
        // ── 名义类型：仅同名相等 ──
        (Type::Named(a), Type::Named(b)) => {
            if a == b { Ok(()) } else { Err(TypingError::Conformance(sub, sup)) }
        }

        // ── 泛型（含归一化后的 Option/Optional/Result）：base 相等 + 实参协变 ──
        (Type::Generic { base: sb, args: sa }, Type::Generic { base: ub, args: ua }) => {
            conforms(ctx, sb, ub)?;
            if sa.len() != ua.len() {
                return Err(TypingError::Arity(sa.len(), ua.len()));
            }
            for (s, u) in sa.iter().zip(ua.iter()) {
                conforms(ctx, s, u)?; // 协变
            }
            Ok(())
        }

        // ── HKT: Apply 与 Generic 同构 ──
        (Type::Apply { constructor: sc, args: sa }, Type::Apply { constructor: uc, args: ua }) => {
            conforms(ctx, sc, uc)?;
            if sa.len() != ua.len() {
                return Err(TypingError::Arity(sa.len(), ua.len()));
            }
            for (s, u) in sa.iter().zip(ua.iter()) {
                conforms(ctx, s, u)?;
            }
            Ok(())
        }
        (Type::Apply { constructor, args: sa }, Type::Generic { base, args: ua })
        | (Type::Generic { base, args: sa }, Type::Apply { constructor, args: ua }) => {
            conforms(ctx, constructor, base)?;
            if sa.len() != ua.len() {
                return Err(TypingError::Arity(sa.len(), ua.len()));
            }
            for (s, u) in sa.iter().zip(ua.iter()) {
                conforms(ctx, s, u)?;
            }
            Ok(())
        }

        // ── 元组：等长 + 逐元素协变 ──
        (Type::Tuple(s), Type::Tuple(u)) => {
            if s.len() != u.len() {
                return Err(TypingError::Arity(s.len(), u.len()));
            }
            s.iter().zip(u.iter()).try_for_each(|(a, b)| conforms(ctx, a, b))
        }

        // ── 引用变型 ──
        (Type::Ref(s), Type::Ref(u)) => conforms(ctx, s, u),       // &T 协变
        (Type::MutRef(s), Type::MutRef(u)) => {                     // &mut T 不变
            if s == u { Ok(()) } else { Err(TypingError::Conformance(sub, sup)) }
        }

        // ── 函数子类型：参数逆变、返回协变 ──
        (Type::Fn { params: sp, ret: sr }, Type::Fn { params: up, ret: ur }) => {
            if sp.len() != up.len() {
                return Err(TypingError::Arity(sp.len(), up.len()));
            }
            // 参数逆变：要求的参数类型必须 <: 提供的参数类型
            for (required_p, provided_p) in up.iter().zip(sp.iter()) {
                conforms(ctx, required_p, provided_p)?;
            }
            // 返回协变：提供的返回类型必须 <: 要求的返回类型
            conforms(ctx, sr, ur)
        }

        // ── SIMD：宽度必须相等，元素协变 ──
        (Type::Simd { elem: se, width: sw }, Type::Simd { elem: ue, width: uw }) => {
            if sw != uw {
                return Err(TypingError::Conformance(sub, sup));
            }
            conforms(ctx, se, ue)
        }

        (Type::Self_, Type::Self_) => Ok(()),

        // ── 交集类型 ──
        (Type::Intersection(members), _) => {
            for m in members {
                conforms(ctx, m, &sup)?;
            }
            Ok(())
        }
        (_, Type::Intersection(members)) => {
            for m in members {
                conforms(ctx, &sub, m)?;
            }
            Ok(())
        }

        _ => Err(TypingError::Conformance(sub, sup)),
    }
}

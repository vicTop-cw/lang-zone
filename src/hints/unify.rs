//! Robinson 一阶统一算法
//!
//! 统一两类型；失败时返回 [`TypeError`]。核心安全机制是 **occurs-check**，
//! 它从算法层面拒绝 `α = [α]` 这类自引用绑定，杜绝递归类型的无限展开——
//! 这正是调研报告中强调的“递归推断终止条件”第一保险。

use crate::types::def::Type;
use crate::hints::tyvar::{InferCtx, TypeError};

/// 将类型简写归一化为规范形式，使 Option/Optional/Result 能参与 HKT 统一。
///
/// - `Option(T)` → `Generic { base: Named("Option"), args: [T] }`
/// - `Optional(T)` → `Generic { base: Named("Option"), args: [T] }`（T? 语法糖）
/// - `Result { ok, err }` → `Generic { base: Named("Result"), args: [ok, err] }`
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

/// 统一 a 与 b。推断变量在统一过程中被绑定，具体类型按结构递归。
pub fn unify(ctx: &mut InferCtx, a: &Type, b: &Type) -> Result<(), TypeError> {
    // 取得所有权形式后按结构匹配；递归统一内部类型时统一以 & 传参
    let a = ctx.prune(a);
    let b = ctx.prune(b);

    // 归一化：让 Option/Optional/Result 与 Generic/Apply 使用同一套统一规则
    let a = to_canonical(&a);
    let b = to_canonical(&b);

    match (a, b) {
        // ── 变量-变量（同一变量）──
        (Type::Var(v1), Type::Var(v2)) if v1 == v2 => Ok(()),

        // ── 一侧为变量：绑定（先 occurs-check）──
        (Type::Var(v), t) | (t, Type::Var(v)) => {
            if ctx.occurs(v, &t) {
                return Err(TypeError::Occurs(v, t.clone()));
            }
            let rv = ctx.find(v);
            // 若 v 已被绑定到具体类型，则将该“绑定内容”与 t 统一，而非盲目覆盖。
            // 否则 a = Int, b = a 之后再求 b = Bool 会被静默接受，破坏类型安全。
            match ctx.resolve(rv) {
                Some(bound) => unify(ctx, &bound, &t),
                None => {
                    ctx.bind(v, t);
                    Ok(())
                }
            }
        }

        // ── 基本类型 ──
        (Type::Int, Type::Int) => Ok(()),
        (Type::F64, Type::F64)
        | (Type::F64, Type::Float)
        | (Type::Float, Type::F64) => Ok(()),
        (Type::Float, Type::Float) => Ok(()),
        (Type::Str, Type::Str) => Ok(()),
        (Type::Bool, Type::Bool) => Ok(()),
        (Type::Unit, Type::Unit) => Ok(()),
        (Type::None_, Type::None_) => Ok(()),
        (Type::Self_, Type::Self_) => Ok(()),

        // ── Never 为底部类型，与一切协变统一 ──
        (Type::Never, _) | (_, Type::Never) => Ok(()),
        // ── Any 与一切统一（动态兜底）──
        (Type::Any, _) | (_, Type::Any) => Ok(()),

        // ── 命名类型（同名校验）──
        (Type::Named(n1), Type::Named(n2)) if n1 == n2 => Ok(()),

        // ── 容器 / 引用（递归统一内部类型）──
        (Type::Option(i1), Type::Option(i2))
        | (Type::Optional(i1), Type::Optional(i2))
        | (Type::Option(i1), Type::Optional(i2))
        | (Type::Optional(i1), Type::Option(i2)) =>
            unify(ctx, &i1, &i2),
        // None 与 Option/Optional 兼容（None 可赋值给任何 Option<T>）
        (Type::None_, Type::Option(_)) | (Type::Option(_), Type::None_) => Ok(()),
        (Type::None_, Type::Optional(_)) | (Type::Optional(_), Type::None_) => Ok(()),

        (Type::Ref(i1), Type::Ref(i2)) => unify(ctx, &i1, &i2),
        (Type::MutRef(i1), Type::MutRef(i2)) => unify(ctx, &i1, &i2),

        (Type::Result { ok: o1, err: e1 }, Type::Result { ok: o2, err: e2 }) =>
            unify(ctx, &o1, &o2).and_then(|_| unify(ctx, &e1, &e2)),

        (Type::Generic { base: b1, args: a1 }, Type::Generic { base: b2, args: a2 }) => {
            // 构造器（List / Dict / Set …）必须对齐
            unify(ctx, &b1, &b2)?;
            if a1.len() != a2.len() {
                return Err(TypeError::Arity(a1.len(), a2.len()));
            }
            for (x, y) in a1.iter().zip(a2.iter()) {
                unify(ctx, x, y)?;
            }
            Ok(())
        }

        // HKT: Apply 与 Generic 语义等价，交叉统一
        (Type::Apply { constructor: c1, args: a1 }, Type::Apply { constructor: c2, args: a2 }) => {
            unify(ctx, &*c1, &*c2)?;
            if a1.len() != a2.len() {
                return Err(TypeError::Arity(a1.len(), a2.len()));
            }
            for (x, y) in a1.iter().zip(a2.iter()) {
                unify(ctx, x, y)?;
            }
            Ok(())
        }
        (Type::Apply { constructor, args }, Type::Generic { base, args: args2 })
        | (Type::Generic { base, args: args2 }, Type::Apply { constructor, args }) => {
            unify(ctx, &*constructor, &*base)?;
            if args.len() != args2.len() {
                return Err(TypeError::Arity(args.len(), args2.len()));
            }
            for (x, y) in args.iter().zip(args2.iter()) {
                unify(ctx, x, y)?;
            }
            Ok(())
        }

        // 类型构造器：同名同 arity 即统一
        (Type::Constructor { name: n1, arity: a1 }, Type::Constructor { name: n2, arity: a2 }) if n1 == n2 && a1 == a2 => Ok(()),

        (Type::Tuple(e1), Type::Tuple(e2)) => {
            if e1.len() != e2.len() {
                return Err(TypeError::Arity(e1.len(), e2.len()));
            }
            for (x, y) in e1.iter().zip(e2.iter()) {
                unify(ctx, x, y)?;
            }
            Ok(())
        }

        (Type::Fn { params: p1, ret: r1 }, Type::Fn { params: p2, ret: r2 }) => {
            if p1.len() != p2.len() {
                return Err(TypeError::Arity(p1.len(), p2.len()));
            }
            for (x, y) in p1.iter().zip(p2.iter()) {
                unify(ctx, x, y)?;
            }
            unify(ctx, &r1, &r2)
        }

        (Type::Simd { elem: e1, width: w1 }, Type::Simd { elem: e2, width: w2 }) => {
            if w1 != w2 {
                return Err(TypeError::Mismatch(*e1, *e2));
            }
            unify(ctx, &e1, &e2)
        }

        // ── 交集类型：所有成员必须与对方统一 ──
        (Type::Intersection(members), other) => {
            for m in &members {
                unify(ctx, m, &other)?;
            }
            Ok(())
        }
        (other, Type::Intersection(members)) => {
            for m in &members {
                unify(ctx, &other, m)?;
            }
            Ok(())
        }

        // ── 联合类型：任一成员能与对方统一则成功 ──
        (Type::Union(members), other) => {
            for m in &members {
                if unify(ctx, m, &other).is_ok() {
                    return Ok(());
                }
            }
            Err(TypeError::Mismatch(Type::Union(members), other))
        }
        (other, Type::Union(members)) => {
            for m in &members {
                if unify(ctx, &other, m).is_ok() {
                    return Ok(());
                }
            }
            Err(TypeError::Mismatch(other, Type::Union(members)))
        }

        // ── 路径依赖类型 ──
        (Type::PathDependent { path: p1, member: m1 }, Type::PathDependent { path: p2, member: m2 }) if p1 == p2 && m1 == m2 => Ok(()),
        // Any/Never 已在前面统一处理

        // ── 存在类型占位 _ ──
        (Type::Wildcard, t) | (t, Type::Wildcard) => {
            let v = ctx.fresh(0);
            ctx.bind(v, t);
            Ok(())
        }

        other => Err(TypeError::Mismatch(other.0, other.1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_dependent_unify_same_path_member() {
        let mut ctx = InferCtx::new();
        let a = Type::PathDependent { path: "x".into(), member: "T".into() };
        let b = Type::PathDependent { path: "x".into(), member: "T".into() };
        assert!(unify(&mut ctx, &a, &b).is_ok());
    }

    #[test]
    fn path_dependent_unify_different_path_fails() {
        let mut ctx = InferCtx::new();
        let a = Type::PathDependent { path: "x".into(), member: "T".into() };
        let b = Type::PathDependent { path: "y".into(), member: "T".into() };
        assert!(matches!(unify(&mut ctx, &a, &b), Err(TypeError::Mismatch(_, _))));
    }

    #[test]
    fn wildcard_unifies_with_int() {
        let mut ctx = InferCtx::new();
        assert!(unify(&mut ctx, &Type::Wildcard, &Type::Int).is_ok());
    }

    #[test]
    fn wildcard_in_generic() {
        let mut ctx = InferCtx::new();
        let a = Type::Generic {
            base: Box::new(Type::Named("List".into())),
            args: vec![Type::Wildcard],
        };
        let b = Type::Generic {
            base: Box::new(Type::Named("List".into())),
            args: vec![Type::Int],
        };
        assert!(unify(&mut ctx, &a, &b).is_ok());
    }
}

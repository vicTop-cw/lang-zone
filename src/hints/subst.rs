//! 类型替换（zonk）
//!
//! 将类型中所有推断变量**完全解析**为具体类型：递归展开各变量的绑定，
//! 直到遇到非变量类型或残留的未绑定变量（后者应已被推断阶段捕获为错误）。
//!
//! 命名取自 GHC 的 “zonking” 步骤 —— 约束求解后把统一变量替换为最终类型。

use crate::types::def::Type;
use crate::hints::tyvar::InferCtx;

/// zonk：完全解析类型 t 中的所有变量
pub fn zonk(ctx: &InferCtx, t: &Type) -> Type {
    match t {
        Type::Var(v) => match ctx.resolve(*v) {
            Some(bound) => zonk(ctx, &bound),
            None => Type::Int,  // 乐观推断：未解析变量默认 int 而非 unit
        },
        Type::Generic { base, args } => {
            let args = args.iter().map(|a| zonk(ctx, a)).collect();
            Type::Generic { base: base.clone(), args }
        }
        Type::Option(inner) => Type::Option(Box::new(zonk(ctx, inner))),
        Type::Optional(inner) => Type::Optional(Box::new(zonk(ctx, inner))),
        Type::Ref(inner) => Type::Ref(Box::new(zonk(ctx, inner))),
        Type::MutRef(inner) => Type::MutRef(Box::new(zonk(ctx, inner))),
        Type::Result { ok, err } => Type::Result {
            ok: Box::new(zonk(ctx, ok)),
            err: Box::new(zonk(ctx, err)),
        },
        Type::Tuple(es) => Type::Tuple(es.iter().map(|e| zonk(ctx, e)).collect()),
        Type::Fn { params, ret } => Type::Fn {
            params: params.iter().map(|p| zonk(ctx, p)).collect(),
            ret: Box::new(zonk(ctx, ret)),
        },
        Type::Simd { elem, width } => Type::Simd {
            elem: Box::new(zonk(ctx, elem)),
            width: *width,
        },
        Type::Intersection(ts) => Type::Intersection(ts.iter().map(|t| zonk(ctx, t)).collect()),
        Type::Constructor { name, arity } => Type::Constructor { name: name.clone(), arity: *arity },
        Type::Apply { constructor, args } => Type::Apply {
            constructor: Box::new(zonk(ctx, constructor)),
            args: args.iter().map(|a| zonk(ctx, a)).collect(),
        },
        other => other.clone(),
    }
}

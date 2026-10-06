// Lang-Zone 编译器 — ir/builder/comptime_conv.rs
// （由 builder.rs move-only 拆出，逻辑零改动）

use super::*;

/// 将编译期求值结果转为 IR 表达式（Int/Float/Bool/Str/None → 字面量；
/// List/Tuple → vec![...]/元组递归内联，供查找表「焊死」；
/// Map/Type/Inspect 不支持内联，返回 None）
///
/// `ty` 是该值的**声明类型**（来自 `ctx.top_level_consts`），用于把元素类型
/// 递归带进内联产物。传 None 时子表达式退回 `IrType::Any`，与改前行为一致。
/// BUG-5：嵌套空列表在 codegen 侧按 `IrType::Any` 发成 `()`（见 `ListLit` 的
/// `is_nil` 判据），所以引用点的声明类型必须下推，否则 `let a: List<List<i64>>
/// = [[], [[]]]` 的静态定义正确、`println(a)` 的内联体却是 `vec![(), vec![()]]`。
pub(crate) fn comptime_value_to_lit(
    v: &crate::comptime::ComptimeValue,
    ty: Option<&IrType>,
) -> Option<ExprKind> {
    use crate::comptime::ComptimeValue;
    match v {
        ComptimeValue::Int(i) => Some(ExprKind::Lit(LitKind::Int(*i))),
        ComptimeValue::Float(f) => Some(ExprKind::Lit(LitKind::F64(*f))),
        ComptimeValue::Bool(b) => Some(ExprKind::Lit(LitKind::Bool(*b))),
        ComptimeValue::Str(s) => Some(ExprKind::Lit(LitKind::Str(s.clone()))),
        ComptimeValue::None => Some(ExprKind::Lit(LitKind::None_)),
        ComptimeValue::List(xs) => Some(ExprKind::ListLit(comptime_inline_elems(xs, ty))),
        ComptimeValue::Tuple(xs) => Some(ExprKind::TupleLit(comptime_inline_elems(xs, ty))),
        _ => None,
    }
}

/// 内联容器第 `i` 个子元素的声明类型：List/Vec 取唯一泛型实参，Tuple 按下标取。
/// 其它形状（Any / 未标注 / 形状不符）返回 None，子元素退回 `IrType::Any`。
pub(crate) fn comptime_child_ty(ty: Option<&IrType>, i: usize) -> Option<IrType> {
    match ty? {
        IrType::Named { path, args } if (path == "List" || path == "Vec") && args.len() == 1 => {
            Some(args[0].clone())
        }
        IrType::Tuple(elems) => elems.get(i).cloned(),
        _ => None,
    }
}

pub(crate) fn comptime_inline_elems(
    xs: &[crate::comptime::ComptimeValue],
    ty: Option<&IrType>,
) -> Vec<Expr> {
    xs.iter()
        .enumerate()
        .map(|(i, x)| {
            let child = comptime_child_ty(ty, i);
            let kind =
                comptime_value_to_lit(x, child.as_ref()).unwrap_or(ExprKind::Lit(LitKind::None_));
            Expr::new(kind, child.unwrap_or(IrType::Any), Span::unknown())
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════
// M3 单态化特化引擎（AST 级）
//
// 调用点实参编译期可知时，克隆被调函数、将 comptime 形参引用替换为常量、
// mangled 命名、改写自递归调用，生成「焊死该值」的特化副本，附加到
// pending_items 由 build_ir 统一发射。复用现有 AST→IR→codegen 链路。
// ═══════════════════════════════════════════════════════════════════

/// FNV-1a 64 位哈希：将 (函数名, comptime 值组合) 折叠为稳定的 mangled 后缀，
/// 保证不同值组合互不碰撞、且产物为合法 Rust 标识符。
pub(crate) fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 将编译期值内联为 AST 字面量表达式（`ast::Expr`）。不可内联的
/// Type / Map / Inspect 返回 None —— 此时 M3 退化为仅 M2 校验，不特化。
pub(crate) fn comptime_value_to_ast(v: &crate::comptime::ComptimeValue) -> Option<AstExpr> {
    use crate::comptime::ComptimeValue;
    match v {
        ComptimeValue::Int(i) => Some(AstExpr::IntLit(*i)),
        ComptimeValue::Float(f) => Some(AstExpr::FloatLit(*f)),
        ComptimeValue::Bool(b) => Some(AstExpr::BoolLit(*b)),
        ComptimeValue::Str(s) => Some(AstExpr::StrLit(s.clone())),
        ComptimeValue::None => Some(AstExpr::NoneLit),
        ComptimeValue::List(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(comptime_value_to_ast(x)?);
            }
            Some(AstExpr::ListLit(out))
        }
        ComptimeValue::Tuple(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(comptime_value_to_ast(x)?);
            }
            Some(AstExpr::TupleLit(out))
        }
        _ => None,
    }
}

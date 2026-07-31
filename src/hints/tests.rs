//! hints 库单元测试
//!
//! 覆盖 P0 基石：推断变量分配、绑定解析、函数/容器类型统一、
//! occurs-check（防无限展开）、约束求解、zonk 完全替换。

use crate::types::def::Type;
use crate::hints::{InferCtx, unify, solve, zonk, Constraint, TypeError};

fn list_of(inner: Type) -> Type {
    Type::Generic { base: Box::new(Type::Named("List".into())), args: vec![inner] }
}

#[test]
fn fresh_and_find_are_distinct() {
    let mut ctx = InferCtx::new();
    let a = ctx.fresh(0);
    let b = ctx.fresh(0);
    assert_ne!(a, b);
    assert_eq!(ctx.find(a), a);
    assert_eq!(ctx.find(b), b);
}

#[test]
fn bind_then_resolve() {
    let mut ctx = InferCtx::new();
    let a = ctx.fresh_ty(0);
    unify(&mut ctx, &a, &Type::Int).unwrap();
    let resolved = zonk(&ctx, &a);
    assert_eq!(resolved, Type::Int);
}

#[test]
fn transitivity_through_three_vars() {
    let mut ctx = InferCtx::new();
    let a = ctx.fresh_ty(0);
    let b = ctx.fresh_ty(0);
    let c = ctx.fresh_ty(0);
    unify(&mut ctx, &a, &Type::Int).unwrap();
    unify(&mut ctx, &b, &a).unwrap();
    unify(&mut ctx, &c, &b).unwrap();
    assert_eq!(zonk(&ctx, &c), Type::Int);
}

#[test]
fn unify_concrete_equal() {
    let mut ctx = InferCtx::new();
    unify(&mut ctx, &Type::Int, &Type::Int).unwrap();
    unify(&mut ctx, &Type::F64, &Type::F64).unwrap();
    unify(&mut ctx, &Type::Str, &Type::Str).unwrap();
    // float 与 f64 等价
    unify(&mut ctx, &Type::F64, &Type::Float).unwrap();
}

#[test]
fn unify_function_types() {
    let mut ctx = InferCtx::new();
    let fa = Type::Fn { params: vec![Type::Int], ret: Box::new(Type::Bool) };
    let fb = Type::Fn { params: vec![Type::Int], ret: Box::new(Type::Bool) };
    unify(&mut ctx, &fa, &fb).unwrap();

    // 返回类型不同 → 失败
    let fc = Type::Fn { params: vec![Type::Int], ret: Box::new(Type::Int) };
    let err = unify(&mut ctx, &fa, &fc);
    assert!(matches!(err, Err(TypeError::Mismatch(_, _))));

    // 参数个数不同 → 元数错误
    let fd = Type::Fn { params: vec![Type::Int, Type::Int], ret: Box::new(Type::Bool) };
    let err2 = unify(&mut ctx, &fa, &fd);
    assert!(matches!(err2, Err(TypeError::Arity(1, 2))));
}

#[test]
fn unify_generic_container() {
    let mut ctx = InferCtx::new();
    // List<int> == List<int>
    unify(&mut ctx, &list_of(Type::Int), &list_of(Type::Int)).unwrap();
    // List<int> != List<bool>
    let err = unify(&mut ctx, &list_of(Type::Int), &list_of(Type::Bool));
    assert!(matches!(err, Err(TypeError::Mismatch(_, _))));
}

#[test]
fn occurs_check_rejects_infinite_type() {
    let mut ctx = InferCtx::new();
    let a = ctx.fresh(0);
    // α = List<α> 必须被 occurs-check 拒绝
    let list_a = list_of(Type::Var(a));
    let err = unify(&mut ctx, &Type::Var(a), &list_a);
    assert!(matches!(err, Err(TypeError::Occurs(_, _))),
        "occurs-check 应拒绝 α = List<α> 这类无限类型展开");
}

#[test]
fn occurs_check_rejects_nested_self_reference() {
    let mut ctx = InferCtx::new();
    let a = ctx.fresh(0);
    // α = (int, List<α>) 同样自引用
    let tup = Type::Tuple(vec![Type::Int, list_of(Type::Var(a))]);
    let err = unify(&mut ctx, &Type::Var(a), &tup);
    assert!(matches!(err, Err(TypeError::Occurs(_, _))));
}

#[test]
fn unify_var_chain_resolves_through_list() {
    let mut ctx = InferCtx::new();
    let a = ctx.fresh_ty(0);
    // b = List<a>，a = int  → zonk(b) == List<int>
    let b = list_of(a.clone());
    unify(&mut ctx, &a, &Type::Int).unwrap();
    assert_eq!(zonk(&ctx, &b), list_of(Type::Int));
}

#[test]
fn solve_constraints_success_and_failure() {
    let mut ctx = InferCtx::new();
    let a = ctx.fresh_ty(0);
    let b = ctx.fresh_ty(0);
    let cs = vec![
        Constraint::eq(a.clone(), Type::Int),
        Constraint::eq(b.clone(), a.clone()),
    ];
    solve(&mut ctx, &cs).unwrap();
    assert_eq!(zonk(&ctx, &b), Type::Int);

    // 失败：b 已解析为 int，再要求等于 bool
    let cs2 = vec![Constraint::eq(b.clone(), Type::Bool)];
    assert!(solve(&mut ctx, &cs2).is_err());
}

#[test]
fn any_unifies_with_everything() {
    let mut ctx = InferCtx::new();
    unify(&mut ctx, &Type::Any, &Type::Int).unwrap();
    unify(&mut ctx, &Type::Str, &Type::Any).unwrap();
}

#[test]
fn never_unifies_with_everything() {
    let mut ctx = InferCtx::new();
    unify(&mut ctx, &Type::Never, &Type::Bool).unwrap();
    unify(&mut ctx, &Type::Int, &Type::Never).unwrap();
}

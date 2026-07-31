//! `typing` 库单元测试

use crate::hints::InferCtx;
use crate::magic::engine::MagicEngine;
use crate::types::def::Type;
use crate::ast::Pattern;

use super::*;
use std::collections::HashMap;

fn ctx() -> InferCtx {
    InferCtx::new()
}

// ───────────────────────────── conforms: 类型格 ─────────────────────────────

#[test]
fn any_is_top() {
    assert!(conforms(&ctx(), &Type::Int, &Type::Any).is_ok());
    assert!(conforms(&ctx(), &Type::Named("Foo".into()), &Type::Any).is_ok());
    assert!(conforms(&ctx(), &Type::Never, &Type::Any).is_ok());
}

#[test]
fn never_is_bottom() {
    assert!(conforms(&ctx(), &Type::Never, &Type::Int).is_ok());
    assert!(conforms(&ctx(), &Type::Never, &Type::Bool).is_ok());
}

#[test]
fn reflexivity() {
    assert!(conforms(&ctx(), &Type::Int, &Type::Int).is_ok());
    assert!(conforms(&ctx(), &Type::Named("A".into()), &Type::Named("A".into())).is_ok());
    assert!(conforms(&ctx(), &Type::Tuple(vec![Type::Int, Type::Str]),
        &Type::Tuple(vec![Type::Int, Type::Str])).is_ok());
}

#[test]
fn nominal_mismatch() {
    assert!(conforms(&ctx(), &Type::Named("A".into()), &Type::Named("B".into())).is_err());
    assert!(conforms(&ctx(), &Type::Int, &Type::Bool).is_err());
}

// ───────────────────────────── conforms: 容器 / 泛型 ─────────────────────────────

#[test]
fn generic_covariance() {
    let sub = Type::Generic {
        base: Box::new(Type::Named("List".into())),
        args: vec![Type::Int],
    };
    let sup = Type::Generic {
        base: Box::new(Type::Named("List".into())),
        args: vec![Type::Any],
    };
    assert!(conforms(&ctx(), &sub, &sup).is_ok()); // Int <: Any
    assert!(conforms(&ctx(), &sup, &sub).is_err()); // Any <!: Int
}

#[test]
fn option_covariance() {
    assert!(conforms(&ctx(), &Type::Option(Box::new(Type::Int)),
        &Type::Option(Box::new(Type::Any))).is_ok());
    assert!(conforms(&ctx(), &Type::Option(Box::new(Type::Any)),
        &Type::Option(Box::new(Type::Int))).is_err());
}

#[test]
fn result_covariance() {
    let sub = Type::Result { ok: Box::new(Type::Int), err: Box::new(Type::Never) };
    let sup = Type::Result { ok: Box::new(Type::Any), err: Box::new(Type::Str) };
    assert!(conforms(&ctx(), &sub, &sup).is_ok());
    assert!(conforms(&ctx(), &sup, &sub).is_err());
}

#[test]
fn tuple_conformance_with_never() {
    let sub = Type::Tuple(vec![Type::Int, Type::Never]);
    let sup = Type::Tuple(vec![Type::Any, Type::Bool]);
    assert!(conforms(&ctx(), &sub, &sup).is_ok()); // Never <: Bool, Int <: Any
    assert!(conforms(&ctx(), &sup, &sub).is_err());
}

#[test]
fn generic_arity_mismatch() {
    let sub = Type::Generic {
        base: Box::new(Type::Named("Pair".into())),
        args: vec![Type::Int, Type::Str],
    };
    let sup = Type::Generic {
        base: Box::new(Type::Named("Pair".into())),
        args: vec![Type::Any],
    };
    assert!(matches!(conforms(&ctx(), &sub, &sup), Err(TypingError::Arity(2, 1))));
}

// ───────────────────────────── conforms: 函数子类型 ─────────────────────────────

#[test]
fn function_subtyping() {
    // fn(Any) -> Int  <:  fn(Int) -> Any  （参数逆变、返回协变）
    let sub = Type::Fn { params: vec![Type::Any], ret: Box::new(Type::Int) };
    let sup = Type::Fn { params: vec![Type::Int], ret: Box::new(Type::Any) };
    assert!(conforms(&ctx(), &sub, &sup).is_ok());
    // 反向不成立
    assert!(conforms(&ctx(), &sup, &sub).is_err());
}

// ───────────────────────────── conforms: 引用变型 ─────────────────────────────

#[test]
fn shared_ref_covariant() {
    assert!(conforms(&ctx(), &Type::Ref(Box::new(Type::Int)),
        &Type::Ref(Box::new(Type::Any))).is_ok());
    assert!(conforms(&ctx(), &Type::Ref(Box::new(Type::Any)),
        &Type::Ref(Box::new(Type::Int))).is_err());
}

#[test]
fn mut_ref_invariant() {
    assert!(conforms(&ctx(), &Type::MutRef(Box::new(Type::Int)),
        &Type::MutRef(Box::new(Type::Int))).is_ok());
    // &mut T 不变：即使 Int <: Any 也不允许
    assert!(conforms(&ctx(), &Type::MutRef(Box::new(Type::Int)),
        &Type::MutRef(Box::new(Type::Any))).is_err());
}

// ───────────────────────────── conforms: 推断孔 ─────────────────────────────

#[test]
fn unresolved_var_errors() {
    let mut c = InferCtx::new();
    let v = c.fresh_ty(0);
    assert!(matches!(conforms(&c, &v, &Type::Int), Err(TypingError::UnresolvedVar(_))));
}

// ───────────────────────────── trait 满足性 ─────────────────────────────

#[test]
fn mem_provider_satisfies_trait() {
    let mut env = TraitEnv::new();
    env.register(TraitReq::new("Add").require(MethodReq::new("__add__")));

    let mut prov = MemProvider::new();
    prov.add("Point", "__add__", vec![Type::Named("Point".into())], Type::Named("Point".into()));
    assert!(satisfies(&env, &prov, &Type::Named("Point".into()), "Add").is_ok());

    // 缺少方法
    assert!(matches!(
        satisfies(&env, &prov, &Type::Named("Other".into()), "Add"),
        Err(TypingError::MissingMethod(_, _))
    ));
}

#[test]
fn unknown_trait_errors() {
    let env = TraitEnv::new();
    let prov = MemProvider::new();
    assert!(matches!(
        satisfies(&env, &prov, &Type::Named("X".into()), "NoSuch"),
        Err(TypingError::UnknownTrait(_))
    ));
}

#[test]
fn signature_conformance_ok() {
    // 要求：__add__(Int) -> Any
    let mut env = TraitEnv::new();
    env.register(TraitReq::new("Add")
        .require(MethodReq::new("__add__").with_sig(vec![Type::Int], Type::Any)));

    // 实现：(Any) -> Int —— 参数逆变(要求 Int <: 实现 Any ✓)、返回协变(实现 Int <: 要求 Any ✓)
    let mut prov = MemProvider::new();
    prov.add("P", "__add__", vec![Type::Any], Type::Int);
    assert!(satisfies(&env, &prov, &Type::Named("P".into()), "Add").is_ok());

    // 同名同型实现
    let mut prov2 = MemProvider::new();
    prov2.add("Q", "__add__", vec![Type::Int], Type::Any);
    assert!(satisfies(&env, &prov2, &Type::Named("Q".into()), "Add").is_ok());
}

#[test]
fn signature_mismatch_errors() {
    // 要求返回 Bool
    let mut env = TraitEnv::new();
    env.register(TraitReq::new("Add")
        .require(MethodReq::new("__add__").with_sig(vec![Type::Any], Type::Bool)));

    // 实现返回 Int —— Int <: Bool 失败
    let mut prov = MemProvider::new();
    prov.add("R", "__add__", vec![Type::Any], Type::Int);
    assert!(matches!(
        satisfies(&env, &prov, &Type::Named("R".into()), "Add"),
        Err(TypingError::SignatureMismatch(_, _, _, _))
    ));

    // 参数更窄（实现要求 Any，但实现只接受 Int）—— 要求 Any <: 实现 Int 失败
    let mut env2 = TraitEnv::new();
    env2.register(TraitReq::new("Add")
        .require(MethodReq::new("__add__").with_sig(vec![Type::Any], Type::Any)));
    let mut prov2 = MemProvider::new();
    prov2.add("S", "__add__", vec![Type::Int], Type::Any); // 实现只接受 Int
    assert!(matches!(
        satisfies(&env2, &prov2, &Type::Named("S".into()), "Add"),
        Err(TypingError::SignatureMismatch(_, _, _, _))
    ));
}

// ───────────────────────────── magic 桥接 ─────────────────────────────

#[test]
fn magic_register_and_satisfy() {
    let engine = MagicEngine::new();
    let mut env = TraitEnv::new();
    register_magic_traits(&mut env, &engine);

    assert!(env.contains("std::ops::Add"));
    assert!(env.contains("std::cmp::PartialOrd"));
    assert!(env.contains("std::iter::IntoIterator"));

    // Vec2 实现了 __add__ → 满足 Add
    let mut prov = MemProvider::new();
    prov.add("Vec2", "__add__", vec![Type::Named("Vec2".into())], Type::Named("Vec2".into()));
    assert!(satisfies_magic(&env, &prov, &Type::Named("Vec2".into()), &engine, "__add__").is_ok());

    // 未实现 __add__ → 不满足
    let empty = MemProvider::new();
    assert!(satisfies_magic(&env, &empty, &Type::Named("NoAdd".into()), &engine, "__add__").is_err());
}

#[test]
fn magic_unknown_method() {
    let engine = MagicEngine::new();
    let env = TraitEnv::new();
    let prov = MemProvider::new();
    assert!(matches!(
        satisfies_magic(&env, &prov, &Type::Named("X".into()), &engine, "__nonexistent__"),
        Err(TypingError::UnknownTrait(_))
    ));
}

// ───────────────────────────── variance ─────────────────────────────

#[test]
fn variance_vec_covariant() {
    let t = Type::Named("T".into());
    let vec_t = Type::Generic {
        base: Box::new(Type::Named("Vec".into())),
        args: vec![t.clone()],
    };
    assert_eq!(variance_of(&vec_t, &t), Variance::Covariant);
}

#[test]
fn variance_fn_bivariant_is_invariant() {
    // fn(T) -> T：参数逆变 + 返回协变 → 合成不变
    let t = Type::Named("T".into());
    let fn_t = Type::Fn { params: vec![t.clone()], ret: Box::new(t.clone()) };
    assert_eq!(variance_of(&fn_t, &t), Variance::Invariant);
}

#[test]
fn variance_mut_ref_invariant() {
    let t = Type::Named("T".into());
    let mref = Type::MutRef(Box::new(t.clone()));
    assert_eq!(variance_of(&mref, &t), Variance::Invariant);
}

#[test]
fn variance_irrelevant_when_absent() {
    let t = Type::Named("T".into());
    let u = Type::Named("U".into());
    assert_eq!(variance_of(&Type::Int, &t), Variance::Irrelevant);
    assert_eq!(variance_of(&Type::Named("Other".into()), &t), Variance::Irrelevant);
    // 不出现 T 的容器
    let only_u = Type::Option(Box::new(u.clone()));
    assert_eq!(variance_of(&only_u, &t), Variance::Irrelevant);
}

// ───────────────────────────── 跨简写 vs Generic 变体匹配 ─────────────────────────────

#[test]
fn option_shorthand_matches_generic() {
    // Option(Int) 与 Generic{base:Named("Option"), args:[Int]} 应视为相同
    let opt_shorthand = Type::Option(Box::new(Type::Int));
    let opt_generic = Type::Generic {
        base: Box::new(Type::Named("Option".into())),
        args: vec![Type::Int],
    };
    assert!(conforms(&ctx(), &opt_shorthand, &opt_generic).is_ok(),
        "Option shorthand should match Generic form");
    assert!(conforms(&ctx(), &opt_generic, &opt_shorthand).is_ok(),
        "Generic Option should match shorthand form");
}

#[test]
fn optional_sugar_matches_generic() {
    // T? (Optional) 与 Generic{base:"Option"} 匹配
    let opt_sugar = Type::Optional(Box::new(Type::Int));
    let opt_generic = Type::Generic {
        base: Box::new(Type::Named("Option".into())),
        args: vec![Type::Int],
    };
    assert!(conforms(&ctx(), &opt_sugar, &opt_generic).is_ok());
    assert!(conforms(&ctx(), &opt_generic, &opt_sugar).is_ok());
}

#[test]
fn result_shorthand_matches_generic() {
    let res_shorthand = Type::Result { ok: Box::new(Type::Int), err: Box::new(Type::Str) };
    let res_generic = Type::Generic {
        base: Box::new(Type::Named("Result".into())),
        args: vec![Type::Int, Type::Str],
    };
    assert!(conforms(&ctx(), &res_shorthand, &res_generic).is_ok());
    assert!(conforms(&ctx(), &res_generic, &res_shorthand).is_ok());
}

#[test]
fn option_shorthand_covariant_via_normalize() {
    // Option(Int) <: Generic{"Option", [Any]} 应通过（Int <: Any 经归一化后协变）
    let sub = Type::Option(Box::new(Type::Int));
    let sup = Type::Generic {
        base: Box::new(Type::Named("Option".into())),
        args: vec![Type::Any],
    };
    assert!(conforms(&ctx(), &sub, &sup).is_ok());
    assert!(conforms(&ctx(), &sup, &sub).is_err()); // 反向：Any <!: Int
}

// ───────────────────────────── 深层多级嵌套类型判断 ─────────────────────────────

fn list(inner: Type) -> Type {
    Type::Generic {
        base: Box::new(Type::Named("List".into())),
        args: vec![inner],
    }
}

fn option(inner: Type) -> Type {
    Type::Generic {
        base: Box::new(Type::Named("Option".into())),
        args: vec![inner],
    }
}

fn result(ok: Type, err: Type) -> Type {
    Type::Generic {
        base: Box::new(Type::Named("Result".into())),
        args: vec![ok, err],
    }
}

#[test]
fn deep_nesting_3_levels() {
    // List<Option<Int>>
    let nested = list(option(Type::Int));
    // List<Option<Any>>
    let wider = list(option(Type::Any));
    assert!(conforms(&ctx(), &nested, &wider).is_ok(), "3-level covariant");
    assert!(conforms(&ctx(), &wider, &nested).is_err(), "reverse fails");
}

#[test]
fn deep_nesting_5_levels() {
    // Result<Option<List<Option<Int>>>, Never>
    //           |-- level 3 --|
    //      |---- level 2 -----|
    // |------- level 1 -------|
    let deep = result(
        option(list(option(Type::Int))),
        Type::Never,
    );
    let deep_any = result(
        option(list(option(Type::Any))),
        Type::Never,
    );
    assert!(conforms(&ctx(), &deep, &deep_any).is_ok(), "5-level nesting conforms");
    assert!(conforms(&ctx(), &deep_any, &deep).is_err(), "reverse fails");
}

#[test]
fn deep_nesting_with_ref() {
    // &List<Option<&Int>>  <:  &List<Option<&Any>>
    let sub = Type::Ref(Box::new(list(option(Type::Ref(Box::new(Type::Int))))));
    let sup = Type::Ref(Box::new(list(option(Type::Ref(Box::new(Type::Any))))));
    assert!(conforms(&ctx(), &sub, &sup).is_ok());
}

#[test]
fn deep_nesting_with_fn() {
    // fn(List<Option<Int>>) -> List<Option<Any>>
    let sub = Type::Fn {
        params: vec![list(option(Type::Int))],
        ret: Box::new(list(option(Type::Any))),
    };
    let sup = Type::Fn {
        params: vec![list(option(Type::Any))],  // 参数逆变
        ret: Box::new(list(option(Type::Any))),
    };
    // fn(Int) → 简化：参数逆变要求 Isup <: Isub，即 List<Option<Any>> <: List<Option<Int>>
    // 但 Any <!: Int，所以逆变为 false，整体不成立
    assert!(conforms(&ctx(), &sub, &sup).is_err(),
        "fn with narrower params (contravariance) can't have wider params");
}

#[test]
fn simd_deep_nesting() {
    // Simd[Option<List<Int>>, 4] 元素经递归协变
    let sub = Type::Simd {
        elem: Box::new(option(list(Type::Int))),
        width: 4,
    };
    let sup = Type::Simd {
        elem: Box::new(option(list(Type::Any))),
        width: 4,
    };
    assert!(conforms(&ctx(), &sub, &sup).is_ok());
    assert!(conforms(&ctx(), &sup, &sub).is_err());
}

#[test]
fn deep_nesting_mismatch_at_mid_level() {
    // List<Option<Int>>  vs  List<Result<Int, Never>>
    let a = list(option(Type::Int));
    let b = list(result(Type::Int, Type::Never));
    assert!(conforms(&ctx(), &a, &b).is_err(),
        "Option 与 Result 在中层不匹配");
}

#[test]
fn deep_nesting_arity_mismatch() {
    // Result<Int, Str>  vs  Result<Int> （缺少 err 参数）
    let a = result(Type::Int, Type::Str);
    let b = Type::Generic {
        base: Box::new(Type::Named("Result".into())),
        args: vec![Type::Int],
    };
    assert!(matches!(conforms(&ctx(), &a, &b), Err(TypingError::Arity(2, 1))));
}

#[test]
fn tuple_deep_nesting() {
    // (Int, Option<List<Str>>, &Result<Never, Bool>)
    let sub = Type::Tuple(vec![
        Type::Int,
        option(list(Type::Str)),
        Type::Ref(Box::new(result(Type::Never, Type::Bool))),
    ]);
    let sup = Type::Tuple(vec![
        Type::Any,
        option(list(Type::Any)),
        Type::Ref(Box::new(result(Type::Any, Type::Any))),
    ]);
    assert!(conforms(&ctx(), &sub, &sup).is_ok());
    assert!(conforms(&ctx(), &sup, &sub).is_err());
}

// ──────────────────────── 类型别名展开 + conforms ────────────────────────

#[test]
fn type_alias_expands_before_conforms() {
    // 模拟 type MyList<T> = List<T> 展开后的比较
    let expanded = Type::Generic {
        base: Box::new(Type::Named("List".into())),
        args: vec![Type::Int],
    };
    let alias_call = Type::Generic {
        base: Box::new(Type::Named("List".into())),
        args: vec![Type::Int],
    };
    assert!(conforms(&ctx(), &expanded, &alias_call).is_ok());
    assert!(conforms(&ctx(), &alias_call, &expanded).is_ok());
}

#[test]
fn alias_nested_generic_conforms() {
    // 模拟 type Nested<T,U> = Result<Option<T>, U>
    let exact = result(option(Type::Int), Type::Str);
    let wide = result(option(Type::Any), Type::Any);
    assert!(conforms(&ctx(), &exact, &wide).is_ok());
    assert!(conforms(&ctx(), &wide, &exact).is_err());
}

#[test]
fn alias_after_expand_type_conforms() {
    // expand_type 展开别名后，结果应与 conforms 协同
    let body = Type::Generic {
        base: Box::new(Type::Named("HashMap".into())),
        args: vec![Type::Str, Type::Int],
    };
    assert!(conforms(&ctx(), &body, &body).is_ok());
    let body_wider = Type::Generic {
        base: Box::new(Type::Named("HashMap".into())),
        args: vec![Type::Str, Type::Any],
    };
    assert!(conforms(&ctx(), &body, &body_wider).is_ok());
    assert!(conforms(&ctx(), &body_wider, &body).is_err());
}


// ───────────────────────────── 穷尽性检查 ─────────────────────────────

#[test]
fn test_exhaustive_bool_covers_all() {
    let variants = HashMap::new();
    assert!(check_exhaustive(
        &Type::Bool,
        &[Pattern::Bool(true), Pattern::Bool(false)],
        &variants,
    ).is_none());
}

#[test]
fn test_non_exhaustive_enum_missing_variant() {
    let mut variants = HashMap::new();
    variants.insert("Color".into(), vec!["Red".into(), "Green".into(), "Blue".into()]);
    let msg = check_exhaustive(
        &Type::Named("Color".into()),
        &[
            Pattern::Variant("Color.Red".into(), vec![]),
            Pattern::Variant("Color.Green".into(), vec![]),
        ],
        &variants,
    ).unwrap();
    assert!(msg.contains("Blue"), "missing variant report: {}", msg);
}

#[test]
fn test_exhaustive_wildcard_covers_enum() {
    let mut variants = HashMap::new();
    variants.insert("Color".into(), vec!["Red".into(), "Green".into(), "Blue".into()]);
    assert!(check_exhaustive(
        &Type::Named("Color".into()),
        &[Pattern::Wildcard],
        &variants,
    ).is_none());
}

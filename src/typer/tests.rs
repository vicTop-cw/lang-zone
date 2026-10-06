// Lang-Zone 编译器 — typer/tests.rs
// （由 typer/mod.rs move-only 拆出，逻辑零改动）

use super::*;

use super::*;
use crate::ast::*;

/// 快速构造一个含 let 绑定和函数调用的模块，验证推断填充类型
#[test]
fn test_typer_fills_let_binding() {
    let mut module = Module {
        imports: vec![],
        functions: vec![Function {
            name: "add".into(),
            generics: vec![],
            generic_kinds: Vec::new(),
            generic_bounds: vec![],
            generic_defaults: vec![],
            params: vec![
                Param {
                    name: "x".into(),
                    ty: Some(Type::Int),
                    default: None,
                    is_mut: false,
                    is_owned: false,
                    is_ref: false,
                    is_positional_only: false,
                },
                Param {
                    name: "y".into(),
                    ty: Some(Type::Int),
                    default: None,
                    is_mut: false,
                    is_owned: false,
                    is_ref: false,
                    is_positional_only: false,
                },
            ],
            return_type: None,
            raises: None,
            where_clause: vec![],
            body: vec![
                Stmt::Let {
                    name: "z".into(),
                    mutable: false,
                    is_ref: false,
                    comptime: false,
                    ty: None,
                    value: Expr::Binary {
                        left: Box::new(Expr::Ident("x".into())),
                        op: BinOp::Add,
                        right: Box::new(Expr::Ident("y".into())),
                    },
                },
                Stmt::Return(Some(Expr::Ident("z".into()))),
            ],
            is_async: false,
            is_abstract: false,
            comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None,
            params_checker: None,
        }],
        structs: vec![],
        traits: vec![],
        impls: vec![],
        consts: vec![],
        type_aliases: vec![],
        magic_decls: vec![],
        tests: vec![],
        name: Some("test".into()),
        file_path: Some("test.lz".into()),
        package: None,
        is_macro: false,
        doc: None,
    };

    let errors = Typer::infer_module(&mut module);
    assert!(errors.is_empty(), "infer errors: {:?}", errors);

    // let z = x + y 应被推断为 Int
    let f = &module.functions[0];
    let let_stmt = &f.body[0];
    if let Stmt::Let { ty, .. } = let_stmt {
        assert!(ty.is_some(), "let z type should be inferred");
        let inferred = ty.as_ref().unwrap();
        // 应为 Int（Int + Int → Int）
        assert_eq!(
            inferred.to_rust_type_string(),
            "i64",
            "expected i64 for Int-Inferred, got: {}",
            inferred.to_rust_type_string()
        );
    } else {
        panic!("expected Stmt::Let");
    }

    // 返回类型应被推断为 Int
    assert!(f.return_type.is_some(), "return type should be inferred");
    assert_eq!(f.return_type.as_ref().unwrap().to_rust_type_string(), "i64");
}

#[test]
fn test_typer_int_binary() {
    let mut module = Module {
        imports: vec![],
        functions: vec![Function {
            name: "calc".into(),
            generics: vec![],
            generic_kinds: Vec::new(),
            generic_bounds: vec![],
            generic_defaults: vec![],
            params: vec![],
            return_type: None,
            raises: None,
            where_clause: vec![],
            body: vec![
                Stmt::Let {
                    name: "a".into(),
                    mutable: false,
                    is_ref: false,
                    comptime: false,
                    ty: None,
                    value: Expr::IntLit(10),
                },
                Stmt::Let {
                    name: "b".into(),
                    mutable: false,
                    is_ref: false,
                    comptime: false,
                    ty: None,
                    value: Expr::Binary {
                        left: Box::new(Expr::Ident("a".into())),
                        op: BinOp::Mul,
                        right: Box::new(Expr::IntLit(3)),
                    },
                },
                Stmt::Return(Some(Expr::Ident("b".into()))),
            ],
            is_async: false,
            is_abstract: false,
            comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None,
            params_checker: None,
        }],
        structs: vec![],
        traits: vec![],
        impls: vec![],
        consts: vec![ConstDef {
            name: "MAX".into(),
            ty: None,
            value: Expr::IntLit(100),
            mutable: false,
            comptime: false,
        }],
        type_aliases: vec![],
        magic_decls: vec![],
        tests: vec![],
        name: Some("test".into()),
        file_path: Some("test.lz".into()),
        package: None,
        is_macro: false,
        doc: None,
    };

    let errors = Typer::infer_module(&mut module);
    assert!(errors.is_empty(), "infer errors: {:?}", errors);

    // let a = 10 → a: Int
    let a_ty = match &module.functions[0].body[0] {
        Stmt::Let { ty, .. } => ty.clone(),
        _ => panic!("expected Stmt::Let"),
    };
    assert_eq!(a_ty.unwrap().to_rust_type_string(), "i64");

    // let b = a * 3 → b: Int
    let b_ty = match &module.functions[0].body[1] {
        Stmt::Let { ty, .. } => ty.clone(),
        _ => panic!("expected Stmt::Let"),
    };
    assert_eq!(b_ty.unwrap().to_rust_type_string(), "i64");

    // const MAX: inferred as Int
    assert_eq!(
        module.consts[0].ty.as_ref().unwrap().to_rust_type_string(),
        "i64"
    );
}

/// 端到端验证 trait Show + impl Show for int + 泛型函数 print_show[T: Show]
#[test]
fn test_typer_resolves_trait_instance() {
    let show_method = Function {
        name: "show".into(),
        generics: vec![],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        params: vec![Param {
            name: "self".into(),
            ty: Some(Type::Self_),
            default: None,
            is_mut: false,
            is_owned: false,
            is_ref: false,
            is_positional_only: false,
        }],
        return_type: Some(Type::Str),
        raises: None,
        where_clause: vec![],
        body: vec![],
        is_async: false,
        is_abstract: false,
        comptime: false,
        decorators: vec![],
        attributes: vec![],
        variadic: None,
        params_checker: None,
    };

    let trait_show = TraitDef {
        name: "Show".into(),
        generics: vec![],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        methods: vec![show_method.clone()],
        fields: vec![],
        type_aliases: vec![],
    };

    let impl_show_int = ImplDef {
        trait_name: Some("Show".into()),
        type_name: "int".into(),
        generics: vec![],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        where_clause: vec![],
        methods: vec![Function {
            name: "show".into(),
            generics: vec![],
            generic_kinds: vec![],
            generic_bounds: vec![],
            generic_defaults: vec![],
            params: vec![Param {
                name: "self".into(),
                ty: Some(Type::Int),
                default: None,
                is_mut: false,
                is_owned: false,
                is_ref: false,
                is_positional_only: false,
            }],
            return_type: Some(Type::Str),
            raises: None,
            where_clause: vec![],
            body: vec![Stmt::Return(Some(Expr::StrLit("".into())))],
            is_async: false,
            is_abstract: false,
            comptime: false,
            decorators: vec![],
            attributes: vec![],
            variadic: None,
            params_checker: None,
        }],
        type_aliases: vec![],
    };

    let print_show = Function {
        name: "print_show".into(),
        generics: vec!["T".into()],
        generic_kinds: vec![],
        generic_bounds: vec![("T".into(), vec![Type::Named("Show".into())])],
        generic_defaults: vec![],
        params: vec![Param {
            name: "x".into(),
            ty: Some(Type::Named("T".into())),
            default: None,
            is_mut: false,
            is_owned: false,
            is_ref: false,
            is_positional_only: false,
        }],
        return_type: Some(Type::Str),
        raises: None,
        where_clause: vec![],
        body: vec![Stmt::Return(Some(Expr::MethodCall {
            receiver: Box::new(Expr::Ident("x".into())),
            method: "show".into(),
            args: vec![],
        }))],
        is_async: false,
        is_abstract: false,
        comptime: false,
        decorators: vec![],
        attributes: vec![],
        variadic: None,
        params_checker: None,
    };

    let main_fn = Function {
        name: "main".into(),
        generics: vec![],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        params: vec![],
        return_type: None,
        raises: None,
        where_clause: vec![],
        body: vec![Stmt::Expr(Expr::Call {
            func: Box::new(Expr::Ident("print_show".into())),
            args: vec![Expr::IntLit(42)],
            checker: None,
        })],
        is_async: false,
        is_abstract: false,
        comptime: false,
        decorators: vec![],
        attributes: vec![],
        variadic: None,
        params_checker: None,
    };

    let mut module = Module {
        imports: vec![],
        functions: vec![print_show, main_fn],
        structs: vec![],
        traits: vec![trait_show],
        impls: vec![impl_show_int],
        consts: vec![],
        type_aliases: vec![],
        magic_decls: vec![],
        tests: vec![],
        name: Some("test".into()),
        file_path: Some("test.lz".into()),
        package: None,
        is_macro: false,
        doc: None,
    };

    let errors = Typer::infer_module(&mut module);
    assert!(
        errors.is_empty(),
        "expected no infer errors, got: {:?}",
        errors
    );
}

/// 端到端验证缺失 trait 实例时类型推断报错
#[test]
fn test_typer_rejects_missing_trait_instance() {
    let show_method = Function {
        name: "show".into(),
        generics: vec![],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        params: vec![Param {
            name: "self".into(),
            ty: Some(Type::Self_),
            default: None,
            is_mut: false,
            is_owned: false,
            is_ref: false,
            is_positional_only: false,
        }],
        return_type: Some(Type::Str),
        raises: None,
        where_clause: vec![],
        body: vec![],
        is_async: false,
        is_abstract: false,
        comptime: false,
        decorators: vec![],
        attributes: vec![],
        variadic: None,
        params_checker: None,
    };

    let trait_show = TraitDef {
        name: "Show".into(),
        generics: vec![],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        methods: vec![show_method],
        fields: vec![],
        type_aliases: vec![],
    };

    let foo = Function {
        name: "foo".into(),
        generics: vec!["T".into()],
        generic_kinds: vec![],
        generic_bounds: vec![("T".into(), vec![Type::Named("Show".into())])],
        generic_defaults: vec![],
        params: vec![Param {
            name: "x".into(),
            ty: Some(Type::Named("T".into())),
            default: None,
            is_mut: false,
            is_owned: false,
            is_ref: false,
            is_positional_only: false,
        }],
        return_type: Some(Type::Named("T".into())),
        raises: None,
        where_clause: vec![],
        body: vec![Stmt::Return(Some(Expr::Ident("x".into())))],
        is_async: false,
        is_abstract: false,
        comptime: false,
        decorators: vec![],
        attributes: vec![],
        variadic: None,
        params_checker: None,
    };

    let main_fn = Function {
        name: "main".into(),
        generics: vec![],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        params: vec![],
        return_type: None,
        raises: None,
        where_clause: vec![],
        body: vec![Stmt::Expr(Expr::Call {
            func: Box::new(Expr::Ident("foo".into())),
            args: vec![Expr::IntLit(42)],
            checker: None,
        })],
        is_async: false,
        is_abstract: false,
        comptime: false,
        decorators: vec![],
        attributes: vec![],
        variadic: None,
        params_checker: None,
    };

    let mut module = Module {
        imports: vec![],
        functions: vec![foo, main_fn],
        structs: vec![],
        traits: vec![trait_show],
        impls: vec![],
        consts: vec![],
        type_aliases: vec![],
        magic_decls: vec![],
        tests: vec![],
        name: Some("test".into()),
        file_path: Some("test.lz".into()),
        package: None,
        is_macro: false,
        doc: None,
    };

    let errors = Typer::infer_module(&mut module);
    assert!(
        !errors.is_empty(),
        "expected infer error for missing Show instance"
    );
    let joined = errors.join("\n");
    assert!(
        joined.contains("does not implement trait `Show`"),
        "expected Show trait error, got: {}",
        joined
    );
}

/// GADT 构造与模式匹配推断
#[test]
fn test_gadt_construct_and_match() {
    let expr_enum = StructDef {
        name: "Expr".into(),
        generics: vec!["T".into()],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        fields: vec![
            Field {
                name: "Lit".into(),
                ty: Type::Record(vec![("value".into(), Type::Int)]),
                default: None,
                variant_return: Some(Type::Generic {
                    base: Box::new(Type::Named("Expr".into())),
                    args: vec![Type::Int],
                }),
            },
            Field {
                name: "Add".into(),
                ty: Type::Record(vec![
                    (
                        "left".into(),
                        Type::Generic {
                            base: Box::new(Type::Named("Expr".into())),
                            args: vec![Type::Int],
                        },
                    ),
                    (
                        "right".into(),
                        Type::Generic {
                            base: Box::new(Type::Named("Expr".into())),
                            args: vec![Type::Int],
                        },
                    ),
                ]),
                default: None,
                variant_return: Some(Type::Generic {
                    base: Box::new(Type::Named("Expr".into())),
                    args: vec![Type::Int],
                }),
            },
        ],
        methods: vec![],
        is_enum: true,
        decorators: vec![],
        attributes: vec![],
        is_case: false,
        repr_attr: None,
    };

    let main_fn = Function {
        name: "main".into(),
        generics: vec![],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        params: vec![],
        return_type: Some(Type::Int),
        raises: None,
        where_clause: vec![],
        body: vec![
            Stmt::Let {
                name: "e".into(),
                mutable: false,
                is_ref: false,
                comptime: false,
                ty: Some(Type::Generic {
                    base: Box::new(Type::Named("Expr".into())),
                    args: vec![Type::Int],
                }),
                value: Expr::MethodCall {
                    receiver: Box::new(Expr::Ident("Expr".into())),
                    method: "Lit".into(),
                    args: vec![Expr::IntLit(42)],
                },
            },
            Stmt::Return(Some(Expr::Match {
                expr: Box::new(Expr::Ident("e".into())),
                arms: vec![
                    MatchArm {
                        pattern: Pattern::Variant(
                            "Expr.Lit".into(),
                            vec![Pattern::Ident("x".into())],
                        ),
                        guard: None,
                        body: vec![
                            Stmt::Let {
                                name: "z".into(),
                                mutable: false,
                                is_ref: false,
                                comptime: false,
                                ty: Some(Type::Int),
                                value: Expr::Ident("x".into()),
                            },
                            Stmt::Expr(Expr::Ident("z".into())),
                        ],
                    },
                    MatchArm {
                        pattern: Pattern::Wildcard,
                        guard: None,
                        body: vec![Stmt::Expr(Expr::IntLit(0))],
                    },
                ],
            })),
        ],
        is_async: false,
        is_abstract: false,
        comptime: false,
        decorators: vec![],
        attributes: vec![],
        variadic: None,
        params_checker: None,
    };

    let mut module = Module {
        imports: vec![],
        functions: vec![main_fn],
        structs: vec![expr_enum],
        traits: vec![],
        impls: vec![],
        consts: vec![],
        type_aliases: vec![],
        magic_decls: vec![],
        tests: vec![],
        name: Some("test".into()),
        file_path: Some("test.lz".into()),
        package: None,
        is_macro: false,
        doc: None,
    };

    let errors = Typer::infer_module(&mut module);
    assert!(errors.is_empty(), "infer errors: {:?}", errors);
}

/// 泛型枚举变体返回 fresh 泛型类型并与具体索引统一
#[test]
fn test_generic_enum_variant_return_fresh_generics() {
    let option_enum = StructDef {
        name: "Option".into(),
        generics: vec!["T".into()],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        fields: vec![
            Field {
                name: "Some".into(),
                ty: Type::Named("T".into()),
                default: None,
                variant_return: None,
            },
            Field {
                name: "None".into(),
                ty: Type::Unit,
                default: None,
                variant_return: None,
            },
        ],
        methods: vec![],
        is_enum: true,
        decorators: vec![],
        attributes: vec![],
        is_case: false,
        repr_attr: None,
    };

    let main_fn = Function {
        name: "main".into(),
        generics: vec![],
        generic_kinds: vec![],
        generic_bounds: vec![],
        generic_defaults: vec![],
        params: vec![],
        return_type: Some(Type::Int),
        raises: None,
        where_clause: vec![],
        body: vec![
            Stmt::Let {
                name: "o".into(),
                mutable: false,
                is_ref: false,
                comptime: false,
                ty: Some(Type::Generic {
                    base: Box::new(Type::Named("Option".into())),
                    args: vec![Type::Int],
                }),
                value: Expr::MethodCall {
                    receiver: Box::new(Expr::Ident("Option".into())),
                    method: "Some".into(),
                    args: vec![Expr::IntLit(1)],
                },
            },
            Stmt::Return(Some(Expr::Match {
                expr: Box::new(Expr::Ident("o".into())),
                arms: vec![
                    MatchArm {
                        pattern: Pattern::Variant(
                            "Option.Some".into(),
                            vec![Pattern::Ident("v".into())],
                        ),
                        guard: None,
                        body: vec![
                            Stmt::Let {
                                name: "w".into(),
                                mutable: false,
                                is_ref: false,
                                comptime: false,
                                ty: Some(Type::Int),
                                value: Expr::Ident("v".into()),
                            },
                            Stmt::Expr(Expr::Ident("w".into())),
                        ],
                    },
                    MatchArm {
                        pattern: Pattern::Wildcard,
                        guard: None,
                        body: vec![Stmt::Expr(Expr::IntLit(0))],
                    },
                ],
            })),
        ],
        is_async: false,
        is_abstract: false,
        comptime: false,
        decorators: vec![],
        attributes: vec![],
        variadic: None,
        params_checker: None,
    };

    let mut module = Module {
        imports: vec![],
        functions: vec![main_fn],
        structs: vec![option_enum],
        traits: vec![],
        impls: vec![],
        consts: vec![],
        type_aliases: vec![],
        magic_decls: vec![],
        tests: vec![],
        name: Some("test".into()),
        file_path: Some("test.lz".into()),
        package: None,
        is_macro: false,
        doc: None,
    };

    let errors = Typer::infer_module(&mut module);
    assert!(errors.is_empty(), "infer errors: {:?}", errors);
}

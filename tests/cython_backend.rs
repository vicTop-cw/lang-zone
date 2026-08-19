// Lang-Zone 编译器 — tests/cython_backend.rs
// Cython 后端测试套件：验证 LZIR → Cython 代码生成
//
// Ω-spec 验证基准：CY/corpus/cy_*.json
// 测试方式：直接构造 IrModule + 调用 CythonCodeGen::generate()，不依赖 CLI 路由

use lang_zone::ir::codegen_cython::{CythonCodeGen, TypeCtx};
use lang_zone::ir::node::{
    ConstDef, EnumDef, Field, FnDef, GenericParam, Item, Param, StructDef, TypeAliasDef, UseStmt,
    Variant,
};
use lang_zone::ir::node::{DuckDef, DuckField, DuckMethod, FnSig, ImplDef, TestDef, TraitDef};
use lang_zone::ir::IrModule;
use lang_zone::ir::types::IrType;

// ── 辅助函数 ──

fn gen(module: IrModule) -> String {
    let mut cg = CythonCodeGen::new();
    cg.generate(&module).to_string()
}

fn assert_contains(pyx: &str, expected: &[&str], label: &str) {
    for exp in expected {
        assert!(
            pyx.contains(exp),
            "[{label}] 应包含 '{exp}'，实际输出:\n{pyx}"
        );
    }
}

// ── Ω-spec: cy_type_map ──

#[test]
fn cy_omega_gate_type_map() {
    let cg = CythonCodeGen::new();

    // Signature 上下文 → C 类型
    assert_eq!(cg.map_type(&IrType::Int, TypeCtx::Signature), "Py_ssize_t");
    assert_eq!(cg.map_type(&IrType::F64, TypeCtx::Signature), "double");
    assert_eq!(cg.map_type(&IrType::Str, TypeCtx::Signature), "str");
    assert_eq!(cg.map_type(&IrType::Bool, TypeCtx::Signature), "bint");
    assert_eq!(cg.map_type(&IrType::Unit, TypeCtx::Signature), "void");
    assert_eq!(cg.map_type(&IrType::Any, TypeCtx::Signature), "object");

    // Container 上下文 → object
    assert_eq!(cg.map_type(&IrType::Int, TypeCtx::Container), "object");
    assert_eq!(cg.map_type(&IrType::F64, TypeCtx::Container), "object");

    // 容器类型
    assert_eq!(
        cg.map_type(&IrType::named_with("List", vec![IrType::Int]), TypeCtx::Signature),
        "list"
    );
    assert_eq!(
        cg.map_type(
            &IrType::named_with("Dict", vec![IrType::Str, IrType::Int]),
            TypeCtx::Signature
        ),
        "dict"
    );
    assert_eq!(
        cg.map_type(&IrType::named_with("Set", vec![IrType::Int]), TypeCtx::Signature),
        "set"
    );

    // 智能指针 → object
    assert_eq!(
        cg.map_type(&IrType::named_with("Box", vec![IrType::Int]), TypeCtx::Signature),
        "object"
    );
    assert_eq!(
        cg.map_type(&IrType::named_with("Rc", vec![IrType::Int]), TypeCtx::Signature),
        "object"
    );
    assert_eq!(
        cg.map_type(&IrType::named_with("Arc", vec![IrType::Int]), TypeCtx::Signature),
        "object"
    );

    // 引用 → object
    assert_eq!(
        cg.map_type(&IrType::Ref(Box::new(IrType::Int)), TypeCtx::Signature),
        "object"
    );
    assert_eq!(
        cg.map_type(&IrType::MutRef(Box::new(IrType::Int)), TypeCtx::Signature),
        "object"
    );

    // Duck → object
    assert_eq!(
        cg.map_type(&IrType::Duck { fields: vec![] }, TypeCtx::Signature),
        "object"
    );

    // Generic → object
    assert_eq!(
        cg.map_type(&IrType::Generic("T".into()), TypeCtx::Signature),
        "object"
    );

    // Ext → object
    assert_eq!(cg.map_type(&IrType::Ext, TypeCtx::Signature), "object");

    // Option/Result → object
    assert_eq!(
        cg.map_type(&IrType::Option(Box::new(IrType::Int)), TypeCtx::Signature),
        "object"
    );
    assert_eq!(
        cg.map_type(
            &IrType::Result {
                ok: Box::new(IrType::Int),
                err: Box::new(IrType::Str),
            },
            TypeCtx::Signature
        ),
        "object"
    );

    // Tuple → tuple
    assert_eq!(
        cg.map_type(
            &IrType::Tuple(vec![IrType::Int, IrType::F64]),
            TypeCtx::Signature
        ),
        "tuple"
    );

    // Fn → object
    assert_eq!(
        cg.map_type(
            &IrType::Fn {
                params: vec![IrType::Int],
                ret: Box::new(IrType::Int),
            },
            TypeCtx::Signature
        ),
        "object"
    );

    // 自定义类型（未在 known_types 中）→ object
    assert_eq!(
        cg.map_type(&IrType::named("UnknownType"), TypeCtx::Signature),
        "object"
    );
}

// ── Ω-spec: cy_struct ──

#[test]
fn cy_omega_gate_struct() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::StructDef(StructDef {
        name: "Point".into(),
        generics: vec![],
        fields: vec![
            Field { name: "x".into(), ty: IrType::F64 },
            Field { name: "y".into(), ty: IrType::F64 },
        ],
        methods: vec![FnDef {
            name: "area".into(),
            generics: vec![],
            params: vec![Param {
                name: "self".into(),
                ty: IrType::Self_,
                is_mut: false,
                is_ref: true,
                is_owned: false,
                default: None,
                variadic: false,
            }],
            ret_ty: IrType::F64,
            body: lang_zone::ir::node::Block {
                stmts: vec![],
                ty: IrType::F64,
                span: lang_zone::ir::node::Span::unknown(),
            },
            intrinsics: vec![],
            is_async: false,
            is_iterator: false,
            is_test: false,
            checker_param: None,
            default_checker: None,
            where_clause: vec![],
            span: lang_zone::ir::node::Span::unknown(),
        }],
        has_new: false,
        new_params: vec![],
        new_ret_ty: None,
        has_init: false,
        init_params: vec![],
        implicit_froms: vec![],
        span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(
        &pyx,
        &[
            "cdef class Point:",
            "cdef public double x",
            "cdef public double y",
            "def __init__(self, double x, double y):",
            "self.x = x",
            "self.y = y",
            "cpdef double area(self)",
        ],
        "struct",
    );
}

// ── Ω-spec: cy_function ──

#[test]
fn cy_omega_gate_function() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "double".into(),
        generics: vec![],
        params: vec![Param {
            name: "x".into(),
            ty: IrType::Int,
            is_mut: false,
            is_ref: false,
            is_owned: false,
            default: None,
            variadic: false,
        }],
        ret_ty: IrType::Int,
        body: lang_zone::ir::node::Block {
            stmts: vec![],
            ty: IrType::Int,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![],
        is_async: false,
        is_iterator: false,
        is_test: false,
        checker_param: None,
        default_checker: None,
        where_clause: vec![],
        span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(
        &pyx,
        &["cpdef Py_ssize_t double(Py_ssize_t x):"],
        "function",
    );
}

// ── Ω-spec: cy_const ──

#[test]
fn cy_omega_gate_const() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::Const(ConstDef {
        name: "MAX".into(),
        ty: IrType::Int,
        value: lang_zone::ir::node::Expr::new(
            lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(100)),
            IrType::Int,
            lang_zone::ir::node::Span::unknown(),
        ),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["Py_ssize_t MAX = 100"], "const");
}

// ── Ω-spec: cy_type_alias ──

#[test]
fn cy_omega_gate_type_alias() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::TypeAlias(TypeAliasDef {
        name: "MyInt".into(),
        generics: vec![],
        ty: IrType::Int,
    }));
    module.items.push(Item::TypeAlias(TypeAliasDef {
        name: "Callback".into(),
        generics: vec![],
        ty: IrType::Fn {
            params: vec![IrType::Int],
            ret: Box::new(IrType::Int),
        },
    }));

    let pyx = gen(module);
    assert_contains(
        &pyx,
        &["# type MyInt = Py_ssize_t", "# type Callback = object"],
        "type_alias",
    );
}

// ── Ω-spec: cy_import ──

#[test]
fn cy_omega_gate_import() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::Use(UseStmt {
        path: vec!["std".into(), "collections".into()],
        alias: None,
        items: vec![],
        is_from: false,
    }));
    module.items.push(Item::Use(UseStmt {
        path: vec!["std".into(), "io".into()],
        alias: None,
        items: vec!["Read".into()],
        is_from: true,
    }));

    let pyx = gen(module);
    assert_contains(
        &pyx,
        &["import std.collections", "from std.io import Read"],
        "import",
    );
}

// ── Ω-spec: cy_enum ──

#[test]
fn cy_omega_gate_enum() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::EnumDef(EnumDef {
        name: "Shape".into(),
        generics: vec![],
        variants: vec![
            Variant {
                name: "Circle".into(),
                fields: vec![Field { name: "r".into(), ty: IrType::F64 }],
            },
            Variant {
                name: "Rect".into(),
                fields: vec![
                    Field { name: "w".into(), ty: IrType::F64 },
                    Field { name: "h".into(), ty: IrType::F64 },
                ],
            },
        ],
        methods: vec![],
        span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(
        &pyx,
        &[
            "class Shape:",
            "pass",
            "class Circle(Shape):",
            "cdef public double r",
            "def __init__(self, double r):",
            "self.r = r",
            "class Rect(Shape):",
            "cdef public double w",
            "cdef public double h",
            "def __init__(self, double w, double h):",
            "self.w = w",
            "self.h = h",
        ],
        "enum",
    );
}

// ── Ω-spec: cy_enum (C-style 无数据) ──

#[test]
fn cy_omega_gate_enum_cstyle() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::EnumDef(EnumDef {
        name: "Color".into(),
        generics: vec![],
        variants: vec![
            Variant { name: "Red".into(), fields: vec![] },
            Variant { name: "Green".into(), fields: vec![] },
            Variant { name: "Blue".into(), fields: vec![] },
        ],
        methods: vec![],
        span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(
        &pyx,
        &[
            "class Color:",
            "pass",
            "class Red(Color):",
            "pass",
            "class Green(Color):",
            "pass",
            "class Blue(Color):",
            "pass",
        ],
        "enum_cstyle",
    );
}

// ── Ω-spec: cy_function 泛型擦除 ──

#[test]
fn cy_omega_gate_function_generic() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "id".into(),
        generics: vec![GenericParam { name: "T".into(), bounds: vec![], default: None }],
        params: vec![Param { name: "x".into(), ty: IrType::Generic("T".into()), is_mut: false, is_ref: false, is_owned: false, default: None, variadic: false }],
        ret_ty: IrType::Generic("T".into()),
        body: lang_zone::ir::node::Block { stmts: vec![], ty: IrType::Generic("T".into()), span: lang_zone::ir::node::Span::unknown() },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["cpdef object id(object x):", "# generic<T>"], "function_generic");
}

// ── Ω-spec: cy_function 变参 ──

#[test]
fn cy_omega_gate_function_variadic() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "sum_all".into(),
        generics: vec![],
        params: vec![
            Param { name: "first".into(), ty: IrType::Int, is_mut: false, is_ref: false, is_owned: false, default: None, variadic: false },
            Param { name: "args".into(), ty: IrType::named("Tuple"), is_mut: false, is_ref: false, is_owned: false, default: None, variadic: true },
        ],
        ret_ty: IrType::Int,
        body: lang_zone::ir::node::Block { stmts: vec![], ty: IrType::Int, span: lang_zone::ir::node::Span::unknown() },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["Py_ssize_t first", "*args"], "function_variadic");
}

// ── 模块魔法属性 ──

#[test]
fn cy_module_magic() {
    let module = IrModule::new("hello".into());
    let pyx = gen(module);
    assert_contains(
        &pyx,
        &[
            "__name__",
            "__file__",
            "__all__",
            "_Moved",
            "_MOVED",
            "_MovedCheck",
            "import cython",
        ],
        "module_magic",
    );
}

// ── 空模块 ──

#[test]
fn cy_empty_module() {
    let module = IrModule::new("empty".into());
    let pyx = gen(module);
    assert_contains(&pyx, &["def main(): pass"], "empty_module");
}

// ── Ω-spec: cy_trait ──

#[test]
fn cy_omega_gate_trait() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::TraitDef(TraitDef {
        name: "HasArea".into(),
        generics: vec![],
        supertraits: vec![],
        methods: vec![FnSig {
            name: "area".into(),
            generics: vec![],
            params: vec![IrType::Self_],
            params_names: vec!["self".into()],
            where_clause: vec![],
            ret: IrType::F64,
            body: None,
        }],
        assoc_types: vec![],
    }));

    let pyx = gen(module);
    assert_contains(
        &pyx,
        &[
            "class HasArea:",
            "\"\"\"Trait: HasArea\"\"\"",
            "def area(self) -> double: ...",
        ],
        "trait",
    );
}

// ── Ω-spec: cy_impl ──

#[test]
fn cy_omega_gate_impl() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::Impl(ImplDef {
        trait_: Some(IrType::named("HasArea")),
        for_type: IrType::named("Circle"),
        generics: vec![],
        methods: vec![FnDef {
            name: "area".into(),
            generics: vec![],
            params: vec![Param {
                name: "self".into(),
                ty: IrType::Self_,
                is_mut: false,
                is_ref: true,
                is_owned: false,
                default: None,
                variadic: false,
            }],
            ret_ty: IrType::F64,
            body: lang_zone::ir::node::Block {
                stmts: vec![],
                ty: IrType::F64,
                span: lang_zone::ir::node::Span::unknown(),
            },
            intrinsics: vec![],
            is_async: false,
            is_iterator: false,
            is_test: false,
            checker_param: None,
            default_checker: None,
            where_clause: vec![],
            span: lang_zone::ir::node::Span::unknown(),
        }],
        assoc_type_bindings: vec![],
        where_clause: vec![],
    }));

    let pyx = gen(module);
    assert_contains(
        &pyx,
        &["# impl HasArea for Circle", "# HasArea.area → 注入到 Circle"],
        "impl",
    );
}

// ── Ω-spec: cy_duck_def ──

#[test]
fn cy_omega_gate_duck_def() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::DuckDef(DuckDef {
        name: "HasArea".into(),
        generics: vec![],
        assoc_types: vec![],
        satisfies: vec![],
        sealed: false,
        match_rules: vec![],
        param_reqs: vec![],
        methods: vec![DuckMethod {
            owner: None,
            name: "area".into(),
            name_pattern: None,
            params: vec![],
            ret_ty: IrType::F64,
            param_range: None,
            is_default: false,
        }],
        fields: vec![DuckField {
            owner: None,
            name: "radius".into(),
            ty: IrType::F64,
            rel: None,
        }],
    }));

    let pyx = gen(module);
    assert_contains(
        &pyx,
        &[
            "class HasArea:",
            "\"\"\"Duck type constraint: HasArea\"\"\"",
            "# def area(...) -> double",
            "# field radius: double",
        ],
        "duck_def",
    );
}

// ── Ω-spec: cy_test ──

#[test]
fn cy_omega_gate_test() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::Test(TestDef {
        name: "basic add".into(),
        body: lang_zone::ir::node::Block {
            stmts: vec![],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["def test_basic_add():"], "test");
}

// ── Ω-spec: cy_stmt_while_let ──

#[test]
fn cy_omega_gate_stmt_while_let() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::WhileLet {
                    pattern: lang_zone::ir::node::Pattern::Ident("x".into()),
                    expr: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Var("items".into()),
                        IrType::named("List"),
                        lang_zone::ir::node::Span::unknown(),
                    ),
                    guard: None,
                    body: lang_zone::ir::node::Block {
                        stmts: vec![
                            lang_zone::ir::node::Stmt::ExprStmt {
                                expr: lang_zone::ir::node::Expr::new(
                                    lang_zone::ir::node::ExprKind::Var("print(x)".into()),
                                    IrType::Unit,
                                    lang_zone::ir::node::Span::unknown(),
                                ),
                            },
                        ],
                        ty: IrType::Unit,
                        span: lang_zone::ir::node::Span::unknown(),
                    },
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["# while let", "for __while_let__ in items:"], "stmt_while_let");
}

// ── Ω-spec: cy_stmt_yield_from ──

#[test]
fn cy_omega_gate_stmt_yield_from() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "gen".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::YieldFrom {
                    iter: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Var("other".into()),
                        IrType::named("Iter"),
                        lang_zone::ir::node::Span::unknown(),
                    ),
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: true, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["yield from other"], "stmt_yield_from");
}

// ── Ω-spec: cy_stmt_pass ──

#[test]
fn cy_omega_gate_stmt_pass() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "noop".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::Pass],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["pass"], "stmt_pass");
}

// ── Ω-spec: cy_stmt_defer ──

#[test]
fn cy_omega_gate_stmt_defer() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::Defer {
                    body: lang_zone::ir::node::Block {
                        stmts: vec![
                            lang_zone::ir::node::Stmt::ExprStmt {
                                expr: lang_zone::ir::node::Expr::new(
                                    lang_zone::ir::node::ExprKind::Var("cleanup()".into()),
                                    IrType::Unit,
                                    lang_zone::ir::node::Span::unknown(),
                                ),
                            },
                        ],
                        ty: IrType::Unit,
                        span: lang_zone::ir::node::Span::unknown(),
                    },
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["try:", "cleanup()"], "stmt_defer");
}

// ── Ω-spec: cy_stmt_try_catch ──

#[test]
fn cy_omega_gate_stmt_try_catch() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::TryCatch {
                    body: lang_zone::ir::node::Block {
                        stmts: vec![
                            lang_zone::ir::node::Stmt::ExprStmt {
                                expr: lang_zone::ir::node::Expr::new(
                                    lang_zone::ir::node::ExprKind::Var("risky()".into()),
                                    IrType::Unit,
                                    lang_zone::ir::node::Span::unknown(),
                                ),
                            },
                        ],
                        ty: IrType::Unit,
                        span: lang_zone::ir::node::Span::unknown(),
                    },
                    catches: vec![
                        (None, lang_zone::ir::node::Block {
                            stmts: vec![
                                lang_zone::ir::node::Stmt::ExprStmt {
                                    expr: lang_zone::ir::node::Expr::new(
                                        lang_zone::ir::node::ExprKind::Var("handle()".into()),
                                        IrType::Unit,
                                        lang_zone::ir::node::Span::unknown(),
                                    ),
                                },
                            ],
                            ty: IrType::Unit,
                            span: lang_zone::ir::node::Span::unknown(),
                        }),
                    ],
                    else_body: None,
                    finally_body: Some(lang_zone::ir::node::Block {
                        stmts: vec![
                            lang_zone::ir::node::Stmt::ExprStmt {
                                expr: lang_zone::ir::node::Expr::new(
                                    lang_zone::ir::node::ExprKind::Var("finalize()".into()),
                                    IrType::Unit,
                                    lang_zone::ir::node::Span::unknown(),
                                ),
                            },
                        ],
                        ty: IrType::Unit,
                        span: lang_zone::ir::node::Span::unknown(),
                    }),
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["try:", "except:", "finally:", "risky()", "handle()", "finalize()"], "stmt_try_catch");
}

// ── Ω-spec: cy_expr_assign ──

#[test]
fn cy_omega_gate_expr_assign() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::ExprStmt {
                    expr: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::AssignExpr {
                            target: Box::new(lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Var("x".into()),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            )),
                            value: Box::new(lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(10)),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            )),
                        },
                        IrType::Unit,
                        lang_zone::ir::node::Span::unknown(),
                    ),
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["x = 10"], "expr_assign");
}

// ── Ω-spec: cy_expr_cast ──

#[test]
fn cy_omega_gate_expr_cast() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::ExprStmt {
                    expr: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Cast {
                            expr: Box::new(lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Var("x".into()),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            )),
                            target: IrType::F64,
                        },
                        IrType::F64,
                        lang_zone::ir::node::Span::unknown(),
                    ),
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["double(x)"], "expr_cast");
}

// ── Ω-spec: cy_expr_magic_call ──

#[test]
fn cy_omega_gate_expr_magic_call() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::ExprStmt {
                    expr: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::MagicCall {
                            kind: lang_zone::ir::node::MagicKind::Display,
                            args: vec![lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Var("x".into()),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            )],
                        },
                        IrType::Str,
                        lang_zone::ir::node::Span::unknown(),
                    ),
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["str(x)"], "expr_magic_call");
}

// ── Ω-spec: cy_expr_tuple_list_dict ──

#[test]
fn cy_omega_gate_expr_collections() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::ExprStmt {
                    expr: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::TupleLit(vec![
                            lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(1)),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            ),
                            lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(2)),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            ),
                        ]),
                        IrType::Tuple(vec![IrType::Int, IrType::Int]),
                        lang_zone::ir::node::Span::unknown(),
                    ),
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["(1, 2)"], "expr_tuple");
}

// ── Ω-spec: cy_expr_range ──

#[test]
fn cy_omega_gate_expr_range() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::ExprStmt {
                    expr: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Range {
                            start: Some(Box::new(lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(0)),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            ))),
                            end: Box::new(lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(10)),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            )),
                            inclusive: false,
                        },
                        IrType::named("Range"),
                        lang_zone::ir::node::Span::unknown(),
                    ),
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["range(0, 10)"], "expr_range");
}

// ── Ω-spec: cy_expr_paren ──

#[test]
fn cy_omega_gate_expr_paren() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::ExprStmt {
                    expr: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Paren(Box::new(lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::BinOp {
                                op: lang_zone::ir::node::BinOpKind::Add,
                                lhs: Box::new(lang_zone::ir::node::Expr::new(
                                    lang_zone::ir::node::ExprKind::Var("a".into()),
                                    IrType::Int,
                                    lang_zone::ir::node::Span::unknown(),
                                )),
                                rhs: Box::new(lang_zone::ir::node::Expr::new(
                                    lang_zone::ir::node::ExprKind::Var("b".into()),
                                    IrType::Int,
                                    lang_zone::ir::node::Span::unknown(),
                                )),
                            },
                            IrType::Int,
                            lang_zone::ir::node::Span::unknown(),
                        ))),
                        IrType::Int,
                        lang_zone::ir::node::Span::unknown(),
                    ),
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["(a + b)"], "expr_paren");
}

// ── Ω-spec: cy_overload (函数重载) ──

#[test]
fn cy_omega_gate_overload() {
    let mut module = IrModule::new("test".into());
    // 第一个重载: process(x: int) -> int
    module.items.push(Item::FnDef(FnDef {
        name: "process".into(),
        generics: vec![],
        params: vec![Param {
            name: "x".into(),
            ty: IrType::Int,
            is_mut: false,
            is_ref: false,
            is_owned: false,
            default: None,
            variadic: false,
        }],
        ret_ty: IrType::Int,
        body: lang_zone::ir::node::Block {
            stmts: vec![],
            ty: IrType::Int,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![],
        is_async: false,
        is_iterator: false,
        is_test: false,
        checker_param: None,
        default_checker: None,
        where_clause: vec![],
        span: lang_zone::ir::node::Span::unknown(),
    }));
    // 第二个重载: process(x: int, y: int) -> int
    module.items.push(Item::FnDef(FnDef {
        name: "process".into(),
        generics: vec![],
        params: vec![
            Param {
                name: "x".into(),
                ty: IrType::Int,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: false,
            },
            Param {
                name: "y".into(),
                ty: IrType::Int,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: false,
            },
        ],
        ret_ty: IrType::Int,
        body: lang_zone::ir::node::Block {
            stmts: vec![],
            ty: IrType::Int,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![],
        is_async: false,
        is_iterator: false,
        is_test: false,
        checker_param: None,
        default_checker: None,
        where_clause: vec![],
        span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(
        &pyx,
        &[
            "cpdef Py_ssize_t process__0(Py_ssize_t x):",
            "cpdef Py_ssize_t process__1(Py_ssize_t x, Py_ssize_t y):",
            "def process(*args):",
            "if len(args) == 1: return process__0(*args)",
            "elif len(args) == 2: return process__1(*args)",
        ],
        "overload",
    );
}

// ── Ω-spec: cy_pattern_wildcard ──

#[test]
fn cy_omega_gate_pattern_wildcard() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::Match {
                    scrutinee: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Var("x".into()),
                        IrType::Int,
                        lang_zone::ir::node::Span::unknown(),
                    ),
                    arms: vec![
                        lang_zone::ir::node::MatchArm {
                            pattern: lang_zone::ir::node::Pattern::Wildcard,
                            guard: None,
                            body: lang_zone::ir::node::Block {
                                stmts: vec![
                                    lang_zone::ir::node::Stmt::ExprStmt {
                                        expr: lang_zone::ir::node::Expr::new(
                                            lang_zone::ir::node::ExprKind::Var("42".into()),
                                            IrType::Int,
                                            lang_zone::ir::node::Span::unknown(),
                                        ),
                                    },
                                ],
                                ty: IrType::Int,
                                span: lang_zone::ir::node::Span::unknown(),
                            },
                        },
                    ],
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["# match x", "if True:"], "pattern_wildcard");
}

// ── Ω-spec: cy_pattern_ident ──

#[test]
fn cy_omega_gate_pattern_ident() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::Match {
                    scrutinee: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Var("x".into()),
                        IrType::Int,
                        lang_zone::ir::node::Span::unknown(),
                    ),
                    arms: vec![
                        lang_zone::ir::node::MatchArm {
                            pattern: lang_zone::ir::node::Pattern::Ident("n".into()),
                            guard: None,
                            body: lang_zone::ir::node::Block {
                                stmts: vec![
                                    lang_zone::ir::node::Stmt::ExprStmt {
                                        expr: lang_zone::ir::node::Expr::new(
                                            lang_zone::ir::node::ExprKind::Var("n".into()),
                                            IrType::Int,
                                            lang_zone::ir::node::Span::unknown(),
                                        ),
                                    },
                                ],
                                ty: IrType::Int,
                                span: lang_zone::ir::node::Span::unknown(),
                            },
                        },
                    ],
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["# match x", "if True  # bind n:", "n = __scrutinee__"], "pattern_ident");
}

// ── Ω-spec: cy_pattern_lit ──

#[test]
fn cy_omega_gate_pattern_lit() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::Match {
                    scrutinee: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Var("x".into()),
                        IrType::Int,
                        lang_zone::ir::node::Span::unknown(),
                    ),
                    arms: vec![
                        lang_zone::ir::node::MatchArm {
                            pattern: lang_zone::ir::node::Pattern::Lit(lang_zone::ir::node::LitKind::Int(0)),
                            guard: None,
                            body: lang_zone::ir::node::Block {
                                stmts: vec![
                                    lang_zone::ir::node::Stmt::ExprStmt {
                                        expr: lang_zone::ir::node::Expr::new(
                                            lang_zone::ir::node::ExprKind::Var("zero".into()),
                                            IrType::Int,
                                            lang_zone::ir::node::Span::unknown(),
                                        ),
                                    },
                                ],
                                ty: IrType::Int,
                                span: lang_zone::ir::node::Span::unknown(),
                            },
                        },
                    ],
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["# match x", "if __scrutinee__ == 0:"], "pattern_lit");
}

// ── Ω-spec: cy_pattern_tuple ──

#[test]
fn cy_omega_gate_pattern_tuple() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::Match {
                    scrutinee: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Var("x".into()),
                        IrType::named("Tuple"),
                        lang_zone::ir::node::Span::unknown(),
                    ),
                    arms: vec![
                        lang_zone::ir::node::MatchArm {
                            pattern: lang_zone::ir::node::Pattern::Tuple(vec![
                                lang_zone::ir::node::Pattern::Ident("a".into()),
                                lang_zone::ir::node::Pattern::Ident("b".into()),
                            ]),
                            guard: None,
                            body: lang_zone::ir::node::Block {
                                stmts: vec![
                                    lang_zone::ir::node::Stmt::ExprStmt {
                                        expr: lang_zone::ir::node::Expr::new(
                                            lang_zone::ir::node::ExprKind::Var("a".into()),
                                            IrType::Int,
                                            lang_zone::ir::node::Span::unknown(),
                                        ),
                                    },
                                ],
                                ty: IrType::Int,
                                span: lang_zone::ir::node::Span::unknown(),
                            },
                        },
                    ],
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["# match x", "isinstance(__scrutinee__, tuple) && len(__scrutinee__) == 2"], "pattern_tuple");
}

// ── Ω-spec: cy_pattern_list ──

#[test]
fn cy_omega_gate_pattern_list() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::Match {
                    scrutinee: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Var("x".into()),
                        IrType::named("List"),
                        lang_zone::ir::node::Span::unknown(),
                    ),
                    arms: vec![
                        lang_zone::ir::node::MatchArm {
                            pattern: lang_zone::ir::node::Pattern::List(vec![
                                lang_zone::ir::node::Pattern::Ident("first".into()),
                                lang_zone::ir::node::Pattern::Rest(Some("rest".into())),
                            ]),
                            guard: None,
                            body: lang_zone::ir::node::Block {
                                stmts: vec![
                                    lang_zone::ir::node::Stmt::ExprStmt {
                                        expr: lang_zone::ir::node::Expr::new(
                                            lang_zone::ir::node::ExprKind::Var("first".into()),
                                            IrType::Int,
                                            lang_zone::ir::node::Span::unknown(),
                                        ),
                                    },
                                ],
                                ty: IrType::Int,
                                span: lang_zone::ir::node::Span::unknown(),
                            },
                        },
                    ],
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["# match x", "isinstance(__scrutinee__, list) && len(__scrutinee__) == 2"], "pattern_list");
}

// ── Ω-spec: cy_pattern_range ──

#[test]
fn cy_omega_gate_pattern_range() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                lang_zone::ir::node::Stmt::Match {
                    scrutinee: lang_zone::ir::node::Expr::new(
                        lang_zone::ir::node::ExprKind::Var("x".into()),
                        IrType::Int,
                        lang_zone::ir::node::Span::unknown(),
                    ),
                    arms: vec![
                        lang_zone::ir::node::MatchArm {
                            pattern: lang_zone::ir::node::Pattern::Range {
                                start: 1,
                                end: 10,
                                inclusive: true,
                            },
                            guard: None,
                            body: lang_zone::ir::node::Block {
                                stmts: vec![
                                    lang_zone::ir::node::Stmt::ExprStmt {
                                        expr: lang_zone::ir::node::Expr::new(
                                            lang_zone::ir::node::ExprKind::Var("in_range".into()),
                                            IrType::Int,
                                            lang_zone::ir::node::Span::unknown(),
                                        ),
                                    },
                                ],
                                ty: IrType::Int,
                                span: lang_zone::ir::node::Span::unknown(),
                            },
                        },
                    ],
                },
            ],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![], is_async: false, is_iterator: false, is_test: false,
        checker_param: None, default_checker: None, where_clause: vec![], span: lang_zone::ir::node::Span::unknown(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["# match x", "1 <= __scrutinee__ <= 10"], "pattern_range");
}

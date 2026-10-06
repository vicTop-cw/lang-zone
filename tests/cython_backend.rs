// Lang-Zone 编译器 — tests/cython_backend.rs
// Cython 后端测试套件：验证 LZIR → Cython 代码生成
//
// Ω-spec 验证基准：CY/corpus/cy_*.json
// 测试方式：直接构造 IrModule + 调用 CythonCodeGen::generate()，不依赖 CLI 路由

use lang_zone::ir::codegen_cython::{CythonCodeGen, TypeCtx};
use lang_zone::ir::node::{
    ConstDef, EnumDef, Field, FnDef, GenericParam, IrMods, Item, Param, StructDef, TypeAliasDef,
    UseStmt, Variant,
};
use lang_zone::ir::node::{DuckDef, DuckField, DuckMethod, FnSig, ImplDef, TestDef, TraitDef};
use lang_zone::ir::types::IrType;
use lang_zone::ir::IrModule;

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
        cg.map_type(
            &IrType::named_with("List", vec![IrType::Int]),
            TypeCtx::Signature
        ),
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
        cg.map_type(
            &IrType::named_with("Set", vec![IrType::Int]),
            TypeCtx::Signature
        ),
        "set"
    );

    // 智能指针 → object
    assert_eq!(
        cg.map_type(
            &IrType::named_with("Box", vec![IrType::Int]),
            TypeCtx::Signature
        ),
        "object"
    );
    assert_eq!(
        cg.map_type(
            &IrType::named_with("Rc", vec![IrType::Int]),
            TypeCtx::Signature
        ),
        "object"
    );
    assert_eq!(
        cg.map_type(
            &IrType::named_with("Arc", vec![IrType::Int]),
            TypeCtx::Signature
        ),
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
            Field {
                name: "x".into(),
                ty: IrType::F64,
            },
            Field {
                name: "y".into(),
                ty: IrType::F64,
            },
        ],
        derives: vec![],methods: vec![FnDef {
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
                comptime: false,
                mods: IrMods::default(),
            }],
            ret_ty: IrType::F64,
            raises: None,
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
        new_body: None,
        has_init: false,
        init_params: vec![],
        init_body: None,
        implicit_froms: vec![],
        is_case: false,
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
            "def area(self):  # ret: double",
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
            comptime: false,
            mods: IrMods::default(),
        }],
        ret_ty: IrType::Int,
        raises: None,
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
        &["def double(Py_ssize_t x):  # ret: Py_ssize_t"],
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
        mods: IrMods::default(),
    }));

    let pyx = gen(module);
    assert_contains(&pyx, &["MAX = 100  # const: Py_ssize_t"], "const");
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
        derives: vec![],
        variants: vec![
            Variant {
                name: "Circle".into(),
                fields: vec![Field {
                    name: "r".into(),
                    ty: IrType::F64,
                }],
            },
            Variant {
                name: "Rect".into(),
                fields: vec![
                    Field {
                        name: "w".into(),
                        ty: IrType::F64,
                    },
                    Field {
                        name: "h".into(),
                        ty: IrType::F64,
                    },
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
            "def __init__(self, r):",
            "self.r = r",
            "class Rect(Shape):",
            "def __init__(self, w, h):",
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
        derives: vec![],
        variants: vec![
            Variant {
                name: "Red".into(),
                fields: vec![],
            },
            Variant {
                name: "Green".into(),
                fields: vec![],
            },
            Variant {
                name: "Blue".into(),
                fields: vec![],
            },
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
        generics: vec![GenericParam {
            name: "T".into(),
            bounds: vec![],
            default: None,
        }],
        params: vec![Param {
            name: "x".into(),
            ty: IrType::Generic("T".into()),
            is_mut: false,
            is_ref: false,
            is_owned: false,
            default: None,
            variadic: false,
            comptime: false,
            mods: IrMods::default(),
        }],
        ret_ty: IrType::Generic("T".into()),
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![],
            ty: IrType::Generic("T".into()),
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
        &["def id(object x):  # ret: object", "# generic<T>"],
        "function_generic",
    );
}

// ── Ω-spec: cy_function 变参 ──

#[test]
fn cy_omega_gate_function_variadic() {
    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "sum_all".into(),
        generics: vec![],
        params: vec![
            Param {
                name: "first".into(),
                ty: IrType::Int,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: false,
                comptime: false,
                mods: IrMods::default(),
            },
            Param {
                name: "args".into(),
                ty: IrType::named("Tuple"),
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: true,
                comptime: false,
                mods: IrMods::default(),
            },
        ],
        ret_ty: IrType::Int,
        raises: None,
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
    assert_contains(&pyx, &["def main():", "    pass"], "empty_module");
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
            "def area(self):  # ret: double",
            "    ...",
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
                comptime: false,
                mods: IrMods::default(),
            }],
            ret_ty: IrType::F64,
            raises: None,
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
        &[
            "# impl HasArea for Circle (std target: methods not injectable)",
            "#   fn area(self)",
        ],
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::WhileLet {
                pattern: lang_zone::ir::node::Pattern::Ident("x".into()),
                expr: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::Var("items".into()),
                    IrType::named("List"),
                    lang_zone::ir::node::Span::unknown(),
                ),
                guard: None,
                body: lang_zone::ir::node::Block {
                    stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                        expr: lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::Var("print(x)".into()),
                            IrType::Unit,
                            lang_zone::ir::node::Span::unknown(),
                        ),
                    }],
                    ty: IrType::Unit,
                    span: lang_zone::ir::node::Span::unknown(),
                },
            }],
            ty: IrType::Unit,
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
        &["while True:", "__wlv_0 = items", "if not (True):", "break"],
        "stmt_while_let",
    );
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::YieldFrom {
                iter: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::Var("other".into()),
                    IrType::named("Iter"),
                    lang_zone::ir::node::Span::unknown(),
                ),
            }],
            ty: IrType::Unit,
            span: lang_zone::ir::node::Span::unknown(),
        },
        intrinsics: vec![],
        is_async: false,
        is_iterator: true,
        is_test: false,
        checker_param: None,
        default_checker: None,
        where_clause: vec![],
        span: lang_zone::ir::node::Span::unknown(),
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::Pass],
            ty: IrType::Unit,
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::Defer {
                body: lang_zone::ir::node::Block {
                    stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                        expr: lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::Var("cleanup()".into()),
                            IrType::Unit,
                            lang_zone::ir::node::Span::unknown(),
                        ),
                    }],
                    ty: IrType::Unit,
                    span: lang_zone::ir::node::Span::unknown(),
                },
            }],
            ty: IrType::Unit,
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
    // Defer：块级 LIFO 内联（对齐 Rust 端 BUG-IR-002 方案 A）——defer 体在
    // 块退出前逆序 emit，非 try/finally
    assert_contains(&pyx, &["cleanup()"], "stmt_defer");
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::TryCatch {
                body: lang_zone::ir::node::Block {
                    stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                        expr: lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::Var("risky()".into()),
                            IrType::Unit,
                            lang_zone::ir::node::Span::unknown(),
                        ),
                    }],
                    ty: IrType::Unit,
                    span: lang_zone::ir::node::Span::unknown(),
                },
                catches: vec![(
                    None,
                    lang_zone::ir::node::Block {
                        stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                            expr: lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Var("handle()".into()),
                                IrType::Unit,
                                lang_zone::ir::node::Span::unknown(),
                            ),
                        }],
                        ty: IrType::Unit,
                        span: lang_zone::ir::node::Span::unknown(),
                    },
                )],
                else_body: None,
                finally_body: Some(lang_zone::ir::node::Block {
                    stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                        expr: lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::Var("finalize()".into()),
                            IrType::Unit,
                            lang_zone::ir::node::Span::unknown(),
                        ),
                    }],
                    ty: IrType::Unit,
                    span: lang_zone::ir::node::Span::unknown(),
                }),
            }],
            ty: IrType::Unit,
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
            "try:",
            "except BaseException:",
            "finally:",
            "risky()",
            "handle()",
            "finalize()",
        ],
        "stmt_try_catch",
    );
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                expr: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::AssignExpr {
                        target: Box::new(lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::Var("x".into()),
                            IrType::Int,
                            lang_zone::ir::node::Span::unknown(),
                        )),
                        value: Box::new(lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(
                                10,
                            )),
                            IrType::Int,
                            lang_zone::ir::node::Span::unknown(),
                        )),
                    },
                    IrType::Unit,
                    lang_zone::ir::node::Span::unknown(),
                ),
            }],
            ty: IrType::Unit,
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
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
            }],
            ty: IrType::Unit,
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
    // Cast：内建标量 → Python 内建转换函数（int/float/str/bool）
    assert_contains(&pyx, &["float(x)"], "expr_cast");
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
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
            }],
            ty: IrType::Unit,
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                expr: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::TupleLit(vec![
                        lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(
                                1,
                            )),
                            IrType::Int,
                            lang_zone::ir::node::Span::unknown(),
                        ),
                        lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(
                                2,
                            )),
                            IrType::Int,
                            lang_zone::ir::node::Span::unknown(),
                        ),
                    ]),
                    IrType::Tuple(vec![IrType::Int, IrType::Int]),
                    lang_zone::ir::node::Span::unknown(),
                ),
            }],
            ty: IrType::Unit,
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                expr: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::Range {
                        start: Some(Box::new(lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(
                                0,
                            )),
                            IrType::Int,
                            lang_zone::ir::node::Span::unknown(),
                        ))),
                        end: Box::new(lang_zone::ir::node::Expr::new(
                            lang_zone::ir::node::ExprKind::Lit(lang_zone::ir::node::LitKind::Int(
                                10,
                            )),
                            IrType::Int,
                            lang_zone::ir::node::Span::unknown(),
                        )),
                        inclusive: false,
                    },
                    IrType::named("Range"),
                    lang_zone::ir::node::Span::unknown(),
                ),
            }],
            ty: IrType::Unit,
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
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
            }],
            ty: IrType::Unit,
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
            comptime: false,
            mods: IrMods::default(),
        }],
        ret_ty: IrType::Int,
        raises: None,
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
                comptime: false,
                mods: IrMods::default(),
            },
            Param {
                name: "y".into(),
                ty: IrType::Int,
                is_mut: false,
                is_ref: false,
                is_owned: false,
                default: None,
                variadic: false,
                comptime: false,
                mods: IrMods::default(),
            },
        ],
        ret_ty: IrType::Int,
        raises: None,
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
            "def process__0(Py_ssize_t x):  # ret: Py_ssize_t",
            "def process__1(Py_ssize_t x, Py_ssize_t y):  # ret: Py_ssize_t",
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::Match {
                scrutinee: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::Var("x".into()),
                    IrType::Int,
                    lang_zone::ir::node::Span::unknown(),
                ),
                arms: vec![lang_zone::ir::node::MatchArm {
                    pattern: lang_zone::ir::node::Pattern::Wildcard,
                    guard: None,
                    body: lang_zone::ir::node::Block {
                        stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                            expr: lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Var("42".into()),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            ),
                        }],
                        ty: IrType::Int,
                        span: lang_zone::ir::node::Span::unknown(),
                    },
                }],
            }],
            ty: IrType::Unit,
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
    assert_contains(&pyx, &["__scrut_0 = x", "if True:"], "pattern_wildcard");
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::Match {
                scrutinee: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::Var("x".into()),
                    IrType::Int,
                    lang_zone::ir::node::Span::unknown(),
                ),
                arms: vec![lang_zone::ir::node::MatchArm {
                    pattern: lang_zone::ir::node::Pattern::Ident("n".into()),
                    guard: None,
                    body: lang_zone::ir::node::Block {
                        stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                            expr: lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Var("n".into()),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            ),
                        }],
                        ty: IrType::Int,
                        span: lang_zone::ir::node::Span::unknown(),
                    },
                }],
            }],
            ty: IrType::Unit,
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
        &["__scrut_0 = x", "if True:", "n = __scrut_0"],
        "pattern_ident",
    );
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::Match {
                scrutinee: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::Var("x".into()),
                    IrType::Int,
                    lang_zone::ir::node::Span::unknown(),
                ),
                arms: vec![lang_zone::ir::node::MatchArm {
                    pattern: lang_zone::ir::node::Pattern::Lit(lang_zone::ir::node::LitKind::Int(
                        0,
                    )),
                    guard: None,
                    body: lang_zone::ir::node::Block {
                        stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                            expr: lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Var("zero".into()),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            ),
                        }],
                        ty: IrType::Int,
                        span: lang_zone::ir::node::Span::unknown(),
                    },
                }],
            }],
            ty: IrType::Unit,
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
        &["__scrut_0 = x", "if __scrut_0 == 0:"],
        "pattern_lit",
    );
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::Match {
                scrutinee: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::Var("x".into()),
                    IrType::named("Tuple"),
                    lang_zone::ir::node::Span::unknown(),
                ),
                arms: vec![lang_zone::ir::node::MatchArm {
                    pattern: lang_zone::ir::node::Pattern::Tuple(vec![
                        lang_zone::ir::node::Pattern::Ident("a".into()),
                        lang_zone::ir::node::Pattern::Ident("b".into()),
                    ]),
                    guard: None,
                    body: lang_zone::ir::node::Block {
                        stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                            expr: lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Var("a".into()),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            ),
                        }],
                        ty: IrType::Int,
                        span: lang_zone::ir::node::Span::unknown(),
                    },
                }],
            }],
            ty: IrType::Unit,
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
            "isinstance(__scrut_0, tuple) and len(__scrut_0) == 2",
        ],
        "pattern_tuple",
    );
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::Match {
                scrutinee: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::Var("x".into()),
                    IrType::named("List"),
                    lang_zone::ir::node::Span::unknown(),
                ),
                arms: vec![lang_zone::ir::node::MatchArm {
                    pattern: lang_zone::ir::node::Pattern::List(vec![
                        lang_zone::ir::node::Pattern::Ident("first".into()),
                        lang_zone::ir::node::Pattern::Rest(Some("rest".into())),
                    ]),
                    guard: None,
                    body: lang_zone::ir::node::Block {
                        stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                            expr: lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Var("first".into()),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            ),
                        }],
                        ty: IrType::Int,
                        span: lang_zone::ir::node::Span::unknown(),
                    },
                }],
            }],
            ty: IrType::Unit,
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
            "isinstance(__scrut_0, list) and len(__scrut_0) >= 1",
        ],
        "pattern_list",
    );
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
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![lang_zone::ir::node::Stmt::Match {
                scrutinee: lang_zone::ir::node::Expr::new(
                    lang_zone::ir::node::ExprKind::Var("x".into()),
                    IrType::Int,
                    lang_zone::ir::node::Span::unknown(),
                ),
                arms: vec![lang_zone::ir::node::MatchArm {
                    pattern: lang_zone::ir::node::Pattern::Range {
                        start: 1,
                        end: 10,
                        inclusive: true,
                    },
                    guard: None,
                    body: lang_zone::ir::node::Block {
                        stmts: vec![lang_zone::ir::node::Stmt::ExprStmt {
                            expr: lang_zone::ir::node::Expr::new(
                                lang_zone::ir::node::ExprKind::Var("in_range".into()),
                                IrType::Int,
                                lang_zone::ir::node::Span::unknown(),
                            ),
                        }],
                        ty: IrType::Int,
                        span: lang_zone::ir::node::Span::unknown(),
                    },
                }],
            }],
            ty: IrType::Unit,
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
        &["1 <= __scrut_0 <= 10"],
        "pattern_range",
    );
}

// ── 返回类型标注：C 类型名不上签名（台账 BUG-13）──
//
// 实测依据（Cython 3.2.2，`python -m cython -3`，探针 TEMP/annprobe/）：
// · `-> Py_ssize_t`／`-> double`／`-> bint` ⇒ `warning: Unknown type declaration
//   ... in annotation, ignoring`（a_pyssize / h_double / k_bint 三件都只告警不生效）；
// · `-> int` 却是**真做类型检查**：`def f() -> int: return "x"` 直接编译失败
//   （g_int_wrong.pyx）⇒ 换成 Python 内置名等于给语言没承诺的位置加类型义务。
// 本后端的函数一律发 `def`（`fn_decl` 只有 def/async def，没有 cdef/cpdef），
// 所以发不出「既承重又无害」的返回标注 ⇒ 不写，返回类型改由行尾 `# ret:` 承载。
// `raises` 仍写 `-> object`：实测无告警，且 object 对 Cython 就是「任意 Python
// 对象」，不新增义务。语料面同款判据在 tests/cy_codegen_gate.rs 的 L1 扫描里。
#[test]
fn cy_return_annotation_policy() {
    let mk = |raises: Option<IrType>, ret: IrType| {
        let mut m = IrModule::new("ann".into());
        m.items.push(Item::FnDef(FnDef {
            name: "f".into(),
            generics: vec![],
            params: vec![],
            ret_ty: ret,
            raises,
            body: lang_zone::ir::node::Block {
                stmts: vec![],
                ty: IrType::Unit,
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
        m
    };

    for (label, module) in [
        ("int", mk(None, IrType::Int)),
        ("f64", mk(None, IrType::F64)),
        ("bool", mk(None, IrType::Bool)),
    ] {
        let pyx = gen(module);
        for banned in ["-> Py_ssize_t", "-> double", "-> bint", "-> float"] {
            assert!(
                !pyx.contains(banned),
                "[{label}] 签名里出现了 C 类型返回注解 '{banned}'，实际输出:
{pyx}"
            );
        }
    }
    // 承载还在：读 .pyx 的人看得见 LZ 源码里写的返回类型
    let pyx = gen(mk(None, IrType::Int));
    assert_contains(&pyx, &["def f():  # ret: Py_ssize_t"], "ret_note 承载");

    // raises → `-> object`（唯一保留的返回标注）
    let pyx = gen(mk(Some(IrType::Str), IrType::Int));
    assert_contains(&pyx, &["def f() -> object:"], "raises 走 object");
}

#[test]
fn cy_test_runner_emission_is_flag_gated() {
    // BUG-18 的**发射侧**判据，两侧都要咬：
    //   `--test` 打开 ⇒ 发运行器并按**声明顺序**登记 test 名；
    //   关闭 ⇒ 一行都不发——否则语料 L3 的 stdout 会被测试行污染，跨后端 golden 立刻分叉。
    // 只测「开了有」不测「关了没有」，就等着哪天有人把运行器恒发然后拿 golden 去迁就它。
    let mk = || {
        let mut module = IrModule::new("runner_case".into());
        for name in ["alpha", "beta"] {
            module.items.push(Item::Test(TestDef {
                name: name.into(),
                body: lang_zone::ir::node::Block {
                    stmts: vec![],
                    ty: IrType::Unit,
                    span: lang_zone::ir::node::Span::unknown(),
                },
            }));
        }
        module
    };

    let mut cg = CythonCodeGen::new();
    cg.set_test_runner(true);
    let on = cg.generate(&mk()).to_string();
    assert_contains(
        &on,
        &[
            "def _lz_run_tests():",
            "        (\"alpha\", test_alpha),",
            "        (\"beta\", test_beta),",
            "print(\"running %d tests\" % len(_lz_cases))",
            "test result: ok. %d passed; 0 failed",
            "if __name__ == \"__main__\":",
        ],
        "test_runner on",
    );

    let off = gen(mk());
    for banned in ["_lz_run_tests", "__main__", "running %d tests"] {
        assert!(
            !off.contains(banned),
            "[test_runner off] 纯转译产物不该出现 '{banned}'，实际输出:\n{off}"
        );
    }
}

/// 回归锁：BlockExpr 语句提升时**尾表达式只执行一次，前缀语句一条都不丢**。
/// 破坏形态（2026-10-06 实测，语料侧证据 `TEMP/gates-cy-1649.log`）：
/// builder 把 `with` 脱糖成 `let __with_val_<n> = { 体语句; 尾值 }`（尾值捕获，
/// 为了让 `with` 出现在值位置时不返回 ()），而 cy 的 9 个提升点都是
/// `gen_block(block)`（连尾语句一起发）＋ 再发一遍 `block_tail_expr(block)`
/// ⇒ `with` 体里最后一条 print 在产物里出现两次 ⇒ L3 逐字比对判红
/// （`15_feature_matrix/with_defer.lz`：`"using database"` 与
/// `"inside standalone with"` 各多打一行）。
#[test]
fn cy_block_expr_tail_runs_once() {
    use lang_zone::ir::node::{Expr, ExprKind, LitKind, Stmt};
    let sp = lang_zone::ir::node::Span::unknown;
    let print_stmt = |s: &str| Stmt::ExprStmt {
        expr: Expr::new(
            ExprKind::Call {
                callee: Box::new(Expr::new(ExprKind::Var("print".into()), IrType::Unit, sp())),
                args: vec![Expr::new(
                    ExprKind::Lit(LitKind::Str(s.into())),
                    IrType::Str,
                    sp(),
                )],
                type_args: vec![],
            },
            IrType::Unit,
            sp(),
        ),
    };

    let body_val = Expr::new(
        ExprKind::BlockExpr {
            block: lang_zone::ir::node::Block {
                stmts: vec![print_stmt("PREFIX_ONCE"), print_stmt("TAIL_ONCE")],
                ty: IrType::Any,
                span: sp(),
            },
        },
        IrType::Any,
        sp(),
    );

    let mut module = IrModule::new("test".into());
    module.items.push(Item::FnDef(FnDef {
        name: "demo".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Any,
        raises: None,
        body: lang_zone::ir::node::Block {
            stmts: vec![
                Stmt::Let {
                    name: "res".into(),
                    ty: IrType::Any,
                    value: Expr::new(ExprKind::Var("acquire".into()), IrType::Any, sp()),
                    is_mut: true,
                    is_ref: false,
                    mods: IrMods::default(),
                },
                Stmt::Let {
                    name: "__with_val_res".into(),
                    ty: IrType::Any,
                    value: body_val,
                    is_mut: false,
                    is_ref: false,
                    mods: IrMods::default(),
                },
                // builder 收尾把临时变量放在语句位（`with` 的块值）
                Stmt::ExprStmt {
                    expr: Expr::new(
                        ExprKind::Var("__with_val_res".into()),
                        IrType::Any,
                        sp(),
                    ),
                },
            ],
            ty: IrType::Any,
            span: sp(),
        },
        intrinsics: vec![],
        is_async: false,
        is_iterator: false,
        is_test: false,
        checker_param: None,
        default_checker: None,
        where_clause: vec![],
        span: sp(),
    }));

    let pyx = gen(module);
    // 尾语句只跑一次：绑定它就是那次执行，不该再有第二条裸 print
    assert_eq!(
        pyx.matches("PREFIX_ONCE").count(),
        1,
        "[block 提升] 前缀语句应恰好 1 次（丢了=值错，重了=副作用重复），实际输出:\n{pyx}"
    );
    assert_eq!(
        pyx.matches("TAIL_ONCE").count(),
        1,
        "[block 提升] 尾语句应恰好 1 次，实际输出:\n{pyx}"
    );
    assert_contains(
        &pyx,
        &["__with_val_res = print(_lz_dbg(\"TAIL_ONCE\"))"],
        "block 提升绑定的是尾值",
    );
}

/// BUG-12 的发射侧分派判据，**两侧都要咬**（只测一边等于没测）：
///   同步 `def` 里的 `go 体` → `__spawn(lambda: 体)`（体交给线程，主流程不替它跑）；
///   `async def` 里 → 保持 `__go(体)`（调用 async def 只创建协程、不执行体，透传是正确语义，
///   跨后端实测一致的那件语料是 `CY/TESTS/15_feature_matrix/async_spawn.lz`）。
/// 为什么不是「有 `__go(` 就红」：垫片本身定义 `def __go(v)`，全量产物里恒出现，
/// 那样写会让闸门永远咬不住（见 memory「计数命中的是自己刚写的注释」同型）。
#[test]
fn cy_go_dispatch_is_async_gated() {
    use lang_zone::ir::node::{Expr, ExprKind, Stmt};
    let sp = lang_zone::ir::node::Span::unknown;
    let go_body = Stmt::ExprStmt {
        expr: Expr::new(
            ExprKind::Call {
                callee: Box::new(Expr::new(ExprKind::Var("__go".into()), IrType::Any, sp())),
                args: vec![Expr::new(
                    ExprKind::Call {
                        callee: Box::new(Expr::new(ExprKind::Var("boom".into()), IrType::Any, sp())),
                        args: vec![],
                        type_args: vec![],
                    },
                    IrType::Any,
                    sp(),
                )],
                type_args: vec![],
            },
            IrType::Any,
            sp(),
        ),
    };
    let mk = |is_async: bool| {
        let mut module = IrModule::new("test".into());
        module.items.push(Item::FnDef(FnDef {
            name: "main".into(),
            generics: vec![],
            params: vec![],
            ret_ty: IrType::Unit,
            raises: None,
            body: lang_zone::ir::node::Block {
                stmts: vec![go_body.clone()],
                ty: IrType::Unit,
                span: sp(),
            },
            intrinsics: vec![],
            is_async,
            is_iterator: false,
            is_test: false,
            checker_param: None,
            default_checker: None,
            where_clause: vec![],
            span: sp(),
        }));
        gen(module)
    };

    let sync = mk(false);
    assert_contains(&sync, &["__spawn(lambda: boom())"], "sync 上下文走线程");
    assert!(
        !sync.contains("__go(boom())"),
        "[sync 上下文] 不该再把体交给先求值的 `__go(`（那等于同步降级），实际输出:\n{sync}"
    );

    let a_sync = mk(true);
    assert_contains(&a_sync, &["__go(boom())"], "async 上下文走协程");
    assert!(
        !a_sync.contains("__spawn(lambda: boom())"),
        "[async 上下文] 不该把协程再丢进线程，实际输出:\n{a_sync}"
    );
}

// ── 闭包（SYNTAX/03e）：块体必须 hoist 成 def；写捕获要声明；单表达式不许被顺手改掉 ──

/// 两块共用的 IR 小工具（放在函数里，避免和上面手搓 IR 的用例互相污染）
fn closure_fixtures() -> (
    Box<dyn Fn(&str) -> lang_zone::ir::node::Expr>,
    Box<dyn Fn(i64) -> lang_zone::ir::node::Expr>,
) {
    use lang_zone::ir::node::{Expr, ExprKind, LitKind, Span};
    (
        Box::new(|n: &str| Expr::new(ExprKind::Var(n.into()), IrType::Any, Span::unknown())),
        Box::new(|n: i64| {
            Expr::new(ExprKind::Lit(LitKind::Int(n)), IrType::Int, Span::unknown())
        }),
    )
}

#[test]
fn cy_closure_block_body_is_hoisted() {
    use lang_zone::ir::node::{BinOpKind, Block, Expr, ExprKind, Span, Stmt as IrStmt};
    let (var, lit) = closure_fixtures();
    let sp = Span::unknown;
    let param = |n: &str| Param {
        name: n.into(),
        ty: IrType::Int,
        is_mut: false,
        is_ref: false,
        is_owned: false,
        default: None,
        variadic: false,
        comptime: false,
        mods: IrMods::default(),
    };
    let bin = |l: Expr, op: BinOpKind, r: Expr| {
        Expr::new(
            ExprKind::BinOp { lhs: Box::new(l), op, rhs: Box::new(r) },
            IrType::Int,
            sp(),
        )
    };

    // 块体闭包：`|a, b| =>` { c = a + b; c * c }（探针 p_modlevel 的形状）
    let block_body = Expr::new(
        ExprKind::BlockExpr {
            block: Block {
                stmts: vec![
                    IrStmt::Let {
                        name: "c".into(),
                        ty: IrType::Int,
                        value: bin(var("a"), BinOpKind::Add, var("b")),
                        is_mut: false,
                        is_ref: false,
                        mods: IrMods::default(),
                    },
                    IrStmt::ExprStmt {
                        expr: bin(var("c"), BinOpKind::Mul, var("c")),
                    },
                ],
                ty: IrType::Int,
                span: sp(),
            },
        },
        IrType::Int,
        sp(),
    );
    let mut module = IrModule::new("closure_hoist".into());
    module.items.push(Item::Const(ConstDef {
        name: "sq".into(),
        ty: IrType::Any,
        value: Expr::new(
            ExprKind::Lambda {
                params: vec![param("a"), param("b")],
                body: Box::new(block_body),
                is_move: false,
                ret_ty: None,
            },
            IrType::Any,
            sp(),
        ),
        mods: IrMods::default(),
    }));
    // 成对的一支：单表达式闭包**必须仍然是 lambda**（hoist 不是无条件替换）
    module.items.push(Item::Const(ConstDef {
        name: "dbl".into(),
        ty: IrType::Any,
        value: Expr::new(
            ExprKind::Lambda {
                params: vec![param("x")],
                body: Box::new(bin(var("x"), BinOpKind::Mul, lit(2))),
                is_move: false,
                ret_ty: None,
            },
            IrType::Any,
            sp(),
        ),
        mods: IrMods::default(),
    }));
    let out = gen(module);

    assert_contains(&out, &["def __lambda_0(a, b):"], "块体闭包 hoist 成 def");
    assert_contains(&out, &["return c * c"], "块体闭包的尾值转 return");
    assert!(
        out.contains("c = a + b"),
        "块体闭包的前缀语句必须留在产物里（丢掉就是静默少副作用），实际输出:\n{out}"
    );
    assert!(
        !out.contains("lambda a, b:"),
        "块体闭包不该发成 `lambda a, b: <尾>`——那是丢掉 `c = a + b` 的降级形态，实际输出:\n{out}"
    );
    assert_contains(&out, &["dbl = lambda x: x * 2"], "单表达式闭包仍走 lambda");
    assert!(
        !out.contains("nonlocal c"),
        "块内新绑定的 `c` 不是外层变量，不该声明 nonlocal，实际输出:\n{out}"
    );
}

#[test]
fn cy_closure_write_capture_decls() {
    use lang_zone::ir::node::{BinOpKind, Block, Expr, ExprKind, Span, Stmt as IrStmt};
    let (var, _) = closure_fixtures();
    let sp = Span::unknown;
    let param = Param {
        name: "v".into(),
        ty: IrType::Int,
        is_mut: false,
        is_ref: false,
        is_owned: false,
        default: None,
        variadic: false,
        comptime: false,
        mods: IrMods::default(),
    };
    // 闭包体：{ total = total + v; total } —— 写的是**外层** total（SYNTAX/03e §五 可写捕获）
    let lam_body = Expr::new(
        ExprKind::BlockExpr {
            block: Block {
                stmts: vec![
                    IrStmt::Assign {
                        target: var("total"),
                        value: Expr::new(
                            ExprKind::BinOp {
                                lhs: Box::new(var("total")),
                                op: BinOpKind::Add,
                                rhs: Box::new(var("v")),
                            },
                            IrType::Int,
                            sp(),
                        ),
                    },
                    IrStmt::ExprStmt { expr: var("total") },
                ],
                ty: IrType::Int,
                span: sp(),
            },
        },
        IrType::Int,
        sp(),
    );
    let mut module = IrModule::new("closure_writecap".into());
    module.items.push(Item::FnDef(FnDef {
        name: "main".into(),
        generics: vec![],
        params: vec![],
        ret_ty: IrType::Unit,
        raises: None,
        body: Block {
            stmts: vec![
                IrStmt::Let {
                    name: "total".into(),
                    ty: IrType::Int,
                    value: Expr::new(
                        ExprKind::Lit(lang_zone::ir::node::LitKind::Int(0)),
                        IrType::Int,
                        sp(),
                    ),
                    is_mut: true,
                    is_ref: false,
                    mods: IrMods::default(),
                },
                IrStmt::Let {
                    name: "add".into(),
                    ty: IrType::Any,
                    value: Expr::new(
                        ExprKind::Lambda {
                            params: vec![param],
                            body: Box::new(lam_body),
                            is_move: false,
                            ret_ty: None,
                        },
                        IrType::Any,
                        sp(),
                    ),
                    is_mut: false,
                    is_ref: false,
                    mods: IrMods::default(),
                },
                IrStmt::ExprStmt {
                    expr: Expr::new(
                        ExprKind::Call {
                            callee: Box::new(var("add")),
                            args: vec![Expr::new(
                                ExprKind::Lit(lang_zone::ir::node::LitKind::Int(3)),
                                IrType::Int,
                                sp(),
                            )],
                            type_args: vec![],
                        },
                        IrType::Any,
                        sp(),
                    ),
                },
            ],
            ty: IrType::Unit,
            span: sp(),
        },
        intrinsics: vec![],
        is_async: false,
        is_iterator: false,
        is_test: false,
        checker_param: None,
        default_checker: None,
        where_clause: vec![],
        span: sp(),
    }));
    let out = gen(module);

    assert_contains(&out, &["def __lambda_0(v):"], "写捕获的块体闭包要 hoist");
    assert_contains(&out, &["nonlocal total"], "写外层变量必须声明 nonlocal");
    assert_contains(&out, &["total = total + v"], "写回语句本身要留在产物里");
    assert!(
        !out.contains("global total"),
        "total 是函数内绑定，不是模块变量 ⇒ 该走 nonlocal 而不是 global，实际输出:\n{out}"
    );
}

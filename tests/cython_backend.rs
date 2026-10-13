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
        
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
        
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
    
            tco: None,
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
        
            tco: None,
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
    
            tco: None,
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
        &["__with_val_res = print(\"TAIL_ONCE\")"],
        "block 提升绑定的是尾值（字符串实参走 Display：规范 SYNTAX/99-内置预导入库.md:50 与 Rust 面 \
         `codegen/expr_gen.rs:76`；旧期望 `print(_lz_dbg(\"TAIL_ONCE\"))` 钉的是本轮之前的「一律 Debug」，\
         承重的是上面两条 count 断言，不是这个包装）",
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
        
            tco: None,
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
    
            tco: None,
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

// ── 魔法方法面（SYNTAX/12 §3.1-§3.3）：以下四把锁取的是**源码→IR→cy 产物**整条链，
//    与语料探针 `TEMP/magic_probes_r11.py` 同一份 LZ 形状；用手搭 IrModule 测不到
//    「共享 IR 把 `~` 降级成 Not / 移位不在分派表里」这两件事，那正是本轮修的东西。

fn gen_from_source(src: &str) -> String {
    let tokens = lang_zone::lexer::Lexer::new(src).tokenize();
    let mut parser = lang_zone::parser::Parser::new(tokens);
    let ast = parser.parse_module().expect("parse ok");
    let mut m = lang_zone::ir::builder::build_ir(&ast).expect("ir ok");
    m.file_path = Some("magic.lz".to_string());
    gen(m)
}

#[test]
fn cy_shift_and_contains_dispatch_only_for_implementors() {
    let out = gen_from_source(
        "struct Bits =\n    v: int\n\nimpl Bits =\n\
         \x20   def __shl__(self, o: Bits) -> Bits =\n        Bits(v: self.v << o.v)\n\
         \x20   def __contains__(ref self, t: int) -> bool =\n        self.v == t\n\n\
         struct Plain =\n    v: int\n\nimpl Plain =\n\
         \x20   def contains(self, t: int) -> bool =\n        self.v == t\n\n\
         def main() =\n    let p = Bits(v: 1)\n    let q = Bits(v: 2)\n    print((p << q).v)\n\
         \x20   print(3 in p)\n    let pl = Plain(v: 1)\n    print(pl.contains(2))\n",
    );
    assert_contains(&out, &["p.__shl__(q)"], "用户类实现 __shl__ ⇒ 点名直派");
    assert!(
        !out.contains("(p << q)"),
        "用户类上的裸 `<<` 在 Python 里要的是 `__lshift__`，留着就是运行期 TypeError，实际输出:\n{out}"
    );
    // 类体里的 int 移位**必须**保持裸运算符
    assert_contains(&out, &["self.v << o.v"], "整数接收者的移位不许被改写");
    assert_contains(&out, &["pl.contains(2)"], "用户自写的 contains 保持原方法名");
    assert_contains(&out, &["(3 in p)"], "实现 __contains__ 的用户类走 in 协议");
}

#[test]
fn cy_bitnot_is_restored_from_operand_type() {
    let out = gen_from_source(
        "struct Bits =\n    v: int\n\nimpl Bits =\n\
         \x20   def __invert__(self) -> Bits =\n        Bits(v: ~self.v)\n\n\
         def main() =\n    let p = Bits(v: 6)\n    print((~p).v)\n    let flag = true\n    print(not flag)\n",
    );
    // IR 现区分 BitNot(`~`) 与 Not(`not`)（BUG-36）；cy 收敛（BitNot⇒`~`/Not⇒`not `）待
    // cy 工具链 co-verify 后补，当前按操作数类型还原：整数/用户类 ⇒ `~`，bool ⇒ `not `
    assert_contains(&out, &["~self.v"], "整数位非要发 ~（发 not 会恒为 False）");
    assert_contains(&out, &["~p"], "用户类的 ~ 落到 __invert__ 协议");
    assert!(
        !out.contains("not p\n"),
        "`~p` 发成 `not p` 会把对象塌成 bool（实测 .v 取不到），实际输出:\n{out}"
    );
    assert_contains(&out, &["not flag"], "bool 的逻辑非仍是 not");
}

#[test]
fn cy_iterator_for_site_uses_the_adapter_only() {
    let out = gen_from_source(
        "struct Steps =\n    n: int\n\nimpl Steps =\n\
         \x20   def __iter__(self) -> StepsIter =\n        StepsIter(i: 0, n: self.n)\n\n\
         struct StepsIter =\n    i: int\n    n: int\n\nimpl StepsIter =\n\
         \x20   def __next__(mut self) -> Option<int> =\n\
         \x20       if self.i < self.n:\n            let r = self.i\n\
         \x20           self.i = self.i + 1\n            Option.Some(value: r)\n\
         \x20       else:\n            Option.None\n\n\
         def main() =\n    mut total = 0\n    for x in Steps(n: 4):\n        total = total + x\n\
         \x20   mut k = 0\n    for y in [1, 2, 3]:\n        k = k + y\n    print(total)\n    print(k)\n",
    );
    assert_contains(&out, &["for x in _lz_iter(Steps(n = 4)):"], "用户迭代器在取数面套适配器");
    assert_contains(&out, &["for y in [1, 2, 3]:"], "内建序列必须原样进 for");
    assert_contains(&out, &["def __next__(self):"], "用户的 __next__ 不被改名");
    assert!(
        !out.contains("__lz_next__"),
        "适配器只在取数面；把 __next__ 改名会让 it.__next__() 抛 StopIteration 进用户程序，实际输出:\n{out}"
    );
    assert_eq!(
        out.matches("_lz_iter(").count(),
        2,
        "_lz_iter 的出现次数 = prelude 里 def 一行 + 取数面一处（多了就是套到别的地方了）:\n{out}"
    );
}

#[test]
fn cy_option_unwrap_or_is_a_dispatch_not_a_method() {
    let out = gen_from_source(
        "def main() =\n    let a: Option<int> = Option.Some(value: 5)\n\
         \x20   print(a.unwrap_or(0))\n    let b: Option<int> = Option.None\n    print(b.unwrap_or(0))\n",
    );
    assert_contains(&out, &["def _lz_or(v, d):"], "prelude 里有 unwrap_or 的分档助手");
    assert_contains(&out, &["_lz_or(a, 0)"], "unwrap_or 发成分档而不是方法调用");
    assert!(
        !out.contains("a.unwrap_or("),
        "Some 侧是裸值、None 侧是垫片单例，两边都没有 .unwrap_or 方法 ⇒ AttributeError，实际输出:\n{out}"
    );
}

#[test]
fn cy_dynamic_receiver_dispatches_at_runtime() {
    let out = gen_from_source(
        "struct Bucket =\n    items: List<int>\n\nimpl Bucket =\n\
         \x20   def len(ref self) -> int =\n        self.items.len()\n\
         \x20   def push(mut self, v: int) =\n        self.items.push(v)\n\
         \x20   def is_empty(ref self) -> bool =\n        self.items.len() == 0\n\n\
         def main() =\n    let sz = |x| => x.len()\n    print(sz(Bucket(items: [1, 2, 3])))\n\
         \x20   let add = |x| => x.push(9)\n    add(Bucket(items: [1]))\n\
         \x20   let e = |x| => x.is_empty()\n    print(e(Bucket(items: [1])))\n\
         \x20   print(1)\n",
    );
    assert_contains(
        &out,
        &["_lz_len(x, \"len\")", "_lz_meth(x, \"push\", \"append\", (9, ))", "_lz_isempty(x)"],
        "接收者是 `Any`（无注解闭包形参）⇒ 三个静态臂旧形态（`len(x)`／`x.append(9)`／`len(x) == 0`）会把用户类自带的方法名强改掉",
    );
    assert_contains(
        &out,
        &["def _lz_dyn(o):", "_LZ_TYPES = (\"Bucket\",)"],
        "运行时分派要的名单：与静态判据同源（known_types ∪ cdef_classes）",
    );
    assert!(
        !out.contains("x.append(9)"),
        "动态接收者上不许再直发 `.append`——Bucket 没有 append，实测 AttributeError，实际输出:\n{out}"
    );
}

#[test]
fn cy_static_receivers_keep_their_existing_forms() {
    let out = gen_from_source(
        "def main() =\n    mut xs: List<int> = [1]\n    xs.push(2)\n\
         \x20   print(xs.len())\n    print(xs.is_empty())\n    print(xs.contains(2))\n",
    );
    assert_contains(
        &out,
        &["xs.append(2)", "len(xs)", "(len(xs) == 0)", "(2 in xs)"],
        "静态类型是内建容器 ⇒ 改写照旧发生，一条都不许改道到运行时分派",
    );
    for needle in ["_lz_meth(xs", "_lz_len(xs", "_lz_isempty(xs)", "_lz_contains(xs"] {
        assert!(
            !out.contains(needle),
            "内建接收者不该吃运行时分派的开销（`{needle}` 出现在产物里 = 静态通道被顶掉了），实际输出:\n{out}"
        );
    }
}

#[test]
fn cy_clone_unwrap_only_passthrough_when_no_such_method() {
    let out = gen_from_source(
        "struct R =\n    v: int\n    def clone(self) -> R =\n        R(v: self.v + 1)\n\
         \x20   def unwrap(self) -> int =\n        self.v + 100\n\nstruct S =\n    v: int\n\n\
         def main() =\n    let a = R(v: 1)\n    print(a.clone().v)\n    print(a.unwrap())\n\
         \x20   let b = S(v: 2)\n    print(b.clone())\n",
    );
    assert_contains(
        &out,
        &["a.clone()", "a.unwrap()"],
        "用户类**自己定义了**这两个名字 ⇒ 是一次真调用，透传会静默少一步（实测 rust 8 / 旧 cy 7）",
    );
    assert!(
        !out.contains("b.clone()"),
        "没有 clone 的用户类必须继续透传——with 脱糖在共享 IR 里就对用户类发 `alias.clone()`，实际输出:\n{out}"
    );
    // 动态接收者那一支也得有判据：`_lz_self` 是这一轮新加的助手，没有实跑探针碰过它
    // （四支静态探针都走不到 `ty_is_dynamic_receiver` 分支），所以在这里按发射形态钉住。
    let dyn_out = gen_from_source(
        "struct C2 =\n    v: int\n    def clone(self) -> C2 =\n        C2(v: self.v + 1)\n\
         \x20   def unwrap(self) -> int =\n        self.v + 10\n\n\
         def main() =\n    let g = |x| => x.clone()\n    print(g(C2(v: 1)).v)\n\
         \x20   let u = |y| => y.unwrap()\n    print(u(C2(v: 1)))\n",
    );
    assert_contains(
        &dyn_out,
        &["g = lambda x: _lz_self(x, \"clone\")", "u = lambda y: _lz_self(y, \"unwrap\")"],
        "动态接收者上的 clone/unwrap 走运行时属性分派，而不是无条件透传（透传＝静默少一次调用）",
    );
    assert!(
        !dyn_out.contains("g = lambda x: x"),
        "旧形态（透传吃掉调用）不许回来，实际输出:\n{dyn_out}"
    );
}

#[test]
fn cy_dynamic_for_site_wraps_and_the_helper_self_guards() {
    let out = gen_from_source(
        "def main() =\n    let run = |it| =>\n        mut t = 0\n\
         \x20       for x in it:\n            t = t + x\n        t\n\
         \x20   print(run([1, 2, 3]))\n",
    );
    assert_contains(
        &out,
        &["for x in _lz_iter(it):"],
        "迭代面在动态接收者上也要套适配器（`__next__` 的 Option.None 不是元素）",
    );
    assert_contains(
        &out,
        &["if type(o).__name__ not in _LZ_TYPES:"],
        "适配器第一行必须自卫：非本模块用户类（list/生成器）原样透传，否则序列里的 None 会被当成结束",
    );
}

// ── T0r13：方法表里余下三条**无条件**臂（expect / is_none / is_some）──────────────
// 立论「cy 的 Option 就是裸值／None」讲的是**值表示**，不是**接收者是谁**。三条臂过去看都不看
// 接收者 ⇒ 用户类自带的同名方法被整条换成一次 identity 比较，实测两侧都不报错、只是数不对：
//   TEMP/uncond-x13-before-r13.log：x_expect_static rust '42' / cy 'Cfg { n: 21 }'，
//   x_isnone_static rust 'true' / cy 'false'，x_issome_static rust 'false' / cy 'true'。

const CFG_UNCOND: &str = "struct Cfg =\n    n: int\n\
     \x20   def expect(ref self, msg: str) -> int =\n        self.n * 2\n\
     \x20   def is_none(ref self) -> bool =\n        true\n\
     \x20   def is_some(ref self) -> bool =\n        false\n\n";

#[test]
fn cy_unconditional_arms_honor_user_methods() {
    let out = gen_from_source(&format!(
        "{CFG_UNCOND}\ndef main() =\n    let a = Cfg(n: 21)\n    print(a.expect(\"m\"))\n\
         \x20   print(a.is_none())\n    print(a.is_some())\n"
    ));
    assert_contains(
        &out,
        &["a.expect(\"m\")", "a.is_none()", "a.is_some()"],
        "用户类自己定义了这三个名字 ⇒ 是一次真调用，无条件臂会把它换成 identity 比较（静默少一步）",
    );
    // 反向：把接收者换成真的 Option，名字不存在于用户类 ⇒ 兜底形态必须原样保留（见下一支锁）
    assert!(
        !out.contains("(a is None)") && !out.contains("(a is not None)"),
        "用户类接收者不许被换成与 None 的比较，实际输出:\n{out}"
    );
}

#[test]
fn cy_option_face_still_compares_to_none() {
    // 成对反向锁：本轮不是把这三条臂整个顶掉——裸值／内建 Option 上的形态一字没改。
    let out = gen_from_source(
        "def main() =\n    let o: Option<int> = None\n    print(o.is_none())\n\
         \x20   print(o.is_some())\n",
    );
    assert_contains(
        &out,
        &["(o is None)", "(o is not None)"],
        "真 Option 上的 is_none/is_some 仍然是与 None 比（cy 的 Option 表示就是裸值／None）",
    );
    // 断言面只圈**用户码那一段**（prelude 里那些助手定义本来就在，整档计数会把定义当成调用）
    let body = out.split("__all__").last().unwrap_or(&out);
    assert!(
        !body.contains("_lz_isnone(o") && !body.contains("_lz_issome(o"),
        "静态可判的内建接收者不许吃运行时分派，实际输出:\n{body}"
    );
}

#[test]
fn cy_expect_evaluates_receiver_once() {
    let out = gen_from_source(
        "def mk() -> Option<int> =\n    print(\"mk\")\n    Some(5)\n\n\
         def main() =\n    let o: Option<int> = None\n    print(mk().expect(\"m\"))\n\
         \x20   print(o.expect(\"m\"))\n",
    );
    let line = out
        .lines()
        .find(|l| l.contains("__lz_expect(mk()"))
        .unwrap_or_else(|| panic!("产物里没有 __lz_expect(mk(), …) 这一行，实际输出:\n{out}"));
    assert_eq!(
        line.matches("mk()").count(),
        1,
        "接收者被拼了两遍 ⇒ 副作用跑两次（旧形态实测 'mk' 打印两遍而退出码 0），该行是: {line}"
    );
    assert_contains(
        &out,
        &["def __lz_expect(v, m): return v if v is not None else __lz_expect_fail(m)"],
        "只求值一次要靠这个垫片；它必须总在 prelude 里（发射处只看得到调用点）",
    );
    assert!(
        !out.contains(" if mk() is not None else"),
        "旧的双拼形态不许回来，实际输出:\n{out}"
    );
}

#[test]
fn cy_dynamic_arms_dispatch_by_name_list() {
    let out = gen_from_source(&format!(
        "{CFG_UNCOND}\ndef main() =\n    let f = |x| => x.is_none()\n\
         \x20   let g = |y| => y.is_some()\n    let h = |z| => z.expect(\"m\")\n\
         \x20   print(f(Cfg(n: 1)))\n    print(g(Cfg(n: 1)))\n    print(h(Cfg(n: 1)))\n"
    ));
    assert_contains(
        &out,
        &[
            "f = lambda x: _lz_isnone(x, \"is_none\")",
            "g = lambda y: _lz_issome(y, \"is_some\")",
            "h = lambda z: _lz_exp(z, \"m\", \"expect\")",
        ],
        "动态接收者（无注解闭包形参 ⇒ Any）判不出类型 ⇒ 发运行时助手，按 `_LZ_TYPES` 名单 + 属性存在分派",
    );
    assert!(
        !out.contains("(x is None)") && !out.contains("(y is not None)"),
        "旧形态（不看接收者就比 identity）不许回来，实际输出:\n{out}"
    );
}

// ── T0r14：方法改写表的**形状门**（常驻，不是某一轮的一次性尺子）──────────────────────
// BUG-37／38／39／40／41／43 全是同一个形状反复出现：表里加一条臂（或留一条臂）却没决定
// 「动态接收者（`IrType::Any`／`IrType::Generic`）怎么办」。机制写在 `src/ir/codegen_cython.rs:2021`
// 的注释里：动态时 `!ty_is_user_class` **恒真**（把用户类自带的方法名强改成 Python 名），
// 而 `ty_is_user_class`／`user_class_has_method` **恒假**（该改写的没改）。
// 前两轮都是一条一条人肉试出来的。这条锁把「人肉」换成「解析产品码自己」：
//   R1 每条臂的闸门都要**至少看一次接收者**（无条件臂就是 BUG-40 那一类）；
//   R2 会在动态接收者上发火的臂（闸门里有 `!ty_is_user_class`／`!user_class_has_method`）必须有
//      `ty_is_dynamic_receiver` 分支；只会正向命中的臂（动态时恒假）必须有**同名兄弟臂**在动态那侧接住；
//   R3 臂体发出去的每个 `_lz_*`／`__lz_*` 垫片，必须在同一份源码里 `def` 过（改发射忘改垫片 = 运行期才炸）。
// 取数面是 `include_str!` 的生成器源码，不是产物 ⇒ 不编译、不跑 cython，纯文本，秒级。
const CY_GEN_SRC: &str = include_str!("../src/ir/codegen_cython.rs");
const TABLE_ANCHOR: &str = "            match method.as_str() {";
const TABLE_CLOSE: &str = "            }";

/// 一条臂：`(行号, 方法名, 闸门文本, 臂体文本)`。
#[derive(Debug)]
struct Arm(usize, String, String, String);

fn parse_arms(src: &str) -> Vec<Arm> {
    let lines: Vec<&str> = src.lines().collect();
    let start = lines
        .iter()
        .position(|l| *l == TABLE_ANCHOR)
        .expect("方法表锚 `match method.as_str() {` 找不到 ⇒ 表的结构变了，这把尺要跟着改");
    let mut arms: Vec<Arm> = Vec::new();
    let mut i = start + 1;
    while i < lines.len() {
        let ln = lines[i];
        if ln == TABLE_CLOSE {
            break;
        }
        // 臂的缩进是 16 个空格（match 体内那一层）；臂体更深，缩进不可能是 16。
        if let Some(rest) = ln.strip_prefix("                \"") {
            let Some(q) = rest.find('"') else { break };
            let name = rest[..q].to_string();
            let tail = rest[q + 1..].trim_start();
            let guard = match tail.split_once(" => {") {
                Some(g) => g.0.trim_start_matches("if ").trim().to_string(),
                None => String::new(),
            };
            arms.push(Arm(i + 1, name, guard, String::new()));
        } else if !arms.is_empty() {
            let Arm(_, _, _, body) = arms.last_mut().unwrap();
            body.push_str(ln);
            body.push('\n');
        }
        i += 1;
    }
    arms
}

/// 从一段文本里挑出 `_lz_*`／`__lz_*` 的**调用**（后面紧跟 `(`）。
fn helper_calls(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut from = 0usize;
    while let Some(p) = text[from..].find("_lz_") {
        let abs = from + p;
        // 往前吃掉第二个下划线（`__lz_expect` 里的 `__`），往后取标识符本体。
        let mut s = abs;
        while s > 0 && (b[s - 1] == b'_' || (b[s - 1]).is_ascii_alphanumeric()) {
            s -= 1;
        }
        let mut e = abs + "_lz_".len();
        while e < b.len() && (b[e] == b'_' || (b[e]).is_ascii_alphanumeric()) {
            e += 1;
        }
        // 调用形态：标识符后第一个非空白字符是 `(`。
        let mut k = e;
        while k < text.len() && text.as_bytes()[k] == b' ' {
            k += 1;
        }
        if k < text.len() && text.as_bytes()[k] == b'(' {
            let id = &text[s..e];
            if !out.iter().any(|x| x == id) {
                out.push(id.to_string());
            }
        }
        from = e;
    }
    out
}

#[test]
fn cy_method_table_shape_gate() {
    let arms = parse_arms(CY_GEN_SRC);
    // 提取器取空 = 整道门恒绿（上一轮真实中招过两次）⇒ 先证明「我确实解析到东西」。
    assert!(!arms.is_empty(), "解析到 0 条臂 ⇒ 尺子坏了，不是表干净");
    let roster = [
        "clone", "contains", "ends_with", "expect", "is_empty", "is_none", "is_some", "len", "length",
        "push", "remove", "starts_with", "to_lower", "to_string", "to_upper", "trim", "try_into",
        "unwrap", "unwrap_or",
    ];
    for name in roster {
        assert!(
            arms.iter().any(|a| a.1 == name),
            "改写表里少了 `{name}` 这条臂：名单是**发货面**，静默缩表 = 用户可见行为变了"
        );
    }
    for Arm(line, name, guard, body) in &arms {
        let neg = guard.matches("!ty_is_user_class").count()
            + guard.matches("!user_class_has_method").count();
        let all = guard.matches("ty_is_user_class").count() + guard.matches("user_class_has_method").count();
        // R1：闸门必须至少看一次接收者。
        assert!(
            all >= 1,
            "第 {line} 行 `{name}` 的闸门没看过接收者（guard={guard:?}）⇒ 这就是 BUG-40 那一类无条件臂"
        );
        let dyn_branch = body.contains("ty_is_dynamic_receiver");
        if neg >= 1 {
            assert!(
                dyn_branch,
                "第 {line} 行 `{name}` 的 `!` 型闸门在动态接收者上**恒真**，却没有 ty_is_dynamic_receiver 分支"
            );
        } else {
            // 只正向命中 ⇒ 动态时不发火，必须有同名兄弟臂在动态那侧接住。
            let sibs = arms.iter().filter(|a| a.1 == *name && a.0 != *line);
            assert!(
                sibs.map(|a| a.3.contains("ty_is_dynamic_receiver")).any(|x| x)
                    || name == "contains",
                "第 {line} 行 `{name}` 在动态接收者上不发火，而表里没有同名带动态分支的兄弟臂接住它"
            );
        }
        // R3：发出去的垫片必须 def 过。
        for h in helper_calls(body) {
            assert!(
                CY_GEN_SRC.contains(&format!("def {h}(")),
                "第 {line} 行 `{name}` 发射 `{h}(` 但 prelude 里没有 `def {h}(` ⇒ 运行期才炸"
            );
        }
    }
}

#[test]
fn cy_method_table_shape_gate_actually_reads_the_table() {
    // 正面对照：这把尺读的必须**是**那张表，而不是恰好解析到的别的东西。
    let arms = parse_arms(CY_GEN_SRC);
    let names: Vec<&str> = arms.iter().map(|a| a.1.as_str()).collect();
    assert!(
        names.contains(&"is_none") && names.contains(&"expect") && names.contains(&"to_upper"),
        "解析结果不像方法改写表 ⇒ {names:?}"
    );
    assert_eq!(
        arms.iter().filter(|a| a.1 == "contains").count(),
        2,
        "`contains` 应当是两条（一条 `!ty_is_user_class` 走运行时、一条正向走 `__contains__`）⇒ {names:?}"
    );

    // 形状对照①：锚在、但臂的缩进不是 16 ⇒ 解析得 0 条 ⇒ 证明「臂=缩进 16 的字符串臂」这个认领方式在承重
    // （而不是它恰好每行都能匹配上）。
    let shifted = format!(
        "{}
        \"is_none\" if !user_class_has_method(cg, &receiver.ty, \"is_none\") => {{
            return \"x\".into();
        }}
{}
",
        TABLE_ANCHOR, TABLE_CLOSE
    );
    assert_eq!(parse_arms(&shifted).len(), 0, "换了缩进还能解析出臂 ⇒ 缩进锚没在承重");

    // 形状对照②：同样内容放回缩进 16 ⇒ 必须解析出 1 条，且名字/闸门/臂体都取得到 ⇒ 提取器不是恒空。
    let fixture = format!(
        "{}
        foo();
                \"is_none\" if !user_class_has_method(cg, &receiver.ty, \"is_none\") => {{
                    return \"(_lz_isnone(a, b))\".to_string();
                }}
{}
",
        TABLE_ANCHOR, TABLE_CLOSE
    );
    let got = parse_arms(&fixture);
    assert_eq!(got.len(), 1, "合法形状的臂解析不出 1 条 ⇒ 提取器在空转：{got:?}");
    assert_eq!(got[0].1, "is_none");
    assert!(got[0].2.starts_with("!user_class_has_method"), "闸门取错：{:?}", got[0].2);
    assert!(got[0].3.contains("ty_is_dynamic_receiver") == false && !got[0].3.is_empty(),
        "臂体该取到内容且这条夹具里没有动态分支：{:?}", got[0].3);
    // 这条夹具正是 R2 要拦的形状（`!` 型闸门 + 无动态分支）⇒ 用同一个判据复算必须为「违例」。
    assert!(
        !got[0].3.contains("ty_is_dynamic_receiver"),
        "夹具自己就有动态分支 ⇒ 下面的违例对照不成立"
    );

    // 垫片提取器：调用形态取到、注释形态不取（否则 R3 会把注释当发射、把真发射漏掉）。
    assert_eq!(
        helper_calls("return format!(\"_lz_isnone(a, b)\"); // _lz_or 在注释里没括号"),
        vec!["_lz_isnone".to_string()],
        "调用提取的形状不对 ⇒ 拿到的名单不可信"
    );
    assert!(
        !helper_calls("// _lz_deadbeef 没有括号").contains(&"_lz_deadbeef".to_string()),
        "把非调用也算成发射 ⇒ R3 会误报"
    );
}

// ── T0r14：print 的 Display／Debug 分档，与 `.to_string()` 的 Display 口径 ──
//
// 两条主张各配一条**反向锁**，因为这一轮的失效形态是「两侧都不报错、数字不一样」：
//   ① `print` 的实参按 `print_arg_is_display` 分档（规范 `SYNTAX/99-内置预导入库.md:50`
//      `print(...)` → `println!("{}", ...)`；Rust 面同一判据在 `codegen/expr_gen.rs:76`）。
//      本轮之前 cy **无条件**套 `_lz_dbg` ⇒ `print("after")` 发 `'\"after\"'` 而 rust 发 `'after'`
//      （实测 `TEMP/strq14`：cy `'\"after\"\\n\"hi\"\\n\"a\" 1\\n'` 对 rust `'after\\nhi\\na 1\\n'`）。
//   ② `.to_string()` 走 Display。方法表原本**没有这条臂**（`TEMP/arm_census_r14.py` 普查 = 19 条），
//      内建接收者落通用尾巴 ⇒ Python 上 AttributeError；实测 `TEMP/tostr-y0-r14.log` 6 支分叉，
//      含规范里的 canonical 写法 `"hello".to_string()`（`SYNTAX/00-词法基础.md:187`）。
// 数值口径不在这里判（那是 L3 golden 的活，语料 `15_feature_matrix/print_display_vs_debug.lz`
// 与 `to_string_display.lz`；逐值 oracle 见 `TEMP/f64ladd-y0-r14.log`），这里只钉**发射形态**。

/// ①正向：字符串类实参不带 Debug 包装。
#[test]
fn cy_print_uses_display_for_string_args() {
    let out = gen_from_source(
        "def main() =\n    let s: str = \"hi\"\n    print(\"after\")\n    print(s)\n\
         \x20   print(\"a\", 1)\n    print(f\"s={s}\")\n    print(\"t=\" + s)\n",
    );
    assert_contains(
        &out,
        &["print(\"after\")", "print(s)", "print(\"a\", _lz_dbg(1))"],
        "裸字符串／str 变量必须走 Display（不带引号）；字符串实参被套上 Debug 就是本轮之前的形态",
    );
    for forbidden in ["_lz_dbg(\"after\")", "_lz_dbg(s)", "_lz_dbg(f\""] {
        assert!(
            !out.contains(forbidden),
            "字符串实参被套上 Debug ⇒ print 出来的值带引号，与 Rust 面逐字不等（'{forbidden}' 出现在:\n{out}"
        );
    }
}

/// ①反向：非字符串实参**必须**仍然套 Debug——砍掉这半边就是把 bool／容器／None 打印成 Python 形态。
#[test]
fn cy_print_keeps_debug_for_non_string_args() {
    let out = gen_from_source(
        "def main() =\n    let b: bool = true\n    print(b)\n    let xs: List<int> = [1, 2]\n\
         \x20   print(xs)\n    let m: Dict<str, int> = {\"k\": 3}\n    print(m)\n\
         \x20   let n: Option<int> = None\n    print(n)\n    print(1)\n",
    );
    assert_contains(
        &out,
        &[
            "print(_lz_dbg(b))",
            "print(_lz_dbg(xs))",
            "print(_lz_dbg(m))",
            "print(_lz_dbg(n))",
            "print(_lz_dbg(1))",
        ],
        "bool／List／Dict／Option／int 仍走 `{:?}` 口径（Python 的 True/[1, 2] 直接打出来与 Rust 不等）",
    );
    assert_contains(&out, &["def _lz_dbg(v):"], "Debug 助手还在 prelude 里");
    // 浮点的 `{:?}`：Python 的 repr 数字串与 Rust 相同、**指数拼法**不同（`1e+16` 对 `1e16`）。
    // 实测 `TEMP/f64ladd-y0-r14.log` 的 u_dbg_ladder：22 值里 6 个分叉，进指数的阈值两侧本来就一致
    // ⇒ 只改写指数拼法。分叉是「两侧都 OK 而值不等」，所以这条锁钉的是拼法归一那支在不在。
    assert_contains(
        &out,
        &["m, e = s.split(\"e\")", "return m + \"e\" + str(int(e))"],
        "_lz_dbg 的浮点分支必须把指数拼法归一到 Rust 的 `e16`／`e-5` 形态"
    );
}

/// ②正向：内建接收者上的 `.to_string()` 发成 Display 助手，而不是原样留给 Python。
#[test]
fn cy_to_string_uses_display_ruling() {
    let out = gen_from_source(
        "def main() =\n    let s: str = \"hi\"\n    let n: int = 42\n    let b: bool = true\n\
         \x20   let f: f64 = 1.0\n    print(s.to_string())\n    print(n.to_string())\n\
         \x20   print(b.to_string())\n    print(f.to_string())\n",
    );
    assert_contains(
        &out,
        &[
            "print(_lz_disp(s))",
            "_lz_disp(n)",
            "_lz_disp(b)",
            "_lz_disp(f)",
            "def _lz_disp(v):",
            "def _lz_disp_f(x):",
        ],
        "str／int／bool／f64 上的 to_string 必须走 Display 口径助手",
    );
    for forbidden in ["s.to_string()", "n.to_string()", "b.to_string()", "f.to_string()"] {
        assert!(
            !out.contains(forbidden),
            "原生类型没有 .to_string()（实测 AttributeError），原样发射就是漏修（'{forbidden}' 在:\n{out}"
        );
    }
    // Display 的两条规矩（逐值 oracle 同那张梯）：恒定位记法、整值不带 `.0`。
    assert_contains(
        &out,
        &["out = out.rstrip(\"0\").rstrip(\".\")"],
        "_lz_disp_f 必须剥掉整值的小数点尾巴（1.0 → `1`，实测 rust `<v2>[1]`）"
    );
}

/// ②反向：用户类**自带** `to_string` 时那条路本来就一致，新臂不许把它吃掉。
#[test]
fn cy_to_string_keeps_user_method() {
    let out = gen_from_source(
        "struct Tag =\n    k: str\n    def to_string(ref self) -> str =\n        \"tag-str\"\n\n\
         def main() =\n    print(Tag(k: \"x\").to_string())\n",
    );
    assert_contains(&out, &["print(Tag(k = \"x\").to_string())"], "用户自己的 to_string 是方法调用");
    assert!(
        !out.contains("_lz_disp(Tag"),
        "把用户方法换成 Display 助手 = 值不对而不响（实测两侧本来都是 'cfg-str\\n'），实际输出:\n{out}"
    );
}

/// ②动态面：`Any`／泛型形参上两条静态闸门同时恒真 ⇒ 必须走「先取属性、取不到才 Display」的探针。
#[test]
fn cy_to_string_dynamic_probes_attr_first() {
    let out = gen_from_source(
        "struct Tag =\n    k: str\n    def to_string(ref self) -> str =\n        \"tag-str\"\n\n\
         def main() =\n    let z = |q| => q.to_string()\n    print(z(42))\n    print(z(Tag(k: \"x\")))\n",
    );
    assert_contains(&out, &["def _lz_tostr(o):", "_lz_tostr(q)"], "动态接收者走属性探针助手");
    assert!(
        !out.contains("q.to_string()"),
        "动态侧直发 `.to_string()` 会在原生接收者上 AttributeError，实际输出:\n{out}"
    );
    assert_contains(
        &out,
        &["f = getattr(o, \"to_string\", None)"],
        "_lz_tostr 的次序：先用户方法，再 Display 口径（反过来就吃掉用户类）"
    );
}

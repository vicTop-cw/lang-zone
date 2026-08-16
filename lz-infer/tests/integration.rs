//! lz-infer 集成测试

use std::fs;
use std::path::PathBuf;

use lz_infer::infer::infer_path;
use lz_infer::infer::infer_path_cross_module;
use lz_infer::lzi::LziFile;

#[test]
fn infer_explicit_function_signature() {
    let tmp = std::env::temp_dir().join("lz_infer_test_explicit.lz");
    fs::write(&tmp, "def add(a: int, b: int) -> int = a + b\n").unwrap();

    let file = infer_path(&tmp).unwrap();
    assert_eq!(file.modules.len(), 1);
    let module = file.modules.values().next().unwrap();
    assert!(module.functions.contains_key("add"));
    let add = &module.functions["add"];
    assert_eq!(add.params.len(), 2);
    assert_eq!(add.params[0].ty, "int");
    assert_eq!(add.params[1].ty, "int");
    assert_eq!(add.return_type.as_deref(), Some("int"));
}

#[test]
fn infer_simple_local_inference() {
    let tmp = std::env::temp_dir().join("lz_infer_test_local.lz");
    fs::write(&tmp, "def double(x) = x * 2\n").unwrap();

    let file = infer_path(&tmp).unwrap();
    let module = file.modules.values().next().unwrap();
    let double = &module.functions["double"];
    assert_eq!(double.params[0].ty, "int");
    assert_eq!(double.return_type.as_deref(), Some("int"));
}

#[test]
fn infer_struct_fields() {
    let tmp = std::env::temp_dir().join("lz_infer_test_struct.lz");
    fs::write(&tmp, "struct Point =\n    x: f64\n    y: f64\n").unwrap();

    let file = infer_path(&tmp).unwrap();
    let module = file.modules.values().next().unwrap();
    let point = &module.structs["Point"];
    assert_eq!(point.fields["x"], "f64");
    assert_eq!(point.fields["y"], "f64");
}

#[test]
fn lzi_roundtrip() {
    let mut file = LziFile::new();
    let json = file.to_json().unwrap();
    let parsed = LziFile::from_json(&json).unwrap();
    assert_eq!(file, parsed);
}

#[test]
fn infer_as_expression() {
    let tmp = std::env::temp_dir().join("lz_infer_test_as.lz");
    fs::write(&tmp, "def cast(x: int) -> f64 = x as f64\n").unwrap();

    let file = infer_path(&tmp).unwrap();
    let module = file.modules.values().next().unwrap();
    let cast = &module.functions["cast"];
    assert_eq!(cast.params[0].ty, "int");
    assert_eq!(cast.return_type.as_deref(), Some("f64"));
}

#[test]
fn type_parser_primitives() {
    use lz_infer::type_parser::parse_type;
    use lang_zone::types::Type;

    assert_eq!(parse_type("int").unwrap(), Type::Int);
    assert_eq!(parse_type("str").unwrap(), Type::Str);
    assert_eq!(
        parse_type("List<int>").unwrap(),
        Type::Generic {
            base: Box::new(Type::Named("List".into())),
            args: vec![Type::Int],
        }
    );
}


#[test]
fn infer_type_test_narrowing() {
    let tmp = std::env::temp_dir().join("lz_infer_test_is_narrowing.lz");
    fs::write(
        &tmp,
        r#"def narrow_int(x) -> int =
    if x is int:
        x + 1
    else:
        0
"#,
    )
    .unwrap();

    let file = infer_path(&tmp).unwrap();
    let module = file.modules.values().next().unwrap();
    let check = &module.functions["narrow_int"];
    assert_eq!(check.params[0].ty, "int");
    assert_eq!(check.return_type.as_deref(), Some("int"));
}

#[test]
fn infer_where_clause() {
    let tmp = std::env::temp_dir().join("lz_infer_test_where.lz");
    fs::write(
        &tmp,
        "def combine<T>(a: T, b: T) -> T where T: Number = a + b\n",
    )
    .unwrap();

    let file = infer_path(&tmp).unwrap();
    assert!(file.unresolved.is_empty(), "unresolved: {:?}", file.unresolved);
    let module = file.modules.values().next().unwrap();
    let combine = &module.functions["combine"];
    assert_eq!(combine.generics, vec!["T"]);
    assert_eq!(combine.where_clause.get("T"), Some(&vec!["Number".to_string()]));
}

#[test]
fn infer_const_value() {
    let tmp = std::env::temp_dir().join("lz_infer_test_const_value.lz");
    fs::write(
        &tmp,
        "const x: int = 1 + 2\nconst y: str = \"hello\"\n",
    )
    .unwrap();

    let file = infer_path(&tmp).unwrap();
    assert!(file.unresolved.is_empty(), "unresolved: {:?}", file.unresolved);
    let module = file.modules.values().next().unwrap();

    let x = &module.consts["x"];
    assert_eq!(x.ty, "int");
    assert_eq!(x.value.as_deref(), Some("3"));

    let y = &module.consts["y"];
    assert_eq!(y.ty, "str");
    assert_eq!(y.value.as_deref(), Some("hello"));
}

#[test]
fn infer_union_type() {
    let tmp = std::env::temp_dir().join("lz_infer_union.lz");
    fs::write(
        &tmp,
        r#"def choose(cond: bool) -> int | str =
    if cond:
        42
    else:
        "hello"
"#,
    )
    .unwrap();

    let file = infer_path(&tmp).unwrap();
    assert!(file.unresolved.is_empty(), "unresolved: {:?}", file.unresolved);
    let module = file.modules.values().next().unwrap();
    let choose = &module.functions["choose"];
    assert_eq!(choose.return_type.as_deref(), Some("int | str"));
}

#[test]
fn infer_union_type_match() {
    let tmp = std::env::temp_dir().join("lz_infer_union_match.lz");
    fs::write(
        &tmp,
        r#"def pick(x: bool) -> int | str | bool =
    match x:
        case True => 1
        case False => "two"
        case _ => True
"#,
    )
    .unwrap();

    let file = infer_path(&tmp).unwrap();
    assert!(file.unresolved.is_empty(), "unresolved: {:?}", file.unresolved);
    let module = file.modules.values().next().unwrap();
    let pick = &module.functions["pick"];
    assert_eq!(pick.return_type.as_deref(), Some("int | str | bool"));
}

#[test]
fn infer_intersection_type() {
    let tmp = std::env::temp_dir().join("lz_infer_intersection.lz");
    fs::write(
        &tmp,
        "def both(x: Clone & Debug) -> Clone & Debug = x\n",
    )
    .unwrap();

    let file = infer_path(&tmp).unwrap();
    assert!(file.unresolved.is_empty(), "unresolved: {:?}", file.unresolved);
    let module = file.modules.values().next().unwrap();
    let both = &module.functions["both"];
    assert_eq!(both.params[0].ty, "Clone & Debug");
    assert_eq!(both.return_type.as_deref(), Some("Clone & Debug"));
}

#[test]
fn type_parser_intersection() {
    use lz_infer::type_parser::parse_type;
    use lang_zone::types::Type;

    assert_eq!(
        parse_type("A & B").unwrap(),
        Type::Intersection(vec![Type::Named("A".into()), Type::Named("B".into())])
    );
    assert_eq!(
        parse_type("A & B & A").unwrap(),
        Type::Intersection(vec![Type::Named("A".into()), Type::Named("B".into())])
    );
    assert_eq!(
        parse_type("A | B & C").unwrap(),
        Type::Union(vec![
            Type::Named("A".into()),
            Type::Intersection(vec![Type::Named("B".into()), Type::Named("C".into())])
        ])
    );
}


#[test]
fn infer_hkt_map_signature() {
    let tmp = std::env::temp_dir().join("lz_infer_hkt_map_signature.lz");
    fs::write(
        &tmp,
        "def map<F[_], A, B>(fa: F[A], f: (A) -> B) -> F[B] = fa\n",
    )
    .unwrap();

    let file = infer_path(&tmp).unwrap();
    assert!(file.unresolved.is_empty(), "unresolved: {:?}", file.unresolved);
    let module = file.modules.values().next().unwrap();
    let map = &module.functions["map"];
    assert_eq!(map.generics, vec!["F", "A", "B"]);
    assert_eq!(map.params.len(), 2);
    assert_eq!(map.params[0].ty, "F<A>");
    assert_eq!(map.params[1].ty, "fn(A) -> B");
    assert_eq!(map.return_type.as_deref(), Some("F<B>"));
}

#[test]
fn infer_hkt_map_list_call() {
    let tmp = std::env::temp_dir().join("lz_infer_hkt_map_list.lz");
    fs::write(
        &tmp,
        r#"def map<F[_], A, B>(fa: F[A], f: (A) -> B) -> F[B] = fa

def use_map() -> List<int> =
    let xs: List<int> = [1, 2, 3]
    map(xs, |x| x + 1)
"#,
    )
    .unwrap();

    let file = infer_path(&tmp).unwrap();
    assert!(file.unresolved.is_empty(), "unresolved: {:?}", file.unresolved);
    let module = file.modules.values().next().unwrap();
    let use_map = &module.functions["use_map"];
    assert_eq!(use_map.return_type.as_deref(), Some("List<int>"));
}

#[test]
fn infer_hkt_map_option_call() {
    let tmp = std::env::temp_dir().join("lz_infer_hkt_map_option.lz");
    fs::write(
        &tmp,
        r#"def map<F[_], A, B>(fa: F[A], f: (A) -> B) -> F[B] = fa

def use_map() -> Option<int> =
    let o: Option<int> = Some(5)
    map(o, |x| x + 1)
"#,
    )
    .unwrap();

    let file = infer_path(&tmp).unwrap();
    assert!(file.unresolved.is_empty(), "unresolved: {:?}", file.unresolved);
    let module = file.modules.values().next().unwrap();
    let use_map = &module.functions["use_map"];
    assert_eq!(use_map.return_type.as_deref(), Some("Option<int>"));
}

// ===========================================================================
// 跨模块推断测试
// ===========================================================================

/// 创建临时目录并写入两个 .lz 源文件，返回目录路径
fn setup_cross_module_dir(name: &str, a_source: &str, b_source: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lz_infer_xmod_{}", name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("module_a.lz"), a_source).unwrap();
    std::fs::write(dir.join("module_b.lz"), b_source).unwrap();
    dir
}

#[test]
fn infer_cross_module_struct_fields() {
    let dir = setup_cross_module_dir(
        "struct_fields",
        "from module_b import PointB\nstruct PointA =\n    x: int\n    y: int\n    b: PointB\n",
        "struct PointB =\n    label: str\n",
    );

    let file = infer_path_cross_module(&dir).unwrap();

    assert_eq!(file.modules.len(), 2, "unresolved: {:?}", file.unresolved);

    // 验证 module_a 的 PointA 结构体字段
    let mod_a = &file.modules["module_a"];
    let point_a = mod_a.structs.get("PointA").expect("PointA not found");
    assert_eq!(point_a.fields.get("x").map(|s| s.as_str()), Some("int"));
    assert_eq!(point_a.fields.get("y").map(|s| s.as_str()), Some("int"));
    assert_eq!(point_a.fields.get("b").map(|s| s.as_str()), Some("PointB"));

    // 验证 module_b 的 PointB 结构体字段
    let mod_b = &file.modules["module_b"];
    let point_b = mod_b.structs.get("PointB").expect("PointB not found");
    assert_eq!(point_b.fields.get("label").map(|s| s.as_str()), Some("str"));
}

#[test]
fn infer_cross_module_mutual_structs() {
    let dir = setup_cross_module_dir(
        "mutual",
        "from module_b import PointB\nstruct PointA =\n    x: int\n    b: PointB\n",
        "from module_a import PointA\nstruct PointB =\n    label: str\n    a: PointA\n",
    );

    let file = infer_path_cross_module(&dir).unwrap();

    assert_eq!(file.modules.len(), 2, "unresolved: {:?}", file.unresolved);

    // module_a 的 PointA 有跨模块��段 b: PointB
    let mod_a = &file.modules["module_a"];
    let point_a = mod_a.structs.get("PointA").expect("PointA not found");
    assert_eq!(point_a.fields.get("b").map(|s| s.as_str()), Some("PointB"));

    // module_b 的 PointB 有跨模块字段 a: PointA
    let mod_b = &file.modules["module_b"];
    let point_b = mod_b.structs.get("PointB").expect("PointB not found");
    assert_eq!(point_b.fields.get("a").map(|s| s.as_str()), Some("PointA"));
}

#[test]
fn infer_cross_module_with_type_alias() {
    let dir = setup_cross_module_dir(
        "type_alias",
        "from module_b import UserId\nstruct Order =\n    id: UserId\n    amount: f64\n",
        "type UserId = int\nstruct User =\n    id: UserId\n    name: str\n",
    );

    let file = infer_path_cross_module(&dir).unwrap();

    assert_eq!(file.modules.len(), 2, "unresolved: {:?}", file.unresolved);

    // module_a 引用了 module_b 的 UserId 类型别名
    let mod_a = &file.modules["module_a"];
    let order = mod_a.structs.get("Order").expect("Order not found");
    assert_eq!(order.fields.get("id").map(|s| s.as_str()), Some("UserId"));
}

#[test]
fn infer_cross_module_no_cross_flag_fallback() {
    // legacy 模式（infer_path）单文件推断
    let dir = setup_cross_module_dir(
        "no_cross",
        "struct PointA =\n    x: int\n",
        "struct PointB =\n    y: f64\n",
    );

    let file = infer_path(&dir).unwrap();

    assert_eq!(file.modules.len(), 2, "unresolved: {:?}", file.unresolved);
    // legacy 模式不应有 [cross_module] 注记
    let cross_markers: Vec<_> = file.unresolved.iter()
        .filter(|s| s.starts_with("[cross_module]"))
        .collect();
    assert!(cross_markers.is_empty(), "legacy mode should not have cross_module markers");
}

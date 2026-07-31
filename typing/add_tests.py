#!/usr/bin/env python3
from pathlib import Path

ROOT = Path("e:/IDEProjects/AI/lang-zone")
TEST_FILE = ROOT / "lz-infer/tests/integration.rs"

tests = r'''

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
'''

content = TEST_FILE.read_text(encoding="utf-8")
# avoid duplicate append
if "infer_hkt_map_signature" not in content:
    TEST_FILE.write_text(content + tests, encoding="utf-8")
    print("Tests added.")
else:
    print("Tests already present.")

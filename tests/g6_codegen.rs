// Lang-Zone 编译器 — tests/g6_codegen.rs
// G6（D2 codegen 补缺）验收测试：impl 块 / 列表推导 / 生成器 / match 模式匹配
//
// 结构：
// - 正向用例（check_case）：LZ 源码 → lang-zone 编译（IR 路线）→ rustc → 运行 → 断言 stdout
// - 拒绝用例（reject_case）：编译器必须以非零退出且 stderr 含可读诊断
//
// 边界说明：
// - print(str) 沿既有语义输出带引号字符串（如 "woof rex"）
// - 整数区间 1..N 为左闭右开（1..5 → 1,2,3,4）
// - 集合/字典为无序容器，本文件正向用例不依赖其打印顺序

use std::path::PathBuf;
use std::process::Command;

fn builtins_rlib() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug");
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return direct;
    }
    let deps = dir.join("deps");
    if let Ok(entries) = std::fs::read_dir(&deps) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
                return e.path();
            }
        }
    }
    panic!("lz_builtins rlib not found under target/debug");
}

fn lang_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
}

/// 正向用例：编译 → rustc → 运行 → 断言 stdout
fn check_case(name: &str, source: &str, expected: &str) {
    let work = std::env::temp_dir().join(format!("lz_g6_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");

    let out = Command::new(&lang_bin()).arg(&lz).output().expect("run lang-zone");
    assert!(
        out.status.success(),
        "[{name}] lang-zone 编译失败: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let rs = lz.with_extension("rs");
    let exe = lz.with_extension("exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    assert!(
        rc.status.success(),
        "[{name}] rustc 编译失败:\n{}",
        String::from_utf8_lossy(&rc.stderr)
    );

    let run = Command::new(&exe).output().expect("run compiled exe");
    assert!(
        run.status.success(),
        "[{name}] 程序运行失败: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let got = String::from_utf8_lossy(&run.stdout).to_string();
    assert_eq!(got, expected, "[{name}] 运行输出与 golden 不一致");
}

/// 拒绝用例：编译器必须拒绝，且 stderr 含非空诊断
fn reject_case(name: &str, source: &str) {
    let work = std::env::temp_dir().join(format!("lz_g6_reject_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");

    let out = Command::new(&lang_bin()).arg(&lz).output().expect("run lang-zone");
    assert!(
        !out.status.success(),
        "[{name}] 编译器未拒绝（应拒绝）"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.trim().is_empty(),
        "[{name}] 拒绝但错误信息为空"
    );
}

// ================================================================ impl 块

#[test]
fn g6_impl_inherent_and_trait() {
    check_case(
        "impl_inherent_trait",
        r#"
trait Speaker =
    def speak(self) -> str

struct Dog =
    name: str

impl Dog =
    def age(self) -> int =
        5

impl Speaker for Dog =
    def speak(self) -> str =
        self.name + " woof"

def main() =
    let d = Dog(name: "rex")
    print(d.age())
    print(d.speak().len())
    print(d.speak())
"#,
        "5\n8\n\"rex woof\"\n",
    );
}

#[test]
fn g6_impl_generic() {
    check_case(
        "impl_generic",
        r#"
struct Pair =
    a: int
    b: int

impl Pair =
    def sum(self) -> int =
        self.a + self.b
    def prod(self) -> int =
        self.a * self.b

def main() =
    let p = Pair(a: 6, b: 7)
    print(p.sum())
    print(p.prod())
"#,
        "13\n42\n",
    );
}

#[test]
fn g6_impl_reject_unknown_trait() {
    reject_case(
        "impl_unknown_trait",
        r#"
struct S =
    x: int
impl Missing for S =
    def m(self) -> int = 1
"#,
    );
}

#[test]
fn g6_impl_reject_unknown_target() {
    reject_case(
        "impl_unknown_target",
        r#"
impl Ghost =
    def m(self) -> int = 1
"#,
    );
}

#[test]
fn g6_impl_reject_abstract_missing() {
    reject_case(
        "impl_abstract_missing",
        r#"
trait T =
    def a(self) -> int
struct S =
    x: int
impl T for S =
    def b(self) -> int = 2
"#,
    );
}

#[test]
fn g6_impl_reject_extra_method() {
    reject_case(
        "impl_extra_method",
        r#"
trait T =
    def a(self) -> int
struct S =
    x: int
impl T for S =
    def a(self) -> int = 1
    def extra(self) -> int = 2
"#,
    );
}

// ================================================================ 列表推导

#[test]
fn g6_listcomp_simple_and_guard() {
    check_case(
        "listcomp_simple",
        r#"
def sq(x: int) -> int =
    x * x

def main() =
    let xs = [sq(x) for x in 1..5]
    print(xs)
    let evens = [x for x in 1..10 if x % 2 == 0]
    print(evens)
"#,
        "[1, 4, 9, 16]\n[2, 4, 6, 8]\n",
    );
}

#[test]
fn g6_listcomp_nested() {
    check_case(
        "listcomp_nested",
        r#"
def main() =
    let grid = [[y * 10 + x for x in 1..3] for y in 1..3]
    print(grid)
    let pairs = [(a, b) for a in 1..4 for b in 10..13 if (a + b) % 2 == 0]
    print(pairs)
"#,
        "[[11, 12], [21, 22]]\n[(1, 11), (2, 10), (2, 12), (3, 11)]\n",
    );
}

#[test]
fn g6_listcomp_function_call() {
    check_case(
        "listcomp_fn",
        r#"
def double(x: int) -> int =
    x * 2

def main() =
    let ys = [double(i) for i in 1..6]
    print(ys)
"#,
        "[2, 4, 6, 8, 10]\n",
    );
}

#[test]
fn g6_listcomp_reject_unbound_guard() {
    reject_case(
        "listcomp_unbound_guard",
        r#"
def main() =
    let r = [x for x in 1..5 if y > 0]
    print(r)
"#,
    );
}

// ================================================================ 生成器

#[test]
fn g6_generator_yield() {
    check_case(
        "generator_yield",
        r#"
iterator counter(n: int) -> int =
    for i in 1..n:
        yield i * i

def main() =
    let mut acc = 0
    for v in counter(5):
        acc += v
    print(acc)
    let out = [v for v in counter(4)]
    print(out)
"#,
        "30\n[1, 4, 9]\n",
    );
}

#[test]
fn g6_generator_yield_from() {
    check_case(
        "generator_yield_from",
        r#"
iterator inner() -> int =
    yield 1
    yield 2

iterator outer() -> int =
    yield 0
    yield from inner()
    yield 9

def main() =
    let out = [v for v in outer()]
    print(out)
"#,
        "[0, 1, 2, 9]\n",
    );
}

#[test]
fn g6_generator_reject_top_level_yield() {
    reject_case(
        "generator_top_level_yield",
        "yield 1\n",
    );
}

// ================================================================ match

#[test]
fn g6_match_literal_and_wildcard() {
    check_case(
        "match_literal",
        r#"
def describe(x: int) -> str =
    match x:
        case 0 => "zero"
        case 1 => "one"
        case _ => "many"

def main() =
    print(describe(0))
    print(describe(1))
    print(describe(42))
"#,
        "\"zero\"\n\"one\"\n\"many\"\n",
    );
}

#[test]
fn g6_match_tuple_range_guard() {
    check_case(
        "match_tuple_range_guard",
        r#"
def classify(p: (int, int)) -> str =
    match p:
        case (0, _) => "first"
        case (_, 0) => "second"
        case (1..=3, _) => "low"
        case (x, y) if x + y > 100 => "big"
        case _ => "other"

def main() =
    print(classify((0, 5)))
    print(classify((3, 0)))
    print(classify((2, 9)))
    print(classify((60, 80)))
    print(classify((5, 5)))
"#,
        "\"first\"\n\"second\"\n\"low\"\n\"big\"\n\"other\"\n",
    );
}

#[test]
fn g6_match_reject_dup_pattern() {
    reject_case(
        "match_dup_pattern",
        r#"
def main() =
    let x = 1
    let r = match x:
        case 0 => 1
        case 0 => 2
        case _ => 3
    print(r)
"#,
    );
}

#[test]
fn g6_match_reject_unbound_guard() {
    reject_case(
        "match_unbound_guard",
        r#"
def main() =
    let x = 1
    let r = match x:
        case y if z > 0 => 1
        case _ => 2
    print(r)
"#,
    );
}

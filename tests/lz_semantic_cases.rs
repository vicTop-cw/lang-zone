// Lang-Zone 编译器 — tests/lz_semantic_cases.rs
// 测试强化（阶段2b / FIST 任务 T4.4）：关键路径正例测试对（input → expected）
//
// 设计原则（PROJECT-SPEC/03 §2）：
// - 每个用例 = LZ 源码（input）→ 编译 → rustc → 运行 → 断言 stdout（expected）
// - 覆盖核心语义路径：算术/字符串/递归/容器/循环/闭包/结构体/构建块/异常/布尔
// - 所有用例仅走 IR 路线（默认），golden 基线精确匹配

use std::path::{Path, PathBuf};
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

/// 编译并运行单个 .lz 源，返回运行 stdout；任一环节失败则 panic 并附诊断
fn run_lz(name: &str, source: &str) -> String {
    let work = std::env::temp_dir().join(format!("lz_semantic_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lang-zone");
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
    String::from_utf8_lossy(&run.stdout).to_string()
}

/// 断言用例：stdout 必须与 expected 逐字符一致
fn check_case(name: &str, source: &str, expected: &str) {
    let got = run_lz(name, source);
    assert_eq!(got, expected, "[{name}] 运行输出与 golden 不一致");
}

// ---------------------------------------------------------------- 用例集

#[test]
fn sem_arith_precedence() {
    check_case(
        "arith",
        r#"
def main() =
    print(2 + 3 * 4)
    print((2 + 3) * 4)
    print(100 / 5 - 3)
"#,
        "14\n20\n17\n",
    );
}

#[test]
fn sem_string_concat() {
    check_case(
        "string",
        r#"
def main() =
    let a = "Hello, "
    let b = "LZ!"
    print(a + b)
"#,
        "\"Hello, LZ!\"\n",
    );
}

#[test]
fn sem_recursion_fib() {
    check_case(
        "fib",
        r#"
def fib(n: int) -> int =
    if n <= 1:
        n
    else:
        fib(n - 1) + fib(n - 2)

def main() =
    print(fib(10))
"#,
        "55\n",
    );
}

#[test]
fn sem_list_ops() {
    check_case(
        "list",
        r#"
def main() =
    let xs = [10, 20, 30]
    print(xs.len())
    print(xs[1])
    let ys = xs + [40]
    print(ys.len())
    print(ys[3])
"#,
        "3\n20\n4\n40\n",
    );
}

#[test]
fn sem_for_loop_sum() {
    // 0..10 为左闭右开区间
    check_case(
        "for_sum",
        r#"
def main() =
    let mut acc = 0
    for i in 0..10:
        acc += i
    print(acc)
"#,
        "45\n",
    );
}

#[test]
fn sem_while_loop() {
    check_case(
        "while_sum",
        r#"
def main() =
    let mut i = 0
    let mut acc = 0
    while i < 10:
        i += 1
        acc += i
    print(acc)
"#,
        "55\n",
    );
}

#[test]
fn sem_closure() {
    check_case(
        "closure",
        r#"
def main() =
    let double = |x: int| -> int = x * 2
    print(double(21))
"#,
        "42\n",
    );
}

#[test]
fn sem_struct_method() {
    check_case(
        "struct",
        r#"
struct Rect =
    w: f64
    h: f64

    def area(self) -> f64 =
        self.w * self.h

def main() =
    let r = Rect(w: 3.0, h: 4.0)
    print(r.area())
    print(r.w, r.h)
"#,
        "12.0\n3.0 4.0\n",
    );
}

#[test]
fn sem_build_block() {
    check_case(
        "build",
        r#"
BASE =:
    10 + 20

def apply(f: int) -> int =
    f + 1

def main() =
    print(BASE)
    print(BASE * 2)
    print(apply(BASE))
"#,
        "30\n60\n31\n",
    );
}

#[test]
fn sem_raise_try() {
    check_case(
        "raise",
        r#"
def checked(x: int) -> int raises str =
    if x < 0:
        raise "negative"
    x * 2

def main() =
    try:
        let v = checked(21)
        print(v)
    catch e:
        print("err:" + e)
    try:
        let v2 = checked(-1)
        print(v2)
    catch e2:
        print("caught:" + e2)
"#,
        "42\n\"caught:Any { .. }\"\n",
    );
}

#[test]
fn sem_bool_compare() {
    check_case(
        "bool",
        r#"
def main() =
    print(3 > 2)
    print(2 == 2)
    print(3 != 2)
    print(1 >= 2)
"#,
        "true\ntrue\ntrue\nfalse\n",
    );
}

#[test]
fn sem_multi_line_fn() {
    check_case(
        "multi",
        r#"
def classify(x: int) -> str =
    let doubled = x * 2
    if doubled > 10:
        "big"
    else:
        "small"

def main() =
    print(classify(3))
    print(classify(10))
"#,
        "\"small\"\n\"big\"\n",
    );
}

#[test]
fn sem_generator_yield() {
    check_case(
        "gen",
        r#"
iterator counter(n: int) -> int =
    for i in 0..n:
        yield i

def main() =
    let mut acc = 0
    for x in counter(5):
        acc += x
    print(acc)
"#,
        "10\n",
    );
}

#[test]
fn sem_string_length_index() {
    check_case(
        "strlen",
        r#"
def main() =
    let s = "hello"
    print(s.len())
    print(s[0])
"#,
        "5\n104\n",
    );
}

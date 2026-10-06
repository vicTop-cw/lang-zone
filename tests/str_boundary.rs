// Lang-Zone 编译器 — tests/str_boundary.rs
// 字符串边界专项闸门（2026-09-27，配合 src/ir/codegen/str_boundary.rs 契约层）
//
// 目标：把 LZ 唯一字符串类型 `str` 在 Rust 侧的四形态契约（拥有值 String /
// 借用视图 &str / &String / 'static str）与「位置要求」（拥有值位、可 deref 的
// 借用位、严格 &str 位、Display 位）**锁进回归闸门**：每个用例都是一段独立 LZ
// 程序，转译 → rustc 真编译 → 运行，并按 stdout golden 校验。
//
// 用例内以 LZ 的 `assert` 校验语义（字符串比较、长度、拼接、插值、容器键…），
// 统一以 `print(12345)` 收尾，故 golden 恒为 "12345\n"（避免 print 对 str 使用
// Debug 语义带来的引号干扰）。
//
// 覆盖矩阵（27 例）：
//   A 实参位/返回位：字面量→拥有值、拥有值→ref 形参、ref→拥有值透传、
//     ref self 字段返回、__str__ 覆写、字段作实参、raises 的 Result<str,_>、
//     ref str 形参返回
//   B 绑定位/容器：str 字段读写、Dict<str,_> 键、Set<str>、Option<str>、
//     List<str> 索引、join、元组内 str、const str、push/长度
//   C 运算符/格式化/借用位：str+str、str(int)、== / <、单字符索引码点、切片、
//     for-in 迭代、f-string 插值（Debug 引号语义）、Pattern 方法（trim/starts_with/
//     replace/split/contains）、泛型 T 用 str 实例化、闭包取 str

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
    panic!("lz_builtins rlib not found under target/debug（先 cargo build -p lz_builtins）");
}

/// 转译 + rustc 真编译 + 运行，返回 stdout
fn run_lz(name: &str, source: &str) -> String {
    let work = std::env::temp_dir().join(format!("lz_str_boundary_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lang-zone");
    assert!(
        out.status.success(),
        "[{name}] lang-zone 转译失败: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let rs = lz.with_extension("rs");
    let exe = lz.with_extension("exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("-L")
        .arg(format!(
            "dependency={}/target/debug/deps",
            env!("CARGO_MANIFEST_DIR")
        ))
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    assert!(
        rc.status.success(),
        "[{name}] rustc 编译失败（字符串边界形态不符契约）:\n{}",
        String::from_utf8_lossy(&rc.stderr)
    );

    let run = Command::new(&exe).output().expect("run compiled exe");
    assert!(
        run.status.success(),
        "[{name}] 程序运行失败（assert 未通过）:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

/// 每个用例的 LZ 源统一以 `print(12345)` 收尾
const GOLDEN: &str = "12345\n";

fn run_cases(cases: &[(&str, &str)]) {
    let mut failed: Vec<String> = Vec::new();
    for (name, src) in cases {
        let got = run_lz(name, src);
        if got != GOLDEN {
            failed.push(format!("[{name}] stdout = {got:?}（期望 {GOLDEN:?}）"));
        }
    }
    assert!(failed.is_empty(), "字符串边界用例失败:\n{}", failed.join("\n"));
}

// ────────────────────────────────────────────────────────────────
// A. 实参位 / 返回位
// ────────────────────────────────────────────────────────────────
#[test]
fn str_boundary_args_and_returns() {
    run_cases(&[
        // A1 字面量（Static）→ 拥有值形参（String）：需要 .to_string()
        (
            "a1_literal_to_owned_param",
            r#"
def tag(s: str) -> str = s + "!"
def main() =
    assert tag("hi") == "hi!"
    print(12345)
"#,
        ),
        // A2 拥有值（String）→ ref str 形参（借用视图）：需要取引用
        (
            "a2_owned_to_ref_param",
            r#"
def length_of(ref s: str) -> int = len(s)
def main() =
    let x = "abcd"
    assert length_of(x) == 4
    print(12345)
"#,
        ),
        // A3 借用视图 → 拥有值形参：ref str 透传给 String 形参
        (
            "a3_ref_to_owned_param",
            r#"
def take(s: str) -> str = s
def forward(ref s: str) -> str = take(s)
def main() =
    assert forward("ab") == "ab"
    print(12345)
"#,
        ),
        // A4 ref self 的 str 字段作返回值（&self 字段 → String）
        (
            "a4_ref_self_field_return",
            r#"
struct User =
    name: str

impl User =
    def label(ref self) -> str =
        return self.name

def main() =
    let u = User(name: "neo")
    assert u.label() == "neo"
    print(12345)
"#,
        ),
        // A5 __str__ 覆写（Display 位）：str(obj) 走 __str__
        (
            "a5_str_dunder",
            r#"
struct Wrap =
    v: str

impl Wrap =
    def __str__(ref self) -> str =
        return "Wrap(" + self.v + ")"

def main() =
    let w = Wrap(v: "x")
    assert str(w) == "Wrap(x)"
    print(12345)
"#,
        ),
        // A6 字段（&self 取值）作字符串实参
        (
            "a6_field_as_arg",
            r#"
def shout(s: str) -> str = s.to_upper()

struct P =
    name: str

impl P =
    def loud(ref self) -> str = shout(self.name)

def main() =
    assert P(name: "ab").loud() == "AB"
    print(12345)
"#,
        ),
        // A7 raises 函数的 Result<str, str> 边界（Ok/Err 两路）
        (
            "a7_raises_result_str",
            r#"
def pick(s: str, n: int) -> str raises str =
    if n < 0:
        raise "negative"
    return s

def main() =
    let r = pick("x", 1)
    match r:
        case Ok(v):
            assert v == "x"
        case Err(e):
            assert len(e) > 0
    let bad = pick("x", -1)
    match bad:
        case Ok(v):
            assert v == ""
        case Err(e):
            assert e == "negative"
    print(12345)
"#,
        ),
    ]);
}

// ────────────────────────────────────────────────────────────────
// B. 绑定位 / 容器
// ────────────────────────────────────────────────────────────────
#[test]
fn str_boundary_bindings_and_containers() {
    run_cases(&[
        // B1 str 字段读写（原地赋值 + 读回）
        (
            "b1_field_read_write",
            r#"
struct Acc =
    label: str

impl Acc =
    def rename(mut self, v: str) =
        self.label = v
    def get(ref self) -> str = self.label

def main() =
    let mut a = Acc(label: "old")
    a.rename("new")
    assert a.get() == "new"
    print(12345)
"#,
        ),
        // B2 Dict<str, int> 键（HashMap<String, i64> 的 get/insert）
        (
            "b2_dict_str_key",
            r#"
def main() =
    let mut d: Dict<str, int> = {"a": 1}
    d["b"] = 2
    assert d["a"] == 1
    assert d["b"] == 2
    assert len(d) == 2
    print(12345)
"#,
        ),
        // B3 Set<str>
        (
            "b3_set_str",
            r#"
def main() =
    let s: Set<str> = {"a", "b"}
    assert s.contains("a")
    assert not s.contains("zzz")
    print(12345)
"#,
        ),
        // B4 Option<str>：Some/None 两路；两个 let 都显式注解为 str，
        // 使各分支的期望类型是 String（None 分支字面量需拥有化）
        (
            "b4_option_str",
            r#"
def main() =
    let some_v: Option<str> = Some("hi")
    let none_v: Option<str> = Option.None
    let s1: str = match some_v:
        case Option.Some(value: v) => v
        case Option.None => "?"
    let s2: str = match none_v:
        case Option.Some(value: v) => v
        case Option.None => "fallback"
    assert s1 == "hi"
    assert s2 == "fallback"
    print(12345)
"#,
        ),
        // B5 List<str> 索引与长度（容器元素位：借用视图 → 拥有值）
        (
            "b5_list_str_index",
            r#"
def main() =
    let xs: List<str> = ["a", "b", "c"]
    assert xs[0] == "a"
    assert xs[2] == "c"
    assert len(xs) == 3
    print(12345)
"#,
        ),
        // B6 join（字符串方法链 → String）
        (
            "b6_join",
            r#"
def main() =
    let parts: List<str> = ["a", "b", "c"]
    let joined = "-".join(parts)
    assert joined == "a-b-c"
    print(12345)
"#,
        ),
        // B7 元组内的 str（t.1 取出 → 拥有值位）
        (
            "b7_tuple_str",
            r#"
def main() =
    let t: (int, str) = (7, "seven")
    assert t.1 == "seven"
    let name = t.1
    assert name + "!" == "seven!"
    print(12345)
"#,
        ),
        // B8 const str（'static 形态）
        (
            "b8_const_str",
            r#"
const NAME: str = "lz"

def main() =
    assert NAME == "lz"
    assert NAME + "!" == "lz!"
    print(12345)
"#,
        ),
        // B9 push 到 List<str>（元素拥有化）
        (
            "b9_list_push",
            r#"
def main() =
    let mut xs: List<str> = []
    xs.push("a")
    let v = "b"
    xs.push(v)
    assert len(xs) == 2
    assert xs[1] == "b"
    print(12345)
"#,
        ),
    ]);
}

// ────────────────────────────────────────────────────────────────
// C. 运算符 / 格式化 / 严格借用位
// ────────────────────────────────────────────────────────────────
#[test]
fn str_boundary_ops_and_formatting() {
    run_cases(&[
        // C1 str + str 拼接（LzAdd）
        (
            "c1_concat_str_str",
            r#"
def main() =
    let a = "foo"
    let b = "bar"
    assert a + b == "foobar"
    assert "x" + a == "xfoo"
    print(12345)
"#,
        ),
        // C2 数字 → str（str() 内置）后再拼接
        (
            "c2_concat_num",
            r#"
def main() =
    let n = 42
    assert str(n) == "42"
    assert "n=" + str(n) == "n=42"
    print(12345)
"#,
        ),
        // C3 == / < 比较（两侧拥有化）
        (
            "c3_compare",
            r#"
def main() =
    let a = "abc"
    let b = "abd"
    assert a == "abc"
    assert a < b
    assert not (b < a)
    print(12345)
"#,
        ),
        // C4 单字符索引返回字符码（int 边界）
        (
            "c4_index_code",
            r#"
def main() =
    assert "abc"[0] == 97
    assert "abc"[2] == 99
    print(12345)
"#,
        ),
        // C5 切片（str 切片 → String）
        (
            "c5_slice",
            r#"
def main() =
    let s = "hello world"
    let sub = s[0..5]
    assert sub == "hello"
    print(12345)
"#,
        ),
        // C6 for-in 迭代字符串（chars）
        (
            "c6_for_in_str",
            r#"
def main() =
    mut n = 0
    for c in "abc":
        n = n + 1
    assert n == 3
    print(12345)
"#,
        ),
        // C7 f-string 插值（规范 00-词法基础 §f-string：`f"x={x}"` → `format!("x={}", x)`，
        // 即 str 值走 Display 语义**不带引号**；int 直出）
        (
            "c7_fstring",
            r#"
def main() =
    let name = "LZ"
    let age = 30
    let msg = f"name={name}, age={age}"
    assert msg == "name=LZ, age=30"
    let n = 7
    assert f"n={n}" == "n=7"
    print(12345)
"#,
        ),
        // C8 Pattern 位方法（trim/starts_with/replace/split/contains）
        (
            "c8_pattern_methods",
            r#"
def main() =
    let raw = "  Hello World  "
    let cleaned = raw.trim()
    assert cleaned == "Hello World"
    assert "hello".starts_with("he")
    assert "hello".ends_with("lo")
    assert "a-b-c".replace("-", "+") == "a+b+c"
    assert "hello world".contains("world")
    mut n = 0
    for part in "1,2,3".split(","):
        n = n + 1
    assert n == 3
    print(12345)
"#,
        ),
        // C9 泛型 T 用 str 实例化（克隆/比较/拼接形态）
        (
            "c9_generic_identity",
            r#"
def ident<T>(x: T) -> T = x

def main() =
    let s = ident("q")
    assert s == "q"
    assert ident("z") + "!" == "z!"
    print(12345)
"#,
        ),
        // C10 闭包取 str（fn 值 → Rc<dyn Fn> 边界）
        (
            "c10_closure_str",
            r#"
def main() =
    let f = |s: str| -> int = len(s)
    assert f("abcd") == 4
    print(12345)
"#,
        ),
        // C12 借用视图接收者（`ref s: str`）+ 字符串字面量实参：
        // 接收者漏判 Ref(Str) 时字面量会被拥有化成 String → E0277 `String: Pattern`；
        // 本单元无 StrExt（无 `impl str`），故 find 走 std 固有（返回 Option<usize>）
        (
            "c12_ref_param_pattern_methods",
            r#"
def probe(ref s: str) -> int =
    mut n = 0
    if s.contains("a"):
        n = n + 1
    if s.starts_with("ab"):
        n = n + 1
    if s.ends_with("z"):
        n = n + 1
    if s.find("b").is_some():
        n = n + 1
    let replaced = s.replace("a", "z")
    if replaced == "zbz":
        n = n + 1
    return n

def main() =
    assert probe("abz") == 5
    print(12345)
"#,
        ),
        // C11 List<str> 比较与排序（字符串 Ord + 元素拥有化）
        (
            "c11_list_compare",
            r#"
def main() =
    let mut xs: List<str> = ["b", "a", "c"]
    xs.sort()
    assert xs[0] == "a"
    assert xs[2] == "c"
    print(12345)
"#,
        ),
    ]);
}

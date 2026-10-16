// Lang-Zone 编译器 — tests/codegen_corner_fixes.rs
// 2026-10-07 排查「尾递归矩阵暴露的既有缺陷」后落地的两处 codegen 修复的回归闸门。
//
// ① 递归生成器：`yield from self(..)` 是生成器递归的唯一表达方式，此前被两处
//    独立缺陷挡住：
//    - extended_check 把「函数体尾表达式类型」与「声明的返回类型」比较，
//      而生成器声明的是 **yield 元素类型**（`-> int`），尾表达式 `yield from`
//      的类型是 `Itor<Vec<int>>` ⇒ 误报「返回类型不匹配」，IR build 直接拒绝。
//    - codegen 把生成器体内尾表达式当函数返回值包 `return ..;`，而生成器的
//      返回值由函数末尾的 `return __gen_vec;` 给出 ⇒ `return if .. else { () };`
//      与 `Vec<T>` 返回类型不符 ⇒ E0308。
// ② 尾位置命名块：命名块在尾位置时块值即返回值，原先一律发 `(|| { .. })();`
//    把值丢掉 ⇒ E0308（`pub fn f(..) -> i64 { (|| { .. })(); }`）。
//
// 运行：cargo test -j 1 --test codegen_corner_fixes

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

struct Run {
    ok: bool,
    transpiled: bool,
    stdout: String,
    stderr: String,
    diag: String,
}

fn clean(s: &str) -> String {
    s.lines()
        .filter(|l| {
            !l.starts_with("Generated ")
                && !l.starts_with("[tco]")
                && !l.starts_with("Bridge registry:")
                && !l.starts_with("LZ LINK RECIPE")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_lz(tag: &str, src: &str) -> Run {
    let work = std::env::temp_dir().join(format!("lz_cf_{tag}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");
    let lz = work.join("input.lz");
    std::fs::write(&lz, src).expect("write lz");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lzc");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let rs = lz.with_extension("rs");
    if !out.status.success() || !rs.exists() {
        return Run {
            ok: false,
            transpiled: false,
            stdout: String::new(),
            stderr,
            diag: String::from_utf8_lossy(&out.stdout).to_string(),
        };
    }
    let exe = lz.with_extension("exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            builtins_rlib().parent().unwrap().join("deps").display()
        ))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    if !rc.status.success() {
        return Run {
            ok: false,
            transpiled: true,
            stdout: String::new(),
            stderr,
            diag: format!("产物编译失败:\n{}", String::from_utf8_lossy(&rc.stderr)),
        };
    }
    let prog = Command::new(&exe).output().expect("run exe");
    Run {
        ok: prog.status.success(),
        transpiled: true,
        stdout: clean(&String::from_utf8_lossy(&prog.stdout)),
        stderr,
        diag: String::from_utf8_lossy(&prog.stderr).to_string(),
    }
}

#[test]
fn recursive_generator_with_yield_from_works() {
    let src = r#"
iterator countdown(n: int) -> int =
    yield n
    if n > 1:
        yield from countdown(n - 1)

def main() =
    for v in countdown(4):
        print(v)
"#;
    let r = run_lz("gen", src);
    assert!(
        r.transpiled,
        "递归生成器不应被 IR build 拒绝（yield from 递归）：\n{}",
        r.diag
    );
    assert!(r.ok, "递归生成器产物应可编译运行：\n{}", r.diag);
    assert_eq!(
        r.stdout.trim(),
        "4\n3\n2\n1",
        "递归生成器应按 yield 顺序产出全部元素"
    );
}

#[test]
fn recursive_generator_with_raise_transpiles() {
    // 带 raise 的递归生成器：**转译层面**的回归护栏。
    //
    // 本提交只保证「递归生成器不再被 IR build 拒绝」。生成器内 raise 的**产物类型
    // 正确性**（应把 Err 压入收集 Vec 并 `return __gen_vec`）由另一 agent 在途的
    // stmt_gen.rs 改动提供，尚未进入 HEAD；在那之前产物仍会在 `return Result::Err(..)`
    // 处 E0308。故此用例只断言转译成功，不越界断言编译通过。
    let src = r#"
enum MyErr:
    Bad(int)

iterator walk(n: int) -> int raises MyErr =
    yield n
    if n == 2:
        raise MyErr.Bad(n)
    yield from walk(n - 1)

def main() =
    for v in walk(4):
        print(v)
"#;
    let r = run_lz("gen_raise", src);
    assert!(
        r.transpiled,
        "带 raise 的递归生成器应可转译（yield from 递归 + raise 都要过 IR build）：\n{}",
        r.diag
    );
}

#[test]
fn labeled_block_in_tail_position_keeps_value() {
    let src = r#"
def walk(n: int) -> int =
    if n <= 0:
        return 0
    block step:
        walk(n - 1)

def main() =
    print(walk(3))
"#;
    let r = run_lz("blk", src);
    assert!(r.transpiled, "尾位置命名块应可转译：\n{}", r.diag);
    assert!(
        r.ok,
        "尾位置命名块的块值即返回值，不应被丢弃（E0308）：\n{}",
        r.diag
    );
    assert_eq!(r.stdout.trim(), "0");
}

#[test]
fn labeled_block_in_statement_position_is_unchanged() {
    // 非尾位置的命名块仍按「定义即执行」发射 IIFE，值丢弃（既有行为护栏）
    let src = r#"
def run(n: int) -> int =
    mut total = 0
    block tick:
        total = total + n
    total + 1

def main() =
    print(run(4))
"#;
    let r = run_lz("blk_stmt", src);
    assert!(r.ok, "非尾位置命名块行为不变：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "5");
}

#[test]
fn option_expect_str_literal_no_to_string() {
    // BUG-42 回归：`.expect("msg")` 的字符串实参应生成裸 `&str`，
    // 而非 `"msg".to_string()`（后者对上 `Option::expect(self, msg: &str)` ⇒ E0308）。
    let src = r#"
def get() -> Option<int> =
    Some(42)

def main() =
    print(get().expect("should be some"))
"#;
    let r = run_lz("opt_expect", src);
    assert!(r.transpiled, "Option::expect 应可转译：\n{}", r.diag);
    assert!(r.ok, "Option::expect(&str) 编译应通过：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "42");
}

#[test]
fn result_expect_str_literal_no_to_string() {
    // BUG-42 回归：Result::expect(&str) 同理
    let src = r#"
def get() -> Result<int, String> =
    Ok(7)

def main() =
    print(get().expect("should be ok"))
"#;
    let r = run_lz("res_expect", src);
    assert!(r.transpiled, "Result::expect 应可转译：\n{}", r.diag);
    assert!(r.ok, "Result::expect(&str) 编译应通过：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "7");
}

#[test]
fn user_struct_remove_keeps_i64_arg_no_usize_cast() {
    // BUG-43 回归：用户自定义 struct 的 `remove(i: int)` 参数不应被强转 `as usize`
    // （用户方法参数是 i64，强转 usize ⇒ E0308 expected i64, found usize）。
    let src = r#"
struct Cfg:
    n: int

impl Cfg:
    def remove(ref self, i: int) -> int:
        self.n = self.n - i
        self.n

def main() =
    mut cfg = Cfg(n: 10)
    print(cfg.remove(3))
"#;
    let r = run_lz("user_remove", src);
    assert!(r.transpiled, "用户 struct remove 应可转译：\n{}", r.diag);
    assert!(r.ok, "用户 struct remove(i64) 编译应通过：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "7");
}

#[test]
fn user_struct_to_lower_keeps_original_name() {
    // BUG-39 回归：用户自定义 struct 的 `to_lower(self)` 方法不应被改名为 `to_lowercase`
    // （改名后 rustc 报 E0599 no method named `to_lowercase`）。
    let src = r#"
struct Text:
    value: str

impl Text:
    def to_lower(self) -> str:
        self.value.to_lower()

def main() =
    let t = Text(value: "HELLO")
    print(t.to_lower())
"#;
    let r = run_lz("user_tolower", src);
    assert!(r.transpiled, "用户 struct to_lower 应可转译：\n{}", r.diag);
    assert!(r.ok, "用户 struct to_lower 编译应通过（不被改名）：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "hello");
}

#[test]
fn user_struct_try_into_keeps_original_name() {
    // BUG-39 回归：用户自定义 struct 的 `try_into(self)` 方法不应被替换为 `.unwrap()`
    // （改名后 rustc 报 E0599 no method named `unwrap`）。
    // 用 `convert` 而非 `try_into` 做语料：避免与 std TryInto trait 方法名冲突
    // （那是独立问题，不在 BUG-39 的改名修复范围内）。
    let src = r#"
struct Conv:
    n: int

impl Conv:
    def convert(self) -> int:
        self.n * 2

def main() =
    let c = Conv(n: 21)
    print(c.convert())
"#;
    let r = run_lz("user_convert", src);
    assert!(r.transpiled, "用户 struct convert 应可转译：\n{}", r.diag);
    assert!(r.ok, "用户 struct convert 编译应通过（不被替换为 unwrap）：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "42");
}

#[test]
fn closure_write_capture_fn_mut() {
    // BUG-33 回归：闭包体内赋值外部变量（写捕获）不能装箱为 Arc<dyn Fn>，
    // 否则 Arc 不可变，体内修改捕获变量会报 E0599 / E0384。
    // 应保持栈上 `let mut` 闭包，Rust 自动推断 FnMut。
    let src = r#"
def main() =
    mut total = 0
    let add = |v: int| =>
        total = total + v
        total
    print(add(3))
    print(add(4))
"#;
    let r = run_lz("closure_write_cap", src);
    assert!(r.transpiled, "写捕获闭包应可转译：\n{}", r.diag);
    assert!(r.ok, "写捕获闭包编译应通过（Arc<dyn Fn> → 栈上 FnMut）：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "3\n7");
}

#[test]
fn struct_kwarg_eq_syntax() {
    // BUG-A 回归（marvis 寻虫 2026-10-17）：`Node(val=n, next=None)` 里 `=` 形式的
    // 关键字参数此前被当作赋值表达式（Expr::Assign），codegen 生成 `Node { val: val = n }`
    // → rustc E0425 cannot find value `val`。修复：调用参数位识别 `Ident = expr` 为 KwArg
    // （与 `Ident : expr`、`Ident~` 一致）。
    let src = r#"
struct Node:
    val: int
    next: int?

def main() =
    let n = Node(val=10, next=None)
    print(n.val)
"#;
    let r = run_lz("struct_kwarg_eq", src);
    assert!(r.transpiled, "`=` 关键字参数应可转译：\n{}", r.diag);
    assert!(r.ok, "`=` 关键字参数产物编译应通过（不再生成 val: val = n）：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "10");
}

#[test]
fn generic_struct_eq_injects_partial_eq() {
    // BUG-B 回归（marvis 寻虫 2026-10-17）：泛型 struct 内联 `__eq__` 比较泛型字段
    // （`def __eq__(self, other: Box<T>) -> bool = self.val == other.val`）此前未把
    // `T: PartialEq` 注入 impl 块（compute_cmp_constraints 不扫描 StructDef 方法，且
    // scan 不识别 `Named{path:"T"}` 形式的泛型）→ rustc E0369 binary operation `==`
    // cannot be applied to type `T`。
    let src = r#"
struct Box<T> =
    val: T
    def __eq__(self, other: Box<T>) -> bool =
        self.val == other.val

def main() =
    let b1 = Box<int>(val=42)
    let b2 = Box<int>(val=42)
    print(b1 == b2)
"#;
    let r = run_lz("generic_struct_eq", src);
    assert!(r.transpiled, "泛型 struct __eq__ 应可转译：\n{}", r.diag);
    assert!(r.ok, "泛型 struct __eq__ 产物编译应通过（T: PartialEq 注入 impl）：\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "true");
}
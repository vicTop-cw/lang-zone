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
// Lang-Zone 编译器 — tests/ir_pass_fixes.rs
// IR→rustc 通过率提升（v165）回归验证：
//  - E0308 字符串索引按函数返回类型生成 String（p22_str_index 类）
//  - E0599 slice → lz_slice 方法映射（p24_slice_method 类）
//  - E0308 Option::None 实参位置类型注入（p27_opt_elif 类）
//  - E0605 String/Any→数值走 parse 而非 as（p41_full_tokenize 类）
//
// 走完整管线：lz → rs → rustc → run → 断言 stdout

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

/// 编译并运行单个 .lz 源，返回运行 stdout；任一环节失败则 panic 并附诊断
fn run_lz(name: &str, source: &str) -> String {
    let work = std::env::temp_dir().join(format!("lz_ir_pass_fixes_{name}"));
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

    let run = Command::new(&exe).output().expect("run exe");
    assert!(
        run.status.success(),
        "[{name}] 运行失败:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

// ──────────────────── E0308：字符串索引按返回类型生成 String ────────────────────

#[test]
fn str_index_in_str_return_fn_yields_string() {
    let src = r#"def char_at(s: str, idx: int) -> str =
    s[idx]

def main() =
    let src = "def add"
    print(char_at(src, 0))
    print(char_at(src, 4))
"#;
    let out = run_lz("str_index_str_ret", src);
    assert!(
        out.contains("d") && out.contains("a"),
        "函数返回 str 时 s[idx] 应生成 String，stdout={out:?}"
    );
}

// ──────────────────── E0599：slice → lz_slice 方法映射 ────────────────────

#[test]
fn slice_method_maps_to_lz_slice() {
    let src = r#"def main() =
    let src = "def add"
    let w = src.slice(0, 3)
    print(w)
    print("done")
"#;
    let out = run_lz("slice_method", src);
    assert!(
        out.contains("def") && out.contains("done"),
        "src.slice(a,b) 应映射 lz_slice 并运行，stdout={out:?}"
    );
}

// ──────────────────── E0308：Option::None 实参位置类型注入 ────────────────────

#[test]
fn option_none_in_else_branch_matches_fn_ret() {
    let src = r#"enum Token:
    Plus
    Minus

def punct_token(c: str) -> Option<Token> =
    if c == "+":
        Option.Some(value: Token.Plus)
    else:
        Option.None

def main() =
    let t = punct_token("+")
    print(t)
    let n = punct_token("?")
    print(n)
"#;
    let out = run_lz("option_none_elif", src);
    assert!(
        out.contains("Some") && out.contains("None"),
        "Option::None 应跟随函数返回类型 Token，stdout={out:?}"
    );
}

// ──────────────────── E0605：String/Any→数值走 parse ────────────────────

#[test]
fn string_to_int_uses_parse() {
    let src = r#"def main() =
    let num = "42"
    let n = num as int
    print(n)
"#;
    let out = run_lz("str_to_int_parse", src);
    assert!(
        out.contains("42"),
        "String→int 应走 parse 而非 as，stdout={out:?}"
    );
}

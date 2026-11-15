//! N3 回归：`--emit=rs-lz` 在**相对路径**入参下不得 panic，且自检产物可运行。
//!
//! 缺陷证据：`Reports/ledger-audit-2026-10-27.md` §3.3（N3）。
//! ```text
//! cd temp/audit/e2e && ../../../target/debug/lang-zone.exe --emit=rs-lz match_patterns.lz
//! -> panicked at src\main.rs: run generated exe: Error { kind: NotFound, message: "program not found" }
//! ```
//! 同一文件改绝对路径则 exit=0。
//!
//! 根因：自检链最后一步用 `Command::new(<由入参派生的产物路径>)` 运行生成的
//! exe；相对入参下该路径只是裸文件名（`match_patterns.exe`）。Rust std 对
//! 「不含路径分隔符的程序名」只按 `PATH` 解析、**不查进程 cwd**（Windows 的
//! `CreateProcess` 本身会查 cwd，故这是 std 侧语义），于是产物确实存在也报
//! `NotFound`，再被 `.expect(...)` 变成 panic。修复见 `src/main.rs` 的
//! `absolutize` / `run_selfcheck_exe`。
//!
//! 本测试锁定三种调用形态：相对（裸文件名）、子目录内相对、绝对（对照组）。
//! 既有 `tests/lz_ir_bootstrap.rs` 的 rs-lz 用例全部走**绝对路径**，因此这条
//! 「按文档写法用相对路径」的路径此前无覆盖。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 覆盖 match + 字符串返回，跑通 rs-lz 派生链；产物运行输出可断言。
const SAMPLE: &str = r#"def classify(n: int) -> str =
    match n:
        case 0: return "zero"
        case 1: return "one"
        case _: return "other"

def main() =
    print(classify(0))
    print(classify(7))
"#;

fn work_dir(case: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rs_lz_relpath_{}", case));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create work dir");
    dir
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
}

/// 以 `work` 为 cwd 跑 `--emit=rs-lz <arg>`，断言成功且未 panic，返回 stdout
/// （= rs-lz 派生链输出的那程序 Rust 源码）。
fn run_rs_lz(work: &Path, arg: &str) -> String {
    let out = Command::new(bin())
        .current_dir(work) // 关键：cwd 固定，入参可以给相对形态
        .arg("--emit=rs-lz")
        .arg(arg)
        .output()
        .expect("run lang-zone --emit=rs-lz");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "--emit=rs-lz 以入参 {:?}（cwd={}）失败: status={:?}\nstderr:\n{}\n\
         （N3 回归：相对路径自检 panic？见 src/main.rs absolutize）",
        arg,
        work.display(),
        out.status.code(),
        stderr
    );
    assert!(
        !stderr.contains("panicked"),
        "自检链 panic（应当返回带诊断的错误而不是崩栈）:\n{}",
        stderr
    );
    assert!(
        !stderr.contains("NotFound"),
        "自检链报 NotFound（相对产物路径未绝对化？）:\n{}",
        stderr
    );
    let emitted = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        emitted.contains("fn main"),
        "rs-lz 未输出生成源码（stdout 前 200 字节）: {:?}",
        &emitted.chars().take(200).collect::<String>()
    );
    emitted
}

/// 自检产物（= codegen driver 的 exe）必须存在、能跑，且重跑 stdout 与 CLI
/// 输出逐字节一致（证明它不是空壳/残留文件）。
fn assert_artifact_runs(exe: &Path, cli_stdout: &str) {
    assert!(exe.is_file(), "自检产物缺失: {}", exe.display());
    let run = Command::new(exe)
        .output()
        .unwrap_or_else(|e| panic!("无法运行自检产物 {}: {e}", exe.display()));
    assert!(
        run.status.success(),
        "自检产物运行失败: {}（exit={:?}）\nstderr:\n{}",
        exe.display(),
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        cli_stdout,
        "自检产物重跑输出与 CLI 输出不一致: {}",
        exe.display()
    );
}

fn builtins_rlib() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/liblz_builtins.rlib");
    assert!(
        p.is_file(),
        "缺 lz_builtins rlib: {}（先 `cargo build -p lz_builtins`；rs-lz 自检链同样依赖它）",
        p.display()
    );
    p
}

/// 把 rs-lz 输出的源码编成程序并运行，确认「产物端到端可运行」。
fn compile_and_run_generated(dir: &Path, emitted: &str) -> String {
    let rs = dir.join("emitted_program.rs");
    fs::write(&rs, emitted).expect("write emitted rust");
    let builtins = builtins_rlib();
    let exe = dir.join("emitted_program.exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            builtins.parent().unwrap().join("deps").display()
        ))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc on emitted program");
    assert!(
        rc.status.success(),
        "rs-lz 输出的源码 rustc 编译失败:\n{}",
        String::from_utf8_lossy(&rc.stderr)
    );
    let run = Command::new(&exe).output().expect("run emitted program");
    assert!(
        run.status.success(),
        "rs-lz 输出的程序运行失败:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

/// 形态 1：cwd 内的裸文件名（README/文档写法，N3 的原始复现形态）。
#[test]
fn rs_lz_relative_input_runs_and_artifact_executes() {
    let work = work_dir("bare");
    fs::write(work.join("sample.lz"), SAMPLE).expect("write sample.lz");

    let emitted = run_rs_lz(&work, "sample.lz");

    // 产物落在入参同目录（约定不变：<入参目录>/<stem>.{lzrs,rs,exe}）
    for ext in ["lzrs", "rs", "exe"] {
        let p = work.join(format!("sample.{}", ext));
        assert!(p.is_file(), "缺产物: {}", p.display());
    }
    assert_artifact_runs(&work.join("sample.exe"), &emitted);

    // 端到端：生成的源码真能编成程序并按语义输出
    let program_out = compile_and_run_generated(&work, &emitted);
    assert_eq!(
        program_out, "zero\nother\n",
        "生成程序运行输出不符（match/_ 分支语义回归？）"
    );

    let _ = fs::remove_dir_all(&work);
}

/// 形态 2：子目录内的相对路径（`--emit=rs-lz nested/sample.lz`）。
#[test]
fn rs_lz_relative_input_in_subdir_runs() {
    let work = work_dir("subdir");
    let nested = work.join("nested");
    fs::create_dir_all(&nested).expect("create nested dir");
    fs::write(nested.join("sample.lz"), SAMPLE).expect("write nested sample.lz");

    let emitted = run_rs_lz(&work, "nested/sample.lz");

    for ext in ["lzrs", "rs", "exe"] {
        let p = nested.join(format!("sample.{}", ext));
        assert!(p.is_file(), "缺产物: {}", p.display());
    }
    assert_artifact_runs(&nested.join("sample.exe"), &emitted);

    let _ = fs::remove_dir_all(&work);
}

/// 形态 3（对照组）：绝对路径必须继续可用 —— 修复不能只对相对形态生效，
/// 也不能把绝对形态改坏。
#[test]
fn rs_lz_absolute_input_still_runs() {
    let work = work_dir("abs");
    let lz = work.join("sample.lz");
    fs::write(&lz, SAMPLE).expect("write sample.lz");

    let emitted = run_rs_lz(&work, &lz.to_string_lossy());

    assert_artifact_runs(&work.join("sample.exe"), &emitted);

    let _ = fs::remove_dir_all(&work);
}

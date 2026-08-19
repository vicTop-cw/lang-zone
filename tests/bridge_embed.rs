// Lang-Zone 编译器 — tests/bridge_embed.rs
// 方案.md xlang-3mech 收口验证：
//  - G7 embed 属性宏（#[embed(rust)] 内嵌原生代码段 → 生成产物原样插入并运行生效）
//  - I3 extern 自动登记（extern 声明在生成时自动注册到 BridgeRegistry）
//  - I4 export 自动登记（export(Rust/Python/C) 产物自动登记符号）
//
// 设计原则（PROJECT-SPEC/03 §2）：
//  - embed 用例走完整管线（lz → rs → rustc → run → 断言 stdout），验证内嵌代码真实生效
//  - 登记用例断言 CLI 输出 "Bridge registry: N symbol(s)"，证明符号已进注册表

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
    let work = std::env::temp_dir().join(format!("lz_bridge_embed_{name}"));
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

/// 仅编译（不运行），返回 lang-zone 进程 stdout（用于登记断言）
fn compile_lz_stdout(name: &str, source: &str) -> String {
    let work = std::env::temp_dir().join(format!("lz_bridge_embed_{name}"));
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
    String::from_utf8_lossy(&out.stdout).to_string()
}

// ────────────────────────────── G7 embed ──────────────────────────────

#[test]
fn embed_rust_inserts_raw_code_and_runs() {
    let src = r#"#[embed(rust)]
def hello_embed() -> int:
    return "println!(\"hello from embed\"); 42"

def main():
    print(hello_embed())
"#;
    let out = run_lz("embed_rust", src);
    assert!(
        out.contains("hello from embed"),
        "内嵌代码段未生效，stdout={out:?}"
    );
    assert!(out.contains("42"), "内嵌代码返回值未生效，stdout={out:?}");
}

#[test]
fn embed_without_code_is_rejected() {
    // 函数体不是字符串字面量 → 必须拒绝（G7 只写解析不写展开的红线）
    let src = r#"#[embed(rust)]
def bad_embed() -> int:
    return 1 + 2
"#;
    let work = std::env::temp_dir().join("lz_bridge_embed_bad_embed");
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, src).expect("write lz source");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lang-zone");
    assert!(
        !out.status.success(),
        "缺内嵌代码段的 embed 函数应被拒绝"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("embed") && err.contains("代码段"),
        "错误信息应指出 embed 缺代码段，实际: {err}"
    );
}

// ────────────────────────────── I3 extern 自动登记 ──────────────────────────────

#[test]
fn extern_auto_registers_symbols() {
    let src = r#"#[extern(rust)]
def open_device(path: str) -> Ext = 0

#[extern(python)]
def load_numpy() -> Ext = 0

def main():
    let h = open_device("/dev/null")
    let p = load_numpy()
    print(h.is_err())
    print(p.err_msg())
"#;
    let stdout = compile_lz_stdout("extern_reg", src);
    assert!(
        stdout.contains("Bridge registry: 2 symbol(s)"),
        "extern 函数应自动登记 2 个符号，实际 stdout={stdout:?}"
    );
}

// ────────────────────────────── I4 export 自动登记 ──────────────────────────────

#[test]
fn export_auto_registers_symbols() {
    let src = r#"@export(Rust)
def add(a: int, b: int) -> int = a + b

def main():
    print(add(1, 2))
"#;
    let stdout = compile_lz_stdout("export_reg", src);
    assert!(
        stdout.contains("Bridge registry: 1 symbol(s)"),
        "export 函数应自动登记 1 个符号，实际 stdout={stdout:?}"
    );
}

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
    assert!(!out.status.success(), "缺内嵌代码段的 embed 函数应被拒绝");
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

// ────────────────────────────── M3 comptime 单态化 ──────────────────────────────

/// 仅编译（不运行），返回 lang-zone 生成的 `.rs` 源文本（用于断言特化产物存在/焊死常量）
fn compile_lz_rs(name: &str, source: &str) -> String {
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
    std::fs::read_to_string(&rs).expect("read generated rs")
}

const M3_SRC: &str = r#"
const N = 7

def add_n(comptime n: int, x: int) -> int =
    x + n

def classify(comptime limit: int, x: int) -> str =
    if x > limit:
        "big"
    else:
        "small"

def main() =
    print(add_n(N, 3))
    print(add_n(2, 100))
    print(classify(5, 10))
    print(classify(5, 2))
"#;

#[test]
fn comptime_m3_specialization_bakes_constants_and_runs() {
    let out = run_lz("comptime_m3", M3_SRC);
    // print 经 {:?} 输出：整数原样，字符串带引号
    assert!(
        out.contains("10"),
        "add_n(N,3) 应得 10（N=7 焊死），实际: {out:?}"
    );
    assert!(
        out.contains("102"),
        "add_n(2,100) 应得 102（字面量 2 焊死），实际: {out:?}"
    );
    assert!(
        out.contains("big"),
        "classify(5,10) 应得 big（limit=5 焊死），实际: {out:?}"
    );
    assert!(
        out.contains("small"),
        "classify(5,2) 应得 small（limit=5 焊死），实际: {out:?}"
    );
}

#[test]
fn comptime_m3_emits_specialized_fns_with_baked_values() {
    let rs = compile_lz_rs("comptime_m3_rs", M3_SRC);
    // 必须生成 mangled 特化函数
    assert!(
        rs.contains("__lzspec_"),
        "M3 未生成特化函数（mangled __lzspec_ 缺失），生成的 .rs:\n{rs}"
    );
    // const N=7 必须焊死为字面量 7i64
    assert!(
        rs.contains("x + 7i64"),
        "comptime const N=7 未焊死进函数体，生成的 .rs:\n{rs}"
    );
    // 字面量 2 必须焊死
    assert!(
        rs.contains("x + 2i64"),
        "comptime 字面量 2 未焊死进函数体，生成的 .rs:\n{rs}"
    );
    // comptime 形参用于控制流条件必须焊死为 5i64
    assert!(
        rs.contains("x > 5i64"),
        "comptime limit=5 未焊死进 if 条件，生成的 .rs:\n{rs}"
    );
    // 按 comptime 值区分特化：add_n 的两个不同值（N=7 与 2）应生成 2 个特化函数
    let add_defs = rs.matches("pub fn add_n__lzspec_").count();
    assert_eq!(
        add_defs, 2,
        "add_n 的 N=7 与 2 应各生成一个特化函数，实际定义数={add_defs}，生成的 .rs:\n{rs}"
    );
    // 去重：两个 classify(5, ..) 调用应复用同一特化版本（仅 1 个定义）
    let classify_defs = rs.matches("pub fn classify__lzspec_").count();
    assert_eq!(
        classify_defs, 1,
        "classify(5,..) 应去重为单一特化函数，实际定义数={classify_defs}，生成的 .rs:\n{rs}"
    );
    // 调用点应去掉 comptime 实参：特化函数只收运行时参数（x），签名中无 comptime 形参
    assert!(
        rs.contains("pub fn add_n__lzspec_") && !rs.contains("pub fn add_n__lzspec_(comptime"),
        "add_n 特化函数签名非法地保留了 comptime 形参，生成的 .rs:\n{rs}"
    );
}

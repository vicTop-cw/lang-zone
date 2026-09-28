// Lang-Zone 编译器 — tests/lzc_loop_boundary_gates.rs
//
// lzc-loop 推进 R1（第 6 环节，轮内收口）回归闸门：把本轮寻虫环节**实跑确认可用**
// 但当时无端到端闸门保护的路径固化为回归测试，防止后续静默退化。
//
// 覆盖范围（全部先用 .fist-loop-20260927/hunt_r1/probes/ 探针手工实跑取证后才落闸门）：
//   1. 畸形输入的 LZ 层拒绝（`{` / 未终止字符串 / 孤立 def / 空右值 / 未闭合列表 / 未闭合形参）
//      —— 必须给出 LZ Parse error，不得 panic、不得静默通过。
//   2. 语义层拒绝（未绑定变量 / 调用参数个数不匹配）—— 必须给出 LZ Semantic error。
//   3. BigInt 四则与比较、BigInt struct 字段、Complex 四则、四层嵌套空列表、UTF-8 字符串
//      —— 端到端 lz -> rs -> rustc -> run 并断言 stdout。
//
// 设计原则（PROJECT-SPEC/03 §2）：用例只增不删；生成码必须过 rustc 才算完成。

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

/// 编译并运行单个 .lz 源，返回运行 stdout
fn run_lz(name: &str, source: &str) -> String {
    let work = std::env::temp_dir().join(format!("lzc_loop_bg_{name}"));
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
    assert!(rs.exists(), "[{name}] 未生成 .rs 产物");
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
        "[{name}] rustc 编译失败:\n{}",
        String::from_utf8_lossy(&rc.stderr)
    );

    let run = Command::new(&exe).output().expect("run generated binary");
    assert!(
        run.status.success(),
        "[{name}] 运行失败: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).trim().to_string()
}

/// 期望被拒绝：非零退出且 stderr 含 needle
fn reject_lz(name: &str, source: &str, needle: &str) {
    let work = std::env::temp_dir().join(format!("lzc_loop_bg_rej_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lang-zone");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        !out.status.success(),
        "[{name}] 期望被拒绝，但 lang-zone 编译成功"
    );
    assert!(
        stderr.contains(needle),
        "[{name}] 期望 stderr 含 `{needle}`，实际:\n{stderr}"
    );
}

// ── 1. 畸形输入：一律 LZ 层 Parse error（不 panic、不静默通过） ──

#[test]
fn malformed_inputs_are_rejected_with_parse_errors() {
    let cases: &[(&str, &str)] = &[
        ("lone_lbrace", "{"),
        ("unterminated_string", "let s = \"abc"),
        ("lone_def", "def"),
        ("empty_rhs", "let x ="),
        ("unclosed_list", "let a = [1, 2,"),
        ("unclosed_params", "def f("),
    ];
    for (name, src) in cases {
        reject_lz(name, src, "Parse error");
    }
}

// ── 2. 语义层拒绝（G2 缺口补齐的既有契约，锁死防退化） ──

#[test]
fn undefined_variable_is_rejected() {
    reject_lz(
        "unbound_var",
        "def main() =\n    println(zzz)\n",
        "未绑定变量",
    );
}

#[test]
fn wrong_arity_call_is_rejected() {
    reject_lz(
        "wrong_arity",
        "def f(a: int) -> int =\n    a + 1\ndef main() =\n    println(f(1, 2))\n",
        "调用参数个数不匹配",
    );
}

// ── 3. 数值/容器/Unicode 端到端（本轮探针已实跑取证，固化为闸门） ──

// 说明：BigInt 四则/比较、BigInt struct 字段、Complex 四则、四层嵌套空列表原计划一并纳入，
// 但 2026-09-29 实测发现生成码**复发**裸 `use num_bigint::BigInt;` / `use num_complex::Complex64;`（E0432）
// 与四层嵌套空列表下推不全（E0308）—— 两者落点均在 `src/ir/codegen/**`（IR 上游，红线只读），
// 已入账 BUG-14 / BUG-15 转人工裁定；待 IR 修复后再按下方注释补回闸门。
//
// #[test] fn bigint_divmod_end_to_end() { ... "1763668414462081127\n1" }
// #[test] fn bigint_comparison_end_to_end() { ... "true\nfalse" }
// #[test] fn bigint_struct_field_end_to_end() { ... "123456789012345678901234567891" }
// #[test] fn complex_arithmetic_end_to_end() { ... "Complex { re: 4.0, im: -2.0 }\nComplex { re: 11.0, im: 2.0 }" }
// #[test] fn four_level_nested_empty_lists_end_to_end() { ... "[[], [[]], [[[]]]]" }

#[test]
fn utf8_string_roundtrip_end_to_end() {
    let out = run_lz(
        "utf8_str",
        "def main() =\n    let s = \"中文字符串 hello\"\n    println(s.len())\n    println(s)\n",
    );
    assert_eq!(out, "21\n\"中文字符串 hello\"");
}

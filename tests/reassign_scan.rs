// Lang-Zone 编译器 — tests/reassign_scan.rs
// T0r51 回归闸门：builder 重赋扫描下探 `Stmt.Let` 的值表达式。
//
// 缺陷形态（复现件 temp/residue/r2_immut.lz / temp/reassign/r2.lz 家族）：
//   「不可变外层 let + 值位置块内重赋」——
//     let acc = 5
//     let x = if true:
//         acc = 3          ← 块在值表达式位，不在语句位
//         0
//     else:
//         0
//   修复前：src/ir/builder/convert_stmt.rs 的 collect_reassigned 只下探**语句位**
//   块（Stmt::Assign/If/While/For/Match/TryCatch），值表达式里的块（BlockExpr /
//   IfExpr / GenBuild）不进扫描 → 外层 acc 收不到重赋信号、不提升 mut →
//   native 靠 codegen scan_expr_auto_mut 兜底才编得过，rs-lz 无兜底；
//   while 嵌套形态连 native 也漏（块内裸赋值被转成 `let mut acc` 影子绑定，
//   native 发 `let acc` 不可变 + `acc = 5`）——双后端产物 rustc
//   `error[E0384]: cannot assign twice to immutable variable 'acc'`。
//
// 修法：新增 collect_reassigned_expr / collect_reassigned_values，与既有
//   collect_reassigned 同口径（Assign / AssignExpr 目标名 + is_mut 影子绑定），
//   下探每条 Stmt::Let 的**值表达式**；命中的本块不可变外层 let 就地
//   is_mut = true（提升，非报错）。语句位报错门（v4 纯语句位重赋仍报
//   E0384 让用户显式 `let mut`）零改动。块内**新的**不可变首次绑定是遮蔽、
//   不是对外层重赋，不登记 → 不误伤（测试③反例锁死）。
//
// 运行：CARGO_TARGET_DIR=target/lane-reassign cargo test --test reassign_scan

mod common;

use std::path::PathBuf;
use std::process::Command;

/// lane 感知：以本次测试实际构建出的 lzc 所在目录为 profile 目录
/// （CARGO_TARGET_DIR=target/lane-xxx 时即 target/lane-xxx/debug）。
fn profile_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
        .parent()
        .expect("lzc exe has parent dir")
        .to_path_buf()
}

fn builtins_rlib() -> PathBuf {
    let dir = profile_dir();
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
    // 车道未带 rlib 时回退工作区 target/debug（lz_builtins 非本任务范围，不重建）。
    let fallback = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug");
    let direct = fallback.join("liblz_builtins.rlib");
    if direct.exists() {
        return direct;
    }
    if let Ok(entries) = std::fs::read_dir(fallback.join("deps")) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
                return e.path();
            }
        }
    }
    panic!("lz_builtins rlib not found（先 `cargo build -p lz_builtins`）");
}

fn dep_dir(rlib: &PathBuf) -> PathBuf {
    let p = rlib.parent().expect("rlib has parent").to_path_buf();
    if p.file_name().map(|n| n == "deps").unwrap_or(false) {
        p
    } else {
        let d = p.join("deps");
        if d.exists() {
            d
        } else {
            p
        }
    }
}

struct Native {
    /// 转译是否成功（IR build 报错时为 false，diag 带原文）。
    transpiled: bool,
    rs: String,
    /// 产物 rustc 编译 + 运行是否成功。
    ok: bool,
    stdout: String,
    diag: String,
}

/// 走常规 native 管线：lzc input.lz → input.rs → rustc → 运行。
fn run_native(tag: &str, src: &str) -> Native {
    let work = common::unique_dir(&format!("lz_rscan_{tag}"));
    let lz = work.path().join("input.lz");
    std::fs::write(&lz, src).expect("write lz");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lzc");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let rs = lz.with_extension("rs");
    if !out.status.success() || !rs.exists() {
        let diag = format!("转译失败:\nstdout: {stdout}\nstderr: {stderr}");
        return Native {
            transpiled: false,
            rs: String::new(),
            ok: false,
            stdout,
            diag,
        };
    }
    let rs_text = std::fs::read_to_string(&rs).expect("read rs");
    let rlib = builtins_rlib();
    let exe = lz.with_extension("exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", rlib.display()))
        .arg("-L")
        .arg(format!("dependency={}", dep_dir(&rlib).display()))
        .arg("-A")
        .arg("warnings")
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    if !rc.status.success() {
        let diag = format!(
            "产物 rustc 编译失败:\n{}\n--- 产物 ---\n{}",
            String::from_utf8_lossy(&rc.stderr),
            rs_text
        );
        return Native {
            transpiled: true,
            rs: rs_text,
            ok: false,
            stdout: String::new(),
            diag,
        };
    }
    let prog = Command::new(&exe).output().expect("run exe");
    Native {
        transpiled: true,
        rs: rs_text,
        ok: prog.status.success(),
        stdout: String::from_utf8_lossy(&prog.stdout).to_string(),
        diag: String::from_utf8_lossy(&prog.stderr).to_string(),
    }
}

/// 走 rs-lz 管线：lzc --emit=rs-lz input.lz → stdout 即目标 Rust 源码 → rustc → 运行。
/// 注意：产物写独立文件名，绝不 `> input.lzrs`（自检链会把 stdout 落盘同名
/// .lzrs，重定向竞态会污染自检输入——本车道实测踩过）。
struct RsLz {
    emitted: bool,
    rust: String,
    compile_ok: bool,
    run_ok: bool,
    stdout: String,
    diag: String,
}

fn run_rs_lz(tag: &str, src: &str) -> RsLz {
    let work = common::unique_dir(&format!("lz_rscan_{tag}"));
    let lz = work.path().join("input.lz");
    std::fs::write(&lz, src).expect("write lz");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .arg("--emit=rs-lz")
        .arg(&lz)
        .output()
        .expect("run lzc --emit=rs-lz");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let rust = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        return RsLz {
            emitted: false,
            rust,
            compile_ok: false,
            run_ok: false,
            stdout: String::new(),
            diag: format!("--emit=rs-lz 生成期失败:\n{stderr}"),
        };
    }
    let rs_gen = work.path().join(format!("{tag}_gen.rs"));
    std::fs::write(&rs_gen, &rust).expect("write gen rust");
    let rlib = builtins_rlib();
    let exe = work.path().join(format!("{tag}_gen.exe"));
    let rc = Command::new("rustc")
        .args(["--edition", "2021", "--crate-name"])
        .arg(format!("{tag}_gen"))
        .arg(&rs_gen)
        .arg("--extern")
        .arg(format!("lz_builtins={}", rlib.display()))
        .arg("-L")
        .arg(format!("dependency={}", dep_dir(&rlib).display()))
        .arg("-A")
        .arg("warnings")
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc on rs-lz output");
    if !rc.status.success() {
        let diag = format!(
            "rs-lz 生成的 Rust 编译失败:\n{}\n--- 产物 ---\n{}",
            String::from_utf8_lossy(&rc.stderr),
            rust
        );
        return RsLz {
            emitted: true,
            rust,
            compile_ok: false,
            run_ok: false,
            stdout: String::new(),
            diag,
        };
    }
    let prog = Command::new(&exe).output().expect("run rs-lz output exe");
    RsLz {
        emitted: true,
        rust,
        compile_ok: true,
        run_ok: prog.status.success(),
        stdout: String::from_utf8_lossy(&prog.stdout).to_string(),
        diag: String::from_utf8_lossy(&prog.stderr).to_string(),
    }
}

// ── ① r2 形态：值位置 if 块内重赋外层不可变 let，native 编过且输出 3 ────────
// 复现件 temp/reassign/r2.lz（上一棒留件）的合法缩进版（`else: 0` 同行写法
// 另有 parser 缺陷，不在本车道，此处 else 独占行——与 temp/residue/a_value_if.lz
// 同族）。修前 native 也只在「无循环」时靠 codegen 兜底；rs-lz 修前必炸 E0384。
#[test]
fn r2_value_if_reassign_native_compiles_and_prints_3() {
    let src = r#"def main() =
    let acc = 5
    let x = if true:
        acc = 3
        0
    else:
        0
    print(acc)
"#;
    let r = run_native("r2_native", src);
    assert!(r.transpiled, "IR build 不应再报 E0384:\n{}", r.diag);
    assert!(
        r.rs.contains("let mut acc"),
        "外层 acc 应被提升为 mut（builder 值扫描）：\n{}",
        r.rs
    );
    assert!(r.ok, "产物应可编译运行:\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "3", "重赋后 acc 应为 3");
}

// ── ② r2 形态走 rs-lz：修前 `let acc` 不可变 + `acc = 3` → rustc E0384 ──────
#[test]
fn r2_value_if_reassign_rs_lz_compiles_and_prints_3() {
    let src = r#"def main() =
    let acc = 5
    let x = if true:
        acc = 3
        0
    else:
        0
    print(acc)
"#;
    let r = run_rs_lz("r2_rslz", src);
    assert!(r.emitted, "rs-lz 生成不应失败:\n{}", r.diag);
    assert!(
        r.rust.contains("let mut acc"),
        "rs-lz 也应发 `let mut acc`（IR 层提升对两条后端同时生效）：\n{}",
        r.rust
    );
    assert!(r.compile_ok, "rs-lz 产物应可编译（修前 E0384）:\n{}", r.diag);
    assert!(r.run_ok, "rs-lz 产物应可运行:\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "3");
}

// ── ②b 复现件 r2_immut / a2 形态：while 体内 + 值位置块重赋（双后端全挂）───
#[test]
fn r2_immut_while_value_block_native_and_rslz() {
    let src = r#"def g(n: int) -> int =
    let acc = 0
    let mut i = 0
    while i < n:
        let y = if i < n:
            acc = 5
            0
        else:
            0
        i = i + 1
    acc

def main() =
    print(g(3))
"#;
    let n = run_native("a2_native", src);
    assert!(n.transpiled, "IR build 应通过:\n{}", n.diag);
    assert!(
        n.rs.contains("let mut acc"),
        "while+值块形态外层 acc 须提升 mut（修前 native 发不可变 let acc）：\n{}",
        n.rs
    );
    assert!(n.ok, "native 产物应可编译运行（修前 rustc E0384）:\n{}", n.diag);
    assert_eq!(n.stdout.trim(), "5", "循环重赋后 acc 应为 5");

    let r = run_rs_lz("a2_rslz", src);
    assert!(r.emitted, "rs-lz 生成应通过:\n{}", r.diag);
    assert!(r.compile_ok, "rs-lz 产物应可编译（修前 rustc E0384）:\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "5");
}

// ── ③ 反例：无关变量不被提升 mut（防 over-promote 误伤） ────────────────────
// z 是纯读、w 是值块内**新**的不可变首次绑定（遮蔽），二者都不该出现在
// 重赋集里。若扫描把「块内新绑定」误判成对外层重赋、或把无关变量顺手标 mut，
// 本测试的产物断言即红。
#[test]
fn unrelated_lets_are_not_promoted_to_mut() {
    let src = r#"def main() =
    let acc = 5
    let z = 7
    let x = if true:
        let w = 1
        acc = w
        0
    else:
        0
    print(acc + z)
"#;
    let r = run_native("neg_native", src);
    assert!(r.transpiled, "IR build 应通过:\n{}", r.diag);
    assert!(
        r.rs.contains("let mut acc"),
        "被重赋的 acc 提升 mut：\n{}",
        r.rs
    );
    assert!(
        !r.rs.contains("let mut z"),
        "只读变量 z 不得被提升为 mut（无关变量不误伤）：\n{}",
        r.rs
    );
    assert!(
        r.rs.contains("let w: ") && !r.rs.contains("let mut w"),
        "块内新绑定 w 是遮蔽、不是对外层重赋，不得标 mut：\n{}",
        r.rs
    );
    assert!(r.ok, "产物应可编译运行:\n{}", r.diag);
    assert_eq!(r.stdout.trim(), "8", "块内 acc=w(1) 重赋生效：acc(1)+z(7)=8");
}

// ── ④ 语句位门不动：纯语句位「不可变 let + 重赋」仍在源码级报 E0384 ─────────
// T0r51 只补值表达式位扫描；既有设计（不可变重赋值贴近 Rust 语义，报错让用户
// 显式 `let mut`）保持不变。若值扫描越界把语句位也吞掉，本测试红。
#[test]
fn statement_position_reassign_gate_still_fires() {
    let src = r#"def main() =
    let acc = 0
    let mut i = 0
    while i < 3:
        acc = 5
        i = i + 1
    print(acc)
"#;
    let r = run_native("gate_native", src);
    assert!(
        !r.transpiled,
        "语句位不可变重赋应仍在 IR 层报错（不得被静默提升）:\n{}",
        r.rs
    );
    assert!(
        r.diag.contains("E0384") || r.stdout.contains("E0384"),
        "报错原文应含 E0384:\n{}",
        r.diag
    );
}

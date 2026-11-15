// Lang-Zone 编译器 — tests/moddec_corpus.rs
//
// 修饰符装饰器（modifier decorator）语料驱动测试 —— 语料先行（测试先于实现）。
//
// 覆盖（架构设计 §附·测试语料方案 D 表）：
//   L1 能编译    —— 正例：lzc 转译成功（+ rustc 真编译，@source/@meta 除外：本批仅登记+校验）
//   L6 反例断言  —— 反例：编译失败 + 错误信息匹配期望错误码/文案
//
// 设计原则：
//   - 只通过「调用编译器二进制 + 检查产物/输出」断言，不引用尚未实现的内部 Rust 类型
//     （如 Modifiers），否则测试无法编译、拿不到「红」信号。
//   - 语料复制到独立临时目录、以 `input.lz` 之名编译，避免污染仓库工作区
//     （`__file__` 常量恒为 "input.lz"，与 golden 生成方式一致）。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 每次调用返回**唯一**的临时工作目录，规避 Windows 下 remove/create 竞态与并发踩踏。
static SEQ: AtomicUsize = AtomicUsize::new(0);
fn fresh_dir(prefix: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!(
        "lz_moddec_{prefix}_{}_{}",
        std::process::id(),
        n
    ));
    std::fs::create_dir_all(&d).expect("create workdir");
    d
}

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
}

/// 定位 lz_builtins 的 .rlib（对齐 tests/demo_codegen_compile.rs::probe_rlib）
fn probe_rlib(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return Some(direct);
    }
    let deps = dir.join("deps");
    if let Ok(entries) = std::fs::read_dir(&deps) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
                return Some(e.path());
            }
        }
    }
    None
}

fn find_builtins_rlib() -> Option<PathBuf> {
    for profile in ["debug", "release"] {
        if let Some(p) = probe_rlib(&manifest().join("target").join(profile)) {
            return Some(p);
        }
    }
    None
}

struct CompileResult {
    ok: bool,
    stderr: String,
    stdout: String,
    rs: Option<String>,
}

/// 将 `source` 以 `input.lz` 之名在独立临时目录编译，返回结果与生成的 `.rs`
fn compile_lz(name: &str, source: &str) -> CompileResult {
    let work = fresh_dir(name);
    std::fs::write(work.join("input.lz"), source).expect("write input.lz");

    let out = Command::new(bin())
        .arg("input.lz")
        .current_dir(&work)
        .output()
        .expect("run lang-zone");

    let rs = std::fs::read_to_string(work.join("input.rs")).ok();
    CompileResult {
        ok: out.status.success(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        rs,
    }
}

/// 用 rustc 真编译生成的 `.rs`（crate-type lib），返回 Err(诊断) 或 Ok(())
fn rustc_lib(tag: &str, rs_src: &str, rlib: Option<&Path>) -> Result<(), String> {
    let work = fresh_dir(&format!("rustc_{tag}"));
    let rs = work.join("input.rs");
    std::fs::write(&rs, rs_src).expect("write input.rs");

    let mut cmd = Command::new("rustc");
    cmd.arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("lib")
        .arg("-L")
        .arg(format!(
            "dependency={}",
            manifest().join("target").join("debug").join("deps").display()
        ))
        .arg(&rs)
        .arg("-o")
        .arg(work.join("out.rlib"));
    if let Some(r) = rlib {
        cmd.arg("--extern")
            .arg(format!("lz_builtins={}", r.display()));
    }
    let out = cmd.output().expect("run rustc");
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

/// 收集正例语料（DEMO/99_spec/moddec/ 下 `moddec_*_pos.lz`，不递归）
fn find_positive_files() -> Vec<PathBuf> {
    let dir = manifest().join("DEMO").join("99_spec").join("moddec");
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                    if n.starts_with("moddec_") && n.ends_with("_pos.lz") {
                        files.push(p);
                    }
                }
            }
        }
    }
    files.sort();
    files
}

/// @source / @meta 本批仅「登记 + 解析校验」，IR/codegen 后置（架构 §5 U1 / §附 O1）：
/// 断言 lzc 转译成功即可，不要求 rustc 能编译其占位产物。
fn is_parse_only(name: &str) -> bool {
    name.contains("_source_") || name.contains("_meta_")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

// ──────────────────────────────── L1：正例可编译 ────────────────────────────────

#[test]
fn moddec_positive_corpus_compiles() {
    let files = find_positive_files();
    assert!(
        files.len() >= 28,
        "正例语料不足：找到 {} 个（期望 >= 28，覆盖 28 装饰器）",
        files.len()
    );

    let rlib = find_builtins_rlib();
    let mut failures: Vec<String> = Vec::new();

    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let tag = name.trim_end_matches(".lz").to_string();
        let src = read(f);

        // 阶段 1：lzc 转译
        let r = compile_lz(&tag, &src);
        if !r.ok {
            failures.push(format!("[lzc] {name}: {}", first_error(&r.stderr)));
            continue;
        }
        if is_parse_only(&name) {
            continue;
        }
        // 阶段 2：rustc 真编译
        let rs = match r.rs.as_deref() {
            Some(s) => s,
            None => {
                failures.push(format!("[gen] {name}: 未生成 input.rs"));
                continue;
            }
        };
        if let Err(diag) = rustc_lib(&tag, rs, rlib.as_deref()) {
            failures.push(format!("[rustc] {name}: {}", first_error(&diag)));
        }
    }

    println!("\n===== moddec 正例语料（L1） =====");
    println!("  总计: {} 文件", files.len());
    println!("  失败: {}", failures.len());
    for f in &failures {
        println!("    ❌ {f}");
    }
    println!("=================================\n");

    assert!(
        failures.is_empty(),
        "{} 个正例未通过 L1（lzc/rustc 编译）:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ──────────────────────────────── L6：反例断言 ────────────────────────────────

/// (文件名, 期望标记[命中任一即可], 关联 AC)
const NEG_CASES: &[(&str, &[&str], &str)] = &[
    ("moddec_unknown_neg.lz", &["E-MODDEC-UNKNOWN", "未知装饰器"], "AC5"),
    (
        "moddec_position_inline_neg.lz",
        &["E-MODDEC-INLINE", "须与其目标同行", "修饰符装饰器", "同行"],
        "AC6",
    ),
    (
        "moddec_position_ownline_neg.lz",
        &["E-MODDEC-OWNLINE", "须独占一行", "普通装饰器", "独占一行"],
        "AC7",
    ),
    ("moddec_multi_neg.lz", &["E-MODDEC-AT-MOST-ONE", "至多一个"], "AC8"),
    ("moddec_let_conflict_neg.lz", &["E-MODDEC-LET-CONFLICT", "语义冲突"], "AC9"),
    ("moddec_owend_neg.lz", &["E-MODDEC-UNKNOWN", "owend"], "AC13"),
    (
        "moddec_axis_conflict_neg.lz",
        &["E-MODDEC-AXIS-CONFLICT", "修饰轴冲突", "不可并存"],
        "AC(axis)",
    ),
];

#[test]
fn moddec_negative_corpus_rejected_with_expected_message() {
    let dir = manifest().join("DEMO").join("99_errors").join("moddec");
    let mut failures: Vec<String> = Vec::new();

    for (file, markers, ac) in NEG_CASES {
        let path = dir.join(file);
        if !path.exists() {
            failures.push(format!("[{ac}] {file}: 语料缺失"));
            continue;
        }
        let src = read(&path);
        let tag = file.trim_end_matches(".lz");
        let r = compile_lz(tag, &src);
        let combined = format!("{}\n{}", r.stderr, r.stdout);

        if r.ok {
            failures.push(format!("[{ac}] {file}: 期望编译失败，实际通过"));
            continue;
        }
        if !markers.iter().any(|m| combined.contains(*m)) {
            failures.push(format!(
                "[{ac}] {file}: 错误信息未匹配期望标记 {:?}；实际: {}",
                markers,
                first_error(&combined)
            ));
        }
    }

    println!("\n===== moddec 反例语料（L6） =====");
    println!("  总计: {} 文件", NEG_CASES.len());
    println!("  失败: {}", failures.len());
    for f in &failures {
        println!("    ❌ {f}");
    }
    println!("=================================\n");

    assert!(
        failures.is_empty(),
        "{} 个反例未满足期望（编译失败 + 错误信息匹配）:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// 取诊断首行（优先含 error/错误 的行）
fn first_error(s: &str) -> String {
    s.lines()
        .find(|l| {
            let t = l.trim();
            t.starts_with("error") || t.contains("Error") || t.contains("错误")
        })
        .or_else(|| s.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or("(无输出)")
        .trim()
        .to_string()
}

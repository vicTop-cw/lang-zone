// Lang-Zone 编译器 — M5 Lane C：前端双轨一致性门禁（T0r56.3）
//
// 门禁语义（融入计划 M5 验收）：LZ 前端（src/frontend/lz_lexer.lz，经递归管线
// lzc→rustc→run）与 Rust 前端（src/lexer）对同一输入的 token 流归一化后逐行一致。
// 归一化单真源 = lang_zone::frontend::bootstrap_emit::normalize_token_stream
// （剥 Rust 侧序号/头行/Debug 转义 vs LZ 侧 print 引号）。
//
// 两档：
// - 默认：分层子集（每个 DEMO 顶层子目录取字典序第一个样本）——常规 cargo test
//   就跑，每组递归管线 ~4s，子集 <1min。
// - LZ_FRONTEND_GATE=full：全量非负向样本（DEMO 全域除 99_errors），里程碑验收用，
//   显式 `cargo test --test frontend_dual_track -- --ignored` 跑（数百样本约 20min）。
//
// rlib 车道：测试进程启动即调 set_bootstrap_lane_env()——隔离车道
// target/lane-bootstrap 存在时子进程全部走它（免共享 target/debug 被异版本
// 工具链交叉重建污染；配合仓库根 rust-toolchain.toml 钉版双保险）。
// 隔离车道未建时先：CARGO_TARGET_DIR=target/lane-bootstrap cargo build -p lz_builtins

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use lang_zone::frontend::bootstrap_emit::normalize_token_stream;

fn bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
}

/// 递归收集 DEMO 非负向 .lz 样本（排除 99_errors 负向反例目录）
fn demo_samples() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("DEMO");
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                // 负向反例（99_errors）是「应编译失败」语料，不进一致性门禁
                if p.file_name().and_then(|n| n.to_str()) == Some("99_errors") {
                    continue;
                }
                stack.push(p);
            } else if p.extension().and_then(|e| e.to_str()) == Some("lz") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// 分层子集：每个 DEMO 顶层子目录取字典序第一个样本（确定性、覆盖全类别）
fn stratified_subset(samples: &[PathBuf]) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("DEMO");
    let mut by_cat: std::collections::BTreeMap<String, PathBuf> = Default::default();
    for s in samples {
        let rel = s.strip_prefix(&root).unwrap_or(s);
        let cat = rel
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_else(|| "_root".to_string());
        by_cat.entry(cat).or_insert_with(|| s.clone());
    }
    by_cat.into_values().collect()
}

/// 跑编译器子命令拿 stdout（断言退出码 0）
fn run_bin(bin: &Path, args: &[&str]) -> String {
    let out = Command::new(bin).args(args).output().expect("spawn lang-zone");
    assert!(
        out.status.success(),
        "lang-zone {:?} 失败: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// 单样本双轨比对：Rust --tokens vs LZ --emit=lex-lz，归一化后逐行一致
fn assert_dual_track(bin: &Path, sample: &Path) {
    let rust_out = run_bin(bin, &[&sample.display().to_string(), "--tokens"]);
    let lz_out = run_bin(
        bin,
        &[
            &sample.display().to_string(),
            "--emit=lex-lz",
            "--std-dir",
            "std",
        ],
    );
    let rust_norm = normalize_token_stream(&rust_out, true);
    let lz_norm = normalize_token_stream(&lz_out, false);
    if rust_norm != lz_norm {
        let mut diffs = Vec::new();
        let n = rust_norm.len().max(lz_norm.len());
        for i in 0..n {
            let r = rust_norm.get(i).map(|s| s.as_str()).unwrap_or("<none>");
            let l = lz_norm.get(i).map(|s| s.as_str()).unwrap_or("<none>");
            if r != l {
                diffs.push(format!("[{i}] rust={r} lz={l}"));
                if diffs.len() >= 5 {
                    break;
                }
            }
        }
        panic!(
            "前端双轨不一致: {}（rust {} 行 vs lz {} 行）\n{}",
            sample.display(),
            rust_norm.len(),
            lz_norm.len(),
            diffs.join("\n")
        );
    }
}

/// 分层子集快速门禁（常规 cargo test 即跑）
#[test]
fn frontend_dual_track_stratified_subset() {
    lang_zone::frontend::bootstrap_emit::set_bootstrap_lane_env();
    let bin = bin_path();
    let samples = demo_samples();
    assert!(samples.len() >= 300, "DEMO 非负向样本应 ≥300，实得 {}", samples.len());
    let subset = stratified_subset(&samples);
    assert!(subset.len() >= 5, "分层子集应 ≥5 类，实得 {}", subset.len());
    for s in &subset {
        assert_dual_track(&bin, s);
    }
    // 解析层冒烟：子集样本 parse-lz 全部可解析（退出码 0；描述格式对比不在本门禁）
    for s in &subset {
        let sp = s.display().to_string();
        run_bin(&bin, &[&sp, "--emit=parse-lz", "--std-dir", "std"]);
    }
}

/// 全量门禁（里程碑验收）：327 非负向样本 token 流逐行一致。
/// 显式运行：`cargo test --test frontend_dual_track -- --ignored`
#[test]
#[ignore]
fn frontend_dual_track_full_corpus() {
    lang_zone::frontend::bootstrap_emit::set_bootstrap_lane_env();
    let bin = bin_path();
    let samples = demo_samples();
    let total = samples.len();
    let mut failed = Vec::new();
    for (i, s) in samples.iter().enumerate() {
        let rust_out = Command::new(&bin)
            .args([&s.display().to_string(), "--tokens"])
            .output()
            .expect("spawn lang-zone --tokens");
        let lz_out = Command::new(&bin)
            .args([
                &s.display().to_string(),
                "--emit=lex-lz",
                "--std-dir",
                "std",
            ])
            .output()
            .expect("spawn lang-zone --emit=lex-lz");
        if !rust_out.status.success() || !lz_out.status.success() {
            failed.push(format!(
                "{}: rust_exit={:?} lz_exit={:?}",
                s.display(),
                rust_out.status.code(),
                lz_out.status.code()
            ));
            continue;
        }
        let rust_norm = normalize_token_stream(&String::from_utf8_lossy(&rust_out.stdout), true);
        let lz_norm = normalize_token_stream(&String::from_utf8_lossy(&lz_out.stdout), false);
        if rust_norm != lz_norm {
            failed.push(format!(
                "{}: token 流不一致（rust={} lz={} 行）",
                s.display(),
                rust_norm.len(),
                lz_norm.len()
            ));
        }
        if (i + 1) % 50 == 0 {
            eprintln!("[full gate] {}/{} 已比对", i + 1, total);
        }
    }
    assert!(
        failed.is_empty(),
        "全量双轨门禁 {}/{} 失败:\n{}",
        failed.len(),
        total,
        failed.join("\n")
    );
}

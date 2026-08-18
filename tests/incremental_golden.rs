//! 增量编译 golden 测试（FIST T4.3 / LZ_UPGRADE_PLAN 方向B）
//!
//! 覆盖升级计划第 4.3 节验收标准：
//! 1. 单文件变更，未变更模块读缓存（cached 计数正确）；
//! 2. 缓存命中输出与增量全量输出逐字符一致（golden 对照）；
//! 3. 依赖传播失效正确（变更模块的下游级联重编）；
//! 4. 增量拼接产物 rustc 可编译，运行输出与 --project 全量合并一致（行为级）。

use std::path::{Path, PathBuf};
use std::process::Command;

fn work_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lz_incr_golden_{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create work dir");
    d
}

fn write(d: &Path, name: &str, content: &str) {
    std::fs::write(d.join(name), content).expect("write lz source");
}

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

/// 运行 lang-zone 并返回 (exit_ok, stdout, stderr)
fn run_lz(work: &Path, args: &[&str]) -> (bool, String, String) {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .current_dir(work)
        .args(args)
        .output()
        .expect("run lang-zone");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// 解析 "Incremental: ... (N modules: A cached, B rebuilt, X ms)" 统计
fn parse_incr_stats(stdout: &str) -> Option<(usize, usize)> {
    let line = stdout.lines().find(|l| l.starts_with("Incremental:"))?;
    let rest = line.split_once('(')?.1;
    // 提取全部数字： [N, A, B, X] = [modules, cached, rebuilt, ms]
    let nums: Vec<usize> = rest
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<usize>().ok())
        .collect();
    if nums.len() < 3 {
        return None;
    }
    Some((nums[1], nums[2]))
}

/// rustc 编译生成的 .rs 并运行，返回 stdout
fn rustc_run(work: &Path, rs_name: &str) -> (bool, String) {
    let rs = work.join(rs_name);
    let exe = work.join(rs_name.replace(".rs", "_incr.exe"));
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    if !rc.status.success() {
        return (false, String::from_utf8_lossy(&rc.stderr).to_string());
    }
    let run = Command::new(&exe).output().expect("run compiled exe");
    (true, String::from_utf8_lossy(&run.stdout).to_string())
}

// ── 三模块项目 ──────────────────────────────────────────────
// lib_math（无依赖）← lib_stats（依赖 lib_math）← main_app（依赖 lib_stats）

const LIB_MATH: &str = r#"
// lib_math: 基础数学库（无依赖）
const BASE = 10

def square(x: int) -> int =
    x * x

print("lib_math init")
"#;

const LIB_STATS_V1: &str = r#"
import lib_math

def sum_squares(n: int) -> int =
    mut acc = 0
    mut i = 0
    while i < n:
        acc += lib_math.square(i)
        i += 1
    acc

print("lib_stats v1")
"#;

const LIB_STATS_V2: &str = r#"
import lib_math

def sum_squares(n: int) -> int =
    mut acc = 0
    mut i = 0
    while i < n:
        acc += lib_math.square(i)
        i += 1
    acc

def sum_cubes(n: int) -> int =
    mut acc = 0
    mut i = 0
    while i < n:
        acc += lib_math.square(i) * i
        i += 1
    acc

print("lib_stats v2")
"#;

const MAIN_APP: &str = r#"
import lib_stats
from lib_stats import sum_squares

def main() =
    print(lib_stats.sum_squares(5))
    print(sum_squares(6))
    print("main_app running")
"#;

#[test]
fn incremental_golden_cache_hit_and_propagation() {
    let work = work_dir("basic");
    write(&work, "lib_math.lz", LIB_MATH);
    write(&work, "lib_stats.lz", LIB_STATS_V1);
    write(&work, "main_app.lz", MAIN_APP);

    let cache = ".incr_cache";

    // 1. 首次全量：3 个模块全部重建
    let (ok1, out1, err1) = run_lz(&work, &["main_app.lz", "--incr", &format!("--incr-cache={cache}")]);
    assert!(ok1, "首次增量编译失败: {err1}");
    let (cached1, rebuilt1) = parse_incr_stats(&out1).expect("解析首次统计");
    assert_eq!((cached1, rebuilt1), (0, 3), "首次应全量重建");
    let rs_full = std::fs::read_to_string(work.join("main_app.rs")).expect("读首次产物");

    // 2. 二次：全部命中缓存
    let (ok2, out2, err2) = run_lz(&work, &["main_app.lz", "--incr", &format!("--incr-cache={cache}")]);
    assert!(ok2, "二次增量编译失败: {err2}");
    let (cached2, rebuilt2) = parse_incr_stats(&out2).expect("解析二次统计");
    assert_eq!((cached2, rebuilt2), (3, 0), "未变更应全部命中缓存");
    let rs_cached = std::fs::read_to_string(work.join("main_app.rs")).expect("读二次产物");
    assert_eq!(
        rs_full, rs_cached,
        "缓存命中输出必须与增量全量输出逐字符一致（golden 对照）"
    );

    // 3. 修改 lib_stats（依赖 lib_math）：lib_stats + 下游 main_app 级联重编，lib_math 命中
    write(&work, "lib_stats.lz", LIB_STATS_V2);
    let (ok3, out3, err3) = run_lz(&work, &["main_app.lz", "--incr", &format!("--incr-cache={cache}")]);
    assert!(ok3, "传播失效后编译失败: {err3}");
    let (cached3, rebuilt3) = parse_incr_stats(&out3).expect("解析传播统计");
    assert_eq!((cached3, rebuilt3), (1, 2), "lib_math 命中，lib_stats+main_app 应级联重编");
    let rs_prop = std::fs::read_to_string(work.join("main_app.rs")).expect("读传播产物");

    // 4. 与「修改后」的增量全量基线对照：清缓存重跑首次
    let _ = std::fs::remove_dir_all(work.join(cache));
    let (ok4, out4, err4) = run_lz(&work, &["main_app.lz", "--incr", &format!("--incr-cache={cache}")]);
    assert!(ok4, "清缓存后全量编译失败: {err4}");
    let (cached4, rebuilt4) = parse_incr_stats(&out4).expect("解析全量统计");
    assert_eq!((cached4, rebuilt4), (0, 3), "清缓存后应全量重建");
    let rs_full2 = std::fs::read_to_string(work.join("main_app.rs")).expect("读全量产物");
    assert_eq!(
        rs_prop, rs_full2,
        "依赖传播失效后的输出必须与修改后的增量全量输出逐字符一致"
    );

    // 5. 行为级对照：增量拼接产物 rustc 可编译，且与 --project 全量合并输出 stdout 一致
    let (rc_ok, incr_stdout) = rustc_run(&work, "main_app.rs");
    assert!(rc_ok, "增量拼接产物 rustc 编译失败: {incr_stdout}");
    let (p_ok, _p_out, p_err) = run_lz(&work, &["main_app.lz", "--project"]);
    assert!(p_ok, "--project 全量编译失败: {p_err}");
    let (rcp_ok, proj_stdout) = rustc_run(&work, "main_app.rs");
    assert!(rcp_ok, "--project 产物 rustc 编译失败: {proj_stdout}");
    assert_eq!(
        incr_stdout, proj_stdout,
        "增量拼接产物运行输出必须与 --project 全量合并输出一致"
    );
}

// Lang-Zone 编译器 — tests/demo_codegen_compile.rs
//
// 生成产物编译闸门：对每个 DEMO .lz 先转译为 .rs，再用 rustc **真正编译**。
//
// 为什么需要它：
// - 现有 `tests/ir_snapshots.rs` 只验证 `--emit=ir` 成功，并不断言生成的 Rust 能编译；
// - 原 `tests/deprecated/compile_demos.rs` 已 `#[ignore]` 且未挂在 tests/mod.rs。
// 结果就是：魔法方法"生成即编译失败"（`__contains__` / `__len__` / `__iadd__` / `__new__` 等）
// 在现有测试里**一条都抓不到**——本测试补齐这道闸门。
//
// 实现要点：
// - 复用 src/main.rs `run_test_mode` 的同款机制（cargo build -p lz_builtins → 定位 rlib → rustc --extern）；
// - 生成的 .rs 落在源目录旁（lzc 固定行为），因此**先备份已存在的 .rs、验证后还原**，
//   避免破坏 DEMO/06_control_flow/ 下已提交的 17 个 .rs。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// 两个闸门（魔法子集 / 全量）会在同一 DEMO 目录生成并还原 .rs，
/// 而 cargo test 默认并发执行测试 → 必须串行化，否则互相踩踏产生假失败。
fn gate_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

// ────────────────────────────────────────────────────────────────
// 已知失败基线（allowlist）
//
// 语义：列入此处的文件**不会**让测试变红；任何**不在列表中的新失败**都会让测试失败。
// 每修好一类问题，就从这里删掉对应条目，闸门随之收紧。
// ────────────────────────────────────────────────────────────────

/// 转译阶段（lzc）已知失败 —— 基线快照 2026-09-09
const KNOWN_TRANSPILE_FAILURES: &[&str] = &[
    // 基线原记为产物编译失败，实际为转译期失败（IR build: parse_int 返回类型
    // Result<int,str> vs Result<int,unknown>）——早于本次改动即存在，仅基线阶段记错
    "DEMO/05_expressions/operators.lz",
    // ── 以下 1 项仍为转译期失败（全量基线 2026-09-09）──
    "DEMO/99_spec/combo-syntax/combo_while_guard_try.lz",
];

/// 产物编译阶段（rustc）已知失败 —— 基线快照 2026-09-09（含后续修复收窄后剩余项）
const KNOWN_RUSTC_FAILURES: &[&str] = &[
    "DEMO/lz_std/error.lz",                      // E0658: `!` 类型是实验特性 + E0599 chain_str
    "DEMO/lz_std/traits.lz",                     // 迭代协议 trait 默认方法缺 Item: Clone / LzAdd 约束
    "DEMO/lz_std/iter.lz",                       // 同 traits：Item 约束 + E0502/E0594 闭包捕获
    "DEMO/04_functions/spread_protocol.lz",      // E0403: 泛型参数 `T` 重复
    "DEMO/boundary-coverage/combo-defer-guard.lz", // E0308: 类型不匹配
    // 原为转译期失败，where 子句缩进配平修复后已可转译，转入产物编译失败
    "DEMO/04_functions/generics.lz",
    "DEMO/10_error_handling/panic_raise_try.lz",
    // ── 以下 20 项来自全量基线（2026-09-09）──
    "DEMO/01_basics/keywords.lz",
    "DEMO/01_basics/polish_07_closures.lz",
    "DEMO/01_basics/polish_19_checker.lz",
    "DEMO/01_basics/polish_27_use.lz",
    "DEMO/01_basics/polish_29_combined.lz",
    "DEMO/02_types/method_chains.lz",
    "DEMO/04_functions/checker.lz",
    "DEMO/04_functions/closures_more.lz",
    "DEMO/10_error_handling/try_more.lz",
    "DEMO/99_spec/iterator_demo.lz",
    "DEMO/boundary-coverage/combo-error-control.lz",
    "DEMO/boundary-coverage/combo-iterator-generator.lz",
    "DEMO/boundary-coverage/combo-pipe-lambda.lz",
    "DEMO/boundary-coverage/nesting-closure-lambda.lz",
    "DEMO/combo-syntax/combo_defer_guard_try.lz",
];

// ────────────────────────────────────────────────────────────────
// 基础设施
// ────────────────────────────────────────────────────────────────

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 在目录中定位 lz_builtins 的 .rlib（逻辑对齐 src/main.rs::probe_rlib）
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
    // 回退：cargo test 的可执行文件位于 target/<profile>/deps
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(p) = probe_rlib(dir) {
                return Some(p);
            }
            if let Some(parent) = dir.parent() {
                if let Some(p) = probe_rlib(parent) {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn ensure_builtins_rlib() -> Option<PathBuf> {
    // lang-zone 只在生成的字符串里出现 `use lz_builtins::*`，不直接引用其符号，
    // cargo 可能只产出 .rmeta；显式构建该包以确保 .rlib 存在。
    let _ = Command::new("cargo")
        .current_dir(manifest())
        .args(["build", "-p", "lz_builtins", "--quiet"])
        .status();
    find_builtins_rlib()
}

/// 递归收集 DEMO 下所有 .lz（排除 99_errors/ 负例目录）
fn find_demo_files() -> Vec<PathBuf> {
    let demo_dir = manifest().join("DEMO");
    let mut files = Vec::new();
    let mut stack = vec![demo_dir];
    while let Some(dir) = stack.pop() {
        if dir.file_name().map_or(false, |n| n == "99_errors") {
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(it) => it,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().map_or(false, |e| e == "lz") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// 魔法方法相关 DEMO —— 魔法体系改动的**直接回归面**（改动优先跑这一组）
const MAGIC_DEMOS: &[&str] = &[
    "DEMO/lz_std/box.lz",
    "DEMO/lz_std/traits.lz",
    "DEMO/lz_std/string.lz",
    "DEMO/lz_std/set.lz",
    "DEMO/lz_std/result.lz",
    "DEMO/lz_std/ordering.lz",
    "DEMO/lz_std/option.lz",
    "DEMO/lz_std/list.lz",
    "DEMO/lz_std/iter.lz",
    "DEMO/lz_std/error.lz",
    "DEMO/lz_std/dict.lz",
    "DEMO/07_data_structures/callable_objects.lz",
    "DEMO/06_control_flow/with_defer.lz",
    "DEMO/05_expressions/pipe_semantics.lz",
    "DEMO/04_functions/spread_protocol.lz",
    "DEMO/02_types/duck_nested.lz",
    "DEMO/01_basics/polish_15_magic.lz",
    "DEMO/01_basics/polish_14_operators.lz",
    "DEMO/01_basics/identifiers.lz",
    "DEMO/boundary-coverage/edge-keyword-identifier.lz",
    "DEMO/boundary-coverage/nesting-data-types.lz",
    "DEMO/boundary-coverage/combo-struct-method.lz",
    "DEMO/boundary-coverage/combo-defer-guard.lz",
    "DEMO/99_spec/extractor_unapply.lz",
    "DEMO/99_spec/extractor_unapply_let_for.lz",
];

fn magic_demo_paths() -> Vec<PathBuf> {
    MAGIC_DEMOS.iter().map(|p| manifest().join(p)).collect()
}

fn rel_display(p: &Path) -> String {
    p.strip_prefix(manifest())
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 取诊断信息的首行 —— **优先取 error**（否则 warning 会淹没真正的错误，
/// 例如 `unused import: std::any::Any` 排在 E0xxx 之前）
fn first_line(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    if let Some(e) = s.lines().find(|l| l.trim_start().starts_with("error")) {
        return e.trim().to_string();
    }
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("(无输出)")
        .trim()
        .to_string()
}

/// 还原被覆盖的 .rs：原本存在则写回备份，原本不存在则删除
fn restore_rs(rs: &Path, backup: Option<&Vec<u8>>, pre_existed: bool) {
    if pre_existed {
        if let Some(b) = backup {
            let _ = std::fs::write(rs, b);
        }
    } else {
        let _ = std::fs::remove_file(rs);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Transpile,
    Rustc,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Stage::Transpile => "转译(lzc)",
            Stage::Rustc => "产物编译(rustc)",
        }
    }
}

struct Failure {
    file: String,
    stage: Stage,
    msg: String,
}

/// 校验单个 DEMO：转译 → 编译产物 → 还原
fn check_one(lz: &Path, rlib: Option<&Path>, workdir: &Path) -> Result<(), Failure> {
    let rel = rel_display(lz);
    let rs = lz.with_extension("rs");

    let pre_existed = rs.exists();
    let backup = if pre_existed {
        match std::fs::read(&rs) {
            Ok(b) => Some(b),
            Err(e) => {
                return Err(Failure {
                    file: rel,
                    stage: Stage::Transpile,
                    msg: format!("备份已有 .rs 失败: {e}"),
                })
            }
        }
    } else {
        None
    };

    // ── 阶段 1：转译 .lz → .rs ──
    let out = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_lang-zone")))
        .arg(lz.as_os_str())
        .output();
    match out {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            restore_rs(&rs, backup.as_ref(), pre_existed);
            return Err(Failure {
                file: rel,
                stage: Stage::Transpile,
                msg: first_line(&o.stderr),
            });
        }
        Err(e) => {
            restore_rs(&rs, backup.as_ref(), pre_existed);
            return Err(Failure {
                file: rel,
                stage: Stage::Transpile,
                msg: format!("lzc 执行失败: {e}"),
            });
        }
    }

    // ── 阶段 2：用 rustc 真正编译产物 ──
    let stem = rel.replace(['/', '.'], "_");
    let out_rlib = workdir.join(format!("{stem}.rlib"));
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
        .arg(&out_rlib);
    if let Some(r) = rlib {
        cmd.arg("--extern").arg(format!("lz_builtins={}", r.display()));
    }

    let res = cmd.output();
    restore_rs(&rs, backup.as_ref(), pre_existed);
    let _ = std::fs::remove_file(&out_rlib);

    match res {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(Failure {
            file: rel,
            stage: Stage::Rustc,
            msg: first_line(&o.stderr),
        }),
        Err(e) => Err(Failure {
            file: rel,
            stage: Stage::Rustc,
            msg: format!("rustc 执行失败: {e}"),
        }),
    }
}

/// 跑一批文件并产出报告；只有"不在 allowlist 里的失败"才算失败
fn run_gate(files: &[PathBuf], title: &str) {
    // 串行化：避免与另一个闸门并发操作同一批 DEMO 文件
    let _guard = gate_lock();

    let workdir = manifest().join("target").join("demo_codegen_check");
    let _ = std::fs::create_dir_all(&workdir);
    let rlib = ensure_builtins_rlib();
    if rlib.is_none() {
        eprintln!("⚠️  未找到 lz_builtins rlib，产物编译阶段将大概率因 E0432 失败");
    }

    let mut failures: Vec<Failure> = Vec::new();
    for f in files {
        if let Err(e) = check_one(f, rlib.as_deref(), &workdir) {
            failures.push(e);
        }
    }

    println!("\n===== {title}：生成产物编译闸门 =====");
    println!("  总计: {} 文件", files.len());
    println!("  通过: {}", files.len() - failures.len());
    println!("  失败: {}", failures.len());

    let mut unexpected: Vec<&Failure> = Vec::new();
    if !failures.is_empty() {
        println!("\n  失败明细:");
        for f in &failures {
            let known = match f.stage {
                Stage::Transpile => KNOWN_TRANSPILE_FAILURES.contains(&f.file.as_str()),
                Stage::Rustc => KNOWN_RUSTC_FAILURES.contains(&f.file.as_str()),
            };
            let mark = if known { "·(已在基线)" } else { " ❌新增" };
            println!("    [{}]{} {}\n        {}", f.stage.name(), mark, f.file, f.msg);
            if !known {
                unexpected.push(f);
            }
        }
    }

    // 基线中列出、但实际已通过的文件（提示可以收紧基线）
    let failed_set: Vec<&str> = failures.iter().map(|f| f.file.as_str()).collect();
    let stale: Vec<&&str> = KNOWN_TRANSPILE_FAILURES
        .iter()
        .chain(KNOWN_RUSTC_FAILURES.iter())
        .filter(|k| files.iter().any(|f| rel_display(f) == **k))
        .filter(|k| !failed_set.contains(&**k))
        .collect();
    if !stale.is_empty() {
        println!("\n  ℹ️  以下基线条目现已通过，可从 allowlist 移除以收紧闸门:");
        for s in &stale {
            println!("      {s}");
        }
    }
    println!("==========================================\n");

    assert!(
        unexpected.is_empty(),
        "{} 个文件出现**非基线内**的失败（新增回归）",
        unexpected.len()
    );
}

#[test]
fn magic_demos_generated_rust_compiles() {
    run_gate(&magic_demo_paths(), "魔法相关 DEMO");
}

#[test]
#[ignore = "全量 DEMO 较慢；按需运行：cargo test --test demo_codegen_compile -- --ignored --nocapture"]
fn all_demos_generated_rust_compiles() {
    run_gate(&find_demo_files(), "全量 DEMO");
}

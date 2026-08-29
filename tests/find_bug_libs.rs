// FIND_BUG 12 库全链路测试：lz → rs → rustc → run → stdout 含 OK
// 路径约定：FIND_BUG/<lib>/ 目录内恰好一个 .lz 文件（文件名不必与目录同名）。
// 进度约定：库全链路通过后移除对应 #[ignore] 转正；ignore reason 记录当前卡点阶段。
// 台账：docs/库复现找缺计划-2026-08-24.md

use std::path::{Path, PathBuf};
use std::process::Command;

fn builtins_rlib() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug");
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return direct;
    }
    let deps = dir.join("deps");
    if let Ok(entries) = std::fs::read_dir(&deps) {
        let mut cands: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let n = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                n.starts_with("liblz_builtins-") && n.ends_with(".rlib")
            })
            .collect();
        cands.sort();
        if let Some(p) = cands.pop() {
            return p;
        }
    }
    panic!("lz_builtins rlib not found under {}", dir.display());
}

fn first_lz(lib_dir: &Path) -> PathBuf {
    let mut cands: Vec<PathBuf> = std::fs::read_dir(lib_dir)
        .unwrap_or_else(|e| panic!("read dir {}: {}", lib_dir.display(), e))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "lz").unwrap_or(false))
        .collect();
    cands.sort();
    assert_eq!(
        cands.len(),
        1,
        "库目录应恰好一个 .lz：{} (found {})",
        lib_dir.display(),
        cands.len()
    );
    cands.remove(0)
}

fn run_lib(name: &str) -> Result<String, String> {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_dir = manifest.join(format!("FIND_BUG/{}", name));
    let lz = first_lz(&lib_dir);
    let stem = lz.file_stem().unwrap().to_string_lossy().to_string();
    let rs = lz.with_extension("rs");
    let exe = lib_dir.join("debug").join(format!("{}.exe", stem));

    let _ = std::fs::create_dir_all(lib_dir.join("debug"));

    let out = Command::new(&bin).arg(&lz).output()
        .map_err(|e| format!("lz compile err: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-A")
        .arg("warnings")
        .arg("-o")
        .arg(&exe)
        .output()
        .map_err(|e| format!("rustc err: {}", e))?;
    if !rc.status.success() {
        return Err(String::from_utf8_lossy(&rc.stderr).to_string());
    }

    let run = Command::new(&exe).output()
        .map_err(|e| format!("run err: {}", e))?;
    if !run.status.success() {
        return Err(format!(
            "run failed (exit {:?}): {}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).to_string())
}

#[test]
#[ignore = "待转正：RUSTC_FAIL E0507/E0382 owned 移动语义（P2a）"]
fn lib_sort() {
    assert!(run_lib("lib_sort").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：LZ_REJECT match 未穷尽 OptionInt（P1b 分诊）"]
fn lib_option() {
    assert!(run_lib("lib_option").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：LZ_REJECT and_then 泛型推断（P1b 分诊）"]
fn lib_result() {
    assert!(run_lib("lib_result").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：RUSTC_FAIL E0599 方法未生成（P2a）"]
fn lib_vector() {
    assert!(run_lib("lib_vector").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：RUSTC_FAIL E0382 闭包捕获移动（P2a）"]
fn lib_closure() {
    assert!(run_lib("lib_closure").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：RUN_FAIL 运行输出不符（P1a 收割）"]
fn lib_pattern() {
    assert!(run_lib("lib_pattern").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：RUSTC_FAIL E0308/E0599（P2a）"]
fn lib_linked_list() {
    assert!(run_lib("lib_linked_list").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：RUSTC_FAIL E0308/E0277（P2a）"]
fn lib_string() {
    assert!(run_lib("lib_string").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：RUSTC_FAIL E0308（P2a）"]
fn lib_json() {
    assert!(run_lib("lib_json").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：LZ_REJECT Dict put 方法（P1b 分诊）"]
fn lib_hashmap() {
    assert!(run_lib("lib_hashmap").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：RUSTC_FAIL E0369/E0599（P2a）"]
fn lib_tree() {
    assert!(run_lib("lib_tree").unwrap().contains("OK"));
}

#[test]
#[ignore = "待转正：LZ_REJECT collect 参数类型（P1b 分诊）"]
fn lib_iterator() {
    assert!(run_lib("lib_iterator").unwrap().contains("OK"));
}

//! 热重载（FIST T4.5 / LZ_UPGRADE_PLAN 第4章 方向C）
//!
//! 实现 `lang-zone watch <entry.lz> [-- args...]`：
//! - 监听 entry 及其 import 依赖的 `.lz` 文件变更（内容哈希轮询，纯 std，无第三方依赖）；
//! - 变更时复用增量编译管线（`IncrCompiler`，T4.3）做增量重编译；
//! - 重编译成功 → rustc 生成新 exe → 热替换运行中的子进程（重启）；
//! - 重编译失败 → 保留旧进程继续运行，错误打印到 stderr（不影响运行中程序）；
//! - 状态语义：进程重启 ⇒ 运行时状态（全局变量/已加载数据）明确重置，
//!   符合升级计划验收标准「状态正确保留或明确重置」。
//!
//! 说明：升级计划推荐的动态库级热替换（cdylib + dlopen）需要 libloading 等
//! 运行时依赖，当前 cargo offline 环境不可用；进程级热重载（重编译 + 重启）
//! 作为可落地的替代方案，语义与验收标准一致。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use crate::cache::content_hash;
use crate::incr::IncrCompiler;

/// `lang-zone watch` 子命令入口：解析 args 并进入监听循环
pub fn cmd_watch(args: &[String]) -> i32 {
    // args[0] == exe path, args[1] == "watch"（与 main.rs 其它子命令一致，传入完整 argv）
    let mut entry: Option<PathBuf> = None;
    let mut cache_dir = PathBuf::from(IncrCompiler::default_cache_dir());
    let mut std_dir: Option<PathBuf> = None;
    let mut run_args: Vec<String> = Vec::new();
    let mut after_dd = false;
    let mut i = 2;
    while i < args.len() {
        let a = &args[i];
        if after_dd {
            run_args.push(a.clone());
        } else if a == "--" {
            after_dd = true;
        } else if let Some(rest) = a.strip_prefix("--incr-cache=") {
            cache_dir = PathBuf::from(rest);
        } else if a == "--incr" {
            // 默认即增量；接受该标志保持兼容
        } else if let Some(rest) = a.strip_prefix("--std-dir=") {
            std_dir = Some(PathBuf::from(rest));
        } else if a.starts_with('-') {
            eprintln!("Unknown watch option: {}", a);
            return 2;
        } else if entry.is_none() {
            entry = Some(PathBuf::from(a));
        } else {
            run_args.push(a.clone());
        }
        i += 1;
    }
    let Some(entry) = entry else {
        eprintln!("Usage: lang-zone watch <entry.lz> [--incr-cache=<dir>] [--std-dir <path>] [-- args...]");
        return 2;
    };
    run_watch(WatchConfig {
        entry,
        run_args,
        cache_dir,
        std_dir,
    })
}

/// watch 配置
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// 入口 .lz 文件
    pub entry: PathBuf,
    /// 传递给被监视程序的参数（`--` 之后）
    pub run_args: Vec<String>,
    /// 增量编译缓存目录（默认 `.lzcache_incr`）
    pub cache_dir: PathBuf,
    /// 可选 std 目录（透传给 ProjectCompiler 语义；watch 当前以增量管线为主）
    pub std_dir: Option<PathBuf>,
}

/// 一次热重载构建的结果
#[derive(Debug)]
pub struct ReloadResult {
    /// 生成的 .rs 路径
    pub rs_path: PathBuf,
    /// 生成的 exe 路径
    pub exe_path: PathBuf,
    /// 增量统计摘要
    pub stats: String,
}

/// 收集 entry 及其 import 依赖（递归）的所有 .lz 文件绝对路径。
/// 扫描逻辑与 cli.rs source_hashes 保持一致：仅识别 `import x` / `from x import y` 行。
pub fn collect_deps(entry: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![entry.to_path_buf()];
    while let Some(p) = stack.pop() {
        let canon = p.canonicalize().unwrap_or(p.clone());
        if !seen.insert(canon.clone()) {
            continue;
        }
        out.push(canon.clone());
        if let Ok(src) = fs::read_to_string(&canon) {
            let dir = canon.parent().map(Path::to_path_buf).unwrap_or_default();
            for line in src.lines() {
                let t = line.trim_start();
                let mod_name = if let Some(rest) = t.strip_prefix("import ") {
                    rest.split_whitespace().next()
                } else if let Some(rest) = t.strip_prefix("from ") {
                    rest.split_whitespace().next()
                } else {
                    None
                };
                if let Some(m) = mod_name {
                    if m != "std" && !m.contains('.') {
                        let dep = dir.join(format!("{}.lz", m));
                        if dep.is_file() {
                            stack.push(dep);
                        }
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.cmp(b));
    out
}

/// 依赖集合内容指纹（文件路径 + 内容哈希）
pub fn deps_fingerprint(deps: &[PathBuf]) -> String {
    let mut parts: Vec<String> = deps
        .iter()
        .map(|p| {
            let h = content_hash(p).unwrap_or_else(|_| "?".to_string());
            format!("{}:{}", p.display(), h)
        })
        .collect();
    parts.sort();
    parts.join(";")
}

/// 增量编译 entry → 写入 .rs；返回 (rs 路径, exe 路径, 统计摘要)
pub fn compile_to_rs(
    entry: &Path,
    cache_dir: &Path,
) -> Result<ReloadResult, String> {
    let base_dir = entry
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let mut ic = IncrCompiler::new(base_dir.clone(), cache_dir.to_path_buf());
    let outcome = ic.compile(entry)?;
    let rs_path = replace_ext(entry, ".lz", ".rs");
    let exe_path = replace_ext(entry, ".lz", ".exe");
    fs::write(&rs_path, &outcome.code)
        .map_err(|e| format!("cannot write {}: {}", rs_path.display(), e))?;
    let stats = format!(
        "{} modules: {} cached, {} rebuilt, {} ms",
        outcome.stats.total, outcome.stats.hits, outcome.stats.misses, outcome.stats.elapsed_ms
    );
    Ok(ReloadResult {
        rs_path,
        exe_path,
        stats,
    })
}

/// rustc 编译 .rs → exe（复用 main.rs 的 --incr 管线 rustc 调用方式）
pub fn rustc_compile(rs: &Path, exe: &Path, incremental_dir: Option<&Path>) -> Result<(), String> {
    let builtins = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/liblz_builtins.rlib");
    let mut cmd = Command::new("rustc");
    cmd.args(["--edition", "2021"])
        .arg(rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-o")
        .arg(exe);
    if let Some(dir) = incremental_dir {
        cmd.arg("-C").arg(format!("incremental={}", dir.display()));
    }
    let out = cmd
        .output()
        .map_err(|e| format!("Failed to run rustc: {}", e))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

/// 替换 .lz 扩展名为指定后缀（与 main.rs replace_ext 语义一致）
fn replace_ext(path: &Path, from: &str, to: &str) -> PathBuf {
    let mut s = path.to_string_lossy().to_string();
    if let Some(stem) = path.file_stem() {
        let parent = path.parent().unwrap_or(Path::new(""));
        let new_name = format!("{}{}", stem.to_string_lossy(), to);
        if parent.as_os_str().is_empty() {
            s = new_name;
        } else {
            s = format!("{}/{}", parent.display(), new_name);
        }
    } else {
        s = s.replace(from, to);
    }
    PathBuf::from(s)
}

/// 等待子进程退出并回收
fn reap_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// 主循环：监听 .lz 变更 → 增量重编译 → 重启子进程。
/// 阻塞运行直到 Ctrl+C / 进程终止。
pub fn run_watch(config: WatchConfig) -> i32 {
    let entry = config.entry.clone();
    if !entry.is_file() {
        eprintln!("Error: cannot watch {}: no such file", entry.display());
        return 1;
    }
    let entry_disp = entry.display().to_string();
    let cache_dir = config.cache_dir.clone();
    let exe_path = replace_ext(&entry, ".lz", ".exe");
    let rustc_incr_dir = cache_dir.join("rustc");

    eprintln!("[watch] watching {} (poll 500ms)", entry_disp);

    // 首次构建（失败则退出——没有可运行的基线程序）
    let build = compile_to_rs(&entry, &cache_dir);
    let (mut child, mut fingerprint) = match build {
        Ok(res) => {
            eprintln!("[watch] initial compile: {}", res.stats);
            match rustc_compile(&res.rs_path, &res.exe_path, Some(&rustc_incr_dir)) {
                Ok(()) => {
                    let deps = collect_deps(&entry);
                    let fp = deps_fingerprint(&deps);
                    eprintln!("[watch] launch {}", exe_path.display());
                    match spawn_child(&exe_path, &config.run_args) {
                        Some(c) => (c, fp),
                        None => {
                            eprintln!("Error: failed to launch {}", exe_path.display());
                            return 1;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("rustc error:\n{}", e);
                    return 1;
                }
            }
        }
        Err(e) => {
            eprintln!("Compile error: {}", e);
            return 1;
        }
    };

    let poll = Duration::from_millis(500);
    let mut last_probe = Instant::now() - poll;
    let mut child_exit_notified = false;
    loop {
        // 子进程自行退出：打印一次状态，继续监听（下一次变更会重启）
        if let Some(status) = child.try_wait().ok().flatten() {
            if !child_exit_notified {
                child_exit_notified = true;
                eprintln!(
                    "[watch] child exited with {}; waiting for source changes...",
                    status.code().map(|c| c.to_string()).unwrap_or_else(|| "?".to_string())
                );
            }
            // 循环继续；不 spawn（避免无变更自动重启的无限循环）
        }

        // 轮询文件变更
        if last_probe.elapsed() >= poll {
            last_probe = Instant::now();
            let deps = collect_deps(&entry);
            let fp = deps_fingerprint(&deps);
            if fp != fingerprint {
                fingerprint = fp;
                eprintln!("[watch] source changed, recompiling...");
                match compile_to_rs(&entry, &cache_dir) {
                    Ok(res) => {
                        eprintln!("[watch] recompiled: {}", res.stats);
                        match rustc_compile(&res.rs_path, &res.exe_path, Some(&rustc_incr_dir)) {
                            Ok(()) => {
                                // 热替换：kill 旧进程 → 启动新进程（状态明确重置）
                                reap_child(&mut child);
                                child_exit_notified = false;
                                eprintln!("[watch] restart {}", exe_path.display());
                                match spawn_child(&exe_path, &config.run_args) {
                                    Some(c) => child = c,
                                    None => eprintln!("Error: failed to relaunch {}", exe_path.display()),
                                }
                            }
                            Err(e) => {
                                // 编译失败：保留旧进程继续运行
                                eprintln!("[watch] rustc error (keeping old process):\n{}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[watch] compile error (keeping old process): {}", e);
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

/// 启动被监视的子进程（stdout/stderr 透传）
fn spawn_child(exe: &Path, args: &[String]) -> Option<Child> {
    Command::new(exe)
        .args(args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_ext_works() {
        let p = Path::new("a/b/c.lz");
        assert_eq!(replace_ext(p, ".lz", ".rs"), PathBuf::from("a/b/c.rs"));
        let p2 = Path::new("main.lz");
        assert_eq!(replace_ext(p2, ".lz", ".exe"), PathBuf::from("main.exe"));
    }

    #[test]
    fn deps_fingerprint_stable_and_sensitive() {
        let dir = std::env::temp_dir().join("lz_watch_fp_test");
        let _ = fs::create_dir_all(&dir);
        let a = dir.join("a.lz");
        fs::write(&a, "def main() = print(1)").unwrap();
        let deps1 = vec![a.clone()];
        let fp1 = deps_fingerprint(&deps1);
        let fp2 = deps_fingerprint(&deps1);
        assert_eq!(fp1, fp2, "fingerprint must be stable");
        // 内容变化 → 指纹变化
        fs::write(&a, "def main() = print(2)").unwrap();
        let fp3 = deps_fingerprint(&deps1);
        assert_ne!(fp1, fp3, "fingerprint must change with content");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_deps_finds_imports() {
        let dir = std::env::temp_dir().join("lz_watch_deps_test");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("util.lz"), "def helper() = 1").unwrap();
        let main = dir.join("main.lz");
        fs::write(&main, "import util\n\ndef main() = print(util.helper())").unwrap();
        let deps = collect_deps(&main);
        assert!(
            deps.iter().any(|p| p.ends_with("util.lz")),
            "import dependency should be collected: {:?}",
            deps
        );
        let _ = fs::remove_dir_all(&dir);
    }
}

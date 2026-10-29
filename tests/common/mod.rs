// Lang-Zone 集成测试共享助手 —— 进程内唯一的临时目录。
//
// 动机（T0r27）：集成测试此前用**固定路径**落产物
// （`std::env::temp_dir().join("lz_xxx")` / `target/xxx`），两条车道并发跑时
// 会撞 Windows 文件锁：
//   Os { code: 32, kind: Uncategorized, message: "另一个程序正在使用此文件" }
// 重跑就过 ⇒ flaky，污染验收读数。这里统一提供进程内唯一的临时目录。
//
// 用法（各测试二进制顶部写 `mod common;`，cargo 不会把本文件当独立测试目标）：
//
//   mod common;
//
//   let work = common::unique_dir("lz_xxx_mycase");
//   std::fs::write(work.path().join("input.lz"), src).expect("write lz");
//
// 唯一性：目录名形如 `{prefix}-{pid}-{unix_nanos}-{atomic_seq}`：
//   - `pid` 区分进程（两条并发车道 / 两个并发测试二进制）；
//   - 原子计数区分同进程内的并发测试线程（libtest 默认多线程）；
//   - 纳秒时间戳用于人眼排查，并在 pid 复用时兜底。
//   另外 `create_dir_all` 前会探测已存在目录并追加序号，保证绝不复用旧目录。
//
// 清理策略（`Drop` 时递归删除）：
//   - 正常返回：删除整个目录；
//   - 显式保留：环境变量 `LZ_KEEP_TEMP=1`（也接受 true/yes/on）时**不删**并打印路径，
//     便于复现失败样本：`LZ_KEEP_TEMP=1 cargo test --test xxx`；
//   - 测试 panic（unwrap/断言失败发生在助手内部）时也不删并打印路径，保住现场；
//   - 删除失败（Windows 偶发占用）只告警、不 panic，避免清理动作本身制造 flaky。
//
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// 同进程内的目录序号（libtest 默认并行跑测试，必须原子）。
static SEQ: AtomicU64 = AtomicU64::new(0);

/// 一个在 `Drop` 时递归删除的临时目录。
///
/// 生命周期即清理边界：持有者在，目录就在；持有者销毁，目录随之消失。
pub struct TempDir {
    path: PathBuf,
    /// 显式保留（`keep()` / `into_path()`），不受 `LZ_KEEP_TEMP` 影响。
    keep: bool,
}

impl TempDir {
    /// 目录绝对路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 显式保留该目录（不做删除），用于「失败时想留现场」的调用点。
    pub fn keep(mut self) -> PathBuf {
        self.keep = true;
        let p = self.path.clone();
        // self 在此处 Drop：keep 已置位，Drop 只打印不删除。
        p
    }

    /// 交出路径并放弃所有权（目录保留在磁盘上，由调用者负责清理）。
    pub fn into_path(mut self) -> PathBuf {
        self.keep = true;
        self.path.clone()
    }

    /// 当前是否要求保留现场（显式 keep / 环境变量 / 正在 panic 展开）。
    fn should_keep(&self) -> Option<&'static str> {
        if self.keep {
            return Some("显式 keep");
        }
        if keep_temp_requested() {
            return Some("LZ_KEEP_TEMP");
        }
        if std::thread::panicking() {
            return Some("测试 panic，保留现场");
        }
        None
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Some(reason) = self.should_keep() {
            eprintln!(
                "[lz-test] 保留临时目录（{reason}）: {}",
                self.path.display()
            );
            return;
        }
        // Windows 上刚跑完的 .exe 偶尔还会短暂占用句柄，重试几次再放弃。
        for attempt in 0..3 {
            match std::fs::remove_dir_all(&self.path) {
                Ok(()) => return,
                Err(_) if !self.path.exists() => return,
                Err(e) => {
                    if attempt == 2 {
                        eprintln!(
                            "[lz-test] 清理临时目录失败（已忽略，不影响测试结论）: {}: {e}",
                            self.path.display()
                        );
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(30));
                }
            }
        }
    }
}

/// 创建一个进程内唯一、`Drop` 时递归删除的临时目录。
///
/// `prefix` 用于人眼辨识来源（一般填测试名/tag）；路径分隔符等不安全字符会被换成 `_`。
pub fn unique_dir(prefix: &str) -> TempDir {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let base = format!("{}-{}-{}-{}", sanitize(prefix), pid, nanos, seq);

    let root = std::env::temp_dir();
    let mut path = root.join(&base);
    // 极端情况（pid 复用 + 时钟回拨 + 同序号）下也不复用已有目录。
    let mut attempt = 0u32;
    while path.exists() && attempt < 100 {
        attempt += 1;
        path = root.join(format!("{base}-{attempt}"));
    }

    if let Err(e) = std::fs::create_dir_all(&path) {
        panic!(
            "创建临时目录失败: {}: {e}（临时根目录: {}）",
            path.display(),
            root.display()
        );
    }

    TempDir { path, keep: false }
}

/// `LZ_KEEP_TEMP=1|true|yes|on` ⇒ 保留临时目录并打印路径。
fn keep_temp_requested() -> bool {
    match std::env::var("LZ_KEEP_TEMP") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(_) => false,
    }
}

/// 目录名里的不安全字符（路径分隔符、空白、控制字符）替换为 `_`。
fn sanitize(prefix: &str) -> String {
    let s: String = prefix
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "lz-test".to_string()
    } else {
        s
    }
}

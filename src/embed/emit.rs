// Lang-Zone 编译器 — embed/emit.rs
// M3 入口融合（T0r55.1.1）：把 `emit_embed_artifacts` 从 main.rs 下沉到 lib crate，
// 让 CY/（lzcyc）能通过 `lang_zone::embed::emit::emit_embed_artifacts` 调用。
//
// 原函数在 main.rs:1439-1523，`pub(crate)` 可见性，只被 main crate 的 `src/cli.rs`
// 和 main.rs 内部调用。下沉后改为 `pub fn`，调用面改为 `lang_zone::embed::emit::emit_embed_artifacts`。
//
// 依赖全部是 lib 模块（`lang_zone::embed::artifact::*` / `lang_zone::embed::ArtifactKind`）
// 和 std，无 main-crate 特有依赖 ⇒ 可原样迁移。

use std::path::{Path, PathBuf};

/// 把 embed 产物落盘并（可选）建库。原 `main.rs::emit_embed_artifacts`（T0r55.1.1 下沉）。
///
/// `root_override`：产物根（`build/embed` 的父目录）。`None` = 进程 cwd ——
/// 单文件路径与 `--project` 路径的历史口径一字不动（`dedup::module_key` 也按 cwd
/// 量模块身份）。`lz build` 走 `Some(项目根)`：它可能在**别的目录**被调用
/// （`lz build ../proj`），产物必须跟 `<项目根>/build/<name>.exe` 落在同一棵树下，
/// 否则垫片里的 `build/embed/...` 与盘上位置不是同一把尺。
pub fn emit_embed_artifacts(
    targets: &[crate::embed::EmbedTarget],
    embed_build: bool,
    receipts_to_stderr: bool,
    root_override: Option<&Path>,
) {
    use crate::embed::artifact::{build_artifact, write_artifacts, BuildStatus};
    use crate::embed::ArtifactKind;
    // 回执出口：两条路径共用同一份文案，只是流不同
    macro_rules! receipt {
        ($($arg:tt)*) => {
            if receipts_to_stderr {
                eprintln!($($arg)*)
            } else {
                println!($($arg)*)
            }
        };
    }
    let root = root_override
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let mut failed = 0usize;
    let mut skipped = 0usize;
    for t in targets {
        let kind = match t.kind {
            ArtifactKind::Executable => "executable",
            ArtifactKind::Library => "library",
        };
        match write_artifacts(&root, t) {
            Ok(ops) => {
                let w = ops.iter().filter(|o| o.action == "written").count();
                receipt!(
                    "embed 产物: {} kind={} files={} (写盘 {} / 未变 {})",
                    t.dir_rel,
                    kind,
                    ops.len(),
                    w,
                    ops.len() - w
                );
            }
            Err(e) => {
                eprintln!("embed 产物写盘失败 {}: {}", t.dir_rel, e);
                failed += 1;
            }
        }
        if embed_build {
            let o = build_artifact(&root, t, false);
            match o.status {
                BuildStatus::NotNeeded => {
                    receipt!("embed 建库: {} 无该步（{}）", o.artifact, o.detail)
                }
                BuildStatus::Built => receipt!("embed 建库: {} 完成 ({})", o.artifact, o.cmd),
                BuildStatus::Skipped => {
                    receipt!("embed 建库: {} 跳过 —— {}", o.artifact, o.detail);
                    skipped += 1;
                }
                BuildStatus::Failed => {
                    eprintln!(
                        "embed 建库: {} 失败: {}\n  命令: {}",
                        o.artifact, o.detail, o.cmd
                    );
                    failed += 1;
                }
            }
        }
        // 取不到的调用面逐条点名：静默少一个函数正是本项目历来在清的那类错
        for m in t.unsupported_methods() {
            receipt!(
                "embed 未覆盖: {}::{} —— {}",
                t.artifact,
                m.name,
                m.unsupported
                    .as_deref()
                    .unwrap_or("签名不在可序列化闭集 int/float/str/bool 内")
            );
        }
    }
    if failed > 0 {
        eprintln!("embed: {} 件失败 ⇒ 非零退出（不许报成功）", failed);
        std::process::exit(1);
    }
    if skipped > 0 {
        eprintln!("embed: {} 件因工具链缺失跳过建库 ⇒ 产物已落盘但未验证", skipped);
    }
}

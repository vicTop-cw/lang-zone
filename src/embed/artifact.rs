// Lang-Zone 编译器 — embed/artifact.rs
// Cli 档产物的三件事：落盘（幂等）→ MANIFEST（确定性文本）→ 建库/执行（工具链不在场必须结构化跳过）。
//
// 「不静默成功」是本文件的硬约束：工具链缺失 ⇒ Skipped 带原因；建库失败 ⇒ Failed 带命令与输出尾行。
// 调用方（CLI `--embed-build` / 测试）拿到的是状态集合，不是一个 bool。

use std::path::{Path, PathBuf};

use super::{ArtifactKind, EmbedTarget};

/// 产物根目录（仓内相对路径）
pub const EMBED_ROOT: &str = "build/embed";

pub fn dir_rel(stem: &str, artifact: &str) -> String {
    format!("{}/{}/{}", EMBED_ROOT, stem, artifact)
}

/// 落盘一个产物：内容相同则**完全不写**（不动 mtime），故复跑可被「幂等」判据检查
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOutcome {
    pub rel: String,
    pub action: &'static str, // "written" | "unchanged"
}

pub fn write_artifacts(repo_root: &Path, t: &EmbedTarget) -> std::io::Result<Vec<WriteOutcome>> {
    let dir = artifact_dir(repo_root, t);
    std::fs::create_dir_all(&dir)?;
    let mut out = Vec::new();
    for (rel, content) in &t.files {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = content.as_bytes();
        let action = match std::fs::read(&path) {
            Ok(cur) if cur.as_slice() == bytes => "unchanged",
            _ => {
                std::fs::write(&path, bytes)?;
                "written"
            }
        };
        out.push(WriteOutcome {
            rel: format!("{}/{}", t.dir_rel, rel),
            action,
        });
    }
    // MANIFEST 自身也走幂等写
    let mpath = dir.join("MANIFEST.txt");
    let text = manifest_text(t);
    let action = match std::fs::read(&mpath) {
        Ok(cur) if cur.as_slice() == text.as_bytes() => "unchanged",
        _ => {
            std::fs::write(&mpath, text.as_bytes())?;
            "written"
        }
    };
    out.push(WriteOutcome {
        rel: format!("{}/MANIFEST.txt", t.dir_rel),
        action,
    });
    Ok(out)
}

pub fn artifact_dir(repo_root: &Path, t: &EmbedTarget) -> PathBuf {
    let mut p = repo_root.to_path_buf();
    for seg in t.dir_rel.split('/') {
        p.push(seg);
    }
    p
}

/// 确定性 key=value 文本：不含本机绝对路径、不含时间戳 ⇒ 可进 golden 比对
pub fn manifest_text(t: &EmbedTarget) -> String {
    let mut s = String::new();
    s.push_str(&format!("artifact={}\n", t.artifact));
    s.push_str(&format!("lang={}\n", t.lang));
    s.push_str(&format!(
        "tier={}\n",
        match t.tier {
            super::EmbedTier::Source => "source",
            super::EmbedTier::Cli => "cli",
        }
    ));
    s.push_str(&format!(
        "kind={}\n",
        match t.kind {
            ArtifactKind::Executable => "executable",
            ArtifactKind::Library => "library",
        }
    ));
    s.push_str(&format!("dir={}\n", t.dir_rel));
    for (rel, _) in &t.files {
        s.push_str(&format!("file={}\n", rel));
    }
    s.push_str(&format!(
        "build={}\n",
        match &t.build {
            Some(c) => c.display(),
            None => "none".to_string(),
        }
    ));
    s.push_str(&format!("run={}\n", t.run.display()));
    s.push_str(&format!(
        "toolchain_available={}\n",
        if t.run.tool_available() { "yes" } else { "no" }
    ));
    s.push_str(&format!(
        "dispatch={}\n",
        if t.has_dispatch_file() {
            "generated"
        } else {
            "none"
        }
    ));
    for m in &t.methods {
        s.push_str(&format!(
            "method={} params={} ret={} {}\n",
            m.name,
            m.params
                .iter()
                .map(|p| format!("{}:{}", p.name, p.lz_type))
                .collect::<Vec<_>>()
                .join(","),
            m.ret.clone().unwrap_or_else(|| "none".to_string()),
            if m.callable() {
                "callable".to_string()
            } else {
                m.unsupported
                    .as_deref()
                    .unwrap_or("uncallable")
                    .replace(' ', "_")
            }
        ));
    }
    s
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStatus {
    /// 该语言无建库步骤（解释型）—— 不是失败，也不是成功，是没有这一步
    NotNeeded,
    Built,
    /// 工具链不在场：产物已落盘，但**不许报成功**
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutcome {
    pub artifact: String,
    pub status: BuildStatus,
    pub cmd: String,
    pub detail: String,
}

/// 执行建库步骤。`dry` = 只判工具链在场性、不真跑命令（默认路径用，避免编译期起子进程）。
pub fn build_artifact(repo_root: &Path, t: &EmbedTarget, dry: bool) -> BuildOutcome {
    let base = BuildOutcome {
        artifact: t.artifact.clone(),
        status: BuildStatus::NotNeeded,
        cmd: "none".to_string(),
        detail: "该语言无建库步骤（解释型）".to_string(),
    };
    let cmd = match &t.build {
        None => return base,
        Some(c) => c.clone(),
    };
    if !cmd.tool_available() {
        return BuildOutcome {
            artifact: t.artifact.clone(),
            status: BuildStatus::Skipped,
            cmd: cmd.display(),
            detail: t.hint.clone(),
        };
    }
    if dry {
        return BuildOutcome {
            artifact: t.artifact.clone(),
            status: BuildStatus::NotNeeded,
            cmd: cmd.display(),
            detail: "dry：工具链在场，未执行建库".to_string(),
        };
    }
    let dir = artifact_dir(repo_root, t);
    match std::process::Command::new(&cmd.program)
        .args(&cmd.args)
        .current_dir(&dir)
        .output()
    {
        Ok(o) if o.status.success() => BuildOutcome {
            artifact: t.artifact.clone(),
            status: BuildStatus::Built,
            cmd: cmd.display(),
            detail: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        Ok(o) => BuildOutcome {
            artifact: t.artifact.clone(),
            status: BuildStatus::Failed,
            cmd: cmd.display(),
            detail: tail(&String::from_utf8_lossy(&o.stderr)),
        },
        Err(e) => BuildOutcome {
            artifact: t.artifact.clone(),
            status: BuildStatus::Failed,
            cmd: cmd.display(),
            detail: format!("spawn 失败: {}", e),
        },
    }
}

fn tail(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let start = lines.len().saturating_sub(6);
    lines[start..].join("\n")
}

/// 产物目录里是否已具备运行条件（Library 要有分发入口，Executable 要有建出的可执行或可直接跑）
pub fn runnable(t: &EmbedTarget) -> bool {
    match t.kind {
        ArtifactKind::Library => t.has_dispatch_file(),
        ArtifactKind::Executable => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_root(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("lz_embed_artifact_{}", tag));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn target(lang: &str, src: &str) -> EmbedTarget {
        let mut t = super::super::resolve(lang, "input.lz", src).unwrap();
        t.dir_rel = dir_rel("input", &t.artifact.clone());
        t
    }

    #[test]
    fn manifest_is_deterministic_text() {
        let t = target("python", "def add(a: int, b: int) -> int:\n    return a + b\n");
        let a = manifest_text(&t);
        let b = manifest_text(&t);
        assert_eq!(a, b, "MANIFEST 两次生成必须逐字相同");
        assert!(a.contains("artifact=python_input"));
        assert!(a.contains("kind=library"));
        assert!(a.contains("method=add params=a:int,b:int ret=int callable"));
        assert!(!a.contains("C:\\"), "MANIFEST 不许含本机绝对路径");
    }

    #[test]
    fn writing_twice_is_unchanged() {
        let root = tmp_root("idem");
        let t = target("python", "def f(x: int) -> int:\n    return x\n");
        let w1 = write_artifacts(&root, &t).unwrap();
        assert!(w1.iter().all(|o| o.action == "written"), "{:?}", w1);
        let w2 = write_artifacts(&root, &t).unwrap();
        assert!(
            w2.iter().all(|o| o.action == "unchanged"),
            "复跑必须一格都不写盘：{:?}",
            w2
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unsupported_signature_is_written_not_guessed() {
        let t = target("python", "def f(x: DenseMatrix) -> int:\n    return 1\n");
        let m = manifest_text(&t);
        assert!(m.contains("uncallable") || m.contains("闭集"), "{}", m);
        assert_eq!(t.unsupported_methods().len(), 1);
    }

    #[test]
    fn missing_toolchain_is_skipped_not_success() {
        let root = tmp_root("skip");
        let mut t = target("python", "def f(x: int) -> int:\n    return x\n");
        // 造一个必定不在 PATH 上的工具链 —— 这是「Skipped ≠ Built」的对照格
        t.build = Some(super::super::CommandSpec::new(
            "definitely_not_a_real_tool_lz_r15",
            &["build", "x"],
        ));
        let o = build_artifact(&root, &t, false);
        assert_eq!(o.status, BuildStatus::Skipped, "{:?}", o);
        // 正向对照：解释型语言的 build=None 走 NotNeeded，两者不得混为一谈
        let base = target("python", "def f(x: int) -> int:\n    return x\n");
        assert_eq!(build_artifact(&root, &base, true).status, BuildStatus::NotNeeded);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `Built` 这条分支以前**没有任何判据打到过**（本机没有 mojo/gcc，只测了 Skipped）。
    /// 这里用一定在场的 rustc 当"外语言工具链"，把建库步的成功面真跑一遍并核产物。
    #[test]
    fn build_step_actually_builds_when_tool_present() {
        let root = tmp_root("built");
        let mut t = target("python", "def f(x: int) -> int:\n    return x\n");
        let dir = artifact_dir(&root, &t);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("stub.rs"), "fn main() {}\n").unwrap();
        t.build = Some(super::super::CommandSpec::new(
            "rustc",
            &["--edition", "2021", "-o", "stub.exe", "stub.rs"],
        ));
        let o = build_artifact(&root, &t, false);
        assert_eq!(o.status, BuildStatus::Built, "{:?}", o);
        assert!(
            dir.join(if cfg!(windows) { "stub.exe" } else { "stub" }).is_file()
                || dir.join("stub.exe").is_file(),
            "建库判据必须看到产物落盘：{:?}",
            std::fs::read_dir(&dir).map(|rd| rd.map(|e| e.unwrap().file_name()).collect::<Vec<_>>())
        );

        // 同一套路子上放一个必然编译失败的文件 ⇒ 必须是 Failed 且带诊断尾行
        std::fs::write(dir.join("bad.rs"), "fn main() { let x: i64 = \"s\"; }\n").unwrap();
        t.build = Some(super::super::CommandSpec::new(
            "rustc",
            &["--edition", "2021", "-o", "bad.exe", "bad.rs"],
        ));
        let bad = build_artifact(&root, &t, false);
        assert_eq!(bad.status, BuildStatus::Failed, "{:?}", bad);
        assert!(!bad.detail.trim().is_empty(), "失败必须带诊断，不能只报状态");
        let _ = std::fs::remove_dir_all(&root);
    }
}

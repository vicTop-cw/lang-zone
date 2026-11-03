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
    // MANIFEST 自身也走幂等写（run_available 按**产物目录**量，与垫片同一把尺）
    let mpath = dir.join("MANIFEST.txt");
    let text = manifest_text_at(t, Some(&dir));
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
    manifest_text_at(t, None)
}

/// 同 `manifest_text`，但额外告知产物目录 —— `run_available` 要按**垫片同一把尺**解析
/// 相对程序名（`./c_calc.exe` 在产物目录里，不在编译器 cwd 里）。
/// 传 `None` 时按进程 cwd 解析（旧行为）；落盘路径（`write_artifacts`）传 `Some(产物目录)`。
pub fn manifest_text_at(t: &EmbedTarget, artifact_dir: Option<&Path>) -> String {
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
    // 两行各自可归因（T0r49 §2.1）：旧版只有一行 `toolchain_available=`，量的是 **run 命令**，
    // 于是 gcc 建库成功之后仍报 `no`（c 的 run 是相对产物名 `./c_calc.exe`，按编译器 cwd 解不开），
    // 而 scala 的 run 是 `.bat` 包装、扫文件名说「在场」、spawn 说「找不到」——一个读数回答不了两个问题。
    // `toolchain_available=` 从此只回答**建库工具链在场性**（无建库步 = `n/a`，是「没有这一步」而非失败）；
    // `run_available=` 回答**运行命令/产物在位性**（相对程序名按产物目录解析，与垫片同一把尺）。
    s.push_str(&format!(
        "toolchain_available={}\n",
        match &t.build {
            Some(b) => if b.tool_available() { "yes" } else { "no" },
            None => "n/a",
        }
    ));
    s.push_str(&format!(
        "run_available={}\n",
        if run_available_at(t, artifact_dir) { "yes" } else { "no" }
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

/// `run_available` 的读数口径：run 命令指向的东西现在在不在位。
///
/// 与垫片 `__lz_resolve_program` 同一把尺（`src/bridge/cli.rs`）：
///  - 含分隔符的**相对**程序名（c 的 `./c_calc.exe`）→ 拼到产物目录下判断，
///    修前恒 `no`（按编译器 cwd 解不开），修后建库成功即 `yes`；
///  - 裸名（python / mojo / scala）→ 交给 PATH（`tool_available`）；
///  - 绝对路径 → 直接看存在性。
/// 不 spawn，只判存在性 ⇒ 无副作用。
pub fn run_available_at(t: &EmbedTarget, artifact_dir: Option<&Path>) -> bool {
    let program = &t.run.program;
    let p = Path::new(program);
    if p.is_absolute() {
        return p.exists();
    }
    if program.contains('/') || program.contains('\\') {
        // 相对且含分隔符：按产物目录解析；没有目录信息时无从判定 ⇒ 如实报 no
        return match artifact_dir {
            Some(d) => d.join(p).is_file(),
            None => false,
        };
    }
    t.run.tool_available()
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
    // `.bat`/`.cmd` 形态的工具链（Coursier 的 scalac 是典型）不能直接 CreateProcess ——
    // 裸名只会补 `.exe`、不认 PATHEXT，而 `tool_available` 扫文件名认 `.bat`，
    // 于是「读数说在场、spawn 说找不到」（T0r49 §2.4）。两边必须同一把尺：
    // 这里经 `cmd_route_shape` 换 `cmd /C` 起（cmd 自己按 PATHEXT 解析）。
    let (program, route_prefix) = super::cmd_route_shape(&cmd.program);
    let mut spawn = std::process::Command::new(program);
    spawn
        .args(&route_prefix)
        .args(&cmd.args)
        .current_dir(&dir);
    match spawn.output() {
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

    /// 进程内/跨进程唯一的临时产物根（T0r44）。
    ///
    /// 这里以前是 `%TEMP%/lz_embed_artifact_{tag}` 这种**固定路径**，入口还先 `remove_dir_all`：
    /// 两条车道同时跑 `cargo test --lib` 时，同一个测试二进制的两个实例会互删目录，
    /// `build_step_actually_builds_when_tool_present` 就会偶发 `couldn't read stub.rs`（单跑即过）。
    /// 现在名字里拼上 pid + 进程内原子序号 + 纳秒时间戳，两个实例各自拿到自己的目录。
    fn tmp_root(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "lz_embed_artifact_{}_{}_{}_{}",
            tag,
            std::process::id(),
            seq,
            nanos
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        // 失败时这行会随 panic 一起打出来，路径可直接去看（成功时被 libtest 捕获）
        eprintln!("[lz_embed_artifact] tmp_root={}", p.display());
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

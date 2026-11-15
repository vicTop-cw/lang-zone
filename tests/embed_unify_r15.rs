// Lang-Zone 编译器 — tests/embed_unify_r15.rs
// 嵌入式语法统一抽象的门禁（规划 plans/azure-storm-starling.md，阶段 1–6 的验收面）。
//
// 每道判据都配一个"必然红"的对照面，防止判据自己坏掉：
//   Source 档逐字不变  ⇄  Cli 档不再进 .rs
//   工具链在场真跑通   ⇄  工具链缺失只许报「跳过」
//   取到调用面         ⇄  取不到时必须点名「未覆盖」
//
// 兼容性红线（本文件的存在理由）：`#[embed(tnr)]` / `#[embed(rust)]` 的产物文本、
// `input_tnr` 这类 `<base>_<lang>` 模块名、以及"不 spawn 外部程序"的默认行为都不许变。

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// ── 产物目录解析（车道隔离，照 tests/demo_codegen_compile.rs 的 T0r13 先例）──────
//
// 曾经的硬编码：`repo_root()/target/debug`。后果（T0r50 在隔离 worktree 实测）：
// 用 `CARGO_TARGET_DIR=target/lane-xxx` 跑的车道，rlib 指向**默认车道**目录 ——
// 轻则链到陈旧产物，重则产物不存在而假红。
// 现在一律按**当前运行环境**推导 `<target-dir>/<profile>`，优先级：
//   a) 环境变量 `CARGO_TARGET_DIR`（相对路径按 crate 根解析；自身含 deps/ 直接用，否则补 profile 名）；
//   b) `std::env::current_exe()` 反推（`<target-dir>/<profile>/deps/<test-bin>` → `<target-dir>/<profile>`）；
//   c) 兜底 `CARGO_MANIFEST_DIR/target/debug`。

/// 相对路径按 crate 根解析（cargo 在 crate 根执行，相对 `CARGO_TARGET_DIR` 亦以此为基准）
fn resolve_from_manifest(p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        p
    } else {
        repo_root().join(p)
    }
}

/// `<target-dir>/<profile>` —— 由当前测试可执行文件反推：
/// `.../<target-dir>/<profile>/deps/<test-bin>[.exe]` → `.../<target-dir>/<profile>`
fn profile_dir_from_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    // cargo test 的 exe 恒在 `<profile>/deps` 下；不在则按 `<profile>` 直落处理
    let profile_dir = if dir.file_name().map_or(false, |n| n == "deps") {
        dir.parent()?
    } else {
        dir
    };
    if profile_dir.is_dir() {
        Some(profile_dir.to_path_buf())
    } else {
        None
    }
}

fn profile_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "debug".to_string())
}

/// 本次运行真正使用的 `<target-dir>/<profile>` 目录（见上方 a/b/c 解析链）。
fn build_profile_dir() -> PathBuf {
    if let Some(raw) = std::env::var_os("CARGO_TARGET_DIR") {
        let raw = PathBuf::from(raw);
        if !raw.as_os_str().is_empty() {
            let root = resolve_from_manifest(raw);
            // 已直接指向 profile 目录（自带 deps/）→ 就用它
            if root.join("deps").is_dir() {
                return root;
            }
            // profile 名优先跟随当前测试可执行文件（debug / release / 自定义）
            let profile = profile_dir_from_exe()
                .map(|d| profile_name(&d))
                .unwrap_or_else(|| "debug".to_string());
            return root.join(profile);
        }
    }
    if let Some(dir) = profile_dir_from_exe() {
        return dir;
    }
    repo_root().join("target").join("debug")
}

/// 与 `build_profile_dir()` 配对的 cargo `--target-dir`（即 `<profile>` 的父目录）
fn build_target_root() -> PathBuf {
    let dir = build_profile_dir();
    dir.parent().map(|p| p.to_path_buf()).unwrap_or(dir)
}

/// 目录内**最新**（mtime 最大）的 `deps/liblz_builtins-*.rlib`。
/// 同一 target 下可能残留多个 hash 的 rlib，取最新避免链到陈旧产物。
fn newest_deps_rlib(dir: &Path) -> Option<PathBuf> {
    let deps = dir.join("deps");
    let entries = std::fs::read_dir(&deps).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
            let mtime = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if best.as_ref().map_or(true, |(t, _)| mtime > *t) {
                best = Some((mtime, e.path()));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// 在某个 `<profile>` 目录里定位 lz_builtins 的 .rlib：
/// 先 `deps/liblz_builtins-*.rlib` 取最新，再退回顶层 `liblz_builtins.rlib`
/// （cargo 为 lib target 产的 hardlink，仅作兜底，不再作为首选）。
fn probe_rlib(dir: &Path) -> Option<PathBuf> {
    if let Some(p) = newest_deps_rlib(dir) {
        return Some(p);
    }
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return Some(direct);
    }
    None
}

fn builtins_rlib() -> PathBuf {
    // 生成的 .rs 要 `--extern lz_builtins=…` 才链得上。`lz_builtins` 是 workspace 成员而**不是**
    // 根包的依赖 ⇒ `cargo test --test embed_unify_r15` 在一份干净的树上根本不会把它编出来
    // （冻结快照树实测：profile 目录里 0 件 lz_builtins 产物，11 条用例全死在这一行）。
    // 所以缺件时当场补建一次，让本套判据对任何树都自足，而不是"只在跑过全量 build 的机器上绿"。
    //
    // 取数**与补建**都跟着本次运行的 `<target-dir>/<profile>` 走（见上方解析链）。旧实现把
    // `repo_root()/target/debug` 写死 ⇒ `CARGO_TARGET_DIR=target/lane-xxx` 时补建产物落到车道目录、
    // 解析却看向默认目录，当场 12 条假红（T0r50 在隔离 worktree 实测）。
    let profile_dir = build_profile_dir();
    if let Some(p) = probe_rlib(&profile_dir) {
        return p;
    }
    let _ = std::fs::create_dir_all(&profile_dir);
    // 显式传 `--target-dir`：与 build_profile_dir() 同一口径，补建产物才会落在解析看向的目录。
    let profile = profile_name(&profile_dir);
    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(repo_root())
        .args(["build", "-p", "lz_builtins"])
        .arg("--target-dir")
        .arg(build_target_root());
    if profile != "debug" {
        // release / 自定义 profile 与测试可执行文件所在 profile 对齐
        cmd.arg("--profile").arg(&profile);
    }
    let built = cmd.output();
    if let Ok(b) = &built {
        if !b.status.success() {
            panic!(
                "lz_builtins 补建失败：{}\n若本机无 cargo，请手工产出 {}/liblz_builtins.rlib 后复跑",
                String::from_utf8_lossy(&b.stderr),
                profile_dir.display()
            );
        }
    }
    probe_rlib(&profile_dir).unwrap_or_else(|| {
        panic!(
            "已跑 cargo build -p lz_builtins（{:?}）仍在 {} 找不到产物 ⇒ 取数路径变了，改本行而非放宽判据",
            built.as_ref().map(|b| b.status.code()).ok(),
            profile_dir.display()
        )
    })
}

/// 一次完整的编译：cwd 固定在用例自己的临时目录 ⇒ 产物落 <work>/build/embed/...，
/// 生成的可执行也以同 cwd 运行 ⇒ 垫片里的相对目录可解析，且不污染仓库。
struct Cc {
    work: PathBuf,
    rs_text: String,
    compile_stdout: String,
    compile_stderr: String,
}

impl Cc {
    /// 编译期回执 = stdout + stderr（建库跳过/未验证这类信息落在 stderr）
    fn receipts(&self) -> String {
        format!("{}{}", self.compile_stdout, self.compile_stderr)
    }
}

fn compile_case(name: &str, src: &str, extra: &[&str]) -> Cc {
    let work = repo_root()
        .join("target/embed_r15")
        .join(name);
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");
    let lz = work.join("calc.lz");
    std::fs::write(&lz, src).expect("write lz");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let mut cmd = Command::new(&bin);
    cmd.current_dir(&work).arg(&lz).args(extra);
    let out = cmd.output().expect("run lang-zone");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "[{name}] 编译失败: stdout={stdout} stderr={stderr}"
    );
    let rs_text = std::fs::read_to_string(work.join("calc.rs")).expect("read generated rs");
    Cc {
        work,
        rs_text,
        compile_stdout: stdout,
        compile_stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

/// 编译并运行生成的 .rs，返回程序 stdout
fn run_case(cc: &Cc, name: &str) -> String {
    let rs = cc.work.join("calc.rs");
    let exe = cc.work.join("calc.exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            // 链接目录跟随本次运行的实际产物目录（车道隔离）；旧写法 rlib 在 deps/ 时会算出 deps/deps
            build_profile_dir().join("deps").display()
        ))
        .arg("-o")
        .arg(&exe)
        .current_dir(&cc.work)
        .output()
        .expect("run rustc");
    assert!(
        rc.status.success(),
        "[{name}] rustc 编译失败:\n{}",
        String::from_utf8_lossy(&rc.stderr)
    );
    let run = Command::new(&exe)
        .current_dir(&cc.work)
        .output()
        .expect("run exe");
    let so = String::from_utf8_lossy(&run.stdout).to_string();
    let se = String::from_utf8_lossy(&run.stderr).to_string();
    assert!(run.status.success(), "[{name}] 运行失败: {se}\nstdout={so}");
    so
}

fn py_lib() -> String {
    "// 内嵌 python 库（无 main ⇒ 库产物）\n#[embed(python)]\n```\ndef add(a: int, b: int) -> int:\n    return a + b\n```\n\ndef main():\n    print(python_calc_add(2, 3))\n".to_string()
}

// ───────────────────────── 兼容面：Source 档不许动 ─────────────────────────

#[test]
fn source_tier_embeds_stay_verbatim_and_emit_no_shim() {
    let src = "#[embed(rust)]\n```\nfn rust_only_marker(a: i64) -> i64 { a * 2 }\n```\n\n#[embed(tnr)]\n```\nfn tnr_only_marker(a, b) { a + b }\n```\n\ndef main():\n    print(1)\n";
    let cc = compile_case("source_tier", src, &[]);
    assert!(
        cc.rs_text.contains("fn rust_only_marker(a: i64) -> i64"),
        "Source 档（rust）必须继续原样内嵌，生成的 .rs:\n{}",
        cc.rs_text
    );
    assert!(cc.rs_text.contains("tnr_only_marker"));
    assert!(
        !cc.rs_text.contains("__lz_embed"),
        "没有 Cli 档块时不得凭空生成垫片模块:\n{}",
        cc.rs_text
    );
    assert!(
        !cc.compile_stdout.contains("embed 产物"),
        "Source 档不该产生任何产物落盘: {}",
        cc.compile_stdout
    );
}

#[test]
fn source_tier_text_and_cli_artifact_names_coexist() {
    // 兼容面判据：同一份语料里 Source 档继续逐字进 .rs，Cli 档只出产物 + 垫片；
    // 两侧命名空间互不串台（CLI 注入的 embed_stem 只作用于 Cli 档）。
    let src = "#[embed(tnr)]\n```\nfn helper(a, b) { a + b }\n```\n\n#[embed(python)]\n```\ndef add(a: int, b: int) -> int:\n    return a + b\n```\n\ndef main():\n    print(python_calc_add(2, 3))\n";
    let cc = compile_case("tnr_naming", src, &[]);
    assert!(
        cc.rs_text.contains("helper"),
        "Source 档（tnr）的文本必须留在产物里（feature 关时是注释，也在）:\n{}",
        cc.rs_text
    );
    assert!(cc.rs_text.contains("pub fn python_calc_add("));
    assert!(
        !cc.rs_text.contains("python_main_add"),
        "Cli 档产物名要跟随源文件主干 calc，不许退回 main:\n{}",
        cc.rs_text
    );
    assert!(
        !cc.rs_text.contains("def add(a: int"),
        "python 源码不许被打进 .rs:\n{}",
        cc.rs_text
    );
    assert_eq!(run_case(&cc, "tnr_naming").trim(), "5");
}

#[test]
fn import_of_embed_module_names_no_longer_blocked_by_semantics() {
    // 既有缺口（本轮顺手补上，属修复不属破坏）：语义检查此前完全不认 embed 产物名，
    // `import calc_tnr` / `import python_calc` 会把整个编译挡死在 "import 路径不存在"。
    let src = "#[embed(tnr)]\n```\nfn helper(a, b) { a + b }\n```\n\nimport calc_tnr\n\ndef main():\n    print(1)\n";
    let cc = compile_case("tnr_import", src, &[]);
    assert!(cc.rs_text.contains("fn main"), "语义检查放行后应正常出产物");
}

// ───────────────────────── 主判据：Cli 档端到端 ─────────────────────────

#[test]
fn python_lib_embed_calls_through_cli_and_returns_value() {
    let cc = compile_case("py_lib", &py_lib(), &[]);
    assert!(
        !cc.rs_text.contains("def add(a: int"),
        "外语言源码不得再被打进 .rs（那是上一轮的真实缺陷面）:\n{}",
        cc.rs_text
    );
    assert!(cc.rs_text.contains("pub mod __lz_embed"));
    assert!(cc.rs_text.contains("pub fn python_calc_add("));
    let out = run_case(&cc, "py_lib");
    assert_eq!(out.trim(), "5", "python 库产物经 CLI + 序列化取回的返回值必须是 5");
    // 摘出的源码与分发入口都落到了产物目录
    let dir = cc.work.join("build/embed/calc/python_calc");
    assert!(dir.join("python_calc.py").is_file());
    assert!(dir.join("__lz_dispatch_python_calc.py").is_file());
    let mani = std::fs::read_to_string(dir.join("MANIFEST.txt")).expect("manifest");
    assert!(mani.contains("kind=library"));
    assert!(mani.contains("artifact=python_calc"));
}

#[test]
fn python_main_embed_runs_as_executable() {
    let src = "#[embed(python)]\n```\nif __name__ == \"__main__\":\n    print(\"hi from py main\")\n```\n\ndef main():\n    print(python_calc_run())\n";
    let cc = compile_case("py_main", src, &[]);
    assert!(
        cc.rs_text.contains("pub fn python_calc_run()"),
        "有 main ⇒ 可执行产物，应生成 _run 调用面:\n{}",
        cc.rs_text
    );
    let out = run_case(&cc, "py_main");
    assert!(
        out.contains("hi from py main"),
        "可执行产物的 stdout 应作为结果取回，实际={out:?}"
    );
    let mani = std::fs::read_to_string(
        cc.work
            .join("build/embed/calc/python_calc/MANIFEST.txt"),
    )
    .expect("manifest");
    assert!(mani.contains("kind=executable"), "{}", mani);
    // 可执行产物不需要分发入口（它有自家 main）
    assert!(!mani.contains("dispatch=generated"), "{}", mani);
}

#[test]
fn same_language_can_import_the_library_artifact() {
    // 「无 main 函数作为库被其它相同语言导入」：产物名 = 模块名，同语言程序直接 import 即可
    let cc = compile_case("py_same_lang", &py_lib(), &[]);
    let dir = cc.work.join("build/embed/calc/python_calc");
    let peer = dir.join("peer.py");
    std::fs::write(
        &peer,
        "import python_calc as m\nprint(m.add(40, 2))\n",
    )
    .expect("write peer");
    let run = Command::new("python")
        .args(["-B", "peer.py"])
        .current_dir(&dir)
        .output()
        .expect("run peer");
    assert!(
        run.status.success(),
        "同语言导入失败: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        "42",
        "库产物必须能被同语言的其它程序直接 import"
    );
}

// ───────────────────────── mojo：结构面（本机无工具链）─────────────────────────

#[test]
fn mojo_embed_is_extracted_not_pasted() {
    let src = "#[embed(mojo)]\n```\nfn greet(name: String) -> String:\n    return name\n```\n\ndef main():\n    print(1)\n";
    let cc = compile_case("mojo_struct", src, &[]);
    assert!(
        !cc.rs_text.contains("fn greet(name: String)"),
        "mojo 源码不许再打进 .rs（原行为产出的 Rust 根本无法编译）:\n{}",
        cc.rs_text
    );
    assert!(cc.rs_text.contains("pub fn mojo_calc_greet("));
    let dir = cc.work.join("build/embed/calc/mojo_calc");
    assert_eq!(
        std::fs::read_to_string(dir.join("mojo_calc.mojo")).expect("mojo src"),
        // 词法器给的块文本自带一个前导换行（围栏后紧跟的那一行），摘出必须逐字保留
        "\nfn greet(name: String) -> String:\n    return name\n",
        "摘出的源码必须逐字等于内嵌块内容"
    );
    assert!(dir.join("__lz_dispatch_mojo_calc.mojo").is_file());
    let mani = std::fs::read_to_string(dir.join("MANIFEST.txt")).expect("manifest");
    // 本机无 mojo ⇒ 必须显式承建未验证，不许报成功
    assert!(
        !mani.contains("built") && mani.contains("toolchain_available="),
        "{}",
        mani
    );
}

#[test]
fn unknown_lang_is_not_pasted_and_has_no_callable_surface() {
    let src = "#[embed(cobol)]\n```\nIDENTIFICATION DIVISION.\n```\n\ndef main():\n    print(1)\n";
    let cc = compile_case("unknown_lang", src, &[]);
    assert!(
        !cc.rs_text.contains("IDENTIFICATION DIVISION"),
        "未知语言的文本不得进 .rs:\n{}",
        cc.rs_text
    );
    assert!(
        cc.compile_stdout.contains("embed 产物"),
        "未知语言仍应落盘为产物: {}",
        cc.compile_stdout
    );
    let out = run_case(&cc, "unknown_lang");
    assert_eq!(out.trim(), "1", "没有可调用面时母件照常编译运行");
}

// ───────────────────────── import 路由与「未覆盖」点名 ─────────────────────────

#[test]
fn import_of_artifact_does_not_become_a_bogus_use() {
    let src = "#[embed(python)]\n```\ndef add(a: int, b: int) -> int:\n    return a + b\n```\n\nimport python_calc\n\ndef main():\n    print(python_calc_add(1, 2))\n";
    let cc = compile_case("import_route", src, &[]);
    assert!(
        !cc.rs_text.contains("use python_calc;"),
        "import 产物名不能被转成一个不存在的 Rust 模块:\n{}",
        cc.rs_text
    );
    assert!(cc.rs_text.contains("import python_calc → Cli 档 embed 产物"));
    assert_eq!(run_case(&cc, "import_route").trim(), "3");
}

#[test]
fn non_closed_signature_is_named_as_uncovered_not_dropped() {
    // 形参无注解 ⇒ 无法确定序列化类型 ⇒ 不出垫片，但必须点名
    let src = "#[embed(python)]\n```\ndef needs_obj(x): return 1\ndef ok(n: int) -> int:\n    return n\n```\n\ndef main():\n    print(python_calc_ok(7))\n";
    let cc = compile_case("uncovered", src, &[]);
    assert!(
        !cc.rs_text.contains("python_calc_needs_obj"),
        "闭集外的签名不许出垫片:\n{}",
        cc.rs_text
    );
    assert!(
        cc.compile_stdout.contains("embed 未覆盖: python_calc::needs_obj"),
        "取不到的调用面必须逐条点名，实际回执: {}",
        cc.compile_stdout
    );
    assert_eq!(run_case(&cc, "uncovered").trim(), "7");
}

// ───────────────────────── 建库步骤：不默认 spawn / 缺失只报跳过 ─────────────────────────

#[test]
fn embed_build_flag_is_wired_and_reports_the_no_build_step_shape() {
    // `--embed-build` 这条 CLI 通路以前只在单测里被状态机覆盖，这里从**调用面**钉一次：
    // python 是解释型 ⇒ 建库步应为「无该步」，而不是被静默跳过、更不是失败。
    let cc = compile_case("embed_build_flag", &py_lib(), &["--embed-build"]);
    let r = cc.receipts();
    assert!(
        r.contains("embed 建库: python_calc 无该步"),
        "--embed-build 必须走到建库回执，实际: {r}"
    );
    assert!(!r.contains("失败"), "解释型语言不该被判成建库失败: {r}");
    let out = run_case(&cc, "embed_build_flag");
    assert_eq!(out.trim(), "5", "带 --embed-build 编译出来的母件仍须能跑");
}

#[test]
fn compile_alone_never_spawns_a_toolchain() {
    let cc = compile_case("no_spawn", &py_lib(), &[]);
    assert!(
        !cc.compile_stdout.contains("建库"),
        "默认路径不得跑任何外部建库命令: {}",
        cc.compile_stdout
    );
}

/// 在 PATH 上解析工具的绝对路径（Windows 再按 PATHEXT 补后缀）——只用于 skip 提示里点名「在哪」。
fn resolve_on_path(tool: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .map(|v| {
                v.split(';')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .ok()
            .filter(|v: &Vec<String>| !v.is_empty())
            .unwrap_or_else(|| vec![".exe".to_string()])
    } else {
        vec![String::new()]
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{tool}{ext}")))
            .find(|cand| cand.is_file())
    })
}

/// gcc 是否真在场：先在 PATH 上解析出路径，再用该路径实跑 `--version` 复核（与原先的探测同一口径）。
fn gcc_in_place() -> Option<PathBuf> {
    let p = resolve_on_path("gcc")?;
    let ok = Command::new(&p)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        Some(p)
    } else {
        None
    }
}

#[test]
fn missing_toolchain_is_reported_as_skip_not_success() {
    // c 档：**本机无 gcc** ⇒ --embed-build 必须报「跳过」，且不能报「完成」，退出码仍为 0。
    //
    // 这条判据的前提是「工具链缺失」而不是「工具链在场」：本机装了 gcc（如 CodeArts 自带的
    // MinGW）时前提不成立，本条不适用 ⇒ 点名打印后 return。既不能 panic（会把有 gcc 的机器
    // 整片拖红），也不能挂 #[ignore]（那会让本该跑的机器连这条判据都不跑）。
    if let Some(gcc) = gcc_in_place() {
        println!(
            "[skip] 本机存在 gcc（{}），本用例前提是\"工具链缺失\"，跳过",
            gcc.display()
        );
        return;
    }
    let src = "#[embed(c)]\n```\nint twice(int n) { return n * 2; }\n```\n\ndef main():\n    print(1)\n";
    let cc = compile_case("c_skip", src, &["--embed-build"]);
    assert!(
        cc.receipts().contains("跳过"),
        "gcc 缺失必须报跳过，实际: {}",
        cc.receipts()
    );
    assert!(
        !cc.receipts().contains("完成"),
        "工具链缺失时绝不许出现「建库完成」: {}",
        cc.receipts()
    );
    assert!(
        cc.receipts().contains("未验证"),
        "跳过还要点名「未验证」: {}",
        cc.receipts()
    );
}

// ───────────────────────── 幂等与生成面唯一性 ─────────────────────────

#[test]
fn second_run_writes_nothing_and_bytes_stay() {
    let cc = compile_case("idem", &py_lib(), &[]);
    let dir = cc.work.join("build/embed/calc/python_calc");
    let before = std::fs::read(dir.join("python_calc.py")).unwrap();
    let mtime_before = std::fs::metadata(dir.join("python_calc.py"))
        .unwrap()
        .modified()
        .unwrap();

    // 复跑（同一 work 目录，只重跑编译，不删盘）
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .current_dir(&cc.work)
        .arg(cc.work.join("calc.lz"))
        .output()
        .expect("rerun lang-zone");
    let so = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "复跑编译失败: {so}");
    assert!(
        so.contains("写盘 0 /"),
        "复跑必须一格都不再写盘，实际回执: {so}"
    );
    assert_eq!(std::fs::read(dir.join("python_calc.py")).unwrap(), before);
    assert_eq!(
        std::fs::metadata(dir.join("python_calc.py")).unwrap().modified().unwrap(),
        mtime_before,
        "不动盘就不该动 mtime"
    );
}

#[test]
fn embed_adds_exactly_one_spawn_site_and_it_lives_in_the_shim() {
    // 「子进程代码唯一生成点」这条不变量的诚实版：LZ 自己的 prelude 里本来就有
    // Command::new（process 内建），所以判据是**增量**，不是总数。
    let base = compile_case("spawn_base", "def main():\n    print(1)\n", &[]);
    let with = compile_case("spawn_with", &py_lib(), &[]);
    let n0 = base.rs_text.matches("Command::new(").count();
    let n1 = with.rs_text.matches("Command::new(").count();
    // 垫片固定两个 runner（协议调用 / 原样执行）⇒ 增量恒为 2，多一处就是漏到了别处
    assert_eq!(
        n1,
        n0 + 2,
        "一个 Cli 档产物只该多出两处 spawn（垫片的两个 runner），基线 {n0} 实测 {n1}"
    );
    let shim = &with.rs_text[with.rs_text.find("pub mod __lz_embed").unwrap()..];
    assert_eq!(
        shim.matches("Command::new(").count(),
        2,
        "垫片区外的 Command::new 必须为 0"
    );
}

#[test]
fn artifact_dir_layout_is_under_build() {
    let cc = compile_case("layout", &py_lib(), &[]);
    let dir = cc.work.join("build/embed/calc/python_calc");
    assert!(dir.is_dir(), "产物目录必须是 build/embed/<模块主干>/<产物名>");
    let entries: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    for want in ["python_calc.py", "__lz_dispatch_python_calc.py", "MANIFEST.txt"] {
        assert!(entries.contains(&want.to_string()), "{:?} 缺 {}", entries, want);
    }
}

#[test]
fn multiple_blocks_of_same_lang_merge_into_one_artifact() {
    let src = "#[embed(python)]\n```\ndef a(n: int) -> int:\n    return n\n```\n\n#[embed(python)]\n```\ndef b(n: int) -> int:\n    return n + 1\n```\n\ndef main():\n    print(python_calc_a(1))\n";
    let cc = compile_case("merge", src, &[]);
    let dir = cc.work.join("build/embed/calc/python_calc");
    let joined = std::fs::read_to_string(dir.join("python_calc.py")).unwrap();
    assert!(joined.contains("def a(") && joined.contains("def b("), "{joined}");
    assert!(cc.rs_text.contains("python_calc_a(") && cc.rs_text.contains("python_calc_b("));
    // 只应有一个 python 产物目录
    let sib = cc.work.join("build/embed/calc");
    let dirs: Vec<_> = std::fs::read_dir(&sib).unwrap().collect();
    assert_eq!(dirs.len(), 1, "同语言多块必须聚合成一个产物: {:?}", sib);
    assert_eq!(run_case(&cc, "merge").trim(), "1");
}

#[test]
fn str_params_and_results_cross_the_boundary_byte_exact() {
    // argv 传参的立身之本：含空格的字符串值不能被拆散（stdin 的 `k=v` 空格分隔版会）。
    // 判据不依赖 print 的格式选择：等值比较证明**逐字节**跨过边界，
    // 另用带注解的 let 证明取回的值在 LZ 侧就是普通 str（print 无引号）。
    let src = "#[embed(python)]\n```\ndef shout(s: str) -> str:\n    return s.upper()\n```\n\ndef main():\n    let hit = python_calc_shout(\"hello world lz\") == \"HELLO WORLD LZ\"\n    print(hit)\n    let s: str = python_calc_shout(\"a b c\")\n    print(s)\n";
    let cc = compile_case("space_arg", src, &[]);
    let out = run_case(&cc, "space_arg");
    let lines: Vec<&str> = out.trim().lines().map(|l| l.trim()).collect();
    assert_eq!(
        lines[0], "true",
        "含空格参数与返回值必须逐字节跨过边界，实际输出: {out:?}"
    );
    assert_eq!(
        lines[1], "A B C",
        "取回值经 str 注解后 print 不带引号，实际: {out:?}"
    );
}

#[test]
fn float_and_bool_roundtrip() {
    let src = "#[embed(python)]\n```\ndef half(x: float) -> float:\n    return x / 2.0\ndef is_zero(n: int) -> bool:\n    return n == 0\n```\n\ndef main():\n    print(python_calc_half(10.5))\n    print(python_calc_is_zero(0))\n";
    let cc = compile_case("num_bool", src, &[]);
    let out = run_case(&cc, "num_bool");
    let lines: Vec<&str> = out.trim().lines().map(|l| l.trim()).collect();
    assert_eq!(lines[0], "5.25", "{out}");
    assert!(
        lines[1] == "true" || lines[1] == "True",
        "bool 回传归一化后必须是 LZ 认得的真值: {out}"
    );
}

#[test]
fn path_helper_is_not_needed_when_repo_relative() {
    // 判据本身：产物目录只用相对路径，换 cwd 也能靠 LZ_EMBED_DIR 定位
    let cc = compile_case("envdir", &py_lib(), &[]);
    let rs = cc.work.join("calc.rs");
    let exe = cc.work.join("calc.exe");
    Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            build_profile_dir().join("deps").display()
        ))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("rustc");
    let run = Command::new(&exe)
        // 故意在别处运行，只给 LZ_EMBED_DIR
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
        .env(
            "LZ_EMBED_DIR",
            cc.work.display().to_string(),
        )
        .output()
        .expect("run");
    let so = String::from_utf8_lossy(&run.stdout).to_string();
    let se = String::from_utf8_lossy(&run.stderr).to_string();
    assert!(run.status.success(), "LZ_EMBED_DIR 定位失败: {se}");
    assert_eq!(so.trim(), "5", "垫片必须能从 LZ_EMBED_DIR 根拼出产物目录");
}

// ─────────────────────── C 档：分发入口真能被调起来（T0r49）───────────────────────
//
// 出处：docs/嵌入式统一抽象-实现-2026-10-21.md §六 已知限制第 2 条（「c 的分发入口未经工具链
// 验证」——落笔时本机没有 gcc）。本机 gcc 在场，一验就露出「建得出 ≠ 调得起来」：
// c 的 run 命令是**相对**程序名 `./c_calc.exe`，而 Windows 的 CreateProcess 按**父进程** cwd
// 解析程序名，`current_dir(产物目录)` 只管子进程 cwd ⇒ 修复前从任何非产物目录运行都
// `os error 2`（实测回执：`无法启动 ./c_calc.exe（系统找不到指定的文件。）`）。
// 判据分两半：gcc 在场时走真 gcc 那一路；gcc 缺席时用 rustc 造一个守协议的替身 —— 本条测的是
// 「程序路径怎么解析」，不是「gcc 能不能编」（后者的 Built 分支由 embed/artifact.rs 的单测钉着）。
#[test]
fn c_library_artifact_is_invocable_from_an_unrelated_cwd() {
    let src = "#[embed(c)]\n```\nint add(int a, int b) { return a + b; }\n```\n\ndef main():\n    print(c_calc_add(1, 2))\n";
    let cc = compile_case("c_lib_invoke", src, &["--embed-build"]);
    let dir = cc.work.join("build/embed/calc/c_calc");
    // c adapter 的建库命令固定 `gcc -O2 -o c_calc.exe …`，产物名与平台无关
    let exe = dir.join("c_calc.exe");
    assert!(
        cc.rs_text.contains("c_calc_add("),
        "c 库产物必须出可调用垫片:\n{}",
        cc.rs_text
    );
    if !exe.is_file() {
        eprintln!(
            "[c_lib_invoke] 产物目录里没有 c_calc.exe（gcc 是否在场？编译回执见下），改用 rustc 替身:\n{}",
            cc.compile_stdout
        );
        let stub = dir.join("__lz_stub.rs");
        std::fs::write(
            &stub,
            "fn main() {\n    let a: Vec<String> = std::env::args().collect();\n    let mut sum = 0i64;\n    for kv in a.iter().skip(2) {\n        if let Some((_, v)) = kv.split_once('=') {\n            sum += v.trim().parse::<i64>().unwrap_or(0);\n        }\n    }\n    println!(\"OK:{}\", sum);\n}\n",
        )
        .expect("write stub");
        let rc = Command::new("rustc")
            .args(["--edition", "2021", "-O", "-o"])
            .arg(&exe)
            .arg(&stub)
            .output()
            .expect("rustc stub");
        assert!(
            rc.status.success(),
            "替身编译失败: {}",
            String::from_utf8_lossy(&rc.stderr)
        );
    }
    // 关键一步：母件在**自己的** cwd 里跑，产物在 build/embed/... 子目录 —— 两者不是同一个 cwd
    let out = run_case(&cc, "c_lib_invoke");
    assert_eq!(
        out.trim(),
        "3",
        "c 库产物的分发入口必须能从与产物无关的 cwd 调起来（修复前是 os error 2）: {out:?}"
    );
}

// §6.4 修复判据：垫片调用返回 str 时直接 print 走 Display（{}，无引号），
// 不再被 print_arg_is_display 只看 e.ty 的判断漏掉而走 {:?}（Debug，带引号）。
// 对照面：先绑定成 str 再 print 的行为不变（无引号），证明两条路径一致。
// 负向对照：返回 int 的垫片调用 print 走 Debug 不影响（int 在两种格式下输出相同）。
#[test]
fn print_embed_shim_call_returning_str_uses_display_not_debug() {
    // python 垫片返回 str：直接 print 应无引号（修复前带引号）
    let src = "#[embed(python)]\n```\ndef shout(s: str) -> str:\n    return s.upper()\n```\n\ndef main():\n    print(python_calc_shout(\"ab\"))\n";
    let cc = compile_case("print_shim_str", src, &[]);
    let out = run_case(&cc, "print_shim_str");
    assert_eq!(
        out.trim(),
        "AB",
        "垫片调用返回 str 直接 print 必须走 Display（无引号），修复前带引号: {out:?}"
    );
    // 生成的 .rs 里 print 的格式串必须是 {} 而非 {:?}
    assert!(
        cc.rs_text.contains("println!(\"{}\", python_calc_shout(") || cc.rs_text.contains("println!(\"{}\","),
        "生成代码里 print 垫片调用必须走 {{}} 格式: {}",
        cc.rs_text
    );
}

// 负向对照：返回 int 的垫片调用 print 不受影响（int 在 {} 和 {:?} 下输出相同）
#[test]
fn print_embed_shim_call_returning_int_unchanged() {
    let src = "#[embed(python)]\n```\ndef double(x: int) -> int:\n    return x * 2\n```\n\ndef main():\n    print(python_calc_double(21))\n";
    let cc = compile_case("print_shim_int", src, &[]);
    let out = run_case(&cc, "print_shim_int");
    assert_eq!(
        out.trim(),
        "42",
        "垫片调用返回 int 的 print 输出不变: {out:?}"
    );
}

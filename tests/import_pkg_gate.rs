// 门禁：lz 模块导入 / 包构建 / 三方库 语料（C01–C06）
//
// 为什么需要这个门禁：tests/demo_codegen_compile.rs 的 enum Stage 只有
// Transpile / Rustc 两档，从不执行产物 —— 「能编译」不等于「运行正确」。
// BUG-22（f-string 断言写反长期不响）就是这个盲区的直接产物。
// 本门禁强制走到「转译 → rustc → 运行 → 校验 stdout + 退出码」。
//
// 语料与计划：tests/corpus/import_pkg/ + workbuddy/plan/2026-10-16-模块导入与包管理计划.md
//
// 期望状态的判定：每个用例在 run.rs 的 CASES 表里声明 expected。
//   - expected = PASS：必须真的跑通（stdout 含 OK 标记 + 退出码 0）
//   - expected = FAIL：必须真的不通（用于登记「当前能力边界」，它变绿时必须同步改期望）

use std::path::{Path, PathBuf};
use std::process::Command;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 单个用例的期望与入口
struct Case {
    dir: &'static str,
    entry: &'static str,
    /// 期望该用例当前是否应当跑通（true=跑通，false=当前能力边界，应失败）
    expected_pass: bool,
    desc: &'static str,
    /// 前置步骤（在跑 entry 之前于工作区执行），用于「先 push 发布再 build 消费」场景
    setup: Option<(&'static str, &'static [&'static str])>,
    /// 入口是否用项目模式（`lang-zone build`）而非单文件模式
    project_mode: bool,
    /// 负控断言：期望构建**失败**，且诊断须包含此关键字（如版本冲突）。
    /// `None` = 走正向判定（成功即通过）。
    expect_fail_with: Option<&'static str>,
}

const CASES: &[Case] = &[
    Case {
        dir: "c01_local_relative",
        entry: "main.lz",
        expected_pass: true,
        desc: "本地同目录 import + fn/struct 跨文件可见",
        setup: None,
        project_mode: false,
        expect_fail_with: None,
    },
    Case {
        dir: "c02_subdir_module",
        entry: "main.lz",
        // G1 已修（2026-10-16 阶段 1）：semantic_check.rs 的存在性检查改为与
        // 实际加载器同口径（按完整路径逐段拼接 + 末段平铺兜底）。
        expected_pass: true,
        desc: "子目录模块 import（G1 已修，锁为回归面）",
        setup: None,
        project_mode: false,
        expect_fail_with: None,
    },
    Case {
        dir: "c03_std_import",
        entry: "main.lz",
        expected_pass: true,
        desc: "std 库 import（对照组）",
        setup: None,
        project_mode: false,
        expect_fail_with: None,
    },
    Case {
        dir: "c04_pkg_layout",
        entry: "src/main.lz",
        expected_pass: true,
        desc: "包内多文件布局 + 兄弟模块 import",
        setup: None,
        project_mode: false,
        expect_fail_with: None,
    },
    Case {
        dir: "c05_thirdparty",
        // entry 消费 mathlib；setup 先在**用例根**push（registry 落于根，consumer 与之同级）
        entry: "consumer/src/main.lz",
        expected_pass: true,
        desc: "三方库 import（阶段 3 已通：push 发布 + dependencies 声明 + registry 解析）",
        setup: Some((".", &["push"])),
        project_mode: true,
        expect_fail_with: None,
    },
    Case {
        dir: "c06_version_conflict",
        entry: "consumer/src/main.lz",
        // 负控用例：run_case 在「构建按预期失败且诊断命中」时返回 true，
        // 故此处 expected_pass=true（表示「达到了期望状态」而非「构建成功」）。
        expected_pass: true,
        desc: "依赖版本冲突负控（build 必须失败且诊断点名「版本冲突」）",
        setup: Some((".", &["push"])),
        project_mode: true,
        // 负控：build 必须失败，且诊断须点名「版本冲突」而非泛化错误
        expect_fail_with: Some("版本冲突"),
    },
];

fn builtins_rlib() -> Option<PathBuf> {
    let dir = manifest_dir().join("target/debug/deps");
    let mut cands: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            n.starts_with("liblz_builtins-") && n.ends_with(".rlib")
        })
        .collect();
    cands.sort_by_key(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
    });
    cands.pop()
}

fn copy_dir_all(src: &Path, dst: &Path) {
    let _ = std::fs::create_dir_all(dst);
    if let Ok(rd) = std::fs::read_dir(src) {
        for e in rd.flatten() {
            let to = dst.join(e.file_name());
            if e.path().is_dir() {
                copy_dir_all(&e.path(), &to);
            } else {
                let _ = std::fs::copy(e.path(), &to);
            }
        }
    }
}

/// 跑一个用例，返回 (是否跑通, 诊断信息)
fn run_case(c: &Case, lzc: &Path, rlib: &Path, work_root: &Path) -> (bool, String) {
    let case_src = manifest_dir().join("tests/corpus/import_pkg").join(c.dir);
    if !case_src.is_dir() {
        return (false, format!("语料目录不存在: {}", case_src.display()));
    }
    let wd = work_root.join(c.dir);
    let _ = std::fs::remove_dir_all(&wd);
    copy_dir_all(&case_src, &wd);

    // 前置步骤（如：在用例根把库包 push 到 local-registry）
    if let Some((setup_dir, args)) = c.setup {
        let sp = wd.join(setup_dir);
        for a in args.iter().copied() {
            let out = Command::new(lzc)
                .arg(a)
                .current_dir(&sp)
                .output()
                .expect("failed to spawn lzc (setup)");
            if !out.status.success() {
                let combined = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                return (false, format!("setup 失败({}): {}", a, combined.trim()));
            }
        }
    }

    let entry = wd.join(c.entry);

    // 1) 转译（项目模式走 `build`，须在**包根**（lz.toml 所在目录）执行）
    let out = if c.project_mode {
        // entry 形如 consumer/src/main.lz → 包根是 entry 上两级
        let pkg_root = entry
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(&wd)
            .to_path_buf();
        Command::new(lzc)
            .arg("build")
            .current_dir(&pkg_root)
            .output()
            .expect("failed to spawn lzc")
    } else {
        Command::new(lzc)
            .arg(&entry)
            .current_dir(&wd)
            .output()
            .expect("failed to spawn lzc")
    };
    if !out.status.success() {
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        // 负控用例：期望「构建失败且诊断命中关键字」，此时失败即通过
        if let Some(kw) = c.expect_fail_with {
            return if combined.contains(kw) {
                (true, format!("按预期失败且诊断含「{}」", kw))
            } else {
                (
                    false,
                    format!("构建失败但诊断未含「{}」: {}", kw, combined.trim()),
                )
            };
        }
        let first = combined
            .lines()
            .find(|l| l.to_lowercase().contains("error"))
            .unwrap_or("转译失败");
        return (false, format!("转译失败: {}", first.trim()));
    }

    // 2) rustc（项目模式的 build 已产出 exe，直接运行；单文件模式自行编译）
    if c.project_mode {
        // 负控用例走到这里 = 构建**意外成功**（本应失败却过了）→ 判失败
        if c.expect_fail_with.is_some() {
            return (
                false,
                format!("本应失败却构建成功（期望诊断含「{}」）", c.expect_fail_with.unwrap()),
            );
        }
        // entry 形如 consumer/src/main.lz → 包根是上两级（consumer/），build/ 在其下
        let pkg_root = entry
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(&wd)
            .to_path_buf();
        let name = std::fs::read_to_string(pkg_root.join("lz.toml"))
            .ok()
            .and_then(|t| {
                t.lines().find_map(|l| {
                    l.trim().strip_prefix("name").and_then(|v| {
                        v.split('=')
                            .nth(1)
                            .map(|s| s.trim().trim_matches('"').to_string())
                    })
                })
            })
            .unwrap_or_else(|| "app".to_string());
        let exe = pkg_root.join("build").join(format!("{}.exe", name));
        if !exe.is_file() {
            return (false, format!("build 未产出 {}", exe.display()));
        }
        return run_and_check(&exe, &pkg_root);
    }

    let rs = entry.with_extension("rs");
    if !rs.is_file() {
        return (false, format!("未产出 {}", rs.file_name().unwrap().to_string_lossy()));
    }
    let exe = wd.join("case.exe");
    let dep_dir = rlib.parent().unwrap().to_path_buf();
    let out = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", rlib.display()))
        .arg("-L")
        .arg(format!("dependency={}", dep_dir.display()))
        .args(["-A", "warnings", "-o"])
        .arg(&exe)
        .current_dir(&wd)
        .output()
        .expect("failed to spawn rustc");
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err
            .lines()
            .find(|l| l.trim_start().starts_with("error"))
            .unwrap_or("编译失败");
        return (false, format!("编译失败: {}", first.trim()));
    }

    // 3) 运行 —— 本门禁的关键：不能止于「能编译」
    run_and_check(&exe, &wd)
}

/// 执行产物并校验：退出码 0 且 stdout 含 OK 标记
fn run_and_check(exe: &Path, cwd: &Path) -> (bool, String) {
    let out = Command::new(exe).current_dir(cwd).output().expect("failed to run exe");
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        return (
            false,
            format!(
                "运行退出码 {:?}: {}",
                out.status.code(),
                stdout.lines().last().unwrap_or("").chars().take(60).collect::<String>()
            ),
        );
    }
    if !stdout.contains("OK") {
        return (false, "退出码 0 但 stdout 无 OK 标记".to_string());
    }
    (
        true,
        stdout.lines().last().unwrap_or("").chars().take(60).collect::<String>(),
    )
}

#[test]
fn import_pkg_corpus_matches_expected_capability() {
    let lzc = manifest_dir().join("target/debug/lang-zone.exe");
    if !lzc.is_file() {
        panic!(
            "找不到 lzc: {}（先跑 cargo build --bin lang-zone）",
            lzc.display()
        );
    }
    let rlib = builtins_rlib().unwrap_or_else(|| {
        panic!("找不到 lz_builtins rlib（先跑 cargo build -p lz_builtins）")
    });

    let work = manifest_dir().join("target/import_pkg_gate");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    let mut mismatches: Vec<String> = Vec::new();
    let mut ok_count = 0usize;

    for c in CASES {
        let (ran, detail) = run_case(c, &lzc, &rlib, &work);
        if ran == c.expected_pass {
            ok_count += 1;
            eprintln!("[ok]   {:<22} {}", c.dir, detail);
        } else {
            // 期望跑通却失败 = 真回归；期望失败却跑通 = 能力提升，须同步改 CASES 期望
            let kind = if c.expected_pass { "回归" } else { "能力已提升" };
            mismatches.push(format!(
                "{:<22} [{}] 期望 {}，实际 {}",
                c.dir,
                kind,
                if c.expected_pass { "跑通" } else { "失败" },
                if ran { "跑通" } else { "失败" }
            ));
            eprintln!("[!!]   {:<22} {}", c.dir, detail);
        }
        eprintln!("       └ {}", c.desc);
    }

    eprintln!(
        "\n===== 模块导入/包管理语料：符合预期 {}/{} =====",
        ok_count,
        CASES.len()
    );
    assert!(
        mismatches.is_empty(),
        "以下用例实际表现与预期不符：\n  - {}",
        mismatches.join("\n  - ")
    );
}
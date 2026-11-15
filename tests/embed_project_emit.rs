// Lang-Zone 编译器 — tests/embed_project_emit.rs
// fist-mbt T0r31（命名空间 lang-zone）验收面：**project 路径 / `lz build` 路径真的
// 把 `build/embed/**` 落到盘上**，而且盘上目录与产物里垫子写的路径是同一把尺。
//
// 修前（src/main.rs:864-888、src/cli.rs:576-578）：
//   两条路径的 codegen 成功后**从不调用** `emit_embed_artifacts` ⇒ `temp/projemb`
//   `lang-zone main.lz --project` 退出码 0、`main.rs` 里垫片写着
//   `__lz_call(..., "build/embed/a/python_a", ...)`，但 `build/embed` 整棵目录不存在
//   ⇒ 编译产物一调外部函数就失败（找不到产物目录）。
// 修后：两条路径与单文件路径同口径 —— 默认**只落盘**（不 spawn 外部工具链），
//   显式 `--embed-build` 才建库；回执文案与失败语义（写盘失败非零退出）完全共用
//   同一个 `emit_embed_artifacts`。
//
// 判据只读「真实进程的产出 + 盘上的文件」，不靠内部字段自证：
//   ① project 模式编译后 `build/embed/**` 真的存在，且**每个模块一个产物目录**
//   ② 垫片里的路径与盘上目录双向一致（含 MANIFEST 的 `dir=` 自证）
//   ③ `--embed-build` 语义（有/无该参数）与单文件路径一致

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// ───────────────────────────── 通用夹具 ─────────────────────────────

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 每个用例一棵干净的工作目录（cwd 就是它 ⇒ 产物根 = 该目录，与
/// `lz <相对路径>` 的历史口径同一把尺）。
fn case_dir(name: &str) -> PathBuf {
    let d = repo_root().join("target/embed_project_emit").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create case dir");
    d
}

fn write_src(work: &Path, rel: &str, text: &str) {
    let p = work.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).expect("create src dir");
    }
    std::fs::write(&p, text).expect("write lz");
}

fn lz(work: &Path, args: &[&str]) -> Output {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    Command::new(&bin)
        .args(args)
        .current_dir(work)
        .output()
        .unwrap_or_else(|e| panic!("启动 {} 失败: {e}", bin.display()))
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn ok(work: &Path, args: &[&str], what: &str) -> Output {
    let o = lz(work, args);
    assert!(
        o.status.success(),
        "[{what}] `lang-zone {}` 失败（退出码 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        args.join(" "),
        o.status.code(),
        stdout(&o),
        stderr(&o)
    );
    o
}

/// 直接子目录名（排序）。不存在 ⇒ 空 vec（由调用方断言“该存在”）。
fn subdir_names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect(),
        Err(_) => Vec::new(),
    };
    v.sort();
    v
}

/// 盘上全部产物目录，形如 `build/embed/<stem>/<artifact>`（'/' 分隔，排序）。
fn artifact_dirs_on_disk(work: &Path) -> Vec<String> {
    let root = work.join("build/embed");
    let mut out = Vec::new();
    for stem in subdir_names(&root) {
        for art in subdir_names(&root.join(&stem)) {
            out.push(format!("build/embed/{}/{}", stem, art));
        }
    }
    out.sort();
    out
}

/// 从生成的 Rust 里摘出所有 `"build/embed/..."` 字面量（垫片传给 `__lz_call` 的目录）。
fn shim_dirs_in_rust(rs: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = rs;
    while let Some(i) = rest.find("\"build/embed/") {
        let after = &rest[i + 1..];
        match after.find('"') {
            Some(j) => {
                out.push(after[..j].to_string());
                rest = &after[j..];
            }
            None => break,
        }
    }
    out.sort();
    out.dedup();
    out
}

/// 产物目录里那唯一一份摘出来的源码（排除 dispatch 入口）。
fn one_py(dir: &Path) -> PathBuf {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("产物目录不存在 {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .filter(|p| {
            !p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("__lz_dispatch")
        })
        .collect();
    v.sort();
    assert_eq!(v.len(), 1, "{} 应有且仅有一份摘出的源码: {:?}", dir.display(), v);
    v.remove(0)
}

/// MANIFEST 里声明的 `dir=` —— 产物自己的身份自证。
fn manifest_dir(dir: &Path) -> String {
    let text = std::fs::read_to_string(dir.join("MANIFEST.txt"))
        .unwrap_or_else(|e| panic!("{} 缺 MANIFEST.txt: {e}", dir.display()));
    text.lines()
        .find_map(|l| l.strip_prefix("dir=").map(|v| v.to_string()))
        .unwrap_or_else(|| panic!("{} 的 MANIFEST 没有 dir= 行:\n{text}", dir.display()))
}

// ───────────────────────────── 语料 ─────────────────────────────

const MAIN_LZ: &str = r#"import a
import b

def main():
    print(python_a_add(1, 2))
    print(python_b_mul(3, 4))
"#;

const A_LZ: &str = r#"#[embed(python)]
```
def add(a: int, b: int) -> int:
    return a + b
```

def a_sum(x: int, y: int) -> int:
    return x + y
"#;

const B_LZ: &str = r#"#[embed(python)]
```
def mul(a: int, b: int) -> int:
    return a * b
```

def b_prod(x: int, y: int) -> int:
    return x * y
"#;

fn two_module_project(work: &Path) {
    write_src(work, "main.lz", MAIN_LZ);
    write_src(work, "a.lz", A_LZ);
    write_src(work, "b.lz", B_LZ);
}

// ───────────────────────────── ① 产物真的落盘 ─────────────────────────────

/// `lang-zone main.lz --project` 之后 `build/embed/**` 必须存在，且**每个模块一个目录**。
#[test]
fn project_mode_writes_build_embed_per_module() {
    let work = case_dir("project_two_modules");
    two_module_project(&work);

    let out = ok(&work, &["main.lz", "--project"], "project_two_modules");
    let out_text = stdout(&out);

    // 修前的硬症状：整棵 build/embed 不存在
    let root = work.join("build/embed");
    assert!(
        root.is_dir(),
        "project 模式必须落盘 build/embed（修前整棵目录不存在）\nlang-zone stdout:\n{out_text}\nstderr:\n{}",
        stderr(&out)
    );

    // 每个模块一个产物目录：a.lz / b.lz 各一个；入口 main.lz 没有 embed 块 ⇒ 无 main/
    assert_eq!(
        subdir_names(&root),
        vec!["a".to_string(), "b".to_string()],
        "应按模块分目录（a.lz → a/、b.lz → b/），实得: {:?}",
        subdir_names(&root)
    );
    assert!(
        !root.join("main").exists(),
        "入口 main.lz 没有 #[embed] 块 ⇒ 不应凭空生成 main/ 产物目录"
    );

    // 每个模块目录下恰好一个产物目录，里面源码只含自己的方法
    for (stem, own, other) in [("a", "add", "mul"), ("b", "mul", "add")] {
        let mats = subdir_names(&root.join(stem));
        assert_eq!(mats.len(), 1, "{stem}/ 下应有一个产物目录，实得 {mats:?}");
        let dir = root.join(stem).join(&mats[0]);
        assert!(
            dir.join("MANIFEST.txt").is_file(),
            "{} 缺 MANIFEST.txt",
            dir.display()
        );
        let text = std::fs::read_to_string(one_py(&dir)).expect("read py");
        assert!(text.contains(&format!("def {own}(")), "{stem} 缺自己的方法 {own}");
        assert!(
            !text.contains(&format!("def {other}(")),
            "{stem} 串进了另一个模块的方法 {other}"
        );
    }

    // 回执逐条点名（与单文件路径同一份文案）
    let receipts: Vec<&str> = out_text
        .lines()
        .filter(|l| l.contains("embed 产物:"))
        .collect();
    assert_eq!(
        receipts.len(),
        2,
        "两个产物应各回执一行 `embed 产物:`，实得: {receipts:?}\n{out_text}"
    );
    assert!(
        receipts.iter().any(|l| l.contains("build/embed/a/")),
        "回执里应点名 a 的产物目录: {receipts:?}"
    );
}

// ───────────────────────────── ② 垫片 ↔ 盘上目录 ─────────────────────────────

/// 产物里垫片写的路径，与盘上真实目录必须双向一致（不多、不少、不指向空目录）。
#[test]
fn shim_paths_match_dirs_on_disk() {
    let work = case_dir("shim_vs_disk");
    two_module_project(&work);

    ok(&work, &["main.lz", "--project"], "shim_vs_disk");

    let rs_path = work.join("main.rs");
    let rs = std::fs::read_to_string(&rs_path)
        .unwrap_or_else(|e| panic!("缺生成的 Rust {}: {e}", rs_path.display()));

    let shim = shim_dirs_in_rust(&rs);
    assert_eq!(
        shim.len(),
        2,
        "垫片应有两处产物路径（a/b 各一），实得: {shim:?}"
    );

    // ① 垫片指向的每个目录都真的在盘上，且能取到源码/MANIFEST
    for d in &shim {
        let dir = work.join(d);
        assert!(
            dir.is_dir(),
            "垫片路径 {d} 在盘上不存在（正是修前「指向空目录」的失败形态）"
        );
        assert!(dir.join("MANIFEST.txt").is_file(), "{d} 缺 MANIFEST.txt");
        one_py(&dir);
        // 产物自己的身份自证与垫片路径一致
        assert_eq!(manifest_dir(&dir), *d, "MANIFEST 的 dir= 与垫片路径不一致");
    }

    // ② 反方向：盘上没有垫片不知道的产物目录（否则是“落了盘但调不到”）
    assert_eq!(
        artifact_dirs_on_disk(&work),
        shim,
        "盘上产物目录与垫片路径必须双向一致"
    );

    // ③ 垫片包里的包装函数名与产物名对齐（artifact_method），证明这份垫片是从这批产物生成的
    for (artifact, method) in [
        ("python_a", "add"),
        ("python_b", "mul"),
    ] {
        let want = format!("pub fn {}_{}(", artifact, method);
        assert!(
            rs.contains(&want),
            "生成的 Rust 里缺垫片 {want}…—— 垫片与产物不是同一批"
        );
        assert!(
            shim.iter().any(|d| d.ends_with(&format!("/{artifact}"))),
            "垫片里没有产物 {artifact}: {shim:?}"
        );
    }
}

// ───────────────────────────── ③ --embed-build 语义 ─────────────────────────────

/// `--embed-build` 在两个路径上语义一致：不带 ⇒ 只落盘、**不出现“建库”回执**；
/// 带 ⇒ 每个产物一条建库回执（python 是解释型，这一步是 `无该步`，不是“跳过”也不是“完成”）。
/// 且产物集合/内容不随该开关变化 —— 它只多一步建库，不改产物身份。
#[test]
fn embed_build_flag_has_same_semantics_as_single_file() {
    // ── project 路径 ──
    let proj = case_dir("flag_project");
    two_module_project(&proj);

    let without = ok(&proj, &["main.lz", "--project"], "flag_project/no-flag");
    let without_text = stdout(&without);
    assert!(
        !without_text.contains("embed 建库:"),
        "不带 --embed-build 时不许出现建库回执（默认只落盘，不 spawn 外部工具链）:\n{without_text}"
    );
    assert_eq!(
        without_text.lines().filter(|l| l.contains("embed 产物:")).count(),
        2,
        "不带 --embed-build 也必须落盘两个产物:\n{without_text}"
    );
    let files_without = artifact_dirs_on_disk(&proj);

    let with = ok(
        &proj,
        &["main.lz", "--project", "--embed-build"],
        "flag_project/with-flag",
    );
    let with_text = stdout(&with);
    let build_lines: Vec<&str> = with_text
        .lines()
        .filter(|l| l.contains("embed 建库:"))
        .collect();
    assert_eq!(
        build_lines.len(),
        2,
        "带 --embed-build 时每个产物一条建库回执，实得: {build_lines:?}\n{with_text}"
    );
    assert!(
        build_lines.iter().all(|l| l.contains("无该步")),
        "python 是解释型（build=None）⇒ 建库回执必须是 `无该步`，不是跳过也不是完成: {build_lines:?}"
    );
    assert!(
        !stderr(&with).contains("跳过"),
        "python 无建库步 ≠ 工具链缺失跳过（两者不许混为一谈）:\n{}",
        stderr(&with)
    );
    assert_eq!(
        artifact_dirs_on_disk(&proj),
        files_without,
        "--embed-build 只多一步建库，不许改变产物目录集合"
    );

    // ── 单文件路径（同一份 a.lz，独立工作目录）──
    let single = case_dir("flag_single");
    write_src(&single, "a.lz", A_LZ);

    let s_without = ok(&single, &["a.lz"], "flag_single/no-flag");
    let s_without_text = stdout(&s_without);
    assert!(
        !s_without_text.contains("embed 建库:"),
        "单文件路径不带 --embed-build 时同样不许出现建库回执:\n{s_without_text}"
    );
    let s_with = ok(&single, &["a.lz", "--embed-build"], "flag_single/with-flag");
    let s_with_text = stdout(&s_with);
    let s_build: Vec<&str> = s_with_text
        .lines()
        .filter(|l| l.contains("embed 建库:"))
        .collect();
    assert_eq!(s_build.len(), 1, "单文件一个产物 ⇒ 一条建库回执: {s_build:?}");
    assert!(
        s_build[0].contains("无该步"),
        "单文件路径的建库回执口径必须与 project 路径一致（无该步）: {s_build:?}"
    );

    // ── 两条路径对同一份文件给出同一个产物身份（目录 + 回执前缀）──
    assert_eq!(
        artifact_dirs_on_disk(&single),
        vec!["build/embed/a/python_a".to_string()],
        "单文件路径的产物目录"
    );
    assert!(
        artifact_dirs_on_disk(&proj).contains(&"build/embed/a/python_a".to_string()),
        "project 路径里 a.lz 的产物目录必须与单文件路径逐字相同，实得: {:?}",
        artifact_dirs_on_disk(&proj)
    );

    let proj_a_receipt = with_text
        .lines()
        .find(|l| l.contains("embed 产物: build/embed/a/python_a "))
        .unwrap_or_else(|| panic!("project 回执里没有 a 的产物行:\n{with_text}"));
    let single_a_receipt = s_with_text
        .lines()
        .find(|l| l.contains("embed 产物: build/embed/a/python_a "))
        .unwrap_or_else(|| panic!("单文件回执里没有 a 的产物行:\n{s_with_text}"));
    let head = |l: &str| l.split(" kind=").next().unwrap_or(l).to_string();
    assert_eq!(
        head(proj_a_receipt),
        head(single_a_receipt),
        "两条路径对同一份 a.lz 的产物回执前缀必须逐字相同"
    );
}

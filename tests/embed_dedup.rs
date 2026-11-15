// Lang-Zone 编译器 — tests/embed_dedup.rs
// T0r19 / P1 的验收面：同名跨模块产物去重（实现：`src/embed/dedup.rs`）。
//
// 三条判据，每条都配一个「修前必然红」的对照面：
//   ① 两个模块各嵌同语言同名产物 ⇒ 目录不同、互不覆盖
//      （修前 artifacts 都叫 `python_input`、目录都是 `build/embed/input/python_input`，
//        后一次编译把前一次的源码/dispatch/MANIFEST 整目录盖掉 ⇒ 本文件第一条必红）
//   ② 同一模块多块同语言 ⇒ 仍合成一个产物（现行为不变：`build/embed/merge/python_merge`）
//   ③ 同一输入跑两次 ⇒ 产物路径逐字相同（确定性：无随机数、无时间戳、不读磁盘状态）
//
// 判据只认「编译器自己打出来的产物目录」与「盘上的目录/文件」，不靠内部字段自证。

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 每个用例一个干净工作目录（cwd 就是产物根，`build/embed` 落在它下面）
fn case_dir(name: &str) -> PathBuf {
    let d = repo_root().join("target/embed_dedup").join(name);
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

/// 跑一次编译器（cwd 固定在工作目录），返回 stdout + stderr 便于失败时点名
fn run_lz(work: &Path, arg: &Path) -> String {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .current_dir(work)
        .arg(arg)
        .output()
        .expect("run lang-zone");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "编译失败（arg={}）: stdout={stdout} stderr={stderr}",
        arg.display()
    );
    stdout
}

/// 从回执里摘产物目录：`embed 产物: build/embed/<主干>/<产物名> kind=...`
fn artifact_dirs(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix("embed 产物: "))
        .map(|rest| {
            rest.split(" kind=")
                .next()
                .unwrap_or(rest)
                .trim()
                .to_string()
        })
        .collect()
}

/// 目录里唯一的 `.py` 源码（产物名带后缀，文件名不再写死）
fn only_py(dir: &Path) -> String {
    let mut hits: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("产物目录不存在 {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .filter(|p| !p.file_name().unwrap().to_string_lossy().starts_with("__lz_dispatch"))
        .collect();
    assert_eq!(hits.len(), 1, "产物目录应有且仅有一份摘出的源码: {:?}", hits);
    std::fs::read_to_string(hits.remove(0)).expect("read py")
}

fn manifest(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("MANIFEST.txt")).expect("manifest")
}

const ADD_SRC: &str = "// a/input.lz：内嵌 python 库（同名产物组的第一个模块）\n#[embed(python)]\n```\ndef add(a: int, b: int) -> int:\n    return a + b\n```\n\ndef main():\n    print(1)\n";

const MUL_SRC: &str = "// b/input.lz：内嵌 python 库（与 a/input.lz 同名、不同模块）\n#[embed(python)]\n```\ndef mul(a: int, b: int) -> int:\n    return a * b\n```\n\ndef main():\n    print(1)\n";

// ─────────────────── ① 同名跨模块：目录不同、互不覆盖 ───────────────────

#[test]
fn same_named_artifacts_from_two_modules_do_not_overwrite_each_other() {
    let work = case_dir("two_modules");
    write_src(&work, "alpha/input.lz", ADD_SRC);
    write_src(&work, "beta/input.lz", MUL_SRC);

    let da = artifact_dirs(&run_lz(&work, Path::new("alpha/input.lz")));
    let db = artifact_dirs(&run_lz(&work, Path::new("beta/input.lz")));
    assert_eq!(da.len(), 1, "一个模块只该出一个 python 产物: {da:?}");
    assert_eq!(db.len(), 1, "一个模块只该出一个 python 产物: {db:?}");

    // 撞车点：修前这两条都等于 build/embed/input/python_input（后者覆盖前者）
    assert_ne!(
        da[0], db[0],
        "不同模块的同名产物必须落在不同目录，实际都是 {}",
        da[0]
    );
    for d in [&da[0], &db[0]] {
        assert!(
            d.starts_with("build/embed/input/"),
            "产物目录形状仍是 build/embed/<模块主干>/<产物名>: {d}"
        );
    }

    let (pa, pb) = (work.join(&da[0]), work.join(&db[0]));
    assert!(pa.is_dir() && pb.is_dir(), "两个产物目录都必须真实存在");

    // 互不覆盖：各自目录里仍是自己的那份源码，谁的 def 也没被对方顶掉
    let src_a = only_py(&pa);
    let src_b = only_py(&pb);
    assert!(src_a.contains("def add("), "alpha 的源码被覆盖了: {src_a}");
    assert!(!src_a.contains("def mul("), "alpha 目录里混进了 beta 的源码: {src_a}");
    assert!(src_b.contains("def mul("), "beta 的源码被覆盖了: {src_b}");
    assert!(!src_b.contains("def add("), "beta 目录里混进了 alpha 的源码: {src_b}");

    // MANIFEST 是逐产物写的：两个目录的 artifact 名必须不同，否则「目录不同」只是运气
    let (ma, mb) = (manifest(&pa), manifest(&pb));
    let art_a = ma.lines().find(|l| l.starts_with("artifact=")).unwrap();
    let art_b = mb.lines().find(|l| l.starts_with("artifact=")).unwrap();
    assert_ne!(art_a, art_b, "两个产物不能共用一个 artifact 名");
    assert!(ma.contains("dir="), "MANIFEST 仍带确定性目录: {ma}");
    assert!(ma.contains(&format!("dir={}", da[0])), "{ma}");
    assert!(mb.contains(&format!("dir={}", db[0])), "{mb}");

    // 消歧名的形状：`python_input__<目录slug>_<路径短哈希>`，可做 Python 模块名/Rust 标识符
    for art in [art_a, art_b] {
        let name = art.trim_start_matches("artifact=");
        assert!(
            name.starts_with("python_input__"),
            "消歧名应以 `<语言>_<主干>__` 开头: {name}"
        );
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "产物名要能进 Python `import_module` / Rust 标识符: {name}"
        );
    }
}

// ─────────────────── ② 同模块多块同语言：仍合一个产物 ───────────────────

#[test]
fn multiple_blocks_of_one_module_still_merge_into_a_single_artifact() {
    let work = case_dir("merge");
    let src = "// merge.lz：同一模块内两块 python（必须仍按空行拼接成一个产物）\n#[embed(python)]\n```\ndef a(n: int) -> int:\n    return n\n```\n\n#[embed(python)]\n```\ndef b(n: int) -> int:\n    return n + 1\n```\n\ndef main():\n    print(1)\n";
    write_src(&work, "merge.lz", src);

    let dirs = artifact_dirs(&run_lz(&work, Path::new("merge.lz")));
    assert_eq!(dirs.len(), 1, "同语言多块必须只出一个产物: {dirs:?}");
    // 模块就在产物根下 ⇒ 现行为一字不变（r15 的 multiple_blocks_of_same_lang_merge_into_one_artifact 同口径）
    assert_eq!(dirs[0], "build/embed/merge/python_merge");

    let dir = work.join(&dirs[0]);
    let py = only_py(&dir);
    assert!(py.contains("def a(") && py.contains("def b("), "两块必须合并进同一份源码: {py}");
    assert!(manifest(&dir).contains("artifact=python_merge"));

    // 聚合面：`build/embed/merge/` 下只有一个产物目录（多块没有各出一个）
    let sibs: Vec<String> = std::fs::read_dir(work.join("build/embed/merge"))
        .expect("read merge dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(sibs.len(), 1, "同语言多块必须聚合成一个产物目录: {sibs:?}");
}

// ─────────────────── ③ 同一输入两次运行：路径逐字一致 ───────────────────

#[test]
fn same_input_twice_yields_byte_identical_paths() {
    let work = case_dir("determinism");
    write_src(&work, "pkg/input.lz", ADD_SRC);

    let first = artifact_dirs(&run_lz(&work, Path::new("pkg/input.lz")));
    let second = artifact_dirs(&run_lz(&work, Path::new("pkg/input.lz")));
    assert_eq!(first.len(), 1);
    assert_eq!(
        first, second,
        "同一输入两次运行必须给出逐字相同的产物路径（不许有随机数/时间戳）"
    );

    // 第二次没有另起新目录：`build/embed/<模块主干>` 下仍只有一个产物目录
    let sibs: Vec<String> = std::fs::read_dir(work.join("build/embed/input"))
        .expect("read artifact parent dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(sibs.len(), 1, "复跑不许再长出一个新产物目录: {sibs:?}");
    assert_eq!(sibs[0], first[0].rsplit('/').next().unwrap());

    // 同一文件的两种写法（相对 / 绝对）必须拿到同一个身份 ⇒ 同一个目录
    let absolute = work.join("pkg/input.lz");
    let third = artifact_dirs(&run_lz(&work, &absolute));
    assert_eq!(
        first, third,
        "相对与绝对两种 argv 表述必须落到同一产物目录（身份=相对产物根的模块路径）"
    );

    // 单元面复核：`resolve` 是纯函数，两次调用逐字相同；且 `dir_rel` 只由它派生
    let t1 = lang_zone::embed::resolve("python", "pkg/input.lz", ADD_SRC).unwrap();
    let t2 = lang_zone::embed::resolve("python", "pkg/input.lz", ADD_SRC).unwrap();
    assert_eq!(t1.artifact, t2.artifact);
    assert_eq!(
        lang_zone::embed::artifact::dir_rel("input", &t1.artifact),
        lang_zone::embed::artifact::dir_rel("input", &t2.artifact)
    );
    assert_eq!(
        lang_zone::embed::artifact::dir_rel("input", &t1.artifact),
        first[0],
        "回执里的目录必须与 resolve 派生的目录一致"
    );
}

// ─────────────────── 兼容面：产物根下的模块命名不许动 ───────────────────

#[test]
fn root_level_module_keeps_legacy_artifact_name() {
    // r15 的 20 条门禁把 `python_calc` / `build/embed/calc/python_calc` 钉成了兼容面
    let t = lang_zone::embed::resolve(
        "python",
        "calc.lz",
        "def add(a: int, b: int) -> int:\n    return a + b\n",
    )
    .unwrap();
    assert_eq!(t.artifact, "python_calc");
    assert_eq!(
        lang_zone::embed::artifact::dir_rel("calc", &t.artifact),
        "build/embed/calc/python_calc"
    );
    // 只有「同名但不同模块」才消歧：主干不同 ⇒ 不靠后缀也不撞车
    let other = lang_zone::embed::resolve("python", "other.lz", "def f() -> int:\n    return 1\n").unwrap();
    assert_eq!(other.artifact, "python_other");
}

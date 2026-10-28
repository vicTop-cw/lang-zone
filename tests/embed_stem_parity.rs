// Lang-Zone 编译器 — tests/embed_stem_parity.rs
// fist-mbt T0r23（命名空间 lang-zone）验收面：**同一份**含 `#[embed(python)]` 的源文件，
// 在两条路径下的 Cli 档产物名是否一致。
//
//   ① 原生 Rust 后端（默认路径）：`lzc calc.lz`
//   ② rs-lz 旁路（自举路线 B）：`lzc calc.lz --emit=rs-lz`
//
// 先记结论（本文件的三条判据就是证据，别只看注释）：
//
//   原生：产物名 python_calc、目录 build/embed/calc/python_calc、垫片 pub fn python_calc_add
//         —— 注入点在 src/main.rs:1182 `cg.set_embed_stem(&path.to_string())`；
//            消费点在 src/ir/codegen/mod.rs:697-727（`cli_source_name` → `base_stem`
//            → `crate::embed::resolve` → `artifact::dir_rel`）；
//            命名权威在 src/embed/dedup.rs::artifact_name。
//   旁路：**零产物、零命名**。`--emit=rs-lz` 只把 embed 块序列化成
//         `Item.EmbedBlock(lang, src_len)`（src/ir/lz_codegen.rs:403-404）——
//         连内嵌源码文本都不进数据面；而 LZ 发射器的 `Item` 枚举
//         （src/ir/lz_codegen_lib.lz:154-179）根本没有这个变体，
//         于是含 embed 的源文件在旁路上是**硬编译失败**（rustc E0599），
//         不是「按 module.name 命名」这种程度的分歧。
//
// 也就是说 docs/嵌入式统一抽象-实现-2026-10-21.md §6 第 5 条
// （「`embed_stem` 只在默认 rust 后端注入：旁路仍按 `module.name` 命名产物」）
// **低估**了缺口：旁路既没有 `embed_stem`，也没有任何命名代码，连 embed 块都表示不出来。
//
// 三条判据的分工：
//   native_path_pins_...            绿 —— 钉住基准面（原生侧的产物名/目录/垫片名）
//   rs_lz_bypass_still_cannot_...   绿 —— 栅栏：钉住「旁路目前零命名」这一事实，
//                                         它一变红就说明旁路接上了，届时把下面的
//                                         parity 判据转正、并删掉本栅栏
//   same_source_yields_identical_...  ignored —— 接线后必须成立的目标判据；
//                                         `cargo test --test embed_stem_parity -- --ignored`
//                                         能直接看到今天的确切失败点

use std::path::{Path, PathBuf};
use std::process::Command;

// ───────────────────────────── 用例源文件 ─────────────────────────────

/// 两条路径共用的同一份源文件：Cli 档（python 库 + 垫片调用）+ Source 档（tnr，兼容面）。
/// python 块无 `main` ⇒ 库产物；调用走 `<artifact>_<fn>` 垫片。
const EMBED_SRC: &str = r#"// calc.lz：同一份源文件，两条路径都编译它
#[embed(tnr)]
```
fn helper(a, b) { a + b }
```

#[embed(python)]
```
def add(a: int, b: int) -> int:
    return a + b
```

def main():
    print(python_calc_add(2, 3))
    print(helper(2, 3))
"#;

// ───────────────────────────── 通用夹具 ─────────────────────────────

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 每个用例一个干净工作目录（cwd 就是产物根，`build/embed` 落在它下面）
fn case_dir(name: &str) -> PathBuf {
    let d = repo_root().join("target/embed_stem_parity").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create case dir");
    d
}

struct Ran {
    ok: bool,
    stdout: String,
    stderr: String,
}

impl Ran {
    fn receipts(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

fn run_lz(work: &Path, args: &[&str]) -> Ran {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin)
        .current_dir(work)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("无法启动编译器 {bin:?}（args={args:?}）: {e}"));
    Ran {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
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

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("读不到 {}: {e}", p.display()))
}

/// `--emit=rs-lz` 递归链里 rustc 那一步写死了 `<repo>/target/debug/liblz_builtins.rlib`
/// （src/main.rs:1106）。本车道跑测试时 `CARGO_TARGET_DIR=target/lane-stem`，
/// 一份干净树上那个位置可能还没有产物 ⇒ 缺件时按 main.rs 期望的位置补建一次，
/// 免得旁路因为「找不到 crate」而不是因为「不认识 EmbedBlock」变红。
fn ensure_rs_lz_builtins_rlib() -> PathBuf {
    let want = repo_root().join("target/debug/liblz_builtins.rlib");
    if want.exists() {
        return want;
    }
    let _ = std::fs::create_dir_all(repo_root().join("target/debug"));
    // 显式 --target-dir：CARGO_TARGET_DIR 会覆盖默认落点，必须压回去
    let built = Command::new("cargo")
        .args(["build", "-p", "lz_builtins", "--target-dir"])
        .arg(repo_root().join("target"))
        .current_dir(repo_root())
        .output();
    if let Ok(b) = &built {
        assert!(
            b.status.success(),
            "lz_builtins 补建失败：{}",
            String::from_utf8_lossy(&b.stderr)
        );
    }
    assert!(
        want.exists(),
        "已尝试把 lz_builtins 建到 {:?} 仍缺件 ⇒ rs-lz 链的取数位置变了，改 main.rs:1106 与本行而非放宽判据",
        want
    );
    want
}

// ───────────── ① 基准面：原生路径的产物名跟随源文件主干 ─────────────

#[test]
fn native_path_pins_artifact_name_dir_and_shim() {
    let work = case_dir("native");
    std::fs::write(work.join("calc.lz"), EMBED_SRC).expect("write lz");

    let ran = run_lz(&work, &["calc.lz"]);
    assert!(
        ran.ok,
        "原生路径编译失败: stdout={} stderr={}",
        ran.stdout, ran.stderr
    );

    // 产物名/目录：`crate::embed::dedup::artifact_name` + `artifact::dir_rel` 的现行为
    let dirs = artifact_dirs(&ran.stdout);
    assert_eq!(
        dirs,
        vec!["build/embed/calc/python_calc".to_string()],
        "原生路径的 Cli 档产物目录（calc.lz ⇒ 主干 calc）变了: {}",
        ran.receipts()
    );
    assert!(
        work.join(&dirs[0]).is_dir(),
        "回执点了名但目录不在盘上: {}",
        work.join(&dirs[0]).display()
    );

    // 调用面：垫片名同样带源文件主干
    let rs = read(&work.join("calc.rs"));
    assert!(
        rs.contains("pub fn python_calc_add("),
        "原生路径的垫片名必须带主干 calc：\n{}",
        rs
    );
    assert!(
        !rs.contains("python_main_add"),
        "原生路径不许退回按 module.name（main）命名"
    );
    // Source 档（tnr）保持逐字，不参与产物命名
    assert!(rs.contains("helper"), "Source 档文本必须仍留在产物里");
}

// ───────── ② 栅栏：旁路今天既不能命名、也不能落盘任何产物 ─────────

#[test]
fn rs_lz_bypass_still_cannot_represent_embed_blocks() {
    ensure_rs_lz_builtins_rlib();

    let work = case_dir("rs_lz");
    std::fs::write(work.join("calc.lz"), EMBED_SRC).expect("write lz");

    let ran = run_lz(&work, &["calc.lz", "--emit=rs-lz"]);
    assert!(
        !ran.ok,
        "旁路处理含 embed 的源文件居然成功了 ⇒ 说明已接上 embed 支持：\n\
         请把 same_source_yields_identical_artifact_names_on_both_paths 的 #[ignore] 摘掉、\n\
         让它承认真实的一致性判据，然后删除本栅栏判据。\nstdout={} stderr={}",
        ran.stdout, ran.stderr
    );

    // 失败点必须是「LZ 发射器的 Item 枚举不认识 EmbedBlock」，不是缺 rlib / 别的噪声
    assert!(
        ran.stderr.contains("EmbedBlock") && ran.stderr.contains("no variant"),
        "旁路的失败原因不是预期的 E0599（Item 枚举无 EmbedBlock），需要重新定位:\n{}",
        ran.receipts()
    );

    // 数据面证据：旁路只带 (lang, src_len)，内嵌源码文本根本没进 IR 数据
    let lzrs = read(&work.join("calc.lzrs"));
    assert!(
        lzrs.contains("Item.EmbedBlock(lang: \"python\", src_len: "),
        "旁路序列化 embed 块的形式变了（src/ir/lz_codegen.rs:403），需重新核对:\n{}",
        grep_lines(&lzrs, "EmbedBlock")
    );
    assert!(
        !lzrs.contains("build/embed"),
        "旁路的数据面里不该出现产物目录（它今天不落盘任何 embed 产物）"
    );

    // 命名面：既没有 calc 版垫片，也没有 main 版垫片 ⇒ 旁路**零命名**，
    // 比文档 §6 第 5 条说的「按 module.name 命名」还要靠前一步。
    for shim in ["pub fn python_calc_add", "pub fn python_main_add"] {
        assert!(
            !lzrs.contains(shim),
            "旁路出现了垫片定义 `{shim}` ⇒ 命名面已被接上，需同步更新本判据与文档 §6"
        );
    }

    // 落盘面：原生路径有 build/embed/calc/python_calc，旁路一份都没有
    assert!(
        !work.join("build/embed").exists(),
        "旁路不该落盘 embed 产物; 实际存在 {}",
        work.join("build/embed").display()
    );
}

/// 便于失败时点名：抽出含关键字的行（最多 5 行）
fn grep_lines(text: &str, needle: &str) -> String {
    text.lines()
        .filter(|l| l.contains(needle))
        .take(5)
        .collect::<Vec<_>>()
        .join("\n")
}

// ───────── ③ 目标判据：两条路径产物名/目录逐字一致（接线后转正） ─────────

#[test]
#[ignore = "rs-lz 旁路尚不能表示 embed 块（rustc E0599）⇒ 一致性今天不成立；\
            接线后摘掉本 ignore，它就是验收判据本身。用 `cargo test --test embed_stem_parity -- --ignored` 可看到确切失败点"]
fn same_source_yields_identical_artifact_names_on_both_paths() {
    ensure_rs_lz_builtins_rlib();

    let native = case_dir("parity_native");
    std::fs::write(native.join("calc.lz"), EMBED_SRC).expect("write lz");
    let n = run_lz(&native, &["calc.lz"]);
    assert!(
        n.ok,
        "原生路径编译失败: stdout={} stderr={}",
        n.stdout, n.stderr
    );
    let native_dirs = artifact_dirs(&n.stdout);
    assert_eq!(native_dirs.len(), 1, "原生侧应恰好一个 Cli 产物: {native_dirs:?}");

    let bypass = case_dir("parity_rs_lz");
    std::fs::write(bypass.join("calc.lz"), EMBED_SRC).expect("write lz");
    let b = run_lz(&bypass, &["calc.lz", "--emit=rs-lz"]);
    assert!(
        b.ok,
        "旁路必须能处理同一份源文件（今天死在 E0599：Item 枚举无 EmbedBlock）:\nstdout={}\nstderr={}",
        b.stdout, b.stderr
    );
    let bypass_dirs = artifact_dirs(&b.stdout);

    // 一致性本体：产物目录（含产物名）逐字相同
    assert_eq!(
        native_dirs, bypass_dirs,
        "两条路径的 Cli 档产物目录必须逐字一致（原生 {:?} / 旁路 {:?}）",
        native_dirs, bypass_dirs
    );

    // 两侧都应该在盘上真的落了这个目录
    let dir = &native_dirs[0];
    assert!(native.join(dir).is_dir(), "原生侧缺产物目录");
    assert!(bypass.join(dir).is_dir(), "旁路侧缺产物目录");
}

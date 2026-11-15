// Lang-Zone 编译器 — tests/embed_stem_parity.rs
// fist-mbt T0r23（命名空间 lang-zone）验收面：**同一份**含 `#[embed(python)]` 的源文件，
// 在两条路径下的 Cli 档产物名是否一致。
//
//   ① 原生 Rust 后端（默认路径）：`lzc calc.lz`
//   ② rs-lz 旁路（自举路线 B）：`lzc calc.lz --emit=rs-lz`
//
// 先记结论（本文件的判据就是证据，别只看注释）：
//
//   原生：产物名 python_calc、目录 build/embed/calc/python_calc、垫片 pub fn python_calc_add
//         —— 注入点在 src/main.rs:1339 `cg.set_embed_stem(&path.to_string())`；
//            消费点在 src/ir/codegen/mod.rs:694-727（`cli_source_name` → `base_stem`
//            → `crate::embed::resolve` → `artifact::dir_rel`）；
//            命名权威在 src/embed/dedup.rs::artifact_name。
//   旁路（T0r24 之后）：**同一份结论**。`src/ir/lz_codegen.rs` 在序列化阶段就用
//         `crate::embed::{resolve, artifact::dir_rel, shim::module_text}` 把产物名、
//         产物目录、垫片模块文本算好，作为字符串参数穿透进 LZ 数据面
//         （`Item.EmbedBlock(lang, src, tier, artifact, dir_rel, wrappers)`），
//         LZ 发射器（src/ir/lz_codegen_lib.lz）只消费、不重写 —— FNV 去重不出现在 LZ 里。
//         产物落盘由 Rust 侧 `emit_embed_artifacts` 负责（LZ 程序不写文件）。
//
// T0r24 的栅栏转正记录：本文件原有第三条判据
// `rs_lz_bypass_still_cannot_represent_embed_blocks`（断言旁路必须死在 E0599、
// 且 .lzrs 里只有 `src_len` 没有源码文本）已按它自己的注释所写的条件**删除** ——
// 旁路已能表示 embed 块，那条栅栏钉住的事实不再成立。它留下的证据面由
// `rs_lz_bypass_represents_embed_blocks_with_source_and_artifact_name` 反向钉死
// （数据面带源码文本 + 产物名与原生基准一致），本文件的 parity 判据同时转正。
//
// 三条判据的分工：
//   native_path_pins_...             绿 —— 钉住基准面（原生侧的产物名/目录/垫片名）
//   rs_lz_bypass_represents_...      绿 —— 钉住「旁路已接上」的数据面证据（不是零命名）
//   same_source_yields_identical_... 绿 —— 目标判据：两条路径产物名/目录逐字一致

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

// ───────── ② 旁路已接上：数据面带源码文本 + 产物名与原生基准一致 ─────────

/// T0r24 转正后的判据（取代旧栅栏 `rs_lz_bypass_still_cannot_represent_embed_blocks`）。
///
/// 钉两件事，缺一不可：
///   1. **数据面**：`.lzrs` 里的 `Item.EmbedBlock` 必须带内嵌源码原文（旧形态只有
///      `src_len`，源码根本不进数据 —— 那是 rs-lz 表示不了 embed 的根因），且产物名/
///      目录来自 Rust 侧的 `embed::resolve` / `artifact::dir_rel`（LZ 不重写去重）。
///   2. **调用面 + 落盘面**：生成的 Rust 里有 `pub mod __lz_embed` 与带主干的垫片
///      `python_calc_add`，产物目录真的落在 build/embed/calc/python_calc。
#[test]
fn rs_lz_bypass_represents_embed_blocks_with_source_and_artifact_name() {
    ensure_rs_lz_builtins_rlib();

    let work = case_dir("rs_lz");
    std::fs::write(work.join("calc.lz"), EMBED_SRC).expect("write lz");

    let ran = run_lz(&work, &["calc.lz", "--emit=rs-lz"]);
    assert!(
        ran.ok,
        "旁路必须能处理含 embed 的源文件（T0r24 前死在 E0599：Item 枚举无 EmbedBlock）:\nstdout={}\nstderr={}",
        ran.stdout, ran.stderr
    );

    // ── 数据面：源码原文进数据，不再只有长度 ──
    let lzrs = read(&work.join("calc.lzrs"));
    let embed_line = grep_lines(&lzrs, "Item.EmbedBlock(");
    assert!(
        !embed_line.is_empty(),
        "旁路 .lzrs 里没有 Item.EmbedBlock 构造点:\n{}",
        lzrs
    );
    assert!(
        !lzrs.contains("src_len: "),
        "旁路仍按旧形态序列化（只有 src_len，源码丢了）:\n{embed_line}"
    );
    assert!(
        lzrs.contains("Item.EmbedBlock(lang: \"python\", src: \"") && lzrs.contains("return a + b"),
        "Cli 档 embed 块的源码原文必须进数据面:\n{embed_line}"
    );
    // 产物名/目录是 Rust 侧算好后作为字符串穿透的（LZ 里没有 FNV 实现）
    assert!(
        lzrs.contains("artifact: \"python_calc\"")
            && lzrs.contains("dir_rel: \"build/embed/calc/python_calc\""),
        "旁路的产物名/目录必须与原生基准（python_calc / build/embed/calc/python_calc）一致:\n{embed_line}"
    );
    assert!(
        !lzrs.contains("0xcbf2_9ce4_8422_2325"),
        "去重/短哈希不该被搬到 LZ 侧重写（命名权威只有 embed::dedup::artifact_name）"
    );

    // ── 调用面：垫片模块 + 带主干的垫片名 ──
    assert!(
        ran.stdout.contains("pub mod __lz_embed {"),
        "旁路产物缺 __lz_embed 模块:\n{}",
        ran.stdout
    );
    assert!(
        ran.stdout.contains("pub fn python_calc_add("),
        "旁路的垫片名必须带源文件主干 calc（不是 module.name main）:\n{}",
        grep_lines(&ran.stdout, "_add(")
    );
    assert!(
        !ran.stdout.contains("python_main_add"),
        "旁路不许退回按 module.name（main）命名"
    );
    // 垫片与外部函数桩不能同名共存（原生 unknown_extern_fns.remove 的同口径）
    assert!(
        !ran.stdout.contains("fn python_calc_add(__a0"),
        "旁路既发垫片又发同名 i64 桩 ⇒ E0428 重定义:\n{}",
        grep_lines(&ran.stdout, "python_calc_add")
    );

    // ── 落盘面：产物目录由 Rust 侧真落在盘上（LZ 程序不写文件）──
    let dir = work.join("build/embed/calc/python_calc");
    assert!(dir.is_dir(), "旁路缺产物目录 {}", dir.display());
    let manifest = read(&dir.join("MANIFEST.txt"));
    assert!(
        manifest.contains("artifact=python_calc") && manifest.contains("dir=build/embed/calc/python_calc"),
        "旁路 MANIFEST 与原生不一致:\n{manifest}"
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

// ───────── ③ 目标判据：两条路径产物名/目录逐字一致（T0r24 起转正） ─────────

#[test]
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
        "旁路必须能处理同一份源文件:\nstdout={}\nstderr={}",
        b.stdout, b.stderr
    );
    // 回执读 receipts（stdout+stderr）：`--emit=rs-lz` 的 **stdout 是产物本身**
    // （生成的 Rust 源码，见 tests/rs_lz_behdiff.rs 的契约），所以 embed 落盘回执
    // 只能走 stderr —— 判据因此按「两股流合起来」摘行，一致性要求本身不变。
    let bypass_dirs = artifact_dirs(&b.receipts());
    assert_eq!(
        bypass_dirs.len(),
        1,
        "旁路侧应恰好一个 Cli 产物（回执可能落在 stderr）: {}",
        b.receipts()
    );

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

    // 调用面逐字一致：两边同一份垫片签名 + 同一份产物目录串
    let native_rs = read(&native.join("calc.rs"));
    let shim_native = grep_lines(&native_rs, "pub fn python_calc_add(");
    let shim_bypass = grep_lines(&b.stdout, "pub fn python_calc_add(");
    assert!(
        !shim_native.is_empty() && shim_native == shim_bypass,
        "两条路径的垫片签名必须逐字一致:\n原生={shim_native}\n旁路={shim_bypass}"
    );
    assert!(
        native_rs.contains(dir) && b.stdout.contains(dir),
        "两条路径的产物目录串必须都出现在调用面里"
    );
}

// Lang-Zone 编译器 — tests/embed_project_grouping.rs
// fist-mbt T0r29（命名空间 lang-zone）验收面：**`--project` 模式下 embed 产物按
// (模块, lang) 分组**，而不是把所有模块的块按 lang 拼成一份。
//
// 修前（`src/ir/codegen/mod.rs` 的 by_lang 聚合）：
//   `main.lz` 里 `import a` + `import b`，a/b 各含一段 `#[embed(python)]` ⇒
//   只出**一个**产物 `python_main`（入口主干），把 a 的 `add` 与 b 的 `mul` 合进同一份，
//   目录也只剩 `build/embed/main/python_main`。
// 修后：分组键 `(origin, lang)`，origin 由 `ProjectCompiler::merge_modules` 按 unit.path
//   打进 `ast::Stmt::EmbedBlock` → `ir::Item::EmbedBlock`（单文件路径恒 None）⇒
//   a/b 各自出一份独立产物，命名仍走 `embed::resolve` / `dedup::artifact_name`
//   （不在 codegen 里另写命名规则）。
//
// 判据只读「codegen 吐出来的产物清单 + 盘上落下来的目录/文件」，不靠内部字段自证：
//   ① 两模块同语言 → 两个独立产物、方法面互不串、目录互不覆盖
//   ② 单模块多块同语言 → 仍合并成一份（现行为不变）
//   ③ 产物名与单文件路径同口径（同一文件，project 模式 ≡ 单文件模式）
//   ④ 同一工程再编译一次 → 产物名/目录逐字相同（确定性：无随机数/无时间戳）

use std::path::{Path, PathBuf};

use lang_zone::embed::EmbedTarget;
use lang_zone::ir::builder::build_ir;
use lang_zone::ir::codegen::CodeGen;
use lang_zone::project::ProjectCompiler;

// ───────────────────────────── 通用夹具 ─────────────────────────────

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 每个用例一个干净工作目录（cwd 保持 cargo 给的仓根 ⇒ `dedup::module_key`
/// 的产物根就是仓根，与 `lz <相对路径>` 同一把尺）
fn case_dir(name: &str) -> PathBuf {
    let d = repo_root().join("target/embed_project_grouping").join(name);
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

/// 走项目模式前端（与 `lang-zone x.lz --project` / `lz build` 同一条链：
/// ProjectCompiler → 合并 AST → build_ir → CodeGen），返回 (产物清单, 生成的 Rust)。
fn compile_project(work: &Path, entry_rel: &str) -> (Vec<EmbedTarget>, String) {
    let entry = work.join(entry_rel);
    assert!(entry.is_file(), "入口不存在: {}", entry.display());
    let mut pc = ProjectCompiler::new(work.to_path_buf(), None);
    let mut merged = pc.compile(&entry).expect("project compile");
    // 与 src/main.rs:861 / src/cli.rs:314 同口径：合并模块需要入口文件路径
    if merged.file_path.is_none() {
        merged.file_path = Some(entry.display().to_string());
    }
    let ir = build_ir(&merged).expect("project IR build");
    let mut cg = CodeGen::new();
    let rust = cg.generate(&ir);
    (cg.embed_targets().to_vec(), rust)
}

/// 单文件模式前端（与 `lang-zone calc.lz` 同一条链：parser → build_ir →
/// `CodeGen::set_embed_stem` → generate），产物清单口径与项目模式可比。
fn compile_single_file(path: &Path) -> (Vec<EmbedTarget>, String) {
    let src = std::fs::read_to_string(path).expect("read lz");
    let tokens = lang_zone::lexer::Lexer::new(&src).tokenize();
    let mut module = lang_zone::parser::Parser::new(tokens)
        .parse_module()
        .expect("parse lz");
    module.file_path = Some(path.display().to_string());
    module.source_text = Some(src);
    let ir = build_ir(&module).expect("single-file IR build");
    let mut cg = CodeGen::new();
    cg.set_embed_stem(&path.display().to_string());
    let rust = cg.generate(&ir);
    (cg.embed_targets().to_vec(), rust)
}

fn methods(t: &EmbedTarget) -> Vec<String> {
    let mut v: Vec<String> = t.methods.iter().map(|m| m.name.clone()).collect();
    v.sort();
    v
}

fn artifacts(ts: &[EmbedTarget]) -> Vec<String> {
    let mut v: Vec<String> = ts.iter().map(|t| t.artifact.clone()).collect();
    v.sort();
    v
}

// ───────────────────────────── 语料 ─────────────────────────────

const MAIN_TWO_MODULES: &str = r#"import a
import b

def main():
    print(1)
"#;

const A_PY: &str = r#"#[embed(python)]
```
def add(a: int, b: int) -> int:
    return a + b
```

def a_sum(x: int, y: int) -> int:
    return x + y
"#;

const B_PY: &str = r#"#[embed(python)]
```
def mul(a: int, b: int) -> int:
    return a * b
```

def b_prod(x: int, y: int) -> int:
    return x * y
"#;

/// 单模块两段同语言块：修前修后都应合并成一份（`\n\n` 拼接，与 EmbedCollector 同口径）
const ONE_MODULE_TWO_BLOCKS: &str = r#"#[embed(python)]
```
def one(a: int) -> int:
    return a + 1
```

#[embed(python)]
```
def two(a: int) -> int:
    return a + 2
```

def main():
    print(1)
"#;

// ───────────────────────────── 判据 ─────────────────────────────

/// ① 两模块同语言 ⇒ 两个独立产物：产物名不同、方法面各归各、落盘目录互不覆盖。
#[test]
fn two_modules_same_lang_yield_two_independent_artifacts() {
    let work = case_dir("two_modules");
    write_src(&work, "main.lz", MAIN_TWO_MODULES);
    write_src(&work, "a.lz", A_PY);
    write_src(&work, "b.lz", B_PY);

    let (targets, rust) = compile_project(&work, "main.lz");

    assert_eq!(
        targets.len(),
        2,
        "两个模块各一段 python ⇒ 应有 2 个产物（修前是 1 个 python_main），实得: {:?}",
        artifacts(&targets)
    );

    // 产物名不同（修前两者都叫 python_main）
    let names = artifacts(&targets);
    assert_ne!(names[0], names[1], "两模块产物名不许相同: {:?}", names);

    // 方法面各归各：a 只有 add，b 只有 mul；没有任何一个产物同时含 add 与 mul
    let mut method_sets: Vec<Vec<String>> = targets.iter().map(methods).collect();
    method_sets.sort();
    assert_eq!(
        method_sets,
        vec![vec!["add".to_string()], vec!["mul".to_string()]],
        "每个产物只该带自己模块的方法（修前是同一份里 add+mul 都有）"
    );

    // 落盘目录不同 ⇒ 不会互相覆盖
    let dirs: Vec<&str> = targets.iter().map(|t| t.dir_rel.as_str()).collect();
    assert_ne!(dirs[0], dirs[1], "两个产物目录不许相同: {:?}", dirs);

    // 真落盘：两棵目录各自的源码里只有自己的函数，且都没有对方的函数
    for t in &targets {
        lang_zone::embed::artifact::write_artifacts(&work, t).expect("write artifact");
        let dir = work.join(&t.dir_rel);
        let py: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("产物目录不存在 {}: {e}", dir.display()))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "py"))
            .filter(|p| !p.file_name().unwrap().to_string_lossy().starts_with("__lz_dispatch"))
            .collect();
        assert_eq!(py.len(), 1, "产物目录应有且仅有一份摘出的源码: {:?}", py);
        let text = std::fs::read_to_string(&py[0]).expect("read py");
        let own = if t.artifact.contains("_a") { "add" } else { "mul" };
        let other = if own == "add" { "mul" } else { "add" };
        assert!(text.contains(&format!("def {}(", own)), "{} 缺自己的方法", t.artifact);
        assert!(
            !text.contains(&format!("def {}(", other)),
            "{} 串进了另一个模块的方法（正是修前的合并症状）",
            t.artifact
        );
    }

    // LZ 侧垫片名跟着产物名走：两个名字都在生成的 Rust 里
    for t in &targets {
        let m = methods(t).remove(0);
        assert!(
            rust.contains(&format!("{}_{}", t.artifact, m)),
            "生成的 Rust 里缺垫片 {}_{}",
            t.artifact,
            m
        );
    }
}

/// ② 单模块多块同语言 ⇒ 仍合并成一份（不因为引入模块维就拆成两份）。
#[test]
fn one_module_with_two_blocks_same_lang_still_merges() {
    let work = case_dir("merge_one_module");
    write_src(&work, "main.lz", ONE_MODULE_TWO_BLOCKS);

    let (targets, _rust) = compile_project(&work, "main.lz");

    assert_eq!(
        targets.len(),
        1,
        "同一模块两段 python 仍应只有 1 个产物，实得: {:?}",
        artifacts(&targets)
    );
    assert_eq!(
        methods(&targets[0]),
        vec!["one".to_string(), "two".to_string()],
        "同模块两块应合进同一份产物的方法面"
    );

    // 拼接口径与 EmbedCollector 一致：空行分隔
    let dir = work.join(&targets[0].dir_rel);
    lang_zone::embed::artifact::write_artifacts(&work, &targets[0]).expect("write artifact");
    let py: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("产物目录不存在 {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .filter(|p| !p.file_name().unwrap().to_string_lossy().starts_with("__lz_dispatch"))
        .collect();
    assert_eq!(py.len(), 1, "合并后仍是一份源码: {:?}", py);
    let text = std::fs::read_to_string(&py[0]).expect("read py");
    assert!(text.contains("def one("), "合并后缺第一块的方法");
    assert!(text.contains("def two("), "合并后缺第二块的方法");
}

/// ③ 产物名与单文件路径同口径：同一个 `.lz`，被当项目模块编 vs 单文件编，
///    产物名与产物目录必须逐字相同（这就是「不要在 codegen 里另写一套命名规则」
///    的可执行判据：两边都走 `embed::resolve` + `dedup::artifact_name`）。
#[test]
fn project_artifact_name_matches_single_file_path() {
    let work = case_dir("stem_parity");
    write_src(&work, "main.lz", MAIN_TWO_MODULES);
    write_src(&work, "a.lz", A_PY);
    write_src(&work, "b.lz", B_PY);

    let (proj_targets, _) = compile_project(&work, "main.lz");
    assert_eq!(proj_targets.len(), 2, "前置：项目模式应有两个产物");

    for module in ["a.lz", "b.lz"] {
        let single = compile_single_file(&work.join(module));
        assert_eq!(single.0.len(), 1, "前置：{} 单文件应有一个产物", module);
        let want = &single.0[0];

        // 项目模式里按「方法面」认领对应产物（产物名不能拿模块名硬拼，正是判据本身）
        let got = proj_targets
            .iter()
            .find(|t| methods(t) == methods(want))
            .unwrap_or_else(|| {
                panic!(
                    "项目模式里找不到与单文件 {} 对应的方法面 {:?}；项目产物: {:?}",
                    module,
                    methods(want),
                    proj_targets
                        .iter()
                        .map(|t| (t.artifact.clone(), methods(t)))
                        .collect::<Vec<_>>()
                )
            });

        assert_eq!(
            got.artifact, want.artifact,
            "{} 在项目模式与单文件模式的产物名必须逐字相同",
            module
        );
        assert_eq!(
            got.dir_rel, want.dir_rel,
            "{} 的产物目录也必须逐字相同",
            module
        );
    }
}

/// ④ 确定性：同一工程再编译一次，产物名/目录逐字相同（无随机数/无时间戳/不读盘状态）。
#[test]
fn project_grouping_is_deterministic_across_runs() {
    let work = case_dir("determinism");
    write_src(&work, "main.lz", MAIN_TWO_MODULES);
    write_src(&work, "a.lz", A_PY);
    write_src(&work, "b.lz", B_PY);

    let (first, _) = compile_project(&work, "main.lz");
    let (second, _) = compile_project(&work, "main.lz");

    let pairs = |ts: &[EmbedTarget]| -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = ts
            .iter()
            .map(|t| (t.artifact.clone(), t.dir_rel.clone()))
            .collect();
        v.sort();
        v
    };
    assert_eq!(
        pairs(&first),
        pairs(&second),
        "两次编译的 (产物名, 目录) 必须逐字相同"
    );
}

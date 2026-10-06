// Cy（Cython）后端验收面 —— 补 2026-10-01 之前完全缺失的本地门禁
//
// 背景（实测，非推测）：
//   - `CY` 不是根 workspace 成员（Cargo.toml:2 `members = [".", "lz-infer", "lz_builtins"]`），
//     所以 `cargo test --workspace` 从不编译 CY/testsrc/cli.rs；
//   - CI 的 `for f in TESTS/*/ *.lz TESTS/*_blocks/*.lz` 里 `TESTS/*/` 匹配到的是**目录**，
//     `-f` 判假被跳过 ⇒ 实际只跑 4/54 件，且只生成不对比；
//   - `CY/scripts/cy_verify.py` 是唯一真语法级验证，但全仓无人引用（本轮之前）。
//   ⇒ 决策登记表 D4/D5 记的「cython 融进 lzcyc 后本地没有任何验收面」就是这件事。
//
// 本文件把验收面搬进根 crate 的 `#[test]`，分三层，**任何一层都不允许"环境缺了就当过"**：
//   L1 `cy_l1_transpile_all_corpus`        —— 只依赖产品 CLI（无需 python），进日常 workspace 全量
//   L2 `cy_l2_cythonize_all_corpus`        —— `python -m cython` 语法层，需显式 --ignored
//   L3 `cy_l3_run_all_corpus`              —— 编译成扩展模块真跑一遍，stdout 与 `*.exp` 逐字比
//
// `.exp` 期望值的两个来源（**都不许抄 cy 自己的输出**）：
//   - CY/TESTS/15_feature_matrix/* 与 CY/TESTS/07_data_structures/*：从 **Rust 后端**
//     同源码产物实测得来（TEMP/oracle_rust.py：lzc → rustc --extern lz_builtins → run，
//     配方与 tests/find_bug_libs.rs 同源）；封之前逐件跑过跨后端差分，
//     两侧 stdout 逐字一致才落盘（07_data_structures 8 件 → 差分 8/8 一致 → 封 7 件；
//     `enum_minimal.lz` 只声明 enum、无 print，两侧 stdout 恒为空 ⇒ 空 .exp 不承重，
//     故意不封：它连"产品坏了"都测不出，封了反而把"有 golden"当成覆盖）。
//   - 其余（12_build_blocks / 04_functions / 99_self_test / 99_bootstrap）：由 .lz 源码里的
//     print 字面量推导，写入脚本自带「源码 print 列表 == 期望列表」自检（99_self_test 每件恰好一条 print）。
//
// 语料：CY/TESTS/**/*.lz（件数由语料目录实测得出，L3 汇总行会打印；不在这里钉数字）。
// **一律先复制进 target/cy-gate/**：CY/TESTS 下的 .pyx 是受版管的参考件，
// 直接在语料目录里生成会覆盖它们（本轮开工时 `git status --porcelain CY/TESTS` 已有 53 项脏，
// 正是这么来的——注：那 53 项里除 variadic_test.lz 外都是 autocrlf 的行尾噪声，diff 为空）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const GATE_SUBDIR: &str = "cy-gate";

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn lang_zone_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
}

/// 语料根目录（受版管，只读）
fn corpus_root() -> PathBuf {
    manifest().join("CY/TESTS")
}

/// 工作目录（gitignored）：语料的完整镜像，产物落这里
fn work_root() -> PathBuf {
    manifest().join("target").join(GATE_SUBDIR)
}

/// L2/L3 的**在册**失败基线：键 = 语料相对路径，值 = 本轮实测的卡点原因。
///
/// 口径与 `tests/demo_codegen_compile.rs` 的 KNOWN_* 一致：
/// 非基线内的失败 ⇒ 红（新增回归）；基线内但已转绿 ⇒ 提示移出（防止清单只增不减）。
/// 每条都带可复跑判据，移出时必须同步删除本行。
/// L2 cythonize 层的**在册失败**：两条都已修复并同轮移出（2026-10-01）——
/// - `99_bootstrap/import_runtime.lz`：分发器改从实际发射集合 `emitted_fns` 取分支
///   （src/ir/codegen_cython.rs `gen_overload_dispatchers`），同名重复定义在 CLI 合并处
///   就被过滤（src/main.rs `merge_imports_into`）。
/// - `99_self_test/test_suite.lz`：test 体的嵌套 def 提升不再丢
///   （src/ir/builder.rs 两处 `test_ctx.pending_items = ctx.pending_items.clone()`）。
/// 空清单 = 全语料都该能过 cythonize。
const KNOWN_CYTHONIZE_FAILURES: &[(&str, &str)] = &[];

/// L3 差分层的在册豁免：Rust 侧本身也不通过 ⇒ 无 oracle 可比。
/// 空清单 = 全语料都该能比。
const KNOWN_DIFF_NOT_COMPARABLE: &[(&str, &str)] = &[];

/// L3 的 **oracle 覆盖登记表**——不是豁免清单，登记的是「这件没有逐字期望值，为什么」。
///
/// 为什么要它：L3 汇总行原先只有「执行通过 81」，而 81 件里只有 75 件做了 stdout 逐字比对，
/// 另外 6 件仅证明「编译过、跑完没抛」。没有这张表，报告面会把 81 读成 81 项行为被钉住
/// ——覆盖面窄于主张，是这类门禁最常见的说谎形状（本轮实测来源：`TEMP/why-unsealed-r9.log`）。
/// 双向可红：件无 `.exp` 而未登记 ⇒ 红（覆盖悄悄变小）；已登记但 `.exp` 在场 ⇒ 红（表陈旧，该件其实已覆盖）。
const NO_ORACLE_EXPLAINED: &[(&str, &str)] = &[
    ("01_basics/literals.lz", "rust 面 rustc E0282 取不到 oracle（BUG-29 在册）"),
    (
        "15_feature_matrix/named_args.lz",
        "rust 面 rustc E0425 取不到 oracle（BUG-24 在册）",
    ),
    (
        "99_bootstrap/lz_std.lz",
        "rust 面 rustc E0308 取不到 oracle（BUG-30 在册）",
    ),
    (
        "04_functions/generic_min.lz",
        "两侧 stdout 恒为空 ⇒ 空 golden 不承重，只能证「跑通」",
    ),
    (
        "05_expressions/elif_test.lz",
        "两侧 stdout 恒为空 ⇒ 空 golden 不承重，只能证「跑通」",
    ),
    (
        "07_data_structures/enum_minimal.lz",
        "两侧 stdout 恒为空 ⇒ 空 golden 不承重，只能证「跑通」",
    ),
    (
        "04_functions/closure_write_capture.lz",
        "cy 面按 SYNTAX/03e §五 已实现（'7'），rust 面 E0594 编不出 ⇒ 取不到 oracle（BUG-33 在册）",
    ),
];

fn walk_lz(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_lz(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("lz") {
            out.push(p);
        }
    }
}

/// 全部语料的仓内相对路径（正斜杠），排序稳定
fn corpus_relpaths() -> Vec<String> {
    let root = corpus_root();
    let mut files = Vec::new();
    walk_lz(&root, &mut files);
    let mut rels: Vec<String> = files
        .iter()
        .map(|f| {
            f.strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    rels.sort();
    rels
}

/// 把语料镜像进 target/cy-gate（保留子目录结构），返回工作树里对应的 .lz 路径
fn stage_corpus(rels: &[String]) -> Vec<PathBuf> {
    let root = corpus_root();
    let work = work_root();
    let mut out = Vec::new();
    for rel in rels {
        let src = root.join(rel);
        let dst = work.join(rel);
        std::fs::create_dir_all(dst.parent().unwrap()).expect("create gate dir");
        std::fs::copy(&src, &dst).expect("mirror corpus");
        out.push(dst);
    }
    out
}

/// 工具链探测。缺失时**必须红**，不得静默通过（这正是 CY/testsrc/cli.rs
/// `run_full_regression` 在 python 缺失时 return 报绿的缺陷）。
fn require_toolchain() -> (String, Vec<String>) {
    let py = std::env::var("LZ_PYTHON").unwrap_or_else(|_| "python".to_string());
    let probe = Command::new(&py)
        .args(["-m", "cython", "--version"])
        .output();
    match probe {
        Ok(o) if o.status.success() => (
            py,
            vec![String::from("-m"), String::from("cython")],
        ),
        Ok(o) => panic!(
            "CY-GATE-INACTIVE：`{py} -m cython --version` 非零退出 ⇒ 门禁不可用不是通过。\
             stderr={}",
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(e) => panic!(
            "CY-GATE-INACTIVE：无法启动 `{py}`（{e}）⇒ 门禁不可用不是通过。\
             需要 Python 3 + Cython；可用 LZ_PYTHON 指定解释器。"
        ),
    }
}

// ══════════════════════════════════════════════════════════════════
// L1：产品 CLI 的 cy 出口 + 语料/清单不变量（不依赖 python，进日常全量）
// ══════════════════════════════════════════════════════════════════

#[test]
fn cy_l1_corpus_and_allowlist_invariants() {
    let rels = corpus_relpaths();
    // 语料量级：CY/testsrc/cli.rs 也断言 >=50，口径对齐
    assert!(
        rels.len() >= 50,
        "CY/TESTS 语料只剩 {} 件（<50）⇒ 验收面失去覆盖面，这是门禁自身坏掉的形状",
        rels.len()
    );
    // 在册项必须指向真实存在的语料（悬空路径会一路绿，见 2026-09-30 的引用核验教训）
    let set: BTreeSet<&str> = rels.iter().map(|s| s.as_str()).collect();
    for (path, _) in KNOWN_CYTHONIZE_FAILURES
        .iter()
        .chain(KNOWN_RUNTIME_FAILURES.iter())
        .chain(KNOWN_DIFF_NOT_COMPARABLE.iter())
    {
        assert!(
            set.contains(*path),
            "在册项 {path} 不在语料里（已改名/删除？）⇒ 该条基线已失效，必须删或改指真身"
        );
    }
    // 清单不得有重复键（重复会让「移出数 == 转绿数」的核算失真）
    let mut seen = BTreeSet::new();
    for (path, _) in KNOWN_CYTHONIZE_FAILURES.iter().chain(KNOWN_RUNTIME_FAILURES.iter()) {
        assert!(seen.insert(*path), "基线清单里 {path} 出现了两次");
    }
}

#[test]
fn cy_l1_transpile_all_corpus() {
    let rels = corpus_relpaths();
    let files = stage_corpus(&rels);
    let mut bad: Vec<String> = Vec::new();
    let mut ann_bad: Vec<String> = Vec::new();
    let mut go_bad: Vec<String> = Vec::new();
    for (rel, lz) in rels.iter().zip(files.iter()) {
        let pyx = lz.with_extension("pyx");
        let _ = std::fs::remove_file(&pyx);
        let r = Command::new(lang_zone_bin())
            .arg(lz)
            .arg("--backend=cython")
            .output()
            .expect("run lang-zone");
        if !r.status.success() {
            bad.push(format!("{rel} 退出非零: {}", String::from_utf8_lossy(&r.stderr)));
            continue;
        }
        match std::fs::metadata(&pyx) {
            Ok(m) if m.len() > 0 => {}
            Ok(_) => bad.push(format!("{rel} 产出的 .pyx 是空文件")),
            Err(_) => bad.push(format!("{rel} 没有产出 .pyx（{}）", pyx.display())),
        }
        // 签名不得带 C 类型返回注解（台账 BUG-13）：与本层的转译同批做，不另开一遍。
        // 实测依据（Cython 3.2.2，`python -m cython -3`，探针 TEMP/annprobe/）：
        // `-> Py_ssize_t`／`-> double`／`-> bint` 只出 `warning: Unknown type
        // declaration ... in annotation, ignoring`，即**写了不承重**；而 Python
        // 内置名 `-> int` 是真做类型检查（TEMP/annprobe/g_int_wrong.pyx 编译失败），
        // 换过去等于给语言没承诺的位置加义务。本后端一律发 `def`（没有 cdef/cpdef），
        // 所以发不出「既承重又无害」的返回标注 ⇒ 唯一不说谎的形态是不写。
        // 改前产物快照：TEMP/ann-before-0314.log（88 处 `-> Py_ssize_t` 这类行，
        // 重生成命令写在该文件头两行注释里）。
        let text = std::fs::read_to_string(&pyx).unwrap_or_default();
        for line in text.lines() {
            let head = line.trim_start();
            if !(head.starts_with("def ") || head.starts_with("async def ")) {
                continue;
            }
            if let Some(pos) = head.find("-> ") {
                let tail = &head[pos + 3..];
                let name: String = tail
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if ["Py_ssize_t", "double", "bint", "float"].contains(&name.as_str()) {
                    ann_bad.push(format!("{rel}: {}", head.trim()));
                }
            }
        }
        // go/spawn 的**机制**侧（台账 BUG-12）：同步函数里出现 `__go(` 就等于「体在主流程里内联跑完」，
        // 那正是降级本身。为什么不能只靠 L3 的 .exp：期望值 `"after go"` 只证「任务失败不打断主流程」，
        // 一个把异常吞掉的同步垫片也能过。两侧配起来才是完整主张——行为侧管「不阻塞/失败隔离」，
        // 这条管「体确实被延后交给线程」。
        // 改前逐字（TEMP/probe8/gojar/p_go_panic.pyx:118，同一遍转译的产物）：`__go(boom())`
        // 落在 `def main():` 里 ⇒ 本条在修复前必红。
        // 只允许在 `async def` 里出现：那里 `__go(f(...))` 拿到的是**协程对象**（调用不执行体），
        // 透传是正确语义，跨后端实测一致（CY/TESTS/15_feature_matrix/async_spawn.lz）。
        let mut in_async = false;
        for line in text.lines() {
            let head = line.trim_start();
            if head.starts_with("async def ") {
                in_async = true;
                continue;
            }
            if head.starts_with("def ") {
                in_async = false;
                continue;
            }
            if !in_async && head.contains("__go(") {
                go_bad.push(format!("{rel}: {}", head.trim()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "L1 transpile 失败 {} 件（此层无在册豁免：产品 CLI 的 cy 出口必须全通）:\n{}",
        bad.len(),
        bad.join("\n")
    );
    assert!(
        ann_bad.is_empty(),
        "L1 产物里有 {} 处 C 类型返回注解（BUG-13：Cython 把它当未知类型忽略并告警，\n写了等于说谎；改前快照 TEMP/ann-before-0314.log）:\n{}",
        ann_bad.len(),
        ann_bad.join("\n")
    );
    assert!(
        go_bad.is_empty(),
        "L1 产物里有 {} 处同步上下文仍用 `__go(`（BUG-12：Python 实参先求值，这个形状下 go/spawn\n\
         的体必然在主流程里跑完，并发语义整体丢失；同步上下文应发 `__spawn(lambda: 体)`，\n\
         红证逐字 TEMP/probe8/gojar/p_go_panic.pyx:118 = `__go(boom())`）:\n{}",
        go_bad.len(),
        go_bad.join("\n")
    );
}

// ══════════════════════════════════════════════════════════════════
// L2：cython 语法层（.pyx → .c）。需 python + Cython，显式 --ignored 跑。
// 命令：cargo test -j 1 --test cy_codegen_gate -- --ignored --nocapture
// ══════════════════════════════════════════════════════════════════

/// Cython 的告警行：`warning: <路径>.pyx:<行>:<列>: <消息>`。
/// 只认这一形状 ⇒ 非告警的 stderr（`Error compiling...`、代码回显、续行）一律不当告警算，
/// 免得判据把「长得像」当成「是」。
///
/// 为什么 L2 要看 stderr：这一层的判据原先**只读退出码**，stderr 整个丢掉。BUG-13 那类
/// 缺陷（标注里写了不承重的东西）rc=0、stdout 也对，唯一信号就是这句
/// `Unknown type declaration ... ignoring`。
fn cython_warning_line(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("warning:")?.trim_start();
    let idx = rest.rfind(".pyx:")?;
    let mut parts = rest[idx + ".pyx:".len()..].splitn(3, ':');
    let (row, col, msg) = (parts.next()?, parts.next()?, parts.next()?);
    let is_num = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    if is_num(row) && is_num(col) {
        Some(msg.trim())
    } else {
        None
    }
}

/// 正向对照：拿**已知必响**的形状过一遍取数面，证明「语料 0 告警」是量出来的而不是看不见的。
/// 形状不是猜的，是 `TEMP/cy_warn_shape_probe.py` 在门禁这条命令下实测的
/// （同一实测还证明 `performance hint` 在此命令下**根本不打印** ⇒ 本判据的检测面只有 `warning:`）。
/// 对照失效（Cython 改了措辞之类）会让 L2 直接红，这是要的：静默失明的告警判据比没有更坏。
fn assert_cython_warning_channel(py: &str, cython_prefix: &[String]) {
    let d = work_root().join("_warn-control");
    std::fs::create_dir_all(&d).expect("create warn-control dir");
    let pyx = d.join("ctrl_unknown_type.pyx");
    std::fs::write(&pyx, "def f(x) -> nosuchtype:\n    return x\n").expect("write control pyx");
    let r = Command::new(py)
        .args(cython_prefix)
        .args(["-3", "--module-name", "cy_gate_gen"])
        .arg(pyx.canonicalize().unwrap_or_else(|_| pyx.clone()))
        .current_dir(&d)
        .output()
        .expect("run cython on control");
    let blob = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stderr),
        String::from_utf8_lossy(&r.stdout)
    );
    let hit: Vec<&str> = blob.lines().filter_map(cython_warning_line).collect();
    assert!(
        !hit.is_empty(),
        "L2 告警判据的取数面失明：对照件（标注未知类型）没被解析出任何 warning 行，Cython 回执如下 ⇒ 先修判据再谈语料\n{blob}"
    );
    assert!(
        hit.iter().any(|m| m.contains("Unknown type declaration")),
        "对照件解析出的告警不是预期的那一类: {hit:?}"
    );
    let _ = std::fs::remove_file(pyx.with_extension("c"));
    let _ = std::fs::remove_file(&pyx);
    println!("[cy L2-ctl] 告警取数面对照命中 {} 行: {}", hit.len(), hit[0]);
}

#[test]
#[ignore = "需要 python + Cython 工具链且逐件 cythonize 较慢；显式 --ignored 作为命名门禁跑"]
fn cy_l2_cythonize_all_corpus() {
    let (py, cython_prefix) = require_toolchain();
    assert_cython_warning_channel(&py, &cython_prefix);
    let rels = corpus_relpaths();
    let files = stage_corpus(&rels);
    let mut failed: Vec<(String, String)> = Vec::new();
    let mut warned: Vec<(String, String)> = Vec::new();
    let mut passed: Vec<String> = Vec::new();

    for (rel, lz) in rels.iter().zip(files.iter()) {
        let pyx = lz.with_extension("pyx");
        // L2 自带 transpile（与 L3 同一条纪律）：单跑 `--ignored cy_l2` 判的必须是
        // **当前产品码**的产物。原先直接读盘上残留的 .pyx ⇒ 2026-10-01 实测：修掉
        // prelude 引号缺陷后单独重跑 L2，读到的是修复前那轮的产物，62/62 报
        // "Unclosed string literal"——判据自己造了假红。
        let _ = std::fs::remove_file(&pyx);
        let tz = Command::new(lang_zone_bin())
            .arg(lz)
            .arg("--backend=cython")
            .output()
            .expect("transpile for L2");
        if !tz.status.success() {
            let e = String::from_utf8_lossy(&tz.stderr).trim().to_string();
            failed.push((rel.clone(), format!("transpile 退出非零: {e}")));
            continue;
        }
        if !pyx.exists() {
            failed.push((rel.clone(), "transpile 未产出 .pyx".into()));
            continue;
        }
        let cfile = pyx.with_extension("c");
        let _ = std::fs::remove_file(&cfile);
        // 必须传绝对路径且 cwd=pyx 目录：CY/scripts/cy_verify.py 原先传相对路径
        // 配 cwd 改变，导致 54/54 假红（判据自身缺陷伪装成产品缺陷，2026-10-01 实测）。
        let mut cmd = Command::new(&py);
        cmd.args(&cython_prefix)
            .args(["-3", "--module-name", "cy_gate_gen"])
            .arg(pyx.canonicalize().unwrap_or_else(|_| pyx.clone()))
            .current_dir(pyx.parent().unwrap());
        let r = cmd.output().expect("run cython");
        // 告警与退出码**各自记账**：rc=0 也可能带 warning（BUG-13 那类），rc!=0 时
        // 同一件既进 failed 也进 warned，两条线索都要能在报告里读到。
        let stderr_txt = String::from_utf8_lossy(&r.stderr).to_string();
        for l in stderr_txt.lines() {
            if cython_warning_line(l).is_some() {
                warned.push((rel.clone(), l.trim().to_string()));
            }
        }
        if r.status.success() {
            passed.push(rel.clone());
        } else {
            let mut s = stderr_txt.clone();
            if s.trim().is_empty() {
                s = String::from_utf8_lossy(&r.stdout).to_string();
            }
            let tail: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).rev().take(3).collect();
            failed.push((rel.clone(), tail.into_iter().collect::<Vec<_>>().join(" | ")));
        }
        let _ = std::fs::remove_file(&cfile);
        let _ = std::fs::remove_file(pyx.with_extension("html"));
    }

    let known: BTreeSet<&str> = KNOWN_CYTHONIZE_FAILURES.iter().map(|(p, _)| *p).collect();
    let unexpected: Vec<&(String, String)> = failed
        .iter()
        .filter(|(p, _)| !known.contains(p.as_str()))
        .collect();
    let stale: Vec<&str> = known
        .iter()
        .copied()
        .filter(|p| passed.iter().any(|q| q == p))
        .collect();

    println!(
        "[cy L2] 语料 {} 件 | cythonize 通过 {} | 失败 {}（在册 {}）| Cython 告警行 {}",
        rels.len(),
        passed.len(),
        failed.len(),
        known.len(),
        warned.len()
    );
    for (p, msg) in &failed {
        let tag = if known.contains(p.as_str()) { "在册" } else { "❌新增" };
        println!("   {tag} {p} :: {msg}");
    }
    for (p, msg) in &warned {
        println!("   ⚠ 告警 {p} :: {msg}");
    }
    for p in &stale {
        println!("   ℹ️  {p} 现已通过 cythonize ⇒ 应从 KNOWN_CYTHONIZE_FAILURES 移出");
    }
    // 基线 = 本轮实测（TEMP/cy-warn-census-182119.log：81 件 / 告警 0 行，且对照 2/2 看得见）。
    // 这里**不设告警豁免清单**：告警说的是「写上去的东西没生效」，加清单等于把它登记成常态。
    assert!(
        warned.is_empty(),
        "{} 行 Cython 告警（基线 0）⇒ 产物里存在「编译过但不承重」的东西，逐字如下:\n{}",
        warned.len(),
        warned
            .iter()
            .map(|(p, m)| format!("   {p} :: {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        unexpected.is_empty(),
        "{} 件出现非基线内的 cythonize 失败（新增回归）: {:?}",
        unexpected.len(),
        unexpected.iter().map(|(p, m)| format!("{p}: {m}")).collect::<Vec<_>>()
    );
    // 在册项若已转绿却还留在清单里 ⇒ 清单在变松，同样算红（防止"忘了删"沉淀成假覆盖）
    assert!(
        stale.is_empty(),
        "基线清单里 {stale:?} 已经转绿，必须同轮移出，否则清单口径与盘面脱钩"
    );
}

// ══════════════════════════════════════════════════════════════════
// L3：行为层 —— 把 .pyx 真编译成扩展模块并执行
// ══════════════════════════════════════════════════════════════════
//
// 为什么必须有这一层：L1 只证明「能生成」，L2 只证明「语法合法」。
// 已知的会造成 L1/L2 全绿而行为错的形状：`MethodCall` 未命中内建表时回退成
// `r.method(args)`（src/ir/codegen_cython.rs :2831 一带）——产出的 Python 语法完全合法，
// 语义却是错的。
//
// 判据两档：
//   a) 执行必须 exit 0（语料里 10/54 件是 assert/check 自测，跑通即真判据）；
//   b) 若语料旁存在同名 `<case>.exp`，则 stdout 必须**逐字相等**（期望值只能来自
//      另一后端的实测产物或 .lz 源码里的 print 字面量，见文件头；从 cy 产物反灌 =
//      把当期 bug 封成 golden）。
// 非基线内的运行时失败 ⇒ 红；基线内已转绿 ⇒ 红（清单必须同轮收紧）。

/// L3 运行时失败基线。键 = 语料相对路径，值 = 实测卡点。
const KNOWN_RUNTIME_FAILURES: &[(&str, &str)] = &[];

#[test]
#[ignore = "需要 python + Cython + 可用 C 编译器（本机 MSVC），逐件 pyximport 编译很慢"]
fn cy_l3_run_all_corpus() {
    let (py, _) = require_toolchain();
    let driver = manifest().join("CY/scripts/cy_gate_run.py");
    assert!(
        driver.exists(),
        "驱动脚本缺失：{} ⇒ L3 无从执行",
        driver.display()
    );

    let rels = only_filter(corpus_relpaths());
    let subset = std::env::var("LZ_CY_GATE_ONLY").is_ok();
    let files = stage_corpus(&rels);
    let corpus = corpus_root();

    let mut failed: Vec<(String, String)> = Vec::new();
    let mut mismatched: Vec<(String, String)> = Vec::new();
    let mut passed: Vec<String> = Vec::new();
    let mut compared: Vec<String> = Vec::new();
    let mut no_oracle: Vec<String> = Vec::new();

    for (rel, lz) in rels.iter().zip(files.iter()) {
        // L3 自带 transpile：单跑 `--ignored cy_l3` 也必须可判，不得依赖 L1 先跑过
        let pyx = lz.with_extension("pyx");
        let _ = std::fs::remove_file(&pyx);
        let tz = Command::new(lang_zone_bin())
            .arg(lz)
            .arg("--backend=cython")
            .output()
            .expect("transpile for L3");
        if !tz.status.success() {
            let e = String::from_utf8_lossy(&tz.stderr).trim().to_string();
            failed.push((rel.clone(), format!("transpile 退出非零: {e}")));
            println!("TRANSPILE_FAIL");
            continue;
        }
        if !pyx.exists() {
            failed.push((rel.clone(), "L1 未产出 .pyx".into()));
            println!("NO-PYX");
            continue;
        }
        let stem = pyx.file_stem().unwrap().to_string_lossy().to_string();
        // 逐件即时打印：一次全量要编译 54 个扩展模块，日志必须可中断恢复（不能只有末尾汇总）
        print!("[cy L3] {rel} … ");
        let r = Command::new(&py)
            .arg(&driver)
            .arg(pyx.parent().unwrap())
            .arg(&stem)
            .current_dir(pyx.parent().unwrap())
            .output()
            .expect("run cy_gate_run.py");
        let got = String::from_utf8_lossy(&r.stdout).to_string();
        if !r.status.success() {
            let err = String::from_utf8_lossy(&r.stderr).to_string();
            let tail: Vec<&str> = err
                .lines()
                .filter(|l| !l.trim().is_empty())
                .rev()
                .take(2)
                .collect();
            failed.push((rel.clone(), tail.into_iter().collect::<Vec<_>>().join(" | ")));
            println!("RUN_FAIL");
            continue;
        }
        let exp = corpus.join(rel).with_extension("exp");
        if exp.exists() {
            compared.push(rel.clone());
            let want = std::fs::read_to_string(&exp).expect("read .exp");
            if normalize_eol(&want) != normalize_eol(&got) {
                mismatched.push((rel.clone(), format!("期望 {want:?} / 实得 {got:?}")));
                println!("MISMATCH");
                continue;
            }
        } else {
            no_oracle.push(rel.clone());
        }
        passed.push(rel.clone());
        println!("ok");
    }

    let known: BTreeSet<&str> = KNOWN_RUNTIME_FAILURES.iter().map(|(p, _)| *p).collect();
    let unexpected: Vec<&String> = failed
        .iter()
        .chain(mismatched.iter())
        .map(|(p, _)| p)
        .filter(|p| !known.contains(p.as_str()))
        .collect();
    let stale: Vec<&str> = known
        .iter()
        .copied()
        .filter(|p| passed.iter().any(|q| q == p))
        .collect();

    println!(
        "[cy L3] 语料 {} 件 | 执行通过 {} | 运行异常 {} | 输出不符 {}（在册 {}）| 逐字比对 {} | 无 oracle 仅跑通 {}",
        rels.len(),
        passed.len(),
        failed.len(),
        mismatched.len(),
        known.len(),
        compared.len(),
        no_oracle.len()
    );
    for (p, m) in failed.iter().chain(mismatched.iter()) {
        let tag = if known.contains(p.as_str()) { "在册" } else { "❌新增" };
        println!("   {tag} {p} :: {m}");
    }
    assert!(
        unexpected.is_empty(),
        "{} 件出现非基线内的 L3 运行时失败/输出不符: {:?}",
        unexpected.len(),
        unexpected
    );
    assert!(
        stale.is_empty(),
        "KNOWN_RUNTIME_FAILURES 里 {stale:?} 已转绿，必须同轮移出"
    );

    // oracle 登记表与盘面双向对表（只在**全量**跑时下断言：子集里缺 .exp 的件可能根本没进本轮）
    let reg: BTreeSet<&str> = NO_ORACLE_EXPLAINED.iter().map(|(p, _)| *p).collect();
    if !subset {
        let unregistered: Vec<&String> = no_oracle
            .iter()
            .filter(|p| !reg.contains(p.as_str()))
            .collect();
        let already_covered: Vec<&str> = reg
            .iter()
            .copied()
            .filter(|p| compared.iter().any(|q| q == p))
            .collect();
        for (p, why) in NO_ORACLE_EXPLAINED {
            println!("   ℹ️ 无 oracle {p} :: {why}");
        }
        assert!(
            unregistered.is_empty(),
            "{} 件既无 `.exp` 又不在 NO_ORACLE_EXPLAINED 登记表里 ⇒ 逐字覆盖悄悄变小却没登记: {:?}",
            unregistered.len(),
            unregistered
        );
        assert!(
            already_covered.is_empty(),
            "登记表里 {already_covered:?} 已经有 `.exp` 并被逐字比对 ⇒ 表陈旧，必须同轮移出该条"
        );
        assert!(
            compared.len() + no_oracle.len() == rels.len(),
            "逐字比对 {} + 无 oracle {} != 语料 {} ⇒ 有件既没被比也没登记（差 {} 件）",
            compared.len(),
            no_oracle.len(),
            rels.len(),
            rels.len() as isize - (compared.len() + no_oracle.len()) as isize
        );
    }

    // 子集跑不能被读成"门禁过了"：覆盖面窄于主张是这类门禁最常见的说谎形状。
    assert!(
        !subset,
        "本次按 LZ_CY_GATE_ONLY 只跑了子集（{} 件）⇒ 诊断有效、判定无效：门禁口径恒为全量语料",
        rels.len()
    );
}

/// 单用例复跑旗标：`LZ_CY_GATE_ONLY=<路径前缀>` 只跑匹配的子集，用于定位；
/// 不设即全量。开启时本测试结尾会**主动红**一次，声明"这一遍不构成门禁判定"。
fn only_filter(rels: Vec<String>) -> Vec<String> {
    match std::env::var("LZ_CY_GATE_ONLY") {
        Ok(pref) if !pref.is_empty() => rels.into_iter().filter(|r| r.starts_with(&pref)).collect(),
        _ => rels,
    }
}

/// `.exp` 与产物 stdout 只做行尾归一，不做其它宽松化（放宽比较 = 放水）
fn normalize_eol(s: &str) -> String {
    s.replace("\r\n", "\n")
}

// Lang-Zone 编译器 — tests/moddec_product_assert.rs
//
// 修饰符装饰器「产物断言」——不满足于「能编译」（架构设计 §附·测试语料方案 D 表）：
//   L2/L3 产物比对 —— 装饰器形式 ≡ 等价关键字形式（golden 冻结快照 + 相对等价三重校验）
//   L4 类型断言    —— @mutex/@rwlock/@atomic/@rc/@arc/@cell/@lazy 的 Rust 目标类型
//   L5 运行期语义  —— @lazy 绑定处**不求值**（产物形态 + 副作用时序双重判定）、首次访问求值且仅一次
//                      （KI-4：旧的「计数 == 1」对 eager-once 亦成立，无法区分，已强化）
//                      + 多语句嵌套块（child CodeGen 路径）内读取仍须延迟改写
//                      （moddec_lazy_deferred_in_nested_block）
//
// 设计原则：只通过「调用编译器二进制 + 检查产物/输出」断言，不引用尚未实现的内部 Rust 类型。
// 编译一律以 `input.lz` 之名在独立临时目录进行（`__file__` 恒为 "input.lz"，与 golden 一致）。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 每次调用返回**唯一**的临时工作目录，规避 Windows 下 remove/create 竞态与并发踩踏
/// （`cargo test` 多线程 + 构建期文件锁曾产生偶发 "Error reading input.lz"）。
static SEQ: AtomicUsize = AtomicUsize::new(0);
fn fresh_dir(prefix: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!(
        "lz_moddec_{prefix}_{}_{}",
        std::process::id(),
        n
    ));
    std::fs::create_dir_all(&d).expect("create workdir");
    d
}

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
}

fn probe_rlib(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return Some(direct);
    }
    let deps = dir.join("deps");
    if let Ok(entries) = std::fs::read_dir(&deps) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
                return Some(e.path());
            }
        }
    }
    None
}

fn find_builtins_rlib() -> Option<PathBuf> {
    for profile in ["debug", "release"] {
        if let Some(p) = probe_rlib(&manifest().join("target").join(profile)) {
            return Some(p);
        }
    }
    None
}

fn corpus(name: &str) -> String {
    let p = manifest().join("DEMO").join("99_spec").join("moddec").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read corpus {}: {e}", p.display()))
}

fn golden(name: &str) -> String {
    let p = manifest()
        .join("DEMO")
        .join("99_spec")
        .join("moddec")
        .join("golden")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read golden {}: {e}", p.display()))
}

/// 编译源码（以 input.lz 之名）→ 返回 (ok, stderr, 生成的 .rs)
fn compile(name: &str, source: &str) -> (bool, String, Option<String>) {
    let work = fresh_dir(&format!("prod_{name}"));
    std::fs::write(work.join("input.lz"), source).expect("write input.lz");
    let out = Command::new(bin())
        .arg("input.lz")
        .current_dir(&work)
        .output()
        .expect("run lang-zone");
    let rs = std::fs::read_to_string(work.join("input.rs")).ok();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        rs,
    )
}

/// 编译 + 运行（编译为可执行文件），返回 (ok, stderr, stdout)
fn run_lz(name: &str, source: &str) -> (bool, String, String) {
    let work = fresh_dir(&format!("run_{name}"));
    std::fs::write(work.join("input.lz"), source).expect("write input.lz");

    let out = Command::new(bin())
        .arg("input.lz")
        .current_dir(&work)
        .output()
        .expect("run lang-zone");
    if !out.status.success() {
        return (
            false,
            format!("lzc 失败: {}", String::from_utf8_lossy(&out.stderr)),
            String::new(),
        );
    }
    let rs = work.join("input.rs");
    let exe = work.join("input.exe");
    let mut cmd = Command::new("rustc");
    cmd.args(["--edition", "2021"])
        .arg(&rs)
        .arg("-o")
        .arg(&exe);
    if let Some(r) = find_builtins_rlib() {
        cmd.arg("--extern").arg(format!("lz_builtins={}", r.display()));
    }
    let rc = cmd.output().expect("run rustc");
    if !rc.status.success() {
        return (
            false,
            format!("rustc 失败: {}", String::from_utf8_lossy(&rc.stderr)),
            String::new(),
        );
    }
    let run = Command::new(&exe).output().expect("run exe");
    (
        run.status.success(),
        String::from_utf8_lossy(&run.stderr).into_owned(),
        String::from_utf8_lossy(&run.stdout).into_owned(),
    )
}

// ─────────────────────  L2/L3：装饰器 ≡ 等价关键字（golden 冻结快照）  ─────────────────────

/// (装饰器语料, golden 文件, 等价关键字源码, AC)
const EQUIV: &[(&str, &str, &str, &str)] = &[
    (
        "moddec_basic_mut_var_pos.lz",
        "moddec_basic_mut_var_pos.rs",
        "def main() =\n    x = 42\n    print(x)\n",
        "AC1",
    ),
    (
        "moddec_basic_immut_var_pos.lz",
        "moddec_basic_immut_var_pos.rs",
        "def main() =\n    let pi = 3.14159\n    print(pi)\n",
        "AC2",
    ),
    (
        "moddec_basic_ref_var_pos.lz",
        "moddec_basic_ref_var_pos.rs",
        "def main() =\n    x = 42\n    ref r = x\n    print(r)\n",
        "AC3",
    ),
    (
        "moddec_basic_owned_var_pos.lz",
        "moddec_basic_owned_var_pos.rs",
        "def main() =\n    owned fd = 7\n    fd^\n",
        "AC3",
    ),
    (
        "moddec_basic_const_var_pos.lz",
        "moddec_basic_const_var_pos.rs",
        "const MAX: int = 1000\n\ndef main() =\n    print(MAX)\n",
        "AC3",
    ),
    (
        "moddec_basic_mut_param_pos.lz",
        "moddec_basic_mut_param_pos.rs",
        "def f(mut n: int) -> int = n + 1\n\ndef main() =\n    print(f(41))\n",
        "AC12",
    ),
    (
        "moddec_basic_comptime_param_pos.lz",
        "moddec_basic_comptime_param_pos.rs",
        "def f(comptime n: int) -> int = n + 1\n\ndef main() =\n    print(f(41))\n",
        "AC12",
    ),
    (
        "moddec_fusion_borrow_var_pos.lz",
        "moddec_fusion_borrow_var_pos.rs",
        "def main() =\n    x = 42\n    let ref r = x\n    print(r)\n",
        "AC11",
    ),
    (
        "moddec_fusion_borrow_mut_var_pos.lz",
        "moddec_fusion_borrow_mut_var_pos.rs",
        "def main() =\n    x = 42\n    ref r = x\n    print(r)\n",
        "AC11",
    ),
];

#[test]
fn moddec_decorator_equals_keyword_and_golden() {
    let mut failures: Vec<String> = Vec::new();

    for (corpus_file, golden_file, kw_src, ac) in EQUIV {
        let dec_src = corpus(corpus_file);
        let (ok_d, err_d, rs_d) = compile(&format!("eq_{}", corpus_file.trim_end_matches(".lz")), &dec_src);
        if !ok_d {
            failures.push(format!("[{ac}] {corpus_file}: 装饰器形式编译失败: {}", first_error(&err_d)));
            continue;
        }
        let rs_d = rs_d.unwrap_or_default();

        let (ok_k, err_k, rs_k) = compile(&format!("kw_{}", corpus_file.trim_end_matches(".lz")), kw_src);
        if !ok_k {
            failures.push(format!("[{ac}] {corpus_file}: 等价关键字形式编译失败: {}", first_error(&err_k)));
            continue;
        }
        let rs_k = rs_k.unwrap_or_default();
        let g = golden(golden_file);

        if rs_d.trim() != rs_k.trim() {
            failures.push(format!(
                "[{ac}] {corpus_file}: 装饰器产物 ≠ 等价关键字产物\n--- 装饰器 ---\n{}\n--- 关键字 ---\n{}",
                rs_d, rs_k
            ));
            continue;
        }
        if rs_d.trim() != g.trim() {
            failures.push(format!(
                "[{ac}] {corpus_file}: 装饰器产物 ≠ golden({golden_file})\n--- 生成 ---\n{}\n--- golden ---\n{}",
                rs_d, g
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} 组等价未成立（AC1/2/3/11/12）:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// AC1 三元等价：`@mut x = 42` / `mut x = 42` / `x = 42` 三者产物逐字节一致
#[test]
fn moddec_mut_triple_is_equivalent() {
    let dec = corpus("moddec_basic_mut_var_pos.lz");
    let kw_mut = "def main() =\n    mut x = 42\n    print(x)\n";
    let bare = "def main() =\n    x = 42\n    print(x)\n";

    let (ok1, e1, rs1) = compile("triple_dec", &dec);
    let (ok2, e2, rs2) = compile("triple_mut", kw_mut);
    let (ok3, e3, rs3) = compile("triple_bare", bare);
    assert!(ok1, "装饰器形式失败: {}", first_error(&e1));
    assert!(ok2, "`mut x = 42` 失败: {}", first_error(&e2));
    assert!(ok3, "`x = 42` 失败: {}", first_error(&e3));

    let (a, b, c) = (rs1.unwrap(), rs2.unwrap(), rs3.unwrap());
    assert_eq!(a.trim(), b.trim(), "AC1：@mut x = 42 ≠ mut x = 42");
    assert_eq!(a.trim(), c.trim(), "AC1：@mut x = 42 ≠ x = 42");
    assert_eq!(
        a.trim(),
        golden("equiv_mut_triple.rs").trim(),
        "AC1：三元产物 ≠ golden(equiv_mut_triple.rs)"
    );
}

// ─────────────────────────────  L4：目标类型断言  ─────────────────────────────

/// (装饰器语料, 期望目标类型标记[命中任一即可], AC)
///
/// 说明：断言为「子串命中任一」，故容忍包装形态（如 `@once` 若 codegen 产出
/// `Arc<OnceLock<T>>`，含 `OnceLock<` 即通过）；只有完全缺失该目标类型才判失败。
/// `@once` 轴值经 T01 冻结为 `{lazy,shared:Arc,origin:"once"}`，需 codegen 按 `origin`
/// 派发到 `OnceLock<T>`（而非裸 `Arc<T>`），故此处仍断言 `OnceLock<`。
const TYPE_CASES: &[(&str, &[&str], &str)] = &[
    ("moddec_fusion_mutex_var_pos.lz", &["Arc<Mutex<"], "AC10/AC11"),
    ("moddec_fusion_rwlock_var_pos.lz", &["Arc<RwLock<"], "AC10/AC11"),
    ("moddec_fusion_atomic_var_pos.lz", &["Arc<Atomic", "AtomicI64", "Atomic"], "AC10/AC11"),
    ("moddec_fusion_rc_var_pos.lz", &["Rc<"], "AC10/AC11"),
    ("moddec_fusion_arc_var_pos.lz", &["Arc<"], "AC10/AC11"),
    ("moddec_basic_shared_var_pos.lz", &["Rc<"], "AC10/O5"), // @shared 默认非原子 Rc（O5）
    ("moddec_basic_cell_var_pos.lz", &["Cell<", "RefCell<"], "AC10"),
    ("moddec_basic_lazy_var_pos.lz", &["OnceCell<", "OnceLock<", "LazyLock<"], "AC4"),
    ("moddec_fusion_once_var_pos.lz", &["OnceLock<", "OnceCell<"], "AC10"),
    ("moddec_fusion_lazy_static_var_pos.lz", &["LazyLock<", "OnceLock<"], "AC10"),
];

#[test]
fn moddec_generated_target_types() {
    let mut failures: Vec<String> = Vec::new();

    for (corpus_file, markers, ac) in TYPE_CASES {
        let src = corpus(corpus_file);
        let (ok, err, rs) = compile(&format!("ty_{}", corpus_file.trim_end_matches(".lz")), &src);
        if !ok {
            failures.push(format!("[{ac}] {corpus_file}: 编译失败: {}", first_error(&err)));
            continue;
        }
        let rs = rs.unwrap_or_default();
        if !markers.iter().any(|m| rs.contains(*m)) {
            failures.push(format!(
                "[{ac}] {corpus_file}: 未生成期望目标类型 {:?}\n--- 生成产物 ---\n{}",
                markers, rs
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} 组目标类型断言未通过（AC4/AC10/AC11）:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

// ─────────────────────────────  L5：@lazy 运行期语义  ─────────────────────────────

/// 在生成产物中定位变量 `name` 的**绑定行**：首个同时含 `name` 与 `=` 的行（已 `trim`）。
///
/// 声明/绑定总先于任何访问，故取首个匹配行即为绑定处；用于断言「绑定处」的产物形态。
fn binding_line<'a>(rs: &'a str, name: &str) -> Option<&'a str> {
    rs.lines()
        .find(|l| l.contains(name) && l.contains('='))
        .map(str::trim)
}

/// AC4：`@lazy cfg = load()` **绑定处不求值**；首次访问才求值且仅一次（缓存）。
///
/// 强化点（修复 KI-4）：旧断言仅校验 `lazy-eval` 出现 **次数 == 1**——但「eager-once」
/// （绑定处 `let cfg = load()` 即求值）同样只求值一次，**无法区分**「绑定处求值」与
/// 「首访求值」，故 AC4 实际未被验证。现补两条判定（至少其一即可钉住，此处两条都做）：
///
///   (A) **产物形态**：`@lazy` 产物须含惰性包装类型（`OnceCell<`/`OnceLock<`/`LazyLock<`），
///       且**绑定行不得出现初始化函数 `load` 的调用**——绑定处应为 `OnceCell::new()` 之类，
///       而非 `let cfg = load()`。
///   (B) **运行期副作用时序**：初始化体副作用 `lazy-eval` 必须**出现在 `ready` 之后**
///       （证明绑定处未求值），且总计 **1 次**（证明缓存）。
#[test]
fn moddec_lazy_evaluates_once() {
    let src = corpus("moddec_basic_lazy_var_pos.lz");

    // ── (A) 产物形态断言：绑定处不求值 ─────────────────────────────────
    let (ok_c, err_c, rs) = compile("lazy_once_shape", &src);
    assert!(ok_c, "@lazy 用例编译失败: {}", first_error(&err_c));
    let rs = rs.unwrap_or_default();

    let lazy_markers = ["OnceCell<", "OnceLock<", "LazyLock<"];
    assert!(
        lazy_markers.iter().any(|m| rs.contains(*m)),
        "AC4：@lazy 产物未含惰性包装类型 {:?}（绑定处未走惰性构造）\n--- 生成产物 ---\n{}",
        lazy_markers,
        rs
    );

    if let Some(line) = binding_line(&rs, "cfg") {
        assert!(
            !line.contains("load("),
            "AC4：@lazy **绑定处即求值**——绑定行调用了初始化函数 `load`，应为惰性构造（如 OnceCell::new()）\n绑定行: {line}\n--- 生成产物 ---\n{}",
            rs
        );
    }

    // ── (B) 运行期副作用时序断言：绑定处未求值 + 仅求值一次 ──────────────
    let (ok, err, out) = run_lz("lazy_once", &src);
    assert!(ok, "@lazy 用例编译/运行失败: {err}");

    let n = out.matches("lazy-eval").count();
    assert_eq!(
        n, 1,
        "AC4：@lazy 求值次数应为 1（缓存），实际 {} 次；stdout={:?}",
        n, out
    );

    let pos_ready = out
        .find("ready")
        .expect("stdout 未见 `ready`（绑定后的首条语句标记）");
    let pos_eval = out
        .find("lazy-eval")
        .expect("stdout 未见 `lazy-eval`（初始化体副作用探针）");
    assert!(
        pos_eval > pos_ready,
        "AC4：@lazy 在**绑定处**即求值——`lazy-eval` 出现在 `ready` 之前（应在之后，证明绑定处不求值）。\nstdout={:?}",
        out
    );
}

/// AC4（KI-4 续）：`@lazy` 变量在**多语句嵌套块**（if/while/for 体 ≥2 语句 → child CodeGen
/// 路径）内被读取时，仍须改写为 `*cfg.get_or_init(|| ...)`；否则会**静默退化**为裸 `cfg`
/// ——能编译但打印的是 `OnceCell` 容器而非值（AC4 静默失效）。
///
/// 回归背景：child `CodeGen` 复刻环境时若未继承 `lazy_bindings`，块内读取不被改写。
/// 语料 `moddec_lazy_nested_block_pos.lz`：`if` 体内 2 条语句，其中一条读取 `cfg`。
#[test]
fn moddec_lazy_deferred_in_nested_block() {
    let src = corpus("moddec_lazy_nested_block_pos.lz");

    // (A) 产物形态：块内 + 块外共 2 处读取均改写为 get_or_init，且不存在裸读 `cfg`。
    let (ok_c, err_c, rs) = compile("lazy_nested_shape", &src);
    assert!(ok_c, "@lazy 嵌套块用例编译失败: {}", first_error(&err_c));
    let rs = rs.unwrap_or_default();

    let reads = rs.matches("get_or_init").count();
    assert_eq!(
        reads, 2,
        "AC4：@lazy 嵌套块内读取未被改写（期望 2 处 `get_or_init`：块内 + 块外，实际 {reads}）\n--- 生成产物 ---\n{rs}"
    );
    assert!(
        !rs.contains(", cfg)"),
        "AC4：@lazy 嵌套块内存在**未改写**的裸读 `cfg`（将打印 OnceCell 容器而非值）\n--- 生成产物 ---\n{rs}"
    );

    // (B) 运行期：仅求值一次（缓存），且发生在块内首访（`ready`/`in-block` 之后）。
    let (ok, err, out) = run_lz("lazy_nested", &src);
    assert!(ok, "@lazy 嵌套块用例编译/运行失败: {err}");
    let n = out.matches("lazy-eval").count();
    assert_eq!(n, 1, "AC4：@lazy 求值次数应为 1（缓存），实际 {n} 次；stdout={out:?}");

    let pos_ready = out.find("ready").expect("stdout 未见 `ready`（绑定后语句标记）");
    let pos_eval = out
        .find("lazy-eval")
        .expect("stdout 未见 `lazy-eval`（初始化体副作用探针）");
    assert!(
        pos_eval > pos_ready,
        "AC4：lazy-eval 应出现在 `ready` 之后（块内首访才求值）；stdout={out:?}"
    );
    assert!(
        !out.contains("OnceCell"),
        "AC4：stdout 出现 `OnceCell` 容器（块内裸读未改写导致语义泄漏）；stdout={out:?}"
    );
}

fn first_error(s: &str) -> String {
    s.lines()
        .find(|l| {
            let t = l.trim();
            t.starts_with("error") || t.contains("Error") || t.contains("错误")
        })
        .or_else(|| s.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or("(无输出)")
        .trim()
        .to_string()
}

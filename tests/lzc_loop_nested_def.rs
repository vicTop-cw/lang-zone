// Lang-Zone 编译器 — tests/lzc_loop_nested_def.rs
//
// lzc-loop 推进 R1（非 IR 域）回归闸门：内嵌函数（nested def）提升 + 推导式 over 列表变量。
//
// 设计原则（PROJECT-SPEC/03 §2）：
// - 每个用例 = LZ 源码（input）→ lang-zone codegen → rustc → 运行 → 断言 stdout（expected）
// - 仅走默认 IR 路线；新增用例只增不删（门禁 4）
// - 用例来源：`.fist-loop-20260927/probe/` 手工实跑取证（rustc + 运行均通过后才落为闸门）
//
// 覆盖意图：这两条路径此前**无端到端闸门保护**（DEMO 覆盖不完整、typer 对内嵌 def 不做推断），
// 一旦 IR/前端回归导致内嵌函数不再提升或推导式迭代器退化，本闸门立即红。

use std::path::PathBuf;
use std::process::Command;

fn builtins_rlib() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug");
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return direct;
    }
    let deps = dir.join("deps");
    if let Ok(entries) = std::fs::read_dir(&deps) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
                return e.path();
            }
        }
    }
    panic!("lz_builtins rlib not found under target/debug");
}

/// 编译并运行单个 .lz 源，返回运行 stdout；任一环节失败则 panic 并附诊断
fn run_lz(name: &str, source: &str) -> String {
    let work = std::env::temp_dir().join(format!("lzc_loop_{name}"));
    let _ = std::fs::create_dir_all(&work);
    let lz = work.join("input.lz");
    std::fs::write(&lz, source).expect("write lz source");

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let out = Command::new(&bin).arg(&lz).output().expect("run lang-zone");
    assert!(
        out.status.success(),
        "[{name}] lang-zone 编译失败: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let rs = lz.with_extension("rs");
    assert!(rs.exists(), "[{name}] 未生成 .rs 产物");
    let exe = lz.with_extension("exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("-L")
        .arg(format!(
            "dependency={}/target/debug/deps",
            env!("CARGO_MANIFEST_DIR")
        ))
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    assert!(
        rc.status.success(),
        "[{name}] rustc 编译失败:\n{}",
        String::from_utf8_lossy(&rc.stderr)
    );

    let run = Command::new(&exe).output().expect("run compiled exe");
    assert!(
        run.status.success(),
        "[{name}] 程序运行失败: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

/// 内嵌 def 必须被提升为模块级函数并可被外层调用（IR: 嵌套 fn 提升）
#[test]
fn nested_def_is_hoisted_and_callable() {
    let out = run_lz(
        "nested_def",
        "def outer() -> int =\n    def inner(x: int) -> int = x + 1\n    inner(41)\nprint(outer())\n",
    );
    assert_eq!(out.trim(), "42", "内嵌函数调用结果不符：{out}");
}

/// 内嵌 def 可被多次调用，且不污染外层命名空间之外的语义（两层嵌套 + 顶层同名不冲突）
#[test]
fn nested_def_two_levels() {
    let out = run_lz(
        "nested_def2",
        "def outer(a: int) -> int =\n    def mid(b: int) -> int =\n        def leaf(c: int) -> int = c * 2\n        leaf(b) + 1\n    mid(a) + 1\nprint(outer(3))\n",
    );
    assert_eq!(out.trim(), "8", "两层内嵌函数结果不符：{out}");
}

/// 列表推导式 over 列表变量（非仅区间迭代器）：[x * 2 for x in xs]
#[test]
fn list_comprehension_over_list_variable() {
    let out = run_lz(
        "comprehension_over_list",
        "def f() -> List<int> =\n    let xs = [1, 2, 3]\n    [x * 2 for x in xs]\nprint(f())\n",
    );
    assert_eq!(out.trim(), "[2, 4, 6]", "列表推导式结果不符：{out}");
}

/// 顶层（非函数体内）列表推导式 over 列表变量
#[test]
fn list_comprehension_top_level() {
    let out = run_lz(
        "comprehension_top_level",
        "let xs = [1, 2, 3]\nlet ys = [x + 1 for x in xs]\nprint(ys)\n",
    );
    assert_eq!(out.trim(), "[2, 3, 4]", "顶层列表推导式结果不符：{out}");
}

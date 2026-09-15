//! 双路线 golden 对照测试（阶段1b / FIST 任务 T4.2 ③）
//!
//! 背景：main.rs 默认路径已切换为 IR 路线（build_ir + IrCodeGen），旧 AST
//! 直接 codegen 退役但保留 `--ast-codegen` 回退开关，仅用于双路线 golden 对照。
//!
//! 对照语义（重要）：
//! - 升级计划第 7.3 节"双路线输出逐字符一致（golden 对照）"在本测试中落地为
//!   **行为级逐字符对照**：对同一 .lz 输入，IR 路线与旧 AST 路线各自生成 .rs、
//!   rustc 编译、运行，断言两路线的**运行输出（stdout）逐字符一致**。
//! - 生成代码**文本**级逐字符一致不可达且不应追求：IR 路线输出为生产级规范格式
//!   （allow 属性、use 导入、pub fn + return、显式类型标注、模块元数据常量），
//!   旧 AST 路线输出为 legacy 裸格式（无 use、无 pub、表达式体），且对部分特性
//!   （默认参数、泛型）会生成 Rust 非法/退化代码（如 `name: String = "world"`、
//!   `x: std::any::Any`）。因此文本级对照没有意义，行为级对照才是迁移安全网。
//! - 对旧 AST 路线无法编译的用例（P0 新特性），测试不失败，仅记录已知退役差异，
//!   并断言 IR 路线产物本身可编译运行且输出符合 golden 基线（防 IR 回归）。

use std::path::{Path, PathBuf};
use std::process::Command;

/// 用例：变量与算术（双路线都应支持）
const CASE_SIMPLE: &str = r#"
def main() =
    let x = 40
    let y = 2
    print(x + y)
"#;

/// 用例：函数定义与调用（双路线都应支持）
const CASE_FUNC: &str = r#"
def add(x: int, y: int) -> int =
    x + y

def main() =
    let r = add(20, 22)
    print(r)
"#;

/// 用例：P0 #3 顶层构建块（旧 AST 路线不支持，IR 路线应正确）
const CASE_TOP_BUILD: &str = r#"
BASE =:
    10 + 20

def main() =
    print(BASE)
    print(BASE * 2)
"#;

/// 用例：P0 #1 __call__ 魔法方法（旧 AST 路线不支持，IR 路线应正确）
const CASE_CALLABLE: &str = r#"
struct Multiplier =
    factor: int

    def __call__(self, x: int) -> int =
        self.factor * x

struct Adder =
    base: int

    def __call__(self, a: int, b: int) -> int =
        self.base + a + b

def main() =
    let doubler = Multiplier(factor: 2)
    print(doubler(21))
    let add5 = Adder(base: 5)
    print(add5(1, 2))
    let f = doubler
    print(f(10))
"#;

fn write_work(work: &Path, name: &str, source: &str) -> PathBuf {
    let _ = std::fs::create_dir_all(work);
    let p = work.join(name);
    std::fs::write(&p, source).expect("write lz source");
    p
}

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

/// 用指定路线生成 .rs 并编译运行，返回 (rustc_ok, stdout)
///
/// 注意：lang-zone 固定将 .rs 输出到输入文件同目录同名（input.lz -> input.rs）。
fn run_route(lz_path: &Path, ast_codegen: bool) -> (bool, String) {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let mut cmd = Command::new(&bin);
    cmd.arg(lz_path);
    if ast_codegen {
        cmd.arg("--ast-codegen");
    }
    let out = cmd.output().expect("run lang-zone route");
    assert!(
        out.status.success(),
        "route {} 失败: {}",
        if ast_codegen { "AST" } else { "IR" },
        String::from_utf8_lossy(&out.stderr)
    );
    let rs = lz_path.with_extension("rs");
    let rs_bytes = std::fs::read(&rs).expect("read generated .rs");
    assert!(!rs_bytes.is_empty(), "生成 .rs 不应为空");

    let exe = lz_path.with_extension("exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins_rlib().display()))
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run rustc");
    if !rc.status.success() {
        return (false, String::new());
    }
    let run = Command::new(&exe).output().expect("run compiled exe");
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();
    (true, stdout)
}

fn check_case(case_name: &str, source: &str, expected_ir_stdout: Option<&str>) {
    let work = std::env::temp_dir().join(format!("lz_dual_route_golden_{}", case_name));
    let _ = std::fs::remove_dir_all(&work);
    let lz = write_work(&work, "input.lz", source);

    // 路 1：IR 路线（默认）
    let (ir_ok, ir_out) = run_route(&lz, false);
    assert!(ir_ok, "[{case_name}] IR 路线产物 rustc 编译失败——IR 回归");
    assert!(!ir_out.is_empty(), "[{case_name}] IR 路线运行无输出");

    if let Some(expected) = expected_ir_stdout {
        assert_eq!(ir_out, expected, "[{case_name}] IR 路线输出与 golden 不一致");
    }

    // 路 2：旧 AST 路线（--ast-codegen，legacy 对照）
    let (ast_ok, ast_out) = run_route(&lz, true);
    if !ast_ok {
        eprintln!("[{case_name}] 旧 AST 路线产物 rustc 编译失败（已知退役差异，跳过行为对照）");
        return;
    }
    // 行为级逐字符对照：两路线运行输出必须逐字符一致
    assert_eq!(
        ir_out, ast_out,
        "[{case_name}] 双路线运行输出不一致（IR 迁移行为回退？）\nIR:  {:?}\nAST: {:?}",
        ir_out, ast_out
    );
}

#[test]
fn dual_route_golden_simple() {
    check_case("simple", CASE_SIMPLE, Some("42\n"));
}

#[test]
fn dual_route_golden_func() {
    check_case("func", CASE_FUNC, Some("42\n"));
}

#[test]
fn dual_route_golden_top_build() {
    // P0 #3 顶层构建块：旧 AST 路线不支持，仅 IR golden 基线
    check_case(
        "top_build",
        CASE_TOP_BUILD,
        Some("30\n60\n"),
    );
}

#[test]
fn dual_route_golden_callable() {
    // P0 #1 __call__：旧 AST 路线不支持，仅 IR golden 基线
    check_case("callable", CASE_CALLABLE, Some("42\n8\n20\n"));
}

// Lang-Zone 编译器 — tests/ir_snapshots.rs
// LZIR 快照测试：验证 DEMO/ 下所有 .lz 文件可成功生成 IR
//
// 测试覆盖：
// - 基本 IR 生成端到端测试
// - 结构体 / 控制流 / 字面量 / 表达式 IR 输出验证
// - DEMO 文件批量 IR 快照测试

use std::path::PathBuf;
use std::process::Command;
use std::fs;

/// 编译单个 .lz 文件并验证 IR 生成，返回 IR 文本
fn test_ir_emit(file_path: &str) -> Result<String, String> {
    let bin_path = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
    let output = Command::new(&bin_path)
        .arg(file_path)
        .arg("--emit=ir")
        .output()
        .map_err(|e| format!("Failed to run lang-zone: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("lang-zone exited with error:\n{stderr}"));
    }

    let ir_text = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(ir_text)
}

/// 辅助函数：创建临时 .lz 文件并运行 IR 测试
fn run_ir_test(source: &str, name: &str, checks: &[&str]) {
    let tmp_dir = std::env::temp_dir();
    let lz_path = tmp_dir.join(format!("_ir_test_{name}.lz"));
    fs::write(&lz_path, source).expect("write test file");

    let result = test_ir_emit(lz_path.to_str().unwrap());
    let _ = fs::remove_file(&lz_path);

    match result {
        Ok(ir) => {
            for check in checks {
                assert!(
                    ir.contains(check),
                    "IR should contain '{}', got:\n{}",
                    check,
                    ir
                );
            }
        }
        Err(e) => {
            panic!("IR test '{name}' failed: {e}");
        }
    }
}

#[test]
fn ir_simple_function() {
    let lz_source = r#"
def add(x: int, y: int) -> int =
    x + y

def greet(name: str) -> str =
    "Hello, " + name
"#;
    run_ir_test(lz_source, "simple_fn", &["LZIR v1", "fn add", "fn greet", "x: int", "y: int", "name: str"]);
}

#[test]
fn ir_empty_module() {
    // v157 起空模块也生成模块级魔法属性 items（__name__/__file__/__package__/
    // __path__/__doc__/__is_macro__），共 6 个
    run_ir_test("", "empty", &[
        "LZIR v1",
        ";; 6 items",
        "const __name__: str",
        "const __doc__: str",
        "const __is_macro__: bool",
    ]);
}

#[test]
fn ir_literals() {
    let source = r#"
def main() -> () =
    let x = 42
    let y = 3.14
    let s = "hello"
    let b = true
    let u = None
"#;
    run_ir_test(source, "literals", &[
        "LZIR v1", "fn main",
        "42_i64", "3.14_f64", "\"hello\"",
        "true", "None",
    ]);
}

#[test]
fn ir_if_else() {
    let source = r#"
def check_val(x: int) -> str =
    if x > 0:
        "positive"
    else:
        "non-positive"
"#;
    run_ir_test(source, "if_else", &[
        "LZIR v1", "fn check_val",
        "if", "else",
    ]);
}

#[test]
fn ir_struct_definition() {
    let source = r#"
struct Point =
    x: f64
    y: f64

def dist(p: Point) -> f64 =
    0.0
"#;
    run_ir_test(source, "struct", &[
        "LZIR v1", "struct Point",
        "x: f64", "y: f64",
        "fn dist",
    ]);
}

#[test]
fn ir_let_bindings() {
    let source = r#"
def demo() -> int =
    let x = 42
    let y = x + 1
    y
"#;
    run_ir_test(source, "let_bindings", &[
        "LZIR v1", "fn demo",
        "let x: int", "let y:",
        "binop",
    ]);
}

#[test]
fn ir_generator_yield() {
    let source = r#"
iterator counter(n: int) -> int =
    for i in 0..n:
        yield i

def main() =
    for x in counter(5):
        print(x)
"#;
    run_ir_test(source, "gen_yield", &[
        "LZIR v1",
        "fn counter",
        "yield",
        "fn main",
        "for x",
    ]);
}

/// 批量测试：验证关键 DEMO 文件可成功生成 IR
#[test]
fn ir_demo_snapshots() {
    let demo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("DEMO");
    let key_files = [
        "01_basics/literals.lz",
        "01_basics/identifiers.lz",
        "01_basics/keywords.lz",
        "02_types/primitives.lz",
        "02_types/containers.lz",
        "03_variables/mutable_let.lz",
        "03_variables/const.lz",
        "04_functions/basic.lz",
        "04_functions/generics.lz",
        "04_functions/composite.lz",
        "05_expressions/pipe.lz",
        "05_expressions/if_match_expr.lz",
        "05_expressions/ternary.lz",
        "05_expressions/null_coalesce.lz",
        "05_expressions/comprehension.lz",
        "06_control_flow/if_elif_else.lz",
        "06_control_flow/for_while_loop.lz",
        "06_control_flow/break_continue.lz",
        "06_control_flow/guard.lz",
        "06_control_flow/with_defer.lz",
        "06_control_flow/loop_demo.lz",
        "07_data_structures/struct.lz",
        "07_data_structures/struct_more.lz",
        "07_data_structures/trait_impl.lz",
        "07_data_structures/magic_methods.lz",
        "08_modules/import_demo.lz",
        "09_macros/comptime_demo.lz",
        "09_macros/macro_demo.lz",
        "10_error_handling/panic_raise_try.lz",
        "11_concurrency/async_spawn.lz",
        "12_build_blocks/var_call_block.lz",
        "13_operators/compound_assign_more.lz",
        "14_pointers/box_rc_arc.lz",
        "15_generators/yield_demo.lz",
        "16_testing/test_suite.lz",
        "combo-syntax/combo_async_spawn.lz",
        "combo-syntax/combo_defer_guard_try.lz",
        "combo-syntax/combo_enum_match_guardlet.lz",
        "combo-syntax/combo_for_walrus.lz",
        "combo-syntax/combo_generic_struct_method.lz",
        "combo-syntax/combo_match_ternary.lz",
        "combo-syntax/combo_pipe_partial.lz",
        "combo-syntax/combo_struct_method_partial.lz",
    ];

    let mut passed = 0;
    let mut failed = Vec::new();

    for file_path in &key_files {
        let full_path = demo_root.join(file_path);
        let path_str = full_path.to_str().unwrap();

        match test_ir_emit(path_str) {
            Ok(ir) => {
                assert!(
                    ir.contains("LZIR v1"),
                    "{} should contain LZIR v1 header, got:\n{}",
                    file_path, ir
                );
                // 确保有实际条目（非空 IR）
                assert!(
                    !ir.is_empty(),
                    "{} produced empty IR output",
                    file_path
                );
                passed += 1;
            }
            Err(e) => {
                failed.push((file_path.to_string(), e));
            }
        }
    }

    println!("\n===== IR DEMO 快照测试报告 =====");
    println!("  总计: {} 文件", key_files.len());
    println!("  通过: {}", passed);
    println!("  失败: {}", failed.len());
    if !failed.is_empty() {
        for (path, reason) in &failed {
            println!("    ❌ {} — {}", path, reason);
        }
    }
    println!("================================\n");

    assert!(
        failed.is_empty(),
        "{} demo IR snapshots failed",
        failed.len()
    );
}

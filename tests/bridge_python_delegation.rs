// BUG-520 回归锁：PyO3Bridge 生成的 `#[pyfunction]` 必须**真实委托**到 LZ 侧
// `__py_<name>`，而非吐 `todo!()` 桩。本测试做两件事：
//   1. 断言生成文本含委托调用 + 委托声明，且不再含 `todo!()`；
//   2. 把生成的委托核（去掉 pyo3 属性）与一个 stub `__py_` 实现拼成一个完整
//      Rust 程序，**真跑 rustc 编译 + 运行**，证明生成代码是可编译、可运行的合法 Rust。
// 判据：编译 0 error、运行输出与 stub 语义一致。

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn fixture_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lz-pyo3-deleg-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&dir);
    dir
}

fn write_manifest(dir: &std::path::Path) -> PathBuf {
    let content = r#"
[bridge]
name = "testmod"
version = "0.1.0"
description = "test"

[functions]
greet = { args = "name: str", ret = "str", doc = "Greet someone" }
add = { args = "a: int, b: int", ret = "int" }
shout = { args = "s: str", ret = "None" }

[types]
Point = { fields = "x: f64, y: f64", doc = "A 2D point" }
"#;
    let path = dir.join("pyo3.toml");
    fs::write(&path, content).unwrap();
    path
}

/// 从生成模块里抽取 `#[pyfunction]` 委托函数体：
/// 只取含 `delegate to lz function __py_` 注释的函数（跳过 `#[pyclass]` 的 `new` 等），
/// 去掉属性行与 `pub` 前缀，拼成可独立 rustc 编译的核。
fn extract_delegation_core(module_src: &str) -> String {
    let mut out = String::new();
    let lines: Vec<&str> = module_src.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        let t = lines[i].trim();
        if t.starts_with("pub fn ") && t.contains("->") {
            // 探测下一个非空行是否为委托注释
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            let is_delegate = j < lines.len()
                && lines[j].contains("delegate to lz function __py_");
            if is_delegate {
                // 收集该函数体（按花括号配平）
                let mut depth = 0i32;
                let mut started = false;
                while i < lines.len() {
                    let line = lines[i];
                    out.push_str(&line.replacen("pub fn", "fn", 1));
                    out.push('\n');
                    depth += line.matches('{').count() as i32;
                    depth -= line.matches('}').count() as i32;
                    if line.contains('{') {
                        started = true;
                    }
                    i += 1;
                    if started && depth <= 0 {
                        break;
                    }
                }
                continue;
            }
        }
        i += 1;
    }
    out
}

#[test]
fn pyo3_pyfunction_delegation_is_valid_rust_and_runs() {
    let dir = fixture_dir();
    let path = write_manifest(&dir);

    // 用 crate 内的公开路径加载并生成
    let bridge = lang_zone::bridge::python::PyO3Bridge::load(&path)
        .expect("PyO3Bridge::load 应成功");
    let module_src = bridge.generate_module();

    // 1) 契约断言
    assert!(
        !module_src.contains("todo!("),
        "生成模块不得含 todo!() 桩：\n{}",
        module_src
    );
    assert!(
        module_src.contains("__py_greet(name)"),
        "greet 应委托：\n{}",
        module_src
    );
    assert!(
        module_src.contains("__py_add(a, b)"),
        "add 应委托：\n{}",
        module_src
    );
    assert!(module_src.contains("fn __py_greet(name: String) -> String;"));
    assert!(module_src.contains("fn __py_add(a: i64, b: i64) -> i64;"));

    // 2) 真编译：抽取委托核 + stub __py_ 实现 → rustc → 运行
    let core = extract_delegation_core(&module_src);
    assert!(!core.trim().is_empty(), "应抽取到委托核函数");

    let stub = r#"
fn __py_greet(name: String) -> String { format!("hi {}", name) }
fn __py_add(a: i64, b: i64) -> i64 { a + b }
fn __py_shout(_s: String) {}

fn main() {
    let g = greet("bob".to_string());
    let s = add(3, 4);
    shout("x".to_string());
    println!("{} {}", g, s);
}
"#;
    let prog = format!("{}\n{}\n", core, stub);
    let src_path = dir.join("gen_core.rs");
    fs::write(&src_path, prog).unwrap();

    let exe = dir.join("gen_core.exe");
    let out = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg(&src_path)
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("rustc 应可执行");
    assert!(
        out.status.success(),
        "生成的委托核应能编译（0 error）：\nstderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let run = Command::new(&exe).output().expect("运行生成程序");
    assert!(run.status.success(), "生成程序应可运行");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert_eq!(stdout.trim(), "hi bob 7", "运行输出应与 stub 语义一致");
}

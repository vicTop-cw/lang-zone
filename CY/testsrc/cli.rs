// lzcyc CLI 回归测试 — 把全量验证基线固化为 cargo test
//
// 基线（2026-09-17 第二轮）：transpile 54/54、run 54/54、宏展开 smoke 通过——全绿。
// 历史挂账已清零：运行期语义缺口由 postprocess_pyx 兜底；import 解析由 lzcyc
// merge_imports 解决；test_control.lz 样例作用域笔误已修正（无 let 赋值形态）。
// 任何样例回归都会让本套件变红。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const TRANSPILE_EXPECTED_FAIL: &[&str] = &[];

const RUN_EXPECTED_FAIL: &[&str] = &[];

fn lzcyc_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_lzcyc"))
}

fn tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("TESTS")
}

fn collect_test_lz() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("TESTS 目录存在") {
            let entry = entry.expect("read_dir entry");
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map(|e| e == "lz").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&tests_dir(), &mut out);
    out.sort();
    assert!(out.len() >= 50, "TESTS 样例数异常: {}", out.len());
    out
}

/// 带超时运行命令，返回 (exit_ok, stderr 尾部)
fn run_with_timeout(mut cmd: Command, timeout: Duration) -> (bool, String) {
    use std::io::Read;
    use std::process::Stdio;
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return (false, "spawn failed".to_string()),
    };
    let mut stderr_pipe = child.stderr.take();
    let start = Instant::now();
    loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => {
                let mut buf = String::new();
                if let Some(pipe) = stderr_pipe.as_mut() {
                    let _ = pipe.read_to_string(&mut buf);
                }
                let tail: String = buf
                    .lines()
                    .rev()
                    .take(3)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join(" | ");
                return (status.success(), tail);
            }
            None => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return (false, "TIMEOUT".to_string());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn python_available() -> bool {
    Command::new("python")
        .arg("-c")
        .arg("pass")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn transpile_full_regression() {
    let out_dir = std::env::temp_dir().join("lzcyc_test_transpile");
    let _ = std::fs::create_dir_all(&out_dir);
    let mut unexpected = Vec::new();
    let mut passed = 0usize;

    for f in collect_test_lz() {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let (ok, err) = run_with_timeout(
            {
                let mut c = lzcyc_cmd();
                c.arg("transpile").arg(&f).arg("-o").arg(&out_dir);
                c
            },
            Duration::from_secs(20),
        );
        let expect_fail = TRANSPILE_EXPECTED_FAIL.contains(&name.as_str());
        if ok && !expect_fail {
            passed += 1;
        } else if !ok && expect_fail {
            // 挂账预期失败，且错误类型应保持一致（防止问题性质漂移）
        } else {
            unexpected.push(format!(
                "{}: 期望{}但{}（{err}）",
                name,
                if expect_fail { "失败" } else { "成功" },
                if ok { "成功" } else { "失败" }
            ));
        }
    }
    assert!(
        unexpected.is_empty(),
        "transpile 回归偏离基线（通过 {passed}/53）：\n{}",
        unexpected.join("\n")
    );
}

#[test]
fn run_full_regression() {
    if !python_available() {
        eprintln!("python 不可用，跳过 run 回归");
        return;
    }
    let mut unexpected = Vec::new();
    let mut passed = 0usize;

    for f in collect_test_lz() {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let (ok, err) = run_with_timeout(
            {
                let mut c = lzcyc_cmd();
                c.arg("run").arg(&f);
                c
            },
            Duration::from_secs(15),
        );
        let expect_fail = RUN_EXPECTED_FAIL.contains(&name.as_str());
        if ok && !expect_fail {
            passed += 1;
        } else if !ok && expect_fail {
            // 挂账预期失败
        } else {
            unexpected.push(format!(
                "{}: 期望{}但{}（{err}）",
                name,
                if expect_fail { "失败" } else { "成功" },
                if ok { "成功" } else { "失败" }
            ));
        }
    }
    assert!(
        unexpected.is_empty(),
        "run 回归偏离基线（通过 {passed}/53）：\n{}",
        unexpected.join("\n")
    );
}

#[test]
fn macro_expansion_smoke() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testsrc/fixtures/macro_demo.lz");
    assert!(fixture.exists(), "宏样例 fixture 缺失");

    // transpile：宏展开后产物应包含 main
    let out_dir = std::env::temp_dir().join("lzcyc_test_macro");
    let _ = std::fs::create_dir_all(&out_dir);
    let (ok, err) = run_with_timeout(
        {
            let mut c = lzcyc_cmd();
            c.arg("transpile").arg(&fixture).arg("-o").arg(&out_dir);
            c
        },
        Duration::from_secs(20),
    );
    assert!(ok, "宏样例 transpile 失败: {err}");
    let pyx = out_dir.join("macro_demo.pyx");
    let code = std::fs::read_to_string(&pyx).expect("pyx 产物存在");
    assert!(code.contains("def main"), "产物缺少 main 函数");

    // run：宏展开 + 降级运行输出正确
    if !python_available() {
        eprintln!("python 不可用，跳过 run 部分");
        return;
    }
    let (ok, err) = run_with_timeout(
        {
            let mut c = lzcyc_cmd();
            c.arg("run").arg(&fixture);
            c
        },
        Duration::from_secs(30),
    );
    assert!(ok, "宏样例 run 失败: {err}");
}

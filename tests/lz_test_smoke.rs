// Lang-Zone 编译器 — tests/lz_test_smoke.rs
// 验证 `lz test`（`--test` 模式）能编译并运行含 test/suite 的 LZ 程序，
// 确认测试框架本身（gen_test_def 生成 #[test]、suite 的 setup/teardown 内联）
// 无回归。
//
// 设计：用 lang-zone 把临时 .lz 编译为 .rs 并 rustc --test 运行，
// 断言进程成功退出且打印 "All tests passed"。

use std::path::PathBuf;
use std::process::Command;

fn lang_zone() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"))
}

#[test]
fn lz_test_basic_test_block() {
    let dir = std::env::temp_dir().join("lz_test_smoke_basic");
    let _ = std::fs::create_dir_all(&dir);
    let lz = dir.join("basic.lz");
    std::fs::write(
        &lz,
        "test \"add\":\n    assert 1 + 1 == 2\ntest \"sub\":\n    assert 5 - 3 == 2\n",
    )
    .unwrap();

    let out = Command::new(lang_zone())
        .arg(&lz)
        .arg("--test")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "lz test 编译/运行失败:\n{stderr}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("All tests passed"),
        "未打印通过信息:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let _ = std::fs::remove_file(&lz);
}

#[test]
fn lz_test_suite_setup_teardown() {
    let dir = std::env::temp_dir().join("lz_test_smoke_suite");
    let _ = std::fs::create_dir_all(&dir);
    let lz = dir.join("suite.lz");
    std::fs::write(
        &lz,
        "suite Math:\n    setup:\n        base = 10\n    test \"add\":\n        assert base + 1 == 11\n    test \"sub\":\n        assert base - 1 == 9\n",
    )
    .unwrap();

    let out = Command::new(lang_zone())
        .arg(&lz)
        .arg("--test")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "lz test suite 编译/运行失败:\n{stderr}"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("All tests passed"),
        "suite 未通过:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let _ = std::fs::remove_file(&lz);
}

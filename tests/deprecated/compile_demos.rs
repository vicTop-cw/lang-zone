// Lang-Zong 编译器正面测试
// 遍历 DEMO/ 目录下所有主 .lz 文件（非 99_errors/），
// 验证编译器能正确解析和编译每个文件。

use std::path::PathBuf;
use std::fs;
use std::process::Command;

/// 递归查找 DEMO 目录下所有主 .lz 文件（排除 99_errors/）
fn find_demo_files() -> Vec<PathBuf> {
    let demo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("DEMO");
    let mut files = Vec::new();
    let mut stack = vec![demo_dir.clone()];

    while let Some(dir) = stack.pop() {
        let dir_name = dir.file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
        // 跳过错误边界目录
        // - 99_errors/ : 预期解析失败的反例
        // - 99_spec/ 目录现已全部通过，纳入测试覆盖
        if dir_name == "99_errors" {
            continue;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(it) => it,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().map_or(false, |ext| ext == "lz") {
                files.push(path);
            }
        }
    }

    files.sort();
    files
}

// TECH_DEBT: 该测试依赖 AST→RUST 代码生成路径（无 --emit=ir），违反 IR-only 技术路线约束。
// 已移至 tests/deprecated/ 并标记 ignore。见 issues/2026-08-05-tech-debt-compile-demos-ast-rust.md。
#[test]
#[ignore = "TECH_DEBT: AST→RUST 路径，违反 IR-only 约束；改用 tests/ir_snapshots.rs"]
fn all_demos_compile_successfully() {
    let files = find_demo_files();
    assert!(!files.is_empty(), "至少应找到 1 个 demo 文件");

    let mut passed = 0;
    let mut failed = Vec::new();

    for file in &files {
        let _source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                failed.push((file.clone(), format!("读取失败: {}", e)));
                continue;
            }
        };

        // 通过 CLI 二进制编译：验证 .lz 文件可成功解析为 .rs
        // 完整的 --test 编译+运行测试通道在单独测试中验证
        let bin_path = PathBuf::from(env!("CARGO_BIN_EXE_lang-zone"));
        let output = Command::new(&bin_path)
            .arg(file.as_os_str())
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    passed += 1;
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let first_line = stderr.lines().next().unwrap_or("unknown error");
                    failed.push((file.clone(), first_line.to_string()));
                }
            }
            Err(e) => {
                failed.push((file.clone(), format!("CLI 执行失败: {}", e)));
            }
        }
    }

    // 输出汇总
    let total = files.len();
    println!("\n===== DEMO 编译测试报告 =====");
    println!("  总计: {} 文件", total);
    println!("  通过: {}", passed);
    println!("  失败: {}", failed.len());

    if !failed.is_empty() {
        println!("\n  失败列表:");
        for (path, reason) in &failed {
            let rel = path.strip_prefix(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("DEMO")
            ).unwrap_or(path);
            println!("    ❌ {} — {}", rel.display(), reason);
        }
    }
    println!("==============================\n");

    assert!(failed.is_empty(), "{} 个 demo 编译失败", failed.len());
}

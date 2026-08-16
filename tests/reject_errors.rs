// Lang-Zong 编译器负面测试
// 遍历 DEMO/99_errors/ 目录下所有 .lz 文件，
// 验证编译器能正确拒绝非法代码并报告错误。

use std::path::PathBuf;
use std::fs;
use lang_zone::lexer::Lexer;
use lang_zone::parser::Parser;
use lang_zone::macros::expand::{extract_macro_defs, MacroExpander};

/// 查找 99_errors/ 目录下所有 .lz 文件
fn find_error_files() -> Vec<PathBuf> {
    let error_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("DEMO")
        .join("99_errors");

    let mut files = Vec::new();
    if !error_dir.exists() {
        return files;
    }

    for entry in fs::read_dir(&error_dir).unwrap() {
        if let Ok(entry) = entry {
            if entry.path().extension().map_or(false, |ext| ext == "lz") {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    files
}

#[test]
fn error_boundaries_are_rejected() {
    let files = find_error_files();
    assert!(!files.is_empty(), "至少应找到 1 个错误边界文件");

    let mut rejected = 0;
    let mut unexpectedly_passed = Vec::new();

    for file in &files {
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                // 文件读取失败本身不是编译器的职责
                eprintln!("  ⚠️  读取失败: {} — {}", file.display(), e);
                continue;
            }
        };

        // 跳过纯注释文件（没有实际代码可以测试编译）
        let non_comment = source.lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with("//"))
            .count();

        if non_comment == 0 {
            eprintln!("  ⏭️  跳过纯注释文件: {}", file.display());
            continue;
        }

        // 使用 lexer + parser 尝试解析源码，预期应该失败
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize();
        
        let parse_result = extract_macro_defs(&tokens)
            .map_err(|e| format!("{e}"))
            .and_then(|(registry, _)| {
                let expander = MacroExpander::new(registry);
                let expanded = expander.expand(&tokens).map_err(|e| format!("{e}"))?;
                let mut parser = Parser::new(expanded);
                parser.parse_module().map_err(|e| format!("{e}"))
            });
        
        match parse_result {
            Err(e) => {
                // 期望行为：编译失败，记录错误信息
                rejected += 1;
                let rel = file.strip_prefix(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("DEMO")
                        .join("99_errors")
                ).unwrap_or(file);
                eprintln!("  ✅ 正确拒绝: {} — {}", rel.display(), e.trim());
            }
            Ok(_) => {
                // 不应该编译通过——如果通过了，说明边界测试需要更新
                unexpectedly_passed.push(file.clone());
            }
        }
    }

    let total = files.len();
    println!("\n===== 错误边界测试报告 =====");
    println!("  总计: {} 文件", total);
    println!("  正确拒绝: {}", rejected);
    println!("  意外通过: {}", unexpectedly_passed.len());

    if !unexpectedly_passed.is_empty() {
        println!("\n  意外通过（需要更新边界测试）:");
        for path in &unexpectedly_passed {
            let rel = path.strip_prefix(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("DEMO")
                    .join("99_errors")
            ).unwrap_or(path);
            println!("    ⚠️  {} — 编译器未拒绝此非法代码", rel.display());
        }
    }
    println!("============================\n");

    assert!(
        unexpectedly_passed.is_empty(),
        "{} 个边界测试意外通过了编译",
        unexpectedly_passed.len()
    );
}

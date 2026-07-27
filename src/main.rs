// Lang-Zong 编译器 — CLI 入口
// 用法: lzc hello.lz [--tokens] [--ast] [--std-dir <path>] [--allow-rustc-private]  → hello.rs

use std::env;
use std::fs;
use std::path::PathBuf;
use lang_zong::lexer::Lexer;
use lang_zong::parser::Parser;
use lang_zong::codegen::CodeGen;
use lang_zong::macros::expand::{extract_macro_defs, MacroExpander};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: lzc <file.lz> [--tokens] [--ast] [--std-dir <path>] [--allow-rustc-private]");
        std::process::exit(1);
    }

    let path = &args[1];
    let source = fs::read_to_string(path)
        .unwrap_or_else(|e| {
            eprintln!("Error reading {}: {}", path, e);
            std::process::exit(1);
        });

    // CLI 标志解析
    let std_dir = extract_flag_value(&args, "--std-dir").map(PathBuf::from);
    let allow_rustc_private = args.iter().any(|a| a == "--allow-rustc-private");

    // 获取 rustc 版本（供 Tier-2 门控校验）
    let rustc_version = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize();

    if args.iter().any(|a| a == "--tokens") {
        println!("=== Tokens ===");
        for (i, t) in tokens.iter().enumerate() {
            println!("{:4} {:?}", i, t);
        }
        return;
    }

    // ── 宏展开（Phase 2: Lexer 之后、Parser 之前） ──
    // 第一遍：提取宏定义并构建注册中心
    let (registry, macro_ranges) = match extract_macro_defs(&tokens) {
        Ok((r, ranges)) => (r, ranges),
        Err(e) => {
            eprintln!("Macro definition error: {}", e);
            std::process::exit(1);
        }
    };

    // 从 Token 流中移除宏定义（展开后不再需要）
    let expander = MacroExpander::new(registry);
    let mut expand_input: Vec<_> = tokens.iter().enumerate()
        .filter(|(i, _)| {
            // 过滤掉宏定义占用的 token
            let mut skip = false;
            for chunk in macro_ranges.chunks(2) {
                if chunk.len() == 2 && *i >= chunk[0] && *i < chunk[1] {
                    skip = true;
                    break;
                }
            }
            !skip
        })
        .map(|(_, t)| t.clone())
        .collect();

    // 第二遍：展开所有 @name! 宏调用
    let expanded_tokens = match expander.expand(&expand_input) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Macro expansion error: {}", e);
            std::process::exit(1);
        }
    };

    if args.iter().any(|a| a == "--dump-macros") {
        println!("=== Expanded Tokens ===");
        for (i, t) in expanded_tokens.iter().enumerate() {
            println!("{:4} {:?}", i, t);
        }
        return;
    }

    // Parse（使用展开后的 Token 流）
    let mut parser = Parser::new(expanded_tokens);
    let module = match parser.parse_module() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };

    if args.iter().any(|a| a == "--ast") {
        println!("{:#?}", module);
        return;
    }

    // Codegen
    let rust_code = CodeGen::generate(&module, std_dir, allow_rustc_private, rustc_version);

    // Output
    let out_path = path.replace(".lz", ".rs");
    fs::write(&out_path, &rust_code).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {}", out_path, e);
        std::process::exit(1);
    });

    println!("Generated {} -> {}", path, out_path);
}

/// 从 CLI 参数中提取标志值（如 --std-dir <path>）
fn extract_flag_value(args: &[String], flag: &str) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == flag && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}

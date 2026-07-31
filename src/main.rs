// Lang-Zong 编译器 — CLI 入口
// 用法: lzc hello.lz [--tokens] [--ast] [--std-dir <path>] [--allow-rustc-private]  → hello.rs

use std::env;
use std::fs;
use std::path::PathBuf;
use lang_zone::lexer::Lexer;
use lang_zone::parser::Parser;
use lang_zone::codegen::CodeGen;
use lang_zone::macros::expand::{extract_macro_defs, MacroExpander};
use lang_zone::ir::builder::build_ir;
use lang_zone::ir::codegen::CodeGen as IrCodeGen;
use lang_zone::project::ProjectCompiler;
use lang_zone::cache::{CacheEntry, content_hash};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: lzc <file.lz> [--tokens] [--ast] [--emit=ir] [--ir-codegen] [--test] [--project] [--std-dir <path>] [--allow-rustc-private]");
        std::process::exit(1);
    }

    let should_use_project = args.iter().any(|a| a == "--project");

    // CLI 标志解析（提前，供 project 和单文件两种模式共用）
    let std_dir = extract_flag_value(&args, "--std-dir").map(PathBuf::from);
    let allow_rustc_private = args.iter().any(|a| a == "--allow-rustc-private");
    let run_tests = args.iter().any(|a| a == "--test");
    let use_cache = args.iter().any(|a| a == "--cached");

    // 获取 rustc 版本
    let rustc_version = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let path = &args[1];

    // --project: 递归加载 import 的所有 .lz 依赖，合并编译
    if should_use_project {
        let base_dir = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new("."));
        let mut pc = ProjectCompiler::new(base_dir.to_path_buf(), std_dir.clone());
        let merged = pc.compile(std::path::Path::new(path))
            .unwrap_or_else(|e| {
                eprintln!("Project compile error: {}", e);
                std::process::exit(1);
            });

        let rust_code = CodeGen::generate(&merged, std_dir, allow_rustc_private, rustc_version);
        let out_path = path.replace(".lz", ".rs");
        fs::write(&out_path, &rust_code).unwrap_or_else(|e| {
            eprintln!("Error writing {}: {}", out_path, e);
            std::process::exit(1);
        });
        println!("Generated {} -> {} (project mode, {} modules)", path, out_path, pc.unit_count());
        return;
    }

    // --cached: 跳过未修改文件
    if use_cache {
        let src_path = std::path::Path::new(path);
        let cache_dir = PathBuf::from(".lzcache");
        if let Ok(Some(entry)) = CacheEntry::load(&cache_dir, src_path) {
            if entry.is_fresh(src_path, &cache_dir) {
                println!("Cached (unchanged): {}", path);
                return;
            }
        }
        // 不新鲜则继续编译；编译成功后在末尾保存缓存
    }

    let source = fs::read_to_string(path)
        .unwrap_or_else(|e| {
            eprintln!("Error reading {}: {}", path, e);
            std::process::exit(1);
        });

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

    // --emit=ir: 输出 LZIR 中间表示并生成 .rs
    let use_ir_codegen = args.iter().any(|a| a == "--emit=ir" || a == "--ir-codegen");
    if args.iter().any(|a| a == "--emit=ir") {
        match build_ir(&module) {
            Ok(ir_module) => {
                println!("{ir_module}");
                // ALSO generate .rs via IR codegen
                let mut cg = IrCodeGen::new();
                let rust_code = cg.generate(&ir_module);
                let out_path = path.replace(".lz", ".rs");
                fs::write(&out_path, &rust_code).unwrap_or_else(|e| {
                    eprintln!("Error writing {}: {}", out_path, e);
                    std::process::exit(1);
                });
                println!("Generated {} -> {} (IR codegen)", path, out_path);
                return;
            }
            Err(e) => {
                eprintln!("IR emission error: {e}");
                std::process::exit(1);
            }
        }
    }
    if use_ir_codegen {
        // --ir-codegen (without display)
        match build_ir(&module) {
            Ok(ir_module) => {
                let mut cg = IrCodeGen::new();
                let rust_code = cg.generate(&ir_module);
                let out_path = path.replace(".lz", ".rs");
                fs::write(&out_path, &rust_code).unwrap_or_else(|e| {
                    eprintln!("Error writing {}: {}", out_path, e);
                    std::process::exit(1);
                });
                println!("Generated {} -> {} (IR codegen)", path, out_path);
                return;
            }
            Err(e) => {
                eprintln!("IR build error: {e}");
                std::process::exit(1);
            }
        }
    }

    // Codegen (old AST path)
    let rust_code = CodeGen::generate(&module, std_dir, allow_rustc_private, rustc_version);

    // Output
    let out_path = path.replace(".lz", ".rs");
    fs::write(&out_path, &rust_code).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {}", out_path, e);
        std::process::exit(1);
    });

    println!("Generated {} -> {}", path, out_path);

    // 编译成功后保存缓存
    if use_cache {
        let hash = content_hash(std::path::Path::new(path)).unwrap_or_default();
        let mut entry = CacheEntry::default();
        entry.hash = hash;
        let _ = entry.save(&PathBuf::from(".lzcache"), std::path::Path::new(path));
    }

    // --test: 编译并运行测试
    if run_tests {
        let out_name = path.replace(".lz", "");
        let test_bin = format!("{}_test", out_name);
        let status = std::process::Command::new("rustc")
            .arg("--test")
            .arg(&out_path)
            .arg("-o")
            .arg(&test_bin)
            .status();

        match status {
            Ok(s) if s.success() => {
                let run = std::process::Command::new(&test_bin).status();
                match run {
                    Ok(s) if s.success() => {
                        println!("✅ All tests passed");
                    }
                    _ => {
                        eprintln!("❌ Some tests failed");
                        std::process::exit(1);
                    }
                }
                let _ = std::fs::remove_file(&test_bin);
            }
            _ => {
                eprintln!("Test compilation failed");
                std::process::exit(1);
            }
        }
    }
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

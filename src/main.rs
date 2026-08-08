// Lang-Zong 编译器 — CLI 入口
// 用法: lzc hello.lz [--tokens] [--ast] [--std-dir <path>] [--allow-rustc-private]  → hello.rs

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use lang_zone::lexer::Lexer;
use lang_zone::parser::Parser;
use lang_zone::macros::expand::{extract_macro_defs, MacroExpander};
use lang_zone::ir::builder::build_ir;
use lang_zone::ir::codegen::CodeGen as IrCodeGen;
use lang_zone::project::ProjectCompiler;
use lang_zone::cache::CacheEntry;

/// 将 .lz 扩展名替换为 .rs（只替换最后的扩展名，避免 `a.lz.lz` → `a.rs.rs` 问题）
fn replace_ext(path: &str, from: &str, to: &str) -> String {
    let p = Path::new(path);
    if let Some(stem) = p.file_stem() {
        let parent = p.parent().unwrap_or(Path::new(""));
        let new_name = format!("{}{}", stem.to_string_lossy(), to);
        if parent.as_os_str().is_empty() {
            new_name
        } else {
            format!("{}/{}", parent.display(), new_name)
        }
    } else {
        // fallback: just replace
        path.replace(from, to)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: lang-zone <file.lz> [--tokens] [--ast] [--emit=ir] [--test] [--project] [--std-dir <path>] [--allow-rustc-private]");
        std::process::exit(1);
    }

    let should_use_project = args.iter().any(|a| a == "--project");

    // CLI 标志解析（提前，供 project 和单文件两种模式共用）
    let std_dir = extract_flag_value(&args, "--std-dir").map(PathBuf::from);
    let run_tests = args.iter().any(|a| a == "--test");
    let use_cache = args.iter().any(|a| a == "--cached");

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

        let (rust_code, label) = match build_ir(&merged) {
            Ok(ir_module) => {
                let mut cg = IrCodeGen::new();
                (cg.generate(&ir_module), "IR codegen")
            }
            Err(e) => {
                eprintln!("IR build error (project mode): {}", e);
                std::process::exit(1);
            }
        };
        let out_path = replace_ext(path, ".lz", ".rs");
        fs::write(&out_path, &rust_code).unwrap_or_else(|e| {
            eprintln!("Error writing {}: {}", out_path, e);
            std::process::exit(1);
        });
        println!("Generated {} -> {} (project mode, {}, {} modules)", path, out_path, label, pc.unit_count());
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
    let mut registry = match extract_macro_defs(&tokens) {
        Ok((r, _ranges)) => r,
        Err(e) => {
            eprintln!("Macro definition error: {}", e);
            std::process::exit(1);
        }
    };

    // 跨模块宏导入：`import macro X` / `from macro X import Y` → 读取 X.lz 并合并其宏定义
    let dir = std::path::Path::new(path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut i = 0usize;
    while i < tokens.len() {
        let is_import = tokens[i] == lang_zone::lexer::Token::Import
            || tokens[i] == lang_zone::lexer::Token::From;
        if is_import {
            // import macro X / from macro X import Y
            let mut j = i + 1;
            let mut is_macro_import = false;
            let mut mod_name: Option<String> = None;
            while j < tokens.len() {
                match &tokens[j] {
                    lang_zone::lexer::Token::Macro => {
                        is_macro_import = true;
                        j += 1;
                    }
                    lang_zone::lexer::Token::Ident(n) if mod_name.is_none() => {
                        mod_name = Some(n.clone());
                        j += 1;
                    }
                    lang_zone::lexer::Token::Newline | lang_zone::lexer::Token::Dedent => break,
                    _ => {
                        if tokens[j] == lang_zone::lexer::Token::Import {
                            j += 1;
                        } else if tokens[j] == lang_zone::lexer::Token::As {
                            j += 2;
                        } else {
                            break;
                        }
                    }
                }
                if tokens[j] == lang_zone::lexer::Token::Newline {
                    break;
                }
            }
            if is_macro_import {
                if let Some(mname) = mod_name {
                    let macro_path = dir.join(format!("{}.lz", mname));
                    if let Ok(src) = std::fs::read_to_string(&macro_path) {
                        let mut mlexer = lang_zone::lexer::Lexer::new(&src);
                        let mtokens = mlexer.tokenize();
                        if let Ok((mreg, _mranges)) = extract_macro_defs(&mtokens) {
                            registry.merge(mreg);
                        }
                    }
                }
            }
            // 跳到本行末尾
            while i < tokens.len() && tokens[i] != lang_zone::lexer::Token::Newline {
                i += 1;
            }
        }
        i += 1;
    }
    let macro_ranges = Vec::<usize>::new();

    // 从 Token 流中移除宏定义（展开后不再需要）
    let expander = MacroExpander::new(registry);
    let expand_input: Vec<_> = tokens.iter().enumerate()
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

    // --emit=ir: 输出 LZIR 中间表示文本（不生成 .rs）
    if args.iter().any(|a| a == "--emit=ir") {
        match build_ir(&module) {
            Ok(ir_module) => {
                println!("{ir_module}");
                return;
            }
            Err(e) => {
                eprintln!("IR emission error: {e}");
                std::process::exit(1);
            }
        }
    }

    // 唯一 codegen 路径: AST → LZIR → Rust（无老路子选项）
    match build_ir(&module) {
        Ok(ir_module) => {
            let mut cg = IrCodeGen::new();
            let rust_code = cg.generate(&ir_module);
            let out_path = replace_ext(path, ".lz", ".rs");
            fs::write(&out_path, &rust_code).unwrap_or_else(|e| {
                eprintln!("Error writing {}: {}", out_path, e);
                std::process::exit(1);
            });
            println!("Generated {} -> {} (IR codegen)", path, out_path);

            // --test: 编译并运行测试（IR 路径）
            if run_tests {
                run_test_mode(path, &out_path);
            }
        }
        Err(e) => {
            eprintln!("IR build error: {e}");
            std::process::exit(1);
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

/// 编译生成的 .rs 文件为测试二进制并运行
fn run_test_mode(source_path: &str, out_path: &str) {
    let out_name = replace_ext(source_path, ".lz", "");
    #[cfg(target_os = "windows")]
    let test_bin = format!("{}_test.exe", out_name);
    #[cfg(not(target_os = "windows"))]
    let test_bin = format!("{}_test", out_name);
    let status = std::process::Command::new("rustc")
        .arg("--test")
        .arg(out_path)
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

// Lang-Zong 编译器 — CLI 入口
// 用法: lzc hello.lz [--tokens] [--ast] [--std-dir <path>] [--allow-rustc-private]  → hello.rs

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use lang_zone::lexer::Lexer;
use lang_zone::parser::Parser;
use lang_zone::macros::expand::{contains_pending_call, extract_macro_defs, extract_template_defs, has_bin_macro_declaration, MacroExpander, TemplateExpander};
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

    // --macro-check=loose|light|strict：宏/template 逐层检查模式
    // （08 §3.6 规则 4）——loose=只最后完整检查；light（默认）=逐层轻量结构
    // 校验 + 最终 Parser 兜底；strict=每层完整 Parser（中间层产物须独立合法）
    let macro_check_mode = match args.iter().find_map(|a| a.strip_prefix("--macro-check=")) {
        Some("loose") => lang_zone::macros::expand::CheckMode::Loose,
        Some("strict") => lang_zone::macros::expand::CheckMode::Strict,
        Some("light") | _ => lang_zone::macros::expand::CheckMode::Light,
    };

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
    // 第一遍：提取宏定义并构建注册中心（ranges 记录宏定义占用 token，用于从流中移除）
    let mut macro_ranges_from_extract: Vec<usize> = Vec::new();
    let mut template_ranges_from_extract: Vec<usize> = Vec::new();
    let mut registry = match extract_macro_defs(&tokens) {
        Ok((r, ranges)) => {
            macro_ranges_from_extract = ranges;
            r
        }
        Err(e) => {
            eprintln!("Macro definition error: {}", e);
            std::process::exit(1);
        }
    };
    // 提取 template 定义（返回 Tokens 的编译期函数，调用 `name!`）
    let mut template_registry = match extract_template_defs(&tokens) {
        Ok((r, ranges)) => {
            template_ranges_from_extract = ranges;
            r
        }
        Err(e) => {
            eprintln!("Template definition error: {}", e);
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
                        if let Ok((treg, _tranges)) = extract_template_defs(&mtokens) {
                            template_registry.merge(treg);
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
    // 从 Token 流中移除宏定义（展开后不再需要；模板定义同样由 expander 处理）
    // 合并宏定义 + 模板定义占用范围
    macro_ranges_from_extract.extend(template_ranges_from_extract);
    let macro_ranges = macro_ranges_from_extract;

    // 从 Token 流中移除宏定义（展开后不再需要）
    let mut expander = MacroExpander::new(registry);
    expander.set_check_mode(macro_check_mode);
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
    // 混合嵌套交替展开循环（08 §3.6）：宏→模板→宏→…直到稳定。
    // 宏产物可含 name!（模板调用）、模板产物可含 @name!（宏调用），
    // 单次宏展开 + 单次模板展开无法覆盖双向混合嵌套，需交替直到无变化。
    let mut template_expander = TemplateExpander::new(template_registry);
    template_expander.set_check_mode(macro_check_mode);
    let mut expanded_tokens = expander.expand(&expand_input).unwrap_or_else(|e| {
        eprintln!("Macro expansion error: {}", e);
        std::process::exit(1);
    });
    let max_passes = 16;
    let mut stable = false;
    for _pass in 0..max_passes {
        let before = expanded_tokens.clone();
        // 模板展开（name!，内部递归处理模板→模板嵌套）
        let after_tpl = match template_expander.expand(&expanded_tokens) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Template expansion error: {}", e);
                std::process::exit(1);
            }
        };
        // 宏展开（@name!，内部递归处理宏→宏嵌套，也展开模板产物中的 @name!）
        let after_mac = match expander.expand(&after_tpl) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Macro expansion error: {}", e);
                std::process::exit(1);
            }
        };
        expanded_tokens = after_mac;
        // 稳定条件：该轮无任何新展开 **且无未展开的嵌套调用残留**——
        // 宏↔模板无限交替时 token 流往返可能相同（如 `// m\nloop_tpl!(x)`
        // ↔ `// t\n@gen_tpl!(x)` 回到同形），仅比较相等会误判稳定并残留调用
        if expanded_tokens == before && !contains_pending_call(&expanded_tokens) {
            stable = true;
            break;
        }
    }
    if !stable {
        eprintln!(
            "Macro/template 交替展开未稳定（可能循环嵌套，超过 {} 轮）",
            max_passes
        );
        std::process::exit(1);
    }

    if args.iter().any(|a| a == "--dump-macros") {
        println!("=== Expanded Tokens ===");
        for (i, t) in expanded_tokens.iter().enumerate() {
            println!("{:4} {:?}", i, t);
        }
        return;
    }

    // Parse（使用展开后的 Token 流）
    let mut parser = Parser::new(expanded_tokens);
    // 宏模块检测：展开已消费 #!bin macro 声明，用原始 token 流（展开前）检测
    // （lexer 对 `#!bin macro` 整行产生单个 Token::Macro + Newline）
    parser.is_macro = has_bin_macro_declaration(&tokens);
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

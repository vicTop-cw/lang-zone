// Lang-Zong 编译器 — CLI 入口
// 用法: lzc hello.lz [--tokens] [--ast] [--emit=ir] [--emit=lex-lz] [--emit=parse-lz] [--std-dir <path>] [--allow-rustc-private]  → hello.rs
// 子命令: lang-zone create|build|check|peek|push → src/cli.rs

mod cli;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use lang_zone::lexer::Lexer;
use lang_zone::parser::Parser;
use lang_zone::macros::expand::{contains_pending_call, extract_macro_defs, extract_template_defs, has_bin_macro_declaration, MacroExpander, TemplateExpander};
use lang_zone::ir::builder::build_ir;
use lang_zone::ir::codegen::CodeGen as IrCodeGen;
use lang_zone::project::ProjectCompiler;
use lang_zone::cache::CacheEntry;
use lang_zone::incr::IncrCompiler;

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

// ── 自举路线 B：LZ 写的前端（src/frontend/*.lz）接入主流程 ──
// --emit=lex-lz / --emit=parse-lz：用 LZ 实现的词法/语法前端处理输入文件，
// 走与 --emit=ir-lz 相同的 lzc→rustc→run 递归管线，输出可与 Rust 版基线 diff。
const LZ_LEXER_LIB: &str = include_str!("frontend/lz_lexer.lz");
const LZ_PARSER_LIB: &str = include_str!("frontend/lz_parser.lz");

/// 剥离 .lz 前端源里自带的 main（保留库代码），由 wrapper main 接管输入
fn lz_frontend_lib_only(lib: &str) -> &str {
    match lib.rfind("\ndef main() =") {
        Some(idx) => &lib[..idx],
        None => {
            eprintln!("LZ frontend source missing `def main() =` marker");
            std::process::exit(1);
        }
    }
}

/// 转义待注入的 .lz 源码（保持单行字符串字面量，避免破坏缩进结构）
fn escape_lz_wrapper(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// 组装 wrapper LZ 源码并递归编译运行（lexer 输出 token 流；parser 输出语句描述）
fn run_lz_frontend(path: &str, source: &str, mode: &str) {
    let (lib_only, body) = match mode {
        "lex" => (
            lz_frontend_lib_only(LZ_LEXER_LIB),
            "    let toks = tokenize(src)\n    for idx in 0..toks.len():\n        print(display_token(toks[idx]))\n",
        ),
        "parse" => (
            lz_frontend_lib_only(LZ_PARSER_LIB),
            "    let toks = tokenize(src)\n    let r = parse_program(toks, 0)\n    let stmts = r.0\n    for idx in 0..stmts.len():\n        print(stmts[idx])\n",
        ),
        _ => unreachable!(),
    };
    let wrapper = format!(
        "// 由 lzc --emit={}-lz 生成（自举路线 B：LZ 写的前端处理输入文件）\n{}\n\ndef main() =\n    let src = \"{}\"\n{}\n",
        mode, lib_only, escape_lz_wrapper(source), body
    );
    let lz_path = replace_ext(path, ".lz", ".lzfront");
    fs::write(&lz_path, &wrapper).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {}", lz_path, e);
        std::process::exit(1);
    });
    eprintln!("Generated {} -> {} (LZ {} frontend)", path, lz_path, mode);

    // 与 --emit=ir-lz 相同的递归管线：lang-zone → .rs → rustc → exe → stdout
    let self_exe = std::env::current_exe().unwrap_or_default();
    let build = Command::new(&self_exe)
        .arg(&lz_path)
        .output()
        .expect("run lang-zone on generated LZ frontend");
    if !build.status.success() {
        eprintln!(
            "LZ {} frontend 编译失败: {}",
            mode,
            String::from_utf8_lossy(&build.stderr)
        );
        std::process::exit(1);
    }
    let rs_path = replace_ext(&lz_path, ".lzfront", ".rs");
    let builtins = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug/liblz_builtins.rlib");
    let exe_path = replace_ext(&lz_path, ".lzfront", ".exe");
    let rc = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs_path)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-o")
        .arg(&exe_path)
        .output()
        .expect("run rustc on generated .rs");
    if !rc.status.success() {
        eprintln!(
            "LZ {} frontend rustc 失败: {}",
            mode,
            String::from_utf8_lossy(&rc.stderr)
        );
        std::process::exit(1);
    }
    let run = Command::new(&exe_path).output().expect("run generated exe");
    if !run.status.success() {
        eprintln!(
            "LZ {} frontend 运行失败: {}",
            mode,
            String::from_utf8_lossy(&run.stderr)
        );
        std::process::exit(1);
    }
    print!("{}", String::from_utf8_lossy(&run.stdout));
}

// Windows 主线程栈默认仅 1MB（链接器默认），深层递归下降（宏展开、嵌套缩进块
// 解析、深层嵌套表达式 codegen）会栈溢出（p43 复现：thread 'main' has
// overflowed its stack）。将整个编译流水线移入 512MB 大栈线程，解除该限制。
fn main() {
    let args: Vec<String> = env::args().collect();
    let code = std::thread::Builder::new()
        .name("lang-zone-compile".to_string())
        .stack_size(512 * 1024 * 1024)
        .spawn(move || compile_main(args))
        .expect("failed to spawn compile thread")
        .join()
        .unwrap_or(1);
    std::process::exit(code);
}

/// 实际编译流水线（在 main 的大栈线程中执行）；返回进程退出码
fn compile_main(args: Vec<String>) -> i32 {
    // ── 子命令分派：args[1] 是 create/build/check/peek/push 或
    // -h/--help/--version 时走子命令路径；否则保持原单文件编译路径不变 ──
    if args.len() >= 2 {
        match args[1].as_str() {
            "-h" | "--help" => {
                cli::print_help();
                return 0;
            }
            "--version" => {
                println!("Lang-Zone compiler (lz) {}", lang_zone::util::version::version());
                return 0;
            }
            "create" => return cli::cmd_create(&args),
            "build" => return cli::cmd_build(&args),
            "check" => return cli::cmd_check(&args),
            "peek" => return cli::cmd_peek(&args),
            "push" => return cli::cmd_push(&args),
            // 方案.md G5：调用台账审计入口（只读，不依赖编译）
            "emit-bridge-report" => return cli::cmd_emit_bridge_report(&args),
            // FIST T4.5 / 升级计划第4章：热重载（方向C）与 LSP（方向D）
            "watch" => return lang_zone::hotreload::cmd_watch(&args),
            "lsp" => return lang_zone::lsp::run_lsp(),
            _ => { /* 单文件编译路径（行为保持原样） */ }
        }
    }

    if args.len() < 2 {
        eprintln!("Usage: lang-zone <file.lz> [--tokens] [--ast] [--emit=ir] [--emit=lex-lz] [--emit=parse-lz] [--test] [--project] [--std-dir <path>] [--allow-rustc-private]");
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
    // 旧 AST 直接 codegen 回退开关（仅用于双路线 golden 对照；默认 IR 路线）
    let use_ast_codegen = args.iter().any(|a| a == "--ast-codegen");
    let allow_rustc_private = args.iter().any(|a| a == "--allow-rustc-private");
    // --backend=cython：选择 Cython 后端（默认 Rust）
    let backend_cython = args.iter().any(|a| a == "--backend=cython");

    // --lzi <file>：加载 lz-infer 生成的跨模块类型签名（.lzi），注入 IR builder，
    // 本地函数查不到返回类型时回退查询外部模块签名（可选增强，infer 特性门控）
    #[cfg(feature = "infer")]
    let lzi_registry = extract_flag_value(&args, "--lzi").map(|p| {
        let reg = lang_zone::infer::LziRegistry::load_single(Path::new(&p))
            .unwrap_or_else(|e| {
                eprintln!("lzi load error ({}): {}", p, e);
                std::process::exit(1);
            });
        std::rc::Rc::new(reg)
    });
    // 非 infer 构建：无 .lzi 支持，占位（build_ir_opt 非 infer 版本忽略该参数）
    #[cfg(not(feature = "infer"))]
    let lzi_registry: Option<()> = None;

    // entry 路径取第一个非 `-` 开头的参数（兼容 `--incr <file>` 与 `<file> --incr` 两种顺序）
    let path = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .unwrap_or(&args[1]);

    // --incr：增量编译（FIST T4.3 / 升级计划方向B）
    // 模块级缓存 + 依赖图 + 传播失效；未变更模块直接复用缓存产物。
    if args.iter().any(|a| a == "--incr") {
        let base_dir = std::path::Path::new(path)
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let cache_dir = args
            .iter()
            .find_map(|a| a.strip_prefix("--incr-cache="))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(IncrCompiler::default_cache_dir()));
        let mut ic = IncrCompiler::new(base_dir, cache_dir);
        let outcome = ic
            .compile(std::path::Path::new(path))
            .unwrap_or_else(|e| {
                eprintln!("Incremental compile error: {}", e);
                std::process::exit(1);
            });
        let out_path = replace_ext(path, ".lz", ".rs");
        fs::write(&out_path, &outcome.code).unwrap_or_else(|e| {
            eprintln!("Error writing {}: {}", out_path, e);
            std::process::exit(1);
        });
        println!(
            "Incremental: {} -> {} ({} modules: {} cached, {} rebuilt, {} ms)",
            path,
            out_path,
            outcome.stats.total,
            outcome.stats.hits,
            outcome.stats.misses,
            outcome.stats.elapsed_ms
        );
        return 0;
    }

    // --project: 递归加载 import 的所有 .lz 依赖，合并编译
    if should_use_project {
        let base_dir = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new("."));
        let mut pc = ProjectCompiler::new(base_dir.to_path_buf(), std_dir.clone());
        let mut merged = pc.compile(std::path::Path::new(path))
            .unwrap_or_else(|e| {
                eprintln!("Project compile error: {}", e);
                std::process::exit(1);
            });
        // 项目模式合并模块也需带 file_path：语义检查 import 同目录解析依赖它
        if merged.file_path.is_none() {
            merged.file_path = Some(path.to_string());
        }

        let (rust_code, label) = if use_ast_codegen {
            // 旧 AST 直接 codegen（仅双路线 golden 对照用；默认已退役）
            let rustc_version = lang_zone::util::version::version();
            (
                lang_zone::codegen::CodeGen::generate(
                    &merged,
                    std_dir.clone(),
                    allow_rustc_private,
                    rustc_version,
                ),
                "AST codegen (legacy)",
            )
        } else {
            match build_ir_opt(&merged, lzi_registry.as_ref()) {
                Ok(ir_module) => {
                    let mut cg = IrCodeGen::new();
                    (cg.generate(&ir_module), "IR codegen")
                }
                Err(e) => {
                    eprintln!("IR build error (project mode): {}", e);
                    std::process::exit(1);
                }
            }
        };
        let out_path = replace_ext(path, ".lz", ".rs");
        fs::write(&out_path, &rust_code).unwrap_or_else(|e| {
            eprintln!("Error writing {}: {}", out_path, e);
            std::process::exit(1);
        });
        println!("Generated {} -> {} (project mode, {}, {} modules)", path, out_path, label, pc.unit_count());
        return 0;
    }

    // --cached: 跳过未修改文件
    if use_cache {
        let src_path = std::path::Path::new(path);
        let cache_dir = PathBuf::from(".lzcache");
        if let Ok(Some(entry)) = CacheEntry::load(&cache_dir, src_path) {
            if entry.is_fresh(src_path, &cache_dir) {
                println!("Cached (unchanged): {}", path);
                return 0;
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
        return 0;
    }

    // ── 宏展开（Phase 2: Lexer 之后、Parser 之前） ──
    // 第一遍：提取宏定义并构建注册中心（ranges 记录宏定义占用 token，用于从流中移除）
    let mut macro_ranges_from_extract: Vec<usize>;
    let template_ranges_from_extract: Vec<usize>;
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
        return 0;
    }

    // Parse（使用展开后的 Token 流）
    let mut parser = Parser::new(expanded_tokens);
    // 宏模块检测：展开已消费 #!bin macro 声明，用原始 token 流（展开前）检测
    // （lexer 对 `#!bin macro` 整行产生单个 Token::Macro + Newline）
    parser.is_macro = has_bin_macro_declaration(&tokens);
    let mut module = match parser.parse_module() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };
    // 模块级魔法属性 __file__/__package__/__path__ 的数据源（06e §一）：
    // parser 不感知文件路径，编译入口（main.rs）在此注入 .lz 源文件路径
    module.file_path = Some(path.to_string());
    // comptime inspect.getsource/getsourcelines 的源码数据源（08b §6）：
    // 注入源文件文本，供 ComptimeContext.with_source 使用
    module.source_text = Some(source.clone());

    if args.iter().any(|a| a == "--ast") {
        println!("{:#?}", module);
        return 0;
    }

    // --emit=ir: 输出 LZIR 中间表示文本（不生成 .rs）
    if args.iter().any(|a| a == "--emit=ir") {
        match build_ir_opt(&module, lzi_registry.as_ref()) {
            Ok(ir_module) => {
                println!("{ir_module}");
                return 0;
            }
            Err(e) => {
                eprintln!("IR emission error: {e}");
                std::process::exit(1);
            }
        }
    }

    // --emit=ir-lz: 自举路线 B —— 生成 LZ 构造代码（lz_ir_lib.lz 库 + main 构造），
    // 经 lang-zone → rustc → 运行 输出 IR 文本（与 --emit=ir 逐字符一致）。
    // 即「用 LZ 实现 IR display」：Rust 编译器只序列化 IR 数据为 LZ 调用，
    // display 逻辑完全由 LZ 侧承担（bootstrap/work/lz_ir 试点落地）。
    if args.iter().any(|a| a == "--emit=ir-lz") {
        match build_ir_opt(&module, lzi_registry.as_ref()) {
            Ok(ir_module) => {
                let lz_source =
                    lang_zone::ir::lz_codegen::ir_module_to_lz_source(&ir_module);
                let lz_path = replace_ext(path, ".lz", ".lzlz");
                fs::write(&lz_path, &lz_source).unwrap_or_else(|e| {
                    eprintln!("Error writing {}: {}", lz_path, e);
                    std::process::exit(1);
                });
                eprintln!("Generated {} -> {} (LZ IR codegen)", path, lz_path);
                // 递归编译运行：lang-zone <lz_path> → .rs → rustc → exe → stdout
                let self_exe = std::env::current_exe().unwrap_or_default();
                let build = Command::new(&self_exe)
                    .arg(&lz_path)
                    .output()
                    .expect("run lang-zone on generated LZ");
                if !build.status.success() {
                    eprintln!(
                        "LZ IR codegen 编译失败: {}",
                        String::from_utf8_lossy(&build.stderr)
                    );
                    std::process::exit(1);
                }
                let rs_path = replace_ext(&lz_path, ".lzlz", ".rs");
                let builtins = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("target/debug/liblz_builtins.rlib");
                let exe_path = replace_ext(&lz_path, ".lzlz", ".exe");
                let rc = Command::new("rustc")
                    .args(["--edition", "2021"])
                    .arg(&rs_path)
                    .arg("--extern")
                    .arg(format!("lz_builtins={}", builtins.display()))
                    .arg("-o")
                    .arg(&exe_path)
                    .output()
                    .expect("run rustc on generated .rs");
                if !rc.status.success() {
                    eprintln!(
                        "LZ IR codegen rustc 失败: {}",
                        String::from_utf8_lossy(&rc.stderr)
                    );
                    std::process::exit(1);
                }
                let run = Command::new(&exe_path)
                    .output()
                    .expect("run generated exe");
                if !run.status.success() {
                    eprintln!("LZ IR codegen 运行失败: {}", String::from_utf8_lossy(&run.stderr));
                    std::process::exit(1);
                }
                print!("{}", String::from_utf8_lossy(&run.stdout));
                return 0;
            }
            Err(e) => {
                eprintln!("IR emission error (ir-lz): {e}");
                std::process::exit(1);
            }
        }
    }

    // --emit=rs-lz: 自举路线 B（D1）—— 生成 LZ codegen 构造代码
    // （lz_codegen_lib.lz 库 + main 构造），经 lang-zone → rustc → 运行
    // 输出 Rust 源码（与常规 IR codegen 路径生成的 .rs 逐字符一致）。
    // 即「用 LZ 实现 Rust codegen」：Rust 编译器只序列化 IR 数据为 LZ 调用，
    // codegen 逻辑完全由 LZ 侧承担（bootstrap/work/lz_codegen 试点落地）。
    if args.iter().any(|a| a == "--emit=rs-lz") {
        match build_ir_opt(&module, lzi_registry.as_ref()) {
            Ok(ir_module) => {
                let lz_source =
                    lang_zone::ir::lz_codegen::ir_module_to_rs_lz_source(&ir_module);
                let lz_path = replace_ext(path, ".lz", ".lzrs");
                fs::write(&lz_path, &lz_source).unwrap_or_else(|e| {
                    eprintln!("Error writing {}: {}", lz_path, e);
                    std::process::exit(1);
                });
                eprintln!("Generated {} -> {} (LZ Rust codegen)", path, lz_path);
                // 递归编译运行：lang-zone <lz_path> → .rs → rustc → exe → stdout
                let self_exe = std::env::current_exe().unwrap_or_default();
                let build = Command::new(&self_exe)
                    .arg(&lz_path)
                    .output()
                    .expect("run lang-zone on generated LZ");
                if !build.status.success() {
                    eprintln!(
                        "LZ Rust codegen 编译失败: {}",
                        String::from_utf8_lossy(&build.stderr)
                    );
                    std::process::exit(1);
                }
                let rs_path = replace_ext(&lz_path, ".lzrs", ".rs");
                let builtins = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("target/debug/liblz_builtins.rlib");
                let exe_path = replace_ext(&lz_path, ".lzrs", ".exe");
                let rc = Command::new("rustc")
                    .args(["--edition", "2021"])
                    .arg(&rs_path)
                    .arg("--extern")
                    .arg(format!("lz_builtins={}", builtins.display()))
                    .arg("-o")
                    .arg(&exe_path)
                    .output()
                    .expect("run rustc on generated .rs");
                if !rc.status.success() {
                    eprintln!(
                        "LZ Rust codegen rustc 失败: {}",
                        String::from_utf8_lossy(&rc.stderr)
                    );
                    std::process::exit(1);
                }
                let run = Command::new(&exe_path)
                    .output()
                    .expect("run generated exe");
                if !run.status.success() {
                    eprintln!("LZ Rust codegen 运行失败: {}", String::from_utf8_lossy(&run.stderr));
                    std::process::exit(1);
                }
                print!("{}", String::from_utf8_lossy(&run.stdout));
                return 0;
            }
            Err(e) => {
                eprintln!("IR emission error (rs-lz): {e}");
                std::process::exit(1);
            }
        }
    }

    // --emit=lex-lz / --emit=parse-lz: 自举路线 B —— 用 LZ 写的前端处理输入文件
    //（src/frontend/lz_lexer.lz / lz_parser.lz），走与 ir-lz 相同的递归管线，
    // 输出与 Rust 版基线（--tokens / --ast）可 diff。
    if args.iter().any(|a| a == "--emit=lex-lz") {
        run_lz_frontend(path, &source, "lex");
        return 0;
    }
    if args.iter().any(|a| a == "--emit=parse-lz") {
        run_lz_frontend(path, &source, "parse");
        return 0;
    }

    // 默认 codegen 路径: AST → LZIR → Rust（IR 路线；AST 直接 codegen 已退役，
    // 仅 --ast-codegen 回退开关保留用于双路线 golden 对照）
    if use_ast_codegen {
        let rustc_version = lang_zone::util::version::version();
        let rust_code = lang_zone::codegen::CodeGen::generate(
            &module,
            std_dir.clone(),
            allow_rustc_private,
            rustc_version,
        );
        let out_path = replace_ext(path, ".lz", ".rs");
        fs::write(&out_path, &rust_code).unwrap_or_else(|e| {
            eprintln!("Error writing {}: {}", out_path, e);
            std::process::exit(1);
        });
        println!("Generated {} -> {} (AST codegen, legacy)", path, out_path);
        return 0;
    }
    match build_ir_opt(&module, lzi_registry.as_ref()) {
        Ok(ir_module) => {
            if backend_cython {
                // ── Cython 后端：IR → .pyx ──
                let mut cg = lang_zone::ir::codegen_cython::CythonCodeGen::new();
                let pyx_code = cg.generate(&ir_module);
                let out_path = replace_ext(path, ".lz", ".pyx");
                fs::write(&out_path, pyx_code).unwrap_or_else(|e| {
                    eprintln!("Error writing {}: {}", out_path, e);
                    std::process::exit(1);
                });
                println!("Generated {} -> {} (Cython backend)", path, out_path);
            } else {
                // ── Rust 后端（默认）──
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
        }
        Err(e) => {
            eprintln!("IR build error: {e}");
            std::process::exit(1);
        }
    }
    0
}

/// 带可选 .lzi 跨模块签名的 build_ir 分发：有 registry 走 build_ir_with_lzi，
/// 否则默认入口（本地函数查不到返回类型时回退查询外部模块签名）
/// 统一入口：先做 G2 语义检查（semantic_check），再进入 IR 构建。
#[cfg(feature = "infer")]
fn build_ir_opt(
    module: &lang_zone::ast::Module,
    lzi: Option<&std::rc::Rc<lang_zone::infer::LziRegistry>>,
) -> Result<lang_zone::ir::IrModule, lang_zone::ir::builder::IrBuildError> {
    let errs = lang_zone::semantic_check::check_module(module);
    if !errs.is_empty() {
        return Err(lang_zone::ir::builder::IrBuildError::Generic(errs.join("\n")));
    }
    match lzi {
        Some(reg) => lang_zone::ir::builder::build_ir_with_lzi(module, reg.clone()),
        None => build_ir(module),
    }
}

/// 非 infer 构建：无 .lzi 支持，直接走默认 build_ir（忽略 lzi 占位参数）
/// 统一入口：先做 G2 语义检查（semantic_check），再进入 IR 构建。
#[cfg(not(feature = "infer"))]
fn build_ir_opt(
    module: &lang_zone::ast::Module,
    _lzi: Option<&()>,
) -> Result<lang_zone::ir::IrModule, lang_zone::ir::builder::IrBuildError> {
    let errs = lang_zone::semantic_check::check_module(module);
    if !errs.is_empty() {
        return Err(lang_zone::ir::builder::IrBuildError::Generic(errs.join("\n")));
    }
    build_ir(module)
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

//! lz-infer CLI 入口
//!
//! 用法：
//!   lz-infer <src> [--output signatures.lzi]
//!   lz-infer <src> -o signatures.lzi
//!   lz-infer <src> --no-cross-module   (回退到逐文件独立推断)

use std::env;
use std::fs;
use std::path::PathBuf;

use lz_infer::infer::{infer_path, infer_path_cross_module};

fn print_usage() {
    eprintln!("Usage: lz-infer <src> [--output <file>]");
    eprintln!("       lz-infer <src> -o <file>");
    eprintln!("       lz-infer <src> --no-cross-module  (legacy single-file mode)");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let input = PathBuf::from(&args[1]);

    let mut output = PathBuf::from("signatures.lzi");
    let mut no_cross_module = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: missing value for {}", args[i]);
                    print_usage();
                    std::process::exit(1);
                }
                output = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--no-cross-module" => {
                no_cross_module = true;
                i += 1;
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                eprintln!("Error: unknown option '{}'", other);
                print_usage();
                std::process::exit(1);
            }
        }
    }

    let file = if no_cross_module {
        match infer_path(&input) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Inference failed: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match infer_path_cross_module(&input) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Inference failed: {}", e);
                std::process::exit(1);
            }
        }
    };

    let json = match file.to_json() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Serialization failed: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = fs::write(&output, json) {
        eprintln!("Failed to write '{}': {}", output.display(), e);
        std::process::exit(1);
    }

    let module_count = file.modules.len();
    let function_count: usize = file.modules.values().map(|m| m.functions.len()).sum();
    let struct_count: usize = file.modules.values().map(|m| m.structs.len()).sum();
    let unresolved_count = file.unresolved.len();
    // 统计跨模块注记数量
    let cross_module_markers: usize = file.unresolved.iter()
        .filter(|s| s.starts_with("[cross_module]"))
        .count();

    println!(
        "Generated {} with {} modules, {} functions, {} structs; {} unresolved ({} cross-module).",
        output.display(),
        module_count,
        function_count,
        struct_count,
        unresolved_count,
        cross_module_markers,
    );

    if unresolved_count > 0 {
        for msg in &file.unresolved {
            eprintln!("  unresolved: {}", msg);
        }
        std::process::exit(unresolved_count.min(255) as i32);
    }
}

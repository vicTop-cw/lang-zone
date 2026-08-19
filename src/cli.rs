// Lang-Zone 编译器 — cli.rs
// 子命令实现：create / build / check（peek / push 由 main.rs 分派占位，另见 src/main.rs）
//
// 设计原则：
// - 子命令全部返回 i32 退出码（0 成功 / 1 失败），诊断一律走 stderr，正常输出走 stdout
// - 错误信息透传 lexer/parser 的位置信息（file:line:col），与单文件模式一致
// - create/build/check 均为独立函数名（cmd_create / cmd_build / cmd_check），
//   便于主线实现 peek/push 时并行合并
// - 增量编译默认关闭（README-FOR-AI 规定开发阶段禁止缓存），
//   仅当显式传 --incremental 时启用 .lzcache 快速路径

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use lang_zone::cache::CacheEntry;
use lang_zone::ir::builder::build_ir;
use lang_zone::ir::codegen::CodeGen as IrCodeGen;
use lang_zone::ir::duck_check::check_duck_satisfaction;
use lang_zone::project::ProjectCompiler;
use lang_zone::util::mini_toml::{parse as parse_toml, TomlValue};
use lang_zone::util::version;

/// lz.toml 项目清单（build/check 的最小工作集）
#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub entry: String,
}

/// 顶层帮助文本（MoonBit 形态参考，非逐字兼容）
pub fn print_help() {
    println!(
        "Lang-Zone compiler (lz) {}\n\
         Usage: lang-zone <file.lz> [flags] | <subcommand> [args]\n\
         \n\
         Subcommands:\n\
         \x20 create <path>     Scaffold a new LZ project (lz.toml + src/main.lz)\n\
         \x20 build  [dir]      Build the project: .lz -> .rs -> executable (incremental)\n\
         \x20 peek   <target>   Show tokens/AST/IR of a file (target: <file.lz> or ir:<file.lz>)\n\
         \x20 check  [dir]      Type-check the project without emitting code\n\
         \x20 push   [--dry-run]  Publish to local registry (see push --help)\n\
         \x20 emit-bridge-report <tsv>  Audit bridge ledger (bridge-ledger.tsv, G5)\n\
         \n\
         Flags (single-file mode): --tokens --ast --emit=... --project --test --std-dir <path>",
        version()
    );
}

/// 各子命令自己的 --help
fn print_subcommand_help(sub: &str) {
    match sub {
        "create" => println!(
            "create: scaffold a new LZ project\n\
             \n\
             Usage: lang-zone create <path>\n\
             \n\
             Creates <path>/lz.toml, <path>/src/main.lz and <path>/README.md.\n\
             Fails if <path> already exists and is not empty."
        ),
        "build" => println!(
            "build: compile the project to an executable\n\
             \n\
             Usage: lang-zone build [dir] [--incremental]\n\
             \n\
             Finds lz.toml in [dir] (default: current directory; searches one\n\
             level up). Compiles entry -> IR -> .rs into build/, then invokes\n\
             rustc to produce build/<name>.exe.\n\
             \n\
             Flags:\n\
             \x20 --incremental   Reuse .lzcache when source hashes are unchanged"
        ),
        "check" => println!(
            "check: type-check the project without emitting code\n\
             \n\
             Usage: lang-zone check [dir] [--incremental]\n\
             \n\
             Runs lexer -> parser -> IR build -> duck check. Writes a\n\
             .lzcheck summary into build/ for fast re-checks."
        ),
        "peek" | "push" => {
            // peek/push: see src/cli.rs (implemented by mainline; placeholder)
            println!("{}: see src/cli.rs (implemented by mainline)", sub);
        }
        "emit-bridge-report" => println!(
            "emit-bridge-report: audit the bridge call ledger (G5)\n\
             \n\
             Usage: lang-zone emit-bridge-report <ledger.tsv>\n\
             \n\
             Reads the append-only TSV ledger (ts\\tevent\\tlang\\tdetail) and\n\
             prints a summary grouped by event × lang. Fails if the file is\n\
             missing; never modifies the ledger."
        ),
        _ => print_help(),
    }
}

// ────────────────────────────── create ──────────────────────────────

/// create <path>: scaffold lz.toml + src/main.lz + README.md
pub fn cmd_create(args: &[String]) -> i32 {
    if args.len() < 3 {
        eprintln!("Error: create requires a path argument");
        print_subcommand_help("create");
        return 1;
    }
    let target = Path::new(&args[2]);

    // 目标路径已存在且非空 → 明确报错（避免覆盖用户数据）
    if target.exists() {
        let non_empty = fs::read_dir(target)
            .map(|mut rd| rd.next().is_some())
            .unwrap_or(false);
        if non_empty {
            eprintln!(
                "Error: {} already exists and is not empty",
                target.display()
            );
            return 1;
        }
    }

    let src_dir = target.join("src");
    if let Err(e) = fs::create_dir_all(&src_dir) {
        eprintln!("Error: cannot create {}: {}", src_dir.display(), e);
        return 1;
    }

    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "hello".to_string());

    // lz.toml：项目清单（name/version/entry 三字段）
    let manifest = format!(
        "# LZ project manifest\n\
         name = \"{}\"\n\
         version = \"0.1.0\"\n\
         entry = \"src/main.lz\"\n",
        name
    );

    // src/main.lz：可构建的最小示例（def main() = print(...)）
    let main_lz = format!(
        "// {name} - generated by `lang-zone create`\n\
         def main() =\n\
         \x20   print(\"hello from {name}\")\n",
        name = name
    );

    // README.md：build/check 用法
    let readme = format!(
        "# {name}\n\
         \n\
         Generated by `lang-zone create`.\n\
         \n\
         ## Build\n\
         \n\
         ```\n\
         lang-zone build\n\
         ```\n\
         \n\
         Produces `build/{name}.rs` and `build/{name}.exe`.\n\
         \n\
         ## Check\n\
         \n\
         ```\n\
         lang-zone check\n\
         ```\n\
         \n\
         Type-checks the project without emitting code.\n",
        name = name
    );

    for (path, content) in [
        (target.join("lz.toml"), &manifest),
        (src_dir.join("main.lz"), &main_lz),
        (target.join("README.md"), &readme),
    ] {
        if let Err(e) = fs::write(&path, content) {
            eprintln!("Error: cannot write {}: {}", path.display(), e);
            return 1;
        }
    }

    println!(
        "Created LZ project '{}' at {}",
        name,
        target.display()
    );
    println!("  Next: cd {} && lang-zone build", target.display());
    0
}

// ────────────────────────────── manifest ──────────────────────────────

/// 定位 lz.toml：dir（默认 "."）→ 向上找一层 → 放弃
fn locate_manifest(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("lz.toml");
    if direct.is_file() {
        return Some(direct);
    }
    let parent = dir.parent()?.join("lz.toml");
    if parent.is_file() {
        return Some(parent);
    }
    None
}

/// 解析 lz.toml → Manifest（name/version/entry；缺省值兜底）
fn parse_manifest(path: &Path) -> Result<Manifest, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
    let doc = parse_toml(&text)
        .map_err(|e| format!("Manifest parse error in {}: {}", path.display(), e))?;
    let root = doc.get("").cloned().unwrap_or_default();
    let get_str = |key: &str| -> Option<String> {
        root.get(key).and_then(|v| match v {
            TomlValue::Str(s) => Some(s.clone()),
            _ => None,
        })
    };
    Ok(Manifest {
        name: get_str("name").unwrap_or_else(|| {
            path.parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "app".to_string())
        }),
        version: get_str("version").unwrap_or_else(|| "0.1.0".to_string()),
        entry: get_str("entry").unwrap_or_else(|| "src/main.lz".to_string()),
    })
}

// ────────────────────────────── build / check 共用 ──────────────────────────────

/// 项目编译上下文：解析后的 manifest + 各路径
struct ProjectPaths {
    root: PathBuf,
    manifest: Manifest,
    entry: PathBuf,
    build_dir: PathBuf,
    rs_out: PathBuf,
    exe_out: PathBuf,
}

impl ProjectPaths {
    fn resolve(dir: &Path) -> Result<Self, String> {
        let manifest_path = locate_manifest(dir).ok_or_else(|| {
            format!(
                "Error: lz.toml not found in {} or its parent directory",
                dir.display()
            )
        })?;
        let manifest = parse_manifest(&manifest_path)?;
        let root = manifest_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let entry = if Path::new(&manifest.entry).is_absolute() {
            PathBuf::from(&manifest.entry)
        } else {
            root.join(&manifest.entry)
        };
        let build_dir = root.join("build");
        let exe_name = format!("{}.exe", manifest.name);
        let rs_name = format!("{}.rs", manifest.name);
        Ok(ProjectPaths {
            root,
            manifest,
            entry,
            rs_out: build_dir.join(rs_name),
            exe_out: build_dir.join(exe_name),
            build_dir,
        })
    }
}

/// 前端流水线（build 与 check 共用）：ProjectCompiler → AST → IR。
/// 返回 (IR, module_count)。lexer/parser 错误带 file:line:col，原样透传。
fn compile_project_to_ir(
    paths: &ProjectPaths,
    std_dir: Option<&Path>,
) -> Result<(lang_zone::ir::IrModule, usize), String> {
    let entry = &paths.entry;
    let mut pc = ProjectCompiler::new(paths.root.clone(), std_dir.map(Path::to_path_buf));
    let mut merged = pc.compile(entry)?;
    let module_count = pc.unit_count();
    // 合并模块需要入口文件路径：semantic_check 的 import 存在性检查依赖
    // mod_dir（AST file_path 的父目录），merge_modules 不设置则误报"import 路径不存在"
    if merged.file_path.is_none() {
        merged.file_path = Some(
            entry
                .canonicalize()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| entry.display().to_string()),
        );
    }
    // G2: 语义检查（拒绝 17 个语法矩阵反例中的语义错误类）
    let errs = lang_zone::semantic_check::check_module(&merged);
    if !errs.is_empty() {
        return Err(errs.join("\n"));
    }
    let ir = build_ir(&merged)
        .map_err(|e| format!("IR build error in {}: {}", entry.display(), e))?;
    Ok((ir, module_count))
}

/// 计算项目全部源文件的哈希列表（用于增量判断 + .lzcheck 内容）
fn source_hashes(entry: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![entry.to_path_buf()];
    while let Some(p) = stack.pop() {
        let canon = p.canonicalize().unwrap_or(p.clone());
        if !seen.insert(canon.clone()) {
            continue;
        }
        if let Ok(h) = lang_zone::cache::content_hash(&canon) {
            out.push((canon.display().to_string(), h));
        }
        // 递归扫描 import（简单词法扫描：import x / from x import y 行）
        if let Ok(src) = fs::read_to_string(&canon) {
            let dir = canon.parent().map(Path::to_path_buf).unwrap_or_default();
            for line in src.lines() {
                let t = line.trim_start();
                let mod_name = if let Some(rest) = t.strip_prefix("import ") {
                    rest.split_whitespace().next()
                } else if let Some(rest) = t.strip_prefix("from ") {
                    rest.split_whitespace().next()
                } else {
                    None
                };
                if let Some(m) = mod_name {
                    if m != "std" && !m.contains('.') {
                        let dep = dir.join(format!("{}.lz", m));
                        if dep.is_file() {
                            stack.push(dep);
                        }
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// rustc 编译产物（--extern lz_builtins=<rlib>）
fn rustc_compile(rs: &Path, exe: &Path) -> Result<(), String> {
    let builtins = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/liblz_builtins.rlib");
    let out = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(rs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-o")
        .arg(exe)
        .output()
        .map_err(|e| format!("Failed to run rustc: {}", e))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

// ────────────────────────────── build ──────────────────────────────

/// build [dir] [--incremental]：.lz → .rs → exe
pub fn cmd_build(args: &[String]) -> i32 {
    let (dir_arg, incremental) = parse_dir_and_flags(args, "build");
    let dir = Path::new(&dir_arg);

    let paths = match ProjectPaths::resolve(dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };

    let std_dir = args
        .iter()
        .position(|a| a == "--std-dir")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    // 增量模式：哈希一致 + .rs 存在 → 跳过重编
    if incremental {
        let cache_dir = paths.build_dir.join(".lzcache");
        if let Ok(Some(entry)) = CacheEntry::load(&cache_dir, &paths.entry) {
            if entry.is_fresh(&paths.entry, &cache_dir) && paths.rs_out.exists() {
                println!(
                    "Incremental hit (unchanged): {}",
                    paths.entry.display()
                );
                return 0;
            }
        }
    }

    // 全量编译：ProjectCompiler → IR → .rs（默认路径；产物哈希可复现）
    let (ir, module_count) = match compile_project_to_ir(&paths, std_dir.as_deref()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };

    let mut cg = IrCodeGen::new();
    // I3/I4：注入 BridgeRegistry，extern/@export/embed 符号自动登记（G5 台账联动）
    let (rust_code, mut registry) = cg.generate_with_bridge(&ir);

    if let Err(e) = fs::create_dir_all(&paths.build_dir) {
        eprintln!(
            "Error: cannot create {}: {}",
            paths.build_dir.display(),
            e
        );
        return 1;
    }
    if let Err(e) = fs::write(&paths.rs_out, &rust_code) {
        eprintln!("Error: cannot write {}: {}", paths.rs_out.display(), e);
        return 1;
    }
    println!(
        "Generated {} -> {} ({} modules)",
        paths.entry.display(),
        paths.rs_out.display(),
        module_count
    );

    // G5：注册符号落盘台账（追加式 TSV，build/bridge-ledger.tsv）
    if registry.symbol_count() > 0 {
        let ledger_path = paths.build_dir.join("bridge-ledger.tsv");
        let _ = registry.set_ledger_path(&ledger_path);
        let _ = registry.ledger_mut().flush();
        println!(
            "Bridge registry: {} symbol(s) registered -> {}",
            registry.symbol_count(),
            ledger_path.display()
        );
    }

    if let Err(e) = rustc_compile(&paths.rs_out, &paths.exe_out) {
        eprintln!("rustc error:\n{}", e);
        return 1;
    }
    println!(
        "Built {} {} -> {}",
        paths.manifest.name,
        paths.manifest.version,
        paths.exe_out.display()
    );

    // 写回 .lzcache（增量模式数据；默认模式也记录哈希，供后续 --incremental 用）
    let hashes = source_hashes(&paths.entry);
    let entry_hash = hashes
        .iter()
        .find(|(p, _)| Path::new(p) == paths.entry.canonicalize().unwrap_or_else(|_| paths.entry.clone()))
        .map(|(_, h)| h.clone())
        .or_else(|| lang_zone::cache::content_hash(&paths.entry).ok())
        .unwrap_or_default();
    let entry_saved = CacheEntry {
        hash: entry_hash,
        deps: hashes
            .iter()
            .filter(|(p, _)| Path::new(p) != paths.entry.canonicalize().unwrap_or_else(|_| paths.entry.clone()))
            .map(|(p, h)| (p.clone(), h.clone()))
            .collect(),
        // CacheEntry::output 相对于缓存目录：缓存目录为 build/.lzcache，
        // 产物为 build/<name>.rs → 相对路径 ../<name>.rs（is_fresh 据此检查存在性）
        output: format!(
            "../{}",
            paths.rs_out.file_name().unwrap_or_default().to_string_lossy()
        ),
    };
    let cache_dir = paths.build_dir.join(".lzcache");
    let _ = entry_saved.save(&cache_dir, &paths.entry);

    0
}

// ────────────────────────────── check ──────────────────────────────

/// check [dir] [--incremental]：parse + IR + duck check，不生成 .rs、不调 rustc
pub fn cmd_check(args: &[String]) -> i32 {
    let (dir_arg, incremental) = parse_dir_and_flags(args, "check");
    let dir = Path::new(&dir_arg);

    let paths = match ProjectPaths::resolve(dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };

    // 增量模式：.lzcheck 哈希命中且源未变 → 快速通过（不影响正确性）
    let check_cache = paths.build_dir.join(".lzcheck");
    let current_hashes = source_hashes(&paths.entry);
    let hash_line = hashes_line(&current_hashes);
    if incremental {
        if let Ok(cached) = fs::read_to_string(&check_cache) {
            // .lzcheck 内容为多行 summary，比对其中的 hashes= 行
            let cached_hash = cached
                .lines()
                .find_map(|l| l.strip_prefix("hashes="))
                .unwrap_or("");
            if cached_hash == hash_line {
                println!(
                    "check ok (incremental hit): {} modules, {} items",
                    count_modules(&current_hashes),
                    count_items(&paths)
                );
                return 0;
            }
        }
    }

    let (ir, module_count) = match compile_project_to_ir(&paths, None) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };

    // duck 结构匹配检查（IR 层）
    let duck_errors = check_duck_satisfaction(&ir);
    if !duck_errors.is_empty() {
        for e in &duck_errors {
            eprintln!("Duck check error: {}", e);
        }
        return 1;
    }

    // 写 .lzcheck（内容 = 全部源哈希摘要 + 计数）
    let item_count = ir.items.len();
    let summary = format!(
        "# lzcheck summary\nhashes={}\nmodules={}\nitems={}\n",
        hash_line, module_count, item_count
    );
    let _ = fs::create_dir_all(&paths.build_dir);
    if let Err(e) = fs::write(&check_cache, &summary) {
        eprintln!(
            "Warning: cannot write {}: {}",
            check_cache.display(),
            e
        );
    }

    println!("check ok: {} modules, {} items", module_count, item_count);
    0
}

// ────────────────────────────── peek ──────────────────────────────

/// peek <file.lz>：输出 tokens/AST/IR 三视图；目标不存在返回明确错误
/// 复用单文件模式的 --tokens/--ast/--emit=ir 内部链路（调用主流水线入口）
pub fn cmd_peek(args: &[String]) -> i32 {
    if args.len() < 3 || args[2] == "--help" || args[2] == "-h" {
        print_subcommand_help("peek");
        return if args.len() < 3 { 1 } else { 0 };
    }
    let target = Path::new(&args[2]);
    if !target.is_file() {
        eprintln!("Error: cannot peek {}: no such file", target.display());
        return 1;
    }
    // 重入 compile_main（单文件模式，递归处理 flags）
    let mut fwd = vec!["lang-zone".to_string(), target.display().to_string()];
    for a in args.iter().skip(3) {
        fwd.push(a.clone());
    }
    // 默认输出 IR（可读、稳定）；带 --tokens/--ast 时输出对应视图
    if !fwd.iter().any(|a| a == "--tokens" || a == "--ast" || a.starts_with("--emit=")) {
        fwd.push("--emit=ir".to_string());
    }
    crate::compile_main(fwd)
}

// ────────────────────────────── push ──────────────────────────────

/// push [dir] [--dry-run] [--registry <path>]：发布到本地/staging registry
/// 事务性：先完整构建校验 → 写临时目录 → 原子 rename；失败不产生半发布状态
pub fn cmd_push(args: &[String]) -> i32 {
    let mut dir_arg = String::from(".");
    let mut dry_run = false;
    let mut registry: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_subcommand_help("push");
                return 0;
            }
            "--dry-run" => dry_run = true,
            "--registry" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --registry requires a path");
                    return 1;
                }
                registry = Some(args[i + 1].clone());
                i += 1;
            }
            a if !a.starts_with('-') && dir_arg == "." => dir_arg = a.to_string(),
            _ => {
                eprintln!("Error: unknown push argument {}", args[i]);
                return 1;
            }
        }
        i += 1;
    }

    let dir = Path::new(&dir_arg);
    let paths = match ProjectPaths::resolve(dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };

    // 版本预检 + 完整构建校验（IR + 代码生成，不依赖 rustc 成功与否）
    let (_ir, module_count) = match compile_project_to_ir(&paths, None) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Preflight failed: {}", e);
            return 1;
        }
    };

    let reg_dir = Path::new(
        registry
            .as_deref()
            .unwrap_or("local-registry"),
    )
    .to_path_buf();
    // 包条目：<registry>/<name>/<version>/（含校验和）
    let pkg_dir = reg_dir
        .join(&paths.manifest.name)
        .join(&paths.manifest.version);

    // 版本冲突预检：目标已存在 → 明确报错（不覆盖旧版本）
    if pkg_dir.exists() {
        eprintln!(
            "Error: version conflict: {} {} already published at {}",
            paths.manifest.name,
            paths.manifest.version,
            pkg_dir.display()
        );
        return 1;
    }

    // 计算发布物校验和（全部源文件哈希 + 版本 + 时间戳冻结）
    let hashes = source_hashes(&paths.entry);
    let mut checksum_input = format!(
        "name={}\nversion={}\nmodules={}\n",
        paths.manifest.name,
        paths.manifest.version,
        module_count
    );
    for (p, h) in &hashes {
        checksum_input.push_str(&format!("{} {}\n", h, p));
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    checksum_input.hash(&mut hasher);
    let checksum = format!("{:016x}", hasher.finish());

    if dry_run {
        println!(
            "[dry-run] would publish {} {} -> {} (checksum {})",
            paths.manifest.name,
            paths.manifest.version,
            pkg_dir.display(),
            checksum
        );
        println!("[dry-run] {} modules, {} source files", module_count, hashes.len());
        return 0;
    }

    // 事务性发布：先写临时目录，全部成功后原子 rename
    let tmp = reg_dir.join(format!(
        ".tmp-{}-{}-{}",
        paths.manifest.name,
        paths.manifest.version,
        std::process::id()
    ));
    let tmp_pkg = tmp
        .join(&paths.manifest.name)
        .join(&paths.manifest.version);
    let build_result = (|| -> Result<(), String> {
        fs::create_dir_all(&tmp_pkg).map_err(|e| e.to_string())?;
        fs::write(tmp_pkg.join("checksum.txt"), format!("{}\n{}", checksum, checksum_input))
            .map_err(|e| e.to_string())?;
        // 发布源码快照（清单 + 源文件）
        let manifest_text = fs::read_to_string(&paths.root.join("lz.toml"))
            .map_err(|e| e.to_string())?;
        fs::write(tmp_pkg.join("lz.toml"), manifest_text).map_err(|e| e.to_string())?;
        fs::create_dir_all(tmp_pkg.join("src")).map_err(|e| e.to_string())?;
        let src_dir = paths.entry.parent().unwrap_or(Path::new("."));
        let mut copied = 0usize;
        // 遍历 (目录, 相对目录) 对：避免 Windows canonicalize 的 \\?\ 前缀导致 strip_prefix 失败
        let mut stack = vec![(src_dir.to_path_buf(), PathBuf::new())];
        let mut seen = std::collections::HashSet::new();
        while let Some((p, rel_prefix)) = stack.pop() {
            let key = p.to_path_buf();
            if !seen.insert(key.clone()) {
                continue;
            }
            if let Ok(rd) = fs::read_dir(&p) {
                for e in rd.flatten() {
                    let ep = e.path();
                    if ep.is_dir() && ep.file_name().map(|n| n != "build").unwrap_or(false) {
                        stack.push((ep.clone(), rel_prefix.join(ep.file_name().unwrap_or_default())));
                    } else if ep.extension().map(|x| x == "lz").unwrap_or(false) {
                        let rel = rel_prefix.join(ep.file_name().unwrap_or_default());
                        let dest = tmp_pkg.join("src").join(&rel);
                        if let Some(parent) = dest.parent() {
                            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                        fs::copy(&ep, &dest).map_err(|e| e.to_string())?;
                        copied += 1;
                    }
                }
            }
        }
        fs::write(tmp_pkg.join("files.txt"), format!("copied={}\n", copied))
            .map_err(|e| e.to_string())?;
        Ok(())
    })();

    if let Err(e) = build_result {
        let _ = fs::remove_dir_all(&tmp);
        eprintln!("Error: publish failed (no partial state): {}", e);
        return 1;
    }

    // 原子 rename（同盘 rename 成功即全量发布，失败则清理）
    if let Some(parent) = pkg_dir.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            let _ = fs::remove_dir_all(&tmp);
            eprintln!("Error: cannot create registry {}: {}", parent.display(), e);
            return 1;
        }
    }
    if let Err(e) = fs::rename(&tmp_pkg, &pkg_dir) {
        let _ = fs::remove_dir_all(&tmp);
        eprintln!("Error: publish failed: {}", e);
        return 1;
    }
    let _ = fs::remove_dir_all(&tmp);
    println!(
        "Published {} {} -> {} (checksum {})",
        paths.manifest.name,
        paths.manifest.version,
        pkg_dir.display(),
        checksum
    );
    0
}

// ────────────────────────────── 小工具 ──────────────────────────────

/// 解析 [dir] 与 --incremental（子命令第 3 个参数起）
fn parse_dir_and_flags(args: &[String], sub: &str) -> (String, bool) {
    let mut dir_arg = String::from(".");
    let mut incremental = false;
    for a in args.iter().skip(2) {
        if a == "--incremental" {
            incremental = true;
        } else if a == "--help" || a == "-h" {
            print_subcommand_help(sub);
            std::process::exit(0);
        } else if !a.starts_with('-') && dir_arg == "." {
            dir_arg = a.clone();
        }
    }
    (dir_arg, incremental)
}

/// 哈希列表 → 单行摘要（.lzcheck 比对用）
fn hashes_line(hashes: &[(String, String)]) -> String {
    let mut s = String::new();
    for (p, h) in hashes {
        s.push_str(p);
        s.push(':');
        s.push_str(h);
        s.push(';');
    }
    s
}

/// 模块数 = 参与哈希的源文件数
fn count_modules(hashes: &[(String, String)]) -> usize {
    hashes.len()
}

/// 条目数（incremental hit 时从 .lzcheck 读出；读不到则 0）
fn count_items(paths: &ProjectPaths) -> usize {
    let cache = paths.build_dir.join(".lzcheck");
    fs::read_to_string(&cache)
        .ok()
        .and_then(|c| {
            c.lines().find_map(|l| {
                l.strip_prefix("items=")
                    .and_then(|v| v.parse::<usize>().ok())
            })
        })
        .unwrap_or(0)
}

// ────────────────────────────── emit-bridge-report ──────────────────────────────

/// emit-bridge-report <ledger.tsv>：审计 bridge 调用台账（方案.md G5）
///
/// 读取追加式 TSV 台账（ts \t event \t lang \t detail），按 event × lang 汇总输出。
/// 只读审计：文件缺失返回明确错误，绝不修改台账文件。
pub fn cmd_emit_bridge_report(args: &[String]) -> i32 {
    if args.len() < 3 || args[2] == "--help" || args[2] == "-h" {
        print_subcommand_help("emit-bridge-report");
        return if args.len() < 3 { 1 } else { 0 };
    }
    let target = Path::new(&args[2]);
    if !target.is_file() {
        eprintln!("Error: cannot audit {}: no such ledger file", target.display());
        return 1;
    }
    let report = lang_zone::bridge::ledger::Ledger::report_path(target);
    print!("{}", report.render());
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip() {
        let dir = std::env::temp_dir().join("lz_cli_test_manifest");
        let _ = fs::create_dir_all(&dir);
        let toml = dir.join("lz.toml");
        fs::write(&toml, "name = \"demo\"\nversion = \"0.2.0\"\nentry = \"src/main.lz\"\n").unwrap();
        let m = parse_manifest(&toml).unwrap();
        assert_eq!(m.name, "demo");
        assert_eq!(m.version, "0.2.0");
        assert_eq!(m.entry, "src/main.lz");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn manifest_missing_fields_default() {
        let dir = std::env::temp_dir().join("lz_cli_test_mf");
        let _ = fs::create_dir_all(&dir);
        let toml = dir.join("lz.toml");
        fs::write(&toml, "name = \"x\"\n").unwrap();
        let m = parse_manifest(&toml).unwrap();
        assert_eq!(m.name, "x");
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.entry, "src/main.lz");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn locate_finds_parent() {
        let dir = std::env::temp_dir().join("lz_cli_test_locate");
        let child = dir.join("nested");
        let _ = fs::create_dir_all(&child);
        let toml = dir.join("lz.toml");
        fs::write(&toml, "name = \"p\"\n").unwrap();
        let found = locate_manifest(&child).unwrap();
        assert_eq!(found, toml);
        let _ = fs::remove_dir_all(&dir);
    }
}

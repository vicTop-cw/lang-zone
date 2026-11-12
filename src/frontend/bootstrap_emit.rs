// Lang-Zone 编译器 — frontend/bootstrap_emit.rs
// M5 自举组件接入（T0r56.1.1）：把 main.rs 的自举路线 B 机器下沉到 lib crate，
// 让 lzcyc（CY/）能通过 `lang_zone::frontend::bootstrap_emit::*` 复用同一套
// 自举管线（车道 rlib 解析 + 递归编译 + LZ 前端自检）。
//
// 迁移内容（原 main.rs:278-646，逐字下沉，仅 include_str 路径随模块同目录化）：
// 1. 车道 rlib 解析机制（T0r25）：crate_root / lane_profile_dir / find_builtins_rlib /
//    deps_dir_for / builtins_link —— rs-lz、ir-lz、lex-lz、parse-lz、测试模式共用
// 2. 自检链助手：replace_ext / absolutize / run_selfcheck_exe
// 3. LZ 前端自检（--emit=lex-lz / --emit=parse-lz）：run_lz_frontend
//    （lz_lexer.lz / lz_parser.lz 源随模块 include_str 嵌入）

use std::path::{Path, PathBuf};
use std::process::Command;

// ─────────────────────────── 车道 rlib 解析（T0r25） ───────────────────────────

/// crate 根（`CARGO_MANIFEST_DIR`，编译期注入）。
pub fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 相对路径按 crate 根解析（cargo 在 crate 根执行，相对 `CARGO_TARGET_DIR` 亦以此为基准）。
pub fn resolve_from_crate_root(p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        p
    } else {
        crate_root().join(p)
    }
}

/// 由当前可执行文件反推 `<target-dir>/<profile>`：
/// `.../<target-dir>/<profile>/deps/<bin>[.exe]` → `.../<target-dir>/<profile>`；
/// 直接落在 `<profile>/<bin>` 的普通二进制则取其父目录。
pub fn profile_dir_from_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let profile_dir = if dir.file_name().map_or(false, |n| n == "deps") {
        dir.parent()?
    } else {
        dir
    };
    if profile_dir.is_dir() {
        Some(profile_dir.to_path_buf())
    } else {
        None
    }
}

pub fn profile_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "debug".to_string())
}

/// 本次运行真正使用的 `<target-dir>/<profile>`（见上方 a/b/c 解析链）。
///
/// 车道示例：`CARGO_TARGET_DIR=target/lane-rlib cargo build` 后跑
/// `target/lane-rlib/debug/lzc.exe --emit=rs-lz foo.lz`
/// → 返回 `target/lane-rlib/debug`，于是 `--extern lz_builtins=` 与
/// `-L dependency=` 都指向**本车道**产物，不再串道。
pub fn lane_profile_dir() -> PathBuf {
    if let Some(raw) = std::env::var_os("CARGO_TARGET_DIR") {
        let raw = PathBuf::from(raw);
        if !raw.as_os_str().is_empty() {
            let root = resolve_from_crate_root(raw);
            // 已直接指向 profile 目录（自带 deps/）→ 就用它
            if root.join("deps").is_dir() {
                return root;
            }
            let profile = profile_dir_from_exe()
                .map(|d| profile_name(&d))
                .unwrap_or_else(|| "debug".to_string());
            return root.join(profile);
        }
    }
    if let Some(dir) = profile_dir_from_exe() {
        return dir;
    }
    crate_root().join("target").join("debug")
}

/// 与 `lane_profile_dir()` 配对的 cargo `--target-dir`（即 `<profile>` 的父目录）。
pub fn lane_target_root() -> PathBuf {
    let dir = lane_profile_dir();
    dir.parent().map(|p| p.to_path_buf()).unwrap_or(dir)
}

/// 目录内**最新**（mtime 最大）的 `deps/liblz_builtins-*.rlib`。
/// 同一 target 下可能残留多个 hash 的 rlib，取最新避免链到陈旧产物
/// （陈旧 rlib 缺失 `Complex64`/`BigInt` 的坑）。
pub fn newest_deps_rlib(dir: &Path) -> Option<PathBuf> {
    let deps = dir.join("deps");
    let entries = std::fs::read_dir(&deps).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("liblz_builtins-") && name.ends_with(".rlib") {
            let mtime = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if best.as_ref().map_or(true, |(t, _)| mtime > *t) {
                best = Some((mtime, e.path()));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// 在某个 `<profile>` 目录里定位 lz_builtins 的 .rlib：
/// 先 `deps/liblz_builtins-*.rlib` 取最新，再退回顶层 `liblz_builtins.rlib`。
pub fn probe_rlib(dir: &Path) -> Option<PathBuf> {
    if let Some(p) = newest_deps_rlib(dir) {
        return Some(p);
    }
    let direct = dir.join("liblz_builtins.rlib");
    if direct.exists() {
        return Some(direct);
    }
    None
}

/// 定位本次运行的 lz_builtins .rlib（a/b/c 解析链，见上方注释）。
pub fn find_builtins_rlib() -> Option<PathBuf> {
    let profile_dir = lane_profile_dir();
    if let Some(p) = probe_rlib(&profile_dir) {
        return Some(p);
    }
    // c) 兜底：默认车道 `CARGO_MANIFEST_DIR/target/debug`（车道没构建时至少给出可用产物）
    let fallback = crate_root().join("target").join("debug");
    if fallback != profile_dir {
        if let Some(p) = probe_rlib(&fallback) {
            return Some(p);
        }
    }
    None
}

/// rlib 对应的 `-L dependency=` 目录：**必须是 deps/**。
///
/// BUG-486：顶层 `liblz_builtins.rlib` 是硬链接，其传递依赖
/// （num-bigint/num-complex 等）只在 `deps/` 里；把 `-L dependency` 指到
/// profile 目录会 E0463。
pub fn deps_dir_for(rlib: &Path) -> PathBuf {
    let parent = rlib.parent().unwrap_or(Path::new("."));
    if parent.file_name().map_or(false, |n| n == "deps") {
        return parent.to_path_buf();
    }
    let deps = parent.join("deps");
    if deps.is_dir() {
        deps
    } else {
        parent.to_path_buf()
    }
}

/// （rlib, `-L dependency=` 目录）—— 自检链与 LINK RECIPE 共用的唯一取数口。
pub fn builtins_link() -> Option<(PathBuf, PathBuf)> {
    let rlib = find_builtins_rlib()?;
    let deps = deps_dir_for(&rlib);
    Some((rlib, deps))
}

// ─────────────────────────── 自检链助手 ───────────────────────────

pub fn replace_ext(path: &str, from: &str, to: &str) -> String {
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

/// 把（可能相对的）产物路径按**进程 cwd** 绝对化（N3）。
///
/// 自举路线 B 的自检链会把用户入参派生成产物路径再交给外部进程：用户按文档
/// 写 `lzc --emit=rs-lz foo.lz`（相对路径）时，派生出的产物就是裸文件名
/// `foo.exe`。Rust std 的 `Command` 对「不含路径分隔符的程序名」只按 `PATH`
/// 解析，**不查进程 cwd**（Windows 的 `CreateProcess` 本身是会查 cwd 的，故这
/// 是 std 侧的解析语义），于是产物明明存在，`Command::new("foo.exe")` 仍报
/// `Error { kind: NotFound }`。统一绝对化后，相对 / 绝对 / 含子目录三种入参
/// 形态走同一条路径。
pub fn absolutize(path: &str) -> std::path::PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(p),
        // 取不到 cwd 时保留原样：由 run_selfcheck_exe 的 is_file() 给出带路径的
        // 诊断，而不是 panic。
        Err(_) => p.to_path_buf(),
    }
}

/// 运行自举链路的自检产物；一切可预期失败都返回 `Err`（不再 `expect` panic，
/// 把「工具链跑不起来」伪装成崩溃）。成功时返回子进程输出，由调用方 print。
pub fn run_selfcheck_exe(exe_path: &str) -> Result<std::process::Output, String> {
    let exe_abs = absolutize(exe_path);
    if !exe_abs.is_file() {
        return Err(format!(
            "自检产物不存在: {}（rustc 报告成功但未产出可执行文件）",
            exe_abs.display()
        ));
    }
    let out = Command::new(&exe_abs)
        .output()
        .map_err(|e| format!("无法启动自检产物 {}: {e}", exe_abs.display()))?;
    if !out.status.success() {
        return Err(format!(
            "自检产物运行失败: {} (exit={}): {}",
            exe_abs.display(),
            out.status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string()),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out)
}

// ─────────────────── 自举路线 B：LZ 写的前端（src/frontend/*.lz） ───────────────────
// --emit=lex-lz / --emit=parse-lz：用 LZ 实现的词法/语法前端处理输入文件，
// 走与 --emit=ir-lz 相同的 lzc→rustc→run 递归管线，输出可与 Rust 版基线 diff。

pub const LZ_LEXER_LIB: &str = include_str!("lz_lexer.lz");
pub const LZ_PARSER_LIB: &str = include_str!("lz_parser.lz");

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

/// 组装 wrapper LZ 源码并递归编译运行（lexer 输出 token 流；parser 输出语句描述）。
/// 递归管线：当前编译器 → .rs → rustc → exe → stdout（M5 下沉后 lzcyc 可直接复用）。
pub fn run_lz_frontend(path: &str, source: &str, mode: &str) {
    print!("{}", run_lz_frontend_capture(path, source, mode));
}

/// LZ 前端自检（捕获变体，M5 Lane B/C）：返回 LZ 前端 stdout（token 流 / 解析描述）
/// 而不直接打印。供 lzcyc `--frontend=lz` 一致性门禁捕获对比；run_lz_frontend 为其
/// 打印包装（主编译器 --emit=lex-lz/parse-lz 行为一字不变）。
pub fn run_lz_frontend_capture(path: &str, source: &str, mode: &str) -> String {
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
    std::fs::write(&lz_path, &wrapper).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {}", lz_path, e);
        std::process::exit(1);
    });
    eprintln!("Generated {} -> {} (LZ {} frontend)", path, lz_path, mode);

    // 与 --emit=ir-lz 相同的递归管线（共享实现，见 run_recursive_selfcheck）
    let label = format!("LZ {} frontend", mode);
    run_recursive_selfcheck(&lz_path, ".lzfront", &label, "LZ frontend")
}
/// 递归自检管线公共体（T0r56.1.2 抽取）：源文件已写盘 →
/// 当前编译器自编译 → rustc 链接 lz_builtins → 运行 → 返回 stdout（不打印）。
/// 打印由调用方自行决定：透传方直接 `print!`（ir-lz/rs-lz），捕获方直接返回
/// （lex-lz/parse-lz 的 `run_lz_frontend_capture`，M5 Lane B/C 一致性门禁对比用）。
///
/// `src_ext`：派生产物扩展名（".lzfront"/".lzlz"/".lzrs"）；
/// `label`：错误信息前缀（如 "LZ IR codegen"）；
/// `link_label`：rlib 缺失报错前缀（lex/parse 共用 "LZ frontend"，与
/// 原 builtins_link_or_exit 的 what 参数逐字一致）。
fn run_recursive_selfcheck(lz_path: &str, src_ext: &str, label: &str, link_label: &str) -> String {
    // M5：优先隔离车道 target/lane-bootstrap（免受共享 target/debug 被异版本
    // 工具链交叉重建污染；rust-toolchain.toml 钉版后此为纵深防御）。
    set_bootstrap_lane_env();
    // N3 同族：产物路径先绝对化（相对入参下裸文件名不按 cwd 解析）。
    let self_exe = std::env::current_exe().unwrap_or_default();
    let lz_path_abs = absolutize(lz_path);
    let build = match Command::new(&self_exe).arg(&lz_path_abs).output() {
        Ok(out) => out,
        Err(e) => {
            eprintln!(
                "{label} 自检失败: 无法启动子编译器 {} 处理 {}: {e}",
                self_exe.display(),
                lz_path_abs.display()
            );
            std::process::exit(1);
        }
    };
    if !build.status.success() {
        eprintln!("{label} 编译失败: {}", String::from_utf8_lossy(&build.stderr));
        std::process::exit(1);
    }
    let rs_path = replace_ext(lz_path, src_ext, ".rs");
    // T0r25：rlib 与 -L dependency 按当前车道推导（不再硬编码 target/debug）
    let (builtins, builtins_deps) = match builtins_link() {
        Some(v) => v,
        None => {
            eprintln!(
                "{link_label} 自检: 未找到 lz_builtins 的 .rlib（已查找 {}；先 `cargo build -p lz_builtins`{}）",
                lane_profile_dir().display(),
                if lane_profile_dir() == crate_root().join("target").join("debug") {
                    String::new()
                } else {
                    format!("，车道 target-dir={}", lane_target_root().display())
                }
            );
            std::process::exit(1);
        }
    };
    let exe_path = replace_ext(lz_path, src_ext, ".exe");
    let rs_path_abs = absolutize(&rs_path);
    let exe_path_abs = absolutize(&exe_path);
    let rc = match Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs_path_abs)
        .arg("--extern")
        .arg(format!("lz_builtins={}", builtins.display()))
        .arg("-L")
        .arg(format!("dependency={}", builtins_deps.display()))
        .arg("-o")
        .arg(&exe_path_abs)
        .output()
    {
        Ok(out) => out,
        Err(e) => {
            eprintln!("{label} 失败: 无法启动 rustc（不在 PATH？）: {e}");
            std::process::exit(1);
        }
    };
    if !rc.status.success() {
        eprintln!("{label} rustc 失败: {}", String::from_utf8_lossy(&rc.stderr));
        std::process::exit(1);
    }
    match run_selfcheck_exe(&exe_path) {
        Ok(run) => String::from_utf8_lossy(&run.stdout).into_owned(),
        Err(msg) => {
            eprintln!("{label} 自检失败: {msg}");
            std::process::exit(1);
        }
    }
}

/// --emit=ir-lz 自举管线（M5 T0r56.1.2 下沉）：IR 已构建，生成 LZ 构造代码
/// （lz_ir_lib.lz 库 + main 构造），递归编译运行输出 IR 文本（与 --emit=ir 逐字符一致）。
/// 即「用 LZ 实现 IR display」：display 逻辑完全由 LZ 侧承担。
pub fn run_lz_ir_selfcheck(ir_module: &crate::ir::IrModule, path: &str) {
    let lz_source = crate::ir::lz_codegen::ir_module_to_lz_source(ir_module);
    let lz_path = replace_ext(path, ".lz", ".lzlz");
    std::fs::write(&lz_path, &lz_source).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {}", lz_path, e);
        std::process::exit(1);
    });
    eprintln!("Generated {} -> {} (LZ IR codegen)", path, lz_path);
    print!("{}", run_recursive_selfcheck(&lz_path, ".lzlz", "LZ IR codegen", "LZ IR codegen"));
}

/// --emit=rs-lz 自举管线（M5 T0r56.1.2 下沉）：IR 已构建，生成 LZ 版 Rust codegen
/// 源码（lz_codegen_lib.lz 库 + main 构造），递归编译运行输出 Rust 源码
/// （与常规 codegen 逐字符一致）。embed 产物由 Rust 侧末尾落盘（T0r24 契约：
/// LZ 程序不写文件；stdout 是产物本身，回执走 stderr）。
pub fn run_lz_rs_selfcheck(ir_module: &crate::ir::IrModule, path: &str, embed_build: bool) {
    // T0r24：Cli 档 embed 的产物名/目录/垫片在 Rust 侧一次算好，**同一份**
    // 结论既穿透给 LZ（数据面 + `pub mod __lz_embed`），也用于末尾落盘。
    // 命名主干取 CLI 入参原样（`calc.lz`）——与原生分支 `cg.set_embed_stem(path)`
    // 是同一把尺（`embed::base_stem` + `dedup::artifact_name`），产物名不可能漂移。
    let embed_stem = path;
    let embed_targets = crate::ir::lz_codegen::embed_targets_of(ir_module, Some(embed_stem));
    let rs_lz_src =
        crate::ir::lz_codegen::ir_module_to_rs_lz_source_with_stem(ir_module, Some(embed_stem));
    let lz_path = replace_ext(path, ".lz", ".lzrs");
    std::fs::write(&lz_path, &rs_lz_src).unwrap_or_else(|e| {
        eprintln!("Error writing {}: {}", lz_path, e);
        std::process::exit(1);
    });
    eprintln!("Generated {} -> {} (LZ Rust codegen)", path, lz_path);
    print!("{}", run_recursive_selfcheck(&lz_path, ".lzrs", "LZ Rust codegen", "LZ Rust codegen"));
    // T0r24：Cli 档产物**只能由 Rust 侧落盘**（LZ 程序不写文件）。
    // 落盘是幂等的（artifact::write_artifacts 内容相同则一格都不写），
    // 且自检链里的递归编译产物是 LZ 程序本身（无 embed 块）⇒ 不会重复落盘。
    if !embed_targets.is_empty() {
        crate::embed::emit::emit_embed_artifacts(&embed_targets, embed_build, true, None);
    }
}

// ─────────────────── 双轨一致性（M5 Lane B/C 共用） ───────────────────

/// 自举链 rlib 车道选择：未显式设置 CARGO_TARGET_DIR 且隔离车道
/// `target/lane-bootstrap/debug/deps` 已建时，为**本进程及其子进程**指向隔离车道。
/// 共享 target/debug 会被不同入口（人工 gnu shell / cron 默认工具链）交叉重建，
/// mtime 最新的 rlib 可能与当前 rustc 版本错配（E0514 反复复发的根因）；
/// 隔离车道只由钉版工具链构建，永不串道。已显式设置车道时尊重现值。
pub fn set_bootstrap_lane_env() {
    if std::env::var_os("CARGO_TARGET_DIR").is_some() {
        return;
    }
    let lane = crate_root().join("target").join("lane-bootstrap");
    if lane.join("debug").join("deps").is_dir() {
        std::env::set_var("CARGO_TARGET_DIR", "target/lane-bootstrap");
    }
}

/// 还原 Rust Debug 转义（\" \\ \n \r \t \0 \u{XXXX}）为原始字符。
/// 双轨归一化用：Rust 侧 Token Debug 的字符串 payload 是 Debug 转义形态。
fn debug_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('0') => out.push('\0'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('\'') => out.push('\''),
            Some('u') => {
                // \u{XXXX} → 字符；畸形序列原样保留（门禁失败时可见原文）
                let mut hex = String::new();
                if chars.peek() == Some(&'{') {
                    chars.next();
                    for hc in chars.by_ref() {
                        if hc == '}' {
                            break;
                        }
                        hex.push(hc);
                    }
                }
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    None => {
                        out.push_str("\\u{");
                        out.push_str(&hex);
                        out.push('}');
                    }
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// 双轨 token 流归一化（M5 Lane B/C 门禁共用单真源）。
///
/// 两侧原始形态：
/// - Rust（`--tokens` / `format!("{:?}")`）：`=== Tokens ===` 头 + 行首序号
///   `  12 Ge` + 带值变体 Debug 转义 `Ident("f")` / `StrLit("ok")`
/// - LZ（display_token + print）：无头无序号、payload 裸形态 `Ident(f)`，
///   print 可能给整行裹引号（golden 测试的 trim_matches('"') 即此历史）
///
/// 归一后两侧应为同一向量：变体名 + 还原后的裸 payload。
/// `rust_side=true` 走 Rust 形态解析；false 走 LZ 形态。
pub fn normalize_token_stream(raw: &str, rust_side: bool) -> Vec<String> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let mut l = line.trim();
        if l.is_empty() {
            continue;
        }
        if l.starts_with("=== Tokens ===") {
            continue;
        }
        if rust_side {
            // 剥行首序号："12 Ge"（trim 后首段全数字才剥，payload 含空格不受扰）
            if let Some(pos) = l.find(' ') {
                if l[..pos].chars().all(|c| c.is_ascii_digit()) {
                    l = l[pos..].trim();
                }
            }
        }
        // LZ print 的整体引号（两侧防御性剥离，Rust 侧无引号时无害）
        let l = l.trim_matches('"');
        if l.is_empty() {
            continue;
        }
        // Var("payload") → Var(还原 payload)；无括号行原样
        let norm = match (l.find('('), l.rfind(')')) {
            (Some(open), Some(close)) if close > open => {
                let variant = &l[..open];
                let payload = &l[open + 1..close];
                let payload = if rust_side
                    && payload.len() >= 2
                    && payload.starts_with('"')
                    && payload.ends_with('"')
                {
                    debug_unescape(&payload[1..payload.len() - 1])
                } else {
                    payload.to_string()
                };
                format!("{}({})", variant, payload)
            }
            _ => l.to_string(),
        };
        out.push(norm);
    }
    out
}

// Lang-Zong 编译器 — bridge_cli.rs
// Level 3: CLI 序列化桥接
// 跨进程序列化通信，通过 stdin/stdout 请求-响应协议调用外部程序。
// 实现 Bridge trait，生成子进程管理的 Rust 代码。

use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeLevel, BridgeMeta, CallResolveResult, ExportEntry, ExportKind,
};
use std::collections::HashMap;
use std::time::Duration;

// ──────────────── CLI 端点配置 ────────────────

/// CLI 桥接端点配置
#[derive(Debug, Clone)]
pub struct CliEndpoint {
    pub name: String,
    pub command: String,             // 可执行文件路径
    pub args: Vec<String>,           // 命令行参数
    pub timeout_ms: u64,             // 超时（毫秒）
    pub max_retries: u32,            // 最大重试次数
    pub format: SerializationFormat, // 序列化格式
    pub pool_size: usize,            // 进程池大小（0=无池，每次启动新进程）
}

/// 序列化格式
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializationFormat {
    LineProtocol, // METHOD key=val key=val  :: OK result / ERR code msg
    JsonLines,    // {"method":"...", "params":{...}} 换行分隔
    StdinRaw,     // 仅 stdin 单向传递，无响应
}

impl Default for CliEndpoint {
    fn default() -> Self {
        CliEndpoint {
            name: "default".to_string(),
            command: String::new(),
            args: vec![],
            timeout_ms: 5000,
            max_retries: 3,
            format: SerializationFormat::LineProtocol,
            pool_size: 0,
        }
    }
}

// ──────────────── CLI 桥接 ────────────────

/// Level 3: CLI 序列化桥接
#[derive(Debug)]
pub struct CliBridge {
    endpoints: HashMap<String, CliEndpoint>,
}

impl CliBridge {
    pub fn new() -> Self {
        CliBridge {
            endpoints: HashMap::new(),
        }
    }

    pub fn register(&mut self, endpoint: CliEndpoint) {
        self.endpoints.insert(endpoint.name.clone(), endpoint);
    }

    pub fn get(&self, name: &str) -> Option<&CliEndpoint> {
        self.endpoints.get(name)
    }

    // ─── 代码生成 ───

    /// 生成进程池管理模块（供 lz 生成的 Rust 代码使用）
    pub fn generate_pool_module(&self) -> String {
        if self.endpoints.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        out.push_str("// ── Lang-Zong CLI Bridge helper module ──\n");
        out.push_str("use std::io::{BufRead, BufReader, Write};\n");
        out.push_str("use std::process::{Command, Stdio, Child};\n");
        out.push_str("use std::time::{Duration, Instant};\n\n");

        // 生成每个 endpoint 的调用函数
        for (name, ep) in &self.endpoints {
            out.push_str(&self.generate_endpoint_func(name, ep));
        }

        out
    }

    fn generate_endpoint_func(&self, name: &str, ep: &CliEndpoint) -> String {
        let timeout = Duration::from_millis(ep.timeout_ms);
        let timeout_secs = timeout.as_secs_f64();

        let mut out = String::new();
        out.push_str(&format!(
            "pub fn cli_call_{name}(method: &str, params: &[(&str, &str)]) -> Result<String, String> {{\n",
            name = name
        ));

        // 启动子进程
        out.push_str(&format!(
            "    let mut child = Command::new(\"{cmd}\")\n",
            cmd = ep.command
        ));
        for arg in &ep.args {
            out.push_str(&format!("        .arg(\"{arg}\")\n", arg = arg));
        }
        out.push_str("        .stdin(Stdio::piped())\n");
        out.push_str("        .stdout(Stdio::piped())\n");
        out.push_str("        .stderr(Stdio::inherit())\n");
        out.push_str("        .spawn()\n");
        out.push_str("        .map_err(|e| format!(\"spawn failed: {}\", e))?;\n\n");

        // 序列化请求
        match ep.format {
            SerializationFormat::LineProtocol => {
                out.push_str("    let mut stdin = child.stdin.take().unwrap();\n");
                out.push_str("    write!(stdin, \"{} \", method).unwrap();\n");
                out.push_str("    for (k, v) in params {\n");
                out.push_str("        write!(stdin, \"{}={} \", k, v).unwrap();\n");
                out.push_str("    }\n");
                out.push_str("    writeln!(stdin).unwrap();\n");
                out.push_str("    drop(stdin);\n\n");

                // 等待响应（带超时）
                out.push_str(&format!(
                    "    let start = Instant::now();\n\
                     let timeout = Duration::from_secs_f64({secs});\n\
                     let mut reader = BufReader::new(child.stdout.take().unwrap());\n\
                     let mut line = String::new();\n\
                     loop {{\n\
                         if start.elapsed() > timeout {{\n\
                             let _ = child.kill();\n\
                             return Err(\"timeout\".to_string());\n\
                         }}\n\
                         line.clear();\n\
                         if reader.read_line(&mut line).map_err(|e| format!(\"read: {{}}\", e))? == 0 {{\n\
                             break;\n\
                         }}\n\
                         let line = line.trim();\n\
                         if line.starts_with(\"OK:\") {{\n\
                             let status = child.wait().unwrap_or_default();\n\
                             if status.success() {{\n\
                                 return Ok(line[3..].to_string());\n\
                             }}\n\
                         }} else if line.starts_with(\"ERR:\") {{\n\
                             return Err(line[4..].to_string());\n\
                         }}\n\
                     }}\n\
                     Err(\"no response\".to_string())\n",
                    secs = timeout_secs
                ));
            }
            SerializationFormat::JsonLines => {
                out.push_str("    // JSON Lines format (requires serde_json crate)\n");
                out.push_str("    Err(\"JsonLines format not implemented\".to_string())\n");
            }
            SerializationFormat::StdinRaw => {
                out.push_str("    // StdinRaw: one-way data pipe, no response expected\n");
                out.push_str("    let mut stdin = child.stdin.take().unwrap();\n");
                out.push_str("    for (_, v) in params {\n");
                out.push_str("        write!(stdin, \"{}\", v).unwrap();\n");
                out.push_str("    }\n");
                out.push_str("    drop(stdin);\n");
                out.push_str("    let status = child.wait().unwrap_or_default();\n");
                out.push_str("    if status.success() { Ok(String::new()) } else { Err(\"process failed\".to_string()) }\n");
            }
        }

        out.push_str("}\n\n");
        out
    }
}

impl Bridge for CliBridge {
    fn name(&self) -> &str {
        "cli"
    }

    fn level(&self) -> BridgeLevel {
        BridgeLevel::InterProcess
    }

    fn capabilities(&self) -> BridgeCapability {
        BridgeCapability::FUNCTION_CALL
    }

    fn gen_call(&self, func_name: &str, _args: &[String]) -> Option<String> {
        // 查找匹配的 endpoint
        for (name, _) in &self.endpoints {
            // 简化匹配：函数名前缀匹配 endpoint 名
            if func_name.starts_with(name) {
                return Some(format!("cli_call_{name}"));
            }
        }
        None
    }

    fn meta(&self) -> BridgeMeta {
        BridgeMeta {
            version: "0.1.0".into(),
            description: format!("CLI bridge: {} endpoints", self.endpoints.len()),
            ..Default::default()
        }
    }

    fn resolve_call_full(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        self.gen_call(func_name, _args)
            .map(|rust_path| CallResolveResult {
                rust_path,
                shim: String::new(),
                module_name: "cli".into(),
                is_macro: false,
                is_template: false,
                ret_result: false,
            })
    }

    fn list_exports(&self, kind: ExportKind) -> Vec<ExportEntry> {
        match kind {
            ExportKind::Function => self
                .endpoints
                .keys()
                .map(|name| ExportEntry {
                    name: name.clone(),
                    kind: ExportKind::Function,
                    signature: format!("cli endpoint: {}", name),
                    module: "cli".into(),
                })
                .collect(),
            _ => vec![],
        }
    }

    fn export_count(&self) -> usize {
        self.endpoints.len()
    }
}

// ──────────────── Embed 垫片（src/embed 的 Cli 档） ────────────────
//
// 与上面的 `CliEndpoint`（stdin 行协议）**不是同一套东西**：embed 垫片走 argv
// （理由见 src/embed/shim.rs 顶部注释），请求 = `<method> key=value ...`，
// 响应 = stdout 最后一行 `OK:<值>` / `ERR:<原因>`。
//
// 生成物里只允许有两处 `Command::new`（协议调用 / 原样执行各一个 runner），
// 由 tests/embed_unify_r15.rs::embed_adds_exactly_one_spawn_site 钉着。

/// 一个可调用函数进垫片的形状（wrapper = 垫片里生成的 Rust 函数名）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedFnSig {
    pub wrapper: String,
    pub method: String,
    pub params: Vec<(String, String)>, // (形参名, lz 基础类型)
    pub ret: String,                  // int/float/bool/str/unit
}

/// 一个 Cli 档 embed 产物端点（产物名 + 落点 + 运行命令 + 可调用面）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedEndpoint {
    pub artifact: String,
    /// 仓内相对产物目录（确定性文本，不含绝对路径）
    pub dir_rel: String,
    pub program: String,
    /// 运行命令的相对参数（分发入口 / 源文件）
    pub prefix: Vec<String>,
    /// "executable" | "library"
    pub kind: &'static str,
    pub fns: Vec<EmbedFnSig>,
}

const EMBED_SHIM_HEADER: &str = r#"// ── Lang-Zone Cli 档 embed 垫片（argv 请求 / stdout 末行响应；由 lang-zone 生成，勿手改）──
use std::process::Command;

/// 产物目录：优先 `LZ_EMBED_DIR`（运行 cwd 与仓根不同时用），否则按仓内相对路径解析。
fn __lz_embed_dir(rel: &str) -> std::path::PathBuf {
    match std::env::var("LZ_EMBED_DIR") {
        Ok(root) if !root.is_empty() => std::path::Path::new(&root).join(rel),
        _ => std::path::PathBuf::from(rel),
    }
}

/// 分发入口把值里的反斜杠/换行转义成 `\\` / `\n`，这里逐字还原。
fn __lz_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// 协议调用：`<method> key=value ...` 走 argv，响应取 stdout 最后一行。
/// 起不来 / 非零退出 / ERR: / 无响应 一律 panic 并带上产物名与原因 —— 绝不返回默认值。
fn __lz_call(
    artifact: &str,
    rel: &str,
    program: &str,
    prefix: &[&str],
    method: &str,
    params: &[(&str, String)],
) -> String {
    let dir = __lz_embed_dir(rel);
    let mut cmd = Command::new(program);
    cmd.args(prefix);
    cmd.arg(method);
    for (k, v) in params {
        cmd.arg(format!("{}={}", k, v));
    }
    cmd.current_dir(&dir);
    let out = cmd.output().unwrap_or_else(|e| {
        panic!(
            "embed 产物 {} 调用 {} 失败：无法启动 `{}`（{}）；产物目录 {}",
            artifact, method, program, e, dir.display()
        )
    });
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let last = stdout
        .lines()
        .rev()
        .find(|l| l.starts_with("OK:") || l.starts_with("ERR:"));
    let line = match last {
        Some(l) => l.to_string(),
        None => panic!(
            "embed 产物 {} 调用 {} 无响应（期望 stdout 最后一行 OK:/ERR:）：stdout={:?} stderr={:?}",
            artifact, method, stdout, stderr
        ),
    };
    if let Some(reason) = line.strip_prefix("ERR:") {
        panic!("embed 产物 {} 调用 {} 失败：{}", artifact, method, reason);
    }
    if !out.status.success() {
        panic!(
            "embed 产物 {} 调用 {} 非零退出（code={:?}）：stderr={:?}",
            artifact,
            method,
            out.status.code(),
            stderr
        );
    }
    __lz_unescape(&line[3..])
}

/// 可执行产物：原样跑一遍，取 stdout（不参与 request/response 协议）。
fn __lz_exec(artifact: &str, rel: &str, program: &str, prefix: &[&str]) -> String {
    let dir = __lz_embed_dir(rel);
    let mut cmd = Command::new(program);
    cmd.args(prefix);
    cmd.current_dir(&dir);
    let out = cmd.output().unwrap_or_else(|e| {
        panic!(
            "embed 产物 {} 执行失败：无法启动 `{}`（{}）；产物目录 {}",
            artifact, program, e, dir.display()
        )
    });
    if !out.status.success() {
        panic!(
            "embed 产物 {} 执行失败（code={:?}）：stderr={:?}",
            artifact,
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    String::from_utf8_lossy(&out.stdout)
        .trim_end_matches(|c| c == '\r' || c == '\n')
        .to_string()
}
"#;

fn rust_str_lit(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            _ => o.push(c),
        }
    }
    o.push('"');
    o
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "async", "await", "box", "do", "final", "macro", "override",
    "priv", "try", "typeof", "unsized", "virtual", "yield", "abstract", "become",
];

/// 外语言形参名 → 可编译的 Rust 标识符（关键字/非法字符加前缀，保持一致映射）
fn rust_ident(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if s.is_empty() {
        s.push('_');
    }
    if s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        s.insert(0, '_');
    }
    if RUST_KEYWORDS.contains(&s.as_str()) {
        s.insert_str(0, "p_");
    }
    s
}

fn embed_param_ty(lz: &str) -> &'static str {
    match lz {
        "int" => "i64",
        "float" => "f64",
        "bool" => "bool",
        // LZ 调点对 str 实参可能给 `&str`（字面量直传）也可能给 `String`
        // （codegen 对未知形参类型的默认是 `.to_string()`），两种都得收。
        "str" => "impl Into<String>",
        _ => "&str",
    }
}

fn embed_ret_ty(ret: &str) -> &'static str {
    match ret {
        "int" => "i64",
        "float" => "f64",
        "bool" => "bool",
        "str" => "String",
        _ => "()",
    }
}

fn embed_arg_expr(ident: &str, lz: &str) -> String {
    match lz {
        "bool" => format!("(if {} {{ \"true\" }} else {{ \"false\" }}).to_string()", ident),
        "str" => format!("{}.into()", ident),
        _ => format!("{}.to_string()", ident),
    }
}

fn embed_lit_array(items: &[String]) -> String {
    if items.is_empty() {
        "&[]".to_string()
    } else {
        format!("&[{}]", items.join(", "))
    }
}

fn embed_wrapper_fn(ep: &EmbedEndpoint, f: &EmbedFnSig) -> String {
    let mut out = String::new();

    // 形参名去重 + 关键字规避（映射关系不动 method 的 key=value 键）
    let mut used = std::collections::HashSet::new();
    let mut idents = Vec::new();
    for (name, _) in &f.params {
        let base = rust_ident(name);
        let mut cand = base.clone();
        let mut k = 2;
        while !used.insert(cand.clone()) {
            cand = format!("{}_{}", base, k);
            k += 1;
        }
        idents.push(cand);
    }

    let sig_params: Vec<String> = f
        .params
        .iter()
        .zip(&idents)
        .map(|((_, ty), id)| format!("{}: {}", id, embed_param_ty(ty)))
        .collect();
    out.push_str(&format!("pub fn {}(", f.wrapper));
    out.push_str(&sig_params.join(", "));
    out.push(')');
    let ret_ty = embed_ret_ty(&f.ret);
    if ret_ty != "()" {
        out.push_str(&format!(" -> {}", ret_ty));
    }
    out.push_str(" {\n");

    let prefix = embed_lit_array(&ep.prefix.iter().map(|a| rust_str_lit(a)).collect::<Vec<_>>());
    let args: Vec<String> = f
        .params
        .iter()
        .zip(&idents)
        .map(|((name, ty), id)| {
            format!("({}, {})", rust_str_lit(name), embed_arg_expr(id, ty))
        })
        .collect();
    let params_lit = embed_lit_array(&args);
    let call = format!(
        "__lz_call({}, {}, {}, {}, {}, {})",
        rust_str_lit(&ep.artifact),
        rust_str_lit(&ep.dir_rel),
        rust_str_lit(&ep.program),
        prefix,
        rust_str_lit(&f.method),
        params_lit
    );

    let art = rust_str_lit(&ep.artifact);
    let method = rust_str_lit(&f.method);
    if ret_ty == "()" {
        out.push_str(&format!("    {};\n", call));
    } else {
        out.push_str(&format!("    let __r = {};\n", call));
        match f.ret.as_str() {
            "int" => out.push_str(&format!(
                "    __r.trim().parse::<i64>().unwrap_or_else(|e| panic!(\"embed 产物 {{}} 的 {{}} 返回值 `{{}}` 不是 int：{{}}\", {}, {}, __r, e))\n",
                art, method
            )),
            "float" => out.push_str(&format!(
                "    __r.trim().parse::<f64>().unwrap_or_else(|e| panic!(\"embed 产物 {{}} 的 {{}} 返回值 `{{}}` 不是 float：{{}}\", {}, {}, __r, e))\n",
                art, method
            )),
            "bool" => out.push_str(&format!(
                "    match __r.trim() {{\n        \"true\" | \"True\" | \"1\" => true,\n        \"false\" | \"False\" | \"0\" => false,\n        other => panic!(\"embed 产物 {{}} 的 {{}} 返回值 `{{}}` 不是 bool\", {}, {}, other),\n    }}\n",
                art, method
            )),
            _ => out.push_str("    __r\n"),
        }
    }
    out.push_str("}\n");
    out
}

fn embed_run_fn(ep: &EmbedEndpoint) -> String {
    let prefix = embed_lit_array(&ep.prefix.iter().map(|a| rust_str_lit(a)).collect::<Vec<_>>());
    format!(
        "/// 可执行产物的原样执行面：跑一遍，取 stdout。\npub fn {}_run() -> String {{\n    __lz_exec({}, {}, {}, {})\n}}\n",
        ep.artifact,
        rust_str_lit(&ep.artifact),
        rust_str_lit(&ep.dir_rel),
        rust_str_lit(&ep.program),
        prefix
    )
}

/// 生成 `pub mod __lz_embed { ... }` 的**内容**（模块外壳由调用方包）。
/// 纯函数：同一输入逐字同输出，不含本机绝对路径。
pub fn generate_embed_module(eps: &[EmbedEndpoint]) -> String {
    let mut out = String::from(EMBED_SHIM_HEADER);
    for ep in eps {
        // 可执行产物没有分发入口 ⇒ 不出库函数垫片（出了也只会在运行期静默错）
        if ep.kind == "executable" {
            out.push('\n');
            out.push_str(&embed_run_fn(ep));
            continue;
        }
        for f in &ep.fns {
            out.push('\n');
            out.push_str(&embed_wrapper_fn(ep, f));
        }
    }
    out
}

// ──────────────── 测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_bridge_empty() {
        let bridge = CliBridge::new();
        assert_eq!(bridge.name(), "cli");
        assert_eq!(bridge.level(), BridgeLevel::InterProcess);
        assert!(bridge.endpoints.is_empty());
    }

    #[test]
    fn test_cli_bridge_register() {
        let mut bridge = CliBridge::new();
        bridge.register(CliEndpoint {
            name: "image_converter".to_string(),
            command: "convert".to_string(),
            args: vec!["-".to_string()],
            timeout_ms: 3000,
            max_retries: 2,
            format: SerializationFormat::LineProtocol,
            pool_size: 0,
        });
        assert!(bridge.get("image_converter").is_some());
        assert!(bridge.get("nonexistent").is_none());
    }

    #[test]
    fn test_cli_bridge_generate_empty() {
        let bridge = CliBridge::new();
        let code = bridge.generate_pool_module();
        assert!(code.is_empty());
    }

    #[test]
    fn test_cli_bridge_generate_with_endpoints() {
        let mut bridge = CliBridge::new();
        bridge.register(CliEndpoint {
            name: "ocr".to_string(),
            command: "tesseract".to_string(),
            args: vec!["stdin".to_string(), "stdout".to_string()],
            ..Default::default()
        });
        let code = bridge.generate_pool_module();
        assert!(code.contains("cli_call_ocr"));
        assert!(code.contains("tesseract"));
        assert!(code.contains("OK:"));
        assert!(code.contains("ERR:"));
    }

    #[test]
    fn test_cli_bridge_line_protocol_format() {
        let mut bridge = CliBridge::new();
        bridge.register(CliEndpoint {
            name: "test".to_string(),
            command: "echo".to_string(),
            format: SerializationFormat::LineProtocol,
            ..Default::default()
        });
        let code = bridge.generate_pool_module();
        assert!(code.contains("write!(stdin, \"{} \", method)"));
        assert!(code.contains("line.starts_with(\"OK:\")"));
    }

    #[test]
    fn test_cli_bridge_call() {
        let mut bridge = CliBridge::new();
        bridge.register(CliEndpoint {
            name: "ffmpeg".to_string(),
            command: "ffmpeg".to_string(),
            ..Default::default()
        });
        let result = bridge.gen_call("ffmpeg_convert", &[]);
        assert_eq!(result, Some("cli_call_ffmpeg".to_string()));
    }

    #[test]
    fn test_cli_bridge_capabilities() {
        let bridge = CliBridge::new();
        let caps = bridge.capabilities();
        assert!(caps.contains(BridgeCapability::FUNCTION_CALL));
        assert!(!caps.contains(BridgeCapability::IMPORT));
    }
}

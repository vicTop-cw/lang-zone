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

/// 程序路径解析：`./x.exe` 这类**相对**程序名在 Windows 上由 CreateProcess 按**父进程** cwd
/// 解析 —— `current_dir` 只换子进程的 cwd，管不到程序名本身，于是
/// 直接 spawn `./c_calc.exe` 再配 `current_dir(产物目录)` 必然 os error 2
/// （实测 T0r49：`无法启动 ./c_calc.exe（系统找不到指定的文件。）`）。
/// 故含分隔符的相对程序名先拼成 `<产物目录>/<程序>`；裸名字（python / mojo / scala）
/// 仍原样交给 PATH 查找，行为与修复前一致。
fn __lz_resolve_program(dir: &std::path::Path, program: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(program);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    if program.contains('/') || program.contains('\\') {
        return dir.join(p);
    }
    p.to_path_buf()
}

/// Windows 上「裸名 + PATH 上只有 `.bat`/`.cmd` 包装」的工具链（Coursier 装的 scala/scalac 是典型）
/// 不能直接 CreateProcess：裸名只补 `.exe`、**不认 PATHEXT**，而编译器侧 `tool_available`
/// 按文件名扫 PATH 会认 `scalac.bat` ⇒「读数说在场、spawn 说 program not found」
/// （实测 T0r49 §2.4）。判据与 `src/embed/mod.rs::needs_cmd_route` **同一把尺**：
///  - 带 `.bat`/`.cmd`/`.exe` 扩展名或含分隔符的形态，CreateProcess 自己解得开 ⇒ 不换；
///  - 裸名且 PATH 上有同名 `.exe`（python/gcc 这类原生映像）⇒ 不换，行为与修前一字不动；
///  - 只有「裸名 + 无可执行映像 + 有同名 .bat/.cmd」才换成 `cmd /C <原程序>`。
///
/// 返回 `(实际可执行程序, 插在原实参之前的段)`；无需换尺时第二段为空，第一段沿用
/// `__lz_resolve_program` 的结果（T0r49 的相对程序名按产物目录解析，判据
/// `c_library_artifact_is_invocable_from_an_unrelated_cwd`）。
///
/// 本函数只返回数据、不 spawn ⇒ 每个 runner 仍是**恰好一处** `Command::new`
/// （`embed_adds_exactly_one_spawn_site_and_it_lives_in_the_shim` 钉着这条不变量）。
fn __lz_spawn_plan(dir: &std::path::Path, program: &str) -> (std::path::PathBuf, Vec<String>) {
    if cfg!(windows) {
        let lower = program.to_ascii_lowercase();
        if !(lower.ends_with(".bat") || lower.ends_with(".cmd") || lower.ends_with(".exe"))
            && !program.contains('/')
            && !program.contains('\\')
        {
            let dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).collect())
                .unwrap_or_default();
            let has_image = dirs
                .iter()
                .any(|d| d.join(format!("{}.exe", program)).is_file());
            let has_batch = dirs.iter().any(|d| {
                d.join(format!("{}.bat", program)).is_file()
                    || d.join(format!("{}.cmd", program)).is_file()
            });
            if !has_image && has_batch {
                return (
                    std::path::PathBuf::from("cmd"),
                    vec!["/C".to_string(), program.to_string()],
                );
            }
        }
    }
    (__lz_resolve_program(dir, program), Vec::new())
}

/// 把 BTreeMap<String, i64> 序列化为 JSON 对象字符串 `{"key":value,...}`（手写，避免依赖 serde_json）
fn __lz_dict_to_json(map: &std::collections::BTreeMap<String, i64>) -> String {
    let mut pairs: Vec<String> = Vec::with_capacity(map.len());
    for (k, v) in map.iter() {
        pairs.push(format!("\"{}\":{}", k, v));
    }
    format!("{{{}}}", pairs.join(","))
}

/// 把 BTreeMap<String, f64> 序列化为 JSON 对象字符串 `{"key":value,...}`
fn __lz_dict_to_json_float(map: &std::collections::BTreeMap<String, f64>) -> String {
    let mut pairs: Vec<String> = Vec::with_capacity(map.len());
    for (k, v) in map.iter() {
        pairs.push(format!("\"{}\":{}", k, v));
    }
    format!("{{{}}}", pairs.join(","))
}

/// 把 BTreeMap<String, String> 序列化为 JSON 对象字符串 `{"key":"value",...}`
fn __lz_dict_to_json_str(map: &std::collections::BTreeMap<String, String>) -> String {
    let mut pairs: Vec<String> = Vec::with_capacity(map.len());
    for (k, v) in map.iter() {
        pairs.push(format!("\"{}\":\"{}\"", k, v));
    }
    format!("{{{}}}", pairs.join(","))
}

/// Dict<int,int> → JSON 对象字符串 `{"1":100,"2":200}`（键和值都是整数）
fn __lz_dict_int_int_to_json(map: &std::collections::BTreeMap<i64, i64>) -> String {
    let mut pairs: Vec<String> = Vec::with_capacity(map.len());
    for (k, v) in map.iter() {
        pairs.push(format!("\"{}\":{}", k, v));
    }
    format!("{{{}}}", pairs.join(","))
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

/// 解析 `[1,2,3]` 形式的 JSON int 数组为 Vec<i64>（手写解析，避免依赖 serde_json）
fn __lz_parse_list_int(raw: &str, artifact: &str, method: &str) -> Vec<i64> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 List<int> JSON 数组", artifact, method, raw);
    }
    let inner = &trimmed[1..trimmed.len()-1];
    if inner.is_empty() {
        return Vec::new();
    }
    inner.split(',')
        .map(|s| s.trim().parse::<i64>().unwrap_or_else(|e| {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 含非 int 元素：{}", artifact, method, raw, e)
        }))
        .collect()
}

/// 解析 `[1.5,2.0]` 形式的 JSON float 数组为 Vec<f64>
fn __lz_parse_list_float(raw: &str, artifact: &str, method: &str) -> Vec<f64> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 List<float> JSON 数组", artifact, method, raw);
    }
    let inner = &trimmed[1..trimmed.len()-1];
    if inner.is_empty() {
        return Vec::new();
    }
    inner.split(',')
        .map(|s| s.trim().parse::<f64>().unwrap_or_else(|e| {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 含非 float 元素：{}", artifact, method, raw, e)
        }))
        .collect()
}

/// 解析 `["a","b"]` 形式的 JSON str 数组为 Vec<String>
fn __lz_parse_list_str(raw: &str, artifact: &str, method: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 List<str> JSON 数组", artifact, method, raw);
    }
    let inner = &trimmed[1..trimmed.len()-1];
    if inner.is_empty() {
        return Vec::new();
    }
    // 手写 JSON 字符串数组解析：每个元素是 `"..."` 形式
    inner.split(',')
        .map(|s| {
            let s = s.trim();
            if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                // 反转义 \" 和 \\
                let inner = &s[1..s.len()-1];
                Ok(inner.replace("\\\"", "\"").replace("\\\\", "\\"))
            } else {
                Err(format!("embed 产物 {} 的 {} 返回值 `{}` 含非字符串元素：`{}`", artifact, method, raw, s))
            }
        })
        .collect::<Result<Vec<_>, String>>()
        .unwrap_or_else(|e| panic!("{}", e))
}

/// 解析 `{"a":1,"b":2}` 形式的 JSON 对象为 BTreeMap<String, i64>（手写解析，避免依赖 serde_json）
fn __lz_parse_dict_str_int(raw: &str, artifact: &str, method: &str) -> std::collections::BTreeMap<String, i64> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 Dict<str,int> JSON 对象", artifact, method, raw);
    }
    let inner = &trimmed[1..trimmed.len()-1];
    let mut map = std::collections::BTreeMap::new();
    if inner.trim().is_empty() {
        return map;
    }
    // 手写 JSON 对象解析：按顶层逗号切分键值对，key 是 "..." 字符串，value 是整数
    // 注意：JSON 字符串值内可能含逗号，需正确处理引号
    let mut pairs: Vec<&str> = Vec::new();
    let mut depth = 0;
    let mut in_str = false;
    let mut escape = false;
    let mut start = 0;
    for (i, c) in inner.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' {
            escape = true;
            continue;
        }
        if c == '"' {
            in_str = !in_str;
            continue;
        }
        if in_str {
            continue;
        }
        // BUG-513 第二面：`:` 不是嵌套开启符，只 `{`/`[` 才加深度。
        // 旧实现把 `:` 计入 depth，导致顶层分隔逗号在 depth==1 处被漏判，
        // `{"a": 6, "b": 9}` 被切成一整对（键 `"a"`、值 `6, "b": 9`）→ 解析 panic。
        if c == '{' || c == '[' {
            depth += 1;
        } else if c == '}' || c == ']' {
            depth -= 1;
        }
        if c == ',' && depth == 0 {
            pairs.push(&inner[start..i]);
            start = i + 1;
        }
    }
    pairs.push(&inner[start..]);
    for pair in pairs {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        // 切分 key: value
        let colon = pair.find(':').unwrap_or_else(|| panic!(
            "embed 产物 {} 的 {} 返回值 `{}` 的键值对 `{}` 缺少冒号",
            artifact, method, raw, pair
        ));
        let key_raw = pair[..colon].trim();
        let val_raw = pair[colon+1..].trim();
        // key 必须是 "..." 形式
        let key = if key_raw.starts_with('"') && key_raw.ends_with('"') && key_raw.len() >= 2 {
            &key_raw[1..key_raw.len()-1]
        } else {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 的键 `{}` 不是字符串", artifact, method, raw, key_raw)
        };
        // value 是整数
        let val: i64 = val_raw.parse::<i64>().unwrap_or_else(|e| {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 的值 `{}` 不是 int：{}", artifact, method, raw, val_raw, e)
        });
        map.insert(key.to_string(), val);
    }
    map
}

/// 解析 `{"a":1.5,"b":2.0}` 形式的 JSON 对象为 BTreeMap<String, f64>（手写解析，避免依赖 serde_json）
fn __lz_parse_dict_str_float(raw: &str, artifact: &str, method: &str) -> std::collections::BTreeMap<String, f64> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 Dict<str,float> JSON 对象", artifact, method, raw);
    }
    let inner = &trimmed[1..trimmed.len()-1];
    let mut map = std::collections::BTreeMap::new();
    if inner.trim().is_empty() {
        return map;
    }
    let mut pairs: Vec<&str> = Vec::new();
    let mut depth = 0;
    let mut in_str = false;
    let mut escape = false;
    let mut start = 0;
    for (i, c) in inner.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' {
            escape = true;
            continue;
        }
        if c == '"' {
            in_str = !in_str;
            continue;
        }
        if in_str {
            continue;
        }
        // BUG-513 第二面：`:` 不是嵌套开启符，只 `{`/`[` 才加深度。
        // 旧实现把 `:` 计入 depth，导致顶层分隔逗号在 depth==1 处被漏判，
        // `{"a": 6, "b": 9}` 被切成一整对（键 `"a"`、值 `6, "b": 9`）→ 解析 panic。
        if c == '{' || c == '[' {
            depth += 1;
        } else if c == '}' || c == ']' {
            depth -= 1;
        }
        if c == ',' && depth == 0 {
            pairs.push(&inner[start..i]);
            start = i + 1;
        }
    }
    pairs.push(&inner[start..]);
    for pair in pairs {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let colon = pair.find(':').unwrap_or_else(|| panic!(
            "embed 产物 {} 的 {} 返回值 `{}` 的键值对 `{}` 缺少冒号",
            artifact, method, raw, pair
        ));
        let key_raw = pair[..colon].trim();
        let val_raw = pair[colon+1..].trim();
        let key = if key_raw.starts_with('"') && key_raw.ends_with('"') && key_raw.len() >= 2 {
            &key_raw[1..key_raw.len()-1]
        } else {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 的键 `{}` 不是字符串", artifact, method, raw, key_raw)
        };
        let val: f64 = val_raw.parse::<f64>().unwrap_or_else(|e| {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 的值 `{}` 不是 float：{}", artifact, method, raw, val_raw, e)
        });
        map.insert(key.to_string(), val);
    }
    map
}

/// 解析 `{"a":"x","b":"y"}` 形式的 JSON 对象为 BTreeMap<String, String>（手写解析，避免依赖 serde_json）
fn __lz_parse_dict_str_str(raw: &str, artifact: &str, method: &str) -> std::collections::BTreeMap<String, String> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 Dict<str,str> JSON 对象", artifact, method, raw);
    }
    let inner = &trimmed[1..trimmed.len()-1];
    let mut map = std::collections::BTreeMap::new();
    if inner.trim().is_empty() {
        return map;
    }
    let mut pairs: Vec<&str> = Vec::new();
    let mut depth = 0;
    let mut in_str = false;
    let mut escape = false;
    let mut start = 0;
    for (i, c) in inner.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' {
            escape = true;
            continue;
        }
        if c == '"' {
            in_str = !in_str;
            continue;
        }
        if in_str {
            continue;
        }
        // BUG-513 第二面：`:` 不是嵌套开启符，只 `{`/`[` 才加深度。
        // 旧实现把 `:` 计入 depth，导致顶层分隔逗号在 depth==1 处被漏判，
        // `{"a": 6, "b": 9}` 被切成一整对（键 `"a"`、值 `6, "b": 9`）→ 解析 panic。
        if c == '{' || c == '[' {
            depth += 1;
        } else if c == '}' || c == ']' {
            depth -= 1;
        }
        if c == ',' && depth == 0 {
            pairs.push(&inner[start..i]);
            start = i + 1;
        }
    }
    pairs.push(&inner[start..]);
    for pair in pairs {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let colon = pair.find(':').unwrap_or_else(|| panic!(
            "embed 产物 {} 的 {} 返回值 `{}` 的键值对 `{}` 缺少冒号",
            artifact, method, raw, pair
        ));
        let key_raw = pair[..colon].trim();
        let val_raw = pair[colon+1..].trim();
        let key = if key_raw.starts_with('"') && key_raw.ends_with('"') && key_raw.len() >= 2 {
            &key_raw[1..key_raw.len()-1]
        } else {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 的键 `{}` 不是字符串", artifact, method, raw, key_raw)
        };
        let val = if val_raw.starts_with('"') && val_raw.ends_with('"') && val_raw.len() >= 2 {
            &val_raw[1..val_raw.len()-1]
        } else {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 的值 `{}` 不是字符串", artifact, method, raw, val_raw)
        };
        map.insert(key.to_string(), val.replace("\\\"", "\"").replace("\\\\", "\\"));
    }
    map
}

/// 解析 `{"1":100,"2":200}` 形式的 JSON 对象为 BTreeMap<i64, i64>（手写解析，键和值都是整数）
fn __lz_parse_dict_int_int(raw: &str, artifact: &str, method: &str) -> std::collections::BTreeMap<i64, i64> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 Dict<int,int> JSON 对象", artifact, method, raw);
    }
    let inner = &trimmed[1..trimmed.len()-1];
    let mut map = std::collections::BTreeMap::new();
    if inner.trim().is_empty() {
        return map;
    }
    let mut pairs: Vec<&str> = Vec::new();
    let mut depth = 0;
    let mut in_str = false;
    let mut escape = false;
    let mut start = 0;
    for (i, c) in inner.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' {
            escape = true;
            continue;
        }
        if c == '"' {
            in_str = !in_str;
            continue;
        }
        if in_str {
            continue;
        }
        // BUG-513 第二面：`:` 不是嵌套开启符，只 `{`/`[` 才加深度。
        // 旧实现把 `:` 计入 depth，导致顶层分隔逗号在 depth==1 处被漏判，
        // `{"a": 6, "b": 9}` 被切成一整对（键 `"a"`、值 `6, "b": 9`）→ 解析 panic。
        if c == '{' || c == '[' {
            depth += 1;
        } else if c == '}' || c == ']' {
            depth -= 1;
        }
        if c == ',' && depth == 0 {
            pairs.push(&inner[start..i]);
            start = i + 1;
        }
    }
    pairs.push(&inner[start..]);
    for pair in pairs {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let colon = pair.find(':').unwrap_or_else(|| panic!(
            "embed 产物 {} 的 {} 返回值 `{}` 的键值对 `{}` 缺少冒号",
            artifact, method, raw, pair
        ));
        let key_raw = pair[..colon].trim();
        let val_raw = pair[colon+1..].trim();
        let key_str = if key_raw.starts_with('"') && key_raw.ends_with('"') && key_raw.len() >= 2 {
            &key_raw[1..key_raw.len()-1]
        } else {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 的键 `{}` 不是字符串", artifact, method, raw, key_raw)
        };
        let key: i64 = key_str.parse().unwrap_or_else(|_| {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 的键 `{}` 不是整数", artifact, method, raw, key_str)
        });
        let val: i64 = val_raw.parse().unwrap_or_else(|_| {
            panic!("embed 产物 {} 的 {} 返回值 `{}` 的值 `{}` 不是整数", artifact, method, raw, val_raw)
        });
        map.insert(key, val);
    }
    map
}

/// 顶层逗号切分 JSON 数组内部（引号感知：字符串元素内的逗号/括号不参与切分）
fn __lz_split_json_array(raw: &str, artifact: &str, method: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 JSON 数组", artifact, method, raw);
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut escape = false;
    let mut depth: i32 = 0;
    for c in inner.chars() {
        if escape {
            cur.push(c);
            escape = false;
            continue;
        }
        match c {
            '\\' if in_str => { cur.push(c); escape = true; }
            '"' => { in_str = !in_str; cur.push(c); }
            '[' | '{' if !in_str => { depth += 1; cur.push(c); }
            ']' | '}' if !in_str => { depth -= 1; cur.push(c); }
            ',' if !in_str && depth == 0 => {
                parts.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

/// JSON 数组字符串元素解析（要求带引号，反转义 \" 与 \\）
fn __lz_json_elem_str(elem: &str, raw: &str, artifact: &str, method: &str, idx: usize) -> String {
    let t = elem.trim();
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        let inner = &t[1..t.len() - 1];
        inner.replace("\\\"", "\"").replace("\\\\", "\\")
    } else {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 第 {} 位不是字符串元素：`{}`", artifact, method, raw, idx, elem)
    }
}

/// JSON 数组 bool 元素解析（true/false 字面量）
fn __lz_json_elem_bool(elem: &str, raw: &str, artifact: &str, method: &str, idx: usize) -> bool {
    match elem.trim() {
        "true" => true,
        "false" => false,
        other => panic!("embed 产物 {} 的 {} 返回值 `{}` 第 {} 位不是 bool：`{}`", artifact, method, raw, idx, other),
    }
}

/// LZ 字符串 → JSON 字符串字面量（含引号与最小转义，参数方向用）
fn __lz_json_str_lit(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// 嵌套容器参数 → JSON 序列化：通用入口（T0r60）
/// 嵌套复合类型的参数值在 Rust 侧是结构化值，序列化为 JSON 字符串片段。
fn __lz_nested_to_json(val: &str) -> String {
    // 简化：参数值已是 JSON 字符串片段，直接返回
    val.to_string()
}

/// 嵌套容器参数 → JSON 序列化：list 元素递归序列化（T0r60）
/// __lz_nested_list_to_json(vec, inner_ty)
fn __lz_nested_list_to_json(vec: &[i64], _inner_ty: &str) -> String {
    let arr: Vec<String> = vec.iter().map(|x| x.to_string()).collect();
    format!("[{}]", arr.join(","))
}

/// 嵌套容器参数 → JSON 序列化：dict 值递归序列化（T0r60）
/// __lz_dict_nested_to_json(map, key_is_str, val_expr)
fn __lz_dict_nested_to_json(
    map: &std::collections::BTreeMap<String, String>,
    _key_is_str: bool,
    _val_expr: &str,
) -> String {
    // 简化：dict 值已是 JSON 字符串片段，直接拼装
    let mut pairs: Vec<String> = Vec::with_capacity(map.len());
    for (k, v) in map.iter() {
        pairs.push(format!("\"{}\":{}", k, v));
    }
    format!("{{{}}}", pairs.join(","))
}

/// 嵌套容器参数 → JSON 序列化：dict 通用入口（T0r60）
/// 简化实现：键值对拼接为 JSON 对象字符串
fn __lz_nested_dict_to_json(
    map: &std::collections::BTreeMap<String, i64>,
    _k_ty: &str,
    _v_ty: &str,
) -> String {
    let pairs: Vec<String> = map.iter().map(|(k, v)| format!("\"{}\":{}", k, v)).collect();
    format!("{{{}}}", pairs.join(","))
}

/// 嵌套容器参数 → JSON 序列化：tuple 通用入口（T0r60）
/// 简化实现：元素拼接为 JSON 数组字符串
fn __lz_nested_tuple_to_json(
    t: &(i64, i64),
    _elem_tys: &[&str],
) -> String {
    format!("[{},{}]", t.0, t.1)
}

/// 嵌套容器参数 → JSON 序列化：tuple 元素递归序列化（T0r60）
fn __lz_tuple_nested_to_json(
    t: &(String, String),
    _elem_exprs: &[String],
) -> String {
    format!("[{}]", t.0)
}

/// 嵌套 list 返回值 → 递归 JSON 解析（T0r60）
/// inner_ty: "int"/"float"/"str"/"bool"/"list<...>"/"dict<...>"/"tuple<...>"
fn __lz_parse_nested_list(raw: &str, artifact: &str, method: &str, inner_ty: &str) -> Vec<i64> {
    // 简化实现：委托给扁平解析器（int 场景）
    // 完整嵌套解析需要 serde_json，此处保持最小可用
    __lz_parse_list_int(raw, artifact, method)
}

/// 嵌套 dict 返回值 → 递归 JSON 解析（T0r60）
/// 根据 v_ty 递归解析：扁平值委托给扁平解析器，嵌套值在代码中解析
/// 返回类型由调用方（embed_wrapper_fn）的 embed_ret_ty_nested 决定
fn __lz_parse_dict_nested(
    raw: &str,
    _k_ty: &str,
    _v_ty: &str,
) -> std::collections::BTreeMap<String, String> {
    // 简化实现：解析 JSON 对象为 BTreeMap<String, String>
    // 完整嵌套解析需要 serde_json；此处保持最小可用，值作为 JSON 字符串片段保留
    let mut map = std::collections::BTreeMap::new();
    let trimmed = raw.trim().trim_start_matches('{').trim_end_matches('}');
    for pair in trimmed.split(',') {
        if pair.is_empty() {
            continue;
        }
        let parts: Vec<&str> = pair.splitn(2, ':').collect();
        if parts.len() == 2 {
            let k = parts[0].trim().trim_matches('"');
            let v = parts[1].trim();
            map.insert(k.to_string(), v.to_string());
        }
    }
    map
}

/// 嵌套 tuple 返回值 → 递归 JSON 解析（T0r60）
fn __lz_parse_nested_tuple(
    raw: &str,
    artifact: &str,
    method: &str,
    _elem_tys: &[&str],
) -> (i64, i64) {
    __lz_parse_tuple_int_int(raw, artifact, method)
}

/// (i64, i64) → JSON 数组字符串 [1,2]
fn __lz_tuple_int_int_to_json(t: &(i64, i64)) -> String {
    format!("[{},{}]", t.0, t.1)
}

/// (i64, f64) → JSON 数组字符串 [1,2.5]
fn __lz_tuple_int_float_to_json(t: &(i64, f64)) -> String {
    format!("[{},{}]", t.0, t.1)
}

/// (i64, String) → JSON 数组字符串 [1,"a"]
fn __lz_tuple_int_str_to_json(t: &(i64, String)) -> String {
    format!("[{},{}]", t.0, __lz_json_str_lit(&t.1))
}

/// (i64, bool) → JSON 数组字符串 [1,true]
fn __lz_tuple_int_bool_to_json(t: &(i64, bool)) -> String {
    format!("[{},{}]", t.0, if t.1 { "true" } else { "false" })
}

/// (f64, i64) → JSON 数组字符串 [1.5,2]
fn __lz_tuple_float_int_to_json(t: &(f64, i64)) -> String {
    format!("[{},{}]", t.0, t.1)
}

/// (f64, f64) → JSON 数组字符串 [1.5,2.5]
fn __lz_tuple_float_float_to_json(t: &(f64, f64)) -> String {
    format!("[{},{}]", t.0, t.1)
}

/// (f64, String) → JSON 数组字符串 [1.5,"a"]
fn __lz_tuple_float_str_to_json(t: &(f64, String)) -> String {
    format!("[{},{}]", t.0, __lz_json_str_lit(&t.1))
}

/// (f64, bool) → JSON 数组字符串 [1.5,true]
fn __lz_tuple_float_bool_to_json(t: &(f64, bool)) -> String {
    format!("[{},{}]", t.0, if t.1 { "true" } else { "false" })
}

/// (String, i64) → JSON 数组字符串 ["a",2]
fn __lz_tuple_str_int_to_json(t: &(String, i64)) -> String {
    format!("[{},{}]", __lz_json_str_lit(&t.0), t.1)
}

/// (String, f64) → JSON 数组字符串 ["a",2.5]
fn __lz_tuple_str_float_to_json(t: &(String, f64)) -> String {
    format!("[{},{}]", __lz_json_str_lit(&t.0), t.1)
}

/// (String, String) → JSON 数组字符串 ["a","b"]
fn __lz_tuple_str_str_to_json(t: &(String, String)) -> String {
    format!("[{},{}]", __lz_json_str_lit(&t.0), __lz_json_str_lit(&t.1))
}

/// (String, bool) → JSON 数组字符串 ["a",true]
fn __lz_tuple_str_bool_to_json(t: &(String, bool)) -> String {
    format!("[{},{}]", __lz_json_str_lit(&t.0), if t.1 { "true" } else { "false" })
}

/// (bool, i64) → JSON 数组字符串 [true,2]
fn __lz_tuple_bool_int_to_json(t: &(bool, i64)) -> String {
    format!("[{},{}]", if t.0 { "true" } else { "false" }, t.1)
}

/// (bool, f64) → JSON 数组字符串 [true,2.5]
fn __lz_tuple_bool_float_to_json(t: &(bool, f64)) -> String {
    format!("[{},{}]", if t.0 { "true" } else { "false" }, t.1)
}

/// (bool, String) → JSON 数组字符串 [true,"a"]
fn __lz_tuple_bool_str_to_json(t: &(bool, String)) -> String {
    format!("[{},{}]", if t.0 { "true" } else { "false" }, __lz_json_str_lit(&t.1))
}

/// (bool, bool) → JSON 数组字符串 [true,false]
fn __lz_tuple_bool_bool_to_json(t: &(bool, bool)) -> String {
    format!("[{},{}]", if t.0 { "true" } else { "false" }, if t.1 { "true" } else { "false" })
}

/// (i64, i64, i64) → JSON 数组字符串 [1,2,3]
fn __lz_tuple_int_int_int_to_json(t: &(i64, i64, i64)) -> String {
    format!("[{},{},{}]", t.0, t.1, t.2)
}

/// 解析 `[1,2]` 形式的 JSON 数组为 (i64, i64)
fn __lz_parse_tuple_int_int(raw: &str, artifact: &str, method: &str) -> (i64, i64) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<int,int> JSON 数组", artifact, method, raw);
    }
    (
        parts[0].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 0 位不是 int：{}", artifact, method, raw, e)),
        parts[1].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 1 位不是 int：{}", artifact, method, raw, e)),
    )
}

/// 解析 `[1,2.5]` 形式的 JSON 数组为 (i64, f64)
fn __lz_parse_tuple_int_float(raw: &str, artifact: &str, method: &str) -> (i64, f64) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<int,float> JSON 数组", artifact, method, raw);
    }
    (
        parts[0].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 0 位不是 int：{}", artifact, method, raw, e)),
        parts[1].trim().parse::<f64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 1 位不是 float：{}", artifact, method, raw, e)),
    )
}

/// 解析 `[1,"a"]` 形式的 JSON 数组为 (i64, String)
fn __lz_parse_tuple_int_str(raw: &str, artifact: &str, method: &str) -> (i64, String) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<int,str> JSON 数组", artifact, method, raw);
    }
    (
        parts[0].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 0 位不是 int：{}", artifact, method, raw, e)),
        __lz_json_elem_str(&parts[1], raw, artifact, method, 1),
    )
}

/// 解析 `[1,true]` 形式的 JSON 数组为 (i64, bool)
fn __lz_parse_tuple_int_bool(raw: &str, artifact: &str, method: &str) -> (i64, bool) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<int,bool> JSON 数组", artifact, method, raw);
    }
    (
        parts[0].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 0 位不是 int：{}", artifact, method, raw, e)),
        __lz_json_elem_bool(&parts[1], raw, artifact, method, 1),
    )
}

/// 解析 `[1.5,2]` 形式的 JSON 数组为 (f64, i64)
fn __lz_parse_tuple_float_int(raw: &str, artifact: &str, method: &str) -> (f64, i64) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<float,int> JSON 数组", artifact, method, raw);
    }
    (
        parts[0].trim().parse::<f64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 0 位不是 float：{}", artifact, method, raw, e)),
        parts[1].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 1 位不是 int：{}", artifact, method, raw, e)),
    )
}

/// 解析 `[1.5,2.5]` 形式的 JSON 数组为 (f64, f64)
fn __lz_parse_tuple_float_float(raw: &str, artifact: &str, method: &str) -> (f64, f64) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<float,float> JSON 数组", artifact, method, raw);
    }
    (
        parts[0].trim().parse::<f64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 0 位不是 float：{}", artifact, method, raw, e)),
        parts[1].trim().parse::<f64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 1 位不是 float：{}", artifact, method, raw, e)),
    )
}

/// 解析 `[1.5,"a"]` 形式的 JSON 数组为 (f64, String)
fn __lz_parse_tuple_float_str(raw: &str, artifact: &str, method: &str) -> (f64, String) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<float,str> JSON 数组", artifact, method, raw);
    }
    (
        parts[0].trim().parse::<f64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 0 位不是 float：{}", artifact, method, raw, e)),
        __lz_json_elem_str(&parts[1], raw, artifact, method, 1),
    )
}

/// 解析 `[1.5,true]` 形式的 JSON 数组为 (f64, bool)
fn __lz_parse_tuple_float_bool(raw: &str, artifact: &str, method: &str) -> (f64, bool) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<float,bool> JSON 数组", artifact, method, raw);
    }
    (
        parts[0].trim().parse::<f64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 0 位不是 float：{}", artifact, method, raw, e)),
        __lz_json_elem_bool(&parts[1], raw, artifact, method, 1),
    )
}

/// 解析 `["a",2]` 形式的 JSON 数组为 (String, i64)
fn __lz_parse_tuple_str_int(raw: &str, artifact: &str, method: &str) -> (String, i64) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<str,int> JSON 数组", artifact, method, raw);
    }
    (
        __lz_json_elem_str(&parts[0], raw, artifact, method, 0),
        parts[1].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 1 位不是 int：{}", artifact, method, raw, e)),
    )
}

/// 解析 `["a",2.5]` 形式的 JSON 数组为 (String, f64)
fn __lz_parse_tuple_str_float(raw: &str, artifact: &str, method: &str) -> (String, f64) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<str,float> JSON 数组", artifact, method, raw);
    }
    (
        __lz_json_elem_str(&parts[0], raw, artifact, method, 0),
        parts[1].trim().parse::<f64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 1 位不是 float：{}", artifact, method, raw, e)),
    )
}

/// 解析 `["a","b"]` 形式的 JSON 数组为 (String, String)
fn __lz_parse_tuple_str_str(raw: &str, artifact: &str, method: &str) -> (String, String) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<str,str> JSON 数组", artifact, method, raw);
    }
    (
        __lz_json_elem_str(&parts[0], raw, artifact, method, 0),
        __lz_json_elem_str(&parts[1], raw, artifact, method, 1),
    )
}

/// 解析 `["a",true]` 形式的 JSON 数组为 (String, bool)
fn __lz_parse_tuple_str_bool(raw: &str, artifact: &str, method: &str) -> (String, bool) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<str,bool> JSON 数组", artifact, method, raw);
    }
    (
        __lz_json_elem_str(&parts[0], raw, artifact, method, 0),
        __lz_json_elem_bool(&parts[1], raw, artifact, method, 1),
    )
}

/// 解析 `[true,2]` 形式的 JSON 数组为 (bool, i64)
fn __lz_parse_tuple_bool_int(raw: &str, artifact: &str, method: &str) -> (bool, i64) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<bool,int> JSON 数组", artifact, method, raw);
    }
    (
        __lz_json_elem_bool(&parts[0], raw, artifact, method, 0),
        parts[1].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 1 位不是 int：{}", artifact, method, raw, e)),
    )
}

/// 解析 `[true,2.5]` 形式的 JSON 数组为 (bool, f64)
fn __lz_parse_tuple_bool_float(raw: &str, artifact: &str, method: &str) -> (bool, f64) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<bool,float> JSON 数组", artifact, method, raw);
    }
    (
        __lz_json_elem_bool(&parts[0], raw, artifact, method, 0),
        parts[1].trim().parse::<f64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 1 位不是 float：{}", artifact, method, raw, e)),
    )
}

/// 解析 `[true,"a"]` 形式的 JSON 数组为 (bool, String)
fn __lz_parse_tuple_bool_str(raw: &str, artifact: &str, method: &str) -> (bool, String) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<bool,str> JSON 数组", artifact, method, raw);
    }
    (
        __lz_json_elem_bool(&parts[0], raw, artifact, method, 0),
        __lz_json_elem_str(&parts[1], raw, artifact, method, 1),
    )
}

/// 解析 `[true,false]` 形式的 JSON 数组为 (bool, bool)
fn __lz_parse_tuple_bool_bool(raw: &str, artifact: &str, method: &str) -> (bool, bool) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 2 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 2 元 Tuple<bool,bool> JSON 数组", artifact, method, raw);
    }
    (
        __lz_json_elem_bool(&parts[0], raw, artifact, method, 0),
        __lz_json_elem_bool(&parts[1], raw, artifact, method, 1),
    )
}

/// 解析 `[1,2,3]` 形式的 JSON 数组为 (i64, i64, i64)
fn __lz_parse_tuple_int_int_int(raw: &str, artifact: &str, method: &str) -> (i64, i64, i64) {
    let parts = __lz_split_json_array(raw, artifact, method);
    if parts.len() != 3 {
        panic!("embed 产物 {} 的 {} 返回值 `{}` 不是 3 元 Tuple<int,int,int> JSON 数组", artifact, method, raw);
    }
    (
        parts[0].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 0 位不是 int：{}", artifact, method, raw, e)),
        parts[1].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 1 位不是 int：{}", artifact, method, raw, e)),
        parts[2].trim().parse::<i64>().unwrap_or_else(|e| panic!("embed 产物 {} 的 {} 返回值 `{}` 第 2 位不是 int：{}", artifact, method, raw, e)),
    )
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
    let (__prog, __pre) = __lz_spawn_plan(&dir, program);
    let mut cmd = Command::new(__prog);
    cmd.args(&__pre);
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
    let (__prog, __pre) = __lz_spawn_plan(&dir, program);
    let mut cmd = Command::new(__prog);
    cmd.args(&__pre);
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

/// LZ 嵌套复合类型 → Rust 形参类型（T0r60）。
/// 递归解析 list/dict/tuple 嵌套结构，生成具体 Rust 类型。
/// 例：dict<str,list<int>> → std::collections::BTreeMap<String, Vec<i64>>
///     list<tuple<int,int>> → Vec<(i64, i64)>
fn embed_param_ty_nested(lz: &str) -> Option<&'static str> {
    // 处理所有容器类型（扁平+嵌套）：list<...>/dict<...>/tuple<...>
    // 基类型（int/float/str/bool）不在此处理，由 embed_param_ty 的扁平 match 兜底
    let (base, args) = split_lz_generic(lz);
    match base.as_str() {
        "list" => {
            let args = args?;
            let inner = args.first()?;
            let inner_ty = embed_param_ty_nested(inner).or_else(|| {
                // 扁平内层类型
                Some(match inner.as_str() {
                    "int" => "i64",
                    "float" => "f64",
                    "str" => "String",
                    "bool" => "bool",
                    _ => return None,
                })
            })?;
            Some(Box::leak(format!("Vec<{}>", inner_ty).into_boxed_str()))
        }
        "dict" => {
            let args = args?;
            let k = args.first()?;
            let v = args.get(1)?;
            let k_ty = match k.as_str() {
                "str" => "String",
                "int" => "i64",
                _ => return None,
            };
            let v_ty = embed_param_ty_nested(v).or_else(|| {
                Some(match v.as_str() {
                    "int" => "i64",
                    "float" => "f64",
                    "str" => "String",
                    "bool" => "bool",
                    _ => return None,
                })
            })?;
            Some(Box::leak(format!(
                "std::collections::BTreeMap<{}, {}>",
                k_ty, v_ty
            )
            .into_boxed_str()))
        }
        "tuple" => {
            // Tuple<int,int> → (i64, i64)；Tuple<int,str> → (i64, String)
            let elem_tys: Vec<String> = args?
                .iter()
                .map(|a| {
                    embed_param_ty_nested(a)
                        .map(|t| t.to_string())
                        .or_else(|| {
                            Some(match a.as_str() {
                                "int" => "i64".to_string(),
                                "float" => "f64".to_string(),
                                "str" => "String".to_string(),
                                "bool" => "bool".to_string(),
                                _ => return None,
                            })
                        })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Box::leak(format!("({})", elem_tys.join(", ")).into_boxed_str()))
        }
        _ => None,
    }
}

/// LZ 嵌套复合类型 → Rust 返回类型（T0r60）。与 embed_param_ty_nested 同口径。
fn embed_ret_ty_nested(lz: &str) -> Option<&'static str> {
    // 处理所有容器类型（扁平+嵌套）：list<...>/dict<...>/tuple<...>
    // 基类型不在此处理，由 embed_ret_ty 的扁平 match 兜底
    let (base, args) = split_lz_generic(lz);
    match base.as_str() {
        "list" => {
            let args = args?;
            let inner = args.first()?;
            let inner_ty = embed_ret_ty_nested(inner).or_else(|| {
                Some(match inner.as_str() {
                    "int" => "i64",
                    "float" => "f64",
                    "str" => "String",
                    "bool" => "bool",
                    _ => return None,
                })
            })?;
            Some(Box::leak(format!("Vec<{}>", inner_ty).into_boxed_str()))
        }
        "dict" => {
            let args = args?;
            let k = args.first()?;
            let v = args.get(1)?;
            let k_ty = match k.as_str() {
                "str" => "String",
                "int" => "i64",
                _ => return None,
            };
            // T0r61：嵌套 dict 返回值按值类型递归表达真实嵌套结构，
            // 不再降级为 String 片段。dict<str,list<int>> → BTreeMap<String, Vec<i64>>
            // 值类型递归走 embed_ret_ty_nested，嵌套容器（list/dict/tuple）与扁平基类型均覆盖。
            let v_ty = embed_ret_ty_nested(v).or_else(|| {
                Some(match v.as_str() {
                    "int" => "i64",
                    "float" => "f64",
                    "str" => "String",
                    "bool" => "bool",
                    _ => return None,
                })
            })?;
            Some(Box::leak(format!(
                "std::collections::BTreeMap<{}, {}>",
                k_ty, v_ty
            )
            .into_boxed_str()))
        }
        "tuple" => {
            // 嵌套 tuple 返回值：嵌套元素以 JSON 字符串片段形式返回
            // 扁平元素保持具体类型
            let args = args?;
            let mut elem_tys: Vec<String> = Vec::new();
            for a in args.iter() {
                if is_lz_container(a) {
                    elem_tys.push("String".to_string());
                } else {
                    let ty = embed_ret_ty_nested(a)
                        .map(|t| t.to_string())
                        .or_else(|| {
                            Some(match a.as_str() {
                                "int" => "i64".to_string(),
                                "float" => "f64".to_string(),
                                "str" => "String".to_string(),
                                "bool" => "bool".to_string(),
                                _ => return None,
                            })
                        });
                    elem_tys.push(ty?);
                }
            }
            Some(Box::leak(format!("({})", elem_tys.join(", ")).into_boxed_str()))
        }
        _ => None,
    }
}

/// 把 LZ 类型字符串 `dict<str,list<int>>` 拆成 ("dict", Some(["str", "list<int>"]))
/// 顶层逗号切分（与 adapters.rs::split_top_commas 同口径）。
fn split_lz_generic(lz: &str) -> (String, Option<Vec<String>>) {
    let t = lz.trim().replace(' ', "");
    let chars: Vec<char> = t.chars().collect();
    let mut open = None;
    for (i, &c) in chars.iter().enumerate() {
        if c == '<' {
            open = Some(i);
            break;
        }
    }
    let open_idx = match open {
        Some(i) => i,
        None => return (t.clone(), None),
    };
    let mut depth = 0i32;
    let mut close = None;
    for i in open_idx..chars.len() {
        match chars[i] {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close_idx = match close {
        Some(i) => i,
        None => return (t.clone(), None),
    };
    let base: String = chars[..open_idx].iter().collect();
    let inner: String = chars[open_idx + 1..close_idx].iter().collect();
    // 顶层逗号切分
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut d = 0i32;
    for c in inner.chars() {
        match c {
            '<' => {
                d += 1;
                cur.push(c);
            }
            '>' => {
                d -= 1;
                cur.push(c);
            }
            ',' if d == 0 => {
                parts.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    (base, Some(parts))
}

/// 判断 LZ 类型是否为嵌套容器（T0r60）。
/// 嵌套 = 至少一层容器嵌套（如 dict<str,list<int>>），扁平类型（如 dict<str,int>）不算。
fn is_lz_nested_container(lz: &str) -> bool {
    // 复用 adapters.rs 的实现（T0r60），保持单一真源
    crate::embed::adapters::is_nested_container(lz)
}

/// 生成嵌套复合类型返回值的 JSON 解析调用（T0r60）。
/// 返回完整的 Rust 表达式（含 __r 参数），用于 embed_wrapper_fn 直接输出。
fn embed_ret_parse_expr(lz: &str) -> String {
    let (base, args) = split_lz_generic(lz);
    match base.as_str() {
        "list" => {
            let inner = args.as_ref().and_then(|a| a.first()).map(|s| s.as_str()).unwrap_or("int");
            format!(
                "__lz_parse_nested_list(&__r, \"{}\")",
                inner
            )
        }
        "dict" => {
            let k = args.as_ref().and_then(|a| a.first()).map(|s| s.as_str()).unwrap_or("str");
            let v = args.as_ref().and_then(|a| a.get(1)).map(|s| s.as_str()).unwrap_or("int");
            // T0r61：嵌套 dict 返回值按值类型递归解析 JSON，还原嵌套结构，
            // 不再降级为 String 片段。dict<str,list<int>> 的 value 递归解析为 Vec<i64>。
            // 生成内联递归解析代码：遍历 JSON 对象的键值对，对每个 value 按 v 类型递归解析。
            let v_parse = embed_ret_parse_elem(v);
            format!(
                "{{ let __raw = __r; let mut __map = std::collections::BTreeMap::<String, _>::new(); for __pair in __raw.trim_matches(|c| c=='{{'||c=='}}').split(',') {{ if __pair.is_empty() {{ continue; }} let __parts: Vec<&str> = __pair.splitn(2, ':').collect(); if __parts.len() == 2 {{ let __k = __parts[0].trim().trim_matches('\"'); let __v_str = __parts[1].trim(); __map.insert(__k.to_string(), __lz_parse_nested_value(__v_str, \"{}\")); }} }} __map }}",
                v
            ).replace(&format!("__lz_parse_nested_value(__v_str, \"{}\"", v), &v_parse)
        }
        "tuple" => {
            let elem_tys = args
                .unwrap_or_default()
                .iter()
                .map(|a| a.as_str())
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "__lz_parse_nested_tuple(&__r, &[\"{}\"])",
                elem_tys
            )
        }
        _ => "__lz_parse_nested_json(&__r)".to_string(),
    }
}

/// T0r61：按 LZ 类型生成单值 JSON 片段的递归解析表达式。
/// `__v_str` 是待解析的 JSON 片段字符串（如 `"[1,2,3]"`、`"{\"k\":1}"`、`"42"`）。
/// 标量直接 parse，容器递归解析还原嵌套结构，返回带类型标注的 Rust 表达式。
fn embed_ret_parse_elem(lz: &str) -> String {
    let (base, args) = split_lz_generic(lz);
    match base.as_str() {
        "int" => "__v_str.parse::<i64>().unwrap_or(0)".to_string(),
        "float" => "__v_str.parse::<f64>().unwrap_or(0.0)".to_string(),
        "str" => {
            // 字符串值：JSON 中是带引号的，去掉首尾引号
            "__v_str.trim_matches('\"').to_string()".to_string()
        }
        "bool" => "(__v_str == \"true\" || __v_str == \"1\")".to_string(),
        "list" => {
            let inner = args.as_ref().and_then(|a| a.first()).map(|s| s.as_str()).unwrap_or("int");
            let elem_parse = embed_ret_parse_elem(inner);
            format!(
                "{{ let __arr: Vec<_> = __v_str.trim_matches(|c| c=='['||c==']').split(',').filter(|s| !s.trim().is_empty()).map(|s| {}).collect(); __arr }}",
                elem_parse
            )
        }
        "dict" => {
            // 嵌套 dict 值：内联递归解析 JSON 对象，避免依赖外部运行时函数
            // 生成局部闭包，对每个键值对递归解析 value
            let inner_v = args.as_ref().and_then(|a| a.get(1)).map(|s| s.as_str()).unwrap_or("int");
            let inner_parse = embed_ret_parse_elem(inner_v);
            // 闭包主体为普通字符串常量（避开 format! 的 {} 转义），在插入位用占位符替换
            let closure_body = "{ let __parse_val = |__s: &str| -> _ { let __trimmed = __s.trim().trim_start_matches('{').trim_end_matches('}'); let mut __m = std::collections::BTreeMap::<String, _>::new(); for __pair in __trimmed.split(',') { if __pair.is_empty() { continue; } let __parts: Vec<&str> = __pair.splitn(2, ':').collect(); if __parts.len() == 2 { let __k = __parts[0].trim().trim_matches('\"'); let __vs = __parts[1].trim(); __m.insert(__k.to_string(), __INNER__); } } __m } }; __parse_val(__v_str) }";
            closure_body.replace("__INNER__", &inner_parse)
        }
        "tuple" => {
            let elem_tys = args
                .unwrap_or_default()
                .iter()
                .map(|a| a.as_str())
                .collect::<Vec<_>>()
                .join(",");
            format!("__lz_parse_nested_tuple(__v_str, &[\"{}\"])", elem_tys)
        }
        _ => "__v_str.to_string()".to_string(),
    }
}

/// 判断 LZ 类型是否为扁平容器（无嵌套）。
fn is_lz_flat_container(lz: &str) -> bool {
    let (base, args) = split_lz_generic(lz);
    match base.as_str() {
        "list" => {
            let inner = args.as_ref().and_then(|a| a.first()).map(|s| s.as_str()).unwrap_or("");
            !is_lz_container(inner)
        }
        "dict" => {
            let k = args.as_ref().and_then(|a| a.first()).map(|s| s.as_str()).unwrap_or("");
            let v = args.as_ref().and_then(|a| a.get(1)).map(|s| s.as_str()).unwrap_or("");
            !is_lz_container(k) && !is_lz_container(v)
        }
        "tuple" => {
            let elems = args.as_ref().map(|a| a.as_slice()).unwrap_or(&[]);
            elems.iter().all(|e| !is_lz_container(e))
        }
        _ => false,
    }
}

/// 判断 LZ 类型是否为容器（扁平或嵌套）。
fn is_lz_container(lz: &str) -> bool {
    let (base, _) = split_lz_generic(lz);
    matches!(base.as_str(), "list" | "dict" | "tuple")
}

/// 嵌套复合类型参数 → JSON 序列化表达式（T0r60）。
/// 递归生成 JSON 字符串，由外层 Python 侧 json.loads 解码。

fn embed_arg_expr_nested(ident: &str, lz: &str) -> Option<String> {
    // 只处理嵌套容器类型；扁平类型走 embed_arg_expr 的扁平 match
    // （扁平 tuple 有专门的 __lz_tuple_*_to_json 函数，不能用通用 nested 版本）
    if !is_lz_nested_container(lz) {
        return None;
    }
    let (base, args) = split_lz_generic(lz);
    match base.as_str() {
        "list" => {
            let inner = args.as_ref().and_then(|a| a.first()).map(|s| s.as_str()).unwrap_or("int");
            let elem_expr = embed_elem_to_json(inner);
            Some(format!(
                "{{ let __arr: Vec<String> = {}.iter().map(|x| {}).collect(); \"[\".to_string() + &__arr.join(\",\") + \"]\" }}",
                ident, elem_expr
            ))
        }
        "dict" => {
            let k = args.as_ref().and_then(|a| a.first()).map(|s| s.as_str()).unwrap_or("str");
            let v = args.as_ref().and_then(|a| a.get(1)).map(|s| s.as_str()).unwrap_or("int");
            let v_expr = embed_elem_to_json(v);
            // 直接生成 JSON 序列化代码：遍历 dict 的键值对，递归序列化 value
            // v_expr 中的 x 是迭代变量名，需要替换为 v（dict 迭代的 value 变量名）
            let v_expr_for_dict = v_expr.replace("x", "v");
            // 使用 format! 拼接 JSON 键值对，注意转义 format! 的占位符
            Some(format!(
                "{{ let __pairs: Vec<String> = {}.iter().map(|(_k, v)| format!(\"\\\"{{}}\\\":{{}}\", _k, {} )).collect(); \"{{\".to_string() + &__pairs.join(\",\") + \"}}\" }}",
                ident, v_expr_for_dict
            ))
        }
        "tuple" => {
            let elem_exprs: Vec<String> = args
                .as_ref()
                .map(|a| a.iter().enumerate().map(|(i, _)| embed_elem_to_json_idx(i).to_string()).collect())
                .unwrap_or_default();
            Some(format!(
                "{{ __lz_tuple_nested_to_json({}, &[{}]) }}",
                ident,
                elem_exprs.join(", ")
            ))
        }
        _ => None,
    }
}

/// 生成 list 元素 → JSON 片段（T0r60）。`x` 是迭代变量名。
fn embed_elem_to_json(lz: &str) -> String {
    let (base, args) = split_lz_generic(lz);
    match base.as_str() {
        "int" | "float" => "x.to_string()".to_string(),
        "str" => "format!(\"\\\"{{}}\\\"\", x)".to_string(),
        "bool" => "(if x { \"true\" } else { \"false\" }).to_string()".to_string(),
        "list" => {
            let inner = args.as_ref().and_then(|a| a.first()).map(|s| s.as_str()).unwrap_or("int");
            let elem_expr = embed_elem_to_json(inner);
            format!(
                "{{ let __arr: Vec<String> = x.iter().map(|x| {}).collect(); \"[\".to_string() + &__arr.join(\",\") + \"]\" }}",
                elem_expr
            )
        }
        "dict" => {
            let k = args.as_ref().and_then(|a| a.first()).map(|s| s.as_str()).unwrap_or("str");
            let v = args.as_ref().and_then(|a| a.get(1)).map(|s| s.as_str()).unwrap_or("int");
            let k_is_str = k == "str";
            let v_expr = embed_elem_to_json(v);
            format!(
                "{{ __lz_dict_nested_to_json(x, {}, {}) }}",
                k_is_str, v_expr
            )
        }
        "tuple" => {
            let elem_exprs: Vec<String> = args
                .as_ref()
                .map(|a| a.iter().enumerate().map(|(i, _)| embed_elem_to_json_idx(i).to_string()).collect())
                .unwrap_or_default();
            format!(
                "{{ __lz_tuple_nested_to_json(x, &[{}]) }}",
                elem_exprs.join(", ")
            )
        }
        _ => "x.to_string()".to_string(),
    }
}

/// 生成 tuple 元素索引的 JSON 片段（T0r60）。
fn embed_elem_to_json_idx(idx: usize) -> String {
    match idx {
        0 => "x.0.to_string()".to_string(),
        1 => "x.1.to_string()".to_string(),
        _ => format!("x.{}.to_string()", idx),
    }
}

fn embed_param_ty(lz: &str) -> &'static str {
    // T0r60：嵌套复合类型委托给 embed_param_ty_nested
    if let Some(ty) = embed_param_ty_nested(lz) {
        return ty;
    }
    match lz {
        "int" => "i64",
        "float" => "f64",
        "bool" => "bool",
        // LZ 调点对 str 实参可能给 `&str`（字面量直传）也可能给 `String`
        // （codegen 对未知形参类型的默认是 `.to_string()`），两种都得收。
        "str" => "impl Into<String>",
        "list<int>" => "Vec<i64>",
        "list<float>" => "Vec<f64>",
        "list<str>" => "Vec<String>",
        "dict<str,int>" => "std::collections::BTreeMap<String, i64>",
        "dict<str,float>" => "std::collections::BTreeMap<String, f64>",
        "dict<str,str>" => "std::collections::BTreeMap<String, String>",
        "dict<int,int>" => "std::collections::BTreeMap<i64, i64>",
        "tuple<int,int>" => "(i64, i64)",
        "tuple<int,float>" => "(i64, f64)",
        "tuple<int,str>" => "(i64, String)",
        "tuple<int,bool>" => "(i64, bool)",
        "tuple<float,int>" => "(f64, i64)",
        "tuple<float,float>" => "(f64, f64)",
        "tuple<float,str>" => "(f64, String)",
        "tuple<float,bool>" => "(f64, bool)",
        "tuple<str,int>" => "(String, i64)",
        "tuple<str,float>" => "(String, f64)",
        "tuple<str,str>" => "(String, String)",
        "tuple<str,bool>" => "(String, bool)",
        "tuple<bool,int>" => "(bool, i64)",
        "tuple<bool,float>" => "(bool, f64)",
        "tuple<bool,str>" => "(bool, String)",
        "tuple<bool,bool>" => "(bool, bool)",
        "tuple<int,int,int>" => "(i64, i64, i64)",
        _ => "&str",
    }
}

fn embed_ret_ty(ret: &str) -> &'static str {
    // T0r60：嵌套复合类型委托给 embed_ret_ty_nested
    if let Some(ty) = embed_ret_ty_nested(ret) {
        return ty;
    }
    match ret {
        "int" => "i64",
        "float" => "f64",
        "bool" => "bool",
        "str" => "String",
        "list<int>" => "Vec<i64>",
        "list<float>" => "Vec<f64>",
        "list<str>" => "Vec<String>",
        "dict<str,int>" => "std::collections::BTreeMap<String, i64>",
        "dict<str,float>" => "std::collections::BTreeMap<String, f64>",
        "dict<str,str>" => "std::collections::BTreeMap<String, String>",
        "dict<int,int>" => "std::collections::BTreeMap<i64, i64>",
        "tuple<int,int>" => "(i64, i64)",
        "tuple<int,float>" => "(i64, f64)",
        "tuple<int,str>" => "(i64, String)",
        "tuple<int,bool>" => "(i64, bool)",
        "tuple<float,int>" => "(f64, i64)",
        "tuple<float,float>" => "(f64, f64)",
        "tuple<float,str>" => "(f64, String)",
        "tuple<float,bool>" => "(f64, bool)",
        "tuple<str,int>" => "(String, i64)",
        "tuple<str,float>" => "(String, f64)",
        "tuple<str,str>" => "(String, String)",
        "tuple<str,bool>" => "(String, bool)",
        "tuple<bool,int>" => "(bool, i64)",
        "tuple<bool,float>" => "(bool, f64)",
        "tuple<bool,str>" => "(bool, String)",
        "tuple<bool,bool>" => "(bool, bool)",
        "tuple<int,int,int>" => "(i64, i64, i64)",
        _ => "()",
    }
}

fn embed_arg_expr(ident: &str, lz: &str) -> String {
    // T0r60：嵌套复合类型 → 递归 JSON 序列化
    // list<...> → JSON 数组 [elem, elem, ...]
    // dict<k,v> → JSON 对象 {"key":value,...}
    // tuple<...> → JSON 数组 [elem1, elem2, ...]
    if let Some(expr) = embed_arg_expr_nested(ident, lz) {
        return expr;
    }
    match lz {
        "bool" => format!("(if {} {{ \"true\" }} else {{ \"false\" }}).to_string()", ident),
        "str" => format!("{}.into()", ident),
        "list<int>" => format!(
            "{{ let __arr: Vec<String> = {}.iter().map(|&x| x.to_string()).collect(); \"[\".to_string() + &__arr.join(\",\") + \"]\" }}",
            ident
        ),
        "list<float>" => format!(
            "{{ let __arr: Vec<String> = {}.iter().map(|x| x.to_string()).collect(); \"[\".to_string() + &__arr.join(\",\") + \"]\" }}",
            ident
        ),
        "list<str>" => format!(
            "{{ let __arr: Vec<String> = {}.iter().map(|x| format!(\"\\\"{{}}\\\"\", x)).collect(); \"[\".to_string() + &__arr.join(\",\") + \"]\" }}",
            ident
        ),
        "dict<str,int>" => format!(
            "{{ __lz_dict_to_json({}) }}",
            ident
        ),
        "dict<str,float>" => format!(
            "{{ __lz_dict_to_json_float({}) }}",
            ident
        ),
        "dict<str,str>" => format!(
            "{{ __lz_dict_to_json_str({}) }}",
            ident
        ),
        "dict<int,int>" => format!(
            "{{ __lz_dict_int_int_to_json({}) }}",
            ident
        ),
        "tuple<int,int>" => format!(
            "{{ __lz_tuple_int_int_to_json(&{}) }}",
            ident
        ),
        "tuple<int,float>" => format!(
            "{{ __lz_tuple_int_float_to_json(&{}) }}",
            ident
        ),
        "tuple<int,str>" => format!(
            "{{ __lz_tuple_int_str_to_json(&{}) }}",
            ident
        ),
        "tuple<int,bool>" => format!(
            "{{ __lz_tuple_int_bool_to_json(&{}) }}",
            ident
        ),
        "tuple<float,int>" => format!(
            "{{ __lz_tuple_float_int_to_json(&{}) }}",
            ident
        ),
        "tuple<float,float>" => format!(
            "{{ __lz_tuple_float_float_to_json(&{}) }}",
            ident
        ),
        "tuple<float,str>" => format!(
            "{{ __lz_tuple_float_str_to_json(&{}) }}",
            ident
        ),
        "tuple<float,bool>" => format!(
            "{{ __lz_tuple_float_bool_to_json(&{}) }}",
            ident
        ),
        "tuple<str,int>" => format!(
            "{{ __lz_tuple_str_int_to_json(&{}) }}",
            ident
        ),
        "tuple<str,float>" => format!(
            "{{ __lz_tuple_str_float_to_json(&{}) }}",
            ident
        ),
        "tuple<str,str>" => format!(
            "{{ __lz_tuple_str_str_to_json(&{}) }}",
            ident
        ),
        "tuple<str,bool>" => format!(
            "{{ __lz_tuple_str_bool_to_json(&{}) }}",
            ident
        ),
        "tuple<bool,int>" => format!(
            "{{ __lz_tuple_bool_int_to_json(&{}) }}",
            ident
        ),
        "tuple<bool,float>" => format!(
            "{{ __lz_tuple_bool_float_to_json(&{}) }}",
            ident
        ),
        "tuple<bool,str>" => format!(
            "{{ __lz_tuple_bool_str_to_json(&{}) }}",
            ident
        ),
        "tuple<bool,bool>" => format!(
            "{{ __lz_tuple_bool_bool_to_json(&{}) }}",
            ident
        ),
        "tuple<int,int,int>" => format!(
            "{{ __lz_tuple_int_int_int_to_json(&{}) }}",
            ident
        ),
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
            "list<int>" => out.push_str(&format!(
                "    __lz_parse_list_int(&__r, {}, {})\n",
                art, method
            )),
            "list<float>" => out.push_str(&format!(
                "    __lz_parse_list_float(&__r, {}, {})\n",
                art, method
            )),
            "list<str>" => out.push_str(&format!(
                "    __lz_parse_list_str(&__r, {}, {})\n",
                art, method
            )),
            "dict<str,int>" => out.push_str(&format!(
                "    __lz_parse_dict_str_int(&__r, {}, {})\n",
                art, method
            )),
            "dict<str,float>" => out.push_str(&format!(
                "    __lz_parse_dict_str_float(&__r, {}, {})\n",
                art, method
            )),
            "dict<str,str>" => out.push_str(&format!(
                "    __lz_parse_dict_str_str(&__r, {}, {})\n",
                art, method
            )),
            "dict<int,int>" => out.push_str(&format!(
                "    __lz_parse_dict_int_int(&__r, {}, {})\n",
                art, method
            )),
            "tuple<int,int>" => out.push_str(&format!(
                "    __lz_parse_tuple_int_int(&__r, {}, {})\n",
                art, method
            )),
            "tuple<int,float>" => out.push_str(&format!(
                "    __lz_parse_tuple_int_float(&__r, {}, {})\n",
                art, method
            )),
            "tuple<int,str>" => out.push_str(&format!(
                "    __lz_parse_tuple_int_str(&__r, {}, {})\n",
                art, method
            )),
            "tuple<int,bool>" => out.push_str(&format!(
                "    __lz_parse_tuple_int_bool(&__r, {}, {})\n",
                art, method
            )),
            "tuple<float,int>" => out.push_str(&format!(
                "    __lz_parse_tuple_float_int(&__r, {}, {})\n",
                art, method
            )),
            "tuple<float,float>" => out.push_str(&format!(
                "    __lz_parse_tuple_float_float(&__r, {}, {})\n",
                art, method
            )),
            "tuple<float,str>" => out.push_str(&format!(
                "    __lz_parse_tuple_float_str(&__r, {}, {})\n",
                art, method
            )),
            "tuple<float,bool>" => out.push_str(&format!(
                "    __lz_parse_tuple_float_bool(&__r, {}, {})\n",
                art, method
            )),
            "tuple<str,int>" => out.push_str(&format!(
                "    __lz_parse_tuple_str_int(&__r, {}, {})\n",
                art, method
            )),
            "tuple<str,float>" => out.push_str(&format!(
                "    __lz_parse_tuple_str_float(&__r, {}, {})\n",
                art, method
            )),
            "tuple<str,str>" => out.push_str(&format!(
                "    __lz_parse_tuple_str_str(&__r, {}, {})\n",
                art, method
            )),
            "tuple<str,bool>" => out.push_str(&format!(
                "    __lz_parse_tuple_str_bool(&__r, {}, {})\n",
                art, method
            )),
            "tuple<bool,int>" => out.push_str(&format!(
                "    __lz_parse_tuple_bool_int(&__r, {}, {})\n",
                art, method
            )),
            "tuple<bool,float>" => out.push_str(&format!(
                "    __lz_parse_tuple_bool_float(&__r, {}, {})\n",
                art, method
            )),
            "tuple<bool,str>" => out.push_str(&format!(
                "    __lz_parse_tuple_bool_str(&__r, {}, {})\n",
                art, method
            )),
            "tuple<bool,bool>" => out.push_str(&format!(
                "    __lz_parse_tuple_bool_bool(&__r, {}, {})\n",
                art, method
            )),
            "tuple<int,int,int>" => out.push_str(&format!(
                "    __lz_parse_tuple_int_int_int(&__r, {}, {})\n",
                art, method
            )),
            // T0r60：嵌套复合类型返回值 → 递归 JSON 解析
            ret if is_lz_nested_container(ret) => {
                let parse_expr = embed_ret_parse_expr(ret);
                out.push_str(&format!("    {}\n", parse_expr));
            }
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

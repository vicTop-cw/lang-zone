// Lang-Zong 编译器 — bridge_cli.rs
// Level 3: CLI 序列化桥接
// 跨进程序列化通信，通过 stdin/stdout 请求-响应协议调用外部程序。
// 实现 Bridge trait，生成子进程管理的 Rust 代码。

use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeLevel, BridgeMeta,
    CallResolveResult, ExportEntry, ExportKind,
};
use std::collections::HashMap;
use std::time::Duration;

// ──────────────── CLI 端点配置 ────────────────

/// CLI 桥接端点配置
#[derive(Debug, Clone)]
pub struct CliEndpoint {
    pub name: String,
    pub command: String,          // 可执行文件路径
    pub args: Vec<String>,        // 命令行参数
    pub timeout_ms: u64,          // 超时（毫秒）
    pub max_retries: u32,         // 最大重试次数
    pub format: SerializationFormat, // 序列化格式
    pub pool_size: usize,         // 进程池大小（0=无池，每次启动新进程）
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
        CliBridge { endpoints: HashMap::new() }
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
    fn name(&self) -> &str { "cli" }

    fn level(&self) -> BridgeLevel { BridgeLevel::InterProcess }

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
        self.gen_call(func_name, _args).map(|rust_path| {
            CallResolveResult {
                rust_path,
                shim: String::new(),
                module_name: "cli".into(),
                is_macro: false,
                is_template: false,
            }
        })
    }

    fn list_exports(&self, kind: ExportKind) -> Vec<ExportEntry> {
        match kind {
            ExportKind::Function => {
                self.endpoints.keys().map(|name| ExportEntry {
                    name: name.clone(),
                    kind: ExportKind::Function,
                    signature: format!("cli endpoint: {}", name),
                    module: "cli".into(),
                }).collect()
            }
            _ => vec![],
        }
    }

    fn export_count(&self) -> usize {
        self.endpoints.len()
    }
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

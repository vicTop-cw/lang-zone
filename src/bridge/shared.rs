// Lang-Zong 编译器 — bridge/shared.rs
// Level 4: 共享内存桥接（Zinc）
// 基于 mmap 的零拷贝跨进程通信，src-Zinc 模式（sender/receiver typed channels）。
// 实现 Bridge trait，从 TOML 清单读取段声明并生成 zinc 操作代码。

use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeError, BridgeLevel, BridgeMeta,
    CallResolveResult, ErrorCode, ExportEntry, ExportKind,
};
use crate::util::parse;
use crate::util::mini_toml::TomlValue;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ──────────────── Zinc 声明 ────────────────

/// 共享内存段上的 typed channel 声明
#[derive(Debug, Clone)]
pub struct ChannelDef {
    pub name: String,
    pub item_type: String,    // lz 类型名
    pub capacity: usize,      // ring buffer 容量
    pub mode: ChannelMode,    // sender / receiver / bidirectional
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    Sender,
    Receiver,
    Bidirectional,
}

impl ChannelMode {
    fn from_str(s: &str) -> Self {
        match s {
            "recv" | "receiver" => ChannelMode::Receiver,
            "both" | "bidirectional" => ChannelMode::Bidirectional,
            _ => ChannelMode::Sender,
        }
    }
}

/// 共享内存段配置
#[derive(Debug, Clone)]
pub struct SegmentConfig {
    pub name: String,             // 段名（用于 zinc 标识）
    pub size: usize,              // 段大小（字节），0=自动
    pub channels: Vec<ChannelDef>,
}

/// Zinc 桥接配置
#[derive(Debug, Clone)]
pub struct ZincConfig {
    pub version: String,
    pub description: String,
}

// ──────────────── ZincBridge ────────────────

/// Level 4: Zinc 共享内存桥接实现
#[derive(Debug)]
pub struct ZincBridge {
    config: ZincConfig,
    segments: Vec<SegmentConfig>,
    /// 所有 channel 的平铺索引: channel_name → (segment_name, ChannelDef)
    channel_index: HashMap<String, (String, ChannelDef)>,
}

impl ZincBridge {
    /// 从 TOML 清单加载共享内存声明
    ///
    /// 清单格式：
    /// ```toml
    /// [zinc]
    /// version = "0.1.0"
    /// description = "IPC channels for data pipeline"
    ///
    /// [segments.data_pipe]
    /// size = 65536
    ///
    /// [segments.data_pipe.channels]
    /// input = { type = "f64", capacity = 1024, mode = "recv" }
    /// output = { type = "i64", capacity = 2048, mode = "send" }
    ///
    /// [segments.control]
    /// size = 4096
    ///
    /// [segments.control.channels]
    /// cmd = { type = "str", capacity = 64 }
    /// ```
    pub fn load(path: &Path) -> Result<Self, BridgeError> {
        let content = fs::read_to_string(path)
            .map_err(|e| BridgeError::new(ErrorCode::ConnectionFailed,
                format!("read {}: {}", path.display(), e), "zinc"))?;

        let doc = parse(&content)
            .map_err(|e| BridgeError::new(ErrorCode::InvalidMessage,
                format!("parse {}: {}", path.display(), e), "zinc"))?;

        // [zinc] section
        let zinc_sec = doc.get("zinc")
            .ok_or_else(|| BridgeError::new(ErrorCode::InvalidMessage,
                "missing [zinc] section", "zinc"))?;

        let config = ZincConfig {
            version: zinc_sec.get("version").and_then(|v| v.as_str()).unwrap_or("0.1.0").to_string(),
            description: zinc_sec.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };

        // [segments.X] sections — mini_toml uses flat section names
        let mut segments = Vec::new();
        let mut channel_index = HashMap::new();

        // Collect all section keys starting with "segments."
        let seg_prefix = "segments.";
        let seg_sections: Vec<String> = doc.keys()
            .filter(|k| k.starts_with(seg_prefix))
            .cloned()
            .collect();

        // Group by base segment name (everything before the second dot, if any)
        // "segments.data_pipe" → base="data_pipe", sub=""
        // "segments.data_pipe.channels" → base="data_pipe", sub="channels"
        let mut seg_data: HashMap<String, (Option<usize>, HashMap<String, HashMap<String, TomlValue>>)> = HashMap::new();

        for section_name in &seg_sections {
            let remainder = section_name.strip_prefix(seg_prefix).unwrap();
            if let Some(dot_pos) = remainder.find('.') {
                let base = &remainder[..dot_pos];
                let sub = &remainder[dot_pos + 1..];

                let entry = seg_data.entry(base.to_string()).or_insert_with(|| (None, HashMap::new()));
                if let Some(section_data) = doc.get(section_name) {
                    entry.1.insert(sub.to_string(), section_data.clone());
                }
            } else {
                // Top-level segment: "segments.data_pipe"
                let base = remainder;
                let entry = seg_data.entry(base.to_string()).or_insert_with(|| (None, HashMap::new()));
                if let Some(section_data) = doc.get(section_name) {
                    // parse [segments.X] fields (size, etc.)
                    let size: usize = section_data.get("size")
                        .and_then(|v| v.as_int())
                        .map(|n| n as usize)
                        .unwrap_or(0);
                    entry.0 = Some(size);
                }
            }
        }

        // Build SegmentConfig from grouped data
        for (seg_name, (size_opt, sub_sections)) in &seg_data {
            let size = size_opt.unwrap_or(0);
            let mut channels = Vec::new();

            // Parse channels sub-section
            if let Some(ch_section) = sub_sections.get("channels") {
                for (ch_name, ch_val) in ch_section.iter() {
                    if let Some(ch_def_table) = ch_val.as_table() {
                        let item_type = ch_def_table.get("type")
                            .and_then(|v| v.as_str()).unwrap_or("Any").to_string();
                        let capacity: usize = ch_def_table.get("capacity")
                            .and_then(|v| v.as_int()).map(|n| n as usize).unwrap_or(256);
                        let mode_str = ch_def_table.get("mode")
                            .and_then(|v| v.as_str()).unwrap_or("send");
                        let mode = ChannelMode::from_str(mode_str);

                        let ch_def = ChannelDef {
                            name: ch_name.clone(),
                            item_type,
                            capacity,
                            mode,
                        };
                        channel_index.insert(ch_name.clone(), (seg_name.clone(), ch_def.clone()));
                        channels.push(ch_def);
                    }
                }
            }

            segments.push(SegmentConfig {
                name: seg_name.clone(),
                size,
                channels,
            });
        }

        Ok(ZincBridge {
            config,
            segments,
            channel_index,
        })
    }

    // ─── 代码生成 ───

    /// 生成完整的共享内存模块 Rust 源码
    pub fn generate_module(&self) -> String {
        let mut out = String::new();

        out.push_str("// Generated by Lang-Zong ZincBridge\n");
        out.push_str("// Shared memory IPC using zinc channels\n\n");
        out.push_str("use std::io;\n\n");

        // 为每个 segment 生成 zinc::segment 声明
        for seg in &self.segments {
            out.push_str(&self.generate_segment(seg));
            out.push('\n');
        }

        // Channel 操作的 helper 函数
        for (ch_name, (_seg_name, ch_def)) in &self.channel_index {
            out.push_str(&self.generate_channel_ops(ch_name, ch_def));
            out.push('\n');
        }

        out
    }

    fn generate_segment(&self, seg: &SegmentConfig) -> String {
        let mut out = String::new();
        out.push_str(&format!("// Segment: {}\n", seg.name));
        if seg.size > 0 {
            out.push_str(&format!("const {}_SIZE: usize = {};\n", seg.name.to_uppercase(), seg.size));
        }

        // 为每个 channel 生成 zinc channel 类型别名
        for ch in &seg.channels {
            let rust_type = self.lz_to_rust(&ch.item_type);
            out.push_str(&format!(
                "// Channel '{}.{}': {} items, capacity {}\n",
                seg.name, ch.name, ch.item_type, ch.capacity
            ));
            out.push_str(&format!(
                "type {}_channel_t = zinc::channel::Channel<{}, {}>;\n",
                ch.name, rust_type, ch.capacity
            ));
        }
        out
    }

    fn generate_channel_ops(&self, ch_name: &str, ch_def: &ChannelDef) -> String {
        let rust_type = self.lz_to_rust(&ch_def.item_type);
        let mut out = String::new();

        match ch_def.mode {
            ChannelMode::Sender | ChannelMode::Bidirectional => {
                // send 函数
                out.push_str(&format!(
                    "#[inline]\npub fn zinc_send_{}(channel: &zinc::channel::Sender<{}, {}>, item: {}) -> io::Result<()> {{\n",
                    ch_name, rust_type, ch_def.capacity, rust_type
                ));
                out.push_str("    channel.send(item)\n");
                out.push_str("}\n\n");
            }
            _ => {}
        }

        match ch_def.mode {
            ChannelMode::Receiver | ChannelMode::Bidirectional => {
                // recv 函数
                out.push_str(&format!(
                    "#[inline]\npub fn zinc_recv_{}(channel: &zinc::channel::Receiver<{}, {}>) -> io::Result<{}> {{\n",
                    ch_name, rust_type, ch_def.capacity, rust_type
                ));
                out.push_str("    channel.recv()\n");
                out.push_str("}\n\n");
            }
            _ => {}
        }

        out
    }

    fn lz_to_rust(&self, lz_type: &str) -> String {
        match lz_type {
            "int" | "i32" | "i64" => "i64".into(),
            "f64" | "float" => "f64".into(),
            "str" | "String" => "String".into(),
            "bool" => "bool".into(),
            "u8" => "u8".into(),
            "u32" => "u32".into(),
            "u64" => "u64".into(),
            "bytes" => "Vec<u8>".into(),
            other => other.to_string(),
        }
    }
}

// ──────────────── Bridge trait ────────────────

impl Bridge for ZincBridge {
    fn name(&self) -> &str { "zinc" }

    fn level(&self) -> BridgeLevel { BridgeLevel::SharedMemory }

    fn capabilities(&self) -> BridgeCapability {
        BridgeCapability::FUNCTION_CALL | BridgeCapability::STREAMING
    }

    fn resolve_call(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        // 匹配 "zinc_send_<channel>" 或 "zinc_recv_<channel>" 模式，并校验 channel 模式：
        // 仅当 channel 为 Sender/Bidirectional 才允许 send，Receiver/Bidirectional 才允许 recv。
        // 否则会解析到生成代码中并不存在的函数。
        let rust_path = if let Some(suffix) = func_name.strip_prefix("zinc_send_") {
            match self.channel_index.get(suffix) {
                Some((_, ch)) if matches!(ch.mode, ChannelMode::Sender | ChannelMode::Bidirectional) =>
                    Some(format!("zinc_send_{}", suffix)),
                _ => None,
            }
        } else if let Some(suffix) = func_name.strip_prefix("zinc_recv_") {
            match self.channel_index.get(suffix) {
                Some((_, ch)) if matches!(ch.mode, ChannelMode::Receiver | ChannelMode::Bidirectional) =>
                    Some(format!("zinc_recv_{}", suffix)),
                _ => None,
            }
        } else {
            None
        };
        rust_path.map(|rust_path| CallResolveResult {
            rust_path,
            shim: String::new(),
            module_name: "zinc".into(),
            is_macro: false,
            is_template: false,
                    ret_result: false,
        })
    }

    fn meta(&self) -> BridgeMeta {
        BridgeMeta {
            version: self.config.version.clone(),
            description: format!("Zinc shared memory bridge: {} ({} segments, {} channels)",
                self.config.description, self.segments.len(), self.channel_index.len()),
            provides: vec!["zinc".into(), "shared_memory".into(), "ipc".into()],
            ..Default::default()
        }
    }

    fn list_exports(&self, kind: ExportKind) -> Vec<ExportEntry> {
        match kind {
            ExportKind::Function => {
                let mut entries = Vec::new();
                for (ch_name, (_seg_name, ch_def)) in &self.channel_index {
                    match ch_def.mode {
                        ChannelMode::Sender | ChannelMode::Bidirectional => {
                            entries.push(ExportEntry {
                                name: format!("zinc_send_{}", ch_name),
                                kind: ExportKind::Function,
                                signature: format!("send({}) -> io::Result<()>", ch_def.item_type),
                                module: "zinc".into(),
                            });
                        }
                        _ => {}
                    }
                    match ch_def.mode {
                        ChannelMode::Receiver | ChannelMode::Bidirectional => {
                            entries.push(ExportEntry {
                                name: format!("zinc_recv_{}", ch_name),
                                kind: ExportKind::Function,
                                signature: format!("recv() -> io::Result<{}>", ch_def.item_type),
                                module: "zinc".into(),
                            });
                        }
                        _ => {}
                    }
                }
                entries
            }
            _ => vec![],
        }
    }

    fn export_count(&self) -> usize {
        let mut count = 0;
        for (_, ch_def) in self.channel_index.values() {
            match ch_def.mode {
                ChannelMode::Sender => count += 1,
                ChannelMode::Receiver => count += 1,
                ChannelMode::Bidirectional => count += 2,
            }
        }
        count
    }
}

// ──────────────── 测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::TempDir;

    fn create_test_manifest(dir: &TempDir) -> std::path::PathBuf {
        let content = r#"
[zinc]
version = "0.1.0"
description = "Test IPC channels"

[segments.data_pipe]
size = 65536

[segments.data_pipe.channels]
input = { type = "f64", capacity = 1024, mode = "recv" }
output = { type = "i64", capacity = 2048, mode = "send" }
status = { type = "str", capacity = 64, mode = "both" }

[segments.control]
size = 4096

[segments.control.channels]
cmd = { type = "u8", capacity = 32, mode = "recv" }
"#;
        dir.create_file("zinc.toml", content).unwrap()
    }

    #[test]
    fn test_zinc_bridge_load() {
        let dir = TempDir::new("lz-zinc").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = ZincBridge::load(&path).unwrap();

        assert_eq!(bridge.name(), "zinc");
        assert_eq!(bridge.level(), BridgeLevel::SharedMemory);
        assert!(bridge.capabilities().contains(BridgeCapability::FUNCTION_CALL));
        assert!(bridge.capabilities().contains(BridgeCapability::STREAMING));
        assert_eq!(bridge.segments.len(), 2);
        assert_eq!(bridge.channel_index.len(), 4);
    }

    #[test]
    fn test_zinc_bridge_gen_call() {
        let dir = TempDir::new("lz-zinc").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = ZincBridge::load(&path).unwrap();

        // send channels
        assert_eq!(bridge.resolve_call("zinc_send_output", &[]).map(|r| r.rust_path), Some("zinc_send_output".to_string()));
        assert_eq!(bridge.resolve_call("zinc_send_status", &[]).map(|r| r.rust_path), Some("zinc_send_status".to_string()));

        // recv channels
        assert_eq!(bridge.resolve_call("zinc_recv_input", &[]).map(|r| r.rust_path), Some("zinc_recv_input".to_string()));
        assert_eq!(bridge.resolve_call("zinc_recv_status", &[]).map(|r| r.rust_path), Some("zinc_recv_status".to_string()));

        // non-existent channel
        assert!(bridge.resolve_call("zinc_send_nonexistent", &[]).is_none());
    }

    #[test]
    fn test_zinc_module_generation() {
        let dir = TempDir::new("lz-zinc").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = ZincBridge::load(&path).unwrap();

        let code = bridge.generate_module();
        assert!(code.contains("input_channel_t"));
        assert!(code.contains("output_channel_t"));
        assert!(code.contains("zinc_send_output"));
        assert!(code.contains("zinc_recv_input"));
        assert!(code.contains("zinc_send_status"));
        assert!(code.contains("zinc_recv_status"));
    }

    #[test]
    fn test_zinc_list_exports() {
        let dir = TempDir::new("lz-zinc").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = ZincBridge::load(&path).unwrap();

        let funcs = bridge.list_exports(ExportKind::Function);
        // input=recv(1), output=send(1), status=both(2), cmd=recv(1) = 5
        assert_eq!(funcs.len(), 5);
        assert!(funcs.iter().any(|e| e.name == "zinc_send_output"));
        assert!(funcs.iter().any(|e| e.name == "zinc_recv_input"));
        assert!(funcs.iter().any(|e| e.name == "zinc_send_status"));
        assert!(funcs.iter().any(|e| e.name == "zinc_recv_status"));
        assert!(funcs.iter().any(|e| e.name == "zinc_recv_cmd"));
    }
}

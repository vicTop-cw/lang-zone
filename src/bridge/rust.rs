// Lang-Zong 编译器 — bridge/rust.rs
// Level 1: Rust 直接桥接（直通 Rust 第三方库/标准库）
//
// 设计动机（自举迭代）：
//   当 lz 标准库缺失某个功能时，直接 `import std::bridge::rust::xxx`
//   借用 Rust 生态，无需等待 lz std 更新。对标 Mojo 的 `from python import numpy`。
//
// 实现机制：
//   拦截 import std::bridge::rust::crate_name 路径，剥离前缀，
//   直接生成 `use crate_name;` 语句，函数/类型调用直通 Rust。
//   无需逐函数 TOML 映射，零配置即可使用任何 Rust crate。
//
// 使用示例：
//   import std::bridge::rust::serde_json           → use serde_json;
//   from std::bridge::rust::serde_json import from_str → use serde_json::{from_str};
//   import std::bridge::rust::tokio::net            → use tokio::net;
//
// CLI 用法：
//   lzc myfile.lz --std-dir std/ --rust-crate serde_json=1.0.0 --rust-crate tokio=1.35
//   → 在生成的文件尾部注明 Cargo.toml 依赖提示

use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeError, BridgeLevel, BridgeMeta,
    CallResolveResult, ExportEntry, ExportKind, ImportResolveResult,
    MethodResolveResult, RoutePattern,
};
use std::collections::HashMap;

/// 注册的 Rust crate 信息
#[derive(Debug, Clone)]
pub struct CrateEntry {
    pub name: String,
    pub version: Option<String>,
    pub features: Vec<String>,
}

/// Rust 直接桥接：import std::bridge::rust::* → use *;
#[derive(Debug)]
pub struct RustBridge {
    /// 注册的 crate（通过 --rust-crate CLI 标志或 TOML 文件注册）
    crates: HashMap<String, CrateEntry>,
    /// 本次编译实际用到的 crate（用于 Cargo.toml 提示）
    used_crates: Vec<CrateEntry>,
    /// 是否允许未注册的 crate 透传（默认 true：import 一个没显式声明的 crate 也允许）
    allow_unregistered: bool,
    /// 缓存的路由模式（惰性初始化）
    patterns: Vec<RoutePattern>,
}

impl RustBridge {
    pub fn new() -> Self {
        RustBridge {
            crates: HashMap::new(),
            used_crates: Vec::new(),
            allow_unregistered: true,
            patterns: vec![
                RoutePattern::new("std::bridge::rust", BridgeCapability::IMPORT, 1000),
                RoutePattern::new("std::bridge::rust", BridgeCapability::FUNCTION_CALL, 1000),
                RoutePattern::new("std::bridge::rust", BridgeCapability::TYPE_REWRITE, 1000),
            ],
        }
    }

    /// 注册一个 Rust crate
    pub fn register_crate(&mut self, name: impl Into<String>, version: Option<String>, features: Vec<String>) {
        let name = name.into();
        self.crates.insert(name.clone(), CrateEntry { name, version, features });
    }

    /// 返回本次编译用到的 crate 列表（供 codegen 生成 Cargo.toml 提示）
    pub fn used_crates(&self) -> &[CrateEntry] {
        &self.used_crates
    }
}

impl Bridge for RustBridge {
    fn name(&self) -> &str { "rust_bridge" }

    fn level(&self) -> BridgeLevel { BridgeLevel::CompileTime }

    fn capabilities(&self) -> BridgeCapability {
        BridgeCapability::IMPORT | BridgeCapability::FUNCTION_CALL | BridgeCapability::TYPE_REWRITE
    }

    fn meta(&self) -> BridgeMeta {
        BridgeMeta {
            version: "0.1.0".into(),
            description: "Rust direct bridge: import std::bridge::rust::<crate> for zero-config Rust crate access".into(),
            provides: vec!["rust_bridge".into(), "rust_direct".into()],
            ..Default::default()
        }
    }

    fn route_patterns(&self) -> &[RoutePattern] {
        &self.patterns
    }

    fn resolve_import(&self, module_path: &[String], _items: &[String]) -> Option<ImportResolveResult> {
        // 只处理 std::bridge::rust::xxx 路径
        if module_path.len() < 4 { return None; }
        if module_path[0] != "std" || module_path[1] != "bridge" || module_path[2] != "rust" {
            return None;
        }

        // 剥离前缀，剩余部分就是 Rust 路径
        let rust_path: String = module_path[3..].join("::");
        if rust_path.is_empty() { return None; }

        // 提取 crate 名（路径第一个组件）
        let crate_name = &module_path[3];

        // 允许未注册 crate 时直接透传
        if !self.allow_unregistered && !self.crates.contains_key(crate_name) {
            let err = BridgeError::new(
                crate::bridge::core::ErrorCode::CapabilityMissing,
                format!("Rust crate '{}' not registered. Use --rust-crate flag or add to bridge TOML.", crate_name),
                "rust_bridge",
            );
            return Some(ImportResolveResult {
                rust_path,
                type_aliases: vec![],
                requires_shim: false,
                is_tier2: false,
                feature_flags: vec![],
                extern_crates: vec![crate_name.clone()],
                error: Some(err.to_string()),
            });
        }

        Some(ImportResolveResult {
            rust_path,
            type_aliases: vec![],
            requires_shim: false,
            is_tier2: false,
            feature_flags: vec![],
            extern_crates: vec![crate_name.clone()],
            error: None,
        })
    }

    fn resolve_call(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        // 如果函数名以 crate:: 开头，检查是否是已知 crate
        if let Some((crate_name, _rest)) = func_name.split_once("::") {
            if self.crates.contains_key(crate_name) || self.allow_unregistered {
                return Some(CallResolveResult {
                    rust_path: func_name.to_string(),
                    shim: String::new(),
                    module_name: crate_name.to_string(),
                    is_macro: false,
                    is_template: false,
                    ret_result: false,
                });
            }
        }
        None
    }

    fn resolve_type(&self, lz_type: &str) -> Option<String> {
        // 允许透传任何 Rust 类型路径（如 serde_json::Value）
        // 只要路径中包含 `::` 且第一个组件是已知 crate
        if lz_type.contains("::") {
            if let Some((crate_name, _)) = lz_type.split_once("::") {
                if self.crates.contains_key(crate_name) || self.allow_unregistered {
                    return Some(lz_type.to_string());
                }
            }
        }
        None
    }

    fn resolve_method(&self, _method: &str, _receiver_type: &str) -> Option<MethodResolveResult> {
        // 方法调用（receiver.method）暂不处理，通过 bridge 默认实现 fallthrough
        None
    }

    fn list_exports(&self, _kind: ExportKind) -> Vec<ExportEntry> {
        // Rust crate 的导出由 rustc 管理，lz 不枚举
        vec![]
    }

    fn export_count(&self) -> usize { self.crates.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bridge() -> RustBridge {
        let mut b = RustBridge::new();
        b.register_crate("serde_json", Some("1.0".into()), vec![]);
        b.register_crate("tokio", Some("1.35".into()), vec!["full".into()]);
        b
    }

    #[test]
    fn test_import_rust_crate() {
        let b = make_bridge();
        let path = vec!["std".into(), "bridge".into(), "rust".into(), "serde_json".into()];
        let r = b.resolve_import(&path, &[]).unwrap();
        assert_eq!(r.rust_path, "serde_json");
        assert_eq!(r.extern_crates, vec!["serde_json"]);
    }

    #[test]
    fn test_import_rust_nested() {
        let b = make_bridge();
        let path = vec!["std".into(), "bridge".into(), "rust".into(), "tokio".into(), "net".into()];
        let r = b.resolve_import(&path, &[]).unwrap();
        assert_eq!(r.rust_path, "tokio::net");
    }

    #[test]
    fn test_ignore_non_bridge() {
        let b = make_bridge();
        let path = vec!["std".into(), "collections".into(), "HashMap".into()];
        assert!(b.resolve_import(&path, &[]).is_none());
    }

    #[test]
    fn test_ignore_short_path() {
        let b = make_bridge();
        let path = vec!["std".into(), "bridge".into()];
        assert!(b.resolve_import(&path, &[]).is_none());
    }

    #[test]
    fn test_resolve_call() {
        let b = make_bridge();
        let r = b.resolve_call("serde_json::from_str", &[]).unwrap();
        assert_eq!(r.rust_path, "serde_json::from_str");
        assert!(!r.is_macro);
    }

    #[test]
    fn test_resolve_type() {
        let b = make_bridge();
        let t = b.resolve_type("serde_json::Value").unwrap();
        assert_eq!(t, "serde_json::Value");
    }

    #[test]
    fn test_crate_not_registered() {
        let b = make_bridge();
        let path = vec!["std".into(), "bridge".into(), "rust".into(), "nonexistent".into()];
        // allow_unregistered=true 时，未注册 crate 也透传
        let r = b.resolve_import(&path, &[]).unwrap();
        assert_eq!(r.rust_path, "nonexistent");
        assert!(r.error.is_none());
    }

    #[test]
    fn test_route_patterns() {
        let b = make_bridge();
        let patterns = b.route_patterns();
        assert!(patterns.iter().any(|p| p.prefix == "std::bridge::rust"));
        assert!(patterns[0].priority == 1000);
    }
}

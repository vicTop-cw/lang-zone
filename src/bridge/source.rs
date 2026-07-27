// Lang-Zong 编译器 — bridge_source.rs
// Level 0: 源码映射桥接
// 编译期消解，零运行时开销。将 lz 符号直接映射为 Rust/std crate 路径。
// 实现 Bridge trait，包裹现有 StdBridge + 三方 crate 支持。

use crate::bridge::StdBridge;
use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeError, BridgeLevel, BridgeMeta,
    CallResolveResult, ErrorCode, ExportEntry, ExportKind, ImportResolveResult,
    MethodResolveResult,
};
use std::path::PathBuf;

/// Level 0: 源码映射桥接
#[derive(Debug)]
pub struct SourceBridge {
    inner: StdBridge,
    std_dir: PathBuf,
}

impl SourceBridge {
    pub fn new(std_dir: PathBuf) -> Result<Self, BridgeError> {
        let mut inner = StdBridge::load(&std_dir)
            .map_err(|e| BridgeError::new(ErrorCode::ConnectionFailed, e, "source"))?;
        // SourceBridge 默认支持 Tier-1（Rust std 无需额外标志）
        inner.set_tier2_allowed(false);
        Ok(SourceBridge { inner, std_dir })
    }

    pub fn with_tier2(mut self, allowed: bool, rustc_version: &str) -> Self {
        self.inner.set_tier2_allowed(allowed);
        self.inner.set_rustc_version(rustc_version.to_string());
        self
    }

    pub fn std_dir(&self) -> &PathBuf {
        &self.std_dir
    }
}

impl Bridge for SourceBridge {
    fn name(&self) -> &str { "source" }

    fn level(&self) -> BridgeLevel { BridgeLevel::CompileTime }

    fn capabilities(&self) -> BridgeCapability {
        BridgeCapability::IMPORT
            | BridgeCapability::FUNCTION_CALL
            | BridgeCapability::METHOD_CALL
            | BridgeCapability::TYPE_REWRITE
            | BridgeCapability::SHIM_INJECT
    }

    // ─── 代码生成 ───

    fn gen_import(&self, module_path: &[String], items: &[String]) -> String {
        let result = self.inner.resolve_import(module_path, items);
        // 基础路径
        let mut out = String::new();

        // 类型别名
        for (alias_name, rust_type) in &result.type_aliases {
            out.push_str(&format!("pub type {} = {};\n", alias_name, rust_type));
        }

        // use 语句
        if !items.is_empty() {
            out.push_str(&format!("use {}::{{{}}};\n", result.rust_path, items.join(", ")));
        } else {
            out.push_str(&format!("use {};\n", result.rust_path));
        }

        out
    }

    fn resolve_import_full(&self, module_path: &[String], items: &[String]) -> Option<ImportResolveResult> {
        // StdBridge.resolve_import 现在直接返回 bridge_core::ImportResolveResult
        Some(self.inner.resolve_import(module_path, items))
    }

    fn gen_call(&self, func_name: &str, _args: &[String]) -> Option<String> {
        self.inner.resolve_call(func_name)
            .map(|r| {
                if r.is_macro {
                    format!("{}!", r.rust_path.trim_end_matches('!'))
                } else {
                    r.rust_path.clone()
                }
            })
    }

    fn gen_method(&self, method: &str, receiver_type: &str) -> String {
        let result = self.inner.resolve_method(method, receiver_type);
        result.rust_method
    }

    fn gen_type(&self, lz_type: &str) -> Option<String> {
        self.inner.rewrite_type(lz_type)
    }

    fn required_shims(&self) -> Vec<String> {
        // 收集所有已加载模块所需的 shim（简化：返回 path_ref 等核心 shim）
        let all_modules = ["fs", "core", "io"];
        let mut all_shims = Vec::new();
        for m in &all_modules {
            all_shims.extend(self.inner.shims_required(m));
        }
        all_shims
    }

    // ─── 扩展 API ───

    fn meta(&self) -> BridgeMeta {
        BridgeMeta {
            version: "0.1.0".into(),
            description: "lz std → Rust std source-level mapping bridge".into(),
            provides: vec!["std".into(), "core".into()],
            ..Default::default()
        }
    }

    fn resolve_call_full(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        self.inner.resolve_call(func_name)
    }

    fn resolve_method_full(&self, method: &str, receiver_type: &str) -> Option<MethodResolveResult> {
        Some(self.inner.resolve_method(method, receiver_type))
    }

    fn list_exports(&self, kind: ExportKind) -> Vec<ExportEntry> {
        // 从 StdBridge 内部枚举已加载的模块符号
        // 简化实现：统计所有 TOML 中声明的符号
        self.inner.list_exports(kind)
    }

    fn export_count(&self) -> usize {
        self.inner.export_count()
    }
}

// ──────────────── 测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_bridge_resolve_import_full() {
        let bridge = SourceBridge::new(PathBuf::from("std")).unwrap();
        let result = bridge.resolve_import_full(&["std".into(), "io".into()], &[]);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rust_path, "std::io");
        let has_ioerror = r.type_aliases.iter().any(|(a, _)| a == "IOError");
        assert!(has_ioerror, "IO import should inject IOError alias via resolve_import_full");
    }

    #[test]
    fn test_source_bridge_create() {
        let bridge = SourceBridge::new(PathBuf::from("std")).unwrap();
        assert_eq!(bridge.name(), "source");
        assert_eq!(bridge.level(), BridgeLevel::CompileTime);
        assert!(bridge.capabilities().contains(BridgeCapability::IMPORT));
        assert!(bridge.capabilities().contains(BridgeCapability::METHOD_CALL));
    }

    #[test]
    fn test_source_bridge_import() {
        let bridge = SourceBridge::new(PathBuf::from("std")).unwrap();
        let result = bridge.gen_import(&["std".into(), "io".into()], &[]);
        assert!(result.contains("use std::io"));
        assert!(result.contains("pub type IOError"));
    }

    #[test]
    fn test_source_bridge_call() {
        let bridge = SourceBridge::new(PathBuf::from("std")).unwrap();
        let result = bridge.gen_call("panic", &[]);
        // panic → panic! (Rust macro)
        assert_eq!(result, Some("panic!".to_string()));
    }

    #[test]
    fn test_source_bridge_method() {
        let bridge = SourceBridge::new(PathBuf::from("std")).unwrap();
        assert_eq!(bridge.gen_method("append", "Vec"), "push");
        assert_eq!(bridge.gen_method("length", "Vec"), "len");
        assert_eq!(bridge.gen_method("startsWith", "String"), "starts_with");
    }

    #[test]
    fn test_source_bridge_type() {
        let bridge = SourceBridge::new(PathBuf::from("std")).unwrap();
        assert_eq!(bridge.gen_type("Never"), Some("!".to_string()));
    }

    #[test]
    fn test_source_bridge_shims() {
        let bridge = SourceBridge::new(PathBuf::from("std")).unwrap();
        let shims = bridge.required_shims();
        assert!(!shims.is_empty());
    }

    #[test]
    fn test_source_bridge_tier2_off_by_default() {
        let bridge = SourceBridge::new(PathBuf::from("std")).unwrap();
        let result = bridge.gen_type("rustc_middle");
        assert!(result.is_none());
    }
}

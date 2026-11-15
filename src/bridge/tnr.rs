// Lang-Zong 编译器 — bridge/tnr.rs
// Tnr 桥接：LZ 通过 bridge 机制调用 tnr 的三种语义
//
// 路由前缀：
//   tnr::func          → tnr lib API 转译（parse + lower + codegen → Rust 函数）
//   tnr_lib::path      → 已编译 tnr 库链接（use tnr::path;）
//   tnr::compute::xxx  → tnr 计算后端委托（生成 shim 代码）
//
// 设计原则：
//   1. 编译期消解（BridgeLevel::CompileTime），零运行时开销
//   2. tnr lib API 转译复用 gen_embed_tnr 的管线（parse → lower → transpile）
//   3. 已编译库链接直接映射为 use tnr::path，零额外开销
//   4. 计算后端委托生成 shim 函数，运行时通过 tnr interp 执行

use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeLevel, BridgeMeta, CallResolveResult, ExportEntry, ExportKind,
    ImportResolveResult,
};

/// Tnr 桥接：LZ ↔ tnr 的三种语义
#[derive(Debug)]
pub struct TnrBridge {
    /// 已转译的函数缓存（func_name → rust_source）
    transpiled: std::collections::HashMap<String, String>,
}

impl TnrBridge {
    pub fn new() -> Self {
        TnrBridge {
            transpiled: std::collections::HashMap::new(),
        }
    }

    /// 路由分类：根据 module_path 前缀判断桥接语义
    fn classify(&self, module_path: &[String]) -> TnrRoute {
        if module_path.is_empty() {
            return TnrRoute::None;
        }
        match module_path[0].as_str() {
            "tnr" => {
                if module_path.len() >= 2 && module_path[1] == "compute" {
                    TnrRoute::Compute
                } else {
                    TnrRoute::LibApi
                }
            }
            "tnr_lib" => TnrRoute::CompiledLib,
            _ => TnrRoute::None,
        }
    }
}

impl Default for TnrBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// tnr 桥接路由分类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TnrRoute {
    /// tnr::func → tnr lib API 转译
    LibApi,
    /// tnr_lib::path → 已编译 tnr 库链接
    CompiledLib,
    /// tnr::compute::xxx → tnr 计算后端委托
    Compute,
    None,
}

impl Bridge for TnrBridge {
    fn name(&self) -> &str {
        "tnr"
    }

    fn level(&self) -> BridgeLevel {
        BridgeLevel::CompileTime
    }

    fn capabilities(&self) -> BridgeCapability {
        BridgeCapability::IMPORT | BridgeCapability::FUNCTION_CALL | BridgeCapability::TYPE_REWRITE
    }

    fn meta(&self) -> BridgeMeta {
        BridgeMeta {
            version: "0.1.0".into(),
            description: "tnr bridge: lib API transpile + compiled lib link + compute backend"
                .into(),
            provides: vec!["tnr".into(), "tnr_lib".into()],
            ..Default::default()
        }
    }

    // ─── 导入解析 ───

    fn resolve_import_full(
        &self,
        module_path: &[String],
        _items: &[String],
    ) -> Option<ImportResolveResult> {
        match self.classify(module_path) {
            TnrRoute::LibApi => {
                // tnr::func → 转译为 Rust 函数，生成 use tnr_transpiled::func;
                let func_name = module_path.get(1)?;
                Some(ImportResolveResult {
                    rust_path: format!("tnr_transpiled::{}", func_name),
                    type_aliases: vec![],
                    requires_shim: true,
                    is_tier2: false,
                    feature_flags: vec!["tnr-embed".into()],
                    extern_crates: vec![],
                    error: None,
                })
            }
            TnrRoute::CompiledLib => {
                // tnr_lib::path → use tnr::path;
                let rest = &module_path[1..];
                let rust_path = if rest.is_empty() {
                    "tnr".to_string()
                } else {
                    format!("tnr::{}", rest.join("::"))
                };
                Some(ImportResolveResult {
                    rust_path,
                    type_aliases: vec![],
                    requires_shim: false,
                    is_tier2: false,
                    feature_flags: vec![],
                    extern_crates: vec!["tnr".into()],
                    error: None,
                })
            }
            TnrRoute::Compute => {
                // tnr::compute::kernel → 生成 tnr 计算委托 shim
                let kernel_name = module_path.get(2)?;
                Some(ImportResolveResult {
                    rust_path: format!("tnr_compute::{}", kernel_name),
                    type_aliases: vec![],
                    requires_shim: true,
                    is_tier2: false,
                    feature_flags: vec!["tnr-embed".into()],
                    extern_crates: vec![],
                    error: None,
                })
            }
            TnrRoute::None => None,
        }
    }

    fn gen_import(&self, module_path: &[String], items: &[String]) -> String {
        let result = match self.resolve_import_full(module_path, items) {
            Some(r) => r,
            None => return String::new(),
        };
        if result.rust_path.is_empty() {
            return String::new();
        }
        if !items.is_empty() {
            format!(
                "use {}::{{{}}};\n",
                result.rust_path,
                items.join(", ")
            )
        } else {
            format!("use {};\n", result.rust_path)
        }
    }

    // ─── 函数调用解析 ───

    fn resolve_call_full(
        &self,
        func_name: &str,
        _args: &[String],
    ) -> Option<CallResolveResult> {
        // tnr::func(args) 或 tnr_lib::func(args) → 路由到对应 Rust 函数
        if let Some(rest) = func_name.strip_prefix("tnr::") {
            return Some(CallResolveResult {
                rust_path: format!("tnr_transpiled::{}", rest),
                shim: format!("__tnr_shim_{}", rest),
                module_name: "tnr".into(),
                is_macro: false,
                is_template: false,
                ret_result: false,
            });
        }
        if let Some(rest) = func_name.strip_prefix("tnr_lib::") {
            return Some(CallResolveResult {
                rust_path: format!("tnr::{}", rest),
                shim: String::new(),
                module_name: "tnr_lib".into(),
                is_macro: false,
                is_template: false,
                ret_result: false,
            });
        }
        None
    }

    // ─── 导出枚举 ───

    fn list_exports(&self, kind: ExportKind) -> Vec<ExportEntry> {
        match kind {
            ExportKind::Function => self
                .transpiled
                .keys()
                .map(|name| ExportEntry {
                    name: name.clone(),
                    kind: ExportKind::Function,
                    signature: format!("fn {}(...) -> Val", name),
                    module: "tnr".into(),
                })
                .collect(),
            ExportKind::Module => vec![
                ExportEntry {
                    name: "tnr".into(),
                    kind: ExportKind::Module,
                    signature: "tnr lib API transpile".into(),
                    module: String::new(),
                },
                ExportEntry {
                    name: "tnr_lib".into(),
                    kind: ExportKind::Module,
                    signature: "compiled tnr crate link".into(),
                    module: String::new(),
                },
            ],
            _ => vec![],
        }
    }

    fn export_count(&self) -> usize {
        self.transpiled.len()
    }
}

// ══════════════════════════════════════════════════════════════
// 单元测试
// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tnr_bridge_lib_api_route() {
        let b = TnrBridge::new();
        let path = vec!["tnr".to_string(), "matmul".to_string()];
        let r = b.resolve_import_full(&path, &[]).unwrap();
        assert_eq!(r.rust_path, "tnr_transpiled::matmul");
        assert!(r.requires_shim);
        assert!(r.feature_flags.contains(&"tnr-embed".to_string()));
    }

    #[test]
    fn tnr_bridge_compiled_lib_route() {
        let b = TnrBridge::new();
        let path = vec![
            "tnr_lib".to_string(),
            "tensor".to_string(),
            "Tensor".to_string(),
        ];
        let r = b.resolve_import_full(&path, &[]).unwrap();
        assert_eq!(r.rust_path, "tnr::tensor::Tensor");
        assert!(!r.requires_shim);
        assert!(r.extern_crates.contains(&"tnr".to_string()));
    }

    #[test]
    fn tnr_bridge_compute_route() {
        let b = TnrBridge::new();
        let path = vec![
            "tnr".to_string(),
            "compute".to_string(),
            "kernel".to_string(),
        ];
        let r = b.resolve_import_full(&path, &[]).unwrap();
        assert_eq!(r.rust_path, "tnr_compute::kernel");
        assert!(r.requires_shim);
    }

    #[test]
    fn tnr_bridge_no_match() {
        let b = TnrBridge::new();
        let path = vec!["std".to_string(), "io".to_string()];
        assert!(b.resolve_import_full(&path, &[]).is_none());
    }

    #[test]
    fn tnr_bridge_gen_import() {
        let b = TnrBridge::new();
        let path = vec!["tnr_lib".to_string(), "tensor".to_string()];
        let s = b.gen_import(&path, &[]);
        assert!(s.contains("use tnr::tensor;"), "got: {s}");
    }
}

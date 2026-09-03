// Lang-Zong 编译器 — bridge/extern.rs
// Level 1: Rust extern 链接桥接
// 编译期消解，链接时绑定。通过 `extern "Rust" {}` 调用同语言 Rust 函数。
// 对标 C FFI (bridge/ffi.rs) 但专用于非 C ABI 的 Rust 函数链接。
//
// 使用场景：
//   import extern::mycrate::utils  → 生成 `use mycrate::utils;`
//   extern { fn rust_func(x: i32) -> i32 }  → 链接到编译好的 Rust 符号
//   @extern("Rust") fn fast_add(a: i64, b: i64) -> i64  → 标注为 extern Rust 调用
//
// 设计原则：
//   1. extern "Rust" 是 Rust 语言的原生 FFI ABI（不同于 extern "C"）
//   2. 不做 C 类型的 marshaling，零额外开销
//   3. 支持 #[no_mangle] + #[link] 组合，或 dlopen 动态加载
//   4. 与 FfiBridge (extern "C") 完全正交，可同时启用

use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeError, BridgeLevel, BridgeMeta,
    CallResolveResult, ErrorCode, ExportEntry, ExportKind, ImportResolveResult,
    MethodResolveResult,
};
use std::cell::RefCell;
use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════
// 外部 Rust 函数签名（用于 codegen）
// ══════════════════════════════════════════════════════════════

/// 单个 extern Rust 函数签名声明
#[derive(Debug, Clone)]
pub struct ExternFn {
    /// 符号名（可与 lz 名不同）
    pub symbol: String,
    /// 参数类型列表（Rust 类型字符串）
    pub params: Vec<String>,
    /// 返回类型（Rust 类型字符串）
    pub return_type: String,
    /// 是否是 #[inline] 函数（codegen 时保留 inline 属性）
    pub inline: bool,
}

/// extern 块配置
#[derive(Debug, Clone)]
pub struct ExternBlock {
    /// 块名称（用于生成唯一的 extern 块 ID）
    pub name: String,
    /// crate 名（用于 #[link] 属性）
    pub crate_name: Option<String>,
    /// 导出函数列表
    pub functions: Vec<ExternFn>,
    /// 是否为系统 crate（std / core / alloc）
    pub is_std: bool,
}

// ══════════════════════════════════════════════════════════════
// ExternBridge — extern "Rust" 桥接
// ══════════════════════════════════════════════════════════════

/// Level 1: Rust extern 链接桥接
///
/// 核心职责：
///   1. 解析 `import extern::<crate>::<path>` 导入语句
///   2. 解析 `extern { fn ... }` 块并生成 Rust extern "Rust" 代码
///   3. 为 @extern("Rust") 标注的函数生成链接指令
///   4. 生成 #[link] 属性（供 cargo 链接 Rust crate）
///
/// 不同于 FfiBridge：
///   - FfiBridge：extern "C"，处理 C ABI，需要类型 marshal
///   - ExternBridge：extern "Rust"，Rust ABI，零 marshal 开销
#[derive(Debug)]
pub struct ExternBridge {
    /// 注册的 extern 块（块名 → ExternBlock）
    blocks: HashMap<String, ExternBlock>,
    /// 符号缓存（符号名 → 所属块名）
    symbol_map: HashMap<String, String>,
    /// 本次编译用到的 crate（codegen 生成 Cargo.toml 提示）
    used_crates: RefCell<Vec<String>>,
}

impl ExternBridge {
    pub fn new() -> Self {
        ExternBridge {
            blocks: HashMap::new(),
            symbol_map: HashMap::new(),
            used_crates: RefCell::new(Vec::new()),
        }
    }

    // ─── 注册 ───

    /// 注册一个 extern 块
    pub fn register_block(&mut self, block: ExternBlock) {
        for func in &block.functions {
            self.symbol_map.insert(func.symbol.clone(), block.name.clone());
        }
        self.blocks.insert(block.name.clone(), block);
    }

    /// 注册单个 extern 函数（快捷方法）
    pub fn register_fn(
        &mut self,
        block_name: &str,
        symbol: &str,
        params: Vec<String>,
        return_type: &str,
    ) {
        let block = self.blocks.entry(block_name.to_string()).or_insert_with(|| {
            ExternBlock {
                name: block_name.to_string(),
                crate_name: None,
                functions: Vec::new(),
                is_std: false,
            }
        });
        block.functions.push(ExternFn {
            symbol: symbol.to_string(),
            params,
            return_type: return_type.to_string(),
            inline: false,
        });
        self.symbol_map.insert(symbol.to_string(), block_name.to_string());
    }

    // ─── 代码生成 ───

    /// 生成所有 extern "Rust" {} 块
    pub fn generate_extern_blocks(&self) -> String {
        let mut out = String::new();

        for block in self.blocks.values() {
            out.push_str(&self.generate_one_block(block));
        }

        out
    }

    fn generate_one_block(&self, block: &ExternBlock) -> String {
        if block.functions.is_empty() {
            return String::new();
        }

        let mut out = String::new();

        // #[link] 属性（仅非 std crate）
        if let Some(crate_name) = &block.crate_name {
            if !block.is_std {
                out.push_str(&format!("#[link(name = \"{}\", kind = \"dylib\")]\n", crate_name));
            }
        }

        out.push_str("extern \"Rust\" {\n");

        for func in &block.functions {
            let params = if func.params.is_empty() {
                String::new()
            } else {
                func.params.join(", ")
            };
            let fn_sig = if func.return_type == "()" {
                format!("    fn {}({});\n", func.symbol, params)
            } else {
                format!("    fn {}({}) -> {};\n", func.symbol, params, func.return_type)
            };
            if func.inline {
                out.push_str("    #[inline]\n");
            }
            out.push_str(&fn_sig);
        }

        out.push_str("}\n\n");
        out
    }

    /// 生成 safe wrapper 函数（将 extern "Rust" 包装为 safe Rust 函数）
    /// 这些 wrapper 可以被 lz codegen 直接调用
    pub fn generate_safe_wrappers(&self) -> String {
        let mut out = String::new();

        for block in self.blocks.values() {
            for func in &block.functions {
                let params = if func.params.is_empty() {
                    String::new()
                } else {
                    func.params.iter()
                        .enumerate()
                        .map(|(i, t)| format!("arg_{}: {}", i, t))
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                let args = if func.params.is_empty() {
                    String::new()
                } else {
                    (0..func.params.len())
                        .map(|i| format!("arg_{}", i))
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                if func.inline {
                    out.push_str("#[inline]\n");
                }

                if func.return_type == "()" {
                    out.push_str(&format!(
                        "pub fn _lz_extern_{}_{}({}) {{\n    unsafe {{ {}({}); }}\n}}\n\n",
                        block.name, func.symbol, params, func.symbol, args
                    ));
                } else {
                    out.push_str(&format!(
                        "pub unsafe fn _lz_extern_{}_{}({}) -> {} {{\n    {}({})\n}}\n\n",
                        block.name, func.symbol, params, func.return_type, func.symbol, args
                    ));
                }
            }
        }

        out
    }

    /// 生成 Cargo.toml 依赖提示（列出本次用到的 Rust crate）
    pub fn generate_cargo_hints(&self) -> String {
        let crates = self.used_crates.borrow();
        if crates.is_empty() {
            return String::new();
        }

        let mut out = String::from("# ═══ extern \"Rust\" Bridge Cargo Dependencies ═══\n");
        out.push_str("# 自动生成 — 请将以下依赖添加到 Cargo.toml\n\n");

        for crate_name in crates.iter() {
            out.push_str(&format!("{crate_name} = \"*\"\n"));
        }

        out.push('\n');
        out
    }

    /// 本次用到的 crate 列表
    pub fn used_crates(&self) -> &RefCell<Vec<String>> {
        &self.used_crates
    }

    /// 记录使用了一个 crate（用于 Cargo.toml 提示）
    pub fn record_crate(&self, crate_name: &str) {
        let mut crates = self.used_crates.borrow_mut();
        if !crates.contains(&crate_name.to_string()) {
            crates.push(crate_name.to_string());
        }
    }
}

impl Default for ExternBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ══════════════════════════════════════════════════════════════
// Bridge trait 实现
// ══════════════════════════════════════════════════════════════

impl Bridge for ExternBridge {
    fn name(&self) -> &str { "extern_rust" }

    fn level(&self) -> BridgeLevel { BridgeLevel::LinkTime }

    fn capabilities(&self) -> BridgeCapability {
        BridgeCapability::IMPORT
            | BridgeCapability::FUNCTION_CALL
            | BridgeCapability::TYPE_REWRITE
    }

    fn meta(&self) -> BridgeMeta {
        let block_count = self.blocks.len();
        let fn_count: usize = self.blocks.values().map(|b| b.functions.len()).sum();
        BridgeMeta {
            version: "0.1.0".into(),
            description: format!(
                "extern \"Rust\" bridge: {} blocks, {} functions registered",
                block_count, fn_count
            ),
            provides: vec!["extern".into()],
            ..Default::default()
        }
    }

    // ─── 导入解析 ───

    /// `import extern::<crate>::<path>` → 生成 `use <crate>::<path>;`
    ///
    /// 示例：
    ///   import extern::serde_json::to_string  → use serde_json::to_string;
    ///   import extern::rayon::prelude        → use rayon::prelude;
    fn resolve_import_full(&self, module_path: &[String], _items: &[String]) -> Option<ImportResolveResult> {
        if module_path.is_empty() {
            return None;
        }
        // 必须以 "extern" 开头
        if module_path[0] != "extern" {
            return None;
        }

        // 格式：extern::crate_name[::path_parts...]
        if module_path.len() < 2 {
            return None;
        }

        let crate_name = &module_path[1];
        let rust_path = if module_path.len() > 2 {
            module_path[2..].join("::")
        } else {
            String::new()
        };

        let full_path = if rust_path.is_empty() {
            crate_name.clone()
        } else {
            format!("{}::{}", crate_name, rust_path)
        };

        // 记录用到的 crate
        self.record_crate(crate_name);

        // 检查是否是已知块
        let is_registered = self.blocks.contains_key(crate_name);

        Some(ImportResolveResult {
            rust_path: full_path,
            type_aliases: vec![],
            requires_shim: false,
            is_tier2: false,
            feature_flags: vec![],
            extern_crates: if is_registered {
                vec![]
            } else {
                vec![crate_name.clone()]
            },
            error: None,
        })
    }

    fn gen_import(&self, module_path: &[String], _items: &[String]) -> String {
        // 委托 resolve_import_full 处理
        let result = self.resolve_import_full(module_path, &[]);
        match result {
            Some(r) if !r.rust_path.is_empty() => {
                format!("use {};\n", r.rust_path)
            }
            _ => String::new(),
        }
    }

    // ─── 函数调用解析 ───

    /// `extern::<block>::<func>(args)` → 路由到对应 extern 块
    fn resolve_call_full(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        // 格式：extern::<block>::<func> 或 <block>::<func> 或直接 <func>
        let func_name = func_name.strip_prefix("extern::").unwrap_or(func_name);

        if let Some((block_name, rest)) = func_name.split_once("::") {
            if let Some(block) = self.blocks.get(block_name) {
                if let Some(extern_fn) = block.functions.iter().find(|f| f.symbol == rest) {
                    return Some(CallResolveResult {
                        rust_path: extern_fn.symbol.clone(),
                        shim: format!("_lz_extern_{}_{}", block_name, extern_fn.symbol),
                        module_name: block_name.to_string(),
                        is_macro: false,
                        is_template: false,
                        ret_result: false,
                    });
                }
            }
        }

        // 直接符号查找
        if let Some(block_name) = self.symbol_map.get(func_name) {
            let block = self.blocks.get(block_name)?;
            let extern_fn = block.functions.iter().find(|f| f.symbol == func_name)?;
            return Some(CallResolveResult {
                rust_path: extern_fn.symbol.clone(),
                shim: format!("_lz_extern_{}_{}", block_name, extern_fn.symbol),
                module_name: block_name.clone(),
                is_macro: false,
                is_template: false,
                ret_result: false,
            });
        }

        None
    }

    fn resolve_call(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        self.resolve_call_full(func_name, _args)
    }

    // ─── 方法解析（不支持，直接透传）───

    fn resolve_method(&self, method: &str, _receiver_type: &str) -> Option<MethodResolveResult> {
        None // 透传给其他 bridge
    }

    // ─── 导出枚举 ───

    fn list_exports(&self, kind: ExportKind) -> Vec<ExportEntry> {
        match kind {
            ExportKind::Function => {
                let mut entries = Vec::new();
                for (block_name, block) in &self.blocks {
                    for func in &block.functions {
                        entries.push(ExportEntry {
                            name: func.symbol.clone(),
                            kind: ExportKind::Function,
                            signature: format!(
                                "fn {}({}) -> {}",
                                func.symbol,
                                func.params.join(", "),
                                func.return_type
                            ),
                            module: block_name.clone(),
                        });
                    }
                }
                entries
            }
            ExportKind::Module => {
                self.blocks.keys().map(|name| ExportEntry {
                    name: name.clone(),
                    kind: ExportKind::Module,
                    signature: format!("extern \"Rust\" block: {}", name),
                    module: String::new(),
                }).collect()
            }
            _ => vec![],
        }
    }

    fn export_count(&self) -> usize {
        self.blocks.values().map(|b| b.functions.len()).sum()
    }
}

// ══════════════════════════════════════════════════════════════
// 单元测试
// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bridge() -> ExternBridge {
        let mut bridge = ExternBridge::new();
        bridge.register_fn("serde", "to_string", vec!["&str".to_string()], "String");
        bridge.register_fn("serde", "from_str", vec!["&str".to_string()], "serde_json::Value");
        bridge.register_fn("rayon", "par_iter", vec!["&[T]".to_string()], "RayonIter<T>");
        bridge
    }

    #[test]
    fn test_extern_bridge_new() {
        let bridge = ExternBridge::new();
        assert_eq!(bridge.name(), "extern_rust");
        assert_eq!(bridge.level(), BridgeLevel::LinkTime);
        assert!(bridge.capabilities().contains(BridgeCapability::FUNCTION_CALL));
        assert!(bridge.capabilities().contains(BridgeCapability::IMPORT));
    }

    #[test]
    fn test_register_fn() {
        let mut bridge = ExternBridge::new();
        bridge.register_fn("test", "foo", vec!["i32".to_string(), "i64".to_string()], "i64");
        assert_eq!(bridge.export_count(), 1);
    }

    #[test]
    fn test_resolve_import_extern_serde() {
        let bridge = make_bridge();
        let path = vec!["extern".into(), "serde".into(), "to_string".into()];
        let r = bridge.resolve_import_full(&path, &[]).unwrap();
        assert_eq!(r.rust_path, "serde::to_string");
    }

    #[test]
    fn test_resolve_import_extern_rayon() {
        let bridge = make_bridge();
        let path = vec!["extern".into(), "rayon".into()];
        let r = bridge.resolve_import_full(&path, &[]).unwrap();
        assert_eq!(r.rust_path, "rayon");
    }

    #[test]
    fn test_resolve_import_non_extern_ignored() {
        let bridge = make_bridge();
        let path = vec!["std".into(), "io".into()];
        assert!(bridge.resolve_import_full(&path, &[]).is_none());
    }

    #[test]
    fn test_resolve_import_short_path_ignored() {
        let bridge = make_bridge();
        // 仅 "extern" 不够
        let path = vec!["extern".into()];
        assert!(bridge.resolve_import_full(&path, &[]).is_none());
    }

    #[test]
    fn test_resolve_call_full() {
        let bridge = make_bridge();
        let r = bridge.resolve_call_full("serde::to_string", &[]).unwrap();
        assert_eq!(r.rust_path, "to_string");
        assert_eq!(r.shim, "_lz_extern_serde_to_string");
        assert_eq!(r.module_name, "serde");
    }

    #[test]
    fn test_resolve_call_extern_prefix() {
        let bridge = make_bridge();
        let r = bridge.resolve_call_full("extern::serde::to_string", &[]).unwrap();
        assert_eq!(r.rust_path, "to_string");
    }

    #[test]
    fn test_resolve_call_unknown() {
        let bridge = make_bridge();
        assert!(bridge.resolve_call_full("nonexistent::func", &[]).is_none());
    }

    #[test]
    fn test_generate_extern_blocks() {
        let bridge = make_bridge();
        let code = bridge.generate_extern_blocks();
        assert!(code.contains("extern \"Rust\""));
        assert!(code.contains("fn to_string(&str)"));
        assert!(code.contains("fn from_str(&str) -> serde_json::Value"));
        assert!(code.contains("fn par_iter(&[T])"));
    }

    #[test]
    fn test_generate_extern_blocks_void_return() {
        let mut bridge = ExternBridge::new();
        bridge.register_fn("log", "info", vec!["&str".to_string()], "()");
        let code = bridge.generate_extern_blocks();
        assert!(code.contains("fn info(&str);"));
        assert!(!code.contains("-> ()"));
    }

    #[test]
    fn test_generate_safe_wrappers() {
        let bridge = make_bridge();
        let code = bridge.generate_safe_wrappers();
        assert!(code.contains("pub unsafe fn _lz_extern_serde_to_string"));
        assert!(code.contains("pub unsafe fn _lz_extern_serde_from_str"));
        // 非 () 返回类型 → unsafe wrapper
        assert!(code.contains("pub unsafe fn _lz_extern_rayon_par_iter"));
    }

    #[test]
    fn test_safe_wrapper_void() {
        let mut bridge = ExternBridge::new();
        bridge.register_fn("log", "info", vec!["&str".to_string()], "()");
        let code = bridge.generate_safe_wrappers();
        // void return → 不使用 unsafe fn
        assert!(code.contains("pub fn _lz_extern_log_info"));
    }

    #[test]
    fn test_generate_cargo_hints() {
        let mut bridge = ExternBridge::new();
        bridge.register_fn("rayon", "par_iter", vec![], "RayonIter");
        // 空记录时无提示
        assert!(bridge.generate_cargo_hints().is_empty());
        // 记录后输出依赖提示
        bridge.record_crate("rayon");
        let hints = bridge.generate_cargo_hints();
        assert!(hints.contains("rayon"));
        assert!(hints.contains("Cargo Dependencies"));
    }

    #[test]
    fn test_list_exports_functions() {
        let bridge = make_bridge();
        let exports = bridge.list_exports(ExportKind::Function);
        assert_eq!(exports.len(), 3);
        assert!(exports.iter().any(|e| e.name == "to_string" && e.module == "serde"));
        assert!(exports.iter().any(|e| e.name == "from_str" && e.module == "serde"));
        assert!(exports.iter().any(|e| e.name == "par_iter" && e.module == "rayon"));
    }

    #[test]
    fn test_list_exports_modules() {
        let bridge = make_bridge();
        let exports = bridge.list_exports(ExportKind::Module);
        assert_eq!(exports.len(), 2);
        assert!(exports.iter().any(|e| e.name == "serde"));
        assert!(exports.iter().any(|e| e.name == "rayon"));
    }

    #[test]
    fn test_used_crates() {
        let mut bridge = ExternBridge::new();
        bridge.register_fn("rayon", "par_iter", vec![], "Iter");
        bridge.record_crate("rayon");
        bridge.record_crate("serde_json");
        bridge.record_crate("rayon"); // 重复

        let crates = bridge.used_crates().borrow();
        assert_eq!(crates.len(), 2);
        assert!(crates.contains(&"rayon".to_string()));
        assert!(crates.contains(&"serde_json".to_string()));
    }

    #[test]
    fn test_gen_import() {
        let bridge = make_bridge();
        let code = bridge.gen_import(&["extern".into(), "serde".into()], &[]);
        assert_eq!(code, "use serde;\n");
    }

    #[test]
    fn test_link_attr_for_non_std_crate() {
        // #[link] 仅在 block 显式携带 crate_name 时生成
        let mut bridge = ExternBridge::new();
        bridge.register_block(ExternBlock {
            name: "rayon".to_string(),
            crate_name: Some("rayon".to_string()),
            functions: vec![ExternFn {
                symbol: "par_iter".to_string(),
                params: vec!["&[T]".to_string()],
                return_type: "RayonIter<T>".to_string(),
                inline: false,
            }],
            is_std: false,
        });
        let code = bridge.generate_extern_blocks();
        assert!(code.contains("#[link(name = \"rayon\", kind = \"dylib\")]"));
    }

    #[test]
    fn test_resolve_call_direct_symbol() {
        let mut bridge = ExternBridge::new();
        bridge.register_fn("crypto", "sha256", vec!["&str".to_string()], "String");
        // 直接符号查找
        let r = bridge.resolve_call_full("sha256", &[]).unwrap();
        assert_eq!(r.rust_path, "sha256");
        assert_eq!(r.shim, "_lz_extern_crypto_sha256");
    }

    #[test]
    fn test_export_count() {
        let bridge = make_bridge();
        assert_eq!(bridge.export_count(), 3);
    }

    #[test]
    fn test_inline_function() {
        let mut bridge = ExternBridge::new();
        let mut fn_entry = ExternFn {
            symbol: "hot_path".to_string(),
            params: vec!["i32".to_string()],
            return_type: "i32".to_string(),
            inline: true,
        };

        let block = ExternBlock {
            name: "perf".to_string(),
            crate_name: None,
            functions: vec![fn_entry],
            is_std: false,
        };
        bridge.register_block(block);

        let code = bridge.generate_extern_blocks();
        assert!(code.contains("#[inline]"));
        assert!(code.contains("fn hot_path(i32) -> i32"));
    }

    #[test]
    fn test_empty_bridge() {
        let bridge = ExternBridge::new();
        assert_eq!(bridge.export_count(), 0);
        assert!(bridge.generate_extern_blocks().is_empty());
        assert!(bridge.generate_safe_wrappers().is_empty());
        assert!(bridge.used_crates().borrow().is_empty());
    }

    #[test]
    fn test_multiple_blocks_independent() {
        let mut bridge = ExternBridge::new();
        bridge.register_fn("block_a", "fn_a", vec![], "()");
        bridge.register_fn("block_b", "fn_b", vec![], "()");
        bridge.register_fn("block_b", "fn_c", vec![], "i64");

        let exports = bridge.list_exports(ExportKind::Function);
        assert_eq!(exports.len(), 3);
        let module_exports = bridge.list_exports(ExportKind::Module);
        assert_eq!(module_exports.len(), 2);
    }

    #[test]
    fn test_resolve_import_nested_path() {
        let bridge = make_bridge();
        let path = vec![
            "extern".into(),
            "serde".into(),
            "ser".into(),
            "to_string".into(),
        ];
        let r = bridge.resolve_import_full(&path, &[]).unwrap();
        assert_eq!(r.rust_path, "serde::ser::to_string");
    }

    #[test]
    fn test_capabilities_no_import() {
        // extern bridge 没有 METHOD_CALL 能力
        let bridge = ExternBridge::new();
        let caps = bridge.capabilities();
        assert!(caps.contains(BridgeCapability::FUNCTION_CALL));
        assert!(caps.contains(BridgeCapability::IMPORT));
        assert!(caps.contains(BridgeCapability::TYPE_REWRITE));
        assert!(!caps.contains(BridgeCapability::METHOD_CALL));
    }
}

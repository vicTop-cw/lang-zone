// Lang-Zong 编译器 — bridge/wasm.rs
// Level 1: WebAssembly 桥接（wasm-bindgen）
// 链接时绑定，通过 wasm-bindgen + wasm-pack 生成 .wasm + JS glue 代码。
// 实现 Bridge trait，从 TOML 清单读取导出声明并生成 wasm-bindgen 包装代码。

use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeError, BridgeLevel, BridgeMeta,
    CallResolveResult, ErrorCode, ExportEntry, ExportKind,
};
use crate::util::parse;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ──────────────── WASM 声明 ────────────────

/// 单个 JS 导出函数声明
#[derive(Debug, Clone)]
pub struct WasmExport {
    pub name: String,
    pub args: Vec<(String, String)>,  // (name, lz_type)
    pub ret: String,                  // lz return type
    pub doc: String,
}

/// JS 导出类型（lz struct → wasm-bindgen class）
#[derive(Debug, Clone)]
pub struct WasmTypeExport {
    pub name: String,
    pub fields: Vec<(String, String)>,  // (field_name, lz_type)
    pub doc: String,
}

/// WASM 包配置
#[derive(Debug, Clone)]
pub struct WasmPackageConfig {
    pub name: String,           // npm 包名
    pub version: String,
    pub description: String,
    pub target: WasmTarget,     // web / nodejs / no-modules
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmTarget {
    Web,          // --target web (ES modules)
    NodeJs,       // --target nodejs (CommonJS)
    NoModules,    // --target no-modules (script tag)
    Bundler,      // --target bundler (webpack/rollup)
}

impl WasmTarget {
    fn from_str(s: &str) -> Self {
        match s {
            "web" => WasmTarget::Web,
            "nodejs" | "node" => WasmTarget::NodeJs,
            "no-modules" | "nomodules" | "script" => WasmTarget::NoModules,
            _ => WasmTarget::Bundler,
        }
    }
    fn as_flag(&self) -> &str {
        match self {
            WasmTarget::Web => "--target web",
            WasmTarget::NodeJs => "--target nodejs",
            WasmTarget::NoModules => "--target no-modules",
            WasmTarget::Bundler => "--target bundler",
        }
    }
}

// ──────────────── WasmBridge ────────────────

/// Level 1: wasm-bindgen WebAssembly 桥接实现
#[derive(Debug)]
pub struct WasmBridge {
    package: WasmPackageConfig,
    functions: HashMap<String, WasmExport>,
    types: HashMap<String, WasmTypeExport>,
}

impl WasmBridge {
    /// 从 TOML 清单加载 WASM 导出声明
    ///
    /// 清单格式：
    /// ```toml
    /// [package]
    /// name = "lz-lib"
    /// version = "0.1.0"
    /// description = "WASM bindings"
    /// target = "web"       # web | nodejs | no-modules | bundler
    ///
    /// [functions]
    /// greet = { args = "name: str", ret = "str", doc = "Greet from WASM" }
    /// fibonacci = { args = "n: int", ret = "int" }
    ///
    /// [types]
    /// Point = { fields = "x: f64, y: f64", doc = "2D point" }
    /// Color = { fields = "r: int, g: int, b: int" }
    /// ```
    pub fn load(path: &Path) -> Result<Self, BridgeError> {
        let content = fs::read_to_string(path)
            .map_err(|e| BridgeError::new(ErrorCode::ConnectionFailed,
                format!("read {}: {}", path.display(), e), "wasm"))?;

        let doc = parse(&content)
            .map_err(|e| BridgeError::new(ErrorCode::InvalidMessage,
                format!("parse {}: {}", path.display(), e), "wasm"))?;

        // [package] section
        let pkg_sec = doc.get("package")
            .ok_or_else(|| BridgeError::new(ErrorCode::InvalidMessage,
                "missing [package] section", "wasm"))?;

        let package = WasmPackageConfig {
            name: pkg_sec.get("name").and_then(|v| v.as_str()).unwrap_or("lz-wasm").to_string(),
            version: pkg_sec.get("version").and_then(|v| v.as_str()).unwrap_or("0.1.0").to_string(),
            description: pkg_sec.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            target: WasmTarget::from_str(
                pkg_sec.get("target").and_then(|v| v.as_str()).unwrap_or("bundler")
            ),
        };

        // [functions] section
        let mut functions = HashMap::new();
        if let Some(funcs_map) = doc.get("functions") {
            for (name, entry) in funcs_map.iter() {
                let table = entry.as_table();
                let args_str = table.and_then(|t| t.get("args")).and_then(|v| v.as_str()).unwrap_or("");
                let ret = table.and_then(|t| t.get("ret")).and_then(|v| v.as_str()).unwrap_or("void").to_string();
                let doc = table.and_then(|t| t.get("doc")).and_then(|v| v.as_str()).unwrap_or("").to_string();

                let args: Vec<(String, String)> = if args_str.is_empty() {
                    vec![]
                } else {
                    args_str.split(',').filter_map(|p| {
                        let parts: Vec<&str> = p.trim().splitn(2, ':').collect();
                        if parts.len() == 2 {
                            Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
                        } else { None }
                    }).collect()
                };

                functions.insert(name.clone(), WasmExport { name: name.clone(), args, ret, doc });
            }
        }

        // [types] section
        let mut types = HashMap::new();
        if let Some(type_map) = doc.get("types") {
            for (name, entry) in type_map.iter() {
                let table = entry.as_table();
                let fields_str = table.and_then(|t| t.get("fields")).and_then(|v| v.as_str()).unwrap_or("");
                let doc = table.and_then(|t| t.get("doc")).and_then(|v| v.as_str()).unwrap_or("").to_string();

                let fields: Vec<(String, String)> = if fields_str.is_empty() {
                    vec![]
                } else {
                    fields_str.split(',').filter_map(|f| {
                        let parts: Vec<&str> = f.trim().splitn(2, ':').collect();
                        if parts.len() == 2 {
                            Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
                        } else { None }
                    }).collect()
                };

                types.insert(name.clone(), WasmTypeExport { name: name.clone(), fields, doc });
            }
        }

        Ok(WasmBridge {
            package,
            functions,
            types,
        })
    }

    // ─── 代码生成 ───

    /// 生成完整的 wasm-bindgen Rust 源码
    pub fn generate_module(&self) -> String {
        let mut out = String::new();

        out.push_str("// Generated by Lang-Zong WasmBridge\n");
        out.push_str(&format!("// NPM package: {}\n", self.package.name));
        out.push_str(&format!("// Build: wasm-pack build {}\n\n", self.package.target.as_flag()));
        out.push_str("use wasm_bindgen::prelude::*;\n\n");

        // 当 JS 未设置 panic hook 时自动设置
        out.push_str("#[wasm_bindgen(start)]\n");
        out.push_str("pub fn _wasm_start() {\n");
        out.push_str("    console_error_panic_hook::set_once();\n");
        out.push_str("}\n\n");

        // 类型导出
        for (_, ty) in &self.types {
            out.push_str(&self.generate_wasm_class(ty));
            out.push('\n');
        }

        // 函数导出
        for (_, func) in &self.functions {
            out.push_str(&self.generate_wasm_function(func));
            out.push('\n');
        }

        out
    }

    /// 生成 #[wasm_bindgen] struct
    fn generate_wasm_class(&self, ty: &WasmTypeExport) -> String {
        let mut out = String::new();

        if !ty.doc.is_empty() {
            out.push_str(&format!("/// {}\n", ty.doc));
        }
        out.push_str("#[wasm_bindgen]\n");
        out.push_str(&format!("pub struct {} {{\n", ty.name));

        let mut field_decls = Vec::new();
        for (field_name, lz_type) in &ty.fields {
            let wasm_type = self.lz_to_wasm(lz_type);
            out.push_str(&format!("    pub {}: {},\n", field_name, wasm_type));
            field_decls.push((field_name.clone(), wasm_type));
        }
        out.push_str("}\n\n");

        // #[wasm_bindgen] impl block
        out.push_str("#[wasm_bindgen]\n");
        out.push_str(&format!("impl {} {{\n", ty.name));
        out.push_str("    #[wasm_bindgen(constructor)]\n");

        let params: Vec<String> = field_decls.iter()
            .map(|(n, t)| format!("{}: {}", n, t))
            .collect();
        let field_inits: Vec<String> = field_decls.iter()
            .map(|(n, _)| n.clone())
            .collect();

        out.push_str(&format!("    pub fn new({}) -> {} {{\n", params.join(", "), ty.name));
        out.push_str(&format!("        {} {{ {} }}\n", ty.name, field_inits.join(", ")));
        out.push_str("    }\n");

        // Getters
        for (field_name, wasm_type) in &field_decls {
            out.push_str(&format!("\n    #[wasm_bindgen(getter)]\n"));
            out.push_str(&format!("    pub fn {}(&self) -> {} {{\n", field_name, wasm_type));
            out.push_str(&format!("        self.{}.clone()\n", field_name));
            out.push_str("    }\n");

            out.push_str(&format!("\n    #[wasm_bindgen(setter)]\n"));
            out.push_str(&format!("    pub fn set_{}(&mut self, value: {}) {{\n", field_name, wasm_type));
            out.push_str(&format!("        self.{} = value;\n", field_name));
            out.push_str("    }\n");
        }

        out.push_str("}\n");
        out
    }

    /// 生成 #[wasm_bindgen] 函数
    fn generate_wasm_function(&self, func: &WasmExport) -> String {
        let mut out = String::new();

        if !func.doc.is_empty() {
            out.push_str(&format!("/// {}\n", func.doc));
        }
        out.push_str("#[wasm_bindgen]\n");

        let params: Vec<String> = func.args.iter()
            .map(|(n, t)| format!("{}: {}", n, self.lz_to_wasm(t)))
            .collect();
        let ret = self.lz_to_wasm(&func.ret);

        out.push_str(&format!("pub fn {}({}) -> {} {{\n", func.name, params.join(", "), ret));
        out.push_str(&format!("    // delegate to lz function __wasm_{}\n", func.name));
        out.push_str(&format!("    __wasm_{}({})\n", func.name,
            func.args.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>().join(", ")));
        out.push_str("}\n");

        out
    }

    /// 生成 package.json
    pub fn generate_package_json(&self) -> String {
        format!(r#"{{
  "name": "{}",
  "version": "{}",
  "description": "{}",
  "main": "pkg/{}.js",
  "types": "pkg/{}.d.ts",
  "scripts": {{
    "build": "wasm-pack build {}",
    "test": "node test.js"
  }},
  "devDependencies": {{
    "wasm-pack": "^0.12.0"
  }}
}}"#, 
            self.package.name,
            self.package.version,
            self.package.description,
            self.package.name,
            self.package.name,
            self.package.target.as_flag(),
        )
    }

    /// 生成 Cargo.toml 片段
    pub fn generate_cargo_deps(&self) -> String {
        format!(r#"[dependencies]
wasm-bindgen = "0.2"
console_error_panic_hook = "0.1"

[lib]
crate-type = ["cdylib", "rlib"]
"#)
    }

    /// lz 类型 → wasm-bindgen 兼容类型
    fn lz_to_wasm(&self, lz_type: &str) -> String {
        match lz_type {
            "int" | "i32" => "i32".into(),
            "i64" => "i64".into(),
            "u32" | "uint" => "u32".into(),
            "u64" => "u64".into(),
            "f64" | "float" | "number" => "f64".into(),
            "f32" => "f32".into(),
            "str" | "String" | "string" => "String".into(),
            "bool" => "bool".into(),
            "void" | "None" | "unit" | "()" => "()".into(),
            "any" | "JsValue" | "object" => "JsValue".into(),
            "Array" | "Vec" => "js_sys::Array".into(),
            "Promise" => "js_sys::Promise".into(),
            "Uint8Array" => "js_sys::Uint8Array".into(),
            other => other.to_string(),
        }
    }
}

// ──────────────── Bridge trait ────────────────

impl Bridge for WasmBridge {
    fn name(&self) -> &str { "wasm" }

    fn level(&self) -> BridgeLevel { BridgeLevel::LinkTime }

    fn capabilities(&self) -> BridgeCapability {
        BridgeCapability::FUNCTION_CALL | BridgeCapability::TYPE_REWRITE
    }

    fn resolve_call(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        if self.functions.contains_key(func_name) {
            Some(CallResolveResult {
                rust_path: format!("__wasm_{}", func_name),
                shim: String::new(),
                module_name: "wasm".into(),
                is_macro: false,
                is_template: false,
                ret_result: false,
            })
        } else {
            None
        }
    }

    fn resolve_type(&self, lz_type: &str) -> Option<String> {
        if self.types.contains_key(lz_type) {
            Some(lz_type.to_string())
        } else {
            None
        }
    }

    fn meta(&self) -> BridgeMeta {
        let target_str = match self.package.target {
            WasmTarget::Web => "web (ES modules)",
            WasmTarget::NodeJs => "nodejs (CommonJS)",
            WasmTarget::NoModules => "no-modules (script tag)",
            WasmTarget::Bundler => "bundler (webpack/rollup)",
        };
        BridgeMeta {
            version: self.package.version.clone(),
            description: format!("WASM bridge: {} (target: {}, {} functions, {} types)",
                self.package.description, target_str, self.functions.len(), self.types.len()),
            provides: vec!["wasm".into(), "wasm-bindgen".into(), "javascript".into()],
            ..Default::default()
        }
    }

    fn list_exports(&self, kind: ExportKind) -> Vec<ExportEntry> {
        match kind {
            ExportKind::Function => {
                self.functions.values().map(|f| ExportEntry {
                    name: f.name.clone(),
                    kind: ExportKind::Function,
                    signature: format!("fn {}({}) -> {}",
                        f.name,
                        f.args.iter().map(|(n, t)| format!("{}: {}", n, t)).collect::<Vec<_>>().join(", "),
                        f.ret),
                    module: self.package.name.clone(),
                }).collect()
            }
            ExportKind::Type => {
                self.types.values().map(|t| ExportEntry {
                    name: t.name.clone(),
                    kind: ExportKind::Type,
                    signature: format!("class {} {{ {} }}",
                        t.name,
                        t.fields.iter().map(|(n, ty)| format!("{}: {}", n, ty)).collect::<Vec<_>>().join(", ")),
                    module: self.package.name.clone(),
                }).collect()
            }
            _ => vec![],
        }
    }

    fn export_count(&self) -> usize {
        self.functions.len() + self.types.len()
    }
}

// ──────────────── 测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::TempDir;

    fn create_test_manifest(dir: &TempDir) -> std::path::PathBuf {
        let content = r#"
[package]
name = "lz-wasm-lib"
version = "0.1.0"
description = "WASM test bindings"
target = "web"

[functions]
greet = { args = "name: str", ret = "str", doc = "Greet from WebAssembly" }
fib = { args = "n: int", ret = "int" }
process = { args = "data: f64, scale: f64", ret = "f64" }

[types]
Point = { fields = "x: f64, y: f64", doc = "A 2D point" }
Color = { fields = "r: int, g: int, b: int" }
"#;
        dir.create_file("wasm.toml", content).unwrap()
    }

    #[test]
    fn test_wasm_bridge_load() {
        let dir = TempDir::new("lz-wasm").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = WasmBridge::load(&path).unwrap();

        assert_eq!(bridge.name(), "wasm");
        assert_eq!(bridge.level(), BridgeLevel::LinkTime);
        assert!(bridge.capabilities().contains(BridgeCapability::FUNCTION_CALL));
        assert_eq!(bridge.functions.len(), 3);
        assert_eq!(bridge.types.len(), 2);
    }

    #[test]
    fn test_wasm_bridge_gen_call() {
        let dir = TempDir::new("lz-wasm").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = WasmBridge::load(&path).unwrap();

        assert_eq!(bridge.resolve_call("greet", &[]).map(|r| r.rust_path), Some("__wasm_greet".to_string()));
        assert_eq!(bridge.resolve_call("fib", &[]).map(|r| r.rust_path), Some("__wasm_fib".to_string()));
        assert!(bridge.resolve_call("nonexistent", &[]).is_none());
    }

    #[test]
    fn test_wasm_module_generation() {
        let dir = TempDir::new("lz-wasm").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = WasmBridge::load(&path).unwrap();

        let code = bridge.generate_module();
        assert!(code.contains("#[wasm_bindgen(start)]"));
        assert!(code.contains("#[wasm_bindgen]\npub fn greet"));
        assert!(code.contains("#[wasm_bindgen]\npub fn fib"));
        assert!(code.contains("#[wasm_bindgen]\npub struct Point"));
        assert!(code.contains("#[wasm_bindgen]\npub struct Color"));
        assert!(code.contains("console_error_panic_hook::set_once"));
        assert!(code.contains("#[wasm_bindgen(constructor)]"));
    }

    #[test]
    fn test_wasm_package_json() {
        let dir = TempDir::new("lz-wasm").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = WasmBridge::load(&path).unwrap();

        let pkg = bridge.generate_package_json();
        assert!(pkg.contains("\"name\": \"lz-wasm-lib\""));
        assert!(pkg.contains("\"build\": \"wasm-pack build"));
    }

    #[test]
    fn test_wasm_cargo_deps() {
        let dir = TempDir::new("lz-wasm").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = WasmBridge::load(&path).unwrap();

        let deps = bridge.generate_cargo_deps();
        assert!(deps.contains("wasm-bindgen"));
        assert!(deps.contains("cdylib"));
    }

    #[test]
    fn test_wasm_list_exports() {
        let dir = TempDir::new("lz-wasm").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = WasmBridge::load(&path).unwrap();

        let funcs = bridge.list_exports(ExportKind::Function);
        assert_eq!(funcs.len(), 3);
        assert!(funcs.iter().any(|e| e.name == "greet"));
        assert!(funcs.iter().any(|e| e.name == "fib"));

        let types = bridge.list_exports(ExportKind::Type);
        assert_eq!(types.len(), 2);
        assert!(types.iter().any(|e| e.name == "Point"));
    }

    #[test]
    fn test_wasm_type_mapping() {
        let dir = TempDir::new("lz-wasm").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = WasmBridge::load(&path).unwrap();

        assert_eq!(bridge.lz_to_wasm("int"), "i32");
        assert_eq!(bridge.lz_to_wasm("f64"), "f64");
        assert_eq!(bridge.lz_to_wasm("str"), "String");
        assert_eq!(bridge.lz_to_wasm("bool"), "bool");
        assert_eq!(bridge.lz_to_wasm("void"), "()");
        assert_eq!(bridge.lz_to_wasm("any"), "JsValue");
        assert_eq!(bridge.lz_to_wasm("Array"), "js_sys::Array");
    }

    #[test]
    fn test_wasm_target_variants() {
        assert_eq!(WasmTarget::from_str("web"), WasmTarget::Web);
        assert_eq!(WasmTarget::from_str("node").as_flag(), "--target nodejs");
        assert_eq!(WasmTarget::from_str("bundler").as_flag(), "--target bundler");
        assert_eq!(WasmTarget::from_str("no-modules").as_flag(), "--target no-modules");
        // unknown defaults to bundler
        assert_eq!(WasmTarget::from_str("unknown").as_flag(), "--target bundler");
    }
}

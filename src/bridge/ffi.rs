// Lang-Zong 编译器 — bridge_ffi.rs
// Level 2: C ABI FFI 桥接
// 运行时动态/静态链接，通过 extern "C" 调用 C 库函数。
// 实现 Bridge trait，从 TOML 清单读取 FFI 声明并生成绑定代码。

use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeError, BridgeLevel, BridgeMeta,
    CallResolveResult, ErrorCode, ExportEntry, ExportKind,
};
use crate::util::parse;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ──────────────── FFI 函数声明 ────────────────

/// 单个 FFI 函数的声明
#[derive(Debug, Clone)]
pub struct FfiFunction {
    pub name: String,
    pub params: Vec<String>,    // C 类型名
    pub return_type: String,    // C 类型名
    pub link_kind: LinkKind,
    pub library: Option<String>, // .so/.dll/.dylib 名称
}

/// 链接类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkKind {
    Static,   // #[link(name = "foo", kind = "static")]
    Dynamic,  // #[link(name = "foo", kind = "dylib")]
    Framework, // #[link(name = "foo", kind = "framework")] (macOS)
}

/// 类型 marshaling 映射表
#[derive(Debug, Clone)]
pub struct TypeMarshal {
    /// lz 类型 → C 类型（参数方向）
    #[allow(dead_code)]
    lz_to_c: HashMap<String, String>,
    /// C 类型 → lz 类型（返回值方向）
    #[allow(dead_code)]
    c_to_lz: HashMap<String, String>,
}

impl Default for TypeMarshal {
    fn default() -> Self {
        let mut lz_to_c = HashMap::new();
        let mut c_to_lz = HashMap::new();

        // 基础类型映射
        lz_to_c.insert("int".to_string(), "i64".to_string());
        lz_to_c.insert("f64".to_string(), "f64".to_string());
        lz_to_c.insert("str".to_string(), "*const std::os::raw::c_char".to_string());
        lz_to_c.insert("bool".to_string(), "i32".to_string()); // C bool → i32
        lz_to_c.insert("*const ()".to_string(), "*const std::os::raw::c_void".to_string());
        lz_to_c.insert("*mut ()".to_string(), "*mut std::os::raw::c_void".to_string());
        lz_to_c.insert("usize".to_string(), "usize".to_string());

        // 返回值映射
        c_to_lz.insert("i64".to_string(), "int".to_string());
        c_to_lz.insert("f64".to_string(), "f64".to_string());
        c_to_lz.insert("i32".to_string(), "int".to_string());
        c_to_lz.insert("usize".to_string(), "int".to_string());
        c_to_lz.insert("void".to_string(), "()".to_string());

        TypeMarshal { lz_to_c, c_to_lz }
    }
}

// ──────────────── FFI 桥接 ────────────────

/// Level 2: C ABI FFI 桥接
#[derive(Debug)]
pub struct FfiBridge {
    functions: HashMap<String, FfiFunction>,
    #[allow(dead_code)]
    marshal: TypeMarshal,
    #[allow(dead_code)]
    manifest_path: String,
}

impl FfiBridge {
    /// 从 TOML 清单加载 FFI 声明
    pub fn load(path: &Path) -> Result<Self, BridgeError> {
        let content = fs::read_to_string(path)
            .map_err(|e| BridgeError::new(ErrorCode::ConnectionFailed, e.to_string(), "ffi"))?;
        let doc = parse(&content)
            .map_err(|e| BridgeError::new(ErrorCode::DeserializationError, e, "ffi"))?;

        let mut functions = HashMap::new();

        // 读取 [functions] section
        if let Some(funcs_section) = doc.get("functions") {
            for (name, val) in funcs_section {
                if let Some(table) = val.as_table() {
                    let params: Vec<String> = table.get("params")
                        .and_then(|v| v.as_str())
                        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
                        .unwrap_or_default();

                    let return_type = table.get("return")
                        .and_then(|v| v.as_str())
                        .unwrap_or("void")
                        .to_string();

                    let library = table.get("library")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let link_kind = table.get("link")
                        .and_then(|v| v.as_str())
                        .map(|s| match s {
                            "static" => LinkKind::Static,
                            "framework" => LinkKind::Framework,
                            _ => LinkKind::Dynamic,
                        })
                        .unwrap_or(LinkKind::Dynamic);

                    functions.insert(name.clone(), FfiFunction {
                        name: name.clone(),
                        params,
                        return_type,
                        link_kind,
                        library,
                    });
                }
            }
        }

        Ok(FfiBridge {
            functions,
            marshal: TypeMarshal::default(),
            manifest_path: path.display().to_string(),
        })
    }

    /// 注册单个 FFI 函数
    pub fn register(&mut self, func: FfiFunction) {
        self.functions.insert(func.name.clone(), func);
    }

    /// 生成 extern "C" 块
    pub fn generate_extern_block(&self) -> String {
        let mut out = String::new();
        out.push_str("// ── FFI extern \"C\" block ──\n");
        out.push_str("extern \"C\" {\n");

        for func in self.functions.values() {
            let params = if func.params.is_empty() {
                String::new()
            } else {
                func.params.join(", ")
            };
            out.push_str(&format!("    fn {}({}) -> {};\n",
                func.name, params, func.return_type));
        }

        out.push_str("}\n\n");
        out
    }

    /// 生成链接属性（用于 cdylib/dylib）
    pub fn generate_link_attrs(&self) -> String {
        let mut out = String::new();
        for func in self.functions.values() {
            if let Some(lib) = &func.library {
                let kind_str = match func.link_kind {
                    LinkKind::Static => "static",
                    LinkKind::Dynamic => "dylib",
                    LinkKind::Framework => "framework",
                };
                out.push_str(&format!(
                    "#[link(name = \"{}\", kind = \"{}\")]\n",
                    lib, kind_str
                ));
            }
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out
    }

    /// 生成类型适配包装函数（lz 安全类型 → C 裸指针）
    pub fn generate_safe_wrappers(&self) -> String {
        let mut out = String::new();
        out.push_str("// ── FFI safe wrappers ──\n");

        for func in self.functions.values() {
            out.push_str(&format!(
                "pub unsafe fn __ffi_{name}({params}) -> {ret} {{\n    {name}({args})\n}}\n\n",
                name = func.name,
                params = self.wrapper_params(func),
                ret = self.marshal_return(&func.return_type),
                args = self.wrapper_args(func),
            ));
        }

        out
    }

    fn wrapper_params(&self, func: &FfiFunction) -> String {
        func.params.iter()
            .map(|p| format!("{}: {}", self.param_name(p), self.marshal_c_to_rust(p)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn wrapper_args(&self, func: &FfiFunction) -> String {
        func.params.iter()
            .map(|p| self.param_name(p))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn param_name(&self, c_type: &str) -> String {
        // 根据 C 类型生成参数名
        match c_type {
            t if t.contains("const") => "data".to_string(),
            t if t.contains("mut") => "out".to_string(),
            _ => "n".to_string(),
        }
    }

    fn marshal_c_to_rust(&self, c_type: &str) -> String {
        match c_type {
            "*const std::os::raw::c_char" | "*const i8" => "&str".to_string(),
            "*mut std::os::raw::c_void" | "*mut u8" => "&mut [u8]".to_string(),
            "*const std::os::raw::c_void" | "*const u8" => "&[u8]".to_string(),
            "i32" => "i32".to_string(),
            "i64" => "i64".to_string(),
            "usize" => "usize".to_string(),
            "f64" => "f64".to_string(),
            other => other.to_string(),
        }
    }

    fn marshal_return(&self, c_type: &str) -> String {
        match c_type {
            "void" => "()".to_string(),
            "i32" => "i32".to_string(),
            "i64" => "i64".to_string(),
            "usize" => "usize".to_string(),
            "f64" => "f64".to_string(),
            other => other.to_string(),
        }
    }
}

impl Bridge for FfiBridge {
    fn name(&self) -> &str { "ffi" }

    fn level(&self) -> BridgeLevel { BridgeLevel::Runtime }

    fn capabilities(&self) -> BridgeCapability {
        BridgeCapability::FUNCTION_CALL | BridgeCapability::TYPE_REWRITE
    }

    fn gen_call(&self, func_name: &str, _args: &[String]) -> Option<String> {
        self.functions.get(func_name).map(|_| {
            // 生成 wrapper 函数调用：__ffi_<func_name>(args)
            format!("__ffi_{}", func_name)
        })
    }

    fn meta(&self) -> BridgeMeta {
        BridgeMeta {
            version: "0.1.0".into(),
            description: format!("FFI bridge: {} functions linked", self.functions.len()),
            ..Default::default()
        }
    }

    fn resolve_call_full(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        self.gen_call(func_name, _args).map(|rust_path| {
            CallResolveResult {
                rust_path,
                shim: String::new(),
                module_name: "ffi".into(),
                is_macro: false,
                is_template: false,
            }
        })
    }

    fn list_exports(&self, kind: ExportKind) -> Vec<ExportEntry> {
        match kind {
            ExportKind::Function => {
                self.functions.keys().map(|name| ExportEntry {
                    name: name.clone(),
                    kind: ExportKind::Function,
                    signature: format!("extern fn {}", name),
                    module: "ffi".into(),
                }).collect()
            }
            _ => vec![],
        }
    }

    fn export_count(&self) -> usize {
        self.functions.len()
    }
}

// ──────────────── 测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_SEQ: AtomicUsize = AtomicUsize::new(0);

    fn create_test_manifest() -> String {
        let id = TEST_SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir();
        let path = dir.join(format!("test_ffi_{}.toml", id));
        let content = r#"
[functions]
strlen = { params = "*const i8", return = "usize", library = "c", link = "dylib" }
memcpy = { params = "*mut u8, *const u8, usize", return = "void", library = "c", link = "dylib" }
malloc = { params = "usize", return = "*mut u8", library = "c", link = "dylib" }
"#;
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path.display().to_string()
    }

    #[test]
    fn test_ffi_bridge_load() {
        let path = create_test_manifest();
        let bridge = FfiBridge::load(Path::new(&path)).unwrap();
        assert_eq!(bridge.name(), "ffi");
        assert_eq!(bridge.level(), BridgeLevel::Runtime);
        assert!(bridge.functions.contains_key("strlen"));
        assert!(bridge.functions.contains_key("memcpy"));
        assert!(bridge.functions.contains_key("malloc"));
    }

    #[test]
    fn test_ffi_bridge_extern_block() {
        let path = create_test_manifest();
        let bridge = FfiBridge::load(Path::new(&path)).unwrap();
        let block = bridge.generate_extern_block();
        assert!(block.contains("extern \"C\" {"));
        assert!(block.contains("fn strlen("));
        assert!(block.contains("fn memcpy("));
        assert!(block.contains("fn malloc("));
    }

    #[test]
    fn test_ffi_bridge_link_attrs() {
        let path = create_test_manifest();
        let bridge = FfiBridge::load(Path::new(&path)).unwrap();
        let attrs = bridge.generate_link_attrs();
        assert!(attrs.contains("name = \"c\""));
        assert!(attrs.contains("kind = \"dylib\""));
    }

    #[test]
    fn test_ffi_bridge_call() {
        let path = create_test_manifest();
        let bridge = FfiBridge::load(Path::new(&path)).unwrap();
        let result = bridge.gen_call("strlen", &[]);
        assert_eq!(result, Some("__ffi_strlen".to_string()));
        // 不存在函数返回 None
        assert!(bridge.gen_call("nonexistent", &[]).is_none());
    }

    #[test]
    fn test_ffi_bridge_safe_wrappers() {
        let path = create_test_manifest();
        let bridge = FfiBridge::load(Path::new(&path)).unwrap();
        let wrappers = bridge.generate_safe_wrappers();
        assert!(wrappers.contains("pub unsafe fn __ffi_strlen"));
        assert!(wrappers.contains("pub unsafe fn __ffi_memcpy"));
    }

    #[test]
    fn test_ffi_bridge_capabilities() {
        let path = create_test_manifest();
        let bridge = FfiBridge::load(Path::new(&path)).unwrap();
        let caps = bridge.capabilities();
        assert!(caps.contains(BridgeCapability::FUNCTION_CALL));
        assert!(!caps.contains(BridgeCapability::IMPORT));
        assert!(!caps.contains(BridgeCapability::METHOD_CALL));
    }
}

// Lang-Zong 编译器 — bridge/python.rs
// Level 1: PyO3 Python 桥接
// 链接时绑定，通过 pyo3 crate 生成 .pyd/.so Python 扩展模块。
// 实现 Bridge trait，从 TOML 清单读取导出声明并生成 pyo3 包装代码。

use crate::bridge::core::{
    Bridge, BridgeCapability, BridgeError, BridgeLevel, BridgeMeta,
    CallResolveResult, ErrorCode, ExportEntry, ExportKind,
};
use crate::util::parse;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ──────────────── PyO3 声明 ────────────────

/// 单个 Python 导出函数声明
#[derive(Debug, Clone)]
pub struct PyExport {
    pub name: String,
    pub args: Vec<(String, String)>,  // (name, lz_type)
    pub ret: String,                  // lz return type
    pub doc: String,                  // Python docstring
}

/// Python 导出类型声明（lz struct → Python class）
#[derive(Debug, Clone)]
pub struct PyTypeExport {
    pub name: String,
    pub fields: Vec<(String, String)>,  // (field_name, lz_type)
    pub doc: String,
}

/// Python 模块配置
#[derive(Debug, Clone)]
pub struct PyModuleConfig {
    pub name: String,            // Python 模块名
    pub version: String,
    pub description: String,
}

// ──────────────── PyO3Bridge ────────────────

/// Level 1: PyO3 Python 桥接实现
#[derive(Debug)]
pub struct PyO3Bridge {
    module: PyModuleConfig,
    functions: HashMap<String, PyExport>,
    types: HashMap<String, PyTypeExport>,
}

impl PyO3Bridge {
    /// 从 TOML 清单加载 Python 导出声明
    ///
    /// 清单格式：
    /// ```toml
    /// [bridge]
    /// name = "mylib"
    /// version = "0.1.0"
    /// description = "Python bindings"
    ///
    /// [functions]
    /// greet = { args = "s: str", ret = "str", doc = "Say hello" }
    /// add = { args = "a: int, b: int", ret = "int" }
    ///
    /// [types]
    /// Point = { fields = "x: f64, y: f64", doc = "2D point" }
    /// ```
    pub fn load(path: &Path) -> Result<Self, BridgeError> {
        let content = fs::read_to_string(path)
            .map_err(|e| BridgeError::new(ErrorCode::ConnectionFailed,
                format!("read {}: {}", path.display(), e), "pyo3"))?;

        let doc = parse(&content)
            .map_err(|e| BridgeError::new(ErrorCode::InvalidMessage,
                format!("parse {}: {}", path.display(), e), "pyo3"))?;

        // [bridge] section
        let bridge_sec = doc.get("bridge")
            .ok_or_else(|| BridgeError::new(ErrorCode::InvalidMessage,
                "missing [bridge] section", "pyo3"))?;

        let module = PyModuleConfig {
            name: bridge_sec.get("name").and_then(|v| v.as_str()).unwrap_or("lz_module").to_string(),
            version: bridge_sec.get("version").and_then(|v| v.as_str()).unwrap_or("0.1.0").to_string(),
            description: bridge_sec.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };

        // [functions] section
        let mut functions = HashMap::new();
        if let Some(funcs_map) = doc.get("functions") {
            for (name, entry) in funcs_map.iter() {
                let table = entry.as_table();
                let args_str = table.and_then(|t| t.get("args")).and_then(|v| v.as_str()).unwrap_or("");
                let ret = table.and_then(|t| t.get("ret")).and_then(|v| v.as_str()).unwrap_or("None").to_string();
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

                functions.insert(name.clone(), PyExport { name: name.clone(), args, ret, doc });
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

                types.insert(name.clone(), PyTypeExport { name: name.clone(), fields, doc });
            }
        }

        Ok(PyO3Bridge {
            module,
            functions,
            types,
        })
    }

    // ─── 代码生成 ───

    /// 生成完整的 Python 扩展模块 Rust 源码
    pub fn generate_module(&self) -> String {
        let mut out = String::new();

        // 文件头
        out.push_str("// Generated by Lang-Zong PyO3Bridge\n");
        out.push_str("// Python module: ");
        out.push_str(&self.module.name);
        out.push_str("\n\n");
        out.push_str("use pyo3::prelude::*;\n\n");

        // Python 类型类
        for (_, ty) in &self.types {
            out.push_str(&self.generate_pyclass(ty));
            out.push('\n');
        }

        // Python 函数
        for (_, func) in &self.functions {
            out.push_str(&self.generate_pyfunction(func));
            out.push('\n');
        }

        // 模块入口
        out.push_str(&self.generate_pymodule());

        out
    }

    /// 生成 #[pyclass] 结构体
    fn generate_pyclass(&self, ty: &PyTypeExport) -> String {
        let mut out = String::new();
        if !ty.doc.is_empty() {
            out.push_str(&format!("#[doc = \"{}\"]\n", ty.doc));
        }
        out.push_str("#[pyclass]\n");
        out.push_str(&format!("pub struct {} {{\n", ty.name));
        for (field_name, lz_type) in &ty.fields {
            let rust_type = self.map_to_pyo3_type(lz_type);
            out.push_str(&format!("    #[pyo3(get, set)]\n"));
            out.push_str(&format!("    pub {}: {},\n", field_name, rust_type));
        }
        out.push_str("}\n\n");

        // #[pymethods] impl
        out.push_str("#[pymethods]\n");
        out.push_str(&format!("impl {} {{\n", ty.name));
        out.push_str("    #[new]\n");
        let params: Vec<String> = ty.fields.iter()
            .map(|(n, t)| format!("{}: {}", n, self.map_to_pyo3_type(t)))
            .collect();
        let field_inits: Vec<String> = ty.fields.iter()
            .map(|(n, _)| n.clone())
            .collect();
        out.push_str(&format!("    pub fn new({}) -> Self {{\n", params.join(", ")));
        out.push_str(&format!("        {} {{ {} }}\n", ty.name, field_inits.join(", ")));
        out.push_str("    }\n");
        out.push_str("}\n");

        out
    }

    /// 生成 #[pyfunction]
    fn generate_pyfunction(&self, func: &PyExport) -> String {
        let mut out = String::new();
        if !func.doc.is_empty() {
            out.push_str(&format!("#[doc = \"{}\"]\n", func.doc));
        }
        out.push_str("#[pyfunction]\n");

        let params: Vec<String> = func.args.iter()
            .map(|(n, t)| format!("{}: {}", n, self.lz_to_rust(t)))
            .collect();
        let ret = self.lz_to_rust(&func.ret);

        out.push_str(&format!("pub fn {}({}) -> {} {{\n", func.name, params.join(", "), ret));
        out.push_str(&format!("    // TODO: delegate to lz function __py_{}\n", func.name));
        out.push_str(&format!("    todo!(\"Implement __py_{}\")\n", func.name));
        out.push_str("}\n");

        out
    }

    /// 生成 #[pymodule]
    fn generate_pymodule(&self) -> String {
        let mut out = String::new();
        out.push_str("#[pymodule]\n");
        out.push_str(&format!("fn {}_py(_py: Python, m: &PyModule) -> PyResult<()> {{\n", self.module.name));

        for name in self.functions.keys() {
            out.push_str(&format!("    m.add_function(wrap_pyfunction!({}, m)?)?;\n", name));
        }
        for name in self.types.keys() {
            out.push_str(&format!("    m.add_class::<{}>()?;\n", name));
        }

        out.push_str("    Ok(())\n");
        out.push_str("}\n");
        out
    }

    /// lz 类型 → Rust 类型映射
    fn lz_to_rust(&self, lz_type: &str) -> String {
        match lz_type {
            "int" | "i32" | "i64" => "i64".into(),
            "f64" | "float" => "f64".into(),
            "str" | "String" => "String".into(),
            "bool" => "bool".into(),
            "None" | "void" => "()".into(),
            "List" | "Vec" => "Vec<PyObject>".into(),
            "Dict" | "HashMap" => "PyObject".into(),
            "Any" | "object" => "PyObject".into(),
            // 自定义类型保持原名（由 #[pyclass] 定义）
            other => other.to_string(),
        }
    }

    /// 字段类型 → pyo3 兼容的 Rust 类型
    ///
    /// 之前该函数忽略字段的 lz 类型、恒返回 `PyObject`，导致 `#[pyclass]`
    /// 字段丢失类型安全（如 `Point.x: f64` 被错误降级为 `PyObject`）。
    /// 现改为按字段 lz 类型映射为对应的 Rust 类型。
    fn map_to_pyo3_type(&self, lz_type: &str) -> String {
        self.lz_to_rust(lz_type)
    }
}

// ──────────────── Bridge trait ────────────────

impl Bridge for PyO3Bridge {
    fn name(&self) -> &str { "pyo3" }

    fn level(&self) -> BridgeLevel { BridgeLevel::LinkTime }

    fn capabilities(&self) -> BridgeCapability {
        BridgeCapability::FUNCTION_CALL | BridgeCapability::TYPE_REWRITE
    }

    fn resolve_call(&self, func_name: &str, _args: &[String]) -> Option<CallResolveResult> {
        if self.functions.contains_key(func_name) {
            Some(CallResolveResult {
                rust_path: format!("__py_{}", func_name),
                shim: String::new(),
                module_name: "pyo3".into(),
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
        BridgeMeta {
            version: self.module.version.clone(),
            description: format!("PyO3 bridge: {} ({} functions, {} types)",
                self.module.description, self.functions.len(), self.types.len()),
            provides: vec!["pyo3".into(), "python".into()],
            ..Default::default()
        }
    }

    fn list_exports(&self, kind: ExportKind) -> Vec<ExportEntry> {
        match kind {
            ExportKind::Function => {
                self.functions.values().map(|f| ExportEntry {
                    name: f.name.clone(),
                    kind: ExportKind::Function,
                    signature: format!("def {}({}) -> {}", f.name,
                        f.args.iter().map(|(n, t)| format!("{}: {}", n, t)).collect::<Vec<_>>().join(", "),
                        f.ret),
                    module: self.module.name.clone(),
                }).collect()
            }
            ExportKind::Type => {
                self.types.values().map(|t| ExportEntry {
                    name: t.name.clone(),
                    kind: ExportKind::Type,
                    signature: format!("class {}: {}", t.name,
                        t.fields.iter().map(|(n, ty)| format!("{}: {}", n, ty)).collect::<Vec<_>>().join(", ")),
                    module: self.module.name.clone(),
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
[bridge]
name = "testmod"
version = "0.1.0"
description = "Test python bindings"

[functions]
greet = { args = "name: str", ret = "str", doc = "Greet someone" }
add = { args = "a: int, b: int", ret = "int" }

[types]
Point = { fields = "x: f64, y: f64", doc = "A 2D point" }
"#;
        dir.create_file("pyo3.toml", content).unwrap()
    }

    #[test]
    fn test_pyo3_bridge_load() {
        let dir = TempDir::new("lz-pyo3").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = PyO3Bridge::load(&path).unwrap();

        assert_eq!(bridge.name(), "pyo3");
        assert_eq!(bridge.level(), BridgeLevel::LinkTime);
        assert!(bridge.capabilities().contains(BridgeCapability::FUNCTION_CALL));
        assert_eq!(bridge.functions.len(), 2);
        assert_eq!(bridge.types.len(), 1);
    }

    #[test]
    fn test_pyo3_bridge_gen_call() {
        let dir = TempDir::new("lz-pyo3").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = PyO3Bridge::load(&path).unwrap();

        assert_eq!(bridge.resolve_call("greet", &[]).map(|r| r.rust_path), Some("__py_greet".to_string()));
        assert_eq!(bridge.resolve_call("add", &[]).map(|r| r.rust_path), Some("__py_add".to_string()));
        assert!(bridge.resolve_call("nonexistent", &[]).is_none());
    }

    #[test]
    fn test_pyo3_module_generation() {
        let dir = TempDir::new("lz-pyo3").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = PyO3Bridge::load(&path).unwrap();

        let code = bridge.generate_module();
        assert!(code.contains("#[pymodule]"));
        assert!(code.contains("fn testmod_py"));
        assert!(code.contains("#[pyfunction]"));
        assert!(code.contains("fn greet"));
        assert!(code.contains("#[pyclass]"));
        assert!(code.contains("struct Point"));
        // 字段必须按 lz 类型映射为 Rust 类型，而非退化为 PyObject
        assert!(code.contains("pub x: f64"));
        assert!(code.contains("pub y: f64"));
        assert!(!code.contains("pub x: PyObject"));
        assert!(!code.contains("pub y: PyObject"));
    }

    #[test]
    fn test_pyo3_list_exports() {
        let dir = TempDir::new("lz-pyo3").unwrap();
        let path = create_test_manifest(&dir);
        let bridge = PyO3Bridge::load(&path).unwrap();

        let funcs = bridge.list_exports(ExportKind::Function);
        assert_eq!(funcs.len(), 2);
        assert!(funcs.iter().any(|e| e.name == "greet"));

        let types = bridge.list_exports(ExportKind::Type);
        assert_eq!(types.len(), 1);
        assert!(types.iter().any(|e| e.name == "Point"));
    }
}

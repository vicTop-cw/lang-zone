// Lang-Zong 编译器 — bridge.rs
// 标准库桥接层：TOML 清单 → 内存符号表
// 源码级映射：lz std 符号 → Rust std 路径
// 对齐 workbuddy/plan/std-bridge-plan.md
// 使用内嵌 mini_toml 解析器，零外部依赖

use crate::util::parse;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ──────────────── TOML 清单数据结构 ────────────────

/// 顶层清单（bridge.toml）
#[derive(Debug)]
pub struct BridgeManifest {
    pub modules: HashMap<String, ModuleEntry>,
    pub type_aliases: HashMap<String, String>,
    pub tier1_channel: String,
    pub tier2_channel: String,
    pub tier2_enabled_by_default: bool,
}

#[derive(Debug)]
pub struct ModuleEntry {
    pub tier: u8,
}

/// 模块清单（modules/*.toml）
#[derive(Debug)]
pub struct ModuleManifest {
    pub tier: u8,
    pub rust_prefix: String,
    pub types: HashMap<String, String>,
    pub functions: HashMap<String, FuncEntry>,
    pub methods: HashMap<String, String>,
    pub aliases: HashMap<String, String>,
}

#[derive(Debug)]
pub struct FuncEntry {
    pub rust: String,
    pub shim: String,
}

// ──────────────── StdBridge ────────────────

/// 标准库桥接核心：加载 TOML 成内存符号表，提供 resolve 系列方法
#[derive(Debug)]
pub struct StdBridge {
    /// 顶层清单
    top: BridgeManifest,
    /// 模块清单（模块名 → ModuleManifest）
    module_mans: HashMap<String, ModuleManifest>,
    /// Tier-2 清单
    tier2_crates: HashMap<String, Tier2CrateEntry>,
    tier2_nightly_required: String,
    /// 运行时状态
    tier2_allowed: bool,
    rustc_version: String,
    /// 导入本模块时需追加的类型别名（模块名 → Vec<(alias_name, rust_type)>）
    import_aliases: HashMap<String, Vec<(String, String)>>,
    /// 导入本模块时需注入的 shim 函数名集合（模块名 → Vec<shim_name>）
    required_shims: HashMap<String, Vec<String>>,
    /// 三方 crate 注册表（crate 名 → CrateEntry）
    known_crates: HashMap<String, CrateEntry>,
    /// 代码生成期间收集的 crate 依赖（用于 Cargo.toml 提示）
    pub used_crates: std::cell::RefCell<Vec<(String, String, Vec<String>)>>,
}

#[derive(Debug)]
pub struct Tier2CrateEntry {
    pub extern_name: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct CrateEntry {
    pub version: String,
    pub features: Vec<String>,
    pub description: String,
}

impl StdBridge {
    /// 加载 std/ 目录下的所有 TOML 清单，构造 StdBridge
    pub fn load(std_dir: &Path) -> Result<Self, String> {
        let bridge_path = std_dir.join("bridge.toml");
        let top = Self::load_top_manifest(&bridge_path)?;

        // 加载各模块清单
        let modules_dir = std_dir.join("modules");
        let mut module_mans = HashMap::new();
        if modules_dir.exists() {
            for entry in fs::read_dir(&modules_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    if !name.is_empty() && top.modules.contains_key(&name) {
                        let man = Self::load_module_manifest(&path)?;
                        module_mans.insert(name, man);
                    }
                }
            }
        }

        // 加载 Tier-2 清单
        let tier2_path = std_dir.join("rustc_private.toml");
        let (tier2_crates, tier2_nightly) = Self::load_tier2_manifest(&tier2_path)?;

        // 扁平化：收集各模块导入时需追加的类型别名
        let mut import_aliases = HashMap::new();
        for (mod_name, man) in &module_mans {
            let aliases: Vec<(String, String)> = man.aliases.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if !aliases.is_empty() {
                import_aliases.insert(mod_name.clone(), aliases);
            }
        }
        // 顶层 type_aliases 追加到 core 模块
        if !top.type_aliases.is_empty() {
            let core_aliases = import_aliases.entry("core".to_string()).or_default();
            for (k, v) in &top.type_aliases {
                core_aliases.push((k.clone(), v.clone()));
            }
        }

        // 收集各模块所需 shim
        let mut required_shims = HashMap::new();
        for (mod_name, man) in &module_mans {
            let shims: Vec<String> = man.functions.iter()
                .filter_map(|(_, f)| if f.shim.is_empty() { None } else { Some(f.shim.clone()) })
                .collect();
            if !shims.is_empty() {
                required_shims.insert(mod_name.clone(), shims);
            }
        }

        // 加载三方 crate 清单
        let crates_path = std_dir.join("crates.toml");
        let known_crates = Self::load_crates_manifest(&crates_path)?;

        Ok(StdBridge {
            top,
            module_mans,
            tier2_crates,
            tier2_nightly_required: tier2_nightly,
            tier2_allowed: false,
            rustc_version: String::new(),
            import_aliases,
            required_shims,
            known_crates,
            used_crates: std::cell::RefCell::new(Vec::new()),
        })
    }

    /// 设置运行时选项
    pub fn set_tier2_allowed(&mut self, allowed: bool) {
        self.tier2_allowed = allowed;
    }

    pub fn set_rustc_version(&mut self, version: String) {
        self.rustc_version = version;
    }

    // ─── resolve_import ───
    pub fn resolve_import(&self, lz_path: &[String], _items: &[String]) -> ImportResolveResult {
        if lz_path.is_empty() {
            return ImportResolveResult {
                rust_path: String::new(),
                type_aliases: vec![],
                requires_shim: false,
                is_tier2: false,
                feature_flags: vec![],
                extern_crates: vec![],
                error: None,
            };
        }

        // 非 std 导入 — 检查是否已知三方 crate
        if lz_path[0] != "std" {
            let crate_name = &lz_path[0];
            if let Some(entry) = self.known_crates.get(crate_name) {
                // 记录 crate 依赖（供 Cargo.toml 提示）
                self.used_crates.borrow_mut().push((
                    crate_name.clone(),
                    entry.version.clone(),
                    entry.features.clone(),
                ));
                // 生成 use 路径（lz 中的 crate::module → Rust use crate::module）
                return ImportResolveResult {
                    rust_path: lz_path.join("::"),
                    type_aliases: vec![],
                    requires_shim: false,
                    is_tier2: false,
                    feature_flags: vec![],
                    extern_crates: vec![],
                    error: None,
                };
            }
            // 未知的非 std 导入 — 身份透传
            return ImportResolveResult {
                rust_path: lz_path.join("::"),
                type_aliases: vec![],
                requires_shim: false,
                is_tier2: false,
                feature_flags: vec![],
                extern_crates: vec![],
                error: None,
            };
        }

        let module_name = if lz_path.len() >= 2 { &lz_path[1] } else { "" };

        // Tier-2 检查
        if module_name.starts_with("rustc_") {
            if !self.tier2_allowed {
                return ImportResolveResult {
                    rust_path: lz_path.join("::"),
                    type_aliases: vec![],
                    requires_shim: false,
                    is_tier2: true,
                    feature_flags: vec!["rustc_private".to_string()],
                    extern_crates: vec![],
                    error: Some("[lang-zone] Tier-2 rustc_private 模块需要 --allow-rustc-private 标志".to_string()),
                };
            }
            let crate_entry = self.tier2_crates.get(module_name);
            if let Some(entry) = crate_entry {
                return ImportResolveResult {
                    rust_path: entry.path.clone(),
                    type_aliases: vec![],
                    requires_shim: false,
                    is_tier2: true,
                    feature_flags: vec!["rustc_private".to_string()],
                    extern_crates: vec![entry.extern_name.clone()],
                    error: None,
                };
            }
        }

        // Tier-1 std 模块查找
        if let Some(man) = self.module_mans.get(module_name) {
            let rust_rest = if lz_path.len() > 2 {
                lz_path[2..].join("::")
            } else {
                String::new()
            };
            let rust_path = if rust_rest.is_empty() {
                man.rust_prefix.clone()
            } else {
                format!("{}::{}", man.rust_prefix, rust_rest)
            };

            let type_aliases = self.import_aliases.get(module_name)
                .cloned()
                .unwrap_or_default();
            let requires_shim = self.required_shims.contains_key(module_name);

            ImportResolveResult {
                rust_path,
                type_aliases,
                requires_shim,
                is_tier2: false,
                feature_flags: vec![],
                extern_crates: vec![],
                error: None,
            }
        } else {
            ImportResolveResult {
                rust_path: lz_path.join("::"),
                type_aliases: vec![],
                requires_shim: false,
                is_tier2: false,
                feature_flags: vec![],
                extern_crates: vec![],
                error: None,
            }
        }
    }

    // ─── resolve_call ───
    pub fn resolve_call(&self, func_name: &str) -> Option<CallResolveResult> {
        for (mod_name, man) in &self.module_mans {
            if let Some(func_entry) = man.functions.get(func_name) {
                let is_template = func_entry.rust.contains("{0}")
                    || func_entry.rust.contains("{1}")
                    || func_entry.rust.contains("{2}");
                return Some(CallResolveResult {
                    rust_path: func_entry.rust.clone(),
                    shim: func_entry.shim.clone(),
                    module_name: mod_name.clone(),
                    is_macro: func_entry.rust.ends_with("!"),
                    is_template,
                });
            }
        }
        None
    }

    // ─── resolve_method ───
    pub fn resolve_method(&self, method: &str, receiver_type: &str) -> MethodResolveResult {
        let modules_to_check = self.modules_for_type(receiver_type);

        for mod_name in modules_to_check {
            if let Some(man) = self.module_mans.get(&mod_name) {
                if let Some(rust_method) = man.methods.get(method) {
                    if rust_method.is_empty() {
                        return MethodResolveResult::identity(method);
                    }
                    return MethodResolveResult::mapped(method, rust_method);
                }
            }
        }

        for (_, man) in &self.module_mans {
            if let Some(rust_method) = man.methods.get(method) {
                if rust_method.is_empty() {
                    return MethodResolveResult::identity(method);
                }
                return MethodResolveResult::mapped(method, rust_method);
            }
        }

        MethodResolveResult::identity(method)
    }

    /// 在 map_type 开头调用：桥接表补充的类型重写
    /// 仅返回 map_type 未覆盖的类型（如 Never→!, IOError→std::io::Error）
    /// map_type 已覆盖的类型（int/str/float/bool/List/Dict/Option/Result/&T）不在桥接表中
    pub fn rewrite_type(&self, lz_type: &str) -> Option<String> {
        // 仅查顶层 type_aliases（专门为 map_type 补充的类型）
        if let Some(rust_type) = self.top.type_aliases.get(lz_type) {
            return Some(rust_type.clone());
        }
        // 再查各模块 aliases（导入时追加的类型别名）
        for (_, man) in &self.module_mans {
            if let Some(rust_type) = man.aliases.get(lz_type) {
                return Some(rust_type.clone());
            }
        }
        // 再查各模块 types（仅对 map_type 未覆盖的类型生效）
        for (_, man) in &self.module_mans {
            if let Some(rust_type) = man.types.get(lz_type) {
                // 安全检查：如果映射结果与输入相同，说明是身份映射，跳过
                // （避免覆盖 map_type 的正确映射）
                if rust_type == lz_type {
                    continue;
                }
                return Some(rust_type.clone());
            }
        }
        None
    }

    // ─── tier2_allowed ───
    pub fn tier2_allowed(&self) -> Tier2CheckResult {
        if !self.tier2_allowed {
            return Tier2CheckResult::DeniedFlag;
        }
        let required = &self.tier2_nightly_required;
        if self.rustc_version.contains(required.replace("nightly-", "").as_str())
            || self.rustc_version.is_empty()
        {
            Tier2CheckResult::Allowed
        } else {
            Tier2CheckResult::VersionMismatch {
                required: required.clone(),
                actual: self.rustc_version.clone(),
            }
        }
    }

    // ─── shims_required ───
    pub fn shims_required(&self, module_name: &str) -> Vec<String> {
        self.required_shims.get(module_name)
            .cloned()
            .unwrap_or_default()
    }

    /// 列出指定类型的所有导出符号（用于 bridge introspection）
    pub fn list_exports(&self, kind: crate::bridge::core::ExportKind) -> Vec<crate::bridge::core::ExportEntry> {
        use crate::bridge::core::{ExportEntry, ExportKind};
        let mut entries = Vec::new();

        match kind {
            ExportKind::Function => {
                for (mod_name, man) in &self.module_mans {
                    for (fn_name, entry) in &man.functions {
                        entries.push(ExportEntry {
                            name: fn_name.clone(),
                            kind: ExportKind::Function,
                            signature: entry.rust.clone(),
                            module: mod_name.clone(),
                        });
                    }
                }
            }
            ExportKind::Type => {
                for (mod_name, man) in &self.module_mans {
                    for (lz_type, rust_type) in &man.types {
                        entries.push(ExportEntry {
                            name: lz_type.clone(),
                            kind: ExportKind::Type,
                            signature: rust_type.clone(),
                            module: mod_name.clone(),
                        });
                    }
                }
            }
            ExportKind::Method => {
                for (mod_name, man) in &self.module_mans {
                    for (lz_method, rust_method) in &man.methods {
                        entries.push(ExportEntry {
                            name: lz_method.clone(),
                            kind: ExportKind::Method,
                            signature: rust_method.clone(),
                            module: mod_name.clone(),
                        });
                    }
                }
            }
            ExportKind::Module => {
                for mod_name in self.module_mans.keys() {
                    entries.push(ExportEntry {
                        name: mod_name.clone(),
                        kind: ExportKind::Module,
                        signature: format!("std::{}", mod_name),
                        module: String::new(),
                    });
                }
            }
            ExportKind::Constant => {
                // StdBridge 暂不支持常量导出
            }
        }
        entries
    }

    /// 导出符号总数
    pub fn export_count(&self) -> usize {
        self.module_mans.values()
            .map(|m| m.functions.len() + m.types.len() + m.methods.len())
            .sum::<usize>() + self.module_mans.len()
    }

    // ─── 内部辅助 ───

    fn modules_for_type(&self, type_name: &str) -> Vec<String> {
        let mut result = vec![];
        for (mod_name, man) in &self.module_mans {
            if man.types.contains_key(type_name) {
                result.push(mod_name.clone());
            }
        }
        result
    }

    fn load_top_manifest(path: &Path) -> Result<BridgeManifest, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("读取 {}: {}", path.display(), e))?;
        let doc = parse(&content)
            .map_err(|e| format!("解析 {}: {}", path.display(), e))?;

        let _meta = doc.get("meta").ok_or("bridge.toml 缺少 [meta]")?;
        let toolchain = doc.get("toolchain").ok_or("bridge.toml 缺少 [toolchain]")?;
        let modules = doc.get("modules").ok_or("bridge.toml 缺少 [modules]")?;

        let tier1_channel = toolchain.get("tier1_channel")
            .and_then(|v| v.as_str()).unwrap_or("stable").to_string();
        let tier2_channel = toolchain.get("tier2_channel")
            .and_then(|v| v.as_str()).unwrap_or("nightly").to_string();

        let tier2_enabled_by_default = doc.get("tier2_gate")
            .and_then(|t| t.get("enabled_by_default"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut module_entries = HashMap::new();
        for (name, val) in modules {
            let tier = val.as_table()
                .and_then(|t| t.get("tier"))
                .and_then(|v| v.as_int())
                .unwrap_or(1) as u8;
            module_entries.insert(name.clone(), ModuleEntry { tier });
        }

        let mut type_aliases = HashMap::new();
        if let Some(aliases) = doc.get("type_aliases") {
            for (k, v) in aliases {
                if let Some(s) = v.as_str() {
                    type_aliases.insert(k.clone(), s.to_string());
                }
            }
        }

        Ok(BridgeManifest {
            modules: module_entries,
            type_aliases,
            tier1_channel,
            tier2_channel,
            tier2_enabled_by_default,
        })
    }

    fn load_module_manifest(path: &Path) -> Result<ModuleManifest, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("读取 {}: {}", path.display(), e))?;
        let doc = parse(&content)
            .map_err(|e| format!("解析 {}: {}", path.display(), e))?;

        let module_section = doc.get("module")
            .ok_or_else(|| format!("{} 缺少 [module]", path.display()))?;
        let tier = module_section.get("tier")
            .and_then(|v| v.as_int()).unwrap_or(1) as u8;
        let rust_prefix = module_section.get("rust_prefix")
            .and_then(|v| v.as_str()).unwrap_or("").to_string();

        // types
        let mut types = HashMap::new();
        if let Some(types_section) = doc.get("types") {
            for (k, v) in types_section {
                if let Some(s) = v.as_str() {
                    types.insert(k.clone(), s.to_string());
                }
            }
        }

        // functions
        let mut functions = HashMap::new();
        if let Some(funcs_section) = doc.get("functions") {
            for (name, val) in funcs_section {
                if let Some(table) = val.as_table() {
                    let rust = table.get("rust").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let shim = table.get("shim").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    functions.insert(name.clone(), FuncEntry { rust, shim });
                }
            }
        }

        // methods
        let mut methods = HashMap::new();
        if let Some(methods_section) = doc.get("methods") {
            for (k, v) in methods_section {
                if let Some(s) = v.as_str() {
                    methods.insert(k.clone(), s.to_string());
                }
            }
        }

        // aliases
        let mut aliases = HashMap::new();
        if let Some(aliases_section) = doc.get("aliases") {
            for (k, v) in aliases_section {
                if let Some(s) = v.as_str() {
                    aliases.insert(k.clone(), s.to_string());
                }
            }
        }

        Ok(ModuleManifest {
            tier,
            rust_prefix,
            types,
            functions,
            methods,
            aliases,
        })
    }

    fn load_tier2_manifest(path: &Path) -> Result<(HashMap<String, Tier2CrateEntry>, String), String> {
        if !path.exists() {
            return Ok((HashMap::new(), String::new()));
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("读取 {}: {}", path.display(), e))?;
        let doc = parse(&content)
            .map_err(|e| format!("解析 {}: {}", path.display(), e))?;

        let meta = doc.get("meta")
            .ok_or_else(|| format!("{} 缺少 [meta]", path.display()))?;
        let nightly_required = meta.get("nightly_required")
            .and_then(|v| v.as_str()).unwrap_or("nightly").to_string();

        let mut crates = HashMap::new();
        if let Some(crates_section) = doc.get("crates") {
            for (name, val) in crates_section {
                if let Some(table) = val.as_table() {
                    let extern_name = table.get("extern").and_then(|v| v.as_str())
                        .unwrap_or(name).to_string();
                    let path_str = table.get("path").and_then(|v| v.as_str())
                        .unwrap_or(name).to_string();
                    crates.insert(name.clone(), Tier2CrateEntry {
                        extern_name,
                        path: path_str,
                    });
                }
            }
        }

        Ok((crates, nightly_required))
    }

    /// 回退构造：无 std/ 目录时提供空桥接（所有 resolve 均身份透传）
    pub fn load_fallback() -> StdBridge {
        StdBridge {
            top: BridgeManifest {
                modules: HashMap::new(),
                type_aliases: HashMap::new(),
                tier1_channel: "stable".to_string(),
                tier2_channel: "nightly".to_string(),
                tier2_enabled_by_default: false,
            },
            module_mans: HashMap::new(),
            tier2_crates: HashMap::new(),
            tier2_nightly_required: String::new(),
            tier2_allowed: false,
            rustc_version: String::new(),
            import_aliases: HashMap::new(),
            required_shims: HashMap::new(),
            known_crates: HashMap::new(),
            used_crates: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn load_crates_manifest(path: &Path) -> Result<HashMap<String, CrateEntry>, String> {
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("读取 {}: {}", path.display(), e))?;
        let doc = parse(&content)
            .map_err(|e| format!("解析 {}: {}", path.display(), e))?;

        let mut crates = HashMap::new();
        if let Some(crates_section) = doc.get("crates") {
            for (name, val) in crates_section {
                if let Some(table) = val.as_table() {
                    let version = table.get("version")
                        .and_then(|v| v.as_str()).unwrap_or("*").to_string();
                    let features: Vec<String> = table.get("features")
                        .and_then(|v| v.as_str())
                        .map(|s| s.split(',').map(|f| f.trim().to_string()).collect())
                        .unwrap_or_default();
                    let description = table.get("description")
                        .and_then(|v| v.as_str()).unwrap_or("").to_string();
                    crates.insert(name.clone(), CrateEntry { version, features, description });
                }
            }
        }
        Ok(crates)
    }
}

// ──────────────── resolve 返回类型 ────────────────

// ImportResolveResult 定义移至 bridge_core.rs（统一桥接层公共类型）
pub use crate::bridge::core::{ImportResolveResult, CallResolveResult, MethodResolveResult};

#[derive(Debug)]
pub enum Tier2CheckResult {
    Allowed,
    DeniedFlag,
    VersionMismatch { required: String, actual: String },
}

// ──────────────── 单元测试 ────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn std_dir() -> PathBuf {
        PathBuf::from("std")
    }

    // ─── Load ───

    #[test]
    fn test_load_bridge_success() {
        let result = StdBridge::load(&std_dir());
        assert!(result.is_ok(), "Load std/ should succeed: {:?}", result.err());
    }

    #[test]
    fn test_load_bridge_has_modules() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // 至少 P0 模块应该加载
        assert!(bridge.module_mans.contains_key("core"));
        assert!(bridge.module_mans.contains_key("io"));
        assert!(bridge.module_mans.contains_key("fs"));
        assert!(bridge.module_mans.contains_key("collections"));
    }

    #[test]
    fn test_load_bridge_has_type_aliases() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        assert!(bridge.top.type_aliases.contains_key("IOError"));
        assert!(bridge.top.type_aliases.contains_key("Never"));
    }

    #[test]
    fn test_load_fallback_empty() {
        let fb = StdBridge::load_fallback();
        assert!(fb.module_mans.is_empty());
        assert!(fb.top.modules.is_empty());
    }

    // ─── resolve_import ───

    #[test]
    fn test_resolve_import_std_io() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(&["std".into(), "io".into()], &[]);
        assert!(!result.is_tier2);
        assert!(result.error.is_none());
        assert_eq!(result.rust_path, "std::io");
    }

    #[test]
    fn test_resolve_import_std_fs() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(&["std".into(), "fs".into()], &[]);
        assert_eq!(result.rust_path, "std::fs");
        // fs 模块函数有 path_ref shim，因此 requires_shim 为 true
        assert!(result.requires_shim);
    }

    #[test]
    fn test_resolve_import_std_collections() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(&["std".into(), "collections".into()], &[]);
        assert!(result.type_aliases.is_empty()); // collections 无 aliases
    }

    #[test]
    fn test_resolve_import_with_items() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(
            &["std".into(), "collections".into()],
            &["HashMap".into(), "HashSet".into()]
        );
        assert_eq!(result.rust_path, "std::collections");
    }

    #[test]
    fn test_resolve_import_non_std_identity() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(&["my".into(), "module".into()], &[]);
        assert_eq!(result.rust_path, "my::module"); // 身份透传
        assert!(!result.is_tier2);
    }

    #[test]
    fn test_resolve_import_unknown_std_module_identity() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(&["std".into(), "nonexistent".into()], &[]);
        assert_eq!(result.rust_path, "std::nonexistent"); // 身份透传
    }

    #[test]
    fn test_resolve_import_io_with_aliases() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(&["std".into(), "io".into()], &[]);
        // io 模块应注入 IOError 别名
        let has_ioerror = result.type_aliases.iter().any(|(a, _)| a == "IOError");
        assert!(has_ioerror, "IO import should inject IOError alias");
    }

    #[test]
    fn test_resolve_import_fallback_non_std() {
        let fb = StdBridge::load_fallback();
        let result = fb.resolve_import(&["my".into(), "module".into()], &[]);
        assert_eq!(result.rust_path, "my::module");
    }

    // ─── Tier-2 ───

    #[test]
    fn test_resolve_import_tier2_denied_by_default() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(&["std".into(), "rustc_middle".into()], &[]);
        assert!(result.is_tier2);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_resolve_import_tier2_allowed_with_flag() {
        let mut bridge = StdBridge::load(&std_dir()).unwrap();
        bridge.set_tier2_allowed(true);
        let result = bridge.resolve_import(&["std".into(), "rustc_middle".into()], &[]);
        assert!(result.is_tier2);
        assert!(result.error.is_none());
        assert!(result.extern_crates.contains(&"rustc_middle".to_string()));
    }

    #[test]
    fn test_tier2_allowed_denied_by_flag() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        match bridge.tier2_allowed() {
            Tier2CheckResult::DeniedFlag => {} // expected
            other => panic!("Expected DeniedFlag, got {:?}", other),
        }
    }

    #[test]
    fn test_tier2_allowed_with_flag_no_version() {
        let mut bridge = StdBridge::load(&std_dir()).unwrap();
        bridge.set_tier2_allowed(true);
        // 无 rustc_version → 放行
        match bridge.tier2_allowed() {
            Tier2CheckResult::Allowed => {}
            other => panic!("Expected Allowed when no version, got {:?}", other),
        }
    }

    #[test]
    fn test_tier2_version_mismatch() {
        let mut bridge = StdBridge::load(&std_dir()).unwrap();
        bridge.set_tier2_allowed(true);
        bridge.set_rustc_version("rustc 1.50.0 (old)".to_string());
        match bridge.tier2_allowed() {
            Tier2CheckResult::VersionMismatch { .. } => {}
            other => panic!("Expected VersionMismatch, got {:?}", other),
        }
    }

    // ─── resolve_method ───

    #[test]
    fn test_resolve_method_push_alias() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_method("append", "");
        assert!(result.rewritten);
        assert_eq!(result.rust_method, "push");
    }

    #[test]
    fn test_resolve_method_len_alias() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_method("length", "");
        assert!(result.rewritten);
        assert_eq!(result.rust_method, "len");
    }

    #[test]
    fn test_resolve_method_is_empty_alias() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_method("isEmpty", "");
        assert!(result.rewritten);
        assert_eq!(result.rust_method, "is_empty");
    }

    #[test]
    fn test_resolve_method_starts_with_alias() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_method("startsWith", "");
        assert!(result.rewritten);
        assert_eq!(result.rust_method, "starts_with");
    }

    #[test]
    fn test_resolve_method_unknown_identity() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_method("nonExistentMethod", "");
        assert!(!result.rewritten);
        assert_eq!(result.rust_method, "nonExistentMethod");
    }

    #[test]
    fn test_resolve_method_fallback_always_identity() {
        let fb = StdBridge::load_fallback();
        let result = fb.resolve_method("append", "");
        assert!(!result.rewritten);
        assert_eq!(result.rust_method, "append");
    }

    // ─── rewrite_type ───

    #[test]
    fn test_rewrite_type_never() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        assert_eq!(bridge.rewrite_type("Never"), Some("!".to_string()));
    }

    #[test]
    fn test_rewrite_type_io_error() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        assert_eq!(bridge.rewrite_type("IOError"), Some("std::io::Error".to_string()));
    }

    #[test]
    fn test_rewrite_type_unknown() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        assert_eq!(bridge.rewrite_type("NonExistentType"), None);
    }

    #[test]
    fn test_rewrite_type_str_not_overwritten() {
        // str 不应被桥接表覆盖（map_type 已处理 str→String）
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // rewrite_type 对 "str" 应返回 None（不在 type_aliases 中）
        assert_eq!(bridge.rewrite_type("str"), None);
    }

    #[test]
    fn test_rewrite_type_fallback_none() {
        let fb = StdBridge::load_fallback();
        assert_eq!(fb.rewrite_type("Never"), None);
        assert_eq!(fb.rewrite_type("Anything"), None);
    }

    // ─── resolve_call ───

    #[test]
    fn test_resolve_call_in_core() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_call("panic");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rust_path, "panic!");
        assert!(r.is_macro);
    }

    #[test]
    fn test_resolve_call_unknown() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        assert!(bridge.resolve_call("nonExistentFunc").is_none());
    }

    #[test]
    fn test_resolve_call_fallback_none() {
        let fb = StdBridge::load_fallback();
        assert!(fb.resolve_call("panic").is_none());
    }

    // ─── shims_required ───

    #[test]
    fn test_shims_required_fs_has_shims() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let shims = bridge.shims_required("fs");
        // fs 模块有 path_ref shim
        assert!(!shims.is_empty());
        assert!(shims.contains(&"path_ref".to_string()));
    }

    #[test]
    fn test_shims_required_core_has_shims() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let shims = bridge.shims_required("core");
        // core 模块有 panic 的 fmt shim 和 print 的 auto_fmt shim
        assert!(!shims.is_empty());
    }

    #[test]
    fn test_shims_required_fallback_empty() {
        let fb = StdBridge::load_fallback();
        assert!(fb.shims_required("anything").is_empty());
    }

    // ─── 时间模块方法别名 ───

    #[test]
    fn test_resolve_method_time_duration() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_method("fromMillis", "Duration");
        assert!(result.rewritten);
        assert_eq!(result.rust_method, "from_millis");
    }

    // ─── 方法别名全局搜索 ───

    #[test]
    fn test_resolve_method_global_fallback() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // "trim" 方法在 str.toml 中是身份映射 "trim" → "trim"
        let result = bridge.resolve_method("trim", "String");
        assert!(result.rewritten);
        assert_eq!(result.rust_method, "trim");
    }

    // ─── 边界/冲突/错误恢复测试 ───

    #[test]
    fn test_all_24_modules_load() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let expected = vec![
            "core", "collections", "io", "fs", "thread", "fmt", "str", "vec",
            "time", "path", "env", "process", "sync", "iter", "num", "net",
            "mem", "cmp", "cell", "rc", "convert", "any", "marker", "hash", "os",
        ];
        for name in expected {
            assert!(bridge.module_mans.contains_key(name),
                "Missing module: {}", name);
        }
    }

    #[test]
    fn test_contains_method_unified() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // contains 应统一映射为 "contains"（所有模块一致）
        let r1 = bridge.resolve_method("contains", "Vec");
        let r2 = bridge.resolve_method("contains", "String");
        // 两个都应返回 "contains"（不会再出现 contains_key）
        assert_eq!(r1.rust_method, "contains");
        assert_eq!(r2.rust_method, "contains");
    }

    #[test]
    fn test_map_filter_unified() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // map/filter 应在所有模块中一致映射
        let r_map_vec = bridge.resolve_method("map", "Vec");
        let r_filter_vec = bridge.resolve_method("filter", "Vec");
        assert_eq!(r_map_vec.rust_method, "map");
        assert_eq!(r_filter_vec.rust_method, "filter");
    }

    #[test]
    fn test_no_weak_type_conflict() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // rc::Weak → "std::rc::Weak"
        let rc_weak = bridge.rewrite_type("Weak");
        // sync::Weak 已改为 SyncWeak，不应与 rc::Weak 冲突
        let sync_weak = bridge.rewrite_type("SyncWeak");
        // 两者应为不同类型（或至少不互相覆盖）
        assert_ne!(rc_weak, sync_weak);
    }

    #[test]
    fn test_resolve_import_empty_path() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(&[], &[]);
        assert_eq!(result.rust_path, "");
        assert!(!result.is_tier2);
    }

    #[test]
    fn test_resolve_import_deeply_nested() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(
            &["std".into(), "collections".into(), "hash_map".into(), "Entry".into()],
            &[]
        );
        assert_eq!(result.rust_path, "std::collections::hash_map::Entry");
    }

    #[test]
    fn test_resolve_import_single_element_std() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(&["std".into()], &[]);
        assert_eq!(result.rust_path, "std"); // 身份透传
    }

    #[test]
    fn test_resolve_import_with_kebab_case_module() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // 连字符模块名应身份透传
        let result = bridge.resolve_import(&["my-lib".into(), "module".into()], &[]);
        assert_eq!(result.rust_path, "my-lib::module");
    }

    #[test]
    fn test_resolve_method_empty_string() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_method("", "");
        assert!(!result.rewritten);
        assert_eq!(result.rust_method, "");
    }

    #[test]
    fn test_resolve_method_case_sensitive() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // "Length" (大写 L) 不应匹配 "length" 的方法别名
        let result = bridge.resolve_method("Length", "");
        assert!(!result.rewritten);
        assert_eq!(result.rust_method, "Length");
    }

    #[test]
    fn test_rewrite_type_not_in_any_module() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // 完全未知的类型
        assert!(bridge.rewrite_type("CompletelyUnknownType123").is_none());
    }

    #[test]
    fn test_rewrite_type_case_sensitive() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // "never" (小写) 不应匹配 "Never" → "!"
        assert!(bridge.rewrite_type("never").is_none());
    }

    #[test]
    fn test_load_bridge_twice_idempotent() {
        let b1 = StdBridge::load(&std_dir()).unwrap();
        let b2 = StdBridge::load(&std_dir()).unwrap();
        assert_eq!(b1.module_mans.len(), b2.module_mans.len());
    }

    #[test]
    fn test_load_fallback_consistent() {
        let fb1 = StdBridge::load_fallback();
        let fb2 = StdBridge::load_fallback();
        assert_eq!(fb1.module_mans.len(), fb2.module_mans.len());
        assert_eq!(fb1.top.modules.len(), fb2.top.modules.len());
    }

    #[test]
    fn test_resolve_call_print_function() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_call("print");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rust_path, "println!");
        assert!(r.is_macro);
    }

    #[test]
    fn test_resolve_call_nonexistent() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        assert!(bridge.resolve_call("nonexistent_func_xyz").is_none());
    }

    #[test]
    fn test_shims_required_all_modules() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // 检查所有有 shim 的模块
        let fs_shims = bridge.shims_required("fs");
        assert!(!fs_shims.is_empty());
        // core 有 panic+print 的 shim
        let core_shims = bridge.shims_required("core");
        assert!(!core_shims.is_empty());
        // 无函数的模块应无 shim
        let fmt_shims = bridge.shims_required("fmt");
        assert!(fmt_shims.is_empty());
    }

    #[test]
    fn test_tier2_allowed_all_states() {
        let mut bridge = StdBridge::load(&std_dir()).unwrap();

        // State 1: denied by flag
        assert!(matches!(bridge.tier2_allowed(), Tier2CheckResult::DeniedFlag));

        // State 2: allowed, no version check
        bridge.set_tier2_allowed(true);
        assert!(matches!(bridge.tier2_allowed(), Tier2CheckResult::Allowed));

        // State 3: version mismatch
        bridge.set_rustc_version("rustc 1.0.0 (000000 2000-01-01)".to_string());
        assert!(matches!(bridge.tier2_allowed(), Tier2CheckResult::VersionMismatch { .. }));

        // State 4: back to denied
        bridge.set_tier2_allowed(false);
        assert!(matches!(bridge.tier2_allowed(), Tier2CheckResult::DeniedFlag));
    }

    #[test]
    fn test_resolve_import_non_std_slashes() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        // 非 std 路径保持原样
        let result = bridge.resolve_import(&["serde".into(), "Serialize".into()], &[]);
        assert_eq!(result.rust_path, "serde::Serialize");
        assert!(!result.is_tier2);
    }

    #[test]
    fn test_resolve_method_all_vec_aliases() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let aliases = vec![
            ("append", "push"), ("length", "len"), ("size", "len"),
            ("isEmpty", "is_empty"), ("sort", "sort"), ("reverse", "reverse"),
            ("contains", "contains"),
        ];
        for (lz, rust) in aliases {
            let result = bridge.resolve_method(lz, "Vec");
            assert_eq!(result.rust_method, rust,
                "Method '{}' should map to '{}', got '{}'", lz, rust, result.rust_method);
        }
    }

    #[test]
    fn test_resolve_method_all_str_aliases() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let aliases = vec![
            ("length", "len"), ("isEmpty", "is_empty"), ("trim", "trim"),
            ("startsWith", "starts_with"), ("endsWith", "ends_with"),
            ("contains", "contains"),
        ];
        for (lz, rust) in aliases {
            let result = bridge.resolve_method(lz, "String");
            assert_eq!(result.rust_method, rust,
                "String method '{}' should map to '{}', got '{}'", lz, rust, result.rust_method);
        }
    }

    #[test]
    fn test_resolve_import_with_alias_path() {
        let bridge = StdBridge::load(&std_dir()).unwrap();
        let result = bridge.resolve_import(&["std".into(), "collections".into()], &[]);
        assert!(!result.is_tier2);
        // 不应有 type_aliases（collections module 的 aliases section 为空）
        assert!(result.type_aliases.is_empty());
    }
}

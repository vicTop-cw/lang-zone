// Lang-Zong 编译器 — infer 模块
// 加载外部 lz-infer 生成的 .lzi 类型签名文件
// 为主编译器提供跨模块类型信息

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// .lzi 文件顶层结构（与 lz-infer 输出格式一致）
#[derive(serde::Deserialize, Debug)]
pub struct LziFile {
    pub version: String,
    pub modules: HashMap<String, LziModule>,
    #[serde(default)]
    pub unresolved: Vec<String>,
}

/// 模块签名
#[derive(serde::Deserialize, Debug)]
pub struct LziModule {
    #[serde(default)]
    pub functions: HashMap<String, LziFunction>,
    #[serde(default)]
    pub structs: HashMap<String, LziStruct>,
    #[serde(default)]
    pub consts: HashMap<String, LziConst>,
    #[serde(default)]
    pub type_aliases: HashMap<String, String>,
}

/// 函数签名
#[derive(serde::Deserialize, Debug)]
pub struct LziFunction {
    pub params: Vec<LziParam>,
    #[serde(rename = "return")]
    pub return_type: Option<String>,
    #[serde(default)]
    pub raises: Option<String>,
    #[serde(default)]
    pub generics: Vec<String>,
    #[serde(default)]
    pub generic_bounds: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub where_clause: HashMap<String, Vec<String>>,
}

/// 函数参数
#[derive(serde::Deserialize, Debug)]
pub struct LziParam {
    pub name: String,
    pub ty: String,
}

/// 结构体签名
#[derive(serde::Deserialize, Debug)]
pub struct LziStruct {
    #[serde(default)]
    pub fields: HashMap<String, String>,
    #[serde(default)]
    pub methods: HashMap<String, LziFunction>,
}

/// 常量签名
#[derive(serde::Deserialize, Debug)]
pub struct LziConst {
    pub ty: String,
    #[serde(default)]
    pub value: Option<String>,
}

impl LziFile {
    /// 从 JSON 字符串解析 .lzi 文件
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| format!("lzi parse error: {}", e))
    }

    /// 从文件路径加载 .lzi
    pub fn load(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("lzi read error ({}): {}", path.display(), e))?;
        Self::from_json(&content)
    }

    /// 查找指定模块的指定函数的签名
    pub fn lookup_function(&self, module: &str, name: &str) -> Option<&LziFunction> {
        self.modules.get(module)
            .and_then(|m| m.functions.get(name))
    }

    /// 查找指定模块的指定结构体的签名
    pub fn lookup_struct(&self, module: &str, name: &str) -> Option<&LziStruct> {
        self.modules.get(module)
            .and_then(|m| m.structs.get(name))
    }

    /// 查找指定模块的指定常量的签名
    pub fn lookup_const(&self, module: &str, name: &str) -> Option<&LziConst> {
        self.modules.get(module)
            .and_then(|m| m.consts.get(name))
    }
}

/// 类型签名目录：管理多个 .lzi 文件的加载和查询
#[derive(Default)]
pub struct LziRegistry {
    pub files: Vec<LziFile>,
}

impl LziRegistry {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// 从单个 .lzi 文件加载并创建 LziRegistry
    pub fn load_single(path: &Path) -> Result<Self, String> {
        let mut reg = Self::new();
        reg.load(path)?;
        Ok(reg)
    }

    /// 加载一个 .lzi 文件到 registry
    pub fn load(&mut self, path: &Path) -> Result<(), String> {
        let lzi = LziFile::load(path)?;
        self.files.push(lzi);
        Ok(())
    }

    /// 在所有已加载的 .lzi 文件中查找函数签名
    pub fn lookup_function(&self, module: &str, name: &str) -> Option<&LziFunction> {
        for file in &self.files {
            if let Some(f) = file.lookup_function(module, name) {
                return Some(f);
            }
        }
        None
    }
}

//! `.lzi` 文件格式定义与序列化
//!
//! `.lzi` 采用 JSON 做模块/函数/结构体索引，类型值使用 LZ 内联类型字符串，
//! 兼顾机器解析与人读调试。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const LZI_VERSION: &str = "0.1.0";

/// 顶层 `.lzi` 文件结构
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LziFile {
    pub version: String,
    pub modules: HashMap<String, LziModule>,
    #[serde(default)]
    pub unresolved: Vec<String>,
}

impl LziFile {
    pub fn new() -> Self {
        Self {
            version: LZI_VERSION.into(),
            modules: HashMap::new(),
            unresolved: Vec::new(),
        }
    }

    /// 序列化为格式化的 JSON 字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// 从 JSON 字符串反序列化
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

impl Default for LziFile {
    fn default() -> Self {
        Self::new()
    }
}

/// 单个模块的类型签名集合
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

impl Default for LziModule {
    fn default() -> Self {
        Self {
            functions: HashMap::new(),
            structs: HashMap::new(),
            consts: HashMap::new(),
            type_aliases: HashMap::new(),
        }
    }
}

/// 函数签名
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LziFunction {
    pub params: Vec<LziParam>,
    #[serde(rename = "return")]
    pub return_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raises: Option<String>,
    #[serde(default)]
    pub generics: Vec<String>,
    #[serde(default)]
    pub generic_bounds: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub where_clause: HashMap<String, Vec<String>>,
}

/// 函数字段
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LziParam {
    pub name: String,
    pub ty: String,
}

/// 结构体签名
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LziStruct {
    #[serde(default)]
    pub fields: HashMap<String, String>,
    #[serde(default)]
    pub methods: HashMap<String, LziFunction>,
}

/// 常量签名
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LziConst {
    pub ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lzi_roundtrip() {
        let mut file = LziFile::new();
        let mut module = LziModule::default();
        module.functions.insert(
            "add".into(),
            LziFunction {
                params: vec![
                    LziParam { name: "a".into(), ty: "int".into() },
                    LziParam { name: "b".into(), ty: "int".into() },
                ],
                return_type: Some("int".into()),
                raises: None,
                generics: vec![],
                generic_bounds: HashMap::new(),
                where_clause: HashMap::new(),
            },
        );
        module.consts.insert(
            "PI".into(),
            LziConst { ty: "f64".into(), value: None },
        );
        file.modules.insert("math".into(), module);

        let json = file.to_json().unwrap();
        let parsed = LziFile::from_json(&json).unwrap();
        assert_eq!(file, parsed);
    }
}

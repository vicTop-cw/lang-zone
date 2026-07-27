// Lang-Zong 编译器 — config/mod.rs
// 项目级配置管理：.lzconfig TOML 解析 + 编译选项
//
// 对标 Python sysconfig / configparser

pub mod paths;

use crate::util::mini_toml;
use std::path::Path;

/// 项目编译配置
#[derive(Debug, Clone)]
pub struct ProjectConfig {
    /// 项目名称
    pub name: Option<String>,
    /// Rust edition
    pub edition: String,
    /// 目标 Rust 版本
    pub rust_version: Option<String>,
    /// 是否允许 rustc_private
    pub allow_rustc_private: bool,
    /// 标准库目录
    pub std_dir: Option<String>,
    /// 自定义 crate 依赖
    pub dependencies: Vec<(String, String, Vec<String>)>, // (name, version, features)
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: None,
            edition: "2021".to_string(),
            rust_version: None,
            allow_rustc_private: false,
            std_dir: None,
            dependencies: Vec::new(),
        }
    }
}

impl ProjectConfig {
    /// 从 .lzconfig 文件加载配置
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        Self::parse(&content)
    }

    /// 从 TOML 字符串解析
    pub fn parse(toml_str: &str) -> Result<Self, String> {
        let doc = mini_toml::parse(toml_str)
            .map_err(|e| format!("Failed to parse config: {}", e))?;

        let mut config = Self::default();

        if let Some(project) = doc.get("project") {
            if let Some(val) = project.get("name") {
                if let mini_toml::TomlValue::Str(s) = val {
                    config.name = Some(s.clone());
                }
            }
            if let Some(val) = project.get("edition") {
                if let mini_toml::TomlValue::Str(s) = val {
                    config.edition = s.clone();
                }
            }
            if let Some(val) = project.get("rust-version") {
                if let mini_toml::TomlValue::Str(s) = val {
                    config.rust_version = Some(s.clone());
                }
            }
        }

        if let Some(compiler) = doc.get("compiler") {
            if let Some(val) = compiler.get("allow-rustc-private") {
                if let mini_toml::TomlValue::Bool(b) = val {
                    config.allow_rustc_private = *b;
                }
            }
            if let Some(val) = compiler.get("std-dir") {
                if let mini_toml::TomlValue::Str(s) = val {
                    config.std_dir = Some(s.clone());
                }
            }
        }

        if let Some(deps) = doc.get("dependencies") {
            for (name, val) in deps.iter() {
                match val {
                    // 简单版本: serde = "1.0"
                    mini_toml::TomlValue::Str(version) => {
                        config.dependencies.push((name.clone(), version.clone(), vec![]));
                    }
                    // 内联表: tokio = { version = "1.0", features = "full,macros" }
                    mini_toml::TomlValue::InlineTable(table) => {
                        let version = table.get("version")
                            .and_then(|v| if let mini_toml::TomlValue::Str(s) = v { Some(s.clone()) } else { None })
                            .unwrap_or_default();
                        let features = table.get("features")
                            .and_then(|v| {
                                if let mini_toml::TomlValue::Str(s) = v {
                                    Some(s.split(',').map(|s| s.trim().to_string()).collect())
                                } else { None }
                            })
                            .unwrap_or_default();
                        config.dependencies.push((name.clone(), version, features));
                    }
                    _ => {}
                }
            }
        }

        Ok(config)
    }
}

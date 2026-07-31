// Lang-Zong 编译器 — export/manifest.rs
// 为 @export 模块自动生成 Cargo.toml

use std::path::Path;

/// 生成的 Cargo.toml 内容
pub fn generate_cargo_toml(
    crate_name: &str,
    targets: &[TargetType],
    bridge_deps: &[String],
) -> String {
    let mut deps = String::new();
    let mut features = Vec::new();
    let mut crate_types = Vec::new();

    // cdylib: 输出 .dll/.so
    if targets.contains(&TargetType::Cdylib) {
        crate_types.push("\"cdylib\"");
    }

    // Python: pyo3 + extension-module
    if targets.contains(&TargetType::Python) {
        deps.push_str("pyo3 = { version = \"0.22\", features = [\"extension-module\"] }\n");
        features.push("\"pyo3\"");
        // Python 也需要 cdylib
        if !crate_types.contains(&"\"cdylib\"") {
            crate_types.push("\"cdylib\"");
        }
    }

    // std bridge 依赖
    for dep in bridge_deps {
        if !deps.contains(dep) {
            deps.push_str(&format!("{} = \"*\"\n", dep));
        }
    }

    let features_str = if features.is_empty() {
        String::new()
    } else {
        format!("\n[features]\ndefault = [{}]\n", features.join(", "))
    };

    format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = [{}]
name = "{}"

[dependencies]
{}{}"#,
        crate_name,
        crate_types.join(", "),
        crate_name,
        deps,
        features_str,
    )
}

/// 产物文件名（不含扩展名）
pub fn lib_filename(base: &str) -> String {
    let base = base.replace('-', "_");
    if cfg!(target_os = "windows") {
        format!("{}.dll", base)
    } else if cfg!(target_os = "macos") {
        format!("lib{}.dylib", base)
    } else {
        format!("lib{}.so", base)
    }
}

/// Python 产物文件名
pub fn pyd_filename(base: &str) -> String {
    let base = base.replace('-', "_");
    format!("{}.pyd", base)
}

/// 导出目标类型
#[derive(Debug, Clone, PartialEq)]
pub enum TargetType {
    /// 共享库 (.dll/.so/.dylib)
    Cdylib,
    /// Python 扩展 (.pyd via PyO3)
    Python,
}

/// 获取输出目录的相对路径
pub fn export_dir(src_path: &Path) -> std::path::PathBuf {
    let parent = src_path.parent().unwrap_or(Path::new("."));
    let stem = src_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("lz_export");
    parent.join(format!("{}_export", stem))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_basic_cargo_toml() {
        let toml = generate_cargo_toml("lz_math", &[TargetType::Cdylib], &[]);
        assert!(toml.contains("name = \"lz_math\""));
        assert!(toml.contains("\"cdylib\""));
    }

    #[test]
    fn test_generate_python_cargo_toml() {
        let toml = generate_cargo_toml("mymath", &[TargetType::Python], &[]);
        assert!(toml.contains("pyo3"));
        assert!(toml.contains("\"cdylib\""));
    }

    #[test]
    fn test_lib_filename() {
        let name = lib_filename("lz_math");
        assert!(name.ends_with(".dll") || name.ends_with(".so") || name.ends_with(".dylib"));
    }
}

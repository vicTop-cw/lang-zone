// Lang-Zong 编译器 — export/builder.rs
// DLL/SO 构建编排：生成 Cargo.toml + 调用 rustc/cargo

use super::manifest::{self, TargetType};
use std::path::{Path, PathBuf};
use std::process::Command;

/// 构建结果
#[derive(Debug)]
pub struct BuildResult {
    pub success: bool,
    pub lib_path: Option<PathBuf>,
    pub pyd_path: Option<PathBuf>,
    pub stderr: String,
}

/// 为 .lz 文件构建导出库
///
/// `rs_path`: 生成的 .rs 文件路径
/// `crate_name`: crate 名称
/// `targets`: 导出目标
/// `bridge_deps`: std bridge 依赖列表
pub fn build_export_lib(
    rs_path: &Path,
    crate_name: &str,
    targets: &[TargetType],
    bridge_deps: &[String],
) -> BuildResult {
    let export_dir = manifest::export_dir(rs_path);
    let _ = std::fs::create_dir_all(&export_dir);

    // 1. 复制 .rs 到导出目录
    let lib_rs = export_dir.join("src/lib.rs");
    let _ = std::fs::create_dir_all(lib_rs.parent().unwrap());
    if let Err(e) = std::fs::copy(rs_path, &lib_rs) {
        return BuildResult {
            success: false,
            lib_path: None,
            pyd_path: None,
            stderr: format!("Failed to copy .rs: {}", e),
        };
    }

    // 2. 生成 Cargo.toml
    let cargo_toml = manifest::generate_cargo_toml(crate_name, targets, bridge_deps);
    let toml_path = export_dir.join("Cargo.toml");
    if let Err(e) = std::fs::write(&toml_path, cargo_toml) {
        return BuildResult {
            success: false,
            lib_path: None,
            pyd_path: None,
            stderr: format!("Failed to write Cargo.toml: {}", e),
        };
    }

    // 3. 调用 cargo build
    let output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir(&export_dir)
        .output();

    match output {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            if !out.status.success() {
                return BuildResult {
                    success: false,
                    lib_path: None,
                    pyd_path: None,
                    stderr,
                };
            }

            // 4. 定位产物
            let build_type = "release";
            let lib_name = manifest::lib_filename(crate_name);
            let lib_path = export_dir
                .join("target")
                .join(build_type)
                .join(&lib_name);

            let pyd_path = if targets.contains(&TargetType::Python) {
                let pyd_name = manifest::pyd_filename(crate_name);
                Some(export_dir.join("target").join(build_type).join(&pyd_name))
            } else {
                None
            };

            BuildResult {
                success: true,
                lib_path: if lib_path.exists() { Some(lib_path) } else { None },
                pyd_path: pyd_path.filter(|p| p.exists()),
                stderr,
            }
        }
        Err(e) => BuildResult {
            success: false,
            lib_path: None,
            pyd_path: None,
            stderr: format!("cargo not available: {}", e),
        },
    }
}

/// 清理导出产物
pub fn clean_export(rs_path: &Path) {
    let export_dir = manifest::export_dir(rs_path);
    let _ = std::fs::remove_dir_all(&export_dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_fails_without_rs() {
        let result = build_export_lib(
            Path::new("nonexistent.rs"),
            "test_crate",
            &[TargetType::Cdylib],
            &[],
        );
        assert!(!result.success);
    }
}

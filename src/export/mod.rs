// Lang-Zong 编译器 — export 模块
// @export 装饰器增强：自动 DLL/SO 生成
//
// 入口: export_module() — 扫描模块中的 @export 标注函数并构建共享库

pub mod manifest;
pub mod builder;

use crate::ast::{Decorator, Expr, Function, Module};
use std::path::Path;

// ═══════════════════════════════════════════════════════
// ExportConfig — 从装饰器提取的导出配置
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct ExportConfig {
    pub targets: Vec<manifest::TargetType>,
    pub crate_name: Option<String>,
    pub python_module: Option<String>,
}

impl ExportConfig {
    /// 判断是否至少有一个导出目标
    pub fn has_exports(&self) -> bool {
        !self.targets.is_empty()
    }
}

// ═══════════════════════════════════════════════════════
// 装饰器解析
// ═══════════════════════════════════════════════════════

/// 从装饰器列表中提取导出配置
pub fn extract_export_config(decorators: &[Decorator]) -> Option<ExportConfig> {
    let export_deco = decorators.iter().find(|d| d.name == "export")?;

    let mut targets = Vec::new();
    let mut crate_name = None;
    let mut python_module = None;

    for arg in &export_deco.args {
        match arg {
            Expr::Ident(s) if s == "Rust" => {
                targets.push(manifest::TargetType::Cdylib);
            }
            Expr::Ident(s) if s == "Python" => {
                targets.push(manifest::TargetType::Python);
            }
            // 带参数: name="xxx", module="xxx"
            Expr::KwArg { name, value } => {
                match name.as_str() {
                    "name" => {
                        if let Expr::StrLit(v) = value.as_ref() {
                            crate_name = Some(v.clone());
                        }
                    }
                    "module" => {
                        if let Expr::StrLit(v) = value.as_ref() {
                            python_module = Some(v.clone());
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    if targets.is_empty() {
        None
    } else {
        Some(ExportConfig { targets, crate_name, python_module })
    }
}

/// 检查函数是否对指定目标导出
pub fn is_exported(decorators: &[Decorator]) -> bool {
    extract_export_config(decorators).is_some()
}

/// 收集模块中所有的 Bridge 依赖（从 import 语句推断）
pub fn collect_bridge_deps(module: &Module) -> Vec<String> {
    let mut deps = Vec::new();
    for imp in &module.imports {
        let path_str = imp.path.join(".");
        if path_str.starts_with("std.bridge.rust.") {
            let crate_name = path_str
                .strip_prefix("std.bridge.rust.")
                .unwrap_or(&path_str)
                .split('.')
                .next()
                .unwrap_or("");
            if !crate_name.is_empty() && !deps.contains(&crate_name.to_string()) {
                deps.push(crate_name.to_string());
            }
        }
    }
    deps
}

// ═══════════════════════════════════════════════════════
// 顶层 API
// ═══════════════════════════════════════════════════════

/// 构建一个模块的所有导出库
pub fn export_module(
    module: &Module,
    rs_path: &Path,
) -> Vec<builder::BuildResult> {
    let mut results = Vec::new();
    let bridge_deps = collect_bridge_deps(module);

    // 收集有 @export 的函数
    let exported_fns: Vec<&Function> = module.functions.iter()
        .filter(|f| is_exported(&f.decorators))
        .collect();

    if exported_fns.is_empty() {
        return results;
    }

    // 取第一个 @export 函数的配置作为模块级配置
    // （多个 @export 函数共享同一个 crate）
    if let Some(config) = extract_export_config(&exported_fns[0].decorators) {
        let crate_name = config.crate_name.unwrap_or_else(|| {
            rs_path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("lz_export")
                .to_string()
        });

        let result = builder::build_export_lib(
            rs_path,
            &crate_name,
            &config.targets,
            &bridge_deps,
        );
        results.push(result);
    }

    results
}

/// 清理导出产物
pub fn clean_exports(rs_path: &Path) {
    builder::clean_export(rs_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export_deco(args: Vec<Expr>) -> Decorator {
        Decorator { name: "export".into(), args }
    }

    #[test]
    fn test_extract_rust_export() {
        let d = export_deco(vec![Expr::Ident("Rust".into())]);
        let cfg = extract_export_config(&[d]).unwrap();
        assert_eq!(cfg.targets, vec![manifest::TargetType::Cdylib]);
    }

    #[test]
    fn test_extract_python_export() {
        let d = export_deco(vec![Expr::Ident("Python".into())]);
        let cfg = extract_export_config(&[d]).unwrap();
        assert_eq!(cfg.targets, vec![manifest::TargetType::Python]);
    }

    #[test]
    fn test_no_export_returns_none() {
        let d = Decorator { name: "unsafe".into(), args: vec![] };
        assert!(extract_export_config(&[d]).is_none());
    }
}

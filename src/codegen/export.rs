// Lang-Zong 编译器 — codegen/export.rs
// @export(Rust|Python|C) 导出代码生成
//
// 设计目标：将 lz 函数/类型导出为目标语言的包或源码。
// - @export(Rust)         → 生成 pub fn（默认），可附带 Cargo.toml 提示
// - @export(Python)       → 生成 PyO3 #[pyfunction] + #[pymodule] 绑定
// - @export(Python, pyo3) → 同上（显式指定 PyO3）
// - @export(C)            → 生成 extern "C" ABI
//
// 对标 Mojo 的 @export：写一次，多语言导出。

use crate::ast::{Decorator, Expr};

/// 导出目标
#[derive(Debug, Clone, PartialEq)]
pub enum ExportTarget {
    /// Rust crate（pub fn，可附带 crate 配置）
    Rust,
    /// Python PyO3 绑定
    Python,
    /// C ABI（extern "C"）
    C,
}

/// 从装饰器列表中提取导出目标
pub fn extract_exports(decorators: &[Decorator]) -> Vec<ExportTarget> {
    let mut targets = Vec::new();
    for d in decorators {
        if d.name != "export" { continue; }
        for arg in &d.args {
            match arg {
                Expr::Ident(s) if s == "Rust" => targets.push(ExportTarget::Rust),
                Expr::Ident(s) if s == "Python" => targets.push(ExportTarget::Python),
                Expr::Ident(s) if s == "C" => targets.push(ExportTarget::C),
                // 配置参数（如 export(Python, module="mymod")）暂不处理具体值
                _ => {}
            }
        }
    }
    targets
}

/// 检查是否对指定目标导出
pub fn is_exported_to(decorators: &[Decorator], target: ExportTarget) -> bool {
    extract_exports(decorators).contains(&target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decorator_named(name: &str) -> Decorator {
        Decorator { name: name.into(), args: vec![] }
    }

    fn export_decorator(args: Vec<&str>) -> Decorator {
        Decorator {
            name: "export".into(),
            args: args.into_iter().map(|s| Expr::Ident(s.into())).collect(),
        }
    }

    #[test]
    fn test_extract_rust_export() {
        let d = export_decorator(vec!["Rust"]);
        let targets = extract_exports(&[d]);
        assert_eq!(targets, vec![ExportTarget::Rust]);
    }

    #[test]
    fn test_extract_multi_export() {
        let d = export_decorator(vec!["Rust", "Python"]);
        let targets = extract_exports(&[d]);
        assert_eq!(targets, vec![ExportTarget::Rust, ExportTarget::Python]);
    }

    #[test]
    fn test_ignore_non_export() {
        let d = decorator_named("simd");
        assert!(extract_exports(&[d]).is_empty());
    }

    #[test]
    fn test_is_exported_to() {
        let d = export_decorator(vec!["Python"]);
        assert!(is_exported_to(&[d.clone()], ExportTarget::Python));
        assert!(!is_exported_to(&[d], ExportTarget::Rust));
    }
}

// Lang-Zong 编译器 — util/import.rs
// 模块导入机制：路径解析 + 循环依赖检测
//
// 对标 Python importlib — 模块搜索、加载、缓存
// 当前阶段：提供路径解析和基础循环检测工具

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 导入栈条目：正在解析的模块
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ImportEntry {
    /// 模块路径（如 ["std", "vec"]）
    path: Vec<String>,
    /// 源文件绝对路径
    file: PathBuf,
}

/// 导入解析器 — 管理模块搜索和循环依赖检测
#[derive(Debug, Default)]
pub struct ImportResolver {
    /// 已解析的模块集合（避免重复导入）
    resolved: HashSet<Vec<String>>,
    /// 当前导入栈（用于循环依赖检测）
    stack: Vec<ImportEntry>,
}

impl ImportResolver {
    pub fn new() -> Self {
        Self {
            resolved: HashSet::new(),
            stack: Vec::new(),
        }
    }

    /// 将 LZ 模块路径转换为文件系统路径
    ///
    /// `lz_path`: 如 ["std", "vec"] → std/vec.lz
    /// `base_dir`: 搜索起点
    pub fn resolve_path(lz_path: &[String], base_dir: &Path) -> Vec<PathBuf> {
        let rel = lz_path.join("/");
        vec![
            base_dir.join(&rel).with_extension("lz"),
            base_dir.join(&rel).join("mod.lz"),
        ]
    }

    /// 检查是否已解析
    pub fn is_resolved(&self, path: &[String]) -> bool {
        self.resolved.contains(path)
    }

    /// 标记为已解析
    pub fn mark_resolved(&mut self, path: Vec<String>) {
        self.resolved.insert(path);
    }

    /// 推进导入栈（开始解析新模块）
    pub fn push(&mut self, path: Vec<String>, file: PathBuf) -> Result<(), String> {
        // 检查是否正在栈中（循环依赖）
        for entry in &self.stack {
            if entry.path == path {
                let cycle: Vec<String> = self.stack.iter()
                    .skip_while(|e| e.path != path)
                    .map(|e| e.path.join("::"))
                    .collect();
                return Err(format!(
                    "Circular import detected: {} -> ...",
                    cycle.join(" -> ")
                ));
            }
        }
        self.stack.push(ImportEntry { path, file });
        Ok(())
    }

    /// 弹出导入栈顶部
    pub fn pop(&mut self) {
        self.stack.pop();
    }

    /// 当前解析深度
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// 获取当前正在解析的模块路径
    pub fn current_module(&self) -> Option<&[String]> {
        self.stack.last().map(|e| e.path.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_cycle() {
        let mut resolver = ImportResolver::new();
        assert!(resolver.push(
            vec!["a".to_string()], PathBuf::from("a.lz")
        ).is_ok());
        assert!(resolver.push(
            vec!["b".to_string()], PathBuf::from("b.lz")
        ).is_ok());
        resolver.pop();
        resolver.pop();
    }

    #[test]
    fn test_detect_cycle() {
        let mut resolver = ImportResolver::new();
        resolver.push(vec!["a".to_string()], PathBuf::from("a.lz")).unwrap();
        resolver.push(vec!["b".to_string()], PathBuf::from("b.lz")).unwrap();
        // b 再次导入 a → 循环
        let err = resolver.push(vec!["a".to_string()], PathBuf::from("a.lz"));
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("Circular import"));
    }

    #[test]
    fn test_resolve_path() {
        let paths = ImportResolver::resolve_path(
            &["std".to_string(), "vec".to_string()],
            Path::new("/tmp")
        );
        assert_eq!(paths.len(), 2);
        assert!(paths[0].to_str().unwrap().replace('\\', "/").ends_with("std/vec.lz"));
        assert!(paths[1].to_str().unwrap().replace('\\', "/").ends_with("std/vec/mod.lz"));
    }
}

// Lang-Zong 编译器 — config/paths.rs
// 标准库路径解析 + 模块搜索路径管理
//
// 对标 Python sys.path / sysconfig — 确定编译器标准库位置
// 保持简单：当前阶段仅支持 cargo 目录内 std/ 目录

use std::path::{Path, PathBuf};

/// 解析标准库目录
///
/// 搜索优先级：
/// 1. 用户通过 --std-dir CLI 显式指定的路径
/// 2. 编译器可执行文件同级 std/ 目录
/// 3. 当前工作目录的 std/ 目录
/// 4. 环境变量 LZ_STD_DIR
pub fn resolve_std_dir(cli_dir: Option<&Path>) -> Option<PathBuf> {
    // 1. CLI 显式指定
    if let Some(dir) = cli_dir {
        if dir.is_dir() {
            return Some(dir.to_path_buf());
        }
    }

    // 2. 可执行文件同级 std/ 目录
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("std");
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }

    // 3. 当前工作目录 std/
    let cwd_std = PathBuf::from("std");
    if cwd_std.is_dir() {
        return Some(cwd_std);
    }

    // 4. 环境变量
    if let Ok(env_dir) = std::env::var("LZ_STD_DIR") {
        let p = PathBuf::from(&env_dir);
        if p.is_dir() {
            return Some(p);
        }
    }

    None
}

/// 模块搜索路径列表
/// 对标 Python sys.path — 有序的模块搜索目录
#[derive(Debug, Clone)]
pub struct SearchPaths {
    pub entries: Vec<PathBuf>,
}

impl Default for SearchPaths {
    fn default() -> Self {
        Self {
            entries: vec![
                PathBuf::from("."),           // 当前目录
                PathBuf::from("std"),         // 标准库
            ],
        }
    }
}

impl SearchPaths {
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加搜索路径
    pub fn push(&mut self, path: PathBuf) {
        self.entries.push(path);
    }

    /// 在路径列表中查找模块文件
    /// module_path: "foo::bar" → 查找 foo/bar.lz 或 foo/bar/mod.lz
    pub fn find_module(&self, module_path: &[String]) -> Option<PathBuf> {
        let rel = module_path.join("/");
        for entry in &self.entries {
            // 尝试 foo/bar.lz
            let file = entry.join(&rel).with_extension("lz");
            if file.exists() {
                return Some(file);
            }
            // 尝试 foo/bar/mod.lz
            let dir_mod = entry.join(&rel).join("mod.lz");
            if dir_mod.exists() {
                return Some(dir_mod);
            }
        }
        None
    }
}

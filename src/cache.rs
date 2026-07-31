//! 增量编译缓存（零外部依赖）
//!
//! 每个 .lz 源文件对应一个 .lzcache 文件在缓存目录中，
//! 记录源文件哈希和依赖信息。编译前先校验缓存，命中则跳过。
//!
//! .lzcache 格式:
//! ```text
//! hash=1a2b3c4d
//! deps=dep_a.lz:hash1,dep_b.lz:hash2
//! output=module.rs
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 单个源文件的缓存记录
#[derive(Debug, Clone, Default)]
pub struct CacheEntry {
    pub hash: String,
    pub deps: Vec<(String, String)>, // (dep_path, dep_hash)
    pub output: String,              // 产物文件名（相对于缓存目录）
}

impl CacheEntry {
    /// 从缓存目录加载某 .lz 文件的 .lzcache
    pub fn load(cache_dir: &Path, source: &Path) -> io::Result<Option<Self>> {
        let cache_file = cache_file_path(cache_dir, source);
        if !cache_file.exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&cache_file)?;
        Ok(Some(Self::parse(&data)))
    }

    /// 解析 .lzcache 文本
    fn parse(data: &str) -> Self {
        let mut entry = CacheEntry::default();
        for line in data.lines() {
            let line = line.trim();
            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "hash" => entry.hash = value.to_string(),
                    "output" => entry.output = value.to_string(),
                    "deps" => {
                        entry.deps = value.split(',')
                            .filter(|s| !s.is_empty())
                            .filter_map(|s| {
                                let (dep_path, dep_hash) = s.split_once(':')?;
                                Some((dep_path.to_string(), dep_hash.to_string()))
                            })
                            .collect();
                    }
                    _ => {}
                }
            }
        }
        entry
    }

    /// 序列化为 .lzcache 文本
    fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("hash={}\n", self.hash));
        if !self.deps.is_empty() {
            let deps_str: Vec<String> = self.deps.iter()
                .map(|(p, h)| format!("{}:{}", p, h))
                .collect();
            out.push_str(&format!("deps={}\n", deps_str.join(",")));
        }
        out.push_str(&format!("output={}\n", self.output));
        out
    }

    /// 写入缓存文件
    pub fn save(&self, cache_dir: &Path, source: &Path) -> io::Result<()> {
        fs::create_dir_all(cache_dir)?;
        let cache_file = cache_file_path(cache_dir, source);
        fs::write(&cache_file, self.serialize())
    }

    /// 检查缓存是否有效（源哈希匹配 + 所有依赖哈希匹配）
    pub fn is_fresh(&self, source: &Path, cache_dir: &Path) -> bool {
        // 1. 源文件哈希
        let current = content_hash(source).unwrap_or_default();
        if current != self.hash {
            return false;
        }
        // 2. 依赖哈希
        for (dep_path, dep_hash) in &self.deps {
            let dep_full = if dep_path.starts_with('/') || dep_path.contains(':') {
                PathBuf::from(dep_path)
            } else {
                // 依赖路径相对于源文件目录
                source.parent().unwrap_or(Path::new(".")).join(dep_path)
            };
            let dep_current = content_hash(&dep_full).unwrap_or_default();
            if &dep_current != dep_hash {
                return false;
            }
        }
        // 3. 产物存在（output 为空则跳过此检查）
        if self.output.is_empty() {
            return true;
        }
        let output = cache_dir.join(&self.output);
        output.exists()
    }
}

/// 计算文件内容哈希
pub fn content_hash(path: &Path) -> io::Result<String> {
    use std::hash::{Hash, Hasher};
    let data = fs::read(path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    let h = hasher.finish();
    Ok(format!("{:016x}", h))
}

/// 缓存文件名：module.lz → module.lzcache
fn cache_file_path(cache_dir: &Path, source: &Path) -> PathBuf {
    let stem = source.file_stem().unwrap_or_default().to_string_lossy();
    cache_dir.join(format!("{}.lzcache", stem))
}

/// 产物文件名：module.lz → module.rs
pub fn output_filename(source: &Path) -> String {
    format!("{}.rs", source.file_stem().unwrap_or_default().to_string_lossy())
}

/// 从 AST 模块提取依赖（非 std 的 import 语句）
pub fn scan_deps(module: &crate::ast::Module) -> Vec<String> {
    module.imports.iter()
        .filter(|imp| imp.path.first().map(|s| s.as_str()) != Some("std"))
        .map(|imp| format!("{}.lz", imp.path.first().unwrap()))
        .collect()
}

/// 批量检查并清除无效缓存条目
pub fn prune_stale(cache_dir: &Path, known_sources: &[PathBuf]) -> io::Result<usize> {
    let mut removed = 0;
    if cache_dir.exists() {
        for entry in fs::read_dir(cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "lzcache" || e == "rs").unwrap_or(false) {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                let source_exists = known_sources.iter().any(|s| {
                    s.file_stem().map(|st| st.to_string_lossy() == stem).unwrap_or(false)
                });
                if !source_exists {
                    let _ = fs::remove_file(&path);
                    removed += 1;
                }
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_parse_roundtrip() {
        let e = CacheEntry {
            hash: "abc123".into(),
            deps: vec![("dep.lz".into(), "def456".into())],
            output: "test.rs".into(),
        };
        let s = e.serialize();
        let parsed = CacheEntry::parse(&s);
        assert_eq!(parsed.hash, "abc123");
        assert_eq!(parsed.deps[0], ("dep.lz".to_string(), "def456".to_string()));
        assert_eq!(parsed.output, "test.rs");
    }

    #[test]
    fn entry_parse_no_deps() {
        let e = CacheEntry {
            hash: "xyz".into(),
            deps: vec![],
            output: "out.rs".into(),
        };
        let s = e.serialize();
        let parsed = CacheEntry::parse(&s);
        assert_eq!(parsed.hash, "xyz");
        assert!(parsed.deps.is_empty());
    }
}

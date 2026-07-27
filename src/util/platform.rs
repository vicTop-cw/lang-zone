// Lang-Zong 编译器 — util/platform.rs
// 平台抽象层：OS 检测、行尾归一化、路径规范化
//
// 对标 Rust `std::env::consts` + 编译目标的平台适配

use std::path::Path;

/// 宿主操作系统
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOS {
    Windows,
    Linux,
    MacOS,
    Unknown,
}

/// 检测当前宿主操作系统
pub fn host_os() -> HostOS {
    if cfg!(target_os = "windows") {
        HostOS::Windows
    } else if cfg!(target_os = "linux") {
        HostOS::Linux
    } else if cfg!(target_os = "macos") {
        HostOS::MacOS
    } else {
        HostOS::Unknown
    }
}

/// 宿主操作系统名称（用于诊断输出）
pub fn host_os_name() -> &'static str {
    match host_os() {
        HostOS::Windows => "windows",
        HostOS::Linux => "linux",
        HostOS::MacOS => "macos",
        HostOS::Unknown => "unknown",
    }
}

/// 宿主架构（x86_64 / aarch64）
pub fn host_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    }
}

/// 宿主目标三元组（如 x86_64-pc-windows-msvc）
pub fn host_target() -> String {
    format!("{}-{}", host_arch(), host_target_os())
}

/// 目标 OS 后缀（用于目标三元组）
fn host_target_os() -> &'static str {
    match host_os() {
        HostOS::Windows => "pc-windows-msvc",
        HostOS::Linux => "unknown-linux-gnu",
        HostOS::MacOS => "apple-darwin",
        HostOS::Unknown => "unknown",
    }
}

/// 平台路径分隔符（`\\` 或 `/`）
pub fn path_separator() -> char {
    if cfg!(target_os = "windows") { '\\' } else { '/' }
}

/// 将平台路径统一为正斜杠格式（内部规范化）
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// 将内部正斜杠路径转为平台原生格式
pub fn to_native_path(path: &str) -> String {
    if cfg!(target_os = "windows") {
        path.replace('/', "\\")
    } else {
        path.to_string()
    }
}

/// 规范化路径（展开 ~、转换分隔符、去除冗余 `.` 和 `..`）
pub fn canonicalize(path: &Path) -> Option<String> {
    std::fs::canonicalize(path).ok()
        .map(|p| normalize_path(&p.to_string_lossy()))
}

/// 将 CRLF/CR 行尾统一为 LF
pub fn normalize_line_endings(source: &str) -> String {
    source.replace("\r\n", "\n").replace('\r', "\n")
}

/// 检测源码是否包含 BOM（UTF-8 BOM: EF BB BF）
pub fn strip_bom(source: &[u8]) -> &[u8] {
    if source.len() >= 3 && source[0] == 0xEF && source[1] == 0xBB && source[2] == 0xBF {
        &source[3..]
    } else {
        source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_os_detected() {
        // 至少不会 panic
        let _ = host_os();
        let _ = host_os_name();
        let _ = host_arch();
        assert!(!host_target().is_empty());
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("foo\\bar\\baz"), "foo/bar/baz");
        assert_eq!(normalize_path("foo/bar/baz"), "foo/bar/baz");
    }

    #[test]
    fn test_line_endings() {
        assert_eq!(normalize_line_endings("a\r\nb\nc\r"), "a\nb\nc\n");
    }

    #[test]
    fn test_strip_bom() {
        let with_bom = &[0xEF, 0xBB, 0xBF, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(strip_bom(with_bom), b"hello");

        let without_bom = b"hello";
        assert_eq!(strip_bom(without_bom), b"hello");
    }
}

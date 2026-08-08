// Lang-Zong 编译器 — util/version.rs
// 编译器版本信息：版本号、最低 Rust 版本要求、版本字符串
//
// 对标 Rust `version!()` 宏 + `rustc --version` 输出格式

/// 编译器主版本号
pub const VERSION_MAJOR: u32 = 0;
/// 编译器次版本号
pub const VERSION_MINOR: u32 = 1;
/// 编译器修订版本号（每小阶段推进一次，v0.133 起 + 13 小阶段 = 146）
pub const VERSION_PATCH: u32 = 146;
/// 预发布标签（空 = 正式版）
pub const VERSION_PRE: &str = "alpha";

/// 编译器完整版本号（如 "0.1.0-alpha"）
pub fn version() -> String {
    let base = format!("{}.{}.{}", VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH);
    if VERSION_PRE.is_empty() {
        base
    } else {
        format!("{}-{}", base, VERSION_PRE)
    }
}

/// 编译器名称 + 版本（如 "Lang-Zong 0.1.0-alpha"）
pub fn version_full() -> String {
    format!("Lang-Zong {}", version())
}

/// 所需的最低 Rust 编译器版本（供 std bridge tier2 检查）
pub const RUSTC_MIN_VERSION: &str = "1.70.0";

/// 支持的 Rust edition
pub const SUPPORTED_EDITIONS: &[&str] = &["2021"];

/// 默认 Rust edition
pub const DEFAULT_EDITION: &str = "2021";

/// 检查 Rust edition 是否受支持
pub fn is_supported_edition(edition: &str) -> bool {
    SUPPORTED_EDITIONS.contains(&edition)
}

/// 编译器构建信息（编译时注入，或默认值）
pub fn build_info() -> String {
    format!(
        "{} (built for {})",
        version_full(),
        crate::util::platform::host_target(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_not_empty() {
        assert!(!version().is_empty());
        assert!(version_full().contains("Lang-Zong"));
    }

    #[test]
    fn test_edition_support() {
        assert!(is_supported_edition("2021"));
        assert!(!is_supported_edition("2018"));
    }
}

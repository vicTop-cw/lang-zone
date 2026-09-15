// Lang-Zong 编译器 — util/mod.rs

pub mod chars;
pub mod error;
pub mod import;
pub mod mini_toml;
pub mod platform;
pub mod source;
pub mod version;

pub mod parallel;

pub use error::{CompilerError, ErrorKind, Result};
pub use import::ImportResolver;
pub use mini_toml::*;
pub use parallel::{JoinHandle, TempDir, ThreadPool};
pub use platform::{
    host_arch, host_os, host_os_name, host_target, normalize_line_endings, normalize_path,
    strip_bom,
};
pub use source::{output_path, write_output, SourceCache};
pub use version::{build_info, version, version_full};

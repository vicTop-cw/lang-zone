// Lang-Zong 编译器 — util/mod.rs

pub mod mini_toml;
pub mod error;
pub mod import;
pub mod chars;
pub mod platform;
pub mod version;
pub mod source;

pub mod parallel;

pub use mini_toml::*;
pub use error::{CompilerError, ErrorKind, Result};
pub use import::ImportResolver;
pub use parallel::{ThreadPool, JoinHandle, TempDir};
pub use platform::{host_os, host_os_name, host_arch, host_target, normalize_path, normalize_line_endings, strip_bom};
pub use version::{version, version_full, build_info};
pub use source::{SourceCache, write_output, output_path};

// Lang-Zone 编译器 — cli.rs（薄 shim）
// M3 T0r55.1.2：子命令实现已下沉到 lib crate 的 `lang_zone::bridge::project`。
// 本文件只做 re-export，保持 `cli::cmd_*` 调用面不变（main.rs 无需改动）。
// CY/（lzcyc）可直接调用 `lang_zone::bridge::project::cmd_*`。

pub use lang_zone::bridge::project::{
    cmd_build, cmd_check, cmd_create, cmd_emit_bridge_report, cmd_peek, cmd_push, print_help,
};

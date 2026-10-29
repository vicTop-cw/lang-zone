// Lang-Zone 编译器 — ir/codegen/helpers.rs
// LZIR → Rust 代码生成辅助函数（原 2076 行单文件按职责拆为 3 个子模块）。
// 拆分口径（move-only：函数体/注释/签名逐字节未改）：
//   pred — 判定/查询（叶子层，无出边）：块/表达式/IrType 只读谓词
//   scan — 收集与可变性扫描：collect_* / scan_* 写 out 集合（let mut、全局提升、写穿别名）
//   emit — 文本发射/改写 + 类型变换：返回 String 的片段生成，及 IrType 构造/替换/推断
// 外部路径 crate::ir::codegen::helpers::<fn> 保持不变：下面逐模块 glob 再导出。
mod emit;
mod pred;
mod scan;

pub(crate) use emit::*;
pub(crate) use pred::*;
pub(crate) use scan::*;

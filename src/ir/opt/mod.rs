// Lang-Zone 编译器 — ir/opt/mod.rs
// LZIR 优化 pass 层（建议书方向 A）
//
// 定位：挂在 `builder::build_ir_inner` 出口（TCO 改写之后、IR 交回 codegen 之前），
// **以 IrModule 为输入输出**，因此 Rust 与 Cython 两个后端同时受益，且
// `ir::node` 结构保持稳定（pass 只改表达式/语句内容，不新增节点类型）。
//
// 与既有能力的职责边界：
//   - `comptime`：处理**编译期可确定**的顶层常量（源码级求值 → 内联为字面量）；
//   - 本层 pass：处理**通用 IR 冗余**（常量表达式下沉、恒定分支、死绑定）。
//
// 开关（照抄 `builder::tail_call` 的 AtomicBool + 环境变量范式）：
//   --opt=0（默认）全部关闭 → 产物与优化前**逐字节一致**，既有 golden 全部照旧通过；
//   --opt=1  常量折叠 + 分支简化；
//   --opt=2  追加死绑定消除（let x = <纯表达式> 且 x 之后无人使用 → 删绑定）。
//   环境变量 `LZ_OPT=0/1/2` 等价（便于测试与基准脚本传参）。
//
// 保守优先原则（与建议书第五节一致）：任何 pass 只要无法证明语义等价，就保持原状。
// 特别地，**整型溢出行为必须保持**：fold 用 checked_*，溢出即放弃折叠，
// 否则 debug 构建下 `i64::MAX + 1` 会从「panic」变成「静默回绕」。

mod branch;
mod dce;
mod fold;
mod walk;

use super::IrModule;
use std::sync::atomic::{AtomicU8, Ordering};

static OPT_LEVEL: AtomicU8 = AtomicU8::new(0);

fn env_level() -> Option<u8> {
    std::env::var("LZ_OPT")
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
}

pub fn set_opt_level(level: u8) {
    OPT_LEVEL.store(level.min(2), Ordering::Relaxed);
}

/// 当前优化层级（未显式设置时读环境变量，默认 0=关闭）。
pub fn opt_level() -> u8 {
    let v = OPT_LEVEL.load(Ordering::Relaxed);
    if v != 0 {
        return v;
    }
    env_level().unwrap_or(0).min(2)
}

/// pass 层入口：就地优化 IrModule。由 build_ir_inner 出口调用。
pub fn optimize_ir(ir_mod: &mut IrModule) {
    let level = opt_level();
    if level == 0 {
        return;
    }
    for item in ir_mod.items.iter_mut() {
        fold::optimize_item(item, level);
        branch::optimize_item(item, level);
        if level >= 2 {
            dce::optimize_item(item, level);
        }
    }
    if level >= 2 {
        // 顶层语句（模块级副作用）也要跑一遍
        for s in ir_mod.top_level_stmts.iter_mut() {
            fold::optimize_stmt(s, level);
            branch::optimize_stmt(s, level);
            dce::optimize_stmt(s, level);
        }
    }
}
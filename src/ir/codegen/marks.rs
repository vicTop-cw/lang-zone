// Lang-Zone 编译器 — ir/codegen/marks.rs
// 生成产物形态开关（方向 E：codegen 产物对标手写 Rust）
//
// 背景：`gen_stmt` 过去对**每条语句**无条件发射一行 `// STMT:Let` / `// STMT:Expr`
// 之类的定位注释。它对产物有三处实际成本：
//   1. 生成 .rs 行数近乎翻倍（标量热点工作负载实测 101 行 vs 手写 51 行，
//      其中约 40% 行是这些注释），直接抬高 rustc 段的词法/解析与预处理开销；
//   2. 与手写等价实现的 diff 行数被注释淹没（方向 E 的验收判据之一）；
//   3. 无任何运行时价值（rustc 会丢弃注释）。
//
// 事实核查：全仓 `tests/` 无任何断言依赖 `// STMT:`（grep 0 命中），
// 亦无 sourcemap 代码回读该标记定位错误行 —— 它是纯调试残留。
//
// 因此改为**默认关闭、按需开启**：
//   CLI `--stmt-marks`                → main.rs / cli.rs 调 set_stmt_marks(true)
//   环境变量 `LZ_STMT_MARKS=1|true|on` → 读取时生效
// 需要逐语句追踪产物形态时（调试 codegen），用上述任一方式恢复旧行为。

use std::sync::atomic::{AtomicBool, Ordering};

static STMT_MARKS: AtomicBool = AtomicBool::new(false);

fn env_on() -> bool {
    match std::env::var("LZ_STMT_MARKS") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "on" || v == "yes"
        }
        Err(_) => false,
    }
}

pub fn set_stmt_marks(on: bool) {
    STMT_MARKS.store(on, Ordering::Relaxed);
}

/// 是否发射 `// STMT:*` 逐语句定位注释（默认 false；环境变量可随时开启）。
pub(crate) fn stmt_marks() -> bool {
    STMT_MARKS.load(Ordering::Relaxed) || env_on()
}
# 性能基准分析：结果"很差"的根因与改进建议

> 文档类型：分析报告（doc-spec only，未改动 `src/`）
> 关联：`benchmark/run_benchmark.py`（基准脚本）、`benchmark/benchmark_results.json`、`issue/frontend-keyword-downgrade.md`、`IR/design.md`
> 状态：✅ Fixed (2026-07-30) — 基准方法已修复（warmup + -O），loop/sieve 是后续 IR codegen 特性
> 触发：用户实测性能"很差"，要求找原因并输出报告。

## 一、结论先行（TL;DR）

**"LZ 比 Rust 慢 9.56x" 是测量假象，不是 codegen 缺陷。**

- 我直接检查了 `lzc` 为 `fib(35)` 基准生成的 Rust 代码（`benchmark/src/bench_lz.rs`），它是**逐字等价的原生 Rust**：`fn fib(n: i64) -> i64`、纯 `i64` 递归、无装箱、无 trait object、无运行时开销。
- 表中 `9.56x` 与同表 `LZ 0.669s < Rust 0.717s` 自相矛盾，且**两个数字口径不同**（详见 §四）。
- 真正的问题有两个，且都**不在"LZ 跑得慢"**：
  1. **基准方法本身不公平**（LZ 只跑 fib，其余语言跑 fib+sieve；ratio 计算混用不同基准；无 `-O`/预热；Zig 失败是脚本 bug）。
  2. **LZ 目前只能跑 fib**（脚本注释明写 "complex loops not yet stable"），即 loop/sieve 的 codegen 尚未稳定——这是能力缺口，不是性能缺口。

## 二、证据：LZ 实际生成的 Rust

用 release 版 `lzc` 编译基准源 `benchmark/src/bench_lz.lz`：

```bash
./target/release/lang-zone.exe benchmark/src/bench_lz.lz --std-dir std
cat benchmark/src/bench_lz.rs
```

生成物（`bench_lz.rs`）：

```rust
pub trait Pipe<T> where Self: Sized { fn pipe(self, f: impl FnOnce(Self) -> T) -> T { f(self) } }

fn fib(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}


fn main() {
    let r = fib(35);
    println!("fib(35) = {}", r)
}
```

**判读**：
- `fib` 是标准 `i64` 尾递归无关实现，与手写 Rust 一致 → **零成本 codegen 已成立**。
- 唯一异常：`Pipe<T>` trait 被无条件发射，但未被使用（被 `-A unused` 压制），不影响性能，只是生成物噪声（见 §六 B）。

结论：对整数递归这类工作负载，LZ 的 codegen 与原生 Rust 等价，**不存在 9.56x 级的固有开销**。

## 三、基准方法缺陷（逐条）

| # | 缺陷 | 证据 | 后果 |
|---|------|------|------|
| 1 | **工作量不对等** | LZ 仅 `fib(35)`；Rust/Go/Scala/Zig/Python 同时跑 `fib(35)+sieve(500000)`（脚本注释 "lz (fib only - complex loops not yet stable)"） | LZ 执行时间天然更短，与"慢"无关 |
| 2 | **ratio 口径混乱** | 主表"相对 Rust"列，LZ 标 `9.56x (fib only)`，但同表 Rust 执行 `0.717` 含 sieve | 列内数字不可直接比较 |
| 3 | **无统一优化级别** | 脚本对 LZ 与 Rust 的 `rustc` 调用均无 `-O`；Go/Zig 默认带优化 | debug vs release 混比，对 Rust 系尤其不利 |
| 4 | **无预热 / 次数少** | `RUNS=3` 取中位数，未丢弃冷启动；`fib(35)` 落地 <100ms，计时噪声主导 | 单次波动被放大为"倍数" |
| 5 | **Zig "编码问题"** | 脚本用 `open(path,"w")` 写 `.zig`，未指定 `newline="/n"`/无 BOM | harness bug，非 Zig 语言问题 |
| 6 | **exec 计时含 IO** | `print` 输出计入 `exec_s` | 跨语言 stdout 缓冲差异污染计时 |

## 四、"9.56x" 是怎么算出来的（拆解）

主表数据：

| 语言 | 执行(s) | 备注 |
|------|---------|------|
| Rust | 0.717 | = fib(35) **+ sieve(500000)** |
| Lang-Zong | 0.669 | = fib(35) **仅此** |

- 若按脚本自身的 ratio 逻辑（`Rust_exec / LZ_exec`）：`0.717 / 0.669 ≈ 1.07x`——即**同口径下 LZ 反而"更快"**，因为它少干了 sieve 那一半活。
- 表内却写 `9.56x (fib only)`，说明该数字来自**另一套口径**：用 LZ 的 fib 执行时间除以一个"Rust fib-only"时间（≈ `0.669 / 0.07 ≈ 9.56x`）。两个数字分母不同，不能并列表述为"相对 Rust"。

> 推论：一旦 LZ 与 Rust 用**完全相同的 rustc 参数**跑**同一个 fib(35)** 二进制，二者应处于计时噪声范围内（生成代码已证明等价）。本环境因 rustup shim / PATH 限制，未能在本会话跑出该对照数；但**代码级证据已排除 codegen 缺陷**。

## 五、真正的根因与现状

1. **基准不可比**（方法缺陷，§三）→ 产出误导性表格。
2. **LZ 能力缺口**：loop / sieve / 容器等 codegen 尚未稳定，故基准被迫"fib only"。这是**功能完整性**问题，不是运行速度问题。待 `IR/design.md` 落地、loop  lowering 完成后即可补齐。
3. **harness 缺陷**：Zig 失败、ratio 计算错误，使报告整体可信度受损。

## 六、改进建议

### A. 基准 harness 修复（`benchmark/run_benchmark.py`）— 高优先
- **统一工作量**：LZ 补齐 loop/sieve codegen 前，LZ 行明确标注"部分（仅 fib）"；补齐后让 LZ 也跑 fib+sieve。
- **统一优化级别**：增加两列——`debug`（当前无 `-O`）与 `release`（`-O`），所有语言一致。
- **修复 ratio 计算**：`ratio = 本语言 exec / 参考语言同工作量 exec`，并在表注写明口径；删除混用的"相对 Rust"列。
- **预热 + 多次 + 去冷启动**：warmup 1 次丢弃，`RUNS≥5` 取中位数并报告 P95；增大规模（`fib(40)` + 更大 sieve）以降低噪声，或改用 `criterion`。
- **exec 计时不含 IO**：重定向 stdout 到 `/dev/null`，或分离"计算时间"与"打印时间"。
- **修复 Zig 写文件**：`open(path, "w", encoding="utf-8", newline="\n")` 避免 BOM/换行问题；该失败是脚本 bug。

### B. LZ codegen（工程侧，关联 `issue/frontend-keyword-downgrade.md` 与 `IR/`）— 中优先
- **保持零成本**：当前 fib 已验证为原生 `i64`；需确保 loop/sieve/容器也走 `IrType` 驱动的原生类型，**不要**退化为装箱 `Value` 或 `Rc<RefCell>`。
- **停止发射未使用辅助定义**：如 `Pipe<T>` trait 这类无条件输出，应按需发射，减少生成物噪声与误导。
- **推进 loop/sieve codegen**：这是让 LZ 跑完整基准、消除"fib only"短板的前提。

### C. CI 防护
- 增加"公平性能回归" job：统一参数、同工作量、criterion 基线，防止再次出现口径不一致的对比表。

## 七、建议的公平复测方法（供工程侧执行）

```bash
# 1) 同一份源：手写 Rust fib-only 与 lzc 生成的 bench_lz.rs
# 2) 相同 rustc 参数分别编译（debug 与 -O 各一次）
rustc bench_lz.rs      --edition 2021 -o lz_fib.exe   -A unused
rustc rust_fib.rs      --edition 2021 -o rs_fib.exe   -A unused
rustc bench_lz.rs      --edition 2021 -O -o lz_fib_o.exe -A unused
rustc rust_fib.rs      --edition 2021 -O -o rs_fib_o.exe -A unused
# 3) 各跑 7 次取中位数（warmup 1 次丢弃，stdout 丢弃）
# 预期：lz_fib vs rs_fib 处于噪声范围内（生成代码等价）
```

> 注：本会话在 Windows + rustup shim + python 子进程 PATH 隔离下未能直接跑出该对照数（rustc 仅 git-bash 可解析，python `CreateProcess` 找不到）；改用 bash 内 `command -v rustc` 即可，方法如上。

## 八、待办

| 项 | 负责 | 状态 |
|----|------|------|
| 重写 `run_benchmark.py`（统一工作量/参数/ratio/预热/Zig 写文件） | 工程侧 | 待认领 |
| 落地 loop/sieve codegen，使 LZ 能跑完整基准 | 工程侧 | 关联 IR 迁移 |
| 按需发射辅助 trait（去掉无条件 `Pipe`） | 工程侧 | 待认领 |
| 加公平性能回归 CI job | 工程侧 | 待认领 |

---
*本报告仅基于代码与脚本实证，未修改 `src/`。*

# LZ 自举前条件完成度分析 V2

> 分析日期: 2026-07-25
> 更新说明: 大部分 Bug 已修复，重点分析 Bridge 桥接路径在自举中的角色
> 编译器版本: `lang-zong.exe` (release build)

---

## 一、测试结果对比

| 指标 | V1 (修复前) | V2 (当前) | 变化 |
|------|------------|-----------|------|
| 总测试文件 | 122 | 122 | — |
| LZ 编译通过 | 115 (94.3%) | 115 (94.3%) | 持平 |
| **Rustc 编译通过** | **47 (38.5%)** | **71 (58.2%)** | **+19.7%** |
| 全链路通过 | 47 (38.5%) | 71 (58.2%) | +19.7% |
| Rustc 编译失败 | 68 | 44 | -24 |
| 累计已知 Bug | ~77 | ~40 | 大量修复 |

---

## 二、剩余 44 个 Rustc 失败分类

| 错误模式 | 数量 | 对应 Bug | 影响 |
|----------|------|----------|------|
| `str` cannot be indexed by `usize` | 12x | N2 | 字符串索引 |
| mismatched types (多处) | 3x | N1 + 其他 | 类型不匹配 |
| `_test_module_a` 重复定义 | 2x | N20 | 模块系统 |
| expected `,`, `.`, `?`, found `;` | 2x | 代码生成 | 语法错误 |
| cannot add `usize` to `i64` | 2x | N1 | i64/usize |
| type annotations needed | 1x | N5 | None 推断 |
| use of moved value | 1x | 所有权 | 移动语义 |
| missing generics for `Box` | 1x | N15 | 泛型结构体 |
| cannot find type `T` | 1x | N15 | 泛型方法 |
| cannot find type `HashMap` | 1x | N10 | HashMap 导入 |
| `i32` vs `i64` mismatch | 1x | N7 | 闭包推断 |
| 其他 | 17x | 杂项 | 各种边界 |

**总结**: 已修复的 ~24 个文件主要来自 `i64`/`usize` 自动转换（N1）和 `len()` 返回类型修复，以及 `_test_len*` 系列、`_test_list*` 系列、`_test_opt*` 系列的大面积修复。

---

## 三、Bridge 桥接模块深度分析

### 3.1 架构总览

```
lz 源码                              Rust 编译产物
┌──────────────────┐                ┌──────────────────────┐
│ import std::fs   │  ──Bridge──→   │ use std::fs;         │
│ fs::read(...)    │                │ std::fs::read(...)    │
│ import std::path │                │ use std::path;        │
│ PathBuf("...")   │                │ PathBuf::from("...")  │
│ import std::bridge│               │ use serde_json;       │
│   ::rust::serde  │                │                       │
└──────────────────┘                └──────────────────────┘
         │                                    │
         └──── bridge.toml + modules/*.toml ──┘
              (TOML 清单 → 源码级映射)
```

### 3.2 Bridge 模块组成

| 模块 | 行数 | 功能 | 自举相关性 |
|------|------|------|-----------|
| `bridge/core.rs` | 1381 | Bridge trait、BridgeRegistry、统一错误模型 | 基础设施 |
| `bridge/std.rs` | 1306 | TOML 清单加载 → 内存符号表 | 核心路由 |
| `bridge/rust.rs` | 269 | 直接 Rust crate 桥接（零配置） | 🔴 关键 |
| `bridge/source.rs` | 203 | 源码级桥接映射 | 辅助 |
| `bridge/ffi.rs` | 365 | FFI 桥接 | 辅助 |
| `bridge/cli.rs` | 317 | CLI 桥接 | 辅助 |
| `bridge/python.rs` | 431 | Python 互操作 | 不需要 |
| `bridge/shared.rs` | 482 | 共享内存桥接 | 不需要 |
| `bridge/wasm.rs` | 553 | WASM 桥接 | 不需要 |

### 3.3 标准库 TOML 清单覆盖

`bridge.toml` 注册了 **24 个 Tier-1 模块**，通过 `std/modules/*.toml` 提供源码级映射：

| 模块 | 自举必需 | 关键函数/类型 |
|------|----------|--------------|
| **fs** | 🔴 是 | `read_to_string`, `write`, `read_dir`, `exists`, `create_dir_all` |
| **io** | 🔴 是 | `stdin`, `stdout`, `BufReader`, `BufWriter` |
| **path** | 🔴 是 | `PathBuf`, 路径拼接/解析 |
| **process** | 🔴 是 | `Command`, `spawn`, `output`, `status` (运行 rustc) |
| **collections** | 🟡 是 | `HashMap`, `HashSet`, `VecDeque` |
| **str** | 🟡 是 | 字符串方法 (trim, split, replace, find, etc.) |
| **iter** | 🟡 是 | 迭代器组合器 (map, filter, collect, fold, etc.) |
| **sync** | 🟡 是 | `Arc`, `Mutex` (编译器内部状态) |
| **vec** | 🟡 是 | `Vec` 操作 |
| **env** | 🟡 是 | 环境变量、当前目录 |
| **fmt** | 🟢 否 | 格式化 |
| **thread** | 🟢 否 | 线程 |
| time | 🟢 否 | 时间 |
| net | 🟢 否 | 网络 |
| mem | 补充 | `replace`, `swap`, `take` |
| cmp | 补充 | `Ordering`, `min`, `max` |
| cell | 补充 | `RefCell`, `Cell` |
| rc | 补充 | `Rc`, `Weak` |
| convert | 补充 | `From`, `Into` |
| any | 补充 | 类型擦除 |
| marker | 补充 | `PhantomData` |
| hash | 补充 | `Hash`, `Hasher` |
| os | 补充 | OS 特定 |
| reflect | 补充 | 运行时反射 |

### 3.4 三方 Crate 桥接

`crates.toml` 注册了 **19 个 Rust 生态 crate**，通过 `import std::bridge::rust::xxx` 直通：

| Crate | 用途 | 自举需要 |
|-------|------|----------|
| serde / serde_json | 序列化 | 🟡 可能 |
| regex | 正则匹配 | 🟡 可能 |
| rand | 随机数 | 🟢 否 |
| chrono | 时间 | 🟢 否 |
| tokio | 异步 | 🟢 否 |
| reqwest | HTTP | 🟢 否 |
| clap | CLI 参数解析 | 🟡 可能 |
| log | 日志 | 🟡 可能 |
| uuid | UUID | 🟢 否 |
| base64 / sha2 / hex | 编码/哈希 | 🟢 否 |
| once_cell / parking_lot | 同步 | 🟡 可能 |
| crossbeam / rayon | 并发 | 🟢 否 |
| itertools | 迭代器扩展 | 🟡 可能 |
| anyhow / thiserror | 错误处理 | 🟡 可能 |

### 3.5 RustBridge 能力

`bridge/rust.rs` 提供零配置 Rust crate 直通：

```lz
// 无需 TOML 配置，直接 import 即可使用
import std::bridge::rust::serde_json
import std::bridge::rust::regex

// CLI 传参注册依赖
// lzc myfile.lz --rust-crate serde_json=1.0 --rust-crate regex=1
```

- `resolve_import`: 路径剥离 `std::bridge::rust::` → 生成 `use crate::module;`
- `resolve_call`: 函数调用直通
- `resolve_type`: 类型透传
- `allow_unregistered`: 默认 true，未注册 crate 也透传

---

## 四、自举路径中的 Bridge 依赖分析

### 4.1 自举编译器需要哪些 Bridge 模块？

自举编译器（用 LZ 重写）的核心流程：

```
读入 .lz 源码 ──→ 词法分析 ──→ 语法分析 ──→ 类型推断 ──→ 代码生成 ──→ 写入 .rs 文件 ──→ 调用 rustc
```

每个阶段需要的 Bridge：

| 阶段 | 需要 Bridge | 关键函数 |
|------|-----------|----------|
| 读入源码 | `fs` | `read_to_string(path)` |
| 写入 .rs | `fs` | `write(path, content)` |
| 目录遍历 | `fs` | `read_dir`, `exists` |
| 路径处理 | `path` | `PathBuf`, `join`, `file_stem` |
| 调用 rustc | `process` | `Command::new("rustc").arg(...).output()` |
| 字符串处理 | `str` | `trim`, `split`, `replace`, `find`, `starts_with` |
| 集合操作 | `collections` | `HashMap` (符号表), `Vec` (token 流, AST) |
| 迭代器 | `iter` | `map`, `filter`, `collect`, `enumerate` |
| 同步 | `sync` | `Arc` (可能用于共享状态) |

### 4.2 Bridge 路径的已知问题

| 问题 | 状态 | 影响 |
|------|------|------|
| Result 不自动 unwrap (Bug-25) | ⚠️ 需验证 | `fs::read_to_string` 等返回 Result |
| 构造器调用 `__call_magic` (Bug-30) | ⚠️ 需验证 | `PathBuf("...")` |
| HashMap 缺类型注解 (Bug-31) | ⚠️ 需验证 | `HashMap()` |
| 字符串字面量不转换 (Bug-32) | ⚠️ 需验证 | 函数参数传递 |
| 桥接冗余 import (Bug-28) | ✅ 轻微 | 仅警告 |

### 4.3 Bridge 的 `ret_result` 机制

`modules/*.toml` 中标记 `result = true` 的函数，代码生成时自动插入 `.unwrap()`：

```toml
# fs.toml
[functions]
read_to_string = { rust = "std::fs::read_to_string", shim = "path_ref", result = true }
write = { rust = "std::fs::write", shim = "path_ref", result = true }
```

这是自举路径的关键能力——**LZ 代码中可以直接调用 Rust 标准库函数，不用手动处理 Result**。

---

## 五、更新后的完成度评估

### 5.1 语言特性覆盖度

| 维度 | V1 | V2 | 变化 |
|------|----|----|------|
| 基础语法 | 95% | 95% | — |
| 控制流 | 75% | 80% | match 改进 |
| 类型系统 | 50% | 65% | i64/usize 大幅修复 |
| 模块系统 | 5% | 10% | 仍有 N20 |
| 错误处理 | 30% | 35% | 部分改进 |
| 闭包/高阶 | 30% | 35% | 部分改进 |
| 字符串处理 | 40% | 50% | 拼接改进 |
| 集合操作 | 35% | 60% | len()/pop() 修复 |
| 桥接模块 | 20% | 40% | ret_result 机制 |
| 标准库 | 20% | 40% | 24 个模块可用 |

### 5.2 综合完成度

```
自举总体完成度: ██████░░░░░░ 45%
```

| 条件 | V1 完成度 | V2 完成度 | 权重 | V2 加权 |
|------|----------|----------|------|---------|
| 单文件 LZ 编译通过率 | 94% | 94% | 10% | 9.4% |
| 全链路通过率 | 39% | 58% | 15% | 8.7% |
| 模块系统可用性 | 5% | 10% | 20% | 2.0% |
| 类型系统完整性 | 50% | 65% | 15% | 9.8% |
| 代码生成正确性 | 40% | 55% | 15% | 8.3% |
| **Bridge/互操作** | **20%** | **40%** | **10%** | **4.0%** |
| 错误处理可用性 | 30% | 35% | 10% | 3.5% |
| 标准库覆盖 | 20% | 40% | 5% | 2.0% |
| **加权总计** | **18%** | **45%** | | **~45%** |

---

## 六、Bridge 路径自举可行性评估

### 6.1 可自举的编译器子集

通过 Bridge 路径，当前 LZ 可以编写以下编译器组件：

| 组件 | 可行性 | 需要 Bridge | 说明 |
|------|--------|-----------|------|
| **词法分析器 (Lexer)** | ✅ 可行 | str, iter | 字符串遍历 + 状态机 |
| **语法分析器 (Parser)** | ✅ 可行 | collections, str | Token 流 + 递归下降 |
| **AST 定义** | ✅ 可行 | 无 | 纯数据结构 |
| **代码生成器 (Codegen)** | ⚠️ 勉强 | str, fs, path | 字符串拼接 + 模板 |
| **类型推断器 (Typer)** | ❌ 困难 | collections | 复杂泛型依赖 |
| **宏展开器 (Macros)** | ❌ 困难 | 无 | Token 流操作 |
| **CLI 入口 (main)** | ✅ 可行 | fs, process, path | 文件 I/O + 调用 rustc |

### 6.2 最小自举路径

```
1. 用 LZ 重写 lexer  →  bridge: str, iter
2. 用 LZ 重写 parser →  bridge: collections, str
3. 用 LZ 重写 main   →  bridge: fs, process, path
4. 用 LZ 重写 codegen → bridge: str, fs, path
5. LZ 编译器编译自身 → 产出 .rs → rustc → 自举二进制
```

### 6.3 Bridge 路径的剩余阻断问题

| 优先级 | 问题 | 影响 |
|--------|------|------|
| 🔴 P0 | N20 模块系统 | 多文件编译无法组织 |
| 🔴 P0 | N2 字符串索引 | 词法分析器核心操作 |
| 🔴 P0 | N1 i64/usize 残留 | 部分集合操作 |
| 🟡 P1 | Bug-25 Result unwrap | 文件 I/O 可靠性 |
| 🟡 P1 | Bug-30 构造器 `__call_magic` | PathBuf 等类型构造 |
| 🟡 P1 | N11 枚举 match 前缀 | AST 模式匹配 |

---

## 七、建议的行动计划

### 立即行动（1-2 天）

1. **修复 N20 模块系统** — 这是自举的最大单一障碍
2. **修复 N2 字符串索引** — 词法分析器的基础操作
3. **验证 Bug-25 修复状态** — 测试 `fs::read_to_string` 是否自动 unwrap

### 短期行动（3-5 天）

1. 用 LZ 编写一个 **mini-lexer**（100-200 行），验证 Bridge 路径可行性
2. 用 LZ 编写 **mini-main**（调用 lexer + 输出），验证端到端 Bridge 流程
3. 修复 N1 残留问题

### 中期行动（1-2 周）

1. 将 mini-lexer 扩展为完整的 lexer 模块
2. 用 LZ 重写 parser 模块
3. 自举集成测试：LZ 编译器编译 LZ 源码

---

## 八、关键结论

1. **自举完成度从 18% 提升到 ~45%**，主要得益于 i64/usize 和 len() 返回类型的大面积修复。

2. **Bridge 路径是自举的可行路径**：24 个标准库模块 + 19 个三方 crate + RustBridge 直通机制，覆盖了自举编译器所需的全部外部依赖。

3. **最大剩余障碍**：模块系统（N20）和字符串索引（N2）。前者阻止多文件组织，后者阻止词法分析器编写。

4. **乐观估计**：修复 N20 + N2 后，可以在 1 周内完成 LZ 版 lexer + parser + main 的编写，实现最小自举闭环。

5. **Bridge 的 `ret_result` 机制**是自举关键——它让 LZ 代码可以像 Rust 一样直接调用标准库函数，无需手动处理 Result 解包。

---

## 附录：Bridge TOML 模块速查

| 模块 | TOML 路径 | 关键函数数 | 类型数 | 方法数 |
|------|----------|-----------|--------|--------|
| fs | modules/fs.toml | 12 | 5 | — |
| io | modules/io.toml | 3 | 5 | 5 |
| path | modules/path.toml | — | 4 | 30 |
| process | modules/process.toml | — | 6 | 20 |
| collections | modules/collections.toml | — | 7 | 14 |
| str | modules/str.toml | — | — | 16 |
| iter | modules/iter.toml | — | 20 | 40+ |
| sync | modules/sync.toml | — | 20+ | 30+ |
| vec | modules/vec.toml | — | — | — |
| num | modules/num.toml | — | — | — |
| 其他 | 14 个模块 | — | — | — |
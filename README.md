# Lang-Zone (LZ) 编译器

LZ 是一门面向系统编程的静态类型语言：默认可变绑定、结构类型（duck typing）、魔法方法驱动的运算符重载、一等构建块语法、编译期宏与 comptime。本仓库是 LZ 编译器 `lzc`（LZ → Rust）与 `lzcyc`（LZ → Cython/Python）的实现。

> **路线决策（2026-07-31，2026-09-05 落实）**：全力走 **IR 中间表示** 路线。代码生成统一以 LZIR 为中间层（AST → LZIR → 目标语言），**不再使用 AST → Rust 直接 codegen 路线**；旧 `src/codegen/` 等路线 B 代码已于 2026-09-05 彻底移除，编译器仅剩唯一 IR 路线。

> **当前状态（2026-09-05 更新）**：路线 B 自举/替代线路（含 `src/codegen/` 的 AST→Rust 直接 codegen、`src/frontend/` 的 lex-lz/parse-lz、`src/ir/lz_codegen*` 等）已**彻底移除**。编译器默认管线**仅有唯一一条 IR 路线**：`AST → LZIR → Rust`（及 Cython 后端）。旧路线源码已备份至仓库外 `E:\IDEProjects\AI\_backup_langzone_routeB_20260905`，不再参与维护。

---

## 一、架构总览

```
        ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
 .lz →  │  L1 Lexer    │ →  │ L2 Parser    │ →  │ L3 语义/类型  │
        │ 词法分析      │     │ 语法分析/AST  │     │ typer/hints  │
        └──────────────┘     └──────────────┘     └──────┬───────┘
                                                          │
                                                          ▼
        ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
 .rs ←  │ L4 IR codegen│ ←  │ L3.5 LZIR    │ ←  │ 宏展开/魔法   │
        │ (Rust 后端)   │     │ build_ir     │     │ (已完成)      │
        └──────────────┘     └──────────────┘     └──────────────┘
 .pyx ← │ L4 IR codegen│
        │ (Cython 后端) │
        └──────────────┘
```

- **L1**：`src/lexer` — 缩进敏感词法分析（构建块符号留白规则等）
- **L2**：`src/parser` / `src/ast` — 递归下降解析、宏定义提取与展开
- **L3**：`src/typer` / `src/hints` / `src/magic` / `src/bridge` — 类型推断、约束求解、魔法方法注册、std 桥接
- **L3.5**：`src/ir` — **LZIR 中间表示**（本项目的主干）
- **L4**：`src/ir/codegen.rs`（Rust 后端）、`src/ir/codegen_cython.rs`（Cython 后端）

## 二、IR 路线现状（已实现）

LZIR 定义于 `src/ir/node.rs`，由 `src/ir/builder.rs` 的 `build_ir()` 从 AST 构建（携带类型信息 `IrType` 与源码 `Span`）。

| 模块 | 职责 | 状态 |
|------|------|------|
| `ir/node.rs` | IR AST：Item / Stmt / Expr / Pattern / IrType | ✅ 完整定义 |
| `ir/builder.rs` | AST → IR：`build_ir` + 类型上下文 `TypeCtx` | ✅ 覆盖主要语句/表达式 |
| `ir/codegen.rs` | IR → Rust：函数/struct/enum/trait/impl/语句/表达式 | ✅ 主要路径 |
| `ir/codegen_cython.rs` | IR → Cython（`.pyx`） | 🟡 基础框架 |
| `ir/display.rs` | IR 文本显示（`--emit=ir`） | ✅ |
| `ir/types.rs` | `IrType` 类型映射 | ✅ |

**语句覆盖**（`node.rs` `Stmt`）：`Let` / `Assign` / `Return` / `ExprStmt` / `If` / `For`（含 guard）/ `While`（含 guard）/ `Match` / `Raise` / `Assert` / `Yield` / `YieldFrom` / `Break` / `Continue` / `Defer` / `TryCatch` / `Block` / `Pass` / `TypeAlias` / `Test` / `Suite`（测试框架）

**表达式覆盖**（`node.rs` `ExprKind`）：字面量 / 变量 / 调用 / 方法调用 / 字段访问 / 下标读写 / 二元一元运算 / 三元 if / lambda / struct 构造（关键字参数）/ enum 构造 / 生成器 `*:` / 类型转换 / 魔法调用 / 管道 `|>` / 块表达式 / 元组 / 列表 / 字典 / Range

**魔法方法**：`MagicKind` 定义 19 类（`__call__` / `__iter__` / `__next__` / `__str__` / `__eq__` / 算术魔法等），映射到 `Callable` / `IntoIterator` / `Display` / `PartialEq` 等 trait。

**入口**：`lzc file.lz --emit=ir`（查看 IR）或 `--ir-codegen`（IR → Rust）；`tests/ir_snapshots.rs` 批量验证 DEMO 的 IR 生成。

## 三、路线迁移：AST → Rust 退役计划（✅ 已完成，2026-09-05）

**目标（已达成）**：`main.rs` 默认编译路径已从「AST 直接 codegen」切换为「AST → LZIR → Rust」（`build_ir` → `IrCodeGen`）；`src/codegen/`、`src/frontend/lz_lexer.*`、`src/frontend/lz_parser.*`、`src/ir/lz_codegen*` 等路线 B 代码已删除，旧 `tests/compile_demos.rs` 已弃用移入 `tests/deprecated/`。当前仅 IR 路线参与维护。

### 差距清单（已随路线 B 移除而作废）

路线 B 删除后，下表所列「IR 需补齐以切换」的对比已无意义——旧 `src/codegen/` 不复存在，IR 路线即为唯一实现。其中 `__call__` 魔法、顶层构建块、`defer`、装饰器等已在 IR 路线落地；`@export` / `std 桥接` / 嵌套函数提升等能力在 IR 路线暂无对应实现，列为已知未支持项（见 `SYNTAX/overview/缺失语法特性报告.md`）。

### 迁移步骤（历史记录）

1. **P0**：`main.rs` 默认路径切换为 `build_ir` + `IrCodeGen`（保留 `--ast-codegen` 回退开关用于对照）
2. **P0**：补齐 `__call__`、魔法 trait 生成、顶层构建块——决定 DEMO 可编译
3. **P1**：`defer`、桥接、可变参数
4. **P1**：`tests/compile_demos` 与 `tests/reject_errors` 全量切换 IR 路线跑绿
5. **P2**：删除 `src/codegen/`（保留 `--ast-codegen` 直到 0 依赖）；`src/ir/codegen.rs` 与 `codegen_cython.rs` 共享 IR 单源

> 上述步骤已全部完成：路线 B 代码已删除，默认管线即 IR-only。

## 四、双后端

| 特性 | lzc（IR → Rust） | lzcyc（IR → Cython） |
|:----|:----------------:|:--------------------:|
| 输出 | `.rs` → 原生二进制 | `.pyx` → `.pyd` |
| 目标 | 生产环境 | 自举 + 原型 |
| 所有权 | 编译期静态检查 | 运行时 `_MOVED` 哨兵 |
| 入口 | `cargo run -- file.lz` | `cd CY && cargo run --bin lzcyc -- <transpile\|compile\|run> file.lz` |

## 五、使用

```bash
# 编译为 .rs（IR 路线）
cargo run -- hello.lz --ir-codegen

# 查看 LZIR 中间表示
cargo run -- hello.lz --emit=ir

# 词法 / AST / 宏展开调试
cargo run -- hello.lz --tokens | --ast | --dump-macros

# 项目模式（import 依赖合并编译）
cargo run -- main.lz --project

# 用 LZ 自带测试框架运行某个 .lz 的 test/suite（IR 路线 --test）
cargo run -- demo.lz --test

# 编译器自身回归测试（Rust 驱动）
cargo test                          # lib + IR 快照 + 语义/错误边界 + 冒烟
cargo test --test ir_snapshots      # DEMO 的 LZIR 生成验证
cargo test --test lz_semantic_cases # 关键路径正例：.lz→编译→运行→断言 stdout
cargo test --test lz_test_smoke     # lz test 框架自身冒烟
```

## 六、测试体系

| 套件 | 内容 |
|------|------|
| `cargo test --lib` | 词法/解析/类型/IR 单元测试 |
| `tests/ir_snapshots.rs` | 全量 DEMO 的 LZIR 生成验证（IR 路线主测试） |
| `tests/lz_semantic_cases.rs` | 关键路径正例：`.lz`→编译→运行→断言 stdout（golden） |
| `tests/reject_errors.rs` / `reject_more.rs` | 99_errors/ 错误边界必须编译失败 |
| `tests/lz_test_smoke.rs` | 用 `lz test`（`--test`）跑含 test/suite 的 LZ 程序，验证框架本身 |
| `tests/fuzz_smoke.rs` / `find_bug_*.rs` | 烟雾与缺陷回归 |
| `tests/cython_backend.rs` | Cython 后端 |

> **为何编译器自检不用 `lz test` 写**：`lz test` 是给 LZ 用户程序的单元测试框架（只能 `assert` 运行时值）；编译器回归需要断言"编译应失败"（reject_errors）、"生成的 IR 文本"（ir_snapshots）、"运行 stdout"（lz_semantic_cases），这些 LZ 测试框架表达不了。此外 `lz test` 框架本身（suite 一等数据组合等）尚在完善，故先用 Rust 驱动 `tests/*.rs`，并额外用 `lz_test_smoke.rs` 反向保证 `lz test` 自身不回归。

## 七、目录速览

```
src/
  lexer/    L1 词法
  parser/   L2 语法
  ast/      L2 AST
  macros/   L2 宏展开
  typer/    L3 类型推断
  hints/    L3 约束求解
  magic/    L3 魔法方法
  bridge/   L3 std 桥接
  ir/       L3.5 LZIR（主线）← 全力投入
  codegen/  （已删除：路线 B AST→Rust 直接 codegen，2026-09-05）
CY/               lzcyc Cython 后端
DEMO/             演示与测试用例
SYNTAX/           语言规范文档
issue/            决策与问题追踪
```

---

*Lang-Zone 编译器 · IR 优先路线（路线 B 已于 2026-09-05 移除）· 2026-09-08*

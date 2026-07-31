# Lang-Zone (LZ) 编译器

LZ 是一门面向系统编程的静态类型语言：默认可变绑定、结构类型（duck typing）、魔法方法驱动的运算符重载、一等构建块语法、编译期宏与 comptime。本仓库是 LZ 编译器 `lzc`（LZ → Rust）与 `lzcyc`（LZ → Cython/Python）的实现。

> **路线决策（2026-07-31）**：全力走 **IR 中间表示** 路线。代码生成统一以 LZIR 为中间层（AST → LZIR → 目标语言），**不再使用 AST → Rust 直接 codegen 路线**（旧 `src/codegen/` 视为遗留，逐步退役）。

> **⚠️ 当前状态**：`lzc` 默认输出**仍是老路线（AST → Rust 直接 codegen）生成的代码**。老路线代码与产物已备份至 [`backup/lzc-legacy/`](backup/lzc-legacy/README.md)（含 `src/codegen/` 源码快照 + `lang-zone-legacy.exe`），**仅作 IR 路线代码生成的参考对照**；以后老路线会丢弃，不参与维护、不在其上新增功能。

---

## 一、架构总览

```
        ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
 .lz → │  L1 Lexer    │ →  │ L2 Parser    │ →  │ L3 语义/类型  │
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

**语句覆盖**（`node.rs` `Stmt`）：`Let` / `Assign` / `Return` / `ExprStmt` / `If` / `For`（含 guard）/ `While`（含 guard）/ `Match` / `Raise` / `Assert` / `Yield` / `YieldFrom` / `Break` / `Continue` / `Defer` / `TryCatch` / `Block` / `Pass` / `TypeAlias`

**表达式覆盖**（`node.rs` `ExprKind`）：字面量 / 变量 / 调用 / 方法调用 / 字段访问 / 下标读写 / 二元一元运算 / 三元 if / lambda / struct 构造（关键字参数）/ enum 构造 / 生成器 `*:` / 类型转换 / 魔法调用 / 管道 `|>` / 块表达式 / 元组 / 列表 / 字典 / Range

**魔法方法**：`MagicKind` 定义 19 类（`__call__` / `__iter__` / `__next__` / `__str__` / `__eq__` / 算术魔法等），映射到 `Callable` / `IntoIterator` / `Display` / `PartialEq` 等 trait。

**入口**：`lzc file.lz --emit=ir`（查看 IR）或 `--ir-codegen`（IR → Rust）；`tests/ir_snapshots.rs` 批量验证 DEMO 的 IR 生成。

## 三、路线迁移：AST → Rust 退役计划

**目标**：`main.rs` 默认编译路径从「AST 直接 codegen」（`CodeGen::generate`，旧 `src/codegen/`）切换为「AST → LZIR → Rust」（`build_ir` → `IrCodeGen`）；`src/codegen/` 停止维护并从 CLI 移除。

### 差距清单（IR 需补齐后即可切换）

| # | 能力 | AST 路线（旧） | IR 路线现状 | 动作 |
|---|------|----------------|-------------|------|
| 1 | 顶层构建块 `x =:` | `CodeGen` 读取 `module.top_level_builds` | builder 未消费 | IR builder 增加顶层构建块收集与生成 |
| 2 | 构建块 `~:` 调用/解包 | `builders.rs`（`in_build_call`、参数包解包） | 仅 `BlockExpr` 兜底 | 补齐调用构建块与 kwargs 拆包 |
| 3 | 魔法方法 trait 自动派生 | `magic.rs` / `derive.rs`（`MagicEngine`） | 仅 `MagicCall` 桩调用 | 生成 `impl Callable/Display/... for T` |
| 4 | `__call__` 直调 `ins()` | **未接线**（生成原样调用，rustc E0618） | `MagicKind::Call` 未接线 | 生成 `Callable::call` 或方法调用 |
| 5 | 可变参数 `..` / args/kwargs | `variadic.rs`（`pack_types`/`pack_names`） | 参数包未建模 | IR 增加 variadic 参数表示 |
| 6 | std 桥接 / 外部类型 | `bridge/`（`StdBridge`、tier2、rustc 私有 API） | 无 | 迁移桥接注册表到 IR 后端 |
| 7 | 导出 `@export(Rust/Python)` | `export.rs`（cdylib / PyO3） | 无 | 迁移 |
| 8 | 嵌套函数提升 | `collect_nested_fns` | 未见 | 迁移 |
| 9 | `defer` 生成 | 已实现 | 输出 `// defer` 注释 | 用 RAII guard 实现 |
| 10 | 装饰器（`@simd`/`@parallel`/`@tail_call`） | 部分实现 | 无 | 迁移 |

### 迁移步骤

1. **P0**：`main.rs` 默认路径切换为 `build_ir` + `IrCodeGen`（保留 `--ast-codegen` 回退开关用于对照）
2. **P0**：补齐差距 #4 `__call__`、#3 魔法 trait 生成、#1 顶层构建块——这三项直接决定 DEMO 是否可编译
3. **P1**：#9 defer、#6 桥接、#5 可变参数
4. **P1**：`tests/compile_demos` 与 `tests/reject_errors` 全量切换 IR 路线跑绿
5. **P2**：删除 `src/codegen/`（保留 `--ast-codegen` 直到 0 依赖）；`src/ir/codegen.rs` 与 `codegen_cython.rs` 共享 IR 单源

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

# 测试
cargo test                 # lib + demo 编译 + 错误拒绝 + IR 快照
cargo test --test compile_demos
cargo test --test ir_snapshots
```

## 六、测试体系

| 套件 | 内容 |
|------|------|
| `cargo test --lib` | 词法/解析/类型/IR 单元测试 |
| `tests/compile_demos.rs` | DEMO/ 全部 `.lz` 可编译 |
| `tests/reject_errors.rs` | 99_errors/ 错误边界拒绝 |
| `tests/ir_snapshots.rs` | 全量 DEMO 的 LZIR 生成验证（IR 路线主测试） |

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
  codegen/  L4 AST→Rust（遗留，计划退役）
CY/               lzcyc Cython 后端
DEMO/             演示与测试用例
SYNTAX/           语言规范文档
issue/            决策与问题追踪
```

---

*Lang-Zone 编译器 · IR 优先路线 · 2026-07-31*

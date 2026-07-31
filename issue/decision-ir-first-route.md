# 设计决策：全力走 IR 中间表示路线，退役 AST → Rust 直接 codegen

**日期**: 2026-07-31
**状态**: 已决策（架构方向，代码迁移待工程执行）
**提出**: 用户拍板「全力走 IR 路线，不要再使用 AST→RUST 代码路线了」

## 决策

代码生成**统一以 LZIR 为中间层**，管线固定为：

```
AST → build_ir → LZIR → (codegen.rs → Rust / codegen_cython.rs → Cython)
```

- **不再使用 AST → Rust 直接 codegen 路线**（旧 `src/codegen/` 视为遗留，逐步退役）
- `main.rs` 默认编译路径需从 `CodeGen::generate`（AST 路径）切换为 `build_ir` + `IrCodeGen`
- 未来所有新后端（WASM、其他目标）一律基于 LZIR，不另起 AST 直接生成

## 理由（实证）

1. **单一中间表示避免双路维护**。当前 `src/codegen/`（AST→Rust）与 `src/ir/`（AST→LZIR→Rust）并存，同一语法特性要在两条路径各实现一遍；`__call__` 直调实测即暴露此问题——AST 路径生成的 `ins(21)` 在 Rust 层报 E0618（未接线 Callable trait），而 IR 路径 `MagicKind::Call` 同样未接线，两条路各有一套逻辑，修复成本翻倍。
2. **LZIR 已具备主干能力**（`src/ir/node.rs` + `builder.rs` + `codegen.rs` + `codegen_cython.rs`）：
   - 语句覆盖 19 类：`Let`/`Assign`/`Return`/`ExprStmt`/`If`/`For`(guard)/`While`(guard)/`Match`/`Raise`/`Assert`/`Yield`/`YieldFrom`/`Break`/`Continue`/`Defer`/`TryCatch`/`Block`/`Pass`/`TypeAlias`
   - 表达式覆盖：字面量/变量/调用/方法调用/字段访问/下标读写/二元一元/三元 if/lambda/struct 构造/enum 构造/生成器 `*:`/类型转换/魔法调用/管道 `|>`/块表达式/元组/列表/字典/Range
   - 魔法方法 `MagicKind` 19 类（`__call__`/`__iter__`/`__str__`/`__eq__` 等）
   - 携带类型 `IrType` 与源码 `Span`，信息比 AST 直接生成更完整
3. **双后端共享 IR 单源**。`codegen_cython.rs` 已存在 IR → Cython 框架，集中到 IR 后 Rust/Cython 两后端只各写一份「IR → 目标语言」映射，语义逻辑（类型、魔法、作用域）不再重复。

## 现状盘点（IR 路线已就绪部分）

| 模块 | 职责 | 状态 |
|------|------|------|
| `src/ir/node.rs` | IR AST 定义 | ✅ 完整 |
| `src/ir/builder.rs` | AST → IR（`build_ir` + `TypeCtx`） | ✅ 主要语句/表达式 |
| `src/ir/codegen.rs` | IR → Rust | ✅ 主要路径 |
| `src/ir/codegen_cython.rs` | IR → Cython（`.pyx`） | 🟡 基础框架 |
| `src/ir/display.rs` | IR 文本显示（`--emit=ir`） | ✅ |
| `src/ir/types.rs` | `IrType` 映射 | ✅ |
| `tests/ir_snapshots.rs` | DEMO 全量 IR 生成验证 | ✅ |

## 差距清单（迁移前需补齐）

| # | 能力 | AST 路线现状（旧） | IR 路线现状 | 动作 |
|---|------|-------------------|-------------|------|
| 1 | 顶层构建块 `x =:` | `CodeGen` 读取 `module.top_level_builds` | builder 未消费 | IR builder 增加顶层构建块收集与生成 |
| 2 | 构建块 `~:` 调用/解包 | `src/codegen/builders.rs`（`in_build_call`、参数包解包） | 仅 `BlockExpr` 兜底 | 补齐调用构建块与 kwargs 拆包 |
| 3 | 魔法方法 trait 自动派生 | `src/codegen/magic.rs` + `derive.rs`（`MagicEngine`） | 仅 `MagicCall` 桩调用 | 生成 `impl Callable/Display/... for T` |
| 4 | `__call__` 直调 `ins()` | 已实现但实测生成坏代码（rustc E0618） | `MagicKind::Call` 未接线 | 接线 `Callable::call` 或方法调用 |
| 5 | 可变参数 `..` / args/kwargs | `src/codegen/variadic.rs`（`pack_types`/`pack_names`） | 参数包未建模 | IR 增加 variadic 参数表示 |
| 6 | std 桥接 / 外部类型 | `src/bridge/`（`StdBridge`、tier2、rustc 私有 API） | 无 | 迁移桥接注册表到 IR 后端 |
| 7 | 导出 `@export(Rust/Python)` | `src/codegen/export.rs`（cdylib / PyO3） | 无 | 迁移 |
| 8 | 嵌套函数提升 | `collect_nested_fns` | 未见 | 迁移 |
| 9 | `defer` 生成 | 已实现 | 输出 `// defer` 注释 | 用 RAII guard 实现 |
| 10 | 装饰器（`@simd`/`@parallel`/`@tail_call`） | 部分实现 | 无 | 迁移 |

## 迁移步骤

1. **P0**：`main.rs` 默认路径切换为 `build_ir` + `IrCodeGen`（保留 `--ast-codegen` 回退开关用于对照回归）
2. **P0**：补齐差距 #4 `__call__` 接线、#3 魔法 trait 生成、#1 顶层构建块——这三项直接决定 DEMO 能否在 IR 路线编译通过
3. **P1**：#9 defer、#6 桥接、#5 可变参数、#7 导出、#8 嵌套函数
4. **P1**：`tests/compile_demos` 与 `tests/reject_errors` 全量切换 IR 路线跑绿（含 99_spec）
5. **P2**：删除 `src/codegen/`（`--ast-codegen` 保留到 0 依赖）；`codegen.rs` 与 `codegen_cython.rs` 共享 IR 单源

## 验证

- 切换后 `cargo test` 全绿（lib + compile_demos + reject_errors + ir_snapshots）
- DEMO 生成的 `.rs` 通过 rustc 编译（以 `__call__` 直调 E0618 修复为标志性验收）
- `--ast-codegen` 与默认 IR 路径对同一 DEMO 产出逐行 diff 对照（可选，回归期）

## 相关

- 路线说明同步写入根目录 `README.md`（「IR 优先路线」节）
- `__call__` 实测结论见 DEMO 文件 `DEMO/07_data_structures/callable_objects.lz`（2026-07-31 测试记录）

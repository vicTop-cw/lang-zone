# LZIR Migration Roadmap — 前端 IR 后端 Codegen 完整计划

> **决策**：后续全走 IR。旧直连路径在过渡期保留，过渡完成后归档下线。
>
> **状态**：🟢 Phase 0 已完成 + Phase 1 进行中。文档同步于 2026-07-30 13:55 验证。
>
> **核心矛盾**：IR builder 已通过所有测试，但 IR codegen 产出的 Rust 代码有语义问题（如发射不稳定 `yield` 关键字）。管线已接但还没对。

---

## 目标管线

```
前端（只实现一次）
.lz → Lexer → Parser/Expand → AST
                                 ↓
                            [build_ir]  ← AST→LZIR 转换
                                 ↓
                            LZIR Module
                                 ↓
                    ┌────────────┴────────────┐
                    ↓                         ↓
          IR CodeGen (Rust)          IR CodeGen (Cython)
                    ↓                         ↓
                 hello.rs                hello.pyx
```

---

## 当前验证状态（2026-07-30 13:55）

| 组件 | 状态 | 证据 |
|------|:----:|------|
| **IR builder** — if/else | ✅ Fixed | `ir_if_else` test passes (was `#[ignore]`) |
| **IR builder** — struct def | ✅ Fixed | `ir_struct_def` test passes (was `#[ignore]`) |
| **IR codegen** — 接入 CLI | ✅ Done | `--emit=ir` / `--ir-codegen` 标志可用 |
| **IR codegen** — 输出 .rs | ✅ Works | 命令行执行成功，产出 .rs 文件 |
| **IR codegen** — 语义正确性 | ❌ **WRONG** | 产出含 `yield i;`（不稳定 Rust 关键字），旧 codegen 产出 `vec![i].next()` |
| **IR codegen (Cython)** — 模块存在 | ✅ 210行 | 但**未接入 CLI** |
| **IR builder** — 完整度 | 🟡 部分 | 仍有 `convert_ast_pattern` 等函数标记为 never-used (dead code) |
| **旧 codegen** — 运行状态 | ✅ 默认 | `cargo test --test compile_demos` 77/77 通过 |
| `TypeCtx` Clone 缺失 | ⚠️ 警告 | builder.rs:489 有 `ctx.clone()` 但 TypeCtx 没有 `Clone` trait |

---

## 阶段 0（已完成 ✅）

**入口条件**：builder 中有 `#[ignore]` 测试。

**实际结果**：
- `ir_if_else` — 从 `#[ignore]` 到通过 ✅
- `ir_struct_def` — 从 `#[ignore]` 到通过 ✅
- IR 单测总计：6 passed / 0 failed / 0 ignored
- CLI 已接入：`--ir-codegen` 和 `--emit=ir` 均可触发 IR 管线
- 死代码清理：若干 `#[allow(dead_code)]` 残留（`convert_ast_pattern`、`begin_fn`、`emit`），非阻塞性

**退出条件已验证**：`cargo test --lib ir_*` → 0 ignore。`cargo build` 有 43 warnings（历史清理遗留，非 IR 阻塞）。

---

## 阶段 1（进行中 🟡 — 最大工作量所在）

### 为什么不是"diff 归零"这么简单

IR codegen 当前能跑、能出 .rs，但它输出的是**语义错误的 Rust 代码**：

```rust
// IR codegen 产出（❌ yield 是 nightly/unstable 关键字）
for mut i in 0..n {
    yield i;          // ← 编译器不接受
}

// 旧 codegen 产出（✅ 稳定可编译）
for i in Range { start: 0, end: n } {
    vec![i].next()    // ← 模拟生成器
}
```

所以阶段 1 的真正任务是**重写 IR codegen 的语义发射层**，让 IR→Rust 的降低在以下几个关键点对齐旧 codegen：

### 子任务清单

| 优先级 | 任务 | 说明 | 涉及文件 |
|--------|------|------|---------|
| **P0** | 生成器降低 — `yield` 不能直接发 Rust' 的 `yield` | 必须使用状态机 / Iter 包装 / `vec![].next()` 等模拟 | `ir/codegen.rs` |
| **P0** | for 循环语法对齐 | 旧 codegen 用 `Range { start, end }`，IR codegen 用 `0..n` — 两者都可以但不是一回事，选一个 | 同上 |
| **P1** | `Pipe` trait 发射 | 旧 codegen 无条件注入 `Pipe<T>`，IR codegen 没发这个（小超集） | 同上 |
| **P1** | `#![allow(...)]` prelude | 旧 codegen 有 3 行 `#![allow(...)]`，IR codegen 应该也有 | 同上 |
| **P1** | `use std::collections` | IR codegen 缺 `HashMap`/`HashSet` 导入 | 同上 |
| **P1** | 类型映射检查 | `List<T>` → `Vec<T>` 等映射是否正确 | `ir/codegen.rs` `rust_type` |
| **P2** | 构建块/魔法方法 | build blocks (`=:`/`~:`/`*:`) 和 magic methods (`__getitem__` 等) 的 IR→Rust 降低 | 同上 |
| **P2** | 模板/可变参数 | template / variadic 的边缘情况 | 同上 |

### 验证方法（修正版）

```bash
# 1. 先确保 parse 通过（77 demo 全绿）
cargo test --test compile_demos

# 2. 再跑 IR codegen + rustc
for demo in DEMO/**/*.lz; do
    cargo run -- "$demo" --ir-codegen 2>/dev/null
    rustc --edition 2021 "${demo%.lz}.rs" -o /dev/null 2>&1 && echo "✅ $demo" || echo "❌ $demo"
    git checkout -- "${demo%.lz}.rs"  # 恢复
done

# 3. 再用 diff 逐 demo 对比产出（行级 diff，非语义等价）
diff <(cargo run -- "$demo" 2>/dev/null) <(cargo run -- "$demo" --ir-codegen 2>/dev/null)
```

### 退出条件
- `rustc --edition 2021` 能编译 IR codegen 产出的所有 .rs（<=77 demo）
- 至少 5 个含 struct/if/for/魔法方法的复杂 demo 运行输出与旧路一致

---

## 阶段 2（规划中 ⚪）

### 子任务清单

| 优先级 | 任务 | 说明 |
|--------|------|------|
| P0 | 默认路径切为 IR codegen | `main.rs` 默认调用 `ir::codegen::CodeGen` |
| P0 | 旧 `--ir-codegen` 改为 `--emit=legacy-codegen`（可选回退） | 保留回退能力 |
| P1 | `src/codegen/` 整目录移入 `_bak/codegen-legacy/` | 不删，归档 |
| P2 | `src/ir/codegen.rs` 按旧 codegen 模块化拆分 | 拆为 `expr.rs` `stmt.rs` `func.rs` `magic.rs` `decl.rs` |

> **注意**：阶段 2 只能在"IR codegen 产出与旧路逐 demo 一致且 rustc 通过"之后启动。现在阶段 1 还没到一半。

---

## 额外清理（并行进行，非阻塞）

| 任务 | 原因 |
|------|------|
| `ir/builder.rs` — `TypeCtx` 加 `#[derive(Clone)]` | builder.rs:489 有 `ctx.clone()` 但缺 Clone |
| `ir/builder.rs` — 删 `begin_fn`/`convert_ast_pattern`（或改为 pub 真正使用） | 当前未用，编译警告 |
| `ir/codegen.rs` — 删 `emit` 或改为 `pub` | 当前未用 |
| 旧 `src/codegen/` 各路 `unused_import` 清理 | ~20 处 unused import 警告（因 IR codegen 接入后部分引用不可达） |

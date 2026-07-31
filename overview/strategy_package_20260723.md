# 策略包 (Strategy Package) — 架构设计与端到端验证

**日期**: 2026-07-23 · **范围**: lang-zone (LZ → Rust 转译器) 魔法方法体系扩展
**参考**: Python 版 vools 库（`stuff` 延迟执行、`Itor` 可控迭代器、`history_strategy` 决策回调）

---

## 1. 目标

从全局架构角度统一抽象「策略」——每个策略本质是一个**被延后的决策/选择**，接口同构、优先级清晰，并与 vools 的设计理念保持一致。具体落地三件事：

1. **核心机制** `stuff` + `itor`：延迟执行原语 + 可控迭代器
2. **for-in 集成**：`itor` 接入 for-in，经 `__iter_strategy__` 魔法方法按优先级自动选策略
3. **策略体系**：clone / 析构 / 序列化 / 迭代 四类策略统一为一个策略包

---

## 2. 架构设计

### 2.1 统一抽象（代码生成于 `STRATEGY_PKG` const，`src/codegen/mod.rs`）

| 组件 | 职责 |
|------|------|
| `Strategy` 超 trait | `type Host` — 所有策略共享「为哪类宿主做决策」 |
| `CloneStrategy` / `DestroyStrategy` / `SerializeStrategy` / `IterStrategy` | 四个**同构**子 trait，各自一个 `decide` 式方法（`clone_of` / `destroy` / `encode`+`decode` / `iterate`） |
| `IterStrategyKind` | 内置迭代策略枚举：`Forward` / `Reverse` / `Strided(i64)` / `Controlled`，**优先级由列表顺序决定**（越靠前越高） |
| `__iter_resolve<T,B>(plan, base)` | 优先级解析器：按列表取首个适用策略包裹 base 迭代器 |
| `Itor<T>` + `itor()` / `__itor_from()` | 可控迭代器（pause/resume/stop）；实现 `Iterator` 后自动获得 blanket `IntoIterator`，可直接用于 for-in |
| `Stuff<R>` + `stuff()` | 延迟执行原语：闭包延后到 `run()` 才求值（「一个策略 = 一个被延后的决策」） |

### 2.2 for-in 集成（魔法方法）

- `src/magic/engine.rs`：新增 `MagicKind::IterStrategy`，注册 `__iter_strategy__` → `IntoIterator::into_iter`
- `src/codegen/magic.rs`：`IterStrategy` 臂生成
  ```rust
  impl std::iter::IntoIterator for MyColl {
      type Item = i64;
      type IntoIter = Box<dyn Iterator<Item = i64>>;
      fn into_iter(self) -> Self::IntoIter {
          let __plan = self.__iter_strategy__();   // [IterStrategyKind]
          let __base = self.__iter__();             // base 迭代器
          __iter_resolve(__plan, __base)            // 按优先级包裹
      }
  }
  ```
- **跳过守卫**：存在 `__iter_strategy__` 时，跳过 `__iter__`/`__into_iter__`/`__next__` 的默认魔法 trait impl，避免冲突
- **触发条件**：定义 `__iter_strategy__` 魔法方法，或显式使用 `stuff`/`itor`/`Itor`/`IterStrategyKind`/`Strategy` 符号（`scan_has_magic` + `scan_uses_symbols` 按需注入 prelude）

### 2.3 设计对齐 vools

- `stuff` ≈ vools `stuff`（延迟/分段求值，无参 `()` 触发执行）
- `Itor` ≈ vools `Itor`（Node 链表 + 状态机 + 可控暂停/恢复/停止）
- `__iter_resolve` 的「策略回调」注入点 ≈ vools `history_strategy`（决策回调注入行为钩子）

---

## 3. 文件变更

| 文件 | 变更 |
|------|------|
| `src/codegen/mod.rs` | 新增 `STRATEGY_PKG` const（策略包运行时）；`scan_uses_symbols` / prelude 触发逻辑 |
| `src/codegen/magic.rs` | 新增 `IterStrategy` 魔法臂 + 跳过守卫；`magic_iter_item_type` 提取 `List<T>`/`Option<T>` 的 Item |
| `src/magic/engine.rs` | 注册 `MagicKind::IterStrategy` + `__iter_strategy__` |
| `src/hints/unify.rs` | 修复 `Box<Type>` 重构残留（预存阻断项，已顺带修复以解锁构建） |
| `test-suite/20260723-01/run_tests.py` | 新增 E2E 用例 M08–M11 |

---

## 4. 验证结果

`cargo build` 全绿；E2E 套件 **80/80 全部通过**（含原有 76 例）。

| 用例 | 模式 | 验证点 | 结果 |
|------|------|--------|------|
| **M08** | run | `__iter_strategy__ → [Controlled]` → for-in 经 `itor` 遍历求和 | `15` ✅ |
| **M09** | run | `__iter_strategy__ → [Reverse]` → for-in 逆序（顺序敏感 `s=s*10+x`） | `54321` ✅ |
| **M10** | rust | 策略包 prelude 生成（`Strategy`/`IterStrategyKind`）+ `IntoIterator` + `__iter_resolve` 调用；不拖入 BuildParams 的 `__Pack` | ✅ |
| **M11** | run | `stuff(| | 6*7).run()` = `42`；`itor([1..4])` 经 for-in 求和 = `10` | `42` ✅ |

---

## 5. 关键修复（排错记录）

`__iter_resolve` 的泛型定义中 `base.rev()` / `base.step_by()` 要求 `B::IntoIter: DoubleEndedIterator`，该约束不在 `where` 子句中。当策略包 prelude 被包含但 `__iter_resolve` **仅定义、不被调用**时（如 M11 只用 `stuff`/`itor`），rustc 报 `E0277`。

**修复**：`Reverse`/`Strided` 分支先 `collect` 为 `Vec<T>` 再 `rev()`/`step_by()`（`Forward`/`Controlled` 保持惰性）。对任意 `B` 均可编译，M08/M09/M11 全绿。

---

## 6. 已知遗留（非本特性范围）

`cargo test --lib` 仍有 55 个测试模块编译错误——bridge 重构后 `#[cfg(test)]` 残留（`resolve_call` 等调用签名不匹配），与策略包无关。魔法方法的权威验证路径为 E2E 套件（`test-suite/20260723-01/run_tests.py`），已全绿。

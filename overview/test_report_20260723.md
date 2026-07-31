# Lang-Zone 测试报告 — 2026-07-23 11:10

## 架构变更后的全面回归测试

### 执行结果

| 测试套件 | 用例数 | 通过 | 失败 | 崩溃 | 通过率 |
|----------|--------|------|------|------|--------|
| 功能套件 #1 (20260722-01) | 39 | **39** | **0** | 0 | **100%** |
| 扩展套件 #2 (20260722-02) | 51 | **51** | **0** | 0 | **100%** |
| Rust 单元测试 (cargo test) | 144 | **144** | **0** | 0 | **100%** |
| **总计** | **234** | **234** | **0** | **0** | **100%** |

**所有 234 个用例全部通过，零失败、零崩溃。**

---

### 发现的架构变更引起的回归 & 修复记录

架构重构后（扁平文件 → `lexer/`、`parser/`、`codegen/` 模块化），编译器管线有 3 处回归，已全部修复：

#### 🟢 修复 1: `parse_primary` 中 `Panic`/`Await` 双重 `advance()`

- **症状**：`panic("oops")` → `Parse error: Expected LParen, got StrLit`（所有含 panic 的用例失败：H01/H02/H08）
- **根因**：`parse_primary()` 入口已通过 `let tok = self.advance()` 消费了当前 token，但 `Token::Panic` 和 `Token::Await` 分支体**再次调用 `self.advance()`**，跳过了 `(`，导致后续解析错位
- **修复**：删除 `src/parser/expr.rs:713` 和 `:720` 的冗余 `self.advance()`（对比 `Try`/`Spawn`/`If` 等正确分支）
- **影响用例**：H01, H02, H08 ✅ 修复

#### 🟢 修复 2: `match` 分支循环缺 `skip_newlines()`

- **症状**：`match x:\n  case 0: "zero"\n  case _: "other"` → `Parse error: Expected Dedent, got Newline`
- **根因**：`match` 臂解析的 `while` 循环（`expr.rs:590`）在每臂处理完毕后，未调用 `self.skip_newlines()` 消费分支间的换行，导致 `while` 条件检查时遇到 Newline 而非 Case，提前退出
- **修复**：在 `expr.rs:621` 的 `for pat in patterns` 块后追加 `self.skip_newlines()`
- **影响用例**：F08 ✅ 修复

#### 🟢 修复 3: `catch` 模式检测未覆盖内置变体 token

- **症状**：`catch Err(msg):` → `Parse error: Expected Colon, got LParen`（模式被降级为 Wildcard）
- **根因**：`catch` 的模式检测条件（`expr.rs:660-663`）仅检查 `Token::Ident`，未覆盖 `Token::Some_/Ok_/Err_/None_/Self_` 这些**内置变体 token**，导致 `Err_` 不被识别为模式起点
- **修复**：在条件中追加对 `Some_/Ok_/Err_/None_/Self_` 的匹配
- **影响用例**：H10 ✅ 修复

#### 🟢 修复 4: 测试用例 `case 0=>` 语法过时

- **症状**：`case 0=> "zero"`（FatArrow）→ `Parse error: Expected Colon, got FatArrow`
- **根因**：新架构将 match arm 分隔符从 `=>`（FatArrow）改回 `:`（Colon），`case 0: "zero"`
- **修复**：更新 `run_tests.py` 中 F08 的测试源
- **影响用例**：F08（两套件均修复）✅

---

### 架构变更概述

| 旧结构 | 新结构 |
|--------|--------|
| `src/token.rs` (扁平, 30000B) | `src/lexer/token.rs` + `src/lexer/lexer.rs` |
| `src/parser.rs` (扁平, 94100B) | `src/parser/ast.rs` + `src/parser/expr.rs` + `src/parser/stmt.rs` + `src/parser/helpers.rs` + `src/parser/mod.rs` |
| `src/codegen.rs` (扁平, 91337B) | `src/codegen/mod.rs` + `src/codegen/expr.rs` + `src/codegen/stmt.rs` + `src/codegen/builders.rs` |
| 无桥接模块 | 新增 `src/bridge_core.rs`, `bridge_ffi.rs`, `bridge_cli.rs`, `bridge_source.rs` |
| 无类型系统 | 新增 `src/type_system.rs` |
| 无魔法方法 | 新增 `src/magic_trait.rs` |

**单元测试增长**: 120 个(首次架构检查) → 144 个(当前)，新增 24 个来自新模块。

### 已知问题（非本次回归）

- `tests_boundary/` 下有 133 个 `.lz` 测试源但有 4 个预存已知缺陷（悬空`^`、`Vec<i64>` 无 Display 等），需单独修复计划
- `FfiBridge` 测试共享 temp 文件导致的并行竞态（串行无问题）

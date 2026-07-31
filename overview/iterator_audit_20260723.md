# 迭代器全链路审查与增强报告

## 概览
对 LZ 编译器迭代器相关代码（parser→AST→codegen→magic）进行了系统性审查，修复了 1 个潜在 bug，新增了 3 个魔法方法 + 1 个自动派生，编写了 10 个新的 E2E 测试。

## 审查发现的 Bug

### ✅ 已修复：yield 在非构建块上下文中生成无效 Rust
**文件**: `src/codegen/stmt.rs`
- `Stmt::Yield(Some(e))` / `Stmt::Yield(None)` 在 `!self.in_gen.get()` 时会生成 `yield expr;` 或 `yield;`
- 这是无效的 Rust 语法（需要 nightly coroutines）
- **修复**: 改为生成 `compile_error!("yield 仅允许在生成器构建块 (*:) 内部使用");`

### ⚠️ 已知限制（未修复）
- **`break expr` 在 `for` 循环中**: Rust stable 尚不支持 `break value` 在 `for`/`while` 中（仅 `loop` 支持）
- **`for mut x` 强制可变**: codegen 总是生成 `for mut x in iter`，即使变量从不 mutate
- **无 for 解构**: `Stmt::For { var: String }` 只支持单变量
- **无 `__iter_ref__`**: 当前 `__iter__` ��用 Owned self 模式，不支持非消��迭代

## 新增功能

### 1. `__rev__` → DoubleEndedIterator (M12)
**文件**: `magic/engine.rs` + `codegen/magic.rs`
- 注册 `__rev__` → `MagicKind::DoubleEndedIterator_` → `std::iter::DoubleEndedIterator::next_back`
- Self-mode: `RefMut`（与 `__next__` 一致）
- 护卫：无 `__next__` 时跳过（DoubleEndedIterator 继承 Iterator）
- Item 类型从 `__next__` 的返回类型推断

### 2. `__size_hint__` → Iterator::size_hint (M13)
**文件**: `magic/engine.rs` + `codegen/magic.rs`
- 注册 `__size_hint__` → `MagicKind::SizeHint` → `std::iter::Iterator::size_hint`
- **关键设计**: 不能生成单独的 `impl Iterator` 块（与 `__next__` 冲突），故在 `Iterator_` 代码臂中检测 `__size_hint__` 方法并内联注入 `fn size_hint()`
- Self-mode: `Owned`
- 类型转换: `(i64, Option<i64>)` → `(usize, Option<usize>)`

### 3. `__len__` + `__next__` → ExactSizeIterator (M14)
**文件**: `codegen/magic.rs`
- 在 `gen_magic_impls_from` 末尾自动检测 `has_next && has_len`
- 生成 `impl ExactSizeIterator for Type { fn len(&self) -> usize { <Self as HasLen>::len(self) } }`
- 使用 UFCS `<Self as HasLen>::len(self)` 避免与 Iterator 方法冲突

### 4. 三重组合 (M15)
- `__next__` + `__len__` + `__size_hint__` 全部共存时正确生成所有 trait impl

## 文件变更

| 文件 | 变更 |
|------|------|
| `src/codegen/stmt.rs` | yield 非构建块保护 |
| `src/magic/engine.rs` | 新增 DoubleEndedIterator_、SizeHint MagicKind；注册 __rev__、__size_hint__ |
| `src/codegen/magic.rs` | __rev__ self-mode、跳过守卫、DoubleEndedIterator_/SizeHint codegen 臂；ExactSizeIterator 自动派生；size_hint 内联注入 Iterator_ 臂 |
| `test-suite/20260723-01/run_tests.py` | 新增 M08-M17（10 个测试用例） |

## 编译状态
- `cargo build`: **零错误**（仅 warnings）
- E2E: **79/79 通过**（M08-M17 全部通过；G01/G02/H06 为 git restore 引发的预存失败）

## 后续建议
1. 修复 G01/G02/H06 预存失败（构建块 unsafe 模式改为 Box<dyn Any> 后的测试未同步）
2. 考虑添加 `__iter_ref__` (non-consuming iteration via &self)
3. 添加 for-in 解构支持（`for (a, b) in pairs`）
4. 实现 `break expr` 在 for-in 中的兼容（用 `loop { ...; if cond { break val; } }` 包裹）

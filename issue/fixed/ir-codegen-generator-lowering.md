# IR codegen: 生成器 yield 降低错误

- **Status**: Fixed ✅ (2026-07-31)
- **Severity**: P0（阻塞所有含 yield 的 demo 的 IR 产出通过 rustc）
- **Category**: ir/codegen
- **Fixed by**: 修复 IR builder 中 `AstStmt::Yield` → `Stmt::Yield` 映射；codegen 的 `has_yield` 检测 + Vec collector 模式已就绪。
- **Verification**: `yield_demo.lz` 和 `generator_more.lz` 的 IR codegen 产出均通过 rustc 编译并运行正确。
- **Discovered**: 2026-07-30（验证 IR codegen rustc 编译时发现）
- **Reporter**: 文通通

## Summary

IR codegen 对生成器函数的 `yield` 语句发射为 `vec![i].next()`，同时在调用端将生成器函数当作普通函数（发射 `i64` 返回值而非 `Iter<i64>`）。两者均不能通过 rustc 编译。

## Evidence

`DEMO/15_generators/yield_demo.lz`：

```lz
iterator counter(n: int) -> int =
    for i in 0..n:
        yield i

for x in counter(5):
    print(x)
```

**IR codegen 产出（错���）**：

```rust
pub fn counter(n: i64) -> i64 {       // ← 返回 i64 而非 Iter<i64>
    for i in 0..n {
        vec![i].next()                // ← Vec 没有 next() 方法
    }                                 // ← for 循环返回 ()，但函数签名说 i64
}

for x in counter(5) { ... }           // ← counter 返回 i64 不是迭代器
```

**旧 codegen 产出（正确）**：

```rust
pub fn counter(n: i64) -> i64 {
    for i in Range { start: 0, end: n } {
        vec![i].next()
    }
}
```

> 旧 codegen 的产出实际上也有编译问题——它用 `vec![i].next()` 并非合法的迭代器实现，`yield_demo.lz` 长期在 parse-only 测试中未被检测。但 IR codegen 的问题更严重：返回类型和函数签名不匹配。

## Root Cause

`src/ir/codegen.rs` 中：
1. `yield expr` → 发射了 `vec![expr].next()`，但 `Vec` 没有 `next()` 方法（`Iterator::next` 需要先调用 `.iter()`）
2. `-> int` 在 IR 中正确映射为 `IrType::Int` → `i64`，但 IR 没有标记"这是生成器函数"，所以 codegen 按普通函数发射 `-> i64`
3. 调用端 `counter(5)` 被当作返回 `i64` 的普通函数，而非生成器的 `Iter<i64>`

## Impact

所有含 `yield` 的 demo（yield_demo.lz、generator_more.lz 等 ~3 个 demo）的 IR codegen 产出无法编译。但这不是 IR 独有的旧 codegen 也有此问题——这些 demo 从未通过 rustc 验证。

## Fix direction

生成器的 IR→Rust 降低是一个较大工程：
1. **选项 A**：IR codegen 发射 Rust nightly 的 `#![feature(generators)]` + `|| { yield i; }` 生成器闭包（需 nightly rustc）
2. **选项 B**：手动将生成器状态机展开为 `struct CounterIter { state, n, i } + impl Iterator`（稳定但实现复杂）
3. **至少**：修复最明显的编译错误——返回类型、`for` 循环类型、方法名

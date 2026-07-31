# parser 不支持 `for i in 0..5:` 区间迭代器

- **Status**: Fixed ✅ (2026-07-29)
- **Severity**: P0（影响 6 个既有 demo，全红）
- **Category**: parser
- **Discovered**: 2026-07-29（组合式语法审计）
- **Reporter**: 文通通
- **Owner**: engineering

## Summary

`for` / `while` 迭代器位置不支持 `..` 区间表达式。写 `for i in 0..5:` 时 parser 报 `Expected Colon, got DotDot`。
注意：comprehension 里的 `[x for x in 1..10]` 走另一条路径可以解析，故**仅 `for` 语句的迭代器位置**不支持。

## Evidence

- 复现：`for i in 0..5: print(i)` → 解析失败。
- 受影响 demo（既有红 demo，共 6 个）：
  - `01_basics/keywords.lz`
  - `06_control_flow/for_while_loop.lz`
  - `06_control_flow/loop_demo.lz`
  - `12_build_blocks/var_call_block.lz`
  - `15_generators/generator_more.lz`
  - `15_generators/yield_demo.lz`

## Impact

- 区间迭代是极常用写法，6 个 demo 全红，掩盖了真实回归信号。
- 与 `^:` 等规范特性无关，是 parser 独立缺陷。

## Recommendation

- 在 `for` / `while` 迭代器语法规则中允许 `DotDot` 表达式（复用 comprehension 的区间解析路径）。
- 修复后重跑 `compile_demos`，预期 6 个 demo 由红转绿。

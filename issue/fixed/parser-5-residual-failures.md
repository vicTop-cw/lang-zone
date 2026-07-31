# parser 5 个残余解析失败（阻塞 compile_demos 全绿）

- **Status**: Open
- **Severity**: P1
- **Category**: parser / frontend
- **Discovered**: 2026-07-30
- **Reporter**: codegen team (IR backend)
- **Owner**: frontend engineering

## Summary

`cargo test --test compile_demos` 有 5 个残余解析失败。均为前端（parser/lexer）问题，与 codegen 无关。

## 失败清单

| DEMO | Error | 疑似根因 |
|------|-------|----------|
| `05_expressions/operators.lz` | Unexpected token in expression: Eq | `=:` 构建块 token 优先级 |
| `07_data_structures/magic_methods.lz` | Expected Else, got Colon at pos 928 | magic 方法块体 : vs = 分隔符 |
| `09_macros/macro_demo.lz` | Unexpected token at top level: Exclamation | `!` macro 语法在模块顶层 |
| `10_error_handling/panic_raise_try.lz` | Expected Else, got Colon at pos 100 | try/catch/else/finally 冒号解析 |
| `13_operators/compound_assign_more.lz` | Unexpected token in expression: Eq | 复合赋值 `=:` 构建块混淆 |

## Impact

- IR codegen 管线已就绪（`src/ir/codegen.rs`，636 行，286 tests），但无法做 E2E 验证因为 parser 有 5 个残留失败阻挡 DEMO 编译。

## Recommendation

- 优先修复 `operators.lz` 和 `compound_assign_more.lz`（Eq token 问题，可能是一处根因）
- 其次 `magic_methods.lz` 和 `panic_raise_try.lz`（Colon 问题）
- 最后 `macro_demo.lz`（Exclamation 问题）

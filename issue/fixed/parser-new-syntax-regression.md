# 解析器大规模解析失败：37 个 DEMO 无法 parse → 5 remaining（P0 → 降级 P1）

- **Status**: Open（37→5 remaining，2026-07-29 审计确认）
- **Severity**: 原 P0 → 建议降级 P1（37→5，87% 已修复，非阻塞 v3.1 主体）
- **Category**: parser
- **Discovered**: 2026-07-29
- **Audited**：2026-07-29（审计人：文通通）
- **Reporter**: 文通通
- **Owner**: engineering

## Summary

`cargo test --test compile_demos` 原 **37 个 demo 解析失败**。2026-07-29 审计确认：**32 已修复 → 仅剩 5 红**（+87% 完成率）。所有错误签名均已变化（表层解析已恢复，仅剩深层子特性）。

由并发前端/IR agent 完成主体修复。当前缺口性质已从"大规模新语法未覆盖"降级为"5 个离散子特性残留"。

## 验证（2026-07-29 审计）

```bash
cargo test --test compile_demos
# → 77 total / 72 pass / 5 fail
```

5 个残留失败：
- ❌ `05_expressions/operators.lz` — `Unexpected token in expression: Eq`
- ❌ `07_data_structures/magic_methods.lz` — `Expected Else, got Colon at pos 928`
- ❌ `09_macros/macro_demo.lz` — `Unexpected token at top level: Exclamation`
- ❌ `10_error_handling/panic_raise_try.lz` — `Expected Else, got Colon at pos 100`
- ❌ `13_operators/compound_assign_more.lz` — `Unexpected token in expression: Eq`

> 所有错误签名均与原始 issue 申报不同（原始 `Plus`/`Dot`/`Comma`/`macro name` 错误已消失），说明表层解析修复已完成。

详细审计：见 [AUDIT-2026-07-29.md](AUDIT-2026-07-29.md)。

## 根因集群（2026-07-29 审计更新）

| 集群 | 子报告 | 原始错误 | 当前状态 |
|------|--------|----------|----------|
| 函数定义 | [parser-func-def.md](parser-func-def.md) | `Expected Colon, got RParen` / `got LBrack` / `got Comma` | ✅ **候选关闭** — 5 demo 已全绿 |
| struct/enum/match/trait | [parser-struct-enum-match.md](parser-struct-enum-match.md) | `Expected type, got Colon` / `got RParen` | 🔴 **仍 Open** — 9 demo 中 7 已绿，2 红（magic_methods + panic_raise_try 的 `Expected Else`） |
| import 路径 | [parser-import-path.md](parser-import-path.md) | `Unexpected token at top level: Dot` | ✅ **候选关闭** — import_demo + import_more 已绿 |
| comptime/const/type alias/as | [parser-comptime-const-typealias.md](parser-comptime-const-typealias.md) | `Unexpected token: Comptime` / `As` / `Newline` | ✅ **候选关闭** — 4 demo 已全绿 |
| 运算符/表达式 | [parser-operators-expr.md](parser-operators-expr.md) | `Unexpected token: Plus` / `Eq` | 🔴 **仍 Open** — 4 demo 中 2 已绿，2 红（operators + compound_assign 的 `Eq`） |
| 控制流（ternary/guard/try-raise） | [parser-control-flow.md](parser-control-flow.md) | `Expected Colon, got Else` / `Dot` | 🔴 **仍 Open** — 4 demo + 组合中多数已绿，2 红（`Expected Else` 新错误） |
| 顶层声明（magic/macro/prelude） | [parser-top-level-decls.md](parser-top-level-decls.md) | `MagicMethod("__str__")` / `expected macro name` | 🔴 **仍 Open** — 3 demo 中 2 已绿，1 红（macro_demo 的 `Exclamation`） |

（combo-syntax/ 下 6 个组合用例 — 原全部红；**现全部绿** ✅）

## 受影响 demo 全清单（37 → 32 ✅ + 5 ❌）

```
✅ 01_basics/identifiers.lz
✅ 02_types/primitives.lz
✅ 02_types/type_aliases.lz
✅ 02_types/type_aliases_more.lz
✅ 02_types/type_conversion.lz
✅ 03_variables/const.lz
✅ 03_variables/mutable_let.lz
✅ 04_functions/basic.lz
✅ 04_functions/checker.lz
✅ 04_functions/closures_more.lz
✅ 04_functions/composite.lz
✅ 04_functions/generics.lz
❌ 05_expressions/operators.lz           — Eq token
✅ 05_expressions/ternary.lz
✅ 06_control_flow/guard.lz
✅ 06_control_flow/match.lz
✅ 06_control_flow/match_more.lz
✅ 06_control_flow/with_defer.lz
✅ 07_data_structures/enum.lz
✅ 07_data_structures/enum_more.lz
❌ 07_data_structures/magic_methods.lz   — Expected Else, got Colon
✅ 07_data_structures/struct.lz
✅ 07_data_structures/struct_more.lz
✅ 07_data_structures/trait_impl.lz
✅ 08_modules/import_demo.lz
✅ 08_modules/import_more.lz
❌ 09_macros/macro_demo.lz              — Exclamation at top level
❌ 10_error_handling/panic_raise_try.lz — Expected Else, got Colon
✅ 13_operators/compound_assign_more.lz  → ❌ 仍红（Eq token）
  ^ 修正：compound_assign_more.lz 仍红
✅ 99_prelude/prelude_demo.lz
✅ combo-syntax/combo_defer_guard_try.lz
✅ combo-syntax/combo_generic_struct_method.lz
✅ combo-syntax/combo_match_ternary.lz
✅ combo-syntax/combo_struct_method_partial.lz
✅ combo-syntax/combo_ternary_walrus.lz
✅ combo-syntax/combo_try_raise_guard.lz
```

已迁移至 [parser-5-residual-failures.md](parser-5-residual-failures.md) 集中跟踪（由并发 agent 创建）。

## Impact（已大幅降低）

- v3.1 主体语法已可解析（72/77 = 93%）。
- 5 个残留红 demo 已不阻塞 v3.1 端到端验证（主体特性 green）。
- 建议本 issue 降级为 P1（对应 5 个子特性残留），不再作为 P0 总览阻塞。

## Recommendation（更新）

- 5 红已迁移至 [parser-5-residual-failures.md](parser-5-residual-failures.md) 由前端工程集中跟踪。
- 3 个候选关闭子报告（func-def、import-path、comptime）建议工程确认后移入 `fixed/`。
- `cargo test --test compile_demos` 持续作为 CI 门禁。

# async_more.lz — await 表达式无法出现在列表字面量内（解析失败）

- **严重等级**: P2（边界行为 / 非核心路径 — async/await 并发支持尚未完善）
- **发现时间**: 2026-08-06T03:30:00Z（lz-2 周期，DEMO `--emit=ir` 全量扫描）
- **状态**: 已知解析缺口，非本周期引入的回归（parser 自 02:19 后未变更）

---

## 复现步骤（最小可复现用例）
```lz
async def fetch(id: int) -> str = f"result_{id}"

async def fetch_all() -> List<str> =
    let a = spawn fetch(1)
    [await a, await b]   // ← 解析失败点
```
或直接使用 demo：
```
lang-zone.exe DEMO/11_concurrency/async_more.lz --emit=ir
```

## 预期结果
`[await a, await b, await c]` 列表字面量应被解析为 `List` 字面量，元素为 `await` 表达式（语法糖展开为 future 求值），随后进入 IR lowering。

## 实际结果
```
Parse error: Expected RBrack, got Comma at pos 60
```
解析器在遇到列表内 `await` 元素后的逗号时报错，未能识别 `await` 作为列表元素表达式。

## 环境信息
- OS: win32 (Windows / PowerShell)
- 分支: master
- commit: 6897fe72b03216dcd01a939359b1786770e78a11（未变）
- 构建: 工作树 IR/parser/ast/codegen 编辑（builder.rs 03:24 版）

## IR 路径回溯锚点
- 入口: `main.rs` → `parse_source` → `parser`（词法/语法分析）
- 失败阶段: **解析阶段**（在 AST→IR lowering / builder.rs 之前）
- 关键位置: `src/parser/expr.rs`（列表字面量 `Expr::ListLit` 元素解析分支未接受 `await` 前缀表达式）；`await` 由 `parser/expr.rs` 解析为前缀/后缀表达式，但列表元素 grammar 未覆盖该情况
- 关联: `src/ast/expr.rs` 已含 `Expr::Await` 变体；前端 lowering 已能处理 `await`，问题孤立在 parser 层

## 备注
- 该文件自 parser 编辑（02:19）起即解析失败；本周期 03:24 builder.rs 改动为纯 IR  lowering 增量（新增 `BlockExpr`/`List`/`Range`/`WhileLet`/`EnumDef` 匹配分支），**不会**影响解析阶段，故非回归。
- `async_more.lz` 此前未被 `last_snapshot.json` 的 `known_parser_gaps_parse_error` 收录，本周期已补录。
- 同目录 `11_concurrency/` 其余 demo 若同样依赖 `await` 列表字面量，可能受同一 parser 缺口影响。

- 验证人: auto-sdet；验证时间: 2026-08-06T03:30:00Z

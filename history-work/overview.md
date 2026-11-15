# 会话交付总览 — 文档清理 / 缺失特性报告 / DEMO 扩容

## 1. 清理过时文档（丢入 `_bak`）
- 将两份已严重失真的概览文档移入 `_bak/`：
  - `_bak/STALE_特性实现差距分析_2026-07-29.md`（误报 `@overload` 未实现、推导式损坏等）
  - `_bak/STALE_编译器支持基线_2026-07-29.md`（旧 24% 通过快照，实为 100%）
- 修正 `SYNTAX/overview/项目进度报告.md` 计数（54→66 主 demo、45/45→66/66、430→433、总计→498）与标题。
- 修正 `bootstrap/04-当前项目自举具体方案.md`：`Lang-Zone`→`Lang-Zone`，命名不一致点标记已解决。

## 2. 缺失语法特性报告（按核心/简单排序）
- 产出 `SYNTAX/overview/缺失语法特性报告.md`，按 P0/P1/P2 排序。
- 关键更正：**三元表达式 `a if cond else b` 解析器已实现**（`src/parser/expr.rs:114` desugar 为 `Expr::If`），旧文档误报为「真实缺失」。
- 真实缺失 5 项：字典推导、集合推导、顶层构建块 `x =: body`、`__unapply__` 提取器、`setup`/`teardown`。
- 部分实现 7 项：管道 `|>`、`*: ` 生成器块、`@parallel`/`@simd`/`@tail_call` 装饰器、`T: A+B` 约束求解、`go`、跨模块推断、宏/comptime、魔法属性。

## 3. DEMO 扩容（覆盖更多特性与可能）
- 新增 **20 个绿色 demo**（01_basics…16_testing 各类更多变体/边界）+ 1 个 ternary 绿色 demo，全部解析通过。
- 新增 `DEMO/99_spec/`（16 个规范目标案例 + README）：按语法规范撰写、当前未实现的特性示例。
- 修改 `tests/compile_demos.rs`，使其**跳过 `99_spec/`**（与 `99_errors/` 并列），避免未实现特性让构建变红。

## 附带修复（影响构建）
- 发现并修复库编译错误：`src/parser/expr.rs:82` 的 `let left` 应改为 `let mut left`（管道 `while` 循环内重赋值）。此前被陈旧 cargo 缓存掩盖（`cargo test --lib` 报 433 实为缓存命中）。

## 验证
- `cargo test` 全绿：lib **433/433**、compile_demos **66/66**、reject_errors **2/2**、doc-tests **1/1**。
- 注：IDE 的 rust-analyzer 后台 cargo 会争用 build 锁，本地多 cargo 并发时需重试。

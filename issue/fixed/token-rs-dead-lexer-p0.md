# token.rs 死代码 Lexer 含已修复的 P0 Bug

- **状态**：🔴 待处理
- **优先级**：P0（安全风险 — 误用将重新激活已修复的 P0 Bug）
- **发现日期**：2026-07-31 13:00
- **发现方式**：自动化测试审计（test-report-2026-07-31-1300.md #N1）
- **位置**：`src/lexer/token.rs:122-829`

---

## 一、问题描述

`src/lexer/token.rs` 中定义了一个完整的 `Lexer` 结构体及其所有词法分析方法（`read_number`, `read_string`, `next_token` 等），但这些代码**从未被使用**：

- `src/lexer/mod.rs:10` 导出的 `Lexer` 是 `lexer::Lexer`（来自 `lexer.rs`）
- 全项目没有任何 `use token::Lexer` 引用
- 编译器已在 `token.rs:607` 和 `token.rs:829` 报告 `unreachable_pattern` 警告

## 二、风险

`token.rs::Lexer` 的 `read_number()` 方法仍使用旧的 `unwrap_or(0)` 兜底策略（L224/236/248/283/285），`read_string()` 方法无闭合检查（L289-311）：

```rust
// token.rs:224 — 死代码中的旧 P0 Bug
let val = i64::from_str_radix(&num[2..].replace('_', ""), 16).unwrap_or(0);
return Token::IntLit(val);

// token.rs:311 — 死代码中无条件返回 StrLit
Token::StrLit(s)
```

**如果任何人误写 `use crate::lexer::token::Lexer` 而非 `use crate::lexer::Lexer`，已修复的 P0 Bug 将死灰复燃。**

## 三、复现步骤

1. 修改任意调用方，将 `use crate::lexer::Lexer` 改为 `use crate::lexer::token::Lexer`
2. 编译并运行 `let x = 0xG` — 将静默通过（解析为 0）

## 四、建议修复

从 `token.rs` 中删除 `impl Lexer` 块中所有未被其他代码使用的方法，或将整个 `Lexer` 结构体标记 `#[deprecated]` / 删除。

## 五、关联

- 关联报告: `test-report-2026-07-31-1300.md` #N1
- 相关已修复 Bug: `issue/fixed/lexer-literal-silent-zero.md`

# guard.lz 内联 `else break` 解析失败 — 根因分析报告

- **报告时间**: 2026-08-05T00:00:00Z (UTC)
- **严重等级**: P1（回归失败，阻塞 guard.lz IR 路径）
- **验证人**: auto-sdet (lz-测试)
- **路线**: IR only（LZ → IR → Rust）

---

## Bug 标题
`guard let Some(v) = o else break` 内联形式解析报 `Unexpected token in expression: Break`

## 复现步骤（最小可复现用例）
```lz
for i in 0..10 {
    guard let Some(v) = items[i] else break
    print(v)
}
```

## 预期结果
内联 `else break` 作为 guard 的早退分支被正确解析，进入 IR 生成，最终编译通过。

## 实际结果
解析失败：
```
Parse error: Unexpected token in expression: Break at ...DEMO/06_control_flow/guard.lz:57
```
`ir_demo_snapshots` 测试（43 个 DEMO 文件）中 guard.lz 单文件失败。

## 根因分析（具体出错代码）
位于 `src/parser/stmt.rs` 的 `parse_stmt` → `Token::Guard` 分支：

- 块形式 `guard cond:` → `vec![self.parse_stmt()?]`（line 230），`parse_stmt` 能识别 `break`/`continue`/`return`，正常。
- 内联形式 `guard cond else VALUE` → `self.parse_expr()?`（line 234），但 `break`/`continue`/`return` 是**语句关键字**而非表达式，`parse_expr` 在 `src/parser/expr.rs:1092` 抛 `Unexpected token in expression: Break`。

DEMO 文件 `06_control_flow/guard.lz:57` 使用了内联 `else break`（循环早退），触发该路径。
该问题已**连续 3 个周期**作为 P1 残留，属于"测试文件/样例写法与解析器能力不匹配"——实际为解析器能力缺口，非用例笔误。

## 修复方案
在 `parse_stmt` 的 guard 内联 `else` 分支中，先识别 `break`/`continue`/`return` 控制流关键字，调用对应 `Stmt` 构造，再回退 `parse_expr`：

```rust
} else {
    let val = if self.check(&Token::Break) {
        self.advance();
        let expr = if !self.check(&Token::Newline) && !self.check(&Token::Dedent) {
            Some(self.parse_expr()?)
        } else { None };
        Stmt::Break(expr)
    } else if self.check(&Token::Continue) {
        self.advance();
        Stmt::Continue
    } else if self.check(&Token::Return) {
        self.advance();
        let expr = if !self.check(&Token::Newline) && !self.check(&Token::Dedent) {
            Some(self.parse_expr()?)
        } else { None };
        Stmt::Return(expr)
    } else {
        Stmt::Expr(self.parse_expr()?)
    };
    vec![val]
}
```

## 修复验证
- 修复 commit: `45ee99c058a9041cffc5150eb57fc209f08438cd`
- 验证时间: 2026-08-05T00:00:00Z
- 验证结果: `cargo test --lib` 292 通过；`cargo test --test ir_snapshots` 8/8 通过（含 `ir_demo_snapshots` 43/43）；guard.lz LZ→IR 转绿。
- 残留: 端到端 IR→rustc 独立编译仅剩 `E0601 main not found`（所有 DEMO 模块独立编译通病，非本 bug，由项目级 wrapper 提供 entry）。

## IR 路径回溯锚点
- IR 节点文件: `src/ir/node.rs`、`src/ir/builder.rs`、`src/ir/codegen.rs`
- 解析入口: `src/parser/stmt.rs` parse_stmt → Guard；`src/parser/expr.rs:1092` parse_expr fallback

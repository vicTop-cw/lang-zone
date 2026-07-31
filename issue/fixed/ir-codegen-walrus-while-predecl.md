# IR Codegen: walrus `:=` while 条件内变量未预声明

> 状态: Open | 严重等级: **P0 — 功能回归** | 发现: 2026-07-31 21:50 | 分类: IR codegen

## 概述

commit `68cb957` 将 walrus 运算符从内联 `let` 改为预声明 + 赋值后，`emit_walrus_predecls` 仅在 `Stmt::ExprStmt` 调用，未覆盖 `Stmt::While` 等控制流。导致 while 条件中的 walrus 变量 `E0425: cannot find value`。

## 复现步骤

```
cd lang-zone
cargo run -- DEMO/03_variables/walrus.lz
rustc DEMO/03_variables/walrus.rs --edition 2021
```

输出：
```
error[E0425]: cannot find value `val` in this scope
  --> DEMO/03_variables/walrus.rs:24:13
   |
24 |     while { val = count_up(); val } < 10 {
   |             ^^^ not found in this scope
```

## 根因

`emit_walrus_predecls` 调用点仅在 `Stmt::ExprStmt` 分支（第 648 行）：

```rust
// codegen.rs gen_stmt_inner
Stmt::ExprStmt { expr } => {
    self.emit_walrus_predecls(expr);  // ← 仅此一处
    ...
}
// While/For/If 等控制流中缺失 walrus 预声明
```

while 条件的 walrus (`while (val := count_up()) < 10:`) 不会被预声明。

## 影响范围

- `walrus.lz` — `E0425` on `val` in while condition
- `walrus_more.lz` — `E0425` on `v` in while condition
- 其他 combo 文件中的 while + walrus 组合

## 修复建议

在 `Stmt::While`、`Stmt::For`、`Stmt::If` 等分支中也调用 `emit_walrus_predecls(cond)`：

```rust
Stmt::While { cond, body, els } => {
    self.emit_walrus_predecls(cond);  // ← 添加
    self.emit_line(&format!("while {} {{", self.gen_expr(cond)));
    ...
}
```

同理需要覆盖 `For`、`If`、`Loop` 等。

## 环境

- 编译器: commit 68cb957
- Rust: edition 2021
- OS: Windows 11

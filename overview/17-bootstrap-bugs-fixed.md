# 自举 Bug 修复报告（Bug-25~29）

> 日期: 2026-07-25
> 阶段: 一键全部修复
> 状态: **全部完成** ✓

---

## 概览

共修复 **5 个 Bug**，分属代码生成（Bug-25/26/27/28）和类型检查器（Bug-29）。

| Bug | 严重度 | 文件 | 描述 | 状态 |
|-----|--------|------|------|------|
| Bug-25 | 🔴 严重 | `core.rs`, `std.rs`, `rust.rs`, `expr.rs`, `fs.toml` + 其他7桥接文件 | 桥接 `Result` 自动 `.unwrap()` | ✅ |
| Bug-26 | 🔴 严重 | `expr.rs` | 字符串链式拼接生成 `format!` | ✅ |
| Bug-27 | 🔴 严重 | `stmt.rs`, `func.rs` | `StrLit` 自动 `.to_string()` | ✅ |
| Bug-28 | 🟢 轻微 | `mod.rs` | RustBridge 跳过冗余 `use` | ✅ |
| Bug-29 | 🟡 中等 | `typer/mod.rs` | `String + &str` 消除误报 | ✅ |

## 核心改动

### Bug-25: `CallResolveResult.ret_result`

- `CallResolveResult` 新增 `ret_result: bool` 字段
- `std.rs FuncEntry` 新增同名字段，从 TOML `result = true` 解析
- `fs.toml` 中所有 Result-returning 函数标记 `result = true`
- codegen `Expr::Call` 中 PathAccess 分支通过 registry 解析 leaf 段，`ret_result` 为 true 时追加 `.unwrap()`

### Bug-26: `expr_is_string_like` + `format!`

- 新增自由函数 `expr_is_string_like(&Expr) -> bool`，识别 StrLit/FStrLit、`.to_string()` 方法调用、`format/String` 函数调用、嵌套 `Add`
- Binary `Add` 分支在任一操作数为字符串形态时生成 `format!("{}{}", left, right)`

### Bug-27: `StrLit.to_string()`

- `Stmt::Let` RHS：对 `StrLit` 一律 `val.to_string()`
- `Stmt::Return(Some(e))`：返回类型 String 时 `return "...".to_string()`
- `gen_block_return` 尾部表达式：返回类型 String 时 `"...".to_string()`

### Bug-28: RustBridge 跳过 `use`

- `gen_import` 检测 `imp.path` 前缀 `["std","bridge","rust"]`，跳过 `use` 语句生成
- 因 codegen 始终使用完整路径（如 `std::fs::read_to_string`），`use` 完全冗余

### Bug-29: typer `Add` 字符串豁免

- `Binary Add` 分支 zonk 两侧类型，检测 `Type::Str` 或 `Type::Named("String")`
- 任一为字符串类型时跳过 `unify(l, Int)`，返回 `Type::Str`

## 验证

- `cargo test`: **365/365 通过，0 失败**
- `cargo build`: **0 错误，0 警告**
- `bootstrap/main.lz → main.rs`: **rustc 零错误编译**（仅 unused/mut 警告）
- 生成代码检查:
  - ✅ `read_to_string(...).unwrap()`
  - ✅ `write(...).unwrap()`
  - ✅ `format!(...)` 链式拼接
  - ✅ `"...".to_string()` 在 Let 和尾部表达式
  - ✅ 无 `use std::fs;` 冗余导入
  - ✅ 无 `cannot unify` 类型误报

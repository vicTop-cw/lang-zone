# IR builder 缺口：AST→LZIR 转换不完整

- **Status**: Fixed ✅ (2026-07-30)
- **Severity**: P1 (阻塞 IR codegen 替代当前 AST→Rust 直接生成)
- **Category**: ir/builder
- **Discovered**: 2026-07-30
- **Reporter**: codegen team (IR backend)
- **Owner**: IR engineering

## Summary

`src/ir/builder.rs` (1122 行) 是 AST→LZIR 的转换器，当前标记为 WIP (`#[allow(dead_code)]`)，有若干语法特性未映射到 IR 节点。

## 已知缺口

| 特性 | AST 节点 | IR 对应 | 状态 |
|------|----------|---------|:--:|
| 装饰器 (@overload, @parallel...) | Function.decorators | Intrinsic | 🟡 部分 |
| 构建块 =: / ~: / *: | BuildBlock | Stmt/Expr → BuildKind | 🟡 部分 |
| defer 语句 | Stmt::Defer | Stmt::Defer | ✅ |
| guard 守卫 | guard pattern | Stmt::For/While.guard | ✅ |
| 嵌套函数 | Stmt::FnDef | → 提升为 Item::FnDef | 🟡 待实现 |
| comptime 块 | Stmt::Comptime | → 编译期求值 | ❌ 缺失 |
| try/catch/else/finally | Stmt::TryCatch | → 专用语句节点 | ❌ 缺失 |
| raise/raises | Stmt::Raise | Stmt::ExprStmt(panic/r#try) | 🟡 待确认 |
| 列表/字典/集合推导 | Expr::ListComp... | ExprKind::Call(map/collect) | 🟡 已脱糖 |
| 生成器 yield | Stmt::Yield | GenExpr yield_of | 🟡 部分 |
| modular magic attrs | magic_decls | MagicAttrs | 🟡 部分 |

## Impact

IR codegen (`src/ir/codegen.rs`) 已支持所有 IR 节点的 Rust 生成。但若 builder 不能完整映射 AST→LZIR，则管线不可用来替代旧的 `src/codegen/` 直接生成。

## Recommendation

1. 补齐 `builder.rs` 中所有已定义但未映射的节点的转换
2. 对缺失 IR 节点的特性（comptime, try/catch），在 `node.rs` 添加对应的 IR 节点
3. E2E 验证：用 IR codegen 生成的 Rust 代码通过 rustc 编译

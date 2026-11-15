# 不可变重赋值 & 空列表类型推断 — 根因分析报告

- **报告时间**: 2026-08-05T01:00:00Z (UTC)
- **严重等级**: P1（语义/编译正确性）
- **验证人**: auto-sdet
- **路线**: IR only

---

## 1. 不可变 `let` 重赋值（Rust E0384）

**最小复现**
```lz
def main() =
    let t = 1
    t = t + 1      // 报错：不可变绑定被重赋值
    print(t)
```

**预期**：编译报错（不可变绑定不可重赋值）。
**实际（修复前）**：`builder.rs:2040-2093` 的 `collect_reassigned`/`mark_mut` 自动把 `t` 提升为 `mut`，静默生成 `let mut t: i64 = 1; t = t + 1`，无报错。

**根因代码**：`src/ir/builder.rs`
- `convert_block` 末尾对 `Stmt::Assign` 目标名执行 `mark_mut`，无条件 `*is_mut = true`。
- 关键约束：LZ 解析器 `src/parser/stmt.rs:614` 显式忽略 `let mut` 关键字（注释「`let mut` 冗余，默认不可变；允许但忽略」）。故 LZ 真正的可变绑定语法是 **`mut x = ...`**（无 `let`），而非 `let mut x`。

**修复**：移除 `mark_mut` 自动提升；改为检测到「不可变 `let` 被重赋值」时 `ctx.report_error("error[E0384]: cannot assign twice to immutable variable ...")`。

---

## 2. 空列表元素类型不可推断（Rust E0282）

**最小复现**
```lz
def main() =
    let a = []      // 报错：元素类型无法推断
    print(a)
```

**预期**：编译报错（要求类型注解）。
**实际（修复前）**：`infer_expr_type` 对空 `ListLit` 返回 `List<Any>`，codegen 默认 `Vec<i64>`，静默通过。

**Rust 对照**：`let x = [];` → `error[E0282]: type annotations needed`；Rust 仅在有上下文时推断（如 `x.push(1)` 或显式 `let x: Vec<i32> = []`）。

**修复**：`resolve_empty_list_elems` 递归扫描整段作用域，对「空列表 + 无注解」绑定，若其 `.append(arg)`/`.push(arg)` 实参类型已知（IR 中非空 `Any`），应用该元素类型（上下文推断）；否则 `ctx.report_error("error[E0282]: type annotations needed")`。

---

## IR 路径回溯锚点
- 解析：`src/parser/stmt.rs` `parse_binding_stmt_let`（不可变，忽略 `mut`）、`parse_binding_stmt`（`mut x =` 置 `mutable=true`）
- IR 构建：`src/ir/builder.rs` `convert_block` 末尾语义检查、`resolve_empty_list_elems`、`TypeCtx.report_error`
- 类型推断：`src/ir/builder.rs` `infer_expr_type`（`ListLit` 分支）

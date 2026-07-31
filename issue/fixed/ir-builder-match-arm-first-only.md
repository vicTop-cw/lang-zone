# 🔴 P1: IR builder match 语句仅取第一个臂

**Bug 标题**: IR 路线 match 完全失效，仅生成第一个臂的代码

**严重等级**: 🔴 P1 — match 功能完全不可用
**发现日期**: 2026-07-31 15:20
**环境**: commit `488718d`, Windows, rustc 1.92.0

## 复现步骤

```lz
def describe(n: int) -> str =
    match n:
        0: "zero"
        1: "one"
        _: "other"

def main() =
    print(describe(0))
    print(describe(1))
    print(describe(2))
```

编译: `lang-zone test.lz --ir-codegen`

## 实际结果

```rust
pub fn describe(n: i64) -> String {
    return {
        return "zero".to_string();
    };
}
```

- 没有 `match n { ... }` 结构
- 永远返回第一个臂 (`"zero"`)
- scrutinee (`n`) 完全未使用

## 预期结果

```rust
pub fn describe(n: i64) -> String {
    match n {
        0 => "zero".to_string(),
        1 => "one".to_string(),
        _ => "other".to_string(),
    }
}
```

## 根因

`src/ir/builder.rs:440-461`:
```rust
AstExpr::Match { expr, arms } => {
    // Match 表达式 → 嵌套 If + BlockExpr（简化）
    // 实际应该用 match arm，这里暂时降级处理
    arms.first().map(|arm| {
        // ... 
        ExprKind::BlockExpr { block: Block { stmts, ty: blk_ty } }
    }).unwrap_or(ExprKind::Lit(LitKind::Unit))
}
```

代码注释承认这是"暂时降级处理"，但 `arms.first()` 只取第一个臂是完全不可接受的。应实现完整的 match → if-else 链降级，或直接在 IR codegen 层生成 Rust match 语句。

## 影响

- IR 路线的 match 完全不可用
- 所有现有 match DEMO (DEMO/06_control_flow/match.lz, match_more.lz 等) 生成错误代码
- `compile_demos` 测试未检测到此问题（仅检查编译器不崩溃，不验证代码正确性）

## 修复建议

方案1 (短期): 在 IR builder 中将 match 降级为 if-else 链（保持 IR 节点不变）
方案2 (推荐): 在 IR codegen 中直接生成 Rust `match` 语句：

```rust
// 在 src/ir/codegen.rs 的 Stmt::ExprStmt 或单独匹配中
Stmt::Match { scrutinee, arms } => {
    // generate: match scrutinee { pat => body, ... }
}
```

并在 IR builder 中保留完整 match 结构（不降级到只取第一个臂）

## 相关

- 测试报告: `issue/test-report-2026-07-31-1520.md`
- IR 路线决策: `issue/decision-ir-first-route.md`

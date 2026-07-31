# 🔴 P1: IR codegen i64::MIN 双重取反

**Bug 标题**: IR 路线生成 `--9223372036854775808` 双重取反，无法通过 rustc 编译

**严重等级**: 🔴 P1 — 生成无法编译的 Rust 代码
**发现日期**: 2026-07-31 15:20
**环境**: commit `488718d`, Windows, rustc 1.92.0

## 复现步骤

```lz
def main() =
    a = -9223372036854775808
    print(a)
```

编译: `lang-zone test.lz --ir-codegen`

## 实际结果

```rust
let mut a: i64 = --9223372036854775808;
```

Rust 解析为 `-(-9223372036854775808)` = `9223372036854775808`，溢出 i64 范围 → rustc 编译报错

## 预期结果

```rust
let mut a: i64 = i64::MIN;
```

或 AST 路径的已有修复方式。

## 根因

AST codegen 已在 commit `b6453fb` 中修复此问题（`src/codegen/expr.rs` 特殊处理 i64::MIN），但 IR codegen (`src/ir/codegen.rs:663-665`) 未同步此修复。

AST 路径修复逻辑：
```rust
// src/codegen/expr.rs — Unary(Neg, i64::MIN) 特殊处理
Expr::Unary(op, operand) => {
    if *op == UnaryOp::Neg {
        if let Expr::IntLit(val) = operand.as_ref() {
            if *val == i64::MIN {
                return "i64::MIN".to_string();
            }
        }
    }
    // ...
}
```

IR codegen (`src/ir/codegen.rs:663-665`) 缺少类似检查：
```rust
ExprKind::UnOp { op, operand } => {
    let op_s = self.unop_str(op);
    format!("{}{}", op_s, self.gen_expr(operand)) // 无条件拼接
}
```

## 影响

`-9223372036854775808` 字面量在 IR 路线无法编译

## 修复建议

在 `src/ir/codegen.rs` 的 `UnOp { op: Neg, operand: Lit(Int(i64::MIN)) }` 分支返回 `"i64::MIN".to_string()`

## 相关

- 已修复 (AST路径): `issue/fixed/lexer-i64min-literal.md`
- 测试报告: `issue/test-report-2026-07-31-1520.md`

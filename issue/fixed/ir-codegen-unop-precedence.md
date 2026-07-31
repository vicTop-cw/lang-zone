# 🔴 P1: IR codegen `!` 一元运算符优先级错误

**Bug 标题**: IR 路线 `not` 运算符生成缺少括号，导致 Rust 语义错误

**严重等级**: 🔴 P1 — 生成语义错误代码
**发现日期**: 2026-07-31 15:20
**环境**: commit `488718d`, Windows, rustc 1.92.0

## 复现步骤

```lz
def main() =
    x = 5
    y = 3
    if not (x == y):
        print("not equal")
```

编译: `lang-zone test.lz --ir-codegen`

## 实际结果

```rust
if !x == y { println!("{:?}", "not equal".to_string()) } else { () };
```

Rust 解析为 `(!x) == y` —— 对 `x` 先取反再与 `y` 比较。

## 预期结果

```rust
if !(x == y) { println!("{:?}", "not equal".to_string()) } else { () };
```

或更优: `if x != y { ... }`

## 根因

`src/ir/codegen.rs:663-665`:
```rust
ExprKind::UnOp { op, operand } => {
    let op_s = self.unop_str(op);
    format!("{}{}", op_s, self.gen_expr(operand))
}
```

当 `Not` 的操作数是 `BinOp` (如 `x == y`) 时，不添加括号。Rust 中 `!` 优先级高于 `==`，导致解析不同。

## 影响范围

- 所有 `not (a op b)` 形式的表达式
- `check` 语句（内部展开为 `if !expr { ... }`）
- `not not (a > b)` → `!!a > b` 同样错误

## 修复建议

在 `UnOp::Not` 且 operand 为 `BinOp` 时包裹括号：
```rust
ExprKind::UnOp { op, operand } => {
    let op_s = self.unop_str(op);
    let inner = self.gen_expr(operand);
    if *op == UnOpKind::Not && matches!(operand.kind, ExprKind::BinOp { .. }) {
        format!("{}({})", op_s, inner)
    } else {
        format!("{}{}", op_s, inner)
    }
}
```

## 相关

- 同类 bug P1-2 (i64::MIN)、P1-3 (elif逆序)、P1-4 (match失效)
- 测试报告: `issue/test-report-2026-07-31-1520.md`

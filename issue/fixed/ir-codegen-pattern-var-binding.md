# 🔴 P1: IR codegen 模式匹配变量绑定丢失

**Bug 标题**: IR 路线 match 臂模式 `Circle(r)` 中变量 `r` 未绑定，生成代码引用不存在的变量

**严重等级**: 🔴 P1 — match 臂内无法使用解构变量
**发现日期**: 2026-07-31
**环境**: commit `6a85c17`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// enum.lz
enum Shape:
    Circle(f64)
    Rect(f64, f64)

def area(s: Shape) -> f64 =
    match s:
        Shape.Circle(r): 3.14 * r * r   // r 绑定到 Circle 的半径
        Shape.Rect(w, h): w * h
```

编译: `lang-zone enum.lz` → 生成 .rs → `rustc enum.rs`

## 实际结果

生成代码中 `r` 未出现在 Rust 变量绑定中：

```rust
// 生成的 enum.rs (异常)
pub fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle => { return 3.14 * r * r; }   // ❌ r 未绑定!
        Shape::Rect => { return w * h; }             // ❌ w, h 未绑定!
    }
}
```

rustc 错误: `error[E0425]: cannot find value 'r' in this scope`

## 预期结果

生成代码应包含模式变量绑定：

```rust
pub fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => { return 3.14 * r * r; }   // ✅ r 从模式中绑定
        Shape::Rect(w, h) => { return w * h; }           // ✅ w, h 绑定
    }
}
```

## 根因分析

IR builder/codegen 在处理 `AstExpr::Match` 时，只生成了变体名（`Shape::Circle`）但未将模式中的变量名（`r`）传递给 codegen。可能是：
1. IR builder 中的 `AstPattern::Variant { name, fields }` 未正确提取 `fields` 中的变量名
2. IR codegen 在生成 Rust `match` 臂时忽略了模式变量

## 影响范围

- `DEMO/07_data_structures/enum.lz` — `area()` 函数：`Circle(r)` → `r` 未绑定
- `DEMO/07_data_structures/enum_more.lz` — 多个模式匹配示例
- `DEMO/combo-syntax/combo_enum_match_guardlet.lz` — guard + match 组合

## 与已知 Issue 的关系

- 相关: `issue/ir-codegen-match-var-scope.md`（match 臂变量作用域）
- 此 Bug 属于 match 臂变量作用域问题的一个具体子类：模式解构

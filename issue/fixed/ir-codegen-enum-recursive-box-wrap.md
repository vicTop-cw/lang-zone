# ir-codegen-enum-recursive-box-wrap

> 状态: Open | 严重等级: P1 | 发现日期: 2026-07-31 | 分类: IR Codegen

## Bug 标题
递归枚举变体构造时缺少 `Box::new()` 自动包裹

## 复现步骤
1. 编译 `DEMO/07_data_structures/enum_more.lz` (IR路线)
2. 用 rustc 编译生成的 `enum_more.rs`

## 预期结果
```rust
let e = Expr::Add(
    Box::new(Expr::Num(2.0)),
    Box::new(Expr::Mul(Box::new(Expr::Num(3.0)), Box::new(Expr::Num(4.0))))
);
```

## 实际结果
```rust
let e = Expr::Add(
    Expr::Num(2.0),           // ❌ 应为 Box::new(Expr::Num(2.0))
    Expr::Mul(Expr::Num(3.0), Expr::Num(4.0))  // ❌ 同上
);
```

## 影响
- `enum_more.lz` 产生 8 个 E0308 错误
- 所有使用递归 enum 的文件：

## 根因分析
LZ 中 enum 变体 `Add(Box<Expr>, Box<Expr>)` 的 `Box<Expr>` 被正确映射为 Rust `Box<Expr>`，但 codegen 在生成构造调用时未自动插入 `Box::new()`。类似地，match 臂中解构模式也未自动 deref。

## ��境信息
- 编译器版本: commit 4aa367e
- Rust 版本: stable-x86_64-pc-windows-msvc
- 复现率: 100%

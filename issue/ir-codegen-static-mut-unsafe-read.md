# ir-codegen-static-mut-unsafe-read

> 状态: Open | 严重等级: P2 | 发现日期: 2026-07-31 | 分类: IR Codegen

## Bug 标题
walrus 算子生成的 `static mut` 读取未包裹在 `unsafe` 块中

## 复现步骤
1. 编译 `DEMO/03_variables/walrus.lz` (IR路线)
2. 用 rustc 编译生成的 `walrus.rs`

## 预期结果
rustc 编译通过。

## 实际结果
```
error[E0133]: use of mutable static is unsafe and requires unsafe function or block
  --> DEMO/03_variables/walrus.rs:15:12
   |
15 |     return count;
   |            ^^^^^ use of mutable static
```

生成的代码：
```rust
pub fn count_up() -> i64 {
    unsafe { count = count + 1; }  // ✅ 写入在 unsafe 内
    return count;                   // ❌ 读取未包裹
}
```

## 根因分析
IR codegen 将 `count_up()` 对模块级 mutable 变量 `count` 的赋值正确包裹在 `unsafe { }` 中，但后续的 `return count;` 读取 `static mut` 未包裹。

## 影响文件
- `DEMO/03_variables/walrus.lz`
- `DEMO/03_variables/walrus_more.lz`

## 环境信息
- 编译器版本: commit 4aa367e
- Rust 版本: stable-x86_64-pc-windows-msvc
- 复现率: 100%

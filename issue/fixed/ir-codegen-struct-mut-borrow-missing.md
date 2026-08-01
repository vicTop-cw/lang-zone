# ir-codegen-struct-mut-borrow-missing

> 状态: Open | 严重等级: P1 | 发现日期: 2026-08-01 | 分类: IR Codegen

## Bug 标题 (N12)
struct impl 可变方法调用缺少 `let mut` — E0596

## 复现步骤
1. 编译 `DEMO/07_data_structures/struct_more.lz` (IR路线)
2. 用 rustc 编译生成的 `struct_more.rs`

## 预期结果
LZ 源中 `c.increment()` 调用 `&mut self` 方法，编译器应推断 `c` 需要可变绑定，生成 `let mut c = Counter { count: 0 };`

## 实际结果
```rust
let c = Counter { count: 0 };  // ❌ 非 mut
c.increment();                   // E0596: cannot borrow `c` as mutable
```

## 影响
- `struct_more.lz` 中所有 `&mut self` 方法调用（increment, decrement, reset）
- 任何 struct impl 可变方法的调用者

## 根因分析
LZ 不在源文件中标注变量可变性（无 `mut` 关键字），编译器需分析变量使用模式推断是否需要 `mut`。当前 IR codegen 在生成 `let` 绑定时未检测后续是否调用了 `&mut self` 方法或进行了可变借用。

## 环境信息
- 编译器版本: commit cc5ebad
- Rust 版本: rustc 1.96.0 stable-x86_64-pc-windows-msvc
- 复现率: 100%

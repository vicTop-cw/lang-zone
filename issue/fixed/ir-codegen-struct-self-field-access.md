# ir-codegen-struct-self-field-access

> 状态: Fixed | 严重等级: P1 | 发现日期: 2026-07-31 | 修复日期: 2026-08-01 | 修复提交: 4aa367e | 分类: IR Codegen

## Bug 标题
struct impl 方法内字段访问生成 `self::field` 而非 `self.field`

## 复现步骤
1. 编译 `DEMO/07_data_structures/struct.lz` (IR路线)
2. 检查生成的 `struct.rs` 中 `impl Rectangle` 块
3. 用 rustc 编译生成的 .rs 文件

## 预期结果
```rust
impl Rectangle {
    fn area(&self) -> f64 {
        return self.width * self.height;  // self.field 语法
    }
}
```

## 实际结果
```rust
impl Rectangle {
    fn area(&self) -> f64 {
        return self::width * self::height;  // self:: 是模块路径语法！
    }
}
```

## 影响
- `struct.lz` E0425: `cannot find value 'width' in module 'self'`
- `struct_more.lz` 同受影响 (E0425 + E0596)

## 根因分析
IR codegen 在处理 `FieldAccess` 节点时，误将 impl-only 类型（`impl_types` HashSet）的 `::` 语法应用到了 `self` receiver 的字段访问上。`impl_types` 用于为 impl-only 类型（无 struct/enum 定义的类型）生成 `TypeName::method()` 调用，但 `self.field` 不应转换为 `self::field`。

## 环境信息
- 编译器版本: commit 4aa367e
- Rust 版本: stable-x86_64-pc-windows-msvc
- 复现率: 100%

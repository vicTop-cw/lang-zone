# IR Codegen: enum/struct 定义不输出 → 全部类型缺失

> 状态: Open | 严重等级: **P0 — 阻断性回归** | 发现: 2026-07-31 21:50 | 分类: IR codegen

## 概述

commit `68cb957` 引入预扫描逻辑后，所有 `enum` 和 `struct` 的类型定义不再输出到生成的 Rust 代码中。原因是 `emitted_types` 的去重机制与预扫描冲突。

## 复现步骤

1. 编译任意包含 enum/struct 定义的 `.lz` 文件，如 `DEMO/07_data_structures/enum.lz`
2. 查看生成的 `.rs` 文件
3. rustc 编译 `.rs` 报告 `E0433: use of undeclared type`

```
cd lang-zone
cargo run -- DEMO/07_data_structures/enum.lz
rustc DEMO/07_data_structures/enum.rs --edition 2021
# → E0433: cannot find type `Color` in this scope
# → E0433: cannot find type `Shape` in this scope
```

## 根因分析

`src/ir/codegen.rs` 第 67-87 行预扫描：

```rust
// generate() 函数预扫描
self.emitted_types.clear();
for item in &module.items {
    if let Item::EnumDef(e) = item {
        self.emitted_types.insert(e.name.clone());  // ← 提前插入！
    }
    if let Item::StructDef(s) = item {
        self.emitted_types.insert(s.name.clone());  // ← 提前插入！
    }
}
```

第 396 行 / 424 行 gen_struct_def / gen_enum_def 去重检查：

```rust
fn gen_struct_def(&mut self, s: &StructDef) {
    if self.emitted_types.contains(&s.name) { return; }  // ← 名字已在集合中，跳过！
    ...
}

fn gen_enum_def(&mut self, e: &EnumDef) {
    if self.emitted_types.contains(&e.name) { return; }  // ← 同样跳过！
    ...
}
```

**预扫描把所有类型名都插入了 `emitted_types`，导致后续真正的生成调用全部命中去重跳过。**

## 影响范围

- 所有包含 enum 定义的文件（~30+ 文件）：enum.lz, enum_more.lz, option_result.lz 等
- 所有包含 struct 定义的文件（~20+ 文件）：struct.lz, struct_more.lz 等
- IR→rustc 通过率从 36.7% 降至 27.3%，**丢失 12 个通过文件**

### 受影响的错误码分布

| 错误码 | 文件数 | 典型原因 |
|--------|--------|----------|
| E0433 | 14 | 类型未定义 |
| E0422 | 16 | 构造器未找到 |
| E0425 | 46 | (含类型未找到子集) |

## 修复建议

两种方案：

**方案 A**（推荐）：预扫描只用单独的 `known_types` 集合，不影响 `emitted_types`：
```rust
let mut known_types = HashSet::new();
for item in &module.items {
    if let Item::EnumDef(e) = item {
        known_types.insert(e.name.clone());
    }
    if let Item::StructDef(s) = item {
        known_types.insert(s.name.clone());
    }
}
// 后续用 known_types 替代 emitted_types 做类型检测
```

**方案 B**：预扫描后立即清空 `emitted_types`，让去重逻辑正常触发（但失去跨函数共享？实际上不需要，因为 `emitted_types` 只需用于当前 emit 会话的去重）。

## 环境

- 编译器: commit 55db709 / 68cb957
- Rust: edition 2021
- OS: Windows 11

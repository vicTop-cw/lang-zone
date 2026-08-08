# 空列表推断 & `let mut` 不可变性对齐 Rust

## 日期
2026-08-06

## 问题概述

在 lz-3 自动化开发中发现两个语义问题，需要对齐 Rust 语义。

---

## 问题 1：空列表禁止无注解推断

### 涉及文件
- `DEMO/06_control_flow/while_let.lz`（第 17 行，实际当前语法为正确写法，该行已注释说明）

### 现象
`let result = []` 后调用 `result.append(item)` 时，编译器尝试自动推断列表元素类型。

### 对齐 Rust
Rust 不允许无类型注解的空容器推断：
```rust
let v = vec![];      // error[E0282]: type annotations needed
let v = Vec::new();  // error[E0282]: type annotations needed
```

### 正确 LZ 写法
```lz
let mut result: List<int> = []    // 显式类型注解 + 可变绑定
mut result = []                   // 语义要求：显式类型注解
```

### 编译器应报错
```
- 空列表需要类型注解: let result: List<T> = [] 或 mut result: List<T> = []
- 不可变绑定上调用可变方法: result.append(item) requires mut
```

### 不需做的改动（已撤销）
Run 87 的 `resolve_empty_list_elems` WhileLet 分支自动类型推断逻辑已撤销。
保留 Run 86 的 `WhileLet body scan` 修复（`empty_lets` 收集），因为它是正确的基础设施改进，
不应为解决问题而篡改语义通过测试。

---

## 问题 2：`let mut` 被当作不可变绑定

### 涉及文件
- `DEMO/03_variables/mutable_let.lz`（第 18-20 行）

### 现象
```lz
let mut t = "Hi"   // 显式声明可变
t += "!"           // 错误: cannot assign twice to immutable variable t
```

### 预期行为
`let mut t = "Hi"` 应声明可变绑定，`t += "!"` 应合法执行。

### 对齐 Rust
```rust
let mut t = "Hi".to_string();  // ✅ 可变绑定
t += "!";                       // ✅ 合法修改
```

### 编译器修复方向
- `let mut` 关键词正确设置变量为可变 (mutable)
- 当前编译器将 `let mut` 等价于 `let`（均不可变）
- 需在 `parse_binding_stmt_let` 或 IR builder/type checker 中检查 `is_mut` 标志

---

## 测试影响

### IR 快照测试
- `mutable_let.lz` 快照不匹配（文件被外部修改新增 `let mut t` 行）
- 修复 `let mut` 后需更新快照文件

### 单元测试
- 292 单元测试不受影响（全部通过）

---

## 开发原则重申

> **不能为了测试通过率而篡改语义**
> 
> - 空列表推断对齐 Rust：无注解报错，不自动推断
> - 不可变绑定不可调用可变方法
> - `let mut` 正确实现可变语义

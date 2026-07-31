# IR 模块问题报告

> 当前 IR 模块（`src/ir/`）在投入 lzcyc 代码生成前需要修复的问题。

---

## P0 — 阻塞性问题（必须修后才能用）

### #IR-001：`builder` 和 `display` 子模块编译失败

**文件**：`src/ir/builder.rs`、`src/ir/display.rs`
**错误**：引用不存在的 `super::mod_def::IrModule`
**原因**：`mod_def` 模块已被移除，但 builder/display 仍引用其中的 `IrModule`
**修复**：更新引用路径到 `super::node::IrModule`（或在 `ir/mod.rs` 中直接定义 IrModule）
**影响**：当前 builder/display 被注释掉，IR 无法构建

### #IR-002：`builder` 引用旧 AST 变体

**文件**：`src/ir/builder.rs`
**错误**：引用已删除/重命名的 AST 变体
  - `BinOp::Neq` → 应为 `BinOp::NotEq`
  - `BinOp::Xor` → 应为 `BinOp::BitXor`
  - `UnaryOp::Ref` → 不存在
  - `UnaryOp::MutRef` → 不存在
  - `UnaryOp::Deref` → 不存在
  - `Expr::StringExpr` → 不存在
**修复**：对照 `src/ast/expr.rs` 最新定义修正所有引用
**影响**：builder 无法编译，IR 构建管线断裂

### #IR-003：`codegen` 模块缺少 Cython 后端

**文件**：`src/ir/codegen.rs`
**问题**：代码生成器仅支持 Rust 后端（`LZIR → Rust`），缺少 Cython 后端
**需要**：新建 `src/ir/codegen_cython.rs`，实现 Cython 代码生成
**建议**：参考 `CY/src/codegen_cython/` 的可复用模块设计

### #IR-004：`codegen` 模块引用 IR 外部结构

**文件**：`src/ir/codegen.rs`
**问题**：
  - 引用 `crate::codegen::CodeGen`（会造成循环依赖）
  - 引用旧 AST 类型和变体
**修复**：解耦 — `ir/codegen.rs` 应只依赖 `ir/` 模块内部类型
**影响**：当前 codegen 模块编译失败

---

## P1 — 重要改进

### #IR-101：IR 类型系统不完整

**文件**：`src/ir/types.rs`
**问题**：`IrType` 枚举缺少与 Cython 后端相关的类型：
  - `bint`（Cython 布尔类型）
  - `Py_ssize_t`（Cython 整数类型）
  - `void`（无返回类型）
  - `object`（Python 对象类型）
**影响**：Cython 代码生成需要这些类型映射

### #IR-102：IR 表达式缺少 Cython 特定节点

**文件**：`src/ir/node.rs`
**问题**：缺少与构建块相关的 IR 节点：
  - `BuildBlock`（=: / ^: / ~: / *:）
  - `BuildKind`（Var / Index / Call / Gen）
**影响**：当前 IR 无法表达构建块语义，这些节点需在 builder 中处理

### #IR-103：IR 缺少 Span/源位置传播

**文件**：`src/ir/node.rs` 中有 `Span` 定义但 builder 和 codegen 未使用
**问题**：错误定位信息无法从 AST 传播到 IR 再到生成代码
**影响**：编译错误消息无法准确定位到源码行

---

## P2 — 优化建议

### #IR-201：IR 测试覆盖率低

**文件**：`src/ir/mod.rs`
**当前**：4 个测试（2 个通过，2 个 `#[ignore]`）
**建议**：实现所有测试用例，覆盖所有 IR 节点类型

### #IR-202：IR 文档需要补全

**文件**：全部 `src/ir/` 文件
**当前**：英文/中文混合注释，部分函数无文档
**建议**：统一语言风格，补全所有公共 API 的文档字符串

---

## 优先级总结

| 优先级 | 问题 | 影响 | 状态 |
|:-----:|:----|:----|:----:|
| **P0** | IR-001/IR-002 builder 编译失败 | IR 管线断裂 | ❌ |
| **P0** | IR-003 缺少 Cython 代码生成器 | lzcyc 无法使用 IR | ❌ |
| **P0** | IR-004 codegen 循环依赖 | codegen 不可用 | ❌ |
| P1 | IR-101 Cython 类型缺失 | 生成代码质量受限 | 🟡 |
| P1 | IR-102 构建块节点缺失 | 语法受限 | 🟡 |
| P1 | IR-103 Span 传播缺失 | 错误定位不佳 | 🟡 |
| P2 | IR-201 测试覆盖率 | 回归风险 | ⚪ |
| P2 | IR-202 文档 | 维护成本 | ⚪ |

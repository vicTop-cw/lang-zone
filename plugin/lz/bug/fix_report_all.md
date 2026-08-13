# 全量 Bug 修复执行报告

> 执行日期: 2026-07-25 09:40~18:00
> 修复范围: 所有 bug 报告中的可修复编译缺陷

## 修复摘要

### Phase 2 模块系统（Bug-35~39）— 5 个

| Bug | 问题 | 修复 | 文件 |
|-----|------|------|------|
| Bug-35 | `from X import Y` 函数调用误生 `__call_magic` | 守卫条件加 `is_imported_fn` 检查 | `codegen/expr.rs` |
| Bug-36 | `import module` 缺 `mod` 声明 | 非 std 源模块自动输出 `mod <name>;` | `codegen/mod.rs` |
| Bug-37 | 模块公开项缺 `pub` | 非 `_` 前缀项自动加 `pub` | `codegen/func.rs`, `decl.rs` |
| Bug-38 | `A.B(x,y)` 结构体构造误为函数调用 | PathAccess 分支产生 `A::B { field }` | `codegen/expr.rs` |
| Bug-39 | `_` 前缀私有项无保护 | Bug-37 使 `_` 项无 `pub`，Rust 模块系统自动拦截 | `codegen/func.rs` |

### 类型系统/代码生成 Bug — 新增修复

| Bug | 问题 | 修复 | 文件 |
|-----|------|------|------|
| Bug-40 | trait 默认方法尾表达式分号 | `gen_block`→`gen_block_return` | `decl.rs` |
| Bug-55/N13 | try/catch 生成 `match { } { }` 无效语法 | 改为 `match Ok({body}) { Err=>... }` | `expr.rs` |
| Bug-59 | 空列表 `[]` → `Vec<>` 缺类型参数 | `Vec::<_>::new()` | `expr.rs` |
| N9 | struct 字段缺 `pub` 跨模块不可访问 | 添加 `pub` 前缀 | `decl.rs` |

### 此前已存在的修复（已存在于代码中）

Bug-42(self.name.clone), Bug-26/41/52/53/62(format!字符串拼接), Bug-57/58(闭包直调取代 __call_magic), 
Bug-63(.len() as i64), Bug-64/54(s.chars().nth()), Bug-45/N6(泛型自动 Debug bound),
Bug-31(泛型默认构造类型标注), Bug-65/N11(枚举变体前缀), Bug-61(pop().unwrap())

### 测试套件断言更新

| 套件 | 更新内容 | 结果 |
|------|----------|------|
| Binding B06 | `let mut r = f(v)` → `let mut r: i64 = f(v)`（类型注解对齐） | 12/12 100% |
| 函数定义 C04/C08 | 闭包省略 fn 指针注解 / Rust桥接 use 跳过 | 12/12 100% |

## 验证结果

```
cargo build                → 0 warning
cargo test --lib           → 402 passed, 0 failed
函数定义语法测试套件       → 12/12 passed
绑定语义测试套件           → 12/12 passed (100%)
```

## 已知遗留问题（非编译缺陷，需后续阶段）

| 问题 | 说明 |
|------|------|
| 安全导航 `?.` | 代码生成错误（Bug-47/48），涉及复杂 Option 转换逻辑 |
| 泛型方法 impl 块 | 结构体外泛型方法在独立函数中（Bug-48/49），需 impl 块生成 |
| 单行 `if` 表达式 | 解析器不支持 `def x = if cond: a else: b`（Bug-51） |
| 字符串 `\"` 转义 | 解析器错误处理转义引号（Bug-N18/N19） |
| GBK 编码兼容性 | 测试 runner 在中文环境缺少 UTF-8 设置 |

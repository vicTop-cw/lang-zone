# Lang-Zong 编译器 — 测试报告 (TEST REPORT #002) — Phase 4

> 归档路径：`test-suite/20260722-02/reports/TEST_REPORT_002.md`
> 生成时间：2026-07-22 ｜ 测试驱动：`test-suite/20260722-02/run_tests.py` ｜ SUT：`target/debug/lang-zone.exe`

---

## 1. 测试范围

### 1.1 Phase 4 新增特性
1. **`panic(msg)` 表达式** — 编译期生成 `panic!("{}", msg)`
2. **`try/catch/else` 块** — 结构化错误处理，转译为 `match { body } { Err(pat) => ..., Ok(v) => ... }`
3. **多 catch 分支** — 支持枚举变体模式匹配 + guard 条件
4. **`else` 分支** — Ok 路径的独立处理逻辑

### 1.2 附带修复
- **c017 悬空 `^`** 修复（已在 Phase 3 末完成）
- **c106 畸形 f-string** 修复（已在 Phase 3 末完成）
- **`Expr::FieldAccess` 枚举变体推断** — `E.B` 在已知枚举类型上生成 `E::B` 而非 `E.B`

---

## 2. 测试结果摘要

| 维度 | 通过 / 总数 | 通过率 |
|------|------------|--------|
| **总体** | 51 / 51 | **100.0%** |
| 功能 Functional (F01-F15) | 15 / 15 | 100.0% |
| 边界 Boundary (B01-B11) | 11 / 11 | 100.0% |
| 构建块 BuildBlock (G01-G07) | 7 / 7 | 100.0% |
| 异常 Exception (E01-E06) | 6 / 6 | 100.0% |
| **错误处理 ErrorHandling (H01-H12)** | **12 / 12** | **100.0%** |

### Phase 4 新增用例明细

| ID | 标题 | 模式 | 结果 |
|----|------|------|------|
| H01 | panic 表达式代码生成 | rust | ✅ |
| H02 | panic + f-string 代码生成 | rust | ✅ |
| H03 | try/catch 基本 Err 捕获 | rust | ✅ |
| H04 | try/catch Ok 穿透 | rust | ✅ |
| H05 | try/catch/else 分支 | rust | ✅ |
| H06 | try/catch 多分支枚举变体 | rust | ✅ |
| H07 | try/catch 带守卫 if | rust | ✅ |
| H08 | panic in catch 代码生成 | rust | ✅ |
| H09 | 嵌套 try/catch | rust | ✅ |
| H10 | catch Err 模式匹配 | rust | ✅ |
| H11 | try 块多语句体 | rust | ✅ |
| H12 | try/catch/else Ok 路径执行 | rust | ✅ |

---

## 3. 实现细节

### 3.1 `panic(msg)` 表达式
- **AST**: `Expr::Panic(Box<Expr>)`
- **代码生成**: `panic!("{}", inner_expr)`
- **词法**: `panic` → `Token::Panic`

### 3.2 `try/catch/else` 块
- **AST**: `Expr::TryCatch { body, catches: Vec<MatchArm>, else_body: Option<Vec<Stmt>> }`
- **代码生成**: 
  ```rust
  match { body_value } {
      Err(pat1) if guard1 => handler1,
      Err(pat2) => handler2,
      Ok(__v) => else_handler,   // else 分支
      Ok(v) => v,                 // 默认透传
  }
  ```
- **catch 模式**: 复用 `parse_pattern()`，支持 `Ident`, `Variant(name, subs)`, `Wildcard`, `Tuple`, `Ok_`/`Err_`/`Some_`/`None_` 关键字模式
- **guard**: `catch pattern if condition:` → `Err(pat) if condition =>`

### 3.3 `Expr::FieldAccess` 枚举推断修复
- **问题**: `E.B` 在 `.lz` 中使用 `.` 语法，codegen 生成 `E.B`，但 Rust 需要 `E::B`
- **修复**: `gen_expr` 中 `FieldAccess` 检查 receiver 是否为已知类型名 + field 是否为已知枚举变体，若是则生成 `Enum::Variant`

---

## 4. 已知限制

### L1: try 块内 `Ok(expr)` 需要显式类型标注
- **现象**: `r = try: Ok(42) catch e: -1` → rustc 报 `E0282: type annotations needed`
- **原因**: Rust 无法从 `Ok(42)` 推断 `Result<T, E>` 的 `E` 类型
- **绕过**: 使用显式类型标注 `r: Result<int, str> = Ok(42)` 或将 `Ok(42)` 赋值给类型明确的变量
- **等级**: 与 Rust 行为一致的正常限制

### L2: catch 模式中自定义枚举变体的类型推断
- **现象**: `catch A():` 生成的 `Err(E::A)` 模式正确工作
- **注意**: 需确保 enum 在当前作用域中已定义

---

## 5. 回归验证

- **demo.lz**: ✅ 编译正常
- **test_hello.lz**: ✅ 编译正常
- **全部 39 个原有用例**: ✅ 零回归

---

*报告完。*

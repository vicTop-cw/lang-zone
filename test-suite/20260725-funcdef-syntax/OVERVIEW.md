# 函数定义语法双逗号修复 + 8 类测试套件 — 完成概览

## 任务目标
1. 将函数参数中的双逗号 `def name(..,,..)` 统一为单逗号 `def name(..,..)`；
2. 同步所有语法文档，保证文档与实际语法一致；
3. 编写覆盖 8 类函数/结构的全面测试，统一放进专用测试目录。

## 代码修改（src/）
| 文件 | 修改 |
|---|---|
| `src/parser/parser.rs` | `parse_params` 三类消费逗号处加"再遇逗号即报错"守卫；`parse_generic_params_rich` 拒绝泛型连续逗号；删死代码 `parse_generic_params` |
| `src/lexer/lexer.rs` | 修复 `#!bin macro` shebang 整行跳过（`peek_n(1)` 判定），修复宏系统因 `macro` 关键字碰撞而不可用 |
| `src/ast/decl.rs` | 注释中 `..,,..` → `..,..` |
| `src/typer/mod.rs` / `src/codegen/mod.rs` | `#[cfg(test)]` 的 `Module {..}` 初始器补 `magic_decls: vec![]`（否则 `cargo test` 编译失败） |

## 文档同步
- `hermes/05-声明与定义.md`、`ready/backup-hermes/09-函数增强设计.md`：`..,,..` → `..,..`
- `test_call.lz`：`..,,..` → `..,..`，`Int` → `int`

## 测试套件 `test-suite/20260725-funcdef-syntax/`
- `run_tests.py`：调用 `target/debug/lang-zone.exe` 解析并断言生成的 Rust 产物
- `cases/`：
  - 正向 8 类：C01 方法、C02 普通函数（默认参数/泛型/raises）、C03 嵌套函数、C04 匿名闭包、C05 仓颉宏、C06 宏模板、C07 装饰器、C08 Rust 宏
  - 负向 4 类：E01 both 双逗号、E02 params 双逗号、E03 trailing 双逗号、E04 泛型双逗号（均断言被拒且报错含"连续逗号"）

## 验证结果
- 专用套件：**12 通过 / 0 失败**
- `cargo build`：**0 warning**
- `cargo test`：**365 passed / 0 failed**

## 关键发现
- 宏系统此前整体不可用（lexer 未跳过 `#!bin macro` shebang），连带 C05/C06 失败；已修复。
- `Module.magic_decls` 字段（并行 magic 工作新增）的 `#[cfg(test)]` 初始器遗漏导致 `cargo test` 编译失败，已补全。

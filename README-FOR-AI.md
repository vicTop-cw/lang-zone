# README-FOR-AI — AI 接手本项目的规范

> 本文件面向 AI 助手（及新接手开发者），定义：项目路线、目录职责、工作流程、清洁规范、验证铁律。**请每次接手先读本文件 + `history-work/` + `issue/README.md`。**

---

## 一、项目定位

**ΣLang / LZ（lang-zone）**：`.lz` → Rust 的源码到源码编译器。
- LZ 是面向系统编程的静态类型语言：默认可变绑定、结构类型（duck typing）、魔法方法驱动运算符重载、构建块语法、编译期宏与 comptime。
- 与 sigma-lang 无关。

## 二、唯一技术路线（强制）

1. **只有一条 codegen 路线：AST → LZIR → Rust**（`src/ir/builder.rs` 构建 IR，`src/ir/codegen.rs` 生成 Rust）。
2. **不存在** `--ast-codegen` 老路子选项；不要在 AST→Rust 直接 codegen 上开发功能。
3. Cython 后端（`lzcyc`、`src/ir/codegen_cython.rs`、`src/codegen/`、`CY/` 目录）**不碰、不写代码**。
4. 运行时 builtins 由 **lz_builtins 子库**提供（workspace 成员）：生成代码 `use lz_builtins::*;`，**禁止**在 codegen 里内联重复 shims（`__Params`/`__spawn_task`/`__block_on` 等）。

## 三、目录职责

| 路径 | 职责 | 规则 |
|------|------|------|
| `src/` | 编译器源码（lexer/parser/ast/ir/bridge/…） | 唯一修改核心 |
| `SYNTAX/` | 语言规范文档（36 份） | 写语法功能前先读对应文档；改语法后必须同步文档 |
| `DEMO/` | 官方测试样例（01_basics … 16_testing + 99_errors 等） | **只放有效测试**；新增功能必须配 DEMO |
| `lz_builtins/` | 运行时内置子库 | 生成代码的 API 来源 |
| `div-tools/` | 有价值/可复用辅助脚本（py/ps1） | 辅助代码统一放这里 |
| `issue/` | 问题跟踪、设计决策、测试报告 | 新问题/决策/报告归档于此 |
| `history-work/` | 工作记录（谁做了什么） | 每次大改动追加记录 |
| `LZSTD/` `std/` `bootstrap/` `benchmark/` `RUST/` | 标准库/自举/基准等子系统 | 各归其位，不随意迁移 |
| `tests/` | Rust 集成测试 | `cargo test` |

## 四、工作流程

1. **先读**：`README-FOR-AI.md` → `history-work/` → `issue/README.md`（决策 + 开放问题）。
2. **规划**：多步任务先 `todowrite` 列出，逐步推进，完成后标记。
3. **先易后难**：用户要求"先易后难"时按难度递增排任务。
4. **改前先读**：修改任何文件前先 `read_file`，用 `edit_file`/`write_file` 改文件，**禁止** sed/重定向改源码。
5. **写完验证**（验证铁律）：生成的 `.rs` 必须能过 `rustc` 编译且运行正确，才算完成。

## 五、清洁规范（保持项目清洁）

- **临时测试文件**：随手创建的一次性测试（`_t*.lz`、`*.tmp` 等）测完即删；有价值的移入 DEMO 对应目录。
- **临时辅助代码**：用完即删；有价值/未来可能复用的移入 `div-tools/`。
- **缓存编译产物**：`target/`、`*.exe/*.pdb/*.o/*.rmeta`、`NUL.*`、`__pycache__` 等一律不提交、不保留（.gitignore 已覆盖）。
- **git 提交**：只提交源码、文档、测试、有价值的工具；提交信息 English + Conventional Commits。

## 六、测试规范

- DEMO 全量回归：逐个 `.lz` → IR codegen → `rustc --edition 2021 --extern lz_builtins=<rlib>` 编译 → 运行。
- 排除 `DEMO/99_errors/`（故意错误语法演示，预期报错）。
- 失败先查测试文件是否正确（是否过时/用错语法），修正后仍失败的才算编译器 bug，记录到 `issue/` 测试报告。
- `lz_std/` 的 DEMO 测试**暂不处理**（2026-08-08 用户决策），避免混淆。

## 七、版本与发布节奏（ΣLang 长期规则）

- 每个小阶段 = 一次版本推进（`src/util/version.rs` VERSION_PATCH，v0.133 起）。
- 每 10 小阶段同步一次仓库（git 提交 + push）。
- 每 100 小阶段发布一次 PyPI（100 小阶段 = 0.0.1）。
- 完全自主自由发挥演化，不弹询问（除非涉及风险操作）。

## 八、已知注意点（避免踩坑）

- `IrType` 无 `List/Dict/Set` 变体——用 `Named { path, args }` 表达。
- `IrType::Any` 在 rust_type 映射为 `"i64"`（fallback）。
- duck 检查器在 `build_ir` 末尾运行，报错会阻止 codegen。
- 关键字实参语法是 `name: value`（`:`）或 `name~` 糖，不是 `=`。
- 生成代码链接 builtins：`rustc --extern lz_builtins=<rlib>`（rlib 由 `cargo build -p lz_builtins` 产出）。
- 完整坑清单见项目记忆 / `issue/`。

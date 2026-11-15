# lzc 老路线备份（Legacy AST→Rust codegen）

**备份日期**: 2026-07-31
**路线决策**: [issue/decision-ir-first-route.md](../../issue/decision-ir-first-route.md)

## 这是什么

当前 `lzc` 的 **AST → Rust 直接 codegen 老路线** 的代码与产物快照。

- `src_codegen/` — 老路线代码生成器源码（`src/codegen/` 的 11 个文件：builders / decl / derive / export / expr / func / helpers / magic / mod / stmt / variadic）
- `lang-zone-legacy.exe` — 老路线编译出的 lzc 可执行文件快照（`target/debug/lang-zone.exe` 复制）

## 定位

> **仅作 IR 路线代码生成参考。** 老路线生成的代码仅供参考对照，**以后会丢弃**，不参与维护。

- 新管线：`AST → build_ir → LZIR → (codegen.rs → Rust / codegen_cython.rs → Cython)`
- 老路线的角色：仅用于对比 IR 路线输出、迁移差距清单（`issue/decision-ir-first-route.md` §差距清单）时对照
- **不要在老路线上新增功能或修复 bug**

## 与工作区的关系

- 本目录是独立快照，与 `src/codegen/` 的当前内容可能不同步（快照时点的状态）
- 迁移完成（`issue/decision-ir-first-route.md` §迁移步骤 P2）后，工作区 `src/codegen/` 将被删除，本备份可保留或一并清除

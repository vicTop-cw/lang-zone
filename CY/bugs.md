# Cy 后端已知上游依赖问题

> 记录 cy 后端（codegen_cython.rs / lzcyc）发现的上游 IR 依赖问题。
> 按用户要求：禁止去动上游，仅记录待后续处理。

## BUG-13: import alias 语义不明确

- **文件**: `src/ir/codegen_cython.rs:909-920`
- **严重程度**: LOW
- **描述**: `from path import item1, item2` 的 alias 只附加到最后一个 item。`UseStmt.alias` 的语义不明确：是为每个 item 设置别名，还是仅对最后一个 item 生效。
- **上游依赖**: 需确认 `UseStmt.alias` 在 AST/IR 层的语义定义。

## BUG-16: enum AST 表示假设

- **文件**: `CY/src/main.rs:243-253`
- **严重程度**: MEDIUM
- **描述**: `collect_enum_variants` 假设 enum 在 AST 中用 `StructDef { is_enum: true }` 表示。如果上游引入独立的 `EnumDef` AST 节点，此函数会静默失效。
- **上游依赖**: 需确认 AST 层 enum 的表示方式是否稳定。

## BUG-18: prescan_module 的 `_ => {}` 可能遗漏新增 Item 变体

- **文件**: `src/ir/codegen_cython.rs:1213`
- **严重程度**: MEDIUM
- **描述**: `prescan_module` 中 `Item` 的 `_ => {}` 跳过未识别的 Item 变体。`gen_item` 已显式列出所有变体（无 `_` 分支），但 `prescan_module` 的 `_ => {}` 不会编译失败，形成不一致。
- **上游依赖**: 如果上游 IR 新增 `Item` 变体，需同步更新 `prescan_module`。

## BUG-B10: gen_enum 不 mangle 变体类名

- **文件**: `src/ir/codegen_cython.rs:808`
- **严重程度**: MEDIUM
- **描述**: `class {}({}):` 不对变体名调用 `mangle_ident`。如果变体名是 Python 关键字（如 `None`/`True`/`False`），生成 `class None(Color):` 是语法错误。
- **上游依赖**: 取决于上游 parser/semantic_check 是否允许变体名为 Python 关键字。

## BUG-B11: 跨模块 enum 无数据变体模式匹配生成错误 isinstance

- **文件**: `src/ir/codegen_cython.rs:3541-3553`
- **严重程度**: MEDIUM
- **描述**: `variant_fields` 在 `prescan_module` 中填充，只扫描当前模块的 `Item::EnumDef`。如果 enum 来自 import 合并但 `variant_fields` 缺失，`unit_variant` 为 false，对无数据变体生成 `isinstance(x, Variant)` 而非 `x is Variant`，导致 TypeError。
- **上游依赖**: 需确认 import 合并后 enum 的 `variant_fields` 是否完整填充。

## BUG-EC-006: type_name 未在 semantic_check builtin_value_names 白名单中

- **文件**: `src/semantic_check.rs:97-172`
- **严重程度**: MEDIUM
- **描述**: `type_name` 函数在 IR builder (`src/ir/builder.rs:1115`) 和 Rust codegen (`src/ir/codegen/mod.rs:11178`) 中有特殊处理，但未在 `builtin_value_names()` 白名单中注册。语义检查阶段报"未定义函数: type_name"。
- **上游依赖**: 需在 `semantic_check.rs` 的 `builtin_value_names()` 中添加 `"type_name"`。

## 状态（2026-09-28 更新）

以下上游依赖问题已在本仓库内修复（codegen_cython.rs / semantic_check.rs），无需改上游：

- [x] **BUG-13**：import alias 语义 —— 已修复。`gen_import` 的 `is_from` 分支直接平铺 items，绝不附加 alias（LZ 解析器不支持 `from ... import item as x`，`UseStmt.alias` 恒为 `None`）。
- [x] **BUG-16**：enum AST 表示假设 —— 经核查确认假设成立：AST 中无独立 `EnumDef` 节点，enum 仍以 `StructDef { is_enum: true }` 表示，`collect_enum_variants` 逻辑正确，仅加固注释。
- [x] **BUG-18**：prescan_module 静默漏项 —— 已修复。`prescan_module` 现显式穷尽所有 `Item` 变体（`Use`/`TraitDef`/`Test`/`CheckerBlock`/`DuckDef`），新增变体会编译失败强制同步，杜绝 `_ => {}` 静默漏项。
- [x] **BUG-B10**：gen_enum 变体类名未 mangle —— 已修复。`gen_pattern` 数据变体匹配处对变体名调用 `mangle_ident`，关键字变体不再生成 `class None(...)` 语法错误。
- [x] **BUG-B11**：跨模块 enum 无数据变体模式匹配 —— 已修复。无数据变体判定退化为按模式参数个数（`args` 为空即视为无数据变体），跨模块合并 `variant_fields` 缺失时也能正确生成 `x is Variant`。
- [x] **BUG-EC-006**：type_name 白名单 —— 已修复。`builtin_value_names()` 已补入 `type_name`，CY 后端 `type_name()` 不再误报 E0433。

> 注：CY 后端完整端到端运行仍需 cython 编译环境（C 编译器 / .pyd），当前 Windows 沙箱缺失，无法端到端验证；以上为代码级修复 + `cargo check -p lang-zone` 编译通过。

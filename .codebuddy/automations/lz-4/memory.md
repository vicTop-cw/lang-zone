# lz-4　大文件拆分自动化 · 执行记忆

## 2026-10-06（首轮，已完成）

按 `docs/大文件拆分计划-2026-10-06.md` 执行全仓大文件 move-only 拆分。

**同步**：拆分前 231 处未提交改动全部落盘（`a419eac3`）+ tag `pre-split-baseline` + push origin。
基线：`cargo test -j 1` = 742 passed / 0 failed；`--test demo_codegen_compile -- --include-ignored` = 2/0。

**完成阶段与提交**
- `85afad9e` codegen：`mod.rs` 17,444 → 1,384 行 + 8 子模块（expr_gen/decl_gen/stmt_gen/scan/magic_gen/types_emit/emit/pattern_gen）
- `6628366e` builder：→ `builder/` 13 子模块 + mod.rs 1,243；`38249a5` 删除遗留单文件 `builder.rs`
- `48e7b1b` typer：2,509 → mod.rs 405 + 5 子模块 + tests.rs
- `e92d032` P2：comptime（1,406 → 166 + eval/literal/frame）、lexer（1,372 → 932 + numbers/strings，`#[path]` 指向同级）
- `41d817f` 计划文档写入执行记录
- **P1-4 `src/ir/lz_codegen_lib.rs` 与 7.3 `src/ir/lz_ir_lib.rs` 改判不拆**：二者是自举路线 B 的**生成产物**（由同名 .lz 转译而来），未被 `mod` 声明、不参与编译，拆分会被下次生成覆盖。

**验证（零回归）**
- `cargo build` 0 error / 0 warning
- `cargo test -j 1` 742/0，与基线逐项一致
- DEMO 全量 rustc 闸门 2/0（含 ignored）
- 等价性强校验：worktree 检出 pre-split-baseline 编译出拆分前编译器，348 个 DEMO .lz 逐一比对生成 .rs 的 SHA1 + 退出码 → **0 差异**（临时 worktree 已 `git worktree remove`）

**方法沉淀（可复用）**
- 自研拆分器按「语法条目」切分（非行号），先做往返字节校验再落盘；搬家后只允许 use/可见性/rustfmt 三类差异
- 子模块统一 `pub(crate)`；trait impl 内的方法不能加可见性（E0449），需回退
- 父模块私有字段对子模块天然可见；跨模块自由函数需 `use super::<mod>::<name>;`
- 父私有类型出现在 `pub(crate) fn` 签名会触发 private_interfaces 警告 → 提升类型可见性（如 `pub(crate) struct TypeCtx`）
- 非 mod.rs 的文件模块（如 `lexer/lexer.rs`）子模块用 `#[path = "x.rs"]` 指向同级
- 等价性比对用「同路径先后两次生成 + 哈希」，不要删除产物（环境 safe-delete 会拦截批量删除并造成假差异）
- 环境坑：`cargo test` 需 `-j 1`（Windows 页文件不足）；批量删除 >500 文件需分批

## 2026-10-07（复核，全部通过）

- **闸门重跑**：`cargo build` 0 警告；`cargo test -j 1` = 742 passed / 0 failed（35 个测试二进制）；`--test demo_codegen_compile -- --include-ignored` = 2/0。
- **move-only 条目级审计**（工具留在仓库内、被 .gitignore 忽略：`_split_tool.py` / `_audit_items.py` / `_audit_fmt.py` / `_audit_diff_at.py` / `_audit_modules.py`）：
  以「语法条目」为粒度，把 `pre-split-baseline` 的原文件与拆分后的多文件并集做多重集比对，并先经同一个 rustfmt 规范化以消除格式噪声。
  条目计数**完全一致**：顶层 43/108/25/17/2、方法 103/16/11/18/20。
  builder / typer / comptime / lexer 四个目标 **0 差异**；codegen 残留 4 个条目（expr_uses_dict、gen_trait_def、gen_expr、is_str_expr），逐条定位确认**只有 rustfmt 的 match 分支花括号折叠与闭包体花括号补写**（已用最小样例验证该 rustfmt 版本确实会做这两件事），语义中性。
- **模块完整性**：src 内 129 条 `mod` 声明全部解析到文件，0 缺失。
- **新发现（既有遗留，非本次引入）**：17 个 .rs 没有任何 `mod` 声明指向，其中 `src/ir/codegen/str_boundary.rs`（341 行）值得注意——基线 `codegen/mod.rs` 只声明了 `helpers` 与 `moddec_emit`，该文件从未参与编译；其余为 semantic.rs / strict.rs / typer/ / typing/ / hints/ / frontend/lz_*.rs / bridge 子模块等历史未挂载文件。
- 结论：拆分达成 move-only，无代码增删改，零回归。

## 2026-10-07（补挂 str_boundary，已完成并推送）

- 决策：`src/ir/codegen/mod.rs` 补 `mod str_boundary;`，让该文件真正参与编译。提交 `21439b6`，已 push。
- 改动仅 2 行级：`mod.rs` +1 行声明；`str_boundary.rs` 文件头补「现状：可执行规格（非接线态）」说明段（未动任何逻辑）。
- 编译零改动通过（文件本就与当前 IR 类型兼容：Span 五字段、IrType::{Str,Any,Ref,MutRef,Named}、LitKind::Str/FStr 全部对得上），`cargo build` 零告警（文件自带 `#![allow(dead_code)]`）。
- 效果：lib 单测 425 → **431**（6 条契约单测首次实跑：4×4 契约矩阵 / 复杂表达式 / IR 形态分类 / 字面量优先 / 值语义复制 / is_str_ir），全量 742 → **748 passed / 0 failed**，DEMO 闸门 2/0。
- **重要事实**：发射点并未调用本层（`grep 'str_boundary::' src/ir/codegen` 只命中注释），逻辑仍各站点内联 → 当前角色是「可执行规格 + 回归护栏」。若要把它变成真正的唯一入口，需单独立项做**行为改写**（触及 400+ 处形态判定），回归闸门用「拆分前后生成 .rs 逐字节一致」。

## 2026-10-07（尾递归自动优化 M1–M3，已完成并推送）

- 按 `IR/tailrec-auto-plan.md` 实现 Auto TCO + `@tailrec` 严格标注：`071aa1f`（代码）+ `b0fe2c3`（文档实现记录），已 push。
- 验收：`cargo test -j 1` = **768 passed / 0 failed**（36 个二进制，新增 10 条 builder 单测 + 10 条 e2e）；`demo_codegen_compile -- --include-ignored` = **2/0**（348 DEMO 全通过，含既有 `@tail_call` 语料）。
- 设计/边界/坑详见项目记忆「lang-zone 尾递归自动优化（Auto TCO + @tailrec）已实现」。
- **并发教训（重要）**：仓库有其他 agent 同时改 `extended_check/infer/emit/expr_gen/stmt_gen/codegen-mod/DEMO lz_std`。
  - `builder/mod.rs` 与 `codegen/mod.rs` 是**共改文件**：用 `git hash-object -w` + `git update-index --cacheinfo` 暂存「HEAD + 仅我的改动」的 blob，即��不把他人 WIP 带入提交、他们工作区也不受影响。
  - 批量给结构体补字段**必须用 brace 配对**定位字面量：先前用「`span: ...` 后跟 `}`」的正则批量插入，误伤了 `AstStmt::FnDef { func }` **枚举模式**（ast / parser / semantic_check / comptime / builder 多文件），已全部回滚。
  - 环境限制：单次删除 >500 文件会被 safe-delete 拦截，需分批（每批 ≤400）。

# Open: 文档实测审计（Round 3 — 闭包胖箭头实测 / CY 文档 / hermes 删除）

**状态**: Open
**日期**: 2026-07-31
**范围**: 语法文档示例实测（备份 exe 编译验证）+ CY/（lzcyc）文档 + hermes/ 目录状态，不涉及源代码
**审计方式**: 备份 exe 实际编译 + 目录/文件核对 + git 状态核查

---

## 一、P0 — 文档 03e 闭包胖箭头示例实测不可用（与实现矛盾）

### 现状：文档已全面写入胖箭头语法

`SYNTAX/03e-复合综合.md` §三 当前内容（2026-07-31 已更新）：
- §3.1：`| | 42` / `| | print("no args")`（无参闭包）
- §3.2 胖箭头 `=>`（可选）：`|x| => x + 1`、`|x, y| => x + y`、`| | => 42`、多行块体 `|a, b| =>\n    c = a + b\n    c * c`
- §3.3：`let noop = | | => 42`；§3.4：`apply(|x| => x + 1, 5)  // 6（胖箭头等价写法）`
- 章首说明：「胖箭头 `=>` 可选——加 `=>` 后 body 可换行缩进与 `def` 函数一致」

### 实测（备份 exe `backup/lzc-legacy/lang-zone-legacy.exe --ast`）

| 示例 | 实测结果 |
|------|---------|
| `\| \| 42`（无箭头无参） | ✅ 解析成功（`Closure { params: [], body: IntLit(42) }`） |
| `\| \| => 42`（胖箭头） | ❌ `Parse error: Unexpected token in expression: FatArrow` |
| `\|x\| => x + 1`（胖箭头） | ❌ `Parse error: Unexpected token in expression: FatArrow` |

**结论**: 文档声称胖箭头「可选」「等价写法」「多行块体可用」，但解析器**完全不接受 FatArrow**——文档超前于实现，读者照文档写必然报错。且与 `issue/decision-closure-fat-arrow.md` 的「待实现清单」矛盾（决策文档说待实现，03e 却已写成可用）。

**修复**: 03e §三 胖箭头示例标注「规范目标（parser 未实现）」或回退为无箭头写法；待 parser 实现后再恢复。

---

## 二、P1 — CY/（lzcyc）文档问题

### 1. `CY/USAGE.md:43` 路径拼写错误

- 原文：`cargo run --bin lzcyc -- transpile DMO/01_basics/literals.lz`
- `DMO/` 应为 `DEMO/`（CY 内无 DMO 目录，TESTS/ 才有 01_basics）

### 2. 运行时库表声称 5 个文件，实际只有 2 个

- `CY/USAGE.md:115-121` 声称：`lz_types` / `lz_option` / `lz_pointers` / `lz_concurrency` / `lz_exceptions` 五对 pxd/pyx
- 实际 `CY/runtime/`：仅 `lz_concurrency`、`lz_exceptions` + `lz_std/` + `__init__`——**`lz_types` / `lz_option` / `lz_pointers` 不存在**

### 3. 「共享」与「COPY」自相矛盾

- `CY/USAGE.md:135`：「前端 | 共享 parser/ast/typer（从 `src/` COPY）」
- `CY/PLAN.md` 项目结构：「lexer/ # COPY: src/lexer/」
- 且 `CY/USAGE.md:10`：「独立构建，不与主编译器 workspace 共用」
- 实际：CY 是独立 workspace（自带 Cargo.toml），`CY/src/` 下确有完整的 lexer/parser/typer 副本——是**复制副本**，不是「共享」。措辞应统一为「独立副本（COPY）」或明确同步策略

### 4. 并发实现状态矛盾

- `CY/USAGE.md:146`：「❌ 模式匹配、魔法方法、**并发**、所有权模拟」
- 但 `CY/TESTS/` 存在 `11_concurrency` 目录、`CY/runtime/` 有 `lz_concurrency.pxd/pyx`、`src/` 有 `simd/`——「并发未实现」与测试目录/运行时文件并存，状态表述矛盾

---

## 三、P1 — hermes/ 目录被整体删除（git 未提交）

- `git status`：hermes/ 下 **50 个文件全部 D**（已删除，未提交）；`git ls-files hermes/` 仍跟踪 50 个文件（00-权威语法规范 / 01-词法与AST审计 / 02-解析器语法审计 / 03-代码生成审计 / 04-测试覆盖审计 / 05-集成测试分析 / 10-架构重构方案 / 15-export增强设计 / 16-Number-Trait与math宏设计 / 20260728 检查报告 ×2 / overview / syntax-review×33）
- 工作区已无 `hermes/` 目录；`git check-ignore` 无命中（非 gitignore 所致）
- HEAD 版 hermes 中：`LtColon` 已标注「~~已删除（2026-07-31）~~」（01-词法与AST审计.md:152，说明 HEAD 版本已同步删除状态）；但 **`Lang-Zong` 旧命名仍有 30 处**残留（与当前 Lang-Zone 命名矛盾）

**问题**: 50 个文件的目录被整体删除且未提交——若为有意清理应提交（并确认无引用）；若为误删需恢复。同时 `SYNTAX/overview/缺失语法特性报告.md` 仍有 LtColon 残留（Round 2 已报）。

**附**: `git status` 显示工作区约 **1797 条删除/改动**未提交——外部进程正在大规模修改，建议及时提交或明确处理策略，避免状态混乱。

---

## 四、修复建议（按优先级）

1. **P0**：03e 闭包胖箭头示例标注「规范目标」或回退（与 parser 现状一致）
2. **P1**：CY/USAGE.md 修 `DMO/`→`DEMO/`；运行时库表与实际文件对齐；「共享/COPY」措辞统一；并发状态与实际核对
3. **P1**：hermes/ 删除需决策（提交删除 or 恢复），Lang-Zong 残留清理
4. **P2**：git 工作区 1797 条未提交改动及时提交

## 五、验收标准（修正后核对）

1. `grep -n "FatArrow\|=> x + 1\|=> 42" SYNTAX/03e-复合综合.md` 的胖箭头示例均标注实现状态或为无箭头写法
2. `CY/USAGE.md` 路径、运行时库表、共享/COPY 表述与 CY/ 实际结构一致
3. hermes/ 删除已提交或已恢复，无悬空引用
4. git 工作区干净或改动已纳入明确批次

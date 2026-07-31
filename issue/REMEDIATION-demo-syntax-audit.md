# 整改收官：DEMO × SYNTAX 交叉审计（demo-syntax-audit-2026-07-31.md）

**日期**：2026-07-31（续 13:14）
**审计源**：`issue/demo-syntax-audit-2026-07-31.md`（14 项发现，P0–P2）
**原则**：以实测为准，不盲信审计；规范 = 红线，编译器若接受更松语法则记录为独立 issue。

---

## 一、审计发现 vs 实测结论

| # | 审计指控 | 实测 / 处置 | 状态 |
|---|---------|------------|:---:|
| 1 | 99_spec/README 谎称 38/38 编译成功 | README 已改为 **37/37（31 主 + 6 combo）已纳入 compile_demos 正面测试覆盖**，并如实标注"解析通过即标记 ✅" | ✅ |
| 2 | 清单列 `duck_demo.lz`（文件不存在） | 已更正为 `duck_demo.lz` 实际在 `99_errors/`，README 清单已同步 | ✅ |
| 3 | 两份 README 对 99_spec 状态打架 | `DEMO/README.md` 已写 37，`99_spec/README.md` 写 37，一致 | ✅ |
| 4 | 推导式引用未定义变量 | `dict/set_comprehension.lz` 已补 `users`/`numbers`/`text` 局部定义 | ✅ |
| 5 | combo let 重赋值 / 跨函数引用 | `combo_while_guard_walrus.lz` 改模块级 `count`+`next()` 推进（消除无限循环）；`combo_while_guard_else.lz` 的 `count` 移到模块级修复跨函数不可见 | ✅ |
| 6 | combo 出处章节号错误 | walrus 文件 `§4.2(else)`→`§3.5(while 守卫)`；else 文件 `§四+§4.2` 本就正确 | ✅ |
| 7 | extractor 位置构造违反文档 | `extractor_unapply.lz` 已用 `Point(x:3, y:4)` 关键字构造 | ✅ |
| 8 | 匿名 `duck {...}` 语法（按用户拍板：DEMO 对齐规范） | `duck_test.lz` 改为命名式 `duck Pet = .name:str; .age:int` + `pet: Pet`，与 `01-类型系统.md §九` 一致 | ✅ |
| 9 | duck_demo 自述/字段名错误 | 前序已修：自述 99_spec→99_errors；`Counter(value:10, count:0)`→`_count:0` | ✅ |
| 10 | owned 注释与行为不符 | `02_variable_errors.lz:14` 改为如实"解析器忽略 owned，退化为 let s（不报错）" | ✅ |
| 11 | 异体字（滿/演/签） | **误报**——实测无错字，跳过 | ⏭ |
| 12 | `缺失语法特性报告.md:102` `struct Point:` | 改为 `struct Point =`（与 06a §七一致） | ✅ |
| 13 | 命名 Lang-Zone / Lang-Zong 混用 | 全局统一 **Lang-Zone**（git diff 38 文件 / 54 处替换，Bash 视图 0 残留） | ✅ |
| 14 | DEMO/README 计数过时 | line 29 已 37；补注"合计 49 = 8 份规范 ❌ 错误边界示例总数（非文件数）" | ✅ |

> **审计根因修正**：#1 审计称 `top_level_build.lz`「config 从未定义」。实测生成的 `.rs` 有 `let config = (||unsafe{...})()`——config **有定义**。真正坏代码是 `println!` 打印元组（E0277，非顶层构建块 bug）。README 已不再谎称 rustc 编译成功，状态准确。

---

## 二、本次修改的文件（节选）

- `DEMO/99_spec/duck_test.lz` — 匿名 duck → 命名 duck（对齐 §九）
- `DEMO/99_spec/combo-syntax/combo_while_guard_walrus.lz` — 修章节号 + 消除无限循环
- `DEMO/99_spec/combo-syntax/combo_while_guard_else.lz` — `count` 移模块级（修跨函数）
- `DEMO/99_spec/combo-syntax/{combo_for_guard_match, combo_for_guard_walrus, combo_while_guard_try}.lz` — 清"未实现"陈旧头
- `DEMO/99_errors/02_variable_errors.lz` — owned 注释如实化
- `SYNTAX/overview/缺失语法特性报告.md` — `struct Point:` → `=`
- `DEMO/README.md` — 补 49 计数口径说明
- 全局 `.md`/`.lz`：`Lang-Zong` → `Lang-Zone`（37 份文档）

---

## 三、验证结果

- **99_spec 全量 transpile**：`lang-zone.exe` 实测 **38/38 全部通过**（0 失败）。
- **负例文件**：`duck_demo.lz` / `02_variable_errors.lz` 属 `99_errors`（`reject_errors` 测试预期拒绝），FAIL 即正确行为。
- **文档版本头**：`SYNTAX/check_doc_versions.py` → `[OK] 全部语法文档版本头合规（规范版本 3.2）`。

---

## 四、遗留事项（不在文档岗范围）

- working tree 仍有大量 **未提交**改动（含 `src/lexer/lexer.rs` 字面量 `LexError` 修复、IR 迁移等），需工程侧 review 后提交。
- 用户拍板「编译器接受更松语法的，记录为独立 issue」：`duck` 约束求解仍为规范目标（匿名 duck 解析通过但未强制），可另立 issue 跟踪。

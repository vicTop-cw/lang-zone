# Open: 文档跟进审计（Round 2 — 新写入文档 / 关键字表 / DEMO 覆盖抽查）

**状态**: ✅ 已全部修复（2026-07-31 16:20）

## 修复记录

| # | 问题 | 修复 |
|---|------|------|
| 1 | README.md `__call__` "已实现"→实测未接线 | 改为"**未接线**（生成原样调用，rustc E0618）" |
| 2 | decision-closure-fat-arrow "关键结论"有歧义 | 补实现状态表（`\| \|`✅, `=>`❌） |
| 3 | 附录B 缺 `and`/`or`/`not`/`is`/`in`/`iterator` | 补 §1.10 逻辑关键字 + §1.11 同一性/成员 + §1.5 补 `iterator` |
| 4 | DEMO/README 特性声明不实 | prelude_demo 删 panic; test_suite 改 test/assert; struct.lz 改泛型 |
**审计方式**: 实测编译验证（备份 exe）+ 全文 grep 交叉核对 + 文件内容抽查

---

## 一、P1 — 新写入文档与实测不符

### 1. `README.md:67` 差距清单 #4 描述错误（`__call__` 直调）

- README 原文：「`__call__` 直调 `ins()` | **已实现**（生成 `ins.__call__(..)` 需验证）」
- **实测**（2026-07-31，`/tmp/lztest/call_test.lz` → `call_test.rs`）：AST 路线生成的代码是 `doubler(21)` **原样保留**，并非 `ins.__call__(..)`；rustc 编译报 **E0618**（expected function, found Multiplier）
- 结论：README 的「已实现」与「生成 `ins.__call__(..)`」均不成立——`__call__` 实际**未接线**，生成的是坏代码
- 修复：改为「未接线（生成原样 `ins(..)` 调用，rustc E0618）」

### 2. `issue/decision-closure-fat-arrow.md` 断言「当前即可解析」部分有误

- 决策文档「关键结论」：「`| |` … **当前即可解析**，只是文档未定义」
- **实测**（备份 exe `lang-zone-legacy.exe`）：
  - `| | 42`（无箭头）→ `--ast` **解析成功**（`Closure { params: [], body: IntLit(42) }`）✅
  - `| | => 42`（带胖箭头）→ `Parse error: Unexpected token in expression: FatArrow` ❌
- 结论：断言只对**无箭头**写法成立；**胖箭头版本需 parser 实现**（文档待实现清单第 1 条已写，但「关键结论」的表述会误导读者以为现成可用）
- 修复：关键结论改为「无箭头 `| | body` 当前可解析；`| | => body` 胖箭头需按待实现清单实现」

---

## 二、P1 — 附录B 关键字全集缺失 5 个真实关键字

- `SYNTAX/00-词法基础.md` 明确列为关键字的：`and`（L130）、`or`（L131）、`not`（L132）、`is`（L135）、`iterator`（L86，生成器函数定义关键字）
- `SYNTAX/附录B-关键字保留字符号语法边界.md` 自称「**一、关键字全集**」，但全文 grep：
  - `and`：0 次；`or`：0 次（仅出现在「内建 vs 关键字」元说明中）；`not`：0 次；`is`：0 次（仅 `raise`/`raises` 子串）；`iterator`：0 次
  - 连 §三 运算符表也没有（`&&`/`||`/`!` 符号有，但关键字等价写法 `and`/`or`/`not` 未列）
- 影响：读者查「关键字全集」会漏掉逻辑运算符关键字与生成器关键字；与 00-词法基础、12-操作符（`and`/`or`/`not`/`is` 在优先级表 L380-383）矛盾
- 修复：附录B §一 补 5 个关键字行（或改为引用 00-词法基础关键字表）

### 附：首轮「两套优先级表」问题已被外部修复（好消息）

- 首轮报告（doc-links-and-demo-readme #1）指出 12-操作符.md 与附录B 优先级表数值矛盾
- 现附录B §三 已改为：「**单一权威来源**：完整优先级表见 12-操作符.md §二，本文件不再维护独立副本」——与 12-操作符.md:372 的声明一致，矛盾已消除 ✅

---

## 三、P2 — DEMO/README 覆盖声明抽查（补充首轮未覆盖文件）

### 3. `prelude_demo.lz`：README 声称覆盖 `panic`，实际无

- `DEMO/README.md:120`：「prelude_demo.lz | 86 | print/**panic**/len/str/int/float/bool/hash/contains/iter/enumerate/zip/clone/sort/reverse/format、str 方法、List 方法」
- 实际（86 行，行数 ✅ 一致）：grep 到 print×22 / sort×4 / clone×3 / format×3 / hash×3 / str×3 / int×3 / bool×3 / zip×2 / enumerate×2 / iter×2 / contains×2 / len×2 / float×2 / reverse×3——**`panic` 0 次**
- 修复：README 删除 panic 或文件补 panic 示例

### 4. `test_suite.lz`：README 声称覆盖 setup/teardown/suite/check，实际只有 assert

- `DEMO/README.md:119`：「test_suite.lz | 84 | test 静态/动态名、suite、**setup/teardown**、SuiteOps 组合（+/[]/-）、**assert/check**、遍历测试」
- 实际（9 行，行数 84→9 首轮已报）：grep `setup|teardown|suite|check` 均 0 命中，仅 `assert` ×2（L6、L9）
- 修复：README 特性清单按实际刷新，或补全文件

### 5. `struct.lz`：README 声称覆盖 `__new__`/`__init__`/ZST/嵌套构造，实际只有基础 struct

- `DEMO/README.md:100`：「struct.lz | 110 | struct 字段/方法/静态方法/泛型/where、**`__new__`/`__init__`**、**嵌套构造**、**ZST**」
- 实际（37 行，行数 110→37 首轮已报）：仅基础 struct（Point/Rectangle/Pair<T> 泛型），grep `__new__`/`__init__`/ZST 均 0 命中
- 修复：README 特性清单按实际刷新

---

## 四、修复建议（按优先级）

1. **P1**：`README.md:67` #4 描述改为实测结论（未接线/E0618）
2. **P1**：闭包决策文档「关键结论」区分「无箭头可解析 / 胖箭头需实现」
3. **P1**：附录B §一 补 `and`/`or`/`not`/`is`/`iterator` 5 个关键字（或引用 00）
4. **P2**：DEMO/README 三个文件特性声明与文件内容对齐（#3/#4/#5）

## 五、验收标准（修正后核对）

1. `grep "__call__" README.md` 的描述与实测生成物一致
2. `issue/decision-closure-fat-arrow.md` 对无箭头/胖箭头的解析状态描述准确
3. `grep -c "and\|or\|not\|is\|iterator" 附录B` 关键字全集表 ≥ 1（或显式引用 00）
4. DEMO/README 声称的每个特性在对应 .lz 文件中可 grep 命中

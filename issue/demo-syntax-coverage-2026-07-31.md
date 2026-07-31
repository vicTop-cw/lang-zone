# Open: DEMO 语法构造 vs 语法文档覆盖缺口审计（2026-07-31）

**状态**: Open
**日期**: 2026-07-31
**范围**: DEMO/ 全部 .lz 代码中使用的语法构造 vs SYNTAX/ 文档是否提及（含实现状态一致性），不涉及源代码
**审计方式**: 全量符号/关键字提取 → 逐一对照 SYNTAX 文档 → 回源核实上下文排除误报

---

## 一、确认的覆盖缺口与状态矛盾

### 1. `__unapply__` 提取器：主文档未收录，DEMO 却在使用

**证据**:
- DEMO 使用：`DEMO/99_spec/extractor_unapply.lz`（`magic __unapply__(self) -> (int, int)`），`DEMO/99_spec/README.md:13` 标 `✅`
- SYNTAX **主文档体系（00-06g，含 06d 魔法方法全表）无任何 `__unapply__` 定义**
- 仅 `overview/缺失语法特性报告.md:14,95-105` 提及，且状态为「**真实缺失**（无任何可用支持）」P2-1，其示例本身还用了错误语法 `struct Point:`（L103，应为 `=`，见 07-31 文档审计 #12）

**矛盾点**: 文档主体未定义该魔法方法（唯一提及处称其"真实缺失"），但 DEMO 已使用并标 ✅——文档主体系与 DEMO 状态完全脱节。`__unapply__` 是否真的已实现、语法是什么，文档无可查证处。

---

### 2. 装饰器状态：文档标「设计阶段」，代码与 DEMO 已实现

**证据**:
- `SYNTAX/03e-复合综合.md:60-64`：`@simd`/`@parallel`/`@tail_call`/`@unsafe`/`@overload` 全部标「🔧 设计阶段」
- `DEMO/99_spec/parallel_decorator.lz:1`：「@parallel 装饰器 — 已实现（生成 #[cfg(feature="rayon")] + rayon::prelude）」
- `DEMO/99_spec/README.md:23`：`parallel_decorator.lz | @parallel 装饰器 | ✅`
- git HEAD 提交信息即为「feat: ... decorator real codegen (@simd/@parallel/@tail_call)」

**矛盾点**: 文档说设计阶段未实现，代码库已提交实现、DEMO 已覆盖并标 ✅——**文档落后于实现**。且 `@unsafe`/`@overload` 在 03e 只有状态行无语法定义，DEMO/04_functions/composite.lz 却声称覆盖「9 种装饰器」并实际使用。

---

### 3. `~` 后缀命名参数糖：文档有定义，但 DEMO 注释称「解析器未实现」

**证据**:
- `SYNTAX/12-操作符.md:310-318`（§1.19）：`~` 后缀命名参数糖有完整定义与示例（`f(b~)` 等价 `f(b = b)`）
- `DEMO/99_spec/tilde_named_arg_1.lz:3`：「规范目标特性（**当前解析器未实现**：src/parser 无后缀 ~ 处理）」
- `DEMO/99_spec/README.md:32-34`：三个 tilde_named_arg 文件均标 `✅`

**矛盾点**: 文档已定义该语法（说明设计定稿），DEMO 注释却称解析器未实现，README 又称已实现——**同一语法三处状态不一致**，读者无法得知 `f(b~)` 现在能否用。

---

### 4. `iterator` 关键字：文档有定义，DEMO 自述未实现 vs README 标已实现

**证据**:
- `SYNTAX/14-生成器.md`：`iterator` 关键字有完整定义（§八，含泛型/return/约束）
- `DEMO/99_spec/iterator_demo.lz:3`：「当前状态：99_spec（**编译器未实现，被 parse 测试跳过**）」
- `DEMO/99_spec/README.md:36`：`iterator_demo.lz | iterator 生成器函数 | ✅`

**矛盾点**: 同 #3——文件自述未实现、README 标已实现，且与 14-生成器.md 的完整文档定义并存。

---

### 5. `Nil` 类型：文档标「设计阶段」，DEMO 已在用

**证据**:
- `SYNTAX/00-词法基础.md:158`：`Nil` 标「（设计阶段），映射 Rust `()`」
- `SYNTAX/01-类型系统.md:419-421`：`Nil`「设计阶段，当前使用 `[]` 直接表示」
- `DEMO/01_basics/literals.lz:84`：`let empty: Nil = []` 实际使用

**矛盾点**: 文档反复标注「设计阶段」，DEMO 已作为正式字面量使用。且语义存在内在疑问：`Nil` 名为「空列表 [] 的类型别名」却映射 Rust `()`——[] 与 () 类型不匹配的映射关系文档未解释。

---

## 二、已排除的候选（文档有覆盖，防重复排查）

以下符号/构造在 DEMO 中出现但 **SYNTAX 文档均有定义**，经回源核实非缺口：

| 构造 | 文档出处 |
|------|---------|
| `.<T>` turbofish 泛型调用 | 04-表达式.md:197（§8.5） |
| `...` 抽象方法占位符 | 00-词法基础.md:38 |
| `::`（PathSep 词法层废弃） | 00-词法基础.md:169-173 |
| `/regex/` duck 正则方法 | 01b-duck关系约束.md:558,729 |
| `f"""` / `r"""` 多行字符串 | 00-词法基础.md:190-191 |
| `{}` 空 dict 消歧规则 | 00-词法基础.md:198 |
| `=:` `~:` `^:` `*:` 构建块 | 12-操作符.md:263-266 |
| `|>` `:=` `??` `?.` `..=` `**` 等 | 12-操作符.md 全表 |
| `spawn`/`go`/`async`/`await` | 10-并发与异步.md |
| `duck`/`magic`/`comptime`/`template`/`setup`/`teardown` | 01b / 06f-g / 08b / 15 |
| `===` / `+/` / `>.` / `:=:`（字符串或注释内容，非语法） | —（误报已排除） |

---

## 三、修复建议（按优先级）

1. **P0**：`__unapply__`（#1）——要么在 06d 魔法方法表中补定义并更新缺失报告状态，要么把 extractor_unapply.lz 标回「规范目标」，二选一，消除「文档说缺失、DEMO 说已实现」的对峙
2. **P0**：装饰器状态（#2）——03e 状态表按当前实现刷新（@simd/@parallel/@tail_call 已 codegen），并为已实现装饰器补语法定义
3. **P1**：`~` 命名参数糖（#3）与 `iterator`（#4）——统一 DEMO 注释、README 状态、文档定义三处的实现状态表述
4. **P2**：`Nil`（#5）——明确实现状态并解释 `[]` → `()` 的映射关系，或调整 DEMO 用例

## 四、验收标准（修正后核对）

1. `grep -rn "__unapply__" SYNTAX/` 在主文档体系（06x）有定义章节，或 DEMO 中对应文件已标注「规范目标」
2. 03e 装饰器状态表与 git HEAD / DEMO 覆盖声明一致
3. tilde_named_arg_* / iterator_demo 三处状态（文档定义 / 文件注释 / README）一致
4. Nil 的文档状态与 DEMO 使用一致，`[]`→`()` 映射有说明

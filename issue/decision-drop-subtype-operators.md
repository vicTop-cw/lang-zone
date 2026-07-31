# 设计决策：移除 `<:` / `>:` 子类型运算符

**日期**: 2026-07-31
**状态**: 已执行（语法文档 + DEMO 已改）
**提出**: 文档审计中发现 `<:` / `>:` 是死语法且造成文档混乱，建议移除；用户拍板「全面改掉」

## 决策

从 LZ 语法中**移除子类型运算符 `<:`（子类型/上界）与 `>:`（超类型/下界）**。
语法只保留两种注解运算符：

- `:` —— 约束 / bound（trait / duck 能力约束，最常用）
- `==` —— 类型等同（duck 关系中的 `A.id == B.id` 等）

型变（协变 / 逆变 / 不变）**不靠运算符表达**，由编译器按位置**自动推断**（与 Rust 一致）。

## 理由（实证）

1. **LZ 无名义继承**。`01b` 文档自身写明「LZ 暂未实现继承」「结构化匹配，无需名义继承」。`<:`（T 是 X 的子类型）在 LZ 里没有名义基础；且文档已承认 `where T: Quackable` 与 `where T <: Quackable` 在约束 trait 时语义相通——即 `<:` 对约束场景只是 `:` 的冗余同义词。
2. **型变已被自动推断覆盖**。`src/typing/variance.rs` 已实现自动方差计算（默认协变、函数参数逆变、`&mut` 不变，多位置取最严），`relate::conforms` 用它在 `F<Sub> <: F<Sup>` 赋值时强制安全。用户从不需要手写 `<:` / `>: ` 来获得类型安全的型变。
3. **两个符号当前都是死语法**（实测 lang-zone 二进制）：
   - `>:` 连词法都没实现——无 `GtColon` token，解析 `where T >: X` 直接报 `Parse error: Expected Colon, got Gt`。
   - `<:` 能解析，但约束求解器 `src/hints/constraint.rs` 仅实现 `Eq`，子类型约束留待 P1——即**能过解析、不强制**，等于无效。
4. **正是此前文档混乱的源头**。用户原话「`:` `<:` `>:` 需要将明白，不然都是乱的」——三运算符语义重叠且其中两个未实现，删掉直接消除混淆与 P1 实现负担。

## 改动范围

- `SYNTAX/01b-duck关系约束.md`：§2.0 速记、§3.2 运算符表与关键区别说明、§3.3 协变/逆变声明、§4.3 子类型关系检查→结构相容性检查、§5.4 协变/逆变容器、§6 对比表——全面移除 `<:` / `>:` 并改为「自动推断」叙述。
- `DEMO/99_spec/subtype_bounds.lz`：**删除**（整文件即围绕 `<:` / `>: `，已无意义）。
- `DEMO/99_spec/duck_demo.lz`：确认无 `<:` / `>:`（仅 `01b` 文档含此符号），无需改动。
- `SYNTAX/03c-检查站.md`、`06f-magic用法.md`、`06g-魔法综合.md`：原多处使用 `where Self <: X` / `where T <: X`（魔法 Self 约束语法），按决策统一改为 `where Self: X` / `where T: X`（与 `:` 约束一致）。
- `SYNTAX/01-类型系统.md` 等其余文档：经核查未使用 `<:` / `>: `（仅 `01b` 含「不使用 `<:` / `>: `」的元说明），无需改动。

## 保留

- `:` 约束（绿 demo `generics.lz` 已在用 `<T: Clone>` / `where T: Clone`）。
- `==` 类型等同（duck 关系：`A.id == B.id`、`A.name: B.name`）。

## 工程侧后续（非文档侧，交工程实现）

- `src/lexer/token.rs` 的 `LtColon` token（枚举 L60）及其 `<` 词法分支（L609）、`src/lexer/lexer.rs` 的 `<` 词法分支（L481）、`src/parser/parser.rs` 的 `parse_where_clause`（L721）、`src/macros/interp.rs` 的 operator 匹配臂（L527）——这 5 处已成为**死代码**，清理清单见 [`cleanup-ltcolon-deadcode.md`](cleanup-ltcolon-deadcode.md)（Open / P3）。
- `src/hints/constraint.rs` 注释「子类型约束（Subtype）留待 P1」：**取消**该 P1 计划（符号已移除，无需实现）。
- 若未来需要「有界泛型」或「显式型变覆盖」，再重新设计符号；当前保持设计开放但不发符号。

## 验证

- `cargo test --test compile_demos` 仍 1 passed（99_spec 被跳过，绿 demo 套件未受影响）。
- 全文 grep 确认：`01b` / `03c` / `06f` / `06g` 中除「不使用 `<:` / `>: `」的元说明外，无任何作为运算符的 `<:` / `>: ` 残留（`03c`/`06f`/`06g` 原魔法 Self 约束 `where Self <: X` 已统一改为 `where Self: X`）。

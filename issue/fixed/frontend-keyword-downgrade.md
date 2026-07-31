# 前端关键字降级：5 个 prelude 内建名从保留 token 改为普通标识符

> 文档类型：实现计划（doc-spec only，未改动 `src/`）
> 关联：`IR/design.md`（LZIR-H 契约）、`SYNTAX/00-词法基础.md`（关键字分类）、`SYNTAX/附录B-*`（关键字全集）
> 状态：🟡 规范拟定 · 已登记 issue · 待工程侧执行
> **决策（已定）**：遮蔽策略与 **Rust 一致**——允许 `let` 遮蔽 prelude 名，不做特殊保护。

## 一、为什么做这件事（痛点）

当前词法层把 5 个 **prelude 内建名** 保留成了专用 token：

| 名字 | 当前身份 | 真实语义 | 应归类 |
|------|----------|----------|--------|
| `panic` | `Token::Panic` | 内建**函数** | 非关键字 |
| `None` | `Token::None_` | `Option` 构造器 | 非关键字 |
| `Some` | `Token::Some_` | `Option` 构造器 | 非关键字 |
| `Ok` | `Token::Ok_` | `Result` 构造器 | 非关键字 |
| `Err` | `Token::Err_` | `Result` 构造器 | 非关键字 |

这与已确立的规范冲突（`SYNTAX/00-词法基础.md` 已写明：*关键字=参与语法结构的保留词；prelude 内建一律不算*），也和**现状不一致**——`Unit`/`Never`/`Nil`/`Number` 已经是普通标识符且能正常解析。

保留为 token 带来三个坏处：

1. **用户无法遮蔽**：`let Ok = 1` 会被词法层拦死，而 Rust 里 `let Ok = 5;` 是合法的（遮蔽 prelude 构造器）。行为既不一致、又与 Rust 习惯相悖。
2. **IR 发射不统一**：前端（`src/ir/`，LZIR-H 已在建）希望所有名称走统一的标识符解析路径，token 特例会让 `Expr::Ident` 与 `Token::Xxx` 双轨并存，增加 IR builder 的分支。
3. **文档与实现错位**：语法文档已改对，但源码仍保留 token，迟早再被人误写成"关键字"。

目标：让这 5 个名字和 `Unit`/`Never`/`Nil`/`Number` 一样，**走普通标识符 → prelude 作用域解析**的路径。

## 二、范围与影响面（已核实）

```
5 个 token 共 47 处引用，分布在 7 个文件：
  src/lexer/lexer.rs        各 1 处（token 映射）
  src/lexer/token.rs        各 1 处（Token 枚举变体）
  src/macros/interp.rs      Panic 1 / None_ 3 / Some_ 1 / Ok_ 1 / Err_ 1
  src/parser/expr.rs        Panic 1 / None_ 5 / Some_ 5 / Ok_ 5 / Err_ 5
  src/parser/helpers.rs     各 1 处
  src/parser/mod.rs         各 1 处
  src/parser/parser.rs      各 1 处

codegen/ / magic/ / semantic/ ：0 处残留  →  无后端影响
```

**结论：纯前端（词法+语法）改动，零后端/codegen 影响，风险低。**

## 三、当前机制（照抄的样板）

- 词法映射（待删）：`src/lexer/lexer.rs:344-357` 与 `src/lexer/token.rs:459-472` 把 `"panic"/"None"/"Some"/"Ok"/"Err"` 显式映射到 `Token::Panic/None_/Some_/Ok_/Err_`。
- 语法特例（待改）：`src/parser/expr.rs` 在多处对这 5 个 token 做 `match` 分支，直接构造 `Expr::NoneLit` / `Expr::Variant` / panic 调用等，绕过普通标识符解析。
- **样板**：`Unit`/`Never`/`Nil`/`Number` 已是普通 `Ident`，经前端统一的 **prelude 作用域查找** 解析回内建类型。降级即让这 5 个名字也落入同一条路。

## 四、目标状态

1. 词法层不再为这 5 个名字设专用 token——它们作为普通标识符进入解析器。
2. 解析器对普通 `Ident("Some")` 等**不再特判**，交由已有的 prelude 标识符解析（与 `Unit` 等完全一致）得到内建含义。
3. **遮蔽策略与 Rust 一致**：用户可写 `let Ok = 5;`（在局部作用域遮蔽 prelude 的 `Ok` 构造器），合法且不做特殊保护——与 Rust `let Ok = 5;` 行为对齐。不在 prelude 作用域给这些名做不可变标记。

## 五、执行步骤（原子、可逐条验证）

> 每一步后跑 `cargo build` + `cargo test --test compile_demos`，确保不扩大现有 37 个 parse 失败（见 `issue/parser-*.md`）。

- **Step 1 — 确认 prelude 解析入口**
  在 `src/typer/` 或前端初始化作用域里，确认 `Some`/`None`/`Ok`/`Err`/`panic` 已被登记进 prelude 作用域（它们当前可用即证明已登记）。记录该注册点路径，作为 Step 3 的对照。

- **Step 2 — 删除词法映射**
  删除 `src/lexer/lexer.rs` 与 `src/lexer/token.rs` 中这 5 行的专用映射（让它们落入默认的标识符分支）。

- **Step 3 — 删除 `Token` 枚举变体**
  从 `src/lexer/token.rs` 删除 `Panic`/`None_`/`Some_`/`Ok_`/`Err_` 五个枚举变体（含其 `Display`/`Debug` 实现中对应分支，见 `expr.rs:343-346, 839-842` 这类 `.to_string()` 映射）。

- **Step 4 — 改写 parser 的 5 处 match 分支**
  在 `src/parser/expr.rs`、`helpers.rs`、`mod.rs`、`parser.rs`、`macros/interp.rs` 中，将 `Token::Xxx` 匹配改为对 `Token::Ident(name)` 且 `name == "Xxx"` 的处理（或直接删除特判，交给通用标识符解析）。共 47 处，按文件批量替换。

- **Step 5 — 宏解释器对齐**
  `src/macros/interp.rs` 中 `None_` 出现 3 次（其余各 1 次），确认宏展开上下文对 `None` 的处理改为标识符路径。

- **Step 6 — 编译 + 全量测试**
  `cargo build` 通过；`cargo test --test compile_demos` 失败数**不增加**（基线 37，见 issue）。

- **Step 7 — 新增遮蔽回归测试（对齐 Rust）**
  在 `DEMO/` 加一个 `99_spec/` 用例：`let Ok = 1; print(Ok)`，断言解析通过且语义为**局部遮蔽**（与 Rust `let Ok = 5;` 行为一致，非错误）。

## 六、风险与验证

| 风险 | 等级 | 缓解 |
|------|------|------|
| prelude 解析未覆盖某名字 → 降级后变未定义 | 中 | Step 1 先确认注册点；Step 6 用 `compile_demos` 兜底 |
| 批量替换误伤其他 `Ident` 匹配 | 低 | 仅替换 `Token::Xxx` 精确模式，不动 `Token::Ident` 通用分支 |
| 与现有 37 个 parse 失败混淆 | 低 | 每步对比基线失败数，确保不增加 |
| `macros/interp.rs` 上下文语义差异 | 低 | Step 5 单独核对宏内 `None` 用法 |

**验收标准**：
- `cargo build` 0 error；
- `compile_demos` 失败数 ≤ 37（不增加）；
- `DEMO/99_spec/` 新增遮蔽用例断言 `let Ok = 1; print(Ok)` 解析通过且为局部遮蔽（与 Rust 一致）。

## 七、与 IR 迁移的关系

`src/ir/` 已在建（LZIR-H：强类型树/ANF，每 `Expr` 携带 `IrType`+`Span`）。本降级让前端产出的 AST 在**名称处理上完全统一**，IR builder（`src/ir/builder.rs`，当前标"待完成"）无需为这 5 个名字维护 token 特例，直接消费 `Expr::Ident` 即可。属于 IR 迁移的**前端清理前置项**。

## 八、决策记录

| 项 | 决定 |
|----|------|
| 遮蔽策略 | **与 Rust 一致**：允许 `let` 遮蔽 prelude 名（`Ok`/`Some`/`None`/`Err`/`panic`），不做特殊保护 |
| 执行方 | 工程侧（本文件已登记为 issue） |

---
*本文件仅定方向与步骤，不含 `src/` 修改。*

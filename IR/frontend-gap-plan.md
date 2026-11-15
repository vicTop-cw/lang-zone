# Lang-Zone 前端缺失特性 — 实现计划

> **范围**: 前端 = lexer → macro expand → parser → AST → IR builder  
> **排除**: codegen/后端/emit/rustc 相关  
> **日期**: 2026-07-30

---

## 勘误：Agent 报告中的误判

以下特性 Agent 报告为"缺失"但实际前端**已完整实现**，不列入本计划：

| 误判项 | 实际情况 |
|--------|----------|
| `def f(x: T) -> T = body` 函数定义 | ✅ parser.rs:447 已支持 `->` 返回类型，53/53 DEMO 全过 |
| `struct Name =` 语法 | ✅ parser.rs:855 已支持，所有 struct demo 均通过 |
| 管道 `a \|> f \|> g` 链式 | ✅ parser/expr.rs:155-178 while 循环正确左结合 |
| `struct Name<T> =` 泛型 struct | ✅ line 849 `parse_generic_params()` 已处理 |
| 构建块 `=:`/`^:`/`~:`/`*:` (函数内) | ✅ Token + parser 均完整 |
| 三元 `a if cond else b` | ✅ desugar 为 Expr::If |
| 列表推导 `[x for x in range]` | ✅ parser/expr.rs:525-546 |
| `as` 类型转换 | ✅ `parse_in_is()` |
| `import a.b.c` (点号路径) | ✅ |

---

## 状态总览（2026-08-15 更新）

> 本计划所列各项的当前实现状态（v157/v158 复核，均为可验证结论）：

| 项 | 特性 | 状态 |
|:--:|:-----|:----:|
| P0-1 | Dict/Set 推导式 | ✅ 已实现（与列表推导共用框架，多 for 子句 + guard 可用） |
| P0-2 | for/while 守卫 + else | ✅ 已实现（AST/IR 均有 guard + else_body 字段） |
| P0-3 | type 别名解析 | ✅ 已实现（`TypeAliasDef` + IR `Item::TypeAlias`） |
| P1-1 | comptime 块内容解析 | ✅ 已实现（`AstStmt::Comptime` → 求值内联/降级） |
| P1-2 | 顶层构建块 | ✅ 已实现（`top_level_builds` → BlockExpr） |
| P1-3 | 模块级魔法属性 | ✅ 已实现（v157：`__name__`/`__file__`/`__package__`/`__path__`/`__doc__`/`__is_macro__`） |
| P1-4 | `_ = expr` 丢弃 | ✅ 已实现 |
| P1-5 | 多行 struct 构造调用 | ✅ 已实现（parser/expr.rs:669-672 处理 LParen 后 Indent） |
| P1-6 | `#!` shebang 属性宏 | ✅ 已实现（`#!bin macro` 宏模块声明，lexer 整行 Token::Macro） |
| P1-7 | 泛型约束 → WhereBound | ✅ 已实现（where 子句 + 尖括号内联约束均可用） |
| P2-1 | 关键字降级 | ✅ 已实现（panic/Some/None/Ok/Err → prelude 解析） |
| P2-2 | `~` 后缀命名参数糖 | ✅ 已实现 |
| P2-3 | magic 声明块 | ✅ 已实现（魔法属性 + 独立 magic 块） |
| P2-4 | `__unapply__` 提取器 | ✅ 已实现（match 解构） |
| P2-5 | 顶层 macro 定义 | ✅ 已实现（main.rs token 层 MacroExpander/TemplateExpander 展开） |
| P2-6 | 非 Range 迭代器列表推导 | ✅ 已修复（`[x*x for x in xs]` 生成 flat_map/filter/map，probe p13 实证） |

**结论：原计划 16 项全部落地**，本文件保留为历史计划存档（原正文记录当时的缺失状态与实现步骤）。

---

## 真正缺失的前端特性（按优先级排序）

### P0 — 阻塞性（3 项）

#### P0-1: Dict/Set 推导式 `{k: v for ...}` `{x for ...}`

**缺失位置**: `parser/expr.rs:558` LBrace 分支  
**问题**: 只识别 `:`（字典）或 `,`/`RBrace`（集合），不检测 `For` token  
**AST 需要**: 新增 `Expr::DictComprehension` + `Expr::SetComprehension`  
**IR builder**: 降级为 `Call(comp!, ...)` 模式  
**99_spec**: `dict_comprehension.lz`, `set_comprehension.lz`  
**工作量**: M (~3h)

#### P0-2: for/while 循环守卫 `for x in xs if cond:` + else 子句

**缺失位置**: `parser/stmt.rs:73-82` (While), `83-119` (For)  
**问题**: 解析器只识别 `cond : body` / `var in iter : body`，不探测 `If` 守卫和 `Else` 子句  
**AST 需要**: `Stmt::For` 新增可选 `guard` 和 `else_body` 字段；`Stmt::While` 同理  
**IR builder**: 守卫展开为嵌套 `If`；else 保留为 `else_branch`  
**99_spec**: `guard_for_1/2/3.lz`, `while_walrus_guard_1.lz` + 6 个 combo-*  
**工作量**: M (~4h)

#### P0-3: `.lz` 中 type 别名的正确解析

**缺失位置**: `parser/parser.rs` 顶层默认分支  
**问题**: `type` token 遇到后跳过整个声明，不产出 AST 节点  
**AST 需要**: 新增 `TypeAliasDef { name, generics, ty }` + `Module.type_aliases`  
**IR builder**: `Item::Const` (type alias → const mapping)  
**工作量**: S (~2h)

---

### P1 — 重要功能（7 项）

#### P1-1: `comptime:` 块内容解析

**缺失位置**: `parser/parser.rs:178-196`  
**问题**: 当前仅跳过整个缩进块（追踪 Indent/Dedent），不解析内部内容  
**需要**: 解析块内语句并产出 `Stmt::Block`，挂上 `comptime` 标记  
**AST 需要**: 可能需要 `Stmt::Comptime(Vec<Stmt>)` 或直接在 FnDef 上标记  
**IR builder**: 生成 `ComptimeBlock` 表达式  
**工作量**: M (~3h)

#### P1-2: 顶层构建块 `x =: body`

**缺失位置**: `parser/parser.rs:254-262`  
**问题**: 显式拒绝抛出错误 "构建块只能出现在函数体内"  
**需要**: 移除限制，在模块级允许 `=:` 和 `^:` (Index) / `~:` (Call) 构建块  
**AST**: 现有 AST 已支持，仅需放宽 parser 校验  
**IR builder**: `Item::Const` 降级  
**99_spec**: `top_level_build.lz`  
**工作量**: S (~1h)

#### P1-3: 模块级魔法属性 `__name__` / `__all__` / `__bridge__` 解析

**缺失位置**: `parser/parser.rs:159-177`  
**问题**: 跳过 `MagicMethod` token，不解析属性值  
**AST 需要**: 新增 `MagicDecl` 或直接在 `Module` 上挂 `MagicAttrs`  
**IR builder**: `module.magic` 字段  
**99_spec**: 无直接 demo，但 `SYNTAX/06e-模块级魔法属性.md` 完整规范已有  
**工作量**: M (~3h)

#### P1-4: `_ = expr` 丢弃表达式

**缺失位置**: `parser/expr.rs` parse_primary  
**问题**: `Token::Underscore` 仅在 `parse_pattern` 中处理，不作为表达式  
**AST**: `Expr::Ident("_".into())` 可能已可用，需验证  
**IR builder**: 生成 `Stmt::ExprStmt`（赋值左侧为 `_` 时忽略）  
**99_spec**: `underscore_discard_3.lz`  
**工作量**: S (~1h)

#### P1-5: 多行 struct 构造器调用

**缺失位置**: `parser/expr.rs` parse_postfix `LParen` 分支  
**问题**: 构造调用 `Point(x: 1, y: 2)` 不支持参数跨多行缩进——期望同在一行  
**需要**: 识别 LParen 后 `Indent` token，支持缩进风格的参数列表  
**AST**: 现有 AST 已支持  
**IR builder**: 已实现 StructCtor  
**工作量**: M (~2h)

#### P1-6: `#!` shebang 属性宏

**缺失位置**: lexer/token.rs + parser/parser.rs  
**问题**: `#!` 开头的行被当作注释忽略，`#!export(Rust)` / `#![derive(Clone)]` 不解析  
**需要**: Lexer 识别 `#!` 为 `Token::Shebang`；Parser 解析为 `Decorator`  
**IR builder**: 生成 `Intrinsic` 节点  
**工作量**: M (~3h)

#### P1-7: 泛型约束解析为 AST WhereBound

**缺失位置**: `parser/parser.rs` parse_generic_params  
**问题**: `<T: Clone + Display>` 中只收集泛型名，约束被跳过  
**AST 需要**: `GenericParam { name, bounds, default }`  
**IR builder**: 已有 `GenericParam` 节点但 bounds 为空  
**工作量**: M (~2h)

---

### P2 — 锦上添花（6 项）

#### P2-1: 关键字降级 `panic`/`Some`/`None`/`Ok`/`Err` → prelude

**缺失位置**: lexer/lexer.rs + parser/expr.rs + macros/interp.rs  
**问题**: 在 parser 中特判 `Ident("panic")`、`Ident("None")` 等，应走 prelude 解析  
**影响**: 7 文件约 47 处  
**99_spec**: `keyword_downgrade.lz`  
**工作量**: L (~4h)

#### P2-2: `~` 后缀命名参数糖

**缺失位置**: `parser/expr.rs` parse_postfix / parse_call  
**问题**: `f(x~)` → `f(x = x)` 语法糖未实现  
**需要**: Postfix 处理 `Tilde` token 紧贴标识符，展开为命名参数  
**99_spec**: `tilde_named_arg_1/2/3.lz`  
**工作量**: M (~3h)

#### P2-3: 魔术声明块 `magic __xxx__:` 解析

**缺失位置**: `parser/parser.rs` 默认分支 + `parse_struct_like`  
**问题**: `magic` 关键字后的 `__xxx__` 块被跳过  
**AST 需要**: `MagicDecl` 节点（如有）/ 或收集到 Module/StructDef 的 magic 字段  
**工作量**: M (~3h)

#### P2-4: `__unapply__` 提取器模式

**缺失位置**: `parser/expr.rs` parse_pattern  
**问题**: `Point(x, y)` 作为匹配模式不支持——`x`/`y` 不绑定到结构体字段  
**AST 需要**: `Pattern::Struct { name, fields }` (已存在但未使用)  
**工作量**: L (~5h, 需要 codegen 配合)

#### P2-5: `.lz` 中的顶层 `macro name(...)` 定义（非 `#!bin macro` 模块）

**缺失位置**: parser/parser.rs  
**问题**: 非宏模块中的 `macro` 定义不被识别  
**99_spec**: `macro_real.lz`  
**工作量**: L (~5h)

#### P2-6: 非 Range 迭代器的列表推导

**状态**: ⚠️ 待验证  
**问题**: `[x for x in some_list]` 可能因 `parse_expr()` 优先匹配导致 `in` 被消费  
**需要**: 验证当前 `[x for x in [1,2,3]]` 是否可行；若不可行则修复  
**工作量**: S (~1h, 大概率无需修改)

---

## 实现顺序

```
阶段 A — P0 阻塞解除 (3 项, ~9h)
  1. Dict/Set 推导式          ── P0-1
  2. for/while 守卫 + else    ── P0-2
  3. type 别名解析             ── P0-3

阶段 B — P1 批量补全 (7 项, ~15h)
  4. comptime 块解析          ── P1-1
  5. 顶层构建块               ── P1-2
  6. 模块级魔法属性           ── P1-3
  7. _ = expr 丢弃            ── P1-4
  8. 多行 struct 构造调用      ── P1-5
  9. #! 属性宏                ── P1-6
  10. 泛型约束 → WhereBound   ── P1-7

阶段 C — P2 甄选 (按需, ~12h)
  11. 关键字降级              ── P2-1
  12. ~ 命名参数糖            ── P2-2
  13. magic 声明块            ── P2-3

暂缓 (需要 codegen 配合或需求不明确):
  - __unapply__ 提取器 (P2-4)
  - 顶层 macro 定义 (P2-5)
  - go / setup / teardown (P2 — 需新增 Token)
```

---

## 每个特性的实现步骤模板

以 P0-1 Dict/Set 推导式为例：

1. **AST 节点**: `ast/expr.rs` 新增 `DictComprehension { key, value, var, iter, cond }` + `SetComprehension { elem, var, iter, cond }`
2. **Parser**: `parser/expr.rs` LBrace 分支，在 `parse_expr()` 后检测 `Token::For`
3. **IR builder**: ExprKind 降级为 `Call(comp!, ...)` 或展开循环
4. **测试**: 运行 `DEMO/99_spec/dict_comprehension.lz` 验证解析通过
5. **IR 验证**: `lzc --emit=ir` 观察输出结构

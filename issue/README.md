# Issue Tracker

**2026-07-31 16:20 文档审计+闭包语法+DEMO更新**: doc-audit-round2 全部 4 项修复。附录B补 6 关键字。`||` 闭包冲突已解决（`| |` 空格）。DEMO 补 `| |` 示例。

## 设计决策 / 重大变更 (Decisions)

| 文件 | 标题 | 摘要 |
|------|------|------|
| [decision-drop-subtype-operators.md](decision-drop-subtype-operators.md) | 移除 `<:` / `>: ` 子类型运算符 ✅ | LZ 无名义继承、型变自动推断；两符号为死语法且致文档混乱，从语法文档与 DEMO 移除 |

## Open (18 IR 路线 Bug)

### 🔴 P1 — IR codegen 产物无效 Rust（11 个）

| 文件 | 标题 |
|------|------|
| [ir-builder-match-arm-first-only.md](ir-builder-match-arm-first-only.md) | match 仅取第一个臂（IR builder 已知） |
| [ir-builder-elif-reversed.md](ir-builder-elif-reversed.md) | elif 链条件逆序（IR builder 已知） |
| [ir-codegen-unop-precedence.md](ir-codegen-unop-precedence.md) | `!` 运算符优先级错误（已知） |
| [ir-codegen-i64min-double-neg.md](ir-codegen-i64min-double-neg.md) | i64::MIN 双重取反（已知） |
| [ir-codegen-match-var-scope.md](ir-codegen-match-var-scope.md) | **NEW** match 臂变量绑定作用域错误 |
| [ir-codegen-math-auto-generic.md](ir-codegen-math-auto-generic.md) | **NEW** @math 自动泛型未实现 |
| [ir-codegen-method-ret-type.md](ir-codegen-method-ret-type.md) | **NEW** 方法调用返回类型推断错误 |
| [ir-codegen-default-params.md](ir-codegen-default-params.md) | **NEW** 默认参数值丢失 |
| [ir-codegen-chained-cmp-and-generics.md](ir-codegen-chained-cmp-and-generics.md) | **NEW** 链式比较与泛型语法错误 |
| [ir-codegen-tuple-enum-ctor.md](ir-codegen-tuple-enum-ctor.md) | **NEW** 元组枚举变体构造语法错误 |
| [ir-codegen-method-def-syntax.md](ir-codegen-method-def-syntax.md) | **NEW** 方法定义语法 `fn X.method()` |

### 🟡 P2 — 次要问题（8 个，详见测试报告）

### 📋 其它 Open

| 文件 | 标题 | 摘要 |
|------|------|------|
| [findings-2026-07-31.md](findings-2026-07-31.md) | 全项目找茬报告 | P1/P2: 文档矛盾 + 死代码（11 项待处理） |
| [syntax-docs-audit-2026-07-31.md](syntax-docs-audit-2026-07-31.md) | 语法文档专项审计 | 已全部处理，归档留存 |
| [AUDIT-2026-07-29.md](AUDIT-2026-07-29.md) | 历史审计 | 07-29 历史审计记录 |

### 测试报告

| 文件 | 标题 |
|------|------|
| [test-report-2026-07-31-1545.md](test-report-2026-07-31-1545.md) | 🔴 **本次 IR 路线深度测试报告**（IR→rustc 27.6%） |
| [test-report-2026-07-31-1520.md](test-report-2026-07-31-1520.md) | 上次 IR 路线测试报告 |
| [ir-directive-plan.md](ir-directive-plan.md) | IR directive 规划 |

## Fixed (22 total)

| 文件 | 标题 | 修复摘要 |
|------|------|----------|
| [fixed/lexer-literal-silent-zero.md](fixed/lexer-literal-silent-zero.md) | 词法器：非法/溢出字面量静默变 0 ✅ | `read_number` 的 `unwrap_or(0)` 兜底 → `LexError`；根因定位，工作区已修复待提交 |
| [fixed/ir-codegen-generator-lowering.md](fixed/ir-codegen-generator-lowering.md) | IR codegen: 生成器 yield 降低 ✅ | builder AstStmt::Yield → Stmt::Yield 映射；Vec collector 模式验证通过 |
| [fixed/ir-codegen-self-type.md](fixed/ir-codegen-self-type.md) | IR codegen: self 参数类型 ✅ | self/&self/&mut self 正确发射 |
| [fixed/ir-codegen-kwargs-struct-ctor.md](fixed/ir-codegen-kwargs-struct-ctor.md) | IR codegen: struct 构造器 ✅ | _KwArg → Struct { field: value } 脱糖 |
| [fixed/ir-codegen-tail-return.md](fixed/ir-codegen-tail-return.md) | IR codegen: 尾表达式隐式返回 ✅ | 函数体尾表达式 → return 语句 |
| [fixed/ir-builder-gaps.md](fixed/ir-builder-gaps.md) | IR builder 缺口 ✅ | if/else/struct/guard/raise 等，6/6 单测 |
| [fixed/parser-new-syntax-regression.md](fixed/parser-new-syntax-regression.md) | 37 parse 失败 ✅ | Dict/Set推导、for/while守卫+else、type别名等 |
| [fixed/cleanup-ltcolon-deadcode.md](fixed/cleanup-ltcolon-deadcode.md) | 清理 LtColon 死代码 ✅ | token.rs/lexer.rs/parser.rs/interp.rs/constraint.rs 共 5 处删除 |
| [fixed/parser-func-def.md](fixed/parser-func-def.md) | def 多形态 ✅ | 5/5 |
| [fixed/parser-struct-enum-match.md](fixed/parser-struct-enum-match.md) | struct/enum/trait/match ✅ | 9/9 |
| [fixed/parser-import-path.md](fixed/parser-import-path.md) | import 路径 ✅ | 2/2 |
| [fixed/parser-comptime-const-typealias.md](fixed/parser-comptime-const-typealias.md) | comptime/const/typealias/as ✅ | 4/4 |
| [fixed/parser-operators-expr.md](fixed/parser-operators-expr.md) | 运算符/复合赋值/构造器 ✅ | 复合赋值补全7种 |
| [fixed/parser-control-flow.md](fixed/parser-control-flow.md) | ternary/guard/try-raise ✅ | parse_expr if 误判修复 |
| [fixed/parser-top-level-decls.md](fixed/parser-top-level-decls.md) | 魔法属性/宏定义 ✅ | #! shebang + magic attr |
| [fixed/parser-5-residual-failures.md](fixed/parser-5-residual-failures.md) | 5 残余失败 ✅ | 全部修复 |
| [fixed/frontend-keyword-downgrade.md](fixed/frontend-keyword-downgrade.md) | panic/Some/None/Ok/Err 降级 ✅ | P2 |
| [fixed/parser-dotdot-in-for.md](fixed/parser-dotdot-in-for.md) | `for i in 0..5:` 区间迭代器 ✅ | — |
| [fixed/build-block-colon-format.md](fixed/build-block-colon-format.md) | `^:` 构建块冒号格式 ✅ | — |
| [fixed/build-block-heading-numbering.md](fixed/build-block-heading-numbering.md) | 11-构建块.md 编号断链 ✅ | — |
| [fixed/lib-compile-breakage-shims.md](fixed/lib-compile-breakage-shims.md) | lib 编译断链 ✅ | — |

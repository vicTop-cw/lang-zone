# Issue Tracker

**2026-07-31 11:12 自动化更新**: 全部 415 项测试通过（292 lib + 114 compile_demos + 8 ir + 1 reject）。**0 open issues**。P0/P1 缺失特性已完成。P3 LtColon 死代码已清理。

## 设计决策 / 重大变更 (Decisions)

| 文件 | 标题 | 摘要 |
|------|------|------|
| [decision-drop-subtype-operators.md](decision-drop-subtype-operators.md) | 移除 `<:` / `>: ` 子类型运算符 ✅ | LZ 无名义继承、型变自动推断；两符号为死语法且致文档混乱，从语法文档与 DEMO 移除 |

## Open

_（无）_

## Fixed (21 total)

| 文件 | 标题 | 修复摘要 |
|------|------|----------|
| [fixed/ir-codegen-generator-lowering.md](fixed/ir-codegen-generator-lowering.md) | IR codegen: 生成器 yield 降低 ✅ | builder AstStmt::Yield → Stmt::Yield 映射；Vec collector 模式验证通过 |
| [fixed/ir-codegen-self-type.md](fixed/ir-codegen-self-type.md) | IR codegen: self 参数类型 ✅ | self/&self/&mut self 正确发射 |
| [fixed/ir-codegen-kwargs-struct-ctor.md](fixed/ir-codegen-kwargs-struct-ctor.md) | IR codegen: struct 构造器 ✅ | _KwArg → Struct { field: value } 脱糖 |
| [fixed/ir-codegen-tail-return.md](fixed/ir-codegen-tail-return.md) | IR codegen: 尾表达式隐式返回 ✅ | 函数体尾表达式 → return 语句 |
| [fixed/ir-builder-gaps.md](fixed/ir-builder-gaps.md) | IR builder 缺口 ✅ | if/else/struct/guard/raise 等，6/6 单测 |
| [fixed/parser-new-syntax-regression.md](fixed/parser-new-syntax-regression.md) | 37 parse 失败 ✅ | Dict/Set推导、for/while守卫+else、type别名等 |
| [cleanup-ltcolon-deadcode.md](cleanup-ltcolon-deadcode.md) | 清理 LtColon 死代码 ✅ | token.rs/lexer.rs/parser.rs/interp.rs/constraint.rs 共 5 处删除 |
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

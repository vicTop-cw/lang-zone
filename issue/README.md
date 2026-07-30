# Issue Tracker

**2026-07-30 更新**: 所有 Fixed issue 文件已移入 `fixed/` 目录。

## Open

| 文件 | 标题 | 严重度 |
|------|------|--------|
| [ir-codegen-generator-lowering.md](ir-codegen-generator-lowering.md) | IR codegen: 生成器 yield 降低（🟡 codegen 已就绪，等 parser+builder 接入 iterator/yield） | P0 |

## Fixed

| 文件 | 标题 | 修复摘要 |
|------|------|----------|
| [fixed/parser-new-syntax-regression.md](fixed/parser-new-syntax-regression.md) | 37 parse 失败 ✅ | Dict/Set推导、for/while守卫+else、type别名、comptime、顶层构建块、魔法属性、_丢弃、多行ctor、#!宏、复合赋值、dot函数名、ternary守卫 |
| [fixed/parser-func-def.md](fixed/parser-func-def.md) | def 多形态 ✅ | 5/5 |
| [fixed/parser-struct-enum-match.md](fixed/parser-struct-enum-match.md) | struct/enum/trait/match ✅ | 9/9 |
| [fixed/parser-import-path.md](fixed/parser-import-path.md) | import 路径 ✅ | 2/2 |
| [fixed/parser-comptime-const-typealias.md](fixed/parser-comptime-const-typealias.md) | comptime/const/typealias/as ✅ | 4/4 |
| [fixed/parser-operators-expr.md](fixed/parser-operators-expr.md) | 运算符/复合赋值/构造器 ✅ | 复合赋值补全7种；struct ctor LBrace postfix |
| [fixed/parser-control-flow.md](fixed/parser-control-flow.md) | ternary/guard/try-raise ✅ | parse_expr if 误判修复 |
| [fixed/parser-top-level-decls.md](fixed/parser-top-level-decls.md) | 魔法属性/宏定义 ✅ | #! shebang + magic attr |
| [fixed/parser-5-residual-failures.md](fixed/parser-5-residual-failures.md) | 5 残余失败 ✅ | 全部修复 |
| [fixed/ir-builder-gaps.md](fixed/ir-builder-gaps.md) | IR builder 缺口 ✅ | if/else/struct/guard/raise 等全部实现，6/6 单测 |
| [fixed/frontend-keyword-downgrade.md](fixed/frontend-keyword-downgrade.md) | panic/Some/None/Ok/Err 降级 ✅ | P2 |
| [fixed/parser-dotdot-in-for.md](fixed/parser-dotdot-in-for.md) | `for i in 0..5:` 区间迭代器 ✅ | — |
| [fixed/build-block-colon-format.md](fixed/build-block-colon-format.md) | `^:` 构建块冒号格式 ✅ | — |
| [fixed/build-block-heading-numbering.md](fixed/build-block-heading-numbering.md) | 11-构建块.md 编号断链 ✅ | — |
| [fixed/lib-compile-breakage-shims.md](fixed/lib-compile-breakage-shims.md) | lib 编译断链 ✅ | — |

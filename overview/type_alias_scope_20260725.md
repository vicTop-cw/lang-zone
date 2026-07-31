# type 别名作用域区分（顶层 vs 嵌套）— 完成记录

## 任务
用户要求：`type` 别名需判断是模块顶层还是函数/块内嵌套，以区分作用域。

## 改动清单

### 1. AST (`src/ast/decl.rs`, `src/ast/stmt.rs`)
- `TypeAlias` 新增字段 `scope: TypeAliasScope`（`Module` / `Local`），区分顶层与嵌套
- `stmt.rs` 新增 `Stmt::TypeAlias(TypeAlias)` 变体，承载嵌套别名

### 2. 解析器 (`src/parser/parser.rs`, `src/parser/stmt.rs`)
- 抽出 `parse_type_alias(scope)` helper（顶层/嵌套共用）
- 顶层：`Token::Type` 分支 → `Module` scope，推入 `module.type_aliases`
- 嵌套：语句级 `Token::Type` → `Local` scope，生成 `Stmt::TypeAlias`

### 3. 代码生成 (`src/codegen/decl.rs`, `src/codegen/stmt.rs`, `src/codegen/mod.rs`)
- `gen_type_alias` 生成 `type Name<T: Bound, R> = ...;`
  - **约束内联为泛型 bound**（因 Rust 禁止 type alias 后跟 `where` 子句，issue #112792）
- 顶层别名在 `gen_module` 输出；嵌套别名经 `Stmt::TypeAlias` 分支**内联**到函数/块体

### 4. 类型推断 (`src/typer/mod.rs`)
- `infer_module` 建立模块别名表
- `expand_type` / `substitute` 递归展开注解/参数/返回类型中的别名引用为底层类型
- `Stmt::TypeAlias` 注册局部别名，并在 `infer_stmt` / `zonk` 中处理
- `comptime` 求值器中 `Stmt::TypeAlias` 为 no-op
- **结果：使用别名不再报 `cannot unify` 警告**

## 验证
e2e 源（顶层 + 嵌套 + 约束 `where T <: Clone`）→ 生成 Rust → `rustc --edition 2024` 编译通过。
- 生成：`type Reduce<T: Clone, R> = fn(T, T) -> R;`、`type Point2D = (i64, i64);`
- 嵌套：`type LocalPair = (i64, i64);` 内联到函数体
- 注解展开：`let lp: (i64, i64)`、`fn(... ta: fn(i64,i64)->i64 ...)` 均展开成功
- typer 零警告

## 关键语言细节（避免再踩坑）
- 字符串类型关键字是 `str`（→ `String`），**不是** `string`
- 返回 unit 用 `()`，**不是** `void`（`void` 非关键字，会生成无效 Rust）
- 不支持 `p.0` 元组字段访问
- 函数体用 `=` + 缩进，不是 `{}` 花括号

## 注意事项
验证时 lib 因 `src/magic/engine.rs` / `src/codegen/magic.rs` 的编译错误无法跑 `cargo test`——这是用户正在并行修复的 magic 阶段遗留问题（`MagicDesc` 缺 `PartialEq`、`codegen/magic.rs:922` 的 `format!` 参数不匹配）。type-alias 相关模块编译无误，未触碰 magic.rs 以免与用户的并发编辑冲突。magic 修好后 `cargo test` 即可确认全量通过。

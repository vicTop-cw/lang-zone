# typer/ 推断管道实施报告

## 摘要

在 Parser 和 CodeGen 之间插入了完整的类型推断管道。以前 `let x = 42` 的 `x` 类型是 `None`（codegen 输出 `_` 由 Rust 推断），现在 `Typer` 会在 AST 上填充 `Some(Type::Int)`，codegen 直接输出 `i64`。

**编译流水线变更**：
```
Parser → Module → [Typer::infer_module] → [escape check] → CodeGen → .rs
                           ↑
                  填充 Param.ty / Function.return_type /
                  Stmt::Let.ty / ConstDef.ty
```

## 改动文件

| 文件 | 动作 | 内容 |
|---|---|---|
| `src/typer/mod.rs` | **新建** | ~720 行，完整类型推断器 |
| `src/lib.rs` | 修改 | 注册 `pub mod typer` |
| `src/main.rs` | 修改 | 在 escape check 前插入 infer_module 调用 |

## 推断引擎覆盖

**语句**：Let / Const / Return / While / For / Loop / Break / Continue / Defer / Raise / Guard / With / Assign / Test / Assert / Suite / Check / Yield / Comptime

**表达式**：IntLit / FloatLit / StrLit / BoolLit / NoneLit / Ident / ListLit / TupleLit / SetLit / DictLit / Binary (算术/比较/逻辑/位运算) / Unary / Call / MethodCall / FieldAccess / PathAccess / Index / If / Match / Closure / Range / Walrus / Pipe / SafeNav / Try / NullCoalesce / ListComprehension / Assign / Comptime / KwArg / TryCatch / BuildBlock

## 覆盖的 lz 类型系统

- 基本类型：`Int`, `Float`, `Str`, `Bool`, `None`, `Unit`
- 容器类型：`List<T>`, `Set<T>`, `Dict<K,V>`, `(A,B,C)`
- 函数类型：`fn(A,B) -> R`
- 复合类型：`Option<T>`, `Optional<T>`, `Ref<T>`, `MutRef<T>`, `Result<O,E>`, `Simd`
- 泛型：`Generic { base, args }`

## 验证

```
cargo test --lib  →  357 passed, 0 failed
  └─ typer::tests::test_typer_fills_let_binding  ✅  let + return 类型推断
  └─ typer::tests::test_typer_int_binary          ✅  IntLit + Mul + const

cargo build (bin) →  0 error
e2e: `def add(x: Int, y: Int) = x + y` + `let counter = 42` → 编译通过
  生成 .rs 中: `const counter: i64 = 42;` ← 类型由推断填充
```

## 已知限制

1. 无跨函数类型传播（每个函数独立 InferCtx）
2. 无 let 泛化（level 字段已预留，P1 实现）
3. 无泛型约束 `<: ` 的子类型约束
4. 函数调用不做 trait resolution

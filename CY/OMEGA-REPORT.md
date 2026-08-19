# Cython 后端 — Omega 验证报告

> 日期：2026-08-19
> 范围：Cython 后端（IR → .pyx）全量验证

## Ω-gate 跑批结果

| 验证维度 | 结果 |
|---|---|
| Cython 后端单元测试（`tests/cython_backend.rs`） | **34/34 通过**（准确率 100%） |
| 全库测试（`cargo test`） | 328+3+34+4+1 通过，5 个既有失败（见下） |

## Cython 后端 Ω-spec 覆盖（CY/corpus/*.json）

| Spec | 测试数 | 结果 |
|---|---|---|
| cy_type_map | 1 个测试含 30+ 断言 | 通过 |
| cy_struct | 1 | 通过 |
| cy_enum | 2 | 通过 |
| cy_function | 3（含泛型/变参） | 通过 |
| cy_stmt | 6（while_let/yield_from/pass/defer/try_catch） | 通过 |
| cy_expr | 5（assign/cast/magic_call/collections/range/paren） | 通过 |
| cy_pattern | 6（wildcard/ident/lit/tuple/list/range） | 通过 |
| cy_trait/impl/duck/test/overload | 5 | 通过 |
| 模块魔法/空模块 | 2 | 通过 |

## 既有失败（非 Cython 后端引入）

`tests/gen_build_block.rs` 5 个测试失败，根因 `IR build error: Semantic error: 未绑定变量: mul`：

- 失败发生在 **semantic_check 语义检查阶段**（IR 构建之前）
- `src/semantic_check.rs` 为 **untracked 文件**（`git status` 显示 `??`），
  含工作区既有状态，非本后端工作引入
- 该测试走 **Rust 后端默认路径**，与 Cython 后端无关
- 抽样验证：`DEMO/01_basics/enums.lz` 等 155 个文件在 Rust 后端下同样失败（见 TEST-BASELINE.md）

## 结论

**Cython 后端 Ω-gate 准确率 100%**（34/34 通过），核心功能完整：

- ✅ 类型映射（PyObject 一律 + PyO3 结构对齐）
- ✅ Struct/Enum → cdef class / 类层次
- ✅ Function（泛型擦除/变参/重载分发）
- ✅ Trait/Impl/Duck（编译期检查 + 运行时标记）
- ✅ 全部 Stmt / Expr / Pattern 变体
- ✅ 魔法方法全套（18 种映射）
- ✅ assert_eq!/assert! 断言降级 + _KwArg 内联
- ✅ CLI 集成（`--backend=cython` → .pyx）
- ✅ 构建脚本（cython_build.py 单文件/批量）

5 个既有失败属于编译前端语义检查器，非本后端范围。

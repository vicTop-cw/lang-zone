# IR 驱动 lzcyc 代码生成 — 实施计划

> 目标：将 lzcyc 从当前 AST→Cython 改造为 IR→Cython，复用现有 IR 基础设施。

---

## 一、架构

```
.lz 源码
  → Lexer       (词法分析)
  → Parser      (语法分析 → AST Module)
  → IR Builder  (AST → IrModule)  ← 已有，部分完成
  → IR Codegen  (IrModule → .pyx) ← 新建 ir/codegen_cython.rs
  → cythonize   (.pyx → .c)
  → MSVC/GCC    (.c → .pyd)
```

## 二、可复用模块（保留不动）

| 模块 | 文件 | 行数 | 说明 |
|:----|:----|:----:|------|
| type_mapper | `codegen_cython/type_mapper.rs` | 129 | LZ→Cython/C 类型映射，可直接调用 |
| preamble | `codegen_cython/preamble.rs` | 31 | .pyx 文件头、_Moved、_MovedCheck |
| build | `codegen_cython/build.rs` | 75 | cythonize + MSVC 编译管道 |
| magic_gen | `codegen_cython/magic_gen.rs` | 59 | 默认魔法方法实现 |
| ownership_gen | `codegen_cython/ownership_gen.rs` | 108 | 所有权跟踪器（编译期） |
| pattern_gen | `codegen_cython/pattern_gen.rs` | 157 | 模式匹配展开为 if-elif 链 |

## 三、新建模块

### `ir/codegen_cython.rs` — IR→Cython 代码生成器

结构：

```rust
pub struct CythonCodeGen {
    indent: usize,
    type_mapper: TypeMapper,
    buf: String,
    ownership: OwnershipTracker,
}

impl CythonCodeGen {
    pub fn generate(module: &IrModule) -> String;
    fn gen_item(&mut self, item: &Item);
    fn gen_function(&mut self, f: &IrFunction);
    fn gen_struct(&mut self, s: &IrStruct);
    fn gen_enum(&mut self, e: &IrEnum);
    fn gen_expr(&mut self, expr: &IrExpr) -> String;
    fn gen_stmt(&mut self, stmt: &IrStmt);
    fn gen_pattern(&mut self, pat: &IrPattern) -> String;
}
```

核心差异：
- 输入：`IrModule` 而非 AST `Module`
- 类型：`IrType` 而非 `Type`
- 表达式：`IrExpr` 已有 ANF 风格，可直接遍历
- 无需重新解析 AST — IR 已包含类型信息

### `CY/src/ir_codegen.rs` — CY 项目入口

将 `ir::codegen_cython` 包装为 lzcyc 的代码生成器：
- 调用 `ir::builder::build_ir(&module)` 从 AST 构建 IR
- 调用 `ir::codegen_cython::CythonCodeGen::generate(&ir_module)` 生成 `.pyx`

## 四、实施步骤（6 阶段）

### Phase 1：基础设施（1 天）

| # | 任务 |
|:-:|------|
| 1.1 | 创建 `src/ir/codegen_cython.rs` —— 骨架 + IrModule 遍历 |
| 1.2 | 注册 `pub mod codegen_cython` 到 `ir/mod.rs` |
| 1.3 | 实现 `gen_item` 分派（Function/Struct/Enum/Const/Import…） |
| 1.4 | 实现 `gen_function` —— 函数头 + 参数 + 返回类型 + 体 |

### Phase 2：表达式生成（1 天）

| # | 任务 |
|:-:|------|
| 2.1 | 字面量（Int/Float/Str/Bool/None/List/Dict/Set/Tuple） |
| 2.2 | Ident + Binary + Unary + 复合赋值 |
| 2.3 | Call + MethodCall + FieldAccess + Index |
| 2.4 | If 表达式 + Match 表达式 |
| 2.5 | Range + Pipe + Walrus + SafeNav + NullCoalesce |
| 2.6 | ListComprehension + DictComprehension + SetComprehension |
| 2.7 | Closure + Move + Try + Spawn + Await + Panic |

### Phase 3：语句生成（1 天）

| # | 任务 |
|:-:|------|
| 3.1 | Let/Const/Assign |
| 3.2 | If/Elif/Else + Match/Case |
| 3.3 | For (含守卫) + While (含守卫) + Loop |
| 3.4 | Break/Continue + Return + Pass |
| 3.5 | With + Defer + Guard/GuardLet |
| 3.6 | Try/Catch/Finally + Raise |

### Phase 4：声明生成（1 天）

| # | 任务 |
|:-:|------|
| 4.1 | Struct → `cdef class` |
| 4.2 | Enum → `cdef class` + 变体 |
| 4.3 | Trait → Python ABC / 虚方法 |
| 4.4 | Impl → 方法注入 |
| 4.5 | TypeAlias → `ctypedef` / 注释 |
| 4.6 | Const → Python 模块级常量 |

### Phase 5：特性补齐（1 天）

| # | 任务 |
|:-:|------|
| 5.1 | 模式匹配 → 复用 `pattern_gen.rs` |
| 5.2 | 魔法方法 → 复用 `magic_gen.rs` |
| 5.3 | 所有权追踪 → 复用 `ownership_gen.rs` |
| 5.4 | Cython 特定类型映射（`Py_ssize_t`, `bint`, `object`） |
| 5.5 | `cdef` vs `cpdef` 决策（IR 节点属性辅助） |

### Phase 6：集成测试（1 天）

| # | 任务 |
|:-:|------|
| 6.1 | 对 `DEMO/*.lz` 跑 `IR → .pyx`，快照对比 |
| 6.2 | 对 `TESTS/*.lz` 跑编译+运行，验证输出 |
| 6.3 | 性能基准测试（fib/numeric）对比旧 codegen |
| 6.4 | 删除旧 `codegen_cython/mod.rs` 中的 AST→Cython 逻辑 |

## 五、预计工时

| 阶段 | 天数 | 产出 |
|:----|:----:|------|
| Phase 1 基础设施 | 1 | 骨架 + function 生成 |
| Phase 2 表达式 | 1 | 全部 15+ 表达式类型 |
| Phase 3 语句 | 1 | 全部 12+ 语句类型 |
| Phase 4 声明 | 1 | struct/enum/trait/impl/alias |
| Phase 5 特性补齐 | 1 | 模式匹配/魔法/所有权 |
| Phase 6 集成测试 | 1 | 快照 + 运行测试 |
| **合计** | **6 天** | IR→.pyx 完整管线 |

## 六、增量迁移路径

为降低风险，可逐步迁移：
1. 新 `ir/codegen_cython.rs` 独立于旧 `codegen_cython/mod.rs`
2. `lzcyc` 添加 `--ir` 标志调用新路线
3. `lzcyc transpile --ir input.lz` = 旧 AST 路径（兜底）
4. `lzcyc transpile --ir input.lz` = 新 IR 路径（默认）
5. 新旧两路线并行运行 1 周，对比产出一致后删除旧路线

# LZCYC — LZ → Cython 编译器

## 项目定位

`lzcyc` 是 `lzc` 的子编译器，语法完全兼容 LZ，但后端输出 **Cython (.pyx)** → 编译为 **.pyd** 二进制扩展。

```
lz source.lz ──→ lzcyc transpile ──→ source.pyx
             ──→ lzcyc compile  ──→ source.pyd
             ──→ lzcyc run      ──→ (编译 + 执行)
```

---

## 一、项目结构

```
CY/
├── Cargo.toml
├── PLAN.md                      # 本文件
│
├── src/
│   ├── main.rs                  # CLI 入口
│   ├── cli.rs                   # 命令解析 (transpile/compile/run)
│   │
│   ├── lib.rs                   # 库根 — 5 层架构
│   │
│   │   # ── L1 基础层 (FROM src/ COPY) ──
│   ├── lexer/                   # COPY: src/lexer/
│   │   ├── mod.rs
│   │   ├── token.rs             # Token 类型
│   │   ├── lexer.rs             # 缩进敏感词法分析
│   │   ├── indent.rs            # 缩进栈
│   │   └── span.rs              # 源码位置
│   ├── util/                    # COPY: src/util/
│   ├── config/                  # COPY: src/config/ (简化)
│   ├── sourcemap/               # COPY: src/sourcemap/
│   │
│   │   # ── L2 语法层 (FROM src/ COPY) ──
│   ├── ast/                     # COPY: src/ast/
│   │   ├── mod.rs
│   │   ├── decl.rs              # Module, Function, StructDef, TraitDef, etc.
│   │   ├── expr.rs              # Expr 枚举
│   │   └── stmt.rs              # Stmt 枚举 + Pattern
│   ├── parser/                  # COPY: src/parser/
│   │   ├── mod.rs
│   │   ├── parser.rs            # 主解析器
│   │   ├── expr.rs              # 表达式解析
│   │   ├── stmt.rs              # 语句解析
│   │   └── helpers.rs           # 解析辅助
│   ├── macros/                  # COPY: src/macros/ (简化 — 宏在 Rust 前端展开)
│   │   ├── mod.rs
│   │   ├── expand.rs
│   │   ├── group.rs
│   │   ├── import_loader.rs
│   │   ├── interp.rs
│   │   └── pattern.rs
│   │
│   │   # ── L3 语义层 (FROM src/ COPY) ──
│   ├── types/                   # COPY: src/types/
│   │   ├── mod.rs
│   │   └── def.rs               # Type 枚举
│   ├── typer/                   # COPY: src/typer/ (类型推断)
│   ├── typing/                  # COPY: src/typing/ (类型检查)
│   │   ├── mod.rs
│   │   ├── bounds.rs
│   │   ├── errors.rs
│   │   ├── magic_bind.rs
│   │   ├── relate.rs
│   │   ├── traits.rs
│   │   └── variance.rs
│   ├── hints/                   # COPY: src/hints/ (unification)
│   ├── scope/                   # COPY: src/scope/
│   │   ├── mod.rs
│   │   └── escape.rs
│   ├── magic/                   # COPY: src/magic/
│   │   ├── mod.rs
│   │   └── engine.rs
│   ├── comptime/                # COPY: src/comptime/
│   ├── semantic.rs              # COPY: src/semantic.rs
│   ├── strict.rs                # COPY: src/strict.rs
│   └── cache.rs                 # COPY: src/cache.rs
│
│   # ── L4 Cython 代码生成层 (NEW) ──
│   ├── codegen_cython/
│   │   ├── mod.rs               # CythonCodeGen 结构体 + generate()
│   │   ├── type_mapper.rs       # Type → Cython/C 类型映射表
│   │   ├── preamble.rs          # .pyx 文件头 + import
│   │   ├── expr_gen.rs          # gen_expr(&Expr) -> String
│   │   ├── stmt_gen.rs          # gen_stmt(&Stmt) -> Vec<String>
│   │   ├── decl_gen.rs          # gen_struct/gen_enum/gen_trait/gen_impl
│   │   ├── func_gen.rs          # gen_function(&Function) -> Vec<String>
│   │   ├── pattern_gen.rs       # pattern → if-elif 树
│   │   ├── magic_gen.rs         # 魔法方法 → __xxx__
│   │   ├── concurrency_gen.rs   # async/spawn/go
│   │   └── ownership_gen.rs     # ^ 所有权模拟
│   │
│   └── build.rs                 # Cython .pyx → .pyd 编译管道
│
├── runtime/                     # Cython 运行时库 (.pxd/.pyx)
│   ├── lz_types.pxd             # List/Dict/Set 类型声明
│   ├── lz_types.pyx
│   ├── lz_option.pxd            # Option/Result
│   ├── lz_option.pyx
│   ├── lz_pointers.pxd          # Box/Rc/Arc
│   ├── lz_pointers.pyx
│   ├── lz_concurrency.pxd       # Future/spawn/go
│   ├── lz_concurrency.pyx
│   ├── lz_iter.pxd              # Range/迭代器/生成器
│   ├── lz_iter.pyx
│   ├── lz_exceptions.pxd        # 异常类型
│   └── lz_test.pxd              # 测试框架
│
├── DEMO/                        # (symlink or copy) 用于集成测试
```

---

## 二、复用的代码（从 `lang-zone/src/` COPY）

### 2.1 完全 COPY，**不改一行**

| 源路径 | 目标路径 | 文件数 | 说明 |
|--------|---------|:-----:|------|
| `src/ast/` | `src/ast/` | 4 | Module, Expr, Stmt, Function, StructDef 等 |
| `src/lexer/` | `src/lexer/` | 5 | Token, Lexer, Indent 栈 |
| `src/parser/` | `src/parser/` | 5 | 递归下降解析器 |
| `src/types/` | `src/types/` | 2 | Type 枚举 + 结构化类型 |
| `src/typer/` | `src/typer/` | 1 | 类型推断 |
| `src/typing/` | `src/typing/` | 8 | 类型检查 + trait 解析 |
| `src/hints/` | `src/hints/` | 8 | Hindley-Milner 合一 |
| `src/scope/` | `src/scope/` | 2 | 作用域分析 |
| `src/magic/` | `src/magic/` | 2 | 魔法方法引擎 |
| `src/comptime/` | `src/comptime/` | 1 | 编译期求值 |
| `src/util/` | `src/util/` | 11 | 工具函数 |
| `src/config/` | `src/config/` | 2 | 配置/路径 |
| `src/sourcemap/` | `src/sourcemap/` | 1 | 源码映射 |
| `src/semantic.rs` | `src/semantic.rs` | 1 | 语义校验 |
| `src/strict.rs` | `src/strict.rs` | 1 | 严格模式 |
| `src/cache.rs` | `src/cache.rs` | 1 | 增量编译缓存 |
| `src/lib.rs` | `src/lib.rs` | 1 | 库入口（调整，去掉不 copy 的模块） |

**共 ~56 个文件**，COPY 后不改任何逻辑，只调整 `lib.rs` 中的 `mod` 声明（去掉 `codegen/` `bridge/` `export/` `simd/`）。

### 2.2 不 COPY

| 模块 | 原因 |
|------|------|
| `src/codegen/` | 替换为 `codegen_cython/` |
| `src/bridge/` | Cython 后端用 Python `import`，不需要 Rust FFI 桥接 |
| `src/export/` | Rust DLL/SO 导出，不需要 |
| `src/simd/` | SIMD 向量，第一阶段暂不支持 |
| `src/runtime/` | Rust 运行时 shim，替换为 Cython `.pxd` |
| `src/main.rs` | 替换为 `lzcyc` CLI |

---

## 三、新写的代码

### 3.1 `codegen_cython/` — Cython 代码生成器（~3000 行）

核心思路：**Visitor 模式**，与现有 `codegen/` 相同的 trait 设计，但输出 `.pyx` 文本。

```rust
// mod.rs — 核心结构体
pub struct CythonCodeGen {
    // 状态
    indent: usize,
    output: Vec<String>,
    type_mapper: TypeMapper,
    struct_defs: HashMap<String, StructDef>,
    enum_defs: HashMap<String, EnumDef>,
    local_vars: HashMap<String, String>,   // 变量名 → Cython 类型
    module: Module,
    // 运行时依赖跟踪
    needs_threading: bool,
    needs_asyncio: bool,
    needs_weakref: bool,
    needs_lz_runtime: bool,
}
```

**类型映射表** (TypeMapper)：

```
LZ Type  →  Cython (cpdef)    C (cdef)
─────────────────────────────────────────
Int       →  int (Python)     Py_ssize_t / int
F64       →  float            double
Str       →  str              str / object
Bool      →  bool             bint
Unit      →  None             void
Never     →  NoReturn         void
List<T>   →  list / object    object
Dict<K,V> →  dict             object
Set<T>    →  set              object
Option<T> →  object           object (None 哨兵)
Result<T,E> → object          object (Ok/Err 哨兵)
Box<T>    →  object           void*
```

### 3.2 CLI (`cli.rs` + `main.rs`) — ~400 行

参考 cypyc 的命令设计：

```
lzcyc transpile input.lz          # 生成 .pyx
lzcyc compile  input.lz           # 生成 .pyd（调用 cythonize + C 编译器）
lzcyc run     input.lz [func]     # 编译 + 运行
lzcyc watch   ./src               # 热重载开发（后续）
```

### 3.3 运行时库 (`runtime/*.pxd/.pyx`) — ~1000 行

在 Cython 层面实现 LZ 标准类型：

| 文件 | 内容 |
|------|------|
| `lz_types.pxd/pyx` | `L2List`(Python list + 类型守卫)、`L2Dict`、`L2Set` |
| `lz_option.pxd/pyx` | `L2Option` (Some/None 双态)、`L2Result` (Ok/Err) |
| `lz_pointers.pxd/pyx` | `L2Box` (独占)、`L2Rc` (引用计数)、`L2Arc` (原子 RC) |
| `lz_concurrency.pxd/pyx` | `L2Future`、`spawn`(threading)、`go`(asyncio) |
| `lz_iter.pxd/pyx` | `L2Range`、生成器包装 |
| `lz_exceptions.pxd/pyx` | `L2Exception` 层次、`panic()` 实现 |

### 3.4 构建脚本 (`build.rs`) — ~200 行

调用 Cython 的工具链完成编译：

```rust
// 调用 cythonize 将 .pyx → .c
// 调用 GCC/MSVC 将 .c → .pyd
// 支持 --debug 和 --release
```

---

## 四、阶段实施计划

### Phase 0：项目骨架 + COPY 代码（1 天）

| # | 任务 |
|:-:|------|
| 0.1 | 初始化 `CY/Cargo.toml`（依赖: 同 `lang-zone/Cargo.toml` — 零外部依赖） |
| 0.2 | COPY `ast/` `lexer/` `parser/` `types/` `typer/` `typing/` `hints/` `scope/` `magic/` `comptime/` `util/` `config/` `sourcemap/` 到 `CY/src/` |
| 0.3 | COPY `semantic.rs` `strict.rs` `cache.rs` |
| 0.4 | 编写 `lib.rs`（声明所有 COPY 模块，去掉 `codegen` `bridge` `export` `simd` `runtime`） |
| 0.5 | `cargo check` 通过 — 验证 COPY 代码编译无误 |

### Phase 1：CLI + 代码生成器骨架（1 天）

| # | 任务 |
|:-:|------|
| 1.1 | `cli.rs` — `lzcyc {transpile,compile,run}` 参数解析 |
| 1.2 | `main.rs` — 入口 + 子命令路由 |
| 1.3 | `codegen_cython/mod.rs` — `CythonCodeGen` 结构体 + `generate(&Module) -> String` |
| 1.4 | `codegen_cython/type_mapper.rs` — 类型映射表 |
| 1.5 | `codegen_cython/preamble.rs` — `.pyx` 头 (cython directives + imports) |

**验证**：`lzcyc transpile hello.lz` 生成合法的 `.pyx` 文件头

### Phase 2：表达式生成（2 天）

| # | 任务 |
|:-:|------|
| 2.1 | `expr_gen.rs` — 字面量 (int/float/str/bool/None/List/Dict/Set/Tuple) |
| 2.2 | `expr_gen.rs` — Ident + 二元运算 + 一元运算 + 复合赋值 |
| 2.3 | `expr_gen.rs` — 函数调用 + 方法调用 + 字段访问 + 索引 |
| 2.4 | `expr_gen.rs` — 管道 `|>` + 海象 `:=` + 安全导航 `?.` + 空值合并 `??` + 错误传播 `?` |
| 2.5 | `expr_gen.rs` — f-string + 三元 `a if b else c` |
| 2.6 | `expr_gen.rs` — 列表/字典/集合推导式 |
| 2.7 | `expr_gen.rs` — 闭包 + `if`/`match` 表达式 |

**验证**：`05_expressions/operators.lz` → 正确 `.pyx`

### Phase 3：语句生成（2 天）

| # | 任务 |
|:-:|------|
| 3.1 | `stmt_gen.rs` — let/var 绑定 (const/let/默认可变) |
| 3.2 | `stmt_gen.rs` — if/elif/else + for + while + loop |
| 3.3 | `stmt_gen.rs` — break/continue (含带值) + return + pass |
| 3.4 | `stmt_gen.rs` — with + defer |
| 3.5 | `stmt_gen.rs` — guard + guard let |
| 3.6 | `stmt_gen.rs` — try/catch/finally + raise + `?` 展开 |

**验证**：`06_control_flow/*.lz` + `10_error_handling/*.lz` → 正确 `.pyx`

### Phase 4：声明生成（2 天）

| # | 任务 |
|:-:|------|
| 4.1 | `decl_gen.rs` — struct → `cdef class` 含字段 + `__init__` 关键字构造 |
| 4.2 | `decl_gen.rs` — struct 方法 (self/mut self) |
| 4.3 | `decl_gen.rs` — enum 无字段变体 → `cdef class` + int 常量 |
| 4.4 | `decl_gen.rs` — enum 带字段变体 → `cdef class` + 联合 |
| 4.5 | `decl_gen.rs` — trait → Python ABC / `cdef class` 虚方法 |
| 4.6 | `decl_gen.rs` — impl → 目标类型上生成方法 |
| 4.7 | `decl_gen.rs` — type alias → `ctypedef` / 注释 |
| 4.8 | `func_gen.rs` — 函数定义 (cpdef/def) + 泛型单态化 + 默认参数 + ref 参数 |

**验证**：`07_data_structures/*.lz` + `04_functions/*.lz`

### Phase 5：模式匹配 + 魔法方法（2 天）

| # | 任务 |
|:-:|------|
| 5.1 | `pattern_gen.rs` — match/case 展开为 if-elif 树 |
| 5.2 | `pattern_gen.rs` — 字面量/变量/变体/元组/通配符模式 |
| 5.3 | `pattern_gen.rs` — 嵌套模式 + OR 模式 |
| 5.4 | `magic_gen.rs` — 算术魔法方法 `__add__`~`__pow__` |
| 5.5 | `magic_gen.rs` — 比较 + 哈希 + 字符串 + 迭代器魔法 |
| 5.6 | `magic_gen.rs` — 上下文管理器 `__enter__`/`__exit__` |
| 5.7 | `magic_gen.rs` — 模块级 magic 块 |

**验证**：match demo + magic_methods.lz

### Phase 6：所有权模拟 + 并发（2 天）

| # | 任务 |
|:-:|------|
| 6.1 | `ownership_gen.rs` — `^` 编译期标注 + 运行时 sentinel 检测 |
| 6.2 | `ownership_gen.rs` — `owned` 参数修饰符 → 传值 + 标记 |
| 6.3 | `concurrency_gen.rs` — `async def` → `async def` |
| 6.4 | `concurrency_gen.rs` — `await` → `await` |
| 6.5 | `concurrency_gen.rs` — `spawn` → `threading.Thread` |
| 6.6 | `concurrency_gen.rs` — `go` → `asyncio.create_task` |

**验证**：`03_variables/ownership.lz` + `11_concurrency/async_spawn.lz`

### Phase 7：运行时库（2 天）

| # | 任务 |
|:-:|------|
| 7.1 | `runtime/lz_types.pxd/pyx` — L2List/L2Dict/L2Set |
| 7.2 | `runtime/lz_option.pxd/pyx` — L2Option/L2Result |
| 7.3 | `runtime/lz_pointers.pxd/pyx` — L2Box/L2Rc/L2Arc |
| 7.4 | `runtime/lz_concurrency.pxd/pyx` — L2Future/spawn/go |
| 7.5 | `runtime/lz_exceptions.pxd/pyx` — Exception 层次 + panic |
| 7.6 | build.rs — .pyx → .c → .pyd 编译管道 |

**验证**：`lzcyc compile xxx.lz` → import xxx 可用

### Phase 8：集成测试（2 天）

| # | 任务 |
|:-:|------|
| 8.1 | 对每份 `DEMO/*.lz` 运行 `lzcyc transpile`，比对 .pyx 快照 |
| 8.2 | 对可运行 DEMO 运行 `lzcyc run`，验证输出 |
| 8.3 | 错误测试：预期编译错误的 .lz 验证错误消息 |
| 8.4 | 所有权运行时测试：`^` 场景行为验证 |

---

## 五、关键设计决策

| 决策 | 方案 |
|:----:|------|
| **所有权** | 编译期数据流分析 + 运行时 `_MovedSentinel` 哨兵。`^` 后变量标记为移动，访问时报错 |
| **泛型** | 单态化 (monomorphization)，在 Rust 前端展开为具体类型再生成 Cython |
| **match** | 展开为 `isinstance()` / 值比较的 if-elif 链 |
| **trait** | 简化为 Python ABC（`abc.ABC` + `@abstractmethod`），不做完整虚表 |
| **import** | 直接映射为 Python `import`，运行时路径依托 Python 模块系统 |
| **`.pyx` vs `.py`** | 始终输出 `.pyx`，利用 Cython 静态类型优化；纯 Python 回退暂不支持 |
| **构建** | `build.rs` 中调用 `cythonize` CLI 完成 `.pyx→.c→.pyd` |

---

## 六、与 `cypyc` 架构对照

```
cypyc (Python)            lzcyc (Rust)
─────────────────         ─────────────────
cypyc/cli.py              src/cli.rs + src/main.rs
cypyc/parser/lexer.py     src/lexer/    (COPY)
cypyc/parser/parser.py    src/parser/  (COPY)
cypyc/analyzer/*.py       src/typer/ + src/typing/ + src/scope/ (COPY)
cypyc/codegen/            src/codegen_cython/  (NEW)
  cython_generator.py       expr_gen.rs + stmt_gen.rs + ...
  type_mapper.py            type_mapper.rs
  bridge_generator.py       (跳过 — 不需要桥接)
  setup_generator.py        build.rs (替代)
runtime/*.pyx              runtime/*.pxd/.pyx (NEW)
```

**关键差异**：
- `lzcyc` 用 Rust 做前端（复用现有 LZ 编译器），`cypyc` 用 Python 做前端
- `lzcyc` 语法 = LZ 语法（缩进敏感 + 类型后置），`cypyc` 语法 = Python 子集
- `lzcyc` 所有权模拟是独有特性（cypyc 没有 `^` / `owned`）

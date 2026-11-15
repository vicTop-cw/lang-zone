# LZ Cython 后端完整实现计划

> 目标：基于 LZIR 实现一个完整的 Cython（.pyx）后端，将 `.lz` 源码通过 IR 翻译为合法 Cython 代码，
> 对象模型一律为 PyObject，并与 PyO3 桥接层设计保持结构一致。

---

## 〇. 现状分析

### 0.1 已有基础

| 文件 | 状态 | 作用 |
|---|---|---|
| `src/ir/codegen_cython.rs` | 骨架（468 行） | 已实现：runtime 哨兵 `_Moved`、prelude、`gen_struct`/`gen_method`/`gen_function` 骨架、部分 Stmt/Expr 映射；**大量 WIP**（enum/const/import/trait/impl/test 全是 `# xxx (WIP)`） |
| `CY/TESTS/*.pyx` | 参考输出样例 | 50+ 份目标风格：`cdef class`/`cpdef`/`cdef public double x` 等 |
| `src/ir/node.rs` | 完整 IR 节点（947 行） | 后端无关的 Item/Stmt/Expr/Pattern 定义 |
| `src/ir/types.rs` | 完整 IR 类型（284 行） | `Int/F64/Str/Bool/Unit/Any/Named/Option/Result/Tuple/Fn/Ref/MutRef/Duck/Generic` |
| `src/ir/codegen/mod.rs` | Rust 后端（8818 行） | **模式参考**：类型映射、所有权、循环 else、checker、变参注入、duck auto-impl 等全部已解决 |
| `src/bridge/python.rs` | PyO3 桥接（428 行） | **结构参照**：`PyO3Bridge`/`PyExport`/`PyTypeExport`/`PyModuleConfig`、`generate_module`/`generate_pyclass`/`generate_pyfunction`/`generate_pymodule` |

### 0.2 核心差距

1. **类型系统**：当前 `map_type` 直接映射 C 类型（`int`/`double`/`str`/`bint`），**未实现"一律 PyObject" + PyO3 一致结构**
2. **对象模型缺失**：没有 PyO3 式的 `PyObject` 包装器、引用计数、`PyResult`、`IntoPy`/`FromPyObject` 对应
3. **Stmt/Expr 覆盖不全**：`ExprStmt`/`Defer`/`ImplicitConvert`/`MagicCall`/`Cast`/`Pipe`/`Lambda`/`Tuple`/`List`/`Dict`/`Range`/`GenExpr`/`GenBuild`/`AssignExpr`/`BlockExpr`/`YieldFrom`/`BreakLabel`/`Continue`/`BlockLabel`/`CheckerBlock`/`Pass`/`TypeAlias` 全部未生成
4. **Item 覆盖不全**：enum/const/trait/impl/test 全是 WIP
5. **无构建/验证管线**：CLI 和 `cmd_build` 只走 Rust，无 `lzcyc` 工具、无 `cythonize` 调用
6. **duck/trait 运行时缺失**：Rust 端靠生成 trait+auto-impl；Cython 端需要等价机制

---

## 一、设计原则：PyObject 一律 + PyO3 结构一致

### 1.1 对象模型（对齐 PyO3）

```
PyO3 (Rust)                          Cython 后端
─────────────                        ───────────
#[pyclass] struct T                 cdef class T:
    #[pyo3(get, set)] x: f64          cdef public object x
#[pymethods] impl T                 # 方法直接写在 cdef class 内
#[pyfunction] fn f(...) -> R        cpdef object f(...):
#[pymodule] fn mod_py(...)          # module 级 __all__ + 顶层函数
wrap_pyfunction!(f, m)?             # 无需包装，Cython 原生导出
m.add_class::<T>()?                 # cdef class 自动注册
PyResult<T>                         → 用 try/except 或返回 None 表错误
PyObject                            object
```

### 1.2 类型映射表（核心决策）

| IrType | Cython 类型 | 说明 |
|---|---|---|
| `Int` | `Py_ssize_t` / `object` | 标量位置用 `Py_ssize_t`（如函数参），泛型/容器内用 `object` |
| `F64` | `double` / `object` | 同上 |
| `Str` | `str` | Cython `str` = Python `str` |
| `Bool` | `bint` | Cython bool |
| `Unit` | `void` / 不写 | 函数返回 |
| `Never` | `void` | 永不返回 |
| `Any` | `object` | 未确定类型 |
| `Self_` | `<当前 cdef class 名>` | 方法返回 self 类型 |
| `Ext` | `object` | 外部句柄 → 不透明 PyObject |
| `Named("List", [T])` | `list` | Python list（元素 object） |
| `Named("Dict", [K,V])` | `dict` | Python dict |
| `Named("Set", [T])` | `set` | Python set |
| `Named("Option", [T])` | `object` | `None` = Some/None 统一用 object+None |
| `Named("Result", [O,E])` | `object` | 用异常传播或 None 表错 |
| `Named("Tuple", [...])` | `tuple` | Python tuple |
| `Named("Box"/"Rc"/"Arc", [T])` | `object` | 智能指针降级为 PyObject |
| `Named(自定义 struct, [])` | `cdef class 名` | 引用同名 cdef class |
| `Named(自定义 enum, [])` | `object` / `int` | 见 §1.4 |
| `Named("Future", [T])` | `object` | 异步句柄 |
| `Fn { params, ret }` | `object` | 闭包/函数对象 → Python callable |
| `Ref(T)` / `MutRef(T)` | `object` | Cython 无原生引用 → PyObject |
| `Duck { fields }` | `object` | duck 约束是编译期检查，运行时为 PyObject |
| `Generic(name)` | `object` | 泛型参数在运行时擦除 |

**关键规则**：
- **函数签名**（参数/返回值）优先使用 C 类型标注（`Py_ssize_t`/`double`/`str`/`bint`/`void`）以获得性能，但内部一律可按 `object` 处理
- **容器/泛型位置**一律用 `object`（PyObject）
- **所有 cdef class 的字段**：`cdef public object xxx`（PyObject 句柄），若字段是标量且确定类型可用 `cdef public double xxx`
- **跨 cdef class 引用**：直接持有 `cdef class` 名（Cython 自动管理 PyObject 引用）

### 1.3 Struct → cdef class（对齐 PyO3 `#[pyclass]` + `#[pymethods]`）

```cython
# LZ:
# struct Point:
#     x: f64
#     y: f64
#     def area(self) -> f64 = self.x * self.y

# 生成:
cdef class Point:
    cdef public double x
    cdef public double y
    def __init__(self, double x, double y):
        self.x = x
        self.y = y
    cpdef double area(self):
        return self.x * self.y
```

- `cpdef`：既可从 Python 调用也可从 Cython 调用（对齐 PyO3 双重访问）
- `cdef public`：字段 Python 可读可写（对齐 `#[pyo3(get, set)]`）
- `__init__` 对齐 PyO3 `#[new]`
- 方法对齐 PyO3 `#[pymethods]`

### 1.4 Enum → 类层次（对齐 PyO3 `#[pyclass]` enum 或 `IntEnum`）

**决策**：用 `cdef class` 层次 + 类属性，**不用** Cython `cdef enum`（后者不可扩展方法）：

```cython
# LZ:
# enum Shape:
#     Circle(r: f64)
#     Rect(w: f64, h: f64)

# 生成:
class Shape:
    pass
class Circle(Shape):
    cdef public double r
    def __init__(self, double r): self.r = r
class Rect(Shape):
    cdef public double w
    cdef public double h
    def __init__(self, double w, double h): self.w = w; self.h = h
```

若 enum 是 C-style 无数据，可退化为 `int` 常量。

### 1.5 Trait/Impl → 抽象基类 + 鸭子方法

Cython 无 trait，**编译期 duck 检查已在 `duck_check.rs` 完成**，运行时：
- `TraitDef` → 不生成独立运行时结构（或生成空 `class` 作标记）
- `Impl` → 方法注入到目标 `cdef class` 的 `#[pymethods]` 风格方法列表
- **duck auto-impl**（Rust 端生成 trait 委托）→ Cython 端：在 `cdef class` 内生成同名方法委托到具体类型的方法

### 1.6 错误处理（对齐 PyO3 `PyResult`）

| LZ 错误语义 | Cython 实现 |
|---|---|
| `raise E` | `raise E`（原生） |
| `Result[T,E]` 返回 | 成功返回值，失败 `raise` 对应异常 |
| `Option[T]` | `None` = 无值，非 `None` = 有值 |
| `try/catch` | Cython `try/except/else/finally` |

---

## 二、架构设计

### 2.1 文件结构

```
src/ir/codegen_cython.rs          ← 主生成器（重构扩展）
src/bridge/python.rs              ← 已存在，Cython 后端复取其 PyExport/PyTypeExport 结构
src/cli.rs                        ← 新增 --backend=cython 路由
src/ir/codegen_cython/
    mod.rs                        ← CythonCodeGen 主结构
    type_map.rs                   ← IrType → Cython 类型映射
    runtime.rs                    ← 运行时 prelude 生成（_Moved 哨兵等）
    struct_gen.rs                 ← StructDef → cdef class
    enum_gen.rs                   ← EnumDef → 类层次
    func_gen.rs                   ← FnDef → cpdef/cdef
    trait_impl_gen.rs             ← Trait/Impl/duck auto-impl
    stmt_gen.rs                   ← Stmt 生成
    expr_gen.rs                   ← Expr 生成
    pattern_gen.rs                ← Pattern 生成
CY/scripts/cython_build.py        ← 已有，.pyx → .c → .pyd
CY/scripts/lzcyc                  ← 新增：LZ→Cython 命令行工具（或集成到 lang-zone.exe --backend=cython）
tests/cython_tests/               ← 新增：Cython 后端测试套件
```

### 2.2 主生成器 `CythonCodeGen` 状态（对齐 Rust `CodeGen` 的精简子集）

```rust
pub struct CythonCodeGen {
    indent: usize,
    buf: String,
    // 上下文
    current_fn_ret_ty: Option<IrType>,
    in_cdef_class: bool,           // 是否在 cdef class 体内
    current_class_name: Option<String>,
    known_types: HashSet<String>,  // 所有用户自定义类型
    cdef_classes: HashSet<String>, // cdef class 名集合
    // 所有权（沿用 Rust 端分析）
    moved_values: HashSet<String>,
    // duck 自动 impl 缓冲
    duck_auto_impls: Vec<String>,
}
```

### 2.3 管线流程

```
.lz 源码
  │
  ▼
Lexer → Parser → AST（不变）
  │
  ▼
build_ir(ast_module) → IrModule（不变）
  │
  ├─ check_duck_satisfaction(...)  ← 编译期结构匹配检查（不变）
  │
  ▼
CythonCodeGen::generate(module) → String（.pyx 源码）
  │
  ▼
写盘 file.pyx
  │
  ▼
cythonize(.pyx) → .c → 编译 → .pyd/.so（CY/scripts/cython_build.py 扩展）
```

---

## 三、实施阶段（子阶段划分）

### 阶段 A：类型映射与运行时基础（~8 小阶段）

**A1** — 重写 `map_type`：实现 §1.2 完整映射表，处理 `Named`/`Option`/`Result`/`Tuple`/`Fn`/`Ref`/`MutRef`/`Generic`/`Duck`/`Ext`/`Future`

**A2** — 实现 `map_type_with_context`：区分"函数签名位置"（用 C 类型）vs "容器/泛型位置"（用 object）

**A3** — 运行时 prelude 扩展：完善 `_Moved`/`_MovedCheck`，增加 `import cython`、`from cpython.ref cimport PyObject`（可选）、标准容器导入

**A4** — 实现 `__name__`/`__file__`/`__all__` 模块魔法（骨架已有，完善）

**A5** — 实现常量生成 `gen_const`：`cdef object NAME = value` 或 `ctypedef` 风格

**A6** — 实现类型别名 `gen_type_alias`：`ctypedef object MyAlias` 或注释

**A7** — 实现 import 生成 `gen_import`：`import xxx` / `from xxx import yyy`

**A8** — 测试：primitives/const/type_alias/import 四类单文件

### 阶段 B：结构体与枚举（~10 小阶段）

**B1** — 重写 `gen_struct`：`cdef class` + `cdef public` 字段 + `__init__` + `cpdef`/`cdef` 方法

**B2** — 实现 `gen_struct` 的 `has_new`/`__new__` 魔术构造（对齐 Rust 端 `__lz_new`）

**B3** — 实现 struct `implicit_froms` 隐式转换 → `__init__` 多态或转换方法

**B4** — 实现 struct 泛型：`cdef class Box2[T]` → 退化为 `cdef class Box2` + 字段 `object`（运行时擦除）

**B5** — 实现 `gen_enum`：类层次方案（§1.4），含字段变体

**B6** — 实现 enum 方法：注入到基类或每个变体类

**B7** — 实现 enum 泛型（运行时擦除）

**B8** — struct/enum 交叉引用处理（`known_types` 预收集）

**B9** — 测试：struct/struct_methods/generic_struct/enum/enum_data

**B10** — 测试：enum_multi/impl_demo/mut_self

### 阶段 C：函数与 Item 完整覆盖（~12 小阶段）

**C1** — 重写 `gen_function`：正确处理 `cpdef`/`cdef`/`def` 选择策略、返回类型、参数类型映射

**C2** — 实现函数泛型（运行时擦除，泛型参数 → object）

**C3** — 实现函数 where 约束（编译期检查，运行时忽略，生成注释）

**C4** — 实现变参函数：`..: Tuple<T>` → `*args`，`..: Dict<K,V>` → `**kwargs`（对齐 `03d-可变参数.md`）

**C5** — 实现变参调用点自动打包

**C6** — 实现 `gen_trait`：生成空 `class TraitName: pass` 作标记 + 方法签名注释

**C7** — 实现 `gen_impl`：方法注入到目标 cdef class

**C8** — 实现 duck auto-impl：在 cdef class 内生成委托方法

**C9** — 实现 checker 块：`def NAME(ps): ...`（对齐 Rust 端 `__Params`）

**C10** — 实现 `gen_test`：`def test_NAME(): ...` + pytest 风格

**C11** — 处理函数重载（mangling）：Cython 不支持重载 → 生成 `_name__0`/`_name__1` + 分发器

**C12** — 测试：functions/basic/generics/overload/variadic/checker

### 阶段 D：语句完整覆盖（~12 小阶段）

**D1** — 补全 `Stmt::Let`：正确处理 `is_mut`/`is_ref`/类型标注、`# type:` 注释

**D2** — 补全 `Stmt::Assign`/`Stmt::Return`/`Stmt::ExprStmt`

**D3** — 补全 `Stmt::If`/`Stmt::For`/`Stmt::While`（含 `guard`、`else_body`）

**D4** — 实现 `Stmt::WhileLet`

**D5** — 实现 `Stmt::Match`：`if/elif` 链 + 模式解构（`Pattern` → 赋值+条件）

**D6** — 实现 `Stmt::Raise`/`Stmt::Assert`/`Stmt::Yield`/`Stmt::YieldFrom`

**D7** — 实现 `Stmt::Break`/`Stmt::Continue`/`Stmt::BreakLabel`/`Stmt::BlockLabel`

**D8** — 实现 `Stmt::Defer`：`try/finally` 包装

**D9** — 实现 `Stmt::TryCatch`：`try/except/else/finally` + 模式匹配 except

**D10** — 实现 `Stmt::Block`（裸块）/`Stmt::Pass`/`Stmt::TypeAlias`

**D11** — 实现 `Stmt::CheckerBlock`：生成 checker 函数

**D12** — 测试：控制流全套（if/for/while/match/break/continue/try_catch/defer）

### 阶段 E：表达式完整覆盖（~14 小阶段）

**E1** — 补全 `ExprKind::Lit`：全部 LitKind（Int/F64/Str/FStr/Bool/Unit/None_）

**E2** — 补全 `ExprKind::Var`/`ExprKind::Call`/`ExprKind::MethodCall`

**E3** — 补全 `ExprKind::FieldAccess`/`ExprKind::IndexGet`/`ExprKind::IndexSet`

**E4** — 补全 `ExprKind::BinOp`/`ExprKind::UnOp`（含比较、位运算、逻辑）

**E5** — 实现 `ExprKind::IfExpr`（三元）

**E6** — 实现 `ExprKind::Lambda`：`lambda args: body`

**E7** — 实现 `ExprKind::StructCtor`：`Name(k=v, ...)` 关键字构造

**E8** — 实现 `ExprKind::EnumCtor`：`Enum.Variant(args)`

**E9** — 实现 `ExprKind::Cast`：`target(expr)` 显式转换

**E10** — 实现 `ExprKind::MagicCall`：`__getitem__`→`[ ]`、`__str__`→`str()`、`__len__`→`len()`、`__iter__`/`__next__` 等

**E11** — 实现 `ExprKind::BlockExpr`：`(...)` 内联或立即执行

**E12** — 实现 `ExprKind::TupleLit`/`ListLit`/`Dict`/`Range`

**E13** — 实现 `ExprKind::Pipe`：`x |> f(args)` → `f(args, x)` 或 `x.f(args)`

**E14** — 实现 `ExprKind::ImplicitConvert`/`ExprKind::AssignExpr`/`ExprKind::GenExpr`/`ExprKind::GenBuild`/`ExprKind::Paren`

### 阶段 F：Pattern 匹配（~6 小阶段）

**F1** — 实现 `Pattern::Wildcard`/`Pattern::Binding`/`Pattern::Literal`

**F2** — 实现 `Pattern::Tuple`/`Pattern::Struct` 解构

**F3** — 实现 `Pattern::Enum` 解构

**F4** — 实现 `Pattern::List`（`[a, b, ..rest]`）

**F5** — 实现 `Pattern::Or`/`Pattern::Guard`/`Pattern::Range`

**F6** — 测试：match_demo/match_patterns/enum_data

### 阶段 G：构建管线与 CLI（~10 小阶段）

**G1** — 扩展 `cli.rs`：`--backend=cython` / `--emit=cython` 标志

**G2** — 实现 `cmd_build` Cython 分支：`.lz → .pyx → cythonize → .pyd`

**G3** — 创建 `CY/scripts/lzcyc`（独立 CLI）或集成到 `lang-zone.exe --backend=cython`

**G4** — 扩展 `CY/scripts/cython_build.py`：支持多文件、自动 `setup.py` 生成、`--inplace`

**G5** — 实现 `cmd_check` Cython 模式：build_ir + duck_check（不生成代码）

**G6** — 实现增量编译：`.lzcache_incr/` 缓存 .pyx 哈希

**G7** — 集成 `CY/TESTS` 批量构建：脚本遍历全部参考 .pyx 验证可 cythonize

**G8** — 错误位置回溯：lexer/parser 错误映射到 `.lz` 行列（复用现有）

**G9** — 测试：端到端 `lz build --backend=cython` 在 3 个 DEMO 项目

**G10** — 文档：`CY/README.md` + `--help` 更新

### 阶段 H：DEMO 覆盖与测试基线（~15 小阶段）

**H1–H5** — 跑通 `DEMO/01_basics` → `DEMO/04_functions`（波兰表示法全集）

**H6–H10** — 跑通 `DEMO/05_expressions` → `DEMO/09_oop`（含 duck_demo/duck_relation/duck_assoc/duck_multigen/duck_nested）

**H11–H13** — 跑通 `DEMO/10_error_handling` / `DEMO/11_concurrency` / `DEMO/12_build_blocks`

**H14–H15** — 跑通 `DEMO/13_macros`（降级为 Str）/ `DEMO/14_pointers`（Box/Rc/Arc → object）/ `DEMO/16_testing`

### 阶段 I：高级特性与打磨（~12 小阶段）

**I1** — 所有权与 move 语义：`_Moved` 哨兵注入、borrow 检查（轻量级）

**I2** — 魔法方法全套（`__eq__`/`__lt__`/`__str__`/`__repr__`/`__hash__`/`__call__`/`__enter__`/`__exit__`）

**I3** — 生成器/`yield`：Python 原生 generator（`yield` 直接可用）

**I4** — 异步/`async`：Cython + `asyncio` 或退化为同步

**I5** — 构建块（`=:`/`~:`/`*:`/`^:`）：闭包 + 立即执行

**I6** — checker 块完整：`__Params` 模拟 + default_checker 链

**I7** — 编译期求值 `comptime`：`__lz_comptime` 调用

**I8** — 装饰器（`@memoize`/`@parallel`/`@export`/`@extern`）：Cython 兼容实现

**I9** — 严格模式 `strict` 与 `no_std` 支持

**I10** — 性能关键路径：`cdef` 局部变量类型推断（标量提升）

**I11** — LSP/热重载对 Cython 后端的支持（`--backend=cython`）

**I12** — 测试套件扩充到 100+ 端到端用例

---

## 四、验证铁律（沿用项目既有规范）

> **生成的 `.pyx` 必须能过 `cythonize` 编译且运行正确，才算完成。**

具体验证层级：
1. **语法层**：`cythonize(file.pyx)` 无 error（生成 .c）
2. **编译层**：`.c` → `.pyd` 通过 C 编译器
3. **运行层**：`import mod; mod.main()` 输出与 Rust 后端一致
4. **行为层**：错误处理、所有权哨兵、duck 约束在运行期正确

---

## 五、关键风险与缓解

| 风险 | 缓解 |
|---|---|
| Cython 无原生引用（`&T`/`&mut T`） | 一律 PyObject，放弃引用语义；所有权靠 `_Moved` 哨兵运行时检查 |
| Cython 无 trait | duck 检查在编译期完成；运行时 trait 对象用 `object` + 鸭子方法 |
| Cython 无函数重载 | 自动 mangling + 分发器（与 Rust 端策略一致） |
| 性能 vs PyObject 一律 | 标量位置（函数参/返回/局部）用 C 类型标注，容器/泛型用 object；两全 |
| 变参 `..` 语义 | 单 `..`→`*args`(object)，`..: Tuple<T>`→`*args`，`..: Dict<K,V>`→`**kwargs` |
| 构建管线复杂度 | 复用 `CY/scripts/cython_build.py`，增量扩展 |

---

## 六、建议优先级

按 **A → B → C → D → E → F → G → H → I** 顺序推进，每阶段 6–15 小阶段。理由：

1. **类型映射（A）** 是一切的地基，先做扎实
2. **结构体/枚举（B）** 是 PyO3 结构对齐的核心，验证"PyObject 一律"策略
3. **函数（C）** + **语句（D）** + **表达式（E）** 是覆盖面的主体
4. **Pattern（F）** 依赖 D 完成
5. **构建管线（G）** 早于大规模 DEMO 验证
6. **DEMO 覆盖（H）** 是回归基线
7. **高级特性（I）** 最后打磨

---

## 七、子阶段统计

| 阶段 | 小阶段数 | 核心产出 |
|---|---|---|
| A 类型映射与运行时 | 8 | 完整类型映射 + prelude |
| B 结构体与枚举 | 10 | cdef class + 类层次 enum |
| C 函数与 Item | 12 | cpdef/cdef + trait/impl/duck |
| D 语句覆盖 | 12 | 全部 Stmt 变体 |
| E 表达式覆盖 | 14 | 全部 ExprKind 变体 |
| F Pattern 匹配 | 6 | 全部 Pattern 变体 |
| G 构建管线与 CLI | 10 | lzcyc + cythonize 集成 |
| H DEMO 覆盖 | 15 | 278 个 DEMO 跑通 |
| I 高级特性 | 12 | 魔法方法/异步/构建块等 |
| **合计** | **~99** | **完整 Cython 后端** |

---

*文档版本：v1.0 | 创建：2026-08-19 | 关联：src/ir/codegen_cython.rs / CY/TESTS / src/bridge/python.rs*

# LZIR-H 中间表示 — 详细实现计划

> **版本**: v1.0 | **日期**: 2026-07-29 | **作者**: Autonomous Optimization Architect  
> **状态**: 🟡 待评审  
> **前置文档**: `IR/README.md` (架构), `IR/design.md` (节点规范), `IR/kinds.md` (IR 选型)

---

## 目录

1. [现状分析](#一现状分析)
2. [IR 设计目标](#二ir-设计目标)
3. [数据结构定义](#三数据结构定义)
4. [实现步骤（4 阶段）](#四实现步骤)
5. [风险与约束](#五风险与约束)
6. [附录：检查清单](#六附录检查清单)

---

## 一、现状分析

### 1.1 项目总览

Lang-Zone 是一个源到源编译器，将 `.lz` 源码编译为 Rust `.rs` 文件。项目规模约 **29,000 行 Rust**，采用**零外部依赖**策略（serde 为可选特性）。

**5 层架构** (`src/lib.rs`):

```
L1: lexer, util, config, simd          ← 词法分析 + 基础设施
L2: parser, ast, macros                ← 语法分析 + 宏展开
L3: types, magic, bridge, hints, 
    typing, typer, scope, semantic     ← 语义与类型（大部分未接入主流程）
L4: codegen                            ← Rust 代码发射器
L5: main                               ← CLI 入口
```

### 1.2 当前编译管线

```
.lz 源码
  │
  ▼
[Lexer::tokenize()]          → Vec<Token>          (L1)
  │
  ▼
[MacroExpand]                → Vec<Token>          (L2, 提取定义 → 展开调用)
  │
  ▼
[Parser::parse_module()]     → Module (AST根)      (L2)
  │
  ▼
[CodeGen::generate()]        → String (.rs源码)    (L4)
  │                           AST 直接 → Rust 代码
  ▼
  .rs 文件输出
```

**关键问题：AST → CodeGen 直连，无 IR 层。**

### 1.3 当前紧耦合的具体表现

| 耦合点 | 位置 | 影响 |
|--------|------|------|
| 类型映射硬编码 | `types/def.rs::to_rust_type_string()` | `List→Vec`, `Dict→HashMap` 写死，多后端无法复用 |
| 构建块脱糖内嵌 | `codegen/builders.rs` | `=:`/`^:`/`~:`/`*:` 的降低逻辑与 Rust 发射耦合 |
| 魔法方法代码生成 | `codegen/magic.rs` | `__getitem__→[]`, `__call__→()` 的映射在 Rust 后端特化 |
| Bridge 桥接逻辑 | `codegen/mod.rs::generate()` | `StdBridge`/`SourceBridge`/`BridgeRegistry` 初始化在 codegen 内部 |
| 语义校验被跳过 | `semantic.rs` | `validate_module()` 未在 `main.rs` 调用 |
| 类型推断未接入 | `hints/`, `typer/`, `typing/` | H-M 推断、约束求解、trait 满足性检查均未接入主流程 |

### 1.4 三后端重复现状

当前仓库实际存在 **3 个后端**，各自完整复制了前端：

```
lang-zone/src  (lzc)    ← 主编译器，含完整前端 + codegen
CY/             (lzcyc) ← Cython 后端，含完整前端复制 + codegen_cython
RUST/           (lzrsc) ← Rust 后端（自举目标），含 lexer + parser + type_checker + gen
```

**共享的前端模块（被复制 2~3 遍）**: lexer, parser, ast, scope, semantic, typer, typing, hints, macros, magic — 共约 10 个模块。

### 1.5 IR 插入位置

IR 应插入在 **Parser → CodeGen 之间**，作为前后端的**唯一契约层**：

```
.lz 源码
  │
  ▼
[前端 passes]  ← 唯一实现，不再复制
  │  lexer → parser → AST
  │  → semantic (接入) → typer (接入) → hints (接入) → magic (归一化)
  ▼
══════════════════ LZIR-H 构造器 ══════════════════  ← 新增 pass
  │  AST → 强类型 ANF 树
  │  构建块脱糖 =:→Let, ^:→IndexGet, ~:→Call, *:→GenExpr
  │  魔法方法归一化 → MagicCall/MethodCall
  ▼
[LZIR.Module]  ← 与后端无关的中间表示
  │
  ├─▶ lzrsc  LZIR → Rust     (替代当前 codegen/)
  ├─▶ lzcyc  LZIR → Cython   (后端瘦身为纯发射器)
  └─▶ 未来   任意目标
```

### 1.6 现有 IR 设计资产

`IR/` 目录已有完整的设计规范（约 300 行），包括：
- **`README.md`**: 架构论证（为什么做 IR）、两层设计、后端契约、迁移路径
- **`design.md`**: LZIR-H 节点完整规范（Module/Item/Stmt/Expr/Type/Pattern/Magic）
- **`kinds.md`**: 10 种 IR 种类对比，选型确认 → **ANF 形态**

这些规范文档质量高，可直接作为实现基准。**当前缺少的是：Rust 代码实现、与现有管线的集成、以及测试验证。**

---

## 二、IR 设计目标

### 2.1 核心目标

| # | 目标 | 衡量标准 |
|---|------|----------|
| 1 | **消除前端重复** | `lzrsc`/`lzcyc` 可删除其复制的 lexer/parser/typer/... 模块 |
| 2 | **后端无关性** | LZIR 节点不包含任何目标语言的类型/语法细节 |
| 3 | **类型完整性** | 每个 `Expr` 携带确定的 `Type`（无 `Inferred` 残留） |
| 4 | **语法脱糖完成** | 构建块 `=:`/`^:`/`~:`/`*:` 在进入 IR 前完成脱糖 |
| 5 | **可序列化** | 支持文本格式（可读、可 diff）和二进制格式 |
| 6 | **向后兼容** | 新增节点需 bump IR 版本号，旧后端可拒绝未知版本 |

### 2.2 LZIR-H 设计原则

1. **ANF 形态**: 非平凡计算均命名临时变量，显式求值顺序
2. **强类型树**: 不是 SSA/字节码/CFG —— 消费者是源码发射器
3. **保留 LZ 语义**: struct/enum/trait/impl/魔法方法/内建类型 均保留
4. **最小化**: 不引入 LLVM/MLIR 等重型框架；LZIR-L（低层）按需启用

### 2.3 与现有模块的接口关系

```
                    ┌─────────────────┐
                    │  types::Type     │ ← 直接复用（不含 to_rust_type_string）
                    └────────┬────────┘
                             │
    ┌────────────┐    ┌──────▼──────┐    ┌──────────────┐
    │ magic::    │───▶│  LZIR-H     │◀───│ bridge::     │
    │ MagicEngine│    │  构造器      │    │ BridgeRegistry│
    └────────────┘    └──────┬──────┘    └──────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  LZIR.Module    │  ← 后端无关的 ANF 树
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
        lzrsc emitter  lzcyc emitter   future emitter
```

**依赖方向**（严格遵守）:
```
types ← hints ← typer ← semantic ← scope  (前置, L3)
  │                                    │
  └────────────────┬───────────────────┘
                   ▼
              LZIR 构造器                 (新增, L3→L4 过渡)
                   │
                   ▼
              lzrsc / lzcyc              (改造后, L4)
```

### 2.4 支持的操作类型

LZIR-H 需要覆盖当前 AST 中**所有**表达式/语句类型，加上脱糖后的等价形态：

| 类别 | 节点 | 说明 |
|------|------|------|
| **字面量** | `Lit { kind, ty }` | Int/F64/Str/Bool/Unit/None |
| **变量** | `Var { name, ty }` | 含局部变量、参数、全局常量 |
| **调用** | `Call { callee, args, ty }` | 函数/闭包调用，含 `~:` 脱糖 |
| **方法调用** | `MethodCall { receiver, method, args, ty }` | 方法调用，含魔法方法 |
| **字段访问** | `FieldAccess { base, field, ty }` | 点号访问 |
| **下标访���** | `IndexGet { base, key, ty }` | `^:` 脱糖目标 |
| **下标赋值** | `IndexSet { base, key, value }` | `__setitem__` |
| **二元运算** | `BinOp { op, lhs, rhs, ty }` | 算术/比较/逻辑/位运算 |
| **一元运算** | `UnOp { op, operand, ty }` | 取负/非/引用/解引用 |
| **条件表达式** | `IfExpr { cond, then, els, ty }` | 表达式型 if |
| **Lambda** | `Lambda { params, body, ty }` | 匿名函数 `\|a,b\| a+b` |
| **结构体构造** | `StructCtor { name, fields, ty }` | 含命名参数 |
| **枚举构造** | `EnumCtor { enum, variant, args, ty }` | `Some(x)`, `Ok(v)` 等 |
| **生成器** | `GenExpr { yield_of, ty }` | `*:` 脱糖目标 |
| **类型转换** | `Cast { expr, target, ty }` | 隐式 `.into()` / 显式 `as` |
| **魔法调用** | `MagicCall { kind, args, ty }` | `__iter__`/`__next__`/`__str__`... |
| **块表达式** | `BlockExpr { block, ty }` | 含返回值的代码块 |

---

## 三、数据结构定义

### 3.1 Rust 类型定义概览

推荐新建 `src/ir/` 模块，文件结构：

```
src/ir/
  mod.rs          ← 模块入口 + LZIR Module 定义
  node.rs         ← Item / Stmt / Expr / Pattern 枚举定义
  types.rs        ← LZIR Type 枚举（与 types::Type 解耦）
  builder.rs      ← AST → LZIR 构造器
  display.rs      ← Display / Debug 文本输出（开发用）
  serialize.rs    ← 序列化（文本/二进制，可选特性）
```

### 3.2 核心节点定义（Rust 伪代码）

```rust
// ── src/ir/node.rs ──

/// LZIR 顶层模块
pub struct Module {
    pub name: String,
    pub magic: MagicAttrs,
    pub items: Vec<Item>,
    pub prelude: Vec<String>,          // 默认导入清单
    pub version: u32,                   // IR 版本号（初始 = 1）
}

/// 顶层定义项
pub enum Item {
    FnDef(FnDef),
    StructDef(StructDef),
    EnumDef(EnumDef),
    TraitDef(TraitDef),
    Impl(ImplDef),
    Use(UseStmt),
}

/// 函数定义（已脱糖、已类型推断）
pub struct FnDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub ret_ty: IrType,
    pub body: Block,
    pub intrinsics: Vec<Intrinsic>,    // @memoize, @parallel, @export(...) 等
    pub is_test: bool,
    pub span: Span,
}

pub struct Param {
    pub name: String,
    pub ty: IrType,
    pub is_mut: bool,
}

/// 结构体定义
pub struct StructDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub fields: Vec<Field>,
    pub methods: Vec<FnDef>,
    pub magic: Vec<MagicImpl>,
    pub span: Span,
}

pub struct Field {
    pub name: String,
    pub ty: IrType,
}

/// 枚举定义
pub struct EnumDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub variants: Vec<Variant>,
    pub span: Span,
}

pub struct Variant {
    pub name: String,
    pub fields: Vec<IrType>,           // 变体携带的数据类型
}

/// Trait 定义（对标魔法协议）
pub struct TraitDef {
    pub name: String,
    pub supertraits: Vec<IrType>,
    pub methods: Vec<FnSig>,
}

pub struct FnSig {
    pub name: String,
    pub params: Vec<IrType>,
    pub ret: IrType,
}

/// Impl 定义
pub struct ImplDef {
    pub trait_: Option<IrType>,        // None = inherent impl
    pub for_type: IrType,
    pub generics: Vec<GenericParam>,
    pub methods: Vec<FnDef>,
}

/// Use 语句（仅记录依赖，不展开）
pub struct UseStmt {
    pub path: Vec<String>,
    pub items: Vec<String>,
}

// ── Stmt 节点 ──

pub enum Stmt {
    Let {
        name: String,
        ty: IrType,
        value: Expr,
        is_mut: bool,
    },
    Assign {
        target: Expr,                  // 可赋值左值（Var/FieldAccess/IndexGet）
        value: Expr,
    },
    Return {
        value: Option<Expr>,
    },
    ExprStmt {
        expr: Expr,
    },
    If {
        cond: Expr,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    For {
        var: String,
        iter: Expr,
        body: Block,
    },
    While {
        cond: Expr,
        body: Block,
    },
    Match {
        scrutinee: Expr,
        arms: Vec<(Pattern, Block)>,
    },
    Block {
        stmts: Vec<Stmt>,
    },
}

// ─�� Expr 节点（强类型，携带 IrType） ──

pub struct Expr {
    pub kind: ExprKind,
    pub ty: IrType,                    // 必定确定，无 Inferred
    pub span: Span,
}

pub enum ExprKind {
    Lit(LitKind),
    Var(String),
    Call { callee: Box<Expr>, args: Vec<Expr> },
    MethodCall { receiver: Box<Expr>, method: String, args: Vec<Expr> },
    FieldAccess { base: Box<Expr>, field: String },
    IndexGet { base: Box<Expr>, key: Box<Expr> },
    IndexSet { base: Box<Expr>, key: Box<Expr>, value: Box<Expr> },
    BinOp { op: BinOpKind, lhs: Box<Expr>, rhs: Box<Expr> },
    UnOp { op: UnOpKind, operand: Box<Expr> },
    IfExpr { cond: Box<Expr>, then: Box<Expr>, els: Box<Expr> },
    Lambda { params: Vec<Param>, body: Box<Expr> },
    StructCtor { name: String, fields: Vec<(String, Expr)> },
    EnumCtor { enum_name: String, variant: String, args: Vec<Expr> },
    GenExpr { yield_of: Box<Expr> },
    Cast { expr: Box<Expr>, target: IrType },
    MagicCall { kind: MagicKind, args: Vec<Expr> },
    BlockExpr { block: Block },
}

// ── Pattern 节点 ──

pub enum Pattern {
    Wildcard,
    Ident(String),
    Lit(LitKind),
    Tuple(Vec<Pattern>),
    Struct { name: String, fields: Vec<(String, Pattern)> },
    Enum { enum_name: String, variant: String, args: Vec<Pattern> },
}

// ── 辅助类型 ──

pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<IrType>,
    pub default: Option<IrType>,
}

pub struct Block {
    pub stmts: Vec<Stmt>,
    pub ty: IrType,                    // 块的结果类型
}

pub enum LitKind { Int(i64), F64(f64), Str(String), Bool(bool), Unit, None_ }

pub enum BinOpKind { Add, Sub, Mul, Div, Mod, Eq, Neq, Lt, Gt, Le, Ge, And, Or, BitAnd, BitOr, Xor, Shl, Shr }

pub enum UnOpKind { Neg, Not, Ref, MutRef, Deref }

pub struct MagicAttrs {
    pub name: Option<String>,
    pub doc: Option<String>,
    pub all: Option<Vec<String>>,
    pub bridge: Option<String>,
    pub bridge_tier: Option<String>,
}

pub struct Span { pub start: usize, pub end: usize, pub line: usize, pub col: usize }

pub struct Intrinsic {
    pub kind: IntrinsicKind,
    pub span: Span,
}

pub enum IntrinsicKind {
    Memoize, Parallel, Curry, Overload, Derive, TailCall, Export(Vec<String>), Init,
}
```

### 3.3 IrType 与 types::Type 的关系

```rust
// src/ir/types.rs

/// LZIR 类型系统 —— 与 crate::types::Type 解耦，独立于后端
#[derive(Debug, Clone, PartialEq)]
pub enum IrType {
    // 内建原语
    Int, F64, Str, Bool, Unit, Never, Any,
    // 命名类型（含泛型参数）
    Named { path: String, args: Vec<IrType> },
    // 特殊容器（语义标记）
    Option(Box<IrType>),
    Result { ok: Box<IrType>, err: Box<IrType> },
    // 复合类型
    Tuple(Vec<IrType>),
    Fn { params: Vec<IrType>, ret: Box<IrType> },
    Ref(Box<IrType>),
    MutRef(Box<IrType>),
    // 泛型变量
    Generic(String),
}
```

**关键设计决策**: `IrType` 独立于 `types::Type`，因为：
1. `types::Type` 携带 `to_rust_type_string()` 等 Rust 专属方法
2. `types::Type` 有 `Optional`（语法糖）、`Simd`、`Self_` 等前端特有变体
3. IR 只表达语义类型，不含语法糖或后端映射

**映射规则**: `types::Type` → `IrType` 在构造阶段完成，`Optional(T)` → `Option(T)`，`Self_` → 具体类型名。

---

## 四、实现步骤

### 阶段 0：前置准备（预计 1-2 天）

#### 0.1 接入类型推断管线

**当前状态**: `hints/`, `typer/`, `typing/` 已实现但未接入 `main.rs`。  
**必须做**: IR 要求每个 `Expr` 携带确定类型。必须先将类型推断接入主流程。

```rust
// src/main.rs 新增（在 CodeGen 之前）：
let typed_module = typer::Typer::infer_module(&module)?;
let errors = semantic::validate_module(&typed_module);
if !errors.is_empty() { /* report and exit */ }
```

**具体任务**:
- [ ] 在 `main.rs` 中接入 `Typer::infer_module()`
- [ ] 在 `main.rs` 中接入 `semantic::validate_module()`
- [ ] 确保推断后的 AST 节点携带完整 Type 信息
- [ ] 验证：所有 DEMO 文件通过推断管线

#### 0.2 接入作用域分析

- [ ] 接入 `scope/` 模块的逃逸分析
- [ ] 标记需要堆分配的闭包变量

#### 0.3 建立基线与测试

- [ ] 记录当前 `lzc` 编译 DEMO/ 全集的成功率
- [ ] 记录当前 `lzc` 编译产物的 rustc 编译通过率
- [ ] 建立回归测试：IR 产物的文本快照

---

### 阶段 1：IR 数据结构落地（预计 2-3 天）

#### 1.1 创建 `src/ir/` 模块

```
src/ir/
  mod.rs       ← Module 定义 + 版本号常量
  node.rs      ← Item / Stmt / Expr / Pattern / 辅助类型
  types.rs     ← IrType 枚举
  display.rs   ← Display trait 实现（树形格式输出）
```

**任务清单**:
- [ ] 实现 §3.2 中所有枚举和结构体
- [ ] 实现 `Display for Module`（树形文本输出，用于 `--emit=ir` 和快照测试）
- [ ] 实现 `PartialEq` / `Clone` / `Debug` 派生
- [ ] 注册 `pub mod ir;` 到 `src/lib.rs`（L3 层）
- [ ] 单元测试：构造手工 LZIR Module，验证 Display 输出

#### 1.2 IrType 映射表

```
types::Type          →  IrType
─────────────────────────────────
Int                  →  Int
F64 / Float          →  F64
Str                  →  Str
Bool                 →  Bool
None_                →  Named { path: "None" }
Never                →  Never
Any                  →  Any
Unit                 →  Unit
Named(s)             →  Named { path: s }
Generic { base, .. } →  Named { path: base.name, args }
Option(t)            →  Option(t)
Result { ok, err }   →  Result { ok, err }
Optional(t)          →  Option(t)            ← 语法糖展开
Fn { params, ret }   →  Fn { params, ret }
Tuple(ts)            →  Tuple(ts)
Ref(t) / MutRef(t)   →  Ref(t) / MutRef(t)
Self_                →  (替换为具体类型名)     ← 需上下文
Simd { .. }          →  Named { path: "Simd" }← 简化（待定）
```

- [ ] 实现 `fn to_ir_type(t: &Type, ctx: &IrCtx) -> IrType`
- [ ] 单元测试：覆盖所有 `Type` 变体的映射

---

### 阶段 2：AST → LZIR 构造器（预计 3-4 天）

#### 2.1 构造器核心 `builder.rs`

```rust
// src/ir/builder.rs

pub struct IrBuilder {
    // 上下文
    module_name: String,
    struct_names: HashSet<String>,
    enum_names: HashSet<String>,
    // 魔法方法引擎（从 magic::MagicEngine 读取）
    magic_engine: MagicEngine,
    // 桥接类型映射
    builtin_types: HashMap<String, IrType>,
}

impl IrBuilder {
    /// 主入口：AST Module → LZIR Module
    pub fn build(ast: &Module, ctx: BuildContext) -> Result<ir::Module, IrBuildError> {
        // 1. 收集 struct/enum 名称
        // 2. 构建项列表
        // 3. 填充 prelude
        // 4. 返回带版本号的 LZIR Module
    }
}
```

#### 2.2 构建块脱糖规则

在 AST → LZIR 转换时完成以下脱糖（后端永远看不到构建块语法）:

| AST 节点 | LZIR 节点 | 脱糖逻辑 |
|----------|-----------|----------|
| `BuildBlock { kind: LetBuild, lhs, rhs }` | `Stmt::Let { name, value }` | `x =: body` → let 绑定 |
| `BuildBlock { kind: IndexBuild, lhs, rhs }` | `Expr::IndexGet { base: lhs, key: rhs }` | `c ^: k` → 下标访问 |
| `BuildBlock { kind: CallBuild, lhs, rhs }` | `Expr::Call { callee: lhs, args: desugar(rhs) }` | `f ~: (args)` → 函数调用 |
| `BuildBlock { kind: GenBuild, lhs, rhs }` | `Expr::GenExpr { yield_of: rhs }` | `g *: yield e` → 生成器 |

#### 2.3 魔法方法归一化

| AST 语义 | LZIR 节点 |
|----------|-----------|
| `a[i]` (Index 表达式) | `IndexGet { base: a, key: i }` |
| `a[i] = v` (赋值) | `IndexSet { base: a, key: i, value: v }` |
| `for x in iter` | `MagicCall { kind: IntoIter, args: [iter] }` + `MagicCall { kind: Next }` |
| `str(x)` (隐式) | `MagicCall { kind: Display, args: [x] }` |
| `x == y` (含魔法) | `MagicCall { kind: Eq, args: [x, y] }` |

#### 2.4 AST 表达式 → LZIR 表达式映射

逐节点对应（简化列表，全量见 `IR/design.md`）:

| AST Expr | LZIR ExprKind |
|----------|---------------|
| `IntLit(n)` | `Lit(Int(n))` |
| `FloatLit(n)` | `Lit(F64(n))` |
| `StrLit(s)` | `Lit(Str(s))` |
| `BoolLit(b)` | `Lit(Bool(b))` |
| `NoneLit` | `Lit(None_)` |
| `Ident(s)` | `Var(s)` |
| `Call { func, args }` | `Call { callee, args }` |
| `MethodCall { .. }` | `MethodCall { .. }` |
| `FieldAccess { .. }` | `FieldAccess { .. }` |
| `Index { receiver, index }` | `IndexGet { base, key }` |
| `Binary { left, op, right }` | `BinOp { op, lhs, rhs }` |
| `Unary { op, operand }` | `UnOp { op, operand }` |
| `If { cond, then_body, .. }` | `IfExpr { cond, then, els }` (表达式型) |
| `Closure { params, body }` | `Lambda { params, body }` |
| `Range { start, end, .. }` | `StructCtor { name: "Range", fields: .. }` |
| `TupleLit(elems)` | `StructCtor { name: "...", fields: elems }` (含 Tuple) |
| `ListLit(items)` | `Call { callee: Var("vec!"), args: items }` 或保留为特殊容器 |
| `Walrus { target, value }` | 展开为 Let + Var |
| `Pipe { receiver, func, args }` | `Call { callee: func, args: [receiver, ..args] }` |
| `SafeNav { receiver, field }` | `IfExpr { cond: null_check, then: FieldAccess, els: None }` |
| `Try(expr)` | `MagicCall { kind: Try, args: [expr] }` |
| `NullCoalesce { left, right }` | `IfExpr { cond: null_check, then: left, els: right }` |

#### 2.5 任务清单

- [ ] `src/ir/builder.rs`: 实现 `IrBuilder` 结构体 + `build()` 入口
- [ ] `build_module()`: Module 顶层转换（items + prelude + magic_attrs）
- [ ] `build_fn()`: Function → FnDef（泛型、参数、返回值、body、intrinsics）
- [ ] `build_struct()`: StructDef → StructDef/EnumDef（根据 is_enum 分派）
- [ ] `build_trait()` / `build_impl()`: TraitDef/ImplDef 转换
- [ ] `build_stmt()`: Stmt 枚举匹配 → LZIR Stmt（含构建块脱糖）
- [ ] `build_expr()`: Expr 枚举匹配 → LZIR Expr（含 Type 提取）
- [ ] `build_pattern()`: Pattern 转换
- [ ] 错误类型 `IrBuildError`（不可恢复：缺少类型、未解析引用等）

---

### 阶段 3：集成与验证（预计 2-3 天）

#### 3.1 CLI 集成

```rust
// src/main.rs 新增流程：
// 原有: lexer → macro expand → parser → codegen → .rs
// 新增: lexer → macro expand → parser → typer → semantic → ir → [codegen | emit-ir]

// 新增 CLI 标志：
// --emit=ir    → 输出 LZIR 文本（不生成 .rs）
// --format=json → LZIR 输出为 JSON（用于工具链）
```

- [ ] 在 `main.rs` 中接入 `IrBuilder::build()`
- [ ] 添加 `--emit=ir` 标志
- [ ] 添加 `--emit=ir --output <file>` 指定输出路径
- [ ] 确保 codegen 路径不受影响（`--emit=ir` 为可选分支）

#### 3.2 快照测试

```rust
// tests/ir_snapshots.rs

#[test]
fn test_ir_roundtrip_demos() {
    for demo in glob("DEMO/**/*.lz") {
        let ir = compile_to_ir(&demo);
        let snapshot_path = demo.replace(".lz", ".ir.txt");
        // 首次运行生成快照，后续运行对比
        assert_snapshot!(snapshot_path, ir);
    }
}
```

- [ ] 创建 `tests/ir_snapshots.rs` 测试文件
- [ ] 为 DEMO/ 下所有 demo 生成 IR 快照文本
- [ ] CI 中自动对比 IR 快照

#### 3.3 端到端验证

- [ ] 选择 5-10 个代表性 DEMO（覆盖基本类型、控制流、数据结构、魔法方法）
- [ ] 验证: `.lz → LZIR → 文本输出` 可读且语义正确
- [ ] 验证: `lzc --emit=ir file.lz` 输出与预期快照匹配
- [ ] 验证: 所有 DEMO 仍然能通过原有 `--test` 路径编译运行

---

### 阶段 4：后端改造（预计 4-5 天）

#### 4.1 `lzrsc` (RUST/) 后端改造

- [ ] 删除 `RUST/` 中的 lexer / parser / type_checker 模块
- [ ] 实现 `fn emit(module: &ir::Module) -> String`
- [ ] 建立"IrType → Rust 类型"映射表（替代 `to_rust_type_string()`）
- [ ] 建立"LZIR 节点 → Rust 语法"映射表（替代 codegen trait 扩展）
- [ ] 验证：`lzrsc` 能通过所有正向测试

#### 4.2 `lzcyc` (CY/) 后端改造

- [ ] 删除 `CY/` 中的 lexer / parser / ... 模块
- [ ] 实现 `fn emit(module: &ir::Module) -> String`
- [ ] 建立"IrType → Cython 类型"映射表
- [ ] 建立"LZIR 节点 → Cython 语法"映射表
- [ ] 验证：`lzcyc` 能编译通过

#### 4.3 当前 `codegen/` 模块的迁移

**策略**: 渐进式替换，不是一次性删除。

```
Phase 4.3a: 保留现有 codegen/，新增 LZIR → Rust 发射器在独立模块
Phase 4.3b: LZIR 发射器通过全部测试后，标记 codegen/ 为 deprecated
Phase 4.3c: 删除 codegen/，完全切换到 LZIR 发射器
```

- [ ] 在 `lang-zone/src` 新增 `emitter/` 模块（LZIR → Rust 发射器）
- [ ] 接口: `fn emit_rust(module: &ir::Module, opts: EmitOptions) -> String`
- [ ] 复用现有 `codegen/` 的辅助逻辑（字符串转义、泛型展开等）
- [ ] 验证：`lzc --emit=rust` 产出与旧 `codegen/` 相同的 .rs 文件

---

## 五、风险与约束

### 5.1 技术风险

| 风险 | 等级 | 缓解措施 |
|------|------|----------|
| **类型推断管线未接入主流程** | 🔴 高 | 阶段 0 必须优先完成；hints/typer/typing 已有架构，主要工作是集成 |
| **构建块脱糖覆盖不全** | 🟡 中 | 以 DEMO/ 中所有构建块用例为回归基准；新增构建块语法需同步更新脱糖规则 |
| **IrType 与 types::Type 映射遗漏** | 🟡 中 | 全量映射表 + 单元测试覆盖所有变体；`Simd`、`Self_` 等边界需明确 |
| **codegen 迁移破坏现有功能** | 🟡 中 | 渐进式替换（4.3 策略）；保留旧 codegen 作为回退路径 |
| **IR 序列化格式不稳定** | 🟢 低 | 文本格式先用于开发/测试；二进制格式按需引入 |
| **三后端迁移不同步** | 🟡 中 | 以 `lzc`（参考编译器）为主；`lzrsc`/`lzcyc` 在 IR 稳定后迁移 |

### 5.2 依赖约束

- ✅ **零外部依赖**: `src/ir/` 不能引入新的 crate 依赖；序列化格式需要自行实现或用可选 feature
- ✅ **不破坏现有测试**: DEMO/ 下所有 `.lz` 文件必须在改造后仍能编译运行
- ✅ **向后兼容**: `--emit=ir` 为纯增量功能，不影响现有 CLI 行为
- ✅ **Rust edition 2021**: 保持与 Cargo.toml 一致

### 5.3 项目规范

1. **命名约定**:
   - IR 数据结构: `PascalCase`（Module, FnDef, StructDef, ExprKind）
   - 枚举变体: `PascalCase`（Lit, Var, Call, ...）
   - 辅助函数: `snake_case`（build_module, build_fn, to_ir_type）
   - 模块路径: `src/ir/` 目录，`pub mod ir` 在 `lib.rs` L3 层

2. **错误处理**:
   - 构造阶段错误: `IrBuildError` 枚举（不可恢复）
   - 不 panic，返回 `Result<Module, IrBuildError>`
   - 错误信息包含 span 位置

3. **测试要求**:
   - 单元测试: IrType 映射、构建块脱糖、AST→IR 逐节点转换
   - 集成测试: DEMO/ 全集的 IR 快照测试
   - 回归测试: 确保旧 codegen 路径仍然正常工作

4. **文档要求**:
   - `IR/README.md` 更新实现状态
   - `IR/design.md` 补充 ANF 示例
   - 每个 `src/ir/*.rs` 文件头部 module doc

### 5.4 暂不处理（显式排除）

| 项目 | 原因 |
|------|------|
| LZIR-L（低层 IR / CFG / SSA） | 设计为 ⚪ 规划中；先完成 HIR 再评估 |
| 二进制序列化 | 文本格式优先；二进制按需（需 serde feature） |
| LZIR 优化 passes（死代码/内联） | 属于 LIR 范畴；HIR 是"忠实记录"而非"优化" |
| 增量编译缓存 | 独立 pass，不阻塞 IR |
| `.lzi` 签名文件的 IR 表达 | `infer/` 模块独立，可后续统一 |

---

## 六、附录：检查清单

### 实现就绪检查

- [ ] `hints/` + `typer/` 类型推断接入 main.rs
- [ ] `semantic.rs` 语义校验接入 main.rs
- [ ] `scope/` 作用域分析接入 main.rs
- [ ] `src/ir/` 模块创建并注册到 lib.rs
- [ ] `IrType` 定义 + `types::Type → IrType` 映射函数
- [ ] `Module` / `Item` / `Stmt` / `Expr` / `Pattern` 节点定义
- [ ] `IrBuilder` 构造器实现（build_fn / build_struct / build_expr / ...）
- [ ] 构建块脱糖（=:/^:/~:/*: → Let/IndexGet/Call/GenExpr）
- [ ] 魔法方法归一化（MagicCall / MethodCall）
- [ ] `Display` 实现（树形文本输出）
- [ ] `--emit=ir` CLI 标志
- [ ] 快照测试框架 + DEMO 全量 IR 快照
- [ ] 端到端验证（5-10 代表性 demo）
- [ ] `lzrsc` (RUST/) 瘦身 + LZIR 发射器
- [ ] `lzcyc` (CY/) 瘦身 + LZIR 发射器
- [ ] 旧 `codegen/` 保留 + 新 LZIR 发射器并行验证
- [ ] 文档更新（IR/README.md + IR/design.md）

### 风险缓解检查

- [ ] 全量 DEMO 在改造后仍编译运行
- [ ] 旧 `--test` 路径不受影响
- [ ] 零新增外部依赖
- [ ] 所有 panic 替换为 Result

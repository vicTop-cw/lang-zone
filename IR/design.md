# LZIR-H 节点设计（规范 / 标准）

> 本文件定义 **LZIR-H（高层 IR）** 的节点契约——前端（`lzc`）产出它，后端（`lzrsc` / `lzcyc` / 未来）消费它。
> 这是**规范文档**：只定义结构与语义，**不含实现**，不修改 `src/`。
> 形态：**强类型、树 / ANF**。每个 `Expr` 携带 `ty: Type` 与 `span`（用于 sourcemap）。

状态图例：✅ 已定 · 🟡 待评审。

---

## 一、顶层结构

```text
Module          一个 .lz 文件编译后的根
  ├─ name: str                 # 模块名（来自 __name__ / 路径）
  ├─ magic: MagicAttrs         # 模块级魔法属性（__name__/__doc__/__all__/__bridge__…）
  ├─ items: Vec<Item>          # 顶层定义
  └─ prelude: Prelude          # 默认导入的 lz.std 内建清单（见 LZSTD/）

Item = FnDef | StructDef | EnumDef | TraitDef | Impl | Use(import 语句残留，仅记录依赖)
```

---

## 二、Item 节点

### FnDef ✅
```text
FnDef {
  name: str
  params: Vec<Param>          # Param { name, ty: Type, mut: bool }
  ret_ty: Type
  body: Block
  intrinsics: Vec<Intrinsic>  # @memoize / @parallel / @curry / @overload / @derive / @tail_call / @export(..) / @init
  is_test: bool               # test 关键字块
}
```

### StructDef ✅
```text
StructDef {
  name: str
  generics: Vec<TypeParam>
  fields: Vec<Field>          # Field { name, ty: Type }
  methods: Vec<FnDef>         # 含 __call__（__call__ 使构造器可调用）
  magic: Vec<MagicImpl>       # 用户魔法方法块
}
```

### EnumDef ✅
```text
EnumDef {
  name: str
  generics: Vec<TypeParam>
  variants: Vec<Variant>      # Variant { name, fields: Vec<Type> }  // 如 Some(T) / None / Ok(T) / Err(E)
}
```
> 内建 `Option` / `Result` / `Ordering` / `ItorState` / `BridgeTier` 也以同形态表达（见 `LZSTD/builtins.md`）。

### TraitDef / Impl ✅
```text
TraitDef { name, supertraits: Vec<Type>, methods: Vec<FnSig> }   # 对应魔法方法协议 Iterable/Iterator/Index/…
Impl    { trait_: Type, for_type: Type, methods: Vec<FnDef> }
```

---

## 三、Stmt 节点

```text
Stmt =
  | Let      { pat: Pattern, ty: Type, value: Expr, mut: bool }   # 来自 =: 构建块 / let
  | Assign   { target: Expr, value: Expr }                        # 重新赋值
  | Return   { value: Option<Expr> }
  | ExprStmt { expr: Expr }
  | If       { cond: Expr, then: Block, els: Option<Block> }
  | For      { var: str, iter: Expr, body: Block }                 # 迭代器（支持 1..10 区间，见 issue/parser-dotdot-in-for.md）
  | While    { cond: Expr, body: Block }
  | Match    { scrut: Expr, arms: Vec<(Pattern, Block)> }
  | Block    { stmts: Vec<Stmt> }                                  # 复合块（含构建块块体）
```

---

## 四、Expr 节点（强类型，携带 ty）

```text
Expr =
  | Lit          { kind: LitKind, ty: Type }           # Int / F64 / Str / Bool / Unit
  | Var          { name: str, ty: Type }
  | Call         { callee: Expr, args: Vec<Expr>, ty: Type }       # 普通调用 / ~: 降此后
  | MethodCall   { receiver: Expr, method: str, args: Vec<Expr>, ty: Type }  # 方法（含魔法方法）
  | FieldAccess  { base: Expr, field: str, ty: Type }
  | IndexGet     { base: Expr, key: Expr, ty: Type }   # ^: 脱糖目标
  | IndexSet     { base: Expr, key: Expr, value: Expr }# 下标赋值（__setitem__）
  | BinOp        { op: BinOp, lhs: Expr, rhs: Expr, ty: Type }
  | UnOp         { op: UnOp, operand: Expr, ty: Type }
  | IfExpr       { cond: Expr, then: Expr, els: Expr, ty: Type }   # 表达式型 if
  | Lambda       { params: Vec<Param>, body: Expr, ty: Type }      # |a,b| a+b（Pipe_）
  | StructCtor   { name: str, fields: Vec<(str, Expr)>, ty: Type }
  | EnumCtor     { enum: str, variant: str, args: Vec<Expr>, ty: Type }
  | GenExpr      { yield_of: Expr, ty: Type }                       # *: 生成器
  | Cast         { expr: Expr, target: Type, ty: Type }             # 隐式/显式转换
  | MagicCall    { kind: MagicKind, args: Vec<Expr>, ty: Type }     # __iter__/__next__/__str__/__eq__/__cmp__/__drop__/__rev__/__len__
  | BlockExpr    { block: Block, ty: Type }
```

> **说明**：`^:` 进入 HIR 时已是 `IndexGet`（不再保留 `^:` 语法节点）；后端见到的永远是 `IndexGet(base, key)`，直接映射 `base[key]`（Rust）或 `base[key]`（Cython）。

---

## 五、构建块脱糖映射（关键红线）

前端的 `ir` pass 在产出 HIR **之前**完成以下脱糖，后端**永远看不到**构建块语法：

| 源码构建块 | HIR 节点 | 脱糖语义 |
|---|---|---|
| `x =:\n  body` | `Let(x, body)` / `Assign` | 变量绑定（已是普通绑定） |
| `c ^:\n  k` | `IndexGet(c, k)` | `c.__getitem__(k)` |
| `f ~:\n  (args)` | `Call(f, args)` | 块尾元组/字典作为参数 |
| `g *:\n  yield e` | `GenExpr(e)` | 生成器表达式 |

> 注意：`=:`/`^:`/`~:`/`*:` 是**整体 token、冒号后换行缩进、前后留白**（见 `SYNTAX/11-构建块.md`）。脱糖发生在前端，HIR 不保留这些 token。

---

## 六、Pattern 节点

```text
Pattern =
  | Wildcard            # _
  | Ident     { name: str }
  | LitPat    { lit: Lit }
  | TuplePat  { elems: Vec<Pattern> }
  | StructPat { name: str, fields: Vec<(str, Pattern)> }
  | EnumPat   { enum: str, variant: str, args: Vec<Pattern> }
```

---

## 七、Type 节点（携带于 Expr / 声明）

```text
Type =
  | Named   { path: str, args: Vec<Type> }   # lz.std 内建 / 用户定义 / 后端原语
  | Tuple   { elems: Vec<Type> }
  | Fn      { params: Vec<Type>, ret: Type }
  | Generic { name: str }                    # 泛型变量
  | Inferred                               # 前端未定（不应出现在 HIR；HIR 必须已定类型）
```

> **类型来自 `lz.std` 命名空间约定**（见 `LZSTD/`）：`Option<T>`/`Result<T,E>`/`Box<T>`/`Itor<T>` 等以 `Named` 表达；`rust.std` 专属类型不进入 HIR（由 `lzrsc` 经 Bridge 在 lowering 时处理）。

---

## 八、Magic / Intrinsic 表示

- **魔法方法调用**：统一为 `MagicCall { kind }` 或 `MethodCall { method: "__getitem__" }`——二者等价，后端按目标映射（如 `__getitem__` → `[]`）。
- **模块级魔法属性**（`__name__`/`__doc__`/`__all__`/`__bridge__`/`__bridge_tier__`）：放在 `Module.magic: MagicAttrs`，供后端生成目标元数据。
- **`@intrinsics`**：挂在 `FnDef.intrinsics`，后端据此生成目标注解（如 `@memoize` → Rust 的 `OnceLock<HashMap>` 缓存；`@parallel` → `rayon::par_iter`）。具体 lowering 规则见各后端文档。

---

## 九、序列化格式（🟡 建议）

- **开发期 / 测试**：文本格式（自描述、可 diff、易写快照测试）。前端跑通后 `lzc --emit=ir file.lz` 应输出可读 HIR。
- **生产期（可选）**：二进制（bincode / 自研紧凑格式）降低大项目序列化开销。
- **契约稳定性**：HIR 节点一旦冻结，后端按节点消费；新增节点需 bump IR 版本号（见 `IR/README.md` 六 待决）。

---

## 十、与现有代码的关系

| 现有位置 | 角色 | 迁移后 |
|---|---|---|
| `lang-zone/src/ast` | 前端 AST（树） | 保留为前端内部；末尾新增 `ir` pass 把 AST → LZIR-H |
| `lang-zone/src/codegen` | Rust 发射器（参考实现） | 改为消费 LZIR-H 而非 AST |
| `CY/src/ast` + `codegen_cython` | 复制的前端 + Cython 发射器 | 删前端，仅留 Cython 发射器消费 LZIR-H |
| `RUST/src/{lexer,parser,type_checker,gen}` | 复制的前端 + Rust 发射器 | 删前端，仅留 `gen` 消费 LZIR-H |

> 本文件是**方向标**：定义"IR 长什么样"。具体 `ir` pass 实现与后端改造由工程侧按 `IR/README.md` 第五节路径落地。

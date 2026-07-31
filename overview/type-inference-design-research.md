# lz 类型自推断系统 & 类型自判断机制 — 调研报告

> 目标：为 lz（.lz → Rust 转译器）设计自有类型推断与类型自判断（reflection / self-awareness）机制。
> 方法论："拿来主义"——融合标准库、三方库、主流语言的成熟设计，落地到 lz 现有架构之上。

---

## 0. 当前基线（代码库探查结论）

lz 当前是**纯注解驱动、完全具体类型**的转译器，推断基础设施为空白：

| 项 | 现状 | 锚点 |
|----|------|------|
| `Type` 枚举 | `src/types/def.rs:23` 只有具体变体（Int/F64/Str/Named/Generic/Option/Result/Ref/Fn/Tuple/Simd/Self_） | **无 type variable / inference hole / unification var** |
| 类型解析 | `parser.rs:544` `parse_type`，注解可选但**无 `auto`/`_`/`infer` 占位** | 需新增占位关键字 |
| 类型映射 | `codegen/mod.rs:304` `map_type` 直接 `to_rust_type_string()` | **无推断步骤** |
| 推断基础设施 | grep `infer\|unify\|constraint\|subst\|Scheme` 零真实匹配 | 从零搭建 |
| 反射钩子 | `comptime` 仅被 lexer 识别为 token（`lexer/token.rs:34`），未接入逻辑 | **可直接复用此 token** |
| magic 系统 | `src/magic/` 已注册 `__add__` 等魔法方法映射 | 类型推断可与 magic dispatch 共享"约束求解"思路 |

**结论**：lz 是为未来推断设计可安全锚定的现实基线。所有推荐方案需叠加在 `Type` / `magic` / `codegen` 之上。

---

## 维度一：现有库的功能挖掘

### 方案 1.1 — `ena` 联合查找（union-find）统一表【直接复用，P0】
- **来源**：`ena`（nikomatsakis 的 Rust crate，rustc 内部统一算法的同款实现）
- **机制**：`ena::unify::UnificationTable<InferVar, Type>` 提供 inference variable 的 union-find 存储、等价绑定、occurs-check 钩子。
- **可落地点**：lz 的 `Type` 新增 `Type::Var(InferVar)` 后，直接用它存统一状态，**无需自己写可变类型环境**。
- **优先级**：P0（推断引擎的基石，且是"拿来即用"的成熟库）。

### 方案 1.2 — `std::any::Any` + `TypeId` 运行时反射骨架【P2】
- **来源**：Rust 标准库
- **机制**：`Any` trait + `TypeId::of::<T>()` 提供运行时类型标识与 `downcast_ref::<T>()`。
- **可落地点**：lz 的"类型自判断"在需要运行时分支时（如 `match typeof x`），编译为 `Box<dyn Any>` + downcast，或直接用 `TypeId` 做字典键。
- **优先级**：P2（编译期推断不依赖它，但"自判断"的运行时层需要）。

### 方案 1.3 — `typetag` 式类型注册表 + 擦除 trait 对象【P2】
- **来源**：`typetag` / `erased-serde`（dtolnay）
- **机制**：通过过程宏把 `trait Foo: Any` 的 impl 注册进全局类型注册表，实现对象安全的序列化/动态分发。
- **可落地点**：lz 若要让用户自定义类型可被"自判断"系统识别（如 `reflect(x)` 返回字段表），可借用"类型注册表 + `Any` downcast"模式，避免每个类型手写反射代码。
- **优先级**：P2（中等成本，解决"自定义类型如何被反射"）。

### 方案 1.4 — `im` 持久化数据结构做不可变类型环境【P3】
- **来源**：`im`（persistent data structures，Clojure-style）
- **机制**：持久化 `HashMap`/`Vec` 支持 O(log n) 拷贝即分支，天然适合不可变上下文。
- **可落地点**：若 lz 推断要做**并行/增量**求解（你之前强调"并行"），不可变环境可避免锁竞争。
- **优先级**：P3（仅在并行推断阶段需要，可后置）。

### 方案 1.5 — `chalk` 逻辑求解器概念用于 magic/trait 分发【P3】
- **来源**：`rust-lang/chalk`（Prolog-like trait 求解器）
- **机制**：把 trait 规则降级为逻辑子句，用 SLG/tabling 求解；coinduction 处理递归 trait。
- **可落地点**：lz 已有 `MagicEngine`，可逐步把"魔法方法 → Rust trait impl"的匹配形式化为小型逻辑求解，**复用 chalk 的 clause/cache 思路**而非整体引入。
- **优先级**：P3（重，建议仅借鉴概念）。

---

## 维度二：主流/优秀语言的类型推断与递归推断机制

### 方案 2.1 — Haskell Hindley-Milner（Algorithm W）+ occurs-check【P0】
- **来源**：Hindley-Milner / Damas-Milner / Algorithm W
- **机制**：
  - 每个子表达式分配**fresh type variable** 作占位；
  - 遍历 AST 收集等式约束（`f x` ⇒ `α = β→γ`）；
  - **Robinson 统一**求解最一般合一子（MGU）；
  - **let-polymorphism**：仅在 `let`/顶层 `def` 处做 generalize（量化自由变量），λ 形参保持单态 → 保证可判定；
  - **occurs-check**：统一 `α = [α]` 时检测到 α 出现在自身内，拒绝无限类型。**这正是"类型无限展开"的根本解法**。
- **可落地点**：lz 的 `def` 顶层绑定天然对应 let-generalization；occurs-check 直接集成进 `ena` 的统一回调。
- **优先级**：P0（推断算法的理论基石）。

### 方案 2.2 — Rust 下一代 trait 求解器：provisional cache / coinduction【P1】
- **来源**：rustc next-gen trait solver（lwn.net/Articles/1063124）、chalk coinduction
- **机制**：求解递归 trait 时，**先假设目标为"暂定真"并写入缓存**，继续沿链条推导；若最终仅剩对暂定条目的义务，则升级为"真"（tying the knot）；否则作废该分支。配合**递归/溢出深度上限**（超出报 `overflow evaluating requirement X: Trait`）。
- **可落地点**：lz 处理递归类型（如链表 `Link = Option<Box<Link>>`）的 trait 推导时，用同一"暂定缓存 + 深度上限"模式**终止递归推断**。
- **优先级**：P1（直接回答"递归推断终止条件"）。

### 方案 2.3 — Zig `comptime` + `@typeInfo` + `@setEvalBranchQuota`【P1】
- **来源**：Zig 语言
- **机制**：
  - `comptime` 把任意代码在编译期执行，**类型是第一等公民**（`comptime T: type`）→ 泛型即语言内建，无独立模板语法；
  - `@typeInfo(T)` 返回描述类型结构的 tagged union（字段/参数/方法），**编译期反射零运行时开销**；
  - `@setEvalBranchQuota(N)` 默认 1000 反向分支上限，**捕获 runaway 循环**防止无限展开。
- **可落地点**：**lz 已预留 `comptime` token！** 直接复用：把 `comptime` 块编译为在转译期求值的 Rust `const`/宏逻辑；`reflect(x)`/类型自判断编译为 `@typeInfo` 式的内建，输出 `size_of`/`align_of`/字段表。深度上限照搬 `@setEvalBranchQuota`。
- **优先级**：P1（类型自判断机制的最佳范本，且与 lz 现有 token 对齐）。

### 方案 2.4 — TypeScript `infer` + 递归条件类型深度上限【P1】
- **来源**：TypeScript 类型系统
- **机制**：
  - `infer` 关键字在条件类型内做**类型级模式匹配**（`T extends (...a:any[])=>infer R ? R : never`）；
  - 递归条件类型（如 `Flatten<T extends Array<infer U> ? Flatten<U> : T`）具**默认 50 层深度上限**（TS 5.6+ 可配置 `--recursiveTypeDepth`），超限即报错。
- **可落地点**：lz 可引入 `infer` 式语法做泛型约束推导；递归类型展开**必须带深度预算**，与方案 2.2/2.3 的上限机制统一。
- **优先级**：P1（递归推断终止的工业级范本）。

### 方案 2.5 — Odin `$T` 隐式多态参数 + `typeid`【P2】
- **来源**：Odin 语言
- **机制**：`proc(p: $T)` 中 `$T` 是**隐式类型参数**，调用点自动推导；`typeid` 提供运行时类型标识，配合 `when` 编译期分支做自判断。
- **可落地点**：lz 的泛型可借鉴 `$T` 隐式参数语法（比显式 `<T>` 更轻），减少注解负担；`typeid` 对应 lz 的 `typeof` 自判断原语。
- **优先级**：P2（语法糖层，降低用户标注成本）。

---

## 维度三：类型系统架构参考

### 方案 3.1 — 约束式推断管道（collect → generate → solve → substitute）【P0】
- **来源**：Pottier & Rémy, *The Essence of ML Type Inference*（约束式推断的标准分解）
- **机制**：把推断拆为独立两遍——(1) **约束生成**：遍历 AST 产出类型等式集合；(2) **约束求解**：统一算法求 MGU。分离使"生成"与"求解"可独立测试、可并行。
- **可落地点**：lz 新增 `typer/` 模块，分 `constraint_gen`（读 AST 写约束）与 `solver`（读约束写 `Subst`）两阶段，最后 `Subst` 回写 AST / 喂给 `codegen`。
- **优先级**：P0（整体骨架）。

### 方案 3.2 — 类型变量表示：fresh named vars + union-find（非 de Bruijn）【P0】
- **来源**：约束式推断（de Bruijn 用于 term 绑定作用域，推断变量用命名 fresh var 更实用）+ `ena`
- **机制**：
  - **de Bruijn 索引**适合 λ 项的作用域（消除命名/α-等价/变量捕获问题），但**不适合类型推断变量**；
  - 推断变量用 `TyVar(i)` 命名 + `ena::unify::UnificationTable` 存等价类，更直观、易调试、易并行。
- **可落地点**：lz 的 `Type::Var(usize)` + 全局 `UnificationTable`；de Bruijn 仅在未来做"带作用域的 term 表示"时考虑。
- **优先级**：P0（表示层基石）。

### 方案 3.3 — Robinson 统一算法 + occurs-check【P0】
- **来源**：Robinson 统一；McBride *First-Order Unification by Structural Recursion*
- **机制**：
  - 变量遇类型 → 绑定（带 occurs-check 防 `α=[α]`）；
  - 原子类型 → 必须精确匹配；
  - 复合类型 → 递归统一对应分量；
  - 失败 → 拒绝程序（类型错误）。
- **可落地点**：直接实现为 `solver::unify(a, b)`，occurs-check 回调挂到 `ena` 的 `UnifyKey` impl。
- **优先级**：P0（求解器核心）。

### 方案 3.4 — 暂定缓存 / SLG tabling 处理递归查询【P1】
- **来源**：rustc next-gen solver / chalk SLG（tabling）
- **机制**：对递归/循环查询做**记忆化 + 暂定真**标记，避免重复展开与无限递归；tabling 让同一 goal 只解一次。
- **可落地点**：lz 的递归类型（链表/树）与 magic trait 推导共享一个 `SolverCache`，键为 `(goal, 已消解约束)`，防止重复工作与无限展开。
- **优先级**：P1（递归推断的性能与终止保障）。

### 方案 3.5 — 诊断链路：约束失败 → 源码 span → "expected vs found"【P2】
- **来源**：rustc 错误报告结构
- **机制**：统一失败时保留**约束来源 span**，报错格式为"期望 T，实得 U"，并指向最内层冲突位置。
- **可落地点**：lz 的每个 `Constraint` 携带 `Span`；求解器产出 `TypeError { span, expected, found }`，由现有诊断系统渲染。
- **优先级**：P2（用户体验，推断可用后再补）。

---

## 4. lz 落地架构草案（把上述方案拼起来）

```
src/
├── types/def.rs          # 新增 Type::Var(InferVar) + Type::Hole
├── typer/                # ← 新增模块
│   ├── ctx.rs            # InferCtxt: ena::UnifyTable<InferVar, Type> + 作用域栈
│   ├── constraints.rs    # Constraint 枚举 + Span
│   ├── gen.rs            # AST → 约束集合（模仿方案 3.1）
│   ├── solver.rs         # Robinson 统一 + occurs-check（方案 3.3）+ 暂定缓存（方案 3.4）
│   └── subst.rs          # Subst 回写 AST / 喂 codegen
├── codegen/mod.rs        # map_type 增加 Type::Var 处理（解出的具体类型）
└── magic/                # 与 solver 共享"约束→候选→合并"思路（方案 1.5）
```

**关键设计决策**
1. **推断与转译解耦**：推断在 `typer/` 跑完，解出完整 `Type` 后再交给现有 `codegen`，不动已稳定的转译逻辑。
2. **递归终止三保险**：occurs-check（方案 3.3）+ 深度预算 `@setEvalBranchQuota` 式上限（方案 2.3）+ 暂定缓存（方案 3.4）。
3. **类型自判断双轨**：编译期用 `comptime` 块（复用已预留 token）+ `@typeInfo` 式内建（方案 2.3）；运行时用 `Any`/`TypeId`/`typetag` 注册表（方案 1.2/1.3）。
4. **占位关键字**：引入 `auto` 或 `_` 作类型占位，触发 `Type::Hole` → 交给推断。

**优先级路线图**
```
P0（基石，本轮必做）      P1（递归/自判断）        P2/P3（增强）
──────────────────      ──────────────────       ──────────────
1.1 ena 统一表            2.2 暂定缓存+深度上限     1.2 Any/TypeId
3.1 约束管道              2.3 comptime+@typeInfo    1.3 typetag 注册表
3.2 TyVar 表示            2.4 infer+深度上限         1.4 im 持久环境
3.3 统一+occurs-check     2.1 HM let-generalize      1.5 chalk 思路
2.1 HM Algorithm W        3.4 SLG tabling            3.5 诊断链路
```

---

## 5. 来源索引

| 来源 | 主题 | 用途 |
|------|------|------|
| `rust-lang/chalk` | Prolog-like trait 求解 | 1.5 / 3.4 |
| lwn.net/Articles/1063124 | Rust next-gen trait solver | 2.2 |
| learningzig.org / ziglang.cc | Zig comptime / @typeInfo | 2.3 |
| mimo/dev.to/blog.openreplay | TypeScript infer / 递归深度 | 2.4 |
| cstopics / handwiki / stormlightlabs | Hindley-Milner / Algorithm W | 2.1 |
| andreasbel/constraint-based-type-inference | de Bruijn 约束式推断 | 3.2 |
| Pottier & Rémy, *Essence of ML Type Inference* | 约束式管道 | 3.1 |
| Robinson / McBride | 统一算法 + occurs-check | 3.3 |
| Odin docs | `$T` 隐式参数 / typeid | 2.5 |
| `ena` crate | union-find 统一表 | 1.1 |
| `typetag`/`erased-serde` | 类型注册表 + 擦除 trait | 1.3 |

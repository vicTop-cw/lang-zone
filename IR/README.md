# LZIR — Lang-Zone 中间表示（Intermediate Representation）

> LZ 编译器的**唯一共享中间层**：前端只跑一次，产出与后端无关的 **LZIR**；`lzrsc` / `lzcyc` / 未来后端只做 *IR → 目标源码* 的降低（lowering）。目的是**消灭双后端（乃至多后端）对前端的重复实现**。

状态图例：✅ 已落地 · 🟡 规范拟定 · ⚪ 规划中。

---

## 一、为什么必须做 IR（问题陈述）

当前仓库里，`lang-zone/src`（参考编译器 `lzc`）与两个后端子编译器各自**完整复制了一遍前端**：

| 前端 pass | `lang-zone/src` | `CY/` (`lzcyc`) | `RUST/` (`lzrsc`) |
|---|:--:|:--:|:--:|
| lexer（词法） | ✅ | ✅ | ✅ |
| parser（语法） | ✅ | ✅ | ✅ |
| ast | ✅ | ✅ | — |
| scope（作用域） | ✅ | ✅ | — |
| semantic（语义） | ✅ | ✅ | — |
| typer / type_checker | ✅ | ✅ | ✅ |
| typing（bounds/traits/relate/variance） | ✅ | ✅ | — |
| hints（unify 推理） | ✅ | ✅ | — |
| macros（展开/解释） | ✅ | ✅ | — |
| magic（魔法方法引擎） | ✅ | ✅ | — |
| **仅有后端不同** | `codegen` | `codegen_cython` | `gen` |

**结论**：除最后的"发射器"外，**约 10 个前端模块被复制了 2~3 遍**。每加一个后端、每改一处语法/类型规则，就要在 N 份代码里同步改——这正是用户说的"双后端很多重复性工作，不做 IR 会越来越复杂"。

IR 把这道题从 **N 前端 × M 后端 = N×M** 降到 **N + M**：

```
          ┌──────────── 前端（只实现一次，在 lang-zone/src）────────────┐
.lz ─▶ lexer▶parser▶ast▶scope▶semantic▶typer▶typing▶macros▶magic ─▶ 【LZIR】
                                                                       │
                          ┌────────────────────────────────────────────┴───────────┐
                          ▼                                                    ▼
                   lzrsc 降低器                                    lzcyc 降低器            （未来 Mojo/Python…）
              LZIR ─▶ Rust 源码                               LZIR ─▶ Cython 源码
```

---

## 二、LZIR 在架构中的位置

```
源代码 (.lz)
   │
   ▼
[ 前端 passes ]   ← 全仓库唯一实现，位于 lang-zone/src
   │                 lexer / parser / ast / scope / semantic
   │                 / typer / typing / hints / macros / magic
   ▼
═════════════════ LZIR（与后端无关、强类型）═════════════════   ← 本文件夹定义
   │
   ├─▶ lzrsc  (RUST/)   : LZIR ─▶ .rs
   ├─▶ lzcyc  (CY/)     : LZIR ─▶ .pyx
   └─▶ 未来后端         : LZIR ─▶ 任意目标

每个后端**只实现"IR → 目标源码"**，不得重新词法/语法/类型分析。
```

**命名约定（沿用项目惯例）**：
- `lzc` — 主编译器（`lang-zone/src`），负责前端 + 产出 LZIR。
- `lzrsc` — 未来 `lzc` 的子编译器（`RUST/`），消费 LZIR 发射 Rust；稳定后自举。
- `lzcyc` — Cython 后端（`CY/`），消费 LZIR 发射 Cython。
- `LZIR` — 本规范定义的中间表示（语言级，与任何后端无关）。

---

## 三、两层 IR 设计（HIR / LIR）

为兼顾"前端产出简单"和"后端消费方便"，LZIR 分两层。**主契约是 HIR**；LIR 为可选的进一步规范化。

### LZIR-H（High-level IR / 高层 IR）✅ 主契约
- **形态**：强类型、**树 / ANF 风格**节点（不是字节码、不是 SSA）。
- **保留 LZ 语义构造**：`struct` / `enum` / `trait` / `impl`、魔法方法调用（`__getitem__` / `__call__` / `__iter__` / `__next__` / `__str__` / `__eq__`…）、内建枚举 `Option`/`Result`、`@intrinsics`（`@memoize`/`@parallel`/…）。
- **构建块已脱糖**：`=:`/`^:`/`~:`/`*:` 在进入 HIR 前就降为显式调用/绑定（见 `IR/design.md` §构建块映射），后端无需理解构建块语法。
- **携带完整类型**：每个 `Expr` 节点带 `Type` 字段（来自前端 typer），后端可直接映射目标类型，无需再推断。
- **为什么是树/ANF 而非 SSA/字节码**：消费者是**源码发射器**而非字节码 VM；高层类型化 IR 与目标语言（Rust/Cython）几乎 1:1 对应，转译最简单。SSA/字节码的"构造+重建"成本在转译场景下纯属浪费——目标编译器自己会优化。

### LZIR-L（Low-level IR / 低层 IR）⚪ 规划中
- **形态**：规范化后的 **CFG（基本块）+ 三地址码 / SSA**。
- **用途**：仅当需要在发射前做**语言级优化 pass**（死代码消除、内联、借用检查雏形）时才需要；纯转译到已有优化器的 Rust/Cython 时可跳过。
- **触发条件**：先标记 ⚪；等 HIR 落地、且确有优化需求时再启用，避免过早设计。

---

## 四、后端契约（Backend Contract）🟡

任何后端都必须满足以下规则（这是消除重复的关键红线）：

1. **只消费 LZIR**：后端输入是 `LZIR.Module`，**不得**重新 lexer / parser / typer / typing / magic。
2. **形态是纯函数**：`fn emit(module: LZIR.Module, opts) -> String`（目标源码文本）。
3. **映射表驱动**：后端维护"LZIR 节点 → 目标语法"的映射；不重写语义。
4. **类型已就绪**：直接读节点的 `Type` 字段生成目标类型标注；无推断逻辑。
5. **不碰 `lz.std` 语义**：`lz.std` 内建的"含义"由前端保证，后端只负责把对应调用/类型落到目标（如 `Option`→`Option`，`Box`→`Box`/`box`）。`rust.std` 的映射由 `lzrsc` 通过 Bridge 处理，不在此契约内。

违反上述任一条 = 回到"前端复制"，等于没做 IR。

---

## 五、对 RUST / CY 的迁移路径 🟡

| 阶段 | 动作 | 产出 |
|---|---|---|
| 1 | 冻结 LZIR-H 节点定义（本文件夹 + `IR/design.md`） | 规范 |
| 2 | `lzc` 前端在现有 `ast` 之后新增 `ir` pass，产出 `LZIR.Module` | 前端新增 emit-to-IR |
| 3 | `lzrsc`(`RUST/`)、`lzcyc`(`CY/`) 改为**只保留发射器**，删除其复制的 lexer/parser/typer/typing/hints/macros/magic | 后端瘦身 |
| 4 | 新增后端（Mojo/Python…）**只需写发射器** | N+M 达成 |

> 注意：`RUST/` 当前把 `rust.std` 桥接清单（`modules/*.toml`）读进自己的 `stdbridge`——这属于未来 `rust.std` 层，**不在 LZIR 范畴内**；LZIR 只表达 `lz.std` 语义。详见 `LZSTD/` 的命名空间边界。

---

## 六、待决 / 开放项

- [ ] HIR 节点最终字段表（见 `IR/design.md`，待工程评审）
- [ ] LZIR 序列化格式：文本调试格式 vs 二进制（如 bincode）——🟡 建议先文本（人可读、易测）
- [ ] `lzrsc` / `lzcyc` 何时开始迁移（依赖阶段 2 前端就绪）
- [ ] LIR 是否启用（默认 ⚪，按需）

---

## 附：与 `LZSTD/`、`SYNTAX/` 的关系

| 文档 | 关注点 | 关系 |
|---|---|---|
| `SYNTAX/` | `.lz` **源码语法**（用户怎么写） | 前端输入 |
| `LZSTD/` | `lz.std` **标准库语义**（内建是什么） | 前端语义保证；IR 引用其类型 |
| `IR/`（本） | **中间表示**（编译器内部契约） | 前端产出、后端消费 |

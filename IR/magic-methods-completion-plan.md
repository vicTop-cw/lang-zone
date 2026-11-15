# LZ 魔法方法 —— 补齐与完善详细计划

> 制定日期：2026-09-08
> 依据：`SYNTAX/06d-内置魔法trait和全局函数.md`（规范，速查索引 ~66 个）、`src/magic/engine.rs`（实际注册 33 个）、以及本次的**转译产物实测**
> 状态：待评审

---

## 0. 一句话结论

规范声明 ~66 个魔法方法，`src/magic/engine.rs` 只注册了 **33 个**；更严重的是——**已注册的也大多没接到运算符上**。根因是 `src/ir/codegen/mod.rs:3958` 把"魔法方法 → 生成 Rust trait impl"写成了 **`__eq__` 的单点特例**，其余魔法方法没有对应的 trait impl 生成分支，导致运算符降级时找不到 trait，生成的代码**连 Rust 都编译不过**。

**"联动"（一个魔法方法应自动带动相关行为 / 一组魔法方法配对生效）是本次重灾区。**

---

## 1. 现状盘点（全部为实测证据，非推断）

### 1.1 调查方法（重要）

`lzc` 是**转译器**，不是解释器：

```
$ lzc foo.lz
Generated foo.lz -> foo.rs (IR codegen)
```

`RC=0` **只代表转译成功，不代表语义正确**。因此不能只看编译是否报错，必须**读生成的 Rust 产物**来判定魔法方法是否真的接通了运算符。

### 1.2 语法路径坑（先修文档/或先明确唯一写法）

魔法方法存在**两条写法**，行为完全不同：

| 写法 | 出处 | 实测结果 |
|---|---|---|
| `impl P =` + `def __eq__(...)` | DEMO / 测试实际用法 | ✅ 生成 `impl std::cmp::PartialEq for P`（委托 `__eq__`） |
| `struct P =` 内 `magic __eq__(...)` | `06d` / `06a` 文档示例 | ❌ 只生成普通固有方法 `fn __eq__`，**不生成任何 trait impl**，运算符完全不接通 |

> `SYNTAX/附录B` 把 `magic` 列为"魔法方法声明"关键字，`06d`/`06a` 示例也用 `magic`，但 DEMO 与测试全用 `impl`+`def`。**文档与实现已脱节**。

### 1.3 实测结果

| # | 联动 | 规范依据 | 实测生成的 Rust | 判定 |
|---|---|---|---|---|
| 1 | `__eq__` → `==` / `!=` | 06d §五 | `impl std::cmp::PartialEq for P { fn eq(&self,o){ self.__eq__(o) } }`；`!=` 走 Rust 默认 `ne` = `!eq` | ✅ **联动成立** |
| 2 | `__contains__` ↔ `in` | 06d §八、附录B（`a in b` → `b.__contains__(&a)`） | 只有固有方法 `fn __contains__`；`in` 降级为 `c.contains(&3i64)` | ❌ 断裂（无 `contains` → **E0599**） |
| 3 | `__len__` ↔ `if` 真值 | 06d §十二 判定链 `__bool__ → __len__ → 默认 true` | `if (c).__bool__()` —— 直接调**未定义**的 `__bool__`，**没走 `__len__` 兜底** | ❌ 断裂（**E0599**） |
| 4 | `__iadd__` ↔ `+=` | 06d §四、`12-操作符.md:144` | `a = a + b;`（`+=` 恒脱糖），`__iadd__` 生成了但**从不被调用**；无 `impl Add` | ❌ 断裂（**E0369**） |
| 5 | `__new__` ↔ `__init__` | 06d §九 | 生成 `pub fn __lz_new`，但调用点写 `P::__new__(5)`（名字不匹配）；且体为 `P { x: 0 }`，**忽略参数与用户逻辑** | ❌ 断裂（**E0599** + 语义错） |

**注意 #2/#3/#4/#5 是"生成即编译失败"**——比"功能不支持"更糟：用户按规范写了魔法方法，却拿到编译不过的 Rust，且 `lzc` 自身还报 RC=0（静默产错）。

### 1.4 根因

`src/ir/codegen/mod.rs:3958-4010`：

```rust
// 自动生成 PartialEq：struct 定义 `__eq__` 魔术方法时 ...
// 委托 __eq__ 生成 impl，并携带 __eq__ 的 where 约束（T: Eq）。
let eq_method = i.methods.iter().find(|m| m.name == "__eq__");   // :3971
...
"impl{} std::cmp::PartialEq for {} {} {{",                        // :3990
self.emit_line("self.__eq__(other)");                             // :4003
```

这是**为 `__eq__` 手写的唯一特例**。而 `src/magic/engine.rs` 的注册表里其实**早已存有**每个魔法方法对应的 `trait_path` / `trait_method`：

```rust
("__add__", "std::ops::Add", "add"),      // engine.rs:98
("__lt__",  ...), ("__gt__", ...), ...
```

**即：映射信息齐全，只是 codegen 没有泛化地消费它。** 这是本计划的核心修复点。

---

## 2. 代码落点地图（已定位）

| 关注点 | 位置 | 说明 |
|---|---|---|
| 魔法方法注册表 | `src/magic/engine.rs:98-251` | 33 条 `register(...)`，含 `trait_path`/`trait_method` |
| **魔法 → trait impl 生成** | `src/ir/codegen/mod.rs:3958-4010` | ⚠️ 仅 `__eq__` 特例（**核心修复点**） |
| 真值条件生成 | `src/ir/codegen/mod.rs:10682-10721` | `format!("({}).__bool__()", s)`，无兜底链 |
| 运算符 → 魔法名映射 | `src/ir/builder.rs:419` `BinOp::Eq => Some("__eq__")` | |
| 比较运算符降级 | `src/ir/builder.rs:5535-5600`、`codegen/mod.rs:9693-9730` | 含 owned/ref 参数处理（`!a.__eq__(&c)`） |
| **复合赋值脱糖** | `src/ir/builder.rs:444-457` `map_assign_op` | `AssignOp::AddEq => BinOpKind::Add` —— **恒脱糖，从不查 `__iadd__`** |
| MagicKind 分派（Rust） | `src/ir/codegen/mod.rs:11280-11400` | Call/GetItem/SetItem/Iter/Next/Display/Eq/Cmp/Add/Len/Rev/IterStrategy |
| MagicKind 分派（Cython） | `src/ir/codegen_cython.rs:1012-1027` | 需同步改 |
| IR 魔法调用节点 | `src/ir/node.rs:893-917` `enum MagicKind` | 20 个变体 |
| `in` → `.contains()` 降级点 | **待定位** | 产物为 `c.contains(&x)`；本计划含定位任务 |

---

## 3. 目标 / 非目标

**目标**
1. 消灭"生成即编译失败"的 4 处（#2 #3 #4 #5）。
2. 建立**通用**的"魔法方法 → Rust trait impl"生成机制，替代 `__eq__` 特例，让注册表驱动一切。
3. 补齐规范中"联动"关系，使配对魔法方法按约定自动派生。
4. 统一魔法方法书写路径（文档 ↔ 实现）。

**非目标**
- 不新增规范里没有的魔法方法（如 Python 的 `__radd__` 反射运算符，规范未列，不纳入）。
- 不动 Cython 后端的既有降级正确性（只做同步，不做增强）。
- 不改 `?`/`as`/管道等"编译器内建协议"的既有语义。

---

## 4. 分级任务

### P0 —— 生成即编译失败（必修，优先）

#### P0-1 `__contains__` ↔ `in`
- **现象**：`in` 降级为 `c.contains(&x)`；`__contains__` 只是固有方法，无 trait impl。
- **方案**：
  1. 定位 `in` 的降级点（产物 `c.contains(&x)`），改为：用户类型定义了 `__contains__` 时，生成 `c.__contains__(&x)`（或生成 `Contains` trait 并调用）。
  2. 由 P1-0 的通用机制生成 `Contains` trait impl（规范：`Contains` 自定义 trait）。
- **验收探针**：`impl C = def __contains__(ref self, value: int) -> bool` + `if 3 in c:` → 产物调用 `__contains__`，且 `rustc` 可编译。

#### P0-2 `__len__` ↔ `if` 真值判定链
- **现象**：`codegen/mod.rs:10721` 无条件生成 `(c).__bool__()`。
- **方案**：实现规范 §十二 判定链 —— `__bool__` → `__len__`（`len() != 0`）→ 默认 `true`：
  ```
  条件生成优先级：
  1. 有 __bool__    → (x).__bool__()
  2. 有 __len__     → ((x).__len__() != 0)
  3. 否则（内建类型）→ 保持现有行为
  ```
- **验收**：`impl C = def __len__(ref self) -> int` + `if c:` → 产物为 `((c).__len__() != 0)`，可编译；`C(n:0)` 走 false 分支。

#### P0-3 `__iadd__` ↔ `+=`（及 `__isub__/__imul__/__idiv__`）
- **现象**：`builder.rs:444 map_assign_op` 把 `AddEq` 直接映射为 `BinOpKind::Add` → 恒脱糖为 `a = a + b`。
- **方案**：在 `map_assign_op` / 复合赋值处理处（`builder.rs:4119` 附近区分 `AssignOp::Eq`）增加判定：
  ```
  复合赋值 x op= y：
  1. 类型定义了 __iadd__(mut self, rhs) → 生成 __iadd__ 调用（AddAssign）
  2. 否则 → 保持现有脱糖 x = x + y
  ```
- **验收**：`impl P = def __iadd__(mut self, rhs: P)` + `a += b` → 产物调用 `__iadd__`（而非 `a = a + b`），可编译。

#### P0-4 `__new__` ↔ `__init__` 构造器链
- **现象**：生成 `pub fn __lz_new` 但调用点写 `P::__new__`；且体是 `P { x: 0 }`（忽略参数/用户逻辑，即设计文档 §1.2 描述的"补齐默认字段"占位）。
- **参考设计**：`IR/design-magic-init-priority.md` 第三/四章（Phase 1/2）。
- **方案**：
  1. 统一命名：调用点改为 `P::__lz_new(...)`（或函数名改为 `__new__`），二选一，全仓一致。
  2. `__lz_new` 体改为**透传用户 `__new__` 的函数体与参数**（不再生成 `P { x: 0 }` 占位）。
  3. 若定义了 `__init__`，在构造后插入 `__lz_init(...)` 调用（构造链 `__new__` → `__init__`）。
- **验收**：`magic/impl` 定义 `__new__(a: int) -> P = P(x: a)` + `P(5)` → 产物 `P::__lz_new(5i64)` 且返回 `x: 5`。

---

### P1 —— 通用机制（治本，P0 的前置/并行）

#### P1-0 将 `__eq__` 特例泛化为「注册表驱动的 trait impl 生成」★核心
- **现状**：`codegen/mod.rs:3958-4010` 手写 `__eq__` → `PartialEq`。
- **方案**：
  1. 新增统一的 `emit_magic_trait_impls(struct_def)`，遍历 `src/magic/engine.rs` 注册表，为每个该 struct 实现的魔法方法生成对应 trait impl（`trait_path` + `trait_method`）。
  2. 保留 `__eq__` 现有的 `where` 约束传递能力（`T: Eq`），泛化到所有魔法方法。
  3. 覆盖映射（取自 engine.rs 现有注册）：
     - 算术：`__add__→Add`、`__sub__→Sub`、`__mul__→Mul`、`__div__→Div`、`__rem__→Rem`、`__pow__→Pow(自定义)`
     - 位：`__bitand__/__bitor__/__bitxor__/__shl__/__shr__` → `BitAnd/BitOr/BitXor/Shl/Shr`
     - 一元：`__neg__→Neg`、`__not__→Not`
     - 比较：`__lt__/__le__/__gt__/__ge__→PartialOrd(partial_cmp)`、`__cmp__→Ord`、`__hash__→Hash`
     - 显示：`__str__→Display`、`__repr__→Debug`
     - 容器：`__getitem__→Index`、`__setitem__→IndexMut`、`__len__→HasLen`、`__contains__→Contains`
     - 迭代：`__iter__/__into_iter__→IntoIterator`、`__next__→Iterator`
     - 转换：`__from__→From`、`__into__→Into`、`__try_from__→TryFrom`、`__try_into__→TryInto`
     - 生命周期：`__drop__→Drop`、`__clone__→Clone`、`__default__→Default`
  4. 删除 `__eq__` 特例代码，改走通用路径（回归用现有 `__eq__` 探针守护）。
- **收益**：一次性接通上表全部运算符联动，`__lt__→>`、`__add__→+` 等自动成立。
- **验收**：对上表每一条写一个最小 struct + 运算符探针，检查产物含对应 `impl ... for ...`，且 `rustc` 可编译。

#### P1-1 补齐未注册的魔法方法（规范有、engine 无）

按规范 `06d` 逐条补 `engine.rs` 注册 + P1-0 自动生成 trait：

| 分类 | 待补魔法方法 | 备注 |
|---|---|---|
| 复合赋值 | `__iadd__` `__isub__` `__imul__` `__idiv__` | 规范 §四 → `AddAssign` 等 |
| 一元 | `__invert__`（`~a`）`__deref__`（`*a`） | 规范 §三 |
| 转换 | `__cast__` `__try_cast__` | 规范 §六，`as` 分发链 `__cast__`→`__try_cast__`→报错 |
| 构造 | `__new__` `__init__` | 见 P0-4 |
| 布尔/数学 | `__bool__` `__abs__` | `__bool__` 为判定链首环（P0-2） |
| 容器 | `__len__` `__contains__` | 见 P0-1/P0-2，需 `HasLen`/`Contains` 自定义 trait |
| 生命周期 | — | 已注册 |
| 隐式策略 | `__implicit_from__` `__implicit_to__` `__implicit_copy__` `__implicit_default__` `__when_move__`（移动前后钩子，见 `SYNTAX/12-操作符.md` §1.14.1） | 见 P1-2 |
| 守卫策略 | `__guarded_pred__` `__guarded_action__` | 配对联动，见 P1-3 |
| 类型缺口 | `__int__` `__float__` `__pos__` | `__int__/__float__` 需**自动生成** `impl From<SelfTy> for i64/f64` |
| 上下文 | `__enter__` `__exit__` | `with` 语句，见 P1-4 |

#### P1-2 隐式转换体系（联动最密集，独立子项目）
- **依据**：`IR/design-magic-init-priority.md` 第五章（Phase 3-6）、`06d` §十四。
- **内容**：
  1. 注册 `__implicit_from__` / `__implicit_to__`。
  2. `__implicit_from__` → **blanket impl 自动派生 `__implicit_to__`**（类 Rust `From→Into`）。
  3. 实现 5 级优先级链：`__implicit_from__` → `__implicit_to__` → `__from__` → `__into__` → `__default__`。
  4. 触发点：`let` 赋值、函数实参、返回值、struct 字段默认值。
  5. 约束：单次转换（禁 `A→B→C` 链）、循环转换检测、`as`/`.into()` 不参与。
- **依赖**：P1-0（trait 生成）、P0-4（构造器链，`__default__` 兜底）。

#### P1-3 守卫策略配对
`__guarded_pred__` + `__guarded_action__` → 配对生成 `GuardedStrategy` impl（规范 §十五）。

#### P1-4 上下文管理器
`__enter__` + `__exit__` → `with` 语句（规范 §十七）。注意 LZ 语义与 Python 不同：`__exit__` 接收 guard 值而非 `self`，且不接收异常三元组。

---

### P2 —— 待验证项（先测再定是否修）

以下**尚未实测**，需先按 §6 方法补探针确认状态，再决定是否纳入 P0/P1：

| 联动 | 规范要求 |
|---|---|
| `__rev__` + `__next__` → 自动生成 `DoubleEndedIterator` | 06d §八"共存时自动生成" |
| `__iter__` ↔ `__next__` ↔ `__iter_strategy__` | 06d §八/§十八，`__iter_resolve` 取首个适用策略 |
| `__getitem__` ↔ `__setitem__` | 06d §十一，索引读/写配对 |
| `__str__` ↔ `__repr__` | 06d §七，Display/Debug |
| `__from__` ↔ `__into__` | Rust `From→Into` blanket 是否自动派生 |
| `__cmp__` ↔ 全部比较运算符 | 实现 `__cmp__` 是否自动支撑 `<` `>` 等 |
| `__is_ok__`/`__unwrap__` ↔ `__err__` | `?` 可传播三件套（`12-操作符.md:223` 称其为"内建协议，非魔法方法"——需确认归属） |
| `__call__` | 06d §十一，`obj(args)` 可调用 |
| `__unapply__` | 06d §九点五；`IR/frontend-gap-plan.md` 标为"✅ 已实现"，需回归验证 |

### P3 —— 文档与语法统一

1. **统一魔法方法书写路径**：明确 `impl X =` + `def __xxx__` 为唯一支持写法，修正 `06d`/`06a` 中的 `magic __xxx__` 示例；或反之补齐 `magic` 路径使其同样生成 trait impl（二选一，建议前者，成本低）。
2. 在 `06d` 为每个魔法方法标注**实现状态**（✅/🔸/❌），与代码同步。
3. `06d` 与 `12-操作符.md` 交叉校验：两处的"操作符→魔法方法映射表"需一致（`12:144` 说 `+=` 有独立魔法方法，`12:523` 说其余复合赋值脱糖，需与实现对齐）。
4. 补充"联动矩阵"章节：明确哪些魔法方法会自动派生其他行为。

---

## 5. 通用机制设计（P1-0 详述）

```
                    ┌──────────────────────────────┐
   struct 定义      │ src/magic/engine.rs          │
   __add__ ────────▶│ 注册表: 33+ 条                │
   __len__          │ ("__add__","std::ops::Add",   │
   __contains__     │  "add")                       │
                    └──────────────┬───────────────┘
                                   │ 消费（现状：只有 __eq__ 被消费）
                                   ▼
                    ┌──────────────────────────────┐
                    │ codegen: emit_magic_trait_impls│
                    │  (新增，替代 :3958 特例)        │
                    └──────────────┬───────────────┘
                                   ▼
      impl std::ops::Add for P { fn add(...) { self.__add__(...) } }
      impl HasLen for C     { fn len(&self) { self.__len__() } }
      impl Contains for C   { fn contains(...) { self.__contains__(...) } }
```

**实现要点**
- 放在 `src/ir/codegen/mod.rs`，紧邻现有 `:3958` 位置，便于复用 `where` 约束传递逻辑。
- 自带 trait（`HasLen`/`Contains`/`Pow`/`Cast`/`Callable`/`HasBool`/`HasAbs`/`ImplicitFrom`…）需在 prelude（`lz_builtins` 或生成头部）中定义，避免 E0412。
- 需处理 self 模式差异（`self` / `ref self` / `mut self`）与参数 owned/ref（`__eq__` 已有此逻辑，见 `codegen/mod.rs:8742-8756`）。
- Cython 后端（`codegen_cython.rs:1012-1027`）需同步：Python 侧多为内建语法（`__getitem__`→`[]`、`__len__`→`len()`），新增自定义 trait 类魔法方法需决定降级策略（或显式不支持）。

---

## 6. 验收方法（必须遵守）

因为 `lzc` 只转译、不运行，验收必须**两步**：

**第 1 步：检查生成的 Rust**
```powershell
cargo run -q -p lang-zone -- probe.lz
# 读 probe.rs，确认：
#   - 生成了预期的 `impl <Trait> for <Type>`
#   - 运算符处调用的是魔法方法（而非裸运算符/不存在的方法）
```

**第 2 步：确认产物能真编译**
```powershell
rustc --edition 2021 probe.rs ...   # 或纳入 tests/ 用 trybuild/临时 crate 编译
```
> 建议：新增一类"魔法方法产物编译测试"，把探针生成的 `.rs` 喂给 `rustc`，把"生成即编译失败"变成 CI 可捕获的红灯。

**探针模板**（`impl` 路径，DEMO 写法）：
```lz
struct P =
    v: int

impl P =
    def __iadd__(mut self, rhs: P) =
        self.v = self.v + rhs.v

def main() =
    a = P(v: 1)
    b = P(v: 2)
    a += b
    print(a.v)
```

**回归守护**：P1-0 改动影响面大，需保证现有 DEMO 编译不劣化（`__eq__` 相关有 `box.lz`/`magic_methods.lz` 等依赖，见代码注释中的引用）。

---

## 7. 实施顺序（依赖排序）

```
Phase 0（治标，快速止血）
  P0-1 __contains__ ↔ in
  P0-2 __len__ ↔ if 判定链
  P0-3 __iadd__ ↔ +=
  P0-4 __new__/__init__ 命名 + 体透传
  └─ 每个都配 1 个探针 + 产物编译检查

Phase 1（治本）
  P1-0 泛化 trait impl 生成（替代 __eq__ 特例）★
        └─ 依赖：Phase 0 的探针作为回归基线
  P1-1 补齐 engine.rs 注册（复合赋值/一元/转换/布尔…）

Phase 2（体系）
  P1-2 隐式转换体系（blanket + 优先级链）
  P1-3 守卫策略配对
  P1-4 上下文管理器 __enter__/__exit__

Phase 3（收敛）
  P2 待验证项补探针 → 按结果转入 Phase 0/1
  P3 文档与语法统一
```

**建议**：Phase 0 与 P1-0 可并行推进；若先做 P1-0，则 P0-1/P0-2/P0-3 中"生成 trait impl"的部分被自动覆盖，只需再修**降级/调用点**（`in`、`if` 判定链、`+=` 脱糖、`__new__` 调用名）。**更推荐先做 P1-0，再一次性收尾 P0 的调用点。**

---

## 8. 风险与注意事项

1. **P1-0 影响面大**：现有 `__eq__` 的 `where T: Eq` 约束传递、`Box<T>` 特例（`codegen/mod.rs:9002-9030`）不能被破坏 → 保留现有探针回归。
2. **自定义 trait 的可见性**：`HasLen`/`Contains`/`Pow`/`Cast` 等自定义 trait 需在生成的 prelude 中定义，注意与 `lz_builtins` 已有定义冲突（命名重复会导致 E0255）。
3. **owned vs ref 参数**：`__eq__` 已有 owned/ref 两种签名分支（`builder.rs:9725-9730`），泛化时每种魔法方法都要正确处理，否则大面积 E0308。
4. **Cython 后端不能落后**：`codegen_cython.rs` 若不同步，会造成 Rust 后端可用、Python 后端不可用的分裂。
5. **静默产错**：当前 `lzc` 对"魔法方法未被接通"报 RC=0。建议在 P1-0 后，对"定义了魔法方法但无对应 trait/调用点"的情况加 warning，避免再次静默。
6. **隐式转换（P1-2）风险最高**：涉及类型系统，易引入歧义；务必遵守"单步转换 + 循环检测"约束。

---

## 9. 未决项（需评审拍板）

1. `magic __xxx__`（struct 内）与 `impl` + `def __xxx__` 是否**都**要支持？还是废弃 `magic` 路径？
2. `__is_ok__`/`__unwrap__`/`__err__` 归属：`12-操作符.md:223` 称其为"编译器内建协议，非魔法方法"，但 `06d` §十一 列入魔法方法表 —— 以谁为准？
3. 自定义 trait（`HasLen`/`Contains`/`Pow`/`Cast`/`HasBool`/`HasAbs`）是放进 `lz_builtins` 还是每次生成到产物头部？
4. 是否引入反射运算符（`__radd__` 等）？规范未列，建议**暂不引入**。
5. P2 待测项（§P2 表）需先补探针确认现状，再排优先级。

---

## 附：实测速查（本次产出的证据）

| 魔法方法 | 生成 trait impl | 运算符是否接通 |
|---|:---:|---|
| `__eq__` | ✅ `PartialEq` | ✅ `==` `!=` |
| `__lt__` | ❌（仅固有方法） | 未测（`>` 预期断） |
| `__len__` | ❌ | ❌ `if` → `__bool__()`（不存在） |
| `__contains__` | ❌ | ❌ `in` → `.contains()`（不存在） |
| `__iadd__` | ❌ | ❌ `+=` → `a = a + b`（缺 `Add`） |
| `__new__` | 🔸 `__lz_new` | ❌ 调用名不匹配 + 体为占位 |


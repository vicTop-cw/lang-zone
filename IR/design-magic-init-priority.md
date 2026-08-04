# 魔法方法补齐 + 隐式转换体系 设计方案

> 日期：2026-08-04 | 状态：设计阶段 | 版本：v2

---

## 零、核心概念：三种转换方式对比

在进入具体设计之前，必须明确 LZ 语言中三种转换方式的语义区别：

```
┌─────────────────────────────────────────────────────────────────┐
│  方式              写法              语义          谁触发       │
├─────────────────────────────────────────────────────────────────┤
│  cast (显式)       expr as Type      强制类型转换   用户手写     │
│  explicit (显式)   T::from(x)        显式转换调用   用户手写     │
│                    x.into()                                      │
│  implicit (隐式)   自动插入          自动类型转换   编译器插入   │
└─────────────────────────────────────────────────────────────────┘
```

**关键规则**：
- `as` 是纯语法层面的强制转换（类似 Rust `as`），不调用任何 trait 方法
- `__from__` / `__into__` 是**显式**转换 trait，仅当用户手写 `.into()` 或 `T::from()` 时调用
- `__implicit_from__` / `__implicit_to__` 是**隐式**转换 trait，编译器在类型不匹配时自动插入

**禁止行为**：编译器绝不自动将 `__implicit_from__`/`__implicit_to__` 展开为 `as` 语句。

---

## 一、背景与现状

### 1.1 当前实现状态

| 魔法方法 | MagicEngine 注册 | Codegen 实现 | 语义类别 |
|----------|:---:|:---:|------|
| `__new__` | ❌ 未注册 | 🔸 部分 | 构造器 |
| `__init__` | ❌ 未注册 | ❌ 无 | 后初始化 |
| `__from__` | ✅ `MagicKind::From` | ✅ | 显式转换 |
| `__into__` | ✅ `MagicKind::Into` | ✅ | 显式转换 |
| `__default__` | ✅ `MagicKind::Default` | ✅ | 默认值 |
| `__try_from__` | ✅ | ✅ | 显式可失败转换 |
| `__try_into__` | ✅ | ✅ | 显式可失败转换 |
| `__implicit_from__` | ❌ 不存在 | ❌ 无 | 隐式转换（本设计新增） |
| `__implicit_to__` | ❌ 不存在 | ❌ 无 | 隐式转换（本设计新增） |

### 1.2 `__new__` 当前行为的问题

```rust
// LZ 代码
let p = Point(x: 3.0)  // 只有 x，y 未提供

// 当前 codegen 输出（struct_has_new == true 时）
Point { x: 3.0, y: 0.0 }  // 自动补齐 y: 0.0（不是真正的构造器调用！）
```

**问题**：
1. 不是真正的构造器函数调用，无法执行用户自定义逻辑
2. 无法处理构造器参数与字段名不同的情况（如 `Point(r, theta)` 转笛卡尔坐标）

---

## 二、设计目标

1. **补齐 `__new__`**：真正的构造器调用
2. **补齐 `__init__`**：后初始化钩子
3. **隐式转换体系**：`__implicit_from__` + `__implicit_to__` 配对，编译器自动插入
4. **初始化优先级链**：`__implicit_from__` → `__implicit_to__` → `__default__`
5. **返回值隐式转换**：函数返回类型与声明类型不匹配时自动转换

---

## 三、`__new__` 完整实现

### 3.1 IR 层设计

在 `src/ir/node.rs` 中，`StructDef` 已有字段：

```rust
pub struct StructDef {
    pub has_new: bool,        // 是否定义了 __new__（已存在）
    // ... 其他字段
}
```

需要新增字段记录 `__new__` 的签名信息：

```rust
pub struct StructDef {
    pub has_new: bool,
    pub new_params: Vec<(String, IrType)>,  // __new__ 的参数列表
    pub new_ret_ty: IrType,                 // 返回类型（应为 Self）
    pub has_init: bool,                     // 是否定义了 __init__
    pub init_params: Vec<(String, IrType)>, // __init__ 的参数列表
    // ... 其他字段
}
```

### 3.2 Codegen 层设计

#### 3.2.1 struct 构造表达式的 codegen 降级

当前 `gen_expr` 中 `ExprKind::Call` 处理 struct 构造的逻辑（`codegen.rs` L2095-2128）需要改为：

```
┌─ 构造表达式 Name(args...) ─┐
│                              │
│  struct 有 __new__ ? ──是──▶ 调用 __new__(args) 函数
│       │                      │
│       否                      │
│       ▼                      │
│  关键字构造 { field: val }   │
│  （保持现有行为）             │
└──────────────────────────────┘
```

#### 3.2.2 Rust 降级示例

```lz
// LZ 代码
struct Point =
    x: f64
    y: f64

    magic __new__(x: f64, y: f64) -> Self =
        Self(x: x, y: y)

let p = Point(x: 3.0, y: 4.0)
```

降级后的 Rust 代码：

```rust
struct Point { x: f64, y: f64, __lz_new_used: bool }

impl Point {
    fn __lz_new(x: f64, y: f64) -> Self {
        Point { x, y, __lz_new_used: true }
    }
}

// 构造调用
let p = Point::__lz_new(3.0, 4.0);
```

**关键设计决策**：`__lz_new_used` 是一个编译时标记字段（零大小），确保当 struct 有 `__new__` 时，用户无法绕过它直接构造 `Point { x, y }`。

**替代方案**：不添加标记字段，而是在 IR 层面将所有 `Point(x: 3.0, y: 4.0)` 构造调用重写为 `Point::__lz_new(3.0, 4.0)` 函数调用。这需要在 IR builder 的 `convert_expr` 中识别 struct 构造并查询 `struct_def.has_new`。

**推荐方案**：IR 层面重写。修改 `builder.rs` 中 `AstExpr::Call` 分支，当 callee 是 struct 名且该 struct 有 `has_new` 时，将调用改写为对 `__lz_new` 函数的调用。

### 3.3 实现步骤（`__new__`）

| 步骤 | 文件 | 改动 |
|------|------|------|
| 1 | `src/ir/node.rs` | `StructDef` 新增 `new_params`, `new_ret_ty` |
| 2 | `src/ir/builder.rs` | `convert_struct()` 提取 `__new__` 方法签名存入 StructDef |
| 3 | `src/ir/builder.rs` | `convert_expr()` Call 分支检测 struct + has_new → 改写为调用 `__lz_new` |
| 4 | `src/ir/codegen.rs` | `gen_struct_def()` 生成 `__lz_new` 关联函数 |
| 5 | `src/ir/codegen.rs` | 移除旧的"补齐默认字段"逻辑（`struct_has_new` + `default_value_for`） |

---

## 四、`__init__` 实现

### 4.1 设计

`__init__` 在 `__new__` 返回后自动调用，用于额外初始化：

```
构造调用 Point(x: 3.0, y: 4.0)
    │
    ├─ 有 __new__ ?
    │   ├─ 是 → tmp = __new__(3.0, 4.0)
    │   └─ 否 → tmp = Point { x: 3.0, y: 4.0 }
    │
    ├─ 有 __init__ ?
    │   └─ 是 → tmp.__init__(args...)  // 自身初始化
    │
    └─ 返回 tmp
```

### 4.2 Rust 降级

```rust
// 仅 __init__（无 __new__）
let tmp = Point { x: 3.0, y: 4.0 };
tmp.__lz_init(/* init 的参数 */);
// → 返回 tmp

// __new__ + __init__
let tmp = Point::__lz_new(3.0, 4.0);
tmp.__lz_init(3.0, 4.0);
// → 返回 tmp
```

### 4.3 实现步骤

| 步骤 | 文件 | 改动 |
|------|------|------|
| 1 | `src/ir/node.rs` | `StructDef` 新增 `has_init`, `init_params` |
| 2 | `src/ir/builder.rs` | `convert_struct()` 提取 `__init__` 签名 |
| 3 | `src/ir/codegen.rs` | `gen_expr()` 构造调用后自动插入 `__lz_init` 调用 |
| 4 | `src/ir/codegen.rs` | `gen_struct_def()` 生成 `__lz_init` 方法 |

---

## 五、隐式转换体系：`__implicit_from__` + `__implicit_to__`

### 5.1 为什么需要独立的隐式转换 trait

| | `__from__` / `__into__` | `__implicit_from__` / `__implicit_to__` |
|---|---|---|
| 触发方式 | 用户显式调用 `.into()` / `T::from()` | 编译器自动插入 |
| 典型场景 | 边界处的手动转换 | let 赋值、参数传递、返回值 |
| 安全性 | 用户明确意图 | 需确保无歧义 |
| 降级 | Rust 标准 `From`/`Into` trait | Rust 自定义 trait |

**关键**：`__implicit_from__` 和 `__implicit_to__` 是**配对**的。实现其中一个是够的，因为 blanket impl 自动提供另一个（类似 Rust `From<T> → Into<U>`）。

### 5.2 语法定义

```lz
// __implicit_from__：从其他类型隐式转换为 Self
// 签名约定：__implicit_from__(source: S) -> Self
magic __implicit_from__(value: str) -> Self =
    Self(int: value.parse().unwrap_or(0))

// __implicit_to__：从 Self 隐式转换为其他类型
// 签名约定：__implicit_to__(self) -> R  
magic __implicit_to__(ref self) -> str =
    self.to_string()
```

**规则**：
1. 如果 struct 实现了 `__implicit_from__`，blanket impl 自动提供对应的 `__implicit_to__`
2. `__implicit_from__` 和 `__implicit_to__` **不能同时手动实现**（避免歧义，类似 Rust 孤儿规则）
3. 每次隐式转换最多执行**一步**（禁止链式自动转换 `A→B→C`）

### 5.3 触发场景

#### 场景 1：let 语句类型不匹配

```lz
s = "1"
x: int = s       // str → int，编译器查找 __implicit_from__<int>(str) 或 __implicit_to__(self: str) -> int
```

编译器行为：
1. 推断 `s` 的类型为 `str`
2. 发现目标类型 `int` 与源类型 `str` 不匹配
3. 查找 `int` 的 `__implicit_from__(str)` 实现
4. 如果存在，自动插入 `int::__implicit_from__(s)`
5. 如果不存在，查找 `str` 的 `__implicit_to__(self) -> int`
6. 如果都不存在 → **编译错误**，提示用户使用显式 `as` 或 `.into()`

#### 场景 2：函数参数传递

```lz
def process(x: int) -> str = ...

s = "42"
result = process(s)  // str → int 隐式转换
```

#### 场景 3：返回值隐式转换 ★

```lz
def calculate() -> int =
    let result: str = "42"
    return result  // 返回 str，但声明返回 int → 隐式转换

// 等价于插入：
// let __tmp: int = __implicit_from__<int>(result) 或 result.__implicit_to__<int>()
// return __tmp
```

**这是用户明确要求的场景**：返回值类型 `R1` 与声明返回类型 `R` 不同时，自动隐式转换。

#### 场景 4：struct 字段默认值

```lz
struct Config =
    port: int = "8080"  // str → int 隐式转换（通过 __implicit_from__）
```

### 5.4 初始化优先级链

当目标类型 T 需要从源值 v: S 隐式构造时：

```
v: S  →  T (隐式)
        │
        ├── 1. T::__implicit_from__(v)     // T 声明了 "我可以从 S 隐式构造"
        ├── 2. v.__implicit_to__::<T>()    // S 声明了 "我可以隐式转换为 T"
        ├── 3. T::__from__(v)          // 回退：显式 From（用户手动调用链中不参与，但此处兜底）
        ├── 4. v.__into__::<T>()       // 回退：显式 Into
        └── 5. T::__default__()        // 最终兜底：默认值（仅当 v 是 default 关键字时）
```

**注意**：步骤 3-4 仅在 `__implicit_from__`/`__implicit_to__` 都不存在时作为回退。如果两者都不存在且源类型和目标类型完全无关，步骤 5 也仅当 v 字面上是 `default` 关键字时使用。

### 5.5 不触发隐式转换的场景

| 场景 | 原因 |
|------|------|
| `x as int` | 这是 cast 语法，走 `as` 路径 |
| `x.into()` | 显式调用，走 `__into__` |
| `T::from(x)` | 显式调用，走 `__from__` |
| `a == b` 且 a,b 类型不同 | 隐式转换可能导致歧义比较，应由用户显式转换 |
| 函数重载决议 | 隐式转换不参与重载选择（避免歧义） |

### 5.6 Rust 降级策略

```rust
// LZ 的 __implicit_from__ trait
trait ImplicitFrom<T> {
    fn __implicit_from__(value: T) -> Self;
}

// LZ 的 __implicit_to__ trait (blanket impl)
trait ImplicitTo<T> {
    fn __implicit_to__(self) -> T;
}

impl<T, U: ImplicitFrom<T>> ImplicitTo<U> for T {
    fn __implicit_to__(self) -> U {
        U::__implicit_from__(self)
    }
}
```

**编译器插入示例**：

```lz
// LZ
x: int = s

// 编译为 Rust
let x: i64 = <i64 as ImplicitFrom<&str>>::__implicit_from__(&s);
```

```lz
// LZ 返回值隐式转换
def calculate() -> int =
    let result: str = "42"
    return result

// 编译为 Rust
fn calculate() -> i64 {
    let result: String = "42".to_string();
    // 编译器发现 result: String 但返回类型是 i64
    // 查找 ImplicitFrom<i64>(String) → 自动插入
    <i64 as ImplicitFrom<String>>::__implicit_from__(result)
}
```

### 5.7 实现步骤

| 步骤 | 文件 | 改动 |
|------|------|------|
| 1 | `src/ir/node.rs` | 新增 `ExprKind::ImplicitConvert { source, target_ty }` |
| 2 | `src/magic/engine.rs` | 注册 `__implicit_from__` + `__implicit_to__` 魔法方法 |
| 3 | `src/ir/builder.rs` | let/return/field-default 分支插入 ImplicitConvert 节点 |
| 4 | `src/ir/codegen.rs` | 生成 `ImplicitFrom`/`ImplicitTo` trait + blanket impl + 调用点展开 |
| 5 | `src/ir/codegen.rs` | 返回值处隐式转换插入逻辑 |

---

## 六、MagicEngine 注册

需要新增注册的魔法方法：

```rust
// MagicKind 新增变体
pub enum MagicKind {
    // ... existing ...
    New,          // __new__
    Init,         // __init__
    ImplicitFrom, // __implicit_from__
    ImplicitTo,   // __implicit_to__
}

// engine.rs 注册
// 构造与初始化
self.register("__new__", MagicEntry {
    trait_path: "",        // 非标准 trait，codegen 自行生成
    trait_method: "__lz_new",
    kind: MagicKind::New,
    multi_dispatch: false,
});
self.register("__init__", MagicEntry {
    trait_path: "",
    trait_method: "__lz_init",
    kind: MagicKind::Init,
    multi_dispatch: false,
});

// 隐式转换体系
self.register("__implicit_from__", MagicEntry {
    trait_path: "",        // codegen 生成为 ImplicitFrom<T> trait
    trait_method: "__implicit_from__",
    kind: MagicKind::ImplicitFrom,
    multi_dispatch: true,  // 同一类型对多种源类型实现隐式转换
});
// __implicit_to__ 由 blanket impl 自动提供，无需单独注册
```

---

## 七、实施顺序与优先级

| 阶段 | 内容 | 预估工作量 | 依赖 |
|------|------|:---:|------|
| **Phase 1** | `__new__` 完整实现（IR 重写构造调用） | 中 | 无 |
| **Phase 2** | `__init__` 实现（构造后自动调用） | 小 | Phase 1 |
| **Phase 3** | `__implicit_from__` IR 节点 + codegen trait 生成 | 中 | Phase 1 |
| **Phase 4** | 隐式转换触发点：let + 参数 + 字段默认值 | 中 | Phase 3 |
| **Phase 5** | **返回值隐式转换**：return 语句处自动插入 | 中 | Phase 3 |
| **Phase 6** | `__implicit_to__` blanket impl | 小 | Phase 3 |
| **Phase 7** | 移除旧的"补齐字段"逻辑 | 小 | Phase 1 |

**建议顺序**：Phase 1 → Phase 2（构造器体系完整）→ Phase 3+4+5（隐式转换体系）→ Phase 6+7（清理）

---

## 八、风险与注意事项

1. **as vs implicit 的边界**：编译器绝不将隐式转换展开为 `as`。`as` 仅当用户手写 `expr as Type` 时使用
2. **显式 vs 隐式的区分**：`__from__`/`__into__` 仅响应用户显式调用；`__implicit_from__`/`__implicit_to__` 仅响应编译器自动插入
3. **禁止链式隐式转换**：`A→B` 后不再继续 `B→C`，一次只能转换一步
4. **循环转换检测**：`A: __implicit_from__(B)` 和 `B: __implicit_from__(A)` 同时存在时报编译错误
5. **返回值转换的边界**：仅当 `return expr` 的类型与函数声明返回类型完全静态可确定时，才插入隐式转换
6. **default 关键字的语义**：`default` 在隐式转换上下文中触发 `__default__`（属于优先级链的第 5 步），不在隐式转换上下文中直接调用 `T::default()`
7. **与现有 `__from__`/`__into__` 的关系**：不冲突。隐式转换独立于显式转换 trait，各自有独立的 codegen 路径

---

## 九、端到端示例

### 示例 1：返回值隐式转换

```lz
// ===== config.lz =====
struct Port =
    value: int

    // Port 可以从 str 隐式构造
    magic __implicit_from__(s: str) -> Self =
        Self(value: s.parse().unwrap_or(8080))

def get_default_port() -> Port =
    return "3000"  // str → Port，编译器自动插入 __implicit_from__
```

降级为 Rust：

```rust
struct Port { value: i64 }

trait ImplicitFrom<T> { fn __implicit_from__(value: T) -> Self; }
trait ImplicitTo<T> { fn __implicit_to__(self) -> T; }
impl<T, U: ImplicitFrom<T>> ImplicitTo<U> for T {
    fn __implicit_to__(self) -> U { U::__implicit_from__(self) }
}

impl ImplicitFrom<String> for Port {
    fn __implicit_from__(s: String) -> Self {
        Port { value: s.parse().unwrap_or(8080) }
    }
}

fn get_default_port() -> Port {
    // 编译器看到 return "3000" (String) 但声明返回 Port
    // 查找 ImplicitFrom<Port>(String) → 存在 → 自动插入
    <Port as ImplicitFrom<String>>::__implicit_from__("3000".to_string())
}
```

### 示例 2：let 赋值隐式转换（用户提到的场景）

```lz
s = "1"
x: int = s   // str → int，不是 as 转换！
```

降级：

```rust
let s = "1".to_string();
// 编译器看到 s: String, 目标类型 i64 → 查找 __implicit_from__<i64>(String)
let x: i64 = <i64 as ImplicitFrom<String>>::__implicit_from__(s);
```

### 示例 3：__new__ + __init__ + 隐式转换 组合

```lz
struct User =
    name: str
    age: int

    // 构造器：接收字符串作为 name
    magic __new__(name: str) -> Self =
        Self(name: name, age: 0)

    // 后初始化：计算年龄
    magic __init__(birth_year: int) =
        self.age = 2026 - birth_year

    // 隐式转换：从 tuple 创建
    magic __implicit_from__(t: (str, int)) -> Self =
        Self(name: t.0, age: t.1)

// 使用场景 1: 关键字构造
let u1 = User(name: "Alice", age: 30)  // → __new__(name: "Alice") → __init__(age: 30) 但参数不匹配...

// 使用场景 2: __new__ 构造
let u2 = User(name: "Bob")  // → __new__("Bob") → __init__() ← 跳过（无匹配参数）
                            // → User { name: "Bob", age: 0 }

// 使用场景 3: 隐式转换
let data = ("Charlie", 25)
let u3: User = data  // → __implicit_from__(("Charlie", 25))
                     // → 不经过 __new__/__init__（__implicit_from__ 内部已构造）
```

### 示例 4：三种转换方式在同一个类型上

```lz
struct Score =
    value: int

    // 显式转换：用户写 x.into() 时才调用
    magic __into__(ref self) -> str =
        self.value.to_string()

    // 隐式转换：编译器自动插入
    magic __implicit_from__(s: str) -> Self =
        Self(value: s.parse().unwrap_or(0))

    // cast：用户写 x as Score 时调用（未来可扩展）
    // 目前 as 不调用 trait，直接做位级转换
```

| 用户代码 | 调用哪个 trait | 原因 |
|----------|---------------|------|
| `score.into()` | `__into__` | 显式调用 |
| `Score::from("100")` | `__from__` | 显式调用 |
| `let s: Score = "100"` | `__implicit_from__` | 类型不匹配，编译器自动插入 |
| `"100" as Score` | 无（`as` 强制转换） | `as` 是语法级转换，不调用 trait |
| `return "100"`（函数返回 Score） | `__implicit_from__` | 返回值类型不匹配，编译器自动插入 |

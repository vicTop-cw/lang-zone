# LZSTD Builtins 清单

> LZ 标准库（`lz.std.*`）的默认导入内建项。**用户无需 `import` 即可使用。**
> 本清单只描述**用途、签名与规范**，不含实现。纯 Rust 便利设施（容器 / IO / 线程…）在 `rust.std`，不在此列。
> 状态图例：✅ 已在用 · 🟡 规范拟定 · ⚪ 规划中。
>
> **跨后端核心原语**：`Box` / `Rc` / `Arc` / `Future` / `spawn` 等所有权与并发原语**归 `lz.std`**（符合命名空间原则第 (2) 条：通过其他后端语言——如 Cython——集成）。Rust 后端将它们经 `rust.std` 桥接映射到 `std::boxed::Box` / `std::rc::Rc` / `std::sync::Arc` 等；Cython 后端在 `lz_std` 中自行实现。它们不是 Rust 专属便利设施，而是 LZ 语言级原语，所有后端都必须提供。

---

## 1. 枚举 (Enums)

### `Option<T>`
- **签名**：`Some(T)` / `None`
- **用途**：可选值（有 / 无）。错误处理的"无值"分支；映射 Rust `Option`。
- **规范**：variant PascalCase；`None` 不带参数；与 `Result` 二选一表达"可能缺失"。
- **方法**（Cython 后端 `lz_std` 已实现）：`is_some() -> bool` · `is_none() -> bool` · `unwrap() -> T`（None 时抛错）· `unwrap_or(default: T) -> T`（None 时回退默认值）。
- **后端命名注意**：Cython 中 Python 已有 `None`，故 `None` 构造在 `lz_std` 中名为 `None_`；LZ 源码仍写 `None`，由后端脱糖，用户无感。
- **状态**：✅ 已在用

### `Result<T, E>`
- **签名**：`Ok(T)` / `Err(E)`
- **用途**：计算结果或错误。映射 Rust `Result`；与 `?` / `??` 错误传播协作。
- **规范**：`Ok` 承载成功值，`Err` 承载错误值（建议 `E` 为 `str` 或自定义错误枚举）。
- **方法**（Cython 后端 `lz_std` 已实现）：`is_ok() -> bool` · `is_err() -> bool` · `unwrap() -> T`（Err 时抛错）· `unwrap_err() -> E`（Ok 时抛错）。
- **状态**：✅ 已在用

### `Ordering`
- **签名**：`Less` / `Equal` / `Greater`
- **用途**：比较结果，作为排序 / 比较协议（`__cmp__`）的返回值。
- **规范**：来自比较协议；Rust 起源但作为 LZ 核心比较词汇暴露。
- **状态**：🟡 规范拟定

### `ItorState`
- **签名**：`Ready` / `Yield` / `Done` / `Paused` / `Restart`
- **用途**：`Itor` 可控迭代器的状态机状态（见 §4）。
- **规范**：由 vools `data.itor` 集成而来。
- **状态**：✅ 已在用（vools 集成）

### `BridgeTier`
- **签名**：`Source` / `Ffi` / `Cli` / `SharedMemory`
- **用途**：跨后端桥接的四种层级（见 §7）。
- **状态**：🟡 规范拟定

---

## 2. 基类 (Base classes)

### `Object`（规划）
- **签名**：隐式根，不要求手写 `extends`
- **用途**：所有 LZ 值的公共根，承载魔法方法协议与基础身份。
- **规范**：多数类型隐式派生；用户通常不直接引用。
- **状态**：⚪ 规划中

### 跨后端对象根（规划）
- **签名**：`BridgeRoot` 一类抽象
- **用途**：来自其他后端语言（Python 等）的对象在 LZ 侧的封装根，使其可被魔法方法 / `Bridge` 体系处理。
- **状态**：⚪ 规划中

---

## 3. 特征 (Traits)

> LZ 用"魔法方法特征"表达运算符 / 协议重载：实现对应 `__xxx__` 即获得该能力。

### `Iterable` — 协议 `__iter__`
- **签名**：`def __iter__(self) -> Iterator`
- **用途**：可被 `for` 迭代；`for x in container` 脱糖到 `__iter__`。
- **状态**：✅ 已在用

### `Iterator` — 协议 `__next__`
- **签名**：`def __next__(self) -> Option<T>`
- **用途**：迭代器推进；返回 `Some(v)` 或 `None`（耗尽）。
- **状态**：✅ 已在用

### `Index` — 协议 `__getitem__` / `__setitem__`
- **签名**：`def __getitem__(self, key) -> V` / `def __setitem__(self, key, value)`
- **用途**：索引读取 / 写入。`container[key]` 与 `^:` 构建块均脱糖到此。
- **规范**：`^:` 右值为单值 key，脱糖为单参 `__getitem__`。
- **状态**：✅ 已在用

### `Callable` — 协议 `__call__`
- **签名**：`def __call__(self, args) -> R`
- **用途**：使实例可像函数一样调用 `obj(args)`。
- **状态**：✅ 已在用

### `Str` — 协议 `__str__`
- **签名**：`def __str__(self) -> str`
- **用途**：对象字符串化（如插值 / `print`）。
- **状态**：✅ 已在用

### `Eq` — 协议 `__eq__`
- **签名**：`def __eq__(self, other) -> bool`
- **用途**：相等比较；`==` 脱糖到此。
- **状态**：✅ 已在用

### `Ord` — 协议 `__cmp__`
- **签名**：`def __cmp__(self, other) -> Ordering`
- **用途**：排序比较；`<` `>` 等脱糖到此。
- **状态**：✅ 已在用

### `Drop` — 协议 `__drop__`
- **签名**：`def __drop__(self)`
- **用途**：析构钩子，对象生命周期结束时调用。
- **状态**：✅ 已在用

### `Rev` — 协议 `__rev__`
- **签名**：`def __rev__(self) -> Iterator`
- **用途**：反向迭代；`Itor` 集成 `DoubleEndedIterator` 时可用。
- **状态**：✅ 已在用

### `Sized` — 协议 `__len__`
- **签名**：`def __len__(self) -> int`
- **用途**：`len(x)` 脱糖到此。
- **状态**：✅ 已在用

### `Strategy`（vools 集成）
- **签名**：`trait Strategy: Clone + Destroy + Serialize + Iter`
- **用途**：可控策略特征（来自 vools `Strategy`）。
- **状态**：🟡 规范拟定

### `Bridge`
- **签名**：`trait Bridge { fn resolve_import(path) -> ... }`
- **用途**：跨后端桥接的统一特征；`SourceBridge` / `FfiBridge` / `CliBridge` / `SharedMemory` 实现之。
- **状态**：🟡 规范拟定

---

## 4. 结构体 (Structs)

### `Itor<T>`（vools 集成）
- **构造**：`itor(iterable) -> Itor<T>`
- **用途**：线程安全的可控迭代器，支持暂停 / 恢复 / 重启 / 跳转。
- **方法**：`pause()` / `resume()` / `restart()` / `jump(n)`（具体签名待定）
- **规范**：对齐 vools `data.itor`；`ItorState` 表达状态机。
- **状态**：✅ 已在用（vools 集成）

### `Strategy` / `Stuff<R>`（vools 集成）
- **构造**：`stuff(expr) -> Stuff<R>`（规划）
- **用途**：延迟执行 / 可控策略原语（来自 vools `stuff` / `history_strategy`）。
- **状态**：🟡 规范拟定

### `MemBudget`
- **构造**：`MemBudget::new(max: int) -> MemBudget`
- **用途**：编译期内存预算守卫，申请 / 释放受 `max` 约束。
- **方法**：
  - `alloc(mut self, n: int) -> Result<(), str>` — 申请 `n` 字节，超预算返回 `Err`
  - `dealloc(mut self, n: int)` — 释放（下限夹到 0）
  - `tryDealloc(mut self, n: int) -> Result<(), str>` — 安全释放（不足返回 `Err`）
  - `remaining() -> int` — 剩余预算
  - `used() -> int` — 已用
- **规范**：内部用 `Cell<int>` 维护当前用量；`import std::cell::Cell` 当前走 `rust.std` 桥接，未来归 `rust.std`。
- **状态**：✅ 已在用（`std/memguard.lz`）

### `BridgeHandle`
- **签名**：`struct BridgeHandle = ...`
- **用途**：跨后端对象的句柄，在 LZ 侧代表远端（Rust / Python…）值。
- **状态**：🟡 规范拟定

### `Box<T>` / `Rc<T>` / `Arc<T>`（所有权原语，跨后端核心）
- **构造**：`Box(x)` / `Rc(x)` / `Arc(x)`
- **用途**：`Box` = 堆分配单所有权；`Rc` = 引用计数共享（非线程安全）；`Arc` = 原子引用计数（线程安全）。
- **方法**：`get() -> T`；`Box` 额外提供 `set(v)`（可变装箱写回）。
- **规范**：归 `lz.std`（命名空间原则第 (2) 条：Cython 等后端在 `lz_std` 中自有实现，属"通过其他后端语言集成"）。Rust 后端经 `rust.std` 桥接映射到 `std::boxed::Box` / `std::rc::Rc` / `std::sync::Arc`；Cython 后端映射到 `lz_std` 的 `LzBox` / `LzRc` / `LzArc`。
- **状态**：✅ 已在用（Cython 后端 `CY/runtime/lz_std/__init__.pyx`）；Rust 后端经 `rust.std` 桥接。

---

## 5. 函数 (Functions)

### `print(args...)`
- **签名**：`print(args: ...)`
- **用途**：输出到 stdout（脱糖 `println!`）。
- **状态**：✅ 已在用

### `panic(msg: str)`
- **签名**：`panic(msg: str)`
- **用途**：异常终止程序（脱糖 `panic!`）。
- **状态**：✅ 已在用

### `assert(cond: bool, msg: str = "")`
- **签名**：`assert(cond: bool, msg: str = "")`
- **用途**：断言；`cond` 为假时 `panic(msg)`。
- **状态**：✅ 已在用

### `comptime(expr)` / `comptime let x = expr`
- **签名**：`comptime <expr>` 或 `comptime let name = <expr>`
- **用途**：编译期求值；`comptime` 上下文内的 `@memoize` 函数结果被硬编码为字面量（永久缓存）。
- **规范**：`comptime:` 后需换行缩进块体；裸 `comptime x = expr` 为隐式 `const`。
- **状态**：✅ 已在用

### `len(x) -> int`
- **签名**：`len(x) -> int`
- **用途**：返回长度 / 元素数；脱糖到 `__len__`（后端 `.len()`）。
- **状态**：✅ 已在用

### `typeof<T>(x: T) -> Type`
- **签名**：`typeof(x) -> Type`
- **用途**：运行时类型查询。
- **状态**：🟡 规范拟定

### `isinstance(x, T) -> bool`
- **签名**：`isinstance(x, T) -> bool`
- **用途**：判定 `x` 是否为类型 `T` 的实例。
- **状态**：🟡 规范拟定

---

## 6. 内建装饰器 / Intrinsics（编译器内建）

均为 `@xxx` 形式，作用于 `def` / 模块，无需 `import`。

### `@memoize`
- **签名**：`@memoize` 置于 `def` 上方
- **用途**：函数级记忆化缓存；生成 `OnceLock<HashMap<(Args), Ret>>`，命中即返回；`comptime` 上下文内结果硬编码。
- **示例**：`@memoize def fibonacci(n: int) -> int = ...`
- **状态**：✅ 已在用

### `@parallel`
- **签名**：`@parallel` 置于 `def` 或 `for` 上方
- **用途**：自动并行化；codegen 将 `.iter()` 替换为 `.par_iter()`（rayon），`for` 循环体分配到线程池。
- **适用**：纯 CPU 密集、无状态变换；不适用于 IO 密集 / 共享可变状态。
- **状态**：✅ 已在用

### `@curry`
- **签名**：`@curry` 置于 `def` 上方
- **用途**：将多参函数柯里化，支持偏应用（与管道 `|>` + `_` 偏应用互补）。
- **状态**：✅ 已在用

### `@overload`
- **签名**：`@overload` 置于 `def` 上方
- **用途**：函数重载（按参数类型 / 数量分派）。
- **状态**：✅ 已在用

### `@derive`
- **签名**：`@derive(Clone, Eq, ...)` 置于 `struct` 上方
- **用途**：自动派生 trait 实现（Clone / Eq / 等）。
- **状态**：✅ 已在用

### `@tail_call`
- **签名**：`@tail_call` 置于 `def` 上方
- **用途**：尾调用优化（消除递归栈溢出）。
- **状态**：✅ 已在用

### `@export(Rust)` / `@export(Python)`
- **签名**：`@export(Rust)` / `@export(Python)` 置于 `def` / 模块上方
- **用途**：多后端导出——`Rust` → `pub fn`；`Python` → PyO3 module（`#[cfg(feature="pyo3")]`）。
- **状态**：✅ 已在用

### `@init`
- **签名**：`@init` 置于模块上方
- **用途**：模块初始化钩子（加载时执行一次）。
- **状态**：✅ 已在用

---

## 7. 跨后端集成 (Bridge / multi-backend)

> 这是 `lz.std` 区别于 `rust.std` 的核心：把"其他后端语言"接入 LZ。当前 4 种桥接范式由 `BridgeRegistry` 统一路由所有 `import` 路径。

### `BridgeRegistry`
- **签名**：`BridgeRegistry.resolve_import_full(path) -> Resolved`
- **用途**：统一路由所有 `import` 路径（含 `rust.` / `py.` 等前缀）。
- **状态**：🟡 规范拟定

### `SourceBridge`（编译期源码桥接）
- **用途**：把目标语言源码在编译期直接桥接进 LZ（源码级转译）。
- **状态**：✅ 已在用

### `FfiBridge`（C ABI 桥接）
- **用途**：通过 C ABI 调用外部库。
- **状态**：✅ 已在用

### `CliBridge`（IPC 桥接）
- **用途**：通过进程间通信调用外部程序 / 服务。
- **状态**：✅ 已在用

### `SharedMemory`（共享内存桥接）
- **用途**：跨进程共享内存数据交换。
- **状态**：⚪ 规划中

### RustBridge 直通
- **形式**：`import lz.std.bridge.rust.serde_json` → `use serde_json;`
- **CLI**：`--rust-crate serde_json=1.0`
- **用途**：把 Rust crate 直接作为 LZ 命名空间导入，零抽象损耗。
- **状态**：🟡 规范拟定

---

## 8. 并发与错误层次（规划中，源自 Cython 后端 `lz_std` 设计）

> CY/USAGE.md §5 规划了 `lz_concurrency`（Future / spawn / go）与 `lz_exceptions`（异常层次）。截至当前，`CY/runtime` 仅落地了 `lz_std/__init__.pyx`（Option / Result / Box / Rc / Arc），`lz_concurrency` / `lz_exceptions` 尚未实现。以下为**规范拟定**，待对应后端落地后转正。它们归 `lz.std`（跨后端语言原语）。

### `Future<T>`（规划）
- **签名**：`Future<T>`
- **用途**：异步结果占位；`spawn` 返回 `Future<T>`，可 `await` 获取。
- **状态**：⚪ 规划中

### `spawn` / `go`（规划）
- **签名**：`spawn(expr) -> Future<T>` · `go expr`（并发派生，不等结果）
- **用途**：轻量并发派生；与 `@parallel`（自动并行）互补（显式派生 vs 自动并行化）。
- **规范**：主项目 `go` 目前为设计阶段关键字，未实现；Cython 后端 `lz_concurrency` 亦未落地。
- **状态**：⚪ 规划中

### 异常层次（规划）
- **签名**：`panic(msg: str)`（已有）· 规划 `Exception` 基类 + 子类（`IOError` / `ValueError` …）
- **用途**：结构化错误层次，替代当前 `Err(str)` 单一错误通道；`panic` 仍为零权终止。
- **状态**：🟡 规范拟定（`CY/runtime` 仅 `lz_exceptions` 规划，未实现）

---

## 附：与 `rust.std` 的边界（速查）

| 类别 | 归 `lz.std`（默认导入） | 归 `rust.std`（显式 import） |
|------|------------------------|------------------------------|
| 标量 | `int` `f64` `str` `bool` `()` | — |
| 核心枚举 | `Option` `Result` `Ordering` | — |
| 容器 | `List` `Dict` `Set`（名称别名） | `Vec` `HashMap` `BTreeMap` `VecDeque` … |
| 协议 | 魔法方法 `__xxx__`、`Bridge` | `Iterator` / `From` / `Eq` 等（如需显式约束） |
| 库 | `@memoize` `Itor` `MemBudget` `Strategy` | `rayon` `tokio` `serde` … |
| 所有权/并发 | `Box` `Rc` `Arc` `Future` `spawn`（LZ 语言原语，所有后端必须提供） | Rust 后端经桥接映射到 `std::boxed::Box` / `std::rc::Rc` / `std::sync::Arc` 等 |
| 后端 | `Bridge` 体系（含 `rust.` 直通） | `rust.std.*` 全部 |

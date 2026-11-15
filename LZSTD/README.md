# LZSTD — Lang-Zone 标准库

> LZ **自己**的标准库命名空间。用户无需 `import`，默认已在作用域内（prelude）。

## 1. 定位与命名空间原则

LZ 的"标准库"分三层，**互不重叠**：

| 命名空间 | 含义 | 导入方式 |
|----------|------|----------|
| `lz.std.*` | **LZ 自己的标准库**——Rust 没有的，或通过其他后端语言集成的 | 默认导入，无需写 `import` |
| `rust.std` | Rust 标准库（`Vec` / `HashMap` / `Iterator` …） | 需显式 `import rust.std` |
| `<backend>.std` | 其他后端语言标准库（如 `py.std` 为 Python） | 需显式 `import py.std` 等 |

**关键原则**：`lz.std` **不是** Rust std 的镜像。它只放两类东西：

1. **Rust 没有的**——LZ 原生概念：魔法方法协议、构建块、内建装饰器（intrinsics）、`Itor`、`Strategy` / `Stuff`、内存预算守卫、跨后端 `Bridge` 体系……
2. **通过其他后端语言集成的**——例如从 vools 引入的 `Itor` / `Strategy` / `Stuff`；以及 Cython 后端在 `lz_std` 中实现的**所有权与并发原语** `Box` / `Rc` / `Arc` / `Future` / `spawn`（LZ 语言级原语，所有后端都必须提供，非 Rust 专属便利设施）。

纯 Rust 便利设施（容器、IO、线程、文件系统……）留在 `rust.std`，**不要抄进 `lz.std`**。

> **与现状的衔接**：当前仓库 `std/modules/*.toml`（`vec.toml` / `collections.toml` / `iter.toml` …）是 Rust std 的桥接清单，属于将来的 `rust.std` 层；`std/intrinsics/*` 与 `std/memguard.lz`、以及魔法方法 / 构建块 / `Bridge` 机制才是 `lz.std` 的既有家底。本目录只收后者。

## 2. 基线结构（要建立的 5 类）

标准库按 **基类 / 特征 / 结构体 / 函数 / 枚举** 五类组织，职责分明：

- **基类 (Base classes)**：类型系统的根与公共抽象（魔法方法接收者根、跨后端对象根）。多数 LZ 类型隐式派生，无需手写。
- **特征 (Traits)**：能力契约。LZ 大量用"魔法方法特征"表达运算符 / 协议重载（`__getitem__`→索引、`__iter__`→迭代、`__call__`→可调用……），以及跨后端集成的策略 / 桥接特征。
- **结构体 (Structs)**：具名复合数据 + 行为。`Itor`、`Strategy`、`Stuff`、`MemBudget`、`BridgeHandle`，以及所有权原语 `Box` / `Rc` / `Arc`（跨后端核心）等 LZ 原生类型在此。
- **函数 (Functions)**：自由函数与内建函数 `print` / `panic` / `assert` / `typeof` / `comptime` 等，以及跨后端桥接辅助。
- **枚举 (Enums)**：代数数据类型。`Option` / `Result` / `Ordering` / `ItorState` / `BridgeTier` 等。

## 3. 默认导入（Prelude）

以下内容**始终在作用域内**，用户不写 `import` 即可用：

- 标量类型：`int` `f64` `str` `bool` `unit ()`
- 核心枚举：`Option<T>` `Result<T,E>` `Ordering`
- 元组与与后端无关的名称别名：`(A,B)`、`List<T>`（后端映到 `Vec`）、`Dict<K,V>`、`Set<T>`
- 所有权原语（跨后端核心，归 `lz.std`，默认导入）：`Box<T>` `Rc<T>` `Arc<T>`
- 内建函数：`print` `panic` `assert` `typeof` `isinstance` `comptime` `len`
- 内建装饰器：`@memoize` `@parallel` `@curry` `@overload` `@derive` `@tail_call` `@export` `@init`
- 魔法方法协议（作为可重载协议存在，可被任意类型实现）

> 重型容器（`Vec` / `HashMap` / `BTreeMap` …）属 `rust.std`，需显式 `import rust.std`；`lz.std` 仅在内核层提供 `List` / `Dict` / `Set` 作为与后端无关的名称别名。

## 4. 命名与签名规范

- **类型**：PascalCase（`MemBudget` / `Itor` / `Strategy`）；跨后端路径用 `.`（`lz.std.itor.Itor`）。
- **方法**：camelCase（`isEmpty` / `append` / `remaining`），codegen 映射到后端 snake_case（`is_empty` / `push` / `remaining`）。
- **自由函数 / 装饰器**：内建装饰器统一 `@xxx` 形式；同类命名保持一致。
- **枚举 variant**：PascalCase（`Some` / `None` / `Ok` / `Err`）。
- **构建块**：`=:` / `^:` / `~:` / `*:` 为整体 token，冒号后**必须换行缩进**；操作符前后留白。
- **魔法方法**：`__` 双下划线包裹（`__getitem__`），禁止作为普通标识符。
- **单元返回**用 `()` 而非 `void`；字符串类型名是 `str` 不是 `string`。
- **注释**用 `//`；`#` 是属性宏标记（`#[derive(...)]`），不是注释。

## 5. 状态图例

- ✅ 已在用：代码库已有对应实现 / 用法
- 🟡 规范拟定：接口已定，实现待补
- ⚪ 规划中：方向已定，接口待定

## 6. 如何新增一个 builtin

1. 确定类别（基类 / 特征 / 结构体 / 函数 / 枚举）。
2. 在 `builtins.md` 对应章节登记：**签名 + 用途 + 规范 + 状态**。
3. LZ 原生 → 实现于 `lz.std`；Rust 映射 → 归入 `rust.std` 桥接清单（不在此库）。
4. 跨后端集成 → 走 `Bridge` 体系，登记到「跨后端集成」章节。

## 7. 目录内容

- [`builtins.md`](builtins.md) — 分类 builtins 清单（本库核心交付物）

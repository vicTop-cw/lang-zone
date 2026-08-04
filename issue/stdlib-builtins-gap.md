# LZSTD Builtins 现状分析与 DEMO/lz_std 迁移建议

> **Status**: Open
> **Severity**: High
> **Category**: stdlib
> **Discovered**: 2026-08-03
> **Reporter**: AI Agent (builtins audit)
> **Owner**: automation-agent

---

## Summary

当前 `LZSTD/` 下的标准库实现 (`builtins.lz` / `prelude.lz` / `traits.lz`) 仅为占位桩代码，无法支撑一般性 LZ 项目开发。作为替代，已在 `DEMO/lz_std/` 中编写完整的标准库定义（14 个模块），严格遵循 SYNTAX 规范。

## 具体发现

### 1. LZSTD/builtins.lz — 仅有签名桩

**当前状态**: 1.41 KB, 9 个 `rust.std.*` 桥接 + 9 个装饰器桩 (`@memoize` / `@parallel` / `@curry` / `@overload` / `@derive` / `@tail_call` / `@export` / `@init`)。所有实现体为 `pass` 或单行桥接。

**缺失内容**:
- `print` / `abort` / `check` 等内建函数无实际逻辑
- 无 Option/Result 方法实现
- 无迭代器基础设施
- 无集合操作方法

### 2. LZSTD/prelude.lz — 仅有空壳定义

**当前状态**: 346 B, `Option` / `Result` / `Ordering` enum 仅定义变体无方法; `Box` / `Rc` / `Arc` struct 使用 `_inner: int` 占位。

**缺失内容**:
- Option: 缺少 `is_some` / `unwrap` / `map` / `and_then` / `or_else` / `ok_or` 等全部方法
- Result: 缺少 `is_ok` / `unwrap` / `map` / `map_err` / `and_then` / `or_else` / `ok` / `err` 等全部方法
- Ordering: 缺少 `reverse` / `then` / `then_with` / `is_lt` 等全部方法
- Box/Rc/Arc: 缺少 `new` / `get` / `clone` / `try_unwrap` 等全部方法

### 3. LZSTD/traits.lz — Trait 体系严重不完整

**当前状态**: 526 B, 仅 9 个 trait (`Iterable` / `Iterator` / `Index` / `Callable` / `Str` / `Eq` / `Ord` / `Drop` / `Sized`), 每个仅有 1 个方法且用占位返回值。

**缺失 trait**:
- `PartialEq` / `PartialOrd` — 部分比较
- `Hash` — 哈希
- `Clone` / `DeepClone` — 克隆
- `Default` — 默认值
- `Display` / `Debug` — 显示调试
- `From<T>` / `Into<T>` / `TryFrom<T>` / `TryInto<T>` — 类型转换
- `Add` / `Sub` / `Mul` / `Div` / `Rem` / `Neg` / `Not` — 算术运算
- `AddAssign` / `SubAssign` 等 — 复合赋值
- `BitAnd` / `BitOr` / `BitXor` / `Shl` / `Shr` — 位运算
- `IndexMut` — 可变索引
- `DoubleEndedIterator` / `IntoIterator` / `FromIterator` — 迭代器体系
- `Len` / `Bool` — 长度/布尔判定
- `Error` — 错误 trait

### 4. Iterator trait 缺少所有适配器和消费器方法

当前 `Iterator` trait 仅有 `__next__` 和 `size_hint`, 缺少:
- 消费器: `for_each` / `collect` / `fold` / `reduce` / `count` / `sum` / `product` / `all` / `any` / `find` / `nth` / `last`
- 适配器: `map` / `filter` / `take` / `skip` / `chain` / `enumerate` / `flat_map` / `rev` / `peekable`

## 新增 DEMO/lz_std/ 模块清单

| 文件 | 大小 | 内容 |
|------|------|------|
| `plan.md` | - | 完整计划文档 |
| `__init__.lz` | ~1 KB | 模块入口 + 版本信息 |
| `traits.lz` | ~15 KB | 40+ trait 定义 + 8 个迭代器适配器 struct |
| `option.lz` | ~7 KB | Option<T> 枚举 + 20+ 方法 + 自测试 |
| `result.lz` | ~8 KB | Result<T, E> 枚举 + 25+ 方法 + 自测试 |
| `ordering.lz` | ~5 KB | Ordering 枚举 + min/max/clamp + 自测试 |
| `string.lz` | ~9 KB | str 扩展方法 + CharIter + join/format + 自测试 |
| `list.lz` | ~10 KB | List<T> 扩展方法 + 迭代器 + 自测试 |
| `dict.lz` | ~8 KB | Dict<K,V> 扩展方法 + 迭代器 + 自测试 |
| `set.lz` | ~7 KB | Set<T> 扩展方法 + 集合运算 + 自测试 |
| `iter.lz` | ~10 KB | Range/Once/Repeat/Zip/StepRange + 全局工具函数 + 自测试 |
| `math.lz` | ~10 KB | 常量/PI/三角函数/对数/取整/数论 + 自测试 |
| `error.lz` | ~7 KB | LzError 枚举 (14 变体) + Error trait + 自测试 |
| `box.lz` | ~6 KB | Box/Rc/Arc/Weak + 自测试 |
| `prelude.lz` | ~4 KB | Prelude 汇总 + API 清单 + 自测试 |

**总计**: 14 个 .lz 模块文件, 覆盖 100+ 类型/方法, 50+ 自测试用例。

## 语法合规说明

所有代码严格遵循 SYNTAX 规范:
- 4 空格缩进, 无分号
- 冒号后换行缩进 (控制流)
- 路径用 `.` (不用 `::`)
- `struct` 用 `=`, `enum` 用 `:`, `trait` 用 `=`
- struct 字段必注解, 闭包参数必注解
- 魔法方法签名符合规范 (`ref self` for `__eq__`/`__str__`/`__hash__`, `self` for `__add__`)
- 使用 `mut self` 表示可变方法

## 建议操作

1. **短期**: 将 `DEMO/lz_std/` 作为编译测试目标, 根据编译错误修复 parser/codegen
2. **中期**: 当 `DEMO/lz_std/` 编译通过后, 将其内容迁移到 `LZSTD/` 替换现有桩代码
3. **长期**: 自举时使用 `DEMO/lz_std/` 作为 `lz::builtins` 的基础实现

## Verification

```bash
# 验证所有文件语法正确 (待 parser/codegen 支持后)
cargo test compile_demos -- DEMO/lz_std/
```

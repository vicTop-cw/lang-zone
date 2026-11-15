# LZ Builtins 完整标准库计划

## 目标
将 `DEMO/lz_std/` 打造成完整的 LZ 标准库，涵盖真实 LZ 项目开发所需的核心类型与工具。

## 合规标准
严格按 SYNTAX 规范编写，即使当前转译器暂不能编译通过，由自动化 agent 根据 issue 修复 parser/codegen。

## 模块规划

### 1. `traits.lz` — 核心 Trait 体系
- `Eq` / `PartialEq` — 相等比较
- `Ord` / `PartialOrd` — 全序/偏序比较
- `Hash` — 哈希
- `Clone` / `DeepClone` — 克隆
- `Default` — 默认值
- `Display` / `Debug` — 显示与调试
- `From<T>` / `Into<T>` — 类型转换
- `Add` / `Sub` / `Mul` / `Div` / `Rem` — 算术运算
- `Neg` / `Not` — 一元取反
- `BitAnd` / `BitOr` / `BitXor` / `Shl` / `Shr` — 位运算
- `Index` / `IndexMut` — 索引
- `Iterator` / `Iterable` / `IntoIterator` — 迭代器体系
- `Drop` — 析构
- `Sized` — 编译期大小
- `Callable` — 可调用
- `Str` — 字符串表示
- `Len` — 长度
- `Bool` — 布尔判定
- `Error` — 错误 trait

### 2. `option.lz` — Option<T> 枚举
- 构造: `Some(value)` / `None`
- 查询: `is_some` / `is_none` / `is_some_and`
- 提取: `unwrap` / `unwrap_or` / `unwrap_or_else` / `expect`
- 转换: `map` / `map_or` / `map_or_else` / `and` / `and_then` / `or` / `or_else` / `filter`
- 转换到 Result: `ok_or` / `ok_or_else`
- 迭代: `iter` / `iter_mut`
- 魔法: `__bool__` / `__eq__` / `__str__`

### 3. `result.lz` — Result<T, E> 枚举
- 构造: `Ok(value)` / `Err(error)`
- 查询: `is_ok` / `is_err` / `is_ok_and` / `is_err_and`
- 提取: `unwrap` / `unwrap_or` / `unwrap_or_else` / `unwrap_err` / `expect` / `expect_err`
- 转换: `map` / `map_or` / `map_or_else` / `map_err` / `and` / `and_then` / `or` / `or_else`
- 转换到 Option: `ok` / `err`
- 传播: `?` 运算符（通过 `__try__`）
- 魔法: `__bool__` / `__eq__` / `__str__`

### 4. `ordering.lz` — Ordering 枚举
- `Less` / `Equal` / `Greater`
- `reverse` / `then` / `then_with`
- 魔法: `__str__` / `__eq__`

### 5. `string.lz` — String 方法与适配
- `Str` trait 实现
- 常用方法: `len` / `is_empty` / `contains` / `starts_with` / `ends_with` / `split` / `trim` / `replace` / `to_upper` / `to_lower` / `chars` / `lines`
- `str` 类型的 `__add__` / `__eq__` / `__len__`

### 6. `list.lz` — List<T> 方法
- 构造: 字面量 `[a, b, c]`
- 查询: `len` / `is_empty` / `first` / `last` / `get` / `contains`
- 修改: `push` / `pop` / `insert` / `remove` / `clear` / `extend`
- 迭代: `iter` / `iter_mut` / `into_iter`
- 转换: `map` / `filter` / `fold` / `reduce` / `collect`
- 排序: `sort` / `sort_by` / `reverse`
- 魔法: `__len__` / `__getitem__` / `__setitem__` / `__iter__` / `__eq__` / `__add__` / `__str__`

### 7. `dict.lz` — Dict<K, V> 方法
- 查询: `len` / `is_empty` / `get` / `contains_key` / `keys` / `values` / `items`
- 修改: `insert` / `remove` / `clear` / `update`
- 迭代: `iter` / `iter_keys` / `iter_values`
- 魔法: `__len__` / `__getitem__` / `__setitem__` / `__iter__` / `__eq__` / `__str__`

### 8. `set.lz` — Set<T> 方法
- 查询: `len` / `is_empty` / `contains`
- 修改: `add` / `remove` / `clear`
- 集合运算: `union` / `intersection` / `difference` / `symmetric_difference` / `is_subset` / `is_superset`
- 魔法: `__len__` / `__iter__` / `__eq__` / `__str__` / `__and__` / `__or__` / `__sub__`

### 9. `iter.lz` — 迭代器模块
- `Iter<T>` struct — 标准迭代器
- `IterState` enum — `Yield(item)` / `Done`
- 适配器: `Map` / `Filter` / `Take` / `Skip` / `Chain` / `Zip` / `Enumerate` / `Rev` / `FlatMap`
- 消费器: `collect` / `fold` / `reduce` / `count` / `sum` / `product` / `any` / `all` / `find` / `nth` / `for_each`
- 构造: `iter` / `range` / `once` / `repeat`

### 10. `math.lz` — 数学模块
- 常量: `PI` / `E` / `TAU`
- 基本: `abs` / `min` / `max` / `clamp` / `sign`
- 幂: `sqrt` / `cbrt` / `pow` / `exp` / `log` / `log2` / `log10`
- 三角: `sin` / `cos` / `tan` / `asin` / `acos` / `atan` / `atan2`
- 取整: `floor` / `ceil` / `round` / `trunc`
- 数论: `gcd` / `lcm` / `is_prime`

### 11. `error.lz` — 异常层级
- `Error` trait
- `LzError` 基础异常 enum
- `ValueError` / `TypeError` / `IndexError` / `KeyError` / `RuntimeError` / `NotImplementedError` / `AssertionError` / `OverflowError` / `ZeroDivisionError`
- `raise` / `raises` / `try`-`catch`-`finally` 支持

### 12. `box.lz` — 智能指针
- `Box<T>` — 堆分配所有权指针
- `Rc<T>` — 引用计数共享指针
- `Arc<T>` — 原子引用计数共享指针
- `Weak<T>` — 弱引用

### 13. `prelude.lz` — Prelude 汇总
- 从各模块导入核心符号
- 便于 `from lz_std.prelude import *`

## 测试约束
- 每个文件必须自包含（可独立编译），因为 DEMO 测试逐文件编译
- 每个文件包含 `def main()` 作为入口
- 使用 `test` 块做自测试（如果编译器支持）

## 交付物
1. 上述所有 `.lz` 文件
2. 一份 issue 记录当前 LZSTD 的不足和迁移建议

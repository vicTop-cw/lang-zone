# LZ 模块系统与多文件编译测试 Bug 报告

> 测试日期: 2026-07-25
> 测试方法: 创建多文件模块导入测试用例，覆盖 `import` 模块、`from ... import`、私有函数访问、结构体跨模块构造
> 编译器: `lang-zong.exe` (release build)
> 阶段: Phase 2

---

## 一、Bug 汇总

### 严重程度图例
- 🔴 **严重**: 编译通过但生成无效 Rust 代码（静默错误）
- 🟡 **中等**: LZ 编译报错，但错误信息不准确或位置偏移
- 🟢 **轻微**: 文档与实现不一致，但不影响使用

---

## 二、🔴 新发现 Bug

### Bug-35: `from ... import` 的函数调用生成 `__call_magic` 而非直接调用

**代码**:
```lz
from _test_module_a import add, greet, make_point

def test_from_import() =
    sum = add(3, 4)          // 直接调用
    msg = greet("World")     // 直接调用
    p = make_point(10, 20)   // 直接调用
```

**生成 Rust**:
```rust
use _test_module_a::{add, greet, Point, make_point};

fn test_from_import() {
    let mut sum = __call_magic(add, (3, 4));
    let mut msg = __call_magic(greet, ("World",));
    let mut p = __call_magic(make_point, (10, 20));
}
```

**Rust 编译错误**: `error[E0425]: cannot find function '__call_magic' in this scope`

**分析**: 通过 `from X import Y` 导入的函数，在调用时仍被误识别为"可调用对象"而非直接函数调用。这是 Bug-30 的变体——Bug-30 针对桥接模块的构造器，而 Bug-35 针对 `from ... import` 的所有函数。

**影响范围**: 所有使用 `from module import func` 语法导入的函数调用。

---

### Bug-36: `import` 模块生成 `use` 语句但缺少 `mod` 声明

**代码**:
```lz
import _test_module_a
```

**生成 Rust**:
```rust
use _test_module_a;
```

**Rust 编译错误**: `error[E0432]: unresolved import '_test_module_a'` — `no external crate '_test_module_a'`

**分析**: lz 编译器将 `import` 直接翻译为 `use`，但没有生成 `mod _test_module_a;` 声明，也没有将模块文件引入编译单元。多文件编译需要：(1) 生成 `mod` 声明 (2) 或将所有模块合并到一个 Rust 文件中。

**影响范围**: 所有多文件 `import` 编译（模块系统完全不可用）。

---

### Bug-37: 模块内函数/结构体缺少 `pub` 可见性

**代码** (_test_module_a.lz):
```lz
def add(a: int, b: int) -> int =
    a + b

struct Point =
    x: int
    y: int
```

**生成 Rust** (_test_module_a.rs):
```rust
fn add(a: i64, b: i64) -> i64 { a + b }

struct Point {
    x: i64,
    y: i64,
}
```

**Rust 编译错误**: 即使 `use` 声明正确，外部模块也无法访问这些非 `pub` 项。

**分析**: 模块内所有公开的函数和结构体都应该生成 `pub` 关键字。`__MODULE_PUBLIC` 元数据正确记录了公开项列表，但代码生成阶段未使用该信息添加 `pub` 修饰符。

**影响范围**: 所有跨模块访问（函数调用、结构体构造、类型引用）。

---

### Bug-38: 模块路径结构体构造生成函数调用语法

**代码**:
```lz
import _test_module_a
p = _test_module_a.Point(5, 15)
```

**生成 Rust**:
```rust
let mut p = _test_module_a.Point(5, 15);
```

**Rust 编译错误**: `error[E0423]: expected function, found struct 'Point'` — Rust 不支持函数调用语法构造结构体。

**分析**: lz 的 `Point(5, 15)` 语法在单文件编译时正确生成为 `Point { x: 5, y: 15 }`，但通过模块路径 `_test_module_a.Point(5, 15)` 访问时，代码生成路径不同，误用了函数调用语法。应生成 `Point { x: 5, y: 15 }`。

**影响范围**: 所有跨模块结构体构造（通过模块路径访问）。

---

### Bug-39: `_internal` 私有函数未在 LZ 编译阶段检查可见性

**代码** (_test_module_a.lz):
```lz
def _internal() -> str =    // 私有函数（_ 前缀）
    "secret"
```

**代码** (_test_module_edge.lz):
```lz
import _test_module_a
result = _test_module_a._internal()   // 应报错：不可访问
```

**LZ 编译**: 通过，无错误信息。

**分析**: LZ 编译器正确识别了 `_internal` 为私有函数（`__MODULE_PRIVATE: &["_internal"]`），但未在编译阶段进行跨模块可见性检查。`_` 前缀命名的私有函数应该仅限模块内访问。

**影响范围**: 所有模块私有函数的安全访问控制（缺少封装保障）。

---

## 三、✅ 验证通过的特性

| 特性 | 状态 |
|------|------|
| `import module` 的 LZ 编译 | ✅ 通过 |
| `from module import item` 的 LZ 编译 | ✅ 通过 |
| `module.func(args)` 的模块路径调用 | ✅ 生成正确 Rust 代码 |
| `module.identity(val)` 的泛型函数调用 | ✅ 生成正确 Rust 代码 |
| `__MODULE_PUBLIC` / `__MODULE_PRIVATE` 元数据 | ✅ 正确区分 |
| `__MODULE_DEPS` 依赖记录 | ✅ 正确记录 |

---

## 四、统计

| 类别 | 数量 |
|------|------|
| 🔴 新发现严重 Bug | 5 (Bug-35 ~ Bug-39) |
| 阶段 2 新增 | **5** |
| 累计 Bug 总数 | **39** |
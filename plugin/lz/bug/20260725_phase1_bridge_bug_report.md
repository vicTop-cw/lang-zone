# LZ 桥接模块深度测试 Bug 报告

> 测试日期: 2026-07-25
> 测试方法: 编写桥接模块深度测试用例，覆盖 Result 解包、导入变体、类型映射
> 编译器: `lang-zong.exe` (release build)
> 阶段: Phase 1

---

## 一、Bug 汇总

### 严重程度图例
- 🔴 **严重**: 编译通过但生成无效 Rust 代码（静默错误）
- 🟡 **中等**: LZ 编译报错，但错误信息不准确或位置偏移
- 🟢 **轻微**: 文档与实现不一致，但不影响使用

---

## 二、🔴 新发现 Bug

### Bug-30: PathBuf 构造生成 `__call_magic` 而非 `PathBuf::from`

**代码**:
```lz
import std::bridge::rust::std::path::PathBuf
path = PathBuf("test.txt")
```

**生成 Rust**:
```rust
let mut path = __call_magic(PathBuf, ("test.txt",));
```

**Rust 编译错误**: `error[E0423]: expected value, found struct 'PathBuf'` + `error[E0425]: cannot find function '__call_magic'`

**分析**: lz 编译器将 `PathBuf("test.txt")` 识别为"可调用对象"而非构造器，误生成 `__call_magic`。应生成 `PathBuf::from("test.txt")`。

**影响范围**: 所有通过桥接导入的 Rust 非单元结构体（需要参数构造）的实例化。

---

### Bug-31: HashMap 默认构造缺少类型注解

**代码**:
```lz
import std::bridge::rust::std::collections::HashMap
map = HashMap()
```

**生成 Rust**:
```rust
let mut map = HashMap::default();
```

**Rust 编译错误**: `error[E0283]: type annotations needed for HashMap<_, _, _>`

**分析**: `HashMap::default()` 需要类型参数，但 lz 编译器生成时未添加类型注解。应生成 `HashMap::<String, String>::default()` 或让用户显式标注类型。

**影响范围**: 所有通过桥接导入的泛型集合类型的默认构造。

---

### Bug-32: 字符串字面量作为函数参数不自动转换

**代码**:
```lz
def test_bridge_params(content: str) =
    print(content.len())
test_bridge_params("hello")
```

**生成 Rust**:
```rust
fn test_bridge_params(content: String) {
    println!("{:?}", content.len())
}
test_bridge_params("hello");  // &str → String 不匹配
```

**Rust 编译错误**: `error[E0308]: mismatched types, expected String, found &str`

**分析**: lz 编译器在函数调用时，对字符串字面量实参没有自动添加 `.to_string()` 转换。当形参类型为 `str`（映射为 Rust `String`）时，实参 `&str` 字面量类型不匹配。

**影响范围**: 所有以字符串字面量作为 `str` 类型参数的函数调用。

---

### Bug-33: 桥接函数 Result 值链式传递错误

**代码**:
```lz
def test_chained_bridge() =
    content = std::fs::read_to_string("file.txt")
    write_result = std::fs::write("output.txt", content)
```

**生成 Rust**:
```rust
fn test_chained_bridge() {
    let mut content = std::fs::read_to_string("file.txt");  // Result<String>
    let mut write_result = std::fs::write("output.txt", content);  // content 是 Result!
}
```

**Rust 编译错误**: `error[E0277]: the trait bound Result<String, Error>: AsRef<[u8]> is not satisfied`

**分析**: 当桥接函数返回值直接传递给另一个桥接函数时，`Result` 类型没有自动 unwrap。`content` 是 `Result<String>` 而非 `String`，导致 `write` 调用类型不匹配。

**影响范围**: 所有桥接函数调用链（返回值作为另一个桥接函数参数）。

---

### Bug-34: 桥接函数返回值类型不匹配（函数尾表达式）

**代码**:
```lz
def test_chained_bridge() =
    write_result = std::fs::write("output.txt", "content")
    write_result  // 尾表达式返回
```

**生成 Rust**:
```rust
fn test_chained_bridge() {
    let mut write_result = std::fs::write("output.txt", "content");
    write_result  // Result<()> 不是 ()
}
```

**Rust 编译错误**: `error[E0308]: mismatched types, expected (), found Result<(), Error>`

**分析**: 函数体末表达式是桥接函数返回值（`Result<()>`），但函数声明返回类型为 `()`（默认）。编译器应生成 `.unwrap()` 或要求函数声明 `raises` 错误类型。

**影响范围**: 所有以桥接函数返回值作为函数尾表达式的场景。

---

## 三、🔴 已知 Bug 复现确认

| Bug 编号 | 描述 | 复现状态 |
|----------|------|----------|
| Bug-25 | Result 不自动 unwrap | ✅ 多处复现（read_to_string, write, read_dir, exists） |
| Bug-27 | 字符串字面量返回类型不匹配 | ✅ test_bridge_return 中复现 |
| Bug-28 | 冗余 import 警告 | ✅ `use std::fs;` 和 `use std::io;` 未使用 |

---

## 四、统计

| 类别 | 数量 |
|------|------|
| 🔴 新发现严重 Bug | 5 (Bug-30 ~ Bug-34) |
| 🔴 已知 Bug 复现 | 3 (Bug-25, 27, 28) |
| **阶段 1 新增** | **5** |
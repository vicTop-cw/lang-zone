# LZ 自举过程 Bug 报告

> 测试日期: 2026-07-25
> 测试方法: 使用 lz 桥接模块（`import std::bridge::rust::*`）实现自举第一步
> 编译器: `lang-zong.exe` (release build)

---

## 一、Bug 汇总

### 严重程度图例
- 🔴 **严重**: 编译通过但生成无效 Rust 代码（静默错误）
- 🟡 **中等**: LZ 编译报错，但错误信息不准确或位置偏移
- 🟢 **轻微**: 文档与实现不一致，但不影响使用

---

## 二、🔴 LZ 编译器代码生成 Bug

### Bug-25: 桥接函数返回 Result 时不自动 unwrap

**代码**:
```lz
import std::bridge::rust::std::fs

def step_import()-> str =
    content = std::fs::read_to_string("src/main.rs")
    content
```

**生成 Rust**:
```rust
fn step_import() -> String {
    let mut content = std::fs::read_to_string("src/main.rs");  // Result<String>
    content  // 类型不匹配！Result<String> != String
}
```

**Rust 编译错误**: `error[E0308]: mismatched types - expected String, found Result<String, Error>`

**分析**: lz 编译器在生成桥接函数调用时，没有对 Rust 标准库返回的 `Result` 类型做自动 unwrap 处理。`std::fs::read_to_string` 返回 `Result<String>`，但 lz 将其直接赋值给 `String` 类型变量。

**影响范围**: 所有通过 `import std::bridge::rust::*` 调用的 Rust 标准库函数，只要返回 `Result` 类型都会触发此问题。

**建议修复**: 桥接函数调用时，对返回 `Result` 类型的函数自动追加 `.unwrap()` 或生成 `?` 操作符。

---

### Bug-26: 字符串拼接 `+` 运算符生成有误

**代码**:
```lz
result = result + "Size: " + content.len().to_string() + " bytes\n"
```

**生成 Rust**:
```rust
result = result + "Size: " + content.len().to_string() + " bytes\n";
// 等价于: ((result + "Size: ") + content.len().to_string()) + " bytes\n"
// 问题: String + &str + String 链式调用中，中间结果 String + String 不合法
```

**Rust 编译错误**: `error[E0308]: mismatched types - expected &str, found String`

**分析**: lz 编译器的 `+` 运算符只考虑了单次 `String + &str` 的场景，没有处理链式拼接。当拼接链中间出现 `String` 类型时（如 `.to_string()` 返回值），Rust 的 `Add` trait 不支持 `String + String`。

**影响范围**: 所有包含两次以上 `+` 运算符的字符串拼接表达式。

**建议修复**: 对于多段字符串拼接，生成 `format!()` 宏调用，或在每个非 `&str` 操作数上自动加 `&`。

---

### Bug-27: 字符串字面量返回类型不匹配

**代码**:
```lz
def step_export(report: str)-> str =
    result = std::fs::write("bootstrap_report.txt", report)
    "bootstrap_report.txt"
```

**生成 Rust**:
```rust
fn step_export(report: String) -> String {
    let mut result = std::fs::write("bootstrap_report.txt", report);
    "bootstrap_report.txt"  // 返回 &str，但函数签名要求 String
}
```

**Rust 编译错误**: `error[E0308]: mismatched types - expected String, found &str`

**分析**: lz 编译器在返回字符串字面量时，直接生成 `&str` 字面量，没有自动添加 `.to_string()` 转换。当函数返回类型声明为 `str`（即 Rust `String`）时，类型不匹配。

**影响范围**: 所有返回字符串字面量的函数（当返回类型为 `str` 时）。

**建议修复**: 当函数返回类型为 `str`/`String` 时，对字符串字面量自动追加 `.to_string()`。

---

### Bug-28: 桥接 `use` 语句生成但未使用（冗余导入）

**代码**:
```lz
import std::bridge::rust::std::fs
import std::bridge::rust::std::io
```

**生成 Rust**:
```rust
use std::fs;   // 警告: unused import
use std::io;   // 警告: unused import
```

**Rust 编译警告**: `warning: unused import: std::fs`

**分析**: 虽然 lz 代码中声明了 `import std::bridge::rust::std::fs`，但实际函数调用使用的是完整路径 `std::fs::read_to_string(...)`，因此顶层 `use std::fs;` 实际上是冗余的。Rust 编译器会发出未使用导入的警告。

**影响范围**: 所有通过 Rust 桥接导入的模块，当函数调用使用完整路径时。

**建议修复**: 当桥接的函数调用已使用完整路径时，不再生成顶层 `use` 语句；或者改为使用简短路径调用。

---

## 三、🟡 LZ 类型检查器问题

### Bug-29: 字符串拼接 `+` 运算符类型推断警告

**代码**:
```lz
result = "LZ Bootstrap Analysis\n"
result = result + "====================\n"
```

**LZ 编译器警告**: `Type error: cannot unify String with ...`

**分析**: lz 编译器对 `String + &str` 的类型推断发出警告，但实际生成的 Rust 代码 `result = result + "..."` 在 Rust 中是合法的（`String + &str` 通过 `Add` trait 支持）。这个警告是误报。

**影响范围**: 使用 `+` 运算符拼接字符串时。

**建议修复**: 修正 lz 类型检查器对 `String + &str` 的类型推断逻辑。

---

## 四、验证结果

### 桥接模块验证 ✅

| 功能 | LZ 编译 | Rust 生成 | 备注 |
|------|---------|-----------|------|
| `import std::bridge::rust::std::fs` | ✅ | `use std::fs;` | 正确 |
| `import std::bridge::rust::std::io` | ✅ | `use std::io;` | 正确 |
| `import std::bridge::rust::serde_json` | ✅ | `use serde_json;` | 正确（需 Cargo 依赖） |
| `std::fs::read_to_string(...)` 调用 | ✅ | 正确翻译 | Bug-25 需手动 unwrap |
| `std::fs::write(...)` 调用 | ✅ | 正确翻译 | Bug-25 需手动 unwrap |

### 控制流闭环 ✅（修复后）

修复 Bug-25/26/27 后，可执行文件成功运行：

```
╔══════════════════════════════════════╗
║  LZ Bootstrap - Control Flow Test   ║
║  Step 1: Import transpiler source   ║
║  Step 2: Process source code        ║
║  Step 3: Export analysis report     ║
╚══════════════════════════════════════╝

[Step 1] Importing transpiler source...
[Step 1] Done.

[Step 2] Processing source code...
=== Source Analysis ===
Content length: 10076
[Step 2] Done.

[Step 3] Exporting analysis report...
[Step 3] Done. Report saved to: bootstrap_report.txt

=== Control Flow Complete ===
```

控制流成功导入转译器 `src/main.rs`（10076 字节），处理后导出分析报告。

---

## 五、总结

本次自举第一步测试共发现 **5 个 Bug**（Bug-25 ~ Bug-29），其中：

- 🔴 严重 Bug: 3 个（Bug-25/26/27）— 导致生成的 Rust 代码无法通过 rustc 编译
- 🟡 中等 Bug: 1 个（Bug-29）— 类型检查器误报
- 🟢 轻微 Bug: 1 个（Bug-28）— 冗余 import 警告

**核心成就**：在手动修复 3 个代码生成 Bug 后，成功实现了「lz 程序 → 桥接导入转译器 Rust 源码 → 处理 → 导出」的完整控制流闭环。这证明了 lz 桥接模块的核心机制（import 路径映射、函数调用翻译）是正确工作的，问题集中在代码生成的后处理阶段（Result unwrap、字符串拼接、类型转换）。
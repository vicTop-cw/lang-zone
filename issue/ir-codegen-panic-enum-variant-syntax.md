# 🔴 P1: panic! 中枚举变体构造语法错误

**Bug ID**: N4
**严重等级**: 🔴 P1 — 生成不可编译的 Rust 代码
**发现日期**: 2026-07-31 20:45
**环境**: commit `b99448f`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// DEMO/10_error_handling/panic_raise_try.lz
enum AppError:
    NotFound(str)
    InvalidInput(str)
    PermissionDenied(str)

enum FileError:
    ReadError(str)
    WriteError(str)
    ParseError(int, str)

def find_user(id: int) -> str =
    if id <= 0:
        panic(AppError.InvalidInput("id must be positive"))
    "user_{id}"

def read_config(path: str) -> str =
    if path == "":
        panic(FileError.ReadError("empty path"))
    "config_data"
```

编译: `lang-zone panic_raise_try.lz` → `rustc panic_raise_try.rs`

## 实际结果

```rust
// panic! 参数类型错误
return panic!("{:?}", AppError::InvalidInput("id must be positive".to_string()));
// ❌ panic!("{:?}", ...) 期望 Display trait

// 枚举变体构造用 . 而非 ::
return panic!("{:?}", FileError.ReadError("empty path".to_string()));
// ❌ FileError.ReadError 应该是 FileError::ReadError
```

rustc 错误:
- `E0423: expected value, found struct 'AppError'` — panic! 参数类型问题
- `E0423: expected value, found struct variant` — 点号语法问题

## 预期结果

```rust
return panic!("AppError::InvalidInput: id must be positive");
// 或: return Err(AppError::InvalidInput("id must be positive".to_string()));
```

对于 LZ 的 `panic(enum_variant)` 语义，可能的正确 Rust 转换：
1. 生成 `panic!("{enum_name}::{variant}: {msg}")` 格式化字符串
2. 或者 LZ 的 `panic` 语义完全不同，需要特殊处理

## 根因分析

1. **枚举变体 `.` 语法**: `FileError.ReadError(...)` 在 codegen 某些路径中仍未转为 `FileError::ReadError(...)`
2. **panic! 参数**: LZ 中 `panic` 可以接受任意值（类似 Python raise），但 Rust 的 `panic!` 只接受格式化字符串
3. **codegen panic 路径**: `panic(expr)` → `panic!("{:?}", expr)` 的转换基本正确，但遇到枚举变体时 `{:?}` 需要 `Debug` trait，且语义可能非用户预期

## 影响范围

- `DEMO/10_error_handling/panic_raise_try.lz` — E0423
- `DEMO/10_error_handling/try_more.lz` — E0423  
- `DEMO/combo-syntax/combo_defer_guard_try.lz` — E0308

## 建议

方案 A: `panic(AppError.InvalidInput(msg))` → `panic!("AppError::InvalidInput: {msg}")`（格式化字符串）
方案 B: 给枚举自动 derive Debug，保持 `panic!("{:?}", variant)`

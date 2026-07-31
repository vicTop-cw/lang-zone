# 🔴 P1: IR codegen 泛型类型参数未正确生成

**Bug 标题**: IR 路线泛型函数 `<T>` 的 Rust 代码保留未解析的 `T`，且泛型调用语法错误

**严重等级**: 🔴 P1 — 泛型功能完全不可用
**发现日期**: 2026-07-31
**环境**: commit `6a85c17`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// generics.lz
def identity<T>(x: T) -> T = x

def clone_all<T>(items: List<T>) -> List<T> =
    let result = []
    for item in items:
        result.push(item)
    result

def main() =
    let cloned: List<T> = clone_all(["a", "b", "c"])  // 应推断为 List<str>
    let p = make_pair < str, str > ("x", "y")  // 显式泛型调用
```

编译: `lang-zone generics.lz` → 生成 .rs → `rustc generics.rs`

## 实际结果

```rust
// 生成的 generics.rs (异常)
let mut cloned: Vec<T> = clone_all(vec!["a".to_string(), ...]);  // ❌ T 未解析
let mut p2 = make_pair < str && str > ("x"...);                  // ❌ 语法错误
convert < int, str > (10, ...);                                   // ❌ 语法错误
```

rustc 错误:
- `E0425: cannot find type T in this scope`（clone_all 返回类型）
- `expected one of ...` (`<str && str>` 不是有效 Rust 泛型语法)

## 预期结果

```rust
// 应生成 (正确)
let mut cloned: Vec<String> = clone_all::<String>(vec!["a".to_string(), ...]);  // ✅ turbofish
let mut p2 = make_pair::<String, String>("x".to_string(), "y".to_string());     // ✅ turbofish
convert::<i64, String>(10, |n: i64| { format!("num: {n}") });                   // ✅ turbofish
```

## 根因分析

1. **泛型类型推断缺失**: `clone_all(["a", "b", "c"])` 调用时，IR builder 未根据实参类型推断 `T = str`，生成代码仍保留 `Vec<T>`
2. **泛型调用语法**: LZ 的 `func < T > (args)` 应转换为 Rust 的 `func::<T>(args)`（turbofish），但 IR codegen 直接输出了 `func < T > (args)`
3. **类型消解**: `str` → `String`，`int` → `i64` 的映射在泛型上下文中可能失效

## 影响范围

- `DEMO/04_functions/generics.lz` — 泛型核心 DEMO
- `DEMO/combo-syntax/combo_generic_struct_method.lz` — 组合泛型+方法

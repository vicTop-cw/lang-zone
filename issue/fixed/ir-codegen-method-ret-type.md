# 🔴 P1: IR codegen 方法调用返回类型推断错误

**Bug 标题**: IR 路线方法调用（push、length 等）返回类型推断错误，生成类型不匹配 Rust 代码

**严重等级**: 🔴 P1 — 多个 DEMO 文件 rustc 编译失败
**发现日期**: 2026-07-31 15:43
**环境**: commit `488718d`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
def modify(data: List<int>) -> int =
    data.push(42)

def main() =
    nums = [1, 2, 3]
    let len = nums.length()
    print(len)
```

编译: `lang-zone test.lz --emit=ir` → 生成 .rs → `rustc x.rs`

## 实际结果

```rust
// push() 返回 ()，但函数签名要求 int
pub fn modify(data: Vec<i64>) -> i64 {
    return data.push(42);
    //     ^^^^^^^^^^^^^ E0308: expected i64, found ()
}

// length() 不存在于 Vec（应为 len()）
pub fn main() {
    let len = nums.length();
    //            ^^^^^^ E0599: no method named `length`
}
```

1. **`push()` 返回类型**: LZ 中 `data.push(42)` 在表达式位置应返回 `data`（链式调用风格）或 `()`，但 IR codegen 直接映射 `Vec::push()` → `()`，与函数返回类型 `int` 冲突
2. **`length()` → `len()`**: IR codegen 未正确映射方法名（`length` 应重写为 Rust 的 `len`）

## 预期结果

- IR codegen 应正确处理链式方法调用的返回类型
- 方法名映射：`length` → `len`, `push` → `push` 且处理返回类型

## 根因

`src/ir/codegen.rs` 在生成方法调用时不查询方法实际返回类型，也不处理方法名映射。

## 影响范围

- `DEMO/04_functions/basic.lz` — `modify` 函数 push 类型不匹配
- `DEMO/02_types/containers.lz` — `nums.length()` 
- 所有使用容器方法调用并依赖返回类型的文件

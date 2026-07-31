# 🔴 P1: IR codegen 链式比较与大括号泛型语法错误

**Bug 标题**: IR 路线 `1 < 5 < 10` 链式比较和 `foo < T >` 泛型语法生成无效 Rust

**严重等级**: 🔴 P1 — 多个文件 rustc 编译失败
**发现日期**: 2026-07-31 15:43
**环境**: commit `488718d`, Windows, rustc 1.92.0

## 复现步骤

### 链式比较
```lz
def main() =
    let chain = 1 < 5 < 10
    print(chain)
```

### 泛型语法
```lz
def make_pair <T>(a: T, b: T) -> (T, T) = (a, b)
```

编译: `lang-zone test.lz --emit=ir` → 生成 .rs → `rustc x.rs`

## 实际结果

```rust
// 链式比较 — Rust 不允许
let chain: i64 = 1 < 5 < 10;
//               ^   ^ error: comparison operators cannot be chained

// 泛型语法 — 空格导致 Rust 解析为大括号
let mut p2 = make_pair < str > ("x", "y");
//           ^^^^^^^^^^^^^^^^^ error: comparison operators cannot be chained
```

## 预期结果

- 链式比较应拆分为 `1 < 5 && 5 < 10`
- 泛型调用应使用 turbofish `make_pair::<str>(...)`

## 影响范围

- `DEMO/05_expressions/operators.lz` — 链式比较
- `DEMO/04_functions/generics.lz` — 泛型调用
- 所有使用此语法的文件

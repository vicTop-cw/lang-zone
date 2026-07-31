# 🔴 P0: `in` 运算符错误生成为 `==` 等值比较

**Bug ID**: N3
**严重等级**: 🔴 P0 — 语义错误（成员测试变成等值比较）
**发现日期**: 2026-07-31 20:45
**环境**: commit `b99448f`, Windows, rustc 1.92.0, IR codegen

## 复现步骤

```lz
// DEMO/05_expressions/operators.lz
let in_str = "llo" in "hello"     // 子串测试 → 期望 true
let in_lst = 3 in [1, 2, 3]       // 成员测试 → 期望 true
let in_dict = "a" in {"a": 1}     // 键测试 → 期望 true
let in_set = 2 in {1, 2, 3}       // 成员测试 → 期望 true
```

编译: `lang-zone operators.lz` → `rustc operators.rs`

## 实际结果

```rust
let in_str: bool = "llo".to_string() == "hello".to_string();       // ❌ 等值比较 → false
let in_lst: bool = 3 == vec![1, 2, 3];                              // ❌ 等值比较
let in_dict: bool = "a".to_string() == HashMap::new();               // ❌ 等值比较
let in_set: bool = 2 == HashSet::from([1, 2, 3]);                   // ❌ 等值比较
```

所有情况都错误地生成了 `==`（等值比较），而不是 `.contains()` 成员测试。

## 预期结果

```rust
let in_str: bool = "hello".contains("llo");                       // ✅
let in_lst: bool = vec![1, 2, 3].contains(&3);                     // ✅
let in_dict: bool = HashMap::from([("a",1)]).contains_key("a");    // ✅
let in_set: bool = HashSet::from([1, 2, 3]).contains(&2);          // ✅
```

## 根因分析

`in` 运算符是 LZ 的成员测试运算符（Python 风格），但 IR codegen 将其统一映射为 `==` 二元比较，而非根据右操作数类型选择合适的 Rust 方法（`.contains()` / `.contains_key()`）。

## 影响范围

- `DEMO/05_expressions/operators.lz` — 直接失败
- 任何使用 `in` 运算符的代码

## 严重程度说明

语义级别的「编译器撒谎」—— `.contains()` 被替换为 `==`，对于大多数输入产生完全错误的布尔结果。子串测试 `"llo" in "hello"` 本应为 `true`，生成 `==` 后变成 `false`。

# 🔴 P1: IR builder elif 链条件逆序

**Bug 标题**: IR 路线 elif 链条件顺序反转，生成语义完全错误代码

**严重等级**: 🔴 P1 — 生成语义错误代码
**发现日期**: 2026-07-31 15:20
**环境**: commit `488718d`, Windows, rustc 1.92.0

## 复现步骤

```lz
def main() =
    x = 42
    if x > 100:
        print("big")
    elif x > 50:
        print("medium")
    elif x > 10:
        print("small")
```

编译: `lang-zone test.lz --ir-codegen`

## 实际结果 (IR)

```
if x > 50 then "medium" else if x > 10 then ... else if x > 100 then "big" else "small"
```

条件检查顺序: `x > 50 → x > 10 → x > 100`（完全逆序！）

## 预期结果

```
if x > 100 then "big" else if x > 50 then "medium" else if x > 10 then "small" ...
```

## 根因

`src/ir/builder.rs:430`:
```rust
for (elif_cond, elif_body) in elif_clauses.iter().rev() {
```

`.rev()` 将 elif 链反转。AST 中的 elif_clauses 已按源代码顺序排列（第一个 elif 在第一位），无需反转。

## 影响

所有含 `elif` 的代码在 IR 路线产生完全错误的控制流。例如分数等级判断会给出错误等级。

## 修复建议

去掉 `.rev()`:
```rust
for (elif_cond, elif_body) in elif_clauses.iter() {
```

## 相关

- 测试报告: `issue/test-report-2026-07-31-1520.md`

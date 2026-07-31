# 🔴 P1: IR codegen 多种模式/类型映射错误汇总

**Bug 标题**: IR 路线多种模式解构、类型映射、关键字语法错误导致 rustc 编译失败

**严重等级**: 🔴 P1 — 影响 ~10 个 DEMO 文件
**发现日期**: 2026-07-31 16:10
**环境**: commit `3101d07`, Windows, rustc 1.92.0, IR codegen（默认路线）

## 子问题列表

### A. match 臂模式解构变量丢失
```lz
Circle(_, _, r): 3.14 * r * r
```
**生成**: `return 3.14 * r * r;` — `r` 从未绑定
**rustc**: `error[E0425]: cannot find value 'r'`
**影响**: enum.lz, match.lz

### B. Result/Error 模式前缀错误
```lz
Ok(Some(v)): ...
```
**生成**: `Error::Ok(Error::Some(v)) => { ... }` — `Error::` 前缀非法
**rustc**: `error[E0433]: cannot find type 'Error'`
**影响**: mutable_let.lz, extractor_unapply.lz

### C. mut 参数语法错误
```lz
def modify(data: mut List<int>) -> int
```
**生成**: `pub fn modify(data: mut Vec<i64>) -> i64`
**rustc**: `error: expected type, found keyword 'mut'`
**影响**: basic.lz

### D. 递归类型未 Box
```lz
enum Expr: Add(Expr, Expr) | Sub(Expr, Expr)
```
**生成**: 直接递归，无 Box 包装
**rustc**: `error[E0072]: recursive type has infinite size`
**影响**: enum_more.lz

### E. Vec 缺泛型参数
**生成**: `const items: Vec = vec![...]` — 缺少 `<_>`
**rustc**: `error[E0107]: missing generics for struct 'Vec'`
**影响**: operators.lz

### F. Null coalesce 类型不兼容
**生成**: `if cond { None } else { "value" }` — None 与 String 类型不同
**rustc**: `error[E0308]: 'if' and 'else' have incompatible types`
**影响**: null_coalesce.lz

## 整体根因

IR codegen 在多个转换路径上尚未处理 LZ 语义与 Rust 语义的差异：
1. 模式解构未生成变量绑定
2. Result 类型路径映射使用错误的路径前缀
3. 参数修饰符 `mut` 位置错误
4. 递归类型需要智能指针包装
5. 泛型参数推断不完整
6. Option 类型需要显式类型标注

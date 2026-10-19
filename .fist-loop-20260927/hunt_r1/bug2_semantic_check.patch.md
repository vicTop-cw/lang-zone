# BUG-2 修复补丁（src/semantic_check.rs）— 待重放

> 状态：**已被并发会话回滚**（2026-09-28 02:12–02:43 间 `src/semantic_check.rs` 被还原至 HEAD，
> 同窗口 `src/ir/codegen_cython.rs` 于 02:08:56 被外部修改）。补丁与验证结论留档，待人工协调后重放。

## 补丁 1／2 — 内建名白名单补齐（`builtin_value_names`，原第 154 行 "inspect" 之后）

```rust
        "collect",
        "inspect",
        // lzc-loop-20260927 R1-修复：G2 未定义函数检查补齐所需的内建入口
        // （实测 DEMO/corpus 312 个 .lz 全量扫描得到的被调用内建名，
        //   这些名此前由 typer/codegen 特判解析，未进入本白名单）
        "println",
        "format",
        "contains",
        "push",
        "sort",
        "reverse",
        "hash",
        "Exception",
        "__as__",
```

## 补丁 2／2 — `Expr::Call` 分支补未定义函数检查（原第 1097 行起）

原：

```rust
                // func 位置不检查 Ident（允许调用尚未显式绑定的函数/宏名）
                if !matches!(func.as_ref(), Expr::Ident(_)) {
                    self.check_expr(func);
                }
                // G2: 同模块函数调用参数个数不匹配（`def f(a); f(1,2)`）→ 报错
                if let Expr::Ident(name) = func.as_ref() {
                    let sig = self.fn_sigs.get(name).map(|s| {
```

改：

```rust
                // func 位置不检查子表达式（Ident 走下方名字解析）
                if !matches!(func.as_ref(), Expr::Ident(_)) {
                    self.check_expr(func);
                }
                // G2: 同模块函数调用参数个数不匹配（`def f(a); f(1,2)`）→ 报错
                if let Expr::Ident(name) = func.as_ref() {
                    // G2 补缺口：未定义函数此前静默通过，codegen 退化为
                    // `fn NAME(..) -> i64 { i64::MAX }` 桩（src/ir/codegen/mod.rs 未知外部函数桩），
                    // 运行得到错误结果而非编译报错。与「未绑定变量」口径对齐：
                    // 内建值/类型、import 符号、本模块函数名、自定义类型、枚举（含变体）、
                    // 局部变量（闭包值）均已由 is_bound 覆盖；fn_names 在 collect 阶段全量收集，
                    // 故同模块前向引用不受影响。
                    if !self.is_bound(name) {
                        self.error(format!("未定义函数: {name}"));
                    }
                    let sig = self.fn_sigs.get(name).map(|s| {
```

## 验证结论（补丁在位时实测，2026-09-28 02:12–02:20）

- `DEMO` + `corpus` 共 **312** 个 `.lz` 全量编译扫描：补齐白名单后 **Semantic error 归零**（无误判）。
  补齐前误判 9 个内建名：`__as__ / contains / Exception / format / hash / println / push / reverse / sort`。
- 最小复现 `def main() = println(foo())` → `IR build error: Semantic error: 未定义函数: foo`（修复前为静默通过并输出 9223372036854775807）。
- 反向用例（新增闸门 `tests_lzc_loop_semantic_check.rs.pending`）：已定义函数调用 `add(40,2)` → 42；
  前向引用 `later()` → 7；二者**均通过**，确认不误伤。
- 未及跑全量 `cargo test` 门禁（补丁随即被回滚）→ **不可判定为已交付**。

## 重放步骤

1. 应用上述两处补丁；
2. 复制 `tests_lzc_loop_semantic_check.rs.pending` → `tests/lzc_loop_semantic_check.rs`；
3. `cargo test --workspace --no-fail-fast -j 2`（门禁：≥748 passed / 0 failed，测试数只增不减）；
4. 注意：本仓库存在并发会话同时修改 `src/**`（本次即因此回滚），重放前先确认工作区无他人进行中改动。

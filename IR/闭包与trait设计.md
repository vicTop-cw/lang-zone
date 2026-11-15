# 闭包与 trait IR 设计（设计会 0.0 产出）

> 日期：2026-09-06 ｜ 归属：bootstrap/07-自举推进计划-2026Q4.md 阶段 0 的 0.0 设计会
> 目标：一次性裁定 TY-001 / IR-001 / IR-002 / IR-003 四个设计相关 bug 的统一 IR 表示，
> 避免分散修带来重复劳动。所有结论均面向唯一 IR 路线（`.lz → LZIR → Rust`）。

---

## 0. 背景与铁律

- 闭包捕获（IR-003）与 `fn` 形参类型（解析已贯通，见 §4）是「用 LZ 写编译器」的前提，
  属自举强依赖。未闭转译缺口不进阶段 2/3。
- 约束：后端即 Rust，最小可信基座 = Rust toolchain，无法脱离 `rustc`。
- 不重新引入已删除的「路线 B LZ 写前端/codegen」替代线路。

---

## 1. BUG-TY-001：duck 自引用参数 → `&dyn`（E0391 修复）

**现象**：`duck Comparable = def __lt__(self, other: Comparable)` 生成
`trait Comparable { fn __lt__(&self, other: Comparable) }`，自引用非 dyn 兼容 → E0391。

**裁定**：trait 方法参数若引用 duck 自身类型，生成 `&dyn Comparable`（对象安全）。
全局映射规则：
- 每个 `duck X { ... }` → 一个 Rust `trait X`。
- duck 方法签名中出现的 `X`（自身名）→ IR 类型 `&dyn X`；方法首个参数 `self` → `&self`。
- duck 作为值/变量类型（非方法参数）→ 生成对应的胖指针/结构体包装（沿用现有 duck 值路径）。

**codegen 模板**：
```rust
trait Comparable {
    fn __lt__(&self, other: &dyn Comparable) -> bool;
}
```

---

## 2. BUG-IR-003：嵌套 `def` 闭包捕获提升

**现象**：内层 `def` 捕获外层参数 → 生成 `static mut` + 全局提升，参数遮蔽冲突 E0530，
且捕获语义静默错（应为闭包捕获而非全局共享）。

**裁定**：采用 `move` 闭包 + 捕获结构体，不使用全局提升。
- 嵌套 `def` 在 IR 中表示为「捕获外层自由变量的闭包」。
- 捕获变量收集：扫描内层函数体引用的、定义在外层作用域的变量，生成捕获结构体
  `struct __Capture_{n} { x: T, ... }`，闭包体通过 `&self`/字段访问捕获变量。
- 短期（本期）若捕获表达性不足，先报明确错误「嵌套 def 暂不支持捕获非局部变量」，
  避免静默生成错误产物（沿现有 codegen 的 `box_lambda`/`nested_fn_ret` 框架扩展）。

**IR 表示**：
```
Closure { params, ret, captures: Vec<Capture>, body: Block }
Capture { name, ty, by_ref: bool }
```

---

## 3. BUG-IR-001：`~:` 参数位脱糖

**现象**：`nums.filter(~: _ % 2 == 0)` 报「Unexpected token in expression: BuildCall」，
`~:` 仅支持值位不支持参数位。

**裁定**：`filter(~: BODY)` 脱糖为 `filter(|__arg| { BODY with _ → __arg })`。
- 解析层：识别参数位 `~:` → 生成 `Expr::Lambda { params: [__arg], body }`，
  并将体中的 `_`（匿名参数）重写为 `__arg`。
- 仅当 `~:` 出现在「期望单参回调」的位置（filter/map/for_each 等方法首参）时启用。

**codegen 模板**：
```rust
nums.filter(|__arg| { __arg % 2 == 0 })
```

---

## 4. BUG-IR-002：defer 作用域内联展开（方案 A，已落地 + 精修）

**裁定**：采用「作用域内联展开」（方案 A），已落地（`deferred: Vec<Block>`，
块退出前逆序 `flush_deferred`，规避闭包捕获 E0499）。`ir002_defer_guard` 已转绿。

**已落地语义**：
- `defer` 体收集到 `deferred`，所属块（gen_block_inner / Stmt::Block）退出前 LIFO 内联 emit。
- 含 defer 的块抑制尾语句 `return`（force_stmt_semicolon），使 cleanup 可达。

**精修项（非阻塞）**：早 `return`（尤其嵌套块内）当前不触发 defer 展开。
- 本会话实测：顶层早 `return` 经「块入口预扫描 defer 入栈」可正确 flush；但 `if` 内早 `return`
  不 flush 外层 defer——根因是分支块（`if/for/while/else` 经 `ExprKind::IfExpr` →
  `gen_expr(then_block)` → `gen_block_inner`）的 `deferred` 收集/隔离路径与函数体不一致，
  外层 defer 在分支生成时已脱离当前 `deferred` 栈。该修复需改 `deferred` 为「作用域帧栈
  `Vec<Vec<Block>>`」并在 `return` 处统一 flush，且须完整回归 `find_bug_bugs`（Windows 内存受限
  须 `-j 1` 单目标跑，见 memory）。**属非阻塞精修，留待自举准备阶段实施，本期不动以免动摇绿基线。**
- 注：方案 A 主体（块退出前内联展开）已落地，`ir002_defer_guard` 等已转绿；本精修仅影响「早 return 跨作用域 flush」。

**codegen 模板**：
```rust
{                       // 块进入
    let mut __log = Vec::new();
    push(&mut __log, "start");
    // defer guard 收集于此
    push(&mut __log, "end");
    // 块退出前 LIFO 内联：
    push(&mut __log, "cleanup");
}                       // 正常退出
// 早 return 路径：return 前同样 flush 上述 defer
```

---

## 5. fn 形参类型（解析已贯通，记录备查）

`parse_type` 已支持 `fn(A, B) -> C` 函数类型（src/parser/parser.rs 1462–1483），
`from_ast_type` 已映射 `AstType::Fn → IrType::Fn`（src/ir/types.rs:219），
typer 亦支持 `Type::Fn`。故「`fn(A,B)->C` 作形参类型」**解析与 IR 类型贯通已完成**。

**剩余关联缺口**（阻塞 core 三例的真正原因，非 fn 解析）：
- **未声明泛型形参** `a`/`b`：`fold(xs: List<a>, ...)` 报「未知类型: a」。
  fold.lz/compose.lz/unique.lz 以隐式泛型写法定义，编译器当前要求显式 `<a,b>` 声明
  或支持 Hindley-Milner 式隐式泛型推断。属设计级决策，需在自举准备阶段裁定
  （建议：支持未声明类型名作为隐式泛型形参，与测试样例写法对齐）。
- **`assert cond, "msg"`** message 未解析（见下方 §6）。

---

## 6. 关联缺口：assert 消息语法

规范 SYNTAX/15-测试框架.md §六 明确支持 `assert expr` 与 `assert expr, "错误消息"`。
当前 parser 仅把 `assert a == b` 拆为相等断言（assert_eq!），未解析 `, msg`。

**裁定**：`assert cond, msg` 中第二操作数为**消息串**（非等号 RHS）。
- AST `Stmt::Assert` 增加 `message: Option<Expr>` 字段（与 IR `Stmt::Assert { cond, message }` 对齐）。
- parser：出现 `,` 时不拆分 `==`，整体作为条件，解析 message。
- builder：message 形式产出 `Stmt::Assert { cond, message }`；codegen 生成
  `assert!(cond, "{:?}", msg)`。相等形式 `assert a == b` 行为保持不变（assert_eq!）。

---

## 7. 回归用例

| Bug | 验证用例 | 预期 |
|-----|----------|------|
| TY-001 | `DEMO/` duck 自引用 | 生成 `&dyn`，rustc 通过 |
| IR-003 | 嵌套 def 捕获外层变量 | 闭包捕获，非全局提升 |
| IR-001 | `filter(~: _ % 2 == 0)` | 脱糖为 `|__arg| ...` |
| IR-002 | `defer` + 早 `return` | return 前 LIFO 执行 defer |
| assert | `assert x == 1, "msg"` | `assert!(x == 1, "{:?}", "msg")` |

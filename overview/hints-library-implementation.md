# hints — 类型自推断基础库实施总结

> 时间：2026-07-23 | 阶段：类型自推断系统 P0 基石 | 状态：✅ 完成，全部测试通过

## 1. 目标

按 `overview/type-inference-design-research.md` 的 P0 路线图，为 Lang-Zone 建立**类型自推断基础库** `hints`，
为后续 `auto` / `_` / `infer` 占位符与 HM 式推断提供可复用的底层原语。

设计锚点（来自调研报告）：
- **Haskell HM Algorithm W + occurs-check** — 递归推断终止第一保险
- **rustc / ena union-find** — 推断变量用并查集管理等价类
- **Zig comptime runaway 上限** — 深度预算（P1 扩展点，已在 solver 注释预留）

## 2. 模块结构

```
src/hints/
├── mod.rs      模块入口：声明子模块 + re-export（InferCtx/TyVar/TypeError/unify/Constraint/solve/zonk）
├── tyvar.rs    InferCtx（union-find）+ Link{Var,Bound,Unbound} + TypeError
├── unify.rs    Robinson 一阶统一算法 + occurs-check
├── constraint.rs  Constraint::Eq 约束式
├── solver.rs   solve() 约束管道（fail-fast）
└── subst.rs    zonk() 求解后完全替换 + is_resolved()
```

注册：`src/lib.rs` 在 L3 层新增 `pub mod hints;`

类型孔：`src/types/def.rs` 的 `Type` 枚举新增 `Type::Var(TypeVar)`，`to_rust_type_string` 输出 `_ /* TyVar#n */`。

## 3. 核心 API

| 组件 | 关键方法 | 说明 |
|------|----------|------|
| `InferCtx` | `fresh(level)` / `fresh_ty(level)` | 分配推断变量（带泛化层级） |
| | `find(v)` | union-find 路径压缩 |
| | `bind(v, t)` | 绑定变量到类型 / 链接两变量 |
| | `resolve(v)` | 取变量已绑定内容（如已 Bound） |
| | `occurs(v, t)` | occurs-check，拒 `α = List<α>` |
| `unify` | `unify(ctx, a, b)` | Robinson 统一，失败返回 `TypeError` |
| `solve` | `solve(ctx, cs)` | 对约束集顺序统一，首个失败即返回 |
| `zonk` | `zonk(ctx, t)` | 完全解析（遍历替换所有推断变量） |

`TypeError` 四态：`Occurs` / `Mismatch` / `Arity` / `Unbound`。

## 4. 本次修复的两个关键问题

### 4.1 测试构建打通（bridge 旧 API 引用）
- **stale incremental cache**：prior session 仅 `touch lib.rs`，bridge 模块沿用旧诊断缓存误报 ~55 个错误。
  重新 `touch src/bridge/{std,source,core}.rs` 后，真实错误收敛到 10 个。
- **linter 自动修复**：`StdBridge`/`SourceBridge` 固有 `resolve_call` → `resolve_call_impl`（避免遮蔽 trait 的 2 参 `resolve_call`）；
  测试代码 `registry.resolve_import(...)` 返回 `String`（非 `Option`），`.unwrap()` 已移除；
  `result.is_some()` / `result.unwrap()` 在 `resolve_import` 已 `.unwrap()` 后得到具体类型处已删除。

### 4.2 `unify` 绑定覆盖 bug（真缺陷，已修复）
**复现**：`a = Int; b = a;` 之后求 `b = Bool` 被静默接受。

**根因**：变量统一分支在 `occurs-check` 后直接 `bind(v, t)`。而 `bind` 内部 `find(v)` 找到 `b` 的代表 `a`，
把 `a` 的 `Bound(Int)` **覆写**为 `Bound(Bool)`，未触发冲突。

**修复**（`src/hints/unify.rs` 变量分支）：
```rust
let rv = ctx.find(v);
match ctx.resolve(rv) {
    Some(bound) => unify(ctx, &bound, &t),  // 已绑定 → 统一其内容，触发 Mismatch
    None => { ctx.bind(v, t); Ok(()) }       // 仍自由 → 正常绑定
}
```
修复后 `solve_constraints_success_and_failure` 测试在 `b` 已解析为 `Int` 时正确拒绝 `b = Bool`。

## 5. 验证结果

```
cargo test  →  304 unit + 1 integration = 305 全部通过，0 失败
```

hints 库 12 个单元测试覆盖：
- 变量分配 / 绑定解析 / 三变量传递性
- 具体类型相等 / 函数类型 / 泛型容器（List<int>）
- **occurs-check 拒无限类型** `α = List<α>` 与嵌套自引用 `(int, List<α>)`
- 变量链经 List 解析 / 约束求解成功与失败
- `Any` 与一切统一 / `Never` 与一切统一（协变兜底）

既有 276+ 单元测试零回归。

## 6. 后续 P1 扩展点（预留，本次未实现）
- `solver.rs` 递归展开深度预算（对齐 Zig `@setEvalBranchQuota` / TS `--recursiveTypeDepth`）
- `let` 泛化（利用 `fresh` 的 `level` 参数做 HM 泛化）
- 暂定缓存 / SLG tabling（复杂约束求解加速）
- 与 codegen 解耦的 `typer/` 上层（collect → generate → solve → substitute 完整管线）

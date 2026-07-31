# typing 库实施总结

## 概述

`src/typing/` 类型判定库——判断「类型符合不符合」的完整方案，建基于 `hints` 推断层之上。

## 架构

```
src/typing/
├── mod.rs          # 模块入口，re-export 全部公开 API
├── errors.rs       # TypingError 枚举（Conformance/Arity/MissingMethod/SignatureMismatch/UnresolvedVar）
├── relate.rs       # conforms(sub, sup) — 子类型包含关系（基于类型格）
├── variance.rs     # Variance 枚举 + variance_of(ty, param) — 变型计算
├── traits.rs       # TraitReq/TraitEnv/MethodProvider/satisfies — trait 方法满足性
├── magic_bind.rs   # MagicEngine → TraitEnv 桥接（魔法方法映射为 trait 要求）
└── tests.rs        # 23 个单元测试
```

## 子类型包含（`relate::conforms`）

基于类型格的判断引擎，借助 `hints::InferCtx::prune` 消解推断孔：

| 规则 | 覆盖 |
|---|---|
| **Any 顶** | 任何类型 <: Any |
| **Never 底** | Never <: 任何类型 |
| **自反** | T <: T |
| **名义不匹配** | Option<Int> 不 <: Result<Int, Err> |
| **泛型协变** | Option<Int> <: Option<Any>, Vec<String> <: Vec<Any> |
| **Result 双参协变** | Result<Int, Err> <: Result<Any, Any> |
| **函数子类型** | 参数逆变 + 返回协变 |
| **引用变型** | `&T` 协变, `&mut T` 不变 |
| **元组** | 逐元素协变, Never 元素 |
| **变量消解** | Type::Var → prune → 继续判定或报 UnresolvedVar |

## Trait 满足性（`traits::satisfies`）

通过 `MethodProvider` 抽象与 AST 解耦：

- `TraitReq { name, methods: Vec<MethodReq> }`
- `MethodReq { name, params, ret }` — 空 params = 仅检查存在性
- `MethodProvider` trait：`has_method(ty, name) -> bool` + `method_sig(ty, name) -> Option<(Vec<Type>, Type)>`
- `MemProvider`：内存 HashMap 实现（测试用）
- `satisfies(ctx, ty, req, provider)`：方法存在性 + 签名符合性（参数类型逆变、返回协变）
- 签名匹配：仅当 `MethodReq.params` 非空时才校验参数类型与 arity

## Magic 桥接（`magic_bind`）

将 `crate::magic::MagicEntry` 映射为 `TraitReq`：

- `trait_req_from_magic(magic_method, entries)` — 聚合同一 trait_path 的全部魔法方法
- 跳过 Rust 自动推导的 trait（如 Deref/TryInto/Into — 已有 blanket impl）
- `satisfies_magic(ctx, ty, magic_method, engine, provider)` — 一键魔法满足性检查

## 测试覆盖（23 个）

- 子类型：reflexivity, any_is_top, never_is_bottom, generic_covariance, option_covariance, result_covariance, function_subtyping, shared_ref_covariant, mut_ref_invariant, tuple_conformance_with_never, nominal_mismatch, generic_arity_mismatch, unresolved_var_errors
- Trait 满足性：mem_provider_satisfies_trait, signature_conformance_ok, signature_mismatch_errors, unknown_trait_errors
- Magic 桥接：magic_register_and_satisfy, magic_unknown_method
- 变型：variance_vec_covariant, variance_mut_ref_invariant, variance_fn_bivariant_is_invariant, variance_irrelevant_when_absent

## 工作树修复（并行完成）

- **macros 测试代码破损**：`interp.rs` 补 `use crate::macros::Delimiter`；`expand.rs` 的 `MacroExpander::new` 补第 2 参数 `TemplateRegistry::new()`；`extract_macro_defs` 补 `true` 参数
- **comptime 模块**：`_wip_full.rs` → `mod.rs` 激活。原 WIP 基于旧版 AST（`Stmt::If` 已移到 `Expr`），整模块被 linter 替换为 43 行编译桩
- **codegen `gen_module_magic`**：stale 缓存导致的误报，`touch` 后消失
- **scoped enum 错误**：已全部修复，回归 0

## 验证结果

```
cargo test  →  330 unit + 1 integration = 331 全部通过，0 失败
        typing  →  23/23 通过
        hints    →  12/12 通过
        既有套件  →  296/296 通过（零回归）
```

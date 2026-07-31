//! Lang-Zong 类型判定库 —— `typing`
//!
//! 在 [`crate::hints`]（类型自推断）之上提供 **类型符合性判定** 两层能力：
//!
//! ## 1. 子类型 / 包含关系 [`relate::conforms`]
//! 基于类型格（`Any` 顶 / `Never` 底），判断 `sub` 能否用于期望 `sup` 的位置。
//! 覆盖泛型协变、函数子类型（参数逆变、返回协变）、引用变型
//! （`&T` 协变、`&mut T` 不变）、容器递归。
//!
//! ## 2. Trait / 方法满足性 [`trait::satisfies`]
//! 判断某类型是否拥有某 trait 要求的全部方法，并可对方法签名做符合性检查。
//! 通过 [`MethodProvider`] 抽象与 AST / codegen 解耦；提供内存实现
//! [`MemProvider`] 供测试与轻量查询。[`magic_bind`] 将 `magic` 引擎的
//! 魔法方法映射为 trait 要求，打通两者。
//!
//! ## 3. Type Class 实例注册与推导 [`instance`]
//! [`InstanceRegistry`] 收集模块中的 `trait`/`impl` 定义，
//! [`resolve_instance`] 支持精确、泛型替换、容器递归派生三种实例解析。
//!
//! ## 与 `hints` 的关系
//! `typing` 复用 `hints::InferCtx` 消解推断孔（`conforms` 内部先 `prune` 变量），
//! 因此应在 **zonk 之后、codegen 之前** 调用。
//!
//! ## 示例
//! ```text
//! 解析 AST → hints 收集约束 → solve(unify) → zonk(替换)
//!         → typing::conforms 判定赋值/传参符合
//!         → typing::satisfies 判定 trait 实现完整
//!         → typing::resolve_instance 解析隐式实例
//!         → codegen
//! ```

mod errors;
mod relate;
mod variance;
mod traits;
mod magic_bind;
mod bounds;
mod instance;
mod exhaustive;

pub use errors::TypingError;
pub use relate::conforms;
pub use variance::{Variance, variance_of};
pub use traits::{MethodReq, TraitReq, TraitEnv, MethodProvider, MemProvider, satisfies};
pub use magic_bind::{trait_req_from_magic, register_magic_traits, satisfies_magic};
pub use bounds::check_trait;
pub use instance::{Instance, InstanceKey, InstanceKind, InstanceRegistry, resolve_instance};
pub use exhaustive::check_exhaustive;

#[cfg(test)]
mod tests;

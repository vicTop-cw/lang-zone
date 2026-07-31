# IR 顶层指令层完整规划

- **Status**: Proposal
- **Date**: 2026-07-30
- **Decision needed**: 确认字段后动手

## 当前问题

信息散落在 5 个地方，codegen 做决策时要跨多个结构体查找：

```
MagicAttrs.name/doc/bridge    →  不确定性：backend 在哪？
FnDef.is_async                →  单独字段，跟 iterator 不统一
FnDef.intrinsics              →  Vec<Intrinsic>，不整齐
Item::Test                    →  Test 是 Item 变体，但 macro 没有变体
IrModule.prelude              →  Vec<String>，太弱
```

## 目标：统一为 ModuleDirective + tidy Item

```
IrModule {
    name: String,
    directive: ModuleDirective,   ← 所有顶层指令
    items: Vec<Item>,              ← 干净的列表
}
```

### ModuleDirective 字段

```rust
pub struct ModuleDirective {
    // ── 后端目标（codegen 选择器）──
    pub backend: Backend,           // Rust（默认）| Cython | Wasm

    // ── 模块类型 ──
    pub kind: ModuleKind,           // Normal | Macro | Template | Prelude | Test

    // ── 桥接 ──
    pub bridge: Option<String>,     // "rust" | "python" | "ffi" | "cli"
    pub bridge_tier: Option<String>,

    // ── 元信息 ──
    pub name: Option<String>,       // __name__
    pub file: Option<String>,       // __file__
    pub package: Option<String>,    // __package__
    pub doc: Option<String>,        // __doc__

    // ── 可见性 ──
    pub public: Vec<String>,        // __all__ 或 __public__
    pub private: Vec<String>,       // __private__
    pub deps: Vec<String>,          // __deps__

    // ── 编译选项 ──
    pub no_std: bool,
    pub extern_crates: Vec<CrateDep>,  // 外部 crate 依赖
}

pub enum Backend { Rust, Cython, Wasm }
pub enum ModuleKind { Normal, Macro, Template, Prelude, Test, Inline }
pub struct CrateDep { pub name: String, pub version: Option<String> }
```

### Item 整理 — 增 MacroDef，统一 iterator/async

```rust
pub enum Item {
    FnDef(FnDef),         // 普通函数 — is_async + intrinsics 判断
    StructDef(StructDef),
    EnumDef(EnumDef),
    TraitDef(TraitDef),
    Impl(ImplDef),
    Use(UseStmt),
    Const(ConstDef),
    MacroDef(MacroDef),   // ← 新增，收编 template + macro
}

// FnDef 已支持:
//   is_async: bool         — async fn
//   intrinsics: Vec<Intrinsic> — Iterator, Inline, Export...
//   is_iterator: bool 通过 intrinsics 判断
//   is_test 标记也通过 intrinsics 表达（移出 Item::Test）

pub struct MacroDef {
    pub name: String,
    pub kind: MacroKind,       // Template | Rule | Proc
    pub params: Vec<Param>,    // macro 参数
    pub ret_ty: IrType,        // 返回类型（通常 Tokens）
    pub body: String,          // 原始 token 文本（不展开）
}

pub enum MacroKind { Template, Rule }
```

## codegen match 分支（简化为 1 个 match）

```rust
// codegen.rs — 入口
fn generate(module: &IrModule) -> String {
    match module.directive.backend {
        Backend::Rust => gen_rust(module),
        Backend::Cython => gen_cython(module),
        Backend::Wasm => gen_wasm(module),
    }
}
// codegen 内部:
fn gen_rust_item(item: &Item) -> String {
    match item {
        Item::FnDef(f) if f.is_iterator() => gen_iterator(f),
        Item::FnDef(f) if f.is_async      => gen_async_fn(f),
        Item::FnDef(f)                    => gen_fn(f),
        Item::MacroDef(m)                 => gen_macro(m),
        Item::StructDef(s)                => gen_struct(s),
        // ... 基本节点
    }
}
```

## 缺失清单（对比 AST/SPEC）

| 特性 | AST 有? | IR 有? | 缺口 |
|------|:--:|:--:|------|
| `iterator` | 🟡 99_spec | ❌ | 需 FnDef.iterator intrinsic |
| `template` | 🟡 99_spec | ❌ | 需 Item::MacroDef |
| `#!bin macro` | ✅ parser | ❌ | 需 ModuleKind::Macro |
| `async/await` | 🟡 | ✅ | is_async 已就绪 |
| `defer` | ✅ | ✅ | 已降级为 Block |
| `@export` | ✅ parser | 🟡 | 需 Intrinsic::Export |
| 后端选择 | ❌ | ❌ | 需 ModuleDirective.backend |
| `no_std` | ❌ | ❌ | 需 ModuleDirective.no_std |
| 外部 crate | ❌ | ❌ | 需 ModuleDirective.extern_crates |

## 实施步骤

1. **node.rs**: 新增 `ModuleDirective` + `MacroDef` + `MacroKind` + `CrateDep`，删除 `Item::Test`（合并到 FnDef via intrinsics），扩展 `IntrinsicKind`
2. **mod.rs**: 重构 `IrModule`，`magic` → `directive`
3. **builder.rs**: AST Module → IrModule 时填充 directive
4. **codegen.rs**: 用 directive 驱动分派
5. **display.rs**: 同步更新
6. 跑 `cargo test --lib` 确保 286+

## 可扩展性

后续加新指令只加字段，不动 match 结构：
```
后续: link, feature, edition, optimize, ...
→ 全塞 ModuleDirective，codegen 只读
```

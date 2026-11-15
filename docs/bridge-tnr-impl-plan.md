# LZ Bridge 接入 Tnr 实现计划

## 一、背景

当前 LZ 与 Tnr 的双向集成仅完成了 embed（内嵌）方向：
- 计划一 T0：`#[embed(tnr)]` → tnr 代码原样嵌入 → 转译为 Rust 模块 ✅
- 计划二 T0r2：Tnr 侧 `#[extern(lz)]` + `import lz::` → Tnr 调用 LZ 库 ✅

**缺失**：LZ 侧的 bridge（桥接）未接入 codegen import 路由。
- `BridgeRegistry` + 6 个桥接实现已存在，但 `gen_use_stmt` 未调用 `bridge_registry.resolve_import`
- `generate_with_bridge` 创建空 registry，不注册任何桥接
- 无 TnrBridge

## 二、目标

LZ 导入时自动路由区分两种来源：
1. **内嵌代码导入**：embed 生成的模块（如 `foo_tnr`）→ `use foo_tnr::*;`
2. **桥接导入**：通过 bridge registry 路由到 TnrBridge 等 → `use tnr::func;` 或转译

## 三、形式

### 3.1 内嵌导入（embed 模块）

```lz
#[embed(tnr)]
fn matmul(a, b) {
    a @ b
}

import foo_tnr::*       // ← 自动路由到 embed 模块
let result = matmul(x, y)
```

### 3.2 桥接导入 — tnr lib API 转译

```lz
import tnr::matmul      // ← TnrBridge 转译为 Rust 函数
let result = matmul(x, y)
```

### 3.3 桥接导入 — 已编译 tnr 库

```lz
import tnr_lib::tensor::Tensor   // ← 直接 use tnr::tensor::Tensor
let t = Tensor::zeros([3, 3])
```

### 3.4 桥接导入 — tnr 计算后端

```lz
import tnr::compute::kernel     // ← 生成 tnr 计算委托 shim
let result = kernel(data)
```

## 四、四阶段计划

### P1：embed 模块导入路由（易，~30 行）

**目标**：`import foo_tnr::*` 能正确生成 `use foo_tnr::*;`

**改动**：
| 文件 | 改动 |
|------|------|
| `ir/embed_collector.rs` | 增加 `module_names(source_name) -> Vec<String>` |
| `ir/codegen/mod.rs` | `IrCodeGen` 加 `embed_modules: HashSet<String>`；`generate` 收集 embed 模块名；`gen_use_stmt` 识别 embed 模块 |

### P2：新建 TnrBridge（中，~120 行）

**目标**：三种 tnr 桥接语义

**改动**：
| 文件 | 改动 |
|------|------|
| `bridge/tnr.rs`（新建） | `TnrBridge` 实现 `Bridge` trait，路由前缀 `tnr::` / `tnr_lib::` / `tnr::compute::` |
| `bridge/mod.rs` | 注册 `pub mod tnr;` |

### P3：codegen gen_use_stmt 接入 bridge_registry（中，~50 行）

**目标**：自动路由——embed → 内嵌，bridge registry → 桥接，白名单 → std

**改动**：
| 文件 | 改动 |
|------|------|
| `ir/codegen/mod.rs` | `gen_use_stmt` 三层路由；`generate_with_bridge` 注册默认桥接 |

### P4：主流程注册 + 测试（易，~40 行）

**改动**：
| 文件 | 改动 |
|------|------|
| `main.rs` | 注册 TnrBridge |
| `tests/bridge_import.rs`（新建） | 4 个端到端测试 |

## 五、量级

~240 行 / 5 文件（新建 2，修改 3）

## 六、依赖

- P1 独立
- P2 独立
- P3 依赖 P1 + P2
- P4 依赖 P3

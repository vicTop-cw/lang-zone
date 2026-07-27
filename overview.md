# Bridge Core API 扩展 — 概览

## 设计思路

当前 Bridge trait 有 13 个方法，BridgeRegistry 有 10 个方法。在编译器实际使用中，以下能力缺失：

1. **结构化解析不统一** — resolve_import_full 是结构化的，但 call/method resolve 不是
2. **桥接元数据缺失** — 无法知道版本、兼容范围、功能集
3. **健康检查太薄** — ping 只返回延迟，没有状态分级
4. **无内省能力** — 无法枚举桥接对外暴露的符号
5. **无依赖管理** — 多桥接间需要依赖声明
6. **无批量操作** — 逐项解析在编译大模块时性能差

## 新增 API 总览

### 新增类型（6 个）
| 类型 | 字段 | 用途 |
|------|------|------|
| `CallResolveResult` | rust_path, shim, module_name, is_macro, is_template | 结构化函数解析（从 std.rs 迁移） |
| `MethodResolveResult` | rust_method, rewritten, shim | 结构化方法解析（新增 shim 字段）|
| `BridgeMeta` | version, description, lz_version_min/max, provides | 桥接自描述 |
| `HealthStatus` | state, latency, active_since, error_count, detail | 结构化健康检查 |
| `BridgeState` | Healthy / Degraded / Unhealthy / Disconnected | 运行状态枚举 |
| `ExportKind` / `ExportEntry` | name, kind, signature, module | 导出符号枚举 |

### Bridge trait 新增方法（8 个）
| 方法 | 返回 | 默认实现 |
|------|------|---------|
| `meta()` | BridgeMeta | default |
| `health()` | HealthStatus | ping() 包装 |
| `on_activate()` / `on_deactivate()` | Result | Ok |
| `depends_on()` | &[String] | 空 |
| `resolve_call_full(fn, args)` | Option\<CallResolveResult\> | None |
| `resolve_method_full(m, ty)` | Option\<MethodResolveResult\> | None |
| `list_exports(kind)` | Vec\<ExportEntry\> | 空 |
| `export_count()` | usize | 0 |
| `batch_import(requests)` | Vec\<ImportResolveResult\> | 逐项回退 |
| `reload()` | Result | NotSupported |

### BridgeRegistry 新增方法（10 个）
| 方法 | 用途 |
|------|------|
| `register_with_deps(bridge)` | 验证依赖后注册 |
| `deregister(name)` | 注销并返回所有权 |
| `count()` / `names()` | 计数和名称列表 |
| `find_by_capability(cap)` | 按能力标志筛选 |
| `best_for(cap)` | 按级别排序选最优 |
| `resolve_call_full(fn, args)` | 结构化调用路由 |
| `resolve_method_full(m, ty)` | 结构化方法路由 |
| `batch_import(requests)` | 批量导入路由 |
| `list_exports(kind)` / `total_exports()` | 聚合导出枚举 |
| `stats()` | 运行统计 |

### BridgeRegistryStats
| 字段 | 用途 |
|------|------|
| bridge_count | 已注册桥接数 |
| default_name | 默认桥接名称 |
| bridge_names | 所有桥接名称列表 |

## 测试覆盖
- bridge::core: 33 测试（净增 24 个）
- 覆盖: 元数据, 健康检查, 依赖注册失败/成功, 注销存在/不存在, 名称列表, 按能力查找, 最优选择, 结构化解析 call/method, 批量导入, 导出枚举, 统计
- 全库: 187 tests pass

## 影响范围
- `bridge/core.rs`: +~320 行（类型定义 + trait 方法 + registry 方法 + 测试）
- `bridge/std.rs`: -15 行（删除重复类型，改用 pub use）
- `bridge/source.rs`: +30 行（新方法实现）
- 零破坏性: 所有默认实现保证 FfiBridge / CliBridge 无需改动即可编译

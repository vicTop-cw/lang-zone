# @export 增强设计：自动 DLL/SO 生成

> 2026-07-26 · v1.0

## 一、目标

写 `.lz`，一键出 `.dll` / `.so` / `.pyd`，且与增量缓存、watch 模式无缝协作。

```
@export(Rust)
def add(a: int, b: int) -> int = a + b

lzc math.lz --export    →  math.rs + math.dll
lzc math.lz --watch     →  文件变更 → 自动重建 .dll
```

## 二、架构设计

### 2.1 流水线

```
.lz 源码
  │
  ├─[cache hit?]──yes──→ 跳过 LZ→RS，直接进入 DLL 检查
  │
  └─[cache miss]──→
       ├─ Lexer → Parser → CodeGen → .rs
       ├─ 更新缓存 (hash + deps)
       └─ 检测到 @export 函数？
            ├─ yes → 生成 Cargo.toml → rustc --crate-type=cdylib → .dll
            └─ no  → 仅输出 .rs
```

### 2.2 模块拆分

```
src/export/
  ├─ mod.rs          # 入口 ::build_dll(source, target)
  ├─ manifest.rs     # 生成 Cargo.toml
  └─ builder.rs      # 调用 rustc/cargo，产物管理
```

### 2.3 与增量缓存的协作

```
CacheEntry 新增字段:
  export_hash: u64     # @export 函数的代码哈希
  export_dll: String   # 产物 .dll 路径

cache hit 判断:
  source_hash match → 缓存有效
    ├─ export_hash match → DLL 也有效，完全跳过
    └─ export_hash diff → .rs 复用，但重建 DLL
  
  source_hash diff → 全量重建（.rs + DLL）
```

## 三、Cargo.toml 自动生成

### 3.1 规则

```toml
# math.lz 的 @export 函数生成
[package]
name = "lz_math"              # {module_name} 或 {filename_base}
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]       # 生成 .dll/.so
name = "lz_math"

[dependencies]
# 如有 @export(Python) 则追加:
# pyo3 = { version = "0.22", features = ["extension-module"] }

# 如引用了 std bridge，自动注入:
# serde = { version = "1", optional = true }
# serde_json = { version = "1", optional = true }
```

### 3.2 产物命名

| 平台 | 类型 | 产物 |
|------|------|------|
| Windows | `cdylib` | `lz_math.dll` |
| Linux | `cdylib` | `liblz_math.so` |
| macOS | `cdylib` | `liblz_math.dylib` |
| Windows + Python | `cdylib` + pyo3 | `lz_math.pyd` |

## 四、@export 装饰器增强

### 4.1 新增参数

```lz
@export(Rust, name="math_ops")     // 自定义 crate 名称
@export(Python, module="mymath")   // Python 模块名
@export(C, abi="cdecl")            // C ABI 约定
```

### 4.2 ExportConfig

```rust
pub struct ExportConfig {
    pub targets: Vec<ExportTarget>,  // Rust | Python | C
    pub crate_name: Option<String>,  // 自定义 crate 名
    pub python_module: Option<String>,
    pub c_abi: Option<String>,
    pub lib_name: Option<String>,    // 输出库名
}

pub fn extract_export_config(decorators: &[Decorator]) -> Option<ExportConfig>;
```

## 五、CLI 集成

```bash
# 仅生成 .rs（当前行为）
lzc math.lz

# 生成 .rs + .dll
lzc math.lz --export

# watch + 自动重建 dll
lzc math.lz --export --watch

# 指定输出目录
lzc math.lz --export --out-dir ./target/

# 清理构建产物
lzc math.lz --export-clean
```

## 六、实现步骤

| Step | 内容 | 文件 | 耗时 |
|:----:|------|------|:---:|
| 1 | `src/export/manifest.rs` — Cargo.toml 生成器 | 新文件 | 小 |
| 2 | `src/export/builder.rs` — rustc/cargo 调用器 | 新文件 | 小 |
| 3 | `src/export/mod.rs` — 入口 + ExportConfig 提取 | 新文件 | 中 |
| 4 | `src/main.rs` — `--export` / `--export-clean` CLI | 修改 | 中 |
| 5 | `src/cache.rs` — export_hash 字段 | 修改 | 小 |
| 6 | 测试 + 文档 | 修改 | 中 |

## 七、风险与边界

| 风险 | 缓解 |
|------|------|
| rustc 不可用 | 优雅降级：仅输出 .rs + Cargo.toml，提示手动构建 |
| Cargo 构建缓慢 | 增量使用 `--emit=dep-info` 检测是否需重建 |
| 跨平台路径 | `std::path::MAIN_SEPARATOR` + cfg 条件编译 |
| 产物冲突 | `--out-dir` 隔离 + 文件名包含 hash 前缀 |

## 八、与热重载的衔接（未来）

`@export` DLL 生成后，热重载模块可直接加载：

```rust
// future: lz_hot_reload crate
let lib = unsafe { libloading::Library::new("lz_math.dll")? };
let add_fn: fn(i64, i64) -> i64 = unsafe { lib.get(b"add")? };
```

两者不矛盾——`@export` 是**构建时**能力（生成 DLL），热重载是**运行时**能力（加载 + 替换）。串行协作。

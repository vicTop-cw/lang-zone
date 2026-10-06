# TnrBridge 与跨语言机制使用指南

> 版本：2026-10-06
> 定位：LZ ↔ Tnr 双向集成的用户面向使用文档。覆盖 bridge 导入路由、embed 嵌入、extern 声明、export 导出、lz_derive 透传、comptime_lz 块六种机制。

## 1. 概览

LZ 与 Tnr 之间支持三种跨语言通信方向：

| 方向 | 机制 | 语法 | 说明 |
|------|------|------|------|
| LZ → Tnr | embed | `#[embed(tnr)]` + 反引号块 | LZ 源中嵌入 Tnr 代码，编译期转译为 Rust 模块 |
| LZ → Tnr | bridge import | `import tnr::func` | LZ 导入 Tnr 函数，通过 bridge registry 自动路由 |
| Tnr → LZ | extern | `#[extern(lz)]` | Tnr 声明外部 LZ 函数，链接期解析 |
| LZ → 外部 | export | `@export(Rust)` | LZ 导出函数供外部 Rust/Python/C 消费 |
| LZ 装饰器 | lz_derive | `#[lz_derive(Clone)]` | 透传为 Rust `#[derive(Clone)]` |
| LZ 编译期 | comptime_lz | `comptime_lz { ... }` | 编译期执行块，降级为 comptime 表达式 |

## 2. TnrBridge 三种路由语义

`import` 语句中 Tnr 相关模块路径自动路由到三种语义：

### 2.1 Lib API 转译（`tnr::func`）

```lz
import tnr::matmul
```

路由到 `use tnr_transpiled::matmul;`。Tnr 源码经 parse → lower → transpile 管线转为 Rust 函数，编译期消解，零运行时开销。

- Feature 依赖：`tnr-embed`
- 生成 shim：是（`__tnr_shim_<func>`）

### 2.2 已编译库链接（`tnr_lib::path`）

```lz
import tnr_lib::tensor::Tensor
```

路由到 `use tnr::tensor::Tensor;`。直接链接已编译的 tnr crate，零额外开销。

- Feature 依赖：无
- 生成 shim：否

### 2.3 计算后端委托（`tnr::compute::kernel`）

```lz
import tnr::compute::kernel
```

路由到 `use tnr_compute::kernel;`。生成 shim 函数，运行时通过 tnr interp 执行计算后端。

- Feature 依赖：`tnr-embed`
- 生成 shim：是

## 3. 三层导入自动路由

`gen_use_stmt` 按以下优先级自动路由 `import` 语句，无需用户显式标注：

```
import 路径
  ├─ 1. embed 模块名匹配 → 直接 use（embed 路由）
  ├─ 2. std 白名单匹配 → 标准 use（std 路由）
  └─ 3. bridge registry 匹配 → 桥接 use（bridge 路由）
```

### 3.1 embed 路由（最高优先级）

当 `import` 路径首段匹配 `#[embed(tnr)]` 生成的模块名时，走 embed 路由：

```lz
#[embed(tnr)]
```
fn helper(a, b) { a + b }
```

import mymod_tnr    -- 路由到 use mymod_tnr;（mymod.lz → mymod_tnr）
```

### 3.2 std 路由（次高优先级）

已知 Rust 标准库模块（`std::collections`、`std::sync` 等 22 个路径）走 std 白名单路由：

```lz
import std::collections::HashMap    -- 路由到 use std::collections::HashMap;
```

### 3.3 bridge 路由（兜底）

非 embed、非 std 的未知模块走 bridge registry 路由：

```lz
import tnr::matmul    -- 路由到 use tnr_transpiled::matmul;
```

## 4. embed 嵌入

### 4.1 反引号块（推荐）

```lz
#[embed(tnr)]
```
fn kernel(a, b) { a + b }
```
```

三反引号显式定界，不受缩进影响，可与后续顶层语句共存。

### 4.2 缩进块

```lz
#[embed(tnr)]
fn kernel(a, b)
    a + b
```

**注意**：缩进块要求后续顶层语句缩进严格小于块首行缩进。若 embed 内容与 `#[embed(tnr)]` 同列（column 0），后续所有同列行会被吞入块内。建议优先使用反引号块。

### 4.3 动态生成式

```lz
@tnr!
f```
fn kernel(a, b) { a + $(expr) }
```
```

`f``` `` 支持插值 `$(var)`，`r``` `` 为原样静态。

## 5. extern 声明（Tnr 侧）

Tnr 侧声明外部 LZ 函数：

```tnr
#[extern(lz)]
fn lz_helper(x: i64) -> i64
```

- 不包装 `__ffi_call`，走 `use` 导入链接
- `Tensor` 类型直接复用 `tnr::tensor::Tensor`，零 marshal

## 6. export 导出

```lz
@export(Rust)
def add(a: int, b: int) -> int = a + b
```

导出函数自动登记到 BridgeRegistry，CLI 输出 `Bridge registry: N symbol(s)`。

## 7. lz_derive 装饰器透传

```lz
#[lz_derive(Clone, Debug)]
def MyStruct(x: int) -> int = x
```

透传为 Rust `#[derive(Clone, Debug)]`，出现在生成产物的函数声明前。

## 8. comptime_lz 块

```lz
comptime_lz {
    let x = 1 + 2
    print(x)
}
```

降级为 comptime 表达式块（`Stmt::Expr(Expr::Block(body, true))`），编译期求值。

## 9. 组合使用示例

四种机制可在同一 LZ 程序中协同工作：

```lz
#[embed(tnr)]
```
fn tnr_kernel(a, b) { a + b }
```

import tnr::matmul
import tnr_lib::tensor::Tensor

#[extern(rust)]
def native_open(path: str) -> Ext = 0

@export(Rust)
def lz_add(a: int, b: int) -> int = a + b

def main():
    print(lz_add(1, 2))
```

## 10. 限制与注意事项

1. **CLI 语义分析**：CLI 编译时语义分析阶段会校验 `import` 路径是否存在对应文件。bridge import（`tnr::matmul` 等）在语义分析阶段会被拒绝。bridge import 路由仅在 codegen 阶段（库 API 路径）生效。
2. **tnr-embed feature**：未启用 `tnr-embed` feature 时，`#[embed(tnr)]` 块降级为注释，不转译。
3. **embed 缩进块**：内容与 `#[embed(tnr)]` 同列时会吞掉后续代码。使用反引号块语法规避。
4. **embed 模块名**：由源文件名 + 语言名拼接（`foo.lz` + `tnr` → `foo_tnr`）。库 API 路径需设置 `ir_module.file_path` 才能正确生成模块名。
5. **P1 路由匹配**：embed 模块名路由检查的是 `import` 完整路径，不是首段。`import foo_tnr::helper` 不会匹配 `foo_tnr`，需用 `import foo_tnr`。

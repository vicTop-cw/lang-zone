# lzcyc — LZ → Cython 编译器使用文档

> `lzcyc` 是 `lzc` 的子编译器，语法完全兼容 LZ，后端输出 **Cython (.pyx)** → 编译为 **.pyd** Python C 扩展。
> 用于 LZ 语言自举：先用 lzcyc 生成 Cython 代码，再用 Cython 编译为 Python 可调用的扩展模块。

---

## 一、安装

项目源码位于 `CY/` 目录，独立构建，不与主编译器 workspace 共用：

```bash
# 构建（从 CY/ 目录内）
cd E:\IDEProjects\AI\lang-zone\CY
cargo build --bin lzcyc

# 查看帮助
cargo run --bin lzcyc -- --help
```

---

## 二、命令

### 2.1 transpile — 将 LZ 代码转译为 Cython (.pyx)

```bash
cargo run --bin lzcyc -- transpile input.lz
```

处理流程：
```
input.lz
  → lzcyc::lexer (词法分析)
  → lzcyc::parser (语法分析 → AST Module)
  → lzcyc::codegen_cython (Cython 代码生成)
  → output.pyx
```

示例：
```bash
# 转译单个文件
cargo run --bin lzcyc -- transpile DMO/01_basics/literals.lz

# 指定输出目录
cargo run --bin lzcyc -- transpile input.lz -o output/
```

### 2.2 compile — 转译 + 编译为 .pyd

```bash
cargo run --bin lzcyc -- compile hello.lz
```

处理流程：
```
input.lz
  → transpile → input.pyx
  → cythonize → input.c
  → GCC/MSVC  → input.pyd (Python 可导入)
```

### 2.3 run — 编译并运行

```bash
cargo run --bin lzcyc -- run hello.lz
cargo run --bin lzcyc -- run hello.lz main  # 指定入口函数
```

---

## 三、输出说明

### 转译输出 (.pyx 文件)

示例 `hello.pyx`：
```cython
# cython: language_level=3
# cython: binding=True

def main():
    print("Hello, LZ!")
    return None
```

### 编译输出 (.pyd 文件)

`compile` 命令生成可供 Python `import` 的扩展模块：

```python
import hello        # 导入 hello.pyd
hello.main()        # 调用 LZ 函数
```

---

## 四、命令行选项

| 选项 | 说明 |
|:----|------|
| `transpile <file>` | 将 .lz 转译为 .pyx（不编译） |
| `compile <file>` | 转译 + cythonize + C 编译为 .pyd |
| `run <file> [func]` | 编译并运行，默认入口 `main` |
| `-o, --output <dir>` | 指定输出目录 |
| `--debug` | debug 模式（保留临时文件） |
| `--release` | release 模式（O2 优化） |
| `--version` | 版本号 |

---

## 五、运行时库

lzcyc 依赖 Cython 运行时库（`CY/runtime/`）：

| 文件 | 提供 |
|------|------|
| `lz_types.pxd/pyx` | List/Dict/Set 类型守卫 |
| `lz_option.pxd/pyx` | Option/Result 实现 |
| `lz_pointers.pxd/pyx` | Box/Rc/Arc 指针模拟 |
| `lz_concurrency.pxd/pyx` | Future/spawn/go 并发 |
| `lz_exceptions.pxd/pyx` | panic 和异常层次 |

运行时库在 `compile` 命令中自动链接。

---

## 六、与 lzc 的关系

| | lzc | lzcyc |
|:----|:----|:------|
| 输出 | Rust `.rs` | Cython `.pyx` / `.pyd` |
| 目标 | 原生二进制 | Python 扩展 |
| 用途 | 生产编译 | 自举 + 原型开发 |
| 语法 | LZ 完整规范 | LZ 完整规范 |
| 前端 | 共享 parser/ast/typer（依赖 `lang-zone` lib，零 COPY） | 共享 parser/ast/typer |

> **架构约束（长期决策）**：lzc 是主编译器（默认 Rust 后端），lzcyc 只能在 `CY/`
> 范围内开发，复用主编译器 lib 公开层（lexer/parser/ast/semantic_check/ir/codegen_cython），
> 不修改 `src/` 下任何文件。主编译器后续将融入子编译器，届时以 lzcyc 为主干。

---

## 七、开发状态（2026-09-17 更新）

lzcyc CLI 已落地（`CY/src/main.rs`），复用主编译器 lib 完整管线：

- ✅ **transpile**：lex → 宏/模板展开 → parse → **import 合并**（源目录 + `--std-dir`）→ semantic_check → build_ir → Cython 后端 → **生成后处理**（运行期缺口兜底）
  - TESTS 全量 54 样例 **53 通过**（1 个失败为主编译器前端既有行为：作用域检查，与 lzc 行为一致）
- ✅ **run**：运行层验证
  - 本机有 cython → pyximport 即时编译
  - 无 cython → **纯 Python 降级运行**（按项目验证约定剥离 cdef/cpdef/ctypedef/参数 C 类型/返回标注）
  - TESTS 全量 run 回归 **52/54 通过**；挂账 2 个：作用域检查（前端）、checker 派发（`函数[checker]` 下标语义，见 BACKLOG）
- ✅ **宏/模板展开已接入**：`lexer → 宏/模板展开（交替至稳定，16 轮上限）→ parser`，编排复用 lib 公开层 `lang_zone::macros::*`，支持 `@name!` 宏调用、`name!` 模板调用、跨模块 `macro import`；`--macro-check=loose|light|strict` 透传
- ✅ **compile**：transpile + `cythonize`（调 `CY/scripts/cython_build.py`；.c → .pyd 需本机 C 编译器）

```powershell
# 构建（CY 独立 workspace）
cd F:\AI\lang-zone\CY
cargo build

# 验证（回归基线已固化为测试：transpile 51/53、run 45/53 期望）
cargo test

# 手动验证
.\target\debug\lzcyc.exe transpile TESTS/01_basics/literals.lz
.\target\debug\lzcyc.exe run TESTS/01_basics/literals.lz
```

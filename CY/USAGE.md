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
| 前端 | 共享 parser/ast/typer（从 `src/` COPY） | 共享 parser/ast/typer |

---

## 七、开发状态

lzcyc 处于 Phase 0–2 阶段，当前支持：
- ✅ 词法分析、语法解析（与 lzc 共享）
- ✅ CLI 框架（transpile/compile/run）
- ✅ 基本表达式生成（字面量、二元运算、函数调用）
- 🟡 语句生成、声明生成、类型推断接入进行中
- ❌ 模式匹配、魔法方法、并发、所有权模拟

```bash
# 运行集成测试
cd E:\IDEProjects\AI\lang-zone\CY
cargo test

# 运行已有转译测试（验证 .pyx 输出）
cargo run --bin lzcyc -- transpile TESTS/01_basics/literals.lz
```

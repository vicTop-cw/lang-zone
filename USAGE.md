# Lang-Zone 编译器 — 使用指南

LZ 编译器提供两个后端：`lzc`（Rust 原生代码生成）和 `lzcyc`（Cython/Python 代码生成）。

---

## 一、lzc — LZ → Rust 编译器

### 编译单个文件

```bash
# 编译 .lz 文件为 Rust .rs（再通过 rustc 编译为原生二进制）
cargo run -- hello.lz
```

输出 `hello.rs`，可通过 `rustc hello.rs -o hello` 生成可执行文件，或直接集成到 Rust 项目中。

### 编译选项

```bash
# Tokenize：查看词法分析结果
cargo run -- hello.lz --tokens

# AST：查看语法分析树
cargo run -- hello.lz --ast

# 宏展开结果
cargo run -- hello.lz --dump-macros

# 指定标准库路径
cargo run -- hello.lz --std-dir ./stdlib

# 项目模式（递归加载 import 依赖）
cargo run -- hello.lz --project

# 增量编译缓存（仅对比源文件哈希）
cargo run -- hello.lz --cached

# 允许使用 rustc 私有 API
cargo run -- hello.lz --allow-rustc-private

# IR 中间表示输出
cargo run -- hello.lz --emit=ir
cargo run -- hello.lz --ir-codegen
```

> **计划中（尚未实现）**: `--cache-dir`、`--force`、`--lzi`、`--rust-crate`。当前只支持 `--cached` + 硬编码 `.lzcache` 目录。

### 运行完整测试

```bash
# 库测试（292 项单元测试）
cargo test --lib

# 仅 DEMO 编译测试（跳过 99_errors 和 99_spec）
cargo test --test compile_demos

# 仅语法错误拒绝测试（99_errors/ 目录下所有 .lz 文件）
cargo test --test reject_errors

# IR 快照测试
cargo test --test ir_snapshots
```

### 使用作为库

```rust
use lang_zone::lexer::Lexer;
use lang_zone::parser::Parser;
use lang_zone::codegen::CodeGen;

// 1. 词法分析
let mut lexer = Lexer::new(source);
let tokens = lexer.tokenize();

// 2. 语法分析
let module = Parser::new(tokens).parse_module()?;

// 3. 代码生成（需提供 std_dir, allow_rustc_private, rustc_version）
let rust_code = CodeGen::generate(&module, None, false, String::new());
```


---

## 二、lz-infer — 外部类型推断引擎

```bash
# 为 hello.lz 生成类型签名文件
cd lz-infer
cargo run --bin lz-infer -- ../hello.lz -o hello.lzi

# 然后 lzc 编译时可加载该签名
cd ..
cargo run -- hello.lz --lzi lz-infer/hello.lzi
```

---

## 三、LZ 语言速览

### Hello World

```lz
def main() =
    print("Hello, LZ!")
```

### 变量与绑定

```lz
def main() =
    // 默认可变（与 Rust 相反）
    x = 42
    x += 1

    // let 不可变绑定
    let name: str = "Alice"

    // const 编译时常量
    const MAX: int = 100

    // ref 引用
    ref r = x

    // ^ 所有权转移
    y = x^
```

### 函数

```lz
// 简单函数（等式体）
def add(a: int, b: int) -> int = a + b

// 块体函数
def greet(name: str) -> str =
    let msg = "Hello, " + name
    msg

// 泛型函数
def identity<T>(x: T) -> T = x

// 带约束的泛型
def clone_and_print<T: Clone>(x: T) -> T =
    let c = x.clone()
    print(c)
    c

// 默认参数
def connect(host: str = "localhost", port: int = 8080) =
    print(host, port)
```

### 控制流

```lz
def main() =
    // if/elif/else
    if x > 0:
        print("pos")
    elif x < 0:
        print("neg")
    else:
        print("zero")

    // match 模式匹配
    match x:
        case 0 => "none"
        case 1 => "one"
        case _ => "many"

    // for 循环（支持守卫）
    for i in 0..5 if i > 1:
        print(i)

    // while 循环（支持守卫）
    while running if cond():
        step()

    // loop 循环
    loop:
        if done: break
```

### 数据结构

```lz
// struct
struct Point =
    x: f64
    y: f64

// enum
enum Color:
    Red
    Green
    Blue

// 带数据变体
enum Option<T>:
    Some(T)
    None

// trait 定义
trait Drawable =
    def draw(self) -> ()
```

### 错误处理

```lz
def faulty() raises str =
    raise "something went wrong"

def main() =
    try:
        faulty()
    catch e:
        print("caught:", e)
    finally:
        print("cleanup")
```

## 五、lzcyc — LZ → Cython 子编译器

`lzcyc` 是 `lzc` 的子编译器，语法完全兼容 LZ，后端输出 Cython（`.pyx`）→ 编译为 Python C 扩展（`.pyd`）。

### 三个命令

```bash
cd CY

# transpile — 仅转译
cargo run --bin lzcyc -- transpile hello.lz

# compile — 转译 + cythonize + C 编译 → .pyd
cargo run --bin lzcyc -- compile hello.lz

# run — 编译并执行
cargo run --bin lzcyc -- run hello.lz
```

### 两个后端对比

| 特性 | lzc (Rust) | lzcyc (Cython/Python) |
|:----|:---------:|:--------------------:|
| 输出 | `.rs` → 原生二进制 | `.pyx` → `.pyd` |
| 编译时间 | 快（源到源） | 中等（需 cythonize + C 编译） |
| 所有权 | 编译期静态检查 | 运行时 `_MOVED` 哨兵 |
| 目标 | 生产环境 | 自举 + 原型 |

### 运行时管道

```
hello.lz → transpile → hello.pyx → cythonize → hello.c
 → MSVC/GCC → hello.pyd → python import → 输出
```

需要：Python 3.x、`pip install cython`、C 编译器。

---

## 六、项目结构

```bash
# 运行语法验证脚本
cd DEMO
bash run_check.sh

# 运行完整 DEMO 编译测试
cd ..
cargo test --test compile_demos

# 覆盖范围（45 个主 DEMO + 8 个错误边界）
# 01_basics/     — 字面量、关键字、注释
# 02_types/      — Option/Result、类型别名
# 03_variables/  — let/ref/const/owned
# 04_functions/  — 函数、泛型、检查站、变参
# 05_expressions/— 运算、范围、管道、推导式
# 06_control_flow/— if/match/for/while/guard
# 07_structures/ — struct/enum/trait/impl/magic
# 08_modules/    — import/from/别名
# 09_macros/     — macro/template
# 10_error_handling/— try/catch/raise/panic
# 11_concurrency/— async/await/spawn
# 12_build_blocks/— =: / ~: / *:
# 13_operators/  — 优先级、空白规则
# 14_pointers/   — Box/Rc/Arc
# 15_generators/ — yield/yield from
# 16_testing/    — test/suite/assert
# 99_errors/     — 49 个预期失败语法边界
# 99_prelude/    — 内置预导入函数
```

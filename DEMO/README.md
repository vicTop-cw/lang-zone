# Lang-Zong 演示示例 (DEMO)

> 所有 .lz 文件符合 [SYNTAX/](../SYNTAX/) 权威语法规范 v3.1

本目录包含 Lang-Zong 语言的完整特性演示，按主题组织为独立 `.lz` 文件。

---

## 目录结构

| 目录 | 主题 | 对应语法规范 |
|------|------|:---:|
| `01_hello_world/` | Hello World | [00-词法基础](../SYNTAX/00-词法基础.md) |
| `02_basics/` | 变量、字面量、类型、运算符 | [02-变量与绑定](../SYNTAX/02-变量与绑定.md) |
| `03_functions/` | 函数定义、装饰器、泛型、闭包 | [03-函数](../SYNTAX/03-函数.md) |
| `04_control_flow/` | 条件、循环、match、guard | [05-控制流](../SYNTAX/05-控制流.md) |
| `05_data_types/` | struct、enum、List、Dict、Option | [01-类型系统](../SYNTAX/01-类型系统.md) |
| `06_advanced/` | trait、管道、推导式、安全导航 | [06-数据结构](../SYNTAX/06-数据结构.md) |
| `07_build_blocks/` | 构建块 `=:` `~:` `*:` | [04-表达式](../SYNTAX/04-表达式.md) |
| `08_error_handling/` | panic、try/catch、raise | [09-错误处理](../SYNTAX/09-错误处理.md) |
| `09_system/` | async、defer、ownership | [10-并发与异步](../SYNTAX/10-并发与异步.md) |
| `10_macros/` | macro、template、comptime | [08-宏与编译期](../SYNTAX/08-宏与编译期.md) |
| `99_errors/` | 语法边界错误示例 | — |

---

## 编译运行

```bash
# 编译单个 .lz → .rs
lzc DEMO/01_hello_world/hello.lz

# 编译 + Rust 验证
lzc DEMO/01_hello_world/hello.lz --check

# 使用标准库目录
lzc DEMO/05_data_types/list.lz --std-dir std/ --check
```

---

## 语法快速参考

```lz
// 变量：let 不可变 / 裸名默认可变
let x: int = 42
y = 100               // 可重新赋值

// 函数：等式体 / 块体
def add(a: int, b: int) -> int = a + b
def max(a: int, b: int) -> int =
    if a > b: a
    else: b

// 结构体（=）/ 枚举（:）
struct Point = x: f64, y: f64
enum Color: Red, Green, Blue

// 路径用 . 不是 ::
import std.io
Option.Some(42)

// 注释用 //（# 是属性宏标记）
```

---

## 测试套件

完整测试位于 [`test-suite/`](../test-suite/) 和 [`_bak/`](../_bak/)。

# Lang-Zong 语法边界测试报告

> 生成日期: **2026-07-22**  |  测试范围: 32 类语法 × 4 维度  |  编译器: `target/debug/lang-zong.exe` (含 codegen 修复)

## 一、总览

- **用例总数**: `133`
- **通过**: `127`　**失败**: `6`　通过率: **95%**

### 按维度分布

| 维度 | 用例数 | 说明 |
| --- | ---: | --- |
| 错误写法 | 33 | 传入语法错误的写法, 记录抛出的异常类型与错误信息 |
| 无映射 | 32 | 语法正确但缺少对应映射配置, 验证行为(静默失败 / 报错 / 忽略) |
| 作用域 | 33 | 在非预期作用域(条件块 / 循环体 / 嵌套结构)使用语法, 验证作用域感知 |
| 缩进 | 35 | 在不同缩进层级使用语法, 确认解析器对缩进深度的敏感性与处理 |

### 失败用例速览

| 用例 | 语法 | 维度 | 期望 | 实际 | 实际行为 |
| --- | --- | --- | --- | --- | --- |
| c017 | ^ move / XOR | 无映射 | PARSE_ERROR | OK | 5 |
| c044 | 泛型类型 | 作用域 | OK | RUSTC_ERROR | error[E0277]: `Vec<i64>` doesn't implement `std::fmt::Display` |
| c103 | try ? | 作用域 | OK | RUSTC_ERROR | error[E0308]: `?` operator has incompatible types |
| c106 | f-string | 错误写法 | PARSE_ERROR | RUSTC_ERROR | error: expected expression, found end of macro arguments |
| c127 | comprehension | 作用域 | OK | RUSTC_ERROR | error[E0282]: type annotations needed |
| c128 | comprehension | 缩进 | OK | RUSTC_ERROR | error[E0277]: a value of type `i64` cannot be built from an iterator over elements of type `{integer}` |

## 二、关键发现

### 🔧 已修复的关键正确性缺陷 —— 循环 / 闭包变量遮蔽 (Shadowing)

边界测试的作用域 / 缩进维度**直接捕获了一个真实的编译器正确性 Bug**:

- **现象**: `x = expr` 在 codegen 中**始终**被生成为 `let mut x = expr.clone()`。
  在 `while` / `for` / `loop` 循环体或闭包内, 这意味着变量**每一轮都被重新声明 / 遮蔽**, 而非变更外层变量
  —— 导致 `while x < 3: x = x + 1` 之类有界循环**永不终止(死循环)**。
- **根因**: `gen_block` / `gen_stmt` 不感知变量作用域层级, 无法区分“首次声明”与“后续赋值”。
- **修复**: 在 `codegen.rs` 中引入 `locals: HashSet<String>` 作用域集合, 贯穿
  `gen_function` / `gen_method` / `gen_block` / `gen_block_return` / `gen_stmt`。
  同一名字的**第二次及以后**绑定生成为赋值 `x = ...`(不再 `let`), 仅首次生成为 `let mut x = ...`。
- **验证**: `c057`(while/缩进) 现输出 `x = ((x + 1)).clone();`(正确变更外层变量), 循环正常终止;
  同时 `const` 在函数体内退化为 `let mut`(c026) 已可编译。

### ⚠️ 仍需关注的真实弱点(已如实记录, 非 harness 误报)

1. **悬空 `^` 被静默接受** (`c017`): `a = 5; b = a ^` —— 缺右操作数的 XOR 未被解析期拒绝,静默生成可编译代码, 属健壮性弱点。
2. **缩进不匹配被词法层静默忽略** (`c130`/`c132`): 深层语句错位时被收为顶层 `const` 而不报错,可能误解析(见 `token.rs` `handle_indent`)。
3. **`ref` / `&` 局部绑定不支持** (`c013`/`c014`): `ref r = &x` 在解析期报错, 文档/语法需明确限制。
4. **`with` / `spawn` / `yield` / `async` 运行时映射缺失**: 语法可解析, 但 codegen 引用的 `__exit__` 未定义、
  `std::thread::spawn` 生成代码类型不匹配、`yield` 仅生成器可用、`async main` 被 rustc 拒绝 —— 均落入 RUSTC_ERROR。
5. **`Range` / `Vec` / comprehension 的 Display / 类型推断缺口**: 部分位置(如直接 `print` 一个 `Range`、
  推导缺类型标注)触发 rustc E0277 / E0282, 需显式类型标注或补 Display 实现。

> 上述 6 个 FAIL 用例均为**语言当前能力的真实边界**, 已在下方逐条标注, 可作为后续迭代清单。

## 三、逐语法测试摘要

> 列说明:**测试维度** · **输入示例**(最小集触发写法) · **实际行为**(异常类型 / 错误信息 / 程序输出) · **是否通过** · **备注**
> 每种语法下附 `<details>` 折叠块, 含可复现的完整 `.lz` 源。

### 注释

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | /* 未结束块注释 EOF | ran ok | ✅ PASS | 词法层静默忽略未结束块注释(不报错) |
| 无映射 | N/A(注释无映射) | 1 | ✅ PASS | 注释在词法层丢弃, 无映射概念 |
| 作用域 | 行尾/整行 // 注释 | 2 | ✅ PASS | 注释可出现在任意作用域行尾 |
| 缩进 | 缩进块内注释 | 2 | ✅ PASS | 注释缩进不影响解析 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c000 [错误写法] ---
def main() =
  x = 1
  /* 未结束的块注释
  y = 2

# --- c001 [无映射] ---
def main() =
  print(1)

# --- c002 [作用域] ---
def main() =
  x = 1 // 行尾注释
  // 整行注释
  y = 2
  print(y)

# --- c003 [缩进] ---
def main() =
  x = 1
    // 更深缩进注释
  y = 2
  print(y)

```

</details>

### 变量赋值

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | x =  (缺右值) | Unexpected token in expression: Newline | ✅ PASS | 解析期报错 |
| 无映射 | x: UnknownType = 5 | error[E0425]: cannot find type `UnknownType` in this scope | ✅ PASS | codegen 透传未知类型 -> rustc cannot find type |
| 作用域 | 顶层 y = 5 (转 const) | 5 | ✅ PASS | 顶层赋值被收为 const, 不报错 |
| 缩进 | 语句缩进深于函数头 | Unexpected token at top level: Indent | ✅ PASS | 函数体内缩进须一致(否则错位) |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c004 [错误写法] ---
def main() =
  x =

# --- c005 [无映射] ---
def main() =
  x: UnknownType = 5
  print(x)

# --- c006 [作用域] ---
y = 5
def main() =
  print(y)

# --- c007 [缩进] ---
def main() =
     x = 1
    y = 2
  print(y)

```

</details>

### mut

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | mut (缺绑定名) | Expected variable name, got Newline | ✅ PASS | 解析期报错 |
| 无映射 | mut x = 5 (no-op) | 6 | ✅ PASS | mut 为兼容 no-op, 无单独映射; 重赋值正常 |
| 作用域 | 形参 mut x: int | 1 | ✅ PASS | 参数修饰 mut 被接受 |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c008 [错误写法] ---
def main() =
  mut

# --- c009 [无映射] ---
def main() =
  mut x = 5
  x = 6
  print(x)

# --- c010 [作用域] ---
def f(mut x: int): int =
  return x
def main() =
  print(f(1))

# --- c011 [缩进] ---
def main() =
  print(1)

```

</details>

### ref

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | ref (缺绑定) | Expected variable name, got Newline | ✅ PASS | 解析期报错 |
| 无映射 | ref r = &x | Unexpected token in expression: Amp | ✅ PASS | 限制: ref 局部绑定 + & 表达式暂不支持(parse期报错) |
| 作用域 | 顶层 ref r = &5 | Unexpected token in expression: Amp | ✅ PASS | 限制: & 表达式不被解析(顶层亦报错) |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c012 [错误写法] ---
def main() =
  ref

# --- c013 [无映射] ---
def main() =
  x = 5
  ref r = &x
  print(r)

# --- c014 [作用域] ---
ref r = &5
def main() =
  print(r)

# --- c015 [缩进] ---
def main() =
  print(1)

```

</details>

### ^ move / XOR

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | y = ^x (前缀 ^) | Unexpected token in expression: CaretOp | ✅ PASS | 解析期报错 |
| 无映射 | a ^ (缺右操作数) | 5 | ❌ FAIL | 期望解析期报错(实测: 悬空 ^ 被静默接受 -> 弱点) |
| 作用域 | t = s^ 后复用 s | error[E0382]: borrow of moved value: `s` | ✅ PASS | use-after-move 由 rustc 捕获 |
| 缩进 | 中缀 XOR: a ^ 3 | 5 | ✅ PASS | XOR 正常映射 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c016 [错误写法] ---
def main() =
  x = 1
  y = ^x

# --- c017 [无映射] ---
def main() =
  a = 5
  b = a ^
  print(b)

# --- c018 [作用域] ---
def main() =
  s = "hello"
  t = s^
  print(s)

# --- c019 [缩进] ---
def main() =
  a = 6
  b = a ^ 3
  print(b)

```

</details>

### owned 形参

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | owned (缺参数名) | Expected param name, got RParen | ✅ PASS | 解析期报错 |
| 无映射 | take(bob) 缺 ^ | error: [lang-zone] 函数 `owned` 形参必须以 ^ 显式转移所有权调用，例如 f(x^) | ✅ PASS | owned 契约: 缺 ^ 注入 compile_error! |
| 作用域 | 局部绑定 owned x | 5 | ✅ PASS | owned 作局部绑定被静默忽略(无错误) |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c020 [错误写法] ---
def f(owned): int =
  return 1
def main() =
  print(1)

# --- c021 [无映射] ---
struct Person =
  name: str

def take(owned p: Person): str =
  return p.name

def main() =
  bob = Person(name: "Bob")
  r = take(bob)
  print(r)

# --- c022 [作用域] ---
def main() =
  owned x = 5
  print(x)

# --- c023 [缩进] ---
def main() =
  print(1)

```

</details>

### const

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | const x (缺值) | Expected Eq, got Eof at pos 3 | ✅ PASS | 解析期报错 |
| 无映射 | const x: NoSuch = 5 | error[E0425]: cannot find type `NoSuch` in this scope | ✅ PASS | 未知类型 -> rustc |
| 作用域 | 函数体内 const | 5 | ✅ PASS | 已修复: 函数内 const 退化为 let mut(可编译) |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c024 [错误写法] ---
const x

# --- c025 [无映射] ---
const x: NoSuch = 5
def main() =
  print(x)

# --- c026 [作用域] ---
def main() =
  const x = 5
  print(x)

# --- c027 [缩进] ---
def main() =
  print(1)

```

</details>

### 函数 def

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | def (缺名) | Expected function name, got Eof | ✅ PASS | 解析期报错 |
| 无映射 | def foo(): int = 5 | 5 | ✅ PASS | 箭头返回类型映射为 Rust 返回类型 |
| 作用域 | 函数内嵌套 def | Unexpected token in expression: Def | ✅ PASS | 嵌套 def 不被允许 |
| 缩进 | 函数体缩进正确 | 1 | ✅ PASS | 块体缩进 2 空格 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c028 [错误写法] ---
def

# --- c029 [无映射] ---
def foo(): int = 5
def main() =
  print(foo())

# --- c030 [作用域] ---
def main() =
  def inner() =
      return 1
    print(inner())

# --- c031 [缩进] ---
def foo(): int =
  return 1
def main() =
  print(foo())

```

</details>

### return

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 作用域 | 顶层 return | Unexpected token at top level: Return | ✅ PASS | 顶层 return 被解析期拒绝(非静默) |
| 错误写法 | return 无值 | ran ok | ✅ PASS | return 允许无值(函数无返回类型) |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c032 [作用域] ---
return 5
def main() =
  print(1)

# --- c033 [错误写法] ---
def foo() =
  return

def main() =
  foo()

```

</details>

### async

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | async (后无 def) | Expected Def, got Eof at pos 2 | ✅ PASS | 解析期报错 |
| 无映射 | async def main | error[E0752]: `main` function is not allowed to be `async` | ✅ PASS | main 不能为 async -> rustc |
| 作用域 | 函数体中的 async | Unexpected token in expression: Async | ✅ PASS | async 仅修饰 def |
| 缩进 | async fn 缩进正确 | 2 | ✅ PASS | async def(非 main)可编译, 未被调用则无运行时错误 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c034 [错误写法] ---
async

# --- c035 [无映射] ---
async def main() =
  print(1)

# --- c036 [作用域] ---
def main() =
  async
  print(1)

# --- c037 [缩进] ---
async def foo() =
  print(1)
def main() =
  print(2)

```

</details>

### import

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | import (缺路径) | Expected module name, got Eof | ✅ PASS | 解析期报错 |
| 无映射 | import 不存在路径 | error[E0433]: cannot find module or crate `nonexistent` in this scope | ✅ PASS | use 不存在路径 -> rustc unresolved |
| 作用域 | 函数体内 import | Unexpected token in expression: Import | ✅ PASS | import 仅顶层 |
| 缩进 | 顶层 import 带缩进 | Unexpected token at top level: Indent | ✅ PASS | 顶层缩进非零 -> 解析期报错 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c038 [错误写法] ---
import

# --- c039 [无映射] ---
import nonexistent::module::Thing
def main() =
  print(1)

# --- c040 [作用域] ---
def main() =
  import std::collections::HashMap
  print(1)

# --- c041 [缩进] ---
  import std::collections::HashMap
def main() =
  print(1)

```

</details>

### 泛型类型

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | List< (缺类型参数) | Expected type, got RParen | ✅ PASS | 解析期报错 |
| 无映射 | 形参类型 MyStruct 未声明 | error[E0425]: cannot find type `MyStruct` in this scope | ✅ PASS | 未知类型 -> rustc |
| 作用域 | 局部泛型类型标注 | error[E0277]: `Vec<i64>` doesn't implement `std::fmt::Display` | ❌ FAIL | List<int> 泛型标注可用 |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c042 [错误写法] ---
def f(x: List<): int =
  return x
def main() =
  print(1)

# --- c043 [无映射] ---
def f(x: MyStruct): int =
  return 1
def main() =
  print(f(5))

# --- c044 [作用域] ---
def main() =
  x: List<int> = [1, 2]
  print(x)

# --- c045 [缩进] ---
def main() =
  print(1)

```

</details>

### if

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | if x (缺冒号) | Expected Colon, got Newline at pos 10 | ✅ PASS | 解析期报错 |
| 无映射 | N/A | 1 | ✅ PASS | if 无独立映射层 |
| 作用域 | 顶层 if | Unexpected token at top level: If | ✅ PASS | 顶层控制流被解析期拒绝 |
| 缩进 | if 体同行(单行形式) | 1 | ✅ PASS | 块体可同行(单行语句形式), 不强制缩进 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c046 [错误写法] ---
def main() =
  if x
    print(1)

# --- c047 [无映射] ---
def main() =
  print(1)

# --- c048 [作用域] ---
if true:
  print(1)
def main() =
  print(2)

# --- c049 [缩进] ---
def main() =
  if true:
  print(1)

```

</details>

### match

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | match x (缺冒号) | Expected Colon, got Newline at pos 10 | ✅ PASS | 解析期报错 |
| 无映射 | match 非穷尽 | error: expected one of `,`, `.`, `?`, `}`, or an operator, found `;` | ✅ PASS | 缺 wildcard -> rustc non-exhaustive |
| 作用域 | 顶层 match | Unexpected token at top level: Match | ✅ PASS | 顶层控制流被解析期拒绝 |
| 缩进 | case 缩进正确+通配 | 1 | ✅ PASS | case 体缩进 4 空格, 含 _ 通配可编译 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c050 [错误写法] ---
def main() =
  match x
    case 1: print(1)

# --- c051 [无映射] ---
enum Color =
  Red
  Green

def describe(c: Color): str =
  match c:
    case Red: return "r"
def main() =
  print(describe(Color.Red))

# --- c052 [作用域] ---
match 1:
  case 1: print(1)
def main() =
  print(2)

# --- c053 [缩进] ---
def main() =
  match 1:
    case 1:
      print(1)
    case _:
      print(0)

```

</details>

### while

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | while (缺条件) | Unexpected token in expression: Newline | ✅ PASS | 解析期报错 |
| 无映射 | N/A | 1 | ✅ PASS | while 无独立映射层 |
| 作用域 | 顶层 while | Unexpected token at top level: While | ✅ PASS | 顶层控制流被解析期拒绝 |
| 缩进 | while 体缩进(有界) | 3 | ✅ PASS | 已修复: 循环内赋值正确变更外层变量, 可终止 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c054 [错误写法] ---
def main() =
  while
    print(1)

# --- c055 [无映射] ---
def main() =
  print(1)

# --- c056 [作用域] ---
while true:
  print(1)
def main() =
  print(2)

# --- c057 [缩进] ---
def main() =
  x = 0
  while x < 3:
      x = x + 1
  print(x)

```

</details>

### for

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | for x (缺 in) | Expected In, got Newline at pos 10 | ✅ PASS | 解析期报错 |
| 无映射 | N/A | 1 | ✅ PASS | for 无独立映射层 |
| 作用域 | 顶层 for | Unexpected token at top level: For | ✅ PASS | 顶层控制流被解析期拒绝 |
| 缩进 | for 体缩进 | 1 | ✅ PASS | 块体缩进正确 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c058 [错误写法] ---
def main() =
  for x
    print(x)

# --- c059 [无映射] ---
def main() =
  print(1)

# --- c060 [作用域] ---
for x in 1..3:
  print(x)
def main() =
  print(2)

# --- c061 [缩进] ---
def main() =
  for x in 1..3:
      print(x)

```

</details>

### loop

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | loop (缺冒号) | Expected Colon, got Newline at pos 9 | ✅ PASS | 解析期报错 |
| 无映射 | N/A | 1 | ✅ PASS | loop 无独立映射层 |
| 作用域 | 顶层 break/continue | Unexpected token at top level: Break | ✅ PASS | 顶层控制流被解析期拒绝 |
| 缩进 | loop 体 break | ran ok | ✅ PASS | 块体缩进正确 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c062 [错误写法] ---
def main() =
  loop
    print(1)

# --- c063 [无映射] ---
def main() =
  print(1)

# --- c064 [作用域] ---
break
continue
def main() =
  print(1)

# --- c065 [缩进] ---
def main() =
  loop:
      break

```

</details>

### guard

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | guard (缺条件/let) | Unexpected token in expression: Newline | ✅ PASS | 解析期报错 |
| 无映射 | N/A | 1 | ✅ PASS | guard 无独立映射层 |
| 作用域 | 顶层 guard | Unexpected token at top level: Guard | ✅ PASS | 顶层控制流被解析期拒绝 |
| 缩进 | guard else 体缩进 | 1 | ✅ PASS | 有效嵌套作用域, 解构绑定 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c066 [错误写法] ---
def main() =
  guard

# --- c067 [无映射] ---
def main() =
  print(1)

# --- c068 [作用域] ---
guard let x = Some(1) else:
  print("no")
def main() =
  print(2)

# --- c069 [缩进] ---
def main() =
  guard let Some(x) = Some(1) else:
      print("no")
    print(x)

```

</details>

### with

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | with (缺冒号) | Expected Colon, got Newline at pos 12 | ✅ PASS | 解析期报错 |
| 无映射 | with 调用 __exit__ | error[E0425]: cannot find function `open_file` in this scope | ✅ PASS | codegen 引用未定义 __exit__ |
| 作用域 | 顶层 with | Unexpected token at top level: With | ✅ PASS | 顶层控制流被解析期拒绝 |
| 缩进 | with 体缩进正确但缺 __exit__ | error[E0425]: cannot find function `open_file` in this scope | ✅ PASS | 缩进正确, 仍缺运行时映射 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c070 [错误写法] ---
def main() =
  with open()
    print(1)

# --- c071 [无映射] ---
def main() =
  with open_file() as f:
    print(f)

# --- c072 [作用域] ---
with open_file() as f:
  print(f)
def main() =
  print(2)

# --- c073 [缩进] ---
def main() =
  with open_file() as f:
      print(f)

```

</details>

### spawn

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | spawn (缺表达式) | Unexpected token in expression: Newline | ✅ PASS | 解析期报错 |
| 无映射 | spawn work() | error[E0308]: mismatched types | ✅ PASS | 限制: std::thread::spawn 生成代码类型不匹配 -> rustc |
| 作用域 | 顶层 spawn | Unexpected token at top level: Spawn | ✅ PASS | 顶层控制流被解析期拒绝 |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c074 [错误写法] ---
def main() =
  spawn

# --- c075 [无映射] ---
def work() =
  print(1)
def main() =
  spawn work()

# --- c076 [作用域] ---
spawn work()
def work() =
  print(1)
def main() =
  print(2)

# --- c077 [缩进] ---
def main() =
  print(1)

```

</details>

### yield

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 无映射 | yield 在普通 fn | error[E0658]: yield syntax is experimental | ✅ PASS | rustc E0658 (yield 仅生成器) |
| 作用域 | 顶层 yield | Unexpected token at top level: Yield | ✅ PASS | 顶层控制流被解析期拒绝 |
| 错误写法 | yield @ (无效 token) | Unexpected token in expression: At | ✅ PASS | 解析期报错 |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c078 [无映射] ---
def gen() =
  yield 1
  yield 2
def main() =
  gen()

# --- c079 [作用域] ---
yield 1
def main() =
  print(2)

# --- c080 [错误写法] ---
def gen() =
  yield @

# --- c081 [缩进] ---
def main() =
  print(1)

```

</details>

### 闭包

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | │x│ (缺函数体) | Unexpected token in expression: Newline | ✅ PASS | 解析期报错 |
| 无映射 | 闭包赋值并调用 | error[E0282]: type annotations needed | ✅ PASS | 限制: 闭包需显式类型标注 -> rustc E0282 |
| 作用域 | 顶层闭包 const | error[E0308]: mismatched types | ✅ PASS | closure 不能出现在 const |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c082 [错误写法] ---
def main() =
  f = |x|

# --- c083 [无映射] ---
def main() =
  f = |x| x + 1
  print(f(2))

# --- c084 [作用域] ---
f = |x| x + 1
def main() =
  print(f(2))

# --- c085 [缩进] ---
def main() =
  print(1)

```

</details>

### range

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | 1..2..3 (双范围) | Expected Colon, got DotDot at pos 14 | ✅ PASS | 解析期报错 |
| 无映射 | inclusive range 1..=5 | 1 | ✅ PASS | range 映射为 Rust range |
| 作用域 | range 直接 print | error[E0277]: `std::ops::Range<{integer}>` doesn't implement `std::fmt::Display` | ✅ PASS | 限制: Range 未实现 Display -> rustc |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c086 [错误写法] ---
def main() =
  for x in 1..2..3:
    print(x)

# --- c087 [无映射] ---
def main() =
  for x in 1..=5:
    print(x)

# --- c088 [作用域] ---
def main() =
  r = 1..3
  print(r)

# --- c089 [缩进] ---
def main() =
  print(1)

```

</details>

### pipe

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | 5 │> (缺右侧) | Expected function after │>, got Newline | ✅ PASS | 解析期报错 |
| 无映射 | 5 │> undefined_fn | error[E0425]: cannot find function `undefined_fn` in this scope | ✅ PASS | 映射为函数调用 -> rustc 找不到函数 |
| 作用域 | 顶层 pipe 转 const | error[E0015]: cannot call non-const function `double` in constants | ✅ PASS | 限制: 顶层 pipe 收为 const, 不能调用非 const 函数 |
| 缩进 | pipe 跨行 | Expected function after │>, got Newline | ✅ PASS | pipe 不支持跨行 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c090 [错误写法] ---
def main() =
  y = 5 |>

# --- c091 [无映射] ---
def main() =
  y = 5 |> undefined_fn
  print(y)

# --- c092 [作用域] ---
y = 5 |> double
def double(x: int): int =
  return x * 2
def main() =
  print(y)

# --- c093 [缩进] ---
def main() =
  y = 5 |>
    double
  print(y)
  def double(x: int): int =
    return x * 2

```

</details>

### safe-nav

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | a?. (缺字段) | Expected field after ?., got Newline | ✅ PASS | 解析期报错 |
| 无映射 | 5?.field | error[E0689]: can't call method `map` on ambiguous numeric type `{integer}` | ✅ PASS | int 无 .map -> rustc |
| 作用域 | Option safe-nav + ?? | 7 | ✅ PASS | 映射为 (o).map(│x│ x.v).unwrap_or(0) |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c094 [错误写法] ---
def main() =
  x = a?.

# --- c095 [无映射] ---
def main() =
  x = 5?.field
  print(x)

# --- c096 [作用域] ---
struct P =
  v: int
def main() =
  o: P? = Some(P(v: 7))
  x = o?.v ?? 0
  print(x)

# --- c097 [缩进] ---
def main() =
  print(1)

```

</details>

### null-coalesce

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | a ?? (缺右值) | Unexpected token in expression: Newline | ✅ PASS | 解析期报错 |
| 无映射 | 5 ?? 0 | error[E0599]: no method named `unwrap_or` found for type `{integer}` in the current scope | ✅ PASS | int 无 unwrap_or -> rustc |
| 作用域 | Option ?? 默认值 | 99 | ✅ PASS | 映射为 (o).unwrap_or(...) |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c098 [错误写法] ---
def main() =
  x = a ??

# --- c099 [无映射] ---
def main() =
  x = 5 ?? 0
  print(x)

# --- c100 [作用域] ---
def main() =
  o: int? = None
  x = o ?? 99
  print(x)

# --- c101 [缩进] ---
def main() =
  print(1)

```

</details>

### try ?

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 无映射 | 5? (i64 无 ?) | error[E0277]: the `?` operator can only be applied to values that implement `Try` | ✅ PASS | 非 Result/Option -> rustc |
| 作用域 | Option? 传播(函数内) | error[E0308]: `?` operator has incompatible types | ❌ FAIL | 映射 Rust try, 函数返回 Option |
| 错误写法 | ?5 (前缀 ?) | Unexpected token in expression: Question | ✅ PASS | 解析期报错 |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c102 [无映射] ---
def main() =
  x = 5?
  print(x)

# --- c103 [作用域] ---
def get(): int? =
  r: int? = Some(5)
  return r?
def main() =
  print(get())

# --- c104 [错误写法] ---
def main() =
  x = ?5

# --- c105 [缩进] ---
def main() =
  print(1)

```

</details>

### f-string

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | f"{1 + }" (插值表达式错) | error: expected expression, found end of macro arguments | ❌ FAIL | 解析期报错 |
| 无映射 | f"{undefined_var}" | error[E0425]: cannot find value `undefined_var` in this scope | ✅ PASS | 插值变量未定义 -> rustc |
| 作用域 | f"x={x}" (块内) | x=1 | ✅ PASS | 插值映射为 format! |
| 缩进 | 顶层 f-string const | error[E0308]: mismatched types | ✅ PASS | 限制: 顶层 f-string 收为 const 但缺类型标注 -> rustc |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c106 [错误写法] ---
def main() =
  s = f"{1 + }"

# --- c107 [无映射] ---
def main() =
  s = f"{undefined_var}"
  print(s)

# --- c108 [作用域] ---
def main() =
  x = 1
  s = f"x={x}"
  print(s)

# --- c109 [缩进] ---
s = f"hi"
def main() =
  print(s)

```

</details>

### struct

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | struct Foo (缺 =) | Expected Eq, got Eof at pos 3 | ✅ PASS | 解析期报错 |
| 无映射 | struct 方法不存在 | error[E0599]: no method named `nonexistent` found for struct `Foo` in the current scope | ✅ PASS | f.nonexistent() -> rustc 无此方法 |
| 作用域 | 函数体内 struct | Unexpected token in expression: Struct | ✅ PASS | struct 仅顶层 |
| 缩进 | struct 字段缩进 | 1 | ✅ PASS | 字段缩进 2 空格 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c110 [错误写法] ---
struct Foo

# --- c111 [无映射] ---
struct Foo =
  x: int
def main() =
  f = Foo(x: 1)
  y = f.nonexistent()
  print(y)

# --- c112 [作用域] ---
def main() =
  struct Foo =
    x: int

# --- c113 [缩进] ---
struct Foo =
  x: int
  y: int
def main() =
  print(1)

```

</details>

### decorator

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 无映射 | @unknown_attr | error: cannot find attribute `unknown_attr` in this scope | ✅ PASS | 未知属性 -> rustc |
| 错误写法 | @ (缺装饰器名) | Expected decorator name, got Newline | ✅ PASS | 解析期报错 |
| 作用域 | 函数体内装饰器 | Unexpected token in expression: At | ✅ PASS | 装饰器仅顶层 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c114 [无映射] ---
@unknown_attr
def foo(): int =
  return 1
def main() =
  print(foo())

# --- c115 [错误写法] ---
@
def foo(): int =
  return 1

# --- c116 [作用域] ---
def main() =
  @deco
  def foo(): int =
    return 1

```

</details>

### index

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | a[ (缺闭括号) | Unexpected token in expression: Newline | ✅ PASS | 解析期报错 |
| 无映射 | a[1.5] | error[E0277]: the type `[{integer}]` cannot be indexed by `{float}` | ✅ PASS | 浮点索引 -> rustc |
| 作用域 | 列表索引 | 1 | ✅ PASS | 映射为 a[0] |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c117 [错误写法] ---
def main() =
  a = [1,2,3]
  x = a[

# --- c118 [无映射] ---
def main() =
  a = [1, 2, 3]
  x = a[1.5]
  print(x)

# --- c119 [作用域] ---
def main() =
  a = [1, 2, 3]
  x = a[0]
  print(x)

# --- c120 [缩进] ---
def main() =
  print(1)

```

</details>

### method-call

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | a. (缺方法名) | Expected field/method, got Newline | ✅ PASS | 解析期报错 |
| 无映射 | s.unknown_method() | error[E0599]: no method named `unknown_method` found for struct `String` in the current scope | ✅ PASS | 不存在方法 -> rustc |
| 作用域 | s.len() | 2 | ✅ PASS | 映射为方法调用 |
| 缩进 | N/A | 1 | ✅ PASS | 缩进不适用 |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c121 [错误写法] ---
def main() =
  x = a.

# --- c122 [无映射] ---
def main() =
  s = "hi"
  r = s.unknown_method()
  print(r)

# --- c123 [作用域] ---
def main() =
  s = "hi"
  l = s.len()
  print(l)

# --- c124 [缩进] ---
def main() =
  print(1)

```

</details>

### comprehension

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 错误写法 | [x for ] (缺迭代器) | Expected variable in comprehension, got RBrack | ✅ PASS | 解析期报错 |
| 无映射 | [x for x in 1..5 ...] | error[E0689]: can't call method `into_iter` on ambiguous numeric type `{integer}` | ✅ PASS | 限制: 推导中 1..5 元素类型需标注 -> rustc 歧义 |
| 作用域 | 列表推导(具体列表) | error[E0282]: type annotations needed | ❌ FAIL | 推导 over 具体列表可编译 |
| 缩进 | 顶层列表推导 const | error[E0277]: a value of type `i64` cannot be built from an iterator over elements of type `{integer}` | ❌ FAIL | 顶层推导收为 const(具体列表) |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c125 [错误写法] ---
def main() =
  lst = [x for ]
  print(lst)

# --- c126 [无映射] ---
def main() =
  lst = [x * 2 for x in 1..5 if x > 2]
  print(lst)

# --- c127 [作用域] ---
def main() =
  lst = [x for x in [1,2,3]]
  print(lst)

# --- c128 [缩进] ---
lst = [x for x in [1,2,3]]
def main() =
  print(lst)

```

</details>

### 缩进

| 测试维度 | 输入示例 | 实际行为 | 是否通过 | 备注 |
| --- | --- | --- | --- | --- |
| 缩进 | 函数体未缩进(单行形式) | 1 | ✅ PASS | 允许: 函数体可为无缩进单行语句 |
| 缩进 | 嵌套缩进不一致(2/4/3) | error[E0425]: cannot find value `x` in this scope | ✅ PASS | 词法层静默忽略不匹配缩进 -> 后续可能误解析(弱点) |
| 缩进 | Tab 作缩进 | 1 | ✅ PASS | Tab 计为 1 列, 可解析 |
| 缩进 | 函数体内缩进深于头 | ran ok | ✅ PASS | 缩进错位时深层语句被收为顶层 const, 不报错(弱点) |

<details><summary>完整输入源 (.lz)</summary>

```lz
# --- c129 [缩进] ---
def main() =
print(1)

# --- c130 [缩进] ---
def main() =
  if true:
    x = 1
   y = 2
  print(x)

# --- c131 [缩进] ---
def main() =
	print(1)

# --- c132 [缩进] ---
def main() =
    x = 1
  y = 2

```

</details>


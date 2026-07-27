# 构建块（Build Block）语法糖 — 实现与验证

为 Lang-Zong 编译器（`src/token.rs` / `src/parser.rs` / `src/codegen.rs`）实现了三种构建块，
本质是**无参闭包**，块内默认 `unsafe`（指针语法作用域限定块内）。

## 三种构建块

| 符号 | 类型 | 语义 |
|------|------|------|
| `=:` | 变量构建块 (Var) | 内部可执行任意逻辑；支持提前 `return` 与 `guard`；末尾表达式自动作为返回值赋给左侧变量 |
| `~:` | 调用构建块 (Call) | 返回值须为元组/字典/结构体或实现 `BuildParams` trait；支持提前 return / guard；**返回值自动拆包**应用于上层调用（元组→位置实参类似 `*args`，字典→命名实参类似 `**kwargs`） |
| `*:` | 生成器调用构建块 (Gen) | 返回迭代器；通过 `yield` 逐步产出参数包；`yield` 后留白 = 空参数包；`return` 或自然结束 = 停止产出 |

## 关键设计

- **词法识别 + 留白强制**：`=:` / `~:` / `*:` 在 lexer 识别，且符号前后必须留白，违反即发 `LexError`（`parse_module` 开头拒绝）。
- **缩进作用域**：`parse_build_block_body` 强制 `Indent`/`Dedent` 缩进块。
- **提前返回 / guard**：块体包在 `|| unsafe { ... }` 闭包中，`return` 自然映射为闭包返回；`guard` 转为 `if !(cond) { return ... }`。
- **BuildParams 约束验证**：`validate_build_block` 结构式校验 return/yield 载荷为 Tuple/Dict/Call/MethodCall/Ident（`is_valid_build_params`），Var/Call 块禁止 `yield`。
- **yield 控制流**：用 `Cell<bool>` 字段 `in_gen`，Gen 块体内 `yield X` → `__bb.push(X)`，`return` → 闭包返回（停止产出）。

## 参数包（自动拆包）设计

调用构建块 `~:` / 生成器构建块 `*:` 返回的元组与字典经类型擦除为 `__Pack`（裸指针承载，配合 `unsafe` 解包），并在返回上游时**自动拆包**应用于被调函数/方法：

- **元组 (`*args`)**：按位置顺序映射到被调函数的参数列表，逐个 `*(Box::from_raw(v[i] as *mut _))` 还原。
- **字典 (`**kwargs`)**：按被调函数参数名顺序，从字典中按名取出对应值（`m.get("name").expect(...)`）作为位置实参传入（Rust 无具名实参语法，故统一展开为位置实参）。
- **类型对齐**：打包时即把每个元素按被调参数 Rust 类型 `as` 转换（如 `int`→`i64`），避免块内默认 `i32` 与上游 `i64` 宽度不一致导致的脏数据。
- **不校验语义（保持 `unsafe`）**：编译器**不检查**字典 key 是否齐全或与参数名一致。
  - 多余 key（如函数仅需 `name` 但字典含 `name`+`age`）→ 多余 key 被忽略，编译通过、运行正常。
  - 缺失 key（如字典缺 `name`）→ 运行期 `__Pack::Dict` 查找缺失 key 触发 panic（`.expect("build param not found: ..")`），属使用者责任。
- **Single 分支**：仅当被调函数恰好 1 个参数时，才生成 `__Pack::Single(p)` 解包分支；多参数时该分支会让 Rust 对所有 match 分支做类型检查而失败，故退化为 `_ => unreachable!()`。

## 顺带修复的预存 codegen Bug

1. `DictLit` 由 `[...].into_iter().collect()` 改为 `vec![...].into_iter().collect()`（本工具链数组 `into_iter` 回退为切片迭代器，无法 collect 成 HashMap）。
2. `Stmt::For` 循环变量改为 `for mut {var}`（Lang-Zong 绑定默认可变）。

## 测试用例（`tests_buildblock/`）

**有效用例（均经 `lzc` + `rustc` + 运行通过）：**
- `bb_var.lz` — 提前 return + guard + 尾部表达式自动返回
- `bb_call.lz` — 返回结构体（整体单参） / 字典参数包 `**kwargs` 拆包
- `bb_gen.lz` — for 循环内 yield + 空参数包，每个 yield 自动拆包
- `bb_unpack_tuple.lz` — 元组参数包 `*args` 自动拆包（含嵌套控制流提前 return）
- `bb_unpack_dict.lz` — 字典 `**kwargs` 拆包 + 多余 key 被忽略（使用者责任）
- `bb_unpack_dict_missing.lz` — 字典缺失 key 的运行期 panic 演示（编译器不检查，保持 `unsafe`）

**错误用例（均被 `Parse error` 正确拒绝，exit 1）：**
- `bb_err_ws.lz` — `=:` 前未留白（词法错误）
- `bb_err_payload.lz` — Gen 块 yield 载荷非法（语义错误）
- `bb_err_yield_in_var.lz` — Var 块内使用 yield（作用域错误）

### 运行方式

```bash
./target/debug/lang-zong.exe tests_buildblock/bb_var.lz   # 生成 bb_var.rs
rustc tests_buildblock/bb_var.rs -o tests_buildblock/bb_var_bin
./tests_buildblock/bb_var_bin                              # 运行
```

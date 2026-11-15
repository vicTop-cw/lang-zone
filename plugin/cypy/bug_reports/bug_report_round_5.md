# Round 5 Bug 报告: 边界与组合测试

> 测试日期: 2026-07-25
> 测试文件: `_test_type_edge.cypy`, `_test_nesting.cypy`, `_test_combo.cypy`, `_test_literal_edge.cypy`, `_test_scope_edge.cypy`, `_test_edge_input.cypy`

## 发现的 Bug

### Bug-R5-1: 类型别名解析失败，返回 `list[object]` 而非 `list[int]` (High)
- **代码**: `type MyInt = int; type MyList = list[int]; def test_type_alias(x: MyInt) -> MyList: return [x]`
- **错误**: `Return type mismatch: expected MyList, got list[object]`
- **分析**: 类型别名 `MyList = list[int]` 在类型检查阶段未正确解析。`[x]`（其中 x: MyInt = int）应该推断为 `list[int]`，但类型检查器推断为 `list[object]`。类型别名中的泛型参数在展开时丢失。
- **影响**: 所有通过 `type` 定义的类型别名在返回类型检查时都会失败

### Bug-R5-2: `tuple` 返回类型被误判为 `list` (High)
- **代码**: `def test_empty_tuple() -> tuple: return ()`
- **错误**: `Return type mismatch: expected list, got list[object]`
- **分析**: `tuple` 类型在类型检查器中被错误映射为 `list` 类型。`()` 空元组字面量也被推断为 `list[object]` 而非 `tuple`。
- **影响**: `tuple` 类型不可用于函数返回类型注解

### Bug-R5-3: 嵌套泛型类型 `list[list[int]]` 解析为 `list[object]` (High)
- **代码**: `def test_nested_alias(x: NestedAlias) -> int` 其中 `type NestedAlias = list[list[int]]`，以及 `def test_deep_list() -> list[list[list[int]]]: return [[[1, 2], [3, 4]], [[5, 6], [7, 8]]]`
- **错误**: 
  - `Return type mismatch: expected list[list], got list[object]`
  - `Return type mismatch: expected list[list[list[int]]], got list[object]`
- **分析**: 嵌套泛型类型（`list[list[...]]`）在类型检查阶段被展开为 `list[list]` 但内部元素类型丢失。`type` 别名中的嵌套泛型参数也丢失。深层嵌套列表字面量类型推断全部降级为 `list[object]`。
- **影响**: 所有嵌套泛型容器类型（`list[list[int]]`、`dict[str, list[int]]` 等）的类型检查不正确

### Bug-R5-4: 函数类型注解 `(int) -> int` 语法不支持 (Medium)
- **代码**: `def test_func_type(f: (int) -> int) -> int: return f(1)`
- **错误**: `Expected RPAREN, got ARROW at 32:29`
- **分析**: Cypy 解析器不支持函数类型注解语法 `(param_type) -> return_type`。`->` 在参数类型注解上下文中被当做意外 token。
- **影响**: 无法在参数中使用函数类型（回调函数类型注解）

### Bug-R5-5: `for i in range(n)` 循环变量类型为 `object` 而非 `int` (Critical)
- **代码**: `for i in range(5): guard i >= 0 else: ...` 或 `for i in range(3): total = total + i`
- **错误**: 
  - `Type mismatch in binary operation: object >= int`
  - `Type mismatch in binary operation: object < int`
  - `Type mismatch in binary operation: int + object`
- **分析**: `for i in range(n)` 循环变量 `i` 的类型未被推断为 `int`，而是保持为 `object`。导致所有对 `i` 的比较和算术操作都报类型不匹配。`range()` 返回的迭代器元素类型未与循环变量关联。
- **影响**: `for i in range(n)` 循环中无法对 `i` 进行任何类型化操作

### Bug-R5-6: `case other:` 变量绑定模式不支持 (High)
- **代码**: `match i: case 0: ... case other: ...`（尝试用 `other` 变量绑定通配值）
- **错误**: `Undefined name 'other'`
- **分析**: `case` 模式中的标识符被当作变量引用而非绑定模式。解析器/作用域分析器不识别 `case variable:` 的变量绑定语法。这与已知 Bug-16（`_` 通配符未注册）和 Bug-25（`case _:` 不支持）相关，但扩展到所有变量绑定模式。
- **影响**: `match/case` 只能使用字面量模式，不能使用变量绑定模式

### Bug-R5-7: `defer expr` 单行语法不支持 (Medium)
- **代码**: `defer result = result + 1`
- **错误**: `Unexpected token ASSIGN at 7:22`
- **分析**: `defer` 关键字不支持单行表达式语法 `defer expr`，只支持块语法 `defer: stmts`。Cypy 文档未明确说明 `defer` 是否支持单行语法。
- **影响**: `defer` 只能使用块语法，增加代码冗余

### Bug-R5-8: 十六进制/八进制/二进制字面量不支持 (Medium)
- **代码**: `0xFF`, `0o77`, `0b1010`
- **错误**: `Undefined name 'xFF'`, `Undefined name 'o77'`, `Undefined name 'b1010'`
- **分析**: Lexer 不识别 `0x`、`0o`、`0b` 前缀的数字字面量。`0xFF` 被错误解析为 `0` 后跟标识符 `xFF`。
- **影响**: 无法使用十六进制、八进制、二进制数字字面量

### Bug-R5-9: 浮点数精度截断 (Low)
- **代码**: `return 3.14159265358979323846`
- **现象**: 生成 .pyx 中为 `3.141592653589793`（精度从 20 位截断到 15 位）
- **分析**: 代码生成阶段浮点数字面量使用了 Python 默认的 `str()` 转换，精度限制在约 15-16 位有效数字。
- **影响**: 高精度浮点数字面量在转译过程中丢失精度

### Bug-R5-10: 模块级 `let` 声明生成 `cdef int` 而非 Python 全局变量 (Medium)
- **代码**: `let GLOBAL_VAL: int = 100`（模块顶层）
- **现象**: 生成 `cdef int GLOBAL_VAL = 100`，而非 Python 模块级全局变量
- **分析**: 模块级 `let` 声明被错误地生成为 Cython `cdef` 变量。`cdef` 变量在 Cython 中仅在 C 级别可见，Python 无法访问。应为 Python 级别的全局变量或 `cpdef` 变量。
- **影响**: 模块级变量在 Python 运行时不可访问

### Bug-R5-11: 转义序列 `\t`, `\n` 在代码生成中丢失 (High, 已知 Bug-3 的扩展确认)
- **代码**: `return "tab:\t newline:\n quote:\" backslash:\\"`
- **现象**: 生成 .pyx 中为 `'tab:t newline:n quote:" backslash:\\'`，`\t` 变成 `t`，`\n` 变成 `n`
- **分析**: 字符串转义序列在代码生成阶段被错误处理。`\t` 和 `\n` 的反斜杠被丢弃，而 `\\` 和 `\"` 正确处理。此 Bug 在之前已报告（Bug-3），本次测试进一步确认了其影响范围。
- **影响**: 所有含 `\t`、`\n`、`\r` 转义的字符串在转译后语义错误

## 正常工作的特性

- 深层嵌套算术表达式（10 层） ✅
- 深层嵌套函数调用（10 层） ✅
- 深层嵌套 if/elif/else（10 层） ✅
- 混合运算符优先级 ✅
- 嵌套 if 条件 ✅
- Unicode 字符串 ✅
- 空字符串 ✅
- 大整数 ✅
- 负数字面量 ✅
- 布尔字面量 ✅
- 科学计数法浮点数 ✅
- 字符串拼接 ✅
- 嵌套函数变量遮蔽 ✅
- 闭包捕获外部变量 ✅
- 多层嵌套函数（3 层） ✅
- if 块内变量作用域 ✅
- 递归函数 ✅
- 尾递归 ✅
- 多参数函数（8 个参数） ✅
- 多返回语句 ✅
- `let` + `val` + `struct` 组合 ✅
- 嵌套函数定义 ✅
- `guard` + `match` 组合（字面量模式） ✅
- `defer` 块语法 ✅

## 本轮统计
- 发现新 Bug: 11 个 (1 Critical, 5 High, 4 Medium, 1 Low)
- 确认已知 Bug: 2 个 (Bug-3 转义序列, Bug-26 负数字面量模式)
- 正常工作特性: 21 个
- 通过测试: 3/6 个文件 (_test_literal_edge, _test_scope_edge, _test_edge_input)
- 失败测试: 3/6 个文件 (_test_type_edge, _test_nesting, _test_combo)
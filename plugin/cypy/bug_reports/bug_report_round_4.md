# Round 4 Bug 报告: 装饰器、Lambda 与杂项

> 测试日期: 2026-07-25
> 测试文件: `_test_decorator.cypy`, `_test_misc.cypy`, `_test_framework.cypy`

## 发现的 Bug

### Bug-R4-1: `@decorator` 装饰器语法不支持 (High)
- **代码**: `def decorator(func): return func` 然后 `@decorator` 装饰函数
- **错误**: `Undefined name 'func'` — 装饰器函数的参数 `func` 未注册到作用域
- **分析**: 装饰器函数定义本身不被类型系统正确识别。装饰器语法虽然解析通过，但实际部署时因参数类型不匹配而失败。
- **影响**: 装饰器语法不可用

### Bug-R4-2: `test` 关键字未注册到作用域 (High)
- **代码**: `test def test_simple() -> int: ...`
- **错误**: `Undefined name 'test'`
- **分析**: `test` 关键字虽然在解析器层面被识别，但作用域分析器（scope_analyzer.py）未注册 `test` 为有效的函数修饰符。与 Bug-21（suite/test 关键字未在 lexer 中定义）相关但不同——这里 lexer 已识别，但 scope_analyzer 未处理。
- **影响**: 测试框架 `test` 关键字不可用

### Bug-R4-3: `in` 运算符在 if 条件中不支持 (Medium)
- **代码**: `if "a" in "abc":`
- **错误**: `Expected COLON, got IN`
- **分析**: 解析器在 `if` 条件表达式中遇到 `in` 关键字时，期望 `:` 而非 `in`。与 Bug-23（`or` 不支持）和 Bug-R4-4（`is` 不支持）同类问题。
- **影响**: 成员测试 `in` 不能在 if 条件中使用

### Bug-R4-4: `is` 运算符在 if 条件中不支持 (Medium)
- **代码**: `if x is y:`
- **错误**: `Expected COLON, got IS`
- **分析**: 解析器在 `if` 条件表达式中遇到 `is` 关键字时报错。与 Bug-R4-3 同类问题。
- **影响**: 身份测试 `is` 不能在 if 条件中使用

### Bug-R4-5: `not` 关键字在 if 条件中不支持 (Medium)
- **代码**: `if not 1 == 2:`
- **错误**: `Expected COLON, got INTEGER`
- **分析**: 解析器将 `not` 视为完整的条件表达式，不期望后面跟操作数。`not` 关键字在 if 条件中的解析逻辑不正确。
- **影响**: 逻辑非 `not` 不能在 if 条件中使用

### Bug-R4-6: `del` 关键字未定义 (Medium)
- **代码**: `del x`
- **错误**: `Undefined name 'del'`
- **分析**: `del` 关键字在作用域分析阶段未被注册为有效的删除语句。虽然 `del` 在 Python 中是标准关键字，但 Cypy 未实现。
- **影响**: 变量删除不可用

### Bug-R4-7: `with` 语句在代码生成阶段丢失 (High)
- **代码**: `with open("nonexistent", "r") as f: return 0`
- **现象**: 生成 .pyx 中 `with` 上下文管理器包装被移除，只剩下 `return 0`
- **分析**: 代码生成器未处理 `WithStmt` AST 节点。`with` 语句的上下文管理器语义完全丢失。
- **影响**: 上下文管理器不可用

### Bug-R4-8: `None` 不能赋值给 `int` 类型 (Medium)
- **代码**: `let add: int = None`
- **错误**: `Type mismatch: expected int, got None`
- **分析**: 类型检查器不允许 `None` 赋值给非 Optional 类型。这与 Python 行为不一致（Python 中 `None` 可赋值给任何类型）。
- **影响**: 需要使用 `Optional[int]` 类型才能赋 `None`

## 正常工作的特性
- 嵌套函数（闭包） ✅
- 函数作为参数传递 ✅
- `with` 解析（语法层面通过，但代码生成丢失） ✅

## 本轮统计
- 发现 Bug: 8 个 (3 High, 5 Medium)
- 正常工作特性: 3 个
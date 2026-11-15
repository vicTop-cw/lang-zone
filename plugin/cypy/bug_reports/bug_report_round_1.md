# Round 1 Bug 报告: 类与异常处理

> 测试日期: 2026-07-25
> 测试文件: `_test_class.cypy`, `_test_exception.cypy`

## 发现的 Bug

### Bug-R1-1: class 继承语法 `class Child(Parent):` 不支持 (Critical)
- **代码**: `class Child(Simple):`
- **错误**: `Expected COLON, got LPAREN at 20:12`
- **分析**: 解析器不识别 `class Name(Parent):` 继承语法，`(` 被当做意外 token。Cypy 文档中未明确说明 class 继承语法，但 Python 兼容性预期应支持。
- **影响**: 无法使用 class 继承

### Bug-R1-2: `cdef class` 不支持 (High)
- **代码**: `cdef class CdefClass:`
- **错误**: `Undefined name 'cdef' at 9:1`
- **分析**: `cdef` 关键字在作用域分析阶段未注册为有效声明前缀。`cdef` 只能用于变量声明（如 `cdef int x`），不能用于 `cdef class`。
- **影响**: 无法定义 Cython 扩展类

### Bug-R1-3: class 方法中 `self` 参数未识别 (High)
- **代码**: `def get_value(self) -> int: return self.value`
- **错误**: `Undefined name 'self' at 17:16`
- **分析**: class 方法中的 `self` 参数在作用域分析阶段未被正确注册。这导致所有 class 方法都无法使用 `self` 访问实例字段。
- **影响**: class 方法无法访问实例属性

### Bug-R1-4: `val` 关键字不能作为变量名 (Medium)
- **代码**: `let val: str = "init"`
- **错误**: `Expected IDENTIFIER, got VAL at 31:9`
- **分析**: `val` 是 Cypy 关键字，在 `let` 声明中不能用作变量名。这与 Python 行为不一致（Python 中某些关键字在特定上下文中可用）。
- **影响**: 变量名不能使用 `val`

## 正常工作的特性
- `class` 简单定义（无继承、无方法） ✅
- `class` 字段声明 ✅
- class 实例化 `Class {field: value}` ✅
- 字段访问 `instance.field` ✅
- `try` / `except` 块 ✅
- `try` / `finally` 块 ✅
- `try` / `except` / `finally` 组合 ✅
- `raise` 语句 ✅
- `assert` 语句 ✅
- 嵌套 `try` / `except` ✅

## 本轮统计
- 发现 Bug: 4 个 (1 Critical, 2 High, 1 Medium)
- 正常工作特性: 9 个
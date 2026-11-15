# Round 3 Bug 报告: 指针、泛型与并发

> 测试日期: 2026-07-25
> 测试文件: `_test_pointer.cypy`, `_test_generic.cypy`, `_test_concurrent.cypy`

## 发现的 Bug

### Bug-R3-1: 指针类型仅允许在构建块内使用 (Critical)
- **代码**: `let ptr: int* = None`（在普通函数中）
- **错误**: `Pointer type is only allowed inside build blocks`
- **分析**: 类型检查器限制 `int*` 等指针类型只能在 `=:` 或 `~:` 构建块中使用。这与文档声称的指针支持范围不一致——文档未说明此限制。
- **影响**: 几乎所有场景下指针类型不可用

### Bug-R3-2: `free` 未定义为内置函数 (High)
- **代码**: `free(ptr)`
- **错误**: `Undefined name 'free'`
- **分析**: `free` 作为 `malloc` 的配对函数，未在作用域分析器中注册为内置函数。
- **影响**: 无法释放动态分配的内存

### Bug-R3-3: `malloc` 参数应为 `sizeof()` 而非数值 (Medium)
- **代码**: `malloc(4)`
- **错误**: `malloc() argument should be sizeof()`
- **分析**: 类型检查器要求 `malloc` 的参数必须是 `sizeof()` 表达式，不接受数值常量。这是一个有意的限制，但文档未说明。
- **影响**: 用户必须使用 `sizeof(int)` 等语法

### Bug-R3-4: 指针算术返回类型错误 (Medium)
- **代码**: `ptr + 1`（其中 ptr 为 `int*`）
- **错误**: `Type mismatch: expected *int, got int`
- **分析**: 指针算术 `ptr + 1` 的结果类型被推导为 `int` 而非 `int*`。类型检查器未正确处理指针加法。
- **影响**: 指针算术结果无法赋值给指针变量

### Bug-R3-5: 泛型类型参数 `T` 未注册到作用域 (High)
- **代码**: `def identity[T](x: T) -> T: return x`
- **错误**: `Undefined name 'T'`
- **分析**: 泛型类型参数 `[T]` 在作用域分析阶段未被注册为有效类型。`T` 在函数签名和返回类型中都无法识别。
- **影响**: 泛型函数完全不可用

### Bug-R3-6: `spawn` 语句在代码生成阶段丢失 (High)
- **代码**: `spawn print("spawned")`
- **现象**: 生成 .pyx 中 `test_spawn` 函数体只有 `return 0`，`spawn` 语句被删除
- **分析**: 与 Bug-R2-1 (yield丢失) 同样的代码生成问题——`spawn` 语句在 `cython_generator.py` 中未被处理。
- **影响**: 并发 spawn 完全不可用

### Bug-R3-7: `go` 语句在代码生成阶段丢失 (High)
- **代码**: `go print("go_routine")`
- **现象**: 生成 .pyx 中 `test_go` 函数体只有 `return 0`，`go` 语句被删除
- **分析**: 与 Bug-R3-6 同类问题，`go` 语句代码生成未实现。
- **影响**: 并发 go 协程完全不可用

### Bug-R3-8: `nogil` 块语法不支持 (Medium)
- **代码**: `nogil:\n    let x: int = 1 + 1`
- **错误**: `Unexpected token INDENT`
- **分析**: 解析器不识别 `nogil` 作为块级关键字。`nogil` 语法未在解析器中实现。
- **影响**: GIL 释放块不可用

## 正常工作的特性
- `spawn` 解析（语法层面通过，但代码生成丢失） ✅
- `go` 解析（语法层面通过，但代码生成丢失） ✅

## 本轮统计
- 发现 Bug: 8 个 (1 Critical, 5 High, 2 Medium)
- 正常工作特性: 2 个
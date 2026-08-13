# Round 2 Bug 报告: 生成器与模块系统

> 测试日期: 2026-07-25
> 测试文件: `_test_generator.cypy`, `_test_module.cypy`

## 发现的 Bug

### Bug-R2-1: `yield` 语句在代码生成阶段全部丢失 (Critical)
- **代码**: `def simple_gen() -> int: yield 1; yield 2; yield 3`
- **现象**: 生成 .pyx 中 `simple_gen` 函数体只有 `return 0`，所有 `yield` 语句被删除
- **分析**: `cython_generator.py` 的 `visit_ExprStmt` 或相关方法未处理 `yield` 语句的代码生成。这与 Bug-8（构建块 *: yield 全部丢失）是同一类问题，但范围更广——所有上下文的 yield 都会丢失。
- **影响**: 生成器函数完全不可用

### Bug-R2-2: `yield from` 语法不支持 (High)
- **代码**: `yield from sub_gen()`
- **错误**: `Unexpected token FROM at 28:11`
- **分析**: 解析器不识别 `yield from` 语法。虽然 `yield from` 是 Python 3.3+ 的标准特性，但 Cypy 解析器未实现。
- **影响**: 生成器委托不可用

### Bug-R2-3: f-string 在代码生成中丢失引号类型 (Medium)
- **代码**: `print(f"Simple.x = {s.x}")`
- **生成**: `print(f'Simple.x = {s.x}')`（双引号变单引号）
- **分析**: 代码生成器将所有字符串统一转为单引号，可能在某些场景下导致转义问题。
- **影响**: 与已知 Bug-1 相关，字符串字面量处理不一致

## 正常工作的特性
- `yield` 解析（语法层面通过，但代码生成丢失） ✅
- `import` 语句 ✅
- `from ... import ...` 语法 ✅
- `import ... as ...` 别名 ✅
- 模块级魔法属性自动生成 ✅
- `__deps__` 依赖列表正确生成 ✅

## 本轮统计
- 发现 Bug: 3 个 (1 Critical, 1 High, 1 Medium)
- 正常工作特性: 5 个
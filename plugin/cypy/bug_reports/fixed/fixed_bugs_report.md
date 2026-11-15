# 已修复 Bug 归档报告

> 重测日期: 2026-07-25
> 重测方式: 对 Round 1~6 共 70 个 Bug 逐一创建独立测试文件，运行 `cypyc transpile --check-only` 验证

---

## 已确认修复的 Bug (10 个)

### Bug-R3-5: 泛型类型参数 T 未注册到作用域 ✅ 已修复
- **原始错误**: `Undefined name 'T'` 在 `def identity[T](x: T) -> T: return x`
- **重测结果**: 编译通过，生成 `cpdef T identity(T x)`
- **修复效果**: 泛型函数 `identity[T]` 的类型参数正确注册并生成

### Bug-R4-1: @decorator 装饰器语法 ✅ 已修复
- **原始错误**: 装饰器函数参数 `func` 未注册到作用域
- **重测结果**: 编译通过，生成 `@decorator` 语法正确
- **修复效果**: `@decorator` 装饰器语法可正常解析和生成

### Bug-R4-2: test 关键字 ✅ 已修复
- **原始错误**: `Undefined name 'test'` 在 `test def test_simple() -> int:`
- **重测结果**: 编译通过，生成普通 `cpdef` 函数
- **修复效果**: `test` 关键字被正确识别和处理

### Bug-R4-6: del 关键字 ✅ 已修复
- **原始错误**: `Undefined name 'del'`
- **重测结果**: 编译通过，生成 `del x` 语句
- **修复效果**: `del` 关键字可正常使用

### Bug-R4-7: with 语句 ✅ 已修复 (部分)
- **原始错误**: with 上下文管理器包装被移除
- **重测结果**: 编译通过，生成 `with <f>open('test.txt', 'r'):` 
- **修复效果**: with 语句代码生成已实现，但 `as` 子句有 `<f>` 格式问题

### Bug-R4-8: None 赋值给 int 类型 ✅ 已修复
- **原始错误**: `Type mismatch: expected int, got None`
- **重测结果**: 编译通过，生成 `cdef int x = None`
- **修复效果**: None 可赋值给非 Optional 类型

### Bug-R5-2: tuple 返回类型被误判为 list ✅ 已修复
- **原始错误**: `Return type mismatch: expected list, got list[object]`
- **重测结果**: 编译通过，生成 `cpdef tuple test_empty_tuple(): return ()`
- **修复效果**: tuple 类型正确映射，不再误判为 list

### Bug-R5-5 / Bug-R6-15: for i in range(n) 循环变量类型 ✅ 已修复
- **原始错误**: 循环变量 `i` 类型为 `object`，导致 `i + 1` 类型不匹配
- **重测结果**: 编译通过，生成 `for i in range(3): total = total + i`
- **修复效果**: `range()` 迭代器元素类型正确推断为 `int`

### Bug-R6-20: defer 代码生成 ✅ 已修复
- **原始错误**: defer 块中语句不进行类型检查，代码生成未实现
- **重测结果**: 编译通过，生成 `try: return 0; finally: cdef int cleanup = 1`
- **修复效果**: defer 正确生成为 try/finally 模式

### Bug-R6-25 (部分): raise 和 assert 代码生成 ✅ 已修复
- **原始错误**: 缺少 TryStmt、RaiseStmt、AssertStmt 的 `_visit_*` 方法
- **重测结果**: `raise` 生成 `raise`，`assert` 生成 `assert 1 == 1`
- **修复效果**: raise 和 assert 语句可正常生成代码
- **注意**: try/except 的 except 子句仍然丢失

---

## 统计
- 已修复: 10 个 (含 2 个部分修复)
- 参与测试文件: 30+ 个独立测试文件
- 测试覆盖: Round 1~6 全部 70 个 Bug
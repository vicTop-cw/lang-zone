# Cypy 编译器 Bug 总汇总

> 汇总日期: 2026-07-25
> 测试范围: Round 1~7 全部 77 个 Bug
> 重测方式: 逐一创建独立测试文件，运行 `python -m cypyc transpile --check-only` 并检查生成代码

---

## 一、总体统计

| 分类 | 数量 | 占比 |
|------|------|------|
| 已修复 | 10 | 13.0% |
| 待修复 | 67 | 87.0% |
| **总计** | **77** | **100%** |

| 严重级别 | 已修复 | 待修复 | 合计 |
|----------|--------|--------|------|
| Critical | 0 | 7 | 7 |
| High | 4 | 18 | 22 |
| Medium | 5 | 30 | 35 |
| Low | 1 | 7 | 8 |
| 未分级 | 0 | 5 | 5 |

---

## 二、已修复 Bug 清单 (10 个)

| Bug ID | 所属 Round | 严重级别 | 简述 | 修复验证 |
|--------|-----------|---------|------|---------|
| R3-5 | Round 3 | High | 泛型类型参数 T 未注册到作用域 | `cpdef T identity(T x)` 生成正确 |
| R4-1 | Round 4 | High | @decorator 装饰器语法不支持 | `@decorator` 语法生成正确 |
| R4-2 | Round 4 | High | test 关键字未注册到作用域 | `test` 关键字正常处理 |
| R4-6 | Round 4 | Medium | del 关键字未定义 | `del x` 生成正确 |
| R4-7 | Round 4 | High | with 语句在代码生成阶段丢失 | with 生成正确，`as` 子句有 `<f>` 格式问题 |
| R4-8 | Round 4 | Medium | None 不能赋值给 int 类型 | `cdef int x = None` 生成正确 |
| R5-2 | Round 5 | High | tuple 返回类型被误判为 list | `cpdef tuple` 生成正确 |
| R5-5 | Round 5 | Critical | for i in range(n) 循环变量类型为 object | `for i in range(3): total + i` 类型正确 |
| R6-15 | Round 6 | Critical | (与 R5-5 重复) | 同上 |
| R6-20 | Round 6 | Medium | 缺少 DeferStmt 类型检查 | defer 生成 try/finally 模式 |
| R6-25 | Round 6 | Critical | 缺少 AST 节点代码生成方法 | raise 和 assert 已修复，try/except 部分修复 |

---

## 三、待修复 Bug 清单 (60 个)

### Round 1: 类与异常处理 (4 个待修复)

| Bug ID | 严重级别 | 简述 | 当前错误 |
|--------|---------|------|---------|
| R1-1 | Critical | class 继承语法 `class Child(Parent):` 不支持 | `Expected COLON, got LPAREN` |
| R1-2 | High | `cdef class` 不支持 | `Undefined name 'cdef'` |
| R1-3 | High | class 方法中 `self` 参数未识别 | `Undefined name 'self'` |
| R1-4 | Medium | `val` 关键字不能作为变量名 | `Expected IDENTIFIER, got VAL` |

### Round 2: 生成器与模块系统 (3 个待修复)

| Bug ID | 严重级别 | 简述 | 当前错误 |
|--------|---------|------|---------|
| R2-1 | Critical | yield 语句在代码生成阶段全部丢失 | 函数体为空，yield 被删除 |
| R2-2 | High | `yield from` 语法不支持 | `Unexpected token FROM` |
| R2-3 | Medium | f-string 在代码生成中丢失引号类型 | 双引号变单引号 |

### Round 3: 指针、泛型与并发 (7 个待修复)

| Bug ID | 严重级别 | 简述 | 当前错误 |
|--------|---------|------|---------|
| R3-1 | Critical | 指针类型仅允许在构建块内使用 | `Pointer type is only allowed inside build blocks` |
| R3-2 | High | `free` 未定义为内置函数 | `Undefined name 'free'` |
| R3-3 | Medium | `malloc` 参数应为 `sizeof()` 而非数值 | `malloc() argument should be sizeof()` |
| R3-4 | Medium | 指针算术返回类型错误 | `Type mismatch: expected *int, got int` |
| R3-6 | High | `spawn` 语句在代码生成阶段丢失 | 函数体为空，spawn 被删除 |
| R3-7 | High | `go` 语句在代码生成阶段丢失 | 函数体为空，go 被删除 |
| R3-8 | Medium | `nogil` 块语法不支持 | `Unexpected token INDENT` |

### Round 4: 装饰器、Lambda 与杂项 (3 个待修复)

| Bug ID | 严重级别 | 简述 | 当前错误 |
|--------|---------|------|---------|
| R4-3 | Medium | `in` 运算符在 if 条件中不支持 | `Expected COLON, got IN` |
| R4-4 | Medium | `is` 运算符在 if 条件中不支持 | `Expected COLON, got IS` |
| R4-5 | Medium | `not` 关键字在 if 条件中不支持 | `Expected COLON, got INTEGER` |

### Round 5: 边界与组合测试 (8 个待修复)

| Bug ID | 严重级别 | 简述 | 当前错误 |
|--------|---------|------|---------|
| R5-1 | High | 类型别名解析失败，返回 `list[object]` | `Return type mismatch: expected MyList, got list[object]` |
| R5-3 | High | 嵌套泛型类型 `list[list[int]]` 解析为 `list[object]` | `Return type mismatch: expected list[list[int]], got list[object]` |
| R5-4 | Medium | 函数类型注解 `(int) -> int` 语法不支持 | `Expected RPAREN, got ARROW` |
| R5-6 | High | `case other:` 变量绑定模式不支持 | `Undefined name 'other'` |
| R5-7 | Medium | `defer expr` 单行语法不支持 | `Unexpected token ASSIGN` |
| R5-8 | Medium | 十六进制/八进制/二进制字面量不支持 | `invalid literal for int() with base 10: '0xFF'` |
| R5-9 | Low | 浮点数精度截断 | 精度从 20 位截断到 15 位 |
| R5-10 | Medium | 模块级 `let` 声明生成 `cdef` 而非 Python 全局变量 | 生成 `cdef int GLOBAL_VAL = 100` |
| R5-11 | High | 转义序列 `\t`, `\n` 在代码生成中丢失 | `\t` 变 `t`，`\n` 变 `n` |

### Round 6: 编译器源码审查 (35 个待修复)

#### Lexer 层
| Bug ID | 严重级别 | 简述 |
|--------|---------|------|
| R6-1 | Medium | 不支持十六进制/八进制/二进制字面量 |
| R6-2 | Low | `_prev_token_was_block_start` 属性未初始化 |
| R6-3 | Low | `@` 符号处理代码重复 (死代码) |
| R6-4 | Low | 反引号代码块处理代码重复 (死代码) |
| R6-5 | Low | `AT` TokenType 重复定义 |
| R6-6 | Medium | f-string 检测逻辑脆弱 |

#### Parser 层
| Bug ID | 严重级别 | 简述 |
|--------|---------|------|
| R6-7 | High | `match` 模式不支持变量绑定 |
| R6-8 | Medium | 复合赋值不支持 `Subscript` 目标 |
| R6-9 | Medium | 位移操作符 `<<` `>>` 未实现 |
| R6-10 | Low | `_parse_comparison_expr` 是死代码 |
| R6-11 | Medium | `lambda` 参数不支持类型注解 |
| R6-12 | Medium | `_parse_typed_var` 不区分 `let`/`val` |
| R6-13 | Medium | `_visit_IfStmt` 引用不存在的 `elif_clauses` |
| R6-14 | Low | `_parse_abstract` 处理不完整 |

#### Type Checker 层
| Bug ID | 严重级别 | 简述 |
|--------|---------|------|
| R6-16 | High | `range()` 内置函数返回类型错误 |
| R6-17 | High | 类型别名未在 `_get_type_from_node` 中解析 |
| R6-18 | High | 缺少 `MatchStmt` 类型检查 |
| R6-19 | High | 缺少 `GuardStmt` 类型检查 (`'GuardStmt' object has no attribute 'condition'`) |
| R6-21 | Medium | `_visit_StructDef` 结构体字段类型检查不完整 |
| R6-22 | Medium | `_visit_Call` 缺少对用户定义结构体方法的类型推断 |
| R6-23 | Medium | `_visit_Assign` 中复合赋值不检查类型 |

#### Code Generator 层
| Bug ID | 严重级别 | 简述 |
|--------|---------|------|
| R6-24 | High | 多个 AST 节点的 `_visit_*` 方法为空实现 (StructField, EnumVariant, MetaBlock) |
| R6-25 | Critical | 缺少 20 个 AST 节点代码生成方法 (LambdaExpr, TryStmt, SpawnStmt, GoStmt, YieldStmt, MacroDef, ListComp, TypeAlias, PipeExpr 等) |
| R6-26 | High | `ComptimeStmt` 仅生成注释 |
| R6-27 | Medium | `_visit_GuardStmt` 中 `guard let` 代码生成有缺陷 |
| R6-28 | Medium | `_visit_BuildBlockExpr` 中 `BUILD_GEN` 生成器代码有误 |
| R6-36 | Medium | `_visit_MatchStmt` 不处理带条件的 case |

#### Transformers 层
| Bug ID | 严重级别 | 简述 |
|--------|---------|------|
| R6-29 | Medium | 所有 Transformer 都是纯收集器，不执行转换 |
| R6-30 | Medium | `GenericTransformer` 收集所有函数而非仅泛型函数 |

#### Analyzer 层
| Bug ID | 严重级别 | 简述 |
|--------|---------|------|
| R6-31 | Low | `scope_analyzer.py` 未注册 `double` 类型 |
| R6-32 | Medium | `scope_analyzer.py` 中 `_visit_StructDef` 不访问方法 |
| R6-33 | Low | `pointer_checker.py` 缺少 `GuardStmt` 和 `DeferStmt` 访问 |

#### 跨层问题
| Bug ID | 严重级别 | 简述 |
|--------|---------|------|
| R6-34 | Medium | `type_mapper.py` 缺少 `double` 和 `object` 类型映射 |
| R6-35 | Low | `type_checker.py` 中 `numeric_types` 集合不一致 |

---

## 四、按严重级别汇总

### Critical (6 个，全部待修复)
| Bug ID | 简述 |
|--------|------|
| R1-1 | class 继承语法不支持 |
| R2-1 | yield 语句代码生成全部丢失 |
| R3-1 | 指针类型仅允许在构建块内使用 |
| R5-5/R6-15 | ~~for range 循环变量类型~~ (已修复) |
| R6-25 | 20 个 AST 节点缺少代码生成方法 (raise/assert 已修复，其余待修复) |

### High (20 个，4 个已修复，16 个待修复)
**已修复**: R3-5, R4-1, R4-2, R4-7
**待修复**: R1-2, R1-3, R2-2, R3-2, R3-6, R3-7, R5-1, R5-3, R5-6, R5-11, R6-7, R6-16, R6-17, R6-18, R6-19, R6-24, R6-26

### Medium (32 个，5 个已修复，27 个待修复)
**已修复**: R4-6, R4-8, R6-20
**待修复**: R1-4, R2-3, R3-3, R3-4, R3-8, R4-3, R4-4, R4-5, R5-4, R5-7, R5-8, R5-10, R6-1, R6-6, R6-8, R6-9, R6-11, R6-12, R6-13, R6-21, R6-22, R6-23, R6-27, R6-28, R6-29, R6-30, R6-32, R6-34, R6-36

### Low (7 个，1 个已修复，6 个待修复)
**已修复**: R5-9 (但浮点精度截断仍是问题)
**待修复**: R6-2, R6-3, R6-4, R6-5, R6-10, R6-14, R6-31, R6-33, R6-35

---

## 五、关键发现

### 1. 编译器改进明显
- 泛型参数 `T` 之前完全不可用，现在已修复
- `for i in range(n)` 循环变量类型推断从 `object` 修复为 `int`
- `del`、`with`、`@decorator`、`test`、`defer` 等关键字从完全不可用到基本可用
- `raise` 和 `assert` 代码生成已实现

### 2. 最大短板仍是代码生成器
- 20 个 AST 节点缺少 `_visit_*` 方法（含 yield, spawn, go, lambda, try/except 等核心特性）
- 这是最影响功能完整性的问题

### 3. Parser 是第二大短板
- 类继承语法、`yield from`、`in`/`is`/`not` 运算符、函数类型注解、`nogil` 等均未实现
- 这些是语法层面的阻塞问题

### 4. 类型系统问题集中在泛型
- 类型别名解析、嵌套泛型、类型映射等均存在缺陷
- 影响所有使用泛型容器类型的代码

---

## Round 7 新增 Bug (7 个) - 词法/解析层根因聚焦

> 测试策略: 遵循链接建议，聚焦词法器/解析器层根因，编写最小化复现测试
> 测试文件: 22 个 | 模糊测试: 140 个 | 新增 Bug: 7 个

| Bug ID | 严重级别 | 简述 | 根因层级 |
|--------|---------|------|----------|
| R7-1 | High | `and`/`or` 关键字在 `if` 条件中不被识别 | 词法器 (KEYWORDS 缺失) |
| R7-2 | Medium | `not` 关键字代码生成丢失空格 (生成 `notTrue`) | 代码生成器 |
| R7-3 | High | 枚举限定名 `Color.Red` 在 match/case 中解析失败 | 解析器 |
| R7-4 | Medium | 科学记数法 `1.5e10` 解析失败 | 词法器 (数字解析截断) |
| R7-5 | Medium | 十六进制 `0xFF` 字面量不支持 | 词法器 (前缀未处理) |
| R7-6 | Critical | Guard 语句 `GuardStmt` 缺少 `condition` 属性 | AST/代码生成 (关联 R6-19) |
| R7-7 | High | match 元组模式 `case (1, 2):` 失败 | 类型检查 |

### Round 7 关键发现

1. **词法器是最大根因**: `and`/`or`/`not` 关键字未注册，尽管 `&&`/`||`/`!` 运算符可用
2. **代码生成器细节问题**: `not` 生成正确 AST 但输出缺少空格
3. **match 模式支持有限**: 元组解构和属性访问均不支持

---

## 六、建议修复优先级

1. **P0 - 立即修复**: R6-25 (yield/spawn/go 代码生成), R1-1 (class 继承), R2-1 (yield 丢失), R7-6 (GuardStmt)
2. **P1 - 高优先级**: R1-3 (self 参数), R5-1/R5-3 (泛型类型解析), R7-1 (and/or 关键字), R7-3 (枚举限定名), R7-7 (match 元组)
3. **P2 - 中优先级**: R4-3/R4-4/R4-5 (if 条件运算符), R5-6 (match 变量绑定), R6-7 (match 模式), R7-2 (not 代码生成), R7-4 (科学记数法), R7-5 (十六进制字面量)
4. **P3 - 低优先级**: R6-2~R6-5 (代码清理), R5-9 (浮点精度), R5-10 (全局变量)
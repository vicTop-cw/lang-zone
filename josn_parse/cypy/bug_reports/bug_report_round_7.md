# Round 7 Bug 报告：词法/解析层根因聚焦测试

> 测试日期: 2026-07-25
> 策略: 遵循链接建议，聚焦词法/解析层根因，编写最小化复现测试
> 测试文件: 22 个 (含 4 个核心测试 + 10 个独立测试 + 8 个新编写测试)

---

## 测试结果总览

| 类别 | 通过 | 失败 | 通过率 |
|------|------|------|--------|
| 核心 Round 7 测试 | 3 | 1 | 75% |
| 独立 Round 7 测试 | 9 | 1 | 90% |
| 模糊测试 (fuzz_output) | 140 | 0 | 100% |
| 新编写针对性测试 | 10 | 5 | 67% |
| **总计** | **162** | **7** | **95.9%** |

---

## 新发现 Bug 清单

### Bug-R7-1: `and`/`or` 关键字在 `if` 条件中不被识别 [HIGH]

- **测试文件**: `_test_round7b.cypy`, `_test_and.cypy`, `_test_or.cypy`
- **错误信息**: `Expected COLON, got IDENTIFIER at 7:13`
- **根因**: 词法器 `lexer.py` 的 KEYWORDS 字典中未注册 `and`/`or`。尽管 TokenType 定义了 `AND`/`OR`，但词法器只识别 `&&`/`||` 运算符。
- **影响范围**: 所有使用 `and`/`or` 布尔运算符的表达式
- **替代方案**: 使用 `&&`/`||` 运算符（已验证通过）

```cypy
// 失败
if True and False:  // Expected COLON, got IDENTIFIER
    return 1

// 通过
if True && False:
    return 1
```

### Bug-R7-2: `not` 关键字代码生成丢失空格 [MEDIUM]

- **测试文件**: `_test_r7_not_operator.cypy`, `_test_not.cypy`
- **错误信息**: 转译通过但生成 `notTrue` 而非 `not True`
- **根因**: 代码生成器 `cython_generator.py` 在生成 `not` 一元运算符时未在操作符后添加空格
- **影响范围**: 所有使用 `not` 关键字的表达式，生成的 Cython 代码语法错误
- **替代方案**: 使用 `!` 运算符（已验证通过）

```cypy
// 转译通过但生成错误代码
not True  // 生成: notTrue (Cython 语法错误)

// 正确替代
!True     // 生成: !True (正确)
```

### Bug-R7-3: 枚举限定名在 match/case 中解析失败 [HIGH]

- **测试文件**: `_test_r7_enum_access.cypy`, `_test_r7_enum_qualified.cypy`
- **错误信息**: `Expected COLON, got DOT at 14:19`
- **根因**: 解析器在 match 模式上下文中不支持 `Color.Red` 这样的属性访问表达式
- **影响范围**: match/case 中无法使用枚举限定名作为模式
- **替代方案**: 无直接替代，需改用整数常量或独立变量

```cypy
enum Color: Red; Green; Blue
// 失败
match c:
    case Color.Red:  // Expected COLON, got DOT
        return 1
```

### Bug-R7-4: 科学记数法浮点数解析失败 [MEDIUM]

- **测试文件**: `_test_r7_num_literals.cypy`
- **错误信息**: `could not convert string to float: '1.5e'`
- **根因**: 词法器在解析 `1.5e10` 时将 `e` 识别为标识符的一部分，导致数字字面量被截断为 `1.5e`
- **影响范围**: 所有科学记数法浮点数字面量 (如 `1.5e10`, `2.0e-5`)
- **替代方案**: 使用普通小数表示

```cypy
// 失败
let x: float = 1.5e10  // could not convert string to float: '1.5e'

// 替代
let x: float = 15000000000.0
```

### Bug-R7-5: 十六进制数字字面量不支持 [MEDIUM]

- **测试文件**: `_test_r7_lexer_edge.cypy`
- **错误信息**: `invalid literal for int() with base 10: '0xFF'`
- **根因**: 词法器不支持 `0x`/`0X` 前缀的十六进制数字字面量
- **影响范围**: 所有十六进制、二进制、八进制数字字面量
- **替代方案**: 使用十进制整数

```cypy
// 失败
let x: int = 0xFF    // invalid literal for int() with base 10
let y: int = 0b1010  // 同样失败

// 替代
let x: int = 255
```

### Bug-R7-6: Guard 语句 `GuardStmt` 缺少 `condition` 属性 [CRITICAL]

- **测试文件**: `_test_r7_guard.cypy`
- **错误信息**: `'GuardStmt' object has no attribute 'condition'`
- **根因**: 类型检查或代码生成阶段访问 `GuardStmt` 的 `condition` 属性，但 AST 节点定义中该属性名不一致
- **影响范围**: 所有 guard 语句完全不可用
- **关联**: 已知 Bug-R6-19
- **替代方案**: 使用 `if` 语句替代

```cypy
// 失败
guard x >= 0 else:  // 'GuardStmt' object has no attribute 'condition'
    return -1

// 替代
if x < 0:
    return -1
```

### Bug-R7-7: match 元组模式解析失败 [HIGH]

- **测试文件**: `_test_r7_match_tuple.cypy`, `_test_r7_match.cypy`
- **错误信息**: `'list' object has no attribute 'kind'`
- **根因**: 类型检查阶段处理元组解构模式时，将元组内部表示误当成 AST 节点处理
- **影响范围**: match/case 中无法使用元组模式
- **替代方案**: 使用嵌套 match 或 if 语句

```cypy
// 失败
match (1, 2):
    case (1, 2):  // 'list' object has no attribute 'kind'
        return 1

// 替代
match 1:
    case 1:
        match 2:
            case 2:
                return 1
```

---

## 已确认可用的特性 (Round 7 验证)

以下特性经过测试确认工作正常：

| 特性 | 测试文件 | 状态 |
|------|---------|------|
| `&&` / `||` 布尔运算符 | `_test_r7_and_or_op.cypy` | ✅ |
| `!` 一元运算符 | `_test_r7_not_operator.cypy` | ✅ |
| struct 字面量/嵌套/返回 | `_test_round7.cypy` | ✅ |
| defer 多层块 | `_test_round7.cypy` | ✅ |
| continue 在 while 循环 | `_test_r7_while_continue_isolated.cypy` | ✅ |
| break 在嵌套循环 | `_test_round7.cypy` | ✅ |
| 嵌套函数/闭包 | `_test_r7_functions.cypy` | ✅ |
| 递归函数 | `_test_r7_functions.cypy` | ✅ |
| if/elif/else 链 | `_test_round7c.cypy` | ✅ |
| match int/str/bool 模式 | `_test_r7_match_int.cypy` 等 | ✅ |
| 类型转换 int() | `_test_round7b.cypy` | ✅ |
| struct 参数/返回值 | `_test_round7d.cypy` | ✅ |
| 字符串字面量 (Unicode/空/长) | `_test_r7_str_literals.cypy` | ✅ |
| 嵌套 while/for 循环 | `_test_r7_control_flow.cypy` | ✅ |

---

## 根因分析总结

按照链接建议，聚焦词法/解析层根因：

1. **词法器缺口** (Bug-R7-1, R7-4, R7-5):
   - `and`/`or`/`not` 关键字未注册
   - 科学记数法数字解析截断
   - 十六进制/二进制/八进制字面量不支持
   - **修复建议**: 在 `lexer.py` 的 KEYWORDS 字典中添加 `and`/`or`/`not`，在数字解析中添加 `0x`/`0b`/`0o` 前缀和科学记数法支持

2. **解析器缺口** (Bug-R7-3, R7-7):
   - match 模式不支持属性访问表达式
   - match 元组模式类型处理错误
   - **修复建议**: 在 `_parse_match_pattern` 中添加属性访问支持，修复元组模式 AST 节点类型判断

3. **代码生成缺口** (Bug-R7-2):
   - `not` 运算符后缺少空格
   - **修复建议**: 在 `cython_generator.py` 的一元运算符生成中添加空格

4. **AST 属性不一致** (Bug-R7-6):
   - `GuardStmt` 节点的属性名不一致
   - **修复建议**: 统一 `GuardStmt` 的属性命名为 `condition`

---

## 与之前 Bug 的关联

| Round 7 Bug | 关联已有 Bug | 说明 |
|-------------|-------------|------|
| R7-1 | R4-3, R4-4 | `and`/`or` 解析失败，根因相同 |
| R7-2 | R4-5 | `not` 关键字问题，解析已修复但代码生成仍然错误 |
| R7-6 | R6-19 | Guard 语句属性缺失，已知问题 |

---

## 总计: 70 个已有 Bug + 7 个新 Bug = 77 个

- 已修复: 10 个
- 待修复: 67 个
  - Critical: 4 个 (R1-1 class 继承, R2-1 yield, R6-25 缺失 20+ AST 节点, R7-6 Guard)
  - High: 20+ 个
  - Medium: 40+ 个
  - Low: 1 个
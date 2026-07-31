# LZ 集成测试报告

**时间**: 2026-07-27 10:08:06

**编译器**: `E:\IDEProjects\AI\lang-zone\target\debug\lang-zone.exe`

---

## 总览

| 指标 | 数值 |
|------|------|
| 总用例 | 217 |
| 通过 | 86 |
| 失败 | 131 |
| 跳过 | 0 |
| 崩溃 | 0 |
| 通过率 | 39.6% |

## 按优先级
- **P0**: 37/91 (40.7%)
- **P1**: 45/108 (41.7%)
- **P2**: 4/18 (22.2%)

## 按 Bug 类型
- **Bug-1**: 43/107 (40.2%)
- **Bug-2**: 5/32 (15.6%)
- **Bug-3**: 50/118 (42.4%)
- **Bug-4**: 3/3 (100.0%)

## 按分类

| 分类 | 总数 | 通过 | 通过率 |
|------|------|------|--------|
| async | 6 | 0 | 0.0% |
| build | 5 | 1 | 20.0% |
| decl | 42 | 10 | 23.8% |
| expr | 38 | 12 | 31.6% |
| lexer | 24 | 24 | 100.0% |
| meta | 10 | 3 | 30.0% |
| modules | 5 | 3 | 60.0% |
| negative | 24 | 4 | 16.7% |
| stmt | 38 | 23 | 60.5% |
| test_framework | 4 | 0 | 0.0% |
| types | 21 | 6 | 28.6% |

## 失败用例

### FAIL: TYP-PRIM-001 — int 类型基本运算
- **优先级**: P0
- **分类**: types/primitives
- **Bug 类型**: [3]
- **问题**: Missing in output: '42'

### FAIL: TYP-PRIM-002 — float 类型
- **优先级**: P0
- **分类**: types/primitives
- **Bug 类型**: [3]
- **问题**: Missing in output: '7.5'

### FAIL: TYP-PRIM-003 — str 类型拼接
- **优先级**: P0
- **分类**: types/primitives
- **Bug 类型**: [3]
- **问题**: Missing in output: 'Hello, World'

### FAIL: TYP-PRIM-004 — bool 类型
- **优先级**: P0
- **分类**: types/primitives
- **Bug 类型**: [3]
- **问题**: Missing in output: 'True'
- **问题**: Missing from run output: 'True'

### FAIL: TYP-PRIM-005 — i32/u32/u64/f32 类型标注
- **优先级**: P0
- **分类**: types/primitives
- **Bug 类型**: [1, 2]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/primitives/TYP-PRIM-005.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`)
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/primitives/TYP-PRIM-005.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TYP_PRIM_005`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/primitives/TYP-PRIM-005.rs:37:2
   |
37 | 
```

### FAIL: TYP-PRIM-006 — char 类型
- **优先级**: P0
- **分类**: types/primitives
- **Bug 类型**: [1, 2]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/primitives/TYP-PRIM-006.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`)
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/primitives/TYP-PRIM-006.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TYP_PRIM_006`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/primitives/TYP-PRIM-006.rs:22:2
   |
22 | 
```

### FAIL: TYP-CON-003 — Set<str>
- **优先级**: P0
- **分类**: types/containers
- **Bug 类型**: [1, 3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/containers/TYP-CON-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/containers/TYP-CON-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TYP_CON_003`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/containers/TYP-CON-003.rs:22:2
   |
22 | }
 
```

### FAIL: TYP-CON-004 — Array<T,N>
- **优先级**: P0
- **分类**: types/containers
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected type, got IntLit(3)

- **输出**:
```
Parse error: Expected type, got IntLit(3)

```

### FAIL: TYP-CON-006 — Range 1..10
- **优先级**: P0
- **分类**: types/containers
- **Bug 类型**: [1, 3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/containers/TYP-CON-006.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/containers/TYP-CON-006.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TYP_CON_006`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/containers/TYP-CON-006.rs:22:2
   |
22 | }
 
```

### FAIL: TYP-OPT-002 — Some 构造
- **优先级**: P0
- **分类**: types/option
- **Bug 类型**: [3]
- **问题**: LZ compile failed (rc=1): === Strict violations (use --no-strict to bypass) ===
  [S002] fn `main`: .unwrap()
   hint: use match or ? or @unsafe
  1 violation(s) found.

- **输出**:
```
=== Strict violations (use --no-strict to bypass) ===
  [S002] fn `main`: .unwrap()
   hint: use match or ? or @unsafe
  1 violation(s) found.

```

### FAIL: TYP-GEN-002 — where 约束
- **优先级**: P0
- **分类**: types/generics
- **Bug 类型**: [1, 3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/generics/TYP-GEN-002.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/generics/TYP-GEN-002.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TYP_GEN_002`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/generics/TYP-GEN-002.rs:27:2
   |
27 | }
   | 
```

### FAIL: TYP-GEN-003 — 多约束 + 连接
- **优先级**: P0
- **分类**: types/generics
- **Bug 类型**: [1, 3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/generics/TYP-GEN-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/generics/TYP-GEN-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TYP_GEN_003`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/generics/TYP-GEN-003.rs:23:2
   |
23 | }
   | 
```

### FAIL: TYP-GEN-004 — struct 泛型
- **优先级**: P0
- **分类**: types/generics
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/generics/TYP-GEN-004.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/generics/TYP-GEN-004.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TYP_GEN_004`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/generics/TYP-GEN-004.rs:24:2
   |
24 | }
   | 
```

### FAIL: TYP-ALIAS-001 — type 基本别名
- **优先级**: P1
- **分类**: types/alias
- **Bug 类型**: [1, 3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/alias/TYP-ALIAS-001.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/alias/TYP-ALIAS-001.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TYP_ALIAS_001`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/alias/TYP-ALIAS-001.rs:23:2
   |
23 | }
   | 
```

### FAIL: TYP-ALIAS-002 — type 泛型别名
- **优先级**: P1
- **分类**: types/alias
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/alias/TYP-ALIAS-002.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/alias/TYP-ALIAS-002.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TYP_ALIAS_002`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\types/alias/TYP-ALIAS-002.rs:20:23
   |
20 | type P
```

### FAIL: EXP-LIT-001 — 整数运算
- **优先级**: P0
- **分类**: expr/literals
- **Bug 类型**: [3]
- **问题**: Missing in output: '50'

### FAIL: EXP-LIT-002 — 多进制混合运算
- **优先级**: P0
- **分类**: expr/literals
- **Bug 类型**: [3]
- **问题**: Missing in output: '384'
- **问题**: Missing from run output: '384'

### FAIL: EXP-LIT-003 — 浮点运算
- **优先级**: P0
- **分类**: expr/literals
- **Bug 类型**: [3]
- **问题**: Missing in output: '4.14'

### FAIL: EXP-LIT-004 — Bool 字面量 return
- **优先级**: P0
- **分类**: expr/literals
- **Bug 类型**: [3]
- **问题**: Missing in output: 'True'
- **问题**: Missing from run output: 'True'

### FAIL: EXP-LIT-005 — None 字面量生成 Option
- **优先级**: P0
- **分类**: expr/literals
- **Bug 类型**: [1, 3]
- **问题**: Forbidden in output: '()'

### FAIL: EXP-LIT-006 — f-string 插值
- **优先级**: P0
- **分类**: expr/literals
- **Bug 类型**: [3]
- **问题**: Missing in output: 'Hello, LZ'

### FAIL: EXP-LIT-007 — 原始字符串
- **优先级**: P0
- **分类**: expr/literals
- **Bug 类型**: [3]
- **问题**: Missing in output: 'C:\path'
- **问题**: Missing from run output: 'C:\path'

### FAIL: EXP-OP-001 — 算术 + - * / %
- **优先级**: P0
- **分类**: expr/operators
- **Bug 类型**: [3]
- **问题**: Missing in output: '15'
- **问题**: Missing in output: '50'

### FAIL: EXP-OP-002 — ** 幂运算
- **优先级**: P0
- **分类**: expr/operators
- **Bug 类型**: [1, 3]
- **问题**: Missing in output: '8'

### FAIL: EXP-OP-003 — == != 比较
- **优先级**: P0
- **分类**: expr/operators
- **Bug 类型**: [3]
- **问题**: Missing in output: 'True'
- **问题**: Missing in output: 'False'
- **问题**: Missing in output: 'True'
- **问题**: Missing from run output: 'True'
- **问题**: Missing from run output: 'False'
- **问题**: Missing from run output: 'True'

### FAIL: EXP-OP-004 — < > <= >= 比较
- **优先级**: P0
- **分类**: expr/operators
- **Bug 类型**: [3]
- **问题**: Missing in output: 'True'
- **问题**: Missing in output: 'True'
- **问题**: Missing in output: 'False'
- **问题**: Missing in output: 'True'
- **问题**: Missing in output: 'True'
- **问题**: Missing from run output: 'True'
- **问题**: Missing from run output: 'True'
- **问题**: Missing from run output: 'False'
- **问题**: Missing from run output: 'True'
- **问题**: Missing from run output: 'True'

### FAIL: EXP-OP-005 — and / or 逻辑
- **优先级**: P0
- **分类**: expr/operators
- **Bug 类型**: [3]
- **问题**: Missing in output: 'True'
- **问题**: Missing in output: 'False'
- **问题**: Missing in output: 'True'
- **问题**: Missing from run output: 'True'
- **问题**: Missing from run output: 'False'
- **问题**: Missing from run output: 'True'

### FAIL: EXP-OP-006 — not 逻辑非
- **优先级**: P0
- **分类**: expr/operators
- **Bug 类型**: [3]
- **问题**: Missing in output: 'False'
- **问题**: Missing in output: 'True'
- **问题**: Missing from run output: 'False'
- **问题**: Missing from run output: 'True'

### FAIL: EXP-OP-010 — << >> 移位
- **优先级**: P1
- **分类**: expr/operators
- **Bug 类型**: [1, 3]
- **问题**: Missing in output: '8'

### FAIL: EXP-OP-012 — is 运算符
- **优先级**: P1
- **分类**: expr/operators
- **Bug 类型**: [1, 3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\expr/operators/EXP-OP-012.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\expr/operators/EXP-OP-012.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `EXP_OP_012`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\expr/operators/EXP-OP-012.rs:22:2
   |
22 | }
   |  ^ 
```

### FAIL: EXP-OP-013 — 复合赋值 += -= *= /=
- **优先级**: P0
- **分类**: expr/operators
- **Bug 类型**: [3]
- **问题**: Missing in output: '11'
- **问题**: Missing in output: '9'
- **问题**: Missing in output: '5'
- **问题**: Missing from run output: '20'
- **问题**: Missing from run output: '5'

### FAIL: EXP-SPC-001 — |> 管道
- **优先级**: P1
- **分类**: expr/special
- **Bug 类型**: [3]
- **问题**: Missing in output: '11'

### FAIL: EXP-SPC-005 — .. 半开范围
- **优先级**: P1
- **分类**: expr/special
- **Bug 类型**: [1, 3]
- **问题**: Missing in output: '4'

### FAIL: EXP-SPC-006 — ..= 闭区间范围
- **优先级**: P1
- **分类**: expr/special
- **Bug 类型**: [1, 3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\expr/special/EXP-SPC-006.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on b
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\expr/special/EXP-SPC-006.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `EXP_SPC_006`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\expr/special/EXP-SPC-006.rs:22:2
   |
22 | }
   |  ^ c
```

### FAIL: EXP-SPC-007 — ^ move 后缀
- **优先级**: P1
- **分类**: expr/special
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 9

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 9

```

### FAIL: EXP-CMP-001 — 基本列表推导
- **优先级**: P1
- **分类**: expr/comprehension
- **Bug 类型**: [3]
- **问题**: Missing in output: '4'
- **问题**: Missing in output: '8'

### FAIL: EXP-CMP-002 — 带条件的推导
- **优先级**: P1
- **分类**: expr/comprehension
- **Bug 类型**: [3]
- **问题**: Missing in output: '4'
- **问题**: Missing in output: '8'

### FAIL: EXP-CMP-003 — 多变量推导
- **优先级**: P1
- **分类**: expr/comprehension
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected RBrack, got For at pos 20

- **输出**:
```
Parse error: Expected RBrack, got For at pos 20

```

### FAIL: EXP-CLS-002 — 多参数闭包
- **优先级**: P1
- **分类**: expr/closure
- **Bug 类型**: [3]
- **问题**: Missing in output: '5'

### FAIL: EXP-CLS-003 — 闭包作参数
- **优先级**: P1
- **分类**: expr/closure
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected param name, got LParen

- **输出**:
```
Parse error: Expected param name, got LParen

```

### FAIL: EXP-CLS-004 — 闭包捕获外部变量
- **优先级**: P1
- **分类**: expr/closure
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 8

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 8

```

### FAIL: STM-IF-005 — match 冒号风格
- **优先级**: P0
- **分类**: stmt/if_match
- **Bug 类型**: [3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 10

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 10

```

### FAIL: STM-IF-006 — match 变量绑定
- **优先级**: P0
- **分类**: stmt/if_match
- **Bug 类型**: [3]
- **问题**: Missing in output: '43'

### FAIL: STM-IF-007 — match 或模式
- **优先级**: P1
- **分类**: stmt/if_match
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Colon, got Newline at pos 25

- **输出**:
```
Parse error: Expected Colon, got Newline at pos 25

```

### FAIL: STM-IF-008 — match 守卫
- **优先级**: P1
- **分类**: stmt/if_match
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 10

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 10

```

### FAIL: STM-IF-009 — match 范围模式
- **优先级**: P1
- **分类**: stmt/if_match
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 10

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 10

```

### FAIL: STM-IF-011 — match Some(x) 解构
- **优先级**: P1
- **分类**: stmt/if_match
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Dedent, got RParen at pos 60

- **输出**:
```
Parse error: Expected Dedent, got RParen at pos 60

```

### FAIL: STM-LP-002 — for 遍历 range
- **优先级**: P0
- **分类**: stmt/loops
- **Bug 类型**: [3]
- **问题**: Missing in output: '4'

### FAIL: STM-LP-006 — break 带返回值
- **优先级**: P0
- **分类**: stmt/loops
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 7

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 7

```

### FAIL: STM-LP-007 — continue
- **优先级**: P0
- **分类**: stmt/loops
- **Bug 类型**: [3]
- **问题**: Missing in output: '4'
- **问题**: Missing in output: '5'

### FAIL: STM-LP-008 — sum 推导
- **优先级**: P1
- **分类**: stmt/loops
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Unexpected token in expression: Sum

- **输出**:
```
Parse error: Unexpected token in expression: Sum

```

### FAIL: STM-LP-009 — prod 推导
- **优先级**: P1
- **分类**: stmt/loops
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Unexpected token in expression: Prod

- **输出**:
```
Parse error: Unexpected token in expression: Prod

```

### FAIL: STM-GRD-002 — guard let 模式守卫
- **优先级**: P1
- **分类**: stmt/guard_defer
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 15

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 15

```

### FAIL: STM-TRY-001 — raise 抛出异常
- **优先级**: P1
- **分类**: stmt/try_catch
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): 语义错误: 函数 'f' 使用了 raise 但未标注 raises 异常类型
  提示: 添加 'raises ErrorType' 到函数签名，或确认所有路径都 raise 后使用 '-> Never'

- **输出**:
```
语义错误: 函数 'f' 使用了 raise 但未标注 raises 异常类型
  提示: 添加 'raises ErrorType' 到函数签名，或确认所有路径都 raise 后使用 '-> Never'

```

### FAIL: STM-TRY-004 — raises 标注
- **优先级**: P1
- **分类**: stmt/try_catch
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 7

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 7

```

### FAIL: STM-TRY-005 — panic 中止
- **优先级**: P1
- **分类**: stmt/try_catch
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\stmt/try_catch/STM-TRY-005.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\stmt/try_catch/STM-TRY-005.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `STM_TRY_005`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\stmt/try_catch/STM-TRY-005.rs:22:2
   |
22 | }
   | 
```

### FAIL: DCL-FN-001 — def 等式风格
- **优先级**: P0
- **分类**: decl/func
- **Bug 类型**: [3]
- **问题**: Missing in output: '5'

### FAIL: DCL-FN-002 — def 块式风格
- **优先级**: P0
- **分类**: decl/func
- **Bug 类型**: [3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 14

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 14

```

### FAIL: DCL-FN-003 — 无返回标注
- **优先级**: P0
- **分类**: decl/func
- **Bug 类型**: [3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 8

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 8

```

### FAIL: DCL-FN-004 — 参数默认值
- **优先级**: P0
- **分类**: decl/func
- **Bug 类型**: [3]
- **问题**: Missing in output: '42'

### FAIL: DCL-FN-005 — mut 参数修饰
- **优先级**: P1
- **分类**: decl/func
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 9

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 9

```

### FAIL: DCL-FN-006 — ref 参数修饰
- **优先级**: P1
- **分类**: decl/func
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 9

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 9

```

### FAIL: DCL-FN-007 — owned 参数修饰
- **优先级**: P1
- **分类**: decl/func
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 9

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 9

```

### FAIL: DCL-FN-008 — 变长参数 ..
- **优先级**: P1
- **分类**: decl/func
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 6

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 6

```

### FAIL: DCL-FN-009 — 变长参数混合
- **优先级**: P1
- **分类**: decl/func
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 14

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 14

```

### FAIL: DCL-FN-010 — raises 标注
- **优先级**: P1
- **分类**: decl/func
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 7

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 7

```

### FAIL: DCL-FN-011 — 嵌套函数
- **优先级**: P0
- **分类**: decl/func
- **Bug 类型**: [3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 7

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 7

```

### FAIL: DCL-FN-012 — async 函数
- **优先级**: P2
- **分类**: decl/func
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/func/DCL-FN-012.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by de
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/func/DCL-FN-012.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `DCL_FN_012`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/func/DCL-FN-012.rs:22:2
   |
22 | }
   |  ^ consider a
```

### FAIL: DCL-FN-013 — 隐式返回
- **优先级**: P0
- **分类**: decl/func
- **Bug 类型**: [3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 7

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 7

```

### FAIL: DCL-FN-014 — return 无值
- **优先级**: P1
- **分类**: decl/func
- **Bug 类型**: [1, 3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/func/DCL-FN-014.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by de
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/func/DCL-FN-014.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `DCL_FN_014`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/func/DCL-FN-014.rs:22:2
   |
22 | }
   |  ^ consider a
```

### FAIL: DCL-ST-003 — struct 泛型
- **优先级**: P0
- **分类**: decl/struct
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/struct/DCL-ST-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/struct/DCL-ST-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `DCL_ST_003`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/struct/DCL-ST-003.rs:24:2
   |
24 | }
   |  ^ consid
```

### FAIL: DCL-ST-004 — 元组结构体
- **优先级**: P0
- **分类**: decl/struct
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Colon, got LParen at pos 5

- **输出**:
```
Parse error: Expected Colon, got LParen at pos 5

```

### FAIL: DCL-ST-005 — 单元结构体
- **优先级**: P0
- **分类**: decl/struct
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/struct/DCL-ST-005.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/struct/DCL-ST-005.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `DCL_ST_005`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/struct/DCL-ST-005.rs:22:2
   |
22 | }
   |  ^ consid
```

### FAIL: DCL-ST-006 — @derive 装饰
- **优先级**: P1
- **分类**: decl/struct
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/struct/DCL-ST-006.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/struct/DCL-ST-006.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `DCL_ST_006`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/struct/DCL-ST-006.rs:24:2
   |
24 | }
   |  ^ consid
```

### FAIL: DCL-EN-002 — enum 带数据变体
- **优先级**: P0
- **分类**: decl/enum
- **Bug 类型**: [3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 26

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 26

```

### FAIL: DCL-EN-003 — enum 泛型
- **优先级**: P0
- **分类**: decl/enum
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/enum/DCL-EN-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by de
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/enum/DCL-EN-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `DCL_EN_003`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/enum/DCL-EN-003.rs:24:2
   |
24 | }
   |  ^ consider a
```

### FAIL: DCL-EN-004 — enum 命名字段变体
- **优先级**: P1
- **分类**: decl/enum
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/enum/DCL-EN-004.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by de
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/enum/DCL-EN-004.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `DCL_EN_004`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/enum/DCL-EN-004.rs:24:2
   |
24 | }
   |  ^ consider a
```

### FAIL: DCL-TR-001 — trait 定义
- **优先级**: P0
- **分类**: decl/trait_impl
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Indent, got Def at pos 4

- **输出**:
```
Parse error: Expected Indent, got Def at pos 4

```

### FAIL: DCL-TR-002 — impl Trait for Type
- **优先级**: P0
- **分类**: decl/trait_impl
- **Bug 类型**: [3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Indent, got Def at pos 14

- **输出**:
```
Parse error: Expected Indent, got Def at pos 14

```

### FAIL: DCL-TR-003 — trait 继承 (+)
- **优先级**: P0
- **分类**: decl/trait_impl
- **Bug 类型**: [1, 2]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Indent, got Def at pos 4

- **输出**:
```
Parse error: Expected Indent, got Def at pos 4

```

### FAIL: DCL-TR-004 — 关联类型
- **优先级**: P1
- **分类**: decl/trait_impl
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Dedent, got Type at pos 6

- **输出**:
```
Parse error: Expected Dedent, got Type at pos 6

```

### FAIL: DCL-TR-005 — trait 默认方���
- **优先级**: P1
- **分类**: decl/trait_impl
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Indent, got Def at pos 4

- **输出**:
```
Parse error: Expected Indent, got Def at pos 4

```

### FAIL: DCL-TR-006 — impl where 约束
- **优先级**: P1
- **分类**: decl/trait_impl
- **Bug 类型**: [1]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Indent, got Def at pos 4

- **输出**:
```
Parse error: Expected Indent, got Def at pos 4

```

### FAIL: DCL-TR-007 — 负向: impl 方法签名不匹配
- **优先级**: P0
- **分类**: decl/trait_impl
- **Bug 类型**: [2]
- **问题**: Error message missing 'mismatch'
Got: Parse error: Expected Indent, got Def at pos 4

- **输出**:
```
Parse error: Expected Indent, got Def at pos 4

```

### FAIL: DCL-TR-008 — 负向: impl 缺少方法
- **优先级**: P0
- **分类**: decl/trait_impl
- **Bug 类型**: [2]
- **问题**: Error message missing 'missing'
Got: Parse error: Expected Eq, got Newline at pos 15

- **输出**:
```
Parse error: Expected Eq, got Newline at pos 15

```

### FAIL: DCL-TR-009 — 负向: impl 返回类型不一致
- **优先级**: P0
- **分类**: decl/trait_impl
- **Bug 类型**: [2]
- **问题**: Error message missing 'return'
Got: Parse error: Expected Indent, got Def at pos 4

- **输出**:
```
Parse error: Expected Indent, got Def at pos 4

```

### FAIL: DCL-IM-003 — import as 别名
- **优先级**: P1
- **分类**: decl/import
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/import/DCL-IM-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/import/DCL-IM-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `DCL_IM_003`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\decl/import/DCL-IM-003.rs:22:2
   |
22 | }
   |  ^ consid
```

### FAIL: DCL-MG-002 — __eq__ 比较
- **优先级**: P1
- **分类**: decl/magic
- **Bug 类型**: [3]
- **问题**: Missing in output: 'True'
- **问题**: Missing from run output: 'True'

### FAIL: META-DEC-001 — @decorator 无参
- **优先级**: P1
- **分类**: meta/decorator
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-001.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) o
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-001.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `META_DEC_001`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-001.rs:26:2
   |
26 | }
  
```

### FAIL: META-DEC-002 — @decorator 带参
- **优先级**: P1
- **分类**: meta/decorator
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-002.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) o
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-002.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `META_DEC_002`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-002.rs:22:2
   |
22 | }
  
```

### FAIL: META-DEC-003 — @export(Rust)
- **优先级**: P1
- **分类**: meta/decorator
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) o
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `META_DEC_003`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-003.rs:22:2
   |
22 | }
  
```

### FAIL: META-DEC-004 — @derive(Clone,Debug)
- **优先级**: P1
- **分类**: meta/derive
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/derive/META-DEC-004.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on b
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/derive/META-DEC-004.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `META_DEC_004`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/derive/META-DEC-004.rs:24:2
   |
24 | }
   |  ^ 
```

### FAIL: META-DEC-005 — @curry 装饰器
- **优先级**: P2
- **分类**: meta/decorator
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-005.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) o
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-005.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `META_DEC_005`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\meta/decorator/META-DEC-005.rs:22:2
   |
22 | }
  
```

### FAIL: META-CPT-001 — comptime 表达式
- **优先级**: P2
- **分类**: meta/comptime
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 7

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 7

```

### FAIL: META-CPT-002 — comptime 块
- **优先级**: P2
- **分类**: meta/comptime
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 10

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 10

```

### FAIL: BLD-VAR-002 — =: 多语句块
- **优先级**: P2
- **分类**: build/var_block
- **Bug 类型**: [3]
- **问题**: Missing in output: '30'

### FAIL: BLD-CALL-001 — ~: 调用块(元组)
- **优先级**: P2
- **分类**: build/call_block
- **Bug 类型**: [3]
- **问题**: Missing in output: '30'

### FAIL: BLD-GEN-001 — *: 生成器 + yield
- **优先级**: P2
- **分类**: build/gen_block
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Unexpected token in expression: BuildGen

- **输出**:
```
Parse error: Unexpected token in expression: BuildGen

```

### FAIL: BLD-GEN-002 — yield from
- **优先级**: P2
- **分类**: build/gen_block
- **Bug 类型**: []
- **问题**: LZ compile failed (rc=1): Parse error: Unexpected token in expression: BuildGen

- **输出**:
```
Parse error: Unexpected token in expression: BuildGen

```

### FAIL: MOD-002 — #!lib
- **优先级**: P1
- **分类**: modules
- **Bug 类型**: [1, 3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\modules/MOD-002.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\modules/MOD-002.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `MOD_002`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\modules/MOD-002.rs:22:2
   |
22 | }
   |  ^ consider adding a `main
```

### FAIL: MOD-005 — #!lenient
- **优先级**: P1
- **分类**: modules
- **Bug 类型**: [1, 2]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\modules/MOD-005.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\modules/MOD-005.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `MOD_005`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\modules/MOD-005.rs:22:2
   |
22 | }
   |  ^ consider adding a `main
```

### FAIL: TST-001 — assert 复合表达式
- **优先级**: P1
- **分类**: test_framework
- **Bug 类型**: [3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-001.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-001.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TST_001`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-001.rs:30:2
   |
30 | }
   |  ^ consider 
```

### FAIL: TST-002 — assert not
- **优先级**: P1
- **分类**: test_framework
- **Bug 类型**: [3]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-002.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-002.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TST_002`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-002.rs:30:2
   |
30 | }
   |  ^ consider 
```

### FAIL: TST-003 — test 引用外部函数
- **优先级**: P1
- **分类**: test_framework
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-003.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TST_003`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-003.rs:35:2
   |
35 | }
   |  ^ consider 
```

### FAIL: TST-004 — suite + const
- **优先级**: P1
- **分类**: test_framework
- **Bug 类型**: [1]
- **问题**: rustc compile failed: warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-004.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by 
- **输出**:
```
warning: unused import: `std::collections::HashMap`
 --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-004.rs:1:5
  |
1 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0601]: `main` function not found in crate `TST_004`
  --> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\test_framework/TST-004.rs:35:2
   |
35 | }
   |  ^ consider 
```

### FAIL: ASY-001 — async def 定义
- **优先级**: P2
- **分类**: async
- **Bug 类型**: []
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 7

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 7

```

### FAIL: ASY-002 — await 前缀
- **优先级**: P2
- **分类**: async
- **Bug 类型**: []
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 7

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 7

```

### FAIL: ASY-003 — await 后缀
- **优先级**: P2
- **分类**: async
- **Bug 类型**: []
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 7

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 7

```

### FAIL: ASY-004 — spawn 启动
- **优先级**: P2
- **分类**: async
- **Bug 类型**: []
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 5

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 5

```

### FAIL: ASY-005 — yield 生成器
- **优先级**: P2
- **分类**: async
- **Bug 类型**: [1, 3]
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 5

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 5

```

### FAIL: ASY-006 — yield from + with
- **优先级**: P2
- **分类**: async
- **Bug 类型**: []
- **问题**: LZ compile failed (rc=1): Parse error: Expected Eq, got Colon at pos 5

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 5

```

### FAIL: NEG-LEX-002 — 未闭合 /* 块注释
- **优先级**: P0
- **分类**: negative/lex
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/lex/NEG-LEX-002.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/lex/NEG-LEX-002.rs

```

### FAIL: NEG-LEX-004 — 未闭合字符串引号
- **优先级**: P0
- **分类**: negative/lex
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/lex/NEG-LEX-004.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/lex/NEG-LEX-004.rs

```

### FAIL: NEG-PARSE-001 — 缺失冒号应拦截
- **优先级**: P0
- **分类**: negative/parse
- **Bug 类型**: [2]
- **问题**: Error message missing 'expected'
Got: Parse error: Expected Eq, got Newline at pos 7

- **输出**:
```
Parse error: Expected Eq, got Newline at pos 7

```

### FAIL: NEG-PARSE-002 — 缺失缩进应拦截
- **优先级**: P0
- **分类**: negative/parse
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/parse/NEG-PARSE-002.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/parse/NEG-PARSE-002.rs

```

### FAIL: NEG-PARSE-003 — 不匹配括号应拦截
- **优先级**: P0
- **分类**: negative/parse
- **Bug 类型**: [2]
- **问题**: Error message missing 'expected'
Got: Parse error: Expected RParen, got Eof at pos 10

- **输出**:
```
Parse error: Expected RParen, got Eof at pos 10

```

### FAIL: NEG-PARSE-005 — catch 后无参数应拦截
- **优先级**: P1
- **分类**: negative/parse
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/parse/NEG-PARSE-005.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/parse/NEG-PARSE-005.rs

```

### FAIL: NEG-TYPE-001 — 参数数量不匹配
- **优先级**: P1
- **分类**: negative/type
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/type/NEG-TYPE-001.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/type/NEG-TYPE-001.rs

```

### FAIL: NEG-TYPE-002 — 返回类型不匹配
- **优先级**: P1
- **分类**: negative/type
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/type/NEG-TYPE-002.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/type/NEG-TYPE-002.rs

```

### FAIL: NEG-TYPE-003 — 使用未定义变量
- **优先级**: P1
- **分类**: negative/type
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/type/NEG-TYPE-003.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/type/NEG-TYPE-003.rs

```

### FAIL: NEG-TYPE-004 — i64 索引 String
- **优先级**: P1
- **分类**: negative/type
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/type/NEG-TYPE-004.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/type/NEG-TYPE-004.rs

```

### FAIL: NEG-TYPE-005 — ?. 用于非 Option 类型
- **优先级**: P1
- **分类**: negative/type
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/type/NEG-TYPE-005.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/type/NEG-TYPE-005.rs

```

### FAIL: NEG-SEM-001 — impl 方法签名≠trait
- **优先级**: P1
- **分类**: negative/semantic
- **Bug 类型**: [2]
- **问题**: Error message missing 'mismatch'
Got: Parse error: Expected Eq, got Newline at pos 15

- **输出**:
```
Parse error: Expected Eq, got Newline at pos 15

```

### FAIL: NEG-SEM-002 — impl 缺方法
- **优先级**: P1
- **分类**: negative/semantic
- **Bug 类型**: [2]
- **问题**: Error message missing 'missing'
Got: Parse error: Expected Eq, got Newline at pos 15

- **输出**:
```
Parse error: Expected Eq, got Newline at pos 15

```

### FAIL: NEG-SEM-003 — impl 返回类型不一致
- **优先级**: P1
- **分类**: negative/semantic
- **Bug 类型**: [2]
- **问题**: Error message missing 'return'
Got: Parse error: Expected Eq, got Newline at pos 15

- **输出**:
```
Parse error: Expected Eq, got Newline at pos 15

```

### FAIL: NEG-SEM-004 — mut 修饰不一致
- **优先级**: P1
- **分类**: negative/semantic
- **Bug 类型**: [2]
- **问题**: Error message missing 'mut'
Got: Parse error: Expected type, got Mut

- **输出**:
```
Parse error: Expected type, got Mut

```

### FAIL: NEG-SEM-005 — 方法名冲突
- **优先级**: P1
- **分类**: negative/semantic
- **Bug 类型**: [2]
- **问题**: Error message missing 'conflict'
Got: Parse error: Expected Eq, got Newline at pos 15

- **输出**:
```
Parse error: Expected Eq, got Newline at pos 15

```

### FAIL: NEG-SEM-006 — match 非穷尽
- **优先级**: P1
- **分类**: negative/semantic
- **Bug 类型**: [2]
- **问题**: Error message missing 'exhaustive'
Got: Parse error: Expected Eq, got Colon at pos 10

- **输出**:
```
Parse error: Expected Eq, got Colon at pos 10

```

### FAIL: NEG-SEM-007 — move 后使用
- **优先级**: P1
- **分类**: negative/semantic
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/semantic/NEG-SEM-007.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/semantic/NEG-SEM-007.rs
=== Strict warnings ===
  [S006] fn `f`: unused `y`
   hint: prefix with `_`

```

### FAIL: NEG-SEM-008 — 重复定义
- **优先级**: P1
- **分类**: negative/semantic
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/semantic/NEG-SEM-008.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/semantic/NEG-SEM-008.rs

```

### FAIL: NEG-SEM-009 — 访问不存在字段
- **优先级**: P1
- **分类**: negative/semantic
- **Bug 类型**: [2]
- **问题**: Expected error but compile succeeded
- **输出**:
```
Generated E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/semantic/NEG-SEM-009.lz -> E:\IDEProjects\AI\lang-zone\test-suite\20260726-integration\cases\negative/semantic/NEG-SEM-009.rs

```

## 发现的 Bug
- **[Bug-3]** TYP-PRIM-001 — 验证 int 运行时输出正确
- **[Bug-3]** TYP-PRIM-002 — float 类型
- **[Bug-3]** TYP-PRIM-003 — str 类型拼接
- **[Bug-3]** TYP-PRIM-004 — bool 类型
- **[Bug-1]** TYP-PRIM-005 — i32/u32/u64/f32 类型标注
- **[Bug-2]** TYP-PRIM-005 — i32/u32/u64/f32 类型标注
- **[Bug-1]** TYP-PRIM-006 — char 类型
- **[Bug-2]** TYP-PRIM-006 — char 类型
- **[Bug-1]** TYP-CON-003 — Set<str>
- **[Bug-3]** TYP-CON-003 — Set<str>
- **[Bug-1]** TYP-CON-004 — Array<T,N>
- **[Bug-3]** TYP-CON-004 — Array<T,N>
- **[Bug-1]** TYP-CON-006 — Range 1..10
- **[Bug-3]** TYP-CON-006 — Range 1..10
- **[Bug-3]** TYP-OPT-002 — Some 构造
- **[Bug-1]** TYP-GEN-002 — where 约束
- **[Bug-3]** TYP-GEN-002 — where 约束
- **[Bug-1]** TYP-GEN-003 — 多约束 + 连接
- **[Bug-3]** TYP-GEN-003 — 多约束 + 连接
- **[Bug-1]** TYP-GEN-004 — struct 泛型
- **[Bug-1]** TYP-ALIAS-001 — type 基本别名
- **[Bug-3]** TYP-ALIAS-001 — type 基本别名
- **[Bug-1]** TYP-ALIAS-002 — type 泛型别名
- **[Bug-3]** EXP-LIT-001 — 整数运算
- **[Bug-3]** EXP-LIT-002 — 多进制混合运算
- **[Bug-3]** EXP-LIT-003 — 浮点运算
- **[Bug-3]** EXP-LIT-004 — Bool 字面量 return
- **[Bug-1]** EXP-LIT-005 — Bug-5: None 不应推断为 ()
- **[Bug-3]** EXP-LIT-005 — Bug-5: None 不应推断为 ()
- **[Bug-3]** EXP-LIT-006 — f-string 插值
- **[Bug-3]** EXP-LIT-007 — 原始字符串
- **[Bug-3]** EXP-OP-001 — 算术 + - * / %
- **[Bug-1]** EXP-OP-002 — ** 幂运算
- **[Bug-3]** EXP-OP-002 — ** 幂运算
- **[Bug-3]** EXP-OP-003 — == != 比较
- **[Bug-3]** EXP-OP-004 — < > <= >= 比较
- **[Bug-3]** EXP-OP-005 — and / or 逻辑
- **[Bug-3]** EXP-OP-006 — not 逻辑非
- **[Bug-1]** EXP-OP-010 — << >> 移位
- **[Bug-3]** EXP-OP-010 — << >> 移位
- **[Bug-1]** EXP-OP-012 — is 运算符
- **[Bug-3]** EXP-OP-012 — is 运算符
- **[Bug-3]** EXP-OP-013 — 复合赋值 += -= *= /=
- **[Bug-3]** EXP-SPC-001 — |> 管道
- **[Bug-1]** EXP-SPC-005 — .. 半开范围
- **[Bug-3]** EXP-SPC-005 — .. 半开范围
- **[Bug-1]** EXP-SPC-006 — ..= 闭区间范围
- **[Bug-3]** EXP-SPC-006 — ..= 闭区间范围
- **[Bug-1]** EXP-SPC-007 — ^ move 后缀
- **[Bug-3]** EXP-CMP-001 — 基本列表推导
- **[Bug-3]** EXP-CMP-002 — 带条件的推导
- **[Bug-1]** EXP-CMP-003 — 多变量推导
- **[Bug-3]** EXP-CMP-003 — 多变量推导
- **[Bug-3]** EXP-CLS-002 — 多参数闭包
- **[Bug-1]** EXP-CLS-003 — 闭包作参数
- **[Bug-3]** EXP-CLS-003 — 闭包作参数
- **[Bug-1]** EXP-CLS-004 — 闭包捕获外部变量
- **[Bug-3]** EXP-CLS-004 — 闭包捕获外部变量
- **[Bug-3]** STM-IF-005 — match 冒号风格
- **[Bug-3]** STM-IF-006 — match 变量绑定
- **[Bug-1]** STM-IF-007 — match 或模式
- **[Bug-3]** STM-IF-007 — match 或模式
- **[Bug-1]** STM-IF-008 — match 守卫
- **[Bug-3]** STM-IF-008 — match 守卫
- **[Bug-1]** STM-IF-009 — match 范围模式
- **[Bug-3]** STM-IF-009 — match 范围模式
- **[Bug-1]** STM-IF-011 — match Some(x) 解构
- **[Bug-3]** STM-IF-011 — match Some(x) 解构
- **[Bug-3]** STM-LP-002 — for 遍历 range
- **[Bug-1]** STM-LP-006 — break 带返回值
- **[Bug-3]** STM-LP-006 — break 带返回值
- **[Bug-3]** STM-LP-007 — continue
- **[Bug-1]** STM-LP-008 — sum 推导
- **[Bug-3]** STM-LP-008 — sum 推导
- **[Bug-1]** STM-LP-009 — prod 推导
- **[Bug-3]** STM-LP-009 — prod 推导
- **[Bug-1]** STM-GRD-002 — guard let 模式守卫
- **[Bug-3]** STM-GRD-002 — guard let 模式守卫
- **[Bug-1]** STM-TRY-001 — raise 抛出异常
- **[Bug-3]** STM-TRY-001 — raise 抛出异常
- **[Bug-1]** STM-TRY-004 — raises 标注
- **[Bug-1]** STM-TRY-005 — panic 中止
- **[Bug-3]** DCL-FN-001 — def 等式风格
- **[Bug-3]** DCL-FN-002 — def 块式风格
- **[Bug-3]** DCL-FN-003 — 无返回标注
- **[Bug-3]** DCL-FN-004 — 参数默认值
- **[Bug-1]** DCL-FN-005 — mut 参数修饰
- **[Bug-1]** DCL-FN-006 — ref 参数修饰
- **[Bug-1]** DCL-FN-007 — owned 参数修饰
- **[Bug-1]** DCL-FN-008 — 变长参数 ..
- **[Bug-1]** DCL-FN-009 — 变长参数混合
- **[Bug-1]** DCL-FN-010 — raises 标注
- **[Bug-3]** DCL-FN-011 — 嵌套函数
- **[Bug-1]** DCL-FN-012 — async 函数
- **[Bug-3]** DCL-FN-013 — 隐式返回
- **[Bug-1]** DCL-FN-014 — return 无值
- **[Bug-3]** DCL-FN-014 — return 无值
- **[Bug-1]** DCL-ST-003 — struct 泛型
- **[Bug-1]** DCL-ST-004 — 元组结构体
- **[Bug-1]** DCL-ST-005 — 单元结构体
- **[Bug-1]** DCL-ST-006 — @derive 装饰
- **[Bug-3]** DCL-EN-002 — enum 带数据变体
- **[Bug-1]** DCL-EN-003 — enum 泛型
- **[Bug-1]** DCL-EN-004 — enum 命名字段变体
- **[Bug-1]** DCL-TR-001 — trait 定义
- **[Bug-3]** DCL-TR-002 — impl Trait for Type
- **[Bug-1]** DCL-TR-003 — trait 继承 (+)
- **[Bug-2]** DCL-TR-003 — trait 继承 (+)
- **[Bug-1]** DCL-TR-004 — 关联类型
- **[Bug-1]** DCL-TR-005 — trait 默认方���
- **[Bug-1]** DCL-TR-006 — impl where 约束
- **[Bug-2]** DCL-TR-007 — Bug-9: trait/impl 签名不匹配应拦截
- **[Bug-2]** DCL-TR-008 — Bug-11: impl 缺方法应拦截
- **[Bug-2]** DCL-TR-009 — Bug-12: impl 返回类型不一致应拦截
- **[Bug-1]** DCL-IM-003 — import as 别名
- **[Bug-3]** DCL-MG-002 — __eq__ 比较
- **[Bug-1]** META-DEC-001 — @decorator 无参
- **[Bug-1]** META-DEC-002 — @decorator 带参
- **[Bug-1]** META-DEC-003 — @export(Rust)
- **[Bug-1]** META-DEC-004 — @derive(Clone,Debug)
- **[Bug-1]** META-DEC-005 — @curry 装饰器
- **[Bug-1]** META-CPT-001 — comptime 表达式
- **[Bug-3]** META-CPT-001 — comptime 表达式
- **[Bug-1]** META-CPT-002 — comptime 块
- **[Bug-3]** META-CPT-002 — comptime 块
- **[Bug-3]** BLD-VAR-002 — =: 多语句块
- **[Bug-3]** BLD-CALL-001 — ~: 调用块(元组)
- **[Bug-1]** BLD-GEN-001 — *: 生成器 + yield
- **[Bug-3]** BLD-GEN-001 — *: 生成器 + yield
- **[Bug-1]** MOD-002 — #!lib
- **[Bug-3]** MOD-002 — #!lib
- **[Bug-1]** MOD-005 — #!lenient
- **[Bug-2]** MOD-005 — #!lenient
- **[Bug-3]** TST-001 — assert 复合表达式
- **[Bug-3]** TST-002 — assert not
- **[Bug-1]** TST-003 — test 引用外部函数
- **[Bug-1]** TST-004 — suite + const
- **[Bug-1]** ASY-005 — yield 生成器
- **[Bug-3]** ASY-005 — yield 生成器
- **[Bug-2]** NEG-LEX-002 — 未闭合 /* 块注释
- **[Bug-2]** NEG-LEX-004 — 未闭合字符串引号
- **[Bug-2]** NEG-PARSE-001 — 缺失冒号应拦截
- **[Bug-2]** NEG-PARSE-002 — 缺失缩进应拦截
- **[Bug-2]** NEG-PARSE-003 — 不匹配括号应拦截
- **[Bug-2]** NEG-PARSE-005 — catch 后无参数应拦截
- **[Bug-2]** NEG-TYPE-001 — 参数数量不匹配
- **[Bug-2]** NEG-TYPE-002 — 返回类型不匹配
- **[Bug-2]** NEG-TYPE-003 — 使用未定义变量
- **[Bug-2]** NEG-TYPE-004 — Bug-20: 类型错误应被 lz 拦截
- **[Bug-2]** NEG-TYPE-005 — ?. 用于非 Option 类型
- **[Bug-2]** NEG-SEM-001 — Bug-9: trait/impl 签名不匹配应拦截
- **[Bug-2]** NEG-SEM-002 — Bug-11: impl 缺方法应拦截
- **[Bug-2]** NEG-SEM-003 — Bug-12: impl 返回类型不一致应拦截
- **[Bug-2]** NEG-SEM-004 — Bug-13: mut 不匹配应拦截
- **[Bug-2]** NEG-SEM-005 — Bug-10: 方法名冲突应拦截
- **[Bug-2]** NEG-SEM-006 — match 非穷尽
- **[Bug-2]** NEG-SEM-007 — move 后使用
- **[Bug-2]** NEG-SEM-008 — 重复定义
- **[Bug-2]** NEG-SEM-009 — 访问不存在字段
# block_demo.lz 测试文档问题

## 日期
2026-08-06 (Run 96-97)

## 依据
`SYNTAX/05b-block命名块.md` v3.3 — block 命名块语法规范

## 问题列表

### 1. `let sum = block sum_block:` — block 无返回值，不能用 `let` 接收

**位置**: `DEMO/06_control_flow/block_demo.lz` 第 8 行

**当前写法（❌ 错误）**:
```lz
let sum = block sum_block:   // block 不是表达式
    let scanner = [1, 2, 3]
    break sum_block: 10       // block 禁止 break NAME v（无返回值）
```

**语法规范（§一）**:
> block 不是表达式、无返回值。它纯为控制流/作用域服务。
> 要「块产出值」用 `loop` + `break v`。

**正确写法**:
```lz
mut sum = 0
block sum_block:
    let scanner = [1, 2, 3]
    sum = 10
```

---

### 2. `break sum_block: 10` — block 禁止带值跳出

**位置**: 同上，第 11 行

**当前写法（❌ 错误）**:
```lz
break sum_block: 10    // block 禁止 break NAME v
```

**语法规范（§4.3）**:
> `break NAME v`（无 `with`）中若 `NAME` 是 block → 编译错误（block 无值）。

**正确写法**（数据写捕获变量）:
```lz
sum = 10
break sum_block
```

---

### 3. `let total = mut 0` — 语法错误

**位置**: 第 17 行

**当前写法（❌）**:
```lz
let total = mut 0
```

**正确写法**:
```lz
mut total = 0
```

---

### 4. `let scanner = [` — 数组字面量跨行问题

**位置**: 第 9 行

`let scanner = [` 后跟跨行数组字面量，解析器期望 `:` 而不是 `LBrack`（pos 94）。
应为: `let scanner: List<int> = [1, 2, 3]`

---

### 5. 缺少 checker 块语法支持（`[ps]`/`[chk]`/`^:`/`[(expr)]`）

以下语法尚未实现（`block_demo.lz` 后半部分）:
- `block NAME[ps: __Params]:` — checker 块定义
- `block NAME ^:` — 标准触发
- `block NAME[(expr)]` — 单行触发
- `break NAME with v` — 循环体内触发
- `block NAME[chk]` — 带检查站

---

## 修复优先级

| 优先级 | 问题 | 状态 |
|--------|------|------|
| P0 | 移除 `let sum = block` 用法 | Run 97 完成（移除表达式级 block） |
| P0 | IR 压缩为闭包（非 Rust label） | Run 97 完成（`(|| { ... })()` 模式） |
| P1 | checker 块 `[ps]`/`[chk]` 解析 | 待实现 |
| P1 | 触发语法 `^:`/`[(expr)]` | 待实现 |
| P2 | block_demo.lz 测试文档修正 | 待用户确认后修改 |

## 开发原则

> block 不是表达式，不能用值接收；`break NAME v` 对 block 非法。
> 数据传递用捕获变量或 `ps`（checker 块）。

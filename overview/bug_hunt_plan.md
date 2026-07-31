# 🐛 Bug Hunt 计划 — Lang-Zong 编译器稳定性攻坚战

> 目标：在启动自举前，系统性找出并消灭所有 P0/P1 Bug，使转译器达到"可靠就能用"。
> 基准：355 unit tests + 8 套件 436 测试 + 边界 133 测试 = **924 已有用例**。

---

## 一、当前状态基线

### ✅ 已有的测试体系

| 层 | 数量 | 类型 | 局限性 |
|----|------|------|--------|
| Rust 单元测试 | 355 | `#[test]` 断言 | 只覆盖解析/桥接/导出三个模块，其余几乎为 0 |
| 黑盒功能套件 | 436 (8 套件) | Python 文本断言 | **只检查 .rs 是否包含子串**，不编译不运行 |
| 边界测试 | 133 | Python `lzc` + `rustc` | 5 个已知失败（均为预期偏差，非漏洞） |

### ❌ 关键的测试盲区

1. **零 E2E 运行时测试** — 没有任何一个 `.lz` 文件经 `lzc → .rs → rustc → run` 后验证 stdout
2. **零组合测试** — 没有交叉组合不同语法特性的测试（如 `match` 中嵌套 `comptime`）
3. **零压力测试** — 没有嵌套深度、超长文件、超大展开的测试
4. **零差分测试** — 没有随机生成 .lz 代码的 fuzzing
5. **零回归自动化** — 没有 CI 或提交钩子自动触发全量测试
6. **Rust 单元覆盖 ~0%** — codegen/func/stmt/decl 等核心模块无单元测试

### 🐞 已知剩余失败

| ID | 类别 | 描述 | 严重度 | 处理方式 |
|----|------|------|--------|---------|
| c006 | 边界 | 顶层 `y = 5` 期望静默通过，实际 rustc 报 unsafe | P2 | 修正预期 |
| c018 | 边界 | `t = s^` 期望 rustc 报 use-after-move，实际通过 | P2 | 修正预期 |
| c083 | 边界 | 闭包赋值期望 rustc 报错，实际通过 | P2 | 修正预期 |
| c088 | 边界 | range print 期望 rustc 报错，实际通过 | P2 | 修正预期 |
| c103 | 边界 | Option? 传播期望 rustc 报错，实际通过 | P2 | 修正预期 |
| D1 | bootstrap | 字面量溢出静默归零；`1e400` → inf 不报错 | **P0** | 需修复 |
| 悬空 `^` | bootstrap | `b = a ^` 被静默接受而非解析期拒绝 | **P1** | 需修复 |
| 缩进不匹配 | bootstrap | 深层语句收为顶层 const 不报错 | **P1** | 需修复 |
| with/spawn/async | bootstrap | 运行时映射缺失 | P2 | 按需修复 |

---

## 二、Bug Hunting 分层策略

虫子分 4 层，从易到难交叉推进：

```
第 1 层 ─ 已知修复 + 预期对齐（半天）
  │
  ├─ 修正 5 个边界测试预期
  ├─ 修复 D1 / 悬空 ^ / 缩进不匹配 3 个 P0/P1
  │
第 2 层 ─ E2E 运行时框架 + 首次扫描（1 天）
  │
  ├─ 写 E2E 测试驱动（compile+rustc+run+assert）
  ├─ 对 ~100 个 .lz 示例跑 E2E 回归
  ├─ 修复发现的运行时 bug
  │
第 3 层 ─ 组合 / 压力 / 边界补全（1 天）
  │
  ├─ 语法交叉组合测试（+200 个新用例）
  ├─ 嵌套深度压力（10/50/100 层）
  ├─ 超大文件 / Unicode / 空文件边界
  │
第 4 层 ─ 差分测试 + 回归保护（持续）
  │
  ├─ 随机 fuzzing：生成随机 .lz 看是否 101 崩溃
  ├─ 两次编译输出对比（确定性检测）
  ├─ 补齐 Rust 单元测试（核心模块 ≥70%）
  ├─ 搭建 CI 自动触发全量回归
```

---

## 三、第 1 层：已知修复 + 预期对齐

### 3.1 修 5 个边界预期偏差

调整 5 个测试的 `expect` 列为实际行为（编译器更宽松/严格了）：

| 用例 | 当前 expect | 实际行为 | 新 expect |
|------|------------|---------|-----------|
| c006 | `NO_ERROR` | `RUSTC_ERROR`（unsafe static） | `RUSTC_ERROR` |
| c018 | `RUSTC_ERROR` | `OK`（`^` 后值被 move，但 Rust 允许一次使用） | `OK` |
| c083 | `RUSTC_ERROR` | `OK`（闭包编译成功） | `OK` |
| c088 | `RUSTC_ERROR` | `OK`（range 打印可行） | `OK` |
| c103 | `RUSTC_ERROR` | `OK`（Option? 传播已实现） | `OK` |

### 3.2 修 3 个 P0/P1 编译器 Bug

**D1 — 字面量溢出静默归零**
```
文件: src/lexer/lexer.rs 或 src/parser/...
现状: 99999999999999999999 → IntLit(0) 静默归零
要求: 溢出时报 LexError
```

**悬空 `^` 被静默接受**
```
文件: src/parser/expr.rs (解析中缀运算符)
现状: b = a ^ → 被解析为 a ^ (缺右操作数)，静默生成可编译代码
要求: 解析期报 PARSE_ERROR
```

**缩进不匹配被静默忽略**
```
文件: src/lexer/lexer.rs (handle_indent 函数)
现状: 深层语句错位 → 被当作顶层 const 不报错
要求: 缩进错位时解析期报错
```

---

## 四、第 2 层：E2E 运行时测试框架

### 4.1 新建 `tests_e2e/` 目录

```
tests_e2e/
├── run.py              # E2E 测试驱动
├── cases/              # 测试用例
│   ├── hello.lz        # 最简单的运行测试
│   ├── math.lz         # 算术
│   ├── fib.lz          # 递归
│   ├── struct.lz       # 结构体
│   └── ...
├── golden/             # 基准输出
│   ├── hello.stdout    # hello.lz 的预期 stdout
│   └── ...
└── results.json        # 运行结果
```

### 4.2 E2E 驱动逻辑

```python
for case in cases/*.lz:
    # 1. lzc 编译
    code = lzc(case.name, "--std-dir", std_dir)
    assert code == 0, f"{case}: lzc failed"
    
    # 2. rustc 编译生成的 .rs
    code = rustc(case.name + ".rs")
    assert code == 0, f"{case}: rustc failed"
    
    # 3. 运行二进制
    stdout = run(case.exe)
    
    # 4. 与 golden 文件比对
    expected = read(f"golden/{case.stem}.stdout")
    assert stdout == expected, f"{case}: output mismatch"
    
    # 5. 可选: 带 --ast 验证 AST 结构
```

### 4.3 首批 ~100 个 E2E 用例来源

- `demo/` 目录中可编译的示例（约 20 个要修复语法）
- `test-suite/` 中的 `rust` 模式测试（约 100 个可转为 E2E）
- `tests_boundary/` 中 expect=OK 的用例（约 80 个）
- 新建 ~20 个核心特性测试

---

## 五、第 3 层：组合 / 压力 / 边界补全

### 5.1 语法交叉组合（`tests_cross/`）

单语法特性测试完了，下一步是**交叉组合**：

```
# 组合 1: 泛型 + match + 闭包
def apply<T, U>(f: (T) -> U, x: T) -> U = f(x)
def main() =
    result = match apply(|x| x * 2, 5):
        case v => v
    print(result)

# 组合 2: 枚举 + impl + 模式匹配
enum Tree<T> =
    Leaf(T)
    Node(Tree<T>, Tree<T>)

impl Tree<int> =
    def sum(self: Self) -> int = ...

# 组合 3: try/catch + 闭包 + generics
# 组合 4: async/await + with + guard
# ...
```

列出所有两两/三三语法特性的交叉组合，每个组合写一个测试用例。预期：**新增约 200-300 个用例**。

### 5.2 压力测试

```
# 100 层嵌套 for 循环
def main() =
    x = 0
    for i1 in 0..2:
        for i2 in 0..2:
            ...
            for i100 in 0..2:
                x = x + 1

# 10000 行源文件
# 1000 个连续闭包嵌套
# 宏展开 100 次
# 超大字符串字面量 (1MB)
```

### 5.3 语言边界补全

| 类别 | 当前覆盖 | 需补全 |
|------|---------|--------|
| Unicode | 无 | CJK 标识符、emoji 注释、多语言字符串 |
| 空/极端 | 空文件已测 | 仅空格、仅换行、仅注释、仅 BOM |
| 数值边界 | i64::MAX | i64::MIN、f64 极值、NaN、负零 |
| 注释 | // 已测 | 块注释嵌套、多行 f-string 内注释 |
| 宏 | 基本已测 | 宏嵌套 10 层、宏循环调用、$ 冲突 |

---

## 六、第 4 层：差分测试 + 回归保护

### 6.1 Fuzz 测试器

```python
# random_fuzz.py
# 从 lz 语法模板随机生成合法代码片段
TEMPLATES = [
    "def {name}() = {body}",
    "struct {Name} =\n  {field}: {type}",
    "match {expr}:\n  case {pat} => {body}",
    ...
]
for _ in range(10000):
    code = generate_lz()
    exit_code = lzc(code)
    if exit_code == 101:  # panic!
        record_bug(code)
```

### 6.2 确定性检测

同一个 .lz 文件编译两次，生成的 .rs 必须完全相同：
```bash
diff <(lzc a.lz --std-dir std/ 2>/dev/null) <(lzc a.lz --std-dir std/ 2>/dev/null)
```

### 6.3 Rust 单元测试补全

按模块覆盖度优先级：

| 模块 | 当前覆盖率 | 目标 | 需增测试数 |
|------|-----------|------|-----------|
| codegen/expr.rs | ≈10% | ≥70% | ~30 |
| codegen/stmt.rs | ≈0% | ≥70% | ~25 |
| codegen/decl.rs | ≈0% | ≥70% | ~20 |
| codegen/func.rs | ≈0% | ≥70% | ~15 |
| lexer/lexer.rs | ≈30% | ≥70% | ~40 |
| parser/expr.rs | ≈20% | ≥70% | ~35 |
| parser/stmt.rs | ≈5% | ≥70% | ~20 |
| bridge/std.rs | ≈20% | ≥50% | ~20 |

---

## 七、优先级排序

```
🔥 P0 (阻塞自举的)
  1. D1 字面量溢出修复          ← 半天
  2. E2E 框架 => 首批运行时回归  ← 1 天
  3. 悬空 ^ / 缩进不匹配修复    ← 半天

⭐⭐ P1 (严重影响稳定性的)
  4. 组合测试 200+ 用例          ← 1 天
  5. 压力测试 (嵌套/超大)       ← 半天
  6. 5 个边界预期偏差修正       ← 1 小时

📌 P2 (应该做但不阻塞)
  7. Fuzz 测试器                ← 1 天
  8. Rust 单元测试补全           ← 2 天
  9. 确定性检测 + CI             ← 1 天
```

---

## 八、建议执行顺序

```
第 1 步 [现在]  修 D1 + 悬空 ^ + 缩进不匹配
第 2 步 [紧接着] 建 E2E 框架 → 跑首批 ~50 用例
第 3 步 [并行]   修 5 个边界预期 + 更新 CSV
第 4 步 [然后]   组合测试 200+ 用例
第 5 步 [压力]   嵌套/超大/Unicode 测试
第 6 步 [持续]   Fuzz + 确定性 + 单元测试 + CI
```

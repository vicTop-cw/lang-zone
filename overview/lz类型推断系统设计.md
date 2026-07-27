# LZ 类型推断系统 — 设计文档

> 版本: 0.1 · 2026-07-27

---

## 一、概述

LZ 的类型推断系统解耦为两层：
- **简单内置推断**：主编译器 `lzc` 内置，仅做局部/单函数内推断，不递归跨函数
- **高级外部推断**：独立工具 `lz-infer`，负责全局/递归/泛型推断，输出签名文件 `.lzi`

```
┌──────────────────────────────────────────┐
│                  lz-infer                 │ ← 独立工具
│  全局推断 / 递归分析 / 泛型展开 / 跨文件  │
│                  ↓ 输出                  │
│            signatures.lzi                │ ← 签名文件
└──────────────────────────────────────────┘
                     ↓ 被读取
┌──────────────────────────────────────────┐
│                  lzc                     │ ← 主编译器
│  简单局部推断 + 签名查表 + 代码生成      │
└──────────────────────────────────────────┘
```

---

## 二、简单内置推断（lzc 内置）

### 2.1 设计原则

| 原则 | 说明 |
|------|------|
| **不递归** | 不分析被调用函数的内部实现，只看函数签名 |
| **单函数内** | 仅分析当前函数体内的 `let` / 字面量 / 运算表达式 |
| **显式优先** | 用户标注了类型就直接用，不推断 |
| **失败不阻塞** | 推断失败 → 编译器报 `type_required` 错误，用户手动标注即可 |

### 2.2 推断能力

```lz
// ✅ 可推断：字面量直接推类型
let x = 42              // → int
let s = "hello"         // → str
let f = 3.14            // → f64
let b = true            // → bool

// ✅ 可推断：运算结果继承类型
let y = x + 10          // → int（x 已知为 int）
let s2 = s + " world"   // → str

// ✅ 可推断：if 表达式（分支类型统一）
let sign = if x > 0:
        "positive"      // → str
    else:
        "negative"      // → str
// sign → str

// ✅ 可推断：for 迭代元素类型
for v in [1, 2, 3]:     // List<int>
    let doubled = v * 2  // v → int, doubled → int

// ❌ 不可推断：跨函数依赖
def foo() = bar()        // 需要知道 bar() 的返回类型 → 查签名文件
def bar() = 42           // 如果 bar 无签名 → 报错

// ❌ 不可推断：递归定义
def fib(n) =             // 无法推断 n 的类型
    if n <= 1:
        n
    else:
        fib(n - 1) + fib(n - 2)
```

### 2.3 实现：`InferEngine::local()`

```
输入: FnDef AST，当前模块符号表
输出: HashMap<VarName, Type>

算法:
  for each stmt in fn.body:
    match stmt:
      Let(name, expr) →
        ty = infer_expr(expr, ctx)
        ctx.bind(name, ty)
      Assign(target, expr) →
        ty = infer_expr(expr, ctx)
        ctx.update(target, ty)
      If(cond, then, els) →
        infer_expr(then, ctx)  // 只分析，不改变外部绑定
      Return(Some(e)) →
        ret_ty = infer_expr(e, ctx)
        ctx.set_return_type(ret_ty)

  infer_expr(e, ctx):
    match e:
      IntLit    → Int
      FloatLit  → F64
      StrLit    → String
      BoolLit   → Bool
      Ident(n)  → ctx.lookup(n) or Var(n) // 自由类型变量
      Binary(l, op, r) →
        tl = infer_expr(l); tr = infer_expr(r)
        unify(tl, tr)  // Int+Int=Int, Str+Str=Str
      Call(func, args) →
        sig = ctx.lookup_signature(func)
        // 签名来自: ① 用户显式标注 ② lzi 文件 ③ 当前模块其他函数
        for (param, arg) in zip(sig.params, args):
          at = infer_expr(arg)
          unify(param.ty, at)
        sig.return_type
      _ → Var(unique_id)  // 不能推断，返回自由变量
```

### 2.4 失败处理

```
推断失败时输出的诊断信息:

error[type_required]: cannot infer type of `fib`
  → src/main.lz:10:1
  10 │ def fib(n) =
     │     ^^^ return type cannot be inferred

help: add type annotation: def fib(n: int) -> int = ...
help: or run `lz-infer src/` to generate signatures
```

---

## 三、高级外部推断（lz-infer 工具）

### 3.1 设计原则

| 原则 | 说明 |
|------|------|
| **独立工具** | 不在 `lzc` 编译管道内，是编译前的可选步骤 |
| **尽力推断** | 启发式算法，不追求 100% 覆盖 |
| **失败可覆盖** | 推断失败 → 用户手动标注 → 重新运行 → 生成完整签名 |
| **增量友好** | 只分析变更的文件，已有 `.lzi` 直接复用 |

### 3.2 签名文件格式 `.lzi`

```json
{
  "version": "0.1",
  "source": "src/utils.lz",
  "modules": {
    "utils": {
      "functions": {
        "add": {
          "params": [
            { "name": "a", "type": "int" },
            { "name": "b", "type": "int" }
          ],
          "return": "int",
          "raises": null
        },
        "filter_positives": {
          "params": [
            { "name": "xs", "type": "List<int>" }
          ],
          "return": "List<int>",
          "raises": null
        }
      },
      "structs": {
        "Point": {
          "fields": {
            "x": "f64",
            "y": "f64"
          }
        }
      },
      "consts": {
        "MAX": { "type": "int", "value": "100" },
        "PI": { "type": "f64", "value": "3.14159" }
      },
      "type_aliases": {
        "UserId": "int",
        "JsonValue": "Dict<str, str>"
      }
    }
  },
  "unresolved": [
    "utils.process_data: param 'raw' type could not be inferred"
  ]
}
```

### 3.3 推断算法（启发式）

```
Phase 1: 收集显式类型
  for each module:
    for each fn with explicit annotations:
      register fn signature in global table

Phase 2: 局部推断（同 §2.3 算法）
  for each fn without return type:
    local_infer(fn) → attempt to derive return type from body

Phase 3: 传播（最大深度 2）
  for each fn where local_infer produced Call nodes:
    if callee signature known:
      propagate param/return types
    elif callee in same module:
      attempt one-level deep inference on callee

Phase 4: 泛型占位
  for each generic fn:
    register generic parameters as TypeVar("T")
    do NOT attempt to solve — mark as "inferred_generic"

Phase 5: 输出
  for each resolved type → write to .lzi
  for each unresolved → append to "unresolved" list
```

### 3.4 不做什么（显式放弃）

| 放弃项 | 理由 |
|--------|------|
| 递归函数推断 | 用户必须在函数签名标注递归参数/返回类型 |
| HM 风格全局统一 | 实现复杂度高，收益低（静态语言用户习惯标注类型） |
| 跨文件推断（无签名） | 必须先对依赖文件运行 `lz-infer`，生成 `.lzi` 后引用 |
| 高阶类型推断 | `fn(fn(int)->str)->bool` 等，要求用户标注 |
| impl trait 推断 | trait 的方法签名必须显式标注 |

---

## 四、源码辅助检查工具 `lz-check`

### 4.1 功能

`lz-check` 是 `lz-infer` 的子命令，用于静态检查而不生成代码：

```
lz-check src/
  → 检查所有 .lz 文件的类型一致性
  → 报告缺失类型注解的位置
  → 报告类型不匹配的警告
  → 报告推断置信度低的变量（如 Var(42) → "推断为 int，置信度 95%"）
```

### 4.2 检查规则

| 规则 | 级别 | 说明 |
|------|:---:|------|
| `missing_annotation` | WARN | 变量类型完全依赖推断，建议显式标注 |
| `type_mismatch` | ERROR | 推断类型与显式标注冲突 |
| `ambiguous_call` | WARN | 调用重载函数，推断选择了特定重载 |
| `unreachable_code` | INFO | 推断过程中发现死代码 |
| `possible_panic` | WARN | 推断发现可能 panic 的路径（如除零） |

### 4.3 CLI 使用

```bash
# 生成签名文件
lz-infer src/ --output signatures.lzi

# 仅检查，不生成文件
lz-check src/

# 使用已有签名文件编译
lzc main.lz --signatures signatures.lzi

# 完整工作流
lz-infer src/ -o sigs.lzi         # 推断
lz-check src/ --sig sigs.lzi      # 检查
lzc main.lz --signatures sigs.lzi  # 编译
```

---

## 五、与现有 `src/typer/` 的关系

当前 `src/typer/mod.rs` 实现了：
- `InferSession`：结构体构造器推断
- `FnSig` 注册表 + `build_fn_registry`
- 泛型实例化 + `substitute()`
- `callable_types`：`__call__` 方法推断
- 类型传播（Phase 1~4）

### 迁移路径

| 阶段 | 内容 |
|:---:|------|
| **Phase 1**（当前） | `src/typer/` 保留为主编译器 `lzc` 的简单内置推断 |
| **Phase 2**（近期） | 从 `src/typer/` 提取复用模块 `src/infer/`，供 `lz-infer` 和 `lzc` 共享 |
| **Phase 3**（中期） | 实现 `lz-infer` 独立 binary，包含全局传播和 `.lzi` 输出 |
| **Phase 4**（远期） | `lz-check` 静态检查工具 |

---

## 六、总结

| 工具 | 职责 | 推断范围 | 失败处理 |
|------|------|:---:|------|
| `lzc`（主编译器） | 编译 `.lz` → `.rs` | 单函数内局部推断 | 报 `type_required` 错误 |
| `lz-infer` | 生成 `.lzi` 签名 | 全局传播（max depth 2） | 写入 `unresolved` 列表 |
| `lz-check` | 静态类型检查 | 全量一致性检查 | 按规则级别报告 |

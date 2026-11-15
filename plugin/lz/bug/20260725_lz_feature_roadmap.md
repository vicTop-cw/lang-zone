# LZ 语言新特性引入计划

> 基于: [20260725_peer_language_research_report.md](20260725_peer_language_research_report.md)
> 计划日期: 2026-07-25
> 目标: 将 LZ 从「Rust 语法简化版」升级为「兼具安全、开发体验、表达力的下一代语言」

---

## 一、计划总览

### 1.1 三阶段路线

```
Phase 1 (1 周)  ──→  Phase 2 (2 周)  ──→  Phase 3 (4 周)
「即刻可用」          「体验升维」          「语言壁垒」
乐观转译 + 数学语法    热重载 + strict 模式    代际引用 + 增量编译
```

### 1.2 各阶段目标

| 阶段 | 核心目标 | 关键指标 |
|------|---------|---------|
| **Phase 1** | 降低编译失败率 + 提升表达力 | LZ 编译通过率 94% → 98%+；代码量减少 20% |
| **Phase 2** | 开发体验质变 + 安全性升维 | 热重载 Demo 可用；strict 模式覆盖 100% 新项目 |
| **Phase 3** | 消除借用检查暴露 + 编译速度飞跃 | 用户不再看到 Rust 借用错误；编译速度 2-5x |

---

## 二、Phase 1：即刻可用（1 周）

### 2.1 特性 A：乐观转译策略

**设计目标**: 类型推断失败时不再阻塞编译，输出合理默认的 Rust 代码，让 rustc 做最终裁决。

**当前问题**:
```
Error: type inference failed for variable 'x' → 编译终止，用户卡住
```

**目标行为**:
```
Warning: type inference uncertain for 'x', defaulting to i64 → 编译继续
用户看到 .rs 文件，rustc 给出最终错误（如果有），source map 映射回 LZ 源码
```

**实现方案**:

```
修改文件: src/typer.rs (类型推断管道)
修改文件: src/codegen/mod.rs (代码生成)
```

**步骤 1: 类型推断降级策略**

在 [typer.rs](file:///e:/IDEProjects/AI/lang-zone/src/typer.rs) 中，当类型推断失败时，不返回 Error，而是返回带警告的默认类型：

```rust
// 当前行为（伪代码）
fn infer_type(expr: &Expr) -> Result<Type, TypeError> {
    match expr {
        Expr::Literal(Lit::Int(_)) => Ok(Type::I64),
        Expr::Var(name) => {
            env.get(name)
                .ok_or(TypeError::UnknownVar(name))  // ← 阻塞
        }
        // ...
    }
}

// 新行为：乐观推断
fn infer_type(expr: &Expr, ctx: &mut InferCtx) -> Type {
    match expr {
        Expr::Literal(Lit::Int(v)) => {
            // 从字面量推断类型
            if *v >= 0 { Type::I64 } else { Type::I64 }
        }
        Expr::Var(name) => {
            if let Some(t) = env.get(name) {
                t
            } else {
                // 乐观推断：默认 i64
                ctx.warn(Warning::TypeInferDefault {
                    var: name,
                    assumed: Type::I64,
                    reason: "variable not found in scope, assuming i64"
                });
                Type::I64
            }
        }
        Expr::FnCall { name, args } => {
            if let Some(sig) = fn_table.get(name) {
                sig.return_type
            } else {
                // 乐观推断：函数调用返回 void
                ctx.warn(Warning::FnCallUnknown {
                    name,
                    assumed: Type::Unit
                });
                Type::Unit
            }
        }
        // ...
    }
}
```

**步骤 2: 警告分级**

新增三级警告体系：

```rust
pub enum WarningLevel {
    /// 几乎确定有问题，但让 rustc 做最终检查
    Suspicious,
    /// 不确定，但选了合理默认值
    Uncertain,
    /// 信息性提示
    Info,
}

pub struct Warning {
    pub level: WarningLevel,
    pub code: String,        // 警告代码，如 "W001"
    pub message: String,
    pub span: Span,          // 源码位置
    pub suggestion: Option<String>,
}
```

**步骤 3: Source Map 错误映射增强**

在 [sourcemap.rs](file:///e:/IDEProjects/AI/lang-zone/src/sourcemap.rs) 中增强错误映射，支持将 rustc 错误自动映射回 LZ 源码：

```rust
pub fn map_rustc_error(
    rustc_error: &str,
    source_map: &SourceMap,
) -> Option<LzDiagnostic> {
    // 解析 rustc 错误格式:
    // error[E0308]: mismatched types
    //   --> src/main.rs:42:15
    //    |
    // 42 |     let x: i64 = "hello".to_string();
    //    |                  ^^^^^^^ expected i64, found String

    // 提取行号、列号 → 查 source_map → 映射回 LZ 源码位置
    // 返回 LZ 友好的诊断信息
}
```

**步骤 4: 增加 `--strict-infer` 标志**

保留原有严格模式，默认开启乐观模式：

```bash
# 默认：乐观模式
lang-zong src/main.lz --std-dir ./std

# 严格模式：类型推断失败 = 编译失败
lang-zong src/main.lz --std-dir ./std --strict-infer
```

**验收标准**:
- [ ] `_test_infer_edge.lz`（类型推断边界测试）全部通过，不再因类型推断失败而终止
- [ ] 乐观推断时输出警告而非错误
- [ ] `--strict-infer` 模式下行为与当前一致
- [ ] rustc 错误能通过 source map 映射回 LZ 源码位置

---

### 2.2 特性 B：数学友好语法

**设计目标**: 支持等式风格函数定义和声明式循环，减少数学密集型代码 20-30% 行数。

**新增语法**:

```lz
// 1. 等式风格函数定义（单表达式函数体）
def square(x) = x * x
def add(a, b) = a + b
def hypot(x, y) = (x * x + y * y).sqrt()

// 等价于
def square(x) =
    x * x

// 2. sum 循环（声明式求和）
def total(arr) =
    sum x in arr: x * x

// 等价于
def total(arr) =
    s = 0
    for x in arr:
        s = s + x * x
    s

// 3. prod 循环（声明式求积）
def factorial(n) =
    prod i in 1..=n: i

// 4. 复合使用
def variance(arr) =
    avg = (sum x in arr: x) / arr.len()
    sum x in arr: (x - avg) * (x - avg) / arr.len()
```

**实现方案**:

修改文件: [parser.rs](file:///e:/IDEProjects/AI/lang-zone/src/parser.rs) — 新增 AST 节点和解析规则

**步骤 1: 新增 AST 节点**

```rust
// src/ast.rs
pub enum Stmt {
    // ... 现有节点

    /// 等式风格函数定义: def f(x) = expr
    EquationDef {
        name: Ident,
        params: Vec<Param>,
        return_type: Option<Type>,
        body: Box<Expr>,  // 单个表达式，不是 block
    },

    /// sum/for 声明式循环
    SumLoop {
        var: Ident,
        iterable: Box<Expr>,
        body: Box<Expr>,
    },
    ProdLoop {
        var: Ident,
        iterable: Box<Expr>,
        body: Box<Expr>,
    },
}
```

**步骤 2: 解析器扩展**

```rust
// 在 parse_function_def 中检测等式风格
fn parse_function_def(&mut self) -> ParseResult<Stmt> {
    self.expect(Token::Def)?;
    let name = self.parse_ident()?;
    self.expect(Token::LParen)?;
    let params = self.parse_params()?;
    self.expect(Token::RParen)?;

    // 检测: def f(x) = expr  (等式风格)
    if self.peek() == Token::Eq {
        self.advance(); // 跳过 =
        let body = self.parse_expr()?;
        return Ok(Stmt::EquationDef { name, params, body: Box::new(body) });
    }

    // 否则: def f(x) = ... (标准风格)
    let body = self.parse_block()?;
    Ok(Stmt::FnDef { name, params, body })
}
```

**步骤 3: 代码生成**

```rust
// 等式风格 → 标准函数定义（脱糖）
fn codegen_equation_def(
    def: &EquationDef,
    ctx: &mut CodegenCtx,
) -> String {
    let params = def.params.iter()
        .map(|p| format!("{}: {}", p.name, type_to_rust(&p.ty)))
        .join(", ");
    let body = codegen_expr(&def.body, ctx);
    let ret_type = type_to_rust(&def.body.ty);

    format!(
        "pub fn {}({}) -> {} {{\n    {}\n}}",
        def.name, params, ret_type, body
    )
}

// sum 循环 → for 循环（脱糖）
fn codegen_sum_loop(
    loop_: &SumLoop,
    ctx: &mut CodegenCtx,
) -> String {
    let var = &loop_.var;
    let iter = codegen_expr(&loop_.iterable, ctx);
    let body = codegen_expr(&loop_.body, ctx);

    // let mut __sum = 0;
    // for var in iterable { __sum += body; }
    // __sum
    format!(
        "{{\n    let mut __sum = 0;\n    for {} in {} {{\n        __sum += {};\n    }}\n    __sum\n}}",
        var, iter, body
    )
}
```

**验收标准**:
- [ ] `def f(x) = x * x` 语法正确解析并生成标准 Rust 函数
- [ ] `sum x in arr: x * x` 正确展开为 for 循环
- [ ] `prod i in 1..=n: i` 正确展开为累乘循环
- [ ] 不破坏现有 `def f(x) = ...` 语法（向后兼容）
- [ ] 测试文件 `_test_math_syntax.lz` 全部通过

---

## 三、Phase 2：体验升维（2 周）

### 3.1 特性 C：Bridge 热重载

**设计目标**: 通过 Bridge 映射 Rust 的动态库加载能力，实现 LZ 代码的热重载。

**架构设计**:

```
┌─────────────────────────────────────────┐
│  LZ 源码 (.lz)                           │
│  def game_logic() = ...                  │
│  @hot_reload                             │
│  def update() = ...                      │
└──────────────┬──────────────────────────┘
               │ lang-zong --hot-reload
               ▼
┌─────────────────────────────────────────┐
│  Rust 动态库 (.dll / .so)                │
│  - 每个模块编译为独立的动态库             │
│  - 导出符号供运行时加载                   │
└──────────────┬──────────────────────────┘
               │ libloading
               ▼
┌─────────────────────────────────────────┐
│  LZ 热重载运行时 (Rust)                  │
│  - 文件监控 (notify crate)               │
│  - 动态库重加载 (libloading)             │
│  - 函数表重定向 (DispatchTable)          │
│  - 状态迁移 (StateMigration)             │
└─────────────────────────────────────────┘
```

**Bridge 映射**:

```toml
# std/modules/hot_reload.toml
[module]
name = "hot_reload"
rust_crate = "lz_hot_reload"  # 新建的辅助 crate
description = "热重载运行时"

[[functions]]
name = "watch"
params = ["path: str", "callback: fn()"]
rust_path = "lz_hot_reload::watch"
result = false

[[functions]]
name = "reload"
params = ["module_name: str"]
rust_path = "lz_hot_reload::reload"
result = true  # 返回 Result<(), Error>

[[functions]]
name = "mark_dirty"
params = ["module_name: str"]
rust_path = "lz_hot_reload::mark_dirty"
result = false
```

**LZ 用户代码示例**:

```lz
import std::hot_reload
import std::time

// 标记为可热重载的模块
@hot_reload
def game_state() =
    player_x: i64 = 0
    player_y: i64 = 0
    score: i64 = 0

// 主循环
def main() =
    hot_reload.watch("game.lz", () => reload_game())

    while true:
        update()
        render()
        time.sleep(16)  // ~60 FPS

def reload_game() =
    print("Game code changed, reloading...")
    hot_reload.reload("game")
    print("Reloaded!")
```

**实现步骤**:

1. **创建 `lz_hot_reload` crate** (Rust)
   - 文件监控: 使用 `notify` crate 监听 `.lz` 文件变化
   - 动态加载: 使用 `libloading` 加载编译后的 `.dll`/`.so`
   - 函数表: 维护 `HashMap<String, FunctionPtr>` 做函数重定向
   - 状态迁移: 支持结构体版本化，旧状态迁移到新定义

2. **创建 Bridge 映射** (`std/modules/hot_reload.toml`)

3. **编译器支持 `@hot_reload` 注解**
   - 识别 `@hot_reload` 标记的函数/模块
   - 生成 `#[no_mangle] pub extern "C"` 导出函数
   - 编译为 `dylib` 而非 `bin`

4. **编写 Demo**
   - 游戏循环热重载 Demo（Pong / 贪吃蛇）
   - 修改游戏逻辑 → 无需重启 → 立即生效

**验收标准**:
- [ ] 修改 `.lz` 文件后，运行时自动检测并重新加载
- [ ] 热重载后函数行为正确更新
- [ ] 状态迁移正确（旧版本状态迁移到新版本结构体）
- [ ] 不影响非热重载模式下的编译和性能
- [ ] Demo 可用（游戏/UI 热重载演示）

---

### 3.2 特性 D：strict 安全模式

**设计目标**: 提供可选的严格模式，在编译期强制安全编码规范，消除运行时隐患。

**strict 模式规则**:

| 规则 | 代码 | 默认模式 | strict 模式 |
|------|------|---------|------------|
| 禁止 `null` 字面量 | S001 | 允许 | **禁止** |
| 禁止 `.unwrap()` | S002 | 允许 | **禁止**（除非 `@unsafe`） |
| Result 必须处理 | S003 | 不检查 | **强制**（match 或 `?`） |
| 禁止隐式类型转换 | S004 | 允许 | **禁止** |
| 模式匹配必须穷尽 | S005 | 不检查 | **强制** |
| 禁止未使用变量 | S006 | 警告 | **错误** |
| 禁止未使用导入 | S007 | 警告 | **错误** |

**`@unsafe` 注解机制**:

```lz
// strict 模式下，需要显式标记 @unsafe 才能使用被禁止的操作
@unsafe("legacy FFI call, result guaranteed by contract")
def legacy_call() =
    let result = unsafe_ffi().unwrap()  // 需要 @unsafe 注解
    result
```

**实现方案**:

```rust
// src/semantic/strict_check.rs (新文件)
pub struct StrictChecker {
    rules: Vec<StrictRule>,
}

impl StrictChecker {
    pub fn check(&self, module: &Module) -> Vec<StrictViolation> {
        let mut violations = Vec::new();

        for stmt in &module.stmts {
            self.check_stmt(stmt, &mut violations);
        }

        violations
    }

    fn check_stmt(&self, stmt: &Stmt, violations: &mut Vec<StrictViolation>) {
        match stmt {
            // S001: 禁止 null 字面量
            Stmt::Let { value: Expr::Literal(Lit::Null), .. } => {
                violations.push(StrictViolation {
                    rule: "S001",
                    message: "null literal is forbidden in strict mode. Use Option<T> instead.",
                    suggestion: Some("Consider using Option::None".to_string()),
                    span: stmt.span(),
                });
            }

            // S002: 禁止 .unwrap()
            Stmt::Expr(Expr::MethodCall { method: "unwrap", .. }) => {
                let in_unsafe = self.is_in_unsafe_context();
                violations.push(StrictViolation {
                    rule: "S002",
                    message: ".unwrap() is forbidden in strict mode. Use match or '?' instead.",
                    suggestion: if !in_unsafe {
                        Some("Use match or add @unsafe annotation".to_string())
                    } else {
                        None // @unsafe 上下文中允许
                    },
                    span: stmt.span(),
                });
            }

            // S003: Result 必须处理
            Stmt::Expr(expr) if self.is_result_type(expr) => {
                if !self.is_result_handled(expr) {
                    violations.push(StrictViolation {
                        rule: "S003",
                        message: "Result value must be handled (match or '?').",
                        suggestion: Some("Add '?' or match on the result".to_string()),
                        span: expr.span(),
                    });
                }
            }

            // ... 其他规则
            _ => {}
        }
    }
}
```

**CLI 使用**:

```bash
# 默认模式（宽松）
lang-zong src/main.lz --std-dir ./std

# strict 模式
lang-zong src/main.lz --std-dir ./std --strict

# strict 模式 + 允许特定 unsafe 操作
lang-zong src/main.lz --std-dir ./std --strict --allow-unsafe=legacy_ffi
```

**验收标准**:
- [ ] `--strict` 模式下，违反 S001-S007 规则时报错
- [ ] `@unsafe` 注解正确豁免特定规则
- [ ] 默认模式（非 strict）行为不变
- [ ] 测试文件 `_test_strict_pass.lz` 和 `_test_strict_fail.lz` 全部通过

---

## 四、Phase 3：语言壁垒（4 周）

### 4.1 特性 E：SafeRef 安全引用类型

**设计目标**: 在 LZ 中提供 `SafeRef<T>` 类型，封装代际引用检查，让用户不再看到 Rust 的借用检查错误。

**类型系统设计**:

```lz
// SafeRef<T> — 安全引用，不需要理解借用检查
// 底层: (ptr, generation, offset)

// 创建
def make_point() =
    p = Point{x: 10, y: 20}
    ref = SafeRef.new(p)      // 对象创建时分配代数 0
    ref

// 解引用（自动验证代数）
def use_point(ref: SafeRef<Point>) =
    x = ref.x                  // 编译器插入: assert(ref.gen == obj.gen); obj.x
    y = ref.y

// 引用失效（安全 panic）
def bad_use(ref: SafeRef<Point>) =
    drop_original()             // 原对象被释放，代数递增
    x = ref.x                   // 运行时报错: "SafeRef: stale reference (gen 0 != 1)"
                                // 安全 panic，不是 UB
```

**实现方案**:

```rust
// src/safe_ref.rs (新文件)
use std::sync::atomic::{AtomicU64, Ordering};

/// 代际引用：指向一个可能已被释放或修改的对象
pub struct SafeRef<T> {
    ptr: *const T,
    gen: u64,
    offset: usize, // 内联对象的偏移量
}

/// 代际对象：包装实际对象 + 代数
pub struct GenObj<T> {
    value: T,
    gen: AtomicU64,
}

impl<T> GenObj<T> {
    pub fn new(value: T) -> Self {
        GenObj {
            value,
            gen: AtomicU64::new(0),
        }
    }

    /// 创建安全引用
    pub fn make_ref(&self) -> SafeRef<T> {
        SafeRef {
            ptr: &self.value as *const T,
            gen: self.gen.load(Ordering::Acquire),
            offset: 0,
        }
    }

    /// 释放对象（代数递增，所有旧引用失效）
    pub fn release(&mut self) {
        self.gen.fetch_add(1, Ordering::Release);
        // value 的 drop 由外部处理
    }
}

impl<T> SafeRef<T> {
    /// 解引用，验证代数
    pub fn deref(&self) -> &T {
        let obj = self.get_gen_obj();
        let current_gen = obj.gen.load(Ordering::Acquire);
        if current_gen != self.gen {
            panic!(
                "SafeRef: stale reference (gen {} != {})",
                self.gen, current_gen
            );
        }
        unsafe { &*self.ptr }
    }

    fn get_gen_obj(&self) -> &GenObj<T> {
        // 从 ptr 反向计算 GenObj 的地址
        unsafe {
            let obj_ptr = (self.ptr as *const u8)
                .offset(-(mem::size_of::<AtomicU64>() as isize));
            &*(obj_ptr as *const GenObj<T>)
        }
    }
}
```

**编译器集成**:

```rust
// src/codegen/safe_ref.rs (代码生成扩展)
impl CodegenCtx {
    /// was:  let x = ref.x
    /// now:  let x = ref.defer()?.x
    pub fn codegen_safe_ref_access(
        &self,
        ref_expr: &Expr,
        field: &str,
    ) -> String {
        let ref_code = self.codegen_expr(ref_expr);
        // 生成: ref.deref().field
        format!("{}.deref()?.{}", ref_code, field)
    }
}
```

**逐步引入策略**:

1. **Phase 3a**: 实现 `SafeRef<T>` 类型和 `GenObj<T>` 容器
2. **Phase 3b**: 编译器自动将 `&T` 引用转换为 `SafeRef<T>`（可选开启）
3. **Phase 3c**: 实现 Regions 静态分析，编译期消除不必要的代数检查

**验收标准**:
- [ ] `SafeRef<T>` 创建、解引用、失效检测正确
- [ ] 过期引用访问导致安全 panic（非 UB）
- [ ] 性能开销可接受（< 引用计数的 50%）
- [ ] 测试文件 `_test_safe_ref.lz` 全部通过

---

### 4.2 特性 F：增量编译缓存

**设计目标**: 缓存编译中间产物，只重编译变更的模块，大幅提升编译速度。

**缓存架构**:

```
.lz 源码                         编译缓存 (.lzc)
─────────                       ─────────
src/
├── main.lz ──hash──→ .lz_cache/
├── lexer.lz ──hash──→   ├── main.lzc      (AST + 类型信息 + 生成的 .rs)
├── parser.lz ──hash──→   ├── lexer.lzc     (AST + 类型信息)
└── codegen.lz ──hash──→  ├── parser.lzc    (AST + 类型信息)
                          └── codegen.lzc   (AST + 类型信息)
```

**缓存文件格式 (.lzc)**:

```rust
// src/cache.rs (新文件)
pub struct LzCache {
    pub version: u32,          // 缓存格式版本
    pub source_hash: u64,      // 源文件内容的哈希
    pub dep_hashes: HashMap<String, u64>, // 依赖模块的哈希
    pub ast: Vec<u8>,          // 序列化后的 AST
    pub types: Vec<u8>,        // 序列化后的类型信息
    pub generated_rs: String,  // 已生成的 Rust 代码
}
```

**增量编译流程**:

```rust
pub fn compile_incremental(
    entry: &Path,
    std_dir: &Path,
    cache_dir: &Path,
) -> Result<String> {
    // 1. 计算文件哈希
    let source_hash = hash_file(entry)?;

    // 2. 检查缓存
    if let Some(cache) = load_cache(entry, cache_dir) {
        if cache.source_hash == source_hash
            && all_deps_unchanged(&cache.dep_hashes)
        {
            // 缓存命中 → 直接使用缓存的 .rs 代码
            return Ok(cache.generated_rs);
        }
    }

    // 3. 缓存未命中 → 重新编译
    let module = parse(entry)?;
    let types = infer_types(&module)?;
    let rust_code = codegen(&module, &types)?;

    // 4. 保存缓存
    save_cache(entry, cache_dir, LzCache {
        version: 1,
        source_hash,
        dep_hashes: collect_dep_hashes(&module),
        ast: serialize(&module.ast)?,
        types: serialize(&types)?,
        generated_rs: rust_code.clone(),
    })?;

    Ok(rust_code)
}
```

**CLI 使用**:

```bash
# 启用增量编译
lang-zong src/main.lz --std-dir ./std --cache-dir .lz_cache

# 强制重新编译（忽略缓存）
lang-zong src/main.lz --std-dir ./std --no-cache

# 清理缓存
lang-zong clean --cache-dir .lz_cache
```

**验收标准**:
- [ ] 未修改文件二次编译使用缓存，耗时 < 10ms
- [ ] 修改文件时正确判定缓存失效
- [ ] 依赖变更时正确级联失效
- [ ] 缓存文件大小合理（< 源码大小的 5x）
- [ ] `--no-cache` 模式行为与当前一致

---

## 五、整合测试计划

### 5.1 测试矩阵

| 特性 | 单元测试 | 集成测试 | 回归测试 | Demo |
|------|---------|---------|---------|------|
| 乐观转译 | `_test_optimistic_infer.lz` | `_test_full_optimistic.lz` | 全部 122 个测试 | — |
| 数学语法 | `_test_math_syntax.lz` | `_test_math_complex.lz` | 全部 122 个测试 | `_demo_math_physics.lz` |
| 热重载 | `_test_hot_reload_unit.lz` | `_test_hot_reload_lifecycle.lz` | 全部 122 个测试 | `_demo_pong_hot_reload.lz` |
| strict 模式 | `_test_strict_pass.lz` / `_test_strict_fail.lz` | `_test_strict_full.lz` | 默认模式全部 122 个测试 | — |
| SafeRef | `_test_safe_ref.lz` | `_test_safe_ref_complex.lz` | 全部 122 个测试 | `_demo_safe_ref_game.lz` |
| 增量编译 | `_test_cache_unit.lz` | `_test_cache_full.lz` | 全部 122 个测试 | — |

### 5.2 回归测试策略

每个 Phase 完成后，必须运行全量测试套件：

```bash
# 全量回归测试
python run_tests.py

# 目标：
# Phase 1: Rustc 通过率 ≥ 95%
# Phase 2: Rustc 通过率 ≥ 95%
# Phase 3: Rustc 通过率 ≥ 95%
```

### 5.3 性能基准

每个 Phase 完成后，运行性能基准测试：

```bash
# 性能基准测试
python run_bench.py

# 目标：
# Phase 1: 性能无退化（±2%）
# Phase 2: 性能无退化（±2%）
# Phase 3: 编译速度提升 2-5x
```

---

## 六、风险评估

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|---------|
| 乐观转译导致生成错误代码 | 中 | 高 | 默认输出警告；`--strict-infer` 保留严格模式；rustc 做最终检查 |
| 热重载状态迁移失败 | 中 | 中 | 参考 Mun 的版本化状态迁移；提供 `@state_migration` 注解 |
| SafeRef 性能开销过大 | 高 | 低 | 引入 Regions 静态分析；允许用户选择不使用 SafeRef |
| 增量缓存不一致 | 高 | 中 | 严格的哈希校验；依赖图完整追踪；`--no-cache` 兜底 |
| 新特性与现有代码冲突 | 中 | 低 | 全量回归测试；每个特性独立开关 |

---

## 七、里程碑

| 里程碑 | 日期 | 交付物 | 关键指标 |
|--------|------|--------|---------|
| **M1** | D+3 | 乐观转译上线 | LZ 编译通过率 94% → 98%+ |
| **M2** | D+7 | 数学语法上线 | 代码量减少 20%；全部 122 测试通过 |
| **M3** | D+14 | 热重载 Demo | Pong 热重载 Demo 可用 |
| **M4** | D+21 | strict 模式 + 增量编译 | strict 模式覆盖新项目；编译速度 2x |
| **M5** | D+35 | SafeRef 上线 | 用户不再看到 Rust 借用错误 |
| **M6** | D+42 | 增量编译 2x-5x | 全量编译速度提升 |

---

## 八、附录：编译开关设计

为保持向后兼容和渐进式引入，所有新特性通过编译开关控制：

```bash
lang-zong src/main.lz --std-dir ./std \
    --optimistic-infer \      # 乐观转译（默认开启）
    --math-syntax \           # 数学语法（默认开启）
    --hot-reload \            # 热重载编译模式
    --strict \                # strict 安全模式
    --safe-ref \              # SafeRef 代码生成
    --cache-dir .lz_cache \   # 增量编译缓存
    --no-cache                # 禁用缓存
```

**默认行为**:
- 乐观转译: **开启**
- 数学语法: **开启**
- 热重载: 关闭
- strict 模式: 关闭
- SafeRef: 关闭
- 增量编译: 关闭（需显式指定 `--cache-dir`）

---

*计划制定时间: 2026-07-25*
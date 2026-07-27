# defer — 延迟执行

> 设计提案 | 对应关键字：`defer`（已在 `01-附录-关键字与保留字.md` §1.1 预留为规划控制流关键字）

---

## 1. 语义定义

### 1.1 执行时机

`defer` 将一段代码推迟到**当前作用域退出前**执行。作用域可以是函数体、控制流块（`if`/`for`/`match` 等）或任意 `do` 块。

```
{
    defer:
        cleanup()
    # ... 正常逻辑 ...
}   # ← 此处 cleanup() 自动执行
```

**捕获所有退出路径**：无论是正常结束、`return`、`break`/`continue` 还是 `guard` 提前返回，`defer` 始终保证执行。`?` 传播运算符和 `guard` 所转译的 Rust `return` 均在 Drop 机制下自然覆盖。

### 1.2 LIFO 栈式顺序

多个 `defer` 按 **LIFO（后进先出，Last-In-First-Out）** 顺序执行——最后注册的 `defer` 最先执行。这与 Go/ Swift/ Zig 一致。

```
defer: print("A")
defer: print("B")
defer: print("C")
# 输出：C B A
```

**设计理由**：LIFO 天然匹配资源的反向释放模式——后获取的资源通常依赖先获取的资源（如先锁外层、再锁内层；释放时先放内锁、再放外锁）。

### 1.3 作用域绑定

`defer` 绑定到其所在的**最近闭合作用域**（函数体、`if`/`for`/`match` 块、或 `do` 块），而非仅限函数级。这使得 `defer` 可精确控制细粒度资源生命周期：

```
if cond:
    f = open("a.txt")
    defer: f.close()       # if 块结束时释放
    process(f)
# ← f.close() 在此执行
f2 = open("b.txt")          # 此时 a.txt 已关，不会 fd 泄漏
```

---

## 2. 语法

### 2.1 块形式

```
defer:
    cleanup()
    more_cleanup()
```

`defer:` 后换行，缩进体为延迟执行的代码块。语法风格与 `if:` / `for:` / `match:` 等一致。

### 2.2 内联形式

```
defer: cleanup()
```

当延迟体为单表达式时可写在同一行，`:` 后保留一个空格。

### 2.3 与现有特性的语法一致性

| 特性 | 语法模板 | 说明 |
|------|----------|------|
| `if` | `if cond:` + 缩进体 | 块形式 |
| `guard` | `guard cond else:` + 缩进体 | 提前返回守卫 |
| `defer` | `defer:` + 缩进体 | 延迟执行，同上 |

`defer:` 的 `:` 后必须留白（一个空格），缩进规则遵循 [03-缩进规则.md](03-缩进规则.md) 的统 一约定。

---

## 3. 与指针的交互机制

> ⚠️ 本节为 Lang-Zong 指针语法（`ref` / `owned` / `^` 转移）配套设计。当前编译器尚未完整实现指针语义（`06-变量与绑定.md` §3 标注"未实现，规划中"），以下为 `defer` 与指针配合的设计方案。

### 3.1 捕获语义：捕获指针本身（地址），而非值

**设计决策**：`defer` 闭包捕获的是**指针变量本身**（即地址/引用），而非其在 `defer` 声明时刻指向的值。这意味着 `defer` 执行时看到的是指针所指向内存的**当前值**。

```
p: *int = alloc(int, 42)
defer:
    print(*p)       # 执行时打印 *p 的当前值，而非 defer 声明时的 42
*p = 99
# ← defer 执行：打印 99，而非 42
free(p)
```

**设计理由**：
- 捕获值（snapshot）在大多数实际场景中无意义——资源释放（`close(f)`）需要的是文件句柄本身，而非其某个历史状态。
- 对于解锁场景，必须在执行时操作互斥量本身。
- 如果用户确实需要快照值，可显式赋值：`snapshot = *p; defer: use(snapshot)`。

### 3.2 循环中的 `defer` + 指针

这是最易踩坑的场景。错误模式：

```lz
# ❌ 错误：所有 defer 捕获同一个 mut ref
for i in 0..3:
    item = &list[i]
    defer:
        process(item)     # 所有三个 defer 都捕获 &list[2]（循环结束时的值）
```

**行为分析**：由于 `defer` 捕获指针本身（引用），每次迭代创建的 `item` 在 Rust 层面是同一栈变量的复用——循环结束后所有 `defer` 都指向最后一次迭代的地址。

**正确写法**：每次迭代创建新的所有权或显式拷贝指针：

```lz
# ✅ 正确：每次迭代创建独立的 owned 变量（值拷贝）
for i in 0..3:
    item = copy(list[i])   # 值拷贝，拥有独立所有权
    defer:
        process(item)      # 每个 defer 持有自己的 item
```

或者使用 `do` 块捕获当前值：

```lz
# ✅ 正确：用 do 块引入新作用域，捕获当前迭代的快照
for i in 0..3:
    do:
        snapshot = &list[i]
        defer:
            process(snapshot)   # ✅ 每个 defer 持有不同的 snapshot
```

### 3.3 `owned` 指针与 `^` 转移

当被 `defer` 引用的变量使用 `^` 转移所有权时，**defer 在该变量被转移后不应再引用它**——编译器应在编译期报错：

```lz
def consume(owned p: *int) =
    defer:
        print(*p)            # ❌ 编译错误：p 的所有权可能已被转移
    other_func(p^)           # p 的所有权已转移
```

### 3.4 可变引用（`mut ref`）的可重入问题

```lz
mut ref guard = &mut mtx.data
defer:
    guard.finalize()         # ✅ 安全：在作用域退出前执行 finalize
# 注意：defer 体不应再次借用 guard 所指向的内存
```

---

## 4. 典型使用场景

### 4.1 资源释放

```lz
def read_config(path: str)-> Config =
    f = File::open(path)
    defer:
        f.close()            # 无论成功失败都关闭文件
    
    content = f.read()
    config = parse_config(content)
    config                   # 自动返回，f.close() 在此之前执行
```

多资源释放自动遵循 LIFO：

```lz
def copy_file(src: str, dst: str) =
    src_f = File::open(src)
    defer: src_f.close()
    
    dst_f = File::create(dst)
    defer: dst_f.close()     # dst_f 先关，src_f 后关
    
    dst_f.write(src_f.read())
```

### 4.2 解锁互斥锁

```lz
def transfer(from: &Account, to: &Account, amount: int) =
    from.lock()
    defer:
        from.unlock()        # 无论提前 return 还是抛出异常都解锁
    
    to.lock()
    defer:
        to.unlock()          # LIFO：先解 to，再解 from
    
    from.withdraw(amount)
    to.deposit(amount)
```

### 4.3 指针接收者方法中的 `defer`

```lz
struct Connection =
    fd: int
    buf: *u8

impl Connection =
    def send(self: *Connection, data: &[u8]) =
        self.buf = alloc(u8, data.len())
        defer:
            free(self.buf)   # ✅ 安全：指针接收者的生命周期在方法结束后结束
            self.buf = null
        
        copy_memory(self.buf, data)
        write(self.fd, self.buf, data.len())
```

**关键**：`self` 作为指针接收者，会被 defer 按引用捕获。在方法返回前，`self` 始终有效——与 RAII 生命周期一致。

### 4.4 计时/日志

```lz
def expensive_operation() =
    start = time::now()
    defer:
        elapsed = time::now() - start
        log(f"expensive_operation took {elapsed}ms")
    
    # ... 实际工作 ...
```

---

## 5. 返回值修改

### 5.1 最终设计：不支持修改返回值

**设计决策**：`defer` **不**支持修改函数的返回值。Lang-Zong 不引入 Go 风格的命名返回值机制。

### 5.2 设计理由

| 方面 | 分析 |
|------|------|
| **语法风格** | Lang-Zong 以表达式体为核（最后一条表达式自动 return），没有显式 return 变量名的位置。引入命名返回值会破坏表达式风格的一致性。 |
| **与指针的交互** | 若命名返回值是引用/指针类型，defer 修改它的语义会与 borrow checker 复杂交织，产生难以预测的运行时行为。 |
| **可读性** | 隐式修改返回值使得代码的控制流难以追踪——defer 本意是"清理"，不应暗含"改变结果"的职责。 |
| **替代方案** | 如需后处理，用普通变量 + 返回前显式处理更清晰。 |

### 5.3 替代方案

```lz
# ✅ 推荐的替代模式：显式变量
def process_files(files: List<str>)-> int =
    mut count = 0
    defer:
        log(f"processed {count} files")   # 可读取局部变量，不可修改返回值
    
    for f in files:
        # ... 处理文件 ...
        count += 1
    count                                    # 显式返回
```

如需在 clean-up 中修改输出：

```lz
# ✅ 包装函数模式
def safe_process()-> Result<int, str> =
    mut result: int = default
    
    # 用一个 do 块 + match 替代"defer 改返回值"
    process_internal()                       # 主逻辑直接修改 result
    if error_occurred:
        Ok(result)                           # 正常返回
    else:
        Err("failed")
```

---

## 6. 编译器实现概要

### 6.1 转译策略：Drop 作用域守卫

由于 Lang-Zong 转译为 Rust，`defer` 最自然的实现是生成一个实现了 `Drop` trait 的守卫结构体：

**Lang-Zong 源码：**
```lz
def read_file(path: str)-> str =
    f = File::open(path)
    defer: f.close()
    f.read()
```

**转译后的 Rust：**
```rust
fn read_file(path: &str) -> String {
    // 运行时生成的 defer 守卫
    use std::ops::Drop;
    struct __DeferGuard<'a, F: FnOnce(&mut ...)> {
        f: Option<F>,
        _marker: std::marker::PhantomData<&'a ()>,
    }
    
    let f = File::open(path);
    let __defer_0 = __DeferGuard {
        f: Some(|_| f.close()),
        _marker: std::marker::PhantomData,
    };
    // 函数体
    let result = f.read();
    result
    // __defer_0 在此 drop → f.close() 执行
}
```

### 6.2 多 defer 的展开

每个 `defer` 生成一个独立的守卫变量，按定义顺序构造。Rust 保证变量按**逆序 drop**，天然实现 LIFO。

```lz
defer: cleanup_a()
defer: cleanup_b()
```

→ Rust: `let __d0 = ...; let __d1 = ...;` → `__d1` 先 drop（cleanup_b），`__d0` 后 drop（cleanup_a）。

### 6.3 与现有特性的交互

| 特性 | 与 defer 的交互 | 正确性 |
|------|-----------------|--------|
| `guard cond else: expr` | 生成 Rust `return` → 作用域退出 → defer 执行 | ✅ 自然覆盖 |
| `?` 传播 | 生成 Rust `?` → 函数提前返回 → defer 执行 | ✅ 自然覆盖 |
| `return expr` | 显式返回 → 作用域退出 → defer 执行 | ✅ 自然覆盖 |
| `break`/`continue` | 循环作用域退出 → 循环内的 defer 执行 | ✅ 自然覆盖 |
| `panic`/`raise` | Rust `panic` → 栈展开 → Drop 执行 | ✅ Rust 保证 |
| `owned` 转移 | 编译器检查 defer 闭包中 `^` 转移后的变量引用 | ⚠️ 需借用检查 |
| `with` 块 | `with` 自身已有 RAII 语义，内部嵌套 defer 可补充非 RAII 清理 | ✅ 互补 |

### 6.4 性能期望

- **零额外运行时开销（无 alloc）**：守卫结构体为栈分配，`FnOnce` 闭包在 Rust 中单态化后等价于手写代码。
- **多 defer 的成本**：N 个 defer 生成 N 个守卫变量 → N 次 Drop 调用（编译器优化后可内联），与 Go 的运行时 defer 栈相比开销更低。

---

## 7. 设计取舍总结

| 决策 | 选择 | 替代方案 | 理由 |
|------|------|----------|------|
| 执行时机 | 作用域退出前 | 仅函数返回前 | 细粒度资源管理，对标 Zig |
| 顺序 | LIFO 栈式 | FIFO 队列式 | 资源反向释放的天然需求，与 Go/Zig/Swift 一致 |
| 指针捕获 | 捕获地址（按引用） | 捕获值（快照） | 释放/解锁等主流场景需要操作原对象 |
| 返回值修改 | 不支持 | Go 风格命名返回值 | 保持表达式风格一致性；清晰 > 魔法 |
| 循环中指针 | 用户负责拷贝/do 块隔离 | 自动按值捕获 | 显式 > 隐式，避免 GC 式幻觉 |
| 实现机制 | Drop 守卫（编译期生成） | 运行时 defer 栈 | 零开销抽象，与 Rust 哲学一致 |
| 关键字状态 | `defer` → 新增 token → parser → codegen | 现有预留规划名 | 已在 `01-附录.md` §1.1 列为规划关键字 |

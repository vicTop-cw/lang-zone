# LZ 自引用与 `Self` 关键字

> 规范版本: 3.3 · 设计讨论定稿 · 最后校订: 2026-08-05
> 关联：[01-类型系统.md](01-类型系统.md) §1.3 · [06a-struct.md](06a-struct.md) §2.3 · [06b-enum.md](06b-enum.md) · [06c-trait和impl.md](06c-trait和impl.md) · [13-指针与引用.md](13-指针与引用.md)

## 〇、核心结论（速览）

LZ 对「自引用（递归类型）」采用**自动独占间接**策略：

> **递归类型（struct 字段 / enum 变体）直接引用自身时，编译器自动插入独占间接层（`Box`）——写 `Self` / `Node<T>`，生成 `Box<Self>` / `Box<Node<T>>`，构造自动 `Box::new`，match 自动解引用。**

- **`next: Self?` ≡ `next: Box<Self>?`**——独占递归零成本糖化（链表、树）。
- **`Rc<Self>` / `Arc<Self>` 必须显式写**——共享所有权是编程意图，编译器不自动选择（`Rc` 单线程共享、`Arc` 多线程共享）。
- **显式指针优先级最高**：写了 `Box`/`Rc`/`Arc`/`&` 则编译器不干预。
- **`Self` 关键字**：类型定义内代表「当前类型自身」，是类型层面自引用的正名（等价写类型全名 `Node<T>`）。

---

## 一、`Self` 关键字的语义

### 1.1 三种语境

| 语境 | `Self` 代表 | 示例 |
|------|-------------|------|
| **trait 定义内** | 实现该 trait 的具体类型 | `trait Clone = def clone(self) -> Self` |
| **impl 块内** | 目标类型 | `impl Clone for Point = def clone(self) -> Self` |
| **类型定义内（struct/enum 字段）** | 当前类型自身 | `struct Node = next: Self`（自动 Box，见 §二） |

### 1.2 规则

- `Self` 是类型层面的关键字，不是值。它可出现在：方法签名（返回/参数）、trait 定义、impl 块、**struct/enum 字段类型**。
- `Self` 不要求带泛型实参——`struct Node<T>` 中 `Self` 自动展开为 `Node<T>`（含当前泛型参数）。
- 在 `impl<T> ... for Node<T>` 中，`Self` ≡ `Node<T>`；方法返回 `Self` 即返回当前实例类型。
- `self`（小写）是实例参数，`Self`（大写）是类型，二者区分明确。

```lz
struct Point =
    x: int
    y: int

impl Point =
    // Self ≡ Point（impl 无泛型时）
    def translate(self, dx: int, dy: int) -> Self =
        Point(x: self.x + dx, y: self.y + dy)
```

---

## 二、递归类型与自动独占间接（核心）

### 2.1 问题：为什么必须间接层

递归类型（链表、树、表达式树）的字段/变体直接引用自身时，若不设间接层，类型大小将是**无限的**（Rust E0072：recursive type has infinite size）——编译器无法确定结构体占多少字节。

```lz
// ❌ 直接递归：类型大小无限，编译错误（对应 Rust E0072）
struct BadNode =
    value: int
    next: BadNode      // 直接引用自身 → 无限大小
```

**解决方案**：间接层（指针）。独占所有权用 `Box`（堆分配），共享所有权用 `Rc`/`Arc`。

### 2.2 自动独占间接规则（LZ 行为）

**当 struct 字段 / enum 变体字段直接引用自身类型（`Self` 或类型全名，且无显式指针）时，编译器自动插入 `Box` 独占间接层。**

| 写法（LZ 源码） | 生成（Rust） | 说明 |
|------------------|--------------|------|
| `next: Self?` | `next: Option<Box<Self>>` | 自动 Box，免写 `Box<` |
| `next: Node<T>?` | `next: Option<Box<Node<T>>>` | 类型全名同样触发 |
| `next: Self` | `next: Box<Self>` | 非 Option 同样触发 |
| `left: Expr`（enum 变体） | `left: Box<Expr>` | enum 变体字段自动 Box |
| `next: Box<Self>?` | `next: Option<Box<Self>>` | 显式 Box：不干预（优先级最高） |
| `next: Rc<Self>?` | `next: Option<Rc<Self>>` | 显式 Rc：不干预（共享意图） |
| `next: Arc<Self>?` | `next: Option<Arc<Self>>` | 显式 Arc：不干预（线程安全共享） |
| `next: &Self` / `next: ref Self` | `next: &Self` | 显式引用：不干预 |

**判定算法**（编译器统一处理 struct 与 enum）：

1. 检查字段/变体的类型是否**直接引用**当前类型（`Self` 或类型全名，含通过 `Option<T>`/`List<T>` 等容器间接包裹但内部仍是自身）。
2. 若引用自身且**无显式指针层**（`Box`/`Rc`/`Arc`/`&`/`ref` 不出现）→ 自动包 `Box`。
3. 若已有显式指针 → 原样保留，不干预。
4. 间接递归（`A` 含 `B`，`B` 含 `A`）同样触发——沿类型图检测环，环上的直接自引用边自动 Box。

### 2.3 三处配套（构造、匹配、访问）

自动 Box 不是孤立糖，编译器在**定义/构造/匹配**三处配套处理：

| 阶段 | 自动行为 |
|------|----------|
| **定义** | 字段/变体类型 `Box` 化（§2.2） |
| **构造** | `Node(next: child)` 自动 `next: Box::new(child)`；`Option<Box<_>>` 时 `Some(Box::new(child))` |
| **匹配** | `case Node(next: n):` 自动解引用 `let n = *n;`（对 `Option` 先解 `Some` 再 `*`） |

```lz
// 链表：源码级零 Box
struct Node<T> =
    value: T
    next: Self?          // 自动 Box<Self>? —— 无需写 Box<

def append<T>(mut head: Node<T>?, value: T) -> Node<T> =
    match head:
        case None:
            Node(value: value, next: None)        // 构造自动 Some(Box::new(None))
        case Some(node):
            // node 已自动解引用（Box → 值）
            let child = append(node.next, value)
            Node(value: node.value, next: Some(child))  // 自动 Box::new
```

```lz
// 表达式树：enum 变体自动 Box
enum Expr:
    Num(value: f64)
    Add(left: Expr, right: Expr)    // 自动 Box<Expr>
    Mul(left: Expr, right: Expr)    // 自动 Box<Expr>

def eval(e: Expr) -> f64 =
    match e:
        case Expr.Num(value: v) => v
        case Expr.Add(left: l, right: r) => eval(l) + eval(r)   // l, r 已自动解引用
        case Expr.Mul(left: l, right: r) => eval(l) * eval(r)
```

### 2.4 边界与限制

- **自动 Box 永远是独占所有权**（`Box`）。共享（`Rc`/`Arc`）与借用（`&`/`ref`）必须显式——因为所有权语义是编程意图，编译器不猜测。
- **`dyn` 对象化**：`Box<Self>` 可进一步对象化为 `Box<dyn Trait>`（见 13-指针与引用 §4.1），递归类型中的自动 Box 同样可被 trait 对象化覆盖。
- **大小保证**：自动 Box 保证结构体大小有限（字段是 8 字节指针），转译器无需担心 E0072。
- **语义透明**：自动 Box 不影响值语义——`node.next` 访问仍像「直接字段」；所有权仍是独占（一个字段只被一个节点拥有）。

---

## 三、共享所有权（Rc / Arc）显式写法

共享递归（图、循环引用、多引用计数）必须显式选择所有权：

```lz
// 单线程共享链表：Rc + 内部可变（图/共享子节点）
struct GraphNode =
    value: int
    neighbors: List<Rc<Self>>        // 显式 Rc：多节点共享子节点

// 多线程共享：Arc
struct SharedNode =
    value: int
    next: Option<Arc<Self>>          // 显式 Arc：跨线程共享

// 循环引用需要 Weak 破环（可选）
struct CycNode =
    value: int
    next: Option<Rc<Self>>
    prev: Option<Weak<Self>>         // 显式 Weak 防循环计数泄漏
```

> **原则**：`Rc`/`Arc`/`Weak` 不自动产生。自动独占（`Box`）覆盖 90% 的自引用场景（链表、树、AST）；共享是架构决策，写出来反而让所有权清晰。

---

## 四、与普通（非递归）字段的区别

| 字段类型 | 是否引用自身 | 处理 |
|----------|:---:|------|
| `data: T`（泛型参数） | 否 | 原样生成 |
| `children: List<OtherStruct>` | 否（引用其他类型） | 原样生成 |
| `parent: Option<Self>` | **是** | **自动 `Option<Box<Self>>`** |
| `left: Expr`（enum 内） | **是** | **自动 `Box<Expr>`** |
| `next: Rc<Self>` | 是（显式指针） | 不干预（保持 Rc） |

---

## 五、编译期验证汇总

| 检查项 | 行为 |
|--------|------|
| 直接递归字段/变体（无显式指针） | 自动 Box（不报错） |
| 显式 `Box`/`Rc`/`Arc`/`&` 递归字段 | 原样保留（不干预） |
| 间接递归（A↔B） | 沿类型图检测环，自动 Box 环上直接自引用 |
| `Self` 用于非法位置（如模块顶层裸用） | 编译错误（`Self` 仅在类型定义/trait/impl 内） |
| 在非泛型 impl 中 `Self` ≡ 具体类型 | 正常 |
| 自动 Box 后字段访问 `node.next` | 透明（等价直接字段访问） |

---

## 六、设计取舍说明

- **为何自动 Box 而非显式**：链表/树是压倒性多数的自引用场景，且天然独占——自动 Box 零语义损失、免 `Box<` 噪声；这是 Rust「坚持显式」与 Swift「indirect」之间的折中，落在「独占自动、共享显式」。
- **为何 `Rc`/`Arc` 不自动**：共享所有权是多引用设计决策（谁持有、何时释放、是否跨线程），编译器无法替你选，显式写出反而让所有权架构可见。
- **与 `?` 后缀的协作**：`Self?` 中的 `?` 是 `Option` 简写（见 01 §2.2），与自动 Box 正交——`Self?` = `Option<Self>`（值），再自动 Box = `Option<Box<Self>>`。
- **转译器要求**：实现时须在 struct/enum 定义生成处做 `type_refers_to` 环检测（enum 已有，struct 需补齐），并在构造/匹配处对称处理（Box 包装、解引用）。

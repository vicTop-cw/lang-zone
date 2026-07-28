# Number Trait 与 @math! 宏设计

> 2026-07-26 · v1.0

## 一、动机

```lz
// 当前写法 —— 所有参数必须标注 int
def f(x: int, y: int) -> int = x * x + y * y

// 期望写法 —— @math! 宏加持，参数自动获得 Number 约束
@math!
def f(x, y) = x * x + y * y
```

数学表达式（`+ - * /` 等二元运算）不应该硬编码到 `i64` 上。`@math!` 宏将无类型参数的语义从"默认 i64"提升为"满足 Number trait 的任意类型"。

## 二、Number Trait 定义

### 2.1 核心接口

```lz
trait Number:
    """所有数值类型的统一抽象"""

    // ── 算术 ──
    def __add__(self, other: Self) -> Self
    def __sub__(self, other: Self) -> Self
    def __mul__(self, other: Self) -> Self
    def __div__(self, other: Self) -> Self
    def __neg__(self) -> Self
    def __abs__(self) -> Self

    // ── 比较 ──
    def __eq__(self, other: Self) -> bool
    def __lt__(self, other: Self) -> bool

    // ── 构造 ──
    def zero() -> Self        // 加法单位元 0
    def one() -> Self         // 乘法单位元 1

    // ── 转换 ──
    def to_int(self) -> int
    def to_float(self) -> f64
```

### 2.2 内置 impl

```lz
impl Number for int:
    def __add__(self, other: int) -> int = self + other
    def zero() -> int = 0
    def one() -> int = 1
    // ... (委托给 rustc 原生运算)

impl Number for f64:
    def __add__(self, other: f64) -> f64 = self + other
    def zero() -> f64 = 0.0
    def one() -> f64 = 1.0
    // ...
```

### 2.3 Rust 侧映射

| LZ trait 方法 | Rust trait 方法 | 对应关系 |
|--------------|----------------|---------|
| `__add__` | `std::ops::Add` | `a + b` |
| `__sub__` | `std::ops::Sub` | `a - b` |
| `__mul__` | `std::ops::Mul` | `a * b` |
| `__div__` | `std::ops::Div` | `a / b` |
| `__neg__` | `std::ops::Neg` | `-a` |
| `zero()` | `num_traits::Zero` | `0` |
| `one()` | `num_traits::One` | `1` |

### 2.4 子 trait 扩展（未来）

```lz
trait Integer: Number:
    def __mod__(self, other: Self) -> Self   // a % b
    def __bit_and__(self, other: Self) -> Self
    def __bit_or__(self, other: Self) -> Self

trait Float: Number:
    def sqrt(self) -> Self
    def sin(self) -> Self
    def cos(self) -> Self
```

## 三、@math! 宏语义

### 3.1 转换规则

```
@math!
def f(x, y) = x * x + y * y

↓ 展开 ↓

def f<T: Number>(x: T, y: T) -> T = x * x + y * y
```

| 特性 | 无 @math! | 有 @math! |
|------|-----------|----------|
| 无注解参数 | `i64` (硬编码) | `T: Number` (泛型) |
| 无注解返回 | `i64` | `T` (与参数同类型) |
| 字面量 `0` | `0i64` | `Number::zero()` |
| 字面量 `1` | `1i64` | `Number::one()` |
| 混合类型 | 报错 | 报错 (需显式转换) |

### 3.2 Codegen 行为（当前阶段）

由于 trait 泛型尚未完全支持 codegen，当前 `@math!` 的实际行为：

```
@math!
def f(x, y) = x * x + y * y

↓ (当前 codegen) ↓

// #[math] — Number-trait polymorphic, currently specialized to i64
pub fn f(x: i64, y: i64) -> i64 { x * x + y * y }
```

生成 `i64` 特化版本 + 注释标注该函数应从 `Number` trait 泛化。

### 3.3 未来完整泛型

当 codegen 支持 `where T: Number` 后：

```rust
pub fn f<T: std::ops::Add<Output = T> + std::ops::Mul<Output = T> + Copy>(x: T, y: T) -> T {
    x * x + y * y
}
```

## 四、解析实现

### 4.1 装饰器注册

```rust
// src/parser/parser.rs — parse_decorator 已支持 @name 语法
@math!   // → Decorator { name: "math", args: [] }
```

### 4.2 AST 标记

```rust
// src/ast/decl.rs
pub struct Function {
    // 已有字段
    pub decorators: Vec<Decorator>,
    // ...
}

// 辅助方法
impl Function {
    pub fn is_math_polymorphic(&self) -> bool {
        self.decorators.iter().any(|d| d.name == "math")
    }
}
```

## 五、语义检查

```
@math! 函数约束:
  1. 所有参数 @math! 宏自动视为 T: Number
  2. 不允许 mixed Number 子类型（int 和 f64 不能混用）
  3. 字面量 0/1 自动升级为 Number::zero()/Number::one()
  4. 返回类型推导为参数类型 T（除非显式标注）
```

## 六、示例

```lz
// ✅ 平方和 — 适用于任意 Number 类型
@math!
def sum_sq(x, y) = x * x + y * y
// → def sum_sq<T: Number>(x: T, y: T) -> T = x * x + y * y

// ✅ 毕达哥拉斯定理
@math!
def hypot(x, y) = (x * x + y * y).sqrt()

// ✅ 两点距离（带显式返回类型）
@math!
def distance(x1, y1, x2, y2) -> f64 =
    dx = x2 - x1
    dy = y2 - y1
    (dx * dx + dy * dy).sqrt()

// ❌ 不允许 — 混合 int 和 f64
@math!
def bad(x: int, y) = x + y   // x:int 与 y:T 类型矛盾
```

## 七、实现顺序

| Phase | 内容 |
|:-----:|------|
| P1 | `Number` trait 规范文档（本文件） |
| P2 | `@math!` 解析 + AST 标记 |
| P3 | `@math!` codegen（当前: i64 特化 + 注释） |
| P4 | 测试套件 |
| P5 | 未来: 泛型 codegen 支持 `T: Number` |

---

*设计完。下一步: 实现 @math! 宏的解析 + codegen。*

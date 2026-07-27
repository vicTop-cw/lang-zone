# 魔法方法 → 自动 trait 推导

> 声明魔法方法 → 编译器自动推导对应 Rust trait + impl。支持签名多分派。

## 核心规则

```
struct T =
    def __add__(self: T, other: T)-> T = ...
    # ↑ 编译器自动生成: impl Add for T { ... }
```

**只要在 struct 内定义了 `__xxx__` 魔法方法，编译器就自动 `impl` 对应的 Rust trait。零样板代码。**

---

## 一、运算符

| 魔法方法 | 签名 | 自动生成 |
|----------|------|---------|
| `__add__` | `(self, other: T) -> T` | `impl Add<T> for T` |
| `__add__` | `(self, other: U) -> T` | `impl Add<U> for T` |
| `__sub__` | `(self, other: T) -> T` | `impl Sub<T> for T` |
| `__mul__` | `(self, other: T) -> T` | `impl Mul<T> for T` |
| `__div__` | `(self, other: T) -> T` | `impl Div<T> for T` |
| `__rem__` | `(self, other: T) -> T` | `impl Rem<T> for T`（对应 `%` 运算符） |
| `__neg__` | `(self) -> T` | `impl Neg for T` |
| `__not__` | `(self) -> T` | `impl Not for T`（对应 `!` 逻辑非） |
| `__bitand__` | `(self, other: T) -> T` | `impl BitAnd<T> for T`（对应 `&`） |
| `__bitor__` | `(self, other: T) -> T` | `impl BitOr<T> for T`（对应 `|`） |
| `__bitxor__` | `(self, other: T) -> T` | `impl BitXor<T> for T`（对应 `^`） |
| `__shl__` | `(self, other: T) -> T` | `impl Shl<T> for T`（对应 `<<`） |
| `__shr__` | `(self, other: T) -> T` | `impl Shr<T> for T`（对应 `>>`） |
| `__eq__`  | `(self, other: T) -> bool` | `impl PartialEq<T> for T` |
| `__ne__`  | `(self, other: T) -> bool` | `impl PartialEq` (补全) |
| `__lt__`  | `(self, other: T) -> bool` | `impl PartialOrd<T> for T` |
| `__cmp__` | `(self, other: T) -> Ordering` | `impl Ord for T` |
| `__iadd__` | `(self, other: T)` | `impl AddAssign<T> for T`（对应 `+=`）
| `__isub__` | `(self, other: T)` | `impl SubAssign<T> for T`（对应 `-=`）
| `__imul__` | `(self, other: T)` | `impl MulAssign<T> for T`（对应 `*=`）
| `__idiv__` | `(self, other: T)` | `impl DivAssign<T> for T`（对应 `/=`）

**多分派示例：**
```
struct Vector =
    x: f64
    y: f64

    def __add__(self: Vector, other: Vector)-> Vector =
        Vector(self.x + other.x, self.y + other.y)

    def __add__(self: Vector, scalar: f64)-> Vector =
        Vector(self.x + scalar, self.y + scalar)

    def __eq__(self: Vector, other: Vector)-> bool =
        self.x == other.x and self.y == other.y
```

编译器根据 `other` 类型自动生成：
- `impl Add<Vector> for Vector`
- `impl Add<f64> for Vector`
- `impl PartialEq<Vector> for Vector`

---

## 二、容器/迭代

| 魔法方法 | 签名 | 自动生成 |
|----------|------|---------|
| `__getitem__` | `(self, index: int) -> T` | `impl Index<int> for Self` |
| `__getitem__` | `(self, index: str) -> T` | `impl Index<str> for Self` |
| `__setitem__` | `(self, index: int, value: T)` | `impl IndexMut<int> for Self` |
| `__len__` | `(self) -> int` | `impl ExactSizeIterator` 辅助 |
| `__iter__` | `(self) -> Iterator` | `impl IntoIterator for Self` |
| `__next__` | `(self) -> Option<T>` | `impl Iterator for Self` |
| `__contains__` | `(self, item: T) -> bool` | 辅助 `contains()` |

---

## 三、生命周期/资源

✅ 已实现：`__drop__`/`__clone__`/`__copy__`/`__default__` 已注册在魔法方法表中，对应的 `Drop`/`Clone`/`Copy`/`Default` trait 自动推导可用。`ptr` 类型与 `null` 内建值仍在规划中：

| 魔法方法 | 签名 | 自动生成 |
|----------|------|---------|
| `__drop__` | `(self)` | `impl Drop for Self` |
| `__clone__` | `(self) -> Self` | `impl Clone for Self` |
| `__copy__` | `(self) -> Self` | `impl Copy for Self` |
| `__enter__` | `(self) -> Self` | 资源获取 |
| `__exit__` | `(self, err: Option<Error>) -> bool` | 资源释放 |

```
struct File =
    path: str
    handle: ptr

    def __enter__(self: File)-> File =
        self.open()
        self

    def __exit__(self: File, err: Option<Error>)-> bool =
        self.close()
        false

    def __drop__(self: File) =
        if self.handle != null:
            self.close()
```

---

## 四、类型转换

> ⚠️ 未实现（规划中）：`int(...)` 内建类型转换（如 `int(hex[1..3], 16)`）当前编译器不支持，`int` 只是 `i64` 的别名而非转换函数，示例暂无法通过 lzc 编译。魔法方法自动推导 trait 本身的概念已实现，可保留。

| 魔法方法 | 签名 | 自动生成 |
|----------|------|---------|
| `__into__` | `(self) -> T` | `impl Into<T> for Self` |
| `__from__` | `(value: T) -> Self` | `impl From<T> for Self` |
| `__try_into__` | `(self) -> Result<T, Error>` | `impl TryInto<T> for Self` |
| `__try_from__` | `(value: T) -> Result<Self, Error>` | `impl TryFrom<T> for Self` |
| `__str__` | `(self) -> str` | `impl Display for Self` |
| `__repr__` | `(self) -> str` | `impl Debug for Self` |
| `__hash__` | `(self) -> int` | `impl Hash for Self` |
| `__call__` | `(self, args...) -> T` | `impl FnOnce/FnMut/Fn for Self` |
| `__default__` | `() -> Self` | `impl Default for Self` |

```
struct Color =
    r: int
    g: int
    b: int

    def __str__(self: Color)-> str =
        f"rgb({self.r}, {self.g}, {self.b})"

    def __from__(hex: str)-> Color =
        Color(
            int(hex[1..3], 16),
            int(hex[3..5], 16),
            int(hex[5..7], 16),
        )

    def __default__()-> Color =
        Color(0, 0, 0)
```

---

## 五、完整示例

```
struct Vector =
    x: f64
    y: f64

    # ── 运算符 ──
    def __add__(self: Vector, other: Vector)-> Vector =
        Vector(self.x + other.x, self.y + other.y)

    def __add__(self: Vector, scalar: f64)-> Vector =
        Vector(self.x + scalar, self.y + scalar)

    def __eq__(self: Vector, other: Vector)-> bool =
        self.x == other.x and self.y == other.y

    # ── 显示 ──
    def __str__(self: Vector)-> str =
        f"({self.x}, {self.y})"

    # ── 迭代（解构） ──
    def __iter__(self: Vector) =
        yield self.x
        yield self.y

    # ── 默认值 ──
    def __default__()-> Vector =
        Vector(0.0, 0.0)

# 编译器自动生成:
# impl Add<Vector> for Vector
# impl Add<f64> for Vector
# impl PartialEq for Vector
# impl Display for Vector
# impl IntoIterator for Vector
# impl Default for Vector

v1 = Vector(1.0, 2.0)
v2 = Vector(3.0, 4.0)
v3 = v1 + v2           # Vector(4.0, 6.0)
v4 = v1 + 5.0          # Vector(6.0, 7.0)  — 多分派
print(v1 == v2)        # false
print(v3)              # "(4.0, 6.0)"
for coord in v1:
    print(coord)        # 1.0\n2.0
```

## 六、多分派规则

同名的 `__xxx__` 可以定义多次，编译器根据**参数类型组合**自动匹配：

```
def __add__(self, T)-> T        → Add<T>
def __add__(self, U)-> T        → Add<U>  (不同右值类型)
def __add__(self, int)-> T      → Add<i64>
def __add__(self, f64)-> T      → Add<f64>
```

**优先级：** 精确类型匹配 > 泛型匹配 > trait 约束匹配。

## 七、不支持自动推导的 trait

需要手动 `impl`：

- `unsafe trait` — 不安全 trait
- 关联类型复杂的 trait
- 需要生命周期标注的 trait

## 语法边界

- ✅ 在 struct 内定义 `__xxx__` 魔法方法，编译器自动 `impl` 对应 Rust trait（零样板），概念已实现。
- ✅ 魔法方法名必须为双下划线包围形式 `__xxx__`（如 `__add__`、`__iter__`、`__str__`、`__from__`）。
- ✅ 方法首个参数 `self` 默认推导为 `&self`；如需所有权/可变可显式写 `self: T` / `mut self`。
- ❌ `ptr` 类型与 `null` 内建值未实现，示例 `handle: ptr` / `self.handle != null` 无法通过 lzc 编译。
- ❌ `int(...)` 内建类型转换未实现（`int` 仅是 `i64` 别名），如 `int(hex[1..3], 16)` 暂不支持。
- ❌ 关联类型复杂的 trait、需要生命周期标注的 trait 不支持自动推导，需手动 `impl`。

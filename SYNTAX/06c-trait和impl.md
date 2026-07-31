# LZ trait 与 impl

> 规范版本: 3.2 · 基于编译器源码 · 最后校订: 2026-07-31

本文档定义 Lang-Zong 的 trait（特征）、impl（实现）及泛型参数语法。

---

## 一、trait 定义

```lz
trait Drawable =
    def draw(self) -> ()
    def bounds(self) -> (f64, f64, f64, f64)
```

- 使用 `trait Name =` 语法（等号 + 换行缩进）
- trait 内定义方法签名，每个方法以 `def` 开头
- **`self` 参数无需显式类型注解**：trait 中 `self` 自动推导为 `Self`，impl 中自动推导为实现类型。编译器内部补全，用户可省略。显式写出 `self: Self` 也是合法的，等价于省略。

---

## 二、抽象方法

```lz
trait Drawable =
    def draw(self) -> () = ...
    def bounds(self) -> (f64, f64, f64, f64) = ...
```

- `= ...` 表示抽象方法 — **无实现**，必须由 impl 块提供
- 包含至少一个 `= ...` 方法的 trait 不能直接实例化
- > **注意 `...` 的多重语义**：trait 中 `= ...` 表示"无默认实现"（不是"待实现/占位"），区别于：可变参数中的 `..` 分隔符、解构中的 `..` rest 模式。三者用法互不重叠，靠上下文区分。

---

## 三、默认实现

```lz
trait Greeter =
    def greet(self) -> str = f"Hello, I am {self.name()}"
    def name(self) -> str = ...
```

- `= expr` 提供**默认实现**
- `= ...` 表示**抽象方法**（必须由 impl 提供）
- trait 可以混合默认实现和抽象方法

---

## 四、泛型 trait

```lz
trait Container<T> =
    def get(self, index: int) -> T = ...
    def len(self) -> int = ...
```

- trait 可以定义泛型参数，在方法签名中使用
- 泛型参数紧跟 trait 名，使用尖括号

---

## 五、关联类型

```lz
trait Iterator =
    type Item
    def next(self) -> Option<Self.Item> = ...
```

- 使用 `type Name` 在 trait 内定义关联类型
- impl 时需要指定具体类型
- 通过 `Self.Item` 在 trait 内引用关联类型

---

## 六、impl 实现

### 6.1 实现 trait

```lz
impl Drawable for Rectangle =
    def draw(self) -> () =
        print(f"Rectangle({self.width}, {self.height})")
    def bounds(self) -> (f64, f64, f64, f64) =
        (0.0, 0.0, self.width, self.height)
```

- 语法：`impl TraitName for TypeName =`
- 必须实现 trait 中所有 `= ...` 的抽象方法
- 可覆盖 trait 的默认实现

### 6.2 固有方法（无 trait）

```lz
impl Rectangle =
    def new(width: f64, height: f64) -> Rectangle =
        Rectangle(width: width, height: height)
```

- 省略 `for Type` 即为"固有 impl"
- 用于定义不属于任何 trait 的类型关联函数和方法
- 一个类型可以有多个 impl 块

---

## 七、泛型 impl

```lz
impl<T> Display for Pair<T, T>
    where T: Display =
    def fmt(self) -> str =
        f"({self.first}, {self.second})"
```

- impl 可以带泛型参数和 trait 约束
- 语法：`impl<TypeParam> TraitName for TypeName where TypeParam: Constraint =`

---

## 八、泛型参数

所有 struct/enum/trait/def 都支持泛型参数，包含以下能力：

### 8.1 类型参数

```lz
def identity<T>(x: T) -> T = x
struct Box<T> =
    value: T
```

### 8.2 trait 约束

约束不写在尖括号内，统一通过 `where` 子句表达：

```lz
def larger<T>(a: T, b: T) -> T
    where T: Ordered =
  a if a > b else b
```

### 8.3 多约束

```lz
def clone_and_print<T>(x: T)
    where T: Clone + Display =
    let copy = x.clone()
    print(copy)
```

- 使用 `+` 连接多个 trait 约束

### 8.4 where 子句

```lz
def process<T, U>(a: T, b: U) -> List<T>
    where T: Clone, U: Into<T> =
    let result = a.clone()
    [result, b.into()]
```

- 复杂约束可使用 `where` 子句，放在函数体之前

### 8.5 默认参数类型

```lz
def make_pair<T = int>(x: T, y: T) -> (T, T) = (x, y)
make_pair(1, 2)      // T = int
make_pair("a", "b")  // T = str
```

- 泛型参数可以有默认类型，调用时类型可被自动推断

---

## 九、错误语法边界

```lz
// ❌ trait 方法用 : 而非 =
trait Bad:
    def f(self): ... // 错误：trait 方法签名用 = 分隔

// ❌ impl 缺少 for
impl for Point =     // 错误：应为 impl TraitName for TypeName

// ❌ trait 用 : 声明
trait Drawable:      // 错误：trait 需用 = 而非 :
    def draw(self)

// ❌ 泛型尖括号后有空格
def foo <T> (x: T)   // 错误：<T> 紧贴函数名
def foo<T>(x: T)     // 正确
```

---

*上一章：[06b-enum](06b-enum.md)* · *下一章：[06d-内置魔法trait和全局函数](06d-内置魔法trait和全局函数.md)*

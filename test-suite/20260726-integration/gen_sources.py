#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""批量生成 ~210 个测试 .lz 源文件"""
import os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
CASES = os.path.join(HERE, "cases")

S = {}  # {relpath: content}

def f(path, content):
    S[path] = content

# ==================== LEX (词法层) ====================
# 关键字 - 前 10 个已在 run_tests.py 中定义，这里补充 tokens 模式的文件
# (tokens 模式不需要完整的可编译程序)

# 注释和字面量也已在 run_tests.py 中定义

# ==================== TYPES (类型系统) ====================
f("types/primitives/TYP-PRIM-001.lz", '''def f(x: int) -> int = x + 1
def main():
    print(f(41))
''')
f("types/primitives/TYP-PRIM-002.lz", '''def add(a: float, b: float) -> float = a + b
def main():
    print(3.5 + 4.0)
''')
f("types/primitives/TYP-PRIM-003.lz", '''def greet(name: str) -> str = "Hello, " + name
def main():
    print(greet("World"))
''')
f("types/primitives/TYP-PRIM-004.lz", '''def is_pos(x: int) -> bool = x > 0
def main():
    print(is_pos(5))
''')
f("types/primitives/TYP-PRIM-005.lz", '''def f_i32(x: i32) -> i32 = x
def f_u32(x: u32) -> u32 = x
def f_u64(x: u64) -> u64 = x
def f_f32(x: f32) -> f32 = x
''')
f("types/primitives/TYP-PRIM-006.lz", '''def ch() -> char = 'a'
''')

f("types/containers/TYP-CON-001.lz", '''def main():
    n: List<int> = [1, 2, 3]
    print(n[0])
    print(n[1])
    print(n[2])
''')
f("types/containers/TYP-CON-002.lz", '''def main():
    d: Dict<str, int> = {"a": 1}
    print(d["a"])
''')
f("types/containers/TYP-CON-003.lz", '''def f() -> Set<str> = {"a", "b"}
''')
f("types/containers/TYP-CON-004.lz", '''def f() -> Array<int, 3> = [1, 2, 3]
''')
f("types/containers/TYP-CON-005.lz", '''def f() -> (int, str) = (42, "hello")
def main():
    (a, b) = f()
    print(a)
    print(b)
''')
f("types/containers/TYP-CON-006.lz", '''def f() = 1..10
''')

f("types/option/TYP-OPT-001.lz", '''def f(x: int?) -> int? = x
''')
f("types/option/TYP-OPT-002.lz", '''def main():
    x = Some(42)
    print(x.unwrap())
''')
f("types/option/TYP-OPT-003.lz", '''def main():
    a: int? = Some(42)
    b: int? = None
    print(a ?? 0)
    print(b ?? 0)
''')

f("types/generics/TYP-GEN-001.lz", '''def id<T>(x: T) -> T = x
def main():
    print(id(42))
    print(id("hello"))
''')
f("types/generics/TYP-GEN-002.lz", '''def max_val<T>(a: T, b: T) -> T where T <: Ord = if a > b: a else: b
''')
f("types/generics/TYP-GEN-003.lz", '''def dup<T>(x: T) -> (T, T) where T <: Clone + Ord = (x.clone(), x.clone())
''')
f("types/generics/TYP-GEN-004.lz", '''struct Pair<T, U> = first: T, second: U
''')

f("types/alias/TYP-ALIAS-001.lz", '''type ID = int
def f(x: ID) -> ID = x
''')
f("types/alias/TYP-ALIAS-002.lz", '''type Pair<T> = (T, T)
''')

# ==================== EXPR (表达式) ====================
f("expr/literals/EXP-LIT-001.lz", '''def main():
    print(42 + 8)
''')
f("expr/literals/EXP-LIT-002.lz", '''def main():
    print(0xFF + 0o77 + 0b1010)
''')
f("expr/literals/EXP-LIT-003.lz", '''def main():
    print(3.14 + 1.0)
''')
f("expr/literals/EXP-LIT-004.lz", '''def is_true() -> bool = True
def main():
    print(is_true())
''')
f("expr/literals/EXP-LIT-005.lz", '''def f() = None
''')
f("expr/literals/EXP-LIT-006.lz", '''def main():
    name = "LZ"
    print(f"Hello, {name}")
''')
f("expr/literals/EXP-LIT-007.lz", '''def main():
    p = r"C:\\path"
    print(p)
''')
f("expr/literals/EXP-LIT-008.lz", '''def main():
    s = """line1
line2"""
    print(s)
''')

# 运算符
f("expr/operators/EXP-OP-001.lz", '''def main():
    a = 10
    b = 5
    print(a + b)
    print(a - b)
    print(a * b)
    print(a / b)
    print(a % b)
''')
f("expr/operators/EXP-OP-002.lz", '''def main():
    print(2 ** 3)
''')
f("expr/operators/EXP-OP-003.lz", '''def main():
    print(1 == 1)
    print(1 == 2)
    print(1 != 2)
''')
f("expr/operators/EXP-OP-004.lz", '''def main():
    print(1 < 2)
    print(2 > 1)
    print(1 >= 2)
    print(1 <= 1)
    print(1 <= 2)
''')
f("expr/operators/EXP-OP-005.lz", '''def main():
    x = 5
    print(x > 0 and x < 10)
    print(x > 10 and x < 20)
    print(x > 0 or x < 0)
''')
f("expr/operators/EXP-OP-006.lz", '''def main():
    print(not False)
    print(not True)
''')
f("expr/operators/EXP-OP-007.lz", '''def main():
    print(3 & 1)
''')
f("expr/operators/EXP-OP-008.lz", '''def main():
    print(1 | 2)
''')
f("expr/operators/EXP-OP-009.lz", '''def main():
    print(3 ^ 5)
''')
f("expr/operators/EXP-OP-010.lz", '''def main():
    print(2 << 2)
    print(4 >> 2)
''')
f("expr/operators/EXP-OP-011.lz", '''def f() -> bool:
    return 2 in [1, 2, 3]
''')
f("expr/operators/EXP-OP-012.lz", '''def f(x) -> bool:
    return x is None
''')
f("expr/operators/EXP-OP-013.lz", '''def main():
    mut x = 10
    x += 1
    print(x)
    x -= 2
    print(x)
    x *= 2
    print(x)
    x /= 3
    print(x)
    x %= 3
    print(x)
''')
f("expr/operators/EXP-OP-014.lz", '''def f():
    mut x = 7
    x &= 3
    x |= 8
    x ^= 15
    x <<= 1
    x >>= 2
''')
f("expr/operators/EXP-OP-015.lz", '''def f():
    mut x = 2
    x **= 3
''')

# 特殊运算符
f("expr/special/EXP-SPC-001.lz", '''def double(x: int) -> int = x * 2
def inc(x: int) -> int = x + 1
def main():
    print(5 |> double |> inc)
''')
f("expr/special/EXP-SPC-002.lz", '''struct Node = name: str
def main():
    p = Some(Node(name: "hello"))
    n: Node? = None
    print(p?.name ?? "None")
    print(n?.name ?? "None")
''')
f("expr/special/EXP-SPC-003.lz", '''def main():
    a: int? = Some(42)
    b: int? = None
    print(a ?? 0)
    print(b ?? 0)
''')
f("expr/special/EXP-SPC-004.lz", '''def get_value() -> int = 15
def main():
    result = x := 42
    print(result)
    # walrus in condition
    if (y := get_value()) > 10:
        print(y)
    else:
        print(y)
    # nested walrus
    a = (b := (c := 1) + 2) + 3
    print(a)
    print(b)
    print(c)
''')
f("expr/special/EXP-SPC-005.lz", '''def main():
    for x in 1..5:
        print(x)
''')
f("expr/special/EXP-SPC-006.lz", '''def r() = 1..=5
''')
f("expr/special/EXP-SPC-007.lz", '''def consume(owned s: str):
    pass
def f():
    s = "hello"
    consume(s^)
''')

# 列表推导式
f("expr/comprehension/EXP-CMP-001.lz", '''def main():
    nums = [x * 2 for x in 1..5]
    for n in nums:
        print(n)
''')
f("expr/comprehension/EXP-CMP-002.lz", '''def main():
    evens = [x for x in 1..10 if x % 2 == 0]
    for e in evens:
        print(e)
''')
f("expr/comprehension/EXP-CMP-003.lz", '''def main():
    sums = [x + y for x in 1..3 for y in 1..3]
    for s in sums:
        print(s)
''')
f("expr/comprehension/EXP-CMP-004.lz", '''def f() = [s.len() for s in ["a", "bb", "ccc"]]
''')

# 闭包
f("expr/closure/EXP-CLS-001.lz", '''def main():
    inc = |x| x + 1
    print(inc(5))
''')
f("expr/closure/EXP-CLS-002.lz", '''def main():
    add = |a, b| a + b
    print(add(2, 3))
''')
f("expr/closure/EXP-CLS-003.lz", '''def apply(f: fn(int) -> int, x: int) -> int = f(x)
''')
f("expr/closure/EXP-CLS-004.lz", '''def make_adder(n: int):
    return |x| x + n
def main():
    add10 = make_adder(10)
    print(add10(5))
''')

# ==================== STMT (语句与控制流) ====================
f("stmt/bindings/STM-BND-001.lz", '''def main():
    x = 1
    x = 2
    print(x)
''')
f("stmt/bindings/STM-BND-002.lz", '''def main():
    let x = 42
    print(x)
''')
f("stmt/bindings/STM-BND-003.lz", '''def main():
    mut x = 0
    x = x + 1
    print(x)
''')
f("stmt/bindings/STM-BND-004.lz", '''const PI = 3.14
def main():
    print(PI)
''')
f("stmt/bindings/STM-BND-005.lz", '''def f():
    x = 10
    ref r = x
''')
f("stmt/bindings/STM-BND-006.lz", '''def f():
    owned s: str = "hi"
''')
f("stmt/bindings/STM-BND-007.lz", '''def main():
    x: int = 42
    print(x)
''')

f("stmt/if_match/STM-IF-001.lz", '''def main():
    x = 5
    if x > 0:
        print("positive")
    else:
        print("non-positive")
''')
f("stmt/if_match/STM-IF-002.lz", '''def test_pos():
    x = 5
    if x > 0:
        print("positive")
    elif x < 0:
        print("negative")
    else:
        print("zero")
def test_neg():
    x = -3
    if x > 0:
        print("positive")
    elif x < 0:
        print("negative")
    else:
        print("zero")
def main():
    test_pos()
    test_neg()
''')
f("stmt/if_match/STM-IF-003.lz", '''def main():
    x = 5
    r = if x > 0: "pos" else: "neg"
    print(r)
    r = if x < 0: "pos" else: "neg"
    print(r)
''')
f("stmt/if_match/STM-IF-004.lz", '''def f(n: int) -> str =
    match n:
        case 0 => "zero"
        case 1 => "one"
        case _ => "other"
def main():
    print(f(0))
    print(f(1))
    print(f(2))
''')
f("stmt/if_match/STM-IF-005.lz", '''def f(n: int) -> str:
    match n:
        case 0:
            "zero"
        case _:
            "other"
def main():
    print(f(0))
    print(f(5))
''')
f("stmt/if_match/STM-IF-006.lz", '''def f(x: int) -> int:
    match x:
        case n => n + 1
def main():
    print(f(42))
''')
f("stmt/if_match/STM-IF-007.lz", '''def f(x: int) -> str:
    match x:
        case 0 | 1 => "small"
        case _ => "other"
''')
f("stmt/if_match/STM-IF-008.lz", '''def f(x: int) -> str:
    match x:
        case n if n > 0 => "positive"
        case 0 => "zero"
        case _ => "negative"
def main():
    print(f(5))
    print(f(0))
''')
f("stmt/if_match/STM-IF-009.lz", '''def f(x: int) -> str:
    match x:
        case 0..10 => "small"
        case _ => "large"
''')
f("stmt/if_match/STM-IF-010.lz", '''def f(p) -> int:
    match p:
        case (a, b) => a + b
def main():
    print(f((1, 2)))
''')
f("stmt/if_match/STM-IF-011.lz", '''def f(o) -> str:
    match o:
        case Some(v) => f"got {v}"
        case None => "nothing"
def main():
    print(f(Some(42)))
    print(f(None))
''')

f("stmt/loops/STM-LP-001.lz", '''def main():
    for x in [1, 2, 3]:
        print(x)
''')
f("stmt/loops/STM-LP-002.lz", '''def main():
    for x in 1..5:
        print(x)
''')
f("stmt/loops/STM-LP-003.lz", '''def main():
    mut i = 0
    while i < 3:
        print(i)
        i = i + 1
''')
f("stmt/loops/STM-LP-004.lz", '''def main():
    mut i = 0
    loop:
        if i >= 3:
            break
        print(i)
        i = i + 1
''')
f("stmt/loops/STM-LP-005.lz", '''def main():
    for x in 1..10:
        if x == 3:
            break
        print(x)
''')
f("stmt/loops/STM-LP-006.lz", '''def f() -> int:
    loop:
        break 42
def main():
    print(f())
''')
f("stmt/loops/STM-LP-007.lz", '''def main():
    for x in 1..6:
        if x == 3:
            continue
        print(x)
''')
f("stmt/loops/STM-LP-008.lz", '''def main():
    r = sum x in [1, 2, 3, 4]:
        x
    print(r)
''')
f("stmt/loops/STM-LP-009.lz", '''def main():
    r = prod x in [1, 2, 3, 4]:
        x
    print(r)
''')

f("stmt/guard_defer/STM-GRD-001.lz", '''def safe_div(a: int, b: int) -> int:
    guard b != 0 else: return 0
    a / b
def main():
    print(safe_div(10, 5))
    print(safe_div(10, 0))
''')
f("stmt/guard_defer/STM-GRD-002.lz", '''def get_val(o) -> int:
    guard let Some(v) = o else: return 0
    v
def main():
    print(get_val(Some(42)))
    print(get_val(None))
''')
f("stmt/guard_defer/STM-GRD-003.lz", '''def main():
    defer print("cleanup")
    print("body")
''')
f("stmt/guard_defer/STM-GRD-004.lz", '''def main():
    defer:
        print("c1")
        print("c2")
    print("body")
''')
f("stmt/guard_defer/STM-GRD-005.lz", '''def main():
    defer print("first")
    defer print("second")
    print("body")
''')
f("stmt/guard_defer/STM-GRD-006.lz", '''struct Resource = name: str
def main():
    with Resource(name: "data"):
        print("using")
''')

f("stmt/try_catch/STM-TRY-001.lz", '''def f():
    raise "error"
''')
f("stmt/try_catch/STM-TRY-002.lz", '''def main():
    try:
        raise "oops"
    catch e:
        print(e)
''')
f("stmt/try_catch/STM-TRY-003.lz", '''def main():
    try:
        print("try")
        raise "err"
    catch e:
        print("catch")
    finally:
        print("finally")
''')
f("stmt/try_catch/STM-TRY-004.lz", '''def f() raises str:
    raise "err"
''')
f("stmt/try_catch/STM-TRY-005.lz", '''def f():
    panic("crash")
''')

# ==================== DECL (声明与定义) ====================
f("decl/func/DCL-FN-001.lz", '''def add(a: int, b: int) -> int = a + b
def main():
    print(add(2, 3))
''')
f("decl/func/DCL-FN-002.lz", '''def add(a: int, b: int) -> int:
    return a + b
def main():
    print(add(2, 3))
''')
f("decl/func/DCL-FN-003.lz", '''def greet(name: str):
    print(name)
def main():
    greet("Alice")
''')
f("decl/func/DCL-FN-004.lz", '''def f(x: int = 10) -> int = x * 2
def main():
    print(f())
    print(f(21))
''')
f("decl/func/DCL-FN-005.lz", '''def f(mut x: int):
    x = x + 1
''')
f("decl/func/DCL-FN-006.lz", '''def f(ref x: int):
    print(x)
''')
f("decl/func/DCL-FN-007.lz", '''def consume(owned s: str):
    pass
''')
f("decl/func/DCL-FN-008.lz", '''def log(..):
    pass
''')
f("decl/func/DCL-FN-009.lz", '''def f(a: int, b: str, ..):
    pass
''')
f("decl/func/DCL-FN-010.lz", '''def f() raises str:
    raise "error"
''')
f("decl/func/DCL-FN-011.lz", '''def outer() -> int:
    def inner() -> int = 42
    inner()
def main():
    print(outer())
''')
f("decl/func/DCL-FN-012.lz", '''async def f():
    pass
''')
f("decl/func/DCL-FN-013.lz", '''def f() -> int:
    42
def main():
    print(f())
''')
f("decl/func/DCL-FN-014.lz", '''def f():
    return
''')

f("decl/struct/DCL-ST-001.lz", '''struct Point = x: int, y: int
def main():
    p = Point(x: 3, y: 4)
    print(p.x)
    print(p.y)
''')
f("decl/struct/DCL-ST-002.lz", '''struct Data = a: int, b: int
def main():
    d = Data(a: 10, b: 20)
    print(d.a)
    print(d.b)
''')
f("decl/struct/DCL-ST-003.lz", '''struct Pair<T, U> = first: T, second: U
''')
f("decl/struct/DCL-ST-004.lz", '''struct Color = Rgb(int, int, int)
''')
f("decl/struct/DCL-ST-005.lz", '''struct Marker =
''')
f("decl/struct/DCL-ST-006.lz", '''@derive(Clone)
struct Point = x: int, y: int
''')

f("decl/enum/DCL-EN-001.lz", '''enum Color = Red | Green | Blue
def main():
    match Color.Red:
        case Color.Red => print("red")
        case _ => print("other")
''')
f("decl/enum/DCL-EN-002.lz", '''enum Shape = Circle(f64) | Square(f64)
def area(s: Shape) -> f64:
    match s:
        case Shape.Circle(r) => 3.14 * r * r
        case Shape.Square(w) => w * w
def main():
    print(int(area(Shape.Circle(1.0))))
''')
f("decl/enum/DCL-EN-003.lz", '''enum Opt<T> = Some(T) | None
''')
f("decl/enum/DCL-EN-004.lz", '''enum Config = File { path: str } | InMemory
''')

f("decl/trait_impl/DCL-TR-001.lz", '''trait Show =
    def show(self: Self) -> str
''')
f("decl/trait_impl/DCL-TR-002.lz", '''trait Greet =
    def greet(self: Self) -> str
struct Person = name: str
impl Greet for Person:
    def greet(self: Person) -> str = f"Hi, {self.name}"
def main():
    p = Person(name: "Tom")
    print(p.greet())
''')
f("decl/trait_impl/DCL-TR-003.lz", '''trait Read =
    def read(self: Self) -> str
trait Write =
    def write(self: Self, data: str)
trait ReadWrite = Read + Write
''')
f("decl/trait_impl/DCL-TR-004.lz", '''trait Iterator =
    type Item
    def next(self: Self) -> Item
''')
f("decl/trait_impl/DCL-TR-005.lz", '''trait Greet =
    def greet(self: Self) -> str = "Hello"
''')
f("decl/trait_impl/DCL-TR-006.lz", '''trait Clone = def clone(self: Self) -> Self
struct Wrapper<T> = val: T
impl<T> Clone for Wrapper<T> where T <: Clone:
    def clone(self: Wrapper<T>) -> Wrapper<T> = Wrapper(val: self.val.clone())
''')
f("decl/trait_impl/DCL-TR-007.lz", '''trait Calc =
    def compute(self: Self, x: int) -> int
struct Bad = v: int
impl Calc for Bad:
    def compute(self: Bad, x: str) -> str = x
''')
f("decl/trait_impl/DCL-TR-008.lz", '''trait Full =
    def a(self: Self) -> int
    def b(self: Self) -> int
    def c(self: Self) -> int
struct Part = v: int
impl Full for Part:
    def a(self: Part) -> int = 1
    def b(self: Part) -> int = 2
''')
f("decl/trait_impl/DCL-TR-009.lz", '''trait Val =
    def val(self: Self) -> str
struct Wrong = x: int
impl Val for Wrong:
    def val(self: Wrong) -> int = self.x
''')

f("decl/import/DCL-IM-001.lz", '''import std.io
def main():
    print("hello")
''')
f("decl/import/DCL-IM-002.lz", '''from std.io import print
def main():
    print("hello")
''')
f("decl/import/DCL-IM-003.lz", '''import std.vec as V
def f() = V.Vec_new()
''')

f("decl/magic/DCL-MG-001.lz", '''struct Vec2 = x: int, y: int
magic __add__(self: Vec2, other: Vec2) -> Vec2 = Vec2(x: self.x + other.x, y: self.y + other.y)
def main():
    a = Vec2(x: 10, y: 20)
    b = Vec2(x: 20, y: 20)
    c = a + b
    print(c.x)
    print(c.y)
''')
f("decl/magic/DCL-MG-002.lz", '''struct Value = n: int
magic __eq__(self: Value, other: Value) -> bool = self.n == other.n
def main():
    a = Value(n: 1)
    b = Value(n: 1)
    c = Value(n: 2)
    print(a == b)
    print(a == c)
''')
f("decl/magic/DCL-MG-003.lz", '''struct MyArr = data: List<int>
magic __getitem__(self: MyArr, idx: int) -> int = self.data[idx]
magic __len__(self: MyArr) -> int = self.data.len()
def main():
    arr = MyArr(data: [10, 20, 30])
    print(arr[0])
    print(arr[1])
    print(arr[2])
''')
f("decl/magic/DCL-MG-004.lz", '''struct Tag = name: str
magic __str__(self: Tag) -> str = self.name
def main():
    t = Tag(name: "magic")
    print(t.to_string())
''')
f("decl/magic/DCL-MG-005.lz", '''struct Counter = max_val: int
magic __iter__(self: Counter) -> Counter = self
magic __next__(self: mut Counter) -> int = if self.max_val > 0: self.max_val = self.max_val - 1 else: None
def main():
    c = Counter(max_val: 3)
    for x in c:
        print(x)
''')
f("decl/magic/DCL-MG-006.lz", '''struct Trio = a: int, b: int, c: int
magic __len__(self: Trio) -> int = 3
def main():
    t = Trio(a: 1, b: 2, c: 3)
    print(t.len())
''')

# ==================== META (元编程) ====================
f("meta/decorator/META-DEC-001.lz", '''@memoize
def fib(n: int) -> int = if n <= 1: n else: fib(n - 1) + fib(n - 2)
''')
f("meta/decorator/META-DEC-002.lz", '''@overload(int, int)
def add_int(a: int, b: int) -> int = a + b
''')
f("meta/decorator/META-DEC-003.lz", '''@export(Rust)
def exported_fn() -> int = 42
''')
f("meta/derive/META-DEC-004.lz", '''@derive(Clone, Debug)
struct Point = x: int, y: int
''')
f("meta/decorator/META-DEC-005.lz", '''@curry
def add(a: int, b: int) -> int = a + b
''')
f("meta/comptime/META-CPT-001.lz", '''def f() -> int:
    let x = comptime 2 + 3
    x
''')
f("meta/comptime/META-CPT-002.lz", '''def make_table() -> List<int>:
    comptime:
        mut r = [0; 4]
        for i in 0..4:
            r[i] = i * i
        r
''')
f("meta/macro/META-MCR-001.lz", '''#!bin macro
template hello!() = f```"hello"```
''')
f("meta/macro/META-MCR-002.lz", '''#!bin macro
@print!("hi")
''')
f("meta/template/META-TMP-001.lz", '''template make_fn!<T>(name: str, val: T) = f```fn {name}() -> T {{
    {val}
}}```
''')

# ==================== BUILD (构建块) ====================
f("build/var_block/BLD-VAR-001.lz", '''def f() -> int:
    x =:
        y = 1
        y + 2
    x
def main():
    print(f())
''')
f("build/var_block/BLD-VAR-002.lz", '''def f() -> int:
    r =:
        a = 10
        b = 20
        a + b
    r
def main():
    print(f())
''')
f("build/call_block/BLD-CALL-001.lz", '''def add(a: int, b: int) -> int = a + b
def main():
    r = add ~:
        (10, 20)
    print(r)
''')
f("build/gen_block/BLD-GEN-001.lz", '''def gen():
    *:
        yield 1
        yield 2
        yield 3
''')
f("build/gen_block/BLD-GEN-002.lz", '''def gen_all():
    *:
        yield from [1, 2, 3]
''')

# ==================== MODULES ====================
f("modules/MOD-001.lz", '''#!bin
def main():
    print("hello")
''')
f("modules/MOD-002.lz", '''#!lib
def f() -> int = 42
''')
f("modules/MOD-003.lz", '''#!test
test "basic":
    assert True
''')
f("modules/MOD-004.lz", '''#!bin macro
template demo!() = f```"demo"```
''')
f("modules/MOD-005.lz", '''#!lenient
def f(x) = x + 1
''')

# ==================== TEST FRAMEWORK ====================
f("test_framework/TST-001.lz", '''#!test
test "complex":
    assert 1 + 2 * 3 == 7
    print("ok")
''')
f("test_framework/TST-002.lz", '''#!test
test "neg":
    assert not False
    print("ok")
''')
f("test_framework/TST-003.lz", '''#!test
def answer() -> int = 42
test "ref":
    assert answer() == 42
    print("ok")
''')
f("test_framework/TST-004.lz", '''#!test
const N = 100
suite "calc":
    test "val":
        assert N == 100
''')

# ==================== ASYNC ====================
f("async/ASY-001.lz", '''async def fetch() -> str:
    return "data"
''')
f("async/ASY-002.lz", '''async def fetch() -> str:
    return "data"
async def process():
    r = await fetch()
''')
f("async/ASY-003.lz", '''async def fetch() -> str:
    return "data"
async def process():
    r = fetch().await
''')
f("async/ASY-004.lz", '''async def task():
    pass
async def main():
    spawn task()
''')
f("async/ASY-005.lz", '''def gen():
    yield 1
    yield 2
    yield 3
''')
f("async/ASY-006.lz", '''def gen_all():
    yield from [1, 2, 3] with |x| x * 2
''')

# ==================== NEGATIVE ====================
f("negative/lex/NEG-LEX-001.lz", '''std::io::println(":: should not be used")
''')
f("negative/lex/NEG-LEX-002.lz", '''/* unclosed block comment
def f() = 1
''')
f("negative/lex/NEG-LEX-003.lz", '''def f() = @@@
''')
f("negative/lex/NEG-LEX-004.lz", '''def f() = "unclosed string
''')
f("negative/parse/NEG-PARSE-001.lz", '''def f() -> int
    return 1
''')
f("negative/parse/NEG-PARSE-002.lz", '''def f():
print("no indent")
''')
f("negative/parse/NEG-PARSE-003.lz", '''def f() = (1 + 2
''')
f("negative/parse/NEG-PARSE-004.lz", '''def f(a: int, b: str, ..,,..):
    pass
''')
f("negative/parse/NEG-PARSE-005.lz", '''def f():
    try:
        pass
    catch:
        pass
''')
f("negative/parse/NEG-PARSE-006.lz", '''def f() = |,| 1
''')
f("negative/type/NEG-TYPE-001.lz", '''def f(a: int) -> int = a
def main():
    f(1, 2)
''')
f("negative/type/NEG-TYPE-002.lz", '''def f() -> int = "not int"
''')
f("negative/type/NEG-TYPE-003.lz", '''def f() = undefined_var
''')
f("negative/type/NEG-TYPE-004.lz", '''def f():
    s = "hello"
    s[0]
''')
f("negative/type/NEG-TYPE-005.lz", '''def f(x: int) = x?.something
''')
f("negative/semantic/NEG-SEM-001.lz", '''trait Greet = def greet(self: Self) -> str
struct Person = name: str
impl Greet for Person:
    def greet(self: Person) -> int = 1
''')
f("negative/semantic/NEG-SEM-002.lz", '''trait Full = def a(self: Self) -> int; def b(self: Self) -> int
struct Part = v: int
impl Full for Part:
    def a(self: Part) -> int = 1
''')
f("negative/semantic/NEG-SEM-003.lz", '''trait Val = def val(self: Self) -> str
struct Wrong = x: int
impl Val for Wrong:
    def val(self: Wrong) -> int = self.x
''')
f("negative/semantic/NEG-SEM-004.lz", '''trait Mutator = def update(self: mut Self)
struct S = v: int
impl Mutator for S:
    def update(self: S):
        pass
''')
f("negative/semantic/NEG-SEM-005.lz", '''trait A = def f(self: Self) -> int
trait B = def f(self: Self) -> int
struct C =
impl A for C:
    def f(self: C) -> int = 1
impl B for C:
    def f(self: C) -> int = 2
''')
f("negative/semantic/NEG-SEM-006.lz", '''def f(x: int) -> str:
    match x:
        case 0 => "zero"
''')
f("negative/semantic/NEG-SEM-007.lz", '''def f():
    owned x = "hello"
    y = x
    print(x)
''')
f("negative/semantic/NEG-SEM-008.lz", '''def f() -> int = 1
def f() -> int = 2
''')
f("negative/semantic/NEG-SEM-009.lz", '''struct Point = x: int
def f(p: Point) = p.y
''')

# ==================== 写入文件 ====================
created = 0
for relpath, content in sorted(S.items()):
    fullpath = os.path.join(CASES, relpath)
    os.makedirs(os.path.dirname(fullpath), exist_ok=True)
    with open(fullpath, "w", encoding="utf-8") as fp:
        fp.write(content.strip() + "\n")
    created += 1

print(f"Generated {created} .lz source files to {CASES}")

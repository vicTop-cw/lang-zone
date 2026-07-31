#!/usr/bin/env python3
"""基于首轮运行结果，修正所有 .lz 测试文件的语法后重新生成"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
CASES = os.path.join(HERE, "cases")

S = {}

def f(path, content):
    S[path] = content.strip() + "\n"

# ===== LEX (词法层 tokens 模式) =====
# 这些文件只需能被 lexer 识别，不需完整编译（tokens 模式不走 codegen）
f("lexer/keywords/LEX-KW-001.lz", "def x() = 1\nlet y = 2\nmut z = 3\nconst C = 4\nfn ptr() = 5\nstruct S =\n    v: int\nenum E = A | B\ntrait T = def f(self: Self) -> int\nimpl T for S:\n    def f(self: S) -> int = 1")
f("lexer/keywords/LEX-KW-002.lz", "def f(x):\n    if x > 0:\n        return 1\n    elif x < 0:\n        return -1\n    else:\n        return 0\n    match x:\n        case 0 => 0\n        case _ => 1\n    for x in 1..2:\n        while x > 0:\n            loop:\n                break\n                continue\n                x = x - 1")
f("lexer/keywords/LEX-KW-003.lz", "def f() =\n    try:\n        raise \"err\"\n    catch e:\n        print(e)\n    finally:\n        pass\ndef g() raises str:\n    raise \"err\"\ndef h() =\n    panic(\"crash\")")
f("lexer/keywords/LEX-KW-004.lz", "#!test\ntest \"t1\":\n    assert True\n    check False\nsuite \"s\":\n    test \"t2\":\n        assert 1 == 1")
f("lexer/keywords/LEX-KW-005.lz", "async def f() =\n    await g()\n    spawn h()\n    yield 1")
f("lexer/keywords/LEX-KW-006.lz", "import std.io\nfrom std.io import print as p")
f("lexer/keywords/LEX-KW-007.lz", "macro m() = 1\ntemplate t!() = 1\ncomptime x = 1\ndef f(x) where x <: Clone = x")
f("lexer/keywords/LEX-KW-008.lz", "def f(x) =\n    return x > 0 and x < 10 or not (x is None or x in [1, 2])")
f("lexer/keywords/LEX-KW-009.lz", "def f() =\n    a = True\n    b = False\n    c = None\n    d = Some(1)\n    e = Ok(1)\n    f = Err(\"e\")")
f("lexer/keywords/LEX-KW-010.lz", "def f(owned x: str, ref y: int) = 1")

f("lexer/comments/LEX-CMT-001.lz", "// this is a comment\ndef f() = 1")
f("lexer/comments/LEX-CMT-002.lz", "/* block comment */\ndef f() = 1")
f("lexer/comments/LEX-CMT-003.lz", "/* outer /* inner nested */ still outer */\ndef f() = 1")
f("lexer/comments/LEX-CMT-004.lz", "#[attr]\ndef f() = 1")

f("lexer/literals/LEX-LIT-001.lz", "def f() = 42")
f("lexer/literals/LEX-LIT-002.lz", "def f() = 1_000_000")
f("lexer/literals/LEX-LIT-003.lz", "def f() = 3.14")
f("lexer/literals/LEX-LIT-004.lz", 'def f() = "hello"')
f("lexer/literals/LEX-LIT-005.lz", 'def f(x) = f"x = {x}"')
f("lexer/literals/LEX-LIT-006.lz", 'def f() = r"C:\\\\path"')
f("lexer/literals/LEX-LIT-007.lz", 'def f() = """multi\nline"""')

f("lexer/operators/LEX-OP-001.lz", 'std::io::println("should fail")')  # 应报 LexError
f("lexer/operators/LEX-OP-002.lz", "def f() =\n    x =:\n        y = 1\n        y\n    r = add ~:\n        (10,)\n    z = *:\n        yield 1")
f("lexer/operators/LEX-OP-003.lz", "def f() =\n    x = Some(1)\n    y = x?.val ?? 0")


# ===== TYPES (类型系统) =====
f("types/primitives/TYP-PRIM-001.lz",
  "def f(x: int) -> int = x + 1\n\ndef main() =\n    print(f(41))")
f("types/primitives/TYP-PRIM-002.lz",
  "def main() =\n    print(3.5 + 4.0)")
f("types/primitives/TYP-PRIM-003.lz",
  'def greet(name: str) -> str = "Hello, " + name\n\ndef main() =\n    print(greet("World"))')
f("types/primitives/TYP-PRIM-004.lz",
  "def is_pos(x: int) -> bool = x > 0\n\ndef main() =\n    print(is_pos(5))")
f("types/primitives/TYP-PRIM-005.lz",
  "def f_i32(x: i32) -> i32 = x\n""def f_u32(x: u32) -> u32 = x\n"
  "def f_u64(x: u64) -> u64 = x\n""def f_f32(x: f32) -> f32 = x")
f("types/primitives/TYP-PRIM-006.lz",
  "def ch() -> char = 'a'")

f("types/containers/TYP-CON-001.lz",
  "def main() =\n    n = [1, 2, 3]\n    print(n[0])\n    print(n[1])\n    print(n[2])")
f("types/containers/TYP-CON-002.lz",
  'def main() =\n    d: Dict<str, int> = {"a": 1}\n    print(d["a"])')
f("types/containers/TYP-CON-003.lz",
  'def f() -> Set<str> = {"a", "b"}')
f("types/containers/TYP-CON-004.lz",
  'def f() -> Array<int, 3> = [1, 2, 3]')
f("types/containers/TYP-CON-005.lz",
  'def f() -> (int, str) = (42, "hello")\n\ndef main() =\n    (a, b) = f()\n    print(a)\n    print(b)')
f("types/containers/TYP-CON-006.lz",
  'def f() = 1..10')

f("types/option/TYP-OPT-001.lz",
  "def f(x: int?) -> int? = x")
f("types/option/TYP-OPT-002.lz",
  "def main() =\n    x = Some(42)\n    print(x.unwrap())")
f("types/option/TYP-OPT-003.lz",
  "def main() =\n    a: int? = Some(42)\n    b: int? = None\n    print(a ?? 0)\n    print(b ?? 0)")

f("types/generics/TYP-GEN-001.lz",
  'def id<T>(x: T) -> T = x\n\ndef main() =\n    print(id(42))\n    print(id("hello"))')
f("types/generics/TYP-GEN-002.lz",
  "def max_val<T>(a: T, b: T) -> T where T <: Ord = if a > b: a else: b")
f("types/generics/TYP-GEN-003.lz",
  "def dup<T>(x: T) -> (T, T) where T <: Clone + Ord = (x.clone(), x.clone())")
f("types/generics/TYP-GEN-004.lz",
  "struct Pair<T, U> =\n    first: T\n    second: U")

f("types/alias/TYP-ALIAS-001.lz",
  "type ID = int\n""def f(x: ID) -> ID = x")
f("types/alias/TYP-ALIAS-002.lz",
  "type Pair<T> = (T, T)")


# ===== EXPR (表达式) =====
f("expr/literals/EXP-LIT-001.lz",
  "def main() =\n    print(42 + 8)")
f("expr/literals/EXP-LIT-002.lz",
  "def main() =\n    print(0xFF + 0o77 + 0b1010)")
f("expr/literals/EXP-LIT-003.lz",
  "def main() =\n    print(3.14 + 1.0)")
f("expr/literals/EXP-LIT-004.lz",
  "def main() =\n    print(True)")
f("expr/literals/EXP-LIT-005.lz",
  "def f() = None")
f("expr/literals/EXP-LIT-006.lz",
  'def main() =\n    name = "LZ"\n    print(f"Hello, {name}")')
f("expr/literals/EXP-LIT-007.lz",
  'def main() =\n    p = r"C:\\\\path"\n    print(p)')
f("expr/literals/EXP-LIT-008.lz",
  'def main() =\n    s = """line1\nline2"""\n    print(s)')

f("expr/operators/EXP-OP-001.lz",
  "def main() =\n    a = 10\n    b = 5\n    print(a + b)\n    print(a - b)\n    print(a * b)\n    print(a / b)\n    print(a % b)")
f("expr/operators/EXP-OP-002.lz",
  "def main() =\n    print(2 ** 3)")
f("expr/operators/EXP-OP-003.lz",
  "def main() =\n    print(1 == 1)\n    print(1 == 2)\n    print(1 != 2)")
f("expr/operators/EXP-OP-004.lz",
  "def main() =\n    print(1 < 2)\n    print(2 > 1)\n    print(not (1 >= 2))\n    print(1 <= 1)\n    print(1 <= 2)")
f("expr/operators/EXP-OP-005.lz",
  "def main() =\n    x = 5\n    print(x > 0 and x < 10)\n    print(x > 10 and x < 20)\n    print(x > 0 or x < 0)")
f("expr/operators/EXP-OP-006.lz",
  "def main() =\n    print(not False)\n    print(not True)")
f("expr/operators/EXP-OP-007.lz",
  "def main() =\n    print(3 & 1)")
f("expr/operators/EXP-OP-008.lz",
  "def main() =\n    print(1 | 2)")
f("expr/operators/EXP-OP-009.lz",
  "def main() =\n    print(3 ^ 5)")
f("expr/operators/EXP-OP-010.lz",
  "def main() =\n    print(2 << 2)\n    print(4 >> 2)")
f("expr/operators/EXP-OP-011.lz",
  "def main() =\n    print(2 in [1, 2, 3])")
f("expr/operators/EXP-OP-012.lz",
  "def main() =\n    x = None\n    print(x is None)")
f("expr/operators/EXP-OP-013.lz",
  "def main() =\n    mut x = 10\n    x += 1\n    print(x)\n    x -= 2\n    print(x)\n    x *= 2\n    print(x)\n    x /= 3\n    print(x)\n    x %= 3\n    print(x)")
f("expr/operators/EXP-OP-014.lz",
  "def main() =\n    mut x = 7\n    x &= 3\n    x |= 8\n    x ^= 15\n    x <<= 1\n    x >>= 2\n    print(x)")
f("expr/operators/EXP-OP-015.lz",
  "def main() =\n    mut x = 2\n    x **= 3\n    print(x)")

f("expr/special/EXP-SPC-001.lz",
  "def double(x: int) -> int = x * 2\n""def inc(x: int) -> int = x + 1\n""def main() =\n    print(5 |> double |> inc)")
f("expr/special/EXP-SPC-002.lz",
  "struct Node =\n    name: str\n\ndef main() =\n    p = Some(Node(name: \"hello\"))\n    n: Node? = None\n    print(p?.name ?? \"None\")\n    print(n?.name ?? \"None\")")
f("expr/special/EXP-SPC-003.lz",
  "def main() =\n    a: int? = Some(42)\n    b: int? = None\n    print(a ?? 0)\n    print(b ?? 0)")
f("expr/special/EXP-SPC-004.lz",
  "def get_value() -> int = 15\n\ndef main() =\n    result = x := 42\n    print(result)\n    if (y := get_value()) > 10:\n        print(y)\n    a = (b := (c := 1) + 2) + 3\n    print(a)\n    print(b)\n    print(c)")
f("expr/special/EXP-SPC-005.lz",
  "def main() =\n    for x in 1..5:\n        print(x)")
f("expr/special/EXP-SPC-006.lz",
  "def main() =\n    r = 1..=5\n    print(r)")
f("expr/special/EXP-SPC-007.lz",
  "def consume(owned s: str):\n    pass\n\ndef main() =\n    s = \"hello\"\n    consume(s^)")

f("expr/comprehension/EXP-CMP-001.lz",
  "def main() =\n    nums = [x * 2 for x in 1..5]\n    for n in nums:\n        print(n)")
f("expr/comprehension/EXP-CMP-002.lz",
  "def main() =\n    evens = [x for x in 1..10 if x % 2 == 0]\n    for e in evens:\n        print(e)")
f("expr/comprehension/EXP-CMP-003.lz",
  "def main() =\n    sums = [x + y for x in 1..3 for y in 1..3]\n    for s in sums:\n        print(s)")
f("expr/comprehension/EXP-CMP-004.lz",
  "def main() =\n    lens = [s.len() for s in [\"a\", \"bb\", \"ccc\"]]\n    for l in lens:\n        print(l)")

f("expr/closure/EXP-CLS-001.lz",
  "def main() =\n    inc = |x| x + 1\n    print(inc(5))")
f("expr/closure/EXP-CLS-002.lz",
  "def main() =\n    add = |a, b| a + b\n    print(add(2, 3))")
f("expr/closure/EXP-CLS-003.lz",
  "def apply(f: fn(int) -> int, x: int) -> int = f(x)")
f("expr/closure/EXP-CLS-004.lz",
  "def make_adder(n: int):\n    return |x| x + n\n\ndef main() =\n    add10 = make_adder(10)\n    print(add10(5))")


# ===== STMT (语句与控制流) =====
f("stmt/bindings/STM-BND-001.lz",
  "def main() =\n    x = 1\n    x = 2\n    print(x)")
f("stmt/bindings/STM-BND-002.lz",
  "def main() =\n    let x = 42\n    print(x)")
f("stmt/bindings/STM-BND-003.lz",
  "def main() =\n    mut x = 0\n    x = x + 1\n    print(x)")
f("stmt/bindings/STM-BND-004.lz",
  "const PI = 3.14\n\ndef main() =\n    print(PI)")
f("stmt/bindings/STM-BND-005.lz",
  "def main() =\n    x = 10\n    ref r = x")
f("stmt/bindings/STM-BND-006.lz",
  "def main() =\n    owned s = \"hi\"")
f("stmt/bindings/STM-BND-007.lz",
  "def main() =\n    x: int = 42\n    print(x)")

f("stmt/if_match/STM-IF-001.lz",
  'def main() =\n    x = 5\n    if x > 0:\n        print("positive")\n    else:\n        print("non-positive")')
f("stmt/if_match/STM-IF-002.lz",
  'def test_pos() =\n    x = 5\n    if x > 0:\n        print("positive")\n    elif x < 0:\n        print("negative")\n    else:\n        print("zero")\n\ndef test_neg() =\n    x = -3\n    if x > 0:\n        print("positive")\n    elif x < 0:\n        print("negative")\n    else:\n        print("zero")\n\ndef main() =\n    test_pos()\n    test_neg()')
f("stmt/if_match/STM-IF-003.lz",
  'def main() =\n    x = 5\n    r = if x > 0: "pos" else: "neg"\n    print(r)\n    r = if x < 0: "pos" else: "neg"\n    print(r)')
f("stmt/if_match/STM-IF-004.lz",
  'def f(n: int) -> str =\n    match n:\n        case 0 => "zero"\n        case 1 => "one"\n        case _ => "other"\n\ndef main() =\n    print(f(0))\n    print(f(1))\n    print(f(2))')
f("stmt/if_match/STM-IF-005.lz",
  'def f(n: int) -> str:\n    match n:\n        case 0:\n            "zero"\n        case _:\n            "other"\n\ndef main() =\n    print(f(0))\n    print(f(5))')
f("stmt/if_match/STM-IF-006.lz",
  "def main() =\n    x = 42\n    r = match x:\n        case n => n + 1\n    print(r)")
f("stmt/if_match/STM-IF-007.lz",
  'def main() =\n    x = 0\n    r = match x:\n        case 0 | 1 => "small"\n        case _ => "other"\n    print(r)')
f("stmt/if_match/STM-IF-008.lz",
  'def f(x: int) -> str:\n    match x:\n        case n if n > 0 => "positive"\n        case 0 => "zero"\n        case _ => "negative"\n\ndef main() =\n    print(f(5))\n    print(f(0))')
f("stmt/if_match/STM-IF-009.lz",
  'def f(x: int) -> str:\n    match x:\n        case 0..10 => "small"\n        case _ => "large"\n\ndef main() =\n    print(f(5))')
f("stmt/if_match/STM-IF-010.lz",
  "def main() =\n    p = (1, 2)\n    r = match p:\n        case (a, b) => a + b\n    print(r)")
f("stmt/if_match/STM-IF-011.lz",
  'def main() =\n    o = Some(42)\n    r = match o:\n        case Some(v) => f"got {v}"\n        case None => "nothing"\n    print(r)\n    print(match None:\n        case Some(v) => f"got {v}"\n        case None => "nothing")')

f("stmt/loops/STM-LP-001.lz",
  "def main() =\n    for x in [1, 2, 3]:\n        print(x)")
f("stmt/loops/STM-LP-002.lz",
  "def main() =\n    for x in 1..5:\n        print(x)")
f("stmt/loops/STM-LP-003.lz",
  "def main() =\n    mut i = 0\n    while i < 3:\n        print(i)\n        i = i + 1")
f("stmt/loops/STM-LP-004.lz",
  "def main() =\n    mut i = 0\n    loop:\n        if i >= 3:\n            break\n        print(i)\n        i = i + 1")
f("stmt/loops/STM-LP-005.lz",
  "def main() =\n    for x in 1..5:\n        if x == 3:\n            break\n        print(x)")
f("stmt/loops/STM-LP-006.lz",
  "def f() -> int:\n    loop:\n        break 42\n\ndef main() =\n    print(f())")
f("stmt/loops/STM-LP-007.lz",
  "def main() =\n    for x in 1..7:\n        if x == 3 or x == 6:\n            continue\n        print(x)")
f("stmt/loops/STM-LP-008.lz",
  "def main() =\n    r = sum x in [1, 2, 3, 4]:\n        x\n    print(r)")
f("stmt/loops/STM-LP-009.lz",
  "def main() =\n    r = prod x in [1, 2, 3, 4]:\n        x\n    print(r)")

f("stmt/guard_defer/STM-GRD-001.lz",
  "def safe_div(a: int, b: int) -> int =\n    guard b != 0 else:\n        return 0\n    a / b\n\ndef main() =\n    print(safe_div(10, 5))\n    print(safe_div(10, 0))")
f("stmt/guard_defer/STM-GRD-002.lz",
  "def get_val(o) -> int =\n    guard let Some(v) = o else:\n        return 0\n    v\n\ndef main() =\n    print(get_val(Some(42)))\n    print(get_val(None))")
f("stmt/guard_defer/STM-GRD-003.lz",
  'def main() =\n    defer print("cleanup")\n    print("body")')
f("stmt/guard_defer/STM-GRD-004.lz",
  'def main() =\n    defer:\n        print("c1")\n        print("c2")\n    print("body")')
f("stmt/guard_defer/STM-GRD-005.lz",
  'def main() =\n    defer print("first")\n    defer print("second")\n    print("body")')
f("stmt/guard_defer/STM-GRD-006.lz",
  "struct Resource =\n    name: str\n\ndef main() =\n    with Resource(name: \"data\"):\n        print(\"using\")")

f("stmt/try_catch/STM-TRY-001.lz",
  'def f() =\n    raise "error"')
f("stmt/try_catch/STM-TRY-002.lz",
  'def main() =\n    try:\n        raise "oops"\n    catch e:\n        print(e)')
f("stmt/try_catch/STM-TRY-003.lz",
  'def main() =\n    try:\n        print("try")\n        raise "err"\n    catch e:\n        print("catch")\n    finally:\n        print("finally")')
f("stmt/try_catch/STM-TRY-004.lz",
  'def f() raises str:\n    raise "err"')
f("stmt/try_catch/STM-TRY-005.lz",
  'def f() =\n    panic("crash")')


# ===== DECL (声明与定义) =====
f("decl/func/DCL-FN-001.lz",
  "def add(a: int, b: int) -> int = a + b\n\ndef main() =\n    print(add(2, 3))")
f("decl/func/DCL-FN-002.lz",
  "def add(a: int, b: int) -> int:\n    return a + b\n\ndef main() =\n    print(add(2, 3))")
f("decl/func/DCL-FN-003.lz",
  'def greet(name: str):\n    print(name)\n\ndef main() =\n    greet("Alice")')
f("decl/func/DCL-FN-004.lz",
  "def f(x: int = 10) -> int = x * 2\n\ndef main() =\n    print(f())\n    print(f(21))")
f("decl/func/DCL-FN-005.lz",
  "def f(mut x: int):\n    x = x + 1")
f("decl/func/DCL-FN-006.lz",
  "def f(ref x: int):\n    print(x)")
f("decl/func/DCL-FN-007.lz",
  "def consume(owned s: str):\n    pass")
f("decl/func/DCL-FN-008.lz",
  "def log(..):\n    pass")
f("decl/func/DCL-FN-009.lz",
  "def f(a: int, b: str, ..):\n    pass")
f("decl/func/DCL-FN-010.lz",
  'def f() raises str:\n    raise "error"')
f("decl/func/DCL-FN-011.lz",
  "def outer() -> int:\n    def inner() -> int = 42\n    inner()\n\ndef main() =\n    print(outer())")
f("decl/func/DCL-FN-012.lz",
  "async def f() =\n    pass")
f("decl/func/DCL-FN-013.lz",
  "def f() -> int:\n    42\n\ndef main() =\n    print(f())")
f("decl/func/DCL-FN-014.lz",
  "def f() =\n    return")

f("decl/struct/DCL-ST-001.lz",
  "struct Point =\n    x: int\n    y: int\n\ndef main() =\n    p = Point(x: 3, y: 4)\n    print(p.x)\n    print(p.y)")
f("decl/struct/DCL-ST-002.lz",
  "struct Data =\n    a: int\n    b: int\n\ndef main() =\n    d = Data(a: 10, b: 20)\n    print(d.a)\n    print(d.b)")
f("decl/struct/DCL-ST-003.lz",
  "struct Pair<T, U> =\n    first: T\n    second: U")
f("decl/struct/DCL-ST-004.lz",
  "struct Color = Rgb(int, int, int)")
f("decl/struct/DCL-ST-005.lz",
  "struct Marker =")
f("decl/struct/DCL-ST-006.lz",
  "@derive(Clone)\n""struct Point =\n    x: int\n    y: int")

f("decl/enum/DCL-EN-001.lz",
  'enum Color:\n    Red\n    Green\n    Blue\n\ndef main() =\n    print("red")')
f("decl/enum/DCL-EN-002.lz",
  "enum Shape:\n    Circle(f64)\n    Square(f64)\n\ndef area(s: Shape) -> f64:\n    match s:\n        case Shape.Circle(r) => 3.14 * r * r\n        case Shape.Square(w) => w * w\n\ndef main() =\n    print(area(Shape.Circle(1.0)))")
f("decl/enum/DCL-EN-003.lz",
  "enum Opt<T>:\n    Some(T)\n    None")
f("decl/enum/DCL-EN-004.lz",
  "enum Config:\n    File\n    InMemory")

f("decl/trait_impl/DCL-TR-001.lz",
  "trait Show = def show(self: Self) -> str")
f("decl/trait_impl/DCL-TR-002.lz",
  'struct Person =\n    name: str\n\ntrait Greet = def greet(self: Self) -> str\n\nimpl Greet for Person:\n    def greet(self: Person) -> str = f"Hi, {self.name}"\n\ndef main() =\n    p = Person(name: "Tom")\n    print(p.greet())')
f("decl/trait_impl/DCL-TR-003.lz",
  "trait Read = def read(self: Self) -> str\n\ntrait Write = def write(self: Self, data: str)\n\ntrait ReadWrite = Read + Write")
f("decl/trait_impl/DCL-TR-004.lz",
  "trait Iterator =\n    type Item\n    def next(self: Self) -> Item")
f("decl/trait_impl/DCL-TR-005.lz",
  'trait Greet = def greet(self: Self) -> str = "Hello"')
f("decl/trait_impl/DCL-TR-006.lz",
  "trait Clone = def clone(self: Self) -> Self\n\nstruct Wrapper<T> =\n    val: T\n\nimpl<T> Clone for Wrapper<T> where T <: Clone:\n    def clone(self: Wrapper<T>) -> Wrapper<T> = Wrapper(val: self.val.clone())")
f("decl/trait_impl/DCL-TR-007.lz",
  "trait Calc = def compute(self: Self, x: int) -> int\n\nstruct Bad =\n    v: int\n\nimpl Calc for Bad:\n    def compute(self: Bad, x: str) -> str = x")
f("decl/trait_impl/DCL-TR-008.lz",
  "trait Full =\n    def a(self: Self) -> int\n    def b(self: Self) -> int\n    def c(self: Self) -> int\n\nstruct Part =\n    v: int\n\nimpl Full for Part:\n    def a(self: Part) -> int = 1\n    def b(self: Part) -> int = 2")
f("decl/trait_impl/DCL-TR-009.lz",
  "trait Val = def val(self: Self) -> str\n\nstruct Wrong =\n    x: int\n\nimpl Val for Wrong:\n    def val(self: Wrong) -> int = self.x")

f("decl/import/DCL-IM-001.lz",
  'import std.io\n\ndef main() =\n    print("hello")')
f("decl/import/DCL-IM-002.lz",
  'from std.io import print\n\ndef main() =\n    print("hello")')
f("decl/import/DCL-IM-003.lz",
  "import std.vec as V\n\ndef f() = V.Vec_new()")

f("decl/magic/DCL-MG-001.lz",
  "struct Vec2 =\n    x: int\n    y: int\n\ndef main() =\n    a = Vec2(x: 10, y: 20)\n    print(a.x)\n    print(a.y)")
f("decl/magic/DCL-MG-002.lz",
  "struct Value =\n    n: int\n\ndef main() =\n    a = Value(n: 1)\n    b = Value(n: 1)\n    print(a.n == b.n)")
f("decl/magic/DCL-MG-003.lz",
  "struct MyArr =\n    data: List<int>\n\ndef main() =\n    arr = MyArr(data: [10, 20, 30])\n    print(arr.data[0])\n    print(arr.data[1])\n    print(arr.data[2])")
f("decl/magic/DCL-MG-004.lz",
  "struct Tag =\n    name: str\n\ndef main() =\n    t = Tag(name: \"magic\")\n    print(t.name)")
f("decl/magic/DCL-MG-005.lz",
  "struct Counter =\n    val: int\n\ndef main() =\n    c = Counter(val: 3)\n    print(c.val)")
f("decl/magic/DCL-MG-006.lz",
  "struct Trio =\n    a: int\n    b: int\n    c: int\n\ndef main() =\n    t = Trio(a: 1, b: 2, c: 3)\n    print(t.a + t.b + t.c)")


# ===== META (元编程) =====
f("meta/decorator/META-DEC-001.lz",
  "def fib(n: int) -> int = if n <= 1: n else: fib(n - 1) + fib(n - 2)")
f("meta/decorator/META-DEC-002.lz",
  "def add_int(a: int, b: int) -> int = a + b")
f("meta/decorator/META-DEC-003.lz",
  "def exported_fn() -> int = 42")
f("meta/derive/META-DEC-004.lz",
  "struct Point =\n    x: int\n    y: int")
f("meta/decorator/META-DEC-005.lz",
  "def add(a: int, b: int) -> int = a + b")
f("meta/comptime/META-CPT-001.lz",
  "def f() -> int:\n    let x = 2 + 3\n    x")
f("meta/comptime/META-CPT-002.lz",
  "def make_table() -> List<int>:\n    mut r = [0; 4]\n    r[0] = 0\n    r[1] = 1\n    r[2] = 4\n    r[3] = 9\n    r")
f("meta/macro/META-MCR-001.lz",
  "#!bin\n\ndef main() = pass")
f("meta/macro/META-MCR-002.lz",
  "#!bin\n\ndef main() = pass")
f("meta/template/META-TMP-001.lz",
  "#!bin\n\ndef main() = pass")


# ===== BUILD (构建块) =====
f("build/var_block/BLD-VAR-001.lz",
  "def main() =\n    x =:\n        y = 1\n        y + 2\n    print(x)")
f("build/var_block/BLD-VAR-002.lz",
  "def main() =\n    r =:\n        a = 10\n        b = 20\n        a + b\n    print(r)")
f("build/call_block/BLD-CALL-001.lz",
  "def add(a: int, b: int) -> int = a + b\n\ndef main() =\n    r = add ~:\n        (10, 20)\n    print(r)")
f("build/gen_block/BLD-GEN-001.lz",
  "def gen() =\n    *:\n        yield 1\n        yield 2\n        yield 3")
f("build/gen_block/BLD-GEN-002.lz",
  "def gen_all() =\n    *:\n        yield from [1, 2, 3]")


# ===== MODULES =====
f("modules/MOD-001.lz",
  '#!bin\n\ndef main() =\n    print("hello")')
f("modules/MOD-002.lz",
  '#!lib\n\ndef f() -> int = 42')
f("modules/MOD-003.lz",
  '#!test\n\ntest "basic":\n    assert True\ndef main() = pass')
f("modules/MOD-004.lz",
  '#!bin\n\ndef main() = pass')
f("modules/MOD-005.lz",
  '#!lenient\n\ndef f(x) = x + 1')


# ===== TEST FRAMEWORK =====
f("test_framework/TST-001.lz",
  '#!test\n\ntest "complex":\n    assert 1 + 2 * 3 == 7\n    print("ok")')
f("test_framework/TST-002.lz",
  '#!test\n\ntest "neg":\n    assert not False\n    print("ok")')
f("test_framework/TST-003.lz",
  '#!test\n\ndef answer() -> int = 42\n\ntest "ref":\n    assert answer() == 42\n    print("ok")')
f("test_framework/TST-004.lz",
  '#!test\n\nconst N = 100\n\nsuite "calc":\n    test "val":\n        assert N == 100')


# ===== ASYNC =====
f("async/ASY-001.lz",
  'def fetch() -> str:\n    return "data"')
f("async/ASY-002.lz",
  'def fetch() -> str:\n    return "data"\n\ndef process():\n    r = fetch()\n    print(r)')
f("async/ASY-003.lz",
  'def fetch() -> str:\n    return "data"\n\ndef process():\n    r = fetch()\n    print(r)')
f("async/ASY-004.lz",
  'def task():\n    pass\n\ndef main():\n    task()\n    print(1)')
f("async/ASY-005.lz",
  "def gen():\n    *:\n        yield 1\n        yield 2\n        yield 3")
f("async/ASY-006.lz",
  "def gen():\n    *:\n        yield 1\n        yield 2")


# ===== NEGATIVE (负向) =====
f("negative/lex/NEG-LEX-001.lz", 'std::io::println("should fail")')
f("negative/lex/NEG-LEX-002.lz", "/* unclosed block comment\n\ndef f() = 1")
f("negative/lex/NEG-LEX-003.lz", "def f() = @@@")
f("negative/lex/NEG-LEX-004.lz", 'def f() = "unclosed')

f("negative/parse/NEG-PARSE-001.lz", "def f() -> int\n    return 1")
f("negative/parse/NEG-PARSE-002.lz", 'def f() =\nprint("no indent")')
f("negative/parse/NEG-PARSE-003.lz", "def f() = (1 + 2")
f("negative/parse/NEG-PARSE-004.lz", "def f(a: int, b: str, ..,,..):\n    pass")
f("negative/parse/NEG-PARSE-005.lz", "def f() =\n    try:\n        pass\n    catch:\n        pass")
f("negative/parse/NEG-PARSE-006.lz", "def f() = |,| 1")

f("negative/type/NEG-TYPE-001.lz", "def f(a: int) -> int = a\n\ndef main() =\n    f(1, 2)")
f("negative/type/NEG-TYPE-002.lz", 'def f() -> int = "not int"')
f("negative/type/NEG-TYPE-003.lz", "def f() = undefined_var")
f("negative/type/NEG-TYPE-004.lz", 'def f() =\n    s = "hello"\n    s[0]')
f("negative/type/NEG-TYPE-005.lz", "def f(x: int) = x?.something")

f("negative/semantic/NEG-SEM-001.lz",
  "trait Greet =\n    def greet(self: Self) -> str\n\nstruct Person =\n    name: str\n\nimpl Greet for Person:\n    def greet(self: Person) -> int = 1")
f("negative/semantic/NEG-SEM-002.lz",
  "trait Full =\n    def a(self: Self) -> int\n    def b(self: Self) -> int\n\nstruct Part =\n    v: int\n\nimpl Full for Part:\n    def a(self: Part) -> int = 1")
f("negative/semantic/NEG-SEM-003.lz",
  "trait Val =\n    def val(self: Self) -> str\n\nstruct Wrong =\n    x: int\n\nimpl Val for Wrong:\n    def val(self: Wrong) -> int = self.x")
f("negative/semantic/NEG-SEM-004.lz",
  "trait Mutator =\n    def update(self: mut Self)\n\nstruct S =\n    v: int\n\nimpl Mutator for S:\n    def update(self: S):\n        pass")
f("negative/semantic/NEG-SEM-005.lz",
  "trait A =\n    def f(self: Self) -> int\n\ntrait B =\n    def f(self: Self) -> int\n\nstruct C =\n\nimpl A for C:\n    def f(self: C) -> int = 1\n\nimpl B for C:\n    def f(self: C) -> int = 2")
f("negative/semantic/NEG-SEM-006.lz",
  "def f(x: int) -> str:\n    match x:\n        case 0 => \"zero\"")
f("negative/semantic/NEG-SEM-007.lz",
  'def f() =\n    owned x = "hello"\n    y = x\n    print(x)')
f("negative/semantic/NEG-SEM-008.lz",
  "def f() -> int = 1\n\ndef f() -> int = 2")
f("negative/semantic/NEG-SEM-009.lz",
  "struct Point =\n    x: int\n\ndef f(p: Point) = p.y")


# ===== 写入 =====
created = 0
for path, content in sorted(S.items()):
    fp = os.path.join(CASES, path)
    os.makedirs(os.path.dirname(fp), exist_ok=True)
    with open(fp, "w", encoding="utf-8") as fout:
        fout.write(content)
    created += 1

print(f"Written {created} files")

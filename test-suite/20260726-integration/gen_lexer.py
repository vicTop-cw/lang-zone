#!/usr/bin/env python3
"""Create lexer test files"""
import os
CASES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "cases")

files = {}

# keywords
files["lexer/keywords/LEX-KW-001.lz"] = """\
def x = 1
let y = 2
mut z = 3
const C = 4
fn g() = 1
struct S = x: int
enum E = A | B
trait T =
    def m(self) -> int
impl T for S:
    def m(self: S) -> int = 1"""

files["lexer/keywords/LEX-KW-002.lz"] = """\
def f(x: int):
    if x > 0:
        return 1
    elif x < 0:
        return -1
    else:
        return 0
def g():
    for i in 1..3:
        print(i)
    while True:
        break
    loop:
        continue
    return"""

files["lexer/keywords/LEX-KW-003.lz"] = '''def f():
    try:
        raise "err"
    catch e:
        print(e)
    finally:
        print("done")
    panic("fatal")'''

files["lexer/keywords/LEX-KW-004.lz"] = """\
#!test
test "t1":
    assert True
    check False
suite "s":
    test "t2":
        assert 1 == 1"""

files["lexer/keywords/LEX-KW-005.lz"] = """\
async def f():
    await g()
    spawn h()
    yield 1"""

files["lexer/keywords/LEX-KW-006.lz"] = """\
import std.io
from std.vec import Vec as V"""

files["lexer/keywords/LEX-KW-007.lz"] = """\
macro m(input: Tokens) -> Tokens = f``` ```
template t!<T>(x: T) = f```{x}```
comptime answer = 42
def f<T>(x: T) where T <: Clone = x"""

files["lexer/keywords/LEX-KW-008.lz"] = """\
def f(x: int, y: int, z: int) -> bool:
    return x > 0 and y >= 5 or not (z is None) and 2 in [1, 2, 3]"""

files["lexer/keywords/LEX-KW-009.lz"] = '''def f():
    a = True
    b = False
    c = None
    d = Some(1)
    e = Ok(1)
    f = Err("e")
    match d:
        case Some(v) => v
        case None => 0'''

files["lexer/keywords/LEX-KW-010.lz"] = """\
abstract def f(owned s: str, ref r: int, private x: int, public y: int):
    move z = x"""

# comments
files["lexer/comments/LEX-CMT-001.lz"] = "// this is a comment\ndef f() = 1\n"
files["lexer/comments/LEX-CMT-002.lz"] = "/* block comment */\ndef f() = 1\n"
files["lexer/comments/LEX-CMT-003.lz"] = "/* outer /* inner nested */ still outer */\ndef f() = 1\n"
files["lexer/comments/LEX-CMT-004.lz"] = "#[attr]\ndef f() = 1\n"

# literals
files["lexer/literals/LEX-LIT-001.lz"] = "def f() = 42 + 0xFF + 0o77 + 0b1010\n"
files["lexer/literals/LEX-LIT-002.lz"] = "def f() = 1_000_000\n"
files["lexer/literals/LEX-LIT-003.lz"] = "def f() = 3.14 + 1e10 + 2.5e-3\n"
files["lexer/literals/LEX-LIT-004.lz"] = 'def f() = "hello"\n'
files["lexer/literals/LEX-LIT-005.lz"] = 'def f(x: int) -> str = f"x = {x}"\n'
files["lexer/literals/LEX-LIT-006.lz"] = 'def f() = r"C:\\\\path"\n'
files["lexer/literals/LEX-LIT-007.lz"] = 'def f() = """multi\nline"""\n'

# operators
files["lexer/operators/LEX-OP-001.lz"] = 'std::io::println("should fail")\n'
files["lexer/operators/LEX-OP-002.lz"] = """\
def f():
    x =:
        y = 1
        y
    r = g() ~:
        (1, 2)
    g *:
        yield 1
"""
files["lexer/operators/LEX-OP-003.lz"] = """\
def f():
    x = Some(1)
    y = x?.val ?? 0
    z = 5 |> |n| n + 1
    a = x^
    b := 42
"""

for path, content in files.items():
    fp = os.path.join(CASES, path)
    os.makedirs(os.path.dirname(fp), exist_ok=True)
    with open(fp, "w", encoding="utf-8") as f:
        f.write(content)

print(f"Created {len(files)} lexer files")

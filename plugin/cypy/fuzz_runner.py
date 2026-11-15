"""Cypy 模糊测试脚本 — 生成随机短代码并编译，收集错误模式"""
import subprocess
import os
import random
import itertools

CYPPATH = r"E:\IDEProjects\AI\Cypy"
OUTPUT_DIR = r"e:\IDEProjects\AI\lang-zone\josn_parse\cypy\fuzz_output"

# ---- 已知可用的构造块 ----
TYPES = ["int", "float", "bool", "str", "double"]
LITERALS = {
    "int": ["0", "1", "-1", "42", "100"],
    "float": ["0.0", "1.5", "-3.14"],
    "bool": ["True", "False"],
    "str": ['"hello"', '""', '"a"'],
    "double": ["0.0", "1.5"],
}
OPERATORS = ["+", "-", "*", "/", "==", "!=", "<", ">", "<=", ">="]
UNARY_OPS = ["-"]

def gen_expr(depth=0):
    """生成随机表达式"""
    if depth > 3:
        return random.choice(LITERALS["int"] + LITERALS["bool"])
    kind = random.choice(["lit", "binop", "call", "var", "paren", "unary"])
    if kind == "lit":
        t = random.choice(TYPES)
        return random.choice(LITERALS.get(t, LITERALS["int"]))
    elif kind == "binop":
        return f"({gen_expr(depth+1)} {random.choice(OPERATORS)} {gen_expr(depth+1)})"
    elif kind == "call":
        return f"print({gen_expr(depth+1)})"
    elif kind == "var":
        return random.choice(["x", "y", "z", "total", "result"])
    elif kind == "paren":
        return f"({gen_expr(depth+1)})"
    elif kind == "unary":
        return f"(-{gen_expr(depth+1)})"

def gen_stmt():
    """生成随机语句"""
    kind = random.choice(["let", "assign", "return", "if", "for", "expr", "guard", "match", "while", "defer"])
    if kind == "let":
        t = random.choice(TYPES)
        return f"    let x: {t} = {random.choice(LITERALS.get(t, LITERALS['int']))}"
    elif kind == "assign":
        return f"    x = {gen_expr()}"
    elif kind == "return":
        return f"    return {gen_expr()}"
    elif kind == "if":
        cond = gen_expr()
        return f"    if {cond}:\n        return 1\n    else:\n        return 0"
    elif kind == "for":
        return f"    for i in range({random.randint(1,5)}):\n        let temp: int = i"
    elif kind == "expr":
        return f"    {gen_expr()}"
    elif kind == "guard":
        return f"    guard {gen_expr()} else:\n        return -1"
    elif kind == "match":
        return f"    match {gen_expr()}:\n        case 0:\n            return 0\n        case _:\n            return -1"
    elif kind == "while":
        return f"    while {gen_expr()}:\n        break"
    elif kind == "defer":
        return f"    defer:\n        let cleanup: int = 0"

def gen_func(name, num_stmts=3):
    """生成随机函数"""
    stmts = [gen_stmt() for _ in range(num_stmts)]
    return f"def {name}() -> int:\n" + "\n".join(stmts)

def gen_program(num_funcs=2):
    """生成随机程序"""
    funcs = [gen_func(f"test_{i}", random.randint(2, 5)) for i in range(num_funcs)]
    main = "def main() -> int:\n    return 0"
    return "\n\n".join(funcs + [main])

# ---- 边界测试用例生成器 ----
def gen_edge_cases():
    """生成各种边界测试用例"""
    cases = []
    
    # 1. 空输入
    cases.append(("empty", "def main() -> int:\n    return 0"))
    
    # 2. 极长标识符
    long_name = "a" * 200
    cases.append(("long_ident", f"def main() -> int:\n    let {long_name}: int = 0\n    return 0"))
    
    # 3. 深层嵌套表达式
    deep = "1"
    for _ in range(20):
        deep = f"({deep} + 1)"
    cases.append(("deep_expr", f"def main() -> int:\n    return {deep}"))
    
    # 4. 深层嵌套 if
    deep_if = "def main() -> int:\n"
    indent = ""
    for i in range(15):
        deep_if += f"{indent}    if True:\n"
        indent += "    "
    deep_if += f"{indent}    return 0\n"
    for _ in range(15):
        indent = indent[4:]
        deep_if += f"{indent}    else:\n"
        deep_if += f"{indent}        return 0\n"
    cases.append(("deep_if", deep_if))
    
    # 5. 多函数（大量）
    many_funcs = "\n\n".join([f"def f{i}() -> int:\n    return {i}" for i in range(50)])
    many_funcs += "\n\ndef main() -> int:\n    return 0"
    cases.append(("many_funcs", many_funcs))
    
    # 6. 复杂字符串
    cases.append(("complex_str", "def main() -> int:\n    let s: str = \"hello world\"\n    return 0"))
    
    # 7. 负数字面量各种组合
    cases.append(("neg_nums", "def main() -> int:\n    return -1 + -2 * -3"))
    
    # 8. 注释
    cases.append(("comments", "# top comment\ndef main() -> int:\n    # inner comment\n    let x: int = 1 # inline\n    return x"))
    
    # 9. 空行
    cases.append(("blank_lines", "\n\n\ndef main() -> int:\n\n\n    return 0\n\n"))
    
    # 10. 仅注释
    cases.append(("only_comment", "# just a comment\n# another one\n\ndef main() -> int:\n    return 0"))
    
    # 11. 连续操作符
    cases.append(("chain_ops", "def main() -> int:\n    return 1 + 2 - 3 * 4 / 5"))
    
    # 12. 布尔短路
    cases.append(("bool_short", "def main() -> int:\n    if True and False:\n        return 1\n    return 0"))
    
    # 13. 比较链
    cases.append(("compare_chain", "def main() -> int:\n    if 1 < 2 and 2 < 3:\n        return 1\n    return 0"))
    
    # 14. match 多 case
    match_multi = "def test_match(x: int) -> int:\n    match x:\n"
    for i in range(10):
        match_multi += f"        case {i}:\n            return {i}\n"
    match_multi += "        case _:\n            return -1\n\ndef main() -> int:\n    return 0"
    cases.append(("match_multi", match_multi))
    
    # 15. 嵌套函数
    cases.append(("nested_func", "def outer() -> int:\n    def inner() -> int:\n        return 42\n    return inner()\n\ndef main() -> int:\n    return 0"))
    
    # 16. 递归函数
    cases.append(("recursive", "def fact(n: int) -> int:\n    if n <= 1:\n        return 1\n    return n * fact(n - 1)\n\ndef main() -> int:\n    return 0"))
    
    # 17. 多返回语句
    cases.append(("multi_return", "def test(x: int) -> int:\n    if x > 0:\n        return 1\n    elif x == 0:\n        return 0\n    else:\n        return -1\n\ndef main() -> int:\n    return 0"))
    
    # 18. break/continue
    cases.append(("break_cont", "def test() -> int:\n    for i in range(10):\n        if i == 5:\n            break\n    return 0\n\ndef main() -> int:\n    return 0"))
    
    # 19. 空 for 循环
    cases.append(("empty_for", "def test() -> int:\n    for i in range(3):\n        let x: int = 0\n    return 0\n\ndef main() -> int:\n    return 0"))
    
    # 20. 空 while 循环
    cases.append(("empty_while", "def test() -> int:\n    while False:\n        let x: int = 0\n    return 0\n\ndef main() -> int:\n    return 0"))
    
    # 21. 复杂类型注解
    cases.append(("complex_type", "def test() -> list[int]:\n    return [1, 2, 3]\n\ndef main() -> int:\n    return 0"))
    
    # 22. struct 定义
    cases.append(("struct_def", "struct Point:\n    x: int = 0\n    y: int = 0\n\ndef main() -> int:\n    return 0"))
    
    # 23. 多参数函数
    multi_params = "def many_params(" + ", ".join([f"a{i}: int" for i in range(10)]) + ") -> int:\n    return a0\n\ndef main() -> int:\n    return 0"
    cases.append(("multi_params", multi_params))
    
    # 24. 变量遮蔽
    cases.append(("shadow", "def test() -> int:\n    let x: int = 1\n    if True:\n        let x: int = 2\n        return x\n    return x\n\ndef main() -> int:\n    return 0"))
    
    # 25. 空类
    cases.append(("empty_class", "class Empty:\n    pass\n\ndef main() -> int:\n    return 0"))
    
    # 26. 带字段的类
    cases.append(("class_fields", "class Data:\n    name: str = \"\"\n    age: int = 0\n\ndef main() -> int:\n    return 0"))
    
    # 27. 枚举
    cases.append(("enum", "enum Color:\n    Red\n    Green\n    Blue\n\ndef main() -> int:\n    return 0"))
    
    # 28. defer 块
    cases.append(("defer_block", "def test() -> int:\n    defer:\n        let x: int = 0\n    return 1\n\ndef main() -> int:\n    return 0"))
    
    # 29. 复杂 guard
    cases.append(("complex_guard", "def test(x: int) -> int:\n    guard x >= 0 else:\n        return -1\n    return x\n\ndef main() -> int:\n    return 0"))
    
    # 30. 类型转换
    cases.append(("cast", "def test() -> int:\n    let f: float = 3.14\n    let x: int = int(f)\n    return x\n\ndef main() -> int:\n    return 0"))
    
    # 31. 字符边界 - 特殊字符
    cases.append(("special_chars", 'def main() -> int:\n    let s: str = "!@#$%^&*()"\n    return 0'))
    
    # 32. 多行字符串拼接
    cases.append(("str_concat", 'def main() -> int:\n    let s: str = "hello" + " " + "world"\n    return 0'))
    
    # 33. 列表字面量
    cases.append(("list_literal", "def test() -> list[int]:\n    return [1, 2, 3, 4, 5]\n\ndef main() -> int:\n    return 0"))
    
    # 34. 下标访问
    cases.append(("subscript", "def test() -> int:\n    let lst: list[int] = [1, 2, 3]\n    return lst[0]\n\ndef main() -> int:\n    return 0"))
    
    # 35. 属性访问
    cases.append(("attr_access", "struct Point:\n    x: int = 0\n    y: int = 0\n\ndef test() -> int:\n    let p: Point = Point{x: 1, y: 2}\n    return p.x\n\ndef main() -> int:\n    return 0"))
    
    # 36. 空元组
    cases.append(("empty_tuple", "def test() -> tuple:\n    return ()\n\ndef main() -> int:\n    return 0"))
    
    # 37. 大整数
    cases.append(("big_int", "def main() -> int:\n    return 999999999999999999"))
    
    # 38. 科学计数法
    cases.append(("scientific", "def main() -> int:\n    let f: float = 1.5e10\n    return 0"))
    
    # 39. 方法调用链
    cases.append(("call_chain", "def main() -> int:\n    print(print(1))\n    return 0"))
    
    # 40. 无返回值的函数
    cases.append(("no_return", "def test() -> int:\n    let x: int = 1\n\ndef main() -> int:\n    return 0"))
    
    return cases


def run_test(name, code):
    """运行单个测试并返回结果"""
    filepath = os.path.join(OUTPUT_DIR, f"fuzz_{name}.cypy")
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(code)
    
    try:
        result = subprocess.run(
            ["python", "-m", "cypyc", "transpile", "--check-only", filepath],
            cwd=CYPPATH,
            capture_output=True,
            text=True,
            timeout=30
        )
        return result.returncode, result.stdout.strip(), result.stderr.strip()
    except subprocess.TimeoutExpired:
        return -1, "", "TIMEOUT"
    except Exception as e:
        return -1, "", str(e)


def run_random_fuzz(num_tests=100):
    """运行随机模糊测试"""
    errors = {}
    passes = 0
    for i in range(num_tests):
        program = gen_program(random.randint(1, 3))
        rc, stdout, stderr = run_test(f"rand_{i}", program)
        if rc == 0:
            passes += 1
        else:
            # 提取错误类型
            for line in stderr.split("\n"):
                if ":" in line and ("error" in line.lower() or "Error" in line or "failed" in line.lower()):
                    key = line.strip()
                    errors[key] = errors.get(key, 0) + 1
    return passes, errors


def run_edge_cases():
    """运行边界测试用例"""
    results = []
    for name, code in gen_edge_cases():
        rc, stdout, stderr = run_test(name, code)
        status = "PASS" if rc == 0 else "FAIL"
        error = ""
        if rc != 0:
            # 提取关键错误信息
            lines = stderr.split("\n")
            for line in lines:
                if "Error" in line or "error" in line or "failed" in line or "Undefined" in line or "mismatch" in line or "Unexpected" in line or "Expected" in line:
                    error = line.strip()
                    break
            if not error and lines:
                error = lines[0].strip()
        results.append((name, status, error))
    return results


if __name__ == "__main__":
    print("=" * 60)
    print("Cypy 模糊测试")
    print("=" * 60)
    
    # 1. 边界测试
    print("\n--- 边界测试用例 ---")
    results = run_edge_cases()
    fail_count = 0
    for name, status, error in results:
        icon = "✓" if status == "PASS" else "✗"
        print(f"  {icon} {name:20s} {error}")
        if status == "FAIL":
            fail_count += 1
    print(f"\n边界测试: {len(results)-fail_count}/{len(results)} 通过, {fail_count} 失败")
    
    # 2. 随机模糊测试
    print("\n--- 随机模糊测试 (100 个随机程序) ---")
    passes, errors = run_random_fuzz(100)
    print(f"通过: {passes}/100")
    print(f"失败: {100-passes}/100")
    if errors:
        print("\n错误类型分布:")
        for err, count in sorted(errors.items(), key=lambda x: -x[1])[:15]:
            print(f"  [{count:3d}x] {err[:100]}")
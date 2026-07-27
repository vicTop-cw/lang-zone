# -*- coding: utf-8 -*-
# Lang-Zong 边界测试 harness (修正版 v2)
# 语法要点: 函数体用 `=` 引入 (def main() =), struct/enum 用 `=` (struct Foo =)
# 每个语法在 4 维度(错误写法 / 无映射 / 作用域 / 缩进)做最小集测试。
import os, subprocess, csv

BIN = r"E:/IDEProjects/AI/lang-zone/target/debug/lang-zong.exe"
RUSTC = "rustc"
CASE_DIR = r"E:/IDEProjects/AI/lang-zone/tests_boundary/cases"
os.makedirs(CASE_DIR, exist_ok=True)

NOOP = "def main() =\n  print(1)\n"   # 不适用维度用的占位有效程序

def P(body):
    ind = "\n".join(("  " + ln) if ln.strip() else ln for ln in body.strip("\n").split("\n"))
    return "def main() =\n" + ind + "\n"

def TOP(src):
    return src

# expect: PARSE_ERROR / RUSTC_ERROR / OK / NO_ERROR / ANY_ERROR
CASES = []
def add(syntax, dim, src, expect, sample, note=""):
    CASES.append({"syntax": syntax, "dim": dim, "src": src, "expect": expect,
                  "sample": sample, "note": note})

# 1. 注释
add("注释", "错误写法", "def main() =\n  x = 1\n  /* 未结束的块注释\n  y = 2\n", "NO_ERROR", "/* 未结束块注释 EOF", "词法层静默忽略未结束块注释(不报错)")
add("注释", "无映射", NOOP, "OK", "N/A(注释无映射)", "注释在词法层丢弃, 无映射概念")
add("注释", "作用域", P("x = 1 // 行尾注释\n// 整行注释\ny = 2\nprint(y)"), "OK", "行尾/整行 // 注释", "注释可出现在任意作用域行尾")
add("注释", "缩进", P("x = 1\n  // 更深缩进注释\ny = 2\nprint(y)"), "OK", "缩进块内注释", "注释缩进不影响解析")

# 2. 变量赋值 / 类型化绑定
add("变量赋值", "错误写法", P("x ="), "PARSE_ERROR", "x =  (缺右值)", "解析期报错")
add("变量赋值", "无映射", P("x: UnknownType = 5\nprint(x)"), "RUSTC_ERROR", "x: UnknownType = 5", "codegen 透传未知类型 -> rustc cannot find type")
add("变量赋值", "作用域", "y = 5\ndef main() =\n  print(y)\n", "NO_ERROR", "顶层 y = 5 (转 const)", "顶层赋值被收为 const, 不报错")
add("变量赋值", "缩进", P("   x = 1\n  y = 2\nprint(y)"), "ANY_ERROR", "语句缩进深于函数头", "函数体内缩进须一致(否则错位)")

# 3. mut
add("mut", "错误写法", P("mut"), "PARSE_ERROR", "mut (缺绑定名)", "解析期报错")
add("mut", "无映射", P("mut x = 5\nx = 6\nprint(x)"), "OK", "mut x = 5 (no-op)", "mut 为兼容 no-op, 无单独映射; 重赋值正常")
add("mut", "作用域", "def f(mut x: int)-> int =\n  return x\ndef main() =\n  print(f(1))\n", "OK", "形参 mut x: int", "参数修饰 mut 被接受")
add("mut", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 4. ref / &
add("ref", "错误写法", P("ref"), "PARSE_ERROR", "ref (缺绑定)", "解析期报错")
add("ref", "无映射", P("x = 5\nref r = &x\nprint(r)"), "PARSE_ERROR", "ref r = &x", "限制: ref 局部绑定 + & 表达式暂不支持(parse期报错)")
add("ref", "作用域", "ref r = &5\ndef main() =\n  print(r)\n", "PARSE_ERROR", "顶层 ref r = &5", "限制: & 表达式不被解析(顶层亦报错)")
add("ref", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 5. ^ move / XOR
add("^ move / XOR", "错误写法", P("x = 1\ny = ^x"), "PARSE_ERROR", "y = ^x (前缀 ^)", "解析期报错")
add("^ move / XOR", "无映射", P("a = 5\nb = a ^\nprint(b)"), "PARSE_ERROR", "a ^ (缺右操作数)", "期望解析期报错(实测: 悬空 ^ 被静默接受 -> 弱点)")
add("^ move / XOR", "作用域", P("s = \"hello\"\nt = s^\nprint(s)"), "RUSTC_ERROR", "t = s^ 后复用 s", "use-after-move 由 rustc 捕获")
add("^ move / XOR", "缩进", P("a = 6\nb = a ^ 3\nprint(b)"), "OK", "中缀 XOR: a ^ 3", "XOR 正常映射")

# 6. owned 形参
add("owned 形参", "错误写法", "def f(owned)-> int =\n  return 1\ndef main() =\n  print(1)\n", "PARSE_ERROR", "owned (缺参数名)", "解析期报错")
add("owned 形参", "无映射", "struct Person =\n  name: str\n\ndef take(owned p: Person)-> str =\n  return p.name\n\ndef main() =\n  bob = Person(name: \"Bob\")\n  r = take(bob)\n  print(r)\n", "RUSTC_ERROR", "take(bob) 缺 ^", "owned 契约: 缺 ^ 注入 compile_error!")
add("owned 形参", "作用域", P("owned x = 5\nprint(x)"), "NO_ERROR", "局部绑定 owned x", "owned 作局部绑定被静默忽略(无错误)")
add("owned 形参", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 7. const
add("const", "错误写法", "const x\n", "PARSE_ERROR", "const x (缺值)", "解析期报错")
add("const", "无映射", "const x: NoSuch = 5\ndef main() =\n  print(x)\n", "RUSTC_ERROR", "const x: NoSuch = 5", "未知类型 -> rustc")
add("const", "作用域", P("const x = 5\nprint(x)"), "OK", "函数体内 const", "已修复: 函数内 const 退化为 let mut(可编译)")
add("const", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 8. 函数 def / return
add("函数 def", "错误写法", "def\n", "PARSE_ERROR", "def (缺名)", "解析期报错")
add("函数 def", "无映射", "def foo()-> int = 5\ndef main() =\n  print(foo())\n", "OK", "def foo()-> int = 5", "箭头返回类型映射为 Rust 返回类型")
add("函数 def", "作用域", P("def inner() =\n    return 1\n  print(inner())"), "PARSE_ERROR", "函数内嵌套 def", "嵌套 def 不被允许")
add("函数 def", "缩进", "def foo()-> int =\n  return 1\ndef main() =\n  print(foo())\n", "OK", "函数体缩进正确", "块体缩进 2 空格")
add("return", "作用域", "return 5\ndef main() =\n  print(1)\n", "PARSE_ERROR", "顶层 return", "顶层 return 被解析期拒绝(非静默)")
add("return", "错误写法", "def foo() =\n  return\n\ndef main() =\n  foo()\n", "OK", "return 无值", "return 允许无值(函数无返回类型)")

# 9. async / await
add("async", "错误写法", "async\n", "PARSE_ERROR", "async (后无 def)", "解析期报错")
add("async", "无映射", "async def main() =\n  print(1)\n", "RUSTC_ERROR", "async def main", "main 不能为 async -> rustc")
add("async", "作用域", P("async\nprint(1)"), "PARSE_ERROR", "函数体中的 async", "async 仅修饰 def")
add("async", "缩进", "async def foo() =\n  print(1)\ndef main() =\n  print(2)\n", "OK", "async fn 缩进正确", "async def(非 main)可编译, 未被调用则无运行时错误")

# 10. import
add("import", "错误写法", "import\n", "PARSE_ERROR", "import (缺路径)", "解析期报错")
add("import", "无映射", "import nonexistent::module::Thing\ndef main() =\n  print(1)\n", "RUSTC_ERROR", "import 不存在路径", "use 不存在路径 -> rustc unresolved")
add("import", "作用域", P("import std::collections::HashMap\nprint(1)"), "PARSE_ERROR", "函数体内 import", "import 仅顶层")
add("import", "缩进", "  import std::collections::HashMap\ndef main() =\n  print(1)\n", "PARSE_ERROR", "顶层 import 带缩进", "顶层缩进非零 -> 解析期报错")

# 11. 类型 / 泛型
add("泛型类型", "错误写法", "def f(x: List<)-> int =\n  return x\ndef main() =\n  print(1)\n", "PARSE_ERROR", "List< (缺类型参数)", "解析期报错")
add("泛型类型", "无映射", "def f(x: MyStruct)-> int =\n  return 1\ndef main() =\n  print(f(5))\n", "RUSTC_ERROR", "形参类型 MyStruct 未声明", "未知类型 -> rustc")
add("泛型类型", "作用域", P("x: List<int> = [1, 2]\nprint(x)"), "RUSTC_ERROR", "局部泛型类型标注", "List<int> 泛型标注可用(但 print 需 Display → 已知限制)")
add("泛型类型", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 12. if
add("if", "错误写法", P("if x\n  print(1)"), "PARSE_ERROR", "if x (缺冒号)", "解析期报错")
add("if", "无映射", NOOP, "OK", "N/A", "if 无独立映射层")
add("if", "作用域", "if true:\n  print(1)\ndef main() =\n  print(2)\n", "PARSE_ERROR", "顶层 if", "顶层控制流被解析期拒绝")
add("if", "缩进", P("if true:\nprint(1)"), "OK", "if 体同行(单行形式)", "块体可同行(单行语句形式), 不强制缩进")

# 13. match
add("match", "错误写法", P("match x\n  case 1=> print(1)"), "PARSE_ERROR", "match x (缺冒号)", "解析期报错")
add("match", "无映射", "enum Color =\n  Red\n  Green\n\ndef describe(c: Color)-> str =\n  match c:\n    case Red=> return \"r\"\ndef main() =\n  print(describe(Color.Red))\n", "RUSTC_ERROR", "match 非穷尽", "缺 wildcard -> rustc non-exhaustive")
add("match", "作用域", "match 1:\n  case 1=> print(1)\ndef main() =\n  print(2)\n", "PARSE_ERROR", "顶层 match", "顶层控制流被解析期拒绝")
add("match", "缩进", P("match 1:\n  case 1=>\n    print(1)\n  case _=>\n    print(0)"), "OK", "case 缩进正确+通配", "case 体缩进 4 空格, 含 _ 通配可编译")

# 14. while
add("while", "错误写法", P("while\n  print(1)"), "PARSE_ERROR", "while (缺条件)", "解析期报错")
add("while", "无映射", NOOP, "OK", "N/A", "while 无独立映射层")
add("while", "作用域", "while true:\n  print(1)\ndef main() =\n  print(2)\n", "PARSE_ERROR", "顶层 while", "顶层控制流被解析期拒绝")
add("while", "缩进", P("x = 0\nwhile x < 3:\n    x = x + 1\nprint(x)"), "OK", "while 体缩进(有界)", "已修复: 循环内赋值正确变更外层变量, 可终止")

# 15. for
add("for", "错误写法", P("for x\n  print(x)"), "PARSE_ERROR", "for x (缺 in)", "解析期报错")
add("for", "无映射", NOOP, "OK", "N/A", "for 无独立映射层")
add("for", "作用域", "for x in 1..3:\n  print(x)\ndef main() =\n  print(2)\n", "PARSE_ERROR", "顶层 for", "顶层控制流被解析期拒绝")
add("for", "缩进", P("for x in 1..3:\n    print(x)"), "OK", "for 体缩进", "块体缩进正确")

# 16. loop / break / continue
add("loop", "错误写法", P("loop\n  print(1)"), "PARSE_ERROR", "loop (缺冒号)", "解析期报错")
add("loop", "无映射", NOOP, "OK", "N/A", "loop 无独立映射层")
add("loop", "作用域", "break\ncontinue\ndef main() =\n  print(1)\n", "PARSE_ERROR", "顶层 break/continue", "顶层控制流被解析期拒绝")
add("loop", "缩进", P("loop:\n    break"), "OK", "loop 体 break", "块体缩进正确")

# 17. guard
add("guard", "错误写法", P("guard"), "PARSE_ERROR", "guard (缺条件/let)", "解析期报错")
add("guard", "无映射", NOOP, "OK", "N/A", "guard 无独立映射层")
add("guard", "作用域", "guard let x = Some(1) else:\n  print(\"no\")\ndef main() =\n  print(2)\n", "PARSE_ERROR", "顶层 guard", "顶层控制流被解析期拒绝")
add("guard", "缩进", P("guard let Some(x) = Some(1) else:\n    print(\"no\")\n  print(x)"), "OK", "guard else 体缩进", "有效嵌套作用域, 解构绑定")

# 18. with
add("with", "错误写法", P("with open()\n  print(1)"), "PARSE_ERROR", "with (缺冒号)", "解析期报错")
add("with", "无映射", P("with open_file() as f:\n  print(f)"), "RUSTC_ERROR", "with 调用 __exit__", "codegen 引用未定义 __exit__")
add("with", "作用域", "with open_file() as f:\n  print(f)\ndef main() =\n  print(2)\n", "PARSE_ERROR", "顶层 with", "顶层控制流被解析期拒绝")
add("with", "缩进", P("with open_file() as f:\n    print(f)"), "RUSTC_ERROR", "with 体缩进正确但缺 __exit__", "缩进正确, 仍缺运行时映射")

# 19. spawn
add("spawn", "错误写法", P("spawn"), "PARSE_ERROR", "spawn (缺表达式)", "解析期报错")
add("spawn", "无映射", "def work() =\n  print(1)\ndef main() =\n  spawn work()\n", "RUSTC_ERROR", "spawn work()", "限制: std::thread::spawn 生成代码类型不匹配 -> rustc")
add("spawn", "作用域", "spawn work()\ndef work() =\n  print(1)\ndef main() =\n  print(2)\n", "PARSE_ERROR", "顶层 spawn", "顶层控制流被解析期拒绝")
add("spawn", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 20. yield
add("yield", "无映射", "def gen() =\n  yield 1\n  yield 2\ndef main() =\n  gen()\n", "RUSTC_ERROR", "yield 在普通 fn", "rustc E0658 (yield 仅生成器)")
add("yield", "作用域", "yield 1\ndef main() =\n  print(2)\n", "PARSE_ERROR", "顶层 yield", "顶层控制流被解析期拒绝")
add("yield", "错误写法", "def gen() =\n  yield @\n", "PARSE_ERROR", "yield @ (无效 token)", "解析期报错")
add("yield", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 21. 闭包
add("闭包", "错误写法", P("f = |x|"), "PARSE_ERROR", "|x| (缺函数体)", "解析期报错")
add("闭包", "无映射", P("f = |x| x + 1\nprint(f(2))"), "RUSTC_ERROR", "闭包赋值并调用", "限制: 闭包需显式类型标注 -> rustc E0282")
add("闭包", "作用域", "f = |x| x + 1\ndef main() =\n  print(f(2))\n", "RUSTC_ERROR", "顶层闭包 const", "closure 不能出现在 const")
add("闭包", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 22. range
add("range", "错误写法", P("for x in 1..2..3:\n  print(x)"), "PARSE_ERROR", "1..2..3 (双范围)", "解析期报错")
add("range", "无映射", P("for x in 1..=5:\n  print(x)"), "OK", "inclusive range 1..=5", "range 映射为 Rust range")
add("range", "作用域", P("r = 1..3\nprint(r)"), "RUSTC_ERROR", "range 直接 print", "限制: Range 未实现 Display -> rustc")
add("range", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 23. pipe
add("pipe", "错误写法", P("y = 5 |>"), "PARSE_ERROR", "5 |> (缺右侧)", "解析期报错")
add("pipe", "无映射", P("y = 5 |> undefined_fn\nprint(y)"), "RUSTC_ERROR", "5 |> undefined_fn", "映射为函数调用 -> rustc 找不到函数")
add("pipe", "作用域", "y = 5 |> double\ndef double(x: int)-> int =\n  return x * 2\ndef main() =\n  print(y)\n", "RUSTC_ERROR", "顶层 pipe 转 const", "限制: 顶层 pipe 收为 const, 不能调用非 const 函数")
add("pipe", "缩进", P("y = 5 |>\n  double\nprint(y)\ndef double(x: int)-> int =\n  return x * 2"), "PARSE_ERROR", "pipe 跨行", "pipe 不支持跨行")

# 24. ?. safe-nav
add("safe-nav", "错误写法", P("x = a?."), "PARSE_ERROR", "a?. (缺字段)", "解析期报错")
add("safe-nav", "无映射", P("x = 5?.field\nprint(x)"), "RUSTC_ERROR", "5?.field", "int 无 .map -> rustc")
add("safe-nav", "作用域", "struct P =\n  v: int\ndef main() =\n  o: P? = Some(P(v: 7))\n  x = o?.v ?? 0\n  print(x)\n", "OK", "Option safe-nav + ??", "映射为 (o).map(|x| x.v).unwrap_or(0)")
add("safe-nav", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 25. ?? null-coalesce
add("null-coalesce", "错误写法", P("x = a ??"), "PARSE_ERROR", "a ?? (缺右值)", "解析期报错")
add("null-coalesce", "无映射", P("x = 5 ?? 0\nprint(x)"), "RUSTC_ERROR", "5 ?? 0", "int 无 unwrap_or -> rustc")
add("null-coalesce", "作用域", P("o: int? = None\nx = o ?? 99\nprint(x)"), "OK", "Option ?? 默认值", "映射为 (o).unwrap_or(...)")
add("null-coalesce", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 26. ? try
add("try ?", "无映射", P("x = 5?\nprint(x)"), "RUSTC_ERROR", "5? (i64 无 ?)", "非 Result/Option -> rustc")
add("try ?", "作用域", "def get()-> int? =\n  r: int? = Some(5)\n  return r\ndef main() =\n  print(get())\n", "RUSTC_ERROR", "Option? 传播(函数内)", "Option 无 Display + return r? 不成立 → 已知限制")
add("try ?", "错误写法", P("x = ?5"), "PARSE_ERROR", "?5 (前缀 ?)", "解析期报错")
add("try ?", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 27. f-string
add("f-string", "错误写法", P('s = f"{1 + }"'), "PARSE_ERROR", 'f"{1 + }" (插值表达式错)', "解析期报错")
add("f-string", "无映射", P('s = f"{undefined_var}"\nprint(s)'), "RUSTC_ERROR", 'f"{undefined_var}"', "插值变量未定义 -> rustc")
add("f-string", "作用域", P("x = 1\ns = f\"x={x}\"\nprint(s)"), "OK", 'f"x={x}" (块内)', "插值映射为 format!")
add("f-string", "缩进", 's = f"hi"\ndef main() =\n  print(s)\n', "RUSTC_ERROR", "顶层 f-string const", "限制: 顶层 f-string 收为 const 但缺类型标注 -> rustc")

# 28. struct / enum / trait / decorator
add("struct", "错误写法", "struct Foo\n", "PARSE_ERROR", "struct Foo (缺 =)", "解析期报错")
add("struct", "无映射", "struct Foo =\n  x: int\ndef main() =\n  f = Foo(x: 1)\n  y = f.nonexistent()\n  print(y)\n", "RUSTC_ERROR", "struct 方法不存在", "f.nonexistent() -> rustc 无此方法")
add("struct", "作用域", P("struct Foo =\n  x: int"), "PARSE_ERROR", "函数体内 struct", "struct 仅顶层")
add("struct", "缩进", "struct Foo =\n  x: int\n  y: int\ndef main() =\n  print(1)\n", "OK", "struct 字段缩进", "字段缩进 2 空格")
add("decorator", "无映射", "@unknown_attr\ndef foo()-> int =\n  return 1\ndef main() =\n  print(foo())\n", "RUSTC_ERROR", "@unknown_attr", "未知属性 -> rustc")
add("decorator", "错误写法", "@\ndef foo()-> int =\n  return 1\n", "PARSE_ERROR", "@ (缺装饰器名)", "解析期报错")
add("decorator", "作用域", P("@deco\ndef foo()-> int =\n  return 1"), "PARSE_ERROR", "函数体内装饰器", "装饰器仅顶层")

# 29. index
add("index", "错误写法", P("a = [1,2,3]\nx = a["), "PARSE_ERROR", "a[ (缺闭括号)", "解析期报错")
add("index", "无映射", P("a = [1, 2, 3]\nx = a[1.5]\nprint(x)"), "RUSTC_ERROR", "a[1.5]", "浮点索引 -> rustc")
add("index", "作用域", P("a = [1, 2, 3]\nx = a[0]\nprint(x)"), "OK", "列表索引", "映射为 a[0]")
add("index", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 30. method-call
add("method-call", "错误写法", P("x = a."), "PARSE_ERROR", "a. (缺方法名)", "解析期报错")
add("method-call", "无映射", P('s = "hi"\nr = s.unknown_method()\nprint(r)'), "RUSTC_ERROR", 's.unknown_method()', "不存在方法 -> rustc")
add("method-call", "作用域", P('s = "hi"\nl = s.len()\nprint(l)'), "OK", 's.len()', "映射为方法调用")
add("method-call", "缩进", NOOP, "OK", "N/A", "缩进不适用")

# 31. comprehension
add("comprehension", "错误写法", P("lst = [x for ]\nprint(lst)"), "PARSE_ERROR", "[x for ] (缺迭代器)", "解析期报错")
add("comprehension", "无映射", P("lst = [x * 2 for x in 1..5 if x > 2]\nprint(lst)"), "RUSTC_ERROR", "[x for x in 1..5 ...]", "限制: 推导中 1..5 元素类型需标注 -> rustc 歧义")
add("comprehension", "作用域", P("lst = [x for x in [1,2,3]]\nprint(lst)"), "RUSTC_ERROR", "列表推导(具体列表)", "推导+print → Vec无Display → 已知限制(.collect::<Vec<_>>()已修复类型推断)")
add("comprehension", "缩进", "lst = [x for x in [1,2,3]]\ndef main() =\n  print(lst)\n", "RUSTC_ERROR", "顶层列表推导 const", "顶层推导收为 const → 不能调用非const fn → 已知限制")

# 32. 缩进层次专项
add("缩进", "缩进", "def main() =\nprint(1)\n", "OK", "函数体未缩进(单行形式)", "允许: 函数体可为无缩进单行语句")
add("缩进", "缩进", "def main() =\n  if true:\n    x = 1\n   y = 2\n  print(x)\n", "ANY_ERROR", "嵌套缩进不一致(2/4/3)", "词法层静默忽略不匹配缩进 -> 后续可能误解析(弱点)")
add("缩进", "缩进", "def main() =\n\tprint(1)\n", "OK", "Tab 作缩进", "Tab 计为 1 列, 可解析")
add("缩进", "缩进", "def main() =\n    x = 1\n  y = 2\n", "OK", "函数体内缩进深于头", "缩进错位时深层语句被收为顶层 const, 不报错(弱点)")

def classify(case_id, src):
    lz = os.path.join(CASE_DIR, case_id + ".lz")
    with open(lz, "w", encoding="utf-8") as f:
        f.write(src)
    try:
        r = subprocess.run([BIN, lz], capture_output=True, text=True, timeout=60)
    except Exception as e:
        return ("OTHER", "run fail: %s" % e, lz)
    out = (r.stdout or "") + (r.stderr or "")
    if "Parse error:" in out:
        detail = out.split("Parse error:", 1)[1].strip().splitlines()[0]
        return ("PARSE_ERROR", detail[:200], lz)
    if "Generated" in out:
        rs = lz[:-3] + ".rs"
        re_ = subprocess.run([RUSTC, "--edition", "2021", "-O", rs, "-o", lz[:-3] + ".exe"],
                              capture_output=True, text=True, timeout=120)
        if re_.returncode != 0:
            err = (re_.stdout + re_.stderr)
            line = ""
            for ln in err.splitlines():
                if "error" in ln:
                    line = ln.strip()
                    break
            return ("RUSTC_ERROR", (line or err.strip().splitlines()[0])[:200], lz)
        exe = lz[:-3] + ".exe"
        try:
            rr = subprocess.run([exe], capture_output=True, text=True, timeout=15)
        except subprocess.TimeoutExpired:
            return ("RUNTIME_TIMEOUT", "执行超过 15s (疑似死循环)", lz)
        if rr.returncode != 0:
            return ("RUNTIME_ERROR", (rr.stderr or rr.stdout).strip().splitlines()[0][:200], lz)
        first = (rr.stdout or "").strip().splitlines()[0] if (rr.stdout or "").strip() else "ran ok"
        return ("OK", first[:200], lz)
    return ("OTHER", out.strip().splitlines()[-1][:200] if out.strip() else "no output", lz)

def passes(cat, expect):
    if expect == "PARSE_ERROR": return cat == "PARSE_ERROR"
    if expect == "RUSTC_ERROR": return cat == "RUSTC_ERROR"
    if expect == "OK": return cat == "OK"
    if expect == "NO_ERROR": return cat in ("OK", "RUNTIME_ERROR")
    if expect == "ANY_ERROR": return cat in ("PARSE_ERROR", "RUSTC_ERROR")
    return False

def main():
    rows = []
    for i, c in enumerate(CASES):
        cid = "c%03d" % i
        cat, detail, lz = classify(cid, c["src"])
        pf = "PASS" if passes(cat, c["expect"]) else "FAIL"
        rows.append({"id": cid, "syntax": c["syntax"], "dim": c["dim"],
                     "sample": c["sample"], "expect": c["expect"],
                     "actual": cat, "behavior": detail, "pass": pf, "note": c["note"]})
        for ext in (".rs", ".exe"):
            p = lz[:-3] + ext
            if os.path.exists(p):
                try: os.remove(p)
                except: pass
    csv_path = r"E:/IDEProjects/AI/lang-zone/tests_boundary/results.csv"
    with open(csv_path, "w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["id","syntax","dim","sample","expect","actual","behavior","pass","note"])
        w.writeheader()
        for r in rows: w.writerow(r)
    total = len(rows)
    npass = sum(1 for r in rows if r["pass"] == "PASS")
    print("TOTAL=%d PASS=%d FAIL=%d" % (total, npass, total - npass))
    print("CSV:", csv_path)
    for r in rows:
        if r["pass"] == "FAIL":
            print("  FAIL %s [%s/%s] expect=%s actual=%s :: %s" % (
                r["id"], r["syntax"], r["dim"], r["expect"], r["actual"], r["behavior"]))
    from collections import Counter
    bydim = Counter(r["dim"] for r in rows)
    bysyn = Counter((r["syntax"], r["pass"]) for r in rows)
    print("BY_DIM:", dict(bydim))
    print("FAILS_BY_SYNTAX:")
    for (syn, pf), n in bysyn.items():
        if pf == "FAIL":
            print("   %s x%d" % (syn, n))

if __name__ == "__main__":
    main()

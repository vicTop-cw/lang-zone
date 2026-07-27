#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Lang-Zong 编译器测试驱动 — Phase 4 扩展 (try/catch/panic)
==========================================================
基于 v1 harness，新增:
  - mode: "compile" — 验证生成的 .rs 经 rustc 编译成功
  - mode: "run"     — 验证 rustc 编译 + 运行输出含预期子串
  - H01-H12: Phase 4 try/catch/panic 用例

SUT: ../../target/debug/lang-zong.exe
"""

import os
import sys
import json
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.path.join(HERE, "_work")
SUT = os.path.join(HERE, "..", "..", "target", "debug", "lang-zong.exe")
SUT = os.path.abspath(SUT)

LONG_IDENT = "a" * 200

# ============ 原有 39 用例 (F01-F15, B01-B11, G01-G07, E01-E06) ============
CATALOG = [
    # ---------------- 功能 (Functional) ----------------
    dict(id="F01", title="关键字词法化", category="functional", priority="P0", mode="tokens",
         source="def if while for match return let struct enum trait impl import const async spawn with guard yield break continue",
         present=["Def", "If", "While", "For", "Match", "Return", "Let", "Struct", "Enum",
                  "Trait", "Impl", "Import", "Const", "Async", "Spawn", "With", "Guard",
                  "Yield", "Break", "Continue"],
         absent=[], note="核验全部核心关键字被识别为对应 Token。"),

    dict(id="F02", title="进制字面量 0x/0o/0b", category="functional", priority="P0", mode="tokens",
         source="x = 0xFF\ny = 0o17\nz = 0b101",
         present=["IntLit(255)", "IntLit(15)", "IntLit(5)"], absent=["LexError"],
         note="0xFF=255, 0o17=15, 0b101=5。"),

    dict(id="F03", title="操作符词法化", category="functional", priority="P0", mode="tokens",
         source="a + b * c\nd == e\nf && g\nh << i\nj >> k",
         present=["Plus", "Star", "EqEq", "AmpAmp", "Shl", "Shr"], absent=[],
         note="核验算术/比较/位/逻辑操作符 Token。"),

    dict(id="F04", title="字符串种类", category="functional", priority="P0", mode="tokens",
         source='a = "hi"\nb = f"x{y}"\nc = r"raw"\nd = """tri"""',
         present=['StrLit("hi")', 'FStrLit("x{y}")', 'RawStrLit("raw")', 'StrLit("tri")'],
         absent=[], note="普通/f-string/原始/三引号四类字符串词法化。"),

    dict(id="F05", title="函数 AST 构造", category="functional", priority="P0", mode="ast",
         source="def add(a: int, b: int)-> int = a + b",
         present=['name: "add"', "params:", "return_type: Some("], absent=[],
         note="函数名/参数/返回类型进入 AST。"),

    dict(id="F06", title="结构体 AST 构造", category="functional", priority="P0", mode="ast",
         source="struct Point =\n    x: f64\n    y: f64",
         present=['name: "Point"', "StructDef", "fields:"], absent=[],
         note="结构体名与字段进入 AST。"),

    dict(id="F07", title="枚举 AST 构造", category="functional", priority="P0", mode="ast",
         source="enum Color =\n    Red\n    Green\n    Blue",
         present=['name: "Color"', "is_enum: true"], absent=[],
         note="枚举标记 is_enum=true。"),

    dict(id="F08", title="match 模式 AST", category="functional", priority="P0", mode="ast",
         source='def f(x: int)-> str =\n    match x:\n        case 0=> "zero"\n        case _=> "other"',
         present=["Match", "pattern: Int(", "pattern: Ident("], absent=[],
         note="match 表达式与整型/通配符模式进入 AST。"),

    dict(id="F09", title="int? -> Option<i64> 代码生成", category="functional", priority="P0", mode="rust",
         source="def f(x: int?)-> int? = x",
         present=["Option<i64>"], absent=[],
         note="int? 语法糖转译为 Option<i64>。"),

    dict(id="F10", title="f-string -> println! 代码生成", category="functional", priority="P0", mode="rust",
         source='def g(n: int) = print(f"n={n}")',
         present=['println!("n={}", n)'], absent=[],
         note='print(f"...") 降级为 println! 宏。'),

    dict(id="F11", title="管道 |> 代码生成", category="functional", priority="P1", mode="rust",
         source="def double(x: int)-> int = x * 2\ndef h(a: int)-> int = a |> double",
         present=["double(a)"], absent=[],
         note="a |> f 转译为 f(a)。"),

    dict(id="F12", title="安全导航 + 空合并", category="functional", priority="P1", mode="rust",
         source='def k(u: Person?)-> str = u?.name ?? "anon"',
         present=[".map(|x| x.name)", "unwrap_or"], absent=[],
         note="?. -> .map(|x| x.field); ?? -> .unwrap_or。"),

    dict(id="F13", title="guard let 模式守卫", category="functional", priority="P1", mode="rust",
         source='def d(opt: int?)-> str =\n    guard let Some(v) = opt else: "none"\n    v',
         present=['let Some(v) = opt else {', 'return "none".to_string();'], absent=[],
         note="guard let 转译为 Rust let...else。"),

    dict(id="F14", title="owned 契约(合规 ^ 调用)", category="functional", priority="P1", mode="rust",
         source='struct Person =\n    name: str\ndef consume(owned p: Person)-> str = f"{p.name}"\ndef main() =\n    bob = Person(name: "Bob")\n    consume(bob^)',
         present=["consume(bob)"], absent=["compile_error!"],
         note="以 ^ 显式转移所有权时, 不应注入 compile_error。"),

    dict(id="F15", title="owned 契约(违规未 ^)", category="functional", priority="P1", mode="rust",
         source='struct Person =\n    name: str\ndef consume(owned p: Person)-> str = f"{p.name}"\ndef main() =\n    bob = Person(name: "Bob")\n    consume(bob)',
         present=["compile_error!"], absent=[],
         note="未以 ^ 调用 owned 形参时, 应注入编译期错误提示。"),

    # ---------------- 边界 (Boundary) ----------------
    dict(id="B01", title="空输入", category="boundary", priority="P1", mode="tokens",
         source="", present=["Eof"], absent=["Def"], note="空源应得到仅含 Eof 的 Token 流。"),
    dict(id="B02", title="单标识符", category="boundary", priority="P1", mode="tokens",
         source="x", present=['Ident("x")', "Eof"], absent=[], note="裸标识符词法化。"),
    dict(id="B03", title="i64 最大值", category="boundary", priority="P1", mode="tokens",
         source="x = 9223372036854775807", present=["IntLit(9223372036854775807)"], absent=["LexError"],
         note="i64::MAX 应有损解析为 IntLit。"),
    dict(id="B04", title="整数溢出静默归零", category="boundary", priority="P1", mode="tokens",
         source="x = 99999999999999999999999", present=["IntLit(0)"], absent=["LexError"],
         note="超出 i64 范围 -> unwrap_or(0) 静默归零。"),
    dict(id="B05", title="深层嵌套泛型", category="boundary", priority="P1", mode="rust",
         source="def f(x: Dict<str, List<List<int>>>)-> int = 0",
         present=["HashMap<String, Vec<Vec<i64>>>"], absent=[],
         note="Dict/List 多层泛型嵌套映射。"),
    dict(id="B06", title="嵌套泛型 >> 拆分", category="boundary", priority="P1", mode="rust",
         source="def f(x: List<List<int>>)-> int = 0",
         present=["Vec<Vec<i64>>"], absent=[],
         note="List<List<int>> 的 >> 应正确拆分为两个 >。"),
    dict(id="B07", title="深层嵌套表达式", category="boundary", priority="P1", mode="tokens",
         source="(1 + (2 + (3 + (4 + 5))))", present=["LParen", "RParen"], absent=["LexError"],
         note="多层括号嵌套应正确词法化。"),
    dict(id="B08", title="超长标识符(200字符)", category="boundary", priority="P1", mode="tokens",
         source=LONG_IDENT, present=['Ident("aaaaaaaaaaaa'], absent=["LexError"],
         note="200 字符标识符应被接受。"),
    dict(id="B09", title="三引号串公共缩进裁剪", category="boundary", priority="P1", mode="tokens",
         source='s = """\n    line1\n    line2\n    """', present=['StrLit('], absent=["    line1"],
         note="三引号串应去除公共缩进。"),
    dict(id="B10", title="Unicode 字符串", category="boundary", priority="P1", mode="tokens",
         source='x = "你好 🌟 world"', present=['StrLit("你好 🌟 world")'], absent=["LexError"],
         note="非 ASCII 字符应被原样保留。"),
    dict(id="B11", title="浮点字面量边界(含科学计数法)", category="boundary", priority="P1", mode="tokens",
         source="x = 3.14\ny = 1e10\nz = 0.5e-3",
         present=["FloatLit(3.14)", "FloatLit(10000000000.0)", "FloatLit(0.0005)"], absent=["LexError"],
         note="普通/科学计数法浮点解析。"),

    # ---------------- 构建块 (Build Block) ----------------
    dict(id="G01", title="变量构建块 =:", category="buildblock", priority="P1", mode="rust",
         source="def f() =\n    x =:\n        y = 1\n        y",
         present=["let x = (|| unsafe {", "})();"], absent=[],
         note="=: 转译为无参 unsafe 闭包并立即调用。"),
    dict(id="G02", title="调用构建块 ~:", category="buildblock", priority="P1", mode="rust",
         source="def f() =\n    x = make ~:\n        (1, 2)",
         present=["__Pack::Tuple", "unsafe { match __p"], absent=[],
         note="~: 语法; 参数包类型擦除为 __Pack::Tuple。"),
    dict(id="G03", title="生成器构建块 *:", category="buildblock", priority="P1", mode="rust",
         source="def f() =\n    x = make *:\n        yield (1,)\n        yield (2,)",
         present=["let mut __bb: Vec<__Pack>", "IterStopException"], absent=[],
         note="*: 语法; 收集参数包向量并以 IterStopException 收尾。"),
    dict(id="G04", title="顶层构建块(应拒绝)", category="buildblock", priority="P1", mode="error",
         source="x =:\n    y = 1", present=["只能出现在函数体内"], absent=[],
         note="构建块仅允许出现在函数体内。"),
    dict(id="G05", title="生成器缺 yield(应拒绝)", category="buildblock", priority="P1", mode="error",
         source="def f() =\n    x = make *:\n        return (1,)",
         present=["必须至少包含一个 yield"], absent=[],
         note="生成器构建块必须至少含一个 yield。"),
    dict(id="G06", title="变量块含 yield(应拒绝)", category="buildblock", priority="P1", mode="error",
         source="def f() =\n    x =:\n        yield (1,)",
         present=["变量构建块"], absent=[],
         note="变量构建块内部禁止 yield。"),
    dict(id="G07", title="调用块返回值非法(应拒绝)", category="buildblock", priority="P1", mode="error",
         source="def f() =\n    x = make ~:\n        123",
         present=["返回值必须是元组"], absent=[],
         note="调用构建块返回值须满足 BuildParams 约束。"),

    # ---------------- 异常 (Exception) ----------------
    dict(id="E01", title="构建块符号缺空白(LexError)", category="exception", priority="P1", mode="tokens",
         source="x=:2", present=["LexError"], absent=[], note="=: 前后须留白。"),
    dict(id="E02", title="未闭合括号", category="exception", priority="P1", mode="error",
         source="def f() = (", present=["Parse error"], absent=[], note="未闭合括号应优雅报错。"),
    dict(id="E03", title="表达式非法 token", category="exception", priority="P1", mode="error",
         source="def f() = @@@", present=["Unexpected token"], absent=[],
         note="非法 token 应报错。"),
    dict(id="E04", title="函数缺函数体 =", category="exception", priority="P1", mode="error",
         source="def f()", present=["Parse error"], absent=[], note="函数缺 `=` 应报错。"),
    dict(id="E05", title="const 缺 =", category="exception", priority="P1", mode="error",
         source="const X: int 5", present=["Parse error"], absent=[], note="const 缺 `=` 应报错。"),
    dict(id="E06", title="浮点溢出静默归零", category="exception", priority="P1", mode="tokens",
         source="x = 1e400", present=["FloatLit(inf)"], absent=["LexError"],
         note="超出 f64 范围 -> 解析为 +inf 静默通过。"),

    # ═══════════════════════════════════════════════════════
    # Phase 4: Error Handling (try/catch/panic)
    # ═══════════════════════════════════════════════════════
    # H01: panic 表达式 → Rust panic! 宏
    dict(id="H01", title="panic 表达式代码生成", category="errorhandling", priority="P1", mode="rust",
         source='def main() = panic("oops")',
         present=['panic!("{}", "oops"'], absent=[],
         note='panic(msg) → panic!("{{}}", msg)。'),

    # H02: panic with f-string
    dict(id="H02", title="panic + f-string 代码生成", category="errorhandling", priority="P1", mode="rust",
         source='def main() =\n  x = 42\n  panic(f"x={x}")',
         present=['panic!("{}", format!("x={}", x))'], absent=[],
         note='panic(f"...") → panic!("{{}}", format!(...))。'),

    # H03: try/catch basic (Err caught)
    dict(id="H03", title="try/catch 基本 Err 捕获", category="errorhandling", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Err("fail")\n  v = try:\n    r\n  catch e:\n    -1\n  print(v)',
         present=["match {", "Err(e) =>", "Ok(v) => v"], absent=[],
         note="try/catch 转译为 match {body} { Err(pat) => ..., Ok(v) => v }。"),

    # H04: try/catch Ok pass-through
    dict(id="H04", title="try/catch Ok 穿透", category="errorhandling", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Ok(99)\n  v = try:\n    r\n  catch e:\n    -1\n  print(v)',
         present=["Ok(v) => v"], absent=[],
         note="Ok 值经 try/catch 透传。"),

    # H05: try/catch/else branch
    dict(id="H05", title="try/catch/else 分支", category="errorhandling", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Ok(42)\n  v = try:\n    r\n  catch e:\n    -1\n  else:\n    99\n  print(v)',
         present=["Ok(__v) => 99"], absent=[],
         note="else 分支生成 Ok(__v) => handler。"),

    # H06: try/catch with multi-catch (custom enum variants)
    dict(id="H06", title="try/catch 多分支枚举变体", category="errorhandling", priority="P1", mode="rust",
         source="enum E =\n  A\n  B\ndef main() =\n  r: Result<int, E> = Err(E.B)\n  v = try:\n    r\n  catch A():\n    -1\n  catch B():\n    -2\n  print(v)",
         present=["Err(E::A) =>", "Err(E::B) =>"], absent=[],
         note="多 catch 分支生成多个 Err(Enum::Variant) 模式。"),

    # H07: try/catch with guard condition
    dict(id="H07", title="try/catch 带守卫 if", category="errorhandling", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Err("bad")\n  v = try:\n    r\n  catch e if e == "bad":\n    -1\n  catch e:\n    -2\n  print(v)',
         present=['if (e == "bad"'], absent=[],
         note='catch e if cond: 生成 Err(e) if cond =>。'),

    # H08: panic in catch (lz→rust generation)
    dict(id="H08", title="panic in catch 代码生成", category="errorhandling", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Err("boom")\n  v = try:\n    r\n  catch e:\n    panic(f"err: {e}")\n  print(v)',
         present=['panic!("{}", format!("err: {}", e))'], absent=[],
         note="catch 分支内 panic 正确生成。"),

    # H09: try/catch nested
    dict(id="H09", title="嵌套 try/catch", category="errorhandling", priority="P1", mode="rust",
         source='def inner()-> Result<int, str> = Err("inner")\ndef outer()-> Result<int, str> = Ok(100)\ndef main() =\n  v = try:\n    try:\n      outer()\n    catch e:\n      -1\n  catch e:\n    -2\n  print(v)',
         present=["match {", "Err(e) =>"], absent=[],
         note="嵌套 try/catch 各自生成独立 match。"),

    # H10: catch with Result::Err literal pattern
    dict(id="H10", title="catch Err 模式匹配", category="errorhandling", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Err("fail")\n  v = try:\n    r\n  catch Err(msg):\n    -1\n  print(v)',
         present=["Err(Err(msg)) =>"], absent=[],
         note="catch Err(msg): 生成嵌套模式 Err(Err(msg)) 。"),

    # H11: try/catch block body with multiple statements
    dict(id="H11", title="try 块多语句体", category="errorhandling", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Err("fail")\n  v = try:\n    x = 1\n    y = 2\n    Err("fail")\n  catch e:\n    x  # x = 1\n  print(v)',
         present=["let mut x = 1;", "Err(e) =>"], absent=[],
         note="try 块体内多语句正确生成。"),

    # H12: try/catch/else with explicit type on Ok path
    dict(id="H12", title="try/catch/else Ok 路径执行", category="errorhandling", priority="P1", mode="rust",
         source='def f()-> Result<int, str> = Ok(200)\ndef main() =\n  v = try:\n    f()\n  catch e:\n    -1\n  else:\n    99\n  print(v)',
         present=["Ok(__v) => 99"], absent=[],
         note="else 分支在 Ok 路径触发。"),

    # ═══════════════════════════════════════════════════════
    # Phase 5: defer 延迟执行 (Drop 守卫)
    # ═══════════════════════════════════════════════════════
    # D01: defer 基本语法 → DeferGuard 代码生成
    dict(id="D01", title="defer 基本代码生成", category="defer", priority="P1", mode="rust",
         source='def main() =\n  defer:\n    print("bye")\n  print("hello")',
         present=["DeferGuard", "Some(||"], absent=[],
         note="defer: 生成 DeferGuard(Some(|| { ... })) Drop 守卫。"),

    # D02: defer 单行内联形式
    dict(id="D02", title="defer 内联语法", category="defer", priority="P1", mode="rust",
         source='def main() =\n  defer: print("cleanup")\n  print("work")',
         present=["DeferGuard", 'println!("cleanup")'], absent=[],
         note="defer: expr 内联形式正确生成。"),

    # D03: 多 defer LIFO 顺序
    dict(id="D03", title="defer 多守卫 LIFO 顺序", category="defer", priority="P1", mode="rust",
         source='def main() =\n  defer: print("A")\n  defer: print("B")\n  defer: print("C")\n  print("START")',
         present=["__defer_0", "__defer_1", "__defer_2"], absent=[],
         note="多 defer 生成多个独立守卫，Rust 逆序 drop 实现 LIFO。"),

    # D04: defer 在 return 前执行
    dict(id="D04", title="defer + return 交互", category="defer", priority="P1", mode="rust",
         source='def f()-> int =\n  defer: print("clean")\n  return 1\ndef main() =\n  v = f()\n  print(v)',
         present=["DeferGuard", "return 1"], absent=[],
         note="return 触发 Drop → defer 体在 return 前执行。"),

    # D05: defer 在 guard 返回时执行
    dict(id="D05", title="defer + guard 交互", category="defer", priority="P1", mode="rust",
         source='def f(n: int)-> int =\n  defer: print("clean")\n  guard n != 0 else: -1\n  n * 2\ndef main() =\n  print(f(5))\n  print(f(0))',
         present=["DeferGuard", 'println!("clean")'], absent=[],
         note="guard 提前返回时 defer 守卫仍通过 Drop 执行。"),

    # ═══════════════════════════════════════════════════════
    # Phase 5b: finally 子句 (try/catch/finally)
    # ═══════════════════════════════════════════════════════
    # J01: try/catch/finally 基本 — 带 finally 的 Rust 输出
    dict(id="J01", title="finally 基本代码生成", category="finally", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Ok(42)\n  v = try:\n    r\n  catch e:\n    -1\n  finally:\n    print("CLEANUP")\n  print(v)',
         present=["__try_result", 'println!("CLEANUP")', "__try_result"], absent=[],
         note="finally 生成块 { let __try = match {..}; finally_body; __try }。"),

    # J02: finally 在 Ok 路径执行
    dict(id="J02", title="finally Ok 路径生成", category="finally", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Ok(42)\n  v = try:\n    r\n  catch e:\n    -1\n  else:\n    99\n  finally:\n    print("END")\n  print(v)',
         present=['__try_result', 'Ok(__v) => 99', 'println!("END")'], absent=[],
         note="else+finally 共存：else 在 match 内，finally 在 match 之外。"),

    # J03: finally 在 Err 路径执行
    dict(id="J03", title="finally Err 路径生成", category="finally", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Err("fail")\n  v = try:\n    r\n  catch e:\n    print(f"err: {e}")\n    -1\n  finally:\n    print("CLEANUP")\n  print(v)',
         present=['__try_result', 'Err(e) =>', 'println!("CLEANUP")'], absent=[],
         note="Err 路径下 finally 仍在 match 之后执行。"),

    # J04: 纯 finally（无 else）
    dict(id="J04", title="finally 无 else", category="finally", priority="P1", mode="rust",
         source='def main() =\n  r: Result<int, str> = Err("oops")\n  v = try:\n    r\n  catch e:\n    -1\n  finally:\n    print("done")\n  print(v)',
         present=['__try_result', 'println!("done")', 'Err(e) =>'], absent=[],
         note="仅 catch+finally 的情况下生成正确。"),
]


# ============ 测试执行引擎 ============

def run_case(case):
    """执行单个用例, 返回结果字典。支持 compile/run 新模式。"""
    cid = case["id"]
    mode = case["mode"]
    lz_path = os.path.join(WORK, cid + ".lz")
    with open(lz_path, "w", encoding="utf-8") as fh:
        fh.write(case["source"])

    args = [SUT, cid + ".lz"]
    if mode == "tokens":
        args.append("--tokens")
    elif mode == "ast":
        args.append("--ast")

    proc = subprocess.run(args, cwd=WORK, capture_output=True, text=True, timeout=30)
    rc = proc.returncode
    out = proc.stdout
    err = proc.stderr
    combined = out + err

    # 生成产物
    rs_path = os.path.join(WORK, cid + ".rs")
    rs_text = ""
    if os.path.exists(rs_path):
        with open(rs_path, "r", encoding="utf-8") as fh:
            rs_text = fh.read()

    # 选择断言目标文本
    if mode == "rust" or mode == "compile" or mode == "run":
        target = rs_text
    elif mode == "error":
        target = combined
    else:
        target = out

    present = case.get("present", [])
    absent = case.get("absent", [])
    problems = []

    # panic 检测
    if rc == 101 or "panicked" in err:
        return dict(id=cid, title=case["title"], category=case["category"],
                    priority=case["priority"], mode=mode, rc=rc,
                    status="CRASH", problems=["进程 panic (退出码 101)"],
                    stdout=out, stderr=err, rs=rs_text)

    # 退出码预期
    if mode == "error":
        if rc != 1:
            problems.append(f"错误用例期望退出码 1, 实际 {rc}")
    else:
        if rc != 0:
            problems.append(f"正常用例期望退出码 0, 实际 {rc}")

    # 子串断言 (compile/run 模式也在 .rs 中检查)
    for p in present:
        if p not in target:
            problems.append(f"缺少预期子串: {p!r}")
    for a in absent:
        if a in target:
            problems.append(f"出现不应存在的子串: {a!r}")

    # compile/run 模式: 实际编译
    compile_ok = True
    run_output = ""
    run_rc = 0
    if (mode == "compile" or mode == "run") and rc == 0 and not problems:
        exe_path = os.path.join(WORK, cid + ".exe")
        rc_proc = subprocess.run(
            ["rustc", "-o", exe_path, rs_path],
            cwd=WORK, capture_output=True, text=True, timeout=60)
        if rc_proc.returncode != 0:
            problems.append(f"rustc 编译失败:\n{rc_proc.stderr[:500]}")
            compile_ok = False

        if compile_ok and mode == "run":
            run_proc = subprocess.run([exe_path], cwd=WORK,
                                      capture_output=True, text=True, timeout=10)
            run_output = run_proc.stdout
            run_rc = run_proc.returncode
            expected_out = case.get("expected_stdout", "")
            if expected_out and expected_out not in run_output:
                problems.append(f"运行输出不匹配, 期望含: {expected_out!r}, 实际: {run_output.strip()!r}")
            if run_rc != 0:
                problems.append(f"运行退出码非 0: {run_rc}")

    status = "PASS" if not problems else "FAIL"
    return dict(id=cid, title=case["title"], category=case["category"],
                priority=case["priority"], mode=mode, rc=rc,
                status=status, problems=problems,
                stdout=out, stderr=err, rs=rs_text,
                run_output=run_output, run_rc=run_rc)


def main():
    os.makedirs(WORK, exist_ok=True)
    if not os.path.exists(SUT):
        print(f"[FATAL] SUT 不存在: {SUT}", file=sys.stderr)
        sys.exit(2)

    print(f"SUT: {SUT}")
    print(f"用例数: {len(CATALOG)}\n")

    results = []
    for case in CATALOG:
        r = run_case(case)
        results.append(r)
        mark = {"PASS": "✅", "FAIL": "❌", "CRASH": "💥"}.get(r["status"], "?")
        print(f"  {mark} {r['id']:>4} [{r['priority']}] {r['title']}  (rc={r['rc']})")
        for p in r["problems"]:
            print(f"         - {p}")

    # 统计
    total = len(results)
    passed = sum(1 for r in results if r["status"] == "PASS")
    failed = sum(1 for r in results if r["status"] == "FAIL")
    crashed = sum(1 for r in results if r["status"] == "CRASH")

    by_cat = {}
    by_pri = {}
    for r in results:
        c = r["category"]
        p = r["priority"]
        by_cat.setdefault(c, {"total": 0, "pass": 0})
        by_pri.setdefault(p, {"total": 0, "pass": 0})
        by_cat[c]["total"] += 1
        by_pri[p]["total"] += 1
        if r["status"] == "PASS":
            by_cat[c]["pass"] += 1
            by_pri[p]["pass"] += 1

    print("\n================ 汇总 ================")
    print(f"总数={total}  通过={passed}  失败={failed}  崩溃={crashed}")
    print(f"总通过率: {passed/total*100:.1f}%")
    print("\n按类别:")
    for c, v in by_cat.items():
        print(f"  {c:<16} {v['pass']}/{v['total']}  ({v['pass']/v['total']*100:.1f}%)")
    print("\n按优先级:")
    for p in sorted(by_pri.keys()):
        v = by_pri[p]
        print(f"  {p:<4} {v['pass']}/{v['total']}  ({v['pass']/v['total']*100:.1f}%)")

    # 写结果数据
    with open(os.path.join(WORK, "results.json"), "w", encoding="utf-8") as fh:
        json.dump(dict(total=total, passed=passed, failed=failed, crashed=crashed,
                       by_cat=by_cat, by_pri=by_pri, results=results),
                  fh, ensure_ascii=False, indent=2)
    print(f"\n结果已写入: {os.path.join(WORK, 'results.json')}")


if __name__ == "__main__":
    main()

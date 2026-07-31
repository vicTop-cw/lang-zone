# -*- coding: utf-8 -*-
"""
Lang-Zong test-suite — Phase 1: lz test 语法全量覆盖
=====================================================
覆盖：test/assert/check/suite 所有语法变体：
- assert bool / assert == / assert != / 带消息 / 不带消息
- check 软断言（同 assert 变体）
- test 块（命名/匿名/含多个断言/含复合语句）
- suite 嵌套（单层/多层/混合 suite+test）
- codegen 内容校验（rust 模式）
- 运行时验证（run 模式）
"""
import os, json, subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.path.join(HERE, "_work")
SUT = os.path.abspath(os.path.join(HERE, "..", "..", "target", "debug", "lang-zone.exe"))
STD_DIR = os.path.join(HERE, "..", "..", "std")

CATALOG = [
    # ============ 1. test 基础 ============
    dict(id="T01", title="test 命名+body", category="test", priority="P0", mode="run",
         source='def f() -> int = 42\ntest "simple":\n  assert f() == 42\n  print("ok")\n',
         present=["ok"], absent=[],
         note="test 块命名 + assert == 编译运行通过。"),

    dict(id="T02", title="test 多项 assert", category="test", priority="P0", mode="run",
         source='def k(x: int) -> int = x\ntest multi:\n  assert k(1) == 1\n  assert k(2) == 2\n  assert k(3) == 3\n  print("multi-ok")\n',
         present=["multi-ok"], absent=[],
         note="一个 test 块含多个 assert。"),

    dict(id="T03", title="test 内复合语句（let + print）", category="test", priority="P1", mode="run",
         source='test "composite":\n  x = 10\n  y = x + 5\n  assert y == 15\n  print("composite-ok")\n',
         present=["composite-ok"], absent=[],
         note="test 体内使用 let/赋值 + assert + print。"),

    # ============ 2. assert 变体 ============
    dict(id="TA1", title="assert bool 表达式", category="assert", priority="P0", mode="rust",
         source='test "a1":\n  assert True\n  assert 1 == 1\n',
         present=["assert!(true)", "assert_eq!(1, 1)"], absent=[],
         note="assert bool → assert!(); assert == → assert_eq!()。"),

    dict(id="TA2", title="assert != 变体", category="assert", priority="P1", mode="rust",
         source='test "a2":\n  assert 1 != 2\n',
         present=["(!2)"], absent=["assert_ne!"],
         note="assert != → assert_eq!(left, (!right))，非 assert_ne!。"),

    dict(id="TA3", title="assert 带字符串消息", category="assert", priority="P1", mode="rust",
         source='test "a3":\n  assert 1 == 2, "one is not two"\n',
         present=['assert_eq!(1, 2, "one is not two")'], absent=[],
         note="assert == 带消息。"),

    dict(id="TA4", title="assert bool 带消息", category="assert", priority="P1", mode="rust",
         source='test "a4":\n  assert False, "should be true"\n',
         present=['assert!(false, "should be true")'], absent=[],
         note="assert bool 带消息。"),

    dict(id="TA5", title="assert != 带消息", category="assert", priority="P1", mode="rust",
         source='test "a5":\n  assert 1 != 2, "one is not two"\n',
         present=['assert_eq!(1, (!2), "one is not two")'], absent=["assert_ne!"],
         note="assert != 带消息 生成 assert_eq!(left, (!right), msg)。"),

    dict(id="TA6", title="assert 比较变量", category="assert", priority="P1", mode="run",
         source='def add(a: int, b: int) -> int = a + b\ntest "var_eq":\n  assert add(2, 3) == 5\n  print("var-eq-ok")\n',
         present=["var-eq-ok"], absent=[],
         note="assert 比较函数调用结果。"),

    # ============ 3. check 软断言 ============
    dict(id="TC1", title="check 通过(成功)", category="check", priority="P1", mode="rust",
         source='test "c1":\n  check 1 == 1\n',
         present=["eprintln!", "[check]"], absent=[],
         note="check 生成 if 守卫 + eprintln!，不终止。"),

    dict(id="TC2", title="check 失败不终止", category="check", priority="P1", mode="run",
         source='test "c2":\n  check 1 != 1\n  print("survived")\n',
         present=["survived"], absent=[],
         note="check 失败不阻止后续执行。"),

    dict(id="TC3", title="check 带消息", category="check", priority="P1", mode="rust",
         source='test "c3":\n  check 1 == 2, "one is not two"\n',
         present=['eprintln!', "one is not two"], absent=[],
         note="check 带消息。"),

    dict(id="TC4", title="check == 变体", category="check", priority="P1", mode="rust",
         source='test "c4":\n  check True == True\n',
         present=["eprintln!"], absent=[],
         note="check == 变体。"),

    dict(id="TC5", title="check != 变体", category="check", priority="P1", mode="rust",
         source='test "c5":\n  check 1 != 2\n',
         present=["(!2)"] , absent=["assert_ne!"],
         note="check != 同样用 (!right) 而非 assert_ne!。"),

    # ============ 4. suite 嵌套 ============
    dict(id="TS1", title="suite 单层", category="suite", priority="P1", mode="rust",
         source='suite "s1":\n  test "t1":\n    assert True\n',
         present=["mod s1", "#[test]", "fn t1"], absent=[],
         note="suite 生成 mod 块。"),

    dict(id="TS2", title="suite 嵌套 suite", category="suite", priority="P1", mode="rust",
         source='suite "outer":\n  suite "inner":\n    test "deep":\n      assert True\n',
         present=["mod outer", "mod inner", "#[test]", "fn deep"], absent=[],
         note="suite 内嵌 suite 生成嵌套 mod。"),

    dict(id="TS3", title="suite 含多个 test", category="suite", priority="P1", mode="rust",
         source='suite "multi":\n  test "t1":\n    assert True\n  test "t2":\n    assert 1 == 1\n',
         present=["mod multi", "fn t1", "fn t2"], absent=[],
         note="suite 内含多个 test。"),

    dict(id="TS4", title="suite + test 混合运行", category="suite", priority="P1", mode="run",
         source='def h() -> int = 10\nsuite "math":\n  test "add":\n    assert h() + 1 == 11\n    print("suite-run-passed")\n',
         present=["suite-run-passed"], absent=[],
         note="suite 内 test 实际编译并运行。"),

    dict(id="TS5", title="suite 多层嵌套运行", category="suite", priority="P1", mode="run",
         source='suite "level1":\n  suite "level2":\n    test "leaf":\n      assert 99 == 99\n      print("nested-run-ok")\n',
         present=["nested-run-ok"], absent=[],
         note="多层嵌套 suite 运行时正确执行。"),

    # ============ 5. 混合场景 ============
    dict(id="TM1", title="assert + check 混用", category="mixed", priority="P1", mode="run",
         source='test "mixed":\n  x = 5\n  assert x == 5\n  check x > 0, "x positive"\n  print("mixed-ok")\n',
         present=["mixed-ok"], absent=[],
         note="同一 test 块内 assert + check 混用。"),

    dict(id="TM2", title="suite 内 assert/check/print 混用", category="mixed", priority="P1", mode="run",
         source='def cube(x: int) -> int = x * x * x\nsuite "geometry":\n  test "test_cube":\n    assert cube(3) == 27\n    check cube(0) == 0, "zero case"\n    print("geometry-ok")\n',
         present=["geometry-ok"], absent=[],
         note="suite 内 test 含 assert + check + print（test 名不可与函数名冲突）。"),

    dict(id="TM3", title="多个独立 test 块", category="mixed", priority="P1", mode="run",
         source='test "first":\n  assert 1 + 1 == 2\n  print("first-ok")\ntest "second":\n  assert 2 * 2 == 4\n  print("second-ok")\ntest "third":\n  assert 9 / 3 == 3\n  print("third-ok")\n',
         present=["first-ok", "second-ok", "third-ok"], absent=[],
         note="三个独立 test 块依次执行。"),

    # ============ 6. 函数外 ============
    dict(id="TX1", title="顶层 test 引用全局函数", category="test", priority="P0", mode="run",
         source='def twice(x: int) -> int = x * 2\ntest "global_fn":\n  assert twice(21) == 42\n  print("global-fn-ok")\n',
         present=["global-fn-ok"], absent=[],
         note="test 块引用模块级函数。"),

    dict(id="TX2", title="suite 引用全局变量", category="suite", priority="P1", mode="rust",
         source='const ANSWER: int = 42\nsuite "const":\n  test "answer":\n    assert True\n',
         present=["const ANSWER", "mod const"], absent=[],
         note="suite 旁有 const 定义，codegen 正确。"),
]

os.makedirs(WORK, exist_ok=True)
results = {"total": 0, "passed": 0, "failed": 0, "crashed": 0, "by_cat": {}}
results_list = []

for t in CATALOG:
    tid = t["id"]
    src = os.path.join(WORK, f"{tid}.lz")
    out_rs = os.path.join(WORK, f"{tid}.rs")
    out_exe = os.path.join(WORK, f"{tid}.exe")

    with open(src, "w", encoding="utf-8") as f:
        f.write(t["source"])

    # Step 1: lz -> rs
    rc = subprocess.run([SUT, src, "--std-dir", STD_DIR],
                        capture_output=True, text=True, timeout=30)
    record = dict(id=tid, title=t["title"], category=t["category"], priority=t["priority"],
                  mode=t["mode"], rc=rc.returncode, status="PASS", problems=[], stdout=rc.stdout, stderr=rc.stderr)

    if rc.returncode != 0:
        record["status"] = "FAIL"
        record["problems"].append(f"lz compile failed (rc={rc.returncode})")
        record["problems"].append(rc.stderr.strip())
        results["crashed"] += 1
        results_list.append(record)
        continue

    # Step 2: check generated .rs
    if t["mode"] in ("rust", "run"):
        if os.path.exists(out_rs):
            with open(out_rs, encoding="utf-8") as f:
                rs = f.read()
            for p in t.get("present", []):
                if p not in rs:
                    record["problems"].append(f"缺少预期子串: '{p}'")
            for a in t.get("absent", []):
                if a in rs:
                    record["problems"].append(f"存在禁止子串: '{a}'")

    # Step 3: rustc compile (with --test for test mode)
    if t["mode"] in ("run",) and not record["problems"]:
        if os.path.exists(out_rs):
            rustc_args = ["rustc", "--edition", "2021", "--test", out_rs, "-o", out_exe]
            compile_rc = subprocess.run(rustc_args, capture_output=True, text=True, timeout=30)
            if compile_rc.returncode != 0:
                record["problems"].append(f"rustc compile failed: {compile_rc.stderr.strip()[:300]}")

    # Step 4: run the test binary (with --nocapture so print output is visible)
    if t["mode"] == "run" and not record["problems"]:
        if os.path.exists(out_exe):
            run_rc = subprocess.run([out_exe, "--nocapture"], capture_output=True, text=True, timeout=15)
            record["run_rc"] = run_rc.returncode
            record["run_output"] = run_rc.stdout
            record["run_stderr"] = run_rc.stderr
            for p in t.get("present", []):
                if p not in run_rc.stdout and p not in run_rc.stderr:
                    record["problems"].append(f"运行输出缺少: '{p}'")
            if run_rc.returncode != 0:
                record["problems"].append(f"test binary exit={run_rc.returncode}: {run_rc.stdout[:200]} {run_rc.stderr[:200]}")

    if record["problems"]:
        record["status"] = "FAIL"
        results["failed"] += 1
    else:
        results["passed"] += 1

    results_list.append(record)

results["total"] = len(CATALOG)
for t in results_list:
    cat = t["category"]
    results["by_cat"].setdefault(cat, {"total": 0, "passed": 0})
    results["by_cat"][cat]["total"] += 1
    if t["status"] == "PASS":
        results["by_cat"][cat]["passed"] += 1

# Report
for t in results_list:
    icon = "✅" if t["status"] == "PASS" else "❌"
    print(f"  {icon}  {t['id']} [{t['priority']}] {t['title']}  (mode={t['mode']})")
    for p in t.get("problems", []):
        print(f"         - {p}")

print(f"\n按优先级:")
priorities = {}
for t in results_list:
    p = t["priority"]
    priorities.setdefault(p, {"total": 0, "passed": 0})
    priorities[p]["total"] += 1
    if t["status"] == "PASS":
        priorities[p]["passed"] += 1
for p in sorted(priorities):
    d = priorities[p]
    print(f"  {p}   {d['passed']}/{d['total']}  ({d['passed']/d['total']*100:.1f}%)")

with open(os.path.join(WORK, "results.json"), "w", encoding="utf-8") as f:
    json.dump(dict(total=results["total"], passed=results["passed"], failed=results["failed"],
                   crashed=results["crashed"], by_cat=results["by_cat"], results=results_list), f, indent=2)

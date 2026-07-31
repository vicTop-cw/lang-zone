#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Lang-Zong test-suite — Phase 5: test 框架加固 + check 关键字
============================================================
验证：assert/check 代码生成正确性 + rustc 编译 + 运行输出
"""
import os, json, subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.path.join(HERE, "_work")
SUT = os.path.join(HERE, "..", "..", "target", "debug", "lang-zone.exe")
SUT = os.path.abspath(SUT)

CATALOG = [
    # ── Test 基础 ──
    dict(id="T01", title="test+assert 编译��行", category="test_framework", priority="P0", mode="run",
         source='def f() -> int = 42\ntest "add works":\n  assert f() == 42\n  print("ok")\n',
         present=[], absent=[],
         note="assert_eq! 在测试函数中运行并通过。"),

    dict(id="T02", title="test 多项 assert 编译运行", category="test_framework", priority="P0", mode="run",
         source='def k(x: int) -> int = x\ntest multi:\n  assert k(1) == 1\n  assert k(2) == 2\n  assert k(3) == 3\n  print("multi-ok")\n',
         present=[], absent=[],
         note="多项 assert_eq! 全部正常。"),

    # ── check 关键字 ──
    dict(id="T03", title="check 软断言(通过)", category="test_framework", priority="P1", mode="rust",
         source='def g() -> int = 1\ntest "check pass":\n  check g() == 1\n',
         present=["eprintln!", "[check]"], absent=[],
         note="check 生成 if 守卫 + eprintln!。"),

    dict(id="T04", title="check 运行不终止", category="test_framework", priority="P1", mode="run",
         source='test "check_soft":\n  check 1 != 1\n  print("survived")\n',
         present=[], absent=[],
         note="check 失败不终止执行。"),

    # ── assert 断言消息 ──
    dict(id="T05", title="assert 带消息", category="test_framework", priority="P1", mode="rust",
         source='test "assert msg":\n  assert 1 == 2, "one is not two"\n',
         present=['assert_eq!(1, 2, "one is not two")'], absent=[],
         note="assert expr, msg 生成 assert_eq! 带消息。"),

    dict(id="T06", title="assert bool 带消息", category="test_framework", priority="P1", mode="rust",
         source='test "bool msg":\n  assert False, "should be true"\n',
         present=['assert!(false, "should be true")'], absent=[],
         note="assert expr, msg (无比较) 生成 assert! 带消息。"),

    # ── suite 嵌套 ──
    dict(id="T07", title="suite 嵌套", category="test_framework", priority="P1", mode="rust",
         source='suite "outer":\n  suite "inner":\n    test "nested":\n      assert True\n',
         present=["mod outer", "mod inner", "#[test]", "fn nested"], absent=[],
         note="suite 内嵌 suite 生成嵌套 mod。"),

    dict(id="T08", title="suite+run 编译执行", category="test_framework", priority="P1", mode="run",
         source='def h() -> int = 10\nsuite "math":\n  test "add":\n    assert h() + 1 == 11\n    print("passed")\n',
         present=["passed"], absent=[],
         note="suite 内 test 实际编译并运行。"),
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
    rc = subprocess.run([SUT, src, "--std-dir", os.path.join(HERE, "..", "..", "std")],
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
    if t["mode"] in ("rust", "compile", "run"):
        if os.path.exists(out_rs):
            with open(out_rs, encoding="utf-8") as f:
                rs = f.read()
            for p in t.get("present", []):
                if p not in rs:
                    record["problems"].append(f"缺少预期子串: '{p}'")
            for a in t.get("absent", []):
                if a in rs:
                    record["problems"].append(f"存在禁止子串: '{a}'")

    # Step 3: rustc compile
    if t["mode"] in ("compile", "run") and not record["problems"]:
        if os.path.exists(out_rs):
            rustc_args = ["rustc", "--edition", "2021"]
            if t["mode"] == "run":
                rustc_args.append("--test")  # 测试模式需要 --test 以生成 test harness + main
            rustc_args.extend([out_rs, "-o", out_exe])
            compile_rc = subprocess.run(rustc_args, capture_output=True, text=True, timeout=30)
            if compile_rc.returncode != 0:
                record["problems"].append(f"rustc compile failed: {compile_rc.stderr.strip()[:200]}")

    # Step 4: run
    if t["mode"] == "run" and not record["problems"]:
        if os.path.exists(out_exe):
            run_rc = subprocess.run([out_exe], capture_output=True, text=True, timeout=10)
            record["run_rc"] = run_rc.returncode
            record["run_output"] = run_rc.stdout
            for p in t.get("present", []):
                if p not in run_rc.stdout:
                    record["problems"].append(f"运行输出缺少: '{p}'")

    if record["problems"]:
        record["status"] = "FAIL"
        results["failed"] += 1
    else:
        results["passed"] += 1

    results_list.append(record)

results["total"] = len(CATALOG)
for t in results_list:
    cat = t["category"]
    if cat not in results["by_cat"]:
        results["by_cat"][cat] = {"total": 0, "passed": 0}
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
    if p not in priorities:
        priorities[p] = {"total": 0, "passed": 0}
    priorities[p]["total"] += 1
    if t["status"] == "PASS":
        priorities[p]["passed"] += 1
for p in sorted(priorities):
    d = priorities[p]
    print(f"  {p}   {d['passed']}/{d['total']}  ({d['passed']/d['total']*100:.1f}%)")

with open(os.path.join(WORK, "results.json"), "w", encoding="utf-8") as f:
    json.dump(dict(total=results["total"], passed=results["passed"], failed=results["failed"],
                   crashed=results["crashed"], by_cat=results["by_cat"], results=results_list), f, indent=2)

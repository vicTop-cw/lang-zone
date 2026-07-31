# -*- coding: utf-8 -*-
"""
Lang-Zong test-suite — Phase 2: lz test 自测试
======================================================
使用 lz 自身 test/suite/assert/check 语法测试 lz 编译器。
流程：lz .lz → .rs → rustc --test → .exe → run --nocapture
"""
import os, json, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
CASE_DIR = os.path.join(HERE, "cases")
SUT = os.path.abspath(os.path.join(HERE, "..", "..", "target", "debug", "lang-zone.exe"))
STD_DIR = os.path.join(HERE, "..", "..", "std")

results = {"total": 0, "passed": 0, "failed": 0, "crashed": 0}
results_list = []

for f in sorted(os.listdir(CASE_DIR)):
    if not f.endswith(".lz"):
        continue
    case_id = f[:-3]
    src = os.path.join(CASE_DIR, f)
    rs_file = src[:-3] + ".rs"
    exe_file = src[:-3] + ".exe"

    print(f"\n--- {case_id} ---")

    # Step 1: lz -> rs
    rc = subprocess.run([SUT, src, "--std-dir", STD_DIR],
                        capture_output=True, text=True, timeout=30)

    if rc.returncode != 0:
        print(f"  ❌ lz compile FAILED (rc={rc.returncode})")
        print(f"     {rc.stderr.strip()[:200]}")
        results["crashed"] += 1
        results["total"] += 1
        results_list.append(dict(id=case_id, status="CRASHED", error=rc.stderr.strip()[:200]))
        continue

    print(f"  ✅ lz compile OK")

    # Step 2: rustc --test -> exe
    if not os.path.exists(rs_file):
        print(f"  ❌ .rs file not found!")
        results["crashed"] += 1
        results["total"] += 1
        continue

    compile_rc = subprocess.run(
        ["rustc", "--edition", "2021", "--test", rs_file, "-o", exe_file],
        capture_output=True, text=True, timeout=30
    )

    if compile_rc.returncode != 0:
        errors = [l for l in compile_rc.stderr.split("\n") if "error[" in l or "error:" in l]
        err_msg = "; ".join(e.strip()[:100] for e in errors[:3])
        print(f"  ❌ rustc compile FAILED: {err_msg}")
        results["failed"] += 1
        results["total"] += 1
        results_list.append(dict(id=case_id, status="FAIL", phase="rustc", error=err_msg))
        continue

    print(f"  ✅ rustc compile OK")

    # Step 3: run --nocapture
    if not os.path.exists(exe_file):
        print(f"  ❌ .exe not found!")
        results["crashed"] += 1
        results["total"] += 1
        continue

    run_rc = subprocess.run([exe_file, "--nocapture"], capture_output=True, text=True, timeout=30)

    stdout = run_rc.stdout or ""
    stderr = run_rc.stderr or ""
    combined = stdout + "\n" + stderr

    # Check for test result in output
    passed_count = 0
    failed_count = 0
    for line in combined.split("\n"):
        if "test result:" in line:
            parts = line.split()
            for i, p in enumerate(parts):
                if p == "passed;" and i > 0:
                    try: passed_count = int(parts[i-1])
                    except: pass
                if p == "failed;" and i > 0:
                    try: failed_count = int(parts[i-1])
                    except: pass

    if run_rc.returncode == 0 and failed_count == 0:
        print(f"  ✅ Run: {passed_count} passed, 0 failed")
        results["passed"] += 1
        results_list.append(dict(id=case_id, status="PASS", tests=passed_count))
    else:
        test_summary = f"{passed_count} passed, {failed_count} failed (rc={run_rc.returncode})"
        print(f"  ❌ Run FAILED: {test_summary}")
        for line in combined.split("\n"):
            if "FAILED" in line or "panicked" in line:
                print(f"     {line.strip()[:120]}")
        results["failed"] += 1
        results_list.append(dict(id=case_id, status="FAIL", phase="run",
                                 error=test_summary, detail=combined[:500]))

    results["total"] += 1

# ── Summary ──
print("\n" + "=" * 60)
print(f"Phase 2 结果: {results['passed']}/{results['total']} 通过")
if results["failed"]:
    print(f"  失败: {results['failed']}")
if results["crashed"]:
    print(f"  崩溃: {results['crashed']}")
print(f"  通过率: {results['passed']/results['total']*100:.1f}%")

with open(os.path.join(HERE, "results.json"), "w") as f:
    json.dump(results, f, indent=2)

# -*- coding: utf-8 -*-
"""批量运行 lz/_tests/ 下所有 .lz 测试文件，收集结果"""
import os, subprocess, sys, json
from datetime import datetime

HERE = os.path.dirname(os.path.abspath(__file__))
TEST_DIR = os.path.join(HERE, "_tests")
SUT = os.path.abspath(os.path.join(HERE, "..", "..", "target", "debug", "lang-zong.exe"))
STD_DIR = os.path.abspath(os.path.join(HERE, "..", "..", "std"))

results = {"total": 0, "lz_pass": 0, "lz_fail": 0, "rustc_pass": 0, "rustc_fail": 0, "run_pass": 0, "run_fail": 0}
details = []

for f in sorted(os.listdir(TEST_DIR)):
    if not f.endswith(".lz"):
        continue
    case_id = f[:-3]
    src = os.path.join(TEST_DIR, f)
    rs_file = src[:-3] + ".rs"
    exe_file = src[:-3] + ".exe"

    entry = {"file": f, "lz": "SKIP", "rustc": "SKIP", "run": "SKIP", "error": ""}
    results["total"] += 1

    # Step 1: lz -> rs
    rc = subprocess.run([SUT, src, "--std-dir", STD_DIR],
                        capture_output=True, text=True, timeout=60)
    if rc.returncode != 0:
        entry["lz"] = "FAIL"
        entry["error"] = rc.stderr.strip()[:300]
        results["lz_fail"] += 1
        details.append(entry)
        continue
    entry["lz"] = "PASS"
    results["lz_pass"] += 1

    # Step 2: rustc --test
    if not os.path.exists(rs_file):
        entry["error"] = "rs file not generated"
        results["lz_fail"] += 1
        details.append(entry)
        continue

    rc2 = subprocess.run(
        ["rustc", "--edition", "2021", "--test", rs_file, "-o", exe_file],
        capture_output=True, text=True, timeout=60
    )
    if rc2.returncode != 0:
        errors = [l.strip() for l in rc2.stderr.split("\n") if "error[" in l or "error:" in l]
        entry["rustc"] = "FAIL"
        entry["error"] = "; ".join(errors[:3])[:400]
        results["rustc_fail"] += 1
        details.append(entry)
        continue
    entry["rustc"] = "PASS"
    results["rustc_pass"] += 1

    # Step 3: run
    if not os.path.exists(exe_file):
        entry["error"] = "exe not found"
        results["rustc_fail"] += 1
        details.append(entry)
        continue

    rc3 = subprocess.run([exe_file, "--nocapture"], capture_output=True, text=True, timeout=30)
    combined = (rc3.stdout or "") + "\n" + (rc3.stderr or "")

    passed = 0
    failed = 0
    for line in combined.split("\n"):
        if "test result:" in line:
            parts = line.split()
            for i, p in enumerate(parts):
                if p == "passed;" and i > 0:
                    try: passed = int(parts[i-1])
                    except: pass
                if p == "failed;" and i > 0:
                    try: failed = int(parts[i-1])
                    except: pass

    if rc3.returncode == 0 and failed == 0:
        entry["run"] = f"PASS ({passed} tests)"
        results["run_pass"] += 1
    else:
        entry["run"] = f"FAIL ({passed}p/{failed}f)"
        entry["error"] = combined[:400]
        results["run_fail"] += 1
    details.append(entry)

# ── Summary ──
print("=" * 70)
print(f"LZ 测试批量运行结果 — {datetime.now().strftime('%Y-%m-%d %H:%M')}")
print("=" * 70)

# Group by status
lz_fail = [(d["file"], d["error"]) for d in details if d["lz"] == "FAIL"]
rustc_fail = [(d["file"], d["error"]) for d in details if d["rustc"] == "FAIL"]
run_fail = [(d["file"], d["error"]) for d in details if "FAIL" in str(d.get("run", ""))]
all_pass = [d["file"] for d in details if d["lz"] == "PASS" and d["rustc"] == "PASS" and "PASS" in str(d.get("run", ""))]

print(f"\n总计: {results['total']} 个测试文件")
print(f"  LZ 编译: {results['lz_pass']} 通过, {results['lz_fail']} 失败")
print(f"  Rustc 编译: {results['rustc_pass']} 通过, {results['rustc_fail']} 失败")
print(f"  运行: {results['run_pass']} 通过, {results['run_fail']} 失败")
print(f"  全链路通过: {len(all_pass)}")

if lz_fail:
    print(f"\n--- LZ 编译失败 ({len(lz_fail)}) ---")
    for fn, err in lz_fail:
        print(f"  ✗ {fn}")
        print(f"    {err[:150]}")

if rustc_fail:
    print(f"\n--- Rustc 编译失败 ({len(rustc_fail)}) ---")
    for fn, err in rustc_fail:
        print(f"  ✗ {fn}")
        print(f"    {err[:150]}")

if run_fail:
    print(f"\n--- 运行失败 ({len(run_fail)}) ---")
    for fn, err in run_fail:
        print(f"  ✗ {fn}")
        print(f"    {err[:150]}")

if all_pass:
    print(f"\n--- 全链路通过 ({len(all_pass)}) ---")
    for fn in all_pass:
        print(f"  ✓ {fn}")

# Save results
result_json = os.path.join(HERE, "test_results.json")
with open(result_json, "w", encoding="utf-8") as fj:
    json.dump({"results": results, "details": details, "lz_fail": lz_fail, "rustc_fail": rustc_fail, "run_fail": run_fail, "all_pass": all_pass}, fj, indent=2, ensure_ascii=False)
print(f"\n结果已保存到: {result_json}")
#!/usr/bin/env python3
"""Batch test all DEMO/*.lz files via IR codegen path."""
import subprocess, sys, os, shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent
LZ_EXE = ROOT / "target" / "debug" / "lang-zone.exe"
if not LZ_EXE.exists():
    print("ERROR: lang-zone.exe not found. Run: cargo build")
    sys.exit(1)
if shutil.which("rustc") is None:
    print("ERROR: rustc not on PATH")
    sys.exit(1)

lz_files = []
for f in ROOT.glob("DEMO/**/*.lz"):
    rel = str(f.relative_to(ROOT))
    if "spec" in rel.lower() or "error" in rel.lower():
        continue
    lz_files.append(f)
lz_files.sort()

print(f"Found {len(lz_files)} .lz files")
print("=" * 70)

results = {"PASS": [], "IR_FAIL": [], "RUSTC_FAIL": []}
error_categories = {}

for lz_file in lz_files:
    name = str(lz_file.relative_to(ROOT))
    rs_file = lz_file.with_suffix(".rs")

    p = subprocess.run([str(LZ_EXE), str(lz_file), "--ir-codegen"],
                       capture_output=True, text=True, cwd=str(ROOT), timeout=30)

    if p.returncode != 0 or not rs_file.exists():
        err = p.stderr.strip() or p.stdout.strip() or "(no output)"
        results["IR_FAIL"].append((name, err[:200]))
        continue

    p2 = subprocess.run(["rustc", "--edition", "2021", str(rs_file)],
                        capture_output=True, text=True, cwd=str(ROOT), timeout=30)

    if p2.returncode == 0:
        results["PASS"].append(name)
        exe = rs_file.with_suffix(".exe")
        if exe.exists():
            exe.unlink()
    else:
        err_text = p2.stderr
        first_err = ""
        err_code = ""
        for line in err_text.splitlines():
            if line.startswith("error[") and "]" in line:
                first_err = line.strip()
                code_start = line.find("[") + 1
                code_end = line.find("]")
                err_code = line[code_start:code_end]
                break
            if "error: an inner attribute" in line:
                first_err = line.strip()
                err_code = "inner_attr"
                break
            if line.startswith("error: ") and "inner attribute" in line:
                first_err = line.strip()
                err_code = "inner_attr"
                break
        if not first_err:
            first_err = err_text.splitlines()[0] if err_text.strip() else "(no output)"
            if "error" in first_err.lower():
                err_code = "OTHER"
        results["RUSTC_FAIL"].append((name, first_err[:200]))
        if err_code:
            error_categories[err_code] = error_categories.get(err_code, 0) + 1

print(f"\nSUMMARY: {len(lz_files)} files")
print(f"  PASS:       {len(results['PASS'])}")
print(f"  IR_FAIL:    {len(results['IR_FAIL'])}")
print(f"  RUSTC_FAIL: {len(results['RUSTC_FAIL'])}")

print(f"\n--- RUSTC_FAIL ({len(results['RUSTC_FAIL'])}) ---")
for name, err in results["RUSTC_FAIL"]:
    print(f"  {name}")
    print(f"    {err}")

print(f"\n--- IR_FAIL ({len(results['IR_FAIL'])}) ---")
for name, err in results["IR_FAIL"]:
    print(f"  {name}")
    print(f"    {err}")

print(f"\n--- PASS ({len(results['PASS'])}) ---")
for name in results["PASS"]:
    print(f"  {name}")

print(f"\n--- ERROR BREAKDOWN ---")
for code, count in sorted(error_categories.items(), key=lambda x: -x[1]):
    print(f"  {code}: {count}")

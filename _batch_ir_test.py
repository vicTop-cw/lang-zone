#!/usr/bin/env python3
import subprocess, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parent
LZ_EXE = ROOT / "target" / "debug" / "lang-zone.exe"
assert LZ_EXE.exists(), "lang-zone.exe not found"
lz_files = sorted(f for f in ROOT.glob("DEMO/**/*.lz") if "spec" not in str(f).lower() and "error" not in str(f).lower() and "99_spec" not in str(f))
print(f"Found {len(lz_files)} .lz files\n" + "=" * 70)
results = {"PASS": [], "IR_FAIL": [], "RUSTC_FAIL": []}
error_categories = {}
for lz_file in lz_files:
    name = str(lz_file.relative_to(ROOT))
    rs_file = lz_file.with_suffix(".rs")
    p = subprocess.run([str(LZ_EXE), str(lz_file), "--ir-codegen"], capture_output=True, text=True, cwd=str(ROOT), timeout=30)
    if p.returncode != 0 or not rs_file.exists():
        err = (p.stderr or p.stdout or "(no output)").strip()
        results["IR_FAIL"].append((name, err[:200]))
        continue
    p2 = subprocess.run(["rustc", "--edition", "2021", str(rs_file)], capture_output=True, text=True, cwd=str(ROOT), timeout=30)
    if p2.returncode == 0:
        results["PASS"].append(name)
        exe = rs_file.with_suffix(".exe")
        if exe.exists(): exe.unlink()
    else:
        first_err = "(no error line)"
        err_code = ""
        for line in p2.stderr.splitlines():
            if line.startswith("error[") and "]" in line:
                first_err = line.strip()
                code_start = line.find("[") + 1
                code_end = line.find("]")
                err_code = line[code_start:code_end]
                break
        results["RUSTC_FAIL"].append((name, first_err[:200]))
        if err_code: error_categories[err_code] = error_categories.get(err_code, 0) + 1
print(f"\nSUMMARY: {len(lz_files)} files")
print(f"  PASS:       {len(results['PASS'])}")
print(f"  IR_FAIL:    {len(results['IR_FAIL'])}")
print(f"  RUSTC_FAIL: {len(results['RUSTC_FAIL'])}")
for label, items in [("RUSTC_FAIL", results["RUSTC_FAIL"])]:
    print(f"\n--- {label} ({len(items)}) ---")
    for n, e in items: print(f"  {n}\n    {e}")
print(f"\n--- ERROR BREAKDOWN ---")
for code, count in sorted(error_categories.items(), key=lambda x: -x[1]):
    print(f"  {code}: {count}")

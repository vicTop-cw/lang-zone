import subprocess, os, json
from datetime import datetime, timezone

DEMO_ROOT = "DEMO"
EXCLUDE = {"99_errors", "99_spec"}

def find_lz(root):
    files = []
    for dp, dn, fn in os.walk(root):
        dn[:] = [d for d in dn if d not in EXCLUDE]
        for f in fn:
            if f.endswith('.lz'):
                files.append(os.path.relpath(os.path.join(dp, f), root))
    return sorted(files)

def run_lz(fp):
    r = subprocess.run(['cargo', 'run', '--', fp, '--ir-codegen'],
        capture_output=True, timeout=120, encoding='utf-8', errors='replace')
    return 'Generated' in (r.stdout + r.stderr), r.stdout + r.stderr

def run_rustc(rs):
    r = subprocess.run(['rustc', '--edition', '2021', '--emit=metadata', rs],
        capture_output=True, timeout=30, encoding='utf-8', errors='replace')
    return r.returncode == 0, (r.stderr or "")

def main():
    lz_files = find_lz(DEMO_ROOT)
    res = {"total": len(lz_files), "lz_pass": 0, "lz_fail": 0,
           "rustc_pass": 0, "rustc_fail": 0, "pass_files": [],
           "errors": {}, "details": [],
           "timestamp": datetime.now(timezone.utc).isoformat()}

    for i, f in enumerate(lz_files):
        full = os.path.join(DEMO_ROOT, f)
        rs = full.replace('.lz', '.rs')
        print(f"[{i+1}/{len(lz_files)}] {f} ... ", end='', flush=True)
        lz_ok, _ = run_lz(full)
        if not lz_ok:
            res["lz_fail"] += 1; print("LZ FAIL")
            res["details"].append({"file": f, "lz_ok": False, "rustc_ok": False, "rustc_errors": ["LZ FAIL"]})
            continue
        res["lz_pass"] += 1
        ok, stderr = run_rustc(rs)
        if ok:
            res["rustc_pass"] += 1; res["pass_files"].append(f); print("OK")
        else:
            res["rustc_fail"] += 1
            errs = []
            for line in stderr.split('\n'):
                if 'error[' in line:
                    code = line.split('error[')[1].split(']')[0]
                    errs.append(code)
                    res["errors"].setdefault(code, [])
                    if f not in res["errors"][code]:
                        res["errors"][code].append(f)
            res["details"].append({"file": f, "lz_ok": True, "rustc_ok": False, "rustc_errors": stderr.strip().split('\n')[:8]})
            print(f"FAIL ({', '.join(set(errs))})")
        if os.path.exists(rs): os.remove(rs)

    print(f"\n{'='*60}")
    print(f"IR Codegen Batch Results")
    print(f"{'='*60}")
    print(f"Total: {res['total']}")
    print(f"LZ->IR: {res['lz_pass']}/{res['total']} ({res['lz_pass']*100//res['total']}%)")
    print(f"IR->rustc: {res['rustc_pass']}/{res['total']} ({res['rustc_pass']*100//res['total']}%)")
    print(f"\nErrors:")
    for code in sorted(res["errors"].keys()):
        print(f"  {code}: {len(res['errors'][code])} files")
    with open('_ir_test_result.json', 'w', encoding='utf-8') as f:
        json.dump(res, f, indent=2, ensure_ascii=False)

if __name__ == '__main__':
    main()

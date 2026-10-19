# -*- coding: utf-8 -*-
import subprocess, os, glob, re

EXE = r"target\debug\lang-zone.exe"
lz = sorted(glob.glob(r"target\debug\deps\liblz_builtins-*.rlib"),
            key=os.path.getmtime)[-1]
probes = sorted(glob.glob(r".fist-wf-20260926\r2\*.lz"))

for src in probes:
    name = os.path.basename(src)
    print("=" * 60)
    print("PROBE:", name)
    # translate
    r = subprocess.run([EXE, src], capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    rs = src[:-3] + ".rs"
    if not os.path.exists(rs):
        print("  [TRANSLATE FAIL] rc=%d" % r.returncode)
        print("  " + r.stderr.strip().splitlines()[-1] if r.stderr.strip() else "  (no stderr)")
        # show last 3 lines of stderr
        for line in r.stderr.strip().splitlines()[-4:]:
            print("   |", line)
        continue
    print("  [TRANSLATE OK]")
    # compile
    out = src[:-3] + "_out.exe"
    cr = subprocess.run(
        ["rustc", "--edition", "2021", "-L", "dependency=target/debug/deps",
         "--extern", "lz_builtins=%s" % lz, rs, "-o", out],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if cr.returncode != 0:
        print("  [COMPILE FAIL] rc=%d" % cr.returncode)
        for line in cr.stderr.strip().splitlines()[:6]:
            print("   |", line)
    else:
        print("  [COMPILE OK]")

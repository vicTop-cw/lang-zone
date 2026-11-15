"""对指定 .lz 跑 lang-zone 生成 .rs，再用 rustc 编译，报告首条错误。
用法: python check.py DEMO/lz_std/dict.lz [更多...]
"""
import subprocess, sys, os, glob

ROOT = r"E:\IDEProjects\AI\lang-zone"
BIN = os.path.join(ROOT, "target", "debug", "lang-zone.exe")
OUT = os.path.join(ROOT, ".fist-wf-20260926", "out.exe")


def latest_rlib():
    libs = glob.glob(os.path.join(ROOT, "target", "debug", "deps", "liblz_builtins-*.rlib"))
    libs.sort(key=os.path.getmtime)
    return libs[-1]


def check(lz_rel):
    lz = os.path.join(ROOT, lz_rel)
    r = subprocess.run([BIN, lz], capture_output=True, text=True)
    rs = lz[:-3] + ".rs"
    if not os.path.exists(rs):
        print("NO_RS  ", lz_rel, "|", r.stderr.strip().splitlines()[:2])
        return False
    c = subprocess.run(
        ["rustc", "--edition", "2021",
         "-L", "dependency=" + os.path.join(ROOT, "target", "debug", "deps"),
         "--extern", "lz_builtins=" + latest_rlib(),
         rs, "-A", "warnings", "-o", OUT],
        capture_output=True, text=True)
    if c.returncode == 0:
        print("OK     ", lz_rel)
        return True
    errs = [l for l in c.stderr.splitlines() if l.strip().startswith("error")]
    print("FAIL   ", lz_rel)
    for e in errs[:3]:
        print("        ", e.strip())
    return False


if __name__ == "__main__":
    for p in sys.argv[1:]:
        check(p)

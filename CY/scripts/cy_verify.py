#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
cy_verify.py — LZ Cython 后端批量验证脚本（PLAN.md G7）

对 DEMO 目录逐个 .lz 文件：
  1. lang-zone.exe <file.lz> --backend=cython 生成 .pyx（transpile 层）
  2. cython -3 <file.pyx> 编译为 .c（语法层验证铁律第 1 级）

用法：
  python cy_verify.py [--lzc <path-to-lang-zone.exe>] [--demo <demo-dir>] [--keep]

默认：
  lzc  = <repo>/target/debug/lang-zone.exe（不存在则尝试 release）
  demo = <repo>/DEMO（排除 99_errors/）
"""

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


def find_lzc(explicit: str | None) -> Path:
    if explicit:
        p = Path(explicit)
        if not p.exists():
            sys.exit(f"lzc not found: {p}")
        return p
    for profile in ("debug", "release"):
        p = REPO / "target" / profile / "lang-zone.exe"
        if p.exists():
            return p
    sys.exit("lang-zone.exe not found; build first: cargo build")


def find_cython() -> list[str]:
    exe = shutil.which("cython")
    if exe:
        return [exe]
    # Windows：pip 安装的脚本可能不在 PATH（如 G:\\miniconda\\conda\\Scripts）
    for scripts in Path(sys.executable).parents:
        cand = scripts / "Scripts" / "cython.exe"
        if cand.exists():
            return [str(cand)]
    return [sys.executable, "-m", "cython"]


def collect_lz(demo: Path) -> list[Path]:
    files = sorted(demo.rglob("*.lz"))
    return [
        f
        for f in files
        if "99_errors" not in f.parts and "__pycache__" not in f.parts
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--lzc", help="lang-zone.exe path")
    ap.add_argument("--demo", help="demo dir (default <repo>/DEMO)")
    ap.add_argument("--keep", action="store_true", help="keep generated .pyx/.c")
    args = ap.parse_args()

    lzc = find_lzc(args.lzc)
    cython_cmd = find_cython()
    demo = Path(args.demo) if args.demo else REPO / "DEMO"
    files = collect_lz(demo)

    print(f"lzc    = {lzc}")
    print(f"cython = {' '.join(cython_cmd)}")
    print(f"demo   = {demo} ({len(files)} files)\\n")

    ok_transpile, ok_syntax, failures = 0, 0, []

    for lz in files:
        pyx = lz.with_suffix(".pyx")
        # 1) transpile
        r = subprocess.run(
            [str(lzc), str(lz), "--backend=cython"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if r.returncode != 0 or not pyx.exists():
            failures.append((lz, "transpile", (r.stderr or r.stdout).strip()[:300]))
            print(f"  T✗ {lz.relative_to(demo)}")
            continue
        ok_transpile += 1
        # 2) cython 语法层（.pyx → .c）
        c_file = pyx.with_suffix(".c")
        r2 = subprocess.run(
            [*cython_cmd, "-3", "--module-name", "cy_verify_gen", str(pyx)],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            cwd=str(pyx.parent),
        )
        if r2.returncode == 0:
            ok_syntax += 1
            print(f"  OK {lz.relative_to(demo)}")
        else:
            err = (r2.stderr or r2.stdout).strip().splitlines()
            tail = "\\n".join(err[-4:])[:400] if err else "(no output)"
            failures.append((pyx, "cythonize", tail))
            print(f"  C✗ {lz.relative_to(demo)}")
        # cleanup
        if not args.keep:
            c_file.unlink(missing_ok=True)
            if c_file.with_suffix(".html").exists():
                c_file.with_suffix(".html").unlink()

    total = len(files)
    print("\\n===== SUMMARY =====")
    print(f"total     : {total}")
    print(f"transpiled: {ok_transpile} ({100 * ok_transpile // max(total, 1)}%)")
    print(f"cython-OK : {ok_syntax} ({100 * ok_syntax // max(total, 1)}%)")
    if failures:
        print(f"\\nfailures  : {len(failures)}")
        for path, stage, msg in failures[:20]:
            print(f"--- {stage}: {path}")
            print(f"    {msg}")
        if len(failures) > 20:
            print(f"  ... and {len(failures) - 20} more")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())

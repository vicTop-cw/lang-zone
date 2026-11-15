#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
cy_verify_run.py — LZ Cython 后端运行层验证（验证铁律第 3 级）

对每个 .lz：
  1. lang-zone.exe --backend=cython 生成 .pyx
  2. cython -3 语法层验证（第 1 级）
  3. pyx → py 纯 Python 降级（剥离 cdef/类型标注/ctypedef）
  4. 运行降级模块的 main() 抓取输出（第 3 级运行层）

用法：python cy_verify_run.py [--lzc <exe>] [--demo <dir>] [--limit N]
"""

import argparse
import importlib.util
import io
import re
import shutil
import subprocess
import sys
import contextlib
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]

C_TYPE_RE = re.compile(
    r"\b(Py_ssize_t|double|bint|float|object|str|list|dict|set|tuple|int|bool)\s+(?=[a-zA-Z_])"
)


def find_lzc(explicit):
    if explicit:
        return Path(explicit)
    for profile in ("debug", "release"):
        p = REPO / "target" / profile / "lang-zone.exe"
        if p.exists():
            return p
    sys.exit("lang-zone.exe not found")


def find_cython():
    exe = shutil.which("cython")
    if exe:
        return [exe]
    for scripts in Path(sys.executable).parents:
        cand = scripts / "Scripts" / "cython.exe"
        if cand.exists():
            return [str(cand)]
    return [sys.executable, "-m", "cython"]


def strip_c_types(src: str, class_names: set[str]) -> str:
    """剥离参数/返回中的类型标注（C 类型 + 已知 cdef class 名）"""
    out_lines = []
    for line in src.splitlines():
        s = line
        # C 类型标注：`Py_ssize_t x` → `x`（仅参数位置近似：全行替换足够安全，
        # 因为 C 类型名不会出现在普通标识符位置）
        s = C_TYPE_RE.sub("", s)
        # 自定义类名标注：`Point p` → `p`（仅 def 签名行处理）
        stripped = s.lstrip()
        if stripped.startswith(("def ", "async def ")):
            for cn in class_names:
                s = re.sub(rf"\b{re.escape(cn)}\s+(?=[a-zA-Z_][a-zA-Z0-9_]*\s*[,)=])", "", s)
            # 返回注解删除：`-> X:` → `:`（注解表达式运行时求值会 NameError）
            s = re.sub(r"\)\s*->\s*[^:]+:", "):", s)
        out_lines.append(s)
    return "\n".join(out_lines) + ("\n" if src.endswith("\n") else "")


def pyx_to_py(src: str) -> tuple[str, list[str]]:
    """pyx → 纯 Python 降级。返回 (py 源码, 警告列表)"""
    warns = []
    # 先收集 cdef class 名（供参数标注剥离）
    class_names = set(re.findall(r"^cdef class\s+([A-Za-z_][A-Za-z0-9_]*)", src, re.M))
    out_lines = []
    for line in src.splitlines():
        stripped = line.lstrip()
        indent = line[: len(line) - len(stripped)]
        if stripped.startswith("# cython:"):
            out_lines.append(line)  # 注释保留
        elif stripped.startswith("cdef class "):
            out_lines.append(indent + stripped.replace("cdef class ", "class ", 1))
        elif stripped.startswith("cdef public "):
            # 字段声明：Python 动态属性，删除（记录以便诊断）
            warns.append(f"drop field decl: {stripped}")
        elif stripped.startswith("ctypedef "):
            out_lines.append(indent + "# " + stripped)
        elif re.match(r"^cdef\s+[A-Za-z_][A-Za-z0-9_<>]*\s+[A-Za-z_][A-Za-z0-9_]*\s*(=|$)", stripped):
            # cdef 标量声明：cdef Py_ssize_t x = 3 → x = 3
            m = re.match(r"^cdef\s+[A-Za-z_][A-Za-z0-9_<>]*\s+([A-Za-z_][A-Za-z0-9_]*\s*(?:=.*)?)$", stripped)
            if m:
                out_lines.append(indent + m.group(1))
            else:
                warns.append(f"drop cdef: {stripped}")
        elif stripped.startswith("cdef "):
            warns.append(f"drop cdef: {stripped}")
        else:
            out_lines.append(line)
    py = "\n".join(out_lines) + "\n"
    py = strip_c_types(py, class_names)
    return py, warns


def run_py(py_path: Path, module_name: str) -> str:
    """子进程运行（带超时保护，防死循环 DEMO）"""
    code = (
        "import importlib.util, contextlib, io\n"
        f"spec = importlib.util.spec_from_file_location({module_name!r}, r'{py_path}')\n"
        "mod = importlib.util.module_from_spec(spec)\n"
        "spec.loader.exec_module(mod)\n"
        "fn = getattr(mod, 'main', None)\n"
        "if callable(fn):\n"
        "    fn()\n"
    )
    r = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
        timeout=10,
    )
    if r.returncode != 0:
        raise RuntimeError((r.stderr or r.stdout).strip().splitlines()[-1] if (r.stderr or r.stdout).strip() else "nonzero exit")
    return r.stdout


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--lzc")
    ap.add_argument("--demo")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--file", help="只验证单个 .lz")
    args = ap.parse_args()

    lzc = find_lzc(args.lzc)
    cython_cmd = find_cython()
    demo = Path(args.demo) if args.demo else REPO / "DEMO"

    if args.file:
        files = [Path(args.file)]
    else:
        files = sorted(
            f for f in demo.rglob("*.lz")
            if "99_errors" not in f.parts and "lz_std" not in f.parts
            and "__pycache__" not in f.parts
        )
    if args.limit:
        files = files[: args.limit]

    ok_syntax = ok_run = 0
    failures = []

    for lz in files:
        rel = lz.relative_to(demo) if lz.is_relative_to(demo) else lz
        pyx = lz.with_suffix(".pyx")
        r = subprocess.run(
            [str(lzc), str(lz), "--backend=cython"],
            capture_output=True, text=True, encoding="utf-8", errors="replace",
        )
        if r.returncode != 0 or not pyx.exists():
            failures.append((str(rel), "transpile", (r.stderr or r.stdout)[:200]))
            continue
        # 语法层
        r2 = subprocess.run(
            [*cython_cmd, "-3", "--module-name", "cy_verify_gen", str(pyx)],
            capture_output=True, text=True, encoding="utf-8", errors="replace",
            cwd=str(pyx.parent),
        )
        if r2.returncode != 0:
            err = (r2.stderr or r2.stdout).strip().splitlines()
            failures.append((str(rel), "cython", "\\n".join(err[-3:])[:250]))
            print(f"  C✗ {rel}")
            continue
        ok_syntax += 1
        # 运行层（纯 Python 降级）
        py_path = pyx.with_suffix(".cpy_run.py")
        try:
            py_src, _ = pyx_to_py(pyx.read_text(encoding="utf-8"))
            py_path.write_text(py_src, encoding="utf-8")
            mod_name = f"cpy_run_{lz.stem.replace('-', '_').replace('.', '_')}"
            out = run_py(py_path, mod_name)
            ok_run += 1
            first = out.splitlines()[0] if out.splitlines() else "(no output)"
            print(f"  OK {rel}  → {first[:60]}")
        except Exception as e:  # noqa: BLE001
            msg = f"{type(e).__name__}: {e}"
            failures.append((str(rel), "run", msg[:250]))
            print(f"  R✗ {rel}")
        finally:
            py_path.unlink(missing_ok=True)

    total = len(files)
    print("\\n===== RUN-VERIFY SUMMARY =====")
    print(f"total    : {total}")
    print(f"syntax-OK: {ok_syntax}")
    print(f"run-OK   : {ok_run} ({100 * ok_run // max(total, 1)}%)")
    if failures:
        print(f"failures : {len(failures)}")
        for path, stage, msg in failures:
            print(f"--- [{stage}] {path}")
            print(f"    {msg}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())

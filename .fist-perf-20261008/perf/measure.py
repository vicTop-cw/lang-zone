#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""lz 性能基准测量器（方向 A/B/D/E 的验收仪器）。

测量口径（与建议书 1.1 对齐）：全新进程冷启动全流程，分三段计时
  lzc 段   : lzc <src> --std-dir std            ->产出 .rs
  rustc 段 : rustc --edition 2021 -O <gen.rs>   ->产出 exe
  run 段   : 运行 exe（采样峰值工作集内存）
每轮每段都是新进程（N 轮取中位数，丢弃前 warmup 轮）。
另附生成产物静态指标：行数 / let临时数 / clone数 / to_string数 / Rc·Arc·Box数，
以及与手写 Rust 锚点的 difflib 差异行数（方向 A/E 的客观判据）。

用法:
  python measure.py --label baseline --rounds 5
  python measure.py --label opt2 --rounds 5 --extra-args "--opt=2"
"""
import argparse
import ctypes
import ctypes.wintypes as wintypes
import difflib
import json
import os
import shutil
import statistics
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
LZC = os.path.join(REPO, "target", "release", "lang-zone.exe")
STD_DIR = os.path.join(REPO, "std")
OUT = os.path.join(HERE, "out")
BENCH_LZ = os.path.join(HERE, "bench.lz")
BENCH_RS = os.path.join(HERE, "bench_rs.rs")

PROC_QUERY_LIMITED_INFORMATION = 0x1000
PROCESS_VM_READ = 0x0010


class PMC(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD),
        ("PageFaultCount", wintypes.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
    ]


def mem_info(handle):
    c = PMC()
    c.cb = ctypes.sizeof(PMC)
    if ctypes.windll.psapi.GetProcessMemoryInfo(handle, ctypes.byref(c), c.cb):
        return c.WorkingSetSize, c.PeakWorkingSetSize
    return 0, 0


def run_measured(cmd, timeout=300):
    """启动全新进程，测墙钟耗时 + 峰值工作集（KB）。返回 (rc, ms, peak_kb, output)。"""
    t0 = time.perf_counter()
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                         text=True, encoding="utf-8", errors="replace")
    h = ctypes.windll.kernel32.OpenProcess(
        PROC_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, False, p.pid)
    peak = [0]

    def sampler():
        while p.poll() is None:
            ws, _ = mem_info(h)
            if ws > peak[0]:
                peak[0] = ws
            time.sleep(0.002)

    th = threading.Thread(target=sampler, daemon=True)
    th.start()
    so, se = p.communicate(timeout=timeout)
    ms = (time.perf_counter() - t0) * 1000.0
    th.join(timeout=1)
    _, pk = mem_info(h)
    ctypes.windll.kernel32.CloseHandle(h)
    peak[0] = max(peak[0], pk)
    return p.returncode, ms, peak[0] / 1024.0, (so or "") + (("\n[stderr] " + se) if se.strip() else "")


def med(xs):
    return round(statistics.median(xs), 2) if xs else None


def static_metrics(rs_path, rs_anchor):
    with open(rs_path, encoding="utf-8", errors="replace") as f:
        gen = f.read()
    with open(rs_anchor, encoding="utf-8", errors="replace") as f:
        anchor = f.read()
    gen_lines = [l.rstrip() for l in gen.splitlines() if l.strip()]
    a_lines = [l.rstrip() for l in anchor.splitlines() if l.strip()]
    sm = difflib.SequenceMatcher(None, a_lines, gen_lines)
    diff_changed = sum(max(i2 - i1, j2 - j1)
                       for tag, i1, i2, j1, j2 in sm.get_opcodes() if tag != "equal")
    return {
        "gen_lines": len(gen_lines),
        "anchor_lines": len(a_lines),
        "let_temps": gen.count("let mut ") + gen.count("let __"),
        "clone_calls": gen.count(".clone()"),
        "to_string_calls": gen.count(".to_string()"),
        "vec_new": gen.count("Vec::new()"),
        "with_capacity": gen.count("with_capacity"),
        "rc_arc_box": gen.count("Rc<") + gen.count("Arc<") + gen.count("Box<"),
        "diff_changed_lines": diff_changed,
        "diff_ratio": round(diff_changed / max(1, len(a_lines)), 4),
    }


def find_rlib():
    """定位 lz_builtins rlib（生成代码 use lz_builtins::*）。"""
    for p in (os.path.join(REPO, "target", "debug", "liblz_builtins.rlib"),
              os.path.join(REPO, "target", "release", "liblz_builtins.rlib")):
        if os.path.isfile(p):
            return p, os.path.join(os.path.dirname(p), "deps")
    deps = os.path.join(REPO, "target", "debug", "deps")
    if os.path.isdir(deps):
        for f in os.listdir(deps):
            if f.startswith("liblz_builtins-") and f.endswith(".rlib"):
                return os.path.join(deps, f), deps
    return None, None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", default="run")
    ap.add_argument("--rounds", type=int, default=5)
    ap.add_argument("--warmup", type=int, default=1)
    ap.add_argument("--extra-args", default="", help="追加给 lzc 的参数，如 '--opt=2'")
    ap.add_argument("--skip-lz", action="store_true", help="只测手写 Rust 锚点")
    ap.add_argument("--src", default=None, help="LZ 源（默认 bench.lz）")
    ap.add_argument("--anchor", default=None, help="手写 Rust 锚点（默认 bench_rs.rs）")
    args = ap.parse_args()
    bench_lz = args.src or BENCH_LZ
    bench_rs = args.anchor or BENCH_RS

    if not os.path.isfile(LZC):
        print("missing release binary: " + LZC)
        return 1
    os.makedirs(OUT, exist_ok=True)
    rustc = shutil.which("rustc") or "rustc"
    extra = args.extra_args.split() if args.extra_args.strip() else []

    lzc_s, rustc_s, run_s, mem_s = [], [], [], []
    gen_rs = os.path.join(OUT, os.path.splitext(os.path.basename(bench_lz))[0] + ".rs")
    gen_exe = os.path.join(OUT, "bench_lz.exe")
    anchor_exe = os.path.join(OUT, "bench_rs.exe")
    rlib, deps = find_rlib()
    lz_rustc = [rustc, "--edition", "2021", "-O", "-A", "unused"]
    if rlib:
        lz_rustc += ["--extern", "lz_builtins=" + rlib, "-L", "dependency=" + deps]
    else:
        print("WARNING: liblz_builtins rlib not found; generated code may fail to link")
    stdout_lz = stdout_rs = ""
    metrics = None
    anchor_stats = {}

    if not args.skip_lz:
        src = os.path.join(OUT, os.path.basename(bench_lz))
        shutil.copy(bench_lz, src)
        for i in range(args.rounds + args.warmup):
            if os.path.exists(gen_rs):
                os.remove(gen_rs)
            rc1, ms1, _, out = run_measured([LZC, src, "--std-dir", STD_DIR] + extra)
            if rc1 != 0 or not os.path.exists(gen_rs):
                print("lzc failed:\n" + out[:2000])
                return 1
            rc2, ms2, _, out2 = run_measured(lz_rustc + [gen_rs, "-o", "%s_lz_r%d.exe" % (gen_exe, i)])
            if rc2 != 0:
                print("rustc failed:\n" + out2[:2000])
                return 1
            rc3, ms3, mb3, out3 = run_measured(["%s_lz_r%d.exe" % (gen_exe, i)])
            if rc3 != 0:
                print("run failed:\n" + out3[:1000])
                return 1
            if i >= args.warmup:
                lzc_s.append(ms1)
                rustc_s.append(ms2)
                run_s.append(ms3)
                mem_s.append(mb3)
            stdout_lz = out3
        metrics = static_metrics(gen_rs, bench_rs)
        shutil.copy(gen_rs, os.path.join(HERE, "gen_%s.rs" % args.label))

    for i in range(args.rounds + args.warmup):
        a_exe = "%s_rs_r%d.exe" % (anchor_exe, i)
        rc, ms, _, _ = run_measured([rustc, bench_rs, "--edition", "2021", "-O",
                                     "-A", "unused", "-o", a_exe])
        if rc != 0:
            print("anchor rustc failed")
            return 1
        rc, ms2, mb2, out = run_measured([a_exe])
        if i >= args.warmup:
            anchor_stats.setdefault("rustc_ms", []).append(ms)
            anchor_stats.setdefault("run_ms", []).append(ms2)
            anchor_stats.setdefault("peak_kb", []).append(mb2)
        stdout_rs = out

    res = {
        "label": args.label,
        "rounds": args.rounds,
        "extra_args": args.extra_args,
        "lz": {
            "lzc_ms": med(lzc_s),
            "rustc_ms": med(rustc_s),
            "run_ms": med(run_s),
            "peak_kb": med(mem_s),
            "stdout": stdout_lz.strip().splitlines(),
            "codegen_metrics": metrics,
        },
        "anchor_rust": {
            "rustc_ms": med(anchor_stats.get("rustc_ms", [])),
            "run_ms": med(anchor_stats.get("run_ms", [])),
            "peak_kb": med(anchor_stats.get("peak_kb", [])),
            "stdout": stdout_rs.strip().splitlines(),
        },
    }
    res["stdout_match"] = (stdout_lz.strip() == stdout_rs.strip())
    with open(os.path.join(HERE, "result_%s.json" % args.label), "w", encoding="utf-8") as f:
        json.dump(res, f, indent=2, ensure_ascii=False)
    print(json.dumps(res, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
#!/usr/bin/env python3
"""
Lang-Zong 跨语言性能基准测试
========================================================================
测试 Fibonacci(35) 递归计算，对比各语言的编译和执行性能。

指标:
  - compile_s: 编译时间（需要编译的语言）
  - exec_s: 执行时间
  - binary_kb: 二进制大小（生成独立二进制的语言）
  - loc: 源代��行数（去注释/空行）

运行: python run_benchmark.py
"""

import os, sys, time, subprocess, json, platform, shutil

HERE = os.path.dirname(os.path.abspath(__file__))
SRC_DIR = os.path.join(HERE, "src")
OUT_DIR = os.path.join(HERE, "_work")
os.makedirs(OUT_DIR, exist_ok=True)

# ── 工具链 ──
LZC = os.path.join(HERE, "..", "target", "release", "lang-zone.exe")
STD_DIR = os.path.join(HERE, "..", "std")
RUSTC = shutil.which("rustc") or "rustc"
ZIG = shutil.which("zig") or "zig"
SCALAC = shutil.which("scalac") or "scalac"
SCALA = shutil.which("scala") or "scala"
GO = shutil.which("go") or "go"
PYTHON = sys.executable
NODE = shutil.which("node") or "node"

RUNS = 4  # 每项跑 4 次，首次为 warmup 丢弃，取后 3 次中位数
RESULTS = {}


def median(arr):
    s = sorted(arr)
    return s[len(s) // 2]


def run_cmd(cmd, timeout=120, cwd=None):
    t0 = time.perf_counter()
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, cwd=cwd)
    except subprocess.TimeoutExpired:
        return (-1, "", "TIMEOUT", 0)
    t = time.perf_counter() - t0
    return (r.returncode, r.stdout, r.stderr, t)


def lines_of_code(path):
    with open(path, encoding="utf-8") as f:
        lines = [l for l in f.readlines() if l.strip()
                 and not l.strip().startswith("//")
                 and not l.strip().startswith("#")]
    return len(lines)


def size_kb(path):
    return os.path.getsize(path) / 1024 if os.path.exists(path) else 0


def report_fail(lang, detail=""):
    print(f"    ❌ {lang}: {detail[:120]}")


# ===================================================================
# 1. 工具链检查
# ===================================================================
print("=" * 65)
print("  Lang-Zong 跨语言性能基准测试")
print(f"  系统: {platform.system()} | CPU: {platform.processor() or 'unknown'}")
print(f"  时间: {time.strftime('%Y-%m-%d %H:%M:%S')}")
print("=" * 65)

available = {}
for name, cmd in [("rustc", f"{RUSTC} --version"),
                   ("zig", f"{ZIG} version"),
                   ("scalac", f"{SCALAC} -version"),
                   ("scala", f"{SCALA} -version"),
                   ("go", f"{GO} version"),
                   ("node", f"{NODE} --version")]:
    try:
        r = subprocess.run(cmd.split(), capture_output=True, text=True, timeout=5)
        ver = r.stdout.strip() or r.stderr.strip()
        available[name] = ver.split("\n")[0]
        print(f"  ✅ {name}: {ver.split(chr(10))[0]}")
    except Exception as e:
        available[name] = None
        print(f"  ⚠️  {name}: not available")

available["python"] = f"Python {sys.version.split()[0]}"
available["lzc"] = os.path.isfile(LZC)
print(f"  ✅ lzc: {'release binary exists' if available['lzc'] else 'NOT BUILT'}")

# ===================================================================
# 2. 源文件定义
# ===================================================================
SOURCES = {}

# LZ (fib only - complex loops not yet stable)
SOURCES["lz"] = ("bench_lz.lz", """def fib(n: int)-> int =
    if n <= 1:
        n
    else:
        fib(n - 1) + fib(n - 2)

def main() =
    let r = fib(35)
    print(f"fib(35) = {r}")
""")

# Rust
SOURCES["rust"] = ("bench_rs.rs", """fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}

fn sieve(n: usize) -> usize {
    let mut p = vec![true; n + 1]; p[0] = false; p[1] = false;
    let mut i = 2;
    while i * i <= n { if p[i] { let mut j = i * i; while j <= n { p[j] = false; j += i; } } i += 1; }
    p.iter().filter(|&&x| x).count()
}

fn main() {
    let r = fib(35); println!("fib(35) = {}", r);
    let c = sieve(500000); println!("sieve(500000) = {} primes", c);
}
""")

# Python
SOURCES["python"] = ("bench.py", """def fib(n):
    return n if n <= 1 else fib(n-1)+fib(n-2)

def sieve(n):
    p = [True]*(n+1); p[0]=p[1]=False
    for i in range(2, int(n**0.5)+1):
        if p[i]: p[i*i:n+1:i] = [False]*((n - i*i)//i + 1)
    return sum(p)

print(f"fib(35) = {fib(35)}")
print(f"sieve(500000) = {sieve(500000)} primes")
""")

# Zig
SOURCES["zig"] = ("bench.zig", """const std = @import("std");

fn fib(n: i64) i64 { if (n <= 1) return n; return fib(n - 1) + fib(n - 2); }

fn sieve(alloc: std.mem.Allocator, n: usize) !usize {
    var p = try alloc.alloc(bool, n + 1);
    defer alloc.free(p);
    @memset(p, true); p[0] = false; p[1] = false;
    var i: usize = 2;
    while (i * i <= n) : (i += 1) { if (p[i]) { var j = i * i; while (j <= n) : (j += i) p[j] = false; } }
    var c: usize = 0; for (p) |v| if (v) c += 1;
    return c;
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    std.debug.print("fib(35) = {d}\n", .{fib(35)});
    std.debug.print("sieve(500000) = {d} primes\n", .{try sieve(alloc, 500000)});
}
""")

# Scala
SOURCES["scala"] = ("Bench.scala", """object Bench {
    def fib(n: Long): Long = if (n <= 1) n else fib(n-1) + fib(n-2)
    def sieve(n: Int): Int = {
        val p = Array.fill(n+1)(true); p(0)=false; p(1)=false
        for (i <- 2 to math.sqrt(n).toInt if p(i)) { for (j <- i*i to n by i) p(j) = false }
        p.count(_ == true)
    }
    def main(a: Array[String]): Unit = {
        println(s"fib(35) = ${fib(35)}")
        println(s"sieve(500000) = ${sieve(500000)} primes")
    }
}
""")

# Go
SOURCES["go"] = ("bench.go", """package main
import "fmt"
func fib(n int64) int64 { if n <= 1 { return n }; return fib(n-1) + fib(n-2) }
func sieve(n int) int {
    p := make([]bool, n+1); for i := 0; i <= n; i++ { p[i] = true }; p[0], p[1] = false, false
    for i := 2; i*i <= n; i++ { if p[i] { for j := i * i; j <= n; j += i { p[j] = false } } }
    c := 0; for _, v := range p { if v { c++ } }; return c
}
func main() {
    fmt.Printf("fib(35) = %d\\n", fib(35))
    fmt.Printf("sieve(500000) = %d primes\\n", sieve(500000))
}
""")

# Write all sources
for lang, (fname, code) in SOURCES.items():
    path = os.path.join(SRC_DIR, fname)
    with open(path, "w", encoding="utf-8") as f:
        f.write(code)

print(f"\n{'='*65}")
print(f"  运行基准测试 (每项 {RUNS} 次, 取中位数)")
print(f"{'='*65}\n")

# ===================================================================
# 3. 运行基准测试
# ===================================================================

# ── LZ (lzc + rustc) ──
print("  [1/6] Lang-Zong (lzc + rustc)")
if available["lzc"]:
    lz_compile_lzc, lz_exec = [], []
    for i in range(RUNS):
        src = os.path.join(SRC_DIR, "bench_lz.lz")
        # Step 1: lzc compile .lz -> .rs
        rc, so, se, t1 = run_cmd([LZC, src, "--std-dir", STD_DIR], timeout=30)
        if rc != 0:
            report_fail("lzc compile", se[:100]); break
        lz_compile_lzc.append(t1)
        gen_rs = src.replace(".lz", ".rs")
        exe = os.path.join(OUT_DIR, f"bench_lz_{i}.exe")
        # Step 2: rustc compile .rs -> .exe
        rc, so, se, t2 = run_cmd([RUSTC, gen_rs, "--edition", "2021", "-O", "-o", exe, "-A", "unused"], timeout=120)
        if rc != 0:
            report_fail("rustc from lz", se[:200]); break
        # Step 3: run
        rc, so, se, t3 = run_cmd([exe], timeout=60)
        if rc == 0:
            lz_exec.append(t3)
            print(f"    Run {i+1}: lzc={t1:.3f}s | rustc={t2:.3f}s | exec={t3:.3f}s | {so.strip()}")
        else:
            report_fail("lz exec", se[:100]); break
    else:
        RESULTS["Lang-Zong (lz)"] = {
            "compile_lzc_s": round(median(lz_compile_lzc), 3),
            "exec_s": round(median(lz_exec), 4),
            "binary_kb": round(size_kb(exe), 1),
            "loc": lines_of_code(src),
        }

# ── Rust (native) ──
print("\n  [2/6] Rust (native rustc)")
if available["rustc"]:
    rs_compile, rs_exec = [], []
    for i in range(RUNS):
        src = os.path.join(SRC_DIR, "bench_rs.rs")
        exe = os.path.join(OUT_DIR, f"bench_rs_{i}.exe")
        rc, so, se, t1 = run_cmd([RUSTC, src, "--edition", "2021", "-o", exe, "-A", "unused"], timeout=120)
        if rc != 0:
            report_fail("rustc", se[:200]); break
        rs_compile.append(t1)
        rc, so, se, t2 = run_cmd([exe], timeout=60)
        if rc == 0:
            rs_exec.append(t2)
            print(f"    Run {i+1}: compile={t1:.3f}s | exec={t2:.3f}s | {so.strip()}")
        else:
            report_fail("rust exec", se[:100]); break
    else:
        RESULTS["Rust"] = {
            "compile_s": round(median(rs_compile), 3),
            "exec_s": round(median(rs_exec), 4),
            "binary_kb": round(size_kb(exe), 1),
            "loc": lines_of_code(src),
        }

# ── Zig ──
print("\n  [3/6] Zig")
if available["zig"]:
    zig_compile, zig_exec = [], []
    for i in range(RUNS):
        src = os.path.join(SRC_DIR, "bench.zig")
        exe = os.path.join(OUT_DIR, f"bench_zig_{i}.exe")
        # zig build-exe outputs to cwd
        rc, so, se, t1 = run_cmd([ZIG, "build-exe", src, f"--name", f"bench_zig_{i}",
                                   f"--cache-dir", os.path.join(OUT_DIR, "zig-cache")], timeout=120)
        built = os.path.join(os.getcwd(), f"bench_zig_{i}.exe")
        if os.path.exists(built):
            shutil.move(built, exe)
        if rc != 0 or not os.path.exists(exe):
            report_fail("zig build-exe", se[:200]); break
        zig_compile.append(t1)
        rc, so, se, t2 = run_cmd([exe], timeout=60)
        if rc == 0:
            zig_exec.append(t2)
            print(f"    Run {i+1}: compile={t1:.2f}s | exec={t2:.3f}s | {so.strip().split(chr(10))[0]}")
        else:
            report_fail("zig exec", se[:200]); break
    else:
        RESULTS["Zig"] = {
            "compile_s": round(median(zig_compile), 2),
            "exec_s": round(median(zig_exec), 4),
            "binary_kb": round(size_kb(exe), 1),
            "loc": lines_of_code(src),
        }

# ── Scala ──
print("\n  [4/6] Scala")
if available["scalac"]:
    sc_compile, sc_exec = [], []
    for i in range(RUNS):
        src = os.path.join(SRC_DIR, "Bench.scala")
        od = os.path.join(OUT_DIR, f"scala_{i}")
        os.makedirs(od, exist_ok=True)
        rc, so, se, t1 = run_cmd([SCALAC, src, "-d", od], timeout=120)
        if rc != 0:
            report_fail("scalac", se[:200]); break
        sc_compile.append(t1)
        rc, so, se, t2 = run_cmd([SCALA, "-cp", od, "Bench"], timeout=120)
        if rc == 0:
            sc_exec.append(t2)
            print(f"    Run {i+1}: compile={t1:.2f}s | exec={t2:.3f}s | {so.strip()}")
        else:
            report_fail("scala run", se[:200]); break
    else:
        RESULTS["Scala"] = {
            "compile_s": round(median(sc_compile), 2),
            "exec_s": round(median(sc_exec), 4),
            "loc": lines_of_code(src),
        }

# ── Go ──
print("\n  [5/6] Go")
if available["go"]:
    go_compile, go_exec = [], []
    for i in range(RUNS):
        src = os.path.join(SRC_DIR, "bench.go")
        exe = os.path.join(OUT_DIR, f"bench_go_{i}.exe")
        rc, so, se, t1 = run_cmd([GO, "build", "-o", exe, src], timeout=120)
        if rc != 0:
            report_fail("go build", se[:200]); break
        go_compile.append(t1)
        rc, so, se, t2 = run_cmd([exe], timeout=60)
        if rc == 0:
            go_exec.append(t2)
            print(f"    Run {i+1}: compile={t1:.2f}s | exec={t2:.3f}s | {so.strip()}")
        else:
            report_fail("go exec", se[:100]); break
    else:
        RESULTS["Go"] = {
            "compile_s": round(median(go_compile), 2),
            "exec_s": round(median(go_exec), 4),
            "binary_kb": round(size_kb(exe), 1),
            "loc": lines_of_code(src),
        }

# ── Python ──
print("\n  [6/6] Python")
py_exec = []
for i in range(RUNS):
    src = os.path.join(SRC_DIR, "bench.py")
    rc, so, se, t = run_cmd([PYTHON, src], timeout=120)
    if rc == 0:
        py_exec.append(t)
        print(f"    Run {i+1}: exec={t:.3f}s | {so.strip()}")
    else:
        report_fail("python", se[:100]); break
else:
    RESULTS["Python"] = {
        "exec_s": round(median(py_exec), 4),
        "loc": lines_of_code(src),
    }

# ===================================================================
# 4. 报告输出
# ===================================================================
print(f"\n{'='*65}")
print(f"  基准测试结果报告")
print(f"{'='*65}")

if not RESULTS:
    print("\n  ❌ 所有测试均失败")
    sys.exit(1)

# Table
langs = ["Lang-Zong (lz)", "Rust", "Zig", "Go", "Scala", "Python"]
h = ["语言", "编译(s)", "执行(s)", "二进制", "代码行", "综合性能比"]
print(f"\n  {'  '.join(h)}")
print(f"  {'-'*65}")

for lang in langs:
    if lang not in RESULTS:
        continue
    d = RESULTS[lang]
    c = d.get("compile_s") or d.get("compile_lzc_s", "-")
    if isinstance(c, float):
        c = f"{c:.2f}"
    e = f"{d['exec_s']:.4f}" if isinstance(d.get("exec_s"), (int, float)) else "-"
    b = f"{d.get('binary_kb', '-')} KB" if isinstance(d.get("binary_kb"), (int, float)) else "-"
    l = str(d.get("loc", "-"))
    # Performance ratio vs Rust
    if "Rust" in RESULTS and lang != "Python":
        ref = RESULTS["Rust"]["exec_s"]
        my = d.get("exec_s", 0)
        ratio = f"{(ref/my):.2f}x" if my else "-"
        print(f"  {lang:20s} {c:>8s}  {e:>10s}  {b:>8s}  {l:>4s}  {ratio:>8s}")
    elif lang == "Python":
        if "Rust" in RESULTS:
            ref = RESULTS["Rust"]["exec_s"]
            my = d.get("exec_s", 0)
            ratio = f"{(ref/my):.2f}x" if my else "-"
        else:
            ratio = "-"
        print(f"  {lang:20s} {'-':>8s}  {e:>10s}  {'-':>8s}  {l:>4s}  {ratio:>8s}")
    else:
        print(f"  {lang:20s} {c:>8s}  {e:>10s}  {b:>8s}  {l:>4s}")

# Bar chart relative to Rust
if "Rust" in RESULTS:
    rust_e = RESULTS["Rust"]["exec_s"]
    print(f"\n  执行时间相对性能 (Rust = 1.00x):")
    print(f"  {'-'*55}")
    for lang in ["Lang-Zong (lz)", "Rust", "Zig", "Go", "Scala", "Python"]:
        if lang not in RESULTS:
            continue
        e = RESULTS[lang].get("exec_s", 0)
        if not e: continue
        ratio = e / rust_e
        bar = "▓" * max(1, int(ratio * 5)) if ratio < 100 else "▓" * 100
        print(f"  {lang:20s} │ {ratio:>6.2f}x {bar}")

# Save
result_path = os.path.join(HERE, "benchmark_results.json")
with open(result_path, "w") as f:
    json.dump(RESULTS, f, indent=2, ensure_ascii=False)
print(f"\n  结果已保存: {result_path}")
print(f"{'='*65}\n")

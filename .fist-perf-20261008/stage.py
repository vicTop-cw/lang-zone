import re, subprocess, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

MINE = [
    "src/ir/opt/mod.rs","src/ir/opt/walk.rs","src/ir/opt/fold.rs","src/ir/opt/branch.rs","src/ir/opt/dce.rs",
    "src/ir/codegen/marks.rs","src/ir/codegen/cap_hint.rs","src/ir/mod.rs","src/ir/builder/mod.rs",
    "src/cli.rs","src/main.rs","src/ir/lz_codegen_lib.lz","tests/perf_opt_gates.rs",
    "tests/find_bug_bugs.rs","tests/g6_codegen.rs","tests/lz_semantic_cases.rs",
    "tests/lzc_loop_boundary_gates.rs","tests/tail_recursion.rs","tests/tail_recursion_matrix.rs",
    "docs/perf-optimization-2026-10-08.md",
    "DEMO/99_spec/moddec/golden/",
]
KEEP = {
 "src/ir/codegen/expr_gen.rs": {15, 1278, 1286, 1384, 1389, 1501, 1506, 4406, 5355},
 "src/ir/codegen/stmt_gen.rs": {126, 408, 659, 667},
 "src/ir/codegen/mod.rs":      {13, 17, 36, 340, 465},
}

def run(a, **kw):
    return subprocess.run(a, capture_output=True, text=True, encoding="utf-8", errors="replace", **kw)

subprocess.run(["git", "reset", "-q"], check=False)   # 清空 index（保留工作树）

for f in MINE:
    r = run(["git", "add", "--", f])
    if r.returncode: print("ADD FAIL", f, r.stderr.strip()[:200])

for f, keeps in KEEP.items():
    out = run(["git", "--no-pager", "diff", "-U0", "--", f]).stdout
    lines = out.splitlines()
    header, hs, cur = [], [], None
    for l in lines:
        if l.startswith("@@"):
            cur = {"hdr": l, "body": []}; hs.append(cur)
        elif cur is not None:
            if l.startswith(("diff ", "index ", "--- ", "+++ ")): cur = None
            else: cur["body"].append(l)
        else:
            header.append(l)
    sel = []
    for h in hs:
        st = int(re.search(r"@@ -(\d+)", h["hdr"]).group(1))
        if st in keeps: sel.append(h)
    patch = "\n".join(header) + "\n" + "\n".join("\n".join([h["hdr"]] + h["body"]) for h in sel) + "\n"
    p = subprocess.run(["git", "apply", "--cached", "--unidiff-zero", "--whitespace=nowarn", "-"], input=patch,
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    print(f"stage {f}: kept {len(sel)}/{len(hs)} hunks -> rc={p.returncode} {p.stderr.strip()[:160]}")

print("\n=== staged summary ===")
print(run(["git", "--no-pager", "diff", "--cached", "--stat"]).stdout)

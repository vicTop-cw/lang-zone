import re, subprocess, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

MINE = [
    "src/ir/opt/mod.rs", "src/ir/opt/walk.rs", "src/ir/opt/fold.rs", "src/ir/opt/branch.rs",
    "src/ir/opt/dce.rs", "src/ir/codegen/marks.rs", "src/ir/codegen/cap_hint.rs",
    "src/ir/mod.rs", "src/ir/builder/mod.rs", "src/cli.rs", "src/main.rs",
    "src/ir/lz_codegen_lib.lz", "tests/perf_opt_gates.rs", "tests/find_bug_bugs.rs",
    "tests/g6_codegen.rs", "tests/lz_semantic_cases.rs", "tests/lzc_loop_boundary_gates.rs",
    "tests/tail_recursion.rs", "tests/tail_recursion_matrix.rs",
    "docs/perf-optimization-2026-10-08.md", "DEMO/99_spec/moddec/golden/",
]
KEEP = {
    "src/ir/codegen/expr_gen.rs": {15, 1278, 1286, 1384, 1389, 1501, 1506, 4406, 5355},
    "src/ir/codegen/stmt_gen.rs": {126, 408, 659, 667},
    "src/ir/codegen/mod.rs": {13, 17, 36, 340, 465},
}
MARKERS = ["print_arg", "vec_cap_hint", "cap_hint", "with_capacity", "方向 B", "方向 E",
           "fresh_temp", "stmt_marks", "set_stmt_marks", "marks::", "_tmp.sort()", "_tmp.reverse()"]


def run(a, **kw):
    return subprocess.run(a, capture_output=True, text=True,
                          encoding="utf-8", errors="replace", **kw)


def split_hunks(path):
    out = run(["git", "--no-pager", "diff", "-U0", "--", path]).stdout
    header, hs, cur = [], [], None
    for l in out.splitlines():
        if l.startswith("@@"):
            cur = {"hdr": l, "body": []}
            hs.append(cur)
        elif cur is not None:
            if l.startswith(("diff ", "index ", "--- ", "+++ ")):
                cur = None
            else:
                cur["body"].append(l)
        else:
            header.append(l)
    return header, hs


subprocess.run(["git", "reset", "-q", "--", "."], check=False)   # index 回到 HEAD，工作树不动

for f in MINE:
    r = run(["git", "add", "--", f])
    if r.returncode:
        print("ADD FAIL", f, r.stderr.strip()[:200])

for f, keeps in KEEP.items():
    header, hs = split_hunks(f)
    delta, sel = 0, []
    for h in hs:
        m = re.match(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)", h["hdr"])
        o_start, o_cnt = int(m.group(1)), int(m.group(2) or 1)
        added = [b for b in h["body"] if b.startswith("+")]
        removed = [b for b in h["body"] if b.startswith("-")]
        n_cnt = len(added)
        if o_start not in keeps:
            continue                      # 丢弃的 hunk 不进patch，故不计入 delta
        n_start = o_start + delta + (1 if o_cnt == 0 else 0)   # 纯插入：新侧起始 = 锚点+1
        tail = m.group(5)
        new_hdr = f"@@ -{o_start},{o_cnt} +{n_start},{n_cnt} @@{tail}"
        sel.append((new_hdr, h["body"]))
        delta += n_cnt - o_cnt
    patch = "\n".join(header) + "\n" + "\n".join("\n".join([hd] + b) for hd, b in sel) + "\n"
    pf = f.replace("/", "_") + ".patch"
    open(pf, "w", encoding="utf-8", newline="").write(patch)
    p = subprocess.run(["git", "apply", "--cached", "--unidiff-zero",
                        "--whitespace=nowarn", "--verbose", pf],
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    print(f"stage {f}: kept {len(sel)}/{len(hs)} rc={p.returncode}")
    print("   ", (p.stdout + p.stderr).replace("\n", "\n    ")[:900])

print("\n=== 校验：暂存区 vs 工作树 的残差里不应有我的任何标记 ===")
bad = 0
for f in KEEP:
    d = run(["git", "--no-pager", "diff", "--", f]).stdout
    hits = {m: d.count(m) for m in MARKERS if m in d}
    print(f"  {f}: {hits if hits else 'clean'}")
    bad += len(hits)
print("RESIDUAL_MARKERS =", bad)
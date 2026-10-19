import re, subprocess, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

MINE = [
    "src/ir/builder/capture.rs",
    "src/ir/builder/convert_decl.rs",
    "src/ir/codegen/owned_local.rs",
    "tests/perf_opt_gates.rs",
]
KEEP = {
    "src/ir/codegen/decl_gen.rs": {514},
    "src/ir/codegen/stmt_gen.rs": {485, 540, 1028},
    "src/ir/codegen/expr_gen.rs": {3541},
}
MARKERS = ["prescan_body_let_bindings", "owned_local_mark_readonly", "只读引用",
           "iter().copied()", "elem_ty", "ReadonlyBorrow"]


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


subprocess.run(["git", "reset", "-q", "--", "."], check=False)

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
        n_cnt = len([b for b in h["body"] if b.startswith("+")])
        if o_start not in keeps:
            continue
        n_start = o_start + delta + (1 if o_cnt == 0 else 0)
        sel.append((f"@@ -{o_start},{o_cnt} +{n_start},{n_cnt} @@{m.group(5)}", h["body"]))
        delta += n_cnt - o_cnt
    patch = "\n".join(header) + "\n" + "\n".join("\n".join([hd] + b) for hd, b in sel) + "\n"
    pf = f.replace("/", "_") + ".stage5.patch"
    open(pf, "w", encoding="utf-8", newline="").write(patch)
    p = subprocess.run(["git", "apply", "--cached", "--unidiff-zero",
                        "--whitespace=nowarn", pf],
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    print(f"stage {f}: kept {len(sel)}/{len(hs)} rc={p.returncode} {p.stderr.strip()[:100]}")

print("\n=== 残差校验（暂存区 vs 工作树不应含我的标记）===")
bad = 0
for f in KEEP:
    d = run(["git", "--no-pager", "diff", "--", f]).stdout
    hits = {m: d.count(m) for m in MARKERS if m in d}
    print("  %s: %s" % (f, hits if hits else "clean"))
    bad += len(hits)
print("RESIDUAL_MARKERS =", bad)
print("\n=== staged ===")
print(run(["git", "--no-pager", "diff", "--cached", "--stat"]).stdout)
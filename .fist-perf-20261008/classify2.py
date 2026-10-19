import re, subprocess, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
KEEP = {
 "src/ir/codegen/expr_gen.rs": {15, 1278, 1286, 1384, 1389, 1501, 1506, 4406, 5355},
 "src/ir/codegen/stmt_gen.rs": {126, 408, 659, 667},
 "src/ir/codegen/mod.rs":      {12, 33, 339, 464},
}
for f, keeps in KEEP.items():
    out = subprocess.run(["git", "--no-pager", "diff", "-U0", "--", f],
                         capture_output=True, text=True, encoding="utf-8", errors="replace").stdout
    lines = out.splitlines()
    hs, cur = [], None
    for l in lines:
        if l.startswith("@@"):
            cur = {"hdr": l, "body": []}; hs.append(cur)
        elif cur is not None:
            if l.startswith(("diff ", "index ", "--- ", "+++ ")): cur = None
            else: cur["body"].append(l)
    print(f"\n=== {f}: {len(hs)} hunks ===")
    for i, h in enumerate(hs, 1):
        m = re.search(r"@@ -(\d+)", h["hdr"]); st = int(m.group(1))
        ch = [b for b in h["body"] if b[:1] in "+-"]
        print(f"{i:>3} start={st:>5} {'KEEP' if st in keeps else 'drop'}  {(ch[0][:76] if ch else '')}")

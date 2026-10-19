import re, subprocess, io, sys
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
for f in ["src/ir/codegen/mod.rs", "src/ir/codegen/stmt_gen.rs", "src/ir/codegen/decl_gen.rs"]:
    out = subprocess.run(["git", "--no-pager", "diff", "-U0", "--", f],
                         capture_output=True, text=True, encoding="utf-8", errors="replace").stdout
    hs, cur = [], None
    for l in out.splitlines():
        if l.startswith("@@"):
            cur = {"hdr": l, "body": []}
            hs.append(cur)
        elif cur is not None:
            if l.startswith(("diff ", "index ", "--- ", "+++ ")):
                cur = None
            else:
                cur["body"].append(l)
    print("=== %s: %d hunks" % (f, len(hs)))
    for i, h in enumerate(hs, 1):
        st = int(re.search(r"@@ -(\d+)", h["hdr"]).group(1))
        ch = [b for b in h["body"] if b[:1] in "+-"]
        first = ch[0][:72] if ch else ""
        print("%3d start=%5d  %s" % (i, st, first))
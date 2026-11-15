import re, subprocess, sys

MINE = [
    "print_arg", "is_fresh_temp", "fresh_temp", "owned_move_or_clone", "move_or_clone",
    "vec_cap_hint", "cap_hint", "marks::stmt_marks", "set_stmt_marks", "with_capacity",
    "方向 B", "方向 E", "_tmp.sort()", "_tmp.reverse()",
]

def hunks(path):
    out = subprocess.run(["git", "--no-pager", "diff", "-U0", "--", path],
                         capture_output=True, text=True, encoding="utf-8", errors="replace").stdout
    lines = out.splitlines()
    head, hs, cur = [], [], None
    for l in lines:
        if l.startswith("@@"):
            cur = {"hdr": l, "body": []}
            hs.append(cur)
        elif cur is not None:
            if l.startswith(("diff ", "index ", "--- ", "+++ ")):
                cur = None
            else:
                cur["body"].append(l)
    return out, hs

for f in ["src/ir/codegen/expr_gen.rs", "src/ir/codegen/stmt_gen.rs", "src/ir/codegen/mod.rs"]:
    _, hs = hunks(f)
    print(f"\n=== {f} : {len(hs)} hunks ===")
    for i, h in enumerate(hs, 1):
        changed = [b for b in h["body"] if b[:1] in "+-"]
        txt = "\n".join(changed)
        keep = any(m in txt for m in MINE)
        m = re.search(r"@@ -(\d+)", h["hdr"])
        print(f"{i:>3} start={m.group(1):>5} {'KEEP' if keep else 'drop'}  {changed[0][:70] if changed else ''}")

import re, subprocess, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
KEEP = {15, 1278, 1286, 1384, 1389, 1501, 1506, 4406, 5355}
f = "src/ir/codegen/expr_gen.rs"
out = subprocess.run(["git","--no-pager","diff","-U0","--",f], capture_output=True, text=True, encoding="utf-8", errors="replace").stdout
header, hs, cur = [], [], None
for l in out.splitlines():
    if l.startswith("@@"): cur={"hdr":l,"body":[]}; hs.append(cur)
    elif cur is not None:
        if l.startswith(("diff ","index ","--- ","+++ ")): cur=None
        else: cur["body"].append(l)
    else: header.append(l)
delta, sel = 0, []
for h in hs:
    m = re.match(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)", h["hdr"])
    o_start, o_cnt = int(m.group(1)), int(m.group(2) or 1)
    n_cnt = len([b for b in h["body"] if b.startswith("+")])
    if o_start not in KEEP: continue
    n_start = o_start + delta + (1 if o_cnt == 0 else 0)
    sel.append((f"@@ -{o_start},{o_cnt} +{n_start},{n_cnt} @@{m.group(5)}", h["body"]))
    delta += n_cnt - o_cnt
patch = "\n".join(header)+"\n"+"\n".join("\n".join([hd]+b) for hd,b in sel)+"\n"
open(".fist-perf-20261008/expr3.patch","w",encoding="utf-8",newline="").write(patch)
for hd,b in sel: print(repr(hd))

import re, subprocess, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
KEEP = {15, 1278, 1286, 1384, 1389, 1501, 1506, 4406, 5355}
f = "src/ir/codegen/expr_gen.rs"
out = subprocess.run(["git","--no-pager","diff","-U0","--",f], capture_output=True, text=True, encoding="utf-8", errors="replace").stdout
lines = out.splitlines()
header, hs, cur = [], [], None
for l in lines:
    if l.startswith("@@"): cur={"hdr":l,"body":[]}; hs.append(cur)
    elif cur is not None:
        if l.startswith(("diff ","index ","--- ","+++ ")): cur=None
        else: cur["body"].append(l)
    else: header.append(l)
sel=[h for h in hs if int(re.search(r"@@ -(\d+)",h["hdr"]).group(1)) in KEEP]
patch = "\n".join(header)+"\n"+"\n".join("\n".join([h["hdr"]]+h["body"]) for h in sel)+"\n"
open(".fist-perf-20261008/expr.patch","w",encoding="utf-8",newline="").write(patch)
print(repr(header))
print("---- first 3 kept hunks ----")
for h in sel[:3]:
    print(repr(h["hdr"]))
    for b in h["body"][:4]: print("   ", repr(b[:100]))

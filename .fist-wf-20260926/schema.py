# -*- coding: utf-8 -*-
import subprocess, json, sys
sys.path.insert(0, r"E:\IDEProjects\AI\lang-zone\.fist-wf-20260926")
from drive import start, rpc, META
proc = start()
r = rpc(proc, 1, "tools/list", {"_meta": dict(META)})
want = {"omega_spec_create","omega_result_verify","omega_spec_review","omega_verify_fix"}
for t in r["result"]["tools"]:
    if t["name"] in want:
        print("###", t["name"])
        print("desc:", t.get("description","")[:600])
        print("schema:", json.dumps(t.get("inputSchema",{}), ensure_ascii=False)[:1200])
proc.stdin.close()

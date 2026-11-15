import json, subprocess, sys
sys.path.insert(0, ".fist-perf-20261008")
import importlib.util
spec = importlib.util.spec_from_file_location("fistc", ".fist-perf-20261008/fist.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
c = m.Client()
tasks = json.loads(c.call("list", {"namespace": "perf-opt"}))
leaves = [t["id"] for t in tasks if t.get("is_leaf")]
by = {}
for t in tasks:
    if t.get("is_leaf"):
        by.setdefault(t["status"], []).append(t["id"])
ex = {e["args"]["task_id"]: e["args"]["deliverable"] for e in json.load(open(".fist-perf-20261008/ops_execute.json", encoding="utf-8"))}
ops = []
for st, ids in by.items():
    for i in ids:
        if st == "待领取":
            ops += [{"tool": "claim", "args": {"task_id": i, "assignee": "codebuddy-agent"}},
                    {"tool": "execute", "args": {"task_id": i, "executor": "codebuddy-agent", "deliverable": ex.get(i, "")}},
                    {"tool": "submit", "args": {"task_id": i}}]
        elif st == "执行中":
            ops += [{"tool": "submit", "args": {"task_id": i}}]
        elif st == "待验收":
            ops += [{"tool": "verify", "args": {"task_id": i, "verifier": "codebuddy-agent"}},
                    {"tool": "archive", "args": {"task_id": i, "by": "codebuddy-agent"}}]
print(json.dumps(by, ensure_ascii=False))
for op in ops:
    r = c.call(op["tool"], op["args"])
    if r.startswith("ERROR"):
        print(op["tool"], op["args"]["task_id"], r[:120])
print("final:", c.call("status_summary", {"namespace": "perf-opt"})[:200])
c.close()

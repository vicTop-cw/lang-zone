import json, subprocess, re, sys

ids = ["T0r8","T0r8.1","T0r8.2","T0r8.3","T0r8.4","T0r8.2.1","T0r8.2.2","T0r8.3.1","T0r8.3.2","T0r8.4.1","T0r8.4.2"]
seq = [{"tool":"get","args":{"task_id":i}} for i in ids]
with open(".fist-wf-20260926/_st.json","w") as f:
    json.dump(seq,f)
out = subprocess.run(["python",".fist-wf-20260926/drive.py","seq",".fist-wf-20260926/_st.json"],
                     capture_output=True, text=True, encoding="utf-8", errors="replace").stdout
print("LEN", len(out), "HEAD", repr(out[:120]))
# find each task's status by locating its id then the next "status":"..."
for tid in ids:
    m = re.search(r'id\\":\\"'+re.escape(tid)+r'\\"', out)
    if not m:
        print(tid, "NOT FOUND"); continue
    seg = out[m.end():m.end()+600]
    sm = re.search(r'status\\":\\"([^"\\]+)', seg)
    val = sm.group(1) if sm else "?"
    with open(".fist-wf-20260926/_status.txt","a",encoding="utf-8") as fh:
        fh.write(f"{tid} => {val}\n")
    print(tid, "=>", val)

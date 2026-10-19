# -*- coding: utf-8 -*-
"""FIST-Mbt MCP one-shot driver (session .fist-wf-20260926).

Usage:
  python drive.py tools:list
  python drive.py call <tool> <json-args-file-or-inline-json>
  python drive.py seq <json-file>   # [{tool, args}, ...] sequential, print each result

Server: node E:/IDEProjects/AI/FIST-Mbt/_build/js/debug/build/cmd/main/main.js
cwd:    E:/IDEProjects/AI/lang-zone  (uses lang-zone/fist-mbt.db, bugs.md ledger)
"""
import subprocess, json, sys, os, io

NODE = "node"
MAIN = r"E:\IDEProjects\AI\FIST-Mbt\_build\js\debug\build\cmd\main\main.js"
CWD = r"E:\IDEProjects\AI\lang-zone"
META = {
    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
    "io.modelcontextprotocol/clientCapabilities": {},
    "io.modelcontextprotocol/clientInfo": {"name": "wf-driver", "version": "0.1.0"},
}

def start():
    return subprocess.Popen([NODE, MAIN], cwd=CWD,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL,
                            text=True, encoding="utf-8", errors="replace", bufsize=1)

def rpc(proc, rid, method, params):
    req = {"jsonrpc": "2.0", "id": rid, "method": method, "params": params}
    proc.stdin.write(json.dumps(req, ensure_ascii=False) + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        raise RuntimeError("server closed stdout (no response)")
    return json.loads(line)

def call_tool(proc, rid, name, args):
    params = {"_meta": dict(META), "name": name, "arguments": args}
    return rpc(proc, rid, "tools/call", params)

def out(x):
    sys.stdout.buffer.write((json.dumps(x, ensure_ascii=False, indent=1) + "\n")
                            .encode("utf-8"))

def main():
    mode = sys.argv[1]
    proc = start()
    rid = 0
    try:
        if mode == "tools:list":
            rid += 1
            r = rpc(proc, rid, "tools/list", {"_meta": dict(META)})
            tools = r.get("result", {}).get("tools", [])
            out([t["name"] for t in tools])
        elif mode == "call":
            name = sys.argv[2]
            raw = sys.argv[3]
            args = json.loads(io.open(raw, encoding="utf-8").read()) if os.path.isfile(raw) else json.loads(raw)
            rid += 1
            r = call_tool(proc, rid, name, args)
            out(r.get("result", r))
        elif mode == "seq":
            plan = json.loads(io.open(sys.argv[2], encoding="utf-8").read())
            results = []
            for step in plan:
                rid += 1
                r = call_tool(proc, rid, step["tool"], step.get("args", {}))
                res = r.get("result", r)
                results.append({"tool": step["tool"], "result": res})
                out({"tool": step["tool"], "result": res})
        else:
            out({"error": "unknown mode"})
    finally:
        try:
            proc.stdin.close()
        except Exception:
            pass
        try:
            proc.wait(timeout=5)
        except Exception:
            proc.kill()

if __name__ == "__main__":
    main()

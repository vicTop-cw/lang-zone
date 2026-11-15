#!/usr/bin/env python3
"""FIST-Mbt stdio MCP client (black-box: only talks JSON-RPC to `fist-mbt serve`).

Usage:
  python fist.py --schema publish          # print JSON schema of one tool
  python fist.py --list                    # list all tool names
  python fist.py call publish '{"...":...}' # invoke one tool, print text result
  python fist.py call-many ops.json        # ops.json = [{"tool":..,"args":{..}}, ..]
"""
import json
import os
import shutil
import subprocess
import sys
import threading
from queue import Queue, Empty


def _resolve_cmd():
    exe = shutil.which("fist-mbt") or shutil.which("fist-mbt.cmd")
    if exe and exe.lower().endswith((".cmd", ".bat")):
        return ["cmd", "/c", exe, "serve"]
    if exe:
        return [exe, "serve"]
    return ["fist-mbt", "serve"]


CMD = _resolve_cmd()
META = {
    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
    "io.modelcontextprotocol/clientCapabilities": {},
    "io.modelcontextprotocol/clientInfo": {"name": "lang-zone-perf", "version": "1.0.0"},
}


def _expand(v):
    """Replace any string arg of form "@path" (parsed JSON) or "@!path" (raw text)."""
    if isinstance(v, str) and v.startswith("@!"):
        return open(v[2:], encoding="utf-8").read()
    if isinstance(v, str) and v.startswith("@"):
        return json.load(open(v[1:], encoding="utf-8"))
    if isinstance(v, dict):
        return {k: _expand(x) for k, x in v.items()}
    if isinstance(v, list):
        return [_expand(x) for x in v]
    return v


class Client:
    def __init__(self, timeout=90):
        self.p = subprocess.Popen(
            CMD, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, encoding="utf-8", errors="replace", bufsize=1,
        )
        self.q = Queue()
        self.err = Queue()
        self.timeout = timeout
        threading.Thread(target=self._pump, args=(self.p.stdout, self.q), daemon=True).start()
        threading.Thread(target=self._pump, args=(self.p.stderr, self.err), daemon=True).start()
        self._id = 0

    @staticmethod
    def _pump(stream, q):
        for line in iter(stream.readline, ""):
            q.put(line)
        stream.close()

    def _req(self, method, params):
        self._id += 1
        rid = self._id
        msg = {"jsonrpc": "2.0", "id": rid, "method": method,
               "params": dict(params or {}, _meta=META)}
        self.p.stdin.write(json.dumps(msg, ensure_ascii=False) + "\n")
        self.p.stdin.flush()
        while True:
            try:
                line = self.q.get(timeout=self.timeout)
            except Empty:
                raise TimeoutError(f"no response for {method}")
            line = line.strip()
            if not line:
                continue
            try:
                resp = json.loads(line)
            except json.JSONDecodeError:
                continue
            if resp.get("id") == rid:
                return resp

    def tools(self):
        return self._req("tools/list", {}).get("result", {}).get("tools", [])

    def call(self, tool, args):
        resp = self._req("tools/call", {"name": tool, "arguments": _expand(args)})
        if "error" in resp:
            return "ERROR: " + json.dumps(resp["error"], ensure_ascii=False)
        out = []
        for c in resp.get("result", {}).get("content", []) or []:
            out.append(c.get("text", ""))
        return "\n".join(out) if out else json.dumps(resp.get("result", {}), ensure_ascii=False)

    def close(self):
        try:
            self.p.stdin.close()
        except Exception:
            pass
        self.p.terminate()


def main():
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass
    mode = sys.argv[1] if len(sys.argv) > 1 else "--list"
    c = Client()
    try:
        if mode == "--list":
            names = [t.get("name") for t in c.tools()]
            print(len(names), "tools")
            print(", ".join(names))
        elif mode == "--schema":
            want = set(sys.argv[2].split(","))
            for t in c.tools():
                if t.get("name") in want:
                    props = t.get("inputSchema", {}).get("properties", {})
                    req = t.get("inputSchema", {}).get("required", [])
                    print(f"== {t.get('name')}: required={req} props={list(props.keys())}")
        elif mode == "--schema-json":
            for t in c.tools():
                if t.get("name") == sys.argv[2]:
                    print(json.dumps(t, ensure_ascii=False, indent=2))
                    return 0
            print("tool not found: " + sys.argv[2])
        elif mode == "call":
            args = json.loads(sys.argv[3]) if len(sys.argv) > 3 else {}
            print(c.call(sys.argv[2], args))
        elif mode == "call-many":
            ops = json.load(open(sys.argv[2], encoding="utf-8"))
            for op in ops:
                res = c.call(op["tool"], op.get("args", {}))
                print(f"### {op['tool']} {json.dumps(op.get('args', {}), ensure_ascii=False)[:160]}")
                print(res[:4000])
        else:
            print("unknown mode " + mode)
            return 2
    finally:
        c.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
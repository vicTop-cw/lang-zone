"""Black-box MCP client driver for FIST-Mbt.

Drives the registered `fist-mbt serve` MCP server over stdio by speaking
JSON-RPC. This is a thin client only — it never touches FIST-Mbt source.
Used to honor the FIST-Mbt self-driving combination loop (修复/寻虫/修复)
with issue_up, call_log, laya, and Omega strong-verification always on.

Usage:
  python fist_driver.py <tool> [json-args-string]
  python fist_driver.py list          # list all tools
"""
import subprocess, json, sys, threading, time

SERVER_CMD = "fist-mbt serve"
PROTOCOL_VERSION = "2026-07-28"  # FIST-Mbt MCP server negotiated version


def _spawn():
    return subprocess.Popen(
        SERVER_CMD,
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, encoding="utf-8", bufsize=1, shell=True,
    )


def _read_line(p, timeout=20):
    box = {}

    def reader():
        try:
            box["line"] = p.stdout.readline()
        except Exception:
            box["line"] = None

    t = threading.Thread(target=reader)
    t.start()
    t.join(timeout)
    return box.get("line")


def _rpc(p, method, params=None, req_id=None):
    msg = {"jsonrpc": "2.0", "method": method}
    if req_id is not None:
        msg["id"] = req_id
    if params is not None:
        msg["params"] = params
    p.stdin.write(json.dumps(msg) + "\n")
    p.stdin.flush()


def call_tool(tool, args=None, timeout=30):
    """Initialize a fresh session, then call one tool. Returns parsed result dict."""
    p = _spawn()
    try:
        _rpc(p, "initialize", {
            "protocolVersion": PROTOCOL_VERSION,
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
                "io.modelcontextprotocol/clientCapabilities": {},
            },
            "capabilities": {},
            "clientInfo": {"name": "lz-driver", "version": "0.1"},
        }, req_id=1)
        _read_line(p, 5)  # consume initialize response
        _rpc(p, "notifications/initialized")
        _rpc(p, "tools/call", {"name": tool, "arguments": args or {}}, req_id=2)
        line = _read_line(p, timeout)
    finally:
        try:
            p.stdin.close()
        except Exception:
            pass
        if p.poll() is None:
            p.kill()
    if not line:
        return {"_error": "no response from server"}
    try:
        return json.loads(line)
    except Exception as e:
        return {"_error": f"bad json: {e}", "_raw": line}


def list_tools():
    p = _spawn()
    try:
        _rpc(p, "initialize", {
            "protocolVersion": PROTOCOL_VERSION,
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
                "io.modelcontextprotocol/clientCapabilities": {},
            },
            "capabilities": {},
            "clientInfo": {"name": "lz-driver", "version": "0.1"},
        }, req_id=1)
        _read_line(p, 5)
        _rpc(p, "notifications/initialized")
        _rpc(p, "tools/list", {}, req_id=2)
        line = _read_line(p, 20)
    finally:
        try:
            p.stdin.close()
        except Exception:
            pass
        if p.poll() is None:
            p.kill()
    if not line:
        return []
    try:
        data = json.loads(line)
        return data.get("result", {}).get("tools", [])
    except Exception:
        return []


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("usage: python fist_driver.py <tool> [json-args]")
        sys.exit(1)
    if sys.argv[1] == "list":
        tools = list_tools()
        for t in tools:
            print(t.get("name"))
        print(f"-- total {len(tools)} tools")
    else:
        args = json.loads(sys.argv[2]) if len(sys.argv) > 2 else {}
        print(json.dumps(call_tool(sys.argv[1], args), ensure_ascii=False, indent=2))

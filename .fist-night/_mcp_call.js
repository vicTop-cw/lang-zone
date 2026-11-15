// FIST-Mbt MCP stdio driver (night-loop).
// Usage: node _mcp_call.js <requests.json>   (cwd = .fist-night)
// requests.json = [ {name, arguments}, ... ]
const { spawn } = require('child_process');
const fs = require('fs');

const SERVER = 'E:/IDEProjects/AI/FIST-Mbt/_build/js/debug/build/cmd/main/main.js';
const META = {
  'io.modelcontextprotocol/protocolVersion': '2026-07-28',
  'io.modelcontextprotocol/clientCapabilities': {},
  'io.modelcontextprotocol/clientInfo': { name: 'lz-night-driver', version: '1.0.0' },
};

const reqFile = process.argv[2];
if (!reqFile) { console.error('usage: node _mcp_call.js <requests.json>'); process.exit(2); }

let reqs;
try { reqs = JSON.parse(fs.readFileSync(reqFile, 'utf8')); }
catch (e) { console.error('bad requests file: ' + e.message); process.exit(2); }

const child = spawn(process.execPath, [SERVER], { stdio: ['pipe', 'pipe', 'pipe'] });
let buf = '';
const lines = [];
let nrecv = 0;
let done = false;
let stderr = '';

child.stdout.on('data', (d) => {
  buf += d.toString('utf8');
  let nl;
  while ((nl = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, nl).trim();
    buf = buf.slice(nl + 1);
    if (!line) continue;
    lines.push(line);
    nrecv++;
    if (nrecv >= reqs.length) finish();
  }
});
child.stderr.on('data', (d) => { stderr += d.toString('utf8'); });
child.on('exit', () => { if (!done) finish(); });

function finish() {
  if (done) return;
  done = true;
  const rows = lines.map((line, i) => {
    let parsed = null;
    try { parsed = JSON.parse(line); } catch (e) { return `#${i} ${(reqs[i] || {}).name} -> RAW ` + line; }
    let payload = parsed.error || parsed.result;
    if (payload && payload.content && Array.isArray(payload.content)) {
      payload = payload.content.map((c) => (c && c.text) || '').join('\n');
    }
    return `#${i} ${(reqs[i] || {}).name} -> ` + (typeof payload === 'string' ? payload : JSON.stringify(payload));
  });
  process.stdout.write(rows.join('\n') + '\n');
  if (stderr.trim()) process.stdout.write('--- stderr ---\n' + stderr.trim() + '\n');
  try { child.kill(); } catch (e) {}
  process.exit(0);
}

for (let i = 0; i < reqs.length; i++) {
  const msg = { jsonrpc: '2.0', id: String(i), method: 'tools/call',
    params: { name: reqs[i].name, arguments: reqs[i].arguments || {}, _meta: META } };
  child.stdin.write(JSON.stringify(msg) + '\n');
}

setTimeout(() => { if (!done) { process.stdout.write('TIMEOUT after ' + lines.length + '/' + reqs.length + '\n'); process.stdout.write(lines.join('\n') + '\n'); finish(); } }, 30000);

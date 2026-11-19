// 全量双轨比对脚本（绕过 cargo test 进程管理，避免 cron 干扰杀进程）
// 用法: node fullgate.js <lang-zone.exe> <起> <止>
// 对 DEMO 非负向样本排序后 [起,止) 区间逐个跑 Rust --tokens vs LZ --emit=lex-lz
// 归一化逻辑复刻 normalize_token_stream（剥序号/头行/Debug 转义 vs 裸 payload）
const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const bin = process.argv[2];
const from = parseInt(process.argv[3] || '0', 10);
const to = parseInt(process.argv[4] || '999999', 10);

function debugUnescape(s) {
  let out = '';
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c !== '\\') { out += c; continue; }
    const n = s[++i];
    if (n === 'n') out += '\n';
    else if (n === 'r') out += '\r';
    else if (n === 't') out += '\t';
    else if (n === '0') out += '\0';
    else if (n === '"') out += '"';
    else if (n === '\\') out += '\\';
    else if (n === "'") out += "'";
    else if (n === 'u') {
      // \u{XXXX}
      let hex = '';
      if (s[i + 1] === '{') { i += 2; while (i < s.length && s[i] !== '}') { hex += s[i++]; } }
      const cp = parseInt(hex, 16);
      out += Number.isFinite(cp) ? String.fromCodePoint(cp) : ('\\u{' + hex + '}');
    } else { out += '\\' + n; }
  }
  return out;
}

function normalize(raw, rustSide) {
  const out = [];
  for (let line of raw.split(/\r?\n/)) {
    let l = line.trim();
    if (!l || l.startsWith('=== Tokens ===')) continue;
    if (rustSide) {
      const pos = l.indexOf(' ');
      if (pos > 0 && /^\d+$/.test(l.slice(0, pos))) l = l.slice(pos).trim();
    }
    l = l.replace(/^"+|"+$/g, '');
    if (!l) continue;
    const open = l.indexOf('('), close = l.lastIndexOf(')');
    if (open >= 0 && close > open) {
      const variant = l.slice(0, open);
      let payload = l.slice(open + 1, close);
      if (payload.length >= 2 && payload.startsWith('"') && payload.endsWith('"')) {
        payload = debugUnescape(payload.slice(1, -1));
      } else {
        // LZ display 的裸 payload 也是 Debug 转义形态（escape_for_debug），
        // 与 Rust 侧统一 unescape（对齐 bootstrap_emit::normalize_token_stream）
        payload = debugUnescape(payload);
      }
      out.push(variant + '(' + payload + ')');
    } else if (l.startsWith('EmbedBlock')) {
      // EmbedBlock { lang: "X", src: "...", form: Y }
      const lang = field(l, 'lang'), src = field(l, 'src'), form = field(l, 'form');
      out.push('EmbedBlock(' + lang + ',' + debugUnescape(src) + ',' + form + ')');
    } else {
      out.push(l);
    }
  }
  return out;
}

function field(line, key) {
  const m = line.indexOf(key + ':');
  if (m < 0) return '';
  let rest = line.slice(m + key.length + 1).trim();
  if (rest.startsWith('"')) {
    const inner = rest.slice(1);
    const end = inner.lastIndexOf('"');
    return end >= 0 ? inner.slice(0, end) : inner;
  }
  const comma = rest.indexOf(',');
  return (comma >= 0 ? rest.slice(0, comma) : rest).trim();
}

function collect(dir, out) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) {
      if (e.name === '99_errors') continue;
      collect(p, out);
    } else if (e.name.endsWith('.lz')) out.push(p);
  }
}

function runOnce(args) {
  // 瞬时启动失败（并发抢占/杀进程）重试一次，避免把环境抖动当分歧
  let last;
  for (let k = 0; k < 2; k++) {
    try {
      return execFileSync(bin, args, { encoding: 'utf8', timeout: 120000, maxBuffer: 50 * 1024 * 1024 });
    } catch (e) { last = e; }
  }
  throw last;
}

const samples = [];
collect(path.join(path.resolve(__dirname, '..'), 'DEMO'), samples);
samples.sort();
const total = samples.length;
console.log('total=' + total + ' range=[' + from + ',' + to + ')');
const failed = [];
for (let i = from; i < Math.min(to, total); i++) {
  const s = samples[i];
  let rustOut, lzOut;
  try {
    rustOut = runOnce([s, '--tokens']);
    lzOut = runOnce([s, '--emit=lex-lz', '--std-dir', 'std']);
  } catch (e) {
    failed.push(s + ': spawn fail ' + (e.status !== undefined ? 'exit=' + e.status : e.message));
    continue;
  }
  const rn = normalize(rustOut, true), ln = normalize(lzOut, false);
  if (JSON.stringify(rn) !== JSON.stringify(ln)) {
    let first = -1;
    for (let k = 0; k < Math.max(rn.length, ln.length); k++) {
      if (rn[k] !== ln[k]) { first = k; break; }
    }
    failed.push(s + ': mismatch at ' + first + ' rust=' + (rn[first] || '<none>') + ' lz=' + (ln[first] || '<none>'));
  }
  if ((i + 1) % 25 === 0) console.log('[progress] ' + (i + 1 - from) + '/' + (Math.min(to, total) - from));
}
console.log('=== RESULT ===');
console.log('checked=' + (Math.min(to, total) - from) + ' failed=' + failed.length);
for (const f of failed) console.log('FAIL: ' + f);
process.exit(failed.length ? 1 : 0);

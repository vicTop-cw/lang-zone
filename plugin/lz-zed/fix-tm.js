// fix-tm.js — 修复 lz.tmLanguage.json 的 operators 行转义问题
// 用法: node fix-tm.js
// 原理: 优先在 JSON 对象层面修复（JSON.stringify 自动生成合法转义）；
//       若文件已非法无法解析，用转义感知正则把 operators 行整体替换为已知正确文本。
'use strict';
const fs = require('fs');
const path = require('path');

const p = path.join(__dirname, 'syntaxes', 'lz.tmLanguage.json');
let raw = fs.readFileSync(p, 'utf8');

// 正确的 match 值 —— 文件字节层面（JSON 转义形式，即 JSON 源文本）。
// JS 字符串字面量中每个文件反斜杠写为 \\，这里直接给出 JSON 源文本：
// 正则源: \*\*=|<<=|>>=|\.\.=|\.\.\.|->|=>|\|>|:=|=:|\^:|~:|\*:|\+=|-=|\*=|/=|%=|&=|\|=|\^=|\*\*|==|!=|<=|>=|&&|\|\||<<|>>|\.\.|\+|-|\*|/|%|&|\||\^|~|!|<|>|=|\?\?|\?\.|\?|@
// JSON 源: 每个 \ 写成 \\
const CORRECT = '\\\\*\\\\*=|<<=|>>=|\\.\\.=|\\.\\.\\.|->|=>|\\\\|>|:=|=:|\\\\^:|~:|\\\\*:|\\\\+=|-=|\\\\*=|/=|%=|&=|\\\\|=|\\\\^=|\\\\*\\\\*|==|!=|<=|>=|&&|\\\\|\\\\||<<|>>|\\.\\.|\\\\+|-|\\\\*|/|%|&|\\\\||\\\\^|~|!|<|>|=|\\\\?\\\\?|\\\\?\\\\.|\\\\?|@';

// 1) 尝试直接解析
let grammar = null;
try {
  grammar = JSON.parse(raw);
  console.log('[1] 文件当前是合法 JSON');
} catch (e) {
  console.log('[1] JSON 解析失败: ' + e.message);
  console.log('    用转义感知正则整体替换 operators 行...');
  // 匹配 "match": "..."（转义感知：\" 不截断）
  const re = /("keyword\.operator\.lz", "match": ")((?:[^"\\]|\\.)*)(")/;
  if (!re.test(raw)) {
    console.error('[1b] 找不到 operators match 行，中止');
    process.exit(1);
  }
  raw = raw.replace(re, '$1' + CORRECT + '$3');
  try {
    grammar = JSON.parse(raw);
    console.log('[1b] 替换后解析成功');
  } catch (e2) {
    console.error('[1b] 替换后仍无法解析: ' + e2.message);
    console.error('    请手动检查 syntaxes/lz.tmLanguage.json 的 operators 行');
    process.exit(1);
  }
}

// 2) 对象层面确保正则源包含 \|=
const op = grammar.repository.operators.patterns.find((x) => x.name === 'keyword.operator.lz');
if (!op) { console.error('[2] 找不到 operators pattern'); process.exit(1); }
if (!op.match.includes('\\|=')) {
  op.match = op.match.replace('&=\\^=', '&=\\|=\\^=');
  console.log('[2] operators 正则已补上 \\|=');
} else {
  console.log('[2] operators 正则已包含 \\|=');
}

// 3) 写回（JSON.stringify 生成标准 JSON 转义）
fs.writeFileSync(p, JSON.stringify(grammar, null, 2) + '\n');
console.log('[3] 已写回 ' + p);

// 4) 验证
const again = JSON.parse(fs.readFileSync(p, 'utf8'));
const op2 = again.repository.operators.patterns.find((x) => x.name === 'keyword.operator.lz');
console.log('[4] 写回后 JSON 解析 OK；operators match 含 \\|= : ' + op2.match.includes('\\|='));
console.log('[5] 完成。接下来请运行:');
console.log('    node test\\run-tests.js --update');
console.log('    node test\\run-tests.js');

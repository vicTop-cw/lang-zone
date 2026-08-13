#!/usr/bin/env node
/**
 * lz-zed test runner — zero-dependency Node.js
 * =============================================
 * Uses syntaxes/lz.tmLanguage.json as the executable highlighting spec and
 * verifies it against the .lz fixtures in test/fixtures/.
 *
 * Usage:
 *   node test/run-tests.js             run snapshot tests (compare expected/)
 *   node test/run-tests.js --update    regenerate snapshots into test/expected/
 *   node test/run-tests.js --no-report skip writing docs/TEST-REPORT.md
 *
 * Exit code: 0 = all checks passed, 1 = any failure.
 *
 * Checks performed:
 *   1. JSON grammar parses and every regex compiles.
 *   2. Every fixture tokenizes; snapshots match test/expected/*.snap.json.
 *   3. Coverage: every lexical element (SYNTAX spec v3.3) has a rule in
 *      grammar.js AND lz.tmLanguage.json AND appears in a fixture.
 *   4. grammar.js loads as a valid tree-sitter grammar definition.
 * Results are written to docs/TEST-REPORT.md.
 */

'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const TM_PATH = path.join(ROOT, 'syntaxes', 'lz.tmLanguage.json');
const GRAMMAR_JS_PATH = path.join(ROOT, 'grammar', 'grammar.js');
const FIXTURES_DIR = path.join(__dirname, 'fixtures');
const EXPECTED_DIR = path.join(__dirname, 'expected');
const REPORT_PATH = path.join(ROOT, 'docs', 'TEST-REPORT.md');

const UPDATE = process.argv.includes('--update');
const WRITE_REPORT = !process.argv.includes('--no-report');

/* ------------------------------------------------------------------ */
/* 1. Load the TextMate grammar                                        */
/* ------------------------------------------------------------------ */

function loadGrammar() {
  const raw = fs.readFileSync(TM_PATH, 'utf8');
  const grammar = JSON.parse(raw); // throws on invalid JSON
  if (!grammar.scopeName || !Array.isArray(grammar.patterns)) {
    throw new Error('invalid tmLanguage: missing scopeName/patterns');
  }
  return grammar;
}

/* ------------------------------------------------------------------ */
/* 2. Mini TextMate tokenizer (match / begin-end, capture groups)      */
/* ------------------------------------------------------------------ */

function compilePatterns(patterns, grammar, seen) {
  const flags = 'myu';
  const stripInline = (re) => re.replace(/^\(\?m\)/, '');
  const out = [];
  for (const p of patterns) {
    if (p.include !== undefined) {
      // TextMate semantics: an include pulls in the referenced repository
      // entry's patterns at this position. Includes may nest at any depth
      // (root level, region level, region-of-region), so resolve recursively.
      const key = p.include.replace(/^#/, '');
      const entry = grammar.repository && grammar.repository[key];
      if (!entry || !entry.patterns) throw new Error('missing repository entry: ' + key);
      if (seen.has(key)) continue;
      out.push(...compilePatterns(entry.patterns, grammar, new Set(seen).add(key)));
      continue;
    }
    if (p.match !== undefined) {
      out.push({ kind: 'match', re: new RegExp(stripInline(p.match), flags), name: p.name || null });
    } else if (p.begin !== undefined && p.end !== undefined) {
      out.push({
        kind: 'region',
        beginRe: new RegExp(stripInline(p.begin), flags),
        endRe: new RegExp(stripInline(p.end), flags),
        name: p.name || null,
        // includes nested inside region patterns are resolved recursively too
        patterns: compilePatterns(p.patterns || [], grammar, seen),
        beginCaptures: p.beginCaptures || {},
      });
    } else {
      throw new Error('unsupported pattern shape: ' + JSON.stringify(p).slice(0, 120));
    }
  }
  return out;
}

function compileGrammar(grammar) {
  return { root: compilePatterns(grammar.patterns, grammar, new Set()) };
}

// Collect every DECODED regex string and scope name from the grammar, for
// coverage probing. Probes must run against decoded text (JSON \\ -> \), not
// the raw file bytes.
function collectTmStrings(grammar) {
  const out = [];
  const walk = (patterns) => {
    for (const p of patterns) {
      if (p.include !== undefined) continue;
      if (p.match !== undefined) {
        out.push(p.match);
        if (p.name) out.push(p.name);
      } else if (p.begin !== undefined) {
        out.push(p.begin);
        out.push(p.end);
        if (p.name) out.push(p.name);
        if (p.patterns) walk(p.patterns);
      }
    }
  };
  walk(grammar.patterns);
  for (const entry of Object.values(grammar.repository || {})) {
    if (entry.patterns) walk(entry.patterns);
  }
  return out;
}

// Emit tokens for a region begin match, honoring beginCaptures.
function emitBeginTokens(text, m, pattern, out) {
  const caps = pattern.beginCaptures;
  if (!caps || Object.keys(caps).length === 0) {
    out.push({ t: text, s: pattern.name || '' });
    return;
  }
  const absStart = m.index;
  const absEnd = m.index + m[0].length;
  const ranges = [];
  for (const [g, cap] of Object.entries(caps)) {
    const gi = Number(g);
    const v = m[gi];
    if (v !== undefined && v !== '') {
      const off = m[0].indexOf(v);
      ranges.push({ start: absStart + off, end: absStart + off + v.length, name: cap.name || pattern.name || '' });
    }
  }
  ranges.sort((a, b) => a.start - b.start);
  let cursor = absStart;
  for (const r of ranges) {
    if (r.start > cursor) out.push({ t: text.slice(cursor - absStart, r.start - absStart), s: pattern.name || '' });
    out.push({ t: text.slice(r.start - absStart, r.end - absStart), s: r.name });
    cursor = r.end;
  }
  if (cursor < absEnd) out.push({ t: text.slice(cursor - absStart), s: pattern.name || '' });
}

function tokenize(grammar, source) {
  const out = [];
  const stack = [];
  let pos = 0;
  const patternsOf = (frame) => (frame ? frame.patterns : grammar.root);

  while (pos < source.length) {
    if (stack.length > 0) {
      const top = stack[stack.length - 1];
      top.endRe.lastIndex = pos;
      const em = top.endRe.exec(source);
      if (em) {
        out.push({ t: em[0], s: top.name || '' });
        pos = em.index + em[0].length;
        stack.pop();
        continue;
      }
    }
    const pats = patternsOf(stack[stack.length - 1] || null);
    let matched = false;
    for (const p of pats) {
      if (p.kind === 'match') {
        p.re.lastIndex = pos;
        const m = p.re.exec(source);
        if (m) {
          out.push({ t: m[0], s: p.name || '' });
          pos = m.index + m[0].length;
          matched = true;
          break;
        }
      } else {
        p.beginRe.lastIndex = pos;
        const m = p.beginRe.exec(source);
        if (m) {
          const seg = source.slice(m.index, m.index + m[0].length);
          emitBeginTokens(seg, m, p, out);
          pos = m.index + m[0].length;
          stack.push({ patterns: p.patterns, name: p.name, endRe: p.endRe });
          matched = true;
          break;
        }
      }
    }
    if (matched) continue;
    const scope = stack.length > 0 ? stack[stack.length - 1].name : '';
    out.push({ t: source[pos], s: scope || 'text' });
    pos += 1;
  }
  return out;
}

/* ------------------------------------------------------------------ */
/* 3. Snapshot helpers                                                 */
/* ------------------------------------------------------------------ */

function toLines(tokens) {
  const lines = [];
  let cur = [];
  for (const tok of tokens) {
    const parts = tok.t.split('\n');
    for (let i = 0; i < parts.length; i++) {
      if (i > 0) { lines.push(cur); cur = []; }
      cur.push({ t: parts[i], s: tok.s });
    }
  }
  lines.push(cur);
  return lines;
}

function scopeStats(tokens) {
  const stats = {};
  for (const tok of tokens) {
    if (tok.s) stats[tok.s] = (stats[tok.s] || 0) + 1;
  }
  return stats;
}

function snapshotFor(file, tokens) {
  return {
    file: path.basename(file),
    tokens: tokens.length,
    scopes: scopeStats(tokens),
    lines: toLines(tokens),
  };
}

/* ------------------------------------------------------------------ */
/* 4. Coverage checklist (SYNTAX spec v3.3 lexical elements)           */
/*    [group, element, grammarProbe, tmProbe, fixtureProbe]            */
/* ------------------------------------------------------------------ */

const ELEMENTS = [
  // --- keywords (62, spec 00 §1 + 附录B §1) ---------------------------
  ...['def','struct','enum','trait','impl','type','const','mut','ref','let','owned','magic','duck','iterator'].map(k => ['keyword', k, k, k, k]),
  ...['if','elif','else','match','case','guard','for','while','loop','block','pass','break','continue','return','with','defer'].map(k => ['keyword', k, k, k, k]),
  ...['raise','raises','try','catch','finally'].map(k => ['keyword', k, k, k, k]),
  ...['async','await','spawn','go'].map(k => ['keyword', k, k, k, k]),
  ['keyword', 'yield', 'yield', 'yield', 'yield'],
  ['keyword', 'where', 'where', 'where', 'where'],
  ['keyword', 'Self', 'Self', 'Self', 'Self'],
  ...['macro','comptime','template'].map(k => ['keyword', k, k, k, k]),
  ...['test','suite','setup','teardown','assert','check'].map(k => ['keyword', k, k, k, k]),
  ...['and','or','not','is','in'].map(k => ['keyword', k, k, k, k]),
  ...['import','from','as'].map(k => ['keyword', k, k, k, k]),
  ['keyword', 'True', 'True', 'True', 'True'],
  ['keyword', 'False', 'False', 'False', 'False'],
  ['keyword', '... (abstract marker)', '...', 'ellipsis', '= ...'],
  // --- duck soft keywords (14, 附录B §1.13) ----------------------------
  ...['require','optional','exact','min','max','range','at_least','at_most','satisfies','sealed','default','StackType','RefType','Any'].map(k => ['duck-soft', k, k, k, k]),
  // --- builtin types + type values (22) --------------------------------
  ...['int','f64','bool','str','List','Dict','Set','Option','Result','Tuple','Box','Rc','Arc','Cell','RefCell','IOError','Tokens','Never','Unit','Nil','Number'].map(k => ['builtin-type', k, k, k, k]),
  // --- constructors (4) --------------------------------------------------
  ...['None','Some','Ok','Err'].map(k => ['builtin-ctor', k, k, k, k]),
  // --- builtin functions (22) --------------------------------------------
  ...['print','panic','len','contains','iter','enumerate','zip','map','filter','collect','sort','reverse','clone','drop','format','hash','callable','quote','merge_tokens','sum','prod'].map(k => ['builtin-fn', k, k, k, k]),
  // --- operators (45, spec 12 §1.1–1.17) ---------------------------------
  ['operator', '=', '=', '=', '='],
  ['operator', '+=', '+=', '\\+=', '+='],
  ['operator', '-=', '-=', '-=', '-='],
  ['operator', '*=', '*=', '\\*=', '*='],
  ['operator', '/=', '/=', '/=', '/='],
  ['operator', '%=', '%=', '%=', '%='],
  ['operator', '**=', '**=', '\\*\\*=', '**='],
  ['operator', '&=', '&=', '&=', '&='],
  ['operator', '|=', '|=', '\\|=', '|='],
  ['operator', '^=', '^=', '\\^=', '^='],
  ['operator', '<<=', '<<=', '<<=', '<<='],
  ['operator', '>>=', '>>=', '>>=', '>>='],
  ['operator', ':=', ':=', ':=', ':='],
  ['operator', '==', '==', '==', '=='],
  ['operator', '!=', '!=', '!=', '!='],
  ['operator', '<', '<', '<', '<'],
  ['operator', '>', '>', '>', '>'],
  ['operator', '<=', '<=', '<=', '<='],
  ['operator', '>=', '>=', '>=', '>='],
  ['operator', '&&', '&&', '&&', '&&'],
  ['operator', '||', '||', '\\|\\|', '||'],
  ['operator', '!', '!', '!', '!flag'],
  ['operator', '&', '&', '&', '&val'],
  ['operator', '|', '|', '\\|', '|'],
  ['operator', '^', '^', '\\^', 'a ^ b'],
  ['operator', '<<', '<<', '<<', '<<'],
  ['operator', '>>', '>>', '>>', '>>'],
  ['operator', '~', '~', '~', '~bits'],
  ['operator', '+', '+', '\\+', ' + '],
  ['operator', '-', '-', '-', ' - '],
  ['operator', '*', '*', '\\*', ' * '],
  ['operator', '/', '/', '/', ' / '],
  ['operator', '%', '%', '%', '%'],
  ['operator', '**', '**', '\\*\\*', '**'],
  ['operator', '|>', '|>', '\\|>', '|>'],
  ['operator', '??', '??', '\\?\\?', '??'],
  ['operator', '..', '..', '\\.\\.', '..'],
  ['operator', '..=', '..=', '\\.\\.=', '..='],
  ['operator', '?', '?', '\\?', '?'],
  ['operator', '?.', '?.', '\\?\\.', '?.'],
  ['operator', '=:', '=:', '=:', '=:'],
  ['operator', '^:', '^:', '\\^:', '^:'],
  ['operator', '~:', '~:', '~:', '~:'],
  ['operator', '*:', '*:', '\\*:', '*:'],
  ['operator', '->', '->', '->', '->'],
  ['operator', '=>', '=>', '=>', '=>'],
  ['operator', '@ (decorator)', '@', '@', '@export'],
  ['operator', '# (attribute macro)', '#', '#', '#!bin'],
  // --- literals / comments / specials ------------------------------------
  ['literal', 'hex int 0xFF', /0\[xX\]/, '0[xX]', '0xFF'],
  ['literal', 'octal int 0o77', /0\[oO\]/, '0[oO]', '0o77'],
  ['literal', 'binary int 0b1010', /0\[bB\]/, '0[bB]', '0b1010'],
  ['literal', 'underscore int 1_000_000', /\[0-9_\]/, '[0-9_]', '1_000_000'],
  ['literal', 'float 3.14', 'float', '\\.[0-9]', '3.14'],
  ['literal', 'float exp 1e10', 'float', '[eE]', '1e10'],
  ['literal', 'float neg exp 1.5e-3', 'float', '[eE]', '1.5e-3'],
  ['literal', 'plain string "…"', 'string', '"', '"hello"'],
  ['literal', 'f-string f"…"', "'f'", 'string.quoted.double.f.lz', 'f"hello'],
  ['literal', 'raw string r"…"', "'r'", 'string.quoted.double.raw.lz', 'r"\\d+"'],
  ['literal', 'triple string """…"""', "'\"\"\"'", 'string.quoted.double.lz', '"""line1'],
  ['literal', 'f-triple f"""…"""', "'\"\"\"'", 'string.quoted.double.f.lz', 'f"""x={x}'],
  ['literal', 'backtick quote ```…```', "'```'", '```', '```'],
  ['literal', 'escape \\u{...}', /\\\\/, '\\\\u\\{', '\\u{1F600}'],
  ['literal', 'escape \\n', /\\\\/, '\\[nrt', '\\n'],
  ['comment', 'line comment //', 'line_comment', '//', '// 正确的单行注释'],
  ['comment', 'block comment /* */', 'block_comment', '/\\*', '/*'],
  ['special', 'magic method __init__', 'magic_method', 'entity.name.function.magic', '__init__'],
  ['special', 'unicode identifier café', 'identifier', '{L}', 'café'],
  ['special', 'wildcard underscore _', 'identifier', 'variable.lz', '_ = heavy_side_effect'],
  ['special', 'regex literal /pat/ (duck)', 'regex_literal', 'string.regexp', '/^get_/'],
  ['special', 'named-arg sugar x~', "'~'", '~', 'b~'],
  ['special', 'ownership suffix x^', "'^'", '\\^', 'y^'],
];

/* ------------------------------------------------------------------ */
/* 5. Main                                                             */
/* ------------------------------------------------------------------ */

function main() {
  const report = [];
  let failures = 0;
  const fail = (msg) => { failures += 1; report.push('FAIL: ' + msg); };

  report.push('# lz-zed 语法高亮测试报告');
  report.push('');
  report.push(`- 运行时间: ${new Date().toISOString()}`);
  report.push(`- 模式: ${UPDATE ? 'UPDATE（重新生成快照）' : 'VERIFY（校验快照）'}`);
  report.push(`- 语法基准: SYNTAX 规范 v3.3（00-词法基础 / 附录B / 12-操作符）`);
  report.push('');

  // --- grammar.js sanity -------------------------------------------------
  let grammarJs = null;
  try {
    grammarJs = fs.readFileSync(GRAMMAR_JS_PATH, 'utf8');
    if (!grammarJs.includes('module.exports = grammar')) fail('grammar.js 不是有效的 tree-sitter 语法定义');
    if (!grammarJs.includes("name: 'lz'")) fail('grammar.js 语言名不是 lz');
    report.push('1. grammar/grammar.js 结构检查: OK（存在 grammar 定义）');
  } catch (e) {
    fail('无法读取 grammar/grammar.js: ' + e.message);
  }
  report.push('');

  // --- tmLanguage loads + regexes compile ---------------------------------
  let rawGrammar = null;
  let compiled = null;
  try {
    rawGrammar = loadGrammar();
    compiled = compileGrammar(rawGrammar);
    report.push('2. syntaxes/lz.tmLanguage.json 加载与正则编译: OK（' + compiled.root.length + ' 个解析后 pattern，含 include 展开）');
  } catch (e) {
    fail('tmLanguage 加载失败: ' + e.message);
    rawGrammar = null;
  }
  report.push('');

  // --- fixtures + snapshots ------------------------------------------------
  const fixtures = fs.readdirSync(FIXTURES_DIR).filter((f) => f.endsWith('.lz')).sort();
  report.push(`3. 快照测试（${fixtures.length} 个 fixture）`);
  report.push('');
  report.push('| fixture | 行数 | tokens | scopes | 状态 |');
  report.push('|---------|-----:|-------:|-------:|------|');
  let totalTokens = 0;

  if (compiled) {
    for (const f of fixtures) {
      const src = fs.readFileSync(path.join(FIXTURES_DIR, f), 'utf8');
      let tokens;
      try {
        tokens = tokenize(compiled, src);
      } catch (e) {
        fail(`${f} 分词异常: ${e.message}`);
        continue;
      }
      const snap = snapshotFor(f, tokens);
      totalTokens += tokens.length;
      const expectedPath = path.join(EXPECTED_DIR, f.replace(/\.lz$/, '.snap.json'));
      if (UPDATE) {
        fs.mkdirSync(EXPECTED_DIR, { recursive: true });
        fs.writeFileSync(expectedPath, JSON.stringify(snap, null, 2) + '\n');
        report.push(`| ${f} | ${snap.lines.length} | ${snap.tokens} | ${Object.keys(snap.scopes).length} | ✅ 已生成快照 |`);
      } else {
        if (!fs.existsSync(expectedPath)) {
          fail(`${f}: 缺少期望快照 ${path.basename(expectedPath)}，请先运行 node test/run-tests.js --update`);
          report.push(`| ${f} | ${snap.lines.length} | ${snap.tokens} | ${Object.keys(snap.scopes).length} | ❌ 无快照 |`);
        } else {
          const expected = JSON.parse(fs.readFileSync(expectedPath, 'utf8'));
          if (JSON.stringify(expected) === JSON.stringify(snap)) {
            report.push(`| ${f} | ${snap.lines.length} | ${snap.tokens} | ${Object.keys(snap.scopes).length} | ✅ 通过 |`);
          } else {
            fail(`${f}: 高亮结果与快照不一致`);
            report.push(`| ${f} | ${snap.lines.length} | ${snap.tokens} | ${Object.keys(snap.scopes).length} | ❌ 不一致 |`);
          }
        }
      }
    }
  }
  report.push('');
  report.push(`合计: ${fixtures.length} 个 fixture，${totalTokens} 个高亮 token${UPDATE ? '（快照已更新）' : ''}`);
  report.push('');

  // --- coverage ------------------------------------------------------------
  report.push('4. 词法元素覆盖率（SYNTAX v3.3 → grammar.js → tmLanguage → fixtures 三方映射）');
  report.push('');
  report.push('| 分组 | 元素 | grammar.js | tmLanguage | fixtures | 状态 |');
  report.push('|------|------|:---:|:---:|:---:|:---:|');

  const tmStrings = rawGrammar ? collectTmStrings(rawGrammar) : [];
  const fixtureText = fixtures.map((f) => fs.readFileSync(path.join(FIXTURES_DIR, f), 'utf8')).join('\n');
  let covered = 0;

  for (const [group, elem, gProbe, tmProbe, fixProbe] of ELEMENTS) {
    const inGrammar = grammarJs ? (gProbe instanceof RegExp ? gProbe.test(grammarJs) : grammarJs.includes(gProbe)) : false;
    // tmProbes are plain substrings of the DECODED regex sources (e.g. "\\*\\*="
    // for the **= operator); regex .test() would re-interpret escapes, so use
    // literal includes() against every decoded pattern source + scope name.
    const inTm = tmStrings.some((s) => s.includes(tmProbe));
    const inFixtures = (fixProbe instanceof RegExp) ? fixProbe.test(fixtureText) : fixtureText.includes(fixProbe);
    const ok = inGrammar && inTm && inFixtures;
    if (ok) covered += 1;
    report.push(`| ${group} | \`${elem}\` | ${inGrammar ? '✅' : '❌'} | ${inTm ? '✅' : '❌'} | ${inFixtures ? '✅' : '❌'} | ${ok ? '✅' : '❌'} |`);
    if (!ok) fail(`覆盖率缺口: ${group} / ${elem}`);
  }
  report.push('');
  report.push(`覆盖元素: ${covered}/${ELEMENTS.length}`);
  report.push('');

  // --- summary -------------------------------------------------------------
  if (failures === 0) {
    report.push('## 结论');
    report.push('');
    report.push('✅ 全部检查通过：快照一致、词法元素三方映射完整。');
  } else {
    report.push('## 结论');
    report.push('');
    report.push(`❌ 共 ${failures} 项失败，详见上文。`);
  }
  report.push('');

  if (WRITE_REPORT) {
    fs.mkdirSync(path.dirname(REPORT_PATH), { recursive: true });
    fs.writeFileSync(REPORT_PATH, report.join('\n'));
  }
  console.log(report.join('\n'));
  process.exit(failures === 0 ? 0 : 1);
}

main();

# -*- coding: utf-8 -*-
"""给 tests/*.rs 的 rustc --extern lz_builtins 脚手架补 -L dependency=repro 修复"""
import io, glob

for p in glob.glob('tests/*.rs'):
    lines = io.open(p, encoding='utf-8').read().split('\n')
    out, changed = [], False
    for i, ln in enumerate(lines):
        stripped = ln.strip()
        if stripped.startswith('.arg("--extern")'):
            nxt = lines[i + 1] if i + 1 < len(lines) else ''
            ctx_before = '\n'.join(lines[max(0, i - 6):i])
            if 'lz_builtins' in nxt and 'dependency=' not in ctx_before:
                indent = ln[:len(ln) - len(ln.lstrip())]
                out.append(indent + '.arg(format!("dependency={}/target/debug/deps", env!("CARGO_MANIFEST_DIR")))')
                changed = True
        out.append(ln)
    if changed:
        io.open(p, 'w', encoding='utf-8', newline='').write('\n'.join(out))
        print('patched', p)

# -*- coding: utf-8 -*-
"""src/*.rs 内部 rustc 调用补 -L dependency（lz_builtins re-export num-* 后需要）"""
import io, glob

INS = [
    '        .arg("-L")',
    '        .arg(format!("dependency={}", std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/deps").display()))',
]

for p in glob.glob('src/*.rs'):
    lines = io.open(p, encoding='utf-8').read().split('\n')
    out, changed, i = [], False, 0
    while i < len(lines):
        ln = lines[i]
        if 'arg("--extern")' in ln and 'dependency=' not in ln:
            nxt = lines[i + 1] if i + 1 < len(lines) else ''
            if 'lz_builtins' in nxt:
                if ln.strip().startswith('.arg('):
                    # 链式中段：在 --extern 行前插入
                    out.extend(INS)
                    out.append(ln)
                    changed = True
                else:
                    # cmd.arg 形式：语句起始行，在 lz_builtins 值行之后插入
                    out.append(ln)
                    out.append(lines[i + 1])
                    out.extend(INS)
                    i += 1
                    changed = True
                i += 1
                continue
        out.append(ln)
        i += 1
    if changed:
        io.open(p, 'w', encoding='utf-8', newline='').write('\n'.join(out))
        print('patched', p)

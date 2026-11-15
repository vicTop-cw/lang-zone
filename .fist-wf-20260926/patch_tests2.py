# -*- coding: utf-8 -*-
"""给上次插入的 dependency 值参数补上 -L 标志"""
import io, glob

OLD = '.arg(format!("dependency={}/target/debug/deps", env!("CARGO_MANIFEST_DIR")))'
NEW = '.arg("-L")\n                .arg(format!("dependency={}/target/debug/deps", env!("CARGO_MANIFEST_DIR")))'

for p in glob.glob('tests/*.rs'):
    s = io.open(p, encoding='utf-8').read()
    if OLD in s:
        io.open(p, 'w', encoding='utf-8', newline='').write(s.replace(OLD, NEW))
        print('fixed', p)

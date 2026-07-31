import os, sys, subprocess, re
sys.path.insert(0, os.path.dirname(__file__))
import run_tests

import tempfile
CASES_DIR = tempfile.mkdtemp(suffix='_lz_test')
print(f'Using: {CASES_DIR}')

results = []
for case in run_tests.CATALOG:
    cid = case['id']
    mode = case['mode']
    lz_path = os.path.join(CASES_DIR, cid + '.lz')
    with open(lz_path, 'w', encoding='utf-8') as f:
        f.write(case['source'])
    
    args = [run_tests.SUT, cid + '.lz']
    if mode == 'tokens': args.append('--tokens')
    elif mode == 'ast': args.append('--ast')
    if mode != 'error': args.append('--no-strict')
    
    proc = subprocess.run(args, cwd=CASES_DIR, capture_output=True, text=True, timeout=30)
    rc = proc.returncode
    
    rs_path = os.path.join(CASES_DIR, cid + '.rs')
    rs_text = ''
    if os.path.exists(rs_path):
        with open(rs_path, 'r', encoding='utf-8') as f:
            rs_text = f.read()
    
    expected_rc = 1 if mode == 'error' else 0
    problems = []
    if rc != expected_rc:
        problems.append(f'rc={rc} exp={expected_rc}')
    
    target = rs_text if mode in ('rust','compile','run') else (proc.stdout + proc.stderr)
    for p in case.get('present', []):
        if p not in target:
            problems.append(f'missing sub: {p[:40]}')
    
    status = 'PASS' if not problems else 'FAIL'
    results.append({'id': cid, 'status': status, 'problems': problems})

passed = [r for r in results if r['status'] == 'PASS']
failed = [r for r in results if r['status'] == 'FAIL']

print(f'通过: {len(passed)}/{len(results)}  失败: {len(failed)}')
print()
for r in failed:
    print(f'  ✗ {r["id"]}: {r["problems"]}')
print()
print('通过项:', ', '.join(r['id'] for r in passed[:20]) + ('...' if len(passed) > 20 else ''))

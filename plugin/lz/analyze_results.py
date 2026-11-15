import json

d = json.load(open(r'e:\IDEProjects\AI\lang-zone\josn_parse\lz\test_results.json', encoding='utf-8'))

print('=== LZ FAIL (7) ===')
for x in d['details']:
    if x.get('lz') == 'FAIL':
        fname = x['file']
        err = x.get('error', '')[:150]
        print(f'  {fname}')
        print(f'    {err}')
print()

print('=== RUSTC FAIL (43) ===')
for x in d['details']:
    if x.get('rustc') == 'FAIL':
        fname = x['file']
        err = x.get('error', '')[:120]
        print(f'  {fname}  |  {err}')
print()

print('=== Summary ===')
r = d['results']
print(f'Total:      {r["total"]}')
print(f'LZ pass:    {r["lz_pass"]} / {r["total"]} = {r["lz_pass"]/r["total"]*100:.1f}%')
print(f'Rustc pass: {r["rustc_pass"]} / {r["total"]} = {r["rustc_pass"]/r["total"]*100:.1f}%')
print(f'LZ fail:    {r["lz_fail"]}')
print(f'Rustc fail: {r["rustc_fail"]}')

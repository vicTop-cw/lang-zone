import json
from collections import Counter

d = json.load(open(r'e:\IDEProjects\AI\lang-zone\josn_parse\lz\test_results.json', encoding='utf-8'))

print('=== Rustc 失败错误模式分类 ===')
err_counter = Counter()
err_files = {}

for x in d['details']:
    if x.get('rustc') == 'FAIL':
        err = x.get('error', '')
        fname = x['file']
        lines = err.split(';')
        for line in lines:
            line = line.strip()
            if line.startswith('error') and 'aborting' not in line and 'warning' not in line:
                key = line[:100]
                err_counter[key] += 1
                if key not in err_files:
                    err_files[key] = []
                err_files[key].append(fname)

print(f'\n共 {len(err_counter)} 种不同错误模式\n')
print('=== Top 错误模式 ===')
for err, count in err_counter.most_common(20):
    files = ', '.join(err_files[err][:5])
    if len(err_files[err]) > 5:
        files += f' ... (+{len(err_files[err])-5})'
    print(f'  [{count}x] {err}')
    print(f'       文件: {files}')
    print()

print('\n=== 每个 Rustc 失败的完整错误 ===')
for x in d['details']:
    if x.get('rustc') == 'FAIL':
        print(f'\n--- {x["file"]} ---')
        errs = x.get('error', '').split(';')
        for e in errs:
            e = e.strip()
            if e:
                print(f'  {e[:150]}')

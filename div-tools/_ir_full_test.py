#!/usr/bin/env python3
"""IR→rustc 全量编译测试脚本
遍历 DEMO/ 下所有 .lz (排除 99_errors/)，用 IR 路线编译并 rustc 验证。
"""
import subprocess, os, sys, glob, json, re, shutil, tempfile
from pathlib import Path
from datetime import datetime

# Fix Unicode output on Windows
import io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')

ROOT = Path(r"E:\IDEProjects\AI\lang-zone")
BIN = ROOT / "target" / "debug" / "lang-zone.exe"
DEMO_DIR = ROOT / "DEMO"
OUT_DIR = ROOT / "_ir_out"
OUT_DIR.mkdir(exist_ok=True)

# 查找所有 .lz 文件
all_lz = []
for f in sorted(DEMO_DIR.rglob("*.lz")):
    rel = str(f.relative_to(DEMO_DIR))
    if "99_errors" in rel.split("\\") + rel.split("/"):
        continue
    all_lz.append(f)

print(f"=== IR→rustc 全量测试 ===")
print(f"测试时间: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
print(f"编译器: {BIN}")
print(f"总文件数: {len(all_lz)}")
print()

results = {
    "total": len(all_lz),
    "lz_pass": 0,       # LZ→IR 编译通过
    "lz_fail": 0,       # LZ→IR 编译失败
    "rustc_pass": 0,    # rustc 编译通过
    "rustc_fail": 0,    # rustc 编译失败
    "pass_files": [],   # 通过的文件列表
    "errors": {},       # 按错误码分类: {error_code: [files]}
    "details": [],      # 详细列表
}

error_pattern = re.compile(r'error\[(E\d+)\]')

for idx, lz_file in enumerate(all_lz):
    rel = str(lz_file.relative_to(DEMO_DIR))
    rs_file = lz_file.with_suffix(".rs")
    
    # Step 1: LZ → IR → Rust
    lz_result = subprocess.run(
        [str(BIN), str(lz_file)],
        capture_output=True, text=True, timeout=30, cwd=str(ROOT)
    )
    
    lz_ok = lz_result.returncode == 0
    if not lz_ok:
        results["lz_fail"] += 1
        err = lz_result.stderr.strip().split("\n")[0] if lz_result.stderr else "unknown"
        results["details"].append({
            "file": rel, "lz_ok": False, "rustc_ok": False,
            "lz_error": err, "rustc_errors": []
        })
        print(f"  [{idx+1}/{len(all_lz)}] ❌ LZ FAIL: {rel} — {err[:80]}")
        continue
    
    results["lz_pass"] += 1
    
    # Step 2: 检查 .rs 是否生成
    if not rs_file.exists():
        results["rustc_fail"] += 1
        results["details"].append({
            "file": rel, "lz_ok": True, "rustc_ok": False,
            "rustc_errors": ["no .rs output"]
        })
        print(f"  [{idx+1}/{len(all_lz)}] ⚠️  SKIP: {rel} — .rs 未生成")
        continue
    
    # Step 3: rustc 编译（输出到 _ir_out 临时目录避免 MSVC linker NUL 问题）
    out_name = rel.replace("\\", "_").replace("/", "_").replace(".lz", "")
    rustc_result = subprocess.run(
        ["rustc", str(rs_file), "--edition", "2021", "--out-dir", str(OUT_DIR)],
        capture_output=True, text=True, timeout=30, cwd=str(ROOT)
    )
    
    rustc_ok = rustc_result.returncode == 0
    if rustc_ok:
        results["rustc_pass"] += 1
        results["pass_files"].append(rel)
        if results["rustc_pass"] <= 10 or results["rustc_pass"] % 20 == 0:
            print(f"  [{idx+1}/{len(all_lz)}] ✅ PASS: {rel}")
    else:
        results["rustc_fail"] += 1
        errors = error_pattern.findall(rustc_result.stderr)
        error_set = list(dict.fromkeys(errors))  # 去重保序
        
        # 按错误码分类
        for e in errors:
            results["errors"].setdefault(e, []).append(rel)
        
        # 取前几个错误信息
        first_errors = rustc_result.stderr.strip().split("\n")[:6]
        results["details"].append({
            "file": rel, "lz_ok": True, "rustc_ok": False,
            "rustc_errors": [e.strip() for e in first_errors if e.strip()]
        })
        
        short_errors = ", ".join(error_set[:3])
        print(f"  [{idx+1}/{len(all_lz)}] ❌ FAIL: {rel} — {short_errors}")

# 汇总
print()
print("=" * 60)
print("汇总")
print("=" * 60)
print(f"总文件数: {results['total']}")
print(f"LZ→IR 通过: {results['lz_pass']}/{results['total']} ({100*results['lz_pass']/max(results['total'],1):.1f}%)")
print(f"LZ→IR 失败: {results['lz_fail']}")
print(f"IR→rustc 通过: {results['rustc_pass']}/{results['lz_pass']} ({100*results['rustc_pass']/max(results['lz_pass'],1):.1f}%)")
print(f"IR→rustc 失败: {results['rustc_fail']}")

if results["lz_fail"] > 0:
    print(f"\n--- LZ 编译失败 ---")
    for d in results["details"]:
        if not d["lz_ok"]:
            print(f"  ❌ {d['file']}: {d.get('lz_error', 'unknown')}")

print(f"\n--- 错误码分布 (仅统计 rustc 失败，去重) ---")
# 针对错误码做文件级去重
unique_errors = {}
for code, files in results["errors"].items():
    unique_errors[code] = sorted(set(files))

for code, files in sorted(unique_errors.items(), key=lambda x: -len(x[1])):
    print(f"  {code}: {len(files)} 文件")
    for f in files[:5]:
        print(f"    - {f}")
    if len(files) > 5:
        print(f"    ... 等 {len(files)} 个")

# 展示通过文件
if results["pass_files"]:
    print(f"\n--- 通过文件 ({len(results['pass_files'])} 个) ---")
    for f in results["pass_files"]:
        print(f"  ✅ {f}")

# 清理临时产物
import glob as _glob
for pat in ["*.exe", "*.pdb"]:
    for tmp in _glob.glob(str(OUT_DIR / pat)):
        try:
            os.remove(tmp)
        except:
            pass

# 保存 JSON
json_path = ROOT / "_ir_test_result.json"
with open(json_path, "w", encoding="utf-8") as jf:
    json.dump(results, jf, indent=2, ensure_ascii=False)
print(f"\n详细结果: {json_path}")

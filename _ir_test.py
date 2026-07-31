#!/usr/bin/env python3
"""IR 路线全面测试：lz .lz → .rs → rustc 编译验证"""
import subprocess, os, sys, json, time
from pathlib import Path

PROJECT = Path(r"E:\IDEProjects\AI\lang-zone")
DEMO = PROJECT / "DEMO"
LZ_EXE = PROJECT / "target" / "debug" / "lang-zone.exe"

def find_lz_files():
    """找所有 DEMO .lz 文件(排除 99_errors/)"""
    files = []
    for root, dirs, filenames in os.walk(DEMO):
        # 跳过 99_errors
        dirs[:] = [d for d in dirs if d != "99_errors"]
        for f in filenames:
            if f.endswith(".lz"):
                files.append(Path(root) / f)
    return sorted(files)

def compile_lz(lz_path):
    """运行 lz 编译器(IR路线)，返回 (success, rs_path, stderr)"""
    rs_path = lz_path.with_suffix(".rs")
    try:
        result = subprocess.run(
            [str(LZ_EXE), str(lz_path)],
            capture_output=True, text=True, timeout=30,
            cwd=str(PROJECT)
        )
        return result.returncode == 0, rs_path, result.stderr.strip()
    except subprocess.TimeoutExpired:
        return False, rs_path, "TIMEOUT"
    except Exception as e:
        return False, rs_path, str(e)

def verify_rustc(rs_path):
    """用 rustc 验证 .rs 文件"""
    if not rs_path.exists():
        return False, "NO_FILE", "No .rs generated"
    
    # 读取 .rs 内容，检查是否为空
    content = rs_path.read_text(encoding="utf-8")
    if not content.strip():
        return False, "EMPTY", "Empty .rs file"
    
    # 在 DEMO 目录下运行 rustc（有些文件需要相对路径 cargo 依赖）
    try:
        result = subprocess.run(
            ["rustc", "--edition", "2021", str(rs_path.name)],
            capture_output=True, text=True, timeout=30,
            cwd=str(rs_path.parent)
        )
        if result.returncode == 0:
            # 清理生成的 .exe
            exe_path = rs_path.with_suffix(".exe")
            if exe_path.exists():
                exe_path.unlink()
            # 清理 .pdb
            pdb_path = rs_path.with_suffix(".pdb")
            if pdb_path.exists():
                pdb_path.unlink()
            return True, "OK", ""
        else:
            # 提取错误码
            errors = result.stderr.strip()
            err_codes = []
            for line in errors.split("\n"):
                if line.startswith("error["):
                    code = line.split("error[")[1].split("]")[0] if "error[" in line else ""
                    if code and code not in err_codes:
                        err_codes.append(code)
            return False, ",".join(err_codes[:5]) if err_codes else "UNKNOWN", errors
    except subprocess.TimeoutExpired:
        return False, "TIMEOUT", "rustc timeout"
    except FileNotFoundError:
        return False, "NO_RUSTC", "rustc not found"
    except Exception as e:
        return False, "ERROR", str(e)

def main():
    print("=" * 70)
    print("IR 路线全面测试")
    print(f"时间: {time.strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"项目: {PROJECT}")
    print("=" * 70)
    
    if not LZ_EXE.exists():
        print(f"ERROR: lz 二进制不存在: {LZ_EXE}")
        sys.exit(1)
    
    # Phase 1: lz → .rs (IR路线)
    lz_files = find_lz_files()
    print(f"\nPhase 1: lz → .rs (IR路线) — {len(lz_files)} 个文件")
    
    lz_fail = []
    lz_pass = []
    
    for i, lz_path in enumerate(lz_files):
        rel = lz_path.relative_to(PROJECT)
        success, rs_path, err = compile_lz(lz_path)
        if success:
            lz_pass.append((rel, rs_path))
            if (i + 1) % 20 == 0:
                print(f"  [{i+1}/{len(lz_files)}] 已处理...")
        else:
            lz_fail.append((rel, err))
            print(f"  ❌ [{i+1}] {rel}: {err[:80]}")
    
    print(f"\n  lz 编译: {len(lz_pass)} 通过, {len(lz_fail)} 失败")
    if lz_fail:
        for rel, err in lz_fail:
            print(f"    ❌ {rel}: {err[:100]}")
    
    # Phase 2: rustc 验证
    print(f"\nPhase 2: rustc 验证 — {len(lz_pass)} 个 .rs 文件")
    
    rustc_pass = []
    rustc_fail = []
    
    for i, (rel, rs_path) in enumerate(lz_pass):
        success, code, details = verify_rustc(rs_path)
        if success:
            rustc_pass.append(rel)
        else:
            rustc_fail.append((rel, code, details))
        if (i + 1) % 20 == 0:
            print(f"  [{i+1}/{len(lz_pass)}] rustc 验证中...")
    
    pass_rate = len(rustc_pass) / len(lz_pass) * 100 if lz_pass else 0
    print(f"\n  rustc 通过: {len(rustc_pass)}/{len(lz_pass)} ({pass_rate:.1f}%)")
    
    if rustc_fail:
        print(f"\n  rustc 失败 ({len(rustc_fail)} 个):")
        for rel, code, _ in rustc_fail:
            print(f"    ❌ {rel}: [{code}]")
    
    # Phase 3: 详细错误分析
    print(f"\nPhase 3: 错误分类")
    
    # 按错误码分类
    err_groups = {}
    for rel, code, details in rustc_fail:
        for ec in code.split(","):
            ec = ec.strip()
            if ec:
                if ec not in err_groups:
                    err_groups[ec] = []
                err_groups[ec].append(str(rel))
    
    for code in sorted(err_groups.keys()):
        files = err_groups[code]
        print(f"  [{code}] — {len(files)} 文件: {', '.join(f.name for f in [Path(f) for f in files[:5]])}")
        if len(files) > 5:
            print(f"         ... 及其他 {len(files)-5} 个文件")
    
    # 输出摘要
    print(f"\n" + "=" * 70)
    print(f"测试摘要")
    print(f"  lz→IR 编译: {len(lz_pass)}/{len(lz_files)} ({len(lz_pass)/len(lz_files)*100:.1f}%)")
    print(f"  IR→rustc:   {len(rustc_pass)}/{len(lz_pass)} ({pass_rate:.1f}%)")
    print(f"=" * 70)
    
    # 输出 JSON 结果供后续分析
    result = {
        "timestamp": time.strftime('%Y-%m-%d %H:%M:%S'),
        "total_demos": len(lz_files),
        "lz_pass": len(lz_pass),
        "lz_fail": len(lz_fail),
        "rustc_pass": len(rustc_pass),
        "rustc_fail": len(rustc_fail),
        "pass_rate": round(pass_rate, 1),
        "lz_fail_list": [(str(r), e) for r, e in lz_fail],
        "rustc_fail_list": [(str(r), c) for r, c, _ in rustc_fail],
        "rustc_fail_details": {str(r): {"code": c, "details": d[:300]} for r, c, d in rustc_fail},
        "error_groups": {c: [str(f) for f in files] for c, files in err_groups.items()}
    }
    
    result_path = PROJECT / "_ir_test_result.json"
    with open(result_path, "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2, ensure_ascii=False)
    print(f"\n详细结果已保存到: {result_path}")

if __name__ == "__main__":
    main()

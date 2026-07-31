#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Lang-Zong 绑定/所有权语法测试驱动 (Test Harness)
=================================================
本套件聚焦"变更的语法"——变量绑定与所有权语义:
  y = 1         默认可变绑定
  let x = 1     不可变绑定
  ref r = x     可变引用 (&mut)
  let ref s = x 不可变引用 (&)
  mut           冗余修饰 (默认已可变)
  const C = v   编译期常量 (函数体内退化为 let mut)
  ^             所有权转移 (move)
  owned         参数所有权修饰 (当前不强制 ^)

相对通用 harness 的增强: 新增 rustc 端到端模式。
  - rustc     : lz 必须成功生成 .rs, 且 rustc 必须编译通过 (rc==0)
  - rustc_err : lz 成功生成 .rs, 但 rustc 必须编译失败 (rc!=0) —— 用于固化"当前已知缺陷"

SUT: ../../target/debug/lang-zone.exe
rustc: 取 PATH 中 rustc (需 1.70+)
"""

import os
import sys
import json
import shutil
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
WORK = os.path.join(HERE, "_work")
SUT = os.path.abspath(os.path.join(HERE, "..", "..", "target", "debug", "lang-zone.exe"))
RUSTC = shutil.which("rustc")

CATALOG = [
    # ---------------- 绑定形式 (Binding) ----------------
    dict(id="B01", title="可变绑定默认 (y = 1)", category="binding", priority="P0", mode="rustc",
         source="def main() =\n    y = 1\n    y = 2\n    print(y)",
         present=["let mut y: i64 = 1;", "y = 2;"], absent=[],
         note="首次裸赋值生成 let mut, 类型推断加 :i64 注解。"),

    dict(id="B02", title="不可变绑定 (let x = 1)", category="binding", priority="P0", mode="rustc",
         source="def main() =\n    let x = 1\n    print(x)",
         present=["let x: i64 = 1;"], absent=["let mut x"],
         note="let 前缀生成不可变绑定。类型推断加 :i64 注解。"),

    dict(id="B03", title="可变引用 (ref r = a)", category="binding", priority="P0", mode="rustc",
         source="def main() =\n    a = 42\n    ref r = a\n    print(r)",
         present=["let mut r = &mut a;"], absent=[],
         note="ref 生成可变引用 &mut (值侧), 跳过类型注解（推断值类型 vs 引用类型不匹配）。"),

    dict(id="B04", title="不可变引用 (let ref s = a)", category="binding", priority="P0", mode="rustc",
         source="def main() =\n    a = 42\n    let ref s = a\n    print(s)",
         present=["let s = &a;"], absent=[],
         note="let ref 生成不可变引用 & (值侧), 跳过类型注解。"),

    dict(id="B05", title="mut 冗余修饰 (mut z = 10)", category="binding", priority="P1", mode="rustc",
         source="def main() =\n    mut z = 10\n    z = 20\n    print(z)",
         present=["let mut z: i64 = 10;"], absent=[],
         note="mut 可加可不加, 默认已是可变; 类型推断加 :i64 注解。"),

    dict(id="B06", title="所有权转移 (^ move)", category="ownership", priority="P0", mode="rustc",
         source="def f(x: int)-> int = x\ndef main() =\n    v = 10\n    r = f(v^)\n    print(r)",
         present=["let mut r: i64 = f(v);", "let mut v: i64 = 10"], absent=[],
         note="^ 降为普通传值; 类型推断加 :i64 注解。"),

    dict(id="B07", title="owned 参数被接受 (不强制 ^)", category="ownership", priority="P1", mode="rust",
         source='struct Person =\n    name: str\ndef consume(owned p: Person)-> str = f"{p.name}"\ndef main() =\n    bob = Person(name: "Bob")\n    consume(bob)',
         present=["fn consume(p: Person)"], absent=[],
         note="⚠️ 已知缺口: owned 不强制 ^ 调用。"),

    # ---------------- 已知缺陷固化 (rustc_err) ----------------
    dict(id="B08", title="不可变重赋值: lz 不拦截 (已知缺口)", category="gap", priority="P1", mode="rustc_err",
         source="def main() =\n    let x = 1\n    x = 2\n    print(x)",
         present=["let x: i64 = 1;"], absent=[],
         note="⚠️ lz 放行不可变重赋值, rustc (E0384) 才拦截。"),

    dict(id="B09", title="对不可变源取 &mut: lz 不拦截 (已知缺口)", category="gap", priority="P1", mode="rustc_err",
         source="def main() =\n    let x = 1\n    ref r = x\n    print(r)",
         present=["let x: i64 = 1;"], absent=[],
         note="⚠️ ref 对不可变源生成 &mut x 非法; rustc 报 E0596。"),

    # ---------------- const 退化 ----------------
    dict(id="B10", title="const int 退化为 let mut", category="const", priority="P1", mode="rustc",
         source="def main() =\n    const N: int = 10\n    print(N)",
         present=["let mut N = 10;"], absent=["const N"],
         note="函数体内 const 退化为 let mut (设计决策); 类型注解省略交由推断。"),

    dict(id="B11", title="const str 退化为 let mut (修复后)", category="const", priority="P1", mode="rustc",
         source='def main() =\n    const G: str = "Hi"\n    print(G)',
         present=['let mut G = "Hi";'], absent=["const G"],
         note="修复点: 旧实现带 String 注解导致 &str 字面量类型不匹配; 现省略注解可编译。"),

    # ---------------- 错误 (Error) ----------------
    dict(id="B12", title="ref 缺初始化应报错", category="exception", priority="P1", mode="error",
         source="def main() =\n    ref r\n    print(r)",
         present=["Parse error"], absent=[],
         note="ref 后必须跟 = 初始化, 否则 Parse error。"),
]


def run_rustc(rs_path):
    """编译生成的 .rs, 返回 (rc, stdout, stderr)。rustc 缺失时返回 (None, '', 'rustc not found')。"""
    if RUSTC is None:
        return None, "", "rustc not found in PATH"
    out_dir = os.path.dirname(rs_path)
    proc = subprocess.run(
        [RUSTC, "--edition", "2021", "--crate-type", "bin", rs_path, "--out-dir", out_dir],
        capture_output=True, text=True, timeout=60,
    )
    return proc.returncode, proc.stdout, proc.stderr


def run_case(case):
    cid = case["id"]
    mode = case["mode"]
    lz_path = os.path.join(WORK, cid + ".lz")
    with open(lz_path, "w", encoding="utf-8") as fh:
        fh.write(case["source"])

    args = [SUT, cid + ".lz"]
    if mode == "tokens":
        args.append("--tokens")
    elif mode == "ast":
        args.append("--ast")

    proc = subprocess.run(args, cwd=WORK, capture_output=True, text=True, timeout=30)
    rc = proc.returncode
    out = proc.stdout
    err = proc.stderr
    combined = out + err

    rs_path = os.path.join(WORK, cid + ".rs")
    rs_text = ""
    if os.path.exists(rs_path):
        with open(rs_path, "r", encoding="utf-8") as fh:
            rs_text = fh.read()

    # rustc / rustc_err 模式先要求 lz 成功
    if mode in ("rustc", "rustc_err"):
        if rc != 0:
            return dict(id=cid, title=case["title"], category=case["category"],
                        priority=case["priority"], mode=mode, rc=rc,
                        status="FAIL", problems=[f"lz 应成功生成 .rs, 实际 rc={rc}: {err.strip()[:120]}"],
                        stdout=out, stderr=err, rs=rs_text, rustc="n/a")

    # 选择断言目标
    if mode in ("rustc", "rustc_err", "rust"):
        target = rs_text
    elif mode == "error":
        target = combined
    else:
        target = out

    present = case.get("present", [])
    absent = case.get("absent", [])
    problems = []

    if mode == "error" and rc != 1:
        problems.append(f"错误用例期望退出码 1, 实际 {rc}")
    elif mode in ("tokens", "ast", "rust") and rc != 0:
        problems.append(f"正常用例期望退出码 0, 实际 {rc}")

    for p in present:
        if p not in target:
            problems.append(f"缺少预期子串: {p!r}")
    for a in absent:
        if a in target:
            problems.append(f"出现不应存在的子串: {a!r}")

    # rustc 端到端校验
    rustc_rc = "skipped"
    if mode == "rustc":
        if not os.path.exists(rs_path):
            problems.append("未生成 .rs 文件")
        else:
            rrc, _, rerr = run_rustc(rs_path)
            rustc_rc = rrc
            if rrc is None:
                problems.append("rustc 不可用, 跳过端到端校验")
            elif rrc != 0:
                problems.append(f"rustc 编译失败 (rc={rrc}): {rerr.strip()[:160]}")
    elif mode == "rustc_err":
        if not os.path.exists(rs_path):
            problems.append("未生成 .rs 文件")
        else:
            rrc, _, rerr = run_rustc(rs_path)
            rustc_rc = rrc
            if rrc is None:
                problems.append("rustc 不可用, 跳过端到端校验")
            elif rrc == 0:
                problems.append("期望 rustc 编译失败, 但实际通过 (缺陷已修复?)")

    status = "PASS" if not problems else "FAIL"
    return dict(id=cid, title=case["title"], category=case["category"],
                priority=case["priority"], mode=mode, rc=rc,
                status=status, problems=problems,
                stdout=out, stderr=err, rs=rs_text, rustc=rustc_rc)


def main():
    os.makedirs(WORK, exist_ok=True)
    if not os.path.exists(SUT):
        print(f"[FATAL] SUT 不存在: {SUT}", file=sys.stderr)
        sys.exit(2)
    if RUSTC is None:
        print("[WARN] PATH 中未找到 rustc, rustc/rustc_err 模式将跳过端到端校验。\n")

    print(f"SUT: {SUT}")
    print(f"rustc: {RUSTC or 'NOT FOUND'}")
    print(f"用例数: {len(CATALOG)}\n")

    results = []
    for case in CATALOG:
        r = run_case(case)
        results.append(r)
        mark = {"PASS": "✅", "FAIL": "❌"}.get(r["status"], "?")
        extra = f" rustc={r['rustc']}" if r["mode"] in ("rustc", "rustc_err") else ""
        print(f"  {mark} {r['id']:>4} [{r['priority']}] {r['title']}  (rc={r['rc']}{extra})")
        for p in r["problems"]:
            print(f"         - {p}")

    total = len(results)
    passed = sum(1 for r in results if r["status"] == "PASS")
    failed = sum(1 for r in results if r["status"] == "FAIL")

    by_cat = {}
    for r in results:
        c = r["category"]
        by_cat.setdefault(c, {"total": 0, "pass": 0})
        by_cat[c]["total"] += 1
        if r["status"] == "PASS":
            by_cat[c]["pass"] += 1

    print("\n================ 汇总 ================")
    print(f"总数={total}  通过={passed}  失败={failed}")
    print(f"总通过率: {passed/total*100:.1f}%")
    print("\n按类别:")
    for c, v in by_cat.items():
        print(f"  {c:<12} {v['pass']}/{v['total']}  ({v['pass']/v['total']*100:.1f}%)")

    with open(os.path.join(WORK, "results.json"), "w", encoding="utf-8") as fh:
        json.dump(dict(total=total, passed=passed, failed=failed, by_cat=by_cat, results=results),
                  fh, ensure_ascii=False, indent=2)
    print(f"\n结果已写入: {os.path.join(WORK, 'results.json')}")


if __name__ == "__main__":
    main()

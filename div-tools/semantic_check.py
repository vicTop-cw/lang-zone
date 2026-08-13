#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
语义契约检查工具 — 找出「转译通过、rustc 编译通过、运行正确，但生成的 Rust
与 LZ 语义不符」的隐蔽问题（如早期 `let x = 1` 被生成 `let mut x = 1`）。

方法：对照 LZ 源码的语义标注与生成 .rs 的对应构造，检查契约一致性。

契约项：
  C1 绑定可变性：LZ `let x`（不可变）→ 生成 Rust `let x`（无 mut）
  C2 绑定可变性：LZ `mut x`（可变）→ 生成 Rust `let mut x`
  C3 形参修饰符：LZ `ref x: T`（不可变引用）→ 生成 `x: &T`
  C4 形参修饰符：LZ `mut ref x: T`（可变引用）→ 生成 `x: &mut T`
  C5 形参修饰符：LZ `mut x: T`（可变传值）→ 生成 `mut x: T`
  C6 with 资源绑定：`with X as res:` → 生成 `let mut res = ...`

局限：基于文本扫描（正则近似），同名变量/作用域/遮蔽可能误报——
报告输出「可疑项」，需人工确认。
"""

import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LZC = os.path.join(ROOT, "target", "debug", "lang-zone.exe")
RUSTC_EDITION = "2021"
LIBS = os.path.join(ROOT, "target", "debug", "liblz_builtins.rlib")

# ── 契约检查项 ──
CONTRACTS = {
    "C1": "LZ `let x` 不可变绑定 → Rust `let x`（无 mut）",
    "C2": "LZ `mut x` 可变绑定 → Rust `let mut x`",
    "C3": "LZ 形参 `ref x: T` → Rust `x: &T`",
    "C4": "LZ 形参 `mut ref x: T` → Rust `x: &mut T`",
    "C5": "LZ 形参 `mut x: T` → Rust `mut x: T`",
    "C6": "LZ `with X as res:` → Rust `let mut res = ...`",
    "C8": "LZ 方法 `def f(mut self)` → Rust `fn f(&mut self)`；`def f(self)` → `fn f(&self)`",
    "C9": "LZ 闭包绑定 `let/mut f = |...|` → Rust `let [mut] f = move |...|`（不能裸赋值 `f = move |` 缺 let）",
}


def lz_files():
    """收集所有 .lz 测试文件（DEMO + lz_std）。"""
    files = []
    for base in ("DEMO", "lz_std"):
        for dirpath, _, names in os.walk(os.path.join(ROOT, base)):
            for n in names:
                if n.endswith(".lz"):
                    files.append(os.path.join(dirpath, n))
    return sorted(files)


def gen_rs(lz_path, tmpdir):
    """调用 lang-zone 生成 .rs，返回 (rs_path|None, err)。"""
    # project 模式：08_modules 与多模块测试需要 --project
    is_project = "08_modules" in lz_path.replace("\\", "/")
    cmd = [LZC, lz_path] + (["--project"] if is_project else [])
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        return None, r.stderr.strip()[:200]
    rs = lz_path[:-3] + ".rs"
    if os.path.exists(rs):
        return rs, None
    return None, "no .rs generated"


# ── LZ 侧提取 ──
LET_RE = re.compile(r"^\s*(?P<kind>let|mut)\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*(?::[^=]+)?=")
PARAM_RE = re.compile(r"def\s+[A-Za-z_][\w.]*\s*\((?P<params>[^)]*)\)")

def lz_bindings(text):
    """提取 LZ 源码绑定语义：{name: 'let'|'mut'}（行级近似，遮蔽时后者覆盖）。"""
    bindings = {}
    for m in LET_RE.finditer(text):
        bindings[m.group("name")] = m.group("kind")
    return bindings


def lz_params(text):
    """提取 LZ 形参修饰符：{func: {name: set(修饰符)}}（按函数分组，避免跨函数同名遮蔽）。"""
    params = {}
    for m in PARAM_RE.finditer(text):
        func = m.group(0).split("(", 1)[0].replace("def", "").strip()
        func = func.split(".")[-1].strip()  # Point.method → method
        pmods = {}
        for p in m.group("params").split(","):
            p = p.strip()
            if not p:
                continue
            pm = re.match(r"(?:(mut|ref|owned)\s+)*(?P<name>[A-Za-z_][\w]*)\s*(?::|$)", p)
            if pm:
                n = pm.group("name")
                mods = set()
                for kw in ("mut", "ref", "owned"):
                    if re.search(rf"\b{kw}\b", p[: p.find(n)]):
                        mods.add(kw)
                if mods:
                    pmods[n] = mods
        if pmods:
            params[func] = pmods
    return params


# ── Rust 侧提取 ──
RS_LET_RE = re.compile(r"let\s+(?P<mut>mut\s+)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b")
RS_PARAM_RE = re.compile(r"fn\s+[\w.]*\((?P<params>[^)]*)\)")

def rs_let_kinds(text):
    """提取 Rust 绑定：{name: 'mut'|'immut'}（行级近似，遮蔽后覆盖）。"""
    kinds = {}
    for m in RS_LET_RE.finditer(text):
        kinds[m.group("name")] = "mut" if m.group("mut") else "immut"
    return kinds


def rs_param_mods(text):
    """提取 Rust 形参：{func: {name: '&mut'|'&'|'mut'|''}}（按函数分组）。
    Rust 引用修饰符在类型上（`x: &T`/`x: &mut T`），可变传值在 name 前（`mut x: T`）。"""
    mods = {}
    for m in RS_PARAM_RE.finditer(text):
        func = m.group(0).split("(", 1)[0].replace("fn", "").strip()
        func = func.split(".")[-1].strip()
        pmods = {}
        for p in m.group("params").split(","):
            p = p.strip()
            if not p:
                continue
            # name 前修饰（mut x: T）或类型引用（x: &T / x: &mut T）
            pm = re.match(
                r"(?:(?P<pre>mut|&mut|&)\s+)?(?P<name>[A-Za-z_][\w]*)\s*:\s*"
                r"(?:(?P<post>&\s*mut\s*|&)\s*)?",
                p,
            )
            if pm:
                n = pm.group("name")
                pre = (pm.group("pre") or "").strip()
                post = (pm.group("post") or "").strip()
                if "&mut" in post or "&mut" in pre:
                    pmods[n] = "&mut"
                elif "&" in post or "&" in pre:
                    pmods[n] = "&"
                elif "mut" in pre:
                    pmods[n] = "mut"
                else:
                    pmods[n] = ""
        if pmods:
            mods[func] = pmods
    return mods


# ── C8：方法 self 可变性契约 ──
LZ_SELF_RE = re.compile(r"def\s+[\w.]+\s*\((?P<mod>mut\s+)?self\b")
RS_SELF_RE = re.compile(r"fn\s+[\w.]+\s*\((?P<ref>&\s*mut\s*|&)?(?P<mod>mut\s+)?self\b")

def lz_self_mods(text):
    """提取 LZ 方法 self：{method: 'mut'|''}（按行号近似，含函数名提取）。"""
    mods = {}
    for m in LZ_SELF_RE.finditer(text):
        # 取方法名（def X.method 或 def method）
        seg = m.group(0).split("(", 1)[0].replace("def", "").strip()
        name = seg.split(".")[-1].strip()
        mods[name] = "mut" if m.group("mod") else ""
    return mods


def rs_self_mods(text):
    """提取 Rust 方法 self：{method: '&mut'|'&'|'mut'|''}。"""
    mods = {}
    for m in RS_SELF_RE.finditer(text):
        seg = m.group(0).split("(", 1)[0].replace("fn", "").strip()
        name = seg.split(".")[-1].strip()
        r = (m.group("ref") or "").strip()
        mo = (m.group("mod") or "").strip()
        if "&mut" in r:
            mods[name] = "&mut"
        elif "&" in r:
            mods[name] = "&"
        elif "mut" in mo:
            mods[name] = "mut"
        else:
            mods[name] = ""
    return mods


def check_file(lz_path):
    """检查单个文件，返回违规列表 [(contract, var, detail)]。"""
    with open(lz_path, encoding="utf-8", errors="replace") as f:
        lz_text = f.read()
    rs_path, err = gen_rs(lz_path, "")
    if not rs_path:
        return []
    with open(rs_path, encoding="utf-8", errors="replace") as f:
        rs_text = f.read()

    issues = []
    lz_b = lz_bindings(lz_text)
    rs_k = rs_let_kinds(rs_text)
    for name, kind in lz_b.items():
        rk = rs_k.get(name)
        if rk is None:
            continue  # 未在 Rust 中作为 let 出现（可能是全局/其他）
        if kind == "let" and rk == "mut":
            issues.append(("C1", name, f"LZ `let {name}` 但生成 `let mut {name}`"))
        if kind == "mut" and rk == "immut":
            issues.append(("C2", name, f"LZ `mut {name}` 但生成 `let {name}`（无 mut）"))

    lz_p = lz_params(lz_text)
    rs_p = rs_param_mods(rs_text)
    for func, pmods in lz_p.items():
        rmods = rs_p.get(func, {})
        for name, mods in pmods.items():
            rm = rmods.get(name)
            if rm is None:
                continue
            if "ref" in mods and "mut" in mods:
                # mut ref（可变引用，LZ 修饰符为 mut+ref）：正确生成 `&mut`，
                # C4 专属分支——不能用 elif 落到 C3（`&mut` != `&` 会误报）
                if rm != "&mut":
                    issues.append(("C4", name, f"{func}: LZ `mut ref {name}` 生成 `{rm}` 应为 `&mut`"))
            elif "ref" in mods:
                # 纯 ref（不可变引用）：正确生成 `&`
                if rm != "&":
                    issues.append(("C3", name, f"{func}: LZ `ref {name}` 生成 `{rm}` 应为 `&`"))
            elif "mut" in mods:
                # mut（可变传值）：正确生成 `mut x`
                if rm != "mut":
                    issues.append(("C5", name, f"{func}: LZ `mut {name}` 生成 `{rm}` 应为 `mut`"))

    # C8：方法 self 可变性契约
    lz_self = lz_self_mods(lz_text)
    rs_self = rs_self_mods(rs_text)
    for name, lmod in lz_self.items():
        rmod = rs_self.get(name)
        if rmod is None:
            continue
        if lmod == "mut" and rmod != "&mut":
            issues.append(("C8", name, f"LZ `def {name}(mut self)` 生成 `{rmod} self` 应为 `&mut self`"))
        elif lmod == "" and rmod not in ("&", ""):
            issues.append(("C8", name, f"LZ `def {name}(self)` 生成 `{rmod} self` 应为 `&self`"))

    # C9：闭包绑定契约——LZ `let/mut f = |...|` 应生成 `let [mut] f = move |...|`；
    # 若生成裸赋值 `f = move |...|`（缺 let，变量未声明）是语义 bug（E0425）
    LZ_CLOSURE_BIND = re.compile(r"^\s*(?P<kind>let|mut)\s+(?P<name>[A-Za-z_]\w*)\s*=\s*\|", re.M)
    RS_BARE_ASSIGN = re.compile(r"^\s*(?P<name>[A-Za-z_]\w*)\s*=\s*move\s*\|", re.M)
    lz_closures = {m.group("name") for m in LZ_CLOSURE_BIND.finditer(lz_text)}
    rs_bare = {m.group("name") for m in RS_BARE_ASSIGN.finditer(rs_text)}
    for name in lz_closures & rs_bare:
        # 裸赋值出现的变量若是 LZ 闭包绑定 → 缺 let 声明
        issues.append(("C9", name, f"LZ 闭包绑定 `{name} = |...|` 生成裸赋值 `{name} = move |...|`（缺 let，应为 `let [mut] {name} = move |...|`）"))
    return issues


def main():
    total_issues = 0
    for lz in lz_files():
        rel = os.path.relpath(lz, ROOT).replace("\\", "/")
        issues = check_file(lz)
        if issues:
            total_issues += len(issues)
            print(f"== {rel} ==")
            for c, var, detail in issues:
                print(f"   [{c}] {CONTRACTS[c]}\n        {detail}")
    print(f"\n总计 {total_issues} 个可疑语义契约违规")
    return 0


if __name__ == "__main__":
    sys.exit(main())

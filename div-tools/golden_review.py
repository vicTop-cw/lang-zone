#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""D8 golden 快照基线变更审查摘要：给出一张「变更分类表」，人工只审这张表，不逐文件比对。

出处：docs/决策登记表-2026-09-29.md D8（人工裁定「半自动化……人工只审 diff 而非逐文件比对」）。

为什么不是 `git diff` 就够了：本仓 golden 是**受版管文件**（`DEMO/99_spec/moddec/golden/*.rs`），
而 `1ccbdc43 test(moddec): golden 快照对齐 const 内联产物 + 读取归一化 CRLF` 说明这里出现过
「只变行尾、语义没变」的整批改动。`git diff` 会把两者混在一起，人工就得逐文件看。
本工具按三类分开报：
  CONTENT   —— 去掉行尾与行首尾空白后仍有差 ⇒ 真语义变更，必须逐条审
  LINEEND   —— 只是 CRLF/LF 翻转 ⇒ 口径变更，审一次「是否有意」即可
  TRAILWS   —— 只是行尾空白 ⇒ 格式噪声
并给出每个变更文件被哪些测试消费（grep `golden("<名>")`），以及**门禁现在信不信**（测试侧读盘是否归一化行尾）。

只读工具：不写 golden、不跑 `--update`、不改测试。

用法：
  python div-tools/golden_review.py                      # 审工作树 vs HEAD
  python div-tools/golden_review.py --dir DEMO/99_spec/moddec/golden
  python div-tools/golden_review.py --json
  python div-tools/golden_review.py --selftest           # 六格（S1/S4/S5/S6 是必然会红的对照）
"""
import argparse
import difflib
import io
import json
import os
import re
import subprocess
import sys

GOLDEN_REF = "DEMO/99_spec/moddec/golden"
CONSUMER = re.compile(r'golden\(\s*"([^"]+)"')


def classify(before, after):
    """返回 (类别, 说明)。before/after 是 bytes。"""
    if before == after:
        return None, ""
    def norm(b):
        return [ln.rstrip() for ln in b.decode("utf-8", "replace").replace("\r\n", "\n").split("\n")]
    if norm(before) == norm(after):
        crlf_b, crlf_a = before.count(b"\r\n"), after.count(b"\r\n")
        if crlf_b != crlf_a:
            return "LINEEND", "CRLF %d→%d，语义行逐条相等" % (crlf_b, crlf_a)
        return "TRAILWS", "仅行尾空白差异（去 rstrip 后逐行相等）"
    diff = list(difflib.unified_diff(norm(before), norm(after), lineterm="", n=0))
    hunks = [d for d in diff if d.startswith(("+", "-")) and not d.startswith(("+++", "---"))]
    return "CONTENT", "+%d/-%d（去行尾与行首尾空白后仍不等）" % (
        sum(1 for h in hunks if h.startswith("+")), sum(1 for h in hunks if h.startswith("-")))


def consumers(repo, stems):
    """哪些测试按名字消费这些 golden。"""
    if not stems:
        return {}
    try:
        out = subprocess.run(["git", "-C", repo, "grep", "-n", "golden(", "--", "tests"],
                             capture_output=True, text=True, encoding="utf-8",
                             errors="replace", timeout=120).stdout
    except Exception:                                          # noqa: BLE001
        return {}
    hit = {}
    for line in out.splitlines():
        for name in CONSUMER.findall(line):
            stem = os.path.splitext(name)[0]
            if stem in stems:
                hit.setdefault(stem, set()).add(line.split(":")[0])
    return {k: sorted(v) for k, v in hit.items()}


def decide(git_dirty, bytes_equal):
    """先问 git 再比字节。

    本仓 `core.autocrlf = true`：index 里存 LF、checkout 落成 CRLF，`git status` 判**干净**。
    若直接拿磁盘字节 vs `git show HEAD:` 的 blob 比，一棵完全干净的树也会报出 9 条「LINEEND 变更」
    （第一版就中了这条），把人工注意力喂给根本不存在的改动。⇒ 只有 git 认脏的文件才进待审清单，
    git 认干净但字节不同的单列为 CHECKOUT（checkout 归一化伪差异，供知情、不需批准）。
    """
    if git_dirty:
        return "REVIEW"
    return None if bytes_equal else "CHECKOUT"


def review(dirrel, repo):
    """审 `dirrel` 下的文件。返回 (CONTENT 类 stem 集合, 行清单)。"""
    rows = []
    try:
        tracked = [p.strip().replace("\\", "/") for p in subprocess.run(
            ["git", "-C", repo, "ls-files", "--", dirrel], capture_output=True, text=True,
            encoding="utf-8", errors="replace", timeout=120).stdout.splitlines() if p.strip()]
        dirty = {p.strip().replace("\\", "/") for p in subprocess.run(
            ["git", "-C", repo, "diff", "--name-only", "HEAD", "--", dirrel], capture_output=True,
            text=True, encoding="utf-8", errors="replace", timeout=120).stdout.splitlines() if p.strip()}
    except Exception as e:                                      # noqa: BLE001
        print("[refuse] git 取数失败（工作树状态不可审）：%s" % e, file=sys.stderr)
        return None, rows
    absdir = os.path.join(repo, dirrel.replace("/", os.sep))
    on_disk = {f for f in os.listdir(absdir) if os.path.isfile(os.path.join(absdir, f))} if os.path.isdir(absdir) else set()
    if not tracked and not on_disk:
        print("[refuse] 目录既无版管文件也无磁盘文件：%s（路径写错还是没 checkout？）" % dirrel, file=sys.stderr)
        return None, rows
    for path in sorted(tracked):
        name = path.split("/")[-1]
        ap = os.path.join(repo, path.replace("/", os.sep))
        if not os.path.exists(ap):
            rows.append({"file": name, "kind": "DELETED", "note": "git 认脏：HEAD 有、盘上没有"})
            continue
        after = io.open(ap, "rb").read()
        head = subprocess.run(["git", "-C", repo, "show", "HEAD:" + path],
                              capture_output=True, timeout=120).stdout
        kind = decide(path in dirty, head == after)
        if kind == "REVIEW":
            c, note = classify(head, after)
            rows.append({"file": name, "kind": c or "DIRTY-EQUAL", "note": note or "git 认脏但去归一化后字节等价"})
        elif kind == "CHECKOUT":
            crlf = after.count(b"\r\n")
            rows.append({"file": name, "kind": "CHECKOUT", "note": "git 判干净，仅磁盘行尾/归一化差异（CRLF %d）" % crlf})
    for name in sorted(on_disk - {p.split("/")[-1] for p in tracked}):
        rows.append({"file": name, "kind": "UNTRACKED", "note": "盘上有、HEAD 没有（新 golden 未经人工确认）"})
    return {r["file"].rsplit(".", 1)[0] for r in rows if r["kind"] == "CONTENT"}, rows


def selftest():
    a = b"fn main() {\r\n  let x = 1;\r\n}\r\n"
    expect("S1 同内容不同行尾 ⇒ LINEEND 而非 CONTENT（对照：把归一化删掉就会红）",
           classify(a, a.replace(b"\r\n", b"\n"))[0] == "LINEEND", classify(a, a.replace(b"\r\n", b"\n")))
    b1 = b"let x = 1;\nlet y = 2;\n"
    expect("S2 真语义变化 ⇒ CONTENT 且给出行数",
           classify(b1, b1.replace(b"y = 2", b"y = 3"))[0] == "CONTENT", classify(b1, b1.replace(b"y = 2", b"y = 3")))
    expect("S3 逐字节相等 ⇒ 不进清单（对照：None 而非误报）", classify(b1, b1)[0] is None, classify(b1, b1))
    ws = b"let x = 1;   \nlet y = 2;\n"
    expect("S4 只有行尾空白 ⇒ TRAILWS 而非 CONTENT（对照）",
           classify(b1, ws)[0] == "TRAILWS", classify(b1, ws))
    expect("S5 行数统计方向正确：+ 是新增行、- 是删除行（对照：反了会误报破坏面）",
           classify(b1, b1.replace(b"let y = 2;\n", b""))[1].startswith("+0/-1"),
           classify(b1, b1.replace(b"let y = 2;\n", b"")))
    expect("S6 只有 git 认脏才进待审；git 干净但字节不同 ⇒ CHECKOUT（对照：拿掉这层会报出 9 条假变更）",
           decide(True, False) == "REVIEW" and decide(False, False) == "CHECKOUT" and decide(False, True) is None,
           (decide(True, False), decide(False, False), decide(False, True)))
    return 0


def expect(name, passed, detail=""):
    print("%s %s%s" % ("PASS" if passed else "FAIL", name, ("  ← " + str(detail)) if not passed else ""))
    if not passed:
        sys.exit(1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=GOLDEN_REF)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    repo = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    stems, rows = review(a.dir, repo)
    if rows is None:
        return 3
    cons = consumers(repo, stems or {})
    if a.json:
        print(json.dumps({"dir": a.dir, "rows": rows, "content_consumers": cons}, ensure_ascii=False, indent=1))
        return 0
    print("golden 变更摘要 ｜ 目录 %s ｜ 判脏口径 = `git diff --name-only HEAD`（本仓 core.autocrlf=%s）"
          % (a.dir, subprocess.run(["git", "-C", repo, "config", "core.autocrlf"],
                                   capture_output=True, text=True, encoding="utf-8").stdout.strip() or "unset"))
    kinds = [r["kind"] for r in rows]
    need = [r for r in rows if r["kind"] in ("CONTENT", "UNTRACKED", "DELETED", "DIRTY-EQUAL")]
    info = [r for r in rows if r["kind"] in ("LINEEND", "TRAILWS", "CHECKOUT")]
    if not need:
        print("  待审 0 条 ⇒ 本轮 golden 基线没动，不需要人工批准（这是结论，不是跳过）")
        if info:
            print("  另有 %d 条 checkout 归一化伪差异（git 判干净），只供知情：%s"
                  % (len(info), ", ".join(sorted({r["kind"] for r in info}))))
        return 0
    order = ["CONTENT", "UNTRACKED", "DELETED", "DIRTY-EQUAL", "LINEEND", "TRAILWS", "CHECKOUT"]
    for kind in order:
        grp = [r for r in rows if r["kind"] == kind]
        if not grp:
            continue
        tag = "← 必须逐条人工审" if kind in ("CONTENT", "UNTRACKED", "DELETED") else (
              "← 只供知情，不构成基线变更" if kind == "CHECKOUT" else "")
        print("\n%s（%d 个）%s" % (kind, len(grp), tag))
        for r in grp:
            stem = r["file"].rsplit(".", 1)[0]
            who = "  消费者：" + ", ".join(cons.get(stem, [])) if cons.get(stem) else ""
            print("  - %-44s %s%s" % (r["file"], r["note"], who))
    print("\n分类合计：待审 %d + 知情 %d = %d（须与各行数相加一致：%s）"
          % (len(need), len(info), len(rows),
             ", ".join("%s=%d" % (k, kinds.count(k)) for k in order if kinds.count(k))))
    print("本工具不写 golden、不跑任何 --update；CONTENT 类若确属预期，请人工改快照并重封指纹。")
    return 0


if __name__ == "__main__":
    sys.exit(main())

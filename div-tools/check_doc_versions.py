#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""校验 SYNTAX/ 文档头部的版本行，防止版本号与校订日期腐烂。

规则
----
1. 每个 `SYNTAX/*.md`（README 除外）第一段必须有一行：
       > 规范版本: <SPEC_VERSION> · <任意描述> · 最后校订: YYYY-MM-DD
2. `规范版本` 必须等于 README.md 标题声明的目录整体版本。
3. `最后校订` 必须等于该文件的真实最后修改日期：
       - 工作区有未提交改动 → 今天
       - 否则 → git 最后一次提交该文件的日期

用法
----
    python SYNTAX/check_doc_versions.py          # 校验，失败退出码 1
    python SYNTAX/check_doc_versions.py --fix    # 自动改写为正确值

CI 集成示例（GitHub Actions）::

    - name: 校验语法文档版本头
      run: python SYNTAX/check_doc_versions.py
"""
from __future__ import annotations

import argparse
import datetime as _dt
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent

HEADER_RE = re.compile(
    r"^> *规范版本[:：] *(?P<ver>[0-9]+(?:\.[0-9]+)*) *·(?P<mid>.*?)· *最后校订[:：] *(?P<date>\d{4}-\d{2}-\d{2}) *$",
    re.MULTILINE,
)
README_VER_RE = re.compile(r"^# .*?v(?P<ver>[0-9]+(?:\.[0-9]+)*)", re.MULTILINE)


def run_git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=REPO, capture_output=True, text=True, encoding="utf-8"
    ).stdout.strip()


def expected_date(rel: str, today: str) -> str:
    if run_git("status", "--porcelain", "--", rel):
        return today
    return run_git("log", "-1", "--format=%ad", "--date=short", "--", rel) or today


def spec_version() -> str:
    m = README_VER_RE.search((ROOT / "README.md").read_text(encoding="utf-8"))
    if not m:
        print("[FATAL] SYNTAX/README.md 标题里读不到规范版本（期望形如 '# ... v3.2'）")
        sys.exit(2)
    return m.group("ver")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--fix", action="store_true", help="自动改写为正确的版本行")
    ap.add_argument("--today", default=_dt.date.today().isoformat(), help="覆盖“今天”（测试用）")
    args = ap.parse_args()

    version = spec_version()
    problems: list[str] = []
    fixed: list[str] = []

    for path in sorted(ROOT.glob("*.md")):
        if path.name == "README.md":
            continue
        rel = f"SYNTAX/{path.name}"
        text = path.read_text(encoding="utf-8")
        m = HEADER_RE.search(text)

        if not m:
            problems.append(f"{rel}: 缺少版本行 '> 规范版本: {version} · ... · 最后校订: YYYY-MM-DD'")
            continue

        want_date = expected_date(rel, args.today)
        errs = []
        if m.group("ver") != version:
            errs.append(f"规范版本 {m.group('ver')} ≠ {version}")
        if m.group("date") != want_date:
            errs.append(f"最后校订 {m.group('date')} ≠ 实际 {want_date}")
        if not errs:
            continue

        if args.fix:
            new_line = f"> 规范版本: {version} ·{m.group('mid')}· 最后校订: {want_date}"
            path.write_text(text[: m.start()] + new_line + text[m.end() :], encoding="utf-8")
            fixed.append(f"{rel}: {'; '.join(errs)}")
        else:
            problems.append(f"{rel}: {'; '.join(errs)}")

    if fixed:
        print(f"已修正 {len(fixed)} 个文件：")
        for f in fixed:
            print(f"  ~ {f}")

    if problems:
        print(f"\n[FAIL] {len(problems)} 个文档版本头不合规：")
        for p in problems:
            print(f"  ✗ {p}")
        print("\n运行 `python SYNTAX/check_doc_versions.py --fix` 自动修正。")
        return 1

    if not fixed:
        print(f"[OK] 全部语法文档版本头合规（规范版本 {version}）。")
    return 0


if __name__ == "__main__":
    sys.exit(main())

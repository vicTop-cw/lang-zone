#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""D11 CHANGELOG 草稿生成：把一段提交范围按 conventional-commit 类型聚类成 Keep-a-Changelog 小节草稿。

出处：docs/决策登记表-2026-09-29.md D11（人工裁定「交给 agent 定期自动化（按 commit 聚类生成草稿），人工抽查」）。

**只出草稿，不写 CHANGELOG.md**（除非显式 --out 到一个你自己指定的路径）。
是否入账、放哪个版本段、措辞怎么改，由人抽查后决定——所以本工具不猜测「哪些 commit 已写过」，
范围一律由 --since / --count 给定（仓库当前 `git tag | wc -l` = 0，没有 tag 可当锚）。

用法：
  python div-tools/changelog_draft.py --count 12
  python div-tools/changelog_draft.py --since 82a2004b --out TEMP/changelog-draft.md
  python div-tools/changelog_draft.py --selftest    # 五格自检（两格是必然会红的对照）
"""
import argparse
import io
import os
import re
import subprocess
import sys
import datetime as dt

TYPES = {
    "feat": "新增",
    "fix": "修复",
    "perf": "性能",
    "refactor": "重构",
    "test": "测试与门禁",
    "docs": "文档",
    "build": "构建",
    "ci": "CI",
    "chore": "杂项",
    "style": "风格",
    "revert": "回退",
}
CONV = re.compile(r"^([a-z]+)(\(([^)]*)\))?(!)?:\s*(.+)$")
SEP = "\x1f"


def log_range(repo, since, count):
    """取范围内的 commit 行：sha SEP subject（用 \x1f 分隔，避开中文与竖线主题的歧义）。"""
    cmd = ["git", "-C", repo, "log", "--pretty=format:%h" + SEP + "%s"]
    if since:
        cmd.append("%s..HEAD" % since)
    else:
        cmd += ["-n", str(count)]
    out = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8",
                         errors="replace", timeout=120).stdout
    rows = []
    for line in out.splitlines():
        if not line.strip():
            continue
        sha, _, subj = line.partition(SEP)
        rows.append((sha.strip(), subj.strip()))
    return rows


def cluster(rows):
    """返回 {小节名: [(sha, scope, 正文, 是否破坏性)]} + 未归类清单 + 覆盖计数。"""
    groups, uncategorised = {}, []
    for sha, subj in rows:
        m = CONV.match(subj)
        if not m:
            uncategorised.append((sha, None, subj, False))
            continue
        typ, scope, bang, body = m.group(1), m.group(3), m.group(4), m.group(5)
        name = TYPES.get(typ)
        if name is None:
            uncategorised.append((sha, scope, subj, bool(bang)))
            continue
        groups.setdefault(name, []).append((sha, scope, body, bool(bang)))
    return groups, uncategorised


def render(groups, uncat, rows, since, count):
    out = ["## 草稿（%s 生成，UTC）" % dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
           "",
           "> 范围：%s ｜ 取到 %d 条提交 ｜ 聚类落位 %d 条 + 未归类 %d 条 = %d（基数须相等）"
           % (("since " + since) if since else ("最近 %d 条" % count),
              len(rows), sum(len(v) for v in groups.values()), len(uncat), len(rows)),
           ""]
    order = [TYPES[t] for t in ["feat", "fix", "perf", "refactor", "test", "docs", "build", "ci", "chore", "style", "revert"]]
    for name in order + [n for n in sorted(groups) if n not in order]:
        items = groups.get(name)
        if not items:
            continue
        out.append("### " + name)
        for sha, scope, body, brk in sorted(items, key=lambda x: ((x[1] or ""), x[2])):
            out.append("- %s%s%s（%s）" % ("**[破坏性]** " if brk else "",
                                          ("`%s` " % scope) if scope else "",
                                          body, sha))
        out.append("")
    if uncat:
        out.append("### 未归类（需人工决定去向，勿默认丢弃）")
        for sha, scope, body, _ in uncat:
            out.append("- %s（%s）" % (body, sha))
        out.append("")
    return "\n".join(out)


def selftest():
    rows = [("a1", "fix(codegen): 修 E0308"),
            ("a2", "feat(ast): 接受 bigint 注解位"),
            ("a3", "test(moddec): golden 快照对齐"),
            ("a4", "merge: PR #1 fix(codegen) FIND_BUG 收尾"),
            ("a5", "Merge branch 'gitcode/feature/lz-decorators' into master"),
            ("a6", "随手改了一行没有类型的提交"),
            ("a7", "wizard(codegen): 未知类型前缀")]
    groups, uncat = cluster(rows)
    n = sum(len(v) for v in groups.values()) + len(uncat)
    expect("S1 基数闭合：聚类落位 + 未归类 == 输入条数（7）", n == len(rows), "%d != %d" % (n, len(rows)))
    expect("S2 fix/feat/test 各归其节且 scope 保留",
           groups["修复"][0][1] == "codegen" and groups["新增"][0][0] == "a2" and "测试与门禁" in groups,
           "%s" % groups)
    expect("S3 merge 与 Merge 两种写法都进未归类而非消失（对照格）",
           {"a4", "a5"} <= {u[0] for u in uncat}, "uncat=%s" % [u[0] for u in uncat])
    expect("S4 未知类型前缀 wizard 被当未归类（对照格：不静默丢）",
           any(u[0] == "a7" for u in uncat) and "wizard" not in "".join(groups), "%s" % uncat)
    txt = render(groups, uncat, rows, None, 7)
    expect("S5 渲染含基数行且未归类小节存在（对照格：删未归类即红）",
           "基数须相等" in txt and "未归类" in txt and txt.count("- ") == len(rows),
           "count=%s rows=%s" % (txt.count("- "), len(rows)))
    return 0


def expect(name, passed, detail=""):
    print("%s %s%s" % ("PASS" if passed else "FAIL", name, ("  ← " + detail) if not passed else ""))
    if not passed:
        sys.exit(1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--since", default="", help="起点 rev（不含），与 --count 二选一")
    ap.add_argument("--count", type=int, default=15)
    ap.add_argument("--out", default="", help="把草稿写到此路径（默认只打印；不写 CHANGELOG.md）")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    repo = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    rows = log_range(repo, a.since or None, a.count)
    if not rows:
        print("[refuse] 取到 0 条提交 ⇒ 范围参数不对或 rev 不存在，不出空草稿", file=sys.stderr)
        return 3
    groups, uncat = cluster(rows)
    if sum(len(v) for v in groups.values()) + len(uncat) != len(rows):
        print("[refuse] 基数不闭合，草稿会漏提交，拒绝出稿", file=sys.stderr)
        return 3
    txt = render(groups, uncat, rows, a.since or None, a.count)
    if a.out:
        if os.path.abspath(a.out).endswith("CHANGELOG.md"):
            print("[refuse] --out 指向 CHANGELOG.md：本工具只出草稿，正式入账需人工抽查后自己改", file=sys.stderr)
            return 3
        d = os.path.dirname(os.path.abspath(a.out))
        if d and not os.path.isdir(d):
            print("[refuse] 目录不存在：%s" % d, file=sys.stderr)
            return 3
        io.open(a.out, "w", encoding="utf-8", newline="\n").write(txt + "\n")
        print("草稿已写：%s（%d 行 / %d 条提交）" % (a.out, txt.count("\n") + 1, len(rows)))
    else:
        print(txt)
    return 0


if __name__ == "__main__":
    sys.exit(main())

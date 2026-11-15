#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""D10 夜巡清偿排序：把 memory/bugs.md 的 OPEN 缺陷按「严重度 × 模块热区」排成待批队列。

出处：docs/决策登记表-2026-09-29.md D10（人工裁定「建议自动排序（严重度 × 模块热区），人工只批准批次边界」）。

只读、只排序：不派单、不 claim、不改台账、不调用任何编译器入口。批次边界以「建议」形式打印，须人批准。

用法：
  python div-tools/bug_queue.py [--ledger memory/bugs.md] [--days 60] [--batch 3] [--json]
  python div-tools/bug_queue.py --selftest      # 六格自检（S1/S4/S5 正向、S2/S3/S6 必然会红的对照）
"""
import argparse
import datetime as dt
import io
import json
import os
import re
import subprocess
import sys

SEVERITY = {"critical": 3.0, "high": 2.0, "medium": 1.0, "low": 0.5}
HEAD = re.compile(r"^## (BUG-\d+)(.*)$", re.M)
SEGS = re.compile(r"\[([^\]]*)\]")
PATHISH = re.compile(r"\b((?:[\w.\-]+/)+[\w.\-]+\.(?:rs|lz|py|toml))\b")


def parse_ledger(text):
    """扫全部 `## BUG-N` 抬头。返回 (可排序条目, 抬头总数, 拒绝原因)。

    未知严重度 / 形状缺段 ⇒ 进拒绝清单并由调用方拒绝出榜（静默丢条目会让榜单看着完整其实少一条）。
    """
    heads = list(HEAD.finditer(text))
    bugs, refused = [], []
    for i, m in enumerate(heads):
        bid = m.group(1)
        rest = m.group(2)
        segs = SEGS.findall(rest)
        tail = text[m.end(): heads[i + 1].start() if i + 1 < len(heads) else len(text)]
        status = re.search(r"\b(OPEN|FIXED|DUPLICATE|FALSE_POSITIVE)\b", rest)
        if len(segs) < 2 or not status:
            refused.append("%s 抬头缺段（需 [时间] [严重度] 且带状态词）：%s" % (bid, rest[:50]))
            continue
        sev = segs[-1].strip().lower()
        if sev not in SEVERITY:
            refused.append("%s 严重度未知 %r（闭集：%s）" % (bid, sev, "/".join(sorted(SEVERITY))))
            continue
        summ = re.search(r"^- summary:\s*(.*)$", tail, re.M)
        mods = sorted({p.split("/")[-2] for p in PATHISH.findall(rest + tail) if "/" in p})
        bugs.append({"id": bid, "ts": segs[0], "severity": sev, "status": status.group(1),
                     "modules": mods, "paths": sorted({p for p in PATHISH.findall(rest + tail)}),
                     "summary": summ.group(1) if summ else ""})
    return bugs, len(heads), refused


def hot_zones(repo, days):
    """热区口径 = 近 days 天 git 改到的文件，按**其父目录名**计的改动文件数。

    用父目录名而不是 `src/x` 前两段，是因为台账里的站点常写成 `codegen/mod.rs:2162` 这种省略
    `src/ir/` 的短形；按前两段取数会让这类条目恒得 0 分，「× 热区」就成了装饰（第一版就踩了这点）。
    """
    try:
        out = subprocess.run(["git", "-C", repo, "log", "--since=%d days ago" % days,
                              "--name-only", "--pretty=format:"],
                             capture_output=True, text=True, encoding="utf-8",
                             errors="replace", timeout=180).stdout
    except Exception as e:                                      # noqa: BLE001
        print("[warn] 热区取数失败 ⇒ 本次只按严重度排：%s" % e, file=sys.stderr)
        return {}
    hot = {}
    for line in out.splitlines():
        p = line.strip().replace("\\", "/")
        parts = p.split("/")
        if len(parts) < 2:
            continue
        parent = parts[-2]
        hot[parent] = hot.get(parent, 0) + 1
    return hot


def rank(bugs, hot):
    top = max(hot.values()) if hot else 0
    for b in bugs:
        h = sum(hot.get(m, 0) for m in b["modules"])
        b["hot"] = h
        b["score"] = round(SEVERITY[b["severity"]] * (1.0 + (h / top if top else 0.0)), 3)
    return sorted(bugs, key=lambda b: (-b["score"], b["id"]))


def batches(items, size):
    return [items[i:i + size] for i in range(0, len(items), size)]


def expect(name, passed, detail=""):
    print("%s %s%s" % ("PASS" if passed else "FAIL", name, ("  ← " + detail) if not passed else ""))
    if not passed:
        sys.exit(1)
    return True


def selftest():
    good = ("## BUG-1 [2026-01-01T00:00:00Z] [critical] OPEN\n- summary: 函数体坍塌\n"
            "  detail: 站点 src/ir/codegen/mod.rs:1\n\n"
            "## BUG-2 [2026-01-02] [low] FIXED\n- summary: 已修\n")
    bugs, total, refused = parse_ledger(good)
    expect("S1 正常两抬头全解析、模块取路径父目录名",
           total == 2 and len(bugs) == 2 and not refused and bugs[0]["modules"] == ["codegen"],
           "total=%s bugs=%s refused=%s" % (total, bugs, refused))
    bad = "## BUG-3 [2026-01-01] [紧急] OPEN\n"
    bugs, total, refused = parse_ledger(bad)
    expect("S2 未知严重度被拒且不留空榜单（对照格，必然红）",
           total == 1 and bugs == [] and len(refused) == 1, "bugs=%s refused=%s" % (bugs, refused))
    bad2 = "## BUG-4 缺方括号与状态词\n"
    bugs, total, refused = parse_ledger(bad2)
    expect("S3 形状不符的抬头计入总数并进拒绝清单（对照格）",
           total == 1 and bugs == [] and len(refused) == 1, "total=%s refused=%s" % (total, refused))
    hot = {"ir": 100, "lexer": 1}
    items = [{"id": "BUG-A", "severity": "medium", "modules": ["lexer"]},
             {"id": "BUG-B", "severity": "medium", "modules": ["ir"]}]
    out = rank(items, hot)
    expect("S4 同严重度下热区高的排前面", out[0]["modules"] == ["ir"], "%s" % out)
    out0 = rank([{"id": "BUG-C", "severity": "high", "modules": []}], {})
    expect("S5 热区取数失败时降级为纯严重度、不弃权（对照：score 必须非零）", out0[0]["score"] == 2.0, "%s" % out0)
    # S6：热区必须承重——省略 src/ 前缀的短形站点也要拿到账，否则「× 热区」是装饰
    txt = ("## BUG-7 [2026-01-03] [medium] OPEN\n- summary: 热区条目\n  detail: 站点 codegen/mod.rs:2162-2164\n\n"
           "## BUG-8 [2026-01-04] [medium] OPEN\n- summary: 冷区条目\n  detail: 站点 simd/x.rs:1\n")
    p, _, _ = parse_ledger(txt)
    r = rank(p, {"codegen": 430, "simd": 1})
    expect("S6 短形 `codegen/mod.rs` 仍计入热区并排到冷区之前（热区不承重即红）",
           r[0]["id"] == "BUG-7" and r[0]["score"] > SEVERITY["medium"], "%s" % [(b["id"], b["modules"], b["score"]) for b in r])
    return 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ledger", default="memory/bugs.md")
    ap.add_argument("--days", type=int, default=60)
    ap.add_argument("--batch", type=int, default=3)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    repo = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    if not os.path.exists(a.ledger):
        print("台账不存在：%s" % a.ledger, file=sys.stderr)
        return 2
    bugs, total, refused = parse_ledger(io.open(a.ledger, encoding="utf-8").read())
    if refused:
        print("[refuse] %d/%d 条抬头无法解析 ⇒ 榜单必然不完整，拒绝出榜：" % (len(refused), total), file=sys.stderr)
        for r in refused:
            print("   - " + r, file=sys.stderr)
        return 3
    opened = [b for b in bugs if b["status"] == "OPEN"]
    if len(bugs) != total:
        print("[refuse] 基数不符：解析 %d != 抬头 %d" % (len(bugs), total), file=sys.stderr)
        return 3
    ranked = rank(opened, hot_zones(repo, a.days))
    if a.json:
        print(json.dumps({"generated_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                          "ledger": a.ledger, "days": a.days, "heads": total, "open": len(opened),
                          "queue": ranked}, ensure_ascii=False, indent=1))
        return 0
    print("D10 清偿队列（严重度 × 模块热区；热区 = 近 %d 天 git 改到的文件按其父目录名计数）" % a.days)
    print("台账 %s ｜ 抬头 %d 条 ｜ OPEN %d 条 ｜ 非 OPEN %d 条" % (a.ledger, total, len(opened), total - len(opened)))
    if not ranked:
        print("\n队列为空 ⇒ 无 OPEN 缺陷（不是错误，是本工具无事可做）")
        return 0
    for i, grp in enumerate(batches(ranked, a.batch), 1):
        print("\n批次 B%d（建议边界，待人工批准）" % i)
        for b in grp:
            print("  %-7s %-8s score=%-6s hot=%-4s %-22s %s"
                  % (b["id"], b["severity"], b["score"], b["hot"], ",".join(b["modules"]) or "?", b["summary"][:50]))
    print("\n口径：score = 严重度权重(critical 3 / high 2 / medium 1 / low 0.5) × (1 + 命中目录热度 / 最热目录热度)。")
    unres = [b["id"] for b in ranked if not b["modules"]]
    if unres:
        print("热度不可定位（站点写成裸文件名或根本没写路径 ⇒ 该项乘数恒 1，`hot=?` 不是「冷区证据」）：%s" % " ".join(unres))
    sev_only = sorted(opened, key=lambda b: (-SEVERITY[b["severity"]], b["id"]))
    flipped = [b["id"] for b in ranked if b["id"] not in [x["id"] for x in sev_only[:ranked.index(b) + 1]]]
    if flipped:
        print("⚠ 与「纯严重度优先」名次不一致的条目：%s —— 批准边界时请逐条确认这是热度带来的正确重排，"
              "还是上一条里的定位缺失。" % " ".join(flipped))
    print("边界由人批准后才派单。")
    return 0


if __name__ == "__main__":
    sys.exit(main())

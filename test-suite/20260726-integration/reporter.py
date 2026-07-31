#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""测试报告生成器：控制台彩色输出 + JSON + Markdown"""

import os
import json
import sys
from collections import defaultdict, OrderedDict
from typing import List, Dict, Any
from dataclasses import asdict
from runner import TestResult, TestCase
from config import Config


# ANSI 颜色代码
GREEN = "\033[92m"
RED = "\033[91m"
YELLOW = "\033[93m"
CYAN = "\033[96m"
BOLD = "\033[1m"
DIM = "\033[2m"
RESET = "\033[0m"


class TestReporter:
    """报告生成器"""

    def __init__(self, config: Config):
        self.config = config

    # ---- 控制台报告 ----
    def console_summary(self, results: List[TestResult]):
        """彩色控制台摘要"""
        total = len(results)
        passed = sum(1 for r in results if r.status == "PASS")
        failed = sum(1 for r in results if r.status == "FAIL")
        skipped = sum(1 for r in results if r.status == "SKIP")
        crashed = sum(1 for r in results if r.status == "CRASH")

        print()
        print(f"{BOLD}{'='*60}{RESET}")
        print(f"{BOLD}  LZ 集成测试报告{RESET}")
        print(f"{BOLD}{'='*60}{RESET}")
        print(f"  Total: {total}  |  "
              f"{GREEN}PASS: {passed}{RESET}  |  "
              f"{RED}FAIL: {failed}{RESET}  |  "
              f"{YELLOW}SKIP: {skipped}{RESET}  |  "
              f"{RED}CRASH: {crashed}{RESET}")
        print(f"{BOLD}{'='*60}{RESET}")

        # 按优先级统计
        by_priority = self._group_by_priority(results)
        print(f"\n  {BOLD}按优先级:{RESET}")
        for p in ["P0", "P1", "P2"]:
            if p in by_priority:
                t = by_priority[p]["total"]
                p_ok = by_priority[p]["passed"]
                pct = p_ok / t * 100 if t > 0 else 0
                color = GREEN if pct >= 90 else (YELLOW if pct >= 70 else RED)
                print(f"    {p}    {color}{p_ok}/{t}  ({pct:.1f}%){RESET}")

        # 按 Bug 类型统计
        by_bug = self._group_by_bug_type(results)
        if by_bug:
            print(f"\n  {BOLD}按 Bug 类型:{RESET}")
            for bt in sorted(by_bug.keys()):
                t = by_bug[bt]["total"]
                p_ok = by_bug[bt]["passed"]
                pct = p_ok / t * 100 if t > 0 else 0
                color = GREEN if pct >= 90 else (YELLOW if pct >= 70 else RED)
                print(f"    Bug-{bt}  {color}{p_ok}/{t}  ({pct:.1f}%){RESET}")

        # 按分类统计
        by_cat = self._group_by_category(results)
        if by_cat:
            print(f"\n  {BOLD}按分类:{RESET}")
            for cat in sorted(by_cat.keys()):
                t = by_cat[cat]["total"]
                p_ok = by_cat[cat]["passed"]
                pct = p_ok / t * 100 if t > 0 else 0
                color = GREEN if pct >= 90 else (YELLOW if pct >= 70 else RED)
                print(f"    {cat:<20s}  {color}{p_ok}/{t}  ({pct:.1f}%){RESET}")

        # 失败列表
        failures = [r for r in results if r.status == "FAIL"]
        crashes = [r for r in results if r.status == "CRASH"]

        if failures or crashes:
            print(f"\n  {BOLD}{RED}失败/崩溃用例:{RESET}")
            for r in failures:
                print(f"    {RED}FAIL{RESET} {r.case.id}  {r.case.title}")
                for prob in r.problems[:3]:
                    print(f"         {DIM}-> {prob}{RESET}")
                if r.output_snippet:
                    print(f"         {DIM}output: {r.output_snippet[:100]}{RESET}")
            for r in crashes:
                print(f"    {RED}CRASH{RESET} {r.case.id}  {r.case.title}")
                for prob in r.problems[:2]:
                    print(f"         {DIM}-> {prob}{RESET}")

        # Bug 发现摘要
        bugs_found = self._extract_bugs(results)
        if bugs_found:
            print(f"\n  {BOLD}{RED}发现的 Bug 摘要:{RESET}")
            for b in bugs_found:
                print(f"    {RED}[Bug-{b['bug_type']}]{RESET} {b['id']} - {b['reason']}")

        print(f"\n{BOLD}{'='*60}{RESET}\n")
        return passed, failed, skipped, crashed

    # ---- JSON 报告 ----
    def generate_json(self, results: List[TestResult]) -> str:
        """生成 JSON 报告文件"""
        out_path = os.path.join(self.config.reports_dir, "report.json")
        summary = self._build_json_summary(results)
        with open(out_path, "w", encoding="utf-8") as f:
            json.dump(summary, f, indent=2, ensure_ascii=False)
        return out_path

    # ---- Markdown 报告 ----
    def generate_markdown(self, results: List[TestResult]) -> str:
        """生成 Markdown 报告文件"""
        out_path = os.path.join(self.config.reports_dir, "report.md")
        total = len(results)
        passed = sum(1 for r in results if r.status == "PASS")
        failed = sum(1 for r in results if r.status == "FAIL")
        skipped = sum(1 for r in results if r.status == "SKIP")
        crashed = sum(1 for r in results if r.status == "CRASH")

        lines = []
        lines.append("# LZ 集成测试报告")
        lines.append(f"\n**时间**: {self._now()}")
        lines.append(f"\n**编译器**: `{self.config.sut_binary}`")
        lines.append("\n---")
        lines.append(f"\n## 总览")
        lines.append(f"\n| 指标 | 数值 |")
        lines.append(f"|------|------|")
        lines.append(f"| 总用例 | {total} |")
        lines.append(f"| 通过 | {passed} |")
        lines.append(f"| 失败 | {failed} |")
        lines.append(f"| 跳过 | {skipped} |")
        lines.append(f"| 崩溃 | {crashed} |")
        if total > 0:
            lines.append(f"| 通过率 | {passed/total*100:.1f}% |")

        # 按优先级
        by_priority = self._group_by_priority(results)
        lines.append("\n## 按优先级")
        for p in ["P0", "P1", "P2"]:
            if p in by_priority:
                t = by_priority[p]["total"]
                p_ok = by_priority[p]["passed"]
                pct = p_ok / t * 100 if t > 0 else 0
                lines.append(f"- **{p}**: {p_ok}/{t} ({pct:.1f}%)")

        # 按 Bug 类型
        by_bug = self._group_by_bug_type(results)
        if by_bug:
            lines.append("\n## 按 Bug 类型")
            for bt in sorted(by_bug.keys()):
                t = by_bug[bt]["total"]
                p_ok = by_bug[bt]["passed"]
                pct = p_ok / t * 100 if t > 0 else 0
                lines.append(f"- **Bug-{bt}**: {p_ok}/{t} ({pct:.1f}%)")

        # 按分类
        by_cat = self._group_by_category(results)
        if by_cat:
            lines.append("\n## 按分类")
            lines.append("\n| 分类 | 总数 | 通过 | 通过率 |")
            lines.append("|------|------|------|--------|")
            for cat in sorted(by_cat.keys()):
                t = by_cat[cat]["total"]
                p_ok = by_cat[cat]["passed"]
                pct = p_ok / t * 100 if t > 0 else 0
                lines.append(f"| {cat} | {t} | {p_ok} | {pct:.1f}% |")

        # 失败详情
        failures = [r for r in results if r.status in ("FAIL", "CRASH")]
        if failures:
            lines.append("\n## 失败用例")
            for r in failures:
                lines.append(f"\n### {r.status}: {r.case.id} — {r.case.title}")
                lines.append(f"- **优先级**: {r.case.priority}")
                lines.append(f"- **分类**: {r.case.category}")
                lines.append(f"- **Bug 类型**: {r.case.bug_types}")
                for prob in r.problems:
                    lines.append(f"- **问题**: {prob}")
                if r.output_snippet:
                    lines.append(f"- **输出**:\n```\n{r.output_snippet}\n```")

        # 发现的 Bug
        bugs_found = self._extract_bugs(results)
        if bugs_found:
            lines.append("\n## 发现的 Bug")
            for b in bugs_found:
                lines.append(f"- **[Bug-{b['bug_type']}]** {b['id']} — {b['reason']}")

        # 跳过用例
        skipped_cases = [r for r in results if r.status == "SKIP"]
        if skipped_cases:
            lines.append("\n## 跳过用例")
            for r in skipped_cases:
                lines.append(f"- {r.case.id} — {r.case.title} ({r.case.skip_reason})")

        with open(out_path, "w", encoding="utf-8") as f:
            f.write("\n".join(lines))
        return out_path

    # ---- helpers ----
    def _group_by_priority(self, results):
        groups = {}
        for r in results:
            p = r.case.priority
            if p not in groups:
                groups[p] = {"total": 0, "passed": 0}
            groups[p]["total"] += 1
            if r.status == "PASS":
                groups[p]["passed"] += 1
        return groups

    def _group_by_bug_type(self, results):
        groups = {}
        for r in results:
            for bt in r.case.bug_types:
                if bt not in groups:
                    groups[bt] = {"total": 0, "passed": 0}
                groups[bt]["total"] += 1
                if r.status == "PASS":
                    groups[bt]["passed"] += 1
        return groups

    def _group_by_category(self, results):
        groups = {}
        for r in results:
            cat = r.case.category.split("/")[0] if "/" in r.case.category else r.case.category
            if cat not in groups:
                groups[cat] = {"total": 0, "passed": 0}
            groups[cat]["total"] += 1
            if r.status == "PASS":
                groups[cat]["passed"] += 1
        return groups

    def _extract_bugs(self, results):
        """提取发现的 Bug（仅失败用例）"""
        bugs = []
        for r in results:
            if r.status != "FAIL":
                continue
            for bt in r.case.bug_types:
                bugs.append({
                    "id": r.case.id,
                    "bug_type": bt,
                    "reason": r.case.bug_reason or r.case.title,
                    "problems": r.problems,
                })
        return bugs

    def _build_json_summary(self, results):
        return {
            "timestamp": self._now(),
            "binary": self.config.sut_binary,
            "summary": {
                "total": len(results),
                "passed": sum(1 for r in results if r.status == "PASS"),
                "failed": sum(1 for r in results if r.status == "FAIL"),
                "skipped": sum(1 for r in results if r.status == "SKIP"),
                "crashed": sum(1 for r in results if r.status == "CRASH"),
            },
            "by_priority": self._group_by_priority(results),
            "by_bug_type": self._group_by_bug_type(results),
            "by_category": self._group_by_category(results),
            "bugs_found": self._extract_bugs(results),
            "results": [self._serialize_result(r) for r in results],
        }

    def _serialize_result(self, r: TestResult):
        return {
            "id": r.case.id,
            "title": r.case.title,
            "category": r.case.category,
            "priority": r.case.priority,
            "mode": r.case.mode,
            "status": r.status,
            "bug_types": r.case.bug_types,
            "bug_reason": r.case.bug_reason,
            "problems": r.problems,
            "duration_ms": round(r.duration_ms, 1),
            "output_snippet": r.output_snippet[:200] if r.output_snippet else "",
        }

    def _now(self):
        import datetime
        return datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")

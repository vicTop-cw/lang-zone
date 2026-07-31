#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""测试执行引擎 (TestRunner)"""

import os
import sys
import subprocess
import time
from dataclasses import dataclass, field
from typing import List, Optional, Dict, Any, Tuple
from config import Config


@dataclass
class TestCase:
    """单个测试用例定义"""
    id: str                       # 唯一标识
    title: str                    # 描述
    category: str                 # 分类路径
    priority: str                 # P0/P1/P2
    mode: str                     # tokens|ast|rust|error|compile|run
    source_file: str              # cases/ 下的相对 .lz 文件路径
    present: List[str] = field(default_factory=list)   # 输出必须包含的子串
    absent: List[str] = field(default_factory=list)     # 输出禁止包含的子串
    error_contains: Optional[str] = None    # error 模式: 错误信息应包含的子串
    bug_types: List[int] = field(default_factory=list)  # 对应 Bug 类型
    bug_reason: str = ""
    needs_std: bool = True        # 是否需要 --std-dir
    rustc_args: List[str] = field(default_factory=list)  # rustc 额外参数
    skip_reason: Optional[str] = None

    def source_path(self):
        """获取 .lz 源文件绝对路径"""
        from config import CASES_DIR
        return os.path.join(CASES_DIR, self.source_file)


@dataclass
class TestResult:
    """单个测试结果"""
    case: TestCase
    status: str = "PASS"     # PASS / FAIL / SKIP / CRASH
    problems: List[str] = field(default_factory=list)
    duration_ms: float = 0.0
    output_snippet: str = "" # 失败时的输出片段

    @classmethod
    def skipped(cls, case: TestCase):
        return cls(case=case, status="SKIP")

    @classmethod
    def crashed(cls, case: TestCase, error: str):
        return cls(case=case, status="CRASH", problems=[error])

    def fail(self, reason: str, output: str = ""):
        self.status = "FAIL"
        self.problems.append(reason)
        if output:
            self.output_snippet = output[:500]

    def add_problem(self, problem: str):
        if self.status == "PASS":
            self.status = "FAIL"
        self.problems.append(problem)


class TestRunner:
    """测试执行引擎"""

    def __init__(self, config: Config):
        self.config = config
        self.results: List[TestResult] = []

    def run_all(self, cases: List[TestCase],
                filter_priority: Optional[str] = None,
                filter_category: Optional[str] = None,
                filter_bug_type: Optional[int] = None,
                filter_ids: Optional[List[str]] = None,
                only_failed: bool = False) -> List[TestResult]:
        """执行所有测试"""
        self.results = []

        for case in cases:
            # 过滤
            if filter_priority and case.priority != filter_priority:
                continue
            if filter_category and not case.category.startswith(filter_category):
                continue
            if filter_bug_type is not None and filter_bug_type not in case.bug_types:
                continue
            if filter_ids and case.id not in filter_ids:
                continue
            if case.skip_reason:
                self.results.append(TestResult.skipped(case))
                continue

            # 执行
            result = self._execute(case)
            self.results.append(result)

        return self.results

    def _execute(self, case: TestCase) -> TestResult:
        """执行单个测试用例"""
        result = TestResult(case=case)
        t0 = time.time()

        try:
            # 1. 读取 .lz 源文件
            src_path = case.source_path()
            if not os.path.isfile(src_path):
                result.fail(f"Source file not found: {src_path}")
                result.duration_ms = (time.time() - t0) * 1000
                return result

            # 2. lz → rs 编译
            # .rs 生成在 .lz 同目录下（编译器行为）
            rs_path = src_path.replace(".lz", ".rs")
            lz_rc, lz_stdout, lz_stderr = self._run_lz(src_path, case)

            # 3. error 模式特殊处理
            if case.mode == "error":
                if lz_rc == 0:
                    result.fail(f"Expected error but compile succeeded", lz_stdout + lz_stderr)
                elif case.error_contains and case.error_contains not in lz_stderr:
                    result.fail(
                        f"Error message missing '{case.error_contains}'\nGot: {lz_stderr[:300]}",
                        lz_stderr
                    )
                else:
                    pass  # PASS
                result.duration_ms = (time.time() - t0) * 1000
                return result

            if lz_rc != 0:
                result.fail(f"LZ compile failed (rc={lz_rc}): {lz_stderr[:200]}", lz_stderr)
                result.duration_ms = (time.time() - t0) * 1000
                return result

            # 4. 验证 .rs 内容 (如果有 present/absent 断言，且不是 tokens/ast 模式)
            if case.mode not in ("tokens", "ast") and (case.present or case.absent):
                rs_content = ""
                if os.path.isfile(rs_path):
                    with open(rs_path, "r", encoding="utf-8") as f:
                        rs_content = f.read()
                for p in case.present:
                    if p not in rs_content:
                        result.add_problem(f"Missing in output: '{p}'")
                for a in case.absent:
                    if a in rs_content:
                        result.add_problem(f"Forbidden in output: '{a}'")

            # 5. tokens / ast / rust 模式到此为止
            if case.mode in ("tokens", "ast", "rust"):
                # tokens/ast 模式: also check stdout
                if case.mode in ("tokens", "ast"):
                    output = lz_stdout + lz_stderr
                    for p in case.present:
                        if p not in output:
                            result.add_problem(f"Missing in output: '{p}'")
                    for a in case.absent:
                        if a in output:
                            result.add_problem(f"Forbidden in output: '{a}'")
                result.duration_ms = (time.time() - t0) * 1000
                return result

            # 6. rustc 编译
            exe_path = os.path.join(self.config.work_dir, case.id + ".exe")
            rustc_ok, rustc_err = self._run_rustc(rs_path, exe_path, case)
            if not rustc_ok:
                result.fail(f"rustc compile failed: {rustc_err[:300]}", rustc_err)
                result.duration_ms = (time.time() - t0) * 1000
                return result

            if case.mode == "compile":
                result.duration_ms = (time.time() - t0) * 1000
                return result

            # 7. run 模式: 运行并验证输出
            run_ok, stdout, stderr = self._run_exe(exe_path)
            output = stdout + stderr
            for p in case.present:
                if p not in output:
                    result.add_problem(f"Missing from run output: '{p}'")
            for a in case.absent:
                if a in output:
                    result.add_problem(f"Forbidden in run output: '{a}'")
            if not run_ok:
                result.add_problem(f"Process exited non-zero")

            result.duration_ms = (time.time() - t0) * 1000
            return result

        except subprocess.TimeoutExpired:
            result.fail("Timeout")
            result.duration_ms = (time.time() - t0) * 1000
            return result
        except Exception as e:
            result.fail(f"Exception: {str(e)}")
            result.duration_ms = (time.time() - t0) * 1000
            return result

    def _run_lz(self, src_path: str, case: TestCase) -> Tuple[int, str, str]:
        """运行 lang-zone 编译器"""
        args = [self.config.sut_binary, src_path]
        # tokens/ast 模式传递对应标志
        if case.mode == "tokens":
            args.append("--tokens")
        elif case.mode == "ast":
            args.append("--ast")
        # 某些测试需要禁止 strict 检查（如使用 .unwrap() 的测试）
        if case.needs_std and self.config.std_dir:
            args += ["--std-dir", self.config.std_dir]
        proc = subprocess.run(
            args,
            capture_output=True, text=True,
            timeout=self.config.timeout,
            cwd=os.path.dirname(src_path)
        )
        return proc.returncode, proc.stdout or "", proc.stderr or ""

    def _run_rustc(self, rs_path: str, exe_path: str, case: TestCase) -> Tuple[bool, str]:
        """rustc 编译 .rs → .exe"""
        args = ["rustc", "--edition", self.config.rustc_edition]
        args += case.rustc_args
        args += [rs_path, "-o", exe_path]
        proc = subprocess.run(
            args,
            capture_output=True, text=True,
            timeout=self.config.rustc_timeout
        )
        return proc.returncode == 0, proc.stderr or ""

    def _run_exe(self, exe_path: str) -> Tuple[bool, str, str]:
        """运行编译产物"""
        proc = subprocess.run(
            [exe_path],
            capture_output=True, text=True,
            timeout=self.config.run_timeout
        )
        return proc.returncode == 0, proc.stdout or "", proc.stderr or ""

#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""测试配置：编译器路径、std 路径、超时设置等"""

import os
import sys
from dataclasses import dataclass, field
from typing import List, Optional

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))  # lang-zone/
CASES_DIR = os.path.join(HERE, "cases")
WORK_DIR = os.path.join(HERE, "_work")
REPORTS_DIR = os.path.join(HERE, "reports")


@dataclass
class Config:
    """全局测试配置"""
    root_dir: str = HERE
    sut_binary: str = ""           # lang-zone.exe 路径
    std_dir: str = ""              # std/ 目录路径
    work_dir: str = WORK_DIR
    reports_dir: str = REPORTS_DIR
    timeout: int = 30              # lz 编译超时
    rustc_timeout: int = 60        # rustc 编译超时
    run_timeout: int = 15          # 运行超时
    rustc_edition: str = "2021"

    @classmethod
    def auto_discover(cls):
        """自动发现编译器和 std 目录"""
        c = cls()
        c.sut_binary = c._find_binary()
        c.std_dir = c._find_std()
        return c

    def _find_binary(self):
        candidates = [
            os.path.join(ROOT, "target", "debug", "lang-zone.exe"),
            os.path.join(ROOT, "target", "debug", "lang-zone"),
            os.path.join(ROOT, "target", "release", "lang-zone.exe"),
            os.path.join(ROOT, "target", "release", "lang-zone"),
        ]
        for p in candidates:
            if os.path.isfile(p):
                return p
        return None

    def _find_std(self):
        candidates = [
            os.path.join(ROOT, "std"),
        ]
        for p in candidates:
            if os.path.isdir(p):
                return p
        return None

    def ensure_binary(self):
        if not self.sut_binary or not os.path.isfile(self.sut_binary):
            print(f"Error: lang-zone binary not found. Run `cargo build` first.")
            print(f"Searched: {self._find_binary()}")
            return False
        return True

    def ensure_std(self):
        if not self.std_dir or not os.path.isdir(self.std_dir):
            print(f"Warning: std/ directory not found at {self.std_dir}")
            print("Some tests may fail without --std-dir.")
            return False
        return True

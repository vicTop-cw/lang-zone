#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
函数定义语法测试套件
====================

验证范围：
  1. 函数定义语法修复：双逗号 ..,,.. 必须拒绝，单逗号 ..,.. 合法
  2. 8 类函数/结构正确解析并产出预期 Rust 代码：
     C01 方法 / C02 普通函数 / C03 嵌套函数 / C04 匿名函数（闭包）
     C05 仓颉宏 / C06 仓颉宏模版 / C07 装饰器 / C08 Rust 宏（直通桥）
  3. 4 个负向用例：双逗号在 参数列表 / 尾随 / 泛型参数 处均被拒绝

用法：
  python run_tests.py            # 自动定位 lang-zone 二进制
  python run_tests.py --bin /path/to/lang-zone[.exe]

退出码：0 全部通过，1 存在失败
"""

import os
import sys
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
CASES_DIR = os.path.join(HERE, "cases")
ROOT = os.path.dirname(os.path.dirname(HERE))  # E:/IDEProjects/AI/lang-zone

# ---------------------------------------------------------------------------
# 二进制定位
# ---------------------------------------------------------------------------
def find_binary(explicit=None):
    if explicit:
        return explicit
    candidates = [
        os.path.join(ROOT, "target", "debug", "lang-zone.exe"),
        os.path.join(ROOT, "target", "debug", "lang-zone"),
        os.path.join(ROOT, "target", "release", "lang-zone.exe"),
        os.path.join(ROOT, "target", "release", "lang-zone"),
    ]
    for c in candidates:
        if os.path.isfile(c):
            return c
    return None


# ---------------------------------------------------------------------------
# 正向用例：解析成功 + 关键产物断言
#   expects: 生成的 .rs 中必须包含的子串（任一缺失即失败）
# ---------------------------------------------------------------------------
POSITIVE = {
    "C01_method": [
        "trait Drawable {",
        "fn draw(&self) -> String;",
        "impl Point {",
        "fn area(&self) -> i64 {",
        "fn shift(&mut self, dx: i64) -> Point {",
    ],
    "C02_func": [
        "fn add(a: i64, b: i64) -> i64 {",
        "fn power(base: i64, exp: i64 = 2) -> i64 {",
        "fn identity<T>(x: T) -> T {",
        "enum AppError {",
        "fn might_fail(x: i64) -> Result<i64, AppError> {",
    ],
    "C03_nested": [
        "fn outer(x: i64) -> i64 {",
        "fn inner(y: i64) -> i64 {",
        "inner(x) + 1",
    ],
    "C04_closure": [
        "let f = |a, b| a + b;",
        "let g = |n| n * n;",
        "f(3, 4)",
    ],
    "C05_cangjie_macro": [
        "__MODULE_IS_MACRO: bool = true;",
        'println!("macro ok")',
    ],
    "C06_macro_template": [
        "__MODULE_IS_MACRO: bool = true;",
        "fn answer() -> i64 {",
        "42",
        "__MODULE_ALL: &[&str] = &[\"answer\", \"get_answer\"];",
    ],
    "C07_decorator": [
        "#[timed]",
        "fn calc(x: i64) -> i64 {",
        "#[log]",
        "fn process(items: Vec<i64>) -> i64 {",
    ],
    "C08_rust_macro": [
        "__MODULE_BRIDGE: bool = true;",
        'println!("rust bridge")',
        "__MODULE_DEPS",
    ],
}

# ---------------------------------------------------------------------------
# 负向用例：必须解析失败，且报错含“连续逗号”
# ---------------------------------------------------------------------------
NEGATIVE = {
    "E01_double_comma_both": "连续逗号",
    "E02_double_comma_params": "连续逗号",
    "E03_double_comma_trailing": "连续逗号",
    "E04_double_comma_generic": "连续逗号",
}


def run_case(binary, name):
    src = os.path.join(CASES_DIR, name + ".lz")
    try:
        proc = subprocess.run(
            [binary, src],
            cwd=CASES_DIR,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except subprocess.TimeoutExpired:
        return False, "超时（>60s）"
    rc = proc.returncode
    out = (proc.stdout or "") + (proc.stderr or "")
    return rc == 0, out


def cleanup_rs():
    for fn in os.listdir(CASES_DIR):
        if fn.endswith(".rs"):
            try:
                os.remove(os.path.join(CASES_DIR, fn))
            except OSError:
                pass


def main():
    explicit = None
    if "--bin" in sys.argv:
        i = sys.argv.index("--bin")
        if i + 1 < len(sys.argv):
            explicit = sys.argv[i + 1]
    binary = find_binary(explicit)
    if not binary:
        print("✗ 找不到 lang-zone 二进制，请先 cargo build，或用 --bin 指定路径")
        return 1

    print(f"使用二进制: {binary}\n")
    passed = 0
    failed = 0

    # 正向
    for name, expects in POSITIVE.items():
        ok, out = run_case(binary, name)
        if not ok:
            print(f"✗ {name} 解析失败（期望成功）:\n  {out.strip()}")
            failed += 1
            continue
        rs_path = os.path.join(CASES_DIR, name + ".rs")
        if not os.path.isfile(rs_path):
            print(f"✗ {name} 未生成 .rs 产物")
            failed += 1
            continue
        with open(rs_path, "r", encoding="utf-8") as f:
            content = f.read()
        miss = [s for s in expects if s not in content]
        if miss:
            print(f"✗ {name} 产物缺少预期片段: {miss}")
            failed += 1
        else:
            print(f"✓ {name} 解析通过 + 产物断言全部命中（{len(expects)} 项）")
            passed += 1

    # 负向
    for name, err_sub in NEGATIVE.items():
        ok, out = run_case(binary, name)
        if ok:
            print(f"✗ {name} 错误地被接受（期望拒绝）")
            failed += 1
            continue
        if err_sub not in out:
            print(f"✗ {name} 被拒绝，但报错信息不含'{err_sub}':\n  {out.strip()}")
            failed += 1
            continue
        print(f"✓ {name} 正确拒绝（报错含'{err_sub}'）")
        passed += 1

    cleanup_rs()
    print(f"\n结果: {passed} 通过 / {failed} 失败 / 共 {passed + failed}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

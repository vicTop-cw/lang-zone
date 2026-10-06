#!/usr/bin/env python3
# -*- coding: utf-8 -*-
r"""cy_test_gate.py — BUG-18 的**调用面**判据：`--backend=cython --test` 必须真的执行测试。

为什么不是单元测试：cy 后端的 `test` 块早就被 `gen_test` 发成 `def test_<name>():`
（src/ir/codegen_cython.rs:1222），缺的是**跑它并逐条报状态**那一段。只看发射形态的
单测会绿，而调用面 `lzc ... --test` 依旧一个 test 都不执行——那正是 BUG-18 卡里记的
「打一行 Generated + rc=0」的形状。所以这里打真入口。

判据形状（每条都给原因，不只比 rc——只比 rc 会把「拒绝」读成「失败」）：
  pass_only        rc=0 且逐条 `test pass_a ... ok` / `test pass_b ... ok` + 汇总 `2 passed; 0 failed`
  one_fails        rc!=0 且**同时**有 `test good ... ok` 与 `test bad ... FAILED` + `1 passed; 1 failed`
  setup_visible    rc=0 且两条 sees_setup 都 ok（setup 变量在每个 test 里可见）
控制格（--selfcheck，喂罐头输出，必须全部判红）：
  C1 rc=0 但没有任何 test 行        ⇒ 「跑过了」是假话
  C2 rc=1 且只有拒绝文案            ⇒ 拒绝 ≠ 执行面可用
  C3 缺 FAILED 行却报 1 failed      ⇒ 汇总与逐条不自洽

用法：
    python CY/scripts/cy_test_gate.py [二进制路径]
    python CY/scripts/cy_test_gate.py --selfcheck
退出码：0 全绿；1 有红格；2 环境错（二进制缺失/不可执行）。
"""
import io
import os
import re
import shutil
import subprocess
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
FIXTURES = os.path.join(REPO, "CY", "fixtures", "testface")
RESULT_RE = r"test result: (\w+)\. (\d+) passed; (\d+) failed"


def default_bin():
    name = "lang-zone.exe" if os.name == "nt" else "lang-zone"
    return os.path.join(REPO, "target", "debug", name)


def spec_pass_only():
    return {
        "rc": ("zero", "全部 test 通过时整体必须 rc=0"),
        "lines": ["test pass_a ... ok", "test pass_b ... ok"],
        "counts": (2, 0),
        "verdict": "ok",
    }


def spec_one_fails():
    return {
        "rc": ("nonzero", "有 FAILED 时整体必须非零"),
        "lines": ["test good ... ok", "test bad ... FAILED"],
        "counts": (1, 1),
        "verdict": "FAILED",
    }


def spec_setup_visible():
    return {
        "rc": ("zero", "setup 内联正确则两条都过"),
        "lines": ["test sees_setup ... ok", "test sees_setup_again ... ok"],
        "counts": (2, 0),
        "verdict": "ok",
    }


SPECS = {
    "pass_only.lz": spec_pass_only(),
    "one_fails.lz": spec_one_fails(),
    "setup_visible.lz": spec_setup_visible(),
}


def judge(stdout, rc, spec):
    """返回 (是否通过, 逐条原因)。原因里带上取到了什么，方便事后复核。"""
    bad = []
    want_rc, why = spec["rc"]
    if want_rc == "zero" and rc != 0:
        bad.append("期望 rc=0，实得 rc=%d（%s）" % (rc, why))
    if want_rc == "nonzero" and rc == 0:
        bad.append("期望 rc!=0，实得 rc=0（%s）" % why)
    for ln in spec["lines"]:
        if ln not in stdout:
            bad.append("逐字缺行 %r" % ln)
    m = re.search(RESULT_RE, stdout)
    if not m:
        bad.append("抽不到汇总行 `test result: …`（模式 %s）" % RESULT_RE)
    else:
        got = (int(m.group(2)), int(m.group(3)))
        if got != spec["counts"]:
            bad.append("汇总 passed/failed=%s 期望 %s" % (got, spec["counts"]))
        if m.group(1) != spec["verdict"]:
            bad.append("汇总结论词=%r 期望 %r" % (m.group(1), spec["verdict"]))
        if (got[1] > 0) != (spec["verdict"] == "FAILED"):
            bad.append("汇总结论词与 failed 数不自洽")
    if re.search(r"未实现|not implemented", stdout):
        bad.append("入口打的是拒绝文案 ⇒ 执行面仍缺（BUG-18 本体）")
    return (len(bad) == 0), bad


def run_case(binary, name, workdir):
    src = os.path.join(FIXTURES, name)
    dst = os.path.join(workdir, name)
    shutil.copyfile(src, dst)
    cmd = [binary, dst, "--backend=cython", "--test"]
    p = subprocess.run(cmd, cwd=workdir, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    out = p.stdout.decode("utf-8", errors="replace")
    return p.returncode, out, cmd


def host_source():
    """从 `src/main.rs` 里抠出内嵌宿主的源码——判据不许另抄一份实现来测。"""
    p = os.path.join(REPO, "src", "main.rs")
    txt = io.open(p, encoding="utf-8", errors="replace").read()
    m = re.search(r'const CY_TEST_HOST: &str = r#"(?P<src>.*?)\n"#;', txt, re.S)
    if not m:
        raise SystemExit("FAIL 反解不到内嵌宿主源码（CY_TEST_HOST 的形状变了）⇒ C4 无从跑")
    return m.group(0)


def extract_inside(src):
    lines = src.splitlines()
    start = [i for i, l in enumerate(lines) if l.startswith("def inside(")]
    if not start:
        raise SystemExit("FAIL 内嵌宿主里没有 `def inside(`（归属判据消失了？）")
    i = start[0]
    out = [lines[i]]
    for l in lines[i + 1:]:
        if l.strip() and not l.startswith((" ", "\t")):
            break
        out.append(l)
    return "\n".join(out)


def selfcheck():
    controls = [
        ("C1 rc=0 但没有任何 test 行", "", 0, SPECS["pass_only.lz"]),
        ("C2 只有拒绝文案", "error: --test 需要测试执行面，cython 后端未实现\n", 1,
         SPECS["one_fails.lz"]),
        ("C3 报了 1 failed 却没有 FAILED 行",
         "test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n", 1,
         SPECS["one_fails.lz"]),
    ]
    bites = 0
    for label, out, rc, spec in controls:
        ok, bad = judge(out, rc, spec)
        if ok:
            print("SELFTEST 红：%s 竟然被判成通过 ⇒ 这道门不承重" % label)
            return 1
        print("对照按预期红：%s（首条原因：%s）" % (label, bad[0]))
        bites += 1
    # C4：归属判据（本轮实测到的两类失效各一支）。宿主实现内嵌在 main.rs 里，
    # 这里**抠出来 exec**，而不是在判据侧另写一份同名函数——否则测的是抄本不是出货面。
    src = host_source()
    if '"__file__"' in src or "mod.__file__" in src:
        print("SELFTEST 红：内嵌宿主又改成读 mod.__file__ 做身份（LZ 产物会自己覆盖它 ⇒ 假红）")
        return 1
    ns = {"os": os}
    exec(extract_inside(src), ns)
    inside = ns["inside"]
    base = os.path.join(REPO, "target")
    good = inside(os.path.join(base, "cy-test", "x.cp313-win_amd64.pyd"),
                  os.path.join(base, "cy-test"))
    sibling = inside(os.path.join(base, "cy-testface", "1", "one_fails.lz"),
                     os.path.join(base, "cy-test"))
    foreign = inside(os.path.join(REPO, "one_fails.lz"), os.path.join(base, "cy-test"))
    if not good:
        print("SELFTEST 红：真归属被判假（inside 恒假 ⇒ 每次都 rc=4，执行面不可用）")
        return 1
    if sibling or foreign:
        print("SELFTEST 红：隔壁目录/仓根的冒名被判真（sibling=%s foreign=%s）⇒ 前缀比法没修掉"
              % (sibling, foreign))
        return 1
    bites += 1
    print("对照按预期红：C4 归属判据（隔壁目录 cy-testface 与仓根冒名都被拒，真归属放行）")
    # 正向对照：罐头绿证必须被判成通过，否则三支红里有一支是「恒红」
    green = ("running 2 tests\n"
             "test pass_a ... ok\n"
             "test pass_b ... ok\n"
             "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n")
    ok, bad = judge(green, 0, SPECS["pass_only.lz"])
    if not ok:
        print("SELFTEST 红：罐头绿证被判红（%s）⇒ 门恒红，读数不可信" % bad)
        return 1
    print("正向对照通过：罐头绿证判绿；%d 支罐头红证各咬一次" % bites)
    return 0


def main():
    if "--selfcheck" in sys.argv:
        return selfcheck()
    argv = [a for a in sys.argv[1:] if not a.startswith("--")]
    binary = argv[0] if argv else default_bin()
    if not os.path.exists(binary):
        print("FAIL 找不到入口二进制：%s（先 cargo build）" % binary)
        return 2
    workdir = os.path.join(REPO, "target", "cy-testface", str(os.getpid()))
    if os.path.isdir(workdir):
        shutil.rmtree(workdir)
    os.makedirs(workdir)
    n_red = 0
    for name, spec in SPECS.items():
        rc, out, cmd = run_case(binary, name, workdir)
        ok, bad = judge(out, rc, spec)
        print("%-22s %s rc=%d 命令：lzc %s --backend=cython --test"
              % (name, "PASS" if ok else "RED", rc, name))
        if not ok:
            n_red += 1
            for b in bad:
                print("     原因：%s" % b)
            print("     stdout 逐字（本用例全部输出）：")
            for line in out.splitlines():
                print("     | %s" % line)
    print("\n合计 %d 格，红 %d 格（红格逐字 stdout 已在上面）" % (len(SPECS), n_red))
    return 1 if n_red else 0


if __name__ == "__main__":
    sys.exit(main())

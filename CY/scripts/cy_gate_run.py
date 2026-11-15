#!/usr/bin/env python3
# -*- coding: utf-8 -*-
r"""cy_gate_run.py — 把一份 .lz 的 Cython 产物编译并执行，stdout 原样透传。

被 `tests/cy_codegen_gate.rs` 的 L3（行为层）调用；也可手工复跑单个用例：

    python CY/scripts/cy_gate_run.py <模块目录> <模块名(不带 .pyx)> [--build-only]

退出码：
  0  执行成功（模块里的 `def main()` 已被调用；没有 main 则仅导入）
  3  编译或运行期异常（traceback 打到 stderr）
  2  参数/环境错
  4  加载到的模块不是本次产物（同名孪生抢占模块名）⇒ 判定作废

本脚本对 stdout 做两件**归一化**，否则 L3 比的是环境不是产物：
  1. 编译噪声不进 stdout：冷缓存那一遍 pyximport 会把 setuptools/MSVC 的链接器
     日志打到 stdout（`variadic_test.c\r\n  正在创建库 …`），而热缓存没有 ⇒
     同一份产物两轮 stdout 不同，门禁时绿时红。做法是先 spawn 自身
     `--build-only` 把编译关进子进程，父进程再 import（缓存命中）⇒
     只有程序输出走 stdout。
  2. 把 stdout 钉成 UTF-8（strict）。GBK 控制台下 `print("✅")` 会抛
     UnicodeEncodeError ⇒ 9 个 self_test 用例的 RUN_FAIL 是代码页造成的，
     不是产物造成的；用 strict 而不是 replace ⇒ 真把非法字节写出来仍会响。

第三件事不是归一化而是**防伪绿**：pyximport 把自己的 finder 追加在
`sys.meta_path` 末尾，同名的外部包（别的仓库的 editable 安装、stdlib、site-packages 里的
旧副本）会先命中 ⇒ import 回来的是别人的纯 Python 包，产物压根没被加载，
而退出码仍是 0（实测 4 个语料名被抢：`test_suite`←`E:\IDEProjects\AI\Cypy\test_suite`、
`enum`/`struct`←stdlib、`coverage`←site-packages ⇒ 那一轮 L3 的 40 个 ok 里含假绿）。
做法：把 .pyx 按内容复制成谁也不可能预占的别名 `_lzcyg_<stem>` 再编译导入
（身份由构造保证），并用产物指纹（`.pyx` 有无 `def main`、恒发的 `_Moved` 垫片）
二次确认；不符就退出码 4。曾试过把 pyx finder 抢到 meta_path[0] —— 会连带打断
setuptools 的 `_distutils_hack`（Py 3.13 无 stdlib distutils）并把 stdlib 的
enum/struct 从 `sys.modules` 里抽走，整目录 8 件编译失败，故弃用。

为什么走 pyximport 而不是 `lzcyc run`：`lzcyc` 不在根 workspace 成员里
（Cargo.toml:2），且它自带一份 postprocess_pyx 副本（CY/src/main.rs:264 一带）
与 lib 的 src/ir/codegen_cython.rs:3527 分叉。门禁要钉的是**主干 lib 的产物**，
所以这里直接吃 `lang-zone --backend=cython` 的输出。
"""
import asyncio
import contextlib
import importlib
import inspect
import io
import re
import subprocess
import sys
from pathlib import Path

BUILD_ONLY = "--build-only"


def _child_build(mod_dir: Path, stem: str) -> int:
    """另起一次自身调用做「编译 + 导入」，把链接器日志关在子进程的 stdout 里。

    编译产物缓存在 `~/.pyxbld/lib.win-amd64-cpython-<ver>/`（跨进程共享），
    所以父进程随后 import 时是缓存命中 ⇒ 本进程的 stdout 只剩程序输出。
    子进程的 stderr（Cython warning / 编译错误）转投本进程 stderr，证据不丢。
    """
    proc = subprocess.run(
        [sys.executable, str(Path(__file__).resolve()), str(mod_dir), stem, BUILD_ONLY],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        cwd=str(mod_dir),
    )
    if proc.stderr and proc.stderr.strip():
        sys.stderr.write(proc.stderr)
    return proc.returncode


def main() -> int:
    args = [a for a in sys.argv[1:] if a != BUILD_ONLY]
    build_only = BUILD_ONLY in sys.argv[1:]
    if len(args) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    mod_dir = Path(args[0]).resolve()
    stem = args[1]
    if not (mod_dir / f"{stem}.pyx").exists():
        print(f"missing {mod_dir / (stem + '.pyx')}", file=sys.stderr)
        return 2

    # 代码页归一化：Windows 默认 GBK 控制台/管道下 print("✅") 直接抛
    # UnicodeEncodeError，测的是终端不是产物。
    sys.stdout.reconfigure(encoding="utf-8", errors="strict")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

    import pyximport  # noqa: E402

    # 隔离工作树由调用方（Rust 测试）通过 cwd 提供，避免污染 CY/TESTS 下受版管的参考件。
    pyximport.install(language_level=3, setup_args={"script_args": ["--quiet"]})
    sys.path.insert(0, str(mod_dir))

    # ── 用**唯一别名**编译并导入产物字节完全相同的一份 .pyx ──
    # 为什么不直接 import 原名：pyximport 把自己的 finder 追加在 sys.meta_path 末尾，
    # 同名包先命中 ⇒ 实测 `E:\IDEProjects\AI\Cypy\test_suite`、stdlib 的 `enum`/`struct`、
    # site-packages 的 `coverage` 抢走了 4 个语料名 ⇒ 加载的是别人的模块，退出码仍 0（假绿）。
    # 而把 pyx finder 抢到位次 0 会连带打断 setuptools 的 `_distutils_hack`
    # （Python 3.13 无 stdlib distutils ⇒ 编译期 `import distutils._msvccompiler` 失败），
    # `sys.modules.pop(stem)` 又会把 stdlib 的 enum/struct 从解释器里抽掉。
    # 别名 `_lzcyg_<stem>` 谁也不可能预占，身份由构造保证；下面的指纹是第二道闸。
    alias = "_lzcyg_" + re.sub(r"[^0-9A-Za-z_]", "_", stem)
    src_pyx = mod_dir / f"{stem}.pyx"
    alias_pyx = mod_dir / f"{alias}.pyx"
    # 仅在内容变化时覆写：保 mtime ⇒ pyximport 缓存命中，否则每次全量重编（+8 min）
    if not alias_pyx.exists() or alias_pyx.read_bytes() != src_pyx.read_bytes():
        alias_pyx.write_bytes(src_pyx.read_bytes())

    noise = io.StringIO()
    # 产物指纹（不靠 `__file__`：生成的模块自己会把 `__file__` 覆写成 .lz 路径）
    # 断的是「加载到的模块确实是这份 .pyx 编出来的」
    pyx_src = src_pyx.read_text(encoding="utf-8", errors="replace")
    # 三种 def 形态都算「模块里有 main」：def / cdef / cpdef / **async def**
    # （漏了 async def ⇒ 15_feature_matrix 的两个 async 用例被误判成"身份不符"，
    #  那是指纹自己错，不是产物错）
    expects_main = bool(
        re.search(r"^(?:async\s+)?(?:cp|c)?def main\s*[(\[]", pyx_src, re.MULTILINE)
    )
    expects_moved = "class _Moved" in pyx_src
    try:
        if build_only:
            # 这一遍只为触发编译，程序顶层输出被吞（父进程会真跑一遍）
            with contextlib.redirect_stdout(noise):
                importlib.import_module(alias)
            return 0
        rc = _child_build(mod_dir, stem)
        if rc == 2:
            return 2
        mod = importlib.import_module(alias)
        # 身份校验（第二道闸）：别名不可能被预占，但指纹再确认一次加载到的
        # 确实是这份 .pyx —— 取生成器恒发的 `_Moved` 垫片 + `.pyx` 里有无 `def main`。
        # 两者与产物不符 ⇒ 判定作废（退出码 4）。
        wrong = (
            expects_main != callable(getattr(mod, "main", None))
            or expects_moved != hasattr(mod, "_Moved")
        )
        if wrong:
            print(
                f"模块身份不符：{stem}.pyx 指纹 (main={expects_main}, _Moved={expects_moved}) "
                f"与 import({alias}) 回来的模块 (main={callable(getattr(mod, 'main', None))}, "
                f"_Moved={hasattr(mod, '_Moved')}) 不一致 ⇒ 加载的不是本次产物"
                f"（loader={type(getattr(mod, '__loader__', None)).__name__}, "
                f"spec={getattr(mod, '__spec__', None)}）",
                file=sys.stderr,
            )
            return 4
        main_fn = getattr(mod, "main", None)
        if callable(main_fn):
            # `async def main()`（LZ 的 async 入口）直接调用只会**返回一个未运行的协程**
            # ⇒ stdout 空、退出码 0，静默假绿。协程结果交 asyncio.run 跑完再收。
            # 谓词必须是 isawaitable：Cython 3.2 的 async def 返回
            # `_cython_3_2_2.coroutine`，inspect.iscoroutine 判它 False（实测）。
            res = main_fn()
            if inspect.isawaitable(res):
                asyncio.run(res)
    except BaseException as e:  # noqa: BLE001 —— 门禁要看到任何异常
        if noise.getvalue().strip():
            sys.stderr.write(noise.getvalue())
        print(f"{type(e).__name__}: {e}", file=sys.stderr)
        return 3
    sys.stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/bin/bash
# 自动化差分：对 .lz 探针同时出 native/rs-lz，diff 后尝试编译 rs-lz 并跑。
# 用法: bash rssweep.sh <probe.lz>
# BUG-481：`--emit=rs-lz` 会把 <probe>.rs 覆盖成自托管库 dump（LZ library 编译出的
#   4126 行 Rust，与用户程序无关）——必须先快照 native .rs 再跑 rs-lz，否则 diff
#   恒为 4100+ 行的库 diff，掩盖真实差异。
set -u
PROBE="$1"
BIN=./target/debug/lang-zone.exe
BASE="${PROBE%.lz}"
DIR="$(dirname "$PROBE")"
rm -f "$BASE".rs "$BASE"_native.rs "$BASE".exe "$BASE".pdb "$BASE".lzlz "$BASE".lzrs "$BASE"_lz.rs "$BASE"_lz.exe

# native（默认 emit 写出 <probe>.rs）——先快照
"$BIN" "$PROBE" 2>/dev/null | grep -v "^Generated"
cp "$BASE.rs" "$BASE"_native.rs
# rs-lz 全量 Rust 在 stdout（过滤 Generated 行）；<probe>.lzrs 是自托管库 dump，勿用
"$BIN" "$PROBE" --emit=rs-lz 2>/dev/null | grep -v "^Generated" > "$BASE"_lz.rs

# diff（rs-lz 含 recipe 头与库自身声明，与 native 前置不同 → 只看 COMPILE/RUN）
if diff -q "$BASE"_native.rs "$BASE"_lz.rs >/dev/null 2>&1; then
    echo "[DIFF] 完全相同"
else
    echo "[DIFF] 存在差异 ($(diff "$BASE"_native.rs "$BASE"_lz.rs | grep -c '^[<>]') 行)"
    diff "$BASE"_native.rs "$BASE"_lz.rs | head -30
fi

# compile rs-lz
if rustc --edition 2021 "$BASE"_lz.rs --extern lz_builtins=target/debug/liblz_builtins.rlib -L dependency=target/debug/deps -o "$BASE"_lz.exe >/dev/null 2>&1; then
    echo "[COMPILE] rs-lz 编译通过"
    OUT=$("$BASE"_lz.exe 2>&1)
    echo "[RUN] 输出: $OUT"
    # native 对照（用快照）
    if rustc --edition 2021 "$BASE"_native.rs --extern lz_builtins=target/debug/liblz_builtins.rlib -L dependency=target/debug/deps -o "$BASE".exe >/dev/null 2>&1; then
        NOUT=$("$BASE".exe 2>&1)
        if [ "$OUT" = "$NOUT" ]; then
            echo "[RUN] 与 native 输出一致"
        else
            echo "[RUN] 输出不一致: rs-lz=[$OUT] native=[$NOUT]"
        fi
    fi
else
    echo "[COMPILE] rs-lz 编译失败:"
    rustc --edition 2021 "$BASE"_lz.rs --extern lz_builtins=target/debug/liblz_builtins.rlib -L dependency=target/debug/deps -o "$BASE"_lz.exe 2>&1 | grep -E "^error" | head -5
fi
#!/bin/bash
# Test all FIND_BUG .lz files
# Output: file, stage (lz/success), result (ok/error), details
# 2026-09-03 修正（首轮实测三教训）：
#   1. rustc 段必须 --extern lz_builtins（否则假性 E0432）
#   2. -o /dev/null 在 Windows bash 下不可写 → 用临时产物目录
#   3. cargo run 每文件重编译开销大 → 用 target/release/lang-zone.exe（需先 cargo build --release）
# 基线参考：FIND_BUG.md「实测记录（2026-09-03）」

set -u
LZC="./target/release/lang-zone.exe"
RESULTS="FIND_BUG/test_results.txt"
WORK=$(mktemp -d)

if [ ! -x "$LZC" ]; then
    echo "lang-zone.exe 不存在，先 cargo build --release" >&2
    exit 1
fi

BUILTINS="target/debug/liblz_builtins.rlib"
if [ ! -f "$BUILTINS" ]; then
    cargo build --package lz_builtins >/dev/null 2>&1
fi

> "$RESULTS"

for f in $(find FIND_BUG -name "*.lz" -not -path "*/debug/*" | sort); do
    echo -n "Testing: $f ... "

    # 1) LZ 编译段
    OUTPUT=$("$LZC" "$f" 2>&1)
    EXIT_CODE=$?
    if [ $EXIT_CODE -ne 0 ]; then
        ERROR_MSG=$(echo "$OUTPUT" | head -1)
        echo "LZ_ERROR: $ERROR_MSG" | tee -a "$RESULTS"
        continue
    fi

    # 2) 产物检查段
    RS_FILE="${f%.lz}.rs"
    if [ ! -f "$RS_FILE" ]; then
        echo "LZ_ERROR: no .rs generated" | tee -a "$RESULTS"
        continue
    fi

    # 3) rustc 段（带 rlib）
    RUSTC_OUTPUT=$(rustc --edition 2021 --extern lz_builtins="$BUILTINS" -A warnings "$RS_FILE" -o "$WORK/$(basename "${f%.lz}").exe" 2>&1)
    RUSTC_EXIT=$?
    if [ $RUSTC_EXIT -eq 0 ]; then
        echo "OK" | tee -a "$RESULTS"
    else
        FIRST_ERROR=$(echo "$RUSTC_OUTPUT" | grep "^error" | head -1)
        echo "RUSTC_ERROR: $FIRST_ERROR" | tee -a "$RESULTS"
    fi
done

rm -rf "$WORK"
echo ""
echo "=== SUMMARY ==="
echo "Total files: $(find FIND_BUG -name "*.lz" -not -path "*/debug/*" | wc -l)"
echo "LZ errors: $(grep -c "LZ_ERROR" "$RESULTS" 2>/dev/null || echo 0)"
echo "Rustc errors: $(grep -c "RUSTC_ERROR" "$RESULTS" 2>/dev/null || echo 0)"
echo "OK: $(grep -c "^OK$" "$RESULTS" 2>/dev/null || echo 0)"

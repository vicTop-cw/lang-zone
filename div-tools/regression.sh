#!/usr/bin/env bash
# ============================
# LZ 全量回归脚本（IR 唯一路线）
# 用法: bash div-tools/regression.sh [--verbose]
# 对 DEMO 下所有 .lz（排除 99_errors）: lang-zone.exe → .rs → rustc → 运行
# ============================
set -uo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
EXE="$ROOT/target/debug/lang-zone.exe"
RLIB="$ROOT/target/debug/liblz_builtins.rlib"
DEMO_DIR="$ROOT/DEMO"
VERBOSE=false
[[ "${1:-}" == "--verbose" ]] && VERBOSE=true

PASS=0; FAIL=0; PARSE_FAIL=0; RUSTC_FAIL=0; RUN_FAIL=0
FAIL_LIST=()
LOG="$ROOT/div-tools/regression-progress.txt"
: > "$LOG"

run_one() {
    local lz="$1" rel="${1#$DEMO_DIR/}"
    local rs="${lz%.lz}.rs" exe="${lz%.lz}.exe"
    local out err rc

    # Step 1: lz -> rs
    err=$("$EXE" "$lz" 2>&1)
    rc=$?
    if [[ $rc -ne 0 ]]; then
        PARSE_FAIL=$((PARSE_FAIL+1)); FAIL=$((FAIL+1))
        local msg
        msg=$(echo "$err" | grep -oE "error(\[E[0-9]+\])?: [^|]*" | head -1)
        FAIL_LIST+=("PARSE|$rel|${msg:-$rc}")
        return
    fi
    if [[ ! -f "$rs" ]]; then
        PARSE_FAIL=$((PARSE_FAIL+1)); FAIL=$((FAIL+1))
        FAIL_LIST+=("PARSE|$rel|no .rs generated")
        return
    fi

    # Step 2: rustc
    err=$(rustc --edition 2021 --extern lz_builtins="$RLIB" "$rs" -o "$exe" 2>&1)
    rc=$?
    if [[ $rc -ne 0 ]]; then
        RUSTC_FAIL=$((RUSTC_FAIL+1)); FAIL=$((FAIL+1))
        local msg2
        msg2=$(echo "$err" | grep -oE "error(\[E[0-9]+\])?: [^|]*" | head -1)
        [[ -z "$msg2" ]] && msg2=$(echo "$err" | grep -oE "^error: [^|]*" | head -1)
        FAIL_LIST+=("RUSTC|$rel|${msg2:-$rc}")
        return
    fi

    # Step 3: run
    err=$("$exe" 2>&1)
    rc=$?
    if [[ $rc -ne 0 ]]; then
        RUN_FAIL=$((RUN_FAIL+1)); FAIL=$((FAIL+1))
        FAIL_LIST+=("RUN|$rel|$(echo "$err" | head -1)")
        echo "RUN|$rel|$(echo "$err" | head -1)" >> "$LOG"
        return
    fi
    PASS=$((PASS+1))
    $VERBOSE && echo "  ✅ $rel"
}

count=0
while IFS= read -r -d '' f; do
    count=$((count+1))
    run_one "$f"
done < <(find "$DEMO_DIR" -name "*.lz" -not -path "*/99_errors/*" -print0)

echo "TOTAL_PASS=$PASS TOTAL_FAIL=$FAIL PARSE=$PARSE_FAIL RUSTC=$RUSTC_FAIL RUN=$RUN_FAIL" >> "$LOG"
echo ""
echo "=========================================="
echo "  LZ 全量回归报告  $(date +'%Y-%m-%d %H:%M')"
echo "=========================================="
echo "  测试文件: $count"
echo "  PASS: $PASS   FAIL: $FAIL"
echo "  (PARSE $PARSE_FAIL / RUSTC $RUSTC_FAIL / RUN $RUN_FAIL)"
echo ""
if [[ $FAIL -gt 0 ]]; then
    echo "--- 失败清单 ---"
    printf '%s\n' "${FAIL_LIST[@]}"
fi
echo "=========================================="

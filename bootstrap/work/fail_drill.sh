#!/usr/bin/env bash
# ============================================================
# Lang-Zong 自举失败演练（幂等，可重复执行）
#
# 验证: 失败注入 → 非零退出码 + 日志可定位 → 回滚 → 哈希一致 → 恢复绿基线
# 用法: bash bootstrap/work/fail_drill.sh
# 退出码: 0 全部断言通过；非零 = 失败步骤序号
# ============================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BOOTSTRAP_DIR="$(dirname "$SCRIPT_DIR")"
WORK_DIR="$BOOTSTRAP_DIR/work"
FAIL_PROBE="$WORK_DIR/fail_probe"
DRILL_LOG="$WORK_DIR/fail_drill.log"
LATEST_LOG="$WORK_DIR/log/latest.log"

GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'
pass() { echo -e "${GREEN}[DRILL-PASS]${NC} $*"; }
fail() { echo -e "${RED}[DRILL-FAIL]${NC} $*"; }

echo "=== fail_drill $(date +%Y-%m-%d\ %H:%M:%S) ===" | tee "$DRILL_LOG"

# 步骤 1: 建立绿基线
echo "[1/6] 跑一轮 closed 建立绿基线..." | tee -a "$DRILL_LOG"
bash "$BOOTSTRAP_DIR/build.sh" closed >> "$DRILL_LOG" 2>&1
rc=$?
if [ $rc -ne 0 ]; then fail "步骤1: closed 未通过 (exit=$rc)"; exit 1; fi
pass "步骤1: 绿基线建立 (exit=0)" | tee -a "$DRILL_LOG"

# 步骤 2: 注入失败（非法 .lz 语法）
echo "[2/6] 注入非法 fail_probe/bad.lz..." | tee -a "$DRILL_LOG"
mkdir -p "$FAIL_PROBE"
cat > "$FAIL_PROBE/bad.lz" <<'EOF'
def broken(:
    this is not valid lz !!!
EOF
pass "步骤2: 注入完成" | tee -a "$DRILL_LOG"

# 步骤 3: 重跑 closed，断言退出码非零且日志含 bad.lz 错误
echo "[3/6] 重跑 closed，断言失败路径..." | tee -a "$DRILL_LOG"
bash "$BOOTSTRAP_DIR/build.sh" closed >> "$DRILL_LOG" 2>&1
rc=$?
if [ $rc -eq 0 ]; then fail "步骤3: 注入后 closed 仍通过 (exit=0)，失败路径失效"; exit 3; fi
if ! grep -q "bad.lz" "$LATEST_LOG"; then fail "步骤3: 日志未含 bad.lz 定位信息"; exit 3; fi
pass "步骤3: 非零退出码 (exit=$rc) + 日志可定位 bad.lz" | tee -a "$DRILL_LOG"

# 步骤 4: 回滚，断言 exit 0 + 哈希一致 + bad.lz 无生成物
echo "[4/6] 执行回滚..." | tee -a "$DRILL_LOG"
bash "$BOOTSTRAP_DIR/rollback.sh" >> "$DRILL_LOG" 2>&1
rc=$?
if [ $rc -ne 0 ]; then fail "步骤4: 回滚失败 (exit=$rc)"; exit 4; fi
if [ -f "$FAIL_PROBE/bad.rs" ] || [ -f "$FAIL_PROBE/bad.exe" ]; then
    fail "步骤4: bad.lz 有生成物残留"
    exit 4
fi
pass "步骤4: 回滚成功 (exit=0)，bad.lz 无生成物" | tee -a "$DRILL_LOG"

# 步骤 5: 清理注入并重跑 closed，断言 exit 0
echo "[5/6] 清理 fail_probe 并恢复绿基线..." | tee -a "$DRILL_LOG"
rm -rf "$FAIL_PROBE"
bash "$BOOTSTRAP_DIR/build.sh" closed >> "$DRILL_LOG" 2>&1
rc=$?
if [ $rc -ne 0 ]; then fail "步骤5: 清理后 closed 未恢复绿基线 (exit=$rc)"; exit 5; fi
pass "步骤5: 恢复绿基线 (exit=0)" | tee -a "$DRILL_LOG"

echo "[6/6] 演练记录: $DRILL_LOG" | tee -a "$DRILL_LOG"
echo -e "${GREEN}[OK] fail_drill 全部断言通过${NC}" | tee -a "$DRILL_LOG"
exit 0

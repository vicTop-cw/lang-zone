#!/usr/bin/env bash
# ============================================================
# Lang-Zong 自举回滚脚本（git-bash 入口）
# 恢复到上一个可用构建（bootstrap/work/backup/good/）
#
# 用法: bash bootstrap/rollback.sh
# 退出码: 0 回滚成功；2 无回滚基线；3 恢复后哈希校验失败
# 安全: 不使用 rm -rf / git 破坏性命令；当前产物仅移动（保留现场）
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="$SCRIPT_DIR/work"
BACKUP_GOOD="$WORK_DIR/backup/good"

if [ ! -f "$BACKUP_GOOD/manifest.sha256" ]; then
    echo "[FAIL] 无回滚基线: $BACKUP_GOOD/manifest.sha256 不存在（先跑一轮 closed 建立基线）"
    exit 2
fi

TS="$(date +%Y%m%d-%H%M%S)"
BAD_DIR="$WORK_DIR/backup/bad-$TS"

# 1. 当前产物移入 bad-<时间戳>/（保留现场）
mkdir -p "$BAD_DIR"
cd "$WORK_DIR"
while IFS= read -r -d '' f; do
    rel="${f#./}"
    mkdir -p "$BAD_DIR/$(dirname "$rel")"
    mv "$f" "$BAD_DIR/$rel"
done < <(find . -type f \( -name "*.rs" -o -name "*.exe" \) -not -path "./backup/*" -not -path "./manifest/*" -not -path "./log/*" -print0)

# 2. 从 backup/good 恢复
while IFS= read -r -d '' f; do
    rel="${f#./}"
    mkdir -p "$WORK_DIR/$(dirname "$rel")"
    cp "$f" "$WORK_DIR/$rel"
done < <(cd "$BACKUP_GOOD" && find . -type f \( -name "*.rs" -o -name "*.exe" \) -print0)

# 3. 哈希校验（仅 .rs，与基线 manifest 口径一致；.exe 不参与哈希比对）
EXPECTED="$BACKUP_GOOD/manifest.sha256"
ACTUAL="$WORK_DIR/manifest/rollback_check.sha256"
mkdir -p "$WORK_DIR/manifest"
: > "$ACTUAL"
while IFS= read -r -d '' f; do
    rel="${f#./}"
    sha256sum "$f" | awk -v rel="$rel" '{print $1"  "rel}'
done < <(find . -type f -name "*.rs" -not -path "./backup/*" -not -path "./manifest/*" -not -path "./log/*" -not -path "./runout/*" -print0) >> "$ACTUAL"
sort -k2 "$ACTUAL" -o "$ACTUAL"

if diff -q <(sort -k2 "$EXPECTED") "$ACTUAL" > /dev/null; then
    echo "[OK] 回滚成功：产物已恢复并与基线 manifest 一致"
    echo "     被替换产物保留在: $BAD_DIR"
    exit 0
else
    echo "[FAIL] 恢复后哈希校验不一致（基线 manifest vs 实际产物）"
    echo "     现场保留在: $BAD_DIR"
    diff <(sort -k2 "$EXPECTED") "$ACTUAL" | head -20
    exit 3
fi

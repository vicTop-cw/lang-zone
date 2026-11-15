#!/usr/bin/env bash
# 驱动 lz 模块导入 / 包构建 / 三方库 语料（C01–C06）
#
# 设计纪律（承接 workbuddy/plan/2026-09-14-moddec-PRD.md 的「测试语料先行」原则）：
#   每个用例必须走完转译 → rustc → **实际运行**，并校验 stdout 与退出码。
#   「能编译」不算通过 —— 这正是现有 demo_codegen_compile 闸门的盲区
#   （其 enum Stage 只有 Transpile / Rustc，从不执行产物）。
#
# 用例状态：
#   C01 本地同目录 import      期望 PASS（已实测通过）
#   C02 子目录模块 import       期望 PASS/FAIL —— 待测
#   C03 std 库 import（对照组） 期望 PASS
#   C04 包内多文件布局+兄弟模块  期望 PASS
#   C05 三方库（registry）import 期望 FAIL（Manifest 无 dependencies 字段）
#   C06 依赖版本冲突            期望 FAIL（当前无法表达）
#
# 用法： bash tests/corpus/import_pkg/run.sh
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
LZC="$ROOT/target/debug/lang-zone.exe"
CORPUS="$ROOT/tests/corpus/import_pkg"
WORK="$ROOT/TEMP/import-pkg-run"

RLIB="$(ls -t "$ROOT"/target/debug/deps/liblz_builtins-*.rlib 2>/dev/null | head -1)"
DEPS="$(dirname "$RLIB")"

if [ ! -x "$LZC" ]; then
  echo "FATAL: 找不到 lzc 可执行文件：$LZC（先跑 cargo build --bin lang-zone）"
  exit 2
fi
if [ -z "${RLIB:-}" ]; then
  echo "FATAL: 找不到 lz_builtins rlib"
  exit 2
fi

rm -rf "$WORK"; mkdir -p "$WORK"

pass=0; fail=0; declare -a FAILED

# run_case <case-dir> <entry.lz 相对路径> <期望: PASS|FAIL> <说明>
run_case() {
  local casedir="$1" entry="$2" expect="$3" desc="$4"
  local name; name="$(basename "$casedir")"
  local wd="$WORK/$name"
  mkdir -p "$wd"
  # 把整个用例目录复制到工作区（保持相对结构）
  cp -r "$CORPUS/$casedir/." "$wd/" 2>/dev/null

  local stage="RUN_OK" detail=""
  ( cd "$wd" && "$LZC" "$entry" >trans.log 2>&1 ) || { stage="TRANSPILE_FAIL"; detail="$(grep -iE 'error|错误' "$wd/trans.log" | head -1)"; }
  if [ "$stage" = "RUN_OK" ]; then
    local rs; rs="$wd/${entry%.lz}.rs"
    if [ ! -f "$rs" ]; then stage="NO_RS_OUTPUT"; detail="未产出 ${entry%.lz}.rs"; fi
  fi
  if [ "$stage" = "RUN_OK" ]; then
    ( cd "$wd" && rustc --edition 2021 --extern lz_builtins="$RLIB" -L dependency="$DEPS" \
        -A warnings -o case.exe "${entry%.lz}.rs" >rc.log 2>&1 ) \
      || { stage="RUSTC_FAIL"; detail="$(grep -E '^error' "$wd/rc.log" | head -1)"; }
  fi
  if [ "$stage" = "RUN_OK" ]; then
    ( cd "$wd" && ./case.exe >run.log 2>&1 )
    local rc=$?
    if [ $rc -ne 0 ]; then
      stage="RUN_FAIL"; detail="exit=$rc $(tail -1 "$wd/run.log" 2>/dev/null | cut -c1-60)"
    elif ! grep -q "OK" "$wd/run.log" 2>/dev/null; then
      stage="RUN_FAIL"; detail="退出码 0 但 stdout 无 OK 标记: $(tail -1 "$wd/run.log" | cut -c1-60)"
    else
      detail="$(tail -1 "$wd/run.log" | cut -c1-60)"
    fi
  fi

  local verdict="PASS"
  if [ "$expect" = "PASS" ] && [ "$stage" != "RUN_OK" ]; then verdict="FAIL"; fi
  if [ "$expect" = "FAIL" ] && [ "$stage" = "RUN_OK" ]; then verdict="FAIL(意外成功)"; fi

  if [ "$verdict" = "PASS" ]; then
    pass=$((pass+1)); printf '  [ok]   %-22s %-8s %s\n' "$name" "$stage" "$detail"
  else
    fail=$((fail+1)); FAILED+=("$name")
    printf '  [FAIL] %-22s %-8s %s\n' "$name" "$stage" "$detail"
  fi
  printf '         └ %s (期望 %s)\n' "$desc" "$expect"
}

echo "=== lz 模块导入 / 包构建 / 三方库 语料 ==="
echo "工作区：$WORK"
echo

run_case c01_local_relative "main.lz"  PASS  "本地同目录 import + fn/struct 跨文件可见"
run_case c02_subdir_module  "main.lz"  PASS  "子目录模块 import（mathx.util）"
run_case c03_std_import     "main.lz"  PASS  "std 库 import（对照组）"
run_case c04_pkg_layout     "src/main.lz" PASS "包内多文件布局 + 兄弟模块 import"
run_case c05_thirdparty     "main.lz"  FAIL  "三方库 import（当前无依赖声明，预期失败）"
run_case c06_version_conflict "main.lz" FAIL  "依赖版本冲突（当前无法表达，预期失败）"

echo
echo "=== 汇总 ==="
echo "符合预期: $pass / $((pass+fail))"
if [ "$fail" -gt 0 ]; then
  echo "不符合预期: ${FAILED[*]}"
  echo "（注：不符合预期 = 该修的没通，或该失败的反而成功了 —— 两种都要查）"
fi
[ "$fail" -eq 0 ] && echo "全部符合当前能力预期。"
exit 0
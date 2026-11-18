#!/usr/bin/env bash
# ============================================================
# Lang-Zong 自举（Bootstrap）构建脚本
# ============================================================
# 流程：.lz → lang-zone.exe → .rs → rustc → .exe → 运行验证
#
# 用法:
#   ./build.sh all           # 编译所有测试（bootstrap/work/*.lz）
#   ./build.sh closed        # 自举闭环：两代构建 + 一致性校验（降级口径）
#   ./build.sh closed --generations 3   # 三代自举收敛（M5 Lane D 验收口径）
#   ./build.sh closed --compiler lzcyc  # 闭环入口换成 lzcyc（--backend=rust）
#   ./build.sh path/to/file  # 编译单个 .lz 文件
#   ./build.sh clean         # 清理构建产物
#
# 入口/工具链参数（与 build.ps1 同口径）：
#   --compiler <lang-zone|lzcyc|任意路径>   --lzc <显式路径>（优先）
#   --rlib <lz_builtins.rlib>               --std <标准库目录>
#   --rustc <rustc 命令>（须与 rlib 同版本，否则 E0514）
#   --generations <N>    closed 模式构建代数（默认 2）
#
# 退出码（closed 模式）：
#   0 闭环全通过；1 lzc 失败；2 rustc 失败；3 运行失败；4 一致性校验失败；5 环境缺失
#
# 一致性口径（2026-08-16 首轮实测冻结，见 05-自举里程碑台账 §5）：
#   .rs 产物清单+SHA256 两轮一致 && .exe 运行输出两轮一致。
#   .exe 二进制哈希不参与比对——rustc 默认嵌入构建元数据，同源两次编译
#   二进制不同（实测首轮 .rs 全一致、.exe 全不一致），行为等价以运行输出为准。
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WORK_DIR="$SCRIPT_DIR/work"
MANIFEST_DIR="$WORK_DIR/manifest"
LOG_DIR="$WORK_DIR/log"
RUNOUT_DIR="$WORK_DIR/runout"
BACKUP_GOOD="$WORK_DIR/backup/good"

# ── 入口/工具链参数解析（M5 Lane D：与 build.ps1 同口径）──
GENERATIONS="${GENERATIONS:-2}"
RUSTC="${RUSTC:-rustc}"
COMPILER=""
LZC_ARG=""
RLIB_ARG=""
STD_ARG=""
POS_ARGS=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --compiler)    COMPILER="$2"; shift 2 ;;
        --lzc)         LZC_ARG="$2"; shift 2 ;;
        --rlib)        RLIB_ARG="$2"; shift 2 ;;
        --std)         STD_ARG="$2"; shift 2 ;;
        --rustc)       RUSTC="$2"; shift 2 ;;
        --generations) GENERATIONS="$2"; shift 2 ;;
        *)             POS_ARGS+=("$1"); shift ;;
    esac
done
if [ ${#POS_ARGS[@]} -gt 0 ]; then set -- "${POS_ARGS[@]}"; else set -- "all"; fi

if [ -n "$LZC_ARG" ]; then
    LZC="$LZC_ARG"
elif [[ "$COMPILER" == *lzcyc* ]]; then
    LZC="$PROJECT_DIR/CY/target/debug/lzcyc.exe"
else
    LZC="$PROJECT_DIR/target/debug/lang-zone.exe"
fi
STD_DIR="${STD_ARG:-$PROJECT_DIR/std}"
if [ -n "$RLIB_ARG" ]; then
    BUILTINS_RLIB="$RLIB_ARG"
elif [ -f "$PROJECT_DIR/target/lane-bootstrap/debug/liblz_builtins.rlib" ]; then
    # 车道 rlib 优先（钉版工具链构建，规避 E0514 跨版本错配）
    BUILTINS_RLIB="$PROJECT_DIR/target/lane-bootstrap/debug/liblz_builtins.rlib"
else
    BUILTINS_RLIB="$PROJECT_DIR/target/debug/liblz_builtins.rlib"
fi
IS_LZCYC=0
if [[ "$(basename "$LZC")" == *lzcyc* ]]; then IS_LZCYC=1; fi

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

log_pass()  { echo -e "${GREEN}[PASS]${NC} $*"; }
log_fail()  { echo -e "${RED}[FAIL]${NC} $*"; }
log_info()  { echo -e "${YELLOW}[INFO]${NC} $*"; }
log_step()  { echo -e "\n${YELLOW}>>> $*${NC}"; }

# 将 Git Bash /e/... 路径转为 Windows E:\... 格式（lang-zone.exe 需要 Windows 路径）
to_win_path() {
    local p="$1"
    # WSL 路径：/mnt/f/AI/x → F:\AI\x
    if [[ "$p" =~ ^/mnt/([a-zA-Z])/(.*)$ ]]; then
        local wdrive="${BASH_REMATCH[1]}"
        local wrest="${BASH_REMATCH[2]}"
        echo "${wdrive^^}:\\${wrest//\//\\}"
        return
    fi
    if [[ "$p" == /* && "${p:1:1}" != "/" ]]; then
        local drive="${p:1:1}"
        local rest="${p:2}"
        echo "${drive^^}:${rest//\//\\}"
    else
        echo "$p"
    fi
}

# 让本进程与子进程里的裸 rustc 与 --rustc 同源（防 lzcyc/库内嵌调用
# 走异版本 rustc → E0514；WSL 下 /mnt/<d>/ 与 Windows <D>:\ 互转）
if [ "$RUSTC" != "rustc" ]; then
    rustc_dir=""
    if [[ "$RUSTC" == /* ]]; then
        rustc_dir="$(dirname "$RUSTC")"
    elif [[ "$RUSTC" == [A-Za-z]:* ]]; then
        rustc_dir="$(dirname "$(to_win_path "$RUSTC")")"
    fi
    if [ -n "$rustc_dir" ] && [ -d "$rustc_dir" ]; then
        export PATH="$rustc_dir:$PATH"
    fi
fi
# ============================================================
# 编译单个 .lz → .rs → .exe → 运行
# 返回：0 成功；1 lzc 失败；2 rustc 失败；3 运行失败
# ============================================================
compile_and_run() {
    local lz_file="$1"
    # 相对路径转绝对（Windows 侧编译器/exe 需要绝对路径）
    if [[ "$lz_file" != /* && "$lz_file" != [A-Za-z]:* ]]; then
        lz_file="$PWD/$lz_file"
    fi
    local base="$(basename "$lz_file" .lz)"
    local dir="$(dirname "$lz_file")"
    local rs_file="$dir/$base.rs"
    local exe_file="$dir/$base.exe"
    local win_lz="$(to_win_path "$lz_file")"
    local win_std="$(to_win_path "$STD_DIR")"
    local win_rlib="$(to_win_path "$BUILTINS_RLIB")"
    local win_rs="$(to_win_path "$rs_file")"
    local win_exe="$(to_win_path "$exe_file")"
    local win_deps="$(to_win_path "$(dirname "$BUILTINS_RLIB")/deps")"

    log_step "编译: $base"

    # Step 1: LZ → Rust
    log_info "  [LZC] LZ → Rust..."
    if [ "$IS_LZCYC" -eq 1 ]; then
        # lzcyc 入口：compile 子命令 + Rust 后端；透传 rustc/rlib 选择（Lane D 参数化）
        if ! LZ_RUSTC="$(to_win_path "$RUSTC")" LZ_BUILTINS_RLIB="$win_rlib" \
             "$LZC" compile "$win_lz" --backend=rust --std-dir "$win_std" 2>&1 < /dev/null; then
            log_fail "  [LZC] LZ 编译失败: $lz_file"
            return 1
        fi
    else
        if ! "$LZC" "$win_lz" --std-dir "$win_std" 2>&1 < /dev/null; then
            log_fail "  [LZC] LZ 编译失败: $lz_file"
            return 1
        fi
    fi
    log_pass "  [LZC] LZ → $rs_file"

    # Step 2: Rust → Binary（链接 lz_builtins，codegen 恒生成 use lz_builtins::*）
    log_info "  [RUSTC] Rust → Binary..."
    local rustc_output
    rustc_output=$("$RUSTC" --edition 2021 "$win_rs" --extern "lz_builtins=$win_rlib" -L "dependency=$win_deps" -o "$win_exe" 2>&1 < /dev/null) || {
        log_fail "  [RUSTC] rustc 编译失败: $lz_file"
        echo "$rustc_output" | grep -E "error\[" | head -5
        return 2
    }
    log_pass "  [RUSTC] rustc → $exe_file"

    # Step 3: 运行验证
    log_info "  [RUN] 运行..."
    local run_output
    if run_output=$("$exe_file" 2>&1 < /dev/null); then
        log_pass "  [RUN] 运行成功: [$run_output]"
        echo ""
    else
        log_fail "  [RUN] 运行失败 (exit=$?): $run_output"
        return 3
    fi

    return 0
}

# ============================================================
# 编译所有测试 (bootstrap/work/*.lz)，单轮
# ============================================================
build_all() {
    log_step "=== Lang-Zong 自举构建 ==="
    log_info "编译器: $LZC"
    log_info "标准库: $STD_DIR"

    local total=0
    local passed=0
    local failed=()
    local skipped=0

    cd "$PROJECT_DIR"

    while IFS= read -r -d '' lz_file; do
        if [[ "$lz_file" == *"/std/"* ]]; then
            skipped=$((skipped + 1))
            continue
        fi
        total=$((total + 1))
        if compile_and_run "$lz_file"; then
            passed=$((passed + 1))
        else
            failed+=("$lz_file")
        fi
    done < <(find bootstrap/work -name "*.lz" -not -path "*/backup/*" -not -path "*/fail_probe/*" -print0)

    echo ""
    echo "=========================================="
    echo -e "${GREEN}通过: $passed/$total${NC}"
    if [ ${#failed[@]} -gt 0 ]; then
        echo -e "${RED}失败:${NC}"
        for f in "${failed[@]}"; do
            echo "  - $f"
        done
    fi
    if [ $skipped -gt 0 ]; then
        log_info "跳过 (std): $skipped"
    fi
    echo "=========================================="

    [ ${#failed[@]} -eq 0 ]
}

# ============================================================
# 生成产物 manifest：work 下全部 .rs 的 SHA256
# （.exe 二进制哈希受 rustc 非确定性影响，不参与比对，见文件头口径说明）
# ============================================================
make_manifest() {
    local out="$1"
    : > "$out"
    cd "$WORK_DIR"
    while IFS= read -r -d '' f; do
        local rel="${f#./}"
        sha256sum "$f" | awk -v rel="$rel" '{print $1"  "rel}'
    done < <(find . -type f -name "*.rs" -not -path "./backup/*" -not -path "./manifest/*" -not -path "./log/*" -not -path "./runout/*" -print0) >> "$out"
    sort -k2 "$out" -o "$out"
}

# 记录全部 .exe 运行输出（行为一致性口径）
record_runout() {
    local out="$1"
    mkdir -p "$out"
    cd "$WORK_DIR"
    while IFS= read -r -d '' f; do
        local base="$(basename "$f" .exe)"
        "$PWD/${f#./}" > "$out/$base.out" 2>&1 < /dev/null
    done < <(find . -type f -name "*.exe" -not -path "./backup/*" -not -path "./manifest/*" -not -path "./log/*" -not -path "./runout/*" -print0)
}

# 两轮 runout 输出一致性比对
diff_runout() {
    local a="$1"
    local b="$2"
    local ok=true
    while IFS= read -r -d '' f; do
        local rel="${f#$a/}"
        if ! diff -q "$f" "$b/$rel" > /dev/null; then
            echo "  运行输出不一致: $rel"
            ok=false
        fi
    done < <(find "$a" -type f -print0)
    if [ "$ok" = true ]; then return 0; else return 1; fi
}

# ============================================================
# 自举闭环：两轮构建 + 一致性校验 + PROMOTE
# ============================================================
build_closed() {
    # 环境检查（退出码 5）
    if [ ! -f "$LZC" ]; then
        log_fail "环境缺失: $LZC（先 cargo build）"
        return 5
    fi
    if [ ! -f "$BUILTINS_RLIB" ]; then
        log_fail "环境缺失: $BUILTINS_RLIB（先 cargo build -p lz_builtins）"
        return 5
    fi
    mkdir -p "$MANIFEST_DIR" "$LOG_DIR" "$BACKUP_GOOD"

    local ts="$(date +%Y%m%d-%H%M%S)"
    local log_file="$LOG_DIR/$ts.log"
    ln -sf "$log_file" "$LOG_DIR/latest.log"

    {
        echo "=== Lang-Zong 自举闭环 ==="
        echo "时间: $ts"
        echo "编译器: $LZC"
        echo ""
        # ── N 代构建：每代清产物 → 全量 → manifest + runout ──
        if [ "$GENERATIONS" -lt 1 ]; then GENERATIONS=1; fi
        g=1
        while [ "$g" -le "$GENERATIONS" ]; do
            if [ "$g" -gt 1 ]; then
                log_info "[GEN $g] 清理上一代产物 (.rs/.exe) ..."
                find "$WORK_DIR" -name "*.rs" -not -path "*/backup/*" -not -path "*/runout/*" -delete
                find "$WORK_DIR" -name "*.exe" -not -path "*/backup/*" -not -path "*/runout/*" -delete
            fi
            log_step "第 $g 代构建"
            if ! build_all; then
                log_fail "第 $g 代构建失败（见上方阶段日志定位）"
                return 1
            fi
            make_manifest "$MANIFEST_DIR/gen$g.sha256"
            log_info "第 $g 代 manifest: $MANIFEST_DIR/gen$g.sha256 ($(wc -l < "$MANIFEST_DIR/gen$g.sha256") 项)"
            record_runout "$RUNOUT_DIR/gen$g"
            g=$((g + 1))
        done

        # ── 收敛校验：全部 manifest 与 gen1 一致 + 运行输出一致 ──
        log_info "[VERIFY] $GENERATIONS 代 .rs manifest 一致性校验..."
        g=2
        while [ "$g" -le "$GENERATIONS" ]; do
            if ! diff -q "$MANIFEST_DIR/gen1.sha256" "$MANIFEST_DIR/gen$g.sha256" > /dev/null; then
                log_fail "第 1 代与第 $g 代 .rs manifest 不一致（非确定性构建 / 未收敛）"
                diff "$MANIFEST_DIR/gen1.sha256" "$MANIFEST_DIR/gen$g.sha256" | head -20
                return 4
            fi
            log_pass "[VERIFY] gen1 与 gen$g .rs manifest 完全一致"
            log_info "[VERIFY] gen1 与 gen$g .exe 运行输出一致性校验..."
            if ! diff_runout "$RUNOUT_DIR/gen1" "$RUNOUT_DIR/gen$g"; then
                log_fail "gen1 与 gen$g .exe 运行输出不一致"
                return 4
            fi
            log_pass "[VERIFY] gen1 与 gen$g 运行输出完全一致"
            g=$((g + 1))
        done

        # PROMOTE：更新回滚基线（末代 manifest）
        log_info "[PROMOTE] 更新回滚基线 backup/good ..."
        rm -f "$BACKUP_GOOD"/*.rs "$BACKUP_GOOD"/*.exe
        cp "$MANIFEST_DIR/gen$GENERATIONS.sha256" "$BACKUP_GOOD/manifest.sha256"
        while IFS= read -r -d '' f; do
            local rel="${f#./}"
            mkdir -p "$BACKUP_GOOD/$(dirname "$rel")"
            cp "$f" "$BACKUP_GOOD/$rel"
        done < <(find . -type f \( -name "*.rs" -o -name "*.exe" \) -not -path "./backup/*" -not -path "./manifest/*" -not -path "./log/*" -not -path "./runout/*" -print0)
        log_pass "[PROMOTE] 回滚基线已更新"
        echo ""
        echo "=========================================="
        echo -e "${GREEN}[OK] 自举闭环全部通过（$GENERATIONS 代 .rs 一致 + 运行全绿）${NC}"
        echo "日志: $log_file"
        echo "=========================================="
    } 2>&1 | tee -a "$log_file"
    local rc="${PIPESTATUS[0]:-0}"
    if [ "$rc" -ne 0 ]; then
        log_fail "闭环退出码: $rc（0=通过 1=lzc 2=rustc 3=run 4=一致性 5=环境）"
    fi
    return "$rc"
}

# ============================================================
# 清理
# ============================================================
clean() {
    log_info "清理构建产物..."
    find "$WORK_DIR" -name "*.rs" -not -path "*/backup/*" -delete
    find "$WORK_DIR" -name "*.exe" -not -path "*/backup/*" -delete
    find "$WORK_DIR" -name "*.pdb" -delete
    log_pass "清理完成"
}

# ============================================================
# 主入口
# ============================================================
mkdir -p "$WORK_DIR"

case "${1:-all}" in
    all)
        build_all
        ;;
    closed)
        build_closed
        ;;
    clean)
        clean
        ;;
    *)
        compile_and_run "$1"
        ;;
esac

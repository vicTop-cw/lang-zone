#!/usr/bin/env bash
# ============================
# LZ Demo Validation Script
# ============================
# Usage: bash run_check.sh [--verbose]
# Checks all .lz files in DEMO/ for basic syntax consistency
# and generates a coverage report.
set -uo pipefail

DEMO_DIR="$(cd "$(dirname "$0")" && pwd)"
SYNTAX_DIR="$(cd "$DEMO_DIR/../SYNTAX" && pwd 2>/dev/null || echo "../SYNTAX")"
VERBOSE=false
[[ "${1:-}" == "--verbose" ]] && VERBOSE=true

PASS=0
FAIL=0
WARN=0

check() {
    local file="$1" desc="$2"
    if [[ -f "$file" ]]; then
        PASS=$((PASS + 1))
        $VERBOSE && echo "  ✅ $desc"
    else
        FAIL=$((FAIL + 1))
        echo "  ❌ 缺失: $desc ($file)"
    fi
}

syntax_check() {
    local file="$1"
    local issues=0

    # Check for struct/trait using : instead of =
    if grep -Eq '^\s*(struct|trait)\s+\w+:' "$file" 2>/dev/null; then
        echo "  ⚠️  语法: $file 中 struct/trait 使用 : (应为 =)"
        issues=$((issues + 1))
    fi

    # Check for impl using : instead of =
    if grep -Eq '^\s*impl\s+.*:' "$file" 2>/dev/null; then
        # impl Color: is OK for enum, but impl Trait for Type: is wrong
        if grep -Eq '^\s*impl\s+\w+\s+for\s+\w+:' "$file" 2>/dev/null; then
            echo "  ⚠️  语法: $file 中 impl for 使用 : (应为 =)"
            issues=$((issues + 1))
        fi
    fi

    # Check for catch: without pattern
    if grep -Eq '^\s*catch:' "$file" 2>/dev/null; then
        echo "  ⚠️  语法: $file 中 catch: 缺少模式参数"
        issues=$((issues + 1))
    fi

    # Check for dangling function definition (def parse.<T> etc.)
    if grep -Eq 'def \w+\.<\w+>' "$file" 2>/dev/null; then
        echo "  ⚠️  语法: $file 中 def .<T> 语法错误（.<T> 只能用于调用，不能用于定义）"
        issues=$((issues + 1))
    fi

    if [[ $issues -gt 0 ]]; then
        WARN=$((WARN + issues))
        return 1
    fi
    return 0
}

echo ""
echo "=========================================="
echo "  LZ Demo 验证报告"
echo "  $(date +'%Y-%m-%d %H:%M')"
echo "=========================================="
echo ""

# === 1. 文件完整性 ===
echo "--- 1. 文件完整性 ---"
check "$DEMO_DIR/01_basics/keywords.lz"       "01_basics/keywords.lz"
check "$DEMO_DIR/01_basics/literals.lz"       "01_basics/literals.lz"
check "$DEMO_DIR/01_basics/identifiers.lz"    "01_basics/identifiers.lz"
check "$DEMO_DIR/01_basics/comments.lz"       "01_basics/comments.lz"
check "$DEMO_DIR/02_types/primitives.lz"      "02_types/primitives.lz"
check "$DEMO_DIR/02_types/containers.lz"      "02_types/containers.lz"
check "$DEMO_DIR/02_types/option_result.lz"   "02_types/option_result.lz"
check "$DEMO_DIR/02_types/type_aliases.lz"    "02_types/type_aliases.lz"
check "$DEMO_DIR/02_types/type_conversion.lz" "02_types/type_conversion.lz"
check "$DEMO_DIR/03_variables/mutable_let.lz" "03_variables/mutable_let.lz"
check "$DEMO_DIR/03_variables/const.lz"       "03_variables/const.lz"
check "$DEMO_DIR/03_variables/ref_binding.lz" "03_variables/ref_binding.lz"
check "$DEMO_DIR/03_variables/ownership.lz"   "03_variables/ownership.lz"
check "$DEMO_DIR/03_variables/walrus.lz"      "03_variables/walrus.lz"
check "$DEMO_DIR/04_functions/basic.lz"       "04_functions/basic.lz"
check "$DEMO_DIR/04_functions/generics.lz"    "04_functions/generics.lz"
check "$DEMO_DIR/04_functions/checker.lz"     "04_functions/checker.lz"
check "$DEMO_DIR/04_functions/variadic.lz"    "04_functions/variadic.lz"
check "$DEMO_DIR/04_functions/composite.lz"   "04_functions/composite.lz"
check "$DEMO_DIR/05_expressions/operators.lz" "05_expressions/operators.lz"
check "$DEMO_DIR/05_expressions/pipe.lz"      "05_expressions/pipe.lz"
check "$DEMO_DIR/05_expressions/comprehension.lz" "05_expressions/comprehension.lz"
check "$DEMO_DIR/05_expressions/if_match_expr.lz"  "05_expressions/if_match_expr.lz"
check "$DEMO_DIR/06_control_flow/if_elif_else.lz"   "06_control_flow/if_elif_else.lz"
check "$DEMO_DIR/06_control_flow/match.lz"    "06_control_flow/match.lz"
check "$DEMO_DIR/06_control_flow/for_while_loop.lz" "06_control_flow/for_while_loop.lz"
check "$DEMO_DIR/06_control_flow/break_continue.lz" "06_control_flow/break_continue.lz"
check "$DEMO_DIR/06_control_flow/guard.lz"    "06_control_flow/guard.lz"
check "$DEMO_DIR/06_control_flow/with_defer.lz"    "06_control_flow/with_defer.lz"
check "$DEMO_DIR/07_data_structures/struct.lz"     "07_data_structures/struct.lz"
check "$DEMO_DIR/07_data_structures/enum.lz"       "07_data_structures/enum.lz"
check "$DEMO_DIR/07_data_structures/trait_impl.lz" "07_data_structures/trait_impl.lz"
check "$DEMO_DIR/07_data_structures/magic_methods.lz" "07_data_structures/magic_methods.lz"
check "$DEMO_DIR/07_data_structures/module_magic.lz"  "07_data_structures/module_magic.lz"
check "$DEMO_DIR/08_modules/import_demo.lz"    "08_modules/import_demo.lz"
check "$DEMO_DIR/09_macros/macro_demo.lz"      "09_macros/macro_demo.lz"
check "$DEMO_DIR/09_macros/comptime_demo.lz"   "09_macros/comptime_demo.lz"
check "$DEMO_DIR/10_error_handling/panic_raise_try.lz" "10_error_handling/panic_raise_try.lz"
check "$DEMO_DIR/11_concurrency/async_spawn.lz" "11_concurrency/async_spawn.lz"
check "$DEMO_DIR/12_build_blocks/var_call_block.lz" "12_build_blocks/var_call_block.lz"
check "$DEMO_DIR/13_operators/precedence.lz"   "13_operators/precedence.lz"
check "$DEMO_DIR/14_pointers/box_rc_arc.lz"    "14_pointers/box_rc_arc.lz"
check "$DEMO_DIR/15_generators/yield_demo.lz"  "15_generators/yield_demo.lz"
check "$DEMO_DIR/16_testing/test_suite.lz"     "16_testing/test_suite.lz"
check "$DEMO_DIR/99_prelude/prelude_demo.lz"   "99_prelude/prelude_demo.lz"
check "$DEMO_DIR/99_errors/00_lexical_errors.lz"      "99_errors/00_lexical_errors.lz"
check "$DEMO_DIR/99_errors/01_type_errors.lz"         "99_errors/01_type_errors.lz"
check "$DEMO_DIR/99_errors/02_variable_errors.lz"     "99_errors/02_variable_errors.lz"
check "$DEMO_DIR/99_errors/03_function_errors.lz"     "99_errors/03_function_errors.lz"
check "$DEMO_DIR/99_errors/05_control_flow_errors.lz" "99_errors/05_control_flow_errors.lz"
check "$DEMO_DIR/99_errors/07_module_errors.lz"       "99_errors/07_module_errors.lz"
check "$DEMO_DIR/99_errors/09_error_handling_errors.lz" "99_errors/09_error_handling_errors.lz"
check "$DEMO_DIR/99_errors/10_concurrency_errors.lz"  "99_errors/10_concurrency_errors.lz"

echo ""
echo "--- 2. 语法一致性检查 ---"
SYNTAX_WARN=0
while IFS= read -r -d '' f; do
    syntax_check "$f" && true
done < <(find "$DEMO_DIR" -name "*.lz" -not -path "*/99_errors/*" -print0)

echo ""
echo "--- 3. 覆盖率统计 ---"
TOTAL_FILES=$(find "$DEMO_DIR" -name "*.lz" -not -path "*/99_errors/*" | wc -l)
ERROR_FILES=$(find "$DEMO_DIR/99_errors" -name "*.lz" 2>/dev/null | wc -l)
TOTAL_LINES=$(find "$DEMO_DIR" -name "*.lz" -exec cat {} + | wc -l)

echo "  主 demo 文件: $TOTAL_FILES"
echo "  错误边界文件: $ERROR_FILES"
echo "  总代码行数:  $TOTAL_LINES"

echo ""
echo "--- 4. 关键字覆盖验证 ---"
KW_FILE="$DEMO_DIR/01_basics/keywords.lz"
if [[ -f "$KW_FILE" ]]; then
    FOUND=0
    MISSING=0
    for kw in struct trait impl type const mut ref let owned magic if elif else match case guard for while loop pass break continue return with defer panic raise raises try catch finally async await spawn go yield import from as where Self macro comptime template test assert check suite setup teardown and or not is True False None Some Ok Err Never Unit Nil; do
        if grep -q "\b$kw\b" "$KW_FILE" 2>/dev/null; then
            FOUND=$((FOUND + 1))
        else
            if [[ "$kw" != "template" && "$kw" != "macro" ]]; then
                echo "  ⚠️  关键字缺失: $kw 不在 keywords.lz 中"
                MISSING=$((MISSING + 1))
            fi
        fi
    done
    echo "  关键字覆盖: $FOUND/$(($FOUND + $MISSING)) (macro/template 在 #!bin macro 模块中)"
fi

echo ""
echo "=========================================="
echo "  结果: $PASS 文件存在 | $WARN 语法警告 | $FAIL 缺失"
if [[ $FAIL -eq 0 ]]; then
    echo "  ✅ 所有核心文件就位"
else
    echo "  ❌ 有 $FAIL 个文件缺失"
fi
echo "=========================================="

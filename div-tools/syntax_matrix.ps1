#!/usr/bin/env pwsh
# ============================================================
# lz 语法特性正反例测试矩阵
# 正例：DEMO 文件编译退出码 0；反例：探针编译退出码非 0 且诊断可读
# 用法: powershell -File div-tools\syntax_matrix.ps1
# 退出码: 0 全部通过; 1 有意外结果
# ============================================================
param()
$ErrorActionPreference = 'Continue'
$ROOT = Split-Path -Parent $PSScriptRoot
$LZ = Join-Path $ROOT 'target\debug\lang-zone.exe'
$STD = Join-Path $ROOT 'std'
$PROBES = Join-Path $PSScriptRoot 'syntax_probes'
$PASS_POS = 0; $FAIL_POS = 0; $PASS_NEG = 0; $FAIL_NEG = 0
$posFailList = @(); $negFailList = @()

if (-not (Test-Path $LZ)) { Write-Error "缺少 $LZ"; exit 2 }

# ---------- 特性清单（SYNTAX 36 文档 → 特性）----------
$features = @(
    @{ n = "词法-缩进块"; d = "00"; pos = "01_basics\control_flow.lz" },
    @{ n = "词法-字符串/转义"; d = "00"; pos = "01_basics\strings.lz" },
    @{ n = "词法-数字字面量"; d = "00"; pos = "01_basics\literals.lz" },
    @{ n = "类型-基础类型"; d = "01"; pos = "02_types\primitives.lz" },
    @{ n = "类型-容器"; d = "01"; pos = "02_types\containers.lz" },
    @{ n = "类型-类型标注"; d = "01"; pos = "03_variables\const.lz" },
    @{ n = "duck 关系约束"; d = "01b"; pos = "07_data_structures\duck_typing.lz" },
    @{ n = "变量-不可变绑定"; d = "02"; pos = "03_variables\const.lz" },
    @{ n = "变量-可变绑定"; d = "02"; pos = "03_variables\mutable_let.lz" },
    @{ n = "变量-引用绑定"; d = "02"; pos = "03_variables\ref_binding.lz" },
    @{ n = "变量-海象"; d = "02"; pos = "03_variables\walrus.lz" },
    @{ n = "函数-基础"; d = "03"; pos = "04_functions\basic.lz" },
    @{ n = "函数-泛型"; d = "03b"; pos = "04_functions\generics.lz" },
    @{ n = "函数-复合"; d = "03e"; pos = "04_functions\composite.lz" },
    @{ n = "检查站 checker"; d = "03c"; pos = "06_control_flow\def_checker.lz" },
    @{ n = "可变参数"; d = "03d"; pos = "04_functions\varargs.lz" },
    @{ n = "闭包"; d = "03e"; pos = "01_basics\functions_advanced.lz" },
    @{ n = "表达式-管道"; d = "04"; pos = "05_expressions\pipe.lz" },
    @{ n = "表达式-三元"; d = "04"; pos = "05_expressions\ternary.lz" },
    @{ n = "表达式-推导式"; d = "04"; pos = "05_expressions\comprehension.lz" },
    @{ n = "控制流-if/elif"; d = "05"; pos = "06_control_flow\if_elif_else.lz" },
    @{ n = "控制流-循环"; d = "05"; pos = "06_control_flow\for_while_loop.lz" },
    @{ n = "控制流-break/continue"; d = "05"; pos = "06_control_flow\break_continue.lz" },
    @{ n = "控制流-guard"; d = "05"; pos = "06_control_flow\guard.lz" },
    @{ n = "block 命名块"; d = "05b"; pos = "06_control_flow\block_demo.lz" },
    @{ n = "数据结构-struct"; d = "06a"; pos = "07_data_structures\struct.lz" },
    @{ n = "数据结构-enum"; d = "06b"; pos = "01_basics\enums.lz" },
    @{ n = "trait/impl"; d = "06c"; pos = "07_data_structures\trait_impl.lz" },
    @{ n = "魔法方法"; d = "06f"; pos = "07_data_structures\magic_methods.lz" },
    @{ n = "自引用 Self"; d = "06h"; pos = "07_data_structures\self_recursive.lz" },
    @{ n = "模块与导入"; d = "07"; pos = "08_modules\import_demo.lz" },
    @{ n = "宏"; d = "08"; pos = "09_macros\macro_demo.lz" },
    @{ n = "comptime"; d = "08b"; pos = "09_macros\comptime_demo.lz" },
    @{ n = "错误处理"; d = "09"; pos = "10_error_handling\panic_raise_try.lz" },
    @{ n = "并发异步"; d = "10"; pos = "11_concurrency\async_spawn.lz" },
    @{ n = "构建块"; d = "11"; pos = "12_build_blocks\var_call_block.lz" },
    @{ n = "操作符"; d = "12"; pos = "13_operators\compound_assign_more.lz" },
    @{ n = "指针与引用"; d = "13"; pos = "14_pointers\box_rc_arc.lz" },
    @{ n = "生成器"; d = "14"; pos = "15_generators\yield_demo.lz" },
    @{ n = "测试框架"; d = "15"; pos = "16_testing\test_suite.lz" }
)

Write-Host "===== 正例（DEMO 编译）====="
foreach ($f in $features) {
    $p = Join-Path $ROOT ("DEMO\" + $f.pos)
    if (-not (Test-Path $p)) { Write-Host "[SKIP] $($f.n) — 无正例文件 $($f.pos)"; continue }
    & $LZ $p --std-dir $STD 2>$null | Out-Null
    if ($LASTEXITCODE -eq 0) { $PASS_POS++; Write-Host "[PASS] $($f.n)" }
    else { $FAIL_POS++; $posFailList += "$($f.n) ($($f.pos))"; Write-Host "[FAIL] $($f.n)" }
}

Write-Host "===== 反例（探针应失败）====="
$negProbes = Get-ChildItem $PROBES -Filter "neg_*.lz" -File | Sort-Object Name
foreach ($probe in $negProbes) {
    $errOut = & $LZ $probe.FullName --std-dir $STD 2>&1 | Out-String
    $rc = $LASTEXITCODE
    $readable = $errOut -match 'error'
    if ($rc -ne 0 -and $readable) { $PASS_NEG++; Write-Host "[PASS] $($probe.Name) (rc=$rc)" }
    else { $FAIL_NEG++; $negFailList += $probe.Name; Write-Host "[FAIL] $($probe.Name) (rc=$rc readable=$readable)" }
}

# ---------- 报告 ----------
$totalPos = $PASS_POS + $FAIL_POS
$totalNeg = $PASS_NEG + $FAIL_NEG
$report = @"
# lz 语法特性测试矩阵报告

- 日期：$(Get-Date -Format 'yyyy-MM-dd HH:mm')
- commit：$(git -C $ROOT rev-parse --short HEAD 2>$null)

## 特性清单与正例（DEMO 编译）

- 总特性数：$($features.Count)
- 正例通过：$PASS_POS / $totalPos
- 失败清单：$(if ($posFailList.Count -eq 0) { '无' } else { $posFailList -join '; ' })

| 特性 | 规范文档 | 正例 |
|---|---|---|
$($features | ForEach-Object { "| $($_.n) | $($_.d) | $($_.pos) |" })

## 反例矩阵（应编译失败）

- 反例数：$totalNeg
- 按预期失败：$PASS_NEG / $totalNeg
- 意外结果：$(if ($negFailList.Count -eq 0) { '无' } else { $negFailList -join '; ' })

| 探针 | 说明 |
|---|---|
$($negProbes | ForEach-Object { "| $($_.Name) | 语法错误反例 |" })

## 缺口清单

- 无正例的特性：见上 [SKIP] 行
- 未覆盖反例的特性：$(($features.Count - $negProbes.Count) -gt 0)（反例覆盖核心特性子集）
"@
Set-Content -Path (Join-Path $ROOT 'tests\syntax-matrix-report.md') -Value $report -Encoding UTF8
Write-Host ""
Write-Host "=========================================="
Write-Host "正例: $PASS_POS/$totalPos  反例: $PASS_NEG/$totalNeg"
Write-Host "报告: tests\syntax-matrix-report.md"
Write-Host "=========================================="
if ($FAIL_POS -eq 0 -and $FAIL_NEG -eq 0) { exit 0 } else { exit 1 }

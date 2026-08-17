#!/usr/bin/env pwsh
# ============================================================
# C3：--emit=ir vs --emit=ir-lz 双路 diff 对照脚本（自举 50% 里程碑）
# 验证 LZ 版 IR display（lz_ir_lib.lz）与 Rust 版 display.rs 逐字符一致。
#
# 用法:
#   powershell -File bootstrap\work\lz_ir\diff_ir.ps1          # 默认输入集（8 个关键 DEMO）
#   powershell -File bootstrap\work\lz_ir\diff_ir.ps1 <file.lz> [more.lz ...]
#
# 退出码：0 全部一致；1 至少一个输入两路输出不一致；2 环境缺失；3 单输入 ir-lz 失败
# ============================================================
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Inputs
)

$ErrorActionPreference = 'Continue'
$PROJECT = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
$LZC = Join-Path $PROJECT 'target\debug\lang-zone.exe'
$WORK = Join-Path $PSScriptRoot 'diffwork'
$DEMO = Join-Path $PROJECT 'DEMO'

if (-not (Test-Path $LZC)) { Write-Error "[ENV] 缺少 $LZC（先 cargo build）"; exit 2 }
New-Item -ItemType Directory -Force -Path $WORK | Out-Null

# 默认输入集：8 个关键 DEMO（覆盖字面量/控制流/容器/泛型 struct/推导式/guard/trait impl/const）
$defaultInputs = @(
    '01_basics\literals.lz',
    '02_types\containers.lz',
    '03_variables\const.lz',
    '05_expressions\ternary.lz',
    '05_expressions\comprehension.lz',
    '06_control_flow\guard.lz',
    '07_data_structures\struct.lz',
    '07_data_structures\trait_impl.lz'
)
$inputList = @()
if ($Inputs.Count -gt 0) {
    $inputList = $Inputs
} else {
    foreach ($rel in $defaultInputs) { $inputList += (Join-Path $DEMO $rel) }
}

# 运行 lzc 并捕获纯净 stdout（stderr 单独落盘，避免与 IR 文本混流）
function Invoke-Emit($InputPath, $Flag) {
    $name = [System.IO.Path]::GetFileNameWithoutExtension($InputPath)
    $outFile = Join-Path $WORK ($name + '_' + $Flag.Replace('=', '') + '.out')
    $errFile = Join-Path $WORK ($name + '_' + $Flag.Replace('=', '') + '.err')
    $proc = Start-Process -FilePath $LZC `
        -ArgumentList @("`"$InputPath`"", $Flag) `
        -NoNewWindow -Wait -PassThru `
        -RedirectStandardOutput $outFile -RedirectStandardError $errFile
    return @{ rc = $proc.ExitCode; out = $outFile }
}

$total = 0; $identical = 0; $diffList = @(); $envFail = @()
foreach ($p in $inputList) {
    if (-not (Test-Path $p)) { Write-Warning "跳过（不存在）: $p"; continue }
    $total += 1
    $name = [System.IO.Path]::GetFileNameWithoutExtension($p)

    $r1 = Invoke-Emit $p '--emit=ir'
    if ($r1.rc -ne 0) { $envFail += "$name : --emit=ir 失败(rc=$($r1.rc))"; continue }
    $r2 = Invoke-Emit $p '--emit=ir-lz'
    if ($r2.rc -ne 0) { $envFail += "$name : --emit=ir-lz 失败(rc=$($r2.rc))"; continue }

    $diff = git diff --no-index -- $r1.out $r2.out 2>$null
    if ($LASTEXITCODE -eq 0) {
        $identical += 1
        Write-Host "[IDENTICAL] $name"
    } else {
        $diffList += $name
        Write-Host "[DIFF] $name"
    }
}

Write-Host ""
Write-Host "=========================================="
Write-Host "C3 双路 diff 对照：$identical/$total 逐字符一致"
if ($envFail.Count -gt 0) {
    Write-Host "环境/运行失败:"
    foreach ($f in $envFail) { Write-Host "  - $f" }
}
if ($diffList.Count -gt 0) {
    Write-Host "不一致:"
    foreach ($f in $diffList) { Write-Host "  - $f" }
}
Write-Host "=========================================="

if ($envFail.Count -gt 0) { exit 3 }
if ($diffList.Count -gt 0) { exit 1 }
exit 0

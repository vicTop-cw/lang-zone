#!/usr/bin/env pwsh
# ============================================================
# D2：--emit=rs-lz（LZ 版 codegen）vs 常规 codegen 双路 .rs diff 对照
# 验证 LZ 版 codegen 对 DEMO 子集产出与 Rust 版逐字符一致，并 rustc 编译运行。
#
# 用法:
#   powershell -File bootstrap\work\lz_codegen\diff_rs.ps1        # 默认输入集（8 个关键 DEMO）
#   powershell -File bootstrap\work\lz_codegen\diff_rs.ps1 <file.lz> [more.lz ...]
#
# 退出码：0 全部一致且编译运行通过；1 有 diff；2 环境缺失；3 运行/编译失败
# ============================================================
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Inputs
)

$ErrorActionPreference = 'Continue'
$PROJECT = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
$LZC = Join-Path $PROJECT 'target\debug\lang-zone.exe'
$RLIB = Join-Path $PROJECT 'target\debug\liblz_builtins.rlib'
$WORK = Join-Path $PSScriptRoot 'diffwork'
$DEMO = Join-Path $PROJECT 'DEMO'

if (-not (Test-Path $LZC)) { Write-Error "[ENV] 缺少 $LZC（先 cargo build）"; exit 2 }
if (-not (Test-Path $RLIB)) { Write-Error "[ENV] 缺少 $RLIB（先 cargo build -p lz_builtins）"; exit 2 }
New-Item -ItemType Directory -Force -Path $WORK | Out-Null

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

function Invoke-Native($InputPath) {
    $name = [System.IO.Path]::GetFileNameWithoutExtension($InputPath)
    $rsOut = Join-Path $WORK ($name + '_native.rs')
    $errOut = Join-Path $WORK ($name + '_native.err')
    $proc = Start-Process -FilePath $LZC `
        -ArgumentList @("`"$InputPath`"") `
        -NoNewWindow -Wait -PassThru `
        -RedirectStandardOutput (Join-Path $WORK ($name + '_native.out')) -RedirectStandardError $errOut
    # 常规路径 .rs 生成在输入旁；拷到 work 下比对
    $genRs = $InputPath -replace '\.lz$', '.rs'
    if (Test-Path $genRs) { Copy-Item $genRs $rsOut -Force }
    return @{ rc = $proc.ExitCode; rs = $rsOut; genRs = $genRs }
}

function Invoke-Lz($InputPath) {
    $name = [System.IO.Path]::GetFileNameWithoutExtension($InputPath)
    $rsOut = Join-Path $WORK ($name + '_lz.rs')
    $errOut = Join-Path $WORK ($name + '_lz.err')
    $proc = Start-Process -FilePath $LZC `
        -ArgumentList @("`"$InputPath`"", '--emit=rs-lz') `
        -NoNewWindow -Wait -PassThru `
        -RedirectStandardOutput $rsOut -RedirectStandardError $errOut
    return @{ rc = $proc.ExitCode; rs = $rsOut }
}

$total = 0; $identical = 0; $diffList = @(); $failList = @()
foreach ($p in $inputList) {
    if (-not (Test-Path $p)) { Write-Warning "跳过（不存在）: $p"; continue }
    $total += 1
    $name = [System.IO.Path]::GetFileNameWithoutExtension($p)

    $n = Invoke-Native $p
    if ($n.rc -ne 0 -or -not (Test-Path $n.rs)) { $failList += "$name : native codegen 失败(rc=$($n.rc))"; continue }
    $l = Invoke-Lz $p
    if ($l.rc -ne 0) { $failList += "$name : rs-lz 失败(rc=$($l.rc))"; continue }

    $d = git diff --no-index -- $n.rs $l.rs 2>$null
    if ($LASTEXITCODE -ne 0) { $diffList += $name; Write-Host "[DIFF] $name"; continue }

    # rustc 编译 + 运行（行为一致）
    $exe = Join-Path $WORK ($name + '.exe')
    $rc = Start-Process -FilePath rustc -ArgumentList @('--edition','2021',"`"$($l.rs)`"","--extern","lz_builtins=$RLIB",'-o',"`"$exe`"") -NoNewWindow -Wait -PassThru
    if ($rc.ExitCode -ne 0) { $failList += "$name : rustc 编译失败"; continue }
    $run = Start-Process -FilePath $exe -NoNewWindow -Wait -PassThru -RedirectStandardOutput (Join-Path $WORK ($name + '.run')) -RedirectStandardError (Join-Path $WORK ($name + '.runerr'))
    if ($run.ExitCode -ne 0) { $failList += "$name : 运行失败(rc=$($run.ExitCode))"; continue }

    $identical += 1
    Write-Host "[IDENTICAL] $name"
}

Write-Host ""
Write-Host "=========================================="
Write-Host "D2 双路 .rs diff 对照：$identical/$total 逐字符一致且编译运行通过"
if ($failList.Count -gt 0) { Write-Host "失败:"; foreach ($f in $failList) { Write-Host "  - $f" } }
if ($diffList.Count -gt 0) { Write-Host "不一致:"; foreach ($f in $diffList) { Write-Host "  - $f" } }
Write-Host "=========================================="

if ($failList.Count -gt 0) { exit 3 }
if ($diffList.Count -gt 0) { exit 1 }
exit 0

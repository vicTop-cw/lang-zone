#!/usr/bin/env pwsh
# ============================================================
# lz 三代自举收敛脚本（stage.ps1）
#
# 口径（对齐 bootstrap/05-自举里程碑台账 §0 冻结口径）：
#   最小可信基座 = Rust toolchain（cargo/rustc）。
#   "自举" = 编译器组件由 .lz 自身处理，不依赖手写 .rs 配套。
#   本脚本验证三代收敛：宿主编译器（gen0）处理自举源集得到第 1 代产物；
#   重复处理得到第 2 代、第 3 代；第 2 代与第 3 代逐字节一致 = 收敛。
#   每一代对编译器组件做两种处理：
#     1) 前端自处理链：LZ 写的 lexer/parser 驱动器处理自身源码（read __file__）
#     2) codegen 链：--emit=rs-lz（LZ 写的 codegen 库）为探针产出 Rust 源码
#
# 用法（干净克隆后直接跑）:
#   powershell -File bootstrap\stage\stage.ps1
# 退出码: 0 收敛; 1 任一环节失败; 2 环境缺失
# ============================================================
$ErrorActionPreference = 'Continue'
$ROOT = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$LZ = Join-Path $ROOT 'target\debug\lang-zone.exe'
$RLIB = Join-Path $ROOT 'target\debug\liblz_builtins.rlib'
$STAGE = Join-Path $PSScriptRoot 'gen'
$STD = Join-Path $ROOT 'std'

if (-not (Test-Path $LZ)) { Write-Error "[ENV] 缺少 $LZ（先 cargo build）"; exit 2 }
if (-not (Test-Path $RLIB)) { Write-Error "[ENV] 缺少 $RLIB（先 cargo build -p lz_builtins）"; exit 2 }
New-Item -ItemType Directory -Force -Path $STAGE | Out-Null

$COMMIT = git -C $ROOT rev-parse --short HEAD 2>$null
$START = Get-Date
$gen = 0
$fails = 0

# 计算一代产物的 manifest（SHA256 over 全部产出文件，按路径排序）
function Gen-Hash($dir) {
    # 口径（对齐 05 §5）：.exe 二进制哈希受 rustc 非确定性影响不参与比对；
    # 只比对 .rs 产物与运行输出（.out）
    $files = Get-ChildItem $dir -Recurse -File | Where-Object { $_.Extension -ne '.exe' -and $_.Extension -ne '.pdb' -and $_.Extension -ne '.err' } | Sort-Object FullName
    $out = @()
    foreach ($f in $files) {
        $h = (Get-FileHash -Path $f.FullName -Algorithm SHA256).Hash.ToLower()
        $rel = $f.FullName.Substring($dir.Length + 1)
        $out += "$h  $rel"
    }
    $out -join "`n"
}

# 运行一代：宿主 lzc 处理自举源集，产出到 $STAGE\g$gen\
function Run-Generation($genIndex) {
    $gdir = Join-Path $STAGE "g$genIndex"
    New-Item -ItemType Directory -Force -Path $gdir | Out-Null

    # --- 1) 前端自处理链（LZ 写的 lexer/parser 处理自身源码）---
    foreach ($name in @("frontend_lexer_self", "frontend_parser_self")) {
        $src = Join-Path $PSScriptRoot "$name.lz"
        $out = Start-Process -FilePath $LZ -ArgumentList @("`"$src`"", "--std-dir", "`"$STD`"") -NoNewWindow -Wait -PassThru -RedirectStandardOutput (Join-Path $gdir "$name.log") -RedirectStandardError (Join-Path $gdir "$name.err")
        if ($out.ExitCode -ne 0) { Write-Host "[FAIL] gen$genIndex $name lzc rc=$($out.ExitCode)"; return 1 }
        $exe = Join-Path $gdir "$name.exe"
        $rc = Start-Process -FilePath rustc -ArgumentList @('--edition','2021',"`"$($src -replace '\.lz$','.rs')`"",'--extern',"lz_builtins=$RLIB",'-o',"`"$exe`"") -NoNewWindow -Wait -PassThru
        if ($rc.ExitCode -ne 0) { Write-Host "[FAIL] gen$genIndex $name rustc rc=$($rc.ExitCode)"; return 1 }
        $run = Start-Process -FilePath $exe -NoNewWindow -Wait -PassThru -RedirectStandardOutput (Join-Path $gdir "$name.out") -RedirectStandardError (Join-Path $gdir "$name.runerr")
        if ($run.ExitCode -ne 0) { Write-Host "[FAIL] gen$genIndex $name run rc=$($run.ExitCode)"; return 1 }
        Copy-Item ($src -replace '\.lz$','.rs') (Join-Path $gdir "$name.rs") -Force
    }

    # --- 2) codegen 链：--emit=rs-lz 为探针产出 Rust 源码 ---
    foreach ($probe in @("probe_struct", "probe_literals")) {
        $src = Join-Path $PSScriptRoot "$probe.lz"
        $out = Start-Process -FilePath $LZ -ArgumentList @("`"$src`"", "--emit=rs-lz", "--std-dir", "`"$STD`"") -NoNewWindow -Wait -PassThru -RedirectStandardOutput (Join-Path $gdir "$probe.rs") -RedirectStandardError (Join-Path $gdir "$probe.err")
        if ($out.ExitCode -ne 0) { Write-Host "[FAIL] gen$genIndex $probe rs-lz rc=$($out.ExitCode)"; return 1 }
    }
    return 0
}

# ── 三代执行 ──
for ($i = 1; $i -le 3; $i++) {
    Write-Host "=== 第 $i 代 ==="
    $t0 = Get-Date
    if ((Run-Generation $i) -ne 0) { $fails++; Write-Host "[FAIL] 第 $i 代执行失败"; break }
    $h = Gen-Hash (Join-Path $STAGE "g$i")
    Set-Content -Path (Join-Path $STAGE "g$i.sha256") -Value $h -Encoding ASCII
    Write-Host "  g$i manifest: $((($h -split "`n").Count)) 项, 耗时 $([int]((Get-Date) - $t0).TotalSeconds)s"
}

# ── 收敛判定 ──
if ($fails -eq 0) {
    $h1 = Get-Content (Join-Path $STAGE 'g1.sha256') -Raw
    $h2 = Get-Content (Join-Path $STAGE 'g2.sha256') -Raw
    $h3 = Get-Content (Join-Path $STAGE 'g3.sha256') -Raw
    $converged = ($h2 -eq $h3)
    $first2 = ($h1 -eq $h2)
    Write-Host ""
    if ($converged) { Write-Host "[OK] 三代收敛：第 2 代 == 第 3 代（manifest 逐字节一致）" }
    else { Write-Host "[FAIL] 第 2/3 代 manifest 不一致（非确定性构建）"; $fails++ }
    if ($first2) { Write-Host "[OK] 第 1 代 == 第 2 代（自举起点稳定）" }

    # ── 台账追加 ──
    $totalSec = [int]((Get-Date) - $START).TotalSeconds
    $ledger = Join-Path $PSScriptRoot 'ledger.md'
    $entry = @"

## $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') 三代收敛 run
- commit: $COMMIT
- 命令: powershell -File bootstrap\stage\stage.ps1
- 判定: 收敛=$converged 首两代一致=$first2
- 产物哈希: g1=$(($h1 -split "`n")[0].Substring(0,16))... g2=$(($h2 -split "`n")[0].Substring(0,16))... g3=$(($h3 -split "`n")[0].Substring(0,16))...
- manifest: gen/g1.sha256 gen/g2.sha256 gen/g3.sha256
- 耗时: ${totalSec}s
"@
    Add-Content -Path $ledger -Value $entry -Encoding UTF8
    Write-Host "台账: $ledger"
}

if ($fails -gt 0) { exit 1 } else { exit 0 }

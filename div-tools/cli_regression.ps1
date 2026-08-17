#!/usr/bin/env pwsh
# ============================================================
# lz CLI 子命令 E2E 回归脚本（create/build/check/peek/push）
# 覆盖每个命令的成功与失败路径，断言退出码与关键输出。
# 用法: powershell -File div-tools\cli_regression.ps1
# 退出码: 0 全部通过; 1 有失败
# ============================================================
param()
$ErrorActionPreference = 'Continue'
$ROOT = Split-Path -Parent $PSScriptRoot
$LZ = Join-Path $ROOT 'target\debug\lang-zone.exe'
$WORK = Join-Path $env:TEMP 'lz_cli_e2e'
$FAIL = 0

function Assert-Eq($name, $expected, $actual) {
    if ($expected -eq $actual) { Write-Host "[PASS] $name" }
    else { Write-Host "[FAIL] $name (expected=$expected actual=$actual)"; $script:FAIL++ }
}
function Assert-True($name, $cond) {
    if ($cond) { Write-Host "[PASS] $name" }
    else { Write-Host "[FAIL] $name"; $script:FAIL++ }
}

if (-not (Test-Path $LZ)) { Write-Error "缺少 $LZ（先 cargo build）"; exit 2 }
if (Test-Path $WORK) { Remove-Item $WORK -Recurse -Force }
New-Item -ItemType Directory -Force -Path $WORK | Out-Null

# ---------- --help / --version ----------
$out = & $LZ --help 2>&1; $rc = $LASTEXITCODE
Assert-Eq "--help 退出码" 0 $rc
Assert-True "--help 列出 create" (($out -join "`n") -match 'create')
Assert-True "--help 列出 build" (($out -join "`n") -match 'build')
Assert-True "--help 列出 peek" (($out -join "`n") -match 'peek')
Assert-True "--help 列出 check" (($out -join "`n") -match 'check')
Assert-True "--help 列出 push" (($out -join "`n") -match 'push')
$v = & $LZ --version 2>&1; Assert-Eq "--version 退出码" 0 $LASTEXITCODE
Assert-True "--version 含版本号" (($v -join "`n") -match '\d+\.\d+\.\d+')

# ---------- create ----------
$proj = Join-Path $WORK 'demo_proj'
$out = & $LZ create $proj 2>&1; $rc = $LASTEXITCODE
Assert-Eq "create 退出码" 0 $rc
Assert-True "create 生成 lz.toml" (Test-Path (Join-Path $proj 'lz.toml'))
Assert-True "create 生成 src/main.lz" (Test-Path (Join-Path $proj 'src\main.lz'))

# 已存在目录再 create → 非零
$out = & $LZ create $proj 2>&1; $rc = $LASTEXITCODE
Assert-True "create 已存在目录失败" ($rc -ne 0)

# ---------- build（create 产物直接构建）----------
$out = & $LZ build $proj 2>&1; $rc = $LASTEXITCODE
Assert-Eq "build 退出码" 0 $rc
$exe = Join-Path $proj 'build\demo_proj.exe'
Assert-True "build 产出 exe" (Test-Path $exe)

# 干净构建与重复构建产物哈希一致
$h1 = (Get-FileHash (Join-Path $proj 'build\demo_proj.rs')).Hash
$out = & $LZ build $proj 2>&1 | Out-Null; & $LZ build $proj 2>&1 | Out-Null
$h2 = (Get-FileHash (Join-Path $proj 'build\demo_proj.rs')).Hash
Assert-Eq "build 可复现（.rs 哈希一致）" $h1 $h2

# ---------- check ----------
$out = & $LZ check $proj 2>&1; $rc = $LASTEXITCODE
Assert-Eq "check 退出码" 0 $rc
Assert-True "check 不产出 .rs" (-not (Test-Path (Join-Path $proj 'src\main.rs')))
Assert-True "check 不产出 exe" (-not (Test-Path (Join-Path $proj 'src\main.exe')))

# 故意写错 → check/build 失败且诊断含行列
$bad = Join-Path $proj 'src\main.lz'
$utf8n = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($bad, "def broken( =`r`n    x", $utf8n)
$out = & $LZ check $proj 2>&1; $rc = $LASTEXITCODE
Assert-True "check 语法错误非零退出" ($rc -ne 0)
Assert-True "check 诊断可读" (($out -join "`n") -match 'error')
$out = & $LZ build $proj 2>&1; $rc = $LASTEXITCODE
Assert-True "build 语法错误非零退出" ($rc -ne 0)

# ---------- peek ----------
$okfile = Join-Path $WORK 'peek_ok.lz'
[System.IO.File]::WriteAllText($okfile, "def f(x: int) -> int =`r`n    x + 1", $utf8n)
$out = & $LZ peek $okfile 2>&1; $rc = $LASTEXITCODE
Assert-Eq "peek 退出码" 0 $rc
Assert-True "peek 有输出" (($out -join "`n").Length -gt 0)
$out = & $LZ peek (Join-Path $WORK 'no_such_file.lz') 2>&1; $rc = $LASTEXITCODE
Assert-True "peek 不存在文件非零退出" ($rc -ne 0)

# ---------- push（dry-run + 本地 registry）----------
$reg = Join-Path $WORK 'registry'
[System.IO.File]::WriteAllText($bad, "def main() =`r`n    print(`"ok`")", $utf8n)
$out = & $LZ push $proj --dry-run --registry $reg 2>&1; $rc = $LASTEXITCODE
Assert-Eq "push --dry-run 退出码" 0 $rc
Assert-True "dry-run 不产生 registry 文件" (-not (Test-Path (Join-Path $reg 'demo_proj-0.1.0')))

$out = & $LZ push $proj --registry $reg 2>&1; $rc = $LASTEXITCODE
Assert-Eq "push 本地发布退出码" 0 $rc
Assert-True "push 产生 registry 条目" (Test-Path $reg)
$out = & $LZ push $proj --registry $reg 2>&1; $rc = $LASTEXITCODE
Assert-True "push 同版本冲突非零退出" ($rc -ne 0)

# ---------- 汇总 ----------
Write-Host ""
Write-Host "=========================================="
if ($FAIL -eq 0) { Write-Host "CLI E2E 全部通过" } else { Write-Host "CLI E2E 失败: $FAIL 项" }
Write-Host "=========================================="
exit $(if ($FAIL -eq 0) { 0 } else { 1 })


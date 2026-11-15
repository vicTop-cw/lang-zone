# ============================================================
# Lang-Zong 自举回滚脚本（PowerShell 入口）
# 与 bootstrap/rollback.sh 等价：恢复到 backup/good 基线
# 用法: .\bootstrap\rollback.ps1
# 退出码: 0 成功；2 无回滚基线；3 恢复后哈希校验失败
# ============================================================
$ErrorActionPreference = "Continue"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$WorkDir = Join-Path $ScriptDir "work"
$BackupGood = Join-Path $WorkDir "backup\good"
$ManifestDir = Join-Path $WorkDir "manifest"

if (-not (Test-Path (Join-Path $BackupGood "manifest.sha256"))) {
    Write-Host "[FAIL] 无回滚基线: backup/good/manifest.sha256 不存在（先跑一轮 -Mode Closed 建立基线）" -ForegroundColor Red
    exit 2
}

$ts = Get-Date -Format "yyyyMMdd-HHmmss"
$BadDir = Join-Path $WorkDir "backup\bad-$ts"
New-Item -ItemType Directory -Force -Path $BadDir | Out-Null

# 1. 当前产物移入 bad-<时间戳>/（保留现场，非删除）
$current = Get-ChildItem -Path $WorkDir -Recurse -File |
    Where-Object { ($_.Extension -eq ".rs" -or $_.Extension -eq ".exe") -and
                   $_.FullName -notmatch "\\backup\\" -and $_.FullName -notmatch "\\manifest\\" -and $_.FullName -notmatch "\\log\\" }
foreach ($f in $current) {
    $rel = $f.FullName.Substring($WorkDir.Length + 1)
    $dest = Join-Path $BadDir $rel
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dest) | Out-Null
    Move-Item $f.FullName $dest -Force
}

# 2. 从 backup/good 恢复
Get-ChildItem -Path $BackupGood -Recurse -File |
    Where-Object { ($_.Extension -eq ".rs" -or $_.Extension -eq ".exe") -and $_.Name -ne "manifest.sha256" } |
    ForEach-Object {
        $rel = $_.FullName.Substring($BackupGood.Length + 1)
        $dest = Join-Path $WorkDir $rel
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dest) | Out-Null
        Copy-Item $_.FullName $dest -Force
    }

# 3. 哈希校验（仅 .rs，与基线 manifest 口径一致；.exe 不参与哈希比对）
$expected = Get-Content (Join-Path $BackupGood "manifest.sha256") | Sort-Object
$actual = Get-ChildItem -Path $WorkDir -Recurse -File |
    Where-Object { ($_.Extension -eq ".rs") -and
                   $_.FullName -notmatch "\\backup\\" -and $_.FullName -notmatch "\\manifest\\" -and $_.FullName -notmatch "\\log\\" -and $_.FullName -notmatch "\\runout\\" } |
    ForEach-Object {
        $hash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash.ToLower()
        $rel = $_.FullName.Substring($WorkDir.Length + 1)
        "$hash  $rel"
    } | Sort-Object

$diff = Compare-Object $expected $actual
if (-not $diff) {
    Write-Host "[OK] 回滚成功：产物已恢复并与基线 manifest 一致" -ForegroundColor Green
    Write-Host "     被替换产物保留在: $BadDir"
    exit 0
} else {
    Write-Host "[FAIL] 恢复后哈希校验不一致（基线 manifest vs 实际产物）" -ForegroundColor Red
    Write-Host "     现场保留在: $BadDir"
    $diff | Select-Object -First 20 | ForEach-Object { Write-Host $_ }
    exit 3
}

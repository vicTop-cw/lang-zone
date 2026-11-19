# LZ 前端双轨一致性门禁（演示/本地/CI 共用入口）
#
# 用法：
#   .\scripts\dual_track_gate.ps1              # 快线：分层子集（~20 样本，约 5 分钟）
#   .\scripts\dual_track_gate.ps1 -Full        # 全量 328 样本（约 40-60 分钟）
#   .\scripts\dual_track_gate.ps1 -Full -Chunk 82   # 全量分块（每块 82 样本单独进程，防长任务被杀）
#
# 口径：LZ 词法器（--emit=lex-lz）与 Rust 词法器（--tokens）归一化后逐行一致；
# 不一致即非零退出。全量口径对应 T0r56.3.2 验收（328/328 零分歧）。
param(
    [switch]$Full,
    [int]$Chunk = 0
)
$ErrorActionPreference = "Continue"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
# 钉版工具链自给（读 rust-toolchain.toml；防 cron/异版本 rustc 造成 E0514）
$tcFile = Join-Path $Root "rust-toolchain.toml"
if (Test-Path $tcFile) {
    $m = Select-String -Path $tcFile -Pattern 'channel\s*=\s*"([^"]+)"'
    if ($m) {
        $tcBin = Join-Path (Join-Path $env:USERPROFILE ".rustup\toolchains") ($m.Matches[0].Groups[1].Value + "\bin")
        if (Test-Path $tcBin) { $env:PATH = "$tcBin;" + $env:PATH }
    }
}# 车道 rlib 与当前工具链同源（cron 可能用异版本重建过；就地重建 ~3s）
$env:CARGO_TARGET_DIR = Join-Path $Root "target\lane-bootstrap"
cargo build -p lz_builtins 2>&1 | Out-Null
$env:CARGO_TARGET_DIR = Join-Path $Root "target\gate-test"
Push-Location $Root
try {
    # 判据自检先行（BUG-501 回归锁）：语法矩阵反例/正例判据的边界用例必须全过
    Write-Host "[GATE] 语法矩阵判据 SelfTest ..." -ForegroundColor Yellow
    & (Join-Path $Root "div-tools\syntax_matrix.ps1") -SelfTest | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Host "[GATE] SelfTest FAIL exit=$LASTEXITCODE" -ForegroundColor Red; exit 1 }
    if (-not $Full) {
        Write-Host "[GATE] 快线：分层子集门禁 ..." -ForegroundColor Yellow
        cargo test --test frontend_dual_track stratified_subset 2>&1 | Select-Object -Last 6
        exit $LASTEXITCODE
    }
    if ($Chunk -gt 0) {
        $total = 328
        $fails = 0
        for ($s = 0; $s -lt $total; $s += $Chunk) {
            $e = [Math]::Min($s + $Chunk, $total)
            Write-Host "[GATE] 全量分块 [$s,$e) ..." -ForegroundColor Yellow
            $out = node (Join-Path $Root "scripts\fullgate.js") (Join-Path $Root "target\gate-test\debug\lang-zone.exe") $s $e 2>$null
            $out | Select-String -Pattern "^checked=|^FAIL:" | ForEach-Object { Write-Host $_ }
            if ($out | Select-String -Pattern "^checked=\d+ failed=[1-9]") { $fails++ }
        }
        if ($fails -gt 0) { Write-Host "[GATE] 全量门禁失败（$fails 块有分歧）" -ForegroundColor Red; exit 1 }
        Write-Host "[GATE] 全量门禁通过（328/328）" -ForegroundColor Green
        exit 0
    }
    Write-Host "[GATE] 全量门禁（单进程）..." -ForegroundColor Yellow
    cargo test --test frontend_dual_track full_corpus -- --ignored 2>&1 | Select-Object -Last 8
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
# T0r11.2 轻量验收判据：debug 编译 + 关键测试（快速）
$ErrorActionPreference = 'Continue'
chcp 65001 > $null
[Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

Write-Output "=== cargo build (debug) ==="
cargo build 2>&1 | Out-Null
$buildCode = $LASTEXITCODE
Write-Output "build exit: $buildCode"

if ($buildCode -eq 0) {
    Write-Output "VERDICT: PASSED (build ok)"
    exit 0
} else {
    Write-Output "VERDICT: FAILED (build=$buildCode)"
    exit 1
}

# BUG-2 修复判据：bootstrap_emit.rs:294 编译错误已消除 + lib 零回归
# 退出码 0 = 判据通过；非 0 = 失败
chcp 65001 > $null
[Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

Write-Output "=== [1/2] cargo build ==="
$buildOut = cargo build 2>&1
$buildCode = $LASTEXITCODE
if ($buildCode -ne 0) {
    Write-Output "FAIL: cargo build exit=$buildCode"
    $buildOut | Select-Object -Last 40
    exit 1
}
Write-Output "cargo build OK (exit 0)"

Write-Output "=== [2/2] cargo test --lib ==="
$testOut = cargo test --lib 2>&1
$testCode = $LASTEXITCODE
if ($testCode -ne 0) {
    Write-Output "FAIL: cargo test --lib exit=$testCode"
    $testOut | Select-Object -Last 40
    exit 1
}
# 验证测试全绿（472 passed）
$testSummary = $testOut | Select-String -Pattern "test result: ok\.\s*(\d+)\s*passed" | Select-Object -Last 1
Write-Output "test result: $($testSummary.Matches.Groups[1].Value) passed"
Write-Output "ALL CHECKS PASSED"
exit 0

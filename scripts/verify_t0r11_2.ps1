# T0r11.2 验收判据：编译 + embed 序列化测试
$ErrorActionPreference = 'Continue'
chcp 65001 > $null
[Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

Write-Output "=== cargo build --release ==="
cargo build --release 2>&1 | Out-Null
$buildCode = $LASTEXITCODE
Write-Output "build exit: $buildCode"

Write-Output "=== cargo test embed_dict_serialization + embed_tuple_serialization ==="
cargo test --test embed_dict_serialization --test embed_tuple_serialization 2>&1 | Out-Null
$testCode = $LASTEXITCODE
Write-Output "test exit: $testCode"

if ($buildCode -eq 0 -and $testCode -eq 0) {
    Write-Output "VERDICT: PASSED"
    exit 0
} else {
    Write-Output "VERDICT: FAILED (build=$buildCode test=$testCode)"
    exit 1
}

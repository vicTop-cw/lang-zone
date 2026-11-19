# 迭代-18:00 判据脚本：编译 warning 清零门禁
# 判据：cargo build --bin lang-zone 产出 0 warning（旧口径 17）。
# 用法：powershell -NoProfile -File scripts/iter18_warning_gate.ps1
$ErrorActionPreference = "Continue"
$out = cargo build --bin lang-zone 2>&1 | Out-String
$warnings = ($out -split "`n" | Where-Object { $_ -match '^warning' }).Count
$errors = ($out -split "`n" | Where-Object { $_ -match '^error' }).Count
Write-Output "warnings=$warnings errors=$errors"
if ($warnings -eq 0 -and $errors -eq 0) { exit 0 } else { exit 1 }

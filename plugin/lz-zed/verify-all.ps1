# lz-zed one-shot verification script (refresh snapshots + full check)
# Usage:  powershell -ExecutionPolicy Bypass -File verify-all.ps1
#
#   1. Refreshes highlight snapshots (required after tmLanguage/grammar changes)
#   2. Verifies snapshots + 193-element three-way coverage
#   3. Exit code 0 = all green, then hints next steps (build / Zed / preview)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

Write-Host "`n=== [1/2] refresh snapshots (--update) ===" -ForegroundColor Cyan
node test\run-tests.js --update
if ($LASTEXITCODE -ne 0) { Write-Host "snapshot refresh FAILED" -ForegroundColor Red; exit 1 }

Write-Host "`n=== [2/2] full verification ===" -ForegroundColor Cyan
node test\run-tests.js
if ($LASTEXITCODE -ne 0) { Write-Host "verification FAILED, see report above" -ForegroundColor Red; exit 1 }

Write-Host "`n=== ALL GREEN ===" -ForegroundColor Green
Write-Host "Next steps (optional):"
Write-Host "  1) Build grammar:  powershell -ExecutionPolicy Bypass -File build.ps1"
Write-Host "  2) Load in Zed:    Ctrl+Shift+P -> zed: install dev extension -> select this folder"
Write-Host "  3) Browser preview: open showcase.html"

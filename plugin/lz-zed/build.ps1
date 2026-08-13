# lz-zed build script (Windows / PowerShell)
# Output: grammars/lz.wasm (tree-sitter WASM grammar for the Zed extension)
#
# Prereqs:
#   1. Node.js >= 18 (for npx tree-sitter)
#   2. tree-sitter CLI (auto-fetched via npx, or `npm i -g tree-sitter-cli`)
#   3. Emscripten if `tree-sitter build --wasm` needs it
#      (`npm i -g @emscripten/emscripten` or https://emscripten.org)
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File build.ps1
#
# NOTE: This script uses ASCII-only messages on purpose (PowerShell 5.1 on
# Windows decodes UTF-8 scripts with the system ANSI codepage, which garbles
# non-ASCII text).

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

Write-Host "==> [1/3] tree-sitter generate"
Push-Location grammar
npx --yes tree-sitter generate
if ($LASTEXITCODE -ne 0) { throw "tree-sitter generate failed" }
Pop-Location

Write-Host "==> [2/3] tree-sitter build --wasm -> grammars/lz.wasm"
New-Item -ItemType Directory -Force -Path grammars | Out-Null
Push-Location grammar
try {
    npx --yes tree-sitter build --wasm -o ..\grammars\lz.wasm
} catch {
    Write-Warning "tree-sitter build --wasm failed, trying legacy build-wasm ..."
    npx --yes tree-sitter build-wasm . -o ..\grammars\lz.wasm
}
Pop-Location

if (-not (Test-Path grammars\lz.wasm)) {
    throw "grammars/lz.wasm was not generated; check tree-sitter CLI and Emscripten"
}

Write-Host "==> [3/3] running test suite (snapshot + coverage)"
node test\run-tests.js
if ($LASTEXITCODE -ne 0) { throw "tests failed" }

Write-Host ""
Write-Host "Build OK: grammars\lz.wasm"
Write-Host "Next: in Zed run 'zed: install dev extension' and select this folder (lz-zed)"

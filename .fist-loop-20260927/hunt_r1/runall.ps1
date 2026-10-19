# lzc-loop 寻虫批量探针 harness：.lz -> lang-zone -> rustc -> run
param([string]$Dir = "probes")
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$work = Join-Path $root $Dir
$exe = "E:\IDEProjects\AI\lang-zone\target\debug\lang-zone.exe"
$rlib = "E:\IDEProjects\AI\lang-zone\target\debug\liblz_builtins.rlib"
$dep  = "E:\IDEProjects\AI\lang-zone\target\debug\deps"
Get-ChildItem $work -Filter *.lz | Sort-Object Name | ForEach-Object {
  $n = $_.BaseName
  $lz = $_.FullName
  $out = & $exe $lz 2>&1
  if ($LASTEXITCODE -ne 0) { "[$n] LZ-FAIL: " + (($out | Out-String).Trim() -replace "`r?`n"," | "); return }
  $rs = Join-Path $work "$n.rs"
  if (-not (Test-Path $rs)) { "[$n] LZ-FAIL: no .rs emitted"; return }
  $o = Join-Path $work "$n.exe"
  $rc = & rustc --edition 2021 $rs -L "dependency=$dep" --extern "lz_builtins=$rlib" -o $o 2>&1
  if ($LASTEXITCODE -ne 0) {
    $errs = ($rc | Out-String)
    $first = ($errs -split "`r?`n" | Where-Object { $_ -match '^\s*error' } | Select-Object -First 3) -join " ;; "
    "[$n] RUSTC-FAIL: $first"
    return
  }
  $rn = & $o 2>&1
  if ($LASTEXITCODE -ne 0) { "[$n] RUN-FAIL: " + (($rn | Out-String).Trim() -replace "`r?`n"," | "); return }
  "[$n] OK: " + (($rn | Out-String).Trim() -replace "`r?`n"," | ")
}

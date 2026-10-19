# 门禁2：DEMO 全量回归（排除 99_errors 与 lz_std；lzcyz/CY 域不在范围内）
$ErrorActionPreference = 'Continue'
$exe  = "e:\IDEProjects\AI\lang-zone\target\debug\lang-zone.exe"
$rlib = "e:\IDEProjects\AI\lang-zone\target\debug\liblz_builtins.rlib"
$dep  = "e:\IDEProjects\AI\lang-zone\target\debug\deps"
$outDir = "e:\IDEProjects\AI\lang-zone\.fist-loop-20260927\demo_r1_out"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$files = Get-ChildItem "e:\IDEProjects\AI\lang-zone\DEMO" -Recurse -Filter *.lz |
         Where-Object { $_.FullName -notmatch '\\99_errors\\' -and $_.FullName -notmatch '\\lz_std\\' }
$ok = 0; $lzFail = @(); $rcFail = @(); $runFail = @()
$sw = [Diagnostics.Stopwatch]::StartNew()
foreach ($f in $files) {
  $n = $f.BaseName + "_" + ($f.DirectoryName -replace '.*\\DEMO\\','' -replace '\\','_')
  $o = (& $exe $f.FullName 2>&1 | Out-String)
  if ($LASTEXITCODE -ne 0) { $lzFail += "$($f.FullName) :: " + (($o -replace "`r?`n",' ').Substring(0, [Math]::Min(180, $o.Length))); continue }
  $rs = [IO.Path]::ChangeExtension($f.FullName, ".rs")
  if (-not (Test-Path $rs)) { $lzFail += "$($f.FullName) :: no .rs emitted"; continue }
  $bin = Join-Path $outDir "$n.exe"
  $rc = (& rustc --edition 2021 $rs -L "dependency=$dep" --extern "lz_builtins=$rlib" -o $bin 2>&1 | Out-String)
  if ($LASTEXITCODE -ne 0) {
    $e = (($rc -split "`r?`n") | Where-Object { $_ -match '^\s*error' } | Select-Object -First 2) -join ' ;; '
    $rcFail += "$($f.FullName) :: " + ($e -replace "\u001b\[[0-9;]*m",'')
    continue
  }
  $rn = (& $bin 2>&1 | Out-String)
  if ($LASTEXITCODE -ne 0) { $runFail += "$($f.FullName) :: " + (($rn -replace "`r?`n",' ').Substring(0,[Math]::Min(160,$rn.Length))); continue }
  $ok++
}
"=== DEMO REGRESSION R1 ==="
"total=$($files.Count)  ok=$ok  lzFail=$($lzFail.Count)  rustcFail=$($rcFail.Count)  runFail=$($runFail.Count)  elapsed=$($sw.Elapsed)"
"--- LZ-FAIL ---";       $lzFail  | ForEach-Object { $_ }
"--- RUSTC-FAIL ---";    $rcFail  | ForEach-Object { $_ }
"--- RUN-FAIL ---";      $runFail | ForEach-Object { $_ }

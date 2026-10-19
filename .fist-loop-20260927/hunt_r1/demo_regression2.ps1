# 门禁2：DEMO 全量回归 v2（修正 exe 命名 + 运行超时）
$ErrorActionPreference = 'Continue'
$exe  = "e:\IDEProjects\AI\lang-zone\target\debug\lang-zone.exe"
$rlib = "e:\IDEProjects\AI\lang-zone\target\debug\liblz_builtins.rlib"
$dep  = "e:\IDEProjects\AI\lang-zone\target\debug\deps"
$outDir = "e:\IDEProjects\AI\lang-zone\.fist-loop-20260927\demo_r1_out"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$files = @(Get-ChildItem "e:\IDEProjects\AI\lang-zone\DEMO" -Recurse -Filter *.lz |
         Where-Object { $_.FullName -notmatch '\\99_errors\\' -and $_.FullName -notmatch '\\lz_std\\' })
$ok = 0; $lzFail = @(); $rcFail = @(); $runFail = @(); $to = @()
$sw = [Diagnostics.Stopwatch]::StartNew()
$i = 0
foreach ($f in $files) {
  $i++
  $bin = Join-Path $outDir ("d{0}.exe" -f $i)
  $o = (& $exe $f.FullName 2>&1 | Out-String)
  if ($LASTEXITCODE -ne 0) { $lzFail += "$($f.FullName) :: " + (($o -replace "`r?`n",' ').Substring(0, [Math]::Min(160, $o.Length))); continue }
  $rs = [IO.Path]::ChangeExtension($f.FullName, ".rs")
  if (-not (Test-Path $rs)) { $lzFail += "$($f.FullName) :: no .rs emitted"; continue }
  $rc = (& rustc --edition 2021 $rs -L "dependency=$dep" --extern "lz_builtins=$rlib" -o $bin 2>&1 | Out-String)
  if ($LASTEXITCODE -ne 0) {
    $e = (($rc -split "`r?`n") | Where-Object { $_ -match '^\s*error' } | Select-Object -First 2) -join ' ;; '
    $rcFail += "$($f.FullName) :: " + ($e -replace "\u001b\[[0-9;]*m",'')
    continue
  }
  $p = Start-Process -FilePath $bin -PassThru -WindowStyle Hidden -RedirectStandardOutput (Join-Path $outDir ("d{0}.out" -f $i)) -RedirectStandardError (Join-Path $outDir ("d{0}.err" -f $i))
  $done = $p.WaitForExit(20000)
  if (-not $done) { $p.Kill(); $to += $f.FullName; continue }
  $p.Refresh()
  $code = $p.ExitCode
  if ($null -eq $code) { $code = 0 }
  if ($code -ne 0) { $runFail += "$($f.FullName) :: exit=$code"; continue }
  $ok++
}
"=== DEMO REGRESSION R1 (v2) ==="
"total=$($files.Count)  ok=$ok  lzFail=$($lzFail.Count)  rustcFail=$($rcFail.Count)  runFail=$($runFail.Count)  timeout=$($to.Count)  elapsed=$($sw.Elapsed)"
"--- LZ-FAIL ---";    $lzFail  | ForEach-Object { $_ }
"--- RUSTC-FAIL ---"; $rcFail  | ForEach-Object { $_ }
"--- RUN-FAIL ---";   $runFail | ForEach-Object { $_ }
"--- TIMEOUT ---";    $to      | ForEach-Object { $_ }

$ErrorActionPreference = 'Continue'
$bin = 'target/debug/lang-zone.exe'
$ok = 0; $fail = 0
$files = Get-ChildItem -Path 'DEMO' -Recurse -Filter *.lz
foreach ($f in $files) {
  $rel = $f.FullName.Replace((Get-Location).Path + '\', '').Replace('\', '/')
  $out = & $bin $f.FullName --emit=ir 2>&1
  if ($LASTEXITCODE -eq 0) { $ok++ } else { $fail++; Write-Output "FAIL: $rel" }
}
$total = $ok + $fail
Write-Output "TOTAL=$total OK=$ok FAIL=$fail"

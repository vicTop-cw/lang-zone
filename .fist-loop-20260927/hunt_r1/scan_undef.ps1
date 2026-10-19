# 用带「未定义函数」检查的编译器扫描 DEMO/corpus，收集被误判的名字
$exe = "E:\IDEProjects\AI\lang-zone\target\debug\lang-zone.exe"
$roots = @("E:\IDEProjects\AI\lang-zone\DEMO", "E:\IDEProjects\AI\lang-zone\corpus")
$names = @{}
foreach ($r in $roots) {
  if (-not (Test-Path $r)) { continue }
  Get-ChildItem $r -Recurse -Filter *.lz | Where-Object { $_.FullName -notmatch '\\99_errors\\' -and $_.FullName -notmatch '\\lz_std\\' } | ForEach-Object {
    $out = & $exe $_.FullName 2>&1 | Out-String
    foreach ($m in [regex]::Matches($out, "未定义函数: (\S+)")) {
      $n = $m.Groups[1].Value
      if ($names.ContainsKey($n)) { $names[$n]++ } else { $names[$n] = 1 }
    }
  }
}
$names.GetEnumerator() | Sort-Object -Property Name | ForEach-Object { "$($_.Key)`t$($_.Value)" }

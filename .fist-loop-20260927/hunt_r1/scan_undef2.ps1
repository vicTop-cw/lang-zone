$ErrorActionPreference = 'Continue'
$exe = "e:\IDEProjects\AI\lang-zone\target\debug\lang-zone.exe"
$files = @()
$files += Get-ChildItem "e:\IDEProjects\AI\lang-zone\DEMO" -Recurse -Filter *.lz -ErrorAction SilentlyContinue
$files += Get-ChildItem "e:\IDEProjects\AI\lang-zone\corpus" -Recurse -Filter *.lz -ErrorAction SilentlyContinue
$files = $files | Where-Object { $_.FullName -notmatch '\\99_errors\\' -and $_.FullName -notmatch '\\lz_std\\' }
"files: " + $files.Count
$names = @{}
foreach ($f in $files) {
  $o = (& $exe $f.FullName 2>&1 | Out-String)
  foreach ($m in [regex]::Matches($o, "未定义函数: ([A-Za-z_][A-Za-z0-9_]*)")) {
    $n = $m.Groups[1].Value
    if ($names.ContainsKey($n)) { $names[$n] = $names[$n] + 1 } else { $names[$n] = 1 }
  }
}
$names.GetEnumerator() | Sort-Object -Property Name | ForEach-Object { "$($_.Key) $($_.Value)" }
